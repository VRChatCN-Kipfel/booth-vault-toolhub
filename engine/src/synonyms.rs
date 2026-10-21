//! 行业同义词：中文泛称 → BOOTH 常用日文检索词。
//!
//! 为什么需要：BOOTH 的检索是**日文**的。用户拿到的文件名常有中文泛称
//! （「猫耳」「铃铛」），直接丢给站点几乎必然零命中——这不是词的错，是语言层面的
//! 不对齐。`kana` 只能把**日文**汉字读音展开，解决不了中文到日文的跨度。
//!
//! 设计取舍：
//!   - 纯静态表，不引入分词器或在线翻译，行为可预测、可评审、可测试。
//!   - **只做正向追加**：命中时在原候选之后追加日文词，绝不替换原词。
//!     原词命中率未必为零（有些商品名本身就含中文），替换会砍掉本来能中的路径。
//!   - 一个中文词可对应多个日文词（排序即优先级），调用方按请求预算截断。
//!
//! 维护约定：新增词必须来自**实际搜不到的案例**，不要按想象扩充——词表每涨一项，
//! 就多一条可能引入误检索的路径。

/// 中文泛称 → 日文检索词（按优先级排序）。
///
/// 覆盖 VRChat 改模常见物件/功能/素材。刻意保持紧凑：只收录实测会「零命中」的词。
pub const INDUSTRY_SYNONYMS: &[(&str, &[&str])] = &[
    // ── 服饰 ──
    ("猫耳", &["猫耳", "ねこみみ"]),
    ("女仆装", &["メイド服", "メイド"]),
    ("裙子", &["スカート"]),
    ("和服", &["着物", "浴衣"]),
    ("眼镜", &["メガネ", "眼鏡"]),
    ("墨镜", &["サングラス"]),
    ("帽子", &["帽子", "キャップ"]),
    ("鞋子", &["靴", "シューズ"]),
    ("袜子", &["靴下", "ソックス"]),
    ("手套", &["手袋", "グローブ"]),
    ("围巾", &["マフラー"]),
    ("项圈", &["チョーカー", "首輪"]),
    ("尾巴", &["しっぽ", "尻尾"]),
    ("翅膀", &["翼", "ウイング"]),
    ("角", &["角", "ホーン"]),
    // ── 配件/饰物 ──
    ("铃铛", &["鈴", "ベル"]),
    ("发饰", &["ヘアアクセ", "髪飾り"]),
    ("发圈", &["シュシュ"]),
    ("耳环", &["ピアス", "イヤリング"]),
    ("戒指", &["リング", "指輪"]),
    ("项链", &["ネックレス"]),
    ("手镯", &["ブレスレット"]),
    // ── 发型/妆容 ──
    ("发型", &["髪型", "ヘア"]),
    ("双马尾", &["ツインテール"]),
    ("渐变发色", &["グラデーション", "グラデ髪"]),
    ("美瞳", &["瞳", "アイテクスチャ"]),
    ("妆容", &["メイク", "化粧"]),
    // ── 功能/机关 ──
    ("机关", &["ギミック"]),
    ("表情", &["表情", "エクスプレッション"]),
    ("摇摇", &["揺れもの", "物理"]),
    ("换装", &["衣装チェンジ"]),
    ("拍头", &["頭なで"]),
    ("手势", &["ジェスチャー"]),
    // ── 素材 ──
    ("法线贴图", &["ノーマルマップ"]),
    ("贴图", &["テクスチャ"]),
    ("着色器", &["シェーダー"]),
    ("特效", &["エフェクト"]),
    ("粒子", &["パーティクル"]),
    ("音效", &["サウンド", "効果音"]),
    ("字体", &["フォント"]),
    ("图标", &["アイコン"]),
    ("模型", &["3Dモデル"]),
    ("衣装", &["衣装", "コスチューム"]),
];

/// 为单个检索词展开日文同义词。
///
/// 匹配规则：候选词**包含**表项中文词即命中（如「可爱的猫耳」命中 `猫耳`）。
/// 返回的日文词已去掉与输入完全相同的项，并按 `max` 截断。
pub fn expand(text: &str, max: usize) -> Vec<String> {
    if max == 0 || text.trim().is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    seen.insert(text.trim().to_string());
    for (zh, ja_list) in INDUSTRY_SYNONYMS {
        if !text.contains(zh) {
            continue;
        }
        for ja in *ja_list {
            if out.len() >= max {
                return out;
            }
            let key = ja.trim().to_string();
            if key.is_empty() || seen.contains(&key) {
                continue;
            }
            seen.insert(key.clone());
            out.push(key);
        }
    }
    out
}

/// 在一批候选之后追加同义词变体（保持原候选顺序不变）。
///
/// 与 `kana` 变体同一套追加语义：原候选永远优先，同义词只作为补充召回。
pub fn with_synonyms(candidates: Vec<String>, max_per_candidate: usize) -> Vec<String> {
    if max_per_candidate == 0 {
        return candidates;
    }
    let mut seen: std::collections::HashSet<String> =
        candidates.iter().map(|c| c.to_lowercase()).collect();
    let mut extra: Vec<String> = Vec::new();
    for c in &candidates {
        for v in expand(c, max_per_candidate) {
            if seen.insert(v.to_lowercase()) {
                extra.push(v);
            }
        }
    }
    let mut out = candidates;
    out.extend(extra);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_generic_word() {
        let v = expand("猫耳", 3);
        assert!(!v.is_empty(), "猫耳 应有日文同义词");
        assert!(v.iter().any(|s| s.contains("ねこみみ")), "实际：{v:?}");
    }

    #[test]
    fn matches_when_word_is_embedded() {
        // 真实文件名不会只有两个字，包含匹配才有用。
        let v = expand("可爱的猫耳头饰", 3);
        assert!(!v.is_empty(), "包含匹配应生效：{v:?}");
    }

    /// 纯日文/英文不该被中文词表改写——那是 `kana` 的职责，越界会引入噪声。
    #[test]
    fn does_not_touch_non_chinese() {
        assert!(expand("キプフェル", 3).is_empty());
        assert!(expand("LunariaPaperFan", 3).is_empty());
    }

    #[test]
    fn never_echoes_input() {
        // 「猫耳」本身也是日文可检索词，但表里首项与输入相同，必须被过滤掉。
        let v = expand("猫耳", 5);
        assert!(!v.iter().any(|s| s == "猫耳"), "不得回显输入：{v:?}");
    }

    #[test]
    fn respects_max() {
        for cap in 0..=3 {
            assert!(expand("衣装", cap).len() <= cap);
        }
        assert!(expand("衣装", 0).is_empty());
    }

    #[test]
    fn append_keeps_original_order() {
        let base = vec!["orig1".to_string(), "猫耳".to_string()];
        let out = with_synonyms(base.clone(), 2);
        assert_eq!(&out[..2], &base[..], "原候选顺序不得改变");
        assert!(out.len() > base.len(), "应追加同义词");
    }

    #[test]
    fn dedupes_case_insensitively() {
        let base = vec![
            "orig".to_string(),
            "猫耳".to_string(),
            "ねこみみ".to_string(),
        ];
        let out = with_synonyms(base, 2);
        let mut seen = std::collections::HashSet::new();
        for c in &out {
            assert!(seen.insert(c.to_lowercase()), "出现重复候选：{c}");
        }
    }

    #[test]
    fn empty_input_is_safe() {
        assert!(expand("", 3).is_empty());
        assert!(expand("   ", 3).is_empty());
    }
}
