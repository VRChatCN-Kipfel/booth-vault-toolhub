//! Tauri command 层：download / organize / search / audit 的 GUI 接口。
//!
//! engine 使用 blocking reqwest，GUI 为 async —— 用 `spawn_blocking` 包裹。
//! 进度通过 `tauri::ipc::Channel` 流式推送（官方推荐，有序低延迟）。
//! 取消：managed state 维护 `Arc<AtomicBool>` 取消标志，长任务内协作式检查。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tauri::ipc::Channel;

use engine::config::{
    AppConfig, GuiSettings, apply_gui_settings, default_rate_limit_secs, gui_settings_from_config,
    load_config, resolve_cookie, save_user_config,
};
use engine::session::make_session;

/// 全局任务取消状态：`task_id -> cancel_flag`。
#[derive(Clone, Default)]
pub struct TaskRegistry(pub Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>);

/// 进度事件（前端 Channel 载荷）。
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProgressEvent {
    /// 队列级：任务开始。
    TaskStarted { total: usize },
    /// 单项完成（id + 结果状态）。
    #[allow(dead_code)] // organize/search 命令使用
    ItemDone {
        id: String,
        message: String,
        status: String,
    },
    /// 进度推进（done/total）。
    Progress { done: usize, total: usize },
    /// 单项失败。
    ItemError { id: String, message: String },
    /// 任务完成（含统计）。
    Finished { done: usize, failed: usize },
    /// 文本日志（用于 scan/fix 等详细输出）。
    #[allow(dead_code)] // audit 命令使用
    Log { line: String },
    /// 任务被取消。
    Cancelled,
}

/// 解析下载根目录：参数 > 配置。
fn resolve_root(config: &AppConfig, arg: Option<&str>) -> Result<std::path::PathBuf, String> {
    arg.filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| config.download_root.as_ref().map(std::path::PathBuf::from))
        .ok_or_else(|| "未指定输出目录（设置页配置或传参）".to_string())
}

/// 注册任务，返回 (task_id, cancel_flag)。
fn register_task(registry: &State<'_, TaskRegistry>) -> (String, Arc<AtomicBool>) {
    let task_id = uuid();
    let flag = Arc::new(AtomicBool::new(false));
    registry
        .0
        .lock()
        .unwrap()
        .insert(task_id.clone(), flag.clone());
    (task_id, flag)
}

/// 协作式取消：检查标志并睡眠一小段（让出）。
fn cancelled(flag: &AtomicBool) -> bool {
    flag.load(Ordering::Relaxed)
}

/// 简单 task id（时间戳 + 随机后缀）。
fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("task-{now}")
}

/// 取消任务。
#[tauri::command]
pub fn cancel_task(registry: State<'_, TaskRegistry>, task_id: String) -> bool {
    let guard = registry.0.lock().unwrap();
    if let Some(flag) = guard.get(&task_id) {
        flag.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// 立刻返回 task_id，工作在后台跑；结束时从注册表摘掉。
fn spawn_job<F>(registry: &State<'_, TaskRegistry>, job: F) -> String
where
    F: FnOnce(Arc<AtomicBool>) + Send + 'static,
{
    let (task_id, flag) = register_task(registry);
    let map = registry.0.clone();
    let tid = task_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        job(flag);
        if let Ok(mut g) = map.lock() {
            g.remove(&tid);
        }
    });
    task_id
}

#[tauri::command]
pub fn load_app_config() -> GuiSettings {
    gui_settings_from_config(&load_config())
}

#[tauri::command]
pub fn save_app_config(
    booth_root: String,
    proxy: bool,
    proxy_url: String,
    cookie: String,
) -> Result<(), String> {
    let mut cfg = load_config();
    apply_gui_settings(
        &mut cfg,
        &GuiSettings {
            booth_root,
            proxy,
            proxy_url,
            cookie,
        },
    );
    save_user_config(&cfg)
}

/// download：下载免费商品（散链/店铺）。立刻返回 task_id。
#[allow(clippy::too_many_arguments)] // Tauri command 参数由前端 invoke 传入
#[tauri::command]
pub fn download(
    registry: State<'_, TaskRegistry>,
    items: Vec<String>,
    shop: Option<String>,
    out: Option<String>,
    limit: Option<usize>,
    dry_run: bool,
    cookie: Option<String>,
    on_event: Channel<ProgressEvent>,
) -> Result<String, String> {
    let config = load_config();
    let out_root = resolve_root(&config, out.as_deref())?;
    let cookie = resolve_cookie(cookie.as_deref(), &config);
    let client = make_session(&config, cookie.as_deref());
    let rate_limit = config
        .rate_limit_secs
        .unwrap_or_else(default_rate_limit_secs);
    let limit = limit.unwrap_or(0);

    let mut ids: Vec<String> = Vec::new();
    for blob in &items {
        for id in engine::id::parse_discrete(blob) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let mut shop_id = shop.as_deref().map(engine::id::shop_subdomain);
    if let Some(s) = shop.as_deref() {
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
        return Err("提供店铺 URL/子域名，或用 items 提供商品链接/ID".to_string());
    }

    Ok(spawn_job(&registry, move |flag| {
        let mut ids = ids;
        if let Some(sub) = shop_id {
            if cancelled(&flag) {
                let _ = on_event.send(ProgressEvent::Cancelled);
                return;
            }
            match engine::search::crawl_item_ids(&client, &sub, rate_limit) {
                Ok(found) => {
                    for id in found {
                        if !ids.contains(&id) {
                            ids.push(id);
                        }
                    }
                }
                Err(e) => {
                    let _ = on_event.send(ProgressEvent::ItemError {
                        id: sub,
                        message: format!("店铺翻页失败: {e}"),
                    });
                    let _ = on_event.send(ProgressEvent::Finished { done: 0, failed: 1 });
                    return;
                }
            }
        }
        let total = ids.len();
        let _ = on_event.send(ProgressEvent::TaskStarted { total });
        let mut d = 0usize;
        let mut f = 0usize;
        let mut cancelled_now = false;
        for (processed, item_id) in ids.into_iter().enumerate() {
            if cancelled(&flag) {
                cancelled_now = true;
                break;
            }
            if limit > 0 && d >= limit {
                break;
            }
            match download_one(&client, &out_root, &item_id, dry_run, rate_limit) {
                Ok(true) => {
                    d += 1;
                    let _ = on_event.send(ProgressEvent::ItemDone {
                        id: item_id.clone(),
                        message: "已下载".to_string(),
                        status: "ok".to_string(),
                    });
                }
                Ok(false) => {
                    let _ = on_event.send(ProgressEvent::ItemDone {
                        id: item_id.clone(),
                        message: "无免费文件".to_string(),
                        status: "warn".to_string(),
                    });
                }
                Err(e) => {
                    f += 1;
                    let _ = on_event.send(ProgressEvent::ItemError {
                        id: item_id.clone(),
                        message: e,
                    });
                }
            }
            let _ = on_event.send(ProgressEvent::Progress {
                done: processed + 1,
                total,
            });
        }
        if cancelled_now {
            let _ = on_event.send(ProgressEvent::Cancelled);
        } else {
            let _ = on_event.send(ProgressEvent::Finished { done: d, failed: f });
        }
    }))
}

/// 单个商品下载（spawn_blocking 内执行）。
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

/// 图标注入（Windows / macOS 走 engine 默认实现）。
fn apply_icon(cover: &std::path::Path, folder: &std::path::Path) {
    if cover.is_file() {
        let _ = engine::organize::default_icon_fn(cover, folder);
    }
}

/// 图标注入（返回 Result，供 organize 编排）。
fn icon_fn(cover: &std::path::Path, folder: &std::path::Path) -> Result<(), String> {
    engine::organize::default_icon_fn(cover, folder)
}

/// organize：按 ID 整理本地压缩包。立刻返回 task_id。
#[tauri::command]
pub fn organize(
    registry: State<'_, TaskRegistry>,
    archives: Vec<String>,
    out: Option<String>,
    force_id: Option<String>,
    dry_run: bool,
    cookie: Option<String>,
    on_event: Channel<ProgressEvent>,
) -> Result<String, String> {
    let config = load_config();
    let out_root = resolve_root(&config, out.as_deref())?;
    let cookie = resolve_cookie(cookie.as_deref(), &config);
    let client = make_session(&config, cookie.as_deref());
    let total = archives.len();

    Ok(spawn_job(&registry, move |flag| {
        let _ = on_event.send(ProgressEvent::TaskStarted { total });
        let mut ok = 0usize;
        let mut failed = 0usize;
        let mut cancelled_now = false;
        for path_str in archives {
            if cancelled(&flag) {
                cancelled_now = true;
                break;
            }
            let path = std::path::PathBuf::from(&path_str);
            let item_id = match force_id.as_deref().filter(|s| !s.is_empty()) {
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
                failed += 1;
                let _ = on_event.send(ProgressEvent::ItemError {
                    id: path_str.clone(),
                    message: "文件名中未找到 7 位数字 BOOTH ID".to_string(),
                });
                continue;
            }
            let opts = engine::organize::OrganizeOptions {
                out_root: &out_root,
                dry_run,
                cookie: cookie.as_deref(),
            };
            let outcome =
                engine::organize::organize_archive(&client, &path, &item_id, &opts, icon_fn);
            if outcome.ok {
                ok += 1;
                let status_str = match outcome.status {
                    engine::organize::OrganizeStatus::Exists => "exists",
                    engine::organize::OrganizeStatus::Mismatch => "mismatch",
                    _ => "ok",
                };
                let _ = on_event.send(ProgressEvent::ItemDone {
                    id: item_id,
                    message: outcome.target_dir.display().to_string(),
                    status: status_str.to_string(),
                });
            } else {
                failed += 1;
                let _ = on_event.send(ProgressEvent::ItemError {
                    id: path_str,
                    message: outcome.message,
                });
            }
            let _ = on_event.send(ProgressEvent::Progress {
                done: ok + failed,
                total,
            });
        }
        if cancelled_now {
            let _ = on_event.send(ProgressEvent::Cancelled);
        } else {
            let _ = on_event.send(ProgressEvent::Finished { done: ok, failed });
        }
    }))
}

/// search：按名搜索并整理本地文件。立刻返回 task_id。
#[tauri::command]
pub fn search(
    registry: State<'_, TaskRegistry>,
    files: Vec<String>,
    base_dir: Option<String>,
    dry_run: bool,
    force_id: Option<String>,
    cookie: Option<String>,
    on_event: Channel<ProgressEvent>,
) -> Result<String, String> {
    let config = load_config();
    let base = resolve_root(&config, base_dir.as_deref())?;
    let cookie = resolve_cookie(cookie.as_deref(), &config);
    let client = make_session(&config, cookie.as_deref());
    let total = files.len();

    Ok(spawn_job(&registry, move |flag| {
        let _ = on_event.send(ProgressEvent::TaskStarted { total });
        let mut matched = 0usize;
        let mut failed = 0usize;
        let mut cancelled_now = false;
        for path_str in files {
            if cancelled(&flag) {
                cancelled_now = true;
                break;
            }
            let path = std::path::Path::new(&path_str);
            let fname = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if dry_run {
                let candidates = engine::clean::sanitize_query(&fname);
                let mut hit = false;
                for q in candidates {
                    match engine::search::search_booth(&client, &q) {
                        Ok(results) => {
                            if let Some(it) = results.first() {
                                matched += 1;
                                hit = true;
                                let _ = on_event.send(ProgressEvent::ItemDone {
                                    id: it.id.clone(),
                                    message: it.name.clone(),
                                    status: "ok".to_string(),
                                });
                                break;
                            }
                        }
                        Err(e) => {
                            failed += 1;
                            let _ = on_event.send(ProgressEvent::ItemError { id: q, message: e });
                        }
                    }
                }
                if !hit && !cancelled(&flag) {
                    failed += 1;
                    let _ = on_event.send(ProgressEvent::ItemError {
                        id: path_str.clone(),
                        message: "未找到匹配商品".to_string(),
                    });
                }
                continue;
            }
            match process_search_file(&client, path, &base, force_id.as_deref(), cookie.as_deref())
            {
                Ok(Some(id)) => {
                    matched += 1;
                    let _ = on_event.send(ProgressEvent::ItemDone {
                        id,
                        message: path_str,
                        status: "ok".to_string(),
                    });
                }
                Ok(None) => {
                    failed += 1;
                    let _ = on_event.send(ProgressEvent::ItemError {
                        id: path_str,
                        message: "未找到匹配商品".to_string(),
                    });
                }
                Err(e) => {
                    failed += 1;
                    let _ = on_event.send(ProgressEvent::ItemError {
                        id: path_str,
                        message: e,
                    });
                }
            }
            let _ = on_event.send(ProgressEvent::Progress {
                done: matched + failed,
                total,
            });
        }
        if cancelled_now {
            let _ = on_event.send(ProgressEvent::Cancelled);
        } else {
            let _ = on_event.send(ProgressEvent::Finished {
                done: matched,
                failed,
            });
        }
    }))
}

/// 按名搜索 + 评分 + 整理。
fn process_search_file(
    client: &reqwest::blocking::Client,
    path: &std::path::Path,
    base: &std::path::Path,
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
        cookie,
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

/// audit：全库文件夹图标三件套巡检（递归扫描 ID 目录）。立刻返回 task_id。
#[tauri::command]
pub fn audit(
    registry: State<'_, TaskRegistry>,
    base: Option<String>,
    dry_run: bool,
    no_fix: bool,
    on_event: Channel<ProgressEvent>,
) -> Result<String, String> {
    let config = load_config();
    let base_path = resolve_root(&config, base.as_deref())?;
    if !base_path.is_dir() {
        return Err(format!("FATAL: {} 不存在", base_path.display()));
    }

    Ok(spawn_job(&registry, move |flag| {
        let dirs = engine::audit::scan_library(&base_path);
        let total = dirs.len();
        let _ = on_event.send(ProgressEvent::TaskStarted { total });
        let mut missing = 0usize;
        let mut cancelled_now = false;
        for d in &dirs {
            if cancelled(&flag) {
                cancelled_now = true;
                break;
            }
            if !d.missing.is_empty() {
                missing += 1;
                if !dry_run && !no_fix {
                    let cover = d.path.join(engine::audit::COVER_FILENAME);
                    if cover.is_file()
                        && let Err(e) = icon_fn(&cover, &d.path)
                    {
                        let _ = on_event.send(ProgressEvent::Log {
                            line: format!("修复失败 {}: {e}", d.path.display()),
                        });
                    }
                }
            }
            let _ = on_event.send(ProgressEvent::ItemDone {
                id: format!("{} · {}", d.id, d.name),
                message: if d.missing.is_empty() {
                    "[完整]".to_string()
                } else {
                    format!("[缺{}]", d.missing.join("/"))
                },
                status: if d.missing.is_empty() {
                    "ok".to_string()
                } else {
                    "warn".to_string()
                },
            });
        }
        if cancelled_now {
            let _ = on_event.send(ProgressEvent::Cancelled);
            return;
        }
        let _ = on_event.send(ProgressEvent::Log {
            line: format!("共 {total} 件，{missing} 件缺失三件套"),
        });
        let _ = on_event.send(ProgressEvent::Finished {
            done: total,
            failed: missing,
        });
    }))
}

/// version_audit：联网比对官方商品名版本号，报告本地落后于官方的商品。
#[tauri::command]
pub fn version_audit(
    registry: State<'_, TaskRegistry>,
    base: Option<String>,
    on_event: Channel<ProgressEvent>,
) -> Result<String, String> {
    let config = load_config();
    let base_path = resolve_root(&config, base.as_deref())?;
    if !base_path.is_dir() {
        return Err(format!("FATAL: {} 不存在", base_path.display()));
    }
    let cookie = resolve_cookie(None, &config);
    let client = make_session(&config, cookie.as_deref());

    Ok(spawn_job(&registry, move |flag| {
        let dirs = engine::audit::scan_library(&base_path);
        let total = dirs.len();
        let _ = on_event.send(ProgressEvent::TaskStarted { total });
        let mut updateable = 0usize;
        let mut cancelled_now = false;
        for d in &dirs {
            if cancelled(&flag) {
                cancelled_now = true;
                break;
            }
            let item = match engine::fetch::fetch_item(&client, &d.id) {
                Ok(i) => i,
                Err(e) => {
                    let _ = on_event.send(ProgressEvent::Log {
                        line: format!("错误 {}: {e}", d.id),
                    });
                    continue;
                }
            };
            let official = engine::clean::extract_version_tag(&item.name);
            let _ = on_event.send(ProgressEvent::Log {
                line: format!(
                    "核对 {}: 本地 {} / 官方 {}",
                    d.id,
                    if d.local_tag.is_empty() {
                        "-"
                    } else {
                        d.local_tag.as_str()
                    },
                    if official.is_empty() {
                        "-"
                    } else {
                        official.as_str()
                    },
                ),
            });
            if engine::audit::ver_gt(&official, &d.local_tag) {
                updateable += 1;
                let _ = on_event.send(ProgressEvent::ItemDone {
                    id: format!("{} · {}", d.id, d.name),
                    message: format!("本地 {} → 官方 {} 可更新", d.local_tag, official),
                    status: "ok".to_string(),
                });
            }
        }
        if cancelled_now {
            let _ = on_event.send(ProgressEvent::Cancelled);
        } else {
            let _ = on_event.send(ProgressEvent::Finished {
                done: total,
                failed: updateable,
            });
        }
    }))
}

/// mismatch_audit：联网比对官方分类，报告目录所在分类与官方分类不一致的商品。
#[tauri::command]
pub fn mismatch_audit(
    registry: State<'_, TaskRegistry>,
    base: Option<String>,
    on_event: Channel<ProgressEvent>,
) -> Result<String, String> {
    let config = load_config();
    let base_path = resolve_root(&config, base.as_deref())?;
    if !base_path.is_dir() {
        return Err(format!("FATAL: {} 不存在", base_path.display()));
    }
    let cookie = resolve_cookie(None, &config);
    let client = make_session(&config, cookie.as_deref());

    Ok(spawn_job(&registry, move |flag| {
        let total = engine::audit::scan_library(&base_path).len();
        let _ = on_event.send(ProgressEvent::TaskStarted { total });
        let found = engine::audit::mismatch_audit(&base_path, |id| {
            if cancelled(&flag) {
                return None;
            }
            engine::fetch::fetch_item(&client, id).ok()
        });
        if cancelled(&flag) {
            let _ = on_event.send(ProgressEvent::Cancelled);
            return;
        }
        for m in &found {
            if cancelled(&flag) {
                let _ = on_event.send(ProgressEvent::Cancelled);
                return;
            }
            let _ = on_event.send(ProgressEvent::ItemDone {
                id: format!("{} · {}", m.id, m.name),
                message: format!("[{} → 期望 {}]", m.wrong_cat, m.dest_cat),
                status: "warn".to_string(),
            });
        }
        let _ = on_event.send(ProgressEvent::Finished {
            done: total,
            failed: found.len(),
        });
    }))
}

/// fix_mismatch：重检测错位目录并强制重归档到正确分类。
#[tauri::command]
pub fn fix_mismatch(
    registry: State<'_, TaskRegistry>,
    base: Option<String>,
    on_event: Channel<ProgressEvent>,
) -> Result<String, String> {
    let config = load_config();
    let base_path = resolve_root(&config, base.as_deref())?;
    if !base_path.is_dir() {
        return Err(format!("FATAL: {} 不存在", base_path.display()));
    }
    let cookie = resolve_cookie(None, &config);
    let client = make_session(&config, cookie.as_deref());

    Ok(spawn_job(&registry, move |flag| {
        let found = engine::audit::mismatch_audit(&base_path, |id| {
            if cancelled(&flag) {
                return None;
            }
            engine::fetch::fetch_item(&client, id).ok()
        });
        if cancelled(&flag) {
            let _ = on_event.send(ProgressEvent::Cancelled);
            return;
        }
        let total = found.len();
        let _ = on_event.send(ProgressEvent::TaskStarted { total });
        let opts = engine::organize::OrganizeOptions {
            out_root: &base_path,
            dry_run: false,
            cookie: cookie.as_deref(),
        };
        let mut fixed = 0usize;
        let mut failed = 0usize;
        for m in &found {
            if cancelled(&flag) {
                let _ = on_event.send(ProgressEvent::Cancelled);
                return;
            }
            let outcome = engine::organize::reorganize_dir(
                &client,
                std::path::Path::new(&m.path),
                &m.id,
                &opts,
                icon_fn,
            );
            if outcome.ok {
                fixed += 1;
                let _ = on_event.send(ProgressEvent::Log {
                    line: format!("已纠正 {} · {} → {}", m.id, m.name, m.dest_cat),
                });
            } else {
                failed += 1;
                let _ = on_event.send(ProgressEvent::ItemError {
                    id: format!("{} · {}", m.id, m.name),
                    message: outcome.message,
                });
            }
        }
        let _ = on_event.send(ProgressEvent::Finished {
            done: fixed,
            failed,
        });
    }))
}

/// update_check：检查工具自身是否有新版本（GitHub Releases）。
#[tauri::command]
pub async fn update_check(use_proxy: bool) -> Result<serde_json::Value, String> {
    let info =
        tauri::async_runtime::spawn_blocking(move || engine::update::check_update(use_proxy))
            .await
            .map_err(|e| format!("更新检查任务失败: {e}"))?;
    Ok(serde_json::json!({
        "command": "update_check",
        "has_update": info.has_update,
        "local_version": info.local_version,
        "remote_version": info.remote_version,
        "url": info.url,
        "error": info.error,
        "release_title": info.release_title,
        "release_body": info.release_body,
    }))
}
