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

use std::collections::HashMap;
use std::io::Read;
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

/// 归档内条目名的字节 → 字符串。
///
/// zip / tar 条目名的候选码页。
///
/// zip 不存码页：规范名义上是 CP437，实践中是**打包机的 ANSI 码页**
/// （日文 932 / 中文 936 / 西欧 1252），读取端无从直接判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameEncoding {
    Utf8,
    /// 中文 Windows（GBK / CP936）。
    Gbk,
    /// 日文 Windows（Shift-JIS / CP932）。
    ShiftJis,
    /// 西欧 Windows-1252；zip 规范名义默认的 CP437 与之在 ASCII 区一致。
    Latin1,
}

/// 参与打分的候选（UTF-8 由标志位或字节合法性单独判定，不参与竞争）。
///
/// **顺序即平局时的优先级**。GBK 覆盖 0xC0–0xF7，Shift-JIS 字节落进去同样「解得出」
/// （变成汉字）且无解码错误，于是**全汉字的日文名**（如 `利用規約（日本語版）`，
/// 一个假名都没有）会与 GBK 打平——假名奖励在这类名字上失效。此时取 Shift-JIS：
/// - BOOTH 商品以日文占绝大多数；
/// - 中文包不会因此被带偏：常用汉字在 GBK 一级字库、首字节 0xB0–0xD7，落到
///   Shift-JIS 会进半角片假名区 `U+FF61–FF9F` 而被重罚，故中文包仍判 GBK
///   （`same_archive_entries_use_one_decoder` 即为该反向验证）。
const CODEPAGE_CANDIDATES: [NameEncoding; 3] = [
    NameEncoding::ShiftJis,
    NameEncoding::Gbk,
    NameEncoding::Latin1,
];

/// 解码报错的重罚：该码页打不出这些字节，几乎不可能是它。
const DECODE_ERROR_PENALTY: i64 = 1000;

impl NameEncoding {
    /// 按本码页解码；返回 `(文本, 是否出现解码错误)`。
    fn decode<'a>(&self, raw: &'a [u8]) -> (std::borrow::Cow<'a, str>, bool) {
        match self {
            NameEncoding::Utf8 => match std::str::from_utf8(raw) {
                Ok(s) => (std::borrow::Cow::Borrowed(s), false),
                Err(_) => (std::borrow::Cow::Borrowed(""), true),
            },
            NameEncoding::Gbk => {
                let (c, _, e) = encoding_rs::GBK.decode(raw);
                (c, e)
            }
            NameEncoding::ShiftJis => {
                let (c, _, e) = encoding_rs::SHIFT_JIS.decode(raw);
                (c, e)
            }
            NameEncoding::Latin1 => {
                let (c, _, e) = encoding_rs::WINDOWS_1252.decode(raw);
                (c, e)
            }
        }
    }
}

/// 解码结果的「可疑度」——**越高越不可能是对的码页**。
///
/// 判据都取自「正常文件名里不该大量出现」的字符：
/// - `U+FFFD` 替换字符：解码器打不出该字节序列的直证
/// - **半角片假名 `U+FF61–FF9F`**：Shift-JIS 被误用到非日文字节上的招牌产物，
///   `ﾄ｣ﾐﾍ` 这种串看着像日文，其实只是字节错位
/// - 控制字符：任何正确解码都几乎不会出现
/// - Latin-1 补充区 `U+00A0–FF`：`Ä£ÐÍ` 这类产物。西欧语言文件名确实会用到该区，
///   故权重低于前三者，只在成片出现时才主导
///
/// 反向**奖励**只有一项：全角假名 `U+3040–30FF`。日文名几乎必然含假名，而 GBK
/// 误读日文时会把假名字节解成汉字——这项奖励让日文包不会被判成中文码页。
fn suspicion(s: &str) -> i64 {
    let mut score = 0i64;
    for c in s.chars() {
        let u = c as u32;
        if u == 0xFFFD {
            score += 10;
        } else if (0xFF61..=0xFF9F).contains(&u) {
            score += 4;
        } else if u < 0x20 || u == 0x7F {
            score += 5;
        } else if (0xA0..=0xFF).contains(&u) {
            score += 2;
        } else if (0x3040..=0x30FF).contains(&u) {
            score -= 1;
        }
    }
    score
}

/// **按包**选定条目名解码器：对整批条目名的候选解码结果统一打分，取最优。
///
/// 为什么必须按包定一次：一个压缩包只有一个打包者、一种编码。逐条目「挑第一个
/// 不报错的解码器」会让同一个包内混用两套解码器——实测一个纯 GBK 包里同时出现了
/// Latin-1 产物（`Ä£ÐÍ/ÌùÍ¼/ÉíÌå.png`）与半角片假名产物（`ﾄ｣ﾐﾍ/ﾋｵﾃﾄｵｵ.txt`）。
/// 那已不是「少支持一种编码」，而是策略本身不成立。
fn pick_decoder(raws: &[&[u8]]) -> NameEncoding {
    if raws.is_empty() {
        return NameEncoding::Utf8;
    }
    let mut best = NameEncoding::Latin1;
    let mut best_score = i64::MAX;
    for cand in CODEPAGE_CANDIDATES {
        let mut score = 0i64;
        for raw in raws {
            let (cow, err) = cand.decode(raw);
            if err {
                score += DECODE_ERROR_PENALTY;
            }
            score += suspicion(&cow);
        }
        if score < best_score {
            best_score = score;
            best = cand;
        }
    }
    best
}

/// 单条目名解码（打分择优）。
///
/// 供无法按包统一收集的场景。**同批数据请优先用 [`pick_decoder`] 统一**——
/// 单条打分在个别条目上会因特征不足而摇摆。
///
/// 当前 lib 侧（zip / tar）都已改为按包统一，故此函数仅作测试入口与单条解码参考。
#[cfg(test)]
fn decode_name(raw: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.to_string();
    }
    let (cow, _) = pick_decoder(&[raw]).decode(raw);
    cow.into_owned()
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
        Some("unitypackage") => unitypackage_walk(file).is_some(),
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
    // 标志位只能经公开 trait 取：`mod types` 是私有的、`ZipFileData` 字段是 `pub(crate)`。
    use zip::read::HasZipMetadata;

    let fh = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(fh).ok()?;
    let n = zip.len();

    // 第一遍：只取原始字节与 UTF-8 标志位，**不解码**——解码要等码页定下来。
    let mut raws: Vec<(Vec<u8>, bool, u64, u64, String)> = Vec::with_capacity(n);
    for i in 0..n {
        let f = zip.by_index_raw(i).ok()?;
        raws.push((
            f.name_raw().to_vec(),
            f.get_metadata().is_utf8,
            f.size(),
            f.compressed_size(),
            format!("{:?}", f.compression()).to_ascii_lowercase(),
        ));
    }

    // 第二遍：标志位为假的条目**整包共用一个码页**（见 `pick_decoder` 的理由）。
    let ambiguous: Vec<&[u8]> = raws
        .iter()
        .filter(|(_, utf8, ..)| !*utf8)
        .map(|(nm, ..)| nm.as_slice())
        .collect();
    let enc = pick_decoder(&ambiguous);

    let out = raws
        .into_iter()
        .map(|(nm, utf8, size, csize, method)| EntryInfo {
            name: if utf8 {
                String::from_utf8_lossy(&nm).into_owned()
            } else {
                enc.decode(&nm).0.into_owned()
            },
            size,
            compressed_size: Some(csize),
            method,
        })
        .collect();
    Some(out)
}

/// 预览压缩包条目，`limit` 为 0 表示不限。
///
/// 不可解析格式或解析失败返回 `Err`（附原因），供三端直接呈现。
pub fn preview_archive(path: &Path, limit: usize) -> Result<ArchivePreview, String> {
    let (format, listed) = match ext_of(path).as_deref() {
        Some("zip") => ("zip", zip_entries(path)),
        Some("unitypackage") => ("unitypackage", unitypackage_walk(path)),
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

/// unitypackage 单次遍历：解析真实资源路径，同时完成完整性校验。
///
/// unitypackage 的 tar 结构是 `{guid}/asset` + `{guid}/pathname` + …，
/// 直接列 GUID 目录对用户毫无意义；预览取的是 `pathname` 的内容（`Assets/…`），
/// 大小取同 GUID 下 `asset` 的字节数——与 zip 的「中央目录一次解析两用」对称，
/// 完整性判定与预览共用这一次遍历。
///
/// gzip+tar 无尾置索引，截断只能顺序解压到流末尾才发现；且 tar 迭代遇结尾零块
/// 即停止、不会读完 gzip 余下字节，故遍历后还需把内层流读干以触发 CRC32/ISIZE。
fn unitypackage_walk(path: &Path) -> Option<Vec<EntryInfo>> {
    let fh = std::fs::File::open(path).ok()?;
    let dec = GzDecoder::new(fh);
    let mut archive = Archive::new(dec);
    let entries = archive.entries().ok()?;

    let mut asset_sizes: HashMap<String, u64> = HashMap::new();
    // 存原始字节，延后到码页定下来再解码——tar 同样是一个包一种编码。
    let mut named: Vec<(String, Vec<u8>)> = Vec::new();
    let mut sink = std::io::sink();

    for entry in entries {
        let mut e = entry.ok()?;
        let raw = e.path_bytes().into_owned();
        let (guid, base) = split_tar_path(&raw)?;
        if base.eq_ignore_ascii_case("asset") {
            asset_sizes.insert(guid, e.size());
            std::io::copy(&mut e, &mut sink).ok()?;
        } else if base.eq_ignore_ascii_case("pathname") {
            let mut buf = Vec::new();
            e.read_to_end(&mut buf).ok()?;
            named.push((guid, trim_bytes(&buf).to_vec()));
        } else {
            std::io::copy(&mut e, &mut sink).ok()?;
        }
    }
    std::io::copy(&mut archive.into_inner(), &mut sink).ok()?;

    // 整包统一解码（与 zip 侧同构：一个包只有一个打包者、一种编码）。
    let enc = {
        let raws: Vec<&[u8]> = named.iter().map(|(_, b)| b.as_slice()).collect();
        pick_decoder(&raws)
    };

    let mut out: Vec<EntryInfo> = named
        .into_iter()
        .map(|(guid, raw)| EntryInfo {
            name: enc.decode(&raw).0.into_owned(),
            size: asset_sizes.get(&guid).copied().unwrap_or(0),
            compressed_size: None,
            method: "unitypackage".to_string(),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Some(out)
}

/// `{guid}/pathname` → `(guid, pathname)`。
fn split_tar_path(raw: &[u8]) -> Option<(String, String)> {
    let pos = raw.iter().rposition(|b| *b == b'/')?;
    Some((
        String::from_utf8_lossy(&raw[..pos]).into_owned(),
        String::from_utf8_lossy(&raw[pos + 1..]).into_owned(),
    ))
}

/// 去首尾 ASCII 空白（`pathname` 内容常带换行）。
fn trim_bytes(b: &[u8]) -> &[u8] {
    let start = b
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(b.len());
    let end = b
        .iter()
        .rposition(|c| !c.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(start);
    &b[start..end]
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
    ///
    /// 形态与真实包一致：每个资源一个 `{guid}/asset` + `{guid}/pathname`，
    /// 这样遍历才能解析出真实资源路径与资源大小。
    fn unitypackage_multi(entries: usize, payload: usize) -> Vec<u8> {
        let mut tar_buf = Vec::new();
        {
            let mut b = tar::Builder::new(&mut tar_buf);
            let data = vec![b'x'; payload];
            for i in 0..entries {
                let guid = format!("{i:032}");
                let mut h = tar::Header::new_gnu();
                h.set_size(data.len() as u64);
                h.set_cksum();
                b.append_data(&mut h, format!("{guid}/asset"), &data[..])
                    .unwrap();
                let pn = format!("Assets/Item{i}.prefab");
                let mut h2 = tar::Header::new_gnu();
                h2.set_size(pn.len() as u64);
                h2.set_cksum();
                b.append_data(&mut h2, format!("{guid}/pathname"), pn.as_bytes())
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

    /// 条目名取 `pathname` 内容（真实资源路径）而非 GUID 目录名，
    /// 大小取同 GUID 下 `asset` 的字节数——列 GUID 对用户毫无意义。
    #[test]
    fn unitypackage_walk_resolves_real_paths() {
        let p = write_tmp("list.unitypackage", &unitypackage_multi(5, 1024));
        let entries = unitypackage_walk(&p).expect("should list");
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].name, "Assets/Item0.prefab");
        assert_eq!(entries[4].name, "Assets/Item4.prefab");
        assert!(entries.iter().all(|e| e.size == 1024));
        assert!(entries.iter().all(|e| e.compressed_size.is_none()));
        assert!(entries.iter().all(|e| e.method == "unitypackage"));
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

    /// BOOTH 日文商品的 zip 条目名多为 Shift-JIS 且未设 UTF-8 标志位，
    /// 按 UTF-8 lossy 解会得到乱码——预览对此直接失去意义。
    #[test]
    fn decode_name_handles_shift_jis() {
        assert_eq!(decode_name(b"readme.txt"), "readme.txt");
        assert_eq!(
            decode_name("日本語ファイル.txt".as_bytes()),
            "日本語ファイル.txt"
        );

        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("三点だいしゅきツール");
        assert_eq!(decode_name(&sjis), "三点だいしゅきツール");

        let (sjis2, _, _) = encoding_rs::SHIFT_JIS.encode("利用規約（日本語版）.pdf");
        assert_eq!(decode_name(&sjis2), "利用規約（日本語版）.pdf");
    }

    /// GBK 不得只靠「Shift-JIS 解不出错」就放过。
    ///
    /// 中文 Windows 打包的 zip 用 GBK，而 GBK 字节落在 SJIS 的半角片假名区
    /// （0xA1–0xDF）上是「合法」的——旧实现于是停在 SJIS 分支，输出 `ﾄ｣ﾐﾍ` 这种
    /// 看着像日文、实则字节错位的串。
    #[test]
    fn decode_name_should_cover_gbk() {
        let (gbk, _, _) = encoding_rs::GBK.encode("模型/贴图/身体.png");
        assert_eq!(decode_name(&gbk), "模型/贴图/身体.png");
    }

    /// 同一压缩包内所有条目必须由**同一解码器**还原。
    ///
    /// 一个包只有一个打包者、一种编码。逐条目「挑第一个不报错的解码器」会让
    /// 同一个包内混用两套解码器——这正是 B2 的命门，一致性本身就是可断言的判据。
    #[test]
    fn same_archive_entries_use_one_decoder() {
        let names = [
            "模型/贴图/身体.png",
            "模型/说明文档.txt",
            "衣服/连衣裙.fbx",
            "道具/剑.fbx",
        ];
        let raws: Vec<Vec<u8>> = names
            .iter()
            .map(|n| encoding_rs::GBK.encode(n).0.into_owned())
            .collect();
        let refs: Vec<&[u8]> = raws.iter().map(|v| v.as_slice()).collect();
        let enc = pick_decoder(&refs);
        assert_eq!(enc, NameEncoding::Gbk, "整包未选中 GBK");
        for (raw, want) in raws.iter().zip(names.iter()) {
            assert_eq!(
                enc.decode(raw).0.as_ref(),
                *want,
                "同包条目被不同解码器处理"
            );
        }
    }

    /// 日文包不得被判成中文码页。
    ///
    /// GBK 覆盖 0xC0–0xF7 区，Shift-JIS 字节落进去同样「解得出」（变成汉字），
    /// 且无解码错误——只靠报错与否分不开。靠的是假名奖励：正确解出的日文含假名，
    /// 而 GBK 误读会把假名字节解成汉字。
    #[test]
    fn shift_jis_not_mistaken_for_gbk() {
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("三点だいしゅきツール");
        assert_eq!(pick_decoder(&[&sjis]), NameEncoding::ShiftJis);
        assert_eq!(decode_name(&sjis), "三点だいしゅきツール");
    }

    /// 纯 ASCII 条目名三种候选得分相同——选哪个都行，关键是结果不变。
    #[test]
    fn ascii_names_stay_ascii() {
        let raws: Vec<&[u8]> = vec![b"a.txt", b"Assets/Material.mat"];
        let enc = pick_decoder(&raws);
        assert_eq!(enc.decode(b"a.txt").0.as_ref(), "a.txt");
        assert_eq!(
            enc.decode(b"Assets/Material.mat").0.as_ref(),
            "Assets/Material.mat"
        );
    }

    #[test]
    fn decode_name_never_panics_on_junk() {
        let _ = decode_name(&[0xff, 0xfe, 0x00, 0x81]);
        let _ = decode_name(&[]);
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
