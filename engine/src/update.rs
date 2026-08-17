//! 工具自更新：Atom 主通道（无 API 配额）→ HTML latest → API。
//!
//! 只问三件事：本地是哪一版、远端最新是哪一版、要不要打开发布页。
//! GitHub `releases.atom` 形状固定，不引入 feed 解析库。

use std::time::Duration;

use fancy_regex::Regex;
use reqwest::blocking::Client;
use serde::Serialize;

use crate::config::{load_config, resolve_proxy};
use crate::http;
use crate::session::default_headers;
use crate::version::ver_gt;

const CHANNEL_TIMEOUT: Duration = Duration::from_secs(12);

pub const REPO_OWNER: &str = "VRChatCN-Kipfel";
pub const REPO_NAME: &str = "booth-vault-toolhub";

pub fn local_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReleaseSnapshot {
    pub tag: String,
    pub title: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub has_update: bool,
    pub local_version: String,
    pub remote_version: String,
    pub url: String,
    pub error: Option<String>,
    pub release_title: Option<String>,
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

fn releases_url() -> String {
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases")
}

fn releases_latest_url() -> String {
    format!("{}/latest", releases_url())
}

fn releases_atom_url() -> String {
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases.atom")
}

fn api_latest_url() -> String {
    format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest")
}

fn github_client(proxy: Option<String>) -> Client {
    let mut builder = Client::builder()
        .timeout(CHANNEL_TIMEOUT)
        .default_headers(default_headers());
    if let Some(p) = proxy
        && let Ok(pr) = reqwest::Proxy::all(&p)
    {
        builder = builder.proxy(pr);
    }
    builder.build().expect("update: reqwest client build")
}

fn extract_tag_from_url(url: &str) -> Option<String> {
    let re = Regex::new(r#"/releases/tag/([^/?#"' \s]+)"#).expect("valid regex");
    re.captures(url)
        .ok()
        .flatten()
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// 从 GitHub releases.atom 取最新一条（首个 entry）。
pub fn parse_atom_latest(feed: &str) -> Option<ReleaseSnapshot> {
    let start = feed.find("<entry>")?;
    let rest = &feed[start..];
    let end = rest.find("</entry>")?;
    let entry = &rest[..=end + "</entry>".len() - 1];
    let tag = extract_tag_from_url(entry)?;
    let title = xml_tag_text(entry, "title");
    let body = xml_tag_text(entry, "content");
    Some(ReleaseSnapshot { tag, title, body })
}

fn xml_tag_text(hay: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let i = hay.find(&open)?;
    let after = &hay[i + open.len()..];
    let gt = after.find('>')?;
    let inner = &after[gt + 1..];
    let j = inner.find(&close)?;
    let raw = inner[..j].trim();
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

/// GitHub release notes 常见标签 → 可读文本。
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut pending_nl = false;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(end) = html[i..].find('>') {
                let tag = html[i + 1..i + end].trim().to_ascii_lowercase();
                let name = tag
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if matches!(
                    name,
                    "p" | "div" | "br" | "li" | "ul" | "ol" | "h1" | "h2" | "h3" | "pre"
                ) {
                    if !pending_nl && !out.is_empty() {
                        out.push('\n');
                    }
                    pending_nl = true;
                    if name == "li" && !tag.starts_with('/') {
                        out.push_str("- ");
                        pending_nl = false;
                    }
                }
                i += end + 1;
                continue;
            }
            break;
        }
        pending_nl = false;
        let ch = html[i..].chars().next().unwrap();
        if html[i..].starts_with("&amp;") {
            out.push('&');
            i += 5;
        } else if html[i..].starts_with("&lt;") {
            out.push('<');
            i += 4;
        } else if html[i..].starts_with("&gt;") {
            out.push('>');
            i += 4;
        } else if html[i..].starts_with("&quot;") {
            out.push('"');
            i += 6;
        } else if html[i..].starts_with("&#39;") || html[i..].starts_with("&apos;") {
            out.push('\'');
            i += if html[i..].starts_with("&#39;") { 5 } else { 6 };
        } else if html[i..].starts_with("&nbsp;") {
            out.push(' ');
            i += 6;
        } else {
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn fetch_atom(client: &Client) -> Option<ReleaseSnapshot> {
    let resp = client.get(releases_atom_url()).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().ok()?;
    parse_atom_latest(&text)
}

fn fetch_html(client: &Client) -> Option<ReleaseSnapshot> {
    let resp = client.get(releases_latest_url()).send().ok()?;
    if let Some(tag) = extract_tag_from_url(resp.url().as_str()) {
        return Some(ReleaseSnapshot {
            tag,
            title: None,
            body: None,
        });
    }
    if resp.status().is_success()
        && let Ok(text) = resp.text()
        && let Some(tag) = extract_tag_from_url(&text)
    {
        return Some(ReleaseSnapshot {
            tag,
            title: None,
            body: None,
        });
    }
    None
}

fn fetch_api(client: &Client) -> Option<ReleaseSnapshot> {
    let resp = http::get(client, &api_latest_url(), default_headers()).ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json = resp.json::<serde_json::Value>().ok()?;
    let tag = json.get("tag_name")?.as_str()?.to_string();
    Some(ReleaseSnapshot {
        tag,
        title: json
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        body: json
            .get("body")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

fn probe(client: &Client) -> Option<ReleaseSnapshot> {
    fetch_atom(client)
        .or_else(|| fetch_html(client))
        .or_else(|| fetch_api(client))
}

fn build_info(snap: ReleaseSnapshot) -> UpdateInfo {
    let local = local_version();
    let body = snap
        .body
        .as_deref()
        .map(html_to_text)
        .filter(|s| !s.is_empty());
    UpdateInfo {
        has_update: ver_gt(&snap.tag, &local),
        local_version: local,
        remote_version: snap.tag,
        url: releases_url(),
        error: None,
        release_title: snap.title.filter(|s| !s.is_empty()),
        release_body: body,
    }
}

/// `use_proxy=true` 走配置/环境代理；该通道全失败再直连一次。
pub fn check_update(use_proxy: bool) -> UpdateInfo {
    let config = load_config();
    let proxy = if use_proxy {
        resolve_proxy(&config)
    } else {
        None
    };
    let proxied = github_client(proxy.clone());
    if let Some(snap) = probe(&proxied) {
        return build_info(snap);
    }
    if proxy.is_some() {
        let direct = github_client(None);
        if let Some(snap) = probe(&direct) {
            return build_info(snap);
        }
    }
    UpdateInfo {
        error: Some("网络失败或无法访问 GitHub".to_string()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_atom_github_shape() {
        let feed = r#"<?xml version="1.0"?>
<feed>
  <entry>
    <id>tag:github.com,2008:Repository/1/releases/9</id>
    <link rel="alternate" type="text/html" href="https://github.com/VRChatCN-Kipfel/booth-vault-toolhub/releases/tag/v1.4.0"/>
    <title>v1.4.0</title>
    <content type="html">&lt;p&gt;fix proxy&lt;/p&gt;&lt;ul&gt;&lt;li&gt;a&lt;/li&gt;&lt;/ul&gt;</content>
  </entry>
</feed>"#;
        let snap = parse_atom_latest(feed).unwrap();
        assert_eq!(snap.tag, "v1.4.0");
        assert_eq!(snap.title.as_deref(), Some("v1.4.0"));
        assert!(snap.body.as_deref().unwrap().contains("fix proxy"));
    }

    #[test]
    fn html_to_text_list() {
        let t = html_to_text("<p>hello</p><ul><li>one</li><li>two</li></ul>");
        assert!(t.contains("hello"));
        assert!(t.contains("- one"));
        assert!(t.contains("- two"));
    }

    #[test]
    fn info_default_has_no_error() {
        let i = UpdateInfo::default();
        assert!(i.error.is_none());
        assert_eq!(i.local_version, local_version());
    }
}
