//! HTTP 请求重试：transport 错误指数退避（ConnectionError/Timeout 等价物）。
//! HTTP 状态码：404 等留给调用方判断；403/429（风控/限流）在此指数退避重试。

use std::time::Duration;

use reqwest::blocking::{Client, Response};

/// 最大重试次数。
pub const MAX_RETRIES: u32 = 3;

/// 触发限流退避的 HTTP 状态码（Cloudflare 风控 403 / GitHub 限流 429）。
pub const RATE_LIMIT_STATUSES: &[u16] = &[403, 429];

/// 单次退避封顶（秒）：基 2 指数 2/4/8/16/32，杜绝 Retry-After 极端值导致无限 sleep。
pub const MAX_BACKOFF_SECS: u64 = 32;

/// 仅对 GitHub API 域启用 403 退避（其 403 = 限流）：
/// BOOTH 域 403 是 Cloudflare 风控/未登录登录页伪装，重试无益反而叠加延迟。
fn is_rate_limit(url: &str, code: u16) -> bool {
    match code {
        429 => true,
        403 => url.contains("api.github.com"),
        _ => false,
    }
}

/// 退避时长：优先 `Retry-After` 头，否则 2^attempt；一律封顶 `MAX_BACKOFF_SECS`。
///
/// `attempt` 从 1 起（第 1 次失败后等待 2s，第 2 次 4s……）。
fn backoff_secs(attempt: u32, retry_after: Option<u64>) -> u64 {
    retry_after.unwrap_or(1 << attempt).min(MAX_BACKOFF_SECS)
}

/// 通用指数退避重试循环（测试与潜在复用共用）。
///
/// `is_retryable`：判定错误是否值得重试。返回 `Ok(T)` 即成功；重试耗尽返回最后一个 `Err`。
pub fn retry_loop<T, E, F, G>(mut run: F, is_retryable: G) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    G: Fn(&E) -> bool,
{
    let mut last_err = None;
    for attempt in 1..=MAX_RETRIES {
        match run() {
            Ok(v) => return Ok(v),
            Err(e) => {
                if is_retryable(&e) && attempt < MAX_RETRIES {
                    std::thread::sleep(Duration::from_secs(backoff_secs(attempt, None)));
                    last_err = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }
    Err(last_err.expect("retry loop always sets err on exit"))
}

/// 便捷封装：GET + 指定 headers（**不走缓存**）。
///
/// 无缓存版本，供文件下载、封面图等「必须逐字节到站」的场景使用。
/// 需要内容缓存的 HTML/JSON 请求请用 [`get_cached`]。
///
/// 对「传输层错误」与「403/429 限流」双重指数退避：
///   - 传输层错误（连接/超时/响应体截断）走 `is_connect || is_timeout || is_body`。
///   - HTTP 403/429（Cloudflare 风控 / GitHub 限流）按指数档退避，
///     优先尊重响应 `Retry-After` 头（封顶 `MAX_BACKOFF_SECS`），最长不超过 `MAX_RETRIES` 次。
///   - 403 退避仅限 GitHub API 域；BOOTH 域 403 视作最终响应（风控重试无益）。
///
/// 重试耗尽后返回最后一次响应（可能仍带错误状态码），由调用方判定业务语义。
pub fn get(
    client: &Client,
    url: &str,
    headers: reqwest::header::HeaderMap,
) -> Result<Response, reqwest::Error> {
    for attempt in 1..=MAX_RETRIES {
        match client.get(url).headers(headers.clone()).send() {
            Ok(resp) => {
                let code = resp.status().as_u16();
                if is_rate_limit(url, code) && attempt < MAX_RETRIES {
                    let wait = backoff_secs(attempt, retry_after_secs(&resp));
                    std::thread::sleep(Duration::from_secs(wait));
                    continue;
                }
                return Ok(resp);
            }
            Err(e) => {
                if (e.is_connect() || e.is_timeout() || e.is_body()) && attempt < MAX_RETRIES {
                    std::thread::sleep(Duration::from_secs(backoff_secs(attempt, None)));
                    continue;
                }
                return Err(e);
            }
        }
    }
    unreachable!("get: retry loop must return before exhausting")
}

/// 解析 `Retry-After` 头（秒数）；非法或缺失返回 None。
fn retry_after_secs(resp: &Response) -> Option<u64> {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
}

/// 缓存命中后的响应载荷。
///
/// 覆盖调用方真正需要的四样：状态码、最终 URL、响应体字节、**诊断头**。
///
/// 诊断头（`diag_headers`）不是全量响应头，只留决定下一步动作的几项：
/// `Cf-Mitigated`（挑战 vs 阻断）、`CF-RAY`（报障追踪号）、`Retry-After`（该等多久）。
/// 判据写在这里而不是让每个调用方各自看 status，是因为 `403` 有四种完全不同的成因，
/// 只报数字等于没报。
pub struct CachedBody {
    pub status: u16,
    pub final_url: String,
    pub body: Vec<u8>,
    /// `key: value\n` 拼成的诊断头，见 [`collect_diag_headers`]。
    pub diag_headers: String,
}

/// 需要为归因保留的响应头（小写，按此顺序）。
pub const DIAG_HEADER_KEYS: &[&str] = &["cf-mitigated", "cf-ray", "retry-after"];

/// 从响应里挑出诊断头，拼成 `key: value\n`。
///
/// **只挑这三项**：它们决定「该等 / 该换 UA / 该停手 / 该等多久」。
/// 不把 `Set-Cookie` 之类收进来——那会把凭证落到缓存盘上。
pub fn collect_diag_headers(resp: &reqwest::blocking::Response) -> String {
    let mut out = String::new();
    for key in DIAG_HEADER_KEYS {
        if let Some(v) = resp.headers().get(*key)
            && let Ok(s) = v.to_str()
        {
            out.push_str(key);
            out.push_str(": ");
            out.push_str(s.trim());
            out.push('\n');
        }
    }
    out
}

/// 诊断串里附带的响应体片段长度（字符）。
///
/// 取 240：够看清 Cloudflare 挑战页的 `<title>`、状态标识，又不至于把整页 HTML
/// 灌进日志。
pub const DIAG_BODY_CHARS: usize = 240;

/// Cloudflare 挑战页的分类。
///
/// **为什么分类而不只是「是不是挑战」**：`challenge` 与 `block` 的处理动作相反——
/// 前者换 UA / 拿 `cf_clearance` 可能过，后者是站点主动封禁，换什么都没用，该停手。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfMitigation {
    /// 人机校验（`Cf-Mitigated: challenge`）：换真实浏览器 UA / 带 cf_clearance 可能过。
    Challenge,
    /// 站点主动阻断（`Cf-Mitigated: block`）：重试无意义。
    Block,
    /// 响应头未声明，但响应体是挑战页（老版本 CF 或头被中间设备剥掉）。
    BodyLooksLikeChallenge,
}

impl CfMitigation {
    /// 中文说明，供错误信息直接使用。
    pub fn describe(self) -> &'static str {
        match self {
            CfMitigation::Challenge => {
                "Cloudflare 人机校验（非源站拒绝；需真实浏览器 UA / cf_clearance，或稍后重试）"
            }
            CfMitigation::Block => "Cloudflare 站点阻断（重试与换 UA 均无效，应停止请求）",
            CfMitigation::BodyLooksLikeChallenge => {
                "响应体是 Cloudflare 挑战页（未见 Cf-Mitigated 头，可能被中间层剥离）"
            }
        }
    }
}

/// 从诊断头串里取某个头的值（大小写不敏感，`key: value\n` 格式）。
fn diag_header<'a>(diag_headers: &'a str, key: &str) -> Option<&'a str> {
    for line in diag_headers.lines() {
        if let Some((k, v)) = line.split_once(':')
            && k.trim().eq_ignore_ascii_case(key)
        {
            return Some(v.trim());
        }
    }
    None
}

/// 判断这是不是 Cloudflare 拦截，并给出分类。
///
/// 判据优先级（**响应头优先于 body**）：
///   1. `Cf-Mitigated: challenge` / `block` —— 权威，不受页面改版影响；
///   2. body 含 `Just a moment...` / `cf_chl_opt` / `challenges.cloudflare.com`。
///
/// 响应头由 [`get_cached`] 连同 body 一起带出（见 `CachedBody::diag_headers`）。
/// **保守**：两条都不命中才返回 `None`；正常商品页/搜索页不会出现这些串。
pub fn cf_mitigation(diag_headers: &str, body: &str) -> Option<CfMitigation> {
    if let Some(v) = diag_header(diag_headers, "cf-mitigated") {
        let v = v.to_ascii_lowercase();
        return match v.as_str() {
            "challenge" => Some(CfMitigation::Challenge),
            "block" => Some(CfMitigation::Block),
            _ => None,
        };
    }
    let head: String = body.chars().take(4096).collect();
    if head.contains("Just a moment...")
        || head.contains("cf_chl_opt")
        || head.contains("challenges.cloudflare.com")
    {
        return Some(CfMitigation::BodyLooksLikeChallenge);
    }
    None
}

/// 从响应体里抽取一句**给人看**的说明。
///
/// 优先级：
///   1. `<noscript>` 内的文字 —— Cloudflare 挑战页会把「Enable JavaScript and cookies
///      to continue」这类**专门写给无法执行 JS 的客户端**的提示放在这里。这正是本工具
///      所处的位置（blocking HTTP 客户端），所以它比任何 CSS/JS 都更有价值；
///   2. `<title>` 文字；
///   3. 去掉标签与脚本/样式块后的可见文本前若干字；
///   4. 实在没有就退回原始前缀。
///
/// **为什么不是裸截 HTML**：真实 403 响应的前 160 字几乎全是 CSP meta 与内联 CSS，
/// 对人或 agent 都毫无信息量。把预算花在可见文本上才叫「暴露元信息」。
pub fn readable_hint(body: &[u8], max_chars: usize) -> String {
    let s = String::from_utf8_lossy(body);

    // 1) <noscript> 内容（CF 挑战页的关键提示就在这里）
    if let Some(start) = s.find("<noscript")
        && let Some(rest) = s.get(start..)
        && let Some(gt) = rest.find('>')
        && let Some(end) = rest.find("</noscript>")
    {
        let inner = &rest[gt + 1..end];
        let t = strip_tags(inner);
        if !t.is_empty() {
            return truncate_chars(&t, max_chars);
        }
    }

    // 2) <title>
    if let Some(start) = s.find("<title")
        && let Some(rest) = s.get(start..)
        && let Some(gt) = rest.find('>')
        && let Some(end) = rest.find("</title>")
    {
        let inner = &rest[gt + 1..end];
        let t = inner.trim();
        if !t.is_empty() {
            return truncate_chars(t, max_chars);
        }
    }

    // 3) 去标签后的可见文本
    let t = strip_tags(&s);
    if !t.is_empty() {
        return truncate_chars(&t, max_chars);
    }

    // 4) 兜底：原始前缀
    truncate_chars(&s.chars().take(max_chars).collect::<String>(), max_chars)
}

/// 去掉标签、脚本与样式块，压缩空白。
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0usize;
    let lower = s.to_ascii_lowercase();
    while i < bytes.len() {
        // 跳过 <script>...</script> 与 <style>...</style> 整块
        let skipped = ["<script", "<style"].iter().find_map(|open| {
            lower
                .get(i..)
                .filter(|r| r.starts_with(open))
                .and_then(|_| {
                    let close = if *open == "<script" {
                        "</script>"
                    } else {
                        "</style>"
                    };
                    lower.get(i..)?.find(close).map(|p| i + p + close.len())
                })
        });
        if let Some(next) = skipped {
            i = next;
            continue;
        }
        if bytes[i] == b'<' {
            // 跳到下一个 '>'
            match lower.get(i..).and_then(|r| r.find('>')) {
                Some(p) => {
                    i += p + 1;
                    continue;
                }
                None => break,
            }
        }
        // 判边界要按字符来，避免切坏多字节
        let ch = s[i..].chars().next().unwrap_or(' ');
        if ch.is_whitespace() {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
        i += ch.len_utf8();
    }
    out.trim().to_string()
}

/// 按**字符**截断（不是字节），不足则原样返回。
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let t: String = s.chars().take(max_chars).collect();
    format!("{t}…")
}

/// 构造一条**能归因**的 HTTP 失败诊断串。
///
/// 起因：`search`/`download` 曾只报 `status 403`，用户与维护者都无从判断是源站拒绝、
/// Cloudflare 挑战、还是凭证失效。同一时期人工排查也因为「只看状态码不看 body」
/// 把 Cloudflare 挑战误判成「站点不可达」，白停了两轮。
///
/// 本函数把四样东西绑在一起：**状态码 + 解读 + 可见文本摘要 + 追踪号**。
/// 纯函数，不联网，可直接单测。
///
/// `what` 是调用方上下文（如 `"search"` / `"chunk 0-1023"`）；
/// `final_url` 可传空串；`diag_headers` 为 [`CachedBody::diag_headers`] 同款格式。
pub fn describe_failure(
    what: &str,
    status: u16,
    final_url: &str,
    body: &[u8],
    diag_headers: &str,
) -> String {
    let text = String::from_utf8_lossy(body);
    let cf = cf_mitigation(diag_headers, &text);
    let hint = if let Some(c) = cf {
        c.describe()
    } else if status == 401 || status == 403 {
        "被拒绝（可能是凭证失效或站点风控）"
    } else if status == 404 {
        "资源不存在（检查 ID / 端点路径）"
    } else if status == 429 {
        "触发限流（检查配额或放慢请求）"
    } else if (500..600).contains(&status) {
        "服务端错误（稍后重试）"
    } else if (300..400).contains(&status) {
        "重定向（可能被登录页拦截，检查凭证）"
    } else {
        "未预期的状态码"
    };

    let mut out = format!("{what}: 状态码 {status}（{hint}）");
    if !final_url.is_empty() {
        out.push_str(&format!(" url={final_url}"));
    }
    // 追踪号：报障时能直接定位到 CF 侧的一次请求
    if let Some(ray) = diag_header(diag_headers, "cf-ray") {
        out.push_str(&format!(" cf-ray={ray}"));
    }
    if let Some(ra) = diag_header(diag_headers, "retry-after") {
        out.push_str(&format!(" retry-after={ra}s"));
    }
    let excerpt = readable_hint(body, DIAG_BODY_CHARS);
    if !excerpt.is_empty() {
        out.push_str(&format!("，站点提示：{excerpt}"));
    }
    out
}

/// 带内容缓存的 GET（只缓存**可校验的 200**，只用于 HTML/JSON 这类小响应）。
///
/// 语义契约：
///   - 命中缓存**不发起任何网络请求**，因此也不消耗限速与反风控预算；
///   - 命中时**原样回放**存储的 status 与 final_url，不伪造；
///   - 未命中时行为与 [`get`] 完全一致（同样的重试与退避）；
///   - **只有 `status == 200` 且 body 非空**才写缓存。历史实现收 2xx 全段，
///     于是 204/206/空体也入库；登录页伪装与 429/5xx 一旦入库，错误会被长期"冻住"；
///   - `ttl == 0`（`--no-cache`）**既不读也不写**：只挡读会让逃生口当场把坏响应写回；
///   - 缓存本身故障一律退化为无缓存，绝不因此拒绝出站。
pub fn get_cached(
    client: &Client,
    url: &str,
    headers: reqwest::header::HeaderMap,
    cache: Option<&crate::cache::Cache>,
    scope: crate::cache::Scope,
    ttl: u64,
) -> Result<CachedBody, reqwest::Error> {
    if let Some(c) = cache
        && let Some(e) = c.get(scope, url, ttl)
    {
        return Ok(CachedBody {
            status: e.status,
            final_url: e.final_url,
            body: e.body,
            diag_headers: e.diag_headers,
        });
    }
    let resp = get(client, url, headers)?;
    let status = resp.status().as_u16();
    let final_url = resp.url().to_string();
    // **必须在读 body 之前采集诊断头**：`bytes()` 会消费响应。
    let diag_headers = collect_diag_headers(&resp);
    let body = resp.bytes()?.to_vec();
    // 只缓存「200 且非空」：其余（204/206/3xx/4xx/5xx/空体）都可能是一次失败或
    // 不完整的响应，入库后会在 TTL 内持续污染后续请求。
    if let Some(c) = cache
        && ttl > 0
        && status == 200
        && !body.is_empty()
    {
        c.put(
            scope,
            url,
            &crate::cache::StoredEntry {
                status,
                final_url: final_url.clone(),
                body: body.clone(),
                diag_headers: diag_headers.clone(),
            },
        );
    }
    Ok(CachedBody {
        status,
        final_url,
        body,
        diag_headers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    enum FakeErr {
        Transport,
        Other,
    }

    #[test]
    fn retries_transport_errors() {
        let mut calls = 0;
        let out: Result<(), FakeErr> = retry_loop(
            || {
                calls += 1;
                Err(FakeErr::Transport)
            },
            |e| matches!(e, FakeErr::Transport),
        );
        assert_eq!(out, Err(FakeErr::Transport));
        assert_eq!(calls, MAX_RETRIES);
    }

    #[test]
    fn non_transport_errors_immediate() {
        let mut calls = 0;
        let out: Result<(), FakeErr> = retry_loop(
            || {
                calls += 1;
                Err(FakeErr::Other)
            },
            |e| matches!(e, FakeErr::Transport),
        );
        assert_eq!(out, Err(FakeErr::Other));
        assert_eq!(calls, 1);
    }

    #[test]
    fn succeeds_on_retry() {
        let mut calls = 0;
        let out: Result<(), FakeErr> = retry_loop(
            || {
                calls += 1;
                if calls < 2 {
                    Err(FakeErr::Transport)
                } else {
                    Ok(())
                }
            },
            |e| matches!(e, FakeErr::Transport),
        );
        assert_eq!(out, Ok(()));
        assert_eq!(calls, 2);
    }

    #[test]
    fn backoff_caps_at_max() {
        // Retry-After 极端值（如 3600s）被封顶。
        assert_eq!(backoff_secs(1, Some(3600)), MAX_BACKOFF_SECS);
        assert_eq!(backoff_secs(3, Some(100)), MAX_BACKOFF_SECS);
    }

    #[test]
    fn backoff_exponential_without_header() {
        assert_eq!(backoff_secs(1, None), 2);
        assert_eq!(backoff_secs(2, None), 4);
        assert_eq!(backoff_secs(3, None), 8);
        // 指数溢出封顶
        assert_eq!(backoff_secs(10, None), MAX_BACKOFF_SECS);
    }

    #[test]
    fn backoff_respects_retry_after_when_small() {
        assert_eq!(backoff_secs(1, Some(1)), 1);
        assert_eq!(backoff_secs(2, Some(5)), 5);
    }

    #[test]
    fn rate_limit_429_any_domain() {
        assert!(is_rate_limit("https://booth.pm/ja/items/1", 429));
        assert!(is_rate_limit("https://api.github.com/repos/x/y", 429));
    }

    #[test]
    fn rate_limit_403_only_github_api() {
        assert!(is_rate_limit("https://api.github.com/repos/x/y", 403));
        // BOOTH 域 403（Cloudflare 风控）不重试
        assert!(!is_rate_limit("https://booth.pm/ja/items/1", 403));
        assert!(!is_rate_limit("https://github.com/x/y/releases", 403));
    }

    #[test]
    fn rate_limit_other_codes_false() {
        assert!(!is_rate_limit("https://api.github.com/repos/x/y", 404));
        assert!(!is_rate_limit("https://booth.pm/ja/items/1", 500));
    }

    // ── HTTP 失败归因（describe_failure / cf_mitigation / readable_hint）──

    /// 真实的 Cloudflare 挑战页片段（2026-10 从 booth.pm 抓到的响应体节选）。
    /// 注意开头那一大段是 CSP meta 与内联 CSS —— 正是「裸截 HTML」会浪费掉的预算。
    const CF_CHALLENGE_BODY: &str = r#"<!DOCTYPE html><html lang="en-US"><head><title>Just a moment...</title><meta http-equiv="Content-Type" content="text/html; charset=UTF-8"><style>*{box-sizing:border-box;margin:0;padding:0}html{line-height:1.15}</style><meta http-equiv="refresh" content="360"></head><body><div class="main-wrapper" role="main"><div class="main-content"><noscript><div class="h2"><span id="challenge-error-text">Enable JavaScript and cookies to continue</span></div></noscript></div></div><script nonce="x">(function(){window._cf_chl_opt = {cType: 'managed',cZone: 'booth.pm',cRay: 'a4f66f16ecb9b92b',};}());</script></body></html>"#;

    #[test]
    fn cf_challenge_detected_from_header() {
        // 响应头是最权威的判据，不受页面改版影响。
        let diag = "cf-mitigated: challenge\ncf-ray: a4f66f16ecb9b92b-NRT\n";
        assert_eq!(
            cf_mitigation(diag, "<html>whatever</html>"),
            Some(CfMitigation::Challenge)
        );
        // block 与 challenge 必须分开：前者重试无用。
        assert_eq!(
            cf_mitigation("cf-mitigated: block\n", ""),
            Some(CfMitigation::Block)
        );
    }

    #[test]
    fn cf_mitigation_header_is_case_insensitive() {
        assert_eq!(
            cf_mitigation("CF-Mitigated: Challenge\n", ""),
            Some(CfMitigation::Challenge)
        );
    }

    #[test]
    fn cf_challenge_detected_from_body_when_header_absent() {
        // 头被中间设备剥掉时，body 判据仍要能认出。
        assert_eq!(
            cf_mitigation("", CF_CHALLENGE_BODY),
            Some(CfMitigation::BodyLooksLikeChallenge)
        );
        // 正常页面不得误判
        assert_eq!(
            cf_mitigation(
                "",
                r#"<li data-product-id="1" data-product-name="x" data-product-price="0"></li>"#
            ),
            None
        );
    }

    /// **本函数存在的核心理由**：`<noscript>` 里的那句正是 CF 写给「无法执行 JS 的
    /// 客户端」的提示，而我们恰好就是这种客户端。裸截 HTML 前 N 字只会拿到 CSP 与 CSS。
    #[test]
    fn readable_hint_prefers_noscript_over_raw_html() {
        let h = readable_hint(CF_CHALLENGE_BODY.as_bytes(), 240);
        assert!(
            h.contains("Enable JavaScript and cookies to continue"),
            "应优先取 noscript 内的人话，实际：{h}"
        );
        assert!(
            !h.contains("box-sizing"),
            "不得把内联 CSS 当提示，实际：{h}"
        );
        assert!(
            !h.contains("_cf_chl_opt"),
            "不得把挑战脚本当提示，实际：{h}"
        );
    }

    #[test]
    fn readable_hint_falls_back_to_title_then_text() {
        // 无 noscript 时取 title
        let h = readable_hint(
            b"<html><head><title>Forbidden</title></head><body>x</body></html>",
            80,
        );
        assert_eq!(h, "Forbidden");
        // 无 title 时取可见文本（标签剥离）
        let h2 = readable_hint(
            b"<div><p>Access denied</p><script>var a=1;</script></div>",
            80,
        );
        assert_eq!(h2, "Access denied", "脚本内容不得进入可见文本");
    }

    #[test]
    fn readable_hint_truncates_by_chars_not_bytes() {
        // 多字节字符不得被切坏（chars 计数而非字节）
        let s = "あ".repeat(300);
        let h = readable_hint(s.as_bytes(), 10);
        assert_eq!(h.chars().count(), 11, "10 字 + 省略号");
        assert!(h.ends_with('…'));
    }

    #[test]
    fn describe_failure_carries_actionable_attribution() {
        let diag = "cf-mitigated: challenge\ncf-ray: a4f66f16ecb9b92b-NRT\nretry-after: 30\n";
        let msg = describe_failure(
            "search",
            403,
            "https://booth.pm/ja/search/x",
            CF_CHALLENGE_BODY.as_bytes(),
            diag,
        );
        // 必须能回答「谁的 403」「该怎么办」「找谁」
        assert!(msg.contains("状态码 403"), "{msg}");
        assert!(msg.contains("Cloudflare"), "应识别出是 CF：{msg}");
        assert!(msg.contains("人机校验"), "应说明性质：{msg}");
        assert!(
            msg.contains("cf-ray=a4f66f16ecb9b92b-NRT"),
            "应带追踪号：{msg}"
        );
        assert!(msg.contains("retry-after=30s"), "应带等待建议：{msg}");
        assert!(
            msg.contains("Enable JavaScript and cookies to continue"),
            "应带站点原话：{msg}"
        );
        assert!(
            msg.contains("https://booth.pm/ja/search/x"),
            "应带 URL：{msg}"
        );
    }

    #[test]
    fn describe_failure_plain_403_without_cf_markers() {
        // 没有 CF 标记时不得谎称是 Cloudflare —— 归因必须保守。
        let msg = describe_failure("download", 403, "https://booth.pm/x", b"403 Forbidden", "");
        assert!(msg.contains("状态码 403"), "{msg}");
        assert!(!msg.contains("Cloudflare"), "无 CF 标记不得报 CF：{msg}");
        assert!(msg.contains("403 Forbidden"), "应带 body 摘要：{msg}");
    }

    #[test]
    fn describe_failure_handles_empty_body() {
        let msg = describe_failure("chunk 0-1023", 404, "", &[], "");
        assert!(msg.contains("状态码 404"), "{msg}");
        assert!(msg.contains("资源不存在"), "404 应给对应解读：{msg}");
        // 空 body 时不应出现空的「站点提示：」尾巴
        assert!(!msg.contains("站点提示"), "空 body 不应拼接提示段：{msg}");
    }
}
