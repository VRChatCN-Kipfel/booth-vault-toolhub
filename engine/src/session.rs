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
///
/// **必须逐个 `add_cookie_str`**：`Jar::add_cookie_str` 解析的是单个
/// `Set-Cookie` 头，`; ` 之后的片段会被当作该 cookie 的属性处理，未知属性名
/// 静默忽略。整串一次性传入时**只有第一个 cookie 能进 jar**——
/// 而从浏览器复制的串首个往往是 `_ga` 这类与登录无关的统计 cookie，
/// 真正的会话 `_plaza_session_nktz7u` 会连同 `cf_clearance` 一起被丢掉，
/// 症状是「明明填了 Cookie，却提示请到设置页填写 Cookie」。
fn add_cookie_string(jar: &Jar, s: &str) {
    let url = "https://booth.pm/".parse().expect("booth.pm url");
    for pair in s.split(';') {
        let pair = pair.trim();
        if pair.is_empty() || pair.split_once('=').is_none() {
            continue;
        }
        jar.add_cookie_str(pair, &url);
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

    /// 回归：整串一次性 `add_cookie_str` 只会进第一个 cookie，其余被当作属性丢弃。
    /// 必须逐个注入，否则从浏览器复制来的串（首个常是 `_ga`）会丢掉会话 cookie，
    /// 表现为「填了 Cookie 却提示未登录」。
    #[test]
    fn cookie_string_injects_every_pair() {
        use reqwest::cookie::CookieStore;
        let raw = "_ga=GA1.1.111; _gcl_au=1.1.222; _plaza_session_nktz7u=SESSVAL; \
                   cf_clearance=CFVAL; recent_items=1%2C2";
        let jar = parse_cookie(raw);
        let url = reqwest::Url::parse("https://booth.pm/downloadables/1").unwrap();
        let sent = jar
            .cookies(&url)
            .map(|v| v.to_str().unwrap_or("").to_string())
            .unwrap_or_default();
        for expected in [
            "_ga=GA1.1.111",
            "_gcl_au=1.1.222",
            "_plaza_session_nktz7u=SESSVAL",
            "cf_clearance=CFVAL",
        ] {
            assert!(sent.contains(expected), "未发出 {expected}，实际：{sent}");
        }
    }

    /// 会话 cookie 不在首位时也必须送达（这正是线上翻车的形态）。
    #[test]
    fn session_cookie_survives_leading_noise() {
        use reqwest::cookie::CookieStore;
        let raw = "_ga=GA1.1.999; ga_expire_X=1; _plaza_session_nktz7u=REAL";
        let jar = parse_cookie(raw);
        let url = reqwest::Url::parse("https://booth.pm/downloadables/2").unwrap();
        let sent = jar
            .cookies(&url)
            .map(|v| v.to_str().unwrap_or("").to_string())
            .unwrap_or_default();
        assert!(
            sent.contains("_plaza_session_nktz7u=REAL"),
            "会话 cookie 被噪声挤掉，实际：{sent}"
        );
    }

    /// 空段、无 `=` 的残片不得破坏其余 cookie。
    #[test]
    fn malformed_segments_do_not_break_others() {
        use reqwest::cookie::CookieStore;
        let jar = parse_cookie("; ; junk; _plaza_session_nktz7u=OK; ;");
        let url = reqwest::Url::parse("https://booth.pm/").unwrap();
        let sent = jar
            .cookies(&url)
            .map(|v| v.to_str().unwrap_or("").to_string())
            .unwrap_or_default();
        assert!(sent.contains("_plaza_session_nktz7u=OK"), "实际：{sent}");
    }
}
