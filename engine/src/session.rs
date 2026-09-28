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

/// 探针的总时限与连接时限。
///
/// reqwest 默认**无总超时**，网络被黑洞时（连接不 RST、只是不回）`send()`
/// 永不返回，于是 `unreachable` 永远不触发——GUI 停在「检测中…」、MCP/CLI 挂住。
/// 与 `update.rs` 的 `CHANNEL_TIMEOUT` 同款规矩：单个入口不得拖死整体。
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const PROBE_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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

/// 明显是 HTTP 请求头、而非 cookie 的名字。
///
/// 用户实操里常把 DevTools 的 Request Headers **整块**复制过来，块里混着
/// `accept` / `referer` / `sec-ch-ua` 等；万一 `cookie:` 行没被识别到，
/// 这一层兜底，避免把它们注入成 cookie。这些名字不会与业务 cookie 重名。
///
/// 注：HTTP/2 伪头（`:authority` / `:method` …）以冒号开头，键名解析后为空，
/// 在更早一步就被丢弃，无需在此列出。
const HTTP_HEADER_NAMES: &[&str] = &[
    "accept",
    "cache-control",
    "connection",
    "content-",
    "dnt",
    "host",
    "origin",
    "pragma",
    "priority",
    "referer",
    "sec-",
    "user-agent",
    "x-requested-with",
];

/// 构建 blocking Client。
///
/// `cookie`: 'k=v; k2=v2' 串 / Netscape cookies.txt 路径 / 存原始 Cookie 串的文本文件路径。
pub fn make_session(config: &AppConfig, cookie: Option<&str>) -> Client {
    build_client(config, cookie, true, false)
}

/// 同 [`make_session`]，但**不跟随重定向**（供登录态探针判定 302 目标），
/// 且带总超时与连接超时。
///
/// 超时**只加在这里**：`.timeout()` 是整个请求的总时限，加进下载链路会把
/// 正常的大文件下载掐断。详见 [`PROBE_TIMEOUT`]。
pub fn make_session_no_redirect(config: &AppConfig, cookie: Option<&str>) -> Client {
    build_client(config, cookie, false, true)
}

fn build_client(config: &AppConfig, cookie: Option<&str>, follow: bool, probe: bool) -> Client {
    let mut builder = Client::builder()
        .user_agent(UA)
        .default_headers(default_headers());
    if !follow {
        builder = builder.redirect(reqwest::redirect::Policy::none());
    }
    if probe {
        builder = builder
            .timeout(PROBE_TIMEOUT)
            .connect_timeout(PROBE_CONNECT_TIMEOUT);
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

/// 是否与 BOOTH 登录无关：分析类 cookie，或整块 Headers 粘贴时混入的 HTTP 头名。
/// 会话项永远不算无关。
fn is_irrelevant(key: &str) -> bool {
    if key.eq_ignore_ascii_case(SESSION_COOKIE) {
        return false;
    }
    let k = key.to_ascii_lowercase();
    ANALYTICS_PREFIXES
        .iter()
        .chain(HTTP_HEADER_NAMES.iter())
        .any(|p| k.starts_with(p))
}

/// 把用户粘贴的各种形态归一成 `k=v` 列表。
///
/// 自动识别的粘贴形态（无需用户调整）：
/// - **DevTools 表格整块复制**（Application → Cookies，制表符分隔）
/// - `k=v; k2=v2` 串
/// - `Cookie: k=v; k2=v2`（请求头整行，含嵌在整块 Headers 里的那一行）
/// - cURL 命令（`-H 'cookie: …'` / `--header` / `-b` / `--cookie`）
/// - 多行文本（每行 `k=v` 或 `k: v`）
///
/// 同时剔除无关项（见 `is_irrelevant`）。**保底**：若剔除后一条不剩，
/// 则原样保留——宁可多带几条，也不能把用户的 cookie 清空。
pub fn sanitize_cookie(raw: &str) -> SanitizedCookie {
    let payload = extract_cookie_payload(raw);
    // DevTools 表格形态的列序与字符串形态完全不同，必须单独解析，
    // 不能落到下面的 `=` / `:` 切分逻辑上（否则一条都进不去）。
    if let Some(rows) = parse_tabular_rows(&payload) {
        return finalize(rows);
    }
    let mut all: Vec<(String, String)> = Vec::new();
    for seg in payload.split([';', '\n', '\r']) {
        let seg = seg.trim().trim_matches(['"', '\'']).trim();
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
        all.push((k.to_string(), v.to_string()));
    }
    finalize(all)
}

/// 归一收尾：去重（同名保留后者）→ 剔除无关项 → 保底。
fn finalize(rows: Vec<(String, String)>) -> SanitizedCookie {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut duplicates = 0usize;
    for (k, v) in rows {
        if k.is_empty() || k.contains(char::is_whitespace) {
            continue;
        }
        if let Some(pos) = pairs.iter().position(|(k2, _)| k2.eq_ignore_ascii_case(&k)) {
            pairs[pos] = (k, v);
            duplicates += 1;
            continue;
        }
        pairs.push((k, v));
    }
    let (mut keep, mut dropped): (Vec<_>, Vec<_>) =
        pairs.into_iter().partition(|(k, _)| !is_irrelevant(k));
    if keep.is_empty() {
        // 全是无关项 → 保底放行，不退化成空 cookie（swap 后 dropped 自然为空）
        std::mem::swap(&mut keep, &mut dropped);
    }
    SanitizedCookie {
        dropped: dropped.into_iter().map(|(k, _)| k).collect(),
        duplicates,
        pairs: keep,
    }
}

/// 解析 DevTools → Application → Cookies 表格的复制结果（制表符分隔）。
///
/// 列序：`Name · Value · Domain · Path · Expires · Size · HttpOnly · Secure · SameSite …`
/// 与 Netscape cookies.txt **不同**（后者是 `domain · flag · path · secure · expires · name · value`），
/// 两者的 name/value 列号完全不同，不能共用索引。
///
/// 判据：制表符分隔 ≥4 列，且**第 3 列**（domain）含 `booth`。
/// 不满足即返回 `None`，交回字符串形态处理。
fn parse_tabular_rows(text: &str) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        // 完整表格：Name · Value · Domain · …（domain 在第 2 列，0 基）
        if parts.len() >= 4 {
            let (name, value, domain) = (parts[0].trim(), parts[1].trim(), parts[2].trim());
            if !name.is_empty() && domain.contains("booth") {
                out.push((name.to_string(), value.to_string()));
                continue;
            }
        }
        // 退化：只框选了 Name + Value 两列也是常见操作（表格其余列对用户无用）。
        // 此时没有 domain 可校验，但只要第 0 列形如 cookie 名就收下——
        // 否则用户会**静默得到空 cookie**，界面却只说「未登录」，无从下手。
        // 排除域名形态（`.booth.pm`）与路径，避免误收 Netscape cookies.txt 的行。
        if parts.len() >= 2 {
            let (name, value) = (parts[0].trim(), parts[1].trim());
            let looks_like_name = !name.is_empty()
                && !value.is_empty()
                && !name.starts_with('.')
                && !name.contains('/')
                && !name.contains("booth.");
            if looks_like_name {
                out.push((name.to_string(), value.to_string()));
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// 从整段文本里取出真正的 cookie 载荷。
fn extract_cookie_payload(raw: &str) -> String {
    let trimmed = raw.trim();
    // 1) cURL 命令（DevTools「Copy as cURL」）：以 `curl` 开头即按命令行解析。
    // 判据只看开头，不能要求含 "cookie" 字样——`curl -b '…'` 形态里没有该词。
    if trimmed
        .get(..4)
        .is_some_and(|s| s.eq_ignore_ascii_case("curl"))
        && let Some(v) = grab_curl_cookie(trimmed)
    {
        return v;
    }
    // 2) 优先定位 `cookie:` 行。用户实操形态是 Network → 某请求 → Headers 面板
    //    **整块复制**，块里还混着 `:authority`/`accept`/`referer` 等——只认这一行。
    //    没有这一步，`cookie: xxx` 的标签会混进键名（含空格被丢弃），
    //    结果反倒把真正的会话 cookie 弄丢、留下满屏 HTTP 头名。
    if let Some(v) = grab_cookie_header_line(trimmed) {
        return v;
    }
    // 3) 整行 `Cookie: …`（单行、无换行）
    if let Some(rest) = trimmed.to_ascii_lowercase().strip_prefix("cookie:") {
        return trimmed[trimmed.len() - rest.len()..].to_string();
    }
    trimmed.to_string()
}

/// 在多行文本中定位 `cookie:` 行并取其值（大小写不敏感）。
fn grab_cookie_header_line(text: &str) -> Option<String> {
    for line in text.lines() {
        let l = line.trim();
        if l.get(..7)
            .is_some_and(|p| p.eq_ignore_ascii_case("cookie:"))
        {
            return Some(l[7..].trim().to_string());
        }
    }
    None
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
                    // `-H 'cookie: xxx'` 取冒号后；`-b 'xxx'` 值本身就是载荷。
                    // 大小写不敏感：只认 `cookie:` / `Cookie:` 会让全大写 `COOKIE:`
                    // **静默丢掉会话项**（浏览器与各扩展输出的大小写并不统一），
                    // 而单行 / 多行两条路径本来就是忽略大小写的。
                    if body.len() >= 7 && body[..7].eq_ignore_ascii_case("cookie:") {
                        return Some(body[7..].to_string());
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

/// 三端共用的响应信封。
///
/// 此前 CLI / MCP / GUI **各手写一份**同样的 7 字段：新增字段要改三处，
/// 且键序不一致（CLI/GUI 走 `json!` 的字母序，MCP 走结构体声明序），
/// 字节级 diff 对不上——这正是三端漂移的典型形态。现由本结构体单点定义。
#[derive(Debug, Clone, Serialize)]
pub struct CookieCheckResponse {
    command: &'static str,
    #[serde(flatten)]
    check: CookieCheck,
}

impl CookieCheck {
    /// 组装为三端统一的 JSON 信封。
    pub fn to_command_json(&self) -> serde_json::Value {
        let env = CookieCheckResponse {
            command: "cookie_check",
            check: self.clone(),
        };
        serde_json::to_value(env).unwrap_or_else(|_| serde_json::json!({ "ok": false }))
    }
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
    let san = load_cookie_pairs(raw);
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
    let (state, reason) = classify_probe(resp.status().as_u16(), location, has_session);
    let detail = match state {
        "valid" if dropped_count > 0 => {
            format!("{reason}；已自动剔除 {dropped_count} 条无关统计项，保留 {pair_count} 条")
        }
        "valid" => format!("{reason}（{pair_count} 条 cookie）"),
        _ => reason,
    };
    CookieCheck {
        state: state.to_string(),
        ok: state == "valid",
        detail,
        pair_count,
        dropped_count,
        has_session,
    }
}

/// 探针响应归类 → `(state, reason)`。纯函数，不碰网络。
///
/// 抽成纯函数是为了能单测锁住分类：判据写错的症状是「检测说没问题、下载却失败」，
/// 比没有检测更糟——用户会按它给出的结论去排查，方向反而被带偏。
///
/// 已登录的证据只有两个：资源不存在（`404`）或资源存在（`2xx`）。
/// **其余状态码一律不是「凭证有效」的证据**：403 多为 Cloudflare 拦截/challenge，
/// 429 是限流，5xx 是站点故障——把它们判成 valid，等于用一个未知结果冒充确认。
fn classify_probe(status: u16, location: &str, has_session: bool) -> (&'static str, String) {
    if (300..400).contains(&status) && is_login_redirect(location) {
        let hint = if has_session {
            "会话可能已过期，请重新从浏览器复制"
        } else {
            "未找到会话 cookie（_plaza_session_nktz7u），请确认复制时包含它"
        };
        return ("invalid", format!("BOOTH 判定为未登录：{hint}"));
    }
    if status == 404 || (200..300).contains(&status) {
        return ("valid", "登录态有效".to_string());
    }
    let why = match status {
        401 | 403 => "BOOTH 拒绝了请求（可能是 Cloudflare 风控拦截）",
        429 => "请求过于频繁，被限流",
        s if s >= 500 => "BOOTH 服务端故障",
        s if (300..400).contains(&s) => "被重定向到了非登录页的地址",
        s => return ("unreachable", format!("BOOTH 返回了意料外的状态码 {s}")),
    };
    (
        "unreachable",
        format!("{why}（HTTP {status}）：无法据此判定登录态"),
    )
}

/// 响应是否为「被重定向到登录页」——未登录的唯一标志。
fn is_login_redirect(location: &str) -> bool {
    location.contains("/users/sign_in")
}

/// 解析 Netscape cookies.txt 行。
///
/// 列序（**均 0 基**）：`domain · flag · path · secure · expires · name · value`
/// —— domain 第 0 列、name 第 5 列、value 第 6 列。
/// 与 DevTools 表格（name 第 0 列 / value 第 1 列 / domain 第 **2** 列）**完全不同**，
/// 判据混用会导致整块解析错位、cookie 一条都进不去。
fn parse_netscape_rows(text: &str) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 7 && parts[0].contains("booth") {
            out.push((parts[5].to_string(), parts[6].to_string()));
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// cookie 参数的统一入口：'k=v; k2=v2' 串 / Netscape cookies.txt 路径 / 存串的文本文件路径。
///
/// **注入与统计必须共用这里**。统计若绕过它、直接对参数本身调 `sanitize_cookie`，
/// 传入文件路径时会把**路径字符串**当 cookie 解析（`D:` 被切成一条），
/// 得出「生效 1 条、无会话项」这种与事实相反、且自相矛盾的结论。
fn load_cookie_pairs(cookie_arg: &str) -> SanitizedCookie {
    let p = Path::new(cookie_arg);
    if p.is_file()
        && let Ok(text) = std::fs::read_to_string(p)
    {
        if let Some(rows) = parse_netscape_rows(&text) {
            return finalize(rows);
        }
        return sanitize_cookie(&text);
    }
    sanitize_cookie(cookie_arg)
}

/// 注入 `.booth.pm`。统一走 [`load_cookie_pairs`]，再**逐个** `add_cookie_str`。
///
/// **必须逐个**：`Jar::add_cookie_str` 解析的是单个 `Set-Cookie` 头，
/// `; ` 之后的片段会被当作该 cookie 的属性处理，未知属性名静默忽略。
/// 整串一次性传入时**只有第一个 cookie 能进 jar**——而从浏览器复制的串首个
/// 往往是 `_ga` 这类与登录无关的统计 cookie，真正的会话 `_plaza_session_nktz7u`
/// 会连同 `cf_clearance` 一起被丢掉，症状是「明明填了 Cookie，却提示请到设置页
/// 填写 Cookie」（见 PR #56）。
fn parse_cookie(cookie_arg: &str) -> Jar {
    let jar = Jar::default();
    let url = "https://booth.pm/".parse().expect("booth.pm url");
    for (k, v) in load_cookie_pairs(cookie_arg).pairs {
        jar.add_cookie_str(&format!("{k}={v}"), &url);
    }
    jar
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

    /// 用户实操形态：DevTools → Network → 某个 .json → Request Headers 整块复制。
    /// 块里除了 `cookie` 还有 `:authority`/`accept`/`referer` 等，**绝不能当成 cookie**。
    #[test]
    fn extracts_cookie_from_full_request_headers_paste() {
        let raw = ":authority: booth.pm\n:method: GET\n\
                   :path: /items/8900909/wish_list_items.json\n:scheme: https\n\
                   accept: application/json\naccept-encoding: gzip, deflate, br, zstd\n\
                   accept-language: zh-CN,zh;q=0.9,ja;q=0.8,en;q=0.7\n\
                   cache-control: no-cache\ncontent-type: application/json\n\
                   cookie: _plaza_session_nktz7u=HDR; cf_clearance=CF\n\
                   dnt: 1\npragma: no-cache\npriority: u=1, i\n\
                   referer: https://booth.pm/zh-cn/items/8567213\n\
                   sec-ch-ua: \"Chromium\";v=\"153\", \"Not_A Brand\";v=\"8\"";
        let s = sanitize_cookie(raw);
        let kept: Vec<&str> = s.pairs.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(kept.len(), 2, "应只留 cookie 行的两项，实际：{kept:?}");
        assert!(s.has_session(), "实际：{:?}", s.pairs);
        for junk in ["accept", "referer", ":method", "content-type", "dnt"] {
            assert!(!kept.contains(&junk), "{junk} 被当成 cookie：{kept:?}");
        }
    }

    /// 单行 `Cookie:` 前缀 + 大小写混写。
    #[test]
    fn cookie_header_line_case_insensitive() {
        let s = sanitize_cookie("COOKIE: _plaza_session_nktz7u=UP; cf_clearance=C");
        assert!(s.has_session());
        assert_eq!(s.pairs.len(), 2, "实际：{:?}", s.pairs);
    }

    fn write_tmp_cookie_file(tag: &str, body: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "bvt_ck_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, body).unwrap();
        p
    }

    /// 传**文件路径**时，统计必须基于文件内容，而不是把路径字符串当 cookie 解析。
    ///
    /// 曾经的缺陷：`check_cookie` 直接对参数调 `sanitize_cookie`，于是 `D:/…` 被
    /// 切成一条名为 `D` 的 cookie，得出「生效 1 条、无会话项」——与它自己刚探测出的
    /// 「登录态有效」自相矛盾。
    #[test]
    fn file_path_source_is_read_not_parsed_as_string() {
        // DevTools 表格文件
        let tsv = write_tmp_cookie_file(
            "tsv",
            "_ga\tGA1\t.booth.pm\t/\t2027-01-01T00:00:00.000Z\t30\n\
             _plaza_session_nktz7u\tSESS\t.booth.pm\t/\t2027-01-01T00:00:00.000Z\t921\n\
             cf_clearance\tCF\t.booth.pm\t/\t2027-01-01T00:00:00.000Z\t438\n",
        );
        let s = load_cookie_pairs(tsv.to_str().unwrap());
        assert!(s.has_session(), "会话项丢失：{:?}", s.pairs);
        assert_eq!(s.pairs.len(), 2, "实际：{:?}", s.pairs);
        let _ = std::fs::remove_file(&tsv);

        // Netscape cookies.txt
        let ns = write_tmp_cookie_file(
            "ns",
            "# Netscape HTTP Cookie File\n\
             .booth.pm\tTRUE\t/\tTRUE\t1900000000\t_plaza_session_nktz7u\tNSSESS\n\
             .booth.pm\tTRUE\t/\tTRUE\t1900000000\tcf_clearance\tNSCF\n",
        );
        let s2 = load_cookie_pairs(ns.to_str().unwrap());
        assert!(s2.has_session(), "Netscape 会话项丢失：{:?}", s2.pairs);
        assert_eq!(
            s2.pairs
                .iter()
                .find(|(k, _)| k == SESSION_COOKIE)
                .unwrap()
                .1,
            "NSSESS"
        );
        let _ = std::fs::remove_file(&ns);
    }

    /// 用户实操形态 B：DevTools → Application → Cookies **表格整块复制**（TSV）。
    ///
    /// 列序是 `Name / Value / Domain / Path / Expires / Size / HttpOnly / …`，
    /// 与 Netscape cookies.txt **完全不同**（后者 domain 在第 0 列、name/value 在 5/6 列）。
    /// 若沿用后者索引，或按 `=`/`:` 切分，都会整块解析失败——cookie 一条都进不去。
    #[test]
    fn extracts_from_devtools_cookie_table_tsv() {
        let raw = "__cf_bm\tBMVAL\t.booth.pm\t/\t2026-09-28T11:32:11.031Z\t206\t✓\t✓\tNone\t\t\tMedium\t\n\
                   _ga\tGA1.1.111\t.booth.pm\t/\t2027-11-02T10:03:08.515Z\t30\t\t\t\t\t\tMedium\t\n\
                   _plaza_session_nktz7u\tSESSVAL\t.booth.pm\t/\t2027-09-28T10:03:18.450Z\t921\t✓\t✓\tLax\t\t\tMedium\t\n\
                   cf_clearance\tCFVAL\t.booth.pm\t/\t2027-09-28T10:03:08.781Z\t438\t✓\t✓\tNone\thttps://booth.pm\t\tMedium\t";
        let s = sanitize_cookie(raw);
        let kept: Vec<&str> = s.pairs.iter().map(|(k, _)| k.as_str()).collect();
        assert!(s.has_session(), "会话项丢失：{kept:?}");
        assert!(kept.contains(&"cf_clearance"), "{kept:?}");
        assert!(kept.contains(&"__cf_bm"), "{kept:?}");
        assert!(!kept.contains(&"_ga"), "统计项未剔除：{kept:?}");
        // 关键：值必须取自同行的第二列，不得串列
        let sess = &s
            .pairs
            .iter()
            .find(|(k, _)| k == SESSION_COOKIE)
            .expect("会话项")
            .1;
        assert_eq!(sess, "SESSVAL");
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

    /// 探针归类的全态锁定。判据写错的代价是「检测说没问题、下载却失败」，
    /// 用户会照着错误结论排查，所以每个状态码都要有断言。
    #[test]
    fn classify_probe_covers_every_status_class() {
        let (s, _) = classify_probe(404, "", true);
        assert_eq!(s, "valid", "有效 cookie 命中不存在的资源");
        let (s, _) = classify_probe(200, "", true);
        assert_eq!(s, "valid", "命中真实资源");
        let (s, _) = classify_probe(302, "https://booth.pm/users/sign_in", true);
        assert_eq!(s, "invalid");
        // 关键：以下都不是「凭证有效」的证据，判成 valid 会给出假阳性结论。
        let (s, d) = classify_probe(403, "", true);
        assert_eq!(s, "unreachable");
        assert!(d.contains("403"), "{d}");
        let (s, _) = classify_probe(429, "", true);
        assert_eq!(s, "unreachable");
        let (s, _) = classify_probe(500, "", true);
        assert_eq!(s, "unreachable");
        let (s, _) = classify_probe(503, "", true);
        assert_eq!(s, "unreachable");
        let (s, d) = classify_probe(302, "https://booth.pm/other", true);
        assert_eq!(s, "unreachable");
        assert!(d.contains("非登录页"), "{d}");
        let (s, _) = classify_probe(418, "", true);
        assert_eq!(s, "unreachable");
    }

    /// 未登录且未找到会话项时，提示要指向「缺哪一条」而非笼统的过期。
    #[test]
    fn invalid_hint_distinguishes_missing_session_from_expired() {
        let (_, d) = classify_probe(302, "https://booth.pm/users/sign_in", true);
        assert!(d.contains("过期"), "{d}");
        let (_, d) = classify_probe(302, "https://booth.pm/users/sign_in", false);
        assert!(d.contains(SESSION_COOKIE), "{d}");
    }

    /// cURL 里的 `COOKIE:` 全大写：只认小写会静默丢掉会话项。
    #[test]
    fn curl_header_case_insensitive() {
        let lower = sanitize_cookie(
            "curl 'https://booth.pm/x' -H 'cookie: _plaza_session_nktz7u=A; cf_clearance=B'",
        );
        let upper = sanitize_cookie(
            "curl 'https://booth.pm/x' -H 'COOKIE: _plaza_session_nktz7u=A; cf_clearance=B'",
        );
        let mixed = sanitize_cookie(
            "curl 'https://booth.pm/x' -H 'Cookie: _plaza_session_nktz7u=A; cf_clearance=B'",
        );
        for (label, s) in [("lower", &lower), ("upper", &upper), ("mixed", &mixed)] {
            assert!(
                s.has_session(),
                "{label} 丢失会话项：{:?}",
                s.pairs.iter().map(|p| &p.0).collect::<Vec<_>>()
            );
        }
    }

    /// 只框选 Name + Value 两列：不得静默得到空 cookie。
    #[test]
    fn two_column_table_paste_is_accepted() {
        let tsv = "_plaza_session_nktz7u\tSESSVAL\ncf_clearance\tCFVAL\n";
        let s = sanitize_cookie(tsv);
        assert!(
            s.has_session(),
            "两列粘贴应可用，实际：{:?}",
            s.pairs.iter().map(|p| &p.0).collect::<Vec<_>>()
        );
        assert_eq!(s.pairs.len(), 2);
    }

    /// Netscape cookies.txt 的行不得被两列退化分支误收（首列是域名）。
    #[test]
    fn netscape_rows_not_mistaken_for_two_column_table() {
        let netscape = "# Netscape HTTP Cookie File\n.booth.pm\tTRUE\t/\tFALSE\t0\tname\tvalue\n";
        let rows = parse_netscape_rows(netscape).expect("Netscape 应被识别");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "name");
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
