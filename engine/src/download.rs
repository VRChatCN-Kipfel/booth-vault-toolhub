//! 文件下载：流式 + `.part` 原子落盘 + Range 分块续传 + 假文件校验 + 限速。
//!
//! 双路径下载：
//!
//!   1. 快路径：单次流式 GET（小文件实测有效）
//!   2. 兜底：分块 Range 下载（绕过代理切大流）
//!
//! 每次块请求都重新签发原始 URL（BOOTH 签名 S3 有时限，重解析保有效）。

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use reqwest::header::RANGE;

use crate::cover::looks_html;
use crate::http::{MAX_RETRIES, get};

/// 流式块大小。
const CHUNK: usize = 1 << 16;
/// Range 分块大小。
const RANGE_CHUNK: u64 = 64 * 1024;
/// Range 每块最大重试次数。
const RANGE_MAX_RETRY: u32 = 6;

/// 下载行为选项。
///
/// `keep_failed` 为 `false`（默认）时，任何失败都会清理临时文件并上报——
/// 从「既不清理也不上报」（`.part` 只在假 HTML 分支被删，其余失败静默残留）
/// 变为「清理且上报」。置 `true` 才保留 `.part` 供取证。
#[derive(Debug, Clone, Copy)]
pub struct DownloadOptions {
    /// 校验目标非登录页伪装（未登录时 BOOTH 返回伪装成文件的 HTML）。
    pub check_html: bool,
    /// 每文件间限速秒数（三端统一，默认 0.8）。
    pub rate_limit: f64,
    /// 失败时保留 `.part` 供取证；默认清理。
    pub keep_failed: bool,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            check_html: true,
            rate_limit: 0.8,
            keep_failed: false,
        }
    }
}

/// 下载 `url` 到 `dest`。下载过程写入 `{dest}.part`，成功后原子 rename。
///
/// 传输失败（状态码 / 读取 / 写入）与内容校验失败（HTML 伪装 / 损坏包）走
/// **同一个** `keep_failed` 开关，不做「有的留有的删」。
///
/// 注意：保留的 `.part` **不产生续传能力**——两条路径都是 `File::create` 从头写，
/// 重试即截断重来。保留的价值是取证（判断是登录页还是下到一半的包），
/// 且全仓无自动回收，长期开启会持续占用空间。
pub fn download(
    client: &Client,
    url: &str,
    dest: &Path,
    opts: DownloadOptions,
) -> Result<(), String> {
    let tmp = part_path(dest);
    let keep = opts.keep_failed;
    // 1) 快路径：单次流式 GET。
    let mut last_err: Option<String> = None;
    for attempt in 1..=MAX_RETRIES {
        let headers = crate::session::default_headers();
        match get(client, url, headers) {
            Ok(mut r) => {
                if !r.status().is_success() {
                    last_err = Some(format!("status {}", r.status()));
                    break;
                }
                if let Err(e) = write_streamed(&mut r, &tmp) {
                    last_err = Some(e);
                    if attempt < MAX_RETRIES {
                        std::thread::sleep(Duration::from_secs(attempt as u64 * 2));
                        continue;
                    }
                } else {
                    last_err = None;
                    break;
                }
            }
            Err(e) => {
                last_err = Some(e.to_string());
                if attempt < MAX_RETRIES {
                    std::thread::sleep(Duration::from_secs(attempt as u64 * 2));
                }
            }
        }
    }
    // 2) 兜底：分块 Range 下载（仅当快路径失败）。
    if last_err.is_some()
        && let Err(e) = ranged_download(client, url, &tmp)
    {
        return Err(fail(&tmp, keep, format!("ranged fallback failed: {e}")));
    }
    // 3) 假文件校验。
    if opts.check_html
        && let Ok(bytes) = std::fs::read(&tmp)
        && looks_html(&bytes)
    {
        return Err(fail(&tmp, keep, cookie_required_msg()));
    }
    // 4) 结构校验：传输成功不等于内容完整（半截包魔数正常、长度非 0，HTML 检查过不了它）。
    //    不在此拦下的话坏包会静默落地，直到下一轮 `is_locally_valid` 才发现要重下，
    //    届时 `.part` 早已不存在，`keep_failed` 也够不着——开关会名不副实。
    //    只对可解析格式生效；rar/7z 判为无法判定，不拦（宁可漏报，不可误报）。
    if crate::integrity::package_health_of(&tmp, dest) == crate::integrity::PackageHealth::Corrupt {
        return Err(fail(
            &tmp,
            keep,
            format!("下载内容损坏，未落盘：{}", dest.display()),
        ));
    }
    // 5) 原子落盘。
    if let Err(e) = std::fs::rename(&tmp, dest) {
        return Err(fail(&tmp, keep, format!("rename {tmp:?} -> {dest:?}: {e}")));
    }
    if opts.rate_limit > 0.0 {
        std::thread::sleep(Duration::from_secs_f64(opts.rate_limit));
    }
    Ok(())
}

/// 失败收尾：按开关决定保留 `.part` 还是清理，并在保留时上报绝对路径。
fn fail(tmp: &Path, keep: bool, msg: impl Into<String>) -> String {
    let msg = msg.into();
    if keep && tmp.exists() {
        let abs = std::fs::canonicalize(tmp).unwrap_or_else(|_| tmp.to_path_buf());
        format!("{msg} — 失败临时文件已保留：{}", abs.display())
    } else {
        let _ = std::fs::remove_file(tmp);
        msg
    }
}

/// `.part` 路径。
pub fn part_path(dest: &Path) -> std::path::PathBuf {
    let mut s = dest.as_os_str().to_os_string();
    s.push(".part");
    std::path::PathBuf::from(s)
}

/// 流式写入（小文件快路径）。reqwest blocking Response 实现 `std::io::Read`。
fn write_streamed(r: &mut Response, tmp: &Path) -> Result<(), String> {
    let mut fh = std::fs::File::create(tmp).map_err(|e| format!("create {tmp:?}: {e}"))?;
    let mut buf = [0u8; CHUNK];
    loop {
        let n = r.read(&mut buf).map_err(|e| format!("stream read: {e}"))?;
        if n == 0 {
            break;
        }
        fh.write_all(&buf[..n]).map_err(|e| format!("write: {e}"))?;
    }
    fh.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

/// 分块 Range 下载（绕过代理切大流）。
///
/// 每次请求重新签发原始 URL，块小、连接短，绕开单响应被代理截断的问题。
fn ranged_download(client: &Client, url: &str, tmp: &Path) -> Result<(), String> {
    // 探针：Range: bytes=0-0 拿 Content-Range 得总大小。
    let mut headers = crate::session::default_headers();
    headers.insert(RANGE, "bytes=0-0".parse().unwrap());
    let r = get(client, url, headers).map_err(|e| format!("range probe: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("range probe status {}", r.status()));
    }
    let cr = r
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let total = if let Some((_, rest)) = cr.rsplit_once('/') {
        rest.trim()
            .parse::<u64>()
            .map_err(|_| format!("bad Content-Range {cr:?}"))?
    } else {
        return Err(format!(
            "server does not support Range (no Content-Range): {cr:?}"
        ));
    };
    if total == 0 {
        std::fs::File::create(tmp).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let mut fh = std::fs::File::create(tmp).map_err(|e| format!("create {tmp:?}: {e}"))?;
    let mut done: u64 = 0;
    let mut buf = [0u8; CHUNK];
    while done < total {
        let end = (done + RANGE_CHUNK - 1).min(total - 1);
        let mut ok = false;
        for _att in 1..=RANGE_MAX_RETRY {
            let mut h = crate::session::default_headers();
            h.insert(RANGE, format!("bytes={done}-{end}").parse().unwrap());
            match get(client, url, h) {
                Ok(mut r) => {
                    if !r.status().is_success() {
                        return Err(format!("chunk {done}-{end} status {}", r.status()));
                    }
                    loop {
                        let n = r.read(&mut buf).map_err(|e| format!("chunk read: {e}"))?;
                        if n == 0 {
                            break;
                        }
                        fh.write_all(&buf[..n]).map_err(|e| format!("write: {e}"))?;
                    }
                    ok = true;
                    break;
                }
                Err(e) => {
                    if _att < RANGE_MAX_RETRY {
                        std::thread::sleep(Duration::from_secs(1));
                    } else {
                        return Err(format!("chunk {done}-{end} failed: {e}"));
                    }
                }
            }
        }
        if !ok {
            return Err(format!(
                "chunk {done}-{end} failed after {RANGE_MAX_RETRY} retries"
            ));
        }
        done = end + 1;
    }
    fh.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

/// 未登录/假文件时的统一提示：GUI 指设置页，CLI/MCP 用 --cookie。
pub fn cookie_required_msg() -> &'static str {
    "got BOOTH login page instead of file — 请到设置页填写 Cookie（CLI/MCP 用 --cookie）"
}

/// 错误串是否像未登录（假文件 / 缺 Cookie）。
pub fn looks_like_cookie_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("login page")
        || m.contains("supply --cookie")
        || m.contains("设置页填写 cookie")
        || m.contains("disguised html")
}

/// 下载/补全失败时补上设置页 Cookie 指向（已含则原样返回）。
pub fn with_cookie_hint(err: impl std::fmt::Display) -> String {
    let s = err.to_string();
    if looks_like_cookie_error(&s) && !s.contains("设置页") {
        format!("{s} — 请到设置页填写 Cookie（CLI/MCP 用 --cookie）")
    } else {
        s
    }
}

/// 限速接口（三端统一，M5 MCP 不得绕过）。
pub fn sleep_rate_limit(rate_limit: f64) {
    if rate_limit > 0.0 {
        std::thread::sleep(Duration::from_secs_f64(rate_limit));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "bvt_download_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn client() -> Client {
        crate::session::make_session(&crate::config::AppConfig::default(), None)
    }

    fn opts(keep_failed: bool) -> DownloadOptions {
        DownloadOptions {
            check_html: true,
            rate_limit: 0.0,
            keep_failed,
        }
    }

    /// 本地 HTTP 服务，若干次回固定字节。用回环而非外网，不依赖网络可用性。
    fn serve_bytes(times: usize, body: Vec<u8>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for _ in 0..times {
                let Ok((mut s, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let mut resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                resp.extend_from_slice(&body);
                let _ = s.write_all(&resp);
            }
        });
        format!("http://127.0.0.1:{port}/x.zip")
    }

    /// 模拟 BOOTH 未登录时返回的登录页伪装。
    fn serve_html(times: usize) -> String {
        serve_bytes(
            times,
            b"<!doctype html><html><body>login</body></html>".to_vec(),
        )
    }

    fn zip_bytes() -> Vec<u8> {
        use std::io::Write;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            w.start_file("a.txt", o).unwrap();
            w.write_all(b"hello").unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    /// 传输成功但结构损坏：必须在落盘前拦下。否则坏包静默落地，直到下一轮
    /// `is_locally_valid` 才发现要重下，而那时 `.part` 已不在，留痕开关够不着。
    #[test]
    fn corrupt_payload_is_rejected_before_rename() {
        let dir = tmpdir("corrupt");
        let dest = dir.join("x.zip");
        let full = zip_bytes();
        let url = serve_bytes(4, full[..full.len() - 1].to_vec());
        let err = download(&client(), &url, &dest, opts(false)).unwrap_err();
        assert!(err.contains("损坏"), "{err}");
        assert!(!dest.exists(), "损坏内容不得落盘");
        assert!(!part_path(&dest).exists(), "默认应清理临时文件");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 对照组：完整包正常落盘且不留 `.part`。
    #[test]
    fn intact_payload_lands() {
        let dir = tmpdir("intact");
        let dest = dir.join("y.zip");
        let url = serve_bytes(4, zip_bytes());
        download(&client(), &url, &dest, opts(false)).unwrap();
        assert!(dest.exists());
        assert!(!part_path(&dest).exists(), "落盘后不应留 .part");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn part_path_suffix() {
        let p = part_path(Path::new("C:\\x\\file.zip"));
        assert_eq!(p, Path::new("C:\\x\\file.zip.part"));
    }

    /// 内容校验失败 × 默认：临时文件被清理（原先只在假 HTML 分支删，其余静默残留）。
    #[test]
    fn content_failure_cleans_part_by_default() {
        let dir = tmpdir("clean");
        let dest = dir.join("x.zip");
        let url = serve_html(4);
        let err = download(&client(), &url, &dest, opts(false)).unwrap_err();
        assert!(!part_path(&dest).exists(), "默认应清理临时文件");
        assert!(!err.contains("已保留"), "{err}");
        assert_eq!(err, cookie_required_msg());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 内容校验失败 × 开启：临时文件保留，错误串带绝对路径。
    #[test]
    fn content_failure_keeps_part_with_abs_path_when_enabled() {
        let dir = tmpdir("keep");
        let dest = dir.join("x.zip");
        let url = serve_html(4);
        let err = download(&client(), &url, &dest, opts(true)).unwrap_err();
        let part = part_path(&dest);
        assert!(part.exists(), "开启后应保留临时文件");
        assert!(err.contains("已保留"), "{err}");
        let abs = std::fs::canonicalize(&part).unwrap();
        assert!(err.contains(&abs.to_string_lossy().to_string()), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 失败收尾的两态语义（纯函数，不涉网络）。
    /// 传输失败走的就是这条路径——它不建 `.part`，故开启保留也不得谎报「已保留」。
    #[test]
    fn fail_cleans_or_keeps_with_abs_path() {
        let dir = tmpdir("fail");
        let tmp = dir.join("z.zip.part");
        std::fs::write(&tmp, b"half").unwrap();
        assert_eq!(fail(&tmp, false, "boom"), "boom");
        assert!(!tmp.exists(), "默认应清理");

        std::fs::write(&tmp, b"half").unwrap();
        let msg = fail(&tmp, true, "boom");
        assert!(tmp.exists(), "开启应保留");
        assert!(msg.starts_with("boom"), "{msg}");
        let abs = std::fs::canonicalize(&tmp).unwrap();
        assert!(msg.contains(&abs.to_string_lossy().to_string()), "{msg}");

        let _ = std::fs::remove_file(&tmp);
        assert_eq!(fail(&tmp, true, "boom"), "boom", "无临时文件不得谎报");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn looks_html_shared() {
        assert!(looks_html(b"<!doctype html>"));
        assert!(!looks_html(b"PK\x03\x04"));
    }

    #[test]
    fn cookie_hint_points_to_settings() {
        assert!(cookie_required_msg().contains("设置页填写 Cookie"));
        assert!(looks_like_cookie_error(cookie_required_msg()));
        assert!(looks_like_cookie_error(
            "got BOOTH login page instead of file — supply --cookie"
        ));
        let hinted = with_cookie_hint("got BOOTH login page instead of file — supply --cookie");
        assert!(hinted.contains("设置页填写 Cookie"));
        assert_eq!(
            with_cookie_hint(cookie_required_msg()),
            cookie_required_msg()
        );
    }
}
