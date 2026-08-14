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
}
