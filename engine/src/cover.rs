//! 封面下载：Referer 头 + 假文件校验。

use std::path::Path;

use reqwest::blocking::Client;

use crate::http::get;

/// 封面文件名（下载到目录中的固定名）。
pub const COVER_FILENAME: &str = "cover.jpg";

/// 下载封面到 `dest_dir/cover.jpg`。失败返回 Err（调用方决定是否记录）。
pub fn download_cover(
    client: &Client,
    thumb_url: &str,
    dest_dir: &Path,
) -> Result<std::path::PathBuf, String> {
    if thumb_url.is_empty() {
        return Err("empty thumbnail url".to_string());
    }
    let mut headers = crate::session::default_headers();
    headers.insert(
        reqwest::header::REFERER,
        "https://booth.pm/"
            .parse()
            .map_err(|e: reqwest::header::InvalidHeaderValue| e.to_string())?,
    );
    let r = get(client, thumb_url, headers).map_err(|e| format!("cover request failed: {e}"))?;
    let status = r.status();
    if !status.is_success() {
        return Err(format!("cover status {status}"));
    }
    let bytes = r.bytes().map_err(|e| format!("cover read failed: {e}"))?;
    // 假文件校验：未登录时 BOOTH 可能返回伪装成图片的登录页 HTML。
    if looks_html(&bytes) {
        return Err("cover is a disguised HTML (login) page".to_string());
    }
    std::fs::create_dir_all(dest_dir).map_err(|e| format!("mkdir failed: {e}"))?;
    let cover = dest_dir.join(COVER_FILENAME);
    std::fs::write(&cover, &bytes).map_err(|e| format!("cover write failed: {e}"))?;
    Ok(cover)
}

/// 头 256 字节判断是否为 HTML 伪装（未登录响应体表面类型不可信）。
pub fn looks_html(head: &[u8]) -> bool {
    let s = head
        .iter()
        .copied()
        .take(256)
        .skip_while(|b| b.is_ascii_whitespace())
        .collect::<Vec<u8>>();
    let lower: Vec<u8> = s.iter().map(u8::to_ascii_lowercase).collect();
    lower.starts_with(b"<!doctype") || lower.starts_with(b"<html")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_html_detects_doctype() {
        let h = b"<!DOCTYPE html><html><head></head><body>login</body></html>";
        assert!(looks_html(h));
    }

    #[test]
    fn looks_html_detects_leading_whitespace_html() {
        let h = b"  \n  <HTML><body></body></html>";
        assert!(looks_html(h));
    }

    #[test]
    fn looks_html_accepts_binary() {
        let h = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        assert!(!looks_html(h));
    }

    #[test]
    fn looks_html_accepts_short_non_html() {
        assert!(!looks_html(b"PK\x03\x04"));
    }
}
