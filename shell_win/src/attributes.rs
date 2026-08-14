//! 文件属性位操作（Windows）。

use std::path::Path;

use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_SYSTEM,
    GetFileAttributesW, SetFileAttributesW,
};
use windows::core::PCWSTR;

/// HIDDEN 0x02。
pub const ATTR_HIDDEN: u32 = FILE_ATTRIBUTE_HIDDEN.0;
/// SYSTEM 0x04。
pub const ATTR_SYSTEM: u32 = FILE_ATTRIBUTE_SYSTEM.0;
/// READONLY 0x01。
pub const ATTR_READONLY: u32 = FILE_ATTRIBUTE_READONLY.0;
/// NORMAL 0x80。
pub const ATTR_NORMAL: u32 = FILE_ATTRIBUTE_NORMAL.0;

/// 读取属性位；失败返回 0（INVALID_FILE_ATTRIBUTES 归 0）。
pub fn get_attrs(path: &Path) -> u32 {
    let wide = wide(path);
    let pcw = PCWSTR(wide.as_ptr());
    unsafe { GetFileAttributesW(pcw) }
}

/// 设置属性位（整个覆盖）。失败返回 false。
pub fn set_attrs(path: &Path, attrs: u32) -> bool {
    let wide = wide(path);
    let pcw = PCWSTR(wide.as_ptr());
    unsafe {
        SetFileAttributesW(
            pcw,
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(attrs),
        )
        .is_ok()
    }
}

/// H+S 同设：在现有属性上追加 HIDDEN 与 SYSTEM（Explorer 拒读 desktop.ini 的坑：只设 H 漏 S）。
pub fn set_hidden_system(path: &Path) -> bool {
    let attrs = get_attrs(path);
    set_attrs(path, attrs | ATTR_HIDDEN | ATTR_SYSTEM)
}

/// 清 0x80=NORMAL 位（写前清只读/系统残留，否则覆写失败）。
pub fn clear_normal(path: &Path) -> bool {
    let attrs = get_attrs(path);
    set_attrs(path, attrs & !ATTR_NORMAL)
}

/// 文件夹设 READONLY 位（Explorer 应用 desktop.ini 所需）。
pub fn set_folder_readonly(path: &Path) -> bool {
    let attrs = get_attrs(path);
    set_attrs(path, attrs | ATTR_READONLY)
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
        let v = wide(Path::new(r"C:\test"));
        assert_eq!(v.last(), Some(&0));
        assert!(v.len() >= 7);
    }

    #[test]
    fn constants_sane() {
        assert_eq!(ATTR_HIDDEN, 0x02);
        assert_eq!(ATTR_SYSTEM, 0x04);
        assert_eq!(ATTR_READONLY, 0x01);
        assert_eq!(ATTR_NORMAL, 0x80);
    }
}
