//! 文件夹图标三件套完整性巡检（纯文件系统检查，无 Windows FFI）。
//!
//! 属性位检查经回调注入：`audit_one_with_attrs` 是平台无关的纯逻辑核心，
//! `audit_one` / `audit_tree` 提供平台默认（Windows 下读取真实属性，
//! 其余平台返回 0）。desktop.ini 按 utf-16 → utf-8 → gbk 三编码读回，
//! 兼容历史遗留编码契约（当前写入为 UTF-8 无 BOM，旧版曾写 UTF-16）。

use std::path::{Path, PathBuf};

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
    audit_one_with_attrs(dir, fix, default_attr)
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
    audit_tree_with_attrs(base, default_attr)
}

/// `audit_tree` 的属性注入版本（平台无关，便于测试）。
fn audit_tree_with_attrs(base: &Path, attr_fn: impl Fn(&Path) -> u32) -> Vec<AuditResult> {
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
            let result = audit_one_with_attrs(&d, false, &attr_fn);
            if !result.issues.is_empty() {
                out.push(result);
            }
        }
    }
    out
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
        let results = audit_tree_with_attrs(&base, |p| {
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
        let _ = std::fs::remove_dir_all(&base);
    }
}
