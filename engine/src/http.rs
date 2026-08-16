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

/// 便捷封装：GET + 指定 headers。
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
}
