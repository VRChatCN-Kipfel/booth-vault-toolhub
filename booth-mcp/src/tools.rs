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
    /// 下载失败时保留 .part 供取证（默认读配置，缺省关闭；保留件不产生续传能力）。
    #[serde(default)]
    keep_failed: Option<bool>,
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
    /// 下载失败时保留 .part 供取证（默认读配置，缺省关闭；保留件不产生续传能力）。
    #[serde(default)]
    keep_failed: Option<bool>,
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
    #[serde(default)]
    files: Vec<String>,
    /// 通用检索词：给了就走纯检索（只搜索、不整理），与 `files` 互斥。
    #[serde(default)]
    query: Option<String>,
    /// 排序：new / popularity / liked / price_asc / price_desc（仅 query 模式）。
    #[serde(default)]
    sort: Option<String>,
    /// 商品类型：digital / physical（仅 query 模式）。
    #[serde(default, rename = "type")]
    kind: Option<String>,
    /// R-18：include / exclude / only（仅 query 模式）。
    #[serde(default)]
    adult: Option<String>,
    /// 标签过滤，可多个（仅 query 模式）。
    #[serde(default)]
    tag: Vec<String>,
    /// 分类 slug（日语原文，仅 query 模式）。
    #[serde(default)]
    category: Option<String>,
    /// 最低价（日元，仅 query 模式）。
    #[serde(default)]
    min_price: Option<i64>,
    /// 最高价（日元，仅 query 模式）。
    #[serde(default)]
    max_price: Option<i64>,
    /// 仅在售（仅 query 模式）。
    #[serde(default)]
    in_stock: bool,
    /// 排除词，可多个；本地后置过滤（仅 query 模式）。
    #[serde(default)]
    exclude: Vec<String>,
    /// 页码，从 1 起（仅 query 模式）。
    #[serde(default)]
    page: Option<u32>,
    /// 最多返回多少条（仅 query 模式）。
    #[serde(default)]
    limit: Option<usize>,
    /// 跳过内容缓存，强制联网（仅 query 模式）。
    #[serde(default)]
    no_cache: bool,
    /// description 加权：说明文中每个命中词的加分（缺省读配置，配置缺省 = 关闭）。
    #[serde(default)]
    desc_word_bonus: Option<i32>,
    /// description 加权：最多计几个命中词（缺省读配置，未给时按 3）。
    #[serde(default)]
    desc_max_words: Option<usize>,
    /// 归档根目录（默认读配置 download_root）。
    #[serde(default)]
    base_dir: Option<String>,
    /// 只搜索不实际整理。
    #[serde(default)]
    dry_run: bool,
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

// ── version_audit ──────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct VersionAuditParams {
    /// 巡检根目录（默认读配置 download_root）。
    #[serde(default)]
    base: Option<String>,
    /// 对可更新项补免费文件。
    #[serde(default)]
    fix: bool,
    /// BOOTH 登录 Cookie（fix 时需要）。
    #[serde(default)]
    cookie: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct CookieCheckParams {
    /// 直接给 Cookie 串（缺省读配置）。以下形态均自动识别，无需手动整理：
    /// `k=v; k=v` 整串、DevTools 表格整块复制（Application → Cookies）、
    /// `Cookie:` 请求头整行或整块 Request Headers、cURL 命令（Copy as cURL）、
    /// 每行一条的多行文本。分析类 cookie（_ga 等）自动剔除。
    #[serde(default)]
    cookie: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct VersionAuditResult {
    command: String,
    updateable: usize,
    fixed: usize,
    failures: Vec<String>,
}

// ── collections（watch / own）─────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct CollectionsParams {
    /// 清单类型：watch（关注，等降价/补货）或 own（已购，等作者更新）。
    kind: String,
    /// 操作：add / remove / list / check。
    op: String,
    /// 商品 ID 或链接（add / remove 必填）。
    #[serde(default)]
    ids: Vec<String>,
    /// 单次 check 最多检查多少项（默认 30）。
    #[serde(default)]
    limit: Option<usize>,
}

// ── library ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct InspectParams {
    /// 7 位 BOOTH 商品 ID。
    id: String,
    /// 摘录围绕的检索词，可多个；不给则取正文首部（此时 excerpt_hit=false）。
    #[serde(default)]
    term: Vec<String>,
    /// 摘录最多多少字符（缺省 200）。
    #[serde(default)]
    excerpt_chars: Option<usize>,
    /// BOOTH 登录 Cookie（缺省读配置）。
    #[serde(default)]
    cookie: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct LibraryParams {
    /// 归档根目录（默认读配置 download_root）。
    #[serde(default)]
    base: Option<String>,
    /// 忽略索引强制重扫（索引漏掉的变化用这条兜底）。
    #[serde(default)]
    refresh: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct LibraryResult {
    command: String,
    total: usize,
    items: Vec<LibraryRow>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct LibraryRow {
    id: String,
    name: String,
    category: String,
    path: String,
}

// ── preview ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct PreviewParams {
    /// 压缩包路径（zip / unitypackage，可多个）。
    archive: Vec<String>,
    /// 每个包最多列出多少条（默认 200，0 = 不限）。
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct PreviewResult {
    command: String,
    archives: Vec<ArchivePreviewRow>,
    failures: Vec<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct ArchivePreviewRow {
    path: String,
    format: String,
    total_entries: usize,
    truncated: bool,
    entries: Vec<EntryRow>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct EntryRow {
    name: String,
    size: u64,
    compressed_size: Option<u64>,
    method: String,
}

// ── update_check ───────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
struct UpdateCheckParams {
    /// 是否使用配置/环境代理（默认直连）。
    #[serde(default)]
    use_proxy: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct UpdateCheckResult {
    command: String,
    has_update: bool,
    local_version: String,
    remote_version: String,
    url: String,
    release_title: Option<String>,
    release_body: Option<String>,
    error: Option<String>,
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
        let client = make_session(
            &config,
            engine::config::resolve_cookie(params.cookie.as_deref(), &config).as_deref(),
        );
        let rate_limit = config
            .rate_limit_secs
            .unwrap_or_else(default_rate_limit_secs);
        // 入参优先，其次配置项，缺省关闭。
        let keep_failed = params
            .keep_failed
            .unwrap_or_else(|| engine::config::keep_failed_downloads(&config));

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
            match download_one(
                &client,
                &out_root,
                &item_id,
                params.dry_run,
                rate_limit,
                keep_failed,
            ) {
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
        let client = make_session(
            &config,
            engine::config::resolve_cookie(params.cookie.as_deref(), &config).as_deref(),
        );
        let keep_failed = params
            .keep_failed
            .unwrap_or_else(|| engine::config::keep_failed_downloads(&config));
        let opts = engine::organize::OrganizeOptions {
            out_root: &out_root,
            dry_run: params.dry_run,
            cookie: params.cookie.as_deref(),
            keep_failed_downloads: keep_failed,
        };
        let mut ok = 0usize;
        let mut failures: Vec<String> = Vec::new();
        for path_str in &params.archive {
            let path = std::path::PathBuf::from(path_str);
            // 与 CLI 共用 engine 的统一判据。此前 CLI/MCP 各写一份 is_file + 自有文案，
            // 同一目录在 CLI/MCP 报「找不到文件」、在 engine 报「不是文件」，用户无法据此判断该做什么。
            // GUI organize 无预检，由 engine 守卫兜底（见 organize_archive）。
            if let Err(msg) = engine::organize::check_input_file(&path) {
                failures.push(msg);
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

    /// 按名搜索并整理本地文件；或 query 模式通用检索。
    #[tool(
        description = "两种互斥模式：给 files 走「本地文件按名搜索 BOOTH → 评分选优 → 整理归档」；给 query 走纯检索（只搜不整理，支持 sort/type/adult/tag/category/价格/in_stock/exclude/page 过滤轴，走内容缓存，no_cache 可强制联网）。"
    )]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> CallToolResult {
        let config = load_config();
        // query 模式：纯检索，不要求归档目录（那是归档模式的参数）。
        if let Some(q) = params.query.clone() {
            if !params.files.is_empty() {
                return tool_error("files 与 query 互斥：要么按名归档，要么纯检索");
            }
            let client = make_session(
                &config,
                engine::config::resolve_cookie(params.cookie.as_deref(), &config).as_deref(),
            );
            let sq = engine::search::SearchQuery {
                query: q,
                sort: params.sort.clone(),
                kind: params.kind.clone(),
                adult: params.adult.clone(),
                tags: params.tag.clone(),
                category: params.category.clone(),
                min_price: params.min_price,
                max_price: params.max_price,
                in_stock: params.in_stock,
                exclude: params.exclude.clone(),
                page: params.page.unwrap_or(1),
            };
            let (items, parsed_count) =
                match engine::search::search_with_count(&client, &sq, params.no_cache) {
                    Ok(v) => v,
                    Err(e) => return tool_error(&format!("搜索失败: {e}")),
                };
            let limit = params.limit.unwrap_or(20);
            // 统一契约：与 CLI 共用 `engine::search::SearchOutput`，字段逐项一致。
            // 历史实现只输出 6 个字段（缺 price_text/brand/category），而 SKILL.md
            // 声称三端一致，agent 按文档取字段会拿到 null。
            let result = engine::search::into_output(&sq, &items, parsed_count, limit);
            let text = serde_json::to_string_pretty(&result).unwrap_or_default();
            return CallToolResult::success(vec![ContentBlock::text(text)]);
        }
        if params.files.is_empty() {
            return tool_error("未指定输入：要么给 files（按名归档），要么给 query（纯检索）");
        }
        let base = match params.base_dir.or(config.download_root.clone()) {
            Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => {
                return tool_error("未指定归档目录：用 base_dir 或配置文件 download_root");
            }
        };
        let client = make_session(
            &config,
            engine::config::resolve_cookie(params.cookie.as_deref(), &config).as_deref(),
        );
        let mut matched: Vec<String> = Vec::new();
        let mut failures: Vec<String> = Vec::new();

        let desc = match engine::config::desc_weight(
            params.desc_word_bonus,
            params.desc_max_words,
            &config,
        ) {
            Ok(w) => w,
            Err(e) => return tool_error(&e),
        };

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
            match process_search_file(
                &client,
                path,
                &base,
                params.id.as_deref(),
                params.cookie.as_deref(),
                engine::config::keep_failed_downloads(&config),
                desc,
            ) {
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
        let fix = !params.dry_run && !params.no_fix;
        let results = engine::audit::audit_tree_with_fix(&base, fix);
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

    /// 版本巡检：按免费文件名比对，可选补免费文件。
    #[tool(
        description = "巡检归档库免费文件版本：本地文件名 vs 远程免费文件名。fix=true 时补缺失免费文件（需 cookie）。付费无免费文件不进入结果，不自动下。"
    )]
    async fn version_audit(
        &self,
        Parameters(params): Parameters<VersionAuditParams>,
    ) -> CallToolResult {
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
        let client = make_session(
            &config,
            engine::config::resolve_cookie(params.cookie.as_deref(), &config).as_deref(),
        );
        let rate_limit = config
            .rate_limit_secs
            .unwrap_or_else(default_rate_limit_secs);
        let mut fetched = std::collections::HashMap::new();
        let rows = engine::audit::version_audit_with_progress(
            &base,
            rate_limit,
            |id| {
                let item = engine::fetch::fetch_item(&client, id).map_err(|e| e.to_string())?;
                fetched.insert(id.to_string(), item.clone());
                Ok(item)
            },
            |_| true,
        );
        let mut fixed = 0usize;
        let mut failures: Vec<String> = Vec::new();
        if params.fix {
            let has_cookie = params
                .cookie
                .as_deref()
                .is_some_and(|c| !c.trim().is_empty());
            if !has_cookie && rows.iter().any(|r| r.missing > 0) {
                failures.push(engine::download::cookie_required_msg().to_string());
            } else {
                for r in &rows {
                    let Some(item) = fetched.get(&r.id) else {
                        failures.push(format!("{}: 巡检结果缺失商品数据", r.id));
                        continue;
                    };
                    let (n, errs) = engine::organize::backfill_free_files(
                        &client,
                        &r.path,
                        item,
                        params.cookie.as_deref(),
                        engine::config::keep_failed_downloads(&config),
                    );
                    fixed += n;
                    failures.extend(errs.into_iter().map(|e| format!("{}: {e}", r.id)));
                }
            }
        }
        let result = VersionAuditResult {
            command: "version-audit".to_string(),
            updateable: rows.len(),
            fixed,
            failures,
        };
        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    /// 列出归档库存（只读）。
    #[tool(
        description = "列出归档库存：ID / 标题 / 类目 / 路径。类目取 ID 目录的父文件夹名，不联网。走持久化索引（按分类目录 mtime 增量），稳态秒开；refresh=true 强制重扫。"
    )]
    async fn library(&self, Parameters(params): Parameters<LibraryParams>) -> CallToolResult {
        let config = load_config();
        let base = match params.base.or(config.download_root.clone()) {
            Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => {
                return tool_error("未指定归档根目录：用 base 或配置文件 download_root");
            }
        };
        if !base.is_dir() {
            return tool_error(&format!("FATAL: {} 不存在", base.display()));
        }
        // 走持久化索引：稳态下只 stat 各分类目录，不再逐商品扫一遍。
        let cache = engine::config::library_cache_path();
        let items = engine::audit::list_library_cached(&base, cache.as_deref(), params.refresh);
        let result = LibraryResult {
            command: "library".to_string(),
            total: items.len(),
            items: items
                .into_iter()
                .map(|i| LibraryRow {
                    id: i.id,
                    name: i.name,
                    category: i.category,
                    path: i.path.display().to_string(),
                })
                .collect(),
        };
        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    /// 关注/已购清单：add / remove / list / check。
    #[tool(
        description = "关注（watch，等降价/补货）与已购（own，等作者更新）清单：op=add/remove/list/check。check 逐项拉详情比对快照，出站遵守共享限速，单轮受 limit（默认 30）约束。个人数据只存本机。"
    )]
    async fn collections(
        &self,
        Parameters(params): Parameters<CollectionsParams>,
    ) -> CallToolResult {
        let config = load_config();
        let kind = match engine::collections::Kind::parse(&params.kind) {
            Some(k) => k,
            None => {
                return tool_error(&format!("未知 kind：{}（支持 watch / own）", params.kind));
            }
        };
        let Some(path) = engine::collections::lists_path() else {
            return tool_error("无法确定清单路径（用户配置目录不可用）");
        };
        let mut lists = engine::collections::load(&path);
        let op = params.op.as_str();

        match op {
            "add" => {
                if params.ids.is_empty() {
                    return tool_error("add 需要至少一个商品 ID 或链接");
                }
                let client = make_session(&config, None);
                let mut added: Vec<String> = Vec::new();
                let mut failed: Vec<String> = Vec::new();
                for raw in &params.ids {
                    let Some(id) = normalize_item_id(raw) else {
                        failed.push(format!("{raw}: 无法解析出商品 ID"));
                        continue;
                    };
                    match engine::fetch::fetch_item(&client, &id) {
                        Ok(item) => {
                            lists.put(kind, engine::collections::snapshot_from(&item, &id));
                            added.push(id);
                        }
                        Err(e) => failed.push(format!("{id}: {e}")),
                    }
                }
                engine::collections::save(&path, &lists);
                let out = serde_json::json!({
                    "command": kind.as_str(), "op": "add",
                    "added": added, "failed": failed, "count": lists.len(kind),
                });
                CallToolResult::success(vec![ContentBlock::text(
                    serde_json::to_string_pretty(&out).unwrap_or_default(),
                )])
            }
            "remove" => {
                if params.ids.is_empty() {
                    return tool_error("remove 需要至少一个商品 ID 或链接");
                }
                let mut removed: Vec<String> = Vec::new();
                for raw in &params.ids {
                    if let Some(id) = normalize_item_id(raw)
                        && lists.remove(kind, &id)
                    {
                        removed.push(id);
                    }
                }
                engine::collections::save(&path, &lists);
                let out = serde_json::json!({
                    "command": kind.as_str(), "op": "remove",
                    "removed": removed, "count": lists.len(kind),
                });
                CallToolResult::success(vec![ContentBlock::text(
                    serde_json::to_string_pretty(&out).unwrap_or_default(),
                )])
            }
            "list" => {
                let items: Vec<_> = lists
                    .of(kind)
                    .values()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id, "name": s.name, "price": s.price,
                            "is_sold_out": s.is_sold_out, "checked_at": s.checked_at,
                        })
                    })
                    .collect();
                let out = serde_json::json!({
                    "command": kind.as_str(), "op": "list",
                    "count": items.len(), "items": items,
                });
                CallToolResult::success(vec![ContentBlock::text(
                    serde_json::to_string_pretty(&out).unwrap_or_default(),
                )])
            }
            "check" => {
                let limit = params.limit.unwrap_or(30);
                let ids: Vec<String> = lists.of(kind).keys().take(limit).cloned().collect();
                if ids.is_empty() {
                    let out = serde_json::json!({
                        "command": kind.as_str(), "op": "check",
                        "checked": 0, "changed": [],
                    });
                    return CallToolResult::success(vec![ContentBlock::text(
                        serde_json::to_string_pretty(&out).unwrap_or_default(),
                    )]);
                }
                let client = make_session(&config, None);
                let rate = config
                    .rate_limit_secs
                    .unwrap_or_else(engine::config::default_rate_limit_secs);
                let mut changed: Vec<serde_json::Value> = Vec::new();
                let mut checked = 0usize;
                let mut failures: Vec<String> = Vec::new();
                for (i, id) in ids.iter().enumerate() {
                    if i > 0 {
                        engine::download::sleep_rate_limit(rate);
                    }
                    let Some(old) = lists.get(kind, id).cloned() else {
                        continue;
                    };
                    match engine::fetch::fetch_item(&client, id) {
                        Ok(item) => {
                            checked += 1;
                            let mut fresh = engine::collections::snapshot_from(&item, id);
                            fresh.added_at = old.added_at;
                            fresh.checked_at = engine::collections::now_secs();
                            let changes = engine::collections::diff(&old, &fresh);
                            if !changes.is_empty() {
                                changed.push(serde_json::json!({
                                    "id": id, "name": fresh.name,
                                    "changes": changes.iter().map(|c| c.describe()).collect::<Vec<_>>(),
                                }));
                                lists.put(kind, fresh);
                            }
                        }
                        Err(e) => failures.push(format!("{id}: {e}")),
                    }
                }
                if !changed.is_empty() {
                    engine::collections::save(&path, &lists);
                }
                let out = serde_json::json!({
                    "command": kind.as_str(), "op": "check",
                    "checked": checked, "changed": changed, "failures": failures,
                });
                CallToolResult::success(vec![ContentBlock::text(
                    serde_json::to_string_pretty(&out).unwrap_or_default(),
                )])
            }
            other => tool_error(&format!(
                "未知操作 {other}（支持 add / remove / list / check）"
            )),
        }
    }

    /// 预览压缩包内条目（只读，不联网）。
    #[tool(
        description = "预览本地 zip / unitypackage 的条目清单（名称 / 大小 / 压缩后大小 / 压缩方法），只读不联网。用于确认包内容与商品是否对应，或核对归档文件是否完整。"
    )]
    async fn preview(&self, Parameters(params): Parameters<PreviewParams>) -> CallToolResult {
        let mut archives = Vec::new();
        let mut failures = Vec::new();
        let limit = params.limit.unwrap_or(200);
        for p in &params.archive {
            match engine::integrity::preview_archive(std::path::Path::new(p), limit) {
                Ok(pv) => archives.push(ArchivePreviewRow {
                    path: pv.path.display().to_string(),
                    format: pv.format,
                    total_entries: pv.total_entries,
                    truncated: pv.truncated,
                    entries: pv
                        .entries
                        .into_iter()
                        .map(|e| EntryRow {
                            name: e.name,
                            size: e.size,
                            compressed_size: e.compressed_size,
                            method: e.method,
                        })
                        .collect(),
                }),
                Err(e) => failures.push(format!("{p}: {e}")),
            }
        }
        let result = PreviewResult {
            command: "preview".to_string(),
            archives,
            failures,
        };
        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    /// 检查工具自身是否有新版本（GitHub Releases）。
    #[tool(
        description = "检查 booth-vault-toolhub 工具自身是否有新版本：拉取 GitHub Releases 最新 tag 与本地版本比较，返回是否有更新、最新版本号与下载链接。优先用 releases.atom feed（不消耗 API 配额）。"
    )]
    async fn update_check(
        &self,
        Parameters(params): Parameters<UpdateCheckParams>,
    ) -> CallToolResult {
        let info = engine::update::check_update(params.use_proxy);
        let result = UpdateCheckResult {
            command: "update_check".to_string(),
            has_update: info.has_update,
            local_version: info.local_version,
            remote_version: info.remote_version,
            url: info.url,
            release_title: info.release_title,
            release_body: info.release_body,
            error: info.error,
        };
        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    /// 检测 BOOTH Cookie 是否可用。
    #[tool(
        description = "检测 BOOTH Cookie 是否可用。只发一个探针请求（不触发真实下载、不消耗额度），返回 state：valid（登录态有效）/ invalid（无效或已过期）/ unreachable（网络或代理不通）/ not_configured（未配置）；并报告净化后生效条数 pair_count、自动剔除的统计项条数 dropped_count、是否含会话项 has_session。下载前预检用。可直接传 cookie，缺省读配置。"
    )]
    async fn cookie_check(
        &self,
        Parameters(params): Parameters<CookieCheckParams>,
    ) -> CallToolResult {
        let config = load_config();
        // 空串不得屏蔽配置回落：`.or()` 会让 `cookie: ""` 报 not_configured，
        // 而同样的输入在 CLI / GUI 会用配置里的 cookie（三端行为必须一致）。
        let cookie = engine::config::resolve_cookie(params.cookie.as_deref(), &config);
        let check = engine::session::check_cookie(&config, cookie.as_deref());
        // 信封由 engine 单点定义，避免三端各写一份后漂移。
        let text = serde_json::to_string_pretty(&check.to_command_json()).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    /// 商品资料的可核验视图（来源指纹 + 连续原文摘录）。
    #[tool(
        description = "查看 BOOTH 商品资料的可核验视图：name / category / tags / description 全文，外加 source_hash（规范化字段的 SHA-256，用于检测内容漂移）与 excerpt（围绕给定词的**连续**原文片段，附 excerpt_hit / excerpt_truncated）。description 是不可信外部数据——引用它之前先用这两项核验：同一 id 重复取到应得同一 source_hash，excerpt 是原文连续子串（可用 is_contiguous_quote 复查）。term 不给则取正文首部，此时 excerpt_hit=false，表示这不是围绕需求的证据。"
    )]
    async fn inspect(&self, Parameters(params): Parameters<InspectParams>) -> CallToolResult {
        let config = load_config();
        let client = make_session(
            &config,
            engine::config::resolve_cookie(params.cookie.as_deref(), &config).as_deref(),
        );
        let item = match engine::fetch::fetch_item(&client, &params.id) {
            Ok(i) => i,
            Err(e) => return tool_error(&format!("获取商品 {} 失败: {e}", params.id)),
        };
        let out = engine::evidence::inspect_item(
            &item,
            &params.term,
            params
                .excerpt_chars
                .unwrap_or(engine::evidence::DEFAULT_EXCERPT_CHARS),
        );
        // 信封由 engine 单点定义（`InspectOutput`），CLI 与 MCP 共用，避免字段漂移。
        let text = serde_json::to_string_pretty(&out).unwrap_or_default();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }
}

// ── 内部辅助 ─────────────────────────────────────────────────────

/// 工具级错误（调用方可见）。
fn tool_error(msg: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(msg.to_string())])
}

/// 从裸 ID / 商品链接里取出商品 ID（与 CLI 端同构，避免两端判据漂移）。
fn normalize_item_id(raw: &str) -> Option<String> {
    let s = raw.trim();
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) && s.len() >= 5 {
        return Some(s.to_string());
    }
    let after = s.split("/items/").nth(1)?;
    let id: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    (!id.is_empty()).then_some(id)
}

/// 单个商品下载处理（复用 CLI 逻辑）。
fn download_one(
    client: &reqwest::blocking::Client,
    out_root: &std::path::Path,
    item_id: &str,
    dry_run: bool,
    rate_limit: f64,
    keep_failed: bool,
) -> Result<bool, String> {
    let item = engine::fetch::fetch_item(client, item_id)
        .map_err(|e| format!("获取商品元数据失败: {e}"))?;
    let files = engine::fetch::free_downloads(&item);
    if files.is_empty() {
        return Ok(false);
    }
    let folder = engine::organize::target_folder(out_root, &item, item_id);
    if dry_run {
        return Ok(true);
    }
    std::fs::create_dir_all(&folder).map_err(|e| format!("建目录失败: {e}"))?;
    engine::organize::write_booth_txt(&folder, &item);
    for (url, fname) in files {
        let dest = folder.join(engine::clean::sanitize(&fname, 120));
        // 幂等：已存在且有效则跳过（判定与整理链共用同一实现，含完整性校验）。
        if engine::organize::is_locally_valid(&dest) {
            continue;
        }
        let dl_opts = engine::download::DownloadOptions {
            check_html: true,
            rate_limit: 0.0,
            keep_failed,
        };
        engine::download::download(client, &url, &dest, dl_opts).map_err(|e| {
            format!(
                "下载失败 {fname}: {}",
                engine::download::with_cookie_hint(e)
            )
        })?;
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
    cookie: Option<&str>,
    keep_failed: bool,
    desc: engine::score::DescWeight,
) -> Result<Option<String>, String> {
    let fname = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let item = if let Some(id) = force_id.filter(|s| !s.is_empty()) {
        engine::fetch::fetch_item(client, id).map_err(|e| format!("指定 ID {id} 获取失败: {e}"))?
    } else {
        // 与 CLI 共用 engine 的两轮解析：历史实现这里只判 `is_some()`，
        // 既没有名称命中校验也没有歧义校验，且完全不做 kana 变体轮——
        // 同一文件名在 CLI 能命中、在 MCP 落空。
        let candidates = engine::clean::sanitize_query(&fname);
        let names = engine::unitypackage::names_for_score(path);
        let outcome = engine::search::resolve_by_name(
            client,
            &candidates,
            names.as_deref(),
            engine::kana::MAX_VARIANTS,
            engine::synonyms::MAX_SYNONYMS,
            |id| canonical_info(client, id),
            desc,
        );
        match outcome.hit {
            Some(h) => engine::fetch::fetch_item(client, &h.item.id)
                .map_err(|e| format!("获取元数据失败: {e}"))?,
            None => return Ok(None),
        }
    };
    let opts = engine::organize::OrganizeOptions {
        out_root: base,
        dry_run: false,
        cookie,
        keep_failed_downloads: keep_failed,
    };
    let outcome = engine::organize::organize_archive(client, path, &item.id, &opts, icon_fn);
    if !outcome.ok {
        return Err(outcome.message);
    }
    Ok(Some(item.id))
}

/// 规范名与说明文（评分依赖注入）。
///
/// 与 CLI 端同构：取不到返回 `None`，不把失败压成空串——空串会让「规范名命中
/// +100」静默失效，并让同名判定退化成 `"" == ""`（不同商品被判成同名）。
fn canonical_info(
    client: &reqwest::blocking::Client,
    id: &str,
) -> Option<engine::search::CanonInfo> {
    engine::fetch::fetch_item(client, id)
        .map(|i| engine::search::CanonInfo {
            name: i.name,
            description: i.description,
        })
        .ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// description 加权的 MCP 入口必须真的出现在工具 schema 里。
    /// 只加字段不给默认值 / 写成私有字段时，agent 是看不见的——而 CLI 那边照样能跑，
    /// 于是「三端一致」在 MCP 上静默失效。
    #[test]
    fn search_schema_exposes_desc_weight_params() {
        let schema = schemars::schema_for!(SearchParams);
        let text = serde_json::to_string(&schema).expect("schema 应可序列化");
        // 防空转：先确认 schema 确实在列举字段名，否则下面的 contains 无区分力。
        assert!(
            text.contains("no_cache") && text.contains("files"),
            "schema 没有列出字段名，本测试失去区分力"
        );
        assert!(
            text.contains("desc_word_bonus"),
            "MCP search 工具缺少 desc_word_bonus"
        );
        assert!(
            text.contains("desc_max_words"),
            "MCP search 工具缺少 desc_max_words"
        );
    }

    /// `inspect` 工具的参数必须对外可见，且出参结构带指纹/摘录字段。
    ///
    /// 同 search 的理由：字段写成私有或漏 `#[serde(default)]` 时 agent 看不见，
    /// 而 CLI 照常能跑——「三端一致」会在 MCP 上静默失效。
    #[test]
    fn inspect_schema_and_output_are_exposed() {
        let p = schemars::schema_for!(InspectParams);
        let pt = serde_json::to_string(&p).expect("schema 应可序列化");
        // 防空转：确认 schema 确实列举了已有字段。
        assert!(pt.contains("\"id\""), "schema 未列出字段名，测试失去区分力");
        for f in ["term", "excerpt_chars", "cookie"] {
            assert!(pt.contains(f), "inspect 参数缺少 {f}");
        }
        // 出参信封由 engine 定义，这里确认它确实携带核验信息。
        let out = engine::evidence::InspectOutput {
            command: "inspect".into(),
            id: "1".into(),
            name: "n".into(),
            category: String::new(),
            tags: vec![],
            shop: String::new(),
            price_text: "0".into(),
            description: "d".into(),
            source_hash: "h".into(),
            excerpt: "e".into(),
            excerpt_hit: true,
            excerpt_truncated: false,
        };
        let ot = serde_json::to_string(&out).expect("出参应可序列化");
        for f in ["source_hash", "excerpt", "excerpt_hit", "excerpt_truncated"] {
            assert!(ot.contains(f), "InspectOutput 缺少 {f}");
        }
    }
}
