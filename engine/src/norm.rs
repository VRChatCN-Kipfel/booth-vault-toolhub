//! 归一化：小写 + 去所有非字母数字（消空格/标点/中日假名噪声）。
//!
//! 注：此处**不做** Unicode 分解归一化（NFD）。带组合字符的字符串由
//! 去重/归档层先做 NFD 再调用本函数；在此展开会改变比对语义。

pub fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// 与 [`norm`] 相同的去噪语义，但**保留 West-East 文字**（汉字/假名/全角）。
///
/// 存在的理由：`norm` 只留 ASCII，导致任何日文查询串被归一成空串，调用方
/// 的 `!qn.is_empty()` 前置检查因而短路了全部名称命中判定——日文文件名永远
/// 拿不到「名称命中」这一档分。这不是 kana 的局部问题，是评分层对日文的系统性失效。
///
/// 为什么保留两个函数而不是直接改 `norm`：`norm` 的「只留 ASCII」是既有行为契约
/// （`norm_lower_strip` 里 `norm("メカ弾エフェクト") == ""` 钉住了它），改动会让
/// 已在线上调优过的匹配结果整体漂移。新增 `norm_wide` 供需要支持 CJK 的路径显式选用。
pub fn norm_wide(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || is_cjk(*c))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// 是否属于 CJK 相关文字（汉字 / 平假名 / 片假名 / 全角假名扩展）。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3041..=0x309F |   // 平假名
        0x30A0..=0x30FF |   // 片假名
        0x3400..=0x4DBF |   // 汉字扩展 A
        0x4E00..=0x9FFF |   // 汉字基本区
        0xF900..=0xFAFF |   // 兼容汉字
        0xFF66..=0xFF9F     // 半角片假名
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_lower_and_strip() {
        assert_eq!(norm("Hello World"), "helloworld");
        assert_eq!(norm("メカ弾エフェクト"), "");
        assert_eq!(norm("Star-Tiara_v1.0"), "startiarav10");
        assert_eq!(norm(""), "");
    }

    #[test]
    fn norm_no_nfd_expansion() {
        // 不做 NFD 分解：é 直接删除 → "caf"；NFD 形式单独存在 → "cafe"。
        // 两者在此不同，由调用方决定是否先归一化。
        assert_eq!(norm("Café"), "caf");
        assert_eq!(norm("Cafe\u{301}"), "cafe");
    }

    #[test]
    fn norm_wide_keeps_cjk() {
        // 核心回归：日文查询串经 norm 后为空，是本 PR 的 P0-2 根因。
        assert_eq!(
            norm("信濃セット"),
            "",
            "既有 norm 对日文恒空（行为契约，不改）"
        );
        assert_eq!(
            norm_wide("信濃セット"),
            "信濃セット",
            "norm_wide 必须保留 CJK"
        );
        assert_eq!(norm_wide("メカ弾エフェクト"), "メカ弾エフェクト");
        assert_eq!(norm_wide("ねこみみ"), "ねこみみ");
    }

    #[test]
    fn norm_wide_still_strips_noise() {
        // 去噪语义必须与 norm 一致，否则会把空格/标点差异误判成不同名。
        assert_eq!(norm_wide("Star-Tiara_v1.0"), "startiarav10");
        assert_eq!(norm_wide("Hello World"), "helloworld");
        assert_eq!(norm_wide("【信濃セット】"), "信濃セット");
        assert_eq!(norm_wide(""), "");
    }
}
