//! 从 `.unitypackage` / 外层 zip 提取内部资源路径名，供搜索验真。
//!
//! 失败一律返回空名单，不抛致命（调用方按「无资源名」走原评分）。

use std::io::{Cursor, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use tar::Archive;

/// zip → gzip → tar → `pathname` 文件内容。
///
/// 输入可以是外层 zip（内含 `.unitypackage`）、裸 `.unitypackage`（gzip+tar），
/// 或已解压的 tar。任一层失败则跳过该层，全部失败返回空。
pub fn extract_unitypackage_names(path: &Path) -> Vec<String> {
    let bytes = match std::fs::read(path) {
        Ok(b) if !b.is_empty() => b,
        _ => return Vec::new(),
    };
    extract_from_bytes(&bytes)
}

/// 供 `score_and_pick`：空名单当 `None`，不改变「传 None」时的选优。
pub fn names_for_score(path: &Path) -> Option<Vec<String>> {
    let names = extract_unitypackage_names(path);
    if names.is_empty() { None } else { Some(names) }
}

fn extract_from_bytes(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if looks_zip(bytes) {
        out.extend(from_zip(bytes));
        if !out.is_empty() {
            return dedup(out);
        }
    }
    out.extend(from_gzip_tar(bytes));
    if out.is_empty() {
        out.extend(from_tar(bytes));
    }
    dedup(out)
}

fn looks_zip(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == b'P' && bytes[1] == b'K'
}

fn from_zip(bytes: &[u8]) -> Vec<String> {
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let Ok(mut f) = zip.by_index(i) else {
            continue;
        };
        if !f.is_file() {
            continue;
        }
        let name = f.name().to_string();
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_err() {
            continue;
        }
        if name
            .rsplit('/')
            .next()
            .unwrap_or("")
            .eq_ignore_ascii_case("pathname")
        {
            push_pathname(&mut out, &buf);
            continue;
        }
        if name.to_ascii_lowercase().ends_with(".unitypackage") {
            out.extend(from_gzip_tar(&buf));
            if out.is_empty() {
                out.extend(from_tar(&buf));
            }
        }
    }
    out
}

fn from_gzip_tar(bytes: &[u8]) -> Vec<String> {
    let dec = GzDecoder::new(Cursor::new(bytes));
    from_tar_reader(dec)
}

fn from_tar(bytes: &[u8]) -> Vec<String> {
    from_tar_reader(Cursor::new(bytes))
}

fn from_tar_reader<R: Read>(reader: R) -> Vec<String> {
    let mut archive = Archive::new(reader);
    let Ok(entries) = archive.entries() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Ok(path) = entry.path() else {
            continue;
        };
        let is_pathname = path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("pathname"));
        if !is_pathname {
            continue;
        }
        let mut buf = Vec::new();
        let mut e = entry;
        if e.read_to_end(&mut buf).is_err() {
            continue;
        }
        push_pathname(&mut out, &buf);
    }
    out
}

fn push_pathname(out: &mut Vec<String>, buf: &[u8]) {
    let s = String::from_utf8_lossy(buf);
    let t = s.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
}

fn dedup(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    fn tmpfile(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "bvt_upk_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn gzip_tar(pathnames: &[(&str, &str)]) -> Vec<u8> {
        let mut tar_buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);
            for (guid, pathname) in pathnames {
                let mut header = tar::Header::new_gnu();
                let data = pathname.as_bytes();
                header.set_size(data.len() as u64);
                header.set_cksum();
                builder
                    .append_data(&mut header, format!("{guid}/pathname"), data)
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&tar_buf).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn missing_file_empty() {
        assert!(extract_unitypackage_names(Path::new("/no/such/file.unitypackage")).is_empty());
    }

    #[test]
    fn garbage_empty() {
        let p = tmpfile("junk");
        std::fs::write(&p, b"not a package").unwrap();
        assert!(extract_unitypackage_names(&p).is_empty());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn gzip_tar_pathnames() {
        let bytes = gzip_tar(&[
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "Assets/Moonpiercer.prefab",
            ),
            (
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "Assets/Owl/Agent.controller",
            ),
        ]);
        let p = tmpfile("pkg");
        std::fs::write(&p, bytes).unwrap();
        let names = extract_unitypackage_names(&p);
        assert!(names.iter().any(|n| n.contains("Moonpiercer")));
        assert!(names.iter().any(|n| n.contains("Agent.controller")));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn zip_wrapping_unitypackage() {
        let inner = gzip_tar(&[(
            "cccccccccccccccccccccccccccccccc",
            "Assets/LunariaPaperFan.fbx",
        )]);
        let mut zip_buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut zip_buf);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            w.start_file("pack.unitypackage", opts).unwrap();
            w.write_all(&inner).unwrap();
            w.finish().unwrap();
        }
        let p = tmpfile("zip");
        std::fs::write(&p, zip_buf.into_inner()).unwrap();
        let names = extract_unitypackage_names(&p);
        assert_eq!(names, vec!["Assets/LunariaPaperFan.fbx".to_string()]);
        assert!(names_for_score(&p).is_some());
        let _ = std::fs::remove_file(&p);
    }
}
