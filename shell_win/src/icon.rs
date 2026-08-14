//! ICO 生成：封面 → 多尺寸 ICO（正方形画布居中）。

use std::path::Path;

/// ICO 尺寸档（血泪坑：宽幅封面须先贴 max(w,h) 透明正方形画布居中）。
pub const ICO_SIZES: [(u32, u32); 6] = [
    (256, 256),
    (128, 128),
    (64, 64),
    (48, 48),
    (32, 32),
    (16, 16),
];

/// 把封面转为多尺寸 ICO 并写入 `ico_path`。
///
/// 非正方形封面先贴 `max(w,h)` 透明正方形画布居中，再各尺寸缩放。
/// 返回写出的字节数；失败返回 Err（调用方负责清理）。
pub fn write_cover_ico(cover_path: &Path, ico_path: &Path) -> Result<u64, String> {
    let img = image::open(cover_path)
        .map_err(|e| format!("open cover {}: {e}", cover_path.display()))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let side = w.max(h);
    // 透明正方形画布居中（血泪坑：宽幅 cover 直接存 ICO 生成非正方形缩略图居中小图）。
    let mut canvas = image::RgbaImage::from_pixel(side, side, image::Rgba([0, 0, 0, 0]));
    let dx = (side - w) / 2;
    let dy = (side - h) / 2;
    for (x, y, p) in img.enumerate_pixels() {
        canvas.put_pixel(x + dx, y + dy, *p);
    }
    // 各尺寸：resize → IcoFrame（as_png 内部做 PNG 编码，buf 传原始像素）。
    let mut frames: Vec<image::codecs::ico::IcoFrame> = Vec::new();
    for &(sw, sh) in &ICO_SIZES {
        let resized =
            image::imageops::resize(&canvas, sw, sh, image::imageops::FilterType::Lanczos3);
        let frame = image::codecs::ico::IcoFrame::as_png(
            resized.as_raw(),
            sw,
            sh,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("ico frame {sw}x{sh}: {e}"))?;
        frames.push(frame);
    }
    let mut out = std::fs::File::create(ico_path).map_err(|e| format!("create ico: {e}"))?;
    let encoder = image::codecs::ico::IcoEncoder::new(&mut out);
    encoder
        .encode_images(&frames)
        .map_err(|e| format!("encode ico: {e}"))?;
    let size = std::fs::metadata(ico_path)
        .map(|m| m.len())
        .map_err(|e| format!("stat ico: {e}"))?;
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_const() {
        assert_eq!(ICO_SIZES[0], (256, 256));
        assert_eq!(ICO_SIZES[5], (16, 16));
        assert_eq!(ICO_SIZES.len(), 6);
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bvt_icon_{tag}_{}_{}",
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
    fn write_ico_square_cover() {
        let dir = tmpdir("sq");
        let cover = dir.join("cover.png");
        let ico = dir.join(".folder_icon.ico");
        let img = image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
        img.save(&cover).unwrap();
        let size = write_cover_ico(&cover, &ico).unwrap();
        assert!(size > 1024, "ico should be > 1KB, got {size}");
        assert!(ico.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_ico_wide_cover() {
        let dir = tmpdir("wide");
        let cover = dir.join("cover.png");
        let ico = dir.join(".folder_icon.ico");
        let img = image::RgbaImage::from_pixel(64, 32, image::Rgba([0, 255, 0, 255]));
        img.save(&cover).unwrap();
        let size = write_cover_ico(&cover, &ico).unwrap();
        assert!(size > 1024);
        assert!(ico.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
