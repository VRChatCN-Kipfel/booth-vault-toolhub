//! 本地压缩包完整性校验：识别中断下载残留的半截文件。
//!
//! 「存在且非空且非 HTML 伪装即有效」这一幂等契约不足以排除半截文件：
//! 断下载留下的 zip/unitypackage size>0 且魔数正常，会被判为已完成而永久跳过，
//! 换节点或换代理重跑仍不重下。故按结构补一层真实校验。
//!
//! 两类格式的判据完全不同。zip 的中央目录（EOCD）位于物理文件末尾，能定位即证
//! 前缀完整，且列表读取不解码任何条目；unitypackage 是 gzip+tar 流，无尾置索引，
//! 截断只能靠顺序解压到流末尾才能发现。
//!
//! 格式按扩展名分派（与库内 `ID_标题` 命名契约一致）：rar/7z 等无内置解析器，
//! 判为无法判定而非损坏——宁可漏报，不可误报。若将来接入 7z 解析，
//! 未知压缩法导致的失败同样应落在无法判定一侧。

use std::path::Path;

use flate2::read::GzDecoder;
use tar::Archive;

/// 可解析的扩展名。取小写且不含点：`Path::extension()` 返回的是 `zip` 而非 `.zip`。
const CHECKABLE_EXTS: [&str; 2] = ["zip", "unitypackage"];

/// 本地包体检结论。
///
/// 三态而非二值：`Corrupt` 可安全覆盖用户既有文件，`Indeterminate` 不可以。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageHealth {
    /// 结构可解析，完好可用。
    Valid,
    /// 确证损坏：可解析格式校验失败，或文件为空/不可读。
    Corrupt,
    /// 无法判定：格式无内置解析器。不得据此覆盖文件。
    Indeterminate,
}

/// 扩展名是否可解析。
pub fn is_checkable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| CHECKABLE_EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
        .unwrap_or(false)
}

/// 包体检。
pub fn package_health(path: &Path) -> PackageHealth {
    let Ok(meta) = std::fs::metadata(path) else {
        return PackageHealth::Corrupt;
    };
    if meta.len() == 0 {
        return PackageHealth::Corrupt;
    }
    if !is_checkable(path) {
        return PackageHealth::Indeterminate;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let intact = if ext == "zip" {
        zip_intact(path)
    } else {
        unitypackage_intact(path)
    };
    if intact {
        PackageHealth::Valid
    } else {
        PackageHealth::Corrupt
    }
}

/// 是否确证损坏（无法判定不算损坏）。
pub fn is_corrupt_package(path: &Path) -> bool {
    matches!(package_health(path), PackageHealth::Corrupt)
}

/// zip 整体性：EOCD 位于物理文件末尾，`ZipArchive::new()` 能定位到它即证前缀完整。
///
/// 只读尾部中央目录、不解码条目，故加密与未知压缩法都不影响本判定。
/// 实测截断 1 字节即 `Could not find EOCD`，不存在「差一点还能过」的灰度带。
///
/// 已知取舍：中段位翻转需全量读条目 CRC 才能抓到，但那是介质损坏而非下载中断
/// （流式写入是单调拼接），不作为本判据目标。
fn zip_intact(path: &Path) -> bool {
    let Ok(fh) = std::fs::File::open(path) else {
        return false;
    };
    zip::ZipArchive::new(fh).is_ok()
}

/// unitypackage 整体性：gzip+tar 无尾置索引，必须顺序解压到流末尾。
///
/// tar 迭代遇到结尾零块即停止，不会读完 gzip 余下字节，gzip 的 CRC32/ISIZE
/// 校验也就不会触发——因此遍历完成后还需把内层流读干。
fn unitypackage_intact(path: &Path) -> bool {
    let Ok(fh) = std::fs::File::open(path) else {
        return false;
    };
    let dec = GzDecoder::new(fh);
    let mut archive = Archive::new(dec);
    let Ok(entries) = archive.entries() else {
        return false;
    };
    let mut sink = std::io::sink();
    for entry in entries {
        let Ok(mut e) = entry else {
            return false;
        };
        if std::io::copy(&mut e, &mut sink).is_err() {
            return false;
        }
    }
    std::io::copy(&mut archive.into_inner(), &mut sink).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    /// tag 置于末段：扩展名需落在路径末尾才能被 `is_checkable` 识别。
    fn tmp(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "bvt_integrity_{}_{}_{tag}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_tmp(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = tmp(tag);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    /// 夹具必须比实现更狡猾：EOCD 紧贴文件末尾时截断才会真正落在被截区间。
    /// 无尾注的 zip 截断后中央目录可能仍在，测不出任何东西。
    fn zip_with_comment(comment: &str) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            w.set_comment(comment);
            w.start_file("a.txt", o).unwrap();
            w.write_all(b"hello").unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    /// 60 字节的微型夹具取一半连 gzip 头都不完整，会在「读首条目」之前就抛错，
    /// 恰好绕过被测行为——必须造足够大的多条目包，截断才落在首条目之后。
    fn unitypackage_multi(entries: usize, payload: usize) -> Vec<u8> {
        let mut tar_buf = Vec::new();
        {
            let mut b = tar::Builder::new(&mut tar_buf);
            let data = vec![b'x'; payload];
            for i in 0..entries {
                let mut h = tar::Header::new_gnu();
                h.set_size(data.len() as u64);
                h.set_cksum();
                b.append_data(&mut h, format!("guid{i}/pathname"), &data[..])
                    .unwrap();
            }
            b.finish().unwrap();
        }
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(&tar_buf).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn empty_and_missing_are_corrupt() {
        let p = write_tmp("empty.zip", b"");
        assert_eq!(package_health(&p), PackageHealth::Corrupt);
        let _ = std::fs::remove_file(&p);
        assert_eq!(package_health(&p), PackageHealth::Corrupt);
    }

    #[test]
    fn zip_prefix_truncation_is_corrupt() {
        let full = zip_with_comment("tail anchor: EOCD must fall past the cut");
        for keep in [1, 2, 10] {
            let cut = full.len() - keep;
            let p = write_tmp("trunc.zip", &full[..cut]);
            assert_eq!(package_health(&p), PackageHealth::Corrupt, "cut {cut}");
            let _ = std::fs::remove_file(&p);
        }
    }

    #[test]
    fn good_zip_is_valid() {
        let p = write_tmp("ok.zip", &zip_with_comment("ok"));
        assert_eq!(package_health(&p), PackageHealth::Valid);
        let _ = std::fs::remove_file(&p);
    }

    /// 尾部截断：旧实现只读首条目，此处必然漏报。
    #[test]
    fn unitypackage_tail_truncation_is_corrupt() {
        let full = unitypackage_multi(8, 8192);
        let p = write_tmp("tail.unitypackage", &full[..full.len() / 2]);
        assert_eq!(package_health(&p), PackageHealth::Corrupt);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn good_unitypackage_is_valid() {
        let p = write_tmp("ok.unitypackage", &unitypackage_multi(4, 4096));
        assert_eq!(package_health(&p), PackageHealth::Valid);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn unparsable_exts_are_indeterminate() {
        for tag in ["junk.rar", "junk.7z", "junk.bin"] {
            let p = write_tmp(tag, b"\x00\x01garbage");
            assert_eq!(package_health(&p), PackageHealth::Indeterminate, "{tag}");
            assert!(!is_corrupt_package(&p), "{tag}");
            let _ = std::fs::remove_file(&p);
        }
    }

    /// 空文件即便格式不可解析也确证损坏：0 字节不可能是有效包。
    #[test]
    fn empty_unparsable_is_corrupt() {
        let p = write_tmp("empty.rar", b"");
        assert_eq!(package_health(&p), PackageHealth::Corrupt);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn extension_dispatch_is_case_insensitive() {
        assert!(is_checkable(Path::new("a.ZIP")));
        assert!(is_checkable(Path::new("a.UnityPackage")));
        assert!(!is_checkable(Path::new("a.rar")));
        assert!(!is_checkable(Path::new("a")));
    }
}
