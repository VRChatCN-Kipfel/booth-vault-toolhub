//! MCP 工具定义：download / organize / search / audit。
//!
//! 全部复用 engine crate，JSON 输入输出与 CLI 一致；
//! 限速策略（三端统一）在 engine::download 内部保证，此处不绕过。

use engine::config::{default_rate_limit_secs, load_config};
use engine::session::make_session;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

// ── download ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct DownloadParams {
    /// 散链链接或裸商品 ID（可多个）。
    #[serde(default)]
    items: Vec<String>,
    /// 店铺 URL 或子域名。
    #[serde(default)]
    shop: Option<String>,
    /// 输出根目录（默认读配置 download_root）。
    #[serde(default)]
    out: Option<String>,
    /// 最多处理 N 个商品（0 = 不限）。
    #[serde(default)]
    limit: usize,
    /// 只检查不实际下载。
    #[serde(default)]
    dry_run: bool,
    /// BOOTH 登录 Cookie（下载免费文件必需）。
    #[serde(default)]
    cookie: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DownloadResult {
    command: String,
    done: usize,
    failures: Vec<String>,
}

// ── organize ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct OrganizeParams {
    /// 本地压缩包路径（文件名含 7 位 ID）。
    archive: Vec<String>,
    /// 输出根目录（默认读配置 download_root）。
    #[serde(default)]
    out: Option<String>,
    /// 强制指定商品 ID（文件名无 ID 时用）。
    #[serde(default)]
    id: Option<String>,
    /// 只检查不实际移动/下载。
    #[serde(default)]
    dry_run: bool,
    /// BOOTH 登录 Cookie（补全商品页其他免费版本）。
    #[serde(default)]
    cookie: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct OrganizeResult {
    command: String,
    ok: usize,
    total: usize,
    failures: Vec<String>,
}

// ── search ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct SearchParams {
    /// 待整理的文件路径。
    files: Vec<String>,
    /// 归档根目录（默认读配置 download_root）。
    #[serde(default)]
    base_dir: Option<String>,
    /// 只搜索不实际整理。
    #[serde(default)]
    dry_run: bool,
    /// 复制而非移动（与 CLI 接口对齐，当前整理固定移动）。
    #[serde(default)]
    #[allow(dead_code)]
    keep: bool,
    /// 歧义也强制选最佳（与 CLI 接口对齐，当前 MCP 固定选最佳）。
    #[serde(default)]
    #[allow(dead_code)]
    auto: bool,
    /// 强制指定 BOOTH 商品 ID（跳过搜索）。
    #[serde(default)]
    id: Option<String>,
    /// BOOTH 登录 Cookie。
    #[serde(default)]
    cookie: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct SearchResult {
    command: String,
    matched: usize,
    total: usize,
    failures: Vec<String>,
}

// ── audit ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct AuditParams {
    /// 巡检根目录（默认读配置 download_root）。
    #[serde(default)]
    base: Option<String>,
    /// 只扫描不修复。
    #[serde(default)]
    dry_run: bool,
    /// 不自动修复。
    #[serde(default)]
    no_fix: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct AuditResult {
    command: String,
    total_dirs: usize,
    problem_dirs: usize,
    fixed: usize,
    no_cover: usize,
    failed: usize,
}

// ── 服务 ─────────────────────────────────────────────────────────

/// BOOTH MCP 服务。
#[derive(Clone)]
pub struct BoothServer;

impl BoothServer {
    pub fn new() -> Self {
        Self
    }
}

#[tool_router(server_handler)]
impl BoothServer {
    /// 下载 BOOTH 免费商品（散链/整店）。
    #[tool(
        description = "下载 BOOTH 免费商品：接受商品链接/裸 ID 或店铺，按分类归档到本地。免费文件下载需要 --cookie（BOOTH 免费文件也要登录）。"
    )]
    async fn download(&self, Parameters(params): Parameters<DownloadParams>) -> CallToolResult {
        let config = load_config();
        let out_root = match params.out.or(config.download_root.clone()) {
            Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => {
                return tool_error("未指定输出目录：用 out 或配置文件 download_root");
            }
        };
        let client = make_session(&config, params.cookie.as_deref());
        let rate_limit = config
            .rate_limit_secs
            .unwrap_or_else(default_rate_limit_secs);

        // 解析散链/裸 ID。
        let mut ids: Vec<String> = Vec::new();
        for blob in &params.items {
            for id in engine::id::parse_discrete(blob) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        let mut shop_id = params.shop.as_deref().map(engine::id::shop_subdomain);
        if let Some(s) = params.shop.as_deref() {
            let parsed = engine::id::parse_discrete(s);
            if !parsed.is_empty() {
                for id in parsed {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
                shop_id = None;
            }
        }

        if ids.is_empty() && shop_id.is_none() {
            return tool_error("提供店铺 URL/子域名，或用 items 提供商品链接/ID");
        }

        let mut done = 0usize;
        let mut failures: Vec<String> = Vec::new();

        if let Some(sub) = shop_id {
            match engine::search::crawl_item_ids(&client, &sub, rate_limit) {
                Ok(found) => {
                    for id in found {
                        if !ids.contains(&id) {
                            ids.push(id);
                        }
                    }
                }
                Err(e) => failures.push(format!("店铺翻页失败 {sub}: {e}")),
            }
        }

        for item_id in ids {
            if params.limit > 0 && done >= params.limit {
                break;
            }
            match download_one(&client, &out_root, &item_id, params.dry_run, rate_limit) {
                Ok(true) => done += 1,
                Ok(false) => {}
                Err(e) => failures.push(format!("{item_id}: {e}")),
            }
        }

        let result = DownloadResult {
            command: "download".to_string(),
            done,
            failures,
        };
        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    /// 按 ID 整理本地压缩包。
    #[tool(
        description = "整理本地 BOOTH 压缩包：从文件名提取 7 位商品 ID，获取元数据后归档到 <out>/<分类>/<ID>_<标题>/，并下载封面、设置文件夹图标、补全免费版本。"
    )]
    async fn organize(&self, Parameters(params): Parameters<OrganizeParams>) -> CallToolResult {
        let config = load_config();
        let out_root = match params.out.or(config.download_root.clone()) {
            Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => {
                return tool_error("未指定输出目录：用 out 或配置文件 download_root");
            }
        };
        let client = make_session(&config, params.cookie.as_deref());
        let opts = engine::organize::OrganizeOptions {
            out_root: &out_root,
            dry_run: params.dry_run,
            cookie: params.cookie.as_deref(),
        };
        let mut ok = 0usize;
        let mut failures: Vec<String> = Vec::new();
        for path_str in &params.archive {
            let path = std::path::PathBuf::from(path_str);
            if !path.is_file() {
                failures.push(format!("找不到文件: {path_str}"));
                continue;
            }
            let item_id = match params.id.as_deref().filter(|s| !s.is_empty()) {
                Some(id) => id.to_string(),
                None => {
                    let stem = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    engine::id::extract_id(&stem)
                }
            };
            if item_id.is_empty() {
                failures.push(format!(
                    "[{path_str}] 文件名中未找到 7 位数字 BOOTH ID，跳过（可用 id 指定）"
                ));
                continue;
            }
            let outcome =
                engine::organize::organize_archive(&client, &path, &item_id, &opts, icon_fn);
            if outcome.ok {
                ok += 1;
            } else {
                failures.push(format!("{path_str}: {}", outcome.message));
            }
        }
        let result = OrganizeResult {
            command: "organize".to_string(),
            ok,
            total: params.archive.len(),
            failures,
        };
        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    /// 按名搜索并整理本地文件。
    #[tool(
        description = "本地文件（无 ID）按名搜索 BOOTH：生成搜索候选 → 搜索 → 评分选优 → 整理归档。歧义时优先选最佳（auto=true）或报告候选。"
    )]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> CallToolResult {
        let config = load_config();
        let base = match params.base_dir.or(config.download_root.clone()) {
            Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => {
                return tool_error("未指定归档目录：用 base_dir 或配置文件 download_root");
            }
        };
        let client = make_session(&config, params.cookie.as_deref());
        let mut matched: Vec<String> = Vec::new();
        let mut failures: Vec<String> = Vec::new();

        for path_str in &params.files {
            let path = std::path::Path::new(path_str);
            let fname = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if params.dry_run {
                let candidates = engine::clean::sanitize_query(&fname);
                for q in candidates {
                    match engine::search::search_booth(&client, &q) {
                        Ok(results) => {
                            if let Some(it) = results.first() {
                                matched.push(it.id.clone());
                                break;
                            }
                        }
                        Err(e) => failures.push(format!("搜索失败 {q}: {e}")),
                    }
                }
                continue;
            }
            match process_search_file(&client, path, &base, params.id.as_deref()) {
                Ok(Some(id)) => matched.push(id),
                Ok(None) => {}
                Err(e) => failures.push(format!("{path_str}: {e}")),
            }
        }

        let result = SearchResult {
            command: "search".to_string(),
            matched: matched.len(),
            total: params.files.len(),
            failures,
        };
        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    /// 全库文件夹图标三件套巡检。
    #[tool(
        description = "巡检 BOOTH 归档库的文件夹图标三件套完整性（ico/desktop.ini/属性位），可选自动修复。"
    )]
    async fn audit(&self, Parameters(params): Parameters<AuditParams>) -> CallToolResult {
        let config = load_config();
        let base = match params.base.or(config.download_root.clone()) {
            Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => {
                return tool_error("未指定巡检根目录：用 base 或配置文件 download_root");
            }
        };
        if !base.is_dir() {
            return tool_error(&format!("FATAL: {} 不存在", base.display()));
        }
        let results = engine::audit::audit_tree(&base);
        let fix = !params.dry_run && !params.no_fix;
        let mut fixed = 0usize;
        let mut no_cover = 0usize;
        let mut failed = 0usize;
        let problem = results.iter().filter(|r| !r.issues.is_empty()).count();

        if fix {
            for r in results.iter().filter(|r| !r.issues.is_empty()) {
                let cover = r.dir.join("cover.jpg");
                match &r.suggested_fix {
                    Some(engine::audit::FixAction::Rewrite) if cover.is_file() => {
                        match icon_fn(&cover, &r.dir) {
                            Ok(()) => fixed += 1,
                            Err(e) => {
                                failed += 1;
                                let _ = e;
                            }
                        }
                    }
                    Some(engine::audit::FixAction::NeedsCover) => no_cover += 1,
                    _ => {}
                }
            }
        }

        let result = AuditResult {
            command: "audit".to_string(),
            total_dirs: results.len(),
            problem_dirs: problem,
            fixed,
            no_cover,
            failed,
        };
        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }
}

// ── 内部辅助 ─────────────────────────────────────────────────────

/// 工具级错误（调用方可见）。
fn tool_error(msg: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(msg.to_string())])
}

/// 单个商品下载处理（复用 CLI 逻辑）。
fn download_one(
    client: &reqwest::blocking::Client,
    out_root: &std::path::Path,
    item_id: &str,
    dry_run: bool,
    rate_limit: f64,
) -> Result<bool, String> {
    let item = engine::fetch::fetch_item(client, item_id)
        .map_err(|e| format!("获取商品元数据失败: {e}"))?;
    let files = engine::fetch::free_downloads(&item);
    if files.is_empty() {
        return Ok(false);
    }
    let title = if item.name.is_empty() {
        item_id.to_string()
    } else {
        item.name.clone()
    };
    let cat = if item.category.name.is_empty() {
        "その他"
    } else {
        item.category.name.as_str()
    };
    let group = engine::classify::classify(cat, "");
    let folder = out_root
        .join(engine::clean::sanitize(&group, 40))
        .join(format!("{item_id}_{}", engine::clean::sanitize(&title, 70)));
    if dry_run {
        return Ok(true);
    }
    std::fs::create_dir_all(&folder).map_err(|e| format!("建目录失败: {e}"))?;
    for (url, fname) in files {
        let dest = folder.join(engine::clean::sanitize(&fname, 120));
        if dest.exists() && !engine::cover::looks_html(&std::fs::read(&dest).unwrap_or_default()) {
            continue;
        }
        engine::download::download(client, &url, &dest, true, 0.0)
            .map_err(|e| format!("下载失败 {fname}: {e}"))?;
    }
    let thumb = engine::fetch::thumb_from_json(&item);
    let cover = folder.join("cover.jpg");
    if !thumb.is_empty()
        && !cover.exists()
        && let Err(e) = engine::cover::download_cover(client, &thumb, &folder)
    {
        let _ = e;
    }
    apply_icon(&cover, &folder);
    engine::download::sleep_rate_limit(rate_limit);
    Ok(true)
}

/// 按名搜索 + 评分 + 整理。
fn process_search_file(
    client: &reqwest::blocking::Client,
    path: &std::path::Path,
    base: &std::path::Path,
    force_id: Option<&str>,
) -> Result<Option<String>, String> {
    let fname = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let item = if let Some(id) = force_id.filter(|s| !s.is_empty()) {
        engine::fetch::fetch_item(client, id).map_err(|e| format!("指定 ID {id} 获取失败: {e}"))?
    } else {
        let candidates = engine::clean::sanitize_query(&fname);
        let mut best: Option<engine::score::Item> = None;
        for q in candidates {
            let results =
                engine::search::search_booth(client, &q).map_err(|e| format!("搜索失败: {e}"))?;
            let items: Vec<engine::score::Item> = results
                .iter()
                .map(|r| engine::score::Item {
                    id: r.id.clone(),
                    name: r.name.clone(),
                    price: r.price,
                })
                .collect();
            let (picked, _) = engine::score::score_and_pick(
                &q,
                &items,
                false,
                |id| canonical_name(client, id),
                None,
            );
            if let Some(p) = picked {
                best = Some(p.clone());
                break;
            }
        }
        match best {
            Some(it) => engine::fetch::fetch_item(client, &it.id)
                .map_err(|e| format!("获取元数据失败: {e}"))?,
            None => return Ok(None),
        }
    };
    let opts = engine::organize::OrganizeOptions {
        out_root: base,
        dry_run: false,
        cookie: None,
    };
    let outcome = engine::organize::organize_archive(client, path, &item.id, &opts, icon_fn);
    if !outcome.ok {
        return Err(outcome.message);
    }
    Ok(Some(item.id))
}

/// 规范名解析（score_and_pick 依赖注入）。
fn canonical_name(client: &reqwest::blocking::Client, id: &str) -> String {
    engine::fetch::fetch_item(client, id)
        .map(|i| i.name)
        .unwrap_or_default()
}

/// 图标注入：Windows 用 shell_win，其余平台 no-op。
fn icon_fn(cover: &std::path::Path, folder: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        shell_win::folder_icon::make_folder_icon(cover, folder).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (cover, folder);
        Ok(())
    }
}

/// 设置文件夹图标（Windows 用 shell_win；其余平台跳过）。
fn apply_icon(cover: &std::path::Path, folder: &std::path::Path) {
    #[cfg(windows)]
    {
        if cover.is_file() {
            let _ = shell_win::folder_icon::make_folder_icon(cover, folder);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (cover, folder);
    }
}
