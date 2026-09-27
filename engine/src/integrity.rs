//! 本地压缩包完整性校验：识别中断下载残留的半截文件。
//!
//! 「存在且非空且非 HTML 伪装即有效」这一幂等契约不足以排除半截文件：
//! 断下载留下的 zip/unitypackage size>0 且魔数正常，会被判为已完成而永久跳过，
//! 换节点/换代理重跑仍不重下。故对可判定格式补一层真实解析校验。

use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use tar::Archive;

/// 可校验的扩展名（rar/7z 无内置解析器，不在其中）。
const CHECKABLE_EXTS: [&str; 2] = ["zip", "unitypackage"];

/// 扫描库目录时识别的商品目录名前缀：BOOTH ID 位数。
const ID_DIR_RE_LEN: (usize, usize) = (5, 8);

/// 损坏包记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorruptFile {
    pub path: PathBuf,
    pub id: String,
    pub size: u64,
}

/// 扩展名是否可校验。
pub fn is_checkable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| CHECKABLE_EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
        .unwrap_or(false)
}

/// 包是否损坏：可校验格式解析失败即损坏，不可校验格式一律 false（不误报）。
///
/// zip 走中央目录读取（截断/坏包在此暴露），unitypackage 解 gzip+tar 取首条目；
/// 空文件与不存在文件同样判损坏，调用方无需再前置 size 检查。
pub fn is_corrupt_package(path: &Path) -> bool {
    if !is_checkable(path) {
        return false;
    }
    let Ok(meta) = std::fs::metadata(path) else {
        return true;
    };
    if meta.len() == 0 {
        return true;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "zip" {
        return !matches!(zip_probe(path), Ok(true));
    }
    !matches!(unitypackage_probe(path), Ok(true))
}

fn zip_probe(path: &Path) -> Result<bool, std::io::Error> {
    let fh = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(fh).map_err(std::io::Error::other)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(std::io::Error::other)?;
        let mut sink = std::io::sink();
        std::io::copy(&mut entry, &mut sink)?;
    }
    Ok(true)
}

fn unitypackage_probe(path: &Path) -> Result<bool, std::io::Error> {
    let fh = std::fs::File::open(path)?;
    let dec = GzDecoder::new(fh);
    let mut archive = Archive::new(dec);
    let mut entries = archive.entries()?;
    let Some(first) = entries.next() else {
        return Ok(true);
    };
    let mut entry = first?;
    let mut sink = std::io::sink();
    std::io::copy(&mut entry, &mut sink)?;
    Ok(true)
}

/// 扫描库内 `{5-8位数字}_` 商品目录，返回其中损坏的 zip/unitypackage。
pub fn scan_corrupt_in_library(root: &Path) -> Vec<CorruptFile> {
    let Ok(rd) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for group in rd.filter_map(Result::ok) {
        let group_path = group.path();
        if !group_path.is_dir() {
            continue;
        }
        let Ok(items) = std::fs::read_dir(&group_path) else {
            continue;
        };
        for item in items.filter_map(Result::ok) {
            let item_path = item.path();
            if !item_path.is_dir() {
                continue;
            }
            let name = item.file_name();
            let name = name.to_string_lossy();
            let Some(id) = item_id_of(&name) else {
                continue;
            };
            let Ok(files) = std::fs::read_dir(&item_path) else {
                continue;
            };
            for f in files.filter_map(Result::ok) {
                let fp = f.path();
                if !fp.is_file() || !is_checkable(&fp) {
                    continue;
                }
                if is_corrupt_package(&fp) {
                    let size = std::fs::metadata(&fp).map(|m| m.len()).unwrap_or(0);
                    out.push(CorruptFile {
                        path: fp,
                        id: id.clone(),
                        size,
                    });
                }
            }
        }
    }
    out
}

/// 从目录名取 BOOTH ID（前导数字长度在契约范围内且后接下划线）。
fn item_id_of(dir_name: &str) -> Option<String> {
    let digits: String = dir_name
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let (lo, hi) = ID_DIR_RE_LEN;
    if digits.len() < lo || digits.len() > hi {
        return None;
    }
    if !dir_name[digits.len()..].starts_with('_') {
        return None;
    }
    Some(digits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    /// tag 置于末段：扩展名需落在路径末尾才能被 `is_checkable` 识别。
    fn tmp(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "bvt_integrity_{}_{}_{tag}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn good_zip() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            w.start_file("a.txt", o).unwrap();
            w.write_all(b"hello").unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    fn good_upk() -> Vec<u8> {
        let mut tar_buf = Vec::new();
        {
            let mut b = tar::Builder::new(&mut tar_buf);
            let mut h = tar::Header::new_gnu();
            let data = b"Assets/X.prefab";
            h.set_size(data.len() as u64);
            h.set_cksum();
            b.append_data(&mut h, "guid/pathname", &data[..]).unwrap();
            b.finish().unwrap();
        }
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(&tar_buf).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn empty_and_missing_are_corrupt() {
        let p = tmp("empty.zip");
        std::fs::write(&p, b"").unwrap();
        assert!(is_corrupt_package(&p));
        let _ = std::fs::remove_file(&p);
        assert!(is_corrupt_package(&p));
    }

    #[test]
    fn truncated_zip_is_corrupt() {
        let full = good_zip();
        let p = tmp("trunc.zip");
        std::fs::write(&p, &full[..full.len() / 2]).unwrap();
        assert!(is_corrupt_package(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn good_zip_is_not_corrupt() {
        let p = tmp("okzip.zip");
        std::fs::write(&p, good_zip()).unwrap();
        assert!(!is_corrupt_package(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn truncated_unitypackage_is_corrupt() {
        let full = good_upk();
        let p = tmp("truncupk.unitypackage");
        std::fs::write(&p, &full[..full.len() / 2]).unwrap();
        assert!(is_corrupt_package(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn good_unitypackage_is_not_corrupt() {
        let p = tmp("okupk.unitypackage");
        std::fs::write(&p, good_upk()).unwrap();
        assert!(!is_corrupt_package(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn uncheckable_ext_never_corrupt() {
        let p = tmp("junk.rar");
        std::fs::write(&p, b"\x00\x01garbage").unwrap();
        assert!(!is_corrupt_package(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn scan_finds_corrupt_in_id_dirs() {
        let root = tmp("root");
        let item = root.join("3D饰品").join("1234567_Test");
        std::fs::create_dir_all(&item).unwrap();
        let bad = item.join("pack.zip");
        let full = good_zip();
        std::fs::write(&bad, &full[..full.len() / 2]).unwrap();
        let good = item.join("ok.zip");
        std::fs::write(&good, good_zip()).unwrap();
        let found = scan_corrupt_in_library(&root);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, bad);
        assert_eq!(found[0].id, "1234567");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_ignores_non_id_dirs() {
        let root = tmp("root2");
        let item = root.join("3D饰品").join("misc");
        std::fs::create_dir_all(&item).unwrap();
        let full = good_zip();
        std::fs::write(item.join("pack.zip"), &full[..full.len() / 2]).unwrap();
        assert!(scan_corrupt_in_library(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn id_dir_boundaries() {
        assert_eq!(item_id_of("1234_x").as_deref(), None);
        assert_eq!(item_id_of("12345_x").as_deref(), Some("12345"));
        assert_eq!(item_id_of("12345678_x").as_deref(), Some("12345678"));
        assert_eq!(item_id_of("123456789_x").as_deref(), None);
        assert_eq!(item_id_of("1234567x").as_deref(), None);
    }
}
