//! booth CLI 命令编排：download / organize / search / audit / shell。
//!
//! 退出码语义（MCP 依赖）：
//!   0 = 全部成功
//!   1 = 有失败项（部分失败）
//!   2 = 致命错误（参数/配置/网络不可用）

use std::collections::HashMap;
use std::path::Path;

use crate::{Cli, Command, ShellCmd};
use engine::config::{AppConfig, default_rate_limit_secs, load_config, resolve_cookie};
use engine::session::make_session;

/// 运行命令，返回退出码。
pub fn run(cli: Cli) -> u8 {
    let config = load_config();
    // CLI flag 与配置项取或：任一声明即开启取证保留。
    let keep_failed = cli.keep_failed || engine::config::keep_failed_downloads(&config);
    match cli.command {
        Command::Download {
            shop,
            items,
            out,
            dry_run,
            limit,
            cookie,
        } => cmd_download(
            &config,
            shop.as_deref(),
            &items,
            out.as_deref(),
            dry_run,
            limit,
            cookie.as_deref(),
            keep_failed,
            cli.json,
        ),
        Command::Organize {
            archive,
            out,
            id,
            dry_run,
            cookie,
        } => cmd_organize(
            &config,
            &archive,
            out.as_deref(),
            id.as_deref(),
            dry_run,
            cookie.as_deref(),
            keep_failed,
            cli.json,
        ),
        Command::Search(args) => {
            // 两种模式互斥（argparse 已拦混用）：有 query 走纯检索，否则走按名归档。
            if let Some(q) = args.query {
                return cmd_search_query(
                    &config,
                    &engine::search::SearchQuery {
                        query: q,
                        sort: args.sort,
                        kind: args.kind,
                        adult: args.adult,
                        tags: args.tag,
                        category: args.category,
                        min_price: args.min_price,
                        max_price: args.max_price,
                        in_stock: args.in_stock,
                        exclude: args.exclude,
                        page: args.page,
                    },
                    args.limit,
                    args.no_cache,
                    args.cookie.as_deref(),
                    cli.json,
                );
            }
            let desc = match engine::config::desc_weight(
                args.desc_word_bonus,
                args.desc_max_words,
                &config,
            ) {
                Ok(w) => w,
                Err(e) => return fail(cli.json, &e),
            };
            cmd_search(
                &config,
                &args.files,
                args.base_dir.as_deref(),
                args.dry_run,
                args.id.as_deref(),
                args.cookie.as_deref(),
                keep_failed,
                desc,
                cli.json,
            )
        }
        Command::Audit {
            base,
            dry_run,
            no_fix,
        } => cmd_audit(&config, base.as_deref(), dry_run, no_fix, cli.json),
        Command::VersionAudit { base, fix, cookie } => cmd_version_audit(
            &config,
            base.as_deref(),
            fix,
            cookie.as_deref(),
            keep_failed,
            cli.json,
        ),
        Command::Library { base, refresh } => {
            cmd_library(&config, base.as_deref(), refresh, cli.json)
        }
        Command::Preview { archive, limit } => cmd_preview(&archive, limit, cli.json),
        Command::CookieCheck { cookie } => cmd_cookie_check(&config, cookie.as_deref(), cli.json),
        Command::Cache { command } => cmd_cache(command, cli.json),
        Command::Inspect {
            id,
            term,
            excerpt_chars,
            cookie,
        } => cmd_inspect(
            &config,
            &id,
            &term,
            excerpt_chars,
            cookie.as_deref(),
            cli.json,
        ),
        Command::Shell { command } => cmd_shell(command),
        Command::Watch(a) => {
            cmd_collections(&config, engine::collections::Kind::Watch, &a, cli.json)
        }
        Command::Own(a) => cmd_collections(&config, engine::collections::Kind::Own, &a, cli.json),
        Command::UpdateCheck { proxy } => cmd_update_check(proxy, cli.json),
    }
}

/// 解析配置里的下载根目录；未配置时返回 None（调用方用默认值）。
fn download_root<'a>(config: &'a AppConfig, arg: Option<&'a Path>) -> Option<&'a Path> {
    arg.or_else(|| config.download_root.as_deref().map(Path::new))
}

/// 商品资料可核验视图。
///
/// 存在的理由：`description` 是不可信外部数据，直接交给 agent 等于让它拿一段无从
/// 核验的文本当依据。本命令把正文与来源指纹、连续原文摘录一起给出，使上层能判断
/// 「这段资料有没有被改过」「引用的是不是原文里的连续片段」。
fn cmd_inspect(
    config: &AppConfig,
    id: &str,
    terms: &[String],
    excerpt_chars: usize,
    cookie: Option<&str>,
    json: bool,
) -> u8 {
    let cookie = resolve_cookie(cookie, config);
    let client = make_session(
        config,
        engine::config::resolve_cookie(cookie.as_deref(), config).as_deref(),
    );
    let item = match engine::fetch::fetch_item(&client, id) {
        Ok(i) => i,
        Err(e) => return fail(json, &format!("获取商品 {id} 失败: {e}")),
    };
    let out = engine::evidence::inspect_item(&item, terms, excerpt_chars);
    if json {
        match serde_json::to_string_pretty(&out) {
            Ok(s) => println!("{s}"),
            Err(e) => return fail(true, &format!("序列化失败: {e}")),
        }
    } else {
        println!("[{}] {}", out.id, out.name);
        println!("  店铺: {}", out.shop);
        println!(
            "  分类: {}",
            if out.category.is_empty() {
                "-"
            } else {
                &out.category
            }
        );
        println!("  价格: {}", out.price_text);
        if !out.tags.is_empty() {
            println!("  标签: {}", out.tags.join(", "));
        }
        println!("  指纹: {}", out.source_hash);
        println!(
            "  摘录{}: {}",
            if out.excerpt_hit {
                "（命中检索词）"
            } else {
                "（首部，非检索命中）"
            },
            out.excerpt.replace('\n', " ")
        );
        if out.excerpt_truncated {
            println!("  （原文更长，已截断到 {excerpt_chars} 字符；完整正文见 --json）");
        }
    }
    0
}

/// 缓存运维命令：`stats` / `clear`。
///
/// 为什么必须有入口：缓存键含登录态作用域，而「补填 Cookie 后同一 URL 仍命中旧
/// 匿名登录页直到 TTL 过期」是真实故障形态——没有清理入口，用户只能手动删文件。
fn cmd_cache(command: crate::CacheCmd, json: bool) -> u8 {
    let Some(path) = engine::config::http_cache_path() else {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "command": "cache", "ok": false,
                    "error": "无法确定缓存路径（用户配置目录不可用）",
                }))
                .unwrap()
            );
        } else {
            eprintln!("无法确定缓存路径（用户配置目录不可用）");
        }
        return 2;
    };
    let Some(cache) = engine::cache::Cache::open(&path) else {
        // 打不开不等于空缓存：如实报错，不伪造 entries=0。
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "command": "cache", "ok": false,
                    "error": format!("缓存库无法打开（可能被另一进程占用）: {}", path.display()),
                }))
                .unwrap()
            );
        } else {
            eprintln!("缓存库无法打开（可能被另一进程占用）: {}", path.display());
        }
        return 2;
    };
    match command {
        crate::CacheCmd::Stats => {
            let entries = cache.count();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "command": "cache", "op": "stats", "ok": true,
                        "entries": entries, "path": path.display().to_string(),
                    }))
                    .unwrap()
                );
            } else {
                println!("缓存条目 {entries} 条（{}）", path.display());
            }
            0
        }
        crate::CacheCmd::Clear => {
            cache.clear();
            let entries = cache.count();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "command": "cache", "op": "clear", "ok": true,
                        "cleared": true, "entries": entries,
                    }))
                    .unwrap()
                );
            } else {
                println!("缓存已清空（剩余 {entries} 条）");
            }
            0
        }
    }
}

/// 关注/已购清单命令（`watch` 与 `own` 共用实现，仅展示文案不同）。
///
/// `check` 会逐项拉详情比对快照：
///   - 出站计入共享限速（`rate_limit_secs`），与下载链路同一把尺子；
///   - 单轮受 `limit` 约束（默认 30），避免一次跑几十分钟；
///   - 只读不写盘，除非确有变动才回写快照（减少无谓 IO 与并发写窗口）。
fn cmd_collections(
    config: &AppConfig,
    kind: engine::collections::Kind,
    args: &crate::CollectionsArgs,
    json: bool,
) -> u8 {
    use engine::collections::Kind;
    let Some(path) = engine::collections::lists_path() else {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "command": kind.as_str(), "ok": false,
                    "error": "无法确定清单路径（用户配置目录不可用）",
                }))
                .unwrap()
            );
        } else {
            eprintln!("无法确定清单路径（用户配置目录不可用）");
        }
        return 2;
    };
    // 清单读取失败必须在**任何网络请求与写盘之前**中止：历史实现把损坏文件
    // 降级成空清单，随后 add 无条件覆盖，用户列表就此永久丢失。
    let mut lists = match engine::collections::load(&path) {
        Ok(l) => l,
        Err(e) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "command": kind.as_str(), "ok": false, "error": e.to_string(),
                    }))
                    .unwrap()
                );
            } else {
                eprintln!("{e}");
            }
            return 2;
        }
    };
    let op = args.op.as_str();

    // add / remove 不需要联网，先处理掉。
    if op == "add" {
        if args.ids.is_empty() {
            return fail(json, "add 需要至少一个商品 ID 或链接");
        }
        let client = make_session(config, None);
        // 先解析出合法 ID（无法解析的单独记失败），再交给 engine 的带限速批量接口。
        let mut failed: Vec<String> = Vec::new();
        let mut ids: Vec<String> = Vec::new();
        for raw in &args.ids {
            match normalize_item_id(raw) {
                Some(id) => ids.push(id),
                None => failed.push(format!("{raw}: 无法解析出商品 ID")),
            }
        }
        let rate = config
            .rate_limit_secs
            .unwrap_or_else(engine::config::default_rate_limit_secs);
        // 限速下沉到 engine（三端共享），CLI 不再自建循环 —— 历史缺陷是 add
        // 这条路径完全没有 sleep，check 有，两者不一致。
        let (added, fetch_failures) = engine::collections::add_paced(
            kind,
            &mut lists,
            &ids,
            || engine::download::sleep_rate_limit(rate),
            |id| engine::fetch::fetch_item(&client, id).map_err(|e| e.to_string()),
        );
        failed.extend(fetch_failures);
        // 写盘失败必须上抛：历史实现返回 ()、错误被吞，add 打印 added、退出 0，
        // 而磁盘上什么都没有。
        if let Err(e) = engine::collections::save(&path, &lists) {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "command": kind.as_str(), "op": "add", "ok": false,
                        "error": format!("清单写入失败: {e}"),
                    }))
                    .unwrap()
                );
            } else {
                eprintln!("清单写入失败: {e}");
            }
            return 2;
        }
        return report(
            json,
            kind.as_str(),
            "add",
            serde_json::json!({ "added": added, "failed": failed, "count": lists.len(kind) }),
        );
    }

    if op == "remove" {
        if args.ids.is_empty() {
            return fail(json, "remove 需要至少一个商品 ID 或链接");
        }
        let mut removed: Vec<String> = Vec::new();
        for raw in &args.ids {
            if let Some(id) = normalize_item_id(raw)
                && lists.remove(kind, &id)
            {
                removed.push(id);
            }
        }
        if let Err(e) = engine::collections::save(&path, &lists) {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "command": kind.as_str(), "op": "remove", "ok": false,
                        "error": format!("清单写入失败: {e}"),
                    }))
                    .unwrap()
                );
            } else {
                eprintln!("清单写入失败: {e}");
            }
            return 2;
        }
        return report(
            json,
            kind.as_str(),
            "remove",
            serde_json::json!({ "removed": removed, "count": lists.len(kind) }),
        );
    }

    if op == "list" {
        let entries: Vec<_> = lists
            .of(kind)
            .values()
            .map(|s| {
                serde_json::json!({
                    "id": s.id, "name": s.name, "price": s.price,
                    "is_sold_out": s.is_sold_out, "checked_at": s.checked_at,
                })
            })
            .collect();
        let summary = serde_json::json!({ "count": entries.len(), "items": entries });
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "command": kind.as_str(), "op": "list", "ok": true, "data": summary,
                }))
                .unwrap()
            );
        } else if entries.is_empty() {
            println!("清单为空");
        } else {
            for s in lists.of(kind).values() {
                let mark = if s.is_sold_out { " [售罄]" } else { "" };
                println!("  [{}] {}  ¥{}{mark}", s.id, s.name, s.price);
            }
        }
        return 0;
    }

    if op == "check" {
        // 轮换而非字典序截断：历史实现 `keys().take(limit)` 让第 N+1 条永远轮不到，
        // 且 `limit=0` 取 0 条却退出 0（同仓 download/preview 的 0 都表示不限）。
        let ids: Vec<String> = engine::collections::pick_batch(&lists, kind, args.limit);
        if ids.is_empty() {
            return report(
                json,
                kind.as_str(),
                "check",
                serde_json::json!({ "checked": 0, "changed": [] }),
            );
        }
        let client = make_session(config, None);
        let rate = config
            .rate_limit_secs
            .unwrap_or_else(engine::config::default_rate_limit_secs);
        let mut changed: Vec<serde_json::Value> = Vec::new();
        let mut checked = 0usize;
        let mut failures: Vec<String> = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                // 与下载链路同一把限速尺子：BOOTH 风控按请求数计。
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
                            "id": id,
                            "name": fresh.name,
                            "changes": changes.iter().map(|c| c.describe()).collect::<Vec<_>>(),
                        }));
                        // 只有真有变动才更新快照：否则每次 check 都写盘，
                        // 徒增与 GUI 并发的写窗口。
                        lists.put(kind, fresh);
                    }
                }
                Err(e) => failures.push(format!("{id}: {e}")),
            }
        }
        if !changed.is_empty()
            && let Err(e) = engine::collections::save(&path, &lists)
        {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "command": kind.as_str(), "op": "check", "ok": false,
                        "error": format!("清单写入失败: {e}"),
                    }))
                    .unwrap()
                );
            } else {
                eprintln!("清单写入失败: {e}");
            }
            return 2;
        }
        return report(
            json,
            kind.as_str(),
            "check",
            serde_json::json!({
                "checked": checked, "changed": changed, "failures": failures,
            }),
        );
    }

    let _ = Kind::Watch;
    fail(
        json,
        &format!("未知操作 {op}（支持 add / remove / list / check）"),
    )
}

/// 统一输出与退出码：有失败项返回 1，否则 0。
fn report(json: bool, command: &str, op: &str, data: serde_json::Value) -> u8 {
    let has_failure = data
        .get("failures")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
        || data
            .get("failed")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "command": command, "op": op, "ok": !has_failure, "data": data,
            }))
            .unwrap()
        );
    } else {
        println!("{op} 完成: {data}");
    }
    if has_failure { 1 } else { 0 }
}

/// 从裸 ID / 商品链接里取出 7 位商品 ID。
fn normalize_item_id(raw: &str) -> Option<String> {
    let s = raw.trim();
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) && s.len() >= 5 {
        return Some(s.to_string());
    }
    // 链接形态：.../items/1234567 或 .../items/1234567?x=y
    let after = s.split("/items/").nth(1)?;
    let id: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    (!id.is_empty()).then_some(id)
}

/// download 命令（下载免费商品）。
#[allow(clippy::too_many_arguments)]
fn cmd_download(
    config: &AppConfig,
    shop: Option<&str>,
    items: &[String],
    out: Option<&Path>,
    dry_run: bool,
    limit: usize,
    cookie: Option<&str>,
    keep_failed: bool,
    json: bool,
) -> u8 {
    let out_root = match download_root(config, out) {
        Some(p) => p.to_path_buf(),
        None => {
            return fail(json, "未指定输出目录：用 --out 或配置文件 download_root");
        }
    };
    // 解析散链/裸 ID。
    let mut ids: Vec<String> = Vec::new();
    for blob in items {
        for id in engine::id::parse_discrete(blob) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let mut shop_id = shop.map(engine::id::shop_subdomain);
    // shop 参数若含 ID 链接，并入散链。
    if let Some(s) = shop {
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

    let cookie = resolve_cookie(cookie, config);
    let client = make_session(
        config,
        engine::config::resolve_cookie(cookie.as_deref(), config).as_deref(),
    );
    let rate_limit = config
        .rate_limit_secs
        .unwrap_or_else(default_rate_limit_secs);

    if ids.is_empty() && shop_id.is_none() {
        return fail(json, "提供店铺 URL/子域名，或用 --items 提供商品链接/ID");
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
        if limit > 0 && done >= limit {
            break;
        }
        match process_download_one(
            &client,
            &out_root,
            &item_id,
            dry_run,
            rate_limit,
            keep_failed,
        ) {
            Ok(true) => done += 1,
            Ok(false) => {}
            Err(e) => failures.push(format!("{item_id}: {e}")),
        }
    }

    let summary = serde_json::json!({
        "command": "download",
        "done": done,
        "failures": failures,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    } else {
        println!(
            "== 完成: {done} 个免费商品处理, {} 个失败 ==",
            failures.len()
        );
        for f in &failures {
            println!("   ! {f}");
        }
    }
    if failures.is_empty() { 0 } else { 1 }
}

/// 处理单个商品下载（返回是否处理了免费文件）。
fn process_download_one(
    client: &reqwest::blocking::Client,
    out_root: &Path,
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
        println!(
            "[dry-run] {item_id} | {} ({} files)",
            folder.display(),
            files.len()
        );
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
        println!("   -> {fname}");
    }
    // 封面 + 图标。
    let thumb = engine::fetch::thumb_from_json(&item);
    let cover = folder.join("cover.jpg");
    if !thumb.is_empty()
        && !cover.exists()
        && let Err(e) = engine::cover::download_cover(client, &thumb, &folder)
    {
        println!("   ! 封面失败: {e}");
    }
    apply_icon(&cover, &folder);
    engine::download::sleep_rate_limit(rate_limit);
    Ok(true)
}

/// 设置文件夹图标（Windows / macOS 走 engine 默认实现）。
fn apply_icon(cover: &Path, folder: &Path) {
    if cover.is_file() {
        let _ = engine::organize::default_icon_fn(cover, folder);
    }
}

/// organize 命令（按 ID 整理）。
#[allow(clippy::too_many_arguments)]
fn cmd_organize(
    config: &AppConfig,
    archives: &[std::path::PathBuf],
    out: Option<&Path>,
    force_id: Option<&str>,
    dry_run: bool,
    cookie: Option<&str>,
    keep_failed: bool,
    json: bool,
) -> u8 {
    let out_root = match download_root(config, out) {
        Some(p) => p.to_path_buf(),
        None => {
            return fail(json, "未指定输出目录：用 --out 或配置文件 download_root");
        }
    };
    let cookie = resolve_cookie(cookie, config);
    let client = make_session(
        config,
        engine::config::resolve_cookie(cookie.as_deref(), config).as_deref(),
    );
    let opts = engine::organize::OrganizeOptions {
        out_root: &out_root,
        dry_run,
        cookie: cookie.as_deref(),
        keep_failed_downloads: keep_failed,
    };
    let mut ok = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for path in archives {
        // 用 engine 的统一判据，不再各自拼 `is_file()` + 自有文案。
        // 此前本处报「找不到文件」、engine 报「不是文件」，同一种失败给出两种结论。
        // GUI organize 无预检，由 engine 守卫兜底（见 organize_archive）。
        if let Err(msg) = engine::organize::check_input_file(path) {
            failures.push(msg);
            continue;
        }
        let item_id = match force_id {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => {
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                engine::id::extract_id(&stem)
            }
        };
        if item_id.is_empty() {
            failures.push(format!(
                "[{}] 文件名中未找到 7 位数字 BOOTH ID，跳过（可用 --id 指定）",
                path.display()
            ));
            continue;
        }
        let outcome = engine::organize::organize_archive(&client, path, &item_id, &opts, icon_fn);
        if outcome.ok {
            ok += 1;
            if !json {
                println!("== 归档: {}", path.display());
                println!("   {} ({})", outcome.message, outcome.target_dir.display());
            }
        } else {
            failures.push(format!("{}: {}", path.display(), outcome.message));
        }
    }
    let summary = serde_json::json!({
        "command": "organize",
        "ok": ok,
        "total": archives.len(),
        "failures": failures,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    } else {
        println!("== 完成: {ok}/{} 个归档处理成功 ==", archives.len());
        for f in &failures {
            println!("   ! {f}");
        }
    }
    if failures.is_empty() { 0 } else { 1 }
}

/// 图标注入：Windows / macOS 走 engine 默认实现。
fn icon_fn(cover: &Path, folder: &Path) -> Result<(), String> {
    engine::organize::default_icon_fn(cover, folder)
}

/// 通用检索（`booth search --query`）：只检索、不归档。
///
/// 与 `cmd_search`（按名归档）是两条互斥路径：这里不碰磁盘库存，也不改任何文件。
fn cmd_search_query(
    config: &AppConfig,
    q: &engine::search::SearchQuery,
    limit: usize,
    no_cache: bool,
    cookie: Option<&str>,
    json: bool,
) -> u8 {
    let client = make_session(
        config,
        engine::config::resolve_cookie(cookie, config).as_deref(),
    );
    // 用 with_count 版本：has_next 需要「exclude 过滤前」的原始条数，
    // 否则整页被排除时会谎报 total=0/has_next=false。
    let (items, parsed_count) = match engine::search::search_with_count(&client, q, no_cache) {
        Ok(v) => v,
        Err(e) => {
            if json {
                let out = serde_json::json!({
                    "command": "search",
                    "mode": "query",
                    "query": q.query,
                    "error": e,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                eprintln!("搜索失败: {e}");
            }
            return 2;
        }
    };
    let total = items.len();
    if json {
        // 统一契约：字段在 engine::search::SearchOutput 单点定义，与 MCP 端逐字段一致。
        let out = engine::search::into_output(q, &items, parsed_count, limit);
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        let shown: Vec<&engine::search::SearchItem> = items.iter().take(limit).collect();
        println!("找到 {total} 条，显示前 {} 条", shown.len());
        let (_s, note) = engine::search::effective_sort(q.sort.as_deref().unwrap_or(""), q.page);
        if let Some(n) = note {
            println!("  注意：{n}");
        }
        for it in &shown {
            println!("  [{}] {}  {}  {}", it.id, it.name, it.price_text, it.shop);
        }
    }
    // 空结果不算错误：站点确实可能没有匹配项（与「请求失败」必须区分）。
    0
}

/// 搜索命令（按名搜索）。
#[allow(clippy::too_many_arguments)]
fn cmd_search(
    config: &AppConfig,
    files: &[std::path::PathBuf],
    base_dir: Option<&Path>,
    dry_run: bool,
    force_id: Option<&str>,
    cookie: Option<&str>,
    keep_failed: bool,
    desc: engine::score::DescWeight,
    json: bool,
) -> u8 {
    let base = match download_root(config, base_dir) {
        Some(p) => p.to_path_buf(),
        None => {
            return fail(
                json,
                "未指定归档目录：用 --base-dir 或配置文件 download_root",
            );
        }
    };
    let cookie = resolve_cookie(cookie, config);
    let client = make_session(
        config,
        engine::config::resolve_cookie(cookie.as_deref(), config).as_deref(),
    );
    let mut matched: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    for fp in files {
        let path = Path::new(fp);
        if dry_run {
            let candidates = engine::clean::sanitize_query(
                &path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            println!("dry-run: {}", path.display());
            for q in candidates {
                match engine::search::search_booth(&client, &q) {
                    Ok(results) => {
                        if let Some(it) = results.first() {
                            println!("  '{q}' → [{}] {}", it.id, it.name);
                            matched.push(it.id.clone());
                            break;
                        }
                    }
                    Err(e) => failures.push(format!("搜索失败 {q}: {e}")),
                }
            }
            continue;
        }
        // 完整路径：搜索 + 整理。
        match process_search_file(
            &client,
            path,
            &base,
            force_id,
            cookie.as_deref(),
            keep_failed,
            desc,
        ) {
            Ok(Some(id)) => matched.push(id),
            Ok(None) => {}
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }

    let summary = serde_json::json!({
        "command": "search",
        "matched": matched.len(),
        "total": files.len(),
        "failures": failures,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    } else {
        println!("汇总: {}/{} 件成功匹配", matched.len(), files.len());
        for f in &failures {
            println!("   ! {f}");
        }
    }
    if failures.is_empty() { 0 } else { 1 }
}

/// 单文件：搜索 → 评分 → 整理。
fn process_search_file(
    client: &reqwest::blocking::Client,
    path: &Path,
    base: &Path,
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
        // 搜索候选 → 评分选优。
        // 两轮解析（原候选 → kana 读音变体）统一由 engine 实现：
        //   历史实现把两轮 `chain` 成一次循环，「变体轮是否运行」取决于首个候选
        //   是否返回非空结果，而那个判据与「名称是否真的命中」无关——触发与否
        //   实际由标题长度决定。且 MCP/GUI 各写一份、完全不做变体轮。
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
    println!("   -> 归档: {}", outcome.target_dir.display());
    Ok(Some(item.id))
}

/// 规范名与说明文（评分依赖注入）。
///
/// 返回 `Option`：**取不到就返回 None，不再 `unwrap_or_default()` 压成空串**。
/// 压成空串会让「规范名命中 +100」静默失效，并让同名判定退化成 `"" == ""`
/// ——两个不同商品会被判成同名而报假歧义。
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

/// audit 命令（全库巡检）。
fn cmd_audit(
    config: &AppConfig,
    base: Option<&Path>,
    dry_run: bool,
    no_fix: bool,
    json: bool,
) -> u8 {
    let base = match download_root(config, base) {
        Some(p) => p.to_path_buf(),
        None => {
            return fail(json, "未指定巡检根目录：用 --base 或配置文件 download_root");
        }
    };
    if !base.is_dir() {
        return fail(json, &format!("FATAL: {} 不存在", base.display()));
    }
    let fix = !dry_run && !no_fix;
    let results = engine::audit::audit_tree_with_fix(&base, fix);
    let mut fixed = 0usize;
    let mut no_cover = 0usize;
    let mut failed = 0usize;

    let summary_items: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| !r.issues.is_empty())
        .map(|r| {
            serde_json::json!({
                "dir": r.dir.display().to_string(),
                "issues": r.issues,
                "fix": match &r.suggested_fix {
                    Some(engine::audit::FixAction::Rewrite) => "rewrite",
                    Some(engine::audit::FixAction::NeedsCover) => "needs-cover",
                    None => "none",
                },
            })
        })
        .collect();

    if !json {
        println!("扫描 {} 个商品目录", results.len());
        let problem = results.iter().filter(|r| !r.issues.is_empty()).count();
        println!("问题目录 {problem} 个");
        for r in results.iter().filter(|r| !r.issues.is_empty()).take(50) {
            println!("\n  {}", r.dir.display());
            for i in &r.issues {
                println!("    - {i}");
            }
        }
    }

    if fix {
        for r in results.iter().filter(|r| !r.issues.is_empty()) {
            let cover = r.dir.join("cover.jpg");
            match &r.suggested_fix {
                Some(engine::audit::FixAction::Rewrite) if cover.is_file() => {
                    match icon_fn(&cover, &r.dir) {
                        Ok(()) => fixed += 1,
                        Err(e) => {
                            failed += 1;
                            println!("  {e}：{}", r.dir.display());
                        }
                    }
                }
                Some(engine::audit::FixAction::NeedsCover) => no_cover += 1,
                _ => {}
            }
        }
    }

    let summary = serde_json::json!({
        "command": "audit",
        "total_dirs": results.len(),
        "problem_dirs": summary_items.len(),
        "fixed": fixed,
        "no_cover": no_cover,
        "failed": failed,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    } else if fix {
        println!("\n修复完成：{fixed} fixed / {no_cover} no-cover / {failed} failed");
    }
    if summary_items.is_empty() { 0 } else { 1 }
}

/// shell 图标命令。
fn cmd_shell(cmd: ShellCmd) -> u8 {
    match cmd {
        ShellCmd::Set { cover, folder } => match engine::organize::default_icon_fn(&cover, &folder)
        {
            Ok(()) => {
                println!("ok: 图标已写入 {}", folder.display());
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        },
        ShellCmd::Reset { folder } => {
            #[cfg(windows)]
            {
                match shell_win::folder_icon::reset_folder_icon(&folder) {
                    Ok(()) => {
                        println!("ok: 已清理 {}", folder.display());
                        0
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        1
                    }
                }
            }
            #[cfg(target_os = "macos")]
            {
                match shell_mac::folder_icon::reset_folder_icon(&folder) {
                    Ok(()) => {
                        println!("ok: 已清理 {}", folder.display());
                        0
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        1
                    }
                }
            }
            #[cfg(all(not(windows), not(target_os = "macos")))]
            {
                let _ = folder;
                eprintln!("error: shell reset 仅支持 Windows / macOS");
                2
            }
        }
        ShellCmd::Audit { folder } => {
            #[cfg(windows)]
            let ok = shell_win::folder_icon::has_folder_icon(&folder);
            #[cfg(target_os = "macos")]
            let ok = shell_mac::folder_icon::has_folder_icon(&folder);
            #[cfg(all(not(windows), not(target_os = "macos")))]
            let ok = false;
            #[cfg(all(not(windows), not(target_os = "macos")))]
            let _ = &folder;
            if ok {
                println!("audit PASS: 图标完整");
                0
            } else {
                println!("audit FAIL: 图标不完整");
                1
            }
        }
    }
}

/// version-audit：文件名版本巡检，可选补免费文件。
fn cmd_version_audit(
    config: &AppConfig,
    base: Option<&Path>,
    fix: bool,
    cookie: Option<&str>,
    keep_failed: bool,
    json: bool,
) -> u8 {
    let base = match download_root(config, base) {
        Some(p) => p.to_path_buf(),
        None => {
            return fail(json, "未指定巡检根目录：用 --base 或配置文件 download_root");
        }
    };
    if !base.is_dir() {
        return fail(json, &format!("FATAL: {} 不存在", base.display()));
    }
    let cookie = resolve_cookie(cookie, config);
    let client = make_session(
        config,
        engine::config::resolve_cookie(cookie.as_deref(), config).as_deref(),
    );
    let rate_limit = config
        .rate_limit_secs
        .unwrap_or_else(default_rate_limit_secs);
    let mut fetched = HashMap::new();
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
    if fix {
        let has_cookie = cookie.as_deref().is_some_and(|c| !c.trim().is_empty());
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
                    cookie.as_deref(),
                    keep_failed,
                );
                if n > 0 {
                    fixed += n;
                }
                failures.extend(errs.into_iter().map(|e| format!("{}: {e}", r.id)));
            }
        }
    }
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "local": r.local_tag,
                "remote": r.official_tag,
                "path": r.path.display().to_string(),
                "missing": r.missing,
            })
        })
        .collect();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "command": "version-audit",
                "updateable": rows.len(),
                "fixed": fixed,
                "failures": failures,
                "items": items,
            }))
            .unwrap()
        );
    } else {
        println!("可更新 {} 件", rows.len());
        for r in &rows {
            println!(
                "  {} · {}  本地 {} → 远程 {}  缺 {}",
                r.id,
                r.name,
                if r.local_tag.is_empty() {
                    "-"
                } else {
                    &r.local_tag
                },
                if r.official_tag.is_empty() {
                    "-"
                } else {
                    &r.official_tag
                },
                r.missing
            );
        }
        if fix {
            println!("补全文件 +{fixed}");
        }
        for f in &failures {
            println!("   ! {f}");
        }
    }
    if failures.is_empty() { 0 } else { 1 }
}

/// library：列出库存。
fn cmd_library(config: &AppConfig, base: Option<&Path>, refresh: bool, json: bool) -> u8 {
    let base = match download_root(config, base) {
        Some(p) => p.to_path_buf(),
        None => {
            return fail(json, "未指定归档根目录：用 --base 或配置文件 download_root");
        }
    };
    if !base.is_dir() {
        return fail(json, &format!("FATAL: {} 不存在", base.display()));
    }
    // 走持久化索引：稳态下只 stat 各分类目录，不再逐商品扫一遍。
    let cache = engine::config::library_cache_path();
    let items = engine::audit::list_library_cached(&base, cache.as_deref(), refresh);
    if json {
        let rows: Vec<serde_json::Value> = items
            .iter()
            .map(|i| {
                serde_json::json!({
                    "id": i.id,
                    "name": i.name,
                    "category": i.category,
                    "path": i.path.display().to_string(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "command": "library",
                "total": items.len(),
                "items": rows,
            }))
            .unwrap()
        );
    } else {
        println!("库存 {} 件", items.len());
        for i in &items {
            println!(
                "  {}  [{}]  {}  {}",
                i.id,
                i.category,
                i.name,
                i.path.display()
            );
        }
    }
    0
}

/// cookie-check：检测 Cookie 是否真的能登入 BOOTH（单次探针，不触发下载）。
fn cmd_cookie_check(config: &AppConfig, cookie: Option<&str>, json: bool) -> u8 {
    let resolved = resolve_cookie(cookie, config);
    let check = engine::session::check_cookie(config, resolved.as_deref());
    if json {
        println!(
            "{}",
            // 信封由 engine 单点定义，避免三端各写一份后漂移。
            serde_json::to_string_pretty(&check.to_command_json()).unwrap()
        );
    } else {
        println!("{} {}", if check.ok { "✓" } else { "✗" }, check.detail);
        println!(
            "  生效 cookie: {} 条（自动剔除 {} 条统计项）",
            check.pair_count, check.dropped_count
        );
        println!(
            "  含会话项 {}: {}",
            engine::session::SESSION_COOKIE,
            if check.has_session { "是" } else { "否" }
        );
    }
    if check.ok { 0 } else { 1 }
}

/// update_check 命令（检查工具自更新）。
/// preview 命令：列出压缩包内条目（不联网）。
fn cmd_preview(archives: &[std::path::PathBuf], limit: usize, json: bool) -> u8 {
    let mut reports = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for path in archives {
        match engine::integrity::preview_archive(path, limit) {
            Ok(pv) => reports.push(pv),
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }
    if json {
        let payload = serde_json::json!({
            "command": "preview",
            "archives": reports.iter().map(|p| serde_json::json!({
                "path": p.path.display().to_string(),
                "format": p.format,
                "total_entries": p.total_entries,
                "truncated": p.truncated,
                "entries": p.entries.iter().map(|e| serde_json::json!({
                    "name": e.name,
                    "size": e.size,
                    "compressed_size": e.compressed_size,
                    "method": e.method,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "failures": failures,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else {
        for p in &reports {
            println!("{} [{}] {} 条", p.path.display(), p.format, p.total_entries);
            for e in &p.entries {
                match e.compressed_size {
                    Some(c) => println!("   {:>10} {:>10}  {}", e.size, c, e.name),
                    None => println!("   {:>10} {:>10}  {}", e.size, "-", e.name),
                }
            }
            if p.truncated {
                println!("   … 已截断，仅列前 {} 条", p.entries.len());
            }
        }
        for f in &failures {
            println!("   ! {f}");
        }
    }
    if failures.is_empty() { 0 } else { 1 }
}

fn cmd_update_check(use_proxy: bool, json: bool) -> u8 {
    let info = engine::update::check_update(use_proxy);
    if json {
        let out = serde_json::json!({
            "command": "update_check",
            "has_update": info.has_update,
            "local_version": info.local_version,
            "remote_version": info.remote_version,
            "url": info.url,
            "release_title": info.release_title,
            "release_body": info.release_body,
            "error": info.error,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else if let Some(err) = &info.error {
        eprintln!("检查更新失败: {err}");
    } else if info.has_update {
        println!(
            "发现新版本: {} → {}  下载: {}",
            info.local_version, info.remote_version, info.url
        );
        if let Some(title) = &info.release_title {
            println!("{title}");
        }
        if let Some(body) = info
            .release_body
            .as_deref()
            .map(engine::update::html_to_text)
            && !body.is_empty()
        {
            println!("---");
            println!("{body}");
        }
    } else {
        println!("已是最新版本: {}", info.local_version);
    }
    // 退出码：0 成功 / 1 有失败（网络不可达）——按 AGENTS.md 契约，
    // 网络波动属「有失败」而非「致命」（致命通常指参数/配置类错误）。
    if info.error.is_some() { 1 } else { 0 }
}

/// 致命错误输出。
fn fail(json: bool, msg: &str) -> u8 {
    if json {
        println!(
            "{}",
            serde_json::json!({ "command": "error", "error": msg })
        );
    } else {
        eprintln!("error: {msg}");
    }
    2
}
