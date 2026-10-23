//! 日文读音变体：把「汉字写法的检索词」展开成 BOOTH 能匹配的假名写法。
//!
//! 背景：BOOTH 搜索不做跨字形归一（信濃 与 しなの 在站内是两个词）。文件名里常见的
//! 日文汉字名，若只按原写法检索，会漏掉作者用假名写法上架的同款商品；反向同理。
//!
//! 实现：ipadic 词典 + 形态素分析取 `reading` 字段（片假名），再派生平假名写法。
//! 词典经 `embed-ipadic` 编译进二进制——**这会增加约 46MB**，故整个模块挂在
//! cargo feature `kana` 下（默认开启），可用 `--no-default-features` 关掉换取小体积。
//!
//! 边界（实测，勿擅自扩展）：
//!   - 中文词（如「铃铛」）在 ipadic 里没有条目，`reading` 返回 `*`，本模块输出为空
//!     ——中文泛称到日文行业词的映射是另一件事（词表负责），不要指望这里兜住。
//!   - 需求说明、兼容要求等长句不应走这里：变体只对**短检索词**有意义。

use std::borrow::Cow;
use std::sync::OnceLock;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;

/// 词典缺失时一律返回空（降级为「只用原词检索」），绝不 panic。
fn segmenter() -> Option<&'static Segmenter> {
    static SEG: OnceLock<Option<Segmenter>> = OnceLock::new();
    SEG.get_or_init(|| {
        let dict = load_dictionary("embedded://ipadic").ok()?;
        Some(Segmenter::new(Mode::Normal, dict, None))
    })
    .as_ref()
}

/// 单次展开最多产出的变体数。
///
/// 每个变体在被调用方（`search` / `process_search_file`）都会被当成一次独立检索，
/// 而 BOOTH 风控按请求数计。上限压到 2，保证失败路径最多多花 2 次请求。
pub const MAX_VARIANTS: usize = 2;

fn is_kanji(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF)
}

fn is_katakana(c: char) -> bool {
    matches!(c as u32, 0x30A1..=0x30F6 | 0x30FD..=0x30FE)
}

fn is_hiragana(c: char) -> bool {
    matches!(c as u32, 0x3041..=0x3096 | 0x309D..=0x309E)
}

/// 片假名 → 平假名（区间平移 0x60）。非假名字符原样保留。
fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c as u32 {
            0x30A1..=0x30F6 => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// 平假名 → 片假名（区间平移 0x60）。
fn hiragana_to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| match c as u32 {
            0x3041..=0x3096 => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// 取整串读音（ipadic 的 `reading` 字段为片假名）。
///
/// 词典里没有读数字段（`*`）的 token 原样保留写法——ASCII/数字/中文都属于这类，
/// 这样变体仍然能作为一条完整检索词使用，而不是被切碎。
fn reading_of(text: &str) -> Option<String> {
    {
        let seg = segmenter()?;
        let Ok(mut tokens) = seg.segment(Cow::Borrowed(text)) else {
            return None;
        };
        let mut out = String::new();
        for t in tokens.iter_mut() {
            // 必须先复制 surface：`t.get()` 要 &mut self，而持有 surface 的借用会与它冲突。
            let surface = t.surface.to_string();
            match t.get("reading") {
                Some(r) if r != "*" && !r.is_empty() => out.push_str(r),
                _ => out.push_str(&surface),
            }
        }
        let out = out.trim().to_string();
        if out.is_empty() || out == text {
            None
        } else {
            Some(out)
        }
    }
}

/// 生成检索变体（按优先级排序，调用方按序尝试即可）。
///
/// 产出规则：
///   1. 含**汉字** → 取整串读音（片假名），再派生其平假名写法；
///   2. 纯**假名** → 直接在片假名/平假名之间互转（メカ ⇄ めか）；
///   3. 全 ASCII / 纯中文 → 无变体（返回空，调用方无需额外请求）。
pub fn reading_variants(text: &str, max: usize) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() || max == 0 {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut push = |v: String| {
        if out.len() < max && v != text && !out.contains(&v) && !v.trim().is_empty() {
            out.push(v);
        }
    };

    if text.chars().any(is_kanji) {
        if let Some(reading) = reading_of(text) {
            push(reading.clone());
            let hira = katakana_to_hiragana(&reading);
            if hira != reading {
                push(hira);
            }
        }
    } else if text.chars().any(is_katakana) {
        push(katakana_to_hiragana(text));
    } else if text.chars().any(is_hiragana) {
        push(hiragana_to_katakana(text));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kanji_gets_reading_variant() {
        let v = reading_variants("信濃", MAX_VARIANTS);
        assert!(!v.is_empty(), "汉字词必须产出读音变体");
        assert!(
            v.iter()
                .any(|s| s.contains("シナノ") || s.contains("しなの")),
            "读音应包含 シノノ/しなの，实际：{v:?}"
        );
    }

    #[test]
    fn katakana_flips_to_hiragana() {
        let v = reading_variants("メカ", MAX_VARIANTS);
        assert_eq!(v, vec!["めか".to_string()]);
    }

    #[test]
    fn hiragana_flips_to_katakana() {
        let v = reading_variants("ねこみみ", MAX_VARIANTS);
        assert_eq!(v, vec!["ネコミミ".to_string()]);
    }

    /// 中文泛称在 ipadic 里没有读音，必须老实返回空——不能让调用方白花请求。
    #[test]
    fn chinese_has_no_variant() {
        assert!(reading_variants("铃铛", MAX_VARIANTS).is_empty());
    }

    #[test]
    fn ascii_has_no_variant() {
        assert!(reading_variants("LunariaPaperFan", MAX_VARIANTS).is_empty());
    }

    #[test]
    fn respects_max_and_never_echoes_input() {
        for cap in 0..=MAX_VARIANTS {
            let v = reading_variants("信濃", cap);
            assert!(v.len() <= cap);
            assert!(!v.iter().any(|s| s == "信濃"));
        }
    }

    #[test]
    fn empty_input_is_safe() {
        assert!(reading_variants("", MAX_VARIANTS).is_empty());
        assert!(reading_variants("   ", MAX_VARIANTS).is_empty());
    }
}
