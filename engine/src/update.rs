//! 工具自更新检查：拉取 GitHub 最新 release，比版本号，提示用户。
//!
//! 移植自 Python booth-keeper v1.3.2 `pages/updater.py` 的健壮性逻辑：
//!   - **Atom 法（主通道）**：GET `/releases.atom` 解析首个 `<entry>` 里的 release
//!     link（`.../releases/tag/{tag}`）取 tag。静态 feed，**不消耗 API 配额**
//!     （无 token 限流），走 `github.com` 而非 `api.github.com`（无 403 风险）。
//!   - **HTML 重定向法（兜底）**：GET `/releases/latest` 解析 302 `Location` 取 tag。
//!   - **API 法（兜底）**：`/releases/latest` JSON（可能被限流 403）。
//!   - 代理失败直连重试：配置/环境代理不可达时自动改直连。
//!   - 所有通道显式超时，防单个入口挂起拖死检查。
//!   - UA 伪装成浏览器（复用 `session::default_headers`），规避风控。
//!
//! 镜像站（gh-proxy 类）实测对 feed 路径全 403，仅对资产下载有效，故仅为
//! 下载阶段预留（见 `MIRRORS`），当前查版本阶段不发起镜像请求。
//!
//! 单一事实源：CLI / MCP / GUI 三端薄封装，业务逻辑全在 engine。

use fancy_regex::Regex;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use serde::Serialize;
use std::time::Duration;

use crate::config::{load_config, resolve_proxy};
use crate::http;
use crate::session::default_headers;

/// 每个网络通道的显式超时（防入口挂起，tcp/整体耗时上限）。
const CHANNEL_TIMEOUT: Duration = Duration::from_secs(20);

/// 工具所属 GitHub 仓库（自更新检查目标）。
pub const REPO_OWNER: &str = "VRChatCN-Kipfel";
pub const REPO_NAME: &str = "booth-vault-toolhub";

/// 内置公开镜像表（gh-proxy 类）。
///
/// 实测仅对 GitHub 资产/下载路径有效（feed 页面全 403），因此只用于未来
/// 「下载新版安装包」阶段，不在当前查版本阶段发起请求。地址为公开镜像，
/// 非个人代理；AGENTS.md 禁止硬编码个人代理地址，此为公开服务集群。
pub const MIRRORS: &[&str] = &[
    "https://ghproxy.net",
    "https://gh-proxy.com",
    "https://ghfast.top",
];

/// 本地工具版本（workspace 版本，三端一致）。
///
/// 仓库默认版本为 `0.0.0`（本地/未注入的 Nightly 构建），显示为 `Nightly Build`；
/// CI 已注入 tag/分支版本时（如 `1.2.3-f3ab2c1`）原样返回。
pub fn local_version() -> String {
    display_version(env!("CARGO_PKG_VERSION"))
}

/// 原始 Cargo 版本 → 显示版本映射（纯函数，供测试与本地版本展示）。
///
/// `0.0.0` 占位映射为 `Nightly Build`；其余版本原样返回。
pub fn display_version(raw: &str) -> String {
    if raw == "0.0.0" {
        "Nightly Build".to_string()
    } else {
        raw.to_string()
    }
}

/// 比较用本地版本（供 `ver_gt` 与远端 tag 数值比较）。
///
/// 区分构建来源：
///   - `BOOTH_BUILD_SOURCE=branch`（CI 分支注入）：版本串里的数字段（如 `master-deadbee` 的
///     `1234`）会与 release tag 撞档被误判，故比较时恒按 `0.0.0` 视为待更新。
///   - 其余（tag 直发 / 本地占位 `0.0.0`）：用真实 Cargo 版本比较。
///
/// 不能直接用 `local_version()`（`Nightly Build` 非数字，`parse_version` 得空元组，
/// 会导致 `ver_gt` 恒 false）。
fn cmp_version() -> String {
    if option_env!("BOOTH_BUILD_SOURCE") == Some("branch") {
        "0.0.0".to_string()
    } else {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

/// 单个通道抓取函数签名：返回 `(快照, reachable)`。
///
/// `reachable=true` 表示 HTTP 层已连通（无论是否解析到 release），供调用方区分
/// 「网络可达但仓库无 Release」与「网络不可达」。
type Fetcher = fn(&Client) -> (Option<ReleaseSnapshot>, bool);

/// 远端最新 release 快照（tag + 详情）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReleaseSnapshot {
    /// 版本 tag（如 "v1.2.3" 或 "1.2.3"）。
    pub tag: String,
    /// release 标题（Atom entry title 或 API `name`）。
    pub title: Option<String>,
    /// release 正文（HTML，Atom content 或 API `body`）。
    pub body: Option<String>,
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
    /// 远端 release 标题（可选，仅主通道能取到时提供）。
    pub release_title: Option<String>,
    /// 远端 release 正文（HTML，可选）。
    pub release_body: Option<String>,
}

impl Default for UpdateInfo {
    fn default() -> Self {
        Self {
            has_update: false,
            local_version: local_version(),
            remote_version: String::new(),
            url: releases_url(),
            error: None,
            release_title: None,
            release_body: None,
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

/// 拼接 Atom feed（主通道）URL。
fn releases_atom_url() -> String {
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases.atom")
}

/// 拼接 API latest release URL。
fn api_latest_url() -> String {
    format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest")
}

/// 构建 GitHub 专用 client：禁用重定向（捕获 302 Location）+ 浏览器 UA + 可选代理 + 显式超时。
///
/// 复用 `session::default_headers` 的 UA 与 `resolve_proxy` 的代理裁决（禁硬编码）。
/// `timeout` 保证单个入口挂起时不拖死更新检查。
fn github_client(proxy: Option<String>) -> Client {
    let mut builder = Client::builder()
        .redirect(Policy::none())
        .timeout(CHANNEL_TIMEOUT)
        .default_headers(default_headers());
    if let Some(p) = proxy
        && let Ok(pr) = reqwest::Proxy::all(&p)
    {
        builder = builder.proxy(pr);
    }
    builder.build().expect("update: reqwest client build")
}

/// 从 Atom feed 文本提取最新 release 快照（纯函数，供测试）。
///
/// 用 `feed-rs` 严格解析：取首个 `<entry>`，tag 从 alternate 链接（或 id）提取，
/// title/content 取 entry 的标题与正文（HTML）。成熟库处理命名空间/实体/畸形输入。
fn parse_atom_latest(feed_text: &str) -> Option<ReleaseSnapshot> {
    let feed = feed_rs::parser::parse(feed_text.as_bytes()).ok()?;
    let entry = feed.entries.into_iter().next()?;
    let tag = {
        // 优先从 release 链接提取（最可靠）；无则退到 id（`tag:...:releases/tag/v2.0.0`）。
        let mut tag = None;
        for link in entry.links {
            if let Some(href) = extract_tag_from_url(&link.href) {
                tag = Some(href);
                break;
            }
        }
        tag.or_else(|| extract_tag_from_url(&entry.id))?
    };
    Some(ReleaseSnapshot {
        tag,
        title: entry.title.map(|t| t.content),
        body: entry.content.and_then(|c| c.body),
    })
}

/// HTML → 可读纯文本（去除标签、解码常见实体、折叠空白）。
///
/// 供 CLI/GUI 展示 release 正文；仅处理 GitHub release notes 常见的简单 HTML
/// （`<li>`/`<p>`/`<a>`/`<code>` 等），不追求完整 HTML 语义。
pub fn html_to_text(html: &str) -> String {
    // 显式处理块级标签（换行/列表项加 `- `），行内标签（code/a/strong）随后由去标签步骤
    // 直接删除，不打乱文本。
    let spaced = html
        // 相邻列表项边界：一次换成 `\n- `（避免 `</li>\n<li>` 产生空行且保留 dash）
        .replace("</li><li>", "\n- ")
        .replace("</li>\n<li>", "\n- ")
        .replace("</li>\n <li>", "\n- ")
        .replace("<li>", "\n- ")
        .replace("<li ", "\n- ")
        .replace("</li>", "\n")
        // 块级闭合标签相邻时只留一个换行（如 `</p></li>`）
        .replace("</p></li>", "\n")
        .replace("</p>\n</li>", "\n")
        .replace("</p>", "\n")
        .replace("</ul>", "\n")
        .replace("</ol>", "\n")
        .replace("</pre>", "\n")
        .replace("</div>", "\n")
        .replace("</h1>", "\n")
        .replace("</h2>", "\n")
        .replace("</h3>", "\n")
        .replace("</h4>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    // 去标签（行内标签在此删除，不留空白行）
    let re = Regex::new(r"<[^>]*>").expect("valid regex");
    let cleaned = re.replace_all(&spaced, "").into_owned();
    // 折叠连续空行（含 `</ul>\n<p>` 产生的段落分隔），至多保留一个空行。
    let re_blank = Regex::new(r"\n{3,}").expect("valid regex");
    let cleaned = re_blank.replace_all(&cleaned, "\n\n").into_owned();
    // 解码常见实体
    let decoded = cleaned
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    // 折叠空白：行首尾 trim、合并连续空行、孤立 `-` 前缀并入下一行。
    let mut out = String::new();
    let mut prev_blank = false;
    let mut pending_dash = false;
    for line in decoded.lines() {
        let l = line.trim();
        if l == "-" {
            // GitHub 嵌套 `<li>\n<p>`：`- ` 独占一行，并入下一行。
            pending_dash = true;
            continue;
        }
        let l = if pending_dash {
            pending_dash = false;
            format!("- {l}")
        } else {
            l.to_string()
        };
        if l.is_empty() {
            if !prev_blank {
                out.push('\n');
            }
            prev_blank = true;
        } else {
            out.push_str(&l);
            out.push('\n');
            prev_blank = false;
        }
    }
    out.trim().to_string()
}

/// 从含 `/releases/tag/{tag}` 的 URL/`id` 提取 tag；不含则 None。
fn extract_tag_from_url(url: &str) -> Option<String> {
    let re = Regex::new(r"/releases/tag/([\w.\-]+)/?$").expect("valid regex");
    if let Ok(Some(c)) = re.captures(url)
        && let Some(m) = c.get(1)
    {
        return Some(m.as_str().to_string());
    }
    None
}

/// Atom 法（主通道）：解析 `/releases.atom` 首个 `<entry>` 的 release 快照。
///
/// 静态 feed 不消耗 API 配额，`github.com` 路径无 403 限流。
/// 返回 `(快照, reachable)`：`reachable=true` 表示 HTTP 已连通（无论是否解析到 tag），
/// 供调用方区分「网络可达但无 release」与「网络不可达」。该通道能取到 title/content。
fn fetch_atom_tag(client: &Client) -> (Option<ReleaseSnapshot>, bool) {
    let resp = match client
        .get(releases_atom_url())
        .headers(default_headers())
        .send()
    {
        Ok(r) => r,
        Err(_) => return (None, false),
    };
    if !resp.status().is_success() {
        return (None, true);
    }
    let text = match resp.text() {
        Ok(t) => t,
        Err(_) => return (None, true),
    };
    (parse_atom_latest(&text), true)
}

/// HTML 重定向法：GET `/releases/latest`（302）→ `Location` 取 tag。
///
/// 部分网络直连 200 返回页面，从页面里抓 tag；返回 `(快照, reachable)`。
/// 该通道仅能取 tag，无 title/content。
fn fetch_html_tag(client: &Client) -> (Option<ReleaseSnapshot>, bool) {
    let resp = match client
        .get(releases_latest_url())
        .headers(default_headers())
        .send()
    {
        Ok(r) => r,
        Err(_) => return (None, false),
    };
    if resp.status().as_u16() == 302
        && let Some(loc) = resp.headers().get(reqwest::header::LOCATION)
        && let Ok(loc) = loc.to_str()
    {
        let re = Regex::new(r"/releases/tag/([^/]+)/?$").expect("valid regex");
        if let Ok(Some(c)) = re.captures(loc)
            && let Some(m) = c.get(1)
        {
            return (
                Some(ReleaseSnapshot {
                    tag: m.as_str().to_string(),
                    title: None,
                    body: None,
                }),
                true,
            );
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
            return (
                Some(ReleaseSnapshot {
                    tag: m.as_str().to_string(),
                    title: None,
                    body: None,
                }),
                true,
            );
        }
    }
    (None, true)
}

/// API 法（可能被限流 403）：解析 JSON `tag_name` / `name` / `body`。
///
/// 走 `http::get` 享受 403/429 指数退避兜底（不消耗配额失败时有意义）。
/// 返回 `(快照, reachable)`。该通道能取到 API 的 title/body。
fn fetch_api_tag(client: &Client) -> (Option<ReleaseSnapshot>, bool) {
    let resp = match http::get(client, &api_latest_url(), default_headers()) {
        Ok(r) => r,
        Err(_) => return (None, false),
    };
    if !resp.status().is_success() {
        return (None, true);
    }
    let json = match resp.json::<serde_json::Value>() {
        Ok(j) => j,
        Err(_) => return (None, true),
    };
    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let snapshot = tag.map(|tag| ReleaseSnapshot {
        title: json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        body: json
            .get("body")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        tag,
    });
    (snapshot, true)
}

/// 主检查：Atom（主，无配额）→ HTML 重定向（兜底）→ API（兜底）→ 代理失败直连重试。
///
/// `use_proxy=true` 时优先走配置/环境代理；若代理通道全部失败，自动改直连重试。
/// 区分两种失败：
///   - 所有通道 HTTP 层都无法连通（传输/连接失败）→ 网络问题。
///   - 至少一个通道连通但解析不到 tag → 仓库尚无 Release 或 tag 格式不匹配。
///
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

    // 依次尝试候选 client（代理优先、直连兜底），避免重复调同一 client。
    // `proxied` 在 `use_proxy=false` 时与 `direct` 同为无代理 client，去重后只发一次。
    let mut clients = Vec::new();
    clients.push(&proxied);
    if proxy.is_some() {
        clients.push(&direct);
    }

    // 每通道按 Atom → HTML → API 顺序尝试，任一命中即返回（无 API 配额）。
    let chan_candidates: Vec<Fetcher> = vec![fetch_atom_tag, fetch_html_tag, fetch_api_tag];
    let mut any_reachable = false;
    for run in &chan_candidates {
        for c in &clients {
            let (snapshot, reachable) = run(c);
            any_reachable |= reachable;
            if let Some(snapshot) = snapshot {
                return build_info(snapshot, None);
            }
        }
    }

    let error = if any_reachable {
        Some("仓库暂无公开 Release，无法获取最新版本".to_string())
    } else {
        Some("网络失败或无法访问 GitHub".to_string())
    };
    UpdateInfo {
        error,
        ..Default::default()
    }
}

/// 用远端 release 快照组装结果（比版本号，复用 `crate::version` 单一实现）。
fn build_info(snapshot: ReleaseSnapshot, error: Option<String>) -> UpdateInfo {
    UpdateInfo {
        has_update: crate::version::ver_gt(&snapshot.tag, &cmp_version()),
        local_version: local_version(),
        remote_version: snapshot.tag,
        url: releases_url(),
        error,
        release_title: snapshot.title,
        release_body: snapshot.body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_default_has_no_error() {
        let i = UpdateInfo::default();
        assert!(i.error.is_none());
        assert_eq!(i.local_version, local_version());
    }

    #[test]
    fn display_version_maps_placeholder() {
        assert_eq!(display_version("0.0.0"), "Nightly Build");
        assert_eq!(display_version("1.2.3"), "1.2.3");
        assert_eq!(display_version("1.0.0-f3ab2c1"), "1.0.0-f3ab2c1");
    }

    #[test]
    fn dev_placeholder_always_updatable() {
        // 占位 0.0.0 时，任意正式版远端 tag 都被判为可更新。
        assert!(crate::version::ver_gt("v0.0.1", "0.0.0"));
        assert!(crate::version::ver_gt("v1.0.0", "0.0.0"));
        // 注入版（1.2.3）高于远端时才不再提示。
        assert!(!crate::version::ver_gt("v1.0.0", "1.2.3-f3ab2c1"));
    }

    #[test]
    fn atom_parses_first_entry_tag() {
        let feed = r#"<?xml version="1.0"?>
<feed>
  <entry>
    <id>tag:github.com,2008:Repository/1/v2.0.0</id>
    <link rel="alternate" type="text/html" href="https://github.com/o/r/releases/tag/v2.0.0"/>
    <title>v2.0.0: update</title>
    <content type="html">&lt;ul&gt;&lt;li&gt;fix bug A&lt;/li&gt;&lt;/ul&gt;</content>
  </entry>
  <entry>
    <link rel="alternate" type="text/html" href="https://github.com/o/r/releases/tag/v1.0.0"/>
  </entry>
</feed>"#;
        let s = parse_atom_latest(feed).expect("should parse");
        assert_eq!(s.tag, "v2.0.0");
        assert_eq!(s.title.as_deref(), Some("v2.0.0: update"));
        // feed-rs 解码 XML 实体后，content.body 为原始 HTML。
        assert_eq!(s.body.as_deref(), Some("<ul><li>fix bug A</li></ul>"));
    }

    #[test]
    fn atom_parses_prefixed_tag() {
        let feed = r#"<feed><entry>
<link rel="alternate" href="https://github.com/o/r/releases/tag/tauri-cef-v3.0.0-alpha.21"/>
</entry></feed>"#;
        let s = parse_atom_latest(feed).expect("should parse");
        assert_eq!(s.tag, "tauri-cef-v3.0.0-alpha.21");
        assert_eq!(s.title, None);
        assert_eq!(s.body, None);
    }

    #[test]
    fn atom_returns_none_on_empty_or_no_entry() {
        assert_eq!(parse_atom_latest(""), None);
        assert_eq!(parse_atom_latest("<feed></feed>"), None);
        assert_eq!(
            parse_atom_latest("<entry><title>no link</title></entry>"),
            None
        );
    }

    #[test]
    fn html_to_text_strips_tags_and_entities() {
        let html = "<ul><li>fix <code>bug</code></li><li>add &amp; feature</li></ul>";
        let out = html_to_text(html);
        assert_eq!(out, "- fix bug\n- add & feature");
    }

    #[test]
    fn html_to_text_handles_empty_and_plain() {
        assert_eq!(html_to_text(""), "");
        assert_eq!(html_to_text("  \n  "), "");
        assert_eq!(
            html_to_text("plain text with &amp; entity"),
            "plain text with & entity"
        );
    }

    #[test]
    fn html_to_text_github_style_nested_li() {
        // GitHub release notes 常见嵌套：<li> 内再含 <p>，换行标签。
        // 列表项之间紧凑（无空行），列表与后续段落间保留一个空行（markdown 语义）。
        let html = "<ul>\n<li>\n<p>windows fix</p>\n</li>\n</ul>\n<p>only let the parent</p>";
        let out = html_to_text(html);
        assert_eq!(out, "- windows fix\n\nonly let the parent");
    }

    #[test]
    fn html_to_text_multiple_items_no_inner_blank() {
        let html = "<ul><li>one</li><li>two</li><li>three</li></ul>";
        assert_eq!(html_to_text(html), "- one\n- two\n- three");
    }

    #[test]
    fn mirrors_only_for_download() {
        // 镜像表存在且非空（为下载阶段预留）；查版本阶段不发起镜像请求。
        assert!(!MIRRORS.is_empty());
    }
}
