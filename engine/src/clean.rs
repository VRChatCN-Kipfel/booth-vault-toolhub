//! 文件名/查询清洗：sanitize_filename / sanitize / extract_version_tag / sanitize_query。
//!
//! 行为逐条复刻 Python 实现（血泪坑 §8.1.1、§8.2.10-12）。

use fancy_regex::Regex;
use unicode_general_category::GeneralCategory;

/// Windows 文件名非法字符。
const INVALID: &str = r#"<>:"/\|?*"#;

/// 装饰 Unicode → 过滤（血泪坑：装饰 Unicode 目录名会被 Explorer 永久拒绝应用 desktop.ini）。
///
/// 过滤 emoji/装饰区段 + Mn/Me 组合字符 + Cn 未分配。
fn is_decorative(c: char) -> bool {
    let code = c as u32;
    if (0x1F300..=0x1F9FF).contains(&code)
        || (0x2000..=0x27BF).contains(&code)
        || (0x2B0..=0x2FF).contains(&code)
        || (0x2070..=0x209F).contains(&code)
    {
        return true;
    }
    matches!(
        unicode_general_category::get_general_category(c),
        GeneralCategory::EnclosingMark
            | GeneralCategory::NonspacingMark
            | GeneralCategory::Unassigned
    )
}

/// 移除 Windows 非法字符 + 装饰 Unicode。保留 ASCII/中日韩/全角。截断 80。
pub fn sanitize_filename(name: &str) -> String {
    let name: String = name.chars().filter(|c| !INVALID.contains(*c)).collect();
    let cleaned: String = name.chars().filter(|c| !is_decorative(*c)).collect();
    let name = collapse_whitespace(&cleaned)
        .trim_matches(['.', ' '])
        .to_string();
    let name = name.chars().take(80).collect::<String>();
    if name.is_empty() {
        "unnamed".to_string()
    } else {
        name
    }
}

/// 简化清洗（仅过滤非法字符 + 折叠空白 + 截断）。
pub fn sanitize(name: &str, max_len: usize) -> String {
    let out: String = name
        .chars()
        .filter(|c| !INVALID.contains(*c) && *c as u32 >= 32)
        .collect();
    let out = collapse_whitespace(&out)
        .trim()
        .trim_end_matches(['.', ' '])
        .to_string();
    if out.chars().count() > max_len {
        out.chars()
            .take(max_len)
            .collect::<String>()
            .trim_end_matches(['.', ' '])
            .to_string()
    } else if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

/// `\s+` → 单空格。
fn collapse_whitespace(s: &str) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+").expect("valid regex"))
        .replace_all(s, " ")
        .into_owned()
}

/// 版本号标记：`Ver_x.y` 前缀（IGNORECASE）。
const VERSION_RE: &str = r"(?:ver(?:sion)?\.?|v\.?)\s*(\d+(?:\.\d+)*)";
/// 裸版本号：`name_2.0` / `name-1.01` 形式。
const BARE_VERSION_RE: &str = r"[_\-\s](\d+\.\d+(?:\.\d+)*)\s*$";

/// 从文件名提取版本标记（如 `Ver_2.00` / `v1.01` / `_v100` / `2.0`）。
///
/// 血泪坑 §8.2.12：整理名不得丢版本，`extract_version_tag` 输出 `Ver_x.y`。
pub fn extract_version_tag(filename: &str) -> String {
    let stem = basename_stem(filename);
    if let Ok(Some(caps)) = Regex::new(&format!("(?i){VERSION_RE}"))
        .expect("valid regex")
        .captures(stem)
        && let Some(g) = caps.get(1) {
            return format!("Ver_{}", g.as_str());
        }
    if let Ok(Some(caps)) = Regex::new(BARE_VERSION_RE)
        .expect("valid regex")
        .captures(stem)
        && let Some(g) = caps.get(1) {
            return format!("Ver_{}", g.as_str());
        }
    String::new()
}

/// 取文件名末段并去掉最后一个扩展名（等价 Python `Path(filename).stem`）。
fn basename_stem(filename: &str) -> &str {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    match base.rfind('.') {
        Some(idx) if idx > 0 => &base[..idx],
        _ => base,
    }
}

/// 驼峰拆词：`LunariaPaperFan` → `Lunaria Paper Fan`（血泪坑 #8.2.10 环视）。
fn split_camel(name: &str) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let s = RE
        .get_or_init(|| Regex::new(r"(?<=[a-z])(?=[A-Z])").expect("valid regex"))
        .replace_all(name, " ")
        .into_owned();
    collapse_whitespace(&s).trim().to_string()
}

/// 纯日文主体：去尾部英文/数字/版本号，只留日文连续段。
fn ja_body(name: &str) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"[\u3040-\u30ff\u4e00-\u9fff][\u3040-\u30ff\u4e00-\u9fff・ー]*")
            .expect("valid regex")
    })
    .find_iter(name)
    .filter_map(|m| m.ok())
    .map(|m| m.as_str())
    .collect::<String>()
    .trim()
    .to_string()
}

const VRC_STOPPERS: &[&str] = &[
    "vrchat",
    "vrc",
    "unitypackage",
    "package",
    "prefab",
    "gimmick",
    "shader",
    "world",
    "avatar",
    "玩家",
    "加入",
    "退出",
    "弹窗",
    "提示",
    "通知",
    "音效",
];

/// 从文件名生成 BOOTH 搜索候选关键词（按优先级排序，首个最可能命中）。
///
/// 策略（血泪坑 #8.2.11，逐数字复刻，不得"优化"）：
///   0 下划线→空格  1 去扩展名/去括号  1.5 驼峰拆词  1.6 纯日文主体
///   2 去版本号  3 去尾部中文  4 最长 ASCII 段  5 去 VRChat 停用词 → 去重保序。
pub fn sanitize_query(filename: &str) -> Vec<String> {
    let name = basename_stem(filename).replace('_', " ");
    static PAREN_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let name = PAREN_RE
        .get_or_init(|| Regex::new(r"[\(（\[【].*?[\)）\]】]").expect("valid regex"))
        .replace_all(&name, "")
        .into_owned();

    let split_camel = split_camel(&name);
    let ja = ja_body(&name);

    let mut candidates: Vec<String> = Vec::new();
    let c1 = name.trim().to_string();
    if !c1.is_empty() {
        candidates.push(c1.clone());
    }
    if !split_camel.is_empty() && split_camel != c1 {
        candidates.push(split_camel.clone());
    }
    if !ja.is_empty() && ja != c1 && ja != split_camel && ja.chars().count() >= 2 {
        candidates.push(ja);
    }

    // 策略 2：去版本号。
    static V_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static BARE_V_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let c2 = V_RE
        .get_or_init(|| {
            Regex::new(r"(?i)[_\-\s]?(?:v(?:er(?:sion)?)?\.?|version\s*)\d+(?:\.\d+)*[_\-\s]?")
                .expect("valid regex")
        })
        .replace_all(&name, "")
        .into_owned();
    let c2 = BARE_V_RE
        .get_or_init(|| Regex::new(r"[_\-\s]\d+\.\d+(?:\.\d+)*$").expect("valid regex"))
        .replace_all(&c2, "")
        .into_owned();
    let c2 = c2.trim().to_string();
    if !c2.is_empty() && !candidates.contains(&c2) {
        candidates.push(c2.clone());
    }

    // 策略 3：去尾部中文/日文备注。
    static TAIL_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let c3 = TAIL_RE
        .get_or_init(|| {
            Regex::new(r"[\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff].*$").expect("valid regex")
        })
        .replace_all(&c2, "")
        .into_owned();
    let c3 = c3.trim().to_string();
    if !c3.is_empty() && !candidates.contains(&c3) && c3.chars().count() >= 3 {
        candidates.push(c3);
    }

    // 策略 4：最长连续 ASCII 段。
    static ASCII_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let mut ascii_parts: Vec<String> = ASCII_RE
        .get_or_init(|| Regex::new(r"[A-Za-z][A-Za-z0-9_]{2,}").expect("valid regex"))
        .find_iter(&name)
        .filter_map(|m| m.ok())
        .map(|m| m.as_str().to_string())
        .collect();
    ascii_parts.sort_by_key(|p| std::cmp::Reverse(p.len()));
    for part in ascii_parts {
        if !candidates.contains(&part) && part.chars().count() >= 4 {
            candidates.push(part);
        }
    }

    // 策略 5：去 VRChat 停用词。
    let mut c5 = c2;
    for stop in VRC_STOPPERS {
        // 血泪坑：停用词为字面量，须转义后嵌入正则（等价 Python `re.escape`）。
        let pat = format!(r"(?i)[_\-\s]?{}[_\-\s]?", escape_re(stop));
        c5 = Regex::new(&pat)
            .expect("valid regex")
            .replace_all(&c5, " ")
            .into_owned();
    }
    let c5 = collapse_whitespace(&c5).trim().to_string();
    if !c5.is_empty() && !candidates.contains(&c5) && c5.chars().count() >= 3 {
        candidates.push(c5);
    }

    // 去重保序（大小写不敏感）。
    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::new();
    for c in candidates {
        let key = c.to_lowercase();
        if seen.insert(key) {
            unique.push(c);
        }
    }
    if unique.is_empty() {
        vec![name]
    } else {
        unique
    }
}

/// 正则字面量转义（等价 Python `re.escape`）。
fn escape_re(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_invalid_chars() {
        assert_eq!(sanitize_filename(r#"a<b>:c"d/e\f|g?h*i"#), "abcdefghi");
    }

    #[test]
    fn sanitize_filename_emoji_filtered() {
        assert_eq!(sanitize_filename("雪女✨エフェクト"), "雪女エフェクト");
    }

    #[test]
    fn sanitize_filename_cjk_kept() {
        assert_eq!(
            sanitize_filename("メカ弾エフェクトVer_2.00"),
            "メカ弾エフェクトVer_2.00"
        );
    }

    #[test]
    fn sanitize_filename_truncate_80() {
        let long = "a".repeat(100);
        assert_eq!(sanitize_filename(&long).chars().count(), 80);
    }

    #[test]
    fn sanitize_filename_empty() {
        assert_eq!(sanitize_filename(""), "unnamed");
    }

    #[test]
    fn sanitize_basic() {
        assert_eq!(sanitize("a<b:c", 70), "abc");
        assert_eq!(sanitize("", 70), "untitled");
        assert_eq!(sanitize("  hello   world. ", 70), "hello world");
        assert_eq!(sanitize("longname_123456789", 5), "longn");
    }

    #[test]
    fn extract_version_tag_ver() {
        assert_eq!(
            extract_version_tag("メカ弾エフェクトVer_2.00.unitypackage"),
            "Ver_2.00"
        );
        assert_eq!(extract_version_tag("v1.01.zip"), "Ver_1.01");
        assert_eq!(extract_version_tag("_v100"), "Ver_100");
        // 无扩展名时 .0 会被当后缀剥掉 → 仅匹配 ver 前缀
        assert_eq!(extract_version_tag("name_2.0"), "");
    }

    #[test]
    fn extract_version_tag_bare() {
        assert_eq!(extract_version_tag("name_2.0.zip"), "Ver_2.0");
        assert_eq!(extract_version_tag("name-1.01.unitypackage"), "Ver_1.01");
        assert_eq!(extract_version_tag("no_version"), "");
    }

    #[test]
    fn sanitize_query_underscore() {
        assert_eq!(
            sanitize_query("SimpleJoinAlert_v100"),
            vec![
                "SimpleJoinAlert v100",
                "Simple Join Alert v100",
                "SimpleJoinAlert",
                "v100"
            ]
        );
    }

    #[test]
    fn sanitize_query_camel_split() {
        let q = sanitize_query("LunariaPaperFan.zip");
        assert_eq!(q[0], "LunariaPaperFan");
        assert_eq!(q[1], "Lunaria Paper Fan");
    }

    #[test]
    fn sanitize_query_ja_body() {
        let q = sanitize_query("メカ弾エフェクトVer_2.00.unitypackage");
        assert_eq!(q[0], "メカ弾エフェクトVer 2.00");
        assert!(q.contains(&"メカ弾エフェクト".to_string()));
    }

    #[test]
    fn sanitize_query_stoppers() {
        let q = sanitize_query("MyWorld_vrchat_1.2");
        assert!(q.contains(&"MyWorld".to_string()));
    }

    #[test]
    fn sanitize_query_no_dup() {
        let q = sanitize_query("Chocolat_Real_skin");
        let keys: Vec<String> = q.iter().map(|s| s.to_lowercase()).collect();
        let mut uniq = keys.clone();
        uniq.dedup();
        assert_eq!(keys.len(), uniq.len());
    }
}
