//! 归一化：小写 + 去所有非字母数字（消空格/标点/中日假名噪声）。
//! 等价 Python `re.sub(r'[^a-z0-9]', '', (s or '').lower())`，逐字复刻不得漂移。

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
        // Python `_norm` 不做 NFD 分解：é 直接删 → "caf"（非 "cafe"）。
        // NFD 归一化按血泪坑 #8.3.19 由去重/归档层负责，此处不得自作主张。
        assert_eq!(norm("Café"), "caf");
        assert_eq!(norm("Cafe\u{301}"), "cafe");
    }
}
