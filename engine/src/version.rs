//! 版本号解析与比较（单一事实源）。
//!
//! 全库仅此一处实现「取首个数字段拆版本元组 + 严格大于/相等」，供：
//!   - `update`（工具自更新：GitHub tag 与本地版本比较）
//!   - `audit`（版本巡检：官方商品名与本地目录名版本比较）
//!   - `organize`（免费版本补全去重：本地已存在同版本文件则跳过下载）
//!
//! 曾分别内联于 `update::parse_version` 与 `audit::ver_tuple`，两套实现易漂移。

/// 解析版本号：取首个数字段 `\d+(\.\d+)*` 拆成整数元组。
///
/// 兼容 `v1.0.1` / `1.0.1-rc` / `Ver_2.00` / `名前0.5`；无数字返回空元组。
/// 点后必须是数字才续段（`1.2.3.4` → [1,2,3,4]，`1.2-rc` → [1,2]）。
pub fn parse_version(ver: &str) -> Vec<u64> {
    let bytes = ver.as_bytes();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == bytes.len() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            break;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if let Ok(n) = ver[start..i].parse::<u64>() {
            parts.push(n);
        }
        if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
            i += 1;
            continue;
        }
        break;
    }
    parts
}

/// `a` 是否严格大于 `b`（逐段比较，缺段补 0）。
///
/// 任一侧无版本号返回 false。
pub fn ver_gt(a: &str, b: &str) -> bool {
    let ta = parse_version(a);
    let tb = parse_version(b);
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    for k in 0..ta.len().max(tb.len()) {
        let x = ta.get(k).copied().unwrap_or(0);
        let y = tb.get(k).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// `a` 与 `b` 是否版本等价（数值逐段相等，缺段补 0，如 `Ver_2.0` == `Ver_2.00`）。
///
/// 任一侧无版本号返回 false。
pub fn ver_eq(a: &str, b: &str) -> bool {
    let ta = parse_version(a);
    let tb = parse_version(b);
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    for k in 0..ta.len().max(tb.len()) {
        if ta.get(k).copied().unwrap_or(0) != tb.get(k).copied().unwrap_or(0) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_basic() {
        assert_eq!(parse_version("1.0.1"), vec![1, 0, 1]);
        assert_eq!(parse_version("v1.3.2"), vec![1, 3, 2]);
        assert_eq!(parse_version("1.0.1-rc"), vec![1, 0, 1]);
        assert_eq!(parse_version("garbage"), Vec::<u64>::new());
    }

    #[test]
    fn parse_version_bare_and_ja() {
        assert_eq!(parse_version("名前0.5"), vec![0, 5]);
        assert_eq!(parse_version("Ver_2.00"), vec![2, 0]);
        assert_eq!(parse_version("v100"), vec![100]);
    }

    #[test]
    fn ver_gt_basic() {
        assert!(ver_gt("1.0", "0.9"));
        assert!(!ver_gt("0.9", "1.0"));
        assert!(!ver_gt("1.0", "1.0"));
        assert!(ver_gt("2.0", "1.9"));
    }

    #[test]
    fn ver_gt_empty() {
        assert!(!ver_gt("", "1.0"));
        assert!(!ver_gt("1.0", ""));
        assert!(!ver_gt("", ""));
    }

    #[test]
    fn ver_gt_multi_segment() {
        assert!(ver_gt("1.0.1", "1.0"));
        assert!(!ver_gt("1.0", "1.0.1"));
        assert!(!ver_gt("1.0.2", "1.0.10"));
        assert!(ver_gt("1.0.10", "1.0.9"));
    }

    #[test]
    fn ver_gt_padding_zero() {
        assert!(ver_gt("1.5.1", "1.5"));
        assert!(!ver_gt("1.5", "1.5.0"));
        assert!(!ver_gt("Ver_2.00", "Ver_2"));
    }

    #[test]
    fn ver_gt_first_number_segment() {
        assert!(ver_gt("v3.0", "名前0.5"));
        assert!(!ver_gt("名前0.5", "v3.0"));
    }

    #[test]
    fn ver_eq_padding_zero() {
        assert!(ver_eq("Ver_2.0", "Ver_2.00"));
        assert!(ver_eq("Ver_2", "Ver_2.0.0"));
        assert!(ver_eq("1.0.1", "1.0.1"));
        assert!(!ver_eq("1.0.1", "1.0.2"));
        assert!(!ver_eq("1.0.1", "1.0.1.1"));
        assert!(!ver_eq("", "1.0"));
        assert!(!ver_eq("Ver_2.0", ""));
    }
}
