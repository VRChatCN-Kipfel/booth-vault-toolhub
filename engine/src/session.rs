//! 请求会话：reqwest blocking Client 构建。
//!
//! 默认 UA 头 + 代理注入 + cookie 三态加载。
//! 代理来自 `config::resolve_proxy`（配置 > 环境变量 > 系统默认），禁硬编码。

use std::path::Path;
use std::sync::Arc;

use reqwest::blocking::Client;
use reqwest::cookie::Jar;

use crate::config::{AppConfig, proxy_disabled, resolve_proxy};

/// 构建 blocking Client。
///
/// `cookie`: 'k=v; k2=v2' 串 / Netscape cookies.txt 路径 / 存原始 Cookie 串的文本文件路径。
pub fn make_session(config: &AppConfig, cookie: Option<&str>) -> Client {
    let mut builder = Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36",
        )
        .default_headers(default_headers());
    if proxy_disabled(config) {
        builder = builder.no_proxy();
    } else if let Some(proxy) = resolve_proxy(config)
        && let Ok(p) = reqwest::Proxy::all(&proxy)
    {
        builder = builder.proxy(p);
    }
    if let Some(c) = cookie.map(str::trim).filter(|s| !s.is_empty()) {
        let jar = Arc::new(parse_cookie(c));
        builder = builder.cookie_provider(jar);
    }
    builder.build().expect("reqwest client build")
}

/// 默认请求头（含 BOOTH 认可的浏览器指纹 UA）。
pub fn default_headers() -> reqwest::header::HeaderMap {
    use reqwest::header::{ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};
    let mut h = HeaderMap::new();
    h.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36",
        ),
    );
    h.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("ja,en;q=0.9,zh-CN;q=0.8"),
    );
    h
}

/// cookie 三态加载为 Jar：'k=v; k2=v2' 串 / Netscape cookies.txt 路径 / 文本文件内容。
fn parse_cookie(cookie_arg: &str) -> Jar {
    let jar = Jar::default();
    let p = Path::new(cookie_arg);
    if p.is_file()
        && let Ok(text) = std::fs::read_to_string(p)
    {
        // Netscape cookies.txt：含制表符且非注释行 → 逐行解析
        if text
            .lines()
            .any(|l| l.contains('\t') && !l.starts_with('#'))
        {
            for line in text.lines() {
                let line = line.trim_end();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 7 && parts[0].contains("booth") {
                    jar.add_cookie_str(
                        &format!("{}={}", parts[5], parts[6]),
                        &format!("https://{}/", parts[0])
                            .parse()
                            .expect("cookie domain"),
                    );
                }
            }
            return jar;
        }
        add_cookie_string(&jar, &text);
        return jar;
    }
    add_cookie_string(&jar, cookie_arg);
    jar
}

/// 'k=v; k2=v2' 串注入 `.booth.pm`。
fn add_cookie_string(jar: &Jar, s: &str) {
    let url = "https://booth.pm/".parse().expect("booth.pm url");
    let mut pairs: Vec<String> = Vec::new();
    for pair in s.split(';') {
        if let Some((k, v)) = pair.split_once('=') {
            let k = k.trim();
            let v = v.trim();
            if !k.is_empty() {
                pairs.push(format!("{k}={v}"));
            }
        }
    }
    if !pairs.is_empty() {
        jar.add_cookie_str(&pairs.join("; "), &url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_contain_ua() {
        let h = default_headers();
        assert!(h.contains_key(reqwest::header::USER_AGENT));
        assert!(h.contains_key(reqwest::header::ACCEPT_LANGUAGE));
    }

    #[test]
    fn session_builds_without_cookie() {
        let cfg = AppConfig::default();
        let client = make_session(&cfg, None);
        assert_eq!(
            client
                .get("https://booth.pm/")
                .build()
                .unwrap()
                .url()
                .as_str(),
            "https://booth.pm/"
        );
    }

    #[test]
    fn session_builds_with_cookie_string() {
        let cfg = AppConfig::default();
        let _client = make_session(&cfg, Some("_plaza_session_nktz7u=abc123; cf_clearance=xyz"));
    }
}
