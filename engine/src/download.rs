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

/// 下载 `url` 到 `dest`。下载过程写入 `{dest}.part`，成功后原子 rename。
///
/// `check_html`：校验目标非登录页伪装（未登录时 BOOTH 返回伪装成文件的 HTML）。
/// `rate_limit`：每文件间限速秒数（三端统一，默认 0.8）。
pub fn download(
    client: &Client,
    url: &str,
    dest: &Path,
    check_html: bool,
    rate_limit: f64,
) -> Result<(), String> {
    let tmp = part_path(dest);
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
        return Err(format!("ranged fallback failed: {e}"));
    }
    // 3) 假文件校验。
    if check_html
        && let Ok(bytes) = std::fs::read(&tmp)
        && looks_html(&bytes)
    {
        let _ = std::fs::remove_file(&tmp);
        return Err(cookie_required_msg().to_string());
    }
    // 4) 原子落盘。
    std::fs::rename(&tmp, dest).map_err(|e| format!("rename {tmp:?} -> {dest:?}: {e}"))?;
    if rate_limit > 0.0 {
        std::thread::sleep(Duration::from_secs_f64(rate_limit));
    }
    Ok(())
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

    #[test]
    fn part_path_suffix() {
        let p = part_path(Path::new("C:\\x\\file.zip"));
        assert_eq!(p, Path::new("C:\\x\\file.zip.part"));
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
