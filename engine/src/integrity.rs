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

use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use tar::Archive;

/// 可解析的扩展名。取小写且不含点：`Path::extension()` 返回的是 `zip` 而非 `.zip`。
const CHECKABLE_EXTS: [&str; 2] = ["zip", "unitypackage"];

/// 预览默认条目上限：GUI 只需看清单，数千条目全量回传无意义。
pub const PREVIEW_LIMIT: usize = 500;

/// 压缩包内单个条目摘要（预览用，不含内容）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    /// 归档内路径。unitypackage 为 `Assets/...` 形式。
    pub name: String,
    /// 未压缩字节数。
    pub size: u64,
    /// 压缩后字节数；tar 无此概念时为 `None`。
    pub compressed_size: Option<u64>,
    /// 压缩方法名；tar 恒为 `tar`。
    pub method: String,
}

/// 压缩包预览结果。
#[derive(Debug, Clone)]
pub struct ArchivePreview {
    pub path: PathBuf,
    /// `zip` / `unitypackage`。
    pub format: String,
    /// 条目总数；`truncated` 为真时大于 `entries.len()`。
    pub total_entries: usize,
    /// 仅返回前 `limit` 条时为真。
    pub truncated: bool,
    pub entries: Vec<EntryInfo>,
}

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

/// 小写扩展名（不含点）。
fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

/// 扩展名是否可解析。
pub fn is_checkable(path: &Path) -> bool {
    ext_of(path)
        .map(|e| CHECKABLE_EXTS.contains(&e.as_str()))
        .unwrap_or(false)
}

/// 包体检。
pub fn package_health(path: &Path) -> PackageHealth {
    package_health_of(path, path)
}

/// 按 `ext_source` 的扩展名分派，校验 `file` 的结构。
///
/// 供 `{dest}.part` 场景使用：扩展名落在目标名上，内容却还在临时文件里，
/// 直接对 `.part` 调用 `package_health` 会因扩展名不认识而恒判无法判定。
pub fn package_health_of(file: &Path, ext_source: &Path) -> PackageHealth {
    let Ok(meta) = std::fs::metadata(file) else {
        return PackageHealth::Corrupt;
    };
    if meta.len() == 0 {
        return PackageHealth::Corrupt;
    }
    let intact = match ext_of(ext_source).as_deref() {
        Some("zip") => zip_entries(file).is_some(),
        Some("unitypackage") => unitypackage_intact(file),
        _ => return PackageHealth::Indeterminate,
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

/// zip 中央目录的条目列表；解析失败返回 `None`。
///
/// EOCD 位于物理文件末尾，`ZipArchive::new()` 能定位到它即证前缀完整，且该调用
/// **已经把完整中央目录读进内存**——enumerate 条目是零额外 I/O 的纯内存操作。
/// 故完整性判定与预览共用这一份解析结果（`package_health` 的 zip 分支即
/// `zip_entries(..).is_some()`），不存在「为了预览再解析一遍」。
///
/// 用 `by_index_raw` 只读 header、不建立解压器：未知压缩法与加密条目都不会让
/// 本函数失败（预览只列名与大小，不解内容）。
///
/// 已知取舍：中段位翻转需全量读条目 CRC 才能抓到，但那是介质损坏而非下载中断
/// （流式写入是单调拼接），不作为本判据目标。
pub fn zip_entries(path: &Path) -> Option<Vec<EntryInfo>> {
    let fh = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(fh).ok()?;
    let mut out = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).ok()?;
        out.push(EntryInfo {
            name: f.name().to_string(),
            size: f.size(),
            compressed_size: Some(f.compressed_size()),
            method: format!("{:?}", f.compression()).to_ascii_lowercase(),
        });
    }
    Some(out)
}

/// unitypackage（gzip + tar）的条目列表；解析失败返回 `None`。
///
/// 只遍历 tar header，**不读条目内容、不读干 gzip 流**——因此明显快于
/// `package_health`（后者为抓尾部截断必须全量解压）。预览场景够用，
/// 但**不能拿它的成功当完整性结论**。
pub fn unitypackage_entries(path: &Path) -> Option<Vec<EntryInfo>> {
    let fh = std::fs::File::open(path).ok()?;
    let dec = GzDecoder::new(fh);
    let mut archive = Archive::new(dec);
    let entries = archive.entries().ok()?;
    let mut out = Vec::new();
    for e in entries {
        let e = e.ok()?;
        out.push(EntryInfo {
            name: e.path().ok()?.to_string_lossy().to_string(),
            size: e.size(),
            compressed_size: None,
            method: "tar".to_string(),
        });
    }
    Some(out)
}

/// 预览压缩包条目，`limit` 为 0 表示不限。
///
/// 不可解析格式或解析失败返回 `Err`（附原因），供三端直接呈现。
pub fn preview_archive(path: &Path, limit: usize) -> Result<ArchivePreview, String> {
    let (format, listed) = match ext_of(path).as_deref() {
        Some("zip") => ("zip", zip_entries(path)),
        Some("unitypackage") => ("unitypackage", unitypackage_entries(path)),
        _ => {
            return Err(format!(
                "不支持的格式（仅 zip / unitypackage）：{}",
                path.display()
            ));
        }
    };
    let Some(mut entries) = listed else {
        return Err(format!("无法解析，文件可能已损坏：{}", path.display()));
    };
    let total_entries = entries.len();
    let truncated = limit > 0 && total_entries > limit;
    if truncated {
        entries.truncate(limit);
    }
    Ok(ArchivePreview {
        path: path.to_path_buf(),
        format: format.to_string(),
        total_entries,
        truncated,
        entries,
    })
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

    /// 多条目 zip：条目 i 的未压缩长度为 16+i，便于断言 size 取的是条目自身而非文件。
    fn zip_multi(names: &[&str]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (i, n) in names.iter().enumerate() {
                w.start_file(*n, o).unwrap();
                w.write_all(&vec![b'a' + i as u8; 16 + i]).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn zip_entries_lists_names_and_sizes() {
        let p = write_tmp("list.zip", &zip_multi(&["a.txt", "b/c.prefab", "d.png"]));
        let entries = zip_entries(&p).expect("should list");
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a.txt", "b/c.prefab", "d.png"]);
        assert_eq!(entries[0].size, 16);
        assert_eq!(entries[1].size, 17);
        assert_eq!(entries[2].size, 18);
        assert!(entries[0].compressed_size.is_some());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn zip_entries_on_truncated_is_none() {
        let full = zip_with_comment("anchor");
        let p = write_tmp("list-trunc.zip", &full[..full.len() - 1]);
        assert!(zip_entries(&p).is_none());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn unitypackage_entries_lists_all() {
        let p = write_tmp("list.unitypackage", &unitypackage_multi(5, 1024));
        let entries = unitypackage_entries(&p).expect("should list");
        assert_eq!(entries.len(), 5);
        assert!(entries.iter().all(|e| e.name.ends_with("/pathname")));
        assert!(entries.iter().all(|e| e.size == 1024));
        assert!(entries.iter().all(|e| e.compressed_size.is_none()));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn preview_reports_format_and_total() {
        let p = write_tmp("pv.zip", &zip_multi(&["x", "y"]));
        let pv = preview_archive(&p, PREVIEW_LIMIT).unwrap();
        assert_eq!(pv.format, "zip");
        assert_eq!(pv.total_entries, 2);
        assert!(!pv.truncated);
        assert_eq!(pv.path, p);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn preview_truncates_at_limit() {
        let names = ["a", "b", "c", "d", "e"];
        let p = write_tmp("pv-lim.zip", &zip_multi(&names));
        let pv = preview_archive(&p, 2).unwrap();
        assert_eq!(pv.total_entries, 5);
        assert_eq!(pv.entries.len(), 2);
        assert!(pv.truncated);
        let pv_all = preview_archive(&p, 0).unwrap();
        assert_eq!(pv_all.entries.len(), 5);
        assert!(!pv_all.truncated);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn preview_rejects_unsupported_and_corrupt() {
        let p = write_tmp("pv.rar", b"nope");
        assert!(preview_archive(&p, 0).unwrap_err().contains("不支持的格式"));
        let _ = std::fs::remove_file(&p);

        let full = zip_with_comment("anchor");
        let p2 = write_tmp("pv-bad.zip", &full[..full.len() - 1]);
        assert!(preview_archive(&p2, 0).unwrap_err().contains("无法解析"));
        let _ = std::fs::remove_file(&p2);
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
