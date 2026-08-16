//! 工具自更新检查：拉取 GitHub latest release，比版本号，提示用户。
//!
//! 移植自 Python booth-keeper v1.3.2 `pages/updater.py` 的健壮性逻辑：
//!   - HTML 重定向法：GET `/releases/latest` 解析 302 `Location` 取 tag，
//!     不消耗 GitHub API 配额（无 token 限流 60/h）。
//!   - API 兜底：`/releases/latest` JSON（可能被限流 403）。
//!   - 403/429 退避：API 通道走 `http::get` 指数退避。
//!   - 代理失败直连重试：配置/环境代理不可达时自动改直连。
//!   - UA 伪装成浏览器（复用 `session::default_headers`），规避风控。
//!
//! 单一事实源：CLI / MCP / GUI 三端薄封装，业务逻辑全在 engine。

use fancy_regex::Regex;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use serde::Serialize;

use crate::config::{load_config, resolve_proxy};
use crate::http;
use crate::session::default_headers;

/// 工具所属 GitHub 仓库（自更新检查目标）。
pub const REPO_OWNER: &str = "VRChatCN-Kipfel";
pub const REPO_NAME: &str = "booth-vault-toolhub";

/// 本地工具版本（workspace 版本，三端一致）。
pub fn local_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 更新检查结果。
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    /// 是否存在新版本。
    pub has_update: bool,
    /// 本地版本号（如 "0.1.0"）。
    pub local_version: String,
    /// 远端最新版本号（获取失败时为空串）。
    pub remote_version: String,
    /// 最新 release 页面（去下载用）。
    pub url: String,
    /// 错误信息；成功为 None。
    pub error: Option<String>,
}

impl Default for UpdateInfo {
    fn default() -> Self {
        Self {
            has_update: false,
            local_version: local_version(),
            remote_version: String::new(),
            url: releases_url(),
            error: None,
        }
    }
}

/// 拼接 releases 页面 URL。
fn releases_url() -> String {
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases")
}

/// 拼接 `/releases/latest`（HTML 重定向入口）URL。
fn releases_latest_url() -> String {
    format!("{}/latest", releases_url())
}

/// 拼接 API latest release URL。
fn api_latest_url() -> String {
    format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest")
}

/// 构建 GitHub 专用 client：禁用重定向（捕获 302 Location）+ 浏览器 UA + 可选代理。
///
/// 复用 `session::default_headers` 的 UA 与 `resolve_proxy` 的代理裁决（禁硬编码）。
fn github_client(proxy: Option<String>) -> Client {
    let mut builder = Client::builder()
        .redirect(Policy::none())
        .default_headers(default_headers());
    if let Some(p) = proxy
        && let Ok(pr) = reqwest::Proxy::all(&p)
    {
        builder = builder.proxy(pr);
    }
    builder.build().expect("update: reqwest client build")
}

/// 解析版本号 `'1.0.1'` → `[1,0,1]`；兼容 `v1.0.1` / `1.0.1-rc`。
pub fn parse_version(ver: &str) -> Vec<u32> {
    let re = Regex::new(r"v?(\d+(?:\.\d+)*)").expect("valid regex");
    match re.captures(ver) {
        Ok(Some(c)) => c
            .get(1)
            .map(|m| {
                m.as_str()
                    .split('.')
                    .filter_map(|x| x.parse::<u32>().ok())
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// 远端版本是否严格新于本地（`remote > local`，补零等价）。
fn is_newer(local: &[u32], remote: &[u32]) -> bool {
    if remote.is_empty() {
        return false;
    }
    let n = local.len().max(remote.len());
    for i in 0..n {
        let l = local.get(i).copied().unwrap_or(0);
        let r = remote.get(i).copied().unwrap_or(0);
        if r != l {
            return r > l;
        }
    }
    false
}

/// HTML 重定向法：GET `/releases/latest`（302）→ `Location` 取 tag。
///
/// 部分网络直连 200 返回页面，从页面里抓 tag；全部失败返回 None。
fn fetch_html_tag(client: &Client) -> Option<String> {
    let resp = client
        .get(releases_latest_url())
        .headers(default_headers())
        .send()
        .ok()?;
    if resp.status().as_u16() == 302
        && let Some(loc) = resp.headers().get(reqwest::header::LOCATION)
        && let Ok(loc) = loc.to_str()
    {
        let re = Regex::new(r"/releases/tag/([^/]+)/?$").expect("valid regex");
        if let Ok(Some(c)) = re.captures(loc)
            && let Some(m) = c.get(1)
        {
            return Some(m.as_str().to_string());
        }
    }
    // 直连 200：从页面 HTML 抓 tag
    if resp.status().is_success()
        && let Ok(text) = resp.text()
    {
        let re = Regex::new(r"/releases/tag/(v?[\d.]+)").expect("valid regex");
        if let Ok(Some(c)) = re.captures(&text)
            && let Some(m) = c.get(1)
        {
            return Some(m.as_str().to_string());
        }
    }
    None
}

/// API 法（可能被限流 403）：解析 JSON `tag_name`。
///
/// 走 `http::get` 享受 403/429 指数退避兜底（不消耗配额失败时有意义）。
fn fetch_api_tag(client: &Client) -> Option<String> {
    let resp = http::get(client, &api_latest_url(), default_headers()).ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json = resp.json::<serde_json::Value>().ok()?;
    json.get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 主检查：HTML 重定向（主）→ API（兜底）→ 代理失败直连重试。
///
/// `use_proxy=true` 时优先走配置/环境代理；若代理通道全部失败，自动改直连重试。
/// 全部失败返回 `error`，不抛异常。
pub fn check_update(use_proxy: bool) -> UpdateInfo {
    let config = load_config();
    let proxy = if use_proxy {
        resolve_proxy(&config)
    } else {
        None
    };
    // 复用 client：代理与直连各 1 个，避免每次调用建 4 个 reqwest Client。
    let proxied = github_client(proxy.clone());
    let direct = github_client(None);

    // 通道 1：HTML 重定向法（无配额）
    if let Some(tag) = fetch_html_tag(&proxied) {
        return build_info(tag, None);
    }
    // 通道 2：API（可能限流 403）
    if let Some(tag) = fetch_api_tag(&proxied) {
        return build_info(tag, None);
    }
    // 通道 3：代理失败 → 直连重试
    if proxy.is_some() {
        if let Some(tag) = fetch_html_tag(&direct) {
            return build_info(tag, None);
        }
        if let Some(tag) = fetch_api_tag(&direct) {
            return build_info(tag, None);
        }
    }

    UpdateInfo {
        error: Some("网络失败或无法访问 GitHub".to_string()),
        ..Default::default()
    }
}

/// 用远端 tag 组装结果（比版本号）。
fn build_info(remote_tag: String, error: Option<String>) -> UpdateInfo {
    let local = local_version();
    let remote_v = parse_version(&remote_tag);
    let local_v = parse_version(&local);
    UpdateInfo {
        has_update: is_newer(&local_v, &remote_v),
        local_version: local,
        remote_version: remote_tag,
        url: releases_url(),
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_basic() {
        assert_eq!(parse_version("1.0.1"), vec![1, 0, 1]);
        assert_eq!(parse_version("v1.3.2"), vec![1, 3, 2]);
        assert_eq!(parse_version("1.0.1-rc"), vec![1, 0, 1]);
        assert_eq!(parse_version("garbage"), Vec::<u32>::new());
    }

    #[test]
    fn is_newer_compares() {
        assert!(is_newer(&[1, 3, 1], &[1, 4, 0]));
        assert!(is_newer(&[1, 3, 2], &[1, 3, 3]));
        assert!(!is_newer(&[1, 4, 0], &[1, 4, 0]));
        assert!(!is_newer(&[2, 0, 0], &[1, 9, 9]));
        // 补零等价：1.0 == 1.0.0
        assert!(!is_newer(&[1, 0], &[1, 0, 0]));
        assert!(is_newer(&[1, 0], &[1, 0, 1]));
        assert!(!is_newer(&[1, 0, 0], &[]));
    }

    #[test]
    fn info_default_has_no_error() {
        let i = UpdateInfo::default();
        assert!(i.error.is_none());
        assert_eq!(i.local_version, local_version());
    }
}
