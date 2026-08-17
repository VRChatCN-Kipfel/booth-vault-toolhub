//! 文件夹图标三件套完整性巡检（纯文件系统检查，无 Windows FFI）。
//!
//! 属性位检查经回调注入：`audit_one_with_attrs` 是平台无关的纯逻辑核心，
//! `audit_one` / `audit_tree` 提供平台默认（Windows 下读取真实属性，
//! 其余平台返回 0）。desktop.ini 按 utf-16 → utf-8 → gbk 三编码读回，
//! 兼容历史遗留编码契约（当前写入为 UTF-8 无 BOM，旧版曾写 UTF-16）。

use fancy_regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::clean::extract_version_tag;

/// HIDDEN 属性位。
const ATTR_HIDDEN: u32 = 0x02;
/// SYSTEM 属性位。
const ATTR_SYSTEM: u32 = 0x04;
/// READONLY 属性位。
const ATTR_READONLY: u32 = 0x01;

/// 图标文件名。
pub const FOLDER_ICO: &str = ".folder_icon.ico";
/// desktop.ini 文件名。
pub const DESKTOP_INI: &str = "desktop.ini";
/// 封面文件名。
pub const COVER_FILENAME: &str = "cover.jpg";

/// 是否已有文件夹图标：Windows 看 `.folder_icon.ico`；macOS 还认 Finder `Icon\r` / `.folder_icon.png`。
fn folder_icon_present(dir: &Path) -> bool {
    if dir.join(FOLDER_ICO).is_file() {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        return shell_mac::folder_icon::has_folder_icon(dir);
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// macOS 不写 desktop.ini，巡检不把它当缺失项。
fn ini_required() -> bool {
    !cfg!(target_os = "macos")
}

/// 修复动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixAction {
    /// 找到 cover.jpg，走 make_folder_icon 重写。
    Rewrite,
    /// 缺 cover.jpg，需人工补齐。
    NeedsCover,
}

/// 单目录巡检结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditResult {
    pub dir: PathBuf,
    pub issues: Vec<String>,
    pub suggested_fix: Option<FixAction>,
}

/// 三编码读回 desktop.ini：utf-16（要求 BOM）→ utf-8 → gbk。
///
/// 文件不存在或读取失败返回空串。
pub fn read_ini_text(path: &Path) -> String {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    if bytes.is_empty() {
        return String::new();
    }
    // utf-16：带 BOM 时按 BOM 判断端序（LE 为主流）。
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false);
    }
    // utf-8：严格解码成功即用。
    if let Ok(s) = String::from_utf8(bytes.clone()) {
        return s;
    }
    // gbk 兜底（历史遗留编码）。
    let (s, _, _) = encoding_rs::GBK.decode(&bytes);
    s.into_owned()
}

/// UTF-16 解码为 String（无效代理以 U+FFFD 替换，容忍截断的尾字节）。
fn decode_utf16(bytes: &[u8], little: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if little {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// 属性位 → "H/S/R" 字符串；全空返回 "-"。
fn attr_str(a: u32) -> String {
    let mut s = String::new();
    if a & ATTR_HIDDEN != 0 {
        s.push('H');
    }
    if a & ATTR_SYSTEM != 0 {
        s.push('S');
    }
    if a & ATTR_READONLY != 0 {
        s.push('R');
    }
    if s.is_empty() { "-".to_string() } else { s }
}

/// 巡检单个目录（属性位经 `attr_fn` 注入）。
///
/// 检查项：ini 缺失/缺 IconResource 字段、ico 缺失/过小（<1KB）、
/// ini 缺 H+S、ico 缺 H、文件夹缺 R。`fix=true` 且有问题时给出修复动作：
/// 有 cover.jpg → Rewrite，否则 NeedsCover。
pub fn audit_one_with_attrs(dir: &Path, fix: bool, attr_fn: impl Fn(&Path) -> u32) -> AuditResult {
    let ico = dir.join(FOLDER_ICO);
    let ini = dir.join(DESKTOP_INI);
    let cover = dir.join(COVER_FILENAME);
    let mut issues: Vec<String> = Vec::new();

    if ini.exists() {
        let text = read_ini_text(&ini);
        if !text.contains("IconResource=.folder_icon.ico") {
            issues.push("ini 缺 IconResource=.folder_icon.ico 字段".to_string());
        }
    } else {
        issues.push("ini 缺失".to_string());
    }

    if ico.exists() {
        let size = std::fs::metadata(&ico).map(|m| m.len()).unwrap_or(0);
        if size < 1024 {
            issues.push(format!("ico 过小（<1KB，实际 {size}B）"));
        }
    } else {
        issues.push("ico 缺失".to_string());
    }

    if ini.exists() {
        let a = attr_fn(&ini);
        if a & ATTR_HIDDEN == 0 || a & ATTR_SYSTEM == 0 {
            issues.push(format!("ini 属性 {}（缺 H/S）", attr_str(a)));
        }
    }
    if ico.exists() {
        let a = attr_fn(&ico);
        if a & ATTR_HIDDEN == 0 {
            issues.push(format!("ico 属性 {}（缺 H）", attr_str(a)));
        }
    }
    let fa = attr_fn(dir);
    if fa & ATTR_READONLY == 0 {
        issues.push(format!("文件夹属性 {}（缺 R）", attr_str(fa)));
    }

    let suggested_fix = if fix && !issues.is_empty() {
        Some(if cover.exists() {
            FixAction::Rewrite
        } else {
            FixAction::NeedsCover
        })
    } else {
        None
    };

    AuditResult {
        dir: dir.to_path_buf(),
        issues,
        suggested_fix,
    }
}

/// 巡检单个目录（平台默认属性读取）。
pub fn audit_one(dir: &Path, fix: bool) -> AuditResult {
    #[cfg(target_os = "macos")]
    {
        return audit_one_macos(dir, fix);
    }
    #[cfg(not(target_os = "macos"))]
    {
        audit_one_with_attrs(dir, fix, default_attr)
    }
}

#[cfg(target_os = "macos")]
fn audit_one_macos(dir: &Path, fix: bool) -> AuditResult {
    let cover = dir.join(COVER_FILENAME);
    let mut issues: Vec<String> = Vec::new();
    if !cover.is_file() {
        issues.push("封面缺失".to_string());
    }
    if !shell_mac::folder_icon::has_folder_icon(dir) {
        issues.push("Finder 自定义图标缺失".to_string());
    }
    let suggested_fix = if fix && !issues.is_empty() {
        Some(if cover.is_file() {
            FixAction::Rewrite
        } else {
            FixAction::NeedsCover
        })
    } else {
        None
    };
    AuditResult {
        dir: dir.to_path_buf(),
        issues,
        suggested_fix,
    }
}

#[cfg(windows)]
fn default_attr(p: &Path) -> u32 {
    shell_win::attributes::get_attrs(p)
}

#[cfg(not(windows))]
fn default_attr(_p: &Path) -> u32 {
    0
}

/// 遍历 base 下所有分类目录（一级）/商品目录（二级），返回有问题的目录。
///
/// 跳过隐藏目录与非目录项。
pub fn audit_tree(base: &Path) -> Vec<AuditResult> {
    audit_tree_with_fix(base, false)
}

/// 遍历 `base` 下的商品目录；`fix=true` 时为每个问题目录给出修复建议。
pub fn audit_tree_with_fix(base: &Path, fix: bool) -> Vec<AuditResult> {
    #[cfg(target_os = "macos")]
    {
        return audit_tree_macos(base, fix);
    }
    #[cfg(not(target_os = "macos"))]
    {
        audit_tree_with_attrs(base, fix, default_attr)
    }
}

#[cfg(target_os = "macos")]
fn audit_tree_macos(base: &Path, fix: bool) -> Vec<AuditResult> {
    let mut out = Vec::new();
    let Ok(cats) = sorted_dirs(base) else {
        return out;
    };
    for cat in cats {
        if is_dot_hidden(&cat) {
            continue;
        }
        let Ok(items) = sorted_dirs(&cat) else {
            continue;
        };
        for d in items {
            if is_dot_hidden(&d) {
                continue;
            }
            let result = audit_one(&d, fix);
            if !result.issues.is_empty() {
                out.push(result);
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn is_dot_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// `audit_tree` 的属性注入版本（平台无关，便于测试）。
#[cfg(not(target_os = "macos"))]
fn audit_tree_with_attrs(
    base: &Path,
    fix: bool,
    attr_fn: impl Fn(&Path) -> u32,
) -> Vec<AuditResult> {
    let mut out = Vec::new();
    let Ok(cats) = sorted_dirs(base) else {
        return out;
    };
    for cat in cats {
        if attr_fn(&cat) & ATTR_HIDDEN != 0 {
            continue;
        }
        let Ok(items) = sorted_dirs(&cat) else {
            continue;
        };
        for d in items {
            if attr_fn(&d) & ATTR_HIDDEN != 0 {
                continue;
            }
            let result = audit_one_with_attrs(&d, fix, &attr_fn);
            if !result.issues.is_empty() {
                out.push(result);
            }
        }
    }
    out
}

/// 商品目录名匹配：`7 位 ID + 分隔符 + 名称`。
///
/// 分隔符为 `\s`（含全角空格）、下划线、ASCII 连字符、全角连字符、日文长音。
const ID_DIR_RE: &str = r"^(\d{7})[\s_\-－　ー]+(.+)$";
/// ID_DIR_RE 编译缓存。
static ID_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();

/// 递归扫描结果：单个商品目录（ID 目录名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedDir {
    /// 7 位商品 ID。
    pub id: String,
    /// 分隔符之后的目录名。
    pub name: String,
    /// 目录绝对路径。
    pub path: PathBuf,
    /// 缺失的三件套项（0-3 个）："封面"/"图标"/"ini"。
    pub missing: Vec<&'static str>,
    /// 目录名提取的版本标记（`extract_version_tag`）。
    pub local_tag: String,
}

/// 递归扫描 `root` 下所有 ID 目录，统计三件套缺失。
///
/// 遍历所有子目录（含深层嵌套），跳过隐藏目录（点前缀或 Windows 隐藏属性）与非目录项。
/// 与 `audit_tree` 不同：只按目录名匹配 ID，不做属性位/字段级检查。
pub fn scan_library(root: &Path) -> Vec<ScannedDir> {
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    walk_dirs(root, &mut out, &mut visited);
    // 按 id 稳定排序：跨平台目录枚举顺序不一致（Windows vs Linux/macOS），
    // 巡检结果必须有确定顺序，否则测试与后续处理依赖枚举顺序。
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// 递归遍历目录树：匹配 ID 目录名则记录，并继续深入所有非隐藏子目录。
fn walk_dirs(dir: &Path, out: &mut Vec<ScannedDir>, visited: &mut HashSet<PathBuf>) {
    // `is_dir` 会跟随符号链接和 Windows junction；按规范化路径去重以避免环路。
    if let Ok(canonical) = dir.canonicalize()
        && !visited.insert(canonical)
    {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_dir() || is_hidden(&path) {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && let Ok(Some(caps)) = ID_RE
                .get_or_init(|| Regex::new(ID_DIR_RE).expect("valid regex"))
                .captures(name)
            && let (Some(id), Some(nm)) = (caps.get(1), caps.get(2))
        {
            let mut missing = Vec::new();
            if !path.join(COVER_FILENAME).exists() {
                missing.push("封面");
            }
            if !folder_icon_present(&path) {
                missing.push("图标");
            }
            if ini_required() && !path.join(DESKTOP_INI).exists() {
                missing.push("ini");
            }
            out.push(ScannedDir {
                id: id.as_str().to_string(),
                name: nm.as_str().to_string(),
                path: path.clone(),
                missing,
                local_tag: extract_version_tag(name),
            });
        }
        walk_dirs(&path, out, visited);
    }
}

/// 是否隐藏目录：点前缀（跨平台约定）或 Windows 隐藏属性。
fn is_hidden(p: &Path) -> bool {
    let dot = p
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'));
    dot || default_attr(p) & ATTR_HIDDEN != 0
}

/// 目录下的子目录（按名称排序，过滤非目录项）。
fn sorted_dirs(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    Ok(dirs)
}

/// 版本号元组：取第一个数字段（`\d+(\.\d+)*`）拆成整数元组（如 `Ver_2.00` → [2, 0]）。
fn ver_tuple(tag: &str) -> Vec<u64> {
    let b = tag.as_bytes();
    let mut i = 0;
    while i < b.len() && !b[i].is_ascii_digit() {
        i += 1;
    }
    if i == b.len() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            break;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if let Ok(n) = tag[start..i].parse::<u64>() {
            parts.push(n);
        }
        if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
            i += 1;
            continue;
        }
        break;
    }
    parts
}

/// `a` 是否严格大于 `b`（按版本号数字逐段比较，缺段补 0）。
///
/// 任一侧无版本号返回 false。
pub fn ver_gt(a: &str, b: &str) -> bool {
    let ta = ver_tuple(a);
    let tb = ver_tuple(b);
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    for k in 0..ta.len().max(tb.len()) {
        let x = ta.get(k).copied().unwrap_or(0);
        let y = tb.get(k).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// 版本巡检结果：本地版本落后于官方版本的商品。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// 7 位商品 ID。
    pub id: String,
    /// 目录名分隔符之后的名称。
    pub name: String,
    /// 本地目录名提取的版本标记。
    pub local_tag: String,
    /// 官方商品名提取的版本标记。
    pub official_tag: String,
}

/// 版本巡检：经 `fetch` 注入联网比对官方版本号，返回本地落后于官方的商品。
///
/// 每件取官方商品名提取版本标记，与本地目录名版本标记比较，严格大于才记录。
pub fn version_audit(
    root: &Path,
    fetch: impl Fn(&str) -> Option<crate::fetch::ItemJson>,
) -> Vec<VersionInfo> {
    let mut out = Vec::new();
    for d in scan_library(root) {
        let Some(item) = fetch(&d.id) else {
            continue;
        };
        let official = extract_version_tag(&item.name);
        if ver_gt(&official, &d.local_tag) {
            out.push(VersionInfo {
                id: d.id,
                name: d.name,
                local_tag: d.local_tag,
                official_tag: official,
            });
        }
    }
    out
}

/// 错位检测结果：目录所在分类与官方分类不一致的商品。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MismatchInfo {
    /// 7 位商品 ID。
    pub id: String,
    /// 目录名分隔符之后的名称。
    pub name: String,
    /// 当前目录名（父目录名）。
    pub wrong_cat: String,
    /// 目标分类（官方分类映射）。
    pub dest_cat: String,
    /// 目录完整路径。
    pub path: String,
}

/// 错位检测：经 `fetch` 注入联网比对官方分类。
///
/// 目标分类与当前父目录名不一致、且目标分类非「未分类」时记录错位。
pub fn mismatch_audit(
    root: &Path,
    fetch: impl Fn(&str) -> Option<crate::fetch::ItemJson>,
) -> Vec<MismatchInfo> {
    let mut out = Vec::new();
    for d in scan_library(root) {
        let Some(item) = fetch(&d.id) else {
            continue;
        };
        let parent = item
            .category
            .parent
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("");
        let dest_cat = crate::classify::classify(&item.category.name, parent);
        if dest_cat == "未分类" {
            continue;
        }
        let wrong_cat = d
            .path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if wrong_cat != dest_cat {
            out.push(MismatchInfo {
                id: d.id,
                name: d.name,
                wrong_cat,
                dest_cat,
                path: d.path.display().to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bvt_audit_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn good_ini() -> &'static str {
        "[.ShellClassInfo]\r\nIconResource=.folder_icon.ico,0\r\n"
    }

    #[test]
    fn read_ini_utf16_le_bom() {
        let dir = tmpdir("u16le");
        let ini = dir.join("desktop.ini");
        let mut bytes = vec![0xFF, 0xFE];
        for u in good_ini().encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        std::fs::write(&ini, &bytes).unwrap();
        let text = read_ini_text(&ini);
        assert!(text.contains("IconResource=.folder_icon.ico"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_ini_utf16_be_bom() {
        let dir = tmpdir("u16be");
        let ini = dir.join("desktop.ini");
        let mut bytes = vec![0xFE, 0xFF];
        for u in good_ini().encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        std::fs::write(&ini, &bytes).unwrap();
        let text = read_ini_text(&ini);
        assert!(text.contains("IconResource=.folder_icon.ico"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_ini_utf8() {
        let dir = tmpdir("u8");
        let ini = dir.join("desktop.ini");
        std::fs::write(&ini, good_ini()).unwrap();
        let text = read_ini_text(&ini);
        assert!(text.contains("IconResource=.folder_icon.ico"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_ini_gbk() {
        let dir = tmpdir("gbk");
        let ini = dir.join("desktop.ini");
        let content = "[.ShellClassInfo]\r\nIconResource=.folder_icon.ico,0\r\n说明=中文备注\r\n";
        let bytes = encoding_rs::GBK.encode(content).0.into_owned();
        std::fs::write(&ini, &bytes).unwrap();
        let text = read_ini_text(&ini);
        assert!(text.contains("IconResource=.folder_icon.ico"));
        assert!(text.contains("说明"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_ini_missing_returns_empty() {
        assert_eq!(read_ini_text(Path::new("nonexistent_ini")), "");
    }

    #[test]
    fn audit_empty_dir_all_issues() {
        let dir = tmpdir("empty");
        let r = audit_one_with_attrs(&dir, false, |_| 0);
        assert_eq!(
            r.issues,
            vec![
                "ini 缺失".to_string(),
                "ico 缺失".to_string(),
                "文件夹属性 -（缺 R）".to_string(),
            ]
        );
        assert_eq!(r.suggested_fix, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_complete_folder_no_issues() {
        let dir = tmpdir("good");
        std::fs::write(dir.join("desktop.ini"), good_ini()).unwrap();
        std::fs::write(dir.join(".folder_icon.ico"), vec![0u8; 2048]).unwrap();
        std::fs::write(dir.join("cover.jpg"), vec![0u8; 100]).unwrap();
        let r = audit_one_with_attrs(&dir, true, |p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            match name {
                "desktop.ini" => ATTR_HIDDEN | ATTR_SYSTEM,
                ".folder_icon.ico" => ATTR_HIDDEN,
                _ => ATTR_READONLY,
            }
        });
        assert!(r.issues.is_empty());
        assert_eq!(r.suggested_fix, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_ini_missing_icon_resource_field() {
        let dir = tmpdir("nores");
        std::fs::write(
            dir.join("desktop.ini"),
            "[.ShellClassInfo]\r\nIconIndex=0\r\n",
        )
        .unwrap();
        std::fs::write(dir.join(".folder_icon.ico"), vec![0u8; 2048]).unwrap();
        let r = audit_one_with_attrs(&dir, false, |_| 0);
        assert!(
            r.issues
                .iter()
                .any(|i| i == "ini 缺 IconResource=.folder_icon.ico 字段"),
            "issues: {:?}",
            r.issues
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_ico_missing() {
        let dir = tmpdir("noico");
        std::fs::write(dir.join("desktop.ini"), good_ini()).unwrap();
        let r = audit_one_with_attrs(&dir, false, |_| 0);
        assert!(
            r.issues.iter().any(|i| i == "ico 缺失"),
            "issues: {:?}",
            r.issues
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_ico_too_small() {
        let dir = tmpdir("small");
        std::fs::write(dir.join("desktop.ini"), good_ini()).unwrap();
        std::fs::write(dir.join(".folder_icon.ico"), vec![0u8; 512]).unwrap();
        let r = audit_one_with_attrs(&dir, false, |_| 0);
        assert!(
            r.issues
                .iter()
                .any(|i| i.contains("ico 过小（<1KB，实际 512B）")),
            "issues: {:?}",
            r.issues
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_attr_missing_hs() {
        let dir = tmpdir("attrs");
        std::fs::write(dir.join("desktop.ini"), good_ini()).unwrap();
        std::fs::write(dir.join(".folder_icon.ico"), vec![0u8; 2048]).unwrap();
        // ini 缺 S、ico 缺 H、文件夹缺 R。
        let r = audit_one_with_attrs(&dir, false, |p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            match name {
                "desktop.ini" => ATTR_HIDDEN,
                ".folder_icon.ico" => ATTR_SYSTEM,
                _ => 0,
            }
        });
        assert!(
            r.issues.iter().any(|i| i == "ini 属性 H（缺 H/S）"),
            "issues: {:?}",
            r.issues
        );
        assert!(
            r.issues.iter().any(|i| i == "ico 属性 S（缺 H）"),
            "issues: {:?}",
            r.issues
        );
        assert!(
            r.issues.iter().any(|i| i == "文件夹属性 -（缺 R）"),
            "issues: {:?}",
            r.issues
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_fix_rewrite_when_cover() {
        let dir = tmpdir("fix_cover");
        std::fs::write(dir.join("cover.jpg"), vec![0u8; 10]).unwrap();
        let r = audit_one_with_attrs(&dir, true, |_| 0);
        assert_eq!(r.suggested_fix, Some(FixAction::Rewrite));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_fix_needs_cover() {
        let dir = tmpdir("fix_nocover");
        let r = audit_one_with_attrs(&dir, true, |_| 0);
        assert_eq!(r.suggested_fix, Some(FixAction::NeedsCover));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_fix_none_when_no_fix() {
        let dir = tmpdir("nofix");
        let r = audit_one_with_attrs(&dir, false, |_| 0);
        assert_eq!(r.suggested_fix, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn make_id_dir(base: &Path, name: &str) -> PathBuf {
        let d = base.join(name);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn scan_library_normal_dir() {
        let base = tmpdir("scan_ok");
        let d = make_id_dir(&base, "1234567_メカ弾");
        std::fs::write(d.join("cover.jpg"), vec![0u8; 100]).unwrap();
        std::fs::write(d.join(".folder_icon.ico"), vec![0u8; 2048]).unwrap();
        std::fs::write(d.join("desktop.ini"), good_ini()).unwrap();
        let out = scan_library(&base);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "1234567");
        assert_eq!(out[0].name, "メカ弾");
        assert!(out[0].missing.is_empty());
        assert_eq!(out[0].local_tag, "");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_missing_cover() {
        let base = tmpdir("scan_nocover");
        let d = make_id_dir(&base, "1234567_无封面");
        std::fs::write(d.join(".folder_icon.ico"), vec![0u8; 2048]).unwrap();
        std::fs::write(d.join("desktop.ini"), good_ini()).unwrap();
        let out = scan_library(&base);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].missing, vec!["封面"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_missing_icon() {
        let base = tmpdir("scan_noicon");
        let d = make_id_dir(&base, "1234567_无图标");
        std::fs::write(d.join("cover.jpg"), vec![0u8; 100]).unwrap();
        std::fs::write(d.join("desktop.ini"), good_ini()).unwrap();
        let out = scan_library(&base);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].missing, vec!["图标"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_missing_ini() {
        let base = tmpdir("scan_noini");
        let d = make_id_dir(&base, "1234567_无ini");
        std::fs::write(d.join("cover.jpg"), vec![0u8; 100]).unwrap();
        std::fs::write(d.join(".folder_icon.ico"), vec![0u8; 2048]).unwrap();
        let out = scan_library(&base);
        assert_eq!(out.len(), 1);
        if cfg!(target_os = "macos") {
            assert!(out[0].missing.is_empty());
        } else {
            assert_eq!(out[0].missing, vec!["ini"]);
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_missing_all() {
        let base = tmpdir("scan_none");
        make_id_dir(&base, "1234567_全缺");
        let out = scan_library(&base);
        assert_eq!(out.len(), 1);
        if cfg!(target_os = "macos") {
            assert_eq!(out[0].missing, vec!["封面", "图标"]);
        } else {
            assert_eq!(out[0].missing, vec!["封面", "图标", "ini"]);
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_skips_non_id_dirs() {
        let base = tmpdir("scan_skip");
        make_id_dir(&base, "未分类");
        make_id_dir(&base, "1234567"); // 无分隔符
        make_id_dir(&base, "123456_短id");
        make_id_dir(&base, "12345678_长id");
        make_id_dir(&base, "abcdefg_非数字");
        let out = scan_library(&base);
        assert!(out.is_empty(), "out: {:?}", out);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_separators() {
        let base = tmpdir("scan_sep");
        let seps = ["_", "-", " ", "　", "－", "ー"];
        for (i, s) in seps.iter().enumerate() {
            let id = format!("{i:07}");
            make_id_dir(&base, &format!("{id}{s}名称"));
        }
        let out = scan_library(&base);
        assert_eq!(out.len(), seps.len(), "out: {:?}", out);
        for (i, d) in out.iter().enumerate() {
            assert_eq!(d.id, format!("{i:07}"), "{d:?}");
            assert_eq!(d.name, "名称", "{d:?}");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_recurses_nested() {
        let base = tmpdir("scan_nest");
        let nested = base.join("分类").join("子分类");
        std::fs::create_dir_all(&nested).unwrap();
        let d = nested.join("2222222_深层");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("cover.jpg"), vec![0u8; 100]).unwrap();
        std::fs::write(d.join(".folder_icon.ico"), vec![0u8; 2048]).unwrap();
        std::fs::write(d.join("desktop.ini"), good_ini()).unwrap();
        let out = scan_library(&base);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "2222222");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_skips_hidden_dirs() {
        let base = tmpdir("scan_hidden");
        let hidden = make_id_dir(&base, ".1234567_隐藏");
        std::fs::write(hidden.join("cover.jpg"), vec![0u8; 100]).unwrap();
        let out = scan_library(&base);
        assert!(out.is_empty(), "out: {:?}", out);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_library_extracts_local_tag() {
        let base = tmpdir("scan_tag");
        make_id_dir(&base, "1234567_メカ弾v2_商品");
        let out = scan_library(&base);
        assert_eq!(out[0].local_tag, "Ver_2");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn audit_tree_reports_only_problem_dirs() {
        let base = tmpdir("tree");
        let cat = base.join("3D模型");
        let good = cat.join("1111111_good");
        let bad = cat.join("2222222_bad");
        let hidden_cat = base.join(".hidden_cat");
        std::fs::create_dir_all(&good).unwrap();
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::create_dir_all(&hidden_cat).unwrap();
        std::fs::write(good.join("desktop.ini"), good_ini()).unwrap();
        std::fs::write(good.join(".folder_icon.ico"), vec![0u8; 2048]).unwrap();
        // bad 与隐藏目录均为空；隐藏目录应被跳过。
        let results = audit_tree_with_attrs(&base, true, |p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            match name {
                "desktop.ini" => ATTR_HIDDEN | ATTR_SYSTEM,
                ".folder_icon.ico" => ATTR_HIDDEN,
                "1111111_good" => ATTR_READONLY,
                ".hidden_cat" => ATTR_HIDDEN,
                _ => 0,
            }
        });
        assert_eq!(results.len(), 1, "results: {:?}", results);
        assert_eq!(results[0].dir, bad);
        assert!(results[0].issues.iter().any(|i| i == "ini 缺失"));
        assert_eq!(results[0].suggested_fix, Some(FixAction::NeedsCover));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn scan_library_skips_symlink_cycles() {
        let base = tmpdir("scan_cycle");
        make_id_dir(&base, "1234567_商品");
        std::os::unix::fs::symlink(&base, base.join("cycle")).unwrap();

        let out = scan_library(&base);

        assert_eq!(out.len(), 1, "out: {out:?}");
        let _ = std::fs::remove_dir_all(&base);
    }

    fn json_item(name: &str) -> crate::fetch::ItemJson {
        crate::fetch::ItemJson {
            name: name.to_string(),
            ..crate::fetch::ItemJson::default()
        }
    }

    #[test]
    fn ver_gt_basic() {
        assert!(ver_gt("1.0", "0.9"));
        assert!(!ver_gt("0.9", "1.0"));
        assert!(!ver_gt("1.0", "1.0"));
        assert!(ver_gt("2.0", "1.9"));
    }

    #[test]
    fn ver_gt_empty() {
        assert!(!ver_gt("", "1.0"));
        assert!(!ver_gt("1.0", ""));
        assert!(!ver_gt("", ""));
    }

    #[test]
    fn ver_gt_multi_segment() {
        assert!(ver_gt("1.0.1", "1.0"));
        assert!(!ver_gt("1.0", "1.0.1"));
        assert!(!ver_gt("1.0.2", "1.0.10"));
        assert!(ver_gt("1.0.10", "1.0.9"));
    }

    #[test]
    fn ver_gt_padding_zero() {
        assert!(ver_gt("1.5.1", "1.5"));
        assert!(!ver_gt("1.5", "1.5.0"));
        assert!(!ver_gt("Ver_2.00", "Ver_2"));
    }

    #[test]
    fn ver_gt_first_number_segment() {
        assert!(ver_gt("v3.0", "名前0.5"));
        assert!(!ver_gt("名前0.5", "v3.0"));
    }

    #[test]
    fn version_audit_reports_updateable() {
        let base = tmpdir("va_update");
        make_id_dir(&base, "1111111_雪女v1");
        make_id_dir(&base, "2222222_メカv3");
        let out = version_audit(&base, |id| match id {
            "1111111" => Some(json_item("雪女v2")),
            "2222222" => Some(json_item("メカv2")),
            _ => None,
        });
        assert_eq!(out.len(), 1, "out: {:?}", out);
        assert_eq!(out[0].id, "1111111");
        assert_eq!(out[0].local_tag, "Ver_1");
        assert_eq!(out[0].official_tag, "Ver_2");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn version_audit_no_version_skipped() {
        let base = tmpdir("va_nover");
        make_id_dir(&base, "3333333_无版本");
        make_id_dir(&base, "4444444_plain");
        let out = version_audit(&base, |_| Some(json_item("无版本商品")));
        assert!(out.is_empty(), "out: {:?}", out);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn version_audit_empty_library() {
        let base = tmpdir("va_empty");
        let out = version_audit(&base, |_| Some(json_item("雪女Ver_2")));
        assert!(out.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn version_audit_fetch_none_skipped() {
        let base = tmpdir("va_none");
        make_id_dir(&base, "5555555_脱机");
        let out = version_audit(&base, |_| None);
        assert!(out.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    fn json_item_cat(cat: &str) -> crate::fetch::ItemJson {
        crate::fetch::ItemJson {
            name: "商品".to_string(),
            category: crate::fetch::CategoryJson {
                name: cat.to_string(),
                parent: None,
            },
            ..crate::fetch::ItemJson::default()
        }
    }

    #[test]
    fn mismatch_audit_reports_wrong_category() {
        let base = tmpdir("ma_wrong");
        let cat = base.join("3D模型");
        std::fs::create_dir_all(&cat).unwrap();
        make_id_dir(&cat, "1111111_错位");
        let out = mismatch_audit(&base, |id| match id {
            "1111111" => Some(json_item_cat("髪")),
            _ => None,
        });
        assert_eq!(out.len(), 1, "out: {:?}", out);
        assert_eq!(out[0].id, "1111111");
        assert_eq!(out[0].wrong_cat, "3D模型");
        assert_eq!(out[0].dest_cat, "3D发型");
        assert_eq!(out[0].path, cat.join("1111111_错位").display().to_string());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn mismatch_audit_skips_correct_category() {
        let base = tmpdir("ma_ok");
        let cat = base.join("3D发型");
        std::fs::create_dir_all(&cat).unwrap();
        make_id_dir(&cat, "2222222_正确");
        let out = mismatch_audit(&base, |id| match id {
            "2222222" => Some(json_item_cat("髪")),
            _ => None,
        });
        assert!(out.is_empty(), "out: {:?}", out);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn mismatch_audit_skips_uncategorized() {
        let base = tmpdir("ma_uncat");
        let cat = base.join("3D模型");
        std::fs::create_dir_all(&cat).unwrap();
        make_id_dir(&cat, "3333333_未分类");
        let out = mismatch_audit(&base, |id| match id {
            "3333333" => Some(json_item_cat("")),
            _ => None,
        });
        assert!(out.is_empty(), "out: {:?}", out);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn mismatch_audit_uses_parent_classify() {
        let base = tmpdir("ma_parent");
        let cat = base.join("3D模型");
        std::fs::create_dir_all(&cat).unwrap();
        make_id_dir(&cat, "4444444_带父类");
        let item = crate::fetch::ItemJson {
            name: "商品".to_string(),
            category: crate::fetch::CategoryJson {
                name: "衣装".to_string(),
                parent: Some(crate::fetch::ParentCategory {
                    name: "3Dモデル".to_string(),
                }),
            },
            ..crate::fetch::ItemJson::default()
        };
        let out = mismatch_audit(&base, |id| {
            if id == "4444444" {
                Some(item.clone())
            } else {
                None
            }
        });
        assert_eq!(out.len(), 1, "out: {:?}", out);
        assert_eq!(out[0].wrong_cat, "3D模型");
        assert_eq!(out[0].dest_cat, "3D服饰");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn mismatch_audit_empty_library() {
        let base = tmpdir("ma_empty");
        let out = mismatch_audit(&base, |_| Some(json_item_cat("髪")));
        assert!(out.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
