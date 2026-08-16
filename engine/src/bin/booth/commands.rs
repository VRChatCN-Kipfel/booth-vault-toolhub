//! booth CLI 命令编排：download / organize / search / audit / shell。
//!
//! 退出码语义（MCP 依赖）：
//!   0 = 全部成功
//!   1 = 有失败项（部分失败）
//!   2 = 致命错误（参数/配置/网络不可用）

use std::path::Path;

use crate::{Cli, Command, GroupBy, ShellCmd};
use engine::config::{AppConfig, default_rate_limit_secs, load_config};
use engine::session::make_session;

/// 运行命令，返回退出码。
pub fn run(cli: Cli) -> u8 {
    let config = load_config();
    match cli.command {
        Command::Download {
            shop,
            items,
            out,
            dry_run,
            limit,
            folder_by,
            cookie,
            ua,
        } => cmd_download(
            &config,
            shop.as_deref(),
            &items,
            out.as_deref(),
            dry_run,
            limit,
            folder_by,
            cookie.as_deref(),
            ua.as_deref(),
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
            cli.json,
        ),
        Command::Search {
            files,
            base_dir,
            dry_run,
            keep,
            auto,
            id,
            cookie,
        } => cmd_search(
            &config,
            &files,
            base_dir.as_deref(),
            dry_run,
            keep,
            auto,
            id.as_deref(),
            cookie.as_deref(),
            cli.json,
        ),
        Command::Audit {
            base,
            dry_run,
            no_fix,
        } => cmd_audit(&config, base.as_deref(), dry_run, no_fix, cli.json),
        Command::Shell { command } => cmd_shell(command),
        Command::UpdateCheck { proxy } => cmd_update_check(proxy, cli.json),
    }
}

/// 解析配置里的下载根目录；未配置时返回 None（调用方用默认值）。
fn download_root<'a>(config: &'a AppConfig, arg: Option<&'a Path>) -> Option<&'a Path> {
    arg.or_else(|| config.download_root.as_deref().map(Path::new))
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
    _folder_by: GroupBy,
    cookie: Option<&str>,
    ua: Option<&str>,
    json: bool,
) -> u8 {
    let _ = ua;
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

    let client = make_session(config, cookie);
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
        match process_download_one(&client, &out_root, &item_id, dry_run, rate_limit) {
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
        println!(
            "[dry-run] {item_id} | {group} | {title} ({} files)",
            files.len()
        );
        return Ok(true);
    }
    std::fs::create_dir_all(&folder).map_err(|e| format!("建目录失败: {e}"))?;
    for (url, fname) in files {
        let dest = folder.join(engine::clean::sanitize(&fname, 120));
        // 幂等：已存在且有效则跳过。
        if dest.exists() && !engine::cover::looks_html(&std::fs::read(&dest).unwrap_or_default()) {
            continue;
        }
        engine::download::download(client, &url, &dest, true, 0.0)
            .map_err(|e| format!("下载失败 {fname}: {e}"))?;
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

/// 设置文件夹图标（Windows 用 shell_win；其余平台跳过）。
fn apply_icon(cover: &Path, folder: &Path) {
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

/// organize 命令（按 ID 整理）。
#[allow(clippy::too_many_arguments)]
fn cmd_organize(
    config: &AppConfig,
    archives: &[std::path::PathBuf],
    out: Option<&Path>,
    force_id: Option<&str>,
    dry_run: bool,
    cookie: Option<&str>,
    json: bool,
) -> u8 {
    let out_root = match download_root(config, out) {
        Some(p) => p.to_path_buf(),
        None => {
            return fail(json, "未指定输出目录：用 --out 或配置文件 download_root");
        }
    };
    let client = make_session(config, cookie);
    let opts = engine::organize::OrganizeOptions {
        out_root: &out_root,
        dry_run,
        cookie,
    };
    let mut ok = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for path in archives {
        if !path.is_file() {
            failures.push(format!("找不到文件: {}", path.display()));
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

/// 图标注入：Windows 用 shell_win，其余平台 no-op。
fn icon_fn(cover: &Path, folder: &Path) -> Result<(), String> {
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

/// search 命令（按名搜索）。
#[allow(clippy::too_many_arguments)]
fn cmd_search(
    config: &AppConfig,
    files: &[std::path::PathBuf],
    base_dir: Option<&Path>,
    dry_run: bool,
    keep: bool,
    _auto: bool,
    force_id: Option<&str>,
    cookie: Option<&str>,
    json: bool,
) -> u8 {
    let _ = keep;
    let base = match download_root(config, base_dir) {
        Some(p) => p.to_path_buf(),
        None => {
            return fail(
                json,
                "未指定归档目录：用 --base-dir 或配置文件 download_root",
            );
        }
    };
    let client = make_session(config, cookie);
    let mut matched: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    for fp in files {
        let path = std::path::Path::new(fp);
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
        match process_search_file(&client, path, &base, force_id, cookie) {
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
) -> Result<Option<String>, String> {
    let fname = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let item = if let Some(id) = force_id.filter(|s| !s.is_empty()) {
        engine::fetch::fetch_item(client, id).map_err(|e| format!("指定 ID {id} 获取失败: {e}"))?
    } else {
        // 搜索候选 → 评分选优。
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

    // 整理（直接复用 organize 逻辑：目标目录 + 移动 + 封面 + 图标 + 补全）。
    let title = if item.name.is_empty() {
        item.id.clone()
    } else {
        item.name.clone()
    };
    let cat = if item.category.name.is_empty() {
        "その他"
    } else {
        item.category.name.as_str()
    };
    let group = engine::classify::classify(cat, "");
    let folder = base.join(engine::clean::sanitize(&group, 40)).join(format!(
        "{}_{}",
        item.id,
        engine::clean::sanitize(&title, 70)
    ));
    let opts = engine::organize::OrganizeOptions {
        out_root: base,
        dry_run: false,
        cookie,
    };
    // 用统一 organize_archive 处理移动/封面/图标（但 source 是任意文件，ID 已定）。
    let outcome = engine::organize::organize_archive(client, path, &item.id, &opts, icon_fn);
    if !outcome.ok {
        return Err(outcome.message);
    }
    println!("   -> 归档: {}", folder.display());
    Ok(Some(item.id))
}

/// 规范名解析（score_and_pick 依赖注入）。
fn canonical_name(client: &reqwest::blocking::Client, id: &str) -> String {
    engine::fetch::fetch_item(client, id)
        .map(|i| i.name)
        .unwrap_or_default()
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
    let results = engine::audit::audit_tree(&base);
    let fix = !dry_run && !no_fix;
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
        ShellCmd::Set { cover, folder } => {
            #[cfg(windows)]
            {
                match shell_win::folder_icon::make_folder_icon(&cover, &folder) {
                    Ok(()) => {
                        println!("ok: 三件套已写入 {}", folder.display());
                        0
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        1
                    }
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (cover, folder);
                eprintln!("error: shell 命令仅支持 Windows");
                2
            }
        }
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
            #[cfg(not(windows))]
            {
                let _ = folder;
                eprintln!("error: shell 命令仅支持 Windows");
                2
            }
        }
        ShellCmd::Audit { folder } => {
            #[cfg(windows)]
            {
                let ok = shell_win::folder_icon::has_folder_icon(&folder);
                if ok {
                    println!("audit PASS: 三件套完整");
                    0
                } else {
                    println!("audit FAIL: 三件套不完整");
                    1
                }
            }
            #[cfg(not(windows))]
            {
                let _ = folder;
                eprintln!("error: shell 命令仅支持 Windows");
                2
            }
        }
    }
}

/// update_check 命令（检查工具自更新）。
fn cmd_update_check(use_proxy: bool, json: bool) -> u8 {
    let info = engine::update::check_update(use_proxy);
    if json {
        let out = serde_json::json!({
            "command": "update_check",
            "has_update": info.has_update,
            "local_version": info.local_version,
            "remote_version": info.remote_version,
            "url": info.url,
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
    } else if info.remote_version.is_empty() {
        println!("无法获取最新版本（网络不可达）");
    } else {
        println!("已是最新版本: {}", info.local_version);
    }
    if info.error.is_some() { 2 } else { 0 }
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
