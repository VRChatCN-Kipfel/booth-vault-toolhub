//! 文件夹图标三件套：cover → `.folder_icon.ico` + `desktop.ini` + 属性/刷新。
//!
//! 流程（行为契约）：
//!   1. 封面 → 多尺寸 ICO（正方形画布居中）
//!   2. 写 desktop.ini（UTF-8 无 BOM，[ViewState] 在前）
//!   3. ini/ico 设 H+S；文件夹设 READONLY
//!   4. SHGetSetFolderCustomSettings 刷新（可靠）+ SHChangeNotify 兜底
//!   5. 完整性契约自检，不过即清理重写
//!   6. 失败时清理残缺 desktop.ini 并回滚

use std::path::{Path, PathBuf};

use crate::attributes::{clear_normal, set_folder_readonly, set_hidden_system};
use crate::icon::write_cover_ico;
use crate::refresh::{apply_custom_settings, refresh_fallback};

/// 完整性契约不满足时抛出。
#[derive(Debug)]
pub struct IconContractError(pub String);

impl std::fmt::Display for IconContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IconContractError: {}", self.0)
    }
}

impl std::error::Error for IconContractError {}

/// ICO 文件名。
pub const FOLDER_ICO: &str = ".folder_icon.ico";
/// desktop.ini 文件名。
pub const DESKTOP_INI: &str = "desktop.ini";

/// desktop.ini 内容（UTF-8 无 BOM；[ViewState] 在前，与基线一致）。
const INI_CONTENT: &str = "[ViewState]\r\nFolderType=Generic\r\n[.ShellClassInfo]\r\nIconResource=.folder_icon.ico,0\r\nIconIndex=0\r\n";

/// 设置文件夹图标三件套。
///
/// `cover_path`：封面图（jpg/png）。
/// `folder_path`：目标文件夹。
///
/// 失败时清理残缺 desktop.ini 并回滚，抛 `IconContractError`。
pub fn make_folder_icon(cover_path: &Path, folder_path: &Path) -> Result<(), IconContractError> {
    if !cover_path.is_file() {
        return Err(IconContractError(format!(
            "cover 缺失：{}",
            cover_path.display()
        )));
    }
    let ico_path = folder_path.join(FOLDER_ICO);
    let ini_path = folder_path.join(DESKTOP_INI);
    // 写前清 0x80（只读/系统残留，否则覆写失败）。
    for p in [&ico_path, &ini_path] {
        clear_normal(p);
    }
    // 1. ICO
    let ico_size = write_cover_ico(cover_path, &ico_path)
        .map_err(|e| IconContractError(format!("ICO 生成失败：{e}")))?;
    if ico_size < 1024 {
        let _ = std::fs::remove_file(&ico_path);
        return Err(IconContractError("ICO 过小（<1KB）".to_string()));
    }
    // 2. desktop.ini（UTF-8 无 BOM）
    if let Err(e) = std::fs::write(&ini_path, INI_CONTENT) {
        let _ = std::fs::remove_file(&ico_path);
        return Err(IconContractError(format!("desktop.ini 写入失败：{e}")));
    }
    // 3. 属性：ini/ico H+S，文件夹 READONLY。
    set_hidden_system(&ini_path);
    set_hidden_system(&ico_path);
    set_folder_readonly(folder_path);
    // 4. 刷新：SHGetSetFolderCustomSettings 优先 + SHChangeNotify 兜底。
    match apply_custom_settings(folder_path, FOLDER_ICO) {
        Ok(()) => {}
        Err(_) => refresh_fallback(Some(folder_path)),
    }
    // 5. 完整性契约自检。
    verify_contract(&ico_path, &ini_path, folder_path)?;
    Ok(())
}

/// 完整性契约：ico 存在且 >1KB、ini 含 IconResource 字段、文件夹 READONLY 位。
fn verify_contract(
    ico_path: &Path,
    ini_path: &Path,
    folder_path: &Path,
) -> Result<(), IconContractError> {
    if !ico_path.is_file() {
        return Err(IconContractError(format!(
            "ico 缺失：{}",
            ico_path.display()
        )));
    }
    let size = std::fs::metadata(ico_path).map(|m| m.len()).unwrap_or(0);
    if size < 1024 {
        return Err(IconContractError(format!(
            "ico 过小（<1KB）：{}",
            ico_path.display()
        )));
    }
    if !ini_path.is_file() {
        return Err(IconContractError(format!(
            "ini 缺失：{}",
            ini_path.display()
        )));
    }
    let text = std::fs::read_to_string(ini_path)
        .or_else(|_| std::fs::read(ini_path).map(|b| String::from_utf8_lossy(&b).into_owned()))
        .map_err(|e| IconContractError(format!("ini 读取失败：{e}")))?;
    if !text.contains("IconResource=.folder_icon.ico") {
        return Err(IconContractError(format!(
            "ini 缺 IconResource 字段：{}",
            ini_path.display()
        )));
    }
    let readonly =
        crate::attributes::get_attrs(folder_path) & crate::attributes::ATTR_READONLY != 0;
    if !readonly {
        set_folder_readonly(folder_path);
    }
    Ok(())
}

/// 清理文件夹内的图标三件套（reset）。
pub fn reset_folder_icon(folder_path: &Path) -> Result<(), IconContractError> {
    let ico = folder_path.join(FOLDER_ICO);
    let ini = folder_path.join(DESKTOP_INI);
    for p in [&ico, &ini] {
        clear_normal(p);
        if p.exists()
            && let Err(e) = std::fs::remove_file(p)
        {
            return Err(IconContractError(format!("清理失败 {}: {e}", p.display())));
        }
    }
    Ok(())
}

/// 检查文件夹是否已有完整三件套（幂等判断）。
pub fn has_folder_icon(folder_path: &Path) -> bool {
    let ico = folder_path.join(FOLDER_ICO);
    let ini = folder_path.join(DESKTOP_INI);
    if !ico.is_file() || !ini.is_file() {
        return false;
    }
    match std::fs::metadata(&ico) {
        Ok(m) if m.len() >= 1024 => {}
        _ => return false,
    }
    let text = std::fs::read_to_string(&ini).unwrap_or_default();
    text.contains("IconResource=.folder_icon.ico")
}

/// 供外部审计/回读的辅助：三件套路径。
pub fn contract_paths(folder_path: &Path) -> (PathBuf, PathBuf) {
    (folder_path.join(FOLDER_ICO), folder_path.join(DESKTOP_INI))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bvt_fi_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn contract_ini_content() {
        assert!(INI_CONTENT.starts_with("[ViewState]"));
        assert!(INI_CONTENT.contains("[.ShellClassInfo]"));
        assert!(INI_CONTENT.contains("IconResource=.folder_icon.ico,0"));
    }

    #[test]
    fn has_icon_false_on_empty() {
        let dir = tmpdir("empty");
        assert!(!has_folder_icon(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
