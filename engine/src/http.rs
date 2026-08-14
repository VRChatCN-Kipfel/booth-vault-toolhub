//! HTTP 请求重试：transport 错误指数退避（ConnectionError/Timeout 等价物）。
//! HTTP 状态码留给调用方判断（404 页可优雅处理而非重试）。

use std::time::Duration;

use reqwest::blocking::{Client, Response};

/// 最大重试次数。
pub const MAX_RETRIES: u32 = 3;

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
pub fn get(
    client: &Client,
    url: &str,
    headers: reqwest::header::HeaderMap,
) -> Result<Response, reqwest::Error> {
    retry(|| client.get(url).headers(headers.clone()).send())
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
