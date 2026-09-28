//! 请求会话：reqwest blocking Client 构建。
//!
//! 默认 UA 头 + 代理注入 + cookie 三态加载 + 粘贴形态净化 + 可用性探针。
//! 代理来自 `config::resolve_proxy`（配置 > 环境变量 > 系统默认），禁硬编码。

use std::path::Path;
use std::sync::Arc;

use reqwest::blocking::Client;
use reqwest::cookie::Jar;
use serde::Serialize;

use crate::config::{AppConfig, proxy_disabled, resolve_proxy};

/// BOOTH 登录态所依赖的会话 cookie 名。
pub const SESSION_COOKIE: &str = "_plaza_session_nktz7u";

/// 浏览器指纹 UA（BOOTH 对非浏览器 UA 会降级响应，三处共用一份）。
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

/// 登录态探针：一个**不存在**的 downloadables ID。
///
/// 未登录时 BOOTH 会把任何 `downloadables/*` 302 到 `/users/sign_in`，
/// 已登录则直接 404（资源不存在）。用不存在的 ID 是刻意的——
/// **不触发任何真实下载**，零副作用、零带宽、不消耗额度，却能精确区分
/// 「cookie 有效」与「无效 / 缺失」。实测：无 cookie → 302 sign_in；
/// 有效 cookie → 404；缺会话项 → 302 sign_in。
const PROBE_URL: &str = "https://booth.pm/downloadables/1";

/// 分析类 cookie 前缀：与 BOOTH 服务端登录 / 放行逻辑无关，只用于统计上报。
/// 从浏览器复制的串里这类常占九成，剔除后请求头更小，也更不容易被 WAF 盯上。
/// `cf_clearance` / `__cf_bm` 刻意**不在此列**（它们影响 Cloudflare 放行）。
const ANALYTICS_PREFIXES: [&str; 8] = [
    "_ga",
    "_gid",
    "_gcl",
    "ga_expire_",
    "_fbp",
    "_uet",
    "_clck",
    "_clsk",
];

/// 构建 blocking Client。
///
/// `cookie`: 'k=v; k2=v2' 串 / Netscape cookies.txt 路径 / 存原始 Cookie 串的文本文件路径。
pub fn make_session(config: &AppConfig, cookie: Option<&str>) -> Client {
    build_client(config, cookie, true)
}

/// 同 [`make_session`]，但**不跟随重定向**（供登录态探针判定 302 目标）。
pub fn make_session_no_redirect(config: &AppConfig, cookie: Option<&str>) -> Client {
    build_client(config, cookie, false)
}

fn build_client(config: &AppConfig, cookie: Option<&str>, follow: bool) -> Client {
    let mut builder = Client::builder()
        .user_agent(UA)
        .default_headers(default_headers());
    if !follow {
        builder = builder.redirect(reqwest::redirect::Policy::none());
    }
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
    h.insert(USER_AGENT, HeaderValue::from_static(UA));
    h.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("ja,en;q=0.9,zh-CN;q=0.8"),
    );
    h
}

/// cookie 粘贴形态的净化结果。
#[derive(Debug, Clone, Default)]
pub struct SanitizedCookie {
    /// 净化后的 `k=v` 对（保序、同名去重保留后者）。
    pub pairs: Vec<(String, String)>,
    /// 被剔除的分析类键名。
    pub dropped: Vec<String>,
    /// 被同名后者覆盖掉的重复项数。
    pub duplicates: usize,
}

impl SanitizedCookie {
    /// 是否含 BOOTH 会话 cookie。
    pub fn has_session(&self) -> bool {
        self.pairs.iter().any(|(k, _)| k == SESSION_COOKIE)
    }
}

/// 是否分析类 cookie（会话项永远不算）。
fn is_analytics(key: &str) -> bool {
    if key.eq_ignore_ascii_case(SESSION_COOKIE) {
        return false;
    }
    let k = key.to_ascii_lowercase();
    ANALYTICS_PREFIXES.iter().any(|p| k.starts_with(p))
}

/// 把用户粘贴的各种形态归一成 `k=v` 列表。
///
/// 自动识别的粘贴形态（无需用户调整）：
/// - `k=v; k2=v2`（DevTools → Application → Cookies 手工拼、扩展导出）
/// - `Cookie: k=v; k2=v2`（请求头整行）
/// - cURL 命令（`-H 'cookie: …'` / `--header` / `-b` / `--cookie`）
/// - 多行文本（每行 `k=v` 或 `k: v`）
///
/// 同时剔除分析类 cookie（见 `ANALYTICS_PREFIXES`）。**保底**：若剔除后
/// 一条不剩，则原样保留——宁可多带几条，也不能把用户的 cookie 清空。
pub fn sanitize_cookie(raw: &str) -> SanitizedCookie {
    let payload = extract_cookie_payload(raw);
    let mut all: Vec<(String, String)> = Vec::new();
    let mut duplicates = 0usize;
    for seg in payload.split([';', '\n', '\r']) {
        let seg = seg.trim().trim_matches(|c| c == '"' || c == '\'').trim();
        if seg.is_empty() {
            continue;
        }
        // `k=v` 优先；退而支持 `k: v`（部分扩展导出成冒号分隔）。
        let (k, v) = match seg.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => match seg.split_once(':') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => continue,
            },
        };
        if k.is_empty() || k.contains(char::is_whitespace) {
            continue;
        }
        if let Some(pos) = all.iter().position(|(k2, _)| k2.eq_ignore_ascii_case(k)) {
            all[pos] = (k.to_string(), v.to_string());
            duplicates += 1;
            continue;
        }
        all.push((k.to_string(), v.to_string()));
    }

    let (mut keep, mut dropped): (Vec<_>, Vec<_>) =
        all.into_iter().partition(|(k, _)| !is_analytics(k));
    if keep.is_empty() {
        // 全是分析类 → 保底放行，不退化成空 cookie
        std::mem::swap(&mut keep, &mut dropped);
    }
    SanitizedCookie {
        dropped: dropped.into_iter().map(|(k, _)| k).collect(),
        duplicates,
        pairs: keep,
    }
}

/// 从整段文本里取出真正的 cookie 载荷。
fn extract_cookie_payload(raw: &str) -> String {
    let trimmed = raw.trim();
    // cURL 命令（DevTools「Copy as cURL」）：以 `curl` 开头即按命令行解析。
    // 判据只看开头，不能要求含 "cookie" 字样——`curl -b '…'` 形态里没有该词。
    if trimmed
        .get(..4)
        .is_some_and(|s| s.eq_ignore_ascii_case("curl"))
        && let Some(v) = grab_curl_cookie(trimmed)
    {
        return v;
    }
    // 整行请求头 `Cookie: …`（ASCII 前缀，可用长度差切回原文）
    if let Some(rest) = trimmed.to_ascii_lowercase().strip_prefix("cookie:") {
        return trimmed[trimmed.len() - rest.len()..].to_string();
    }
    trimmed.to_string()
}

/// 从 cURL 命令里提取 cookie 值。
fn grab_curl_cookie(cmd: &str) -> Option<String> {
    let lower = cmd.to_ascii_lowercase();
    for flag in ["--header", "-h", "--cookie", "-b"] {
        let mut from = 0usize;
        while let Some(pos) = lower[from..].find(flag) {
            let at = from + pos;
            // 必须是一个独立参数：前一个字符是空白
            let prev_ok = at == 0 || cmd[..at].chars().last().is_some_and(char::is_whitespace);
            let after = cmd[at + flag.len()..].trim_start();
            if prev_ok && after.starts_with(['"', '\'']) {
                let quote = after.chars().next()?;
                if let Some(end) = after[1..].find(quote) {
                    let body = &after[1..1 + end];
                    // `-H 'cookie: xxx'` 取冒号后；`-b 'xxx'` 值本身就是载荷
                    if let Some(v) = body
                        .strip_prefix("cookie:")
                        .or_else(|| body.strip_prefix("Cookie:"))
                    {
                        return Some(v.to_string());
                    }
                    if flag == "-b" || flag == "--cookie" {
                        return Some(body.to_string());
                    }
                }
            }
            from = at + flag.len();
        }
    }
    None
}

/// Cookie 可用性检测结果（供 GUI / CLI / MCP 展示）。
#[derive(Debug, Clone, Serialize)]
pub struct CookieCheck {
    /// 结论：`valid` / `invalid` / `unreachable` / `not_configured`。
    pub state: String,
    /// 是否可用。
    pub ok: bool,
    /// 人类可读说明。
    pub detail: String,
    /// 净化后实际生效的 cookie 条数。
    pub pair_count: usize,
    /// 自动剔除的分析类 cookie 条数。
    pub dropped_count: usize,
    /// 是否含 BOOTH 会话 cookie。
    pub has_session: bool,
}

/// 检测 Cookie 是否真的能登入 BOOTH。
///
/// 判据（实测）：`GET /downloadables/1` 不跟随重定向——
/// 302 到 `/users/sign_in` 即未登录；其它（404 / 200）即已登录。
/// 网络 / 代理不通时返回 `unreachable`，与「Cookie 无效」明确区分开。
pub fn check_cookie(config: &AppConfig, cookie: Option<&str>) -> CookieCheck {
    let Some(raw) = cookie.map(str::trim).filter(|s| !s.is_empty()) else {
        return CookieCheck {
            state: "not_configured".into(),
            ok: false,
            detail: "未填写 Cookie".into(),
            pair_count: 0,
            dropped_count: 0,
            has_session: false,
        };
    };
    let san = sanitize_cookie(raw);
    let pair_count = san.pairs.len();
    let dropped_count = san.dropped.len();
    let has_session = san.has_session();

    let client = make_session_no_redirect(config, Some(raw));
    let resp = match client.get(PROBE_URL).send() {
        Ok(r) => r,
        Err(e) => {
            return CookieCheck {
                state: "unreachable".into(),
                ok: false,
                detail: format!("无法连接 BOOTH（检查网络或代理）：{e}"),
                pair_count,
                dropped_count,
                has_session,
            };
        }
    };
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if resp.status().is_redirection() && is_login_redirect(location) {
        let hint = if has_session {
            "会话可能已过期，请重新从浏览器复制"
        } else {
            "未找到会话 cookie（_plaza_session_nktz7u），请确认复制时包含它"
        };
        CookieCheck {
            state: "invalid".into(),
            ok: false,
            detail: format!("BOOTH 判定为未登录：{hint}"),
            pair_count,
            dropped_count,
            has_session,
        }
    } else {
        CookieCheck {
            state: "valid".into(),
            ok: true,
            detail: if dropped_count > 0 {
                format!("登录态有效；已自动剔除 {dropped_count} 条无关统计项，保留 {pair_count} 条")
            } else {
                format!("登录态有效（{pair_count} 条 cookie）")
            },
            pair_count,
            dropped_count,
            has_session,
        }
    }
}

/// 响应是否为「被重定向到登录页」——未登录的唯一标志。
fn is_login_redirect(location: &str) -> bool {
    location.contains("/users/sign_in")
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

/// 注入 `.booth.pm`。先经 [`sanitize_cookie`] 归一与净化，再**逐个** `add_cookie_str`。
///
/// **必须逐个**：`Jar::add_cookie_str` 解析的是单个 `Set-Cookie` 头，
/// `; ` 之后的片段会被当作该 cookie 的属性处理，未知属性名静默忽略。
/// 整串一次性传入时**只有第一个 cookie 能进 jar**——而从浏览器复制的串首个
/// 往往是 `_ga` 这类与登录无关的统计 cookie，真正的会话 `_plaza_session_nktz7u`
/// 会连同 `cf_clearance` 一起被丢掉，症状是「明明填了 Cookie，却提示请到设置页
/// 填写 Cookie」（见 PR #56）。
fn add_cookie_string(jar: &Jar, s: &str) {
    let url = "https://booth.pm/".parse().expect("booth.pm url");
    for (k, v) in sanitize_cookie(s).pairs {
        jar.add_cookie_str(&format!("{k}={v}"), &url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::cookie::CookieStore;

    fn sent_header(cookie: &str) -> String {
        let jar = parse_cookie(cookie);
        let url = reqwest::Url::parse("https://booth.pm/downloadables/1").unwrap();
        jar.cookies(&url)
            .map(|v| v.to_str().unwrap_or("").to_string())
            .unwrap_or_default()
    }

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
        // 刻意混入一个分析项：它应被剔除，其余四项必须全部送达。
        let raw = "_ga=G.1; recent_items=1; _plaza_session_nktz7u=SESSVAL; \
                   cf_clearance=CFVAL; __cf_bm=BMVAL";
        let sent = sent_header(raw);
        for expected in [
            "recent_items=1",
            "_plaza_session_nktz7u=SESSVAL",
            "cf_clearance=CFVAL",
            "__cf_bm=BMVAL",
        ] {
            assert!(sent.contains(expected), "未发出 {expected}，实际：{sent}");
        }
        assert!(!sent.contains("_ga="), "统计项未剔除：{sent}");
    }

    /// 会话 cookie 不在首位时也必须送达（这正是线上翻车的形态）。
    #[test]
    fn session_cookie_survives_leading_noise() {
        let raw = "_ga=GA1.1.999; ga_expire_X=1; _plaza_session_nktz7u=REAL";
        let sent = sent_header(raw);
        assert!(
            sent.contains("_plaza_session_nktz7u=REAL"),
            "会话 cookie 被噪声挤掉，实际：{sent}"
        );
        // 分析类应被剔除
        assert!(!sent.contains("_ga="), "统计项未剔除：{sent}");
    }

    /// 空段、无 `=` 的残片不得破坏其余 cookie。
    #[test]
    fn malformed_segments_do_not_break_others() {
        let sent = sent_header("; ; junk; _plaza_session_nktz7u=OK; ;");
        assert!(sent.contains("_plaza_session_nktz7u=OK"), "实际：{sent}");
    }

    /// 整行请求头前缀应被剥掉。
    #[test]
    fn strips_cookie_header_prefix() {
        let s = sanitize_cookie("Cookie: _plaza_session_nktz7u=A; cf_clearance=B");
        assert!(s.pairs.iter().any(|(k, v)| k == SESSION_COOKIE && v == "A"));
        assert!(s.pairs.iter().any(|(k, _)| k == "cf_clearance"));
    }

    /// cURL 命令（DevTools「Copy as cURL」）也要能用。
    #[test]
    fn extracts_from_curl_command() {
        let cmd = "curl 'https://booth.pm/zh-cn/items/1' -H 'accept: text/html' \
                   -H 'cookie: _plaza_session_nktz7u=CURLVAL; cf_clearance=CF' --compressed";
        let s = sanitize_cookie(cmd);
        assert!(
            s.pairs
                .iter()
                .any(|(k, v)| k == SESSION_COOKIE && v == "CURLVAL"),
            "实际：{:?}",
            s.pairs
        );
        assert!(s.pairs.iter().any(|(k, _)| k == "cf_clearance"));
        // 非 cookie 的 header 不得混入
        assert!(!s.pairs.iter().any(|(k, _)| k == "accept"));

        // `-b` 形态
        let s2 = sanitize_cookie("curl -b '_plaza_session_nktz7u=BVAL' https://booth.pm/");
        assert!(
            s2.pairs
                .iter()
                .any(|(k, v)| k == SESSION_COOKIE && v == "BVAL")
        );
    }

    /// 多行粘贴（每行一条）也要能用。
    #[test]
    fn handles_multiline_paste() {
        let s = sanitize_cookie("_plaza_session_nktz7u=ML\ncf_clearance=MCF\n__cf_bm=MB");
        assert_eq!(s.pairs.len(), 3, "实际：{:?}", s.pairs);
        assert!(s.has_session());
    }

    /// 分析类剔除，但会话与 Cloudflare 相关项必须留下。
    #[test]
    fn drops_analytics_keeps_essentials() {
        let raw = "_ga=G; _ga_ABC=1; ga_expire_ABC=2; _gcl_au=3; _gid=4; \
                   search_history_with_params_v2=5; recent_items=6; \
                   _plaza_session_nktz7u=S; cf_clearance=C; __cf_bm=B";
        let s = sanitize_cookie(raw);
        let kept: Vec<&str> = s.pairs.iter().map(|(k, _)| k.as_str()).collect();
        for must in [SESSION_COOKIE, "cf_clearance", "__cf_bm"] {
            assert!(kept.contains(&must), "{must} 被误删，保留：{kept:?}");
        }
        assert!(
            kept.contains(&"recent_items"),
            "BOOTH 自有项不应剔除：{kept:?}"
        );
        for gone in ["_ga", "_ga_ABC", "ga_expire_ABC", "_gcl_au", "_gid"] {
            assert!(!kept.contains(&gone), "{gone} 未剔除，保留：{kept:?}");
        }
        assert!(s.dropped.len() >= 5, "剔除计数：{}", s.dropped.len());
    }

    /// 保底：整串都是分析类时不得清空（宁可多带，不能变空）。
    #[test]
    fn keeps_all_when_everything_looks_analytical() {
        let s = sanitize_cookie("_ga=1; _gid=2; _gcl_au=3");
        assert_eq!(s.pairs.len(), 3, "实际：{:?}", s.pairs);
        assert!(s.dropped.is_empty());
    }

    /// 同名 cookie 去重保留后者；计数正确。
    #[test]
    fn dedupes_keeping_last() {
        let s = sanitize_cookie("_plaza_session_nktz7u=OLD; _plaza_session_nktz7u=NEW");
        assert_eq!(s.pairs.len(), 1);
        assert_eq!(s.pairs[0].1, "NEW");
        assert_eq!(s.duplicates, 1);
    }

    /// 冒号分隔形态（部分扩展导出）。
    #[test]
    fn accepts_colon_separated_pairs() {
        let s = sanitize_cookie("_plaza_session_nktz7u: CV; cf_clearance: CF");
        assert!(
            s.pairs
                .iter()
                .any(|(k, v)| k == SESSION_COOKIE && v == "CV")
        );
    }

    /// 未被重定向到登录页即视为已登录。
    #[test]
    fn login_redirect_detection() {
        assert!(is_login_redirect("https://booth.pm/users/sign_in"));
        assert!(is_login_redirect("/users/sign_in?return_to=%2F"));
        assert!(!is_login_redirect("https://s6.booth.pm/f/1/x.zip"));
        assert!(!is_login_redirect(""));
    }

    /// 未配置 cookie 时不发请求，直接给 not_configured。
    #[test]
    fn check_cookie_without_config_reports_missing() {
        let cfg = AppConfig::default();
        let r = check_cookie(&cfg, None);
        assert_eq!(r.state, "not_configured");
        assert!(!r.ok);
    }

    /// 真实粘贴形态（浏览器整串复制）——净化后必须含会话项且条数可控。
    #[test]
    fn realistic_browser_copy_is_sanitized() {
        let raw = "_ga=GA1.1.1495967290.1786884669; _gcl_au=1.1.1814767935.1787941710; \
                   ga_expire_ABC=1788568737492; _ga_ABC=GS2.1.s1788571422$o3$g0; \
                   search_history_with_params_v2=%5B%7B%22q%22%3A%22x%22%7D%5D; \
                   recent_items=8567213%2C8898186; _plaza_session_nktz7u=SESS; \
                   cf_clearance=CF; __cf_bm=BM";
        let s = sanitize_cookie(raw);
        assert!(s.has_session(), "会话项丢失：{:?}", s.pairs);
        assert!(
            s.dropped.len() >= 4,
            "应剔除 4 条以上统计项，实为 {}",
            s.dropped.len()
        );
        assert!(s.pairs.len() <= 6, "保留条数过多：{}", s.pairs.len());
        assert!(sent_header(raw).contains("_plaza_session_nktz7u=SESS"));
    }
}
