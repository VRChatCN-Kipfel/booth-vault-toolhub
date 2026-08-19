//! 按 ID 整理归档：本地压缩包（文件名含 7 位 BOOTH ID）→ `分类目录/ID_标题/`。
//!
//! 纯编排逻辑：元数据获取、目录计算、移入、封面下载、免费版本补全均为现有
//! fetch/cover/download 模块的复用；文件夹图标经参数注入（Windows CLI 传
//! shell_win 的 make_folder_icon，测试传 stub），engine 层不直接依赖 shell。
//! 内部文件名保持原文件名（原名自带版本号，整理名必须保留，否则同商品不同
//! 版本会被合并覆盖）。

use std::path::{Path, PathBuf};

use reqwest::blocking::Client;

use crate::classify::classify;
use crate::clean::{extract_version_tag, sanitize};
use crate::cover::{COVER_FILENAME, download_cover};
use crate::download;
use crate::fetch::{ItemJson, free_downloads, thumb_from_json};

/// 免费补全下载间限速（对齐旧行为 0.5s）。
const BACKFILL_RATE_LIMIT: f64 = 0.5;

/// 整理选项。
pub struct OrganizeOptions<'a> {
    pub out_root: &'a Path,
    pub dry_run: bool,
    pub cookie: Option<&'a str>,
}

/// 整理结果状态（供 GUI 区分处理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizeStatus {
    /// 成功归档。
    Ok,
    /// 目标目录已存在同名文件，跳过移动。
    Exists,
    /// 同 ID 已在其他类目（错位），需确认是否强制重归档。
    Mismatch,
    /// 失败。
    Error,
}

/// 整理结果。
#[derive(Debug, Clone)]
pub struct OrganizeOutcome {
    pub ok: bool,
    pub status: OrganizeStatus,
    pub message: String,
    pub target_dir: PathBuf,
    pub moved: bool,
    pub cover_downloaded: bool,
    pub backfilled: usize,
}

impl OrganizeOutcome {
    fn fail(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            status: OrganizeStatus::Error,
            message: message.into(),
            target_dir: PathBuf::new(),
            moved: false,
            cover_downloaded: false,
            backfilled: 0,
        }
    }
}

/// 计算目标目录：`out_root / sanitize(group, 40) / {item_id}_{sanitize(title, 70)}`。
///
/// title 空时退回 item_id；类目空时退回「その他」，再经分类映射（未命中保留原文）。
pub fn target_folder(out_root: &Path, item: &ItemJson, item_id: &str) -> PathBuf {
    let title = if item.name.is_empty() {
        item_id
    } else {
        &item.name
    };
    let cat = if item.category.name.is_empty() {
        "その他"
    } else {
        &item.category.name
    };
    let parent = item
        .category
        .parent
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("");
    let group = classify(cat, parent);
    let folder_name = format!("{item_id}_{}", sanitize(title, 70));
    out_root.join(sanitize(&group, 40)).join(folder_name)
}

/// 本地缺失的免费文件列表：`(url, filename)`。
///
/// 判定：远程文件版本号与本地任一文件同版本（不同后缀也算已存在，按数值
/// 元组比较，`Ver_2.0` 与 `Ver_2.00` 视为同版本）则跳过；
/// 否则目标文件名已存在且为有效文件（存在、非空、非 HTML 伪装）则跳过；
/// 其余列为缺失。
/// 目录内资源文件名提取的版本标记（跳过封面/图标/sidecar）。
pub fn local_file_versions(dest_dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dest_dir) else {
        return Vec::new();
    };
    rd.filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
        .filter(|n| !is_sidecar_name(n))
        .map(|n| extract_version_tag(&n))
        .filter(|v| !v.is_empty())
        .collect()
}

fn is_sidecar_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("cover.jpg")
        || name.eq_ignore_ascii_case("desktop.ini")
        || name.eq_ignore_ascii_case("booth.txt")
        || name.eq_ignore_ascii_case("thumbs.db")
        || name.eq_ignore_ascii_case(".ds_store")
        || name.starts_with('.')
}

/// 一组版本标记中的最新（`ver_gt`），全空则空串。
pub fn latest_version_tag<I, S>(tags: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut best = String::new();
    for t in tags {
        let t = t.as_ref();
        if t.is_empty() {
            continue;
        }
        if best.is_empty() || crate::version::ver_gt(t, &best) {
            best = t.to_string();
        }
    }
    best
}

/// 远程免费文件名的最新版本标记。
pub fn remote_free_version_tag(item: &ItemJson) -> String {
    latest_version_tag(
        free_downloads(item)
            .into_iter()
            .map(|(_, fname)| extract_version_tag(&fname)),
    )
}

/// 本地文件名的最新版本标记。
pub fn local_file_version_tag(dest_dir: &Path) -> String {
    latest_version_tag(local_file_versions(dest_dir))
}

/// 是否应提示可更新：有远程免费文件，且本地缺文件或远程文件名版本更新。
pub fn free_updateable(dest_dir: &Path, item: &ItemJson) -> bool {
    let remotes = free_downloads(item);
    if remotes.is_empty() {
        return false;
    }
    if !missing_free_files(dest_dir, item).is_empty() {
        return true;
    }
    crate::version::ver_gt(
        &remote_free_version_tag(item),
        &local_file_version_tag(dest_dir),
    )
}

pub fn missing_free_files(dest_dir: &Path, item: &ItemJson) -> Vec<(String, String)> {
    let local_vers = local_file_versions(dest_dir);
    let mut missing = Vec::new();
    for (url, fname) in free_downloads(item) {
        let dest = dest_dir.join(sanitize(&fname, 120));
        let remote_ver = extract_version_tag(&fname);
        if !remote_ver.is_empty()
            && local_vers
                .iter()
                .any(|v| crate::version::ver_eq(v, &remote_ver))
        {
            continue;
        }
        if valid_local_file(&dest) {
            continue;
        }
        missing.push((url, fname));
    }
    missing
}

/// 免费版本补全：下载 `dest_dir` 缺失的免费文件。
///
/// 返回 `(补全数, 失败列表)`。无 cookie 时不下载，返回 `(0, [])`，缺失数量由
/// `missing_free_files` 另行报告。下载失败（含过期 Cookie / 假文件）写入失败列表。
pub fn backfill_free_files(
    client: &Client,
    dest_dir: &Path,
    item: &ItemJson,
    cookie: Option<&str>,
) -> (usize, Vec<String>) {
    let missing = missing_free_files(dest_dir, item);
    if missing.is_empty() {
        return (0, Vec::new());
    }
    let has_cookie = cookie.map(|c| !c.trim().is_empty()).unwrap_or(false);
    if !has_cookie {
        return (0, Vec::new());
    }
    let mut added = 0;
    let mut errors = Vec::new();
    for (url, fname) in missing {
        let dest = dest_dir.join(sanitize(&fname, 120));
        match download::download(client, &url, &dest, true, BACKFILL_RATE_LIMIT) {
            Ok(()) => added += 1,
            Err(e) => errors.push(format!("{fname}: {}", download::with_cookie_hint(e))),
        }
    }
    (added, errors)
}

/// 整理单个 archive 文件。
///
/// 流程：元数据 → 目录计算 → 移入（同盘 rename，跨盘失败回退 copy 且保留原文件）
/// → 封面下载 → 图标（注入）→ 免费版本补全。dry_run 只计算并报告，不落盘。
pub fn organize_archive(
    client: &Client,
    archive: &Path,
    item_id: &str,
    opts: &OrganizeOptions,
    icon_fn: impl Fn(&Path, &Path) -> Result<(), String>,
) -> OrganizeOutcome {
    let item = match crate::fetch::fetch_item(client, item_id) {
        Ok(item) => item,
        Err(e) => {
            return OrganizeOutcome::fail(format!("无法获取商品 {item_id} 元数据: {e}"));
        }
    };
    let folder = target_folder(opts.out_root, &item, item_id);
    if opts.dry_run {
        return OrganizeOutcome {
            ok: true,
            status: OrganizeStatus::Ok,
            message: format!("[dry-run] 目标: {}", folder.display()),
            target_dir: folder,
            moved: false,
            cover_downloaded: false,
            backfilled: 0,
        };
    }

    if let Err(e) = std::fs::create_dir_all(&folder) {
        return OrganizeOutcome::fail(format!("创建目录失败 {}: {e}", folder.display()));
    }
    let mut message = format!("目标: {}", folder.display());

    // 移入归档：内部文件名保持原文件名（清洗后）。
    let dest_arc = folder.join(sanitize(&archive_name(archive), 120));
    let mut moved = false;
    let mut exists = false;
    if archive != dest_arc {
        if dest_arc.exists() {
            exists = true;
            message.push_str("；目标文件已存在，跳过移动");
        } else {
            match std::fs::rename(archive, &dest_arc) {
                Ok(()) => {
                    message.push_str("；已移入");
                    moved = true;
                }
                Err(_) => match std::fs::copy(archive, &dest_arc) {
                    Ok(_) => {
                        message.push_str("；移动失败，已复制（原文件保留）");
                        moved = true;
                    }
                    Err(e) => {
                        return OrganizeOutcome::fail(format!(
                            "移动/复制失败 {}: {e}",
                            dest_arc.display()
                        ));
                    }
                },
            }
        }
    } else {
        message.push_str("；已在目标位置");
    }

    // 封面 + 图标 + 免费版本补全（收尾）。
    let (cover_downloaded, backfilled) =
        finalize_folder(client, &folder, &item, opts, &icon_fn, &mut message);

    OrganizeOutcome {
        ok: true,
        status: if exists {
            OrganizeStatus::Exists
        } else {
            OrganizeStatus::Ok
        },
        message,
        target_dir: folder,
        moved,
        cover_downloaded,
        backfilled,
    }
}

/// 归档收尾：封面下载（幂等）+ 图标注入 + 免费版本补全，追加进度消息。
///
/// 返回 `(cover_downloaded, backfilled)`。
fn finalize_folder(
    client: &Client,
    folder: &Path,
    item: &ItemJson,
    opts: &OrganizeOptions,
    icon_fn: &impl Fn(&Path, &Path) -> Result<(), String>,
    message: &mut String,
) -> (bool, usize) {
    let cover = folder.join(COVER_FILENAME);
    let mut cover_downloaded = false;
    if cover.exists() {
        message.push_str("；封面已存在");
    } else {
        let thumb = thumb_from_json(item);
        if thumb.is_empty() {
            message.push_str("；无封面图");
        } else {
            match download_cover(client, &thumb, folder) {
                Ok(_) => {
                    message.push_str("；封面已下载");
                    cover_downloaded = true;
                }
                Err(e) => message.push_str(&format!("；封面下载失败: {e}")),
            }
        }
    }

    // 图标（注入）：封面存在即设置，失败不影响整体结果。
    if cover.exists() {
        match icon_fn(&cover, folder) {
            Ok(()) => message.push_str("；图标已设置"),
            Err(e) => message.push_str(&format!("；图标失败: {e}")),
        }
    }

    write_booth_txt(folder, item);

    // 免费版本补全。
    let (backfilled, backfill_errors) = backfill_free_files(client, folder, item, opts.cookie);
    if backfilled > 0 {
        message.push_str(&format!("；免费版本补全 +{backfilled}"));
    }
    if !backfill_errors.is_empty() {
        message.push_str(&format!("；补全失败: {}", backfill_errors.join("；")));
    } else if backfilled == 0 {
        let missing = missing_free_files(folder, item).len();
        if missing > 0 {
            let hint = if opts.cookie.map(|c| !c.trim().is_empty()).unwrap_or(false) {
                ""
            } else {
                "（请到设置页填写 Cookie，CLI/MCP 用 --cookie）"
            };
            message.push_str(&format!("；另有 {missing} 个免费版本缺失{hint}"));
        }
    }
    (cover_downloaded, backfilled)
}

/// 条款 sidecar：标题 / ID / 店铺 / 商品 URL / description。失败不挡归档。
pub fn write_booth_txt(folder: &Path, item: &ItemJson) {
    let id = if item.id.is_empty() {
        folder
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.get(..7))
            .unwrap_or("")
            .to_string()
    } else {
        item.id.clone()
    };
    let url = format!("https://booth.pm/ja/items/{id}");
    let shop = if item.shop.name.is_empty() {
        item.shop.subdomain.clone()
    } else {
        item.shop.name.clone()
    };
    let mut body = format!("{}\nID: {id}\n店铺: {shop}\n{url}", item.name);
    if !item.description.trim().is_empty() {
        body.push_str("\n\n");
        body.push_str(item.description.trim());
    }
    body.push('\n');
    let _ = std::fs::write(folder.join("booth.txt"), body);
}

/// 错位纠正：把 `source` 目录内容整体迁入目标分类目录并重建三件套。
///
/// 目标目录已存在先整体清空重建；源目录内 desktop.ini/Thumbs.db 等系统文件不随迁，
/// 迁完删除空源目录；封面/图标/免费版本补全流程与 `organize_archive` 一致。
/// `source` 已位于目标位置时只补齐三件套，不迁移。
pub fn reorganize_dir(
    client: &Client,
    source: &Path,
    item_id: &str,
    opts: &OrganizeOptions,
    icon_fn: impl Fn(&Path, &Path) -> Result<(), String>,
) -> OrganizeOutcome {
    let item = match crate::fetch::fetch_item(client, item_id) {
        Ok(item) => item,
        Err(e) => {
            return OrganizeOutcome::fail(format!("无法获取商品 {item_id} 元数据: {e}"));
        }
    };
    let folder = target_folder(opts.out_root, &item, item_id);
    if opts.dry_run {
        return OrganizeOutcome {
            ok: true,
            status: OrganizeStatus::Ok,
            message: format!("[dry-run] 目标: {}", folder.display()),
            target_dir: folder,
            moved: false,
            cover_downloaded: false,
            backfilled: 0,
        };
    }

    let mut message = format!("目标: {}", folder.display());
    let mut moved = false;
    if source != folder {
        // 强制重归档：目标目录已存在（可能残留旧三件套）先整体清空。
        if folder.exists()
            && let Err(e) = std::fs::remove_dir_all(&folder)
        {
            return OrganizeOutcome::fail(format!("清旧目录失败 {}: {e}", folder.display()));
        }
        if let Err(e) = std::fs::create_dir_all(&folder) {
            return OrganizeOutcome::fail(format!("创建目录失败 {}: {e}", folder.display()));
        }
        if source.is_dir() {
            // 源目录内容整体迁入，跳过 desktop.ini/Thumbs.db 等系统文件。
            let entries: Vec<std::fs::DirEntry> = match std::fs::read_dir(source) {
                Ok(rd) => rd.filter_map(Result::ok).collect(),
                Err(e) => {
                    return OrganizeOutcome::fail(format!(
                        "读取源目录失败 {}: {e}",
                        source.display()
                    ));
                }
            };
            for entry in entries {
                let name = entry.file_name();
                if matches!(
                    name.to_string_lossy().as_ref(),
                    "desktop.ini" | "Thumbs.db" | ".DS_Store"
                ) {
                    continue;
                }
                let dst = folder.join(&name);
                if let Err(e) = std::fs::rename(entry.path(), &dst) {
                    // 跨盘回退复制且保留原文件。
                    if let Err(e2) = std::fs::copy(entry.path(), &dst) {
                        return OrganizeOutcome::fail(format!(
                            "迁移失败 {}: {e} / {e2}",
                            entry.path().display()
                        ));
                    }
                }
                moved = true;
            }
            // 删除残留系统文件后清空源目录。
            let _ = std::fs::remove_file(source.join("desktop.ini"));
            let _ = std::fs::remove_file(source.join("Thumbs.db"));
            let _ = std::fs::remove_file(source.join(".DS_Store"));
            let _ = std::fs::remove_dir(source);
            message.push_str("；内容已迁入");
        } else {
            // 文件形态源（容错）：直接移入。
            let dest = folder.join(sanitize(&archive_name(source), 120));
            if source != dest {
                if let Err(e) = std::fs::rename(source, &dest) {
                    return OrganizeOutcome::fail(format!("移动失败 {}: {e}", source.display()));
                }
                moved = true;
            }
            message.push_str("；已移入");
        }
    } else {
        message.push_str("；已在目标位置");
    }

    // 封面 + 图标 + 免费版本补全（收尾）。
    let (cover_downloaded, backfilled) =
        finalize_folder(client, &folder, &item, opts, &icon_fn, &mut message);

    OrganizeOutcome {
        ok: true,
        status: OrganizeStatus::Ok,
        message,
        target_dir: folder,
        moved,
        cover_downloaded,
        backfilled,
    }
}

/// 归档文件名的末段（保留扩展名）。
fn archive_name(archive: &Path) -> String {
    archive
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

/// 文件有效：存在、非空且非 HTML 伪装（未登录时 BOOTH 返回伪装成文件的登录页）。
fn valid_local_file(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    let Ok(meta) = std::fs::metadata(p) else {
        return false;
    };
    if meta.len() == 0 {
        return false;
    }
    let Ok(mut fh) = std::fs::File::open(p) else {
        return false;
    };
    use std::io::Read;
    let mut head = [0u8; 256];
    let n = fh.read(&mut head).unwrap_or(0);
    !crate::cover::looks_html(&head[..n])
}

/// 默认图标实现：Windows → shell_win；macOS → Finder 自定义图标；其余空操作。
#[cfg(windows)]
pub fn default_icon_fn(cover: &Path, folder: &Path) -> Result<(), String> {
    shell_win::folder_icon::make_folder_icon(cover, folder).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
pub fn default_icon_fn(cover: &Path, folder: &Path) -> Result<(), String> {
    shell_mac::folder_icon::make_folder_icon(cover, folder).map_err(|e| e.to_string())
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn default_icon_fn(_cover: &Path, _folder: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::fetch::{DownloadFileJson, DownloadableJson, VariationJson};

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bvt_organize_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn free_var(files: Vec<(&str, &str)>) -> VariationJson {
        VariationJson {
            price: Some(0),
            downloadable: Some(DownloadableJson {
                no_musics: files
                    .iter()
                    .map(|(u, n)| DownloadFileJson {
                        url: u.to_string(),
                        name: n.to_string(),
                        ..DownloadFileJson::default()
                    })
                    .collect(),
                musics: vec![],
            }),
        }
    }

    #[test]
    fn write_booth_txt_contains_fields() {
        let dir = tmpdir("sidecar");
        let item = ItemJson {
            id: "1234567".to_string(),
            name: "Dress".to_string(),
            description: "利用規約".to_string(),
            shop: crate::fetch::ShopJson {
                name: "ShopA".to_string(),
                ..crate::fetch::ShopJson::default()
            },
            ..ItemJson::default()
        };
        write_booth_txt(&dir, &item);
        let text = std::fs::read_to_string(dir.join("booth.txt")).unwrap();
        assert!(text.contains("Dress"));
        assert!(text.contains("1234567"));
        assert!(text.contains("ShopA"));
        assert!(text.contains("https://booth.pm/ja/items/1234567"));
        assert!(text.contains("利用規約"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn target_folder_basic() {
        let item = ItemJson {
            name: "メカ弾エフェクト".to_string(),
            category: crate::fetch::CategoryJson {
                name: "エフェクト".to_string(),
                parent: None,
            },
            ..ItemJson::default()
        };
        let folder = target_folder(Path::new("C:\\out"), &item, "1234567");
        assert_eq!(
            folder,
            Path::new("C:\\out")
                .join("特效")
                .join("1234567_メカ弾エフェクト")
        );
    }

    #[test]
    fn target_folder_falls_back() {
        let item = ItemJson::default();
        let folder = target_folder(Path::new("/out"), &item, "1234567");
        assert_eq!(
            folder,
            Path::new("/out").join("其他").join("1234567_1234567")
        );
    }

    #[test]
    fn target_folder_uses_parent_like_organize() {
        let item = ItemJson {
            name: "Dress".to_string(),
            category: crate::fetch::CategoryJson {
                name: "衣装".to_string(),
                parent: Some(crate::fetch::ParentCategory {
                    name: "3Dモデル".to_string(),
                }),
            },
            ..ItemJson::default()
        };
        let folder = target_folder(Path::new("/out"), &item, "1234567");
        assert_eq!(
            folder,
            Path::new("/out").join("3D服饰").join("1234567_Dress")
        );
        let no_parent = ItemJson {
            name: "Dress".to_string(),
            category: crate::fetch::CategoryJson {
                name: "衣装".to_string(),
                parent: None,
            },
            ..ItemJson::default()
        };
        assert_eq!(
            target_folder(Path::new("/out"), &no_parent, "1234567"),
            Path::new("/out").join("服饰").join("1234567_Dress")
        );
    }

    #[test]
    fn target_folder_sanitizes_title_and_group() {
        let item = ItemJson {
            name: "a<b:c/d".to_string(),
            category: crate::fetch::CategoryJson {
                name: "未知类目".to_string(),
                parent: None,
            },
            ..ItemJson::default()
        };
        let folder = target_folder(Path::new("/out"), &item, "1111111");
        assert_eq!(
            folder,
            Path::new("/out").join("未知类目").join("1111111_abcd")
        );
    }

    #[test]
    fn missing_free_files_version_dedup() {
        let dir = tmpdir("ver");
        std::fs::write(
            dir.join("メカ弾エフェクトVer_2.00.unitypackage"),
            b"PK\x03\x04",
        )
        .unwrap();
        let item = ItemJson {
            variations: vec![free_var(vec![
                ("https://u/ver200.zip", "メカ弾エフェクトVer_2.00.zip"),
                ("https://u/ver101.zip", "メカ弾エフェクトVer_1.01.zip"),
            ])],
            ..ItemJson::default()
        };
        // 远程 Ver_2.00 与本地 unitypackage 同版本 → 已存在；Ver_1.01 缺失。
        let missing = missing_free_files(&dir, &item);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].1, "メカ弾エフェクトVer_1.01.zip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_free_files_existing_exact_name_skipped() {
        let dir = tmpdir("exact");
        std::fs::write(dir.join("file.zip"), b"PK\x03\x04").unwrap();
        let item = ItemJson {
            variations: vec![free_var(vec![("https://u/f.zip", "file.zip")])],
            ..ItemJson::default()
        };
        assert!(missing_free_files(&dir, &item).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_free_files_empty_dir_all_missing() {
        let dir = tmpdir("empty");
        let item = ItemJson {
            variations: vec![free_var(vec![
                ("https://u/a.zip", "a.zip"),
                ("https://u/b.zip", "b.zip"),
            ])],
            ..ItemJson::default()
        };
        assert_eq!(missing_free_files(&dir, &item).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_free_files_html_lookalike_treated_missing() {
        let dir = tmpdir("html");
        std::fs::write(
            dir.join("file.zip"),
            b"<!DOCTYPE html><html><body>login</body></html>",
        )
        .unwrap();
        let item = ItemJson {
            variations: vec![free_var(vec![("https://u/f.zip", "file.zip")])],
            ..ItemJson::default()
        };
        assert_eq!(missing_free_files(&dir, &item).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backfill_without_cookie_reports_only() {
        let dir = tmpdir("nook");
        let item = ItemJson {
            variations: vec![free_var(vec![("https://u/a.zip", "a.zip")])],
            ..ItemJson::default()
        };
        let client = crate::session::make_session(&crate::config::AppConfig::default(), None);
        // 无 cookie：不触发任何下载。
        let (added, errors) = backfill_free_files(&client, &dir, &item, None);
        assert_eq!(added, 0);
        assert!(errors.is_empty());
        assert!(!dir.join("a.zip").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
