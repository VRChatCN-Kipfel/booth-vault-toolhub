//! Finder 自定义图标：正方形 PNG + NSWorkspace setIcon。

use std::path::Path;
use std::process::Command;

/// Finder 自定义图标资源文件名（`Icon` + CR）。
pub const ICON_R: &str = "Icon\r";

/// 错误包装，与 shell_win 对齐。
#[derive(Debug)]
pub struct IconContractError(pub String);

impl std::fmt::Display for IconContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for IconContractError {}

/// 封面 → 正方形 PNG → Finder 自定义图标。
pub fn make_folder_icon(cover_path: &Path, folder_path: &Path) -> Result<(), IconContractError> {
    if !cover_path.is_file() {
        return Err(IconContractError(format!(
            "封面不存在：{}",
            cover_path.display()
        )));
    }
    if !folder_path.is_dir() {
        return Err(IconContractError(format!(
            "目录不存在：{}",
            folder_path.display()
        )));
    }
    let png = folder_path.join(".folder_icon.png");
    write_square_png(cover_path, &png)?;
    set_finder_icon(&png, folder_path)?;
    let _ = Command::new("chflags").args(["hidden"]).arg(&png).status();
    if !has_folder_icon(folder_path) && !png.is_file() {
        return Err(IconContractError(
            "Finder 未接受自定义图标（setIcon 失败）".to_string(),
        ));
    }
    Ok(())
}

/// 去掉 Finder 自定义图标。
pub fn reset_folder_icon(folder_path: &Path) -> Result<(), IconContractError> {
    let script = format!(
        r#"use framework "AppKit"
current application's NSWorkspace's sharedWorkspace()'s setIcon:(missing value) forFile:"{}" options:0
"#,
        applescript_str(&folder_path.to_string_lossy())
    );
    run_osascript(&script)?;
    let icon_r = folder_path.join(ICON_R);
    if icon_r.exists() {
        let _ = std::fs::remove_file(&icon_r);
    }
    let png = folder_path.join(".folder_icon.png");
    if png.exists() {
        let _ = std::fs::remove_file(&png);
    }
    Ok(())
}

/// 是否已有 Finder 自定义图标（`Icon\r` 或我们写下的正方形 PNG）。
pub fn has_folder_icon(folder_path: &Path) -> bool {
    folder_path.join(ICON_R).is_file() || folder_path.join(".folder_icon.png").is_file()
}

/// 宽幅封面贴 max(w,h) 透明正方形画布居中，写出 PNG。
pub fn write_square_png(cover_path: &Path, png_path: &Path) -> Result<(), IconContractError> {
    let img = image::open(cover_path)
        .map_err(|e| IconContractError(format!("open cover {}: {e}", cover_path.display())))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let side = w.max(h);
    let mut canvas = image::RgbaImage::from_pixel(side, side, image::Rgba([0, 0, 0, 0]));
    let dx = (side - w) / 2;
    let dy = (side - h) / 2;
    for (x, y, p) in img.enumerate_pixels() {
        canvas.put_pixel(x + dx, y + dy, *p);
    }
    canvas
        .save(png_path)
        .map_err(|e| IconContractError(format!("write png: {e}")))?;
    Ok(())
}

fn set_finder_icon(image_path: &Path, folder_path: &Path) -> Result<(), IconContractError> {
    let img = applescript_str(&image_path.to_string_lossy());
    let folder = applescript_str(&folder_path.to_string_lossy());
    let script = format!(
        r#"use framework "AppKit"
set theImage to current application's NSImage's alloc()'s initWithContentsOfFile:"{img}"
if theImage is missing value then error "NSImage load failed"
current application's NSWorkspace's sharedWorkspace()'s setIcon:theImage forFile:"{folder}" options:0
"#
    );
    run_osascript(&script)
}

fn run_osascript(script: &str) -> Result<(), IconContractError> {
    let out = Command::new("osascript")
        .args(["-l", "AppleScript", "-e", script])
        .output()
        .map_err(|e| IconContractError(format!("osascript: {e}")))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(IconContractError(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

fn applescript_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bvt_macicon_{tag}_{}_{}",
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
    fn write_square_png_pads_wide_cover() {
        let dir = tmpdir("wide");
        let cover = dir.join("cover.png");
        let out = dir.join("sq.png");
        let img = image::RgbaImage::from_pixel(64, 32, image::Rgba([0, 255, 0, 255]));
        img.save(&cover).unwrap();
        write_square_png(&cover, &out).unwrap();
        let got = image::open(&out).unwrap();
        assert_eq!(got.width(), got.height());
        assert_eq!(got.width(), 64);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn has_folder_icon_false_on_empty() {
        let dir = tmpdir("empty");
        assert!(!has_folder_icon(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn has_folder_icon_true_when_png_present() {
        let dir = tmpdir("png");
        std::fs::write(dir.join(".folder_icon.png"), [0u8; 8]).unwrap();
        assert!(has_folder_icon(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
