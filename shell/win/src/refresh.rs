//! Explorer 图标刷新：SHGetSetFolderCustomSettings 优先 + SHChangeNotify 兜底。
//!
//! 事件码修正：Python 基线用 `SHCNE_MKDIR(0x8)`/`SHCNE_UPDATEIMAGE(0x8000)` 刷新
//! 文件夹图标是**错误事件**，且 Win11 实测 SHChangeNotify 刷新文件夹图标不可靠。
//! 正确组合：`SHCNE_UPDATEITEM(0x2000)` + `SHCNE_UPDATEDIR(0x1000)` 兜底；
//! 真正可靠路径是 `SHGetSetFolderCustomSettings`（FCSM_ICONFILE|FCS_FORCEWRITE）。

use std::path::Path;

use windows::Win32::UI::Shell::{
    FCS_FORCEWRITE, FCSM_ICONFILE, SHCNE_ID, SHCNE_UPDATEDIR, SHCNE_UPDATEITEM, SHCNF_IDLIST,
    SHChangeNotify, SHFOLDERCUSTOMSETTINGS, SHGetSetFolderCustomSettings,
};
use windows::core::PCWSTR;

/// 通过 shell 写 desktop.ini 的 IconResource 并刷新。
///
/// `icon_file_name`：.ico 文件名（相对路径名，shell 会写进 desktop.ini）。
/// 优先此路径（可靠）；调用方应配合 `refresh_fallback` 兜底。
pub fn apply_custom_settings(folder: &Path, icon_file_name: &str) -> Result<(), String> {
    let folder_wide = wide(folder);
    let icon_wide = wide(Path::new(icon_file_name));
    let mut settings = SHFOLDERCUSTOMSETTINGS {
        dwSize: std::mem::size_of::<SHFOLDERCUSTOMSETTINGS>() as u32,
        dwMask: FCSM_ICONFILE,
        pszIconFile: windows::core::PWSTR(icon_wide.as_ptr() as *mut u16),
        ..Default::default()
    };
    // 注意：pszIconFile 生命周期须覆盖本次调用（icon_wide 存活至调用后）。
    let hr = unsafe {
        SHGetSetFolderCustomSettings(
            &mut settings,
            PCWSTR(folder_wide.as_ptr()),
            FCSM_ICONFILE | FCS_FORCEWRITE,
        )
    };
    hr.map_err(|e| format!("SHGetSetFolderCustomSettings failed: {e}"))
}

/// SHChangeNotify 兜底刷新（正确事件码组合）。
///
/// `folder` 为 Some 时发单目录 UPDATEITEM+UPDATEDIR；随后总发全局 ASSOCCHANGED。
pub fn refresh_fallback(folder: Option<&Path>) {
    if let Some(f) = folder {
        let wide = wide(f);
        unsafe {
            SHChangeNotify(
                SHCNE_UPDATEITEM | SHCNE_UPDATEDIR,
                SHCNF_IDLIST,
                Some(wide.as_ptr() as *const core::ffi::c_void),
                None,
            );
        }
    }
    // 全局兜底：ASSOCCHANGED（0x08000000）
    unsafe {
        SHChangeNotify(SHCNE_ID(134_217_728), SHCNF_IDLIST, None, None);
    }
}

/// 转 UTF-16 空终止。
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let mut v: Vec<u16> = path.as_os_str().encode_wide().collect();
    v.push(0);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_terminates() {
        let v = wide(Path::new(r"C:\x"));
        assert_eq!(v.last(), Some(&0));
    }
}
