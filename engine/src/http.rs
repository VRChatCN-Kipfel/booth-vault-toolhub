//! HTTP 请求重试：transport 错误指数退避（ConnectionError/Timeout 等价物）。
//! HTTP 状态码：404 等留给调用方判断；403/429（风控/限流）在此指数退避重试。

use std::time::Duration;

use reqwest::blocking::{Client, Response};

/// 最大重试次数。
pub const MAX_RETRIES: u32 = 3;

/// 触发限流退避的 HTTP 状态码（Cloudflare 风控 403 / GitHub 限流 429）。
pub const RATE_LIMIT_STATUSES: &[u16] = &[403, 429];

/// 通用指数退避重试循环（生产与测试共用）。
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
                    std::thread::sleep(Duration::from_secs(attempt as u64 * 2));
                    last_err = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }
    Err(last_err.expect("retry loop always sets err on exit"))
}

/// 请求执行器：对「传输层错误」指数退避重试。
///
/// 重试条件覆盖连接失败 / 超时 / 响应体截断（BOOTH 或代理偶发）。
/// reqwest 0.13 无 `is_transport`，用 `is_connect || is_timeout || is_body` 近似。
pub fn retry<F>(mut f: F) -> Result<Response, reqwest::Error>
where
    F: FnMut() -> Result<Response, reqwest::Error>,
{
    retry_loop(&mut f, |e| e.is_connect() || e.is_timeout() || e.is_body())
}

/// 便捷封装：GET + 指定 headers。
///
/// 对「传输层错误」与「403/429 限流」双重指数退避：
///   - 传输层错误（连接/超时/响应体截断）走 `is_connect || is_timeout || is_body`。
///   - HTTP 403/429（Cloudflare 风控 / GitHub 限流）按指数档退避，
///     优先尊重响应 `Retry-After` 头，最长不超过 `MAX_RETRIES` 次。
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
                if RATE_LIMIT_STATUSES.contains(&code) && attempt < MAX_RETRIES {
                    let wait = retry_after_secs(&resp).unwrap_or(u64::from(attempt) * 2);
                    std::thread::sleep(Duration::from_secs(wait));
                    continue;
                }
                return Ok(resp);
            }
            Err(e) => {
                if (e.is_connect() || e.is_timeout() || e.is_body()) && attempt < MAX_RETRIES {
                    std::thread::sleep(Duration::from_secs(u64::from(attempt) * 2));
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
}
