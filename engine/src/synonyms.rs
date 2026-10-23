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
    // 注意：**日文列表里不得再出现与中文键完全相同的项**。历史上 5 组自反
    // （猫耳/帽子/角/表情/衣装），自反项会占满 `max` 预算却不带来任何新写法，
    // 实测 `expand("女仆装猫耳套装", 2)` 因此只拿到 `["猫耳","ねこみみ"]`，
    // `メイド服` 永远轮不到。
    ("猫耳", &["ねこみみ"]),
    ("女仆装", &["メイド服", "メイド"]),
    ("裙子", &["スカート"]),
    ("和服", &["着物", "浴衣"]),
    ("眼镜", &["メガネ", "眼鏡"]),
    ("墨镜", &["サングラス"]),
    ("帽子", &["キャップ", "ハット"]),
    ("鞋子", &["靴", "シューズ"]),
    ("袜子", &["靴下", "ソックス"]),
    ("手套", &["手袋", "グローブ"]),
    ("围巾", &["マフラー"]),
    ("项圈", &["チョーカー", "首輪"]),
    ("尾巴", &["しっぽ", "尻尾"]),
    ("翅膀", &["翼", "ウイング"]),
    // 「角」是单字键，会误命中「角色」「角度」「三角」等无关词（实测
    // `expand("角色模型", 4)` 给出 `ホーン`）。故整条移除：只靠单字无法
    // 区分「犄角」与「角色」，宁缺毋滥。
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
    ("表情", &["エクスプレッション"]),
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
    ("衣装", &["コスチューム"]),
];

/// 判断一个候选商品的名称/规范名是否**真的命中了检索词**。
///
/// 为什么需要它：`score_and_pick` 只返回「最优 + 歧义」，而次序分让首条恒有正分，
/// 于是「搜出东西 ≠ 命中」。同义词轮/读音轮是**猜的词**，猜错代价是错归档，
/// 必须用「名称真的含这个词」把关。
///
/// **不做归一化**：`norm` 只保留 ASCII，日文会被滤成空串而恒判不命中；
/// 这里用大小写不敏感的原串包含，对中日英都成立且不丢字。
pub fn name_hit(query: &str, name: &str, canonical: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return false;
    }
    name.to_lowercase().contains(&q) || canonical.to_lowercase().contains(&q)
}

/// 为单个检索词展开日文同义词。
///
/// 匹配规则：候选词**包含**表项中文词即命中（如「可爱的猫耳」命中 `猫耳`）。
///
/// 三条**程序化**保证（不依赖人工维护表，因为表会腐化）：
///   1. 返回项永不等同于输入本身（自反项占预算却不带来新写法）；
///   2. 中文键长度必须 ≥2——单字键会误命中无关词（「角」命中「角色」）；
///   3. 返回项永不等同于中文键——否则它就是原词，没有扩展意义。
pub fn expand(text: &str, max: usize) -> Vec<String> {
    if max == 0 || text.trim().is_empty() {
        return Vec::new();
    }
    let input = text.trim();
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    seen.insert(input.to_string());
    for (zh, ja_list) in INDUSTRY_SYNONYMS {
        // 保证 2：单字中文键不参与匹配（太容易误命中）。
        if zh.chars().count() < 2 || !text.contains(zh) {
            continue;
        }
        for ja in *ja_list {
            if out.len() >= max {
                return out;
            }
            let key = ja.trim().to_string();
            // 保证 1 与 3：跳过空项、输入本身、以及等于中文键的项。
            if key.is_empty() || key == *zh || key == input || seen.contains(&key) {
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

    /// 全表审计：任何组都不得有「日文项 == 中文键」的自反项。
    ///
    /// 历史上 5 组（猫耳/帽子/角/表情/衣装）有此问题，自反项会占满 `max` 预算，
    /// 让真正有用的日文词（如 `メイド服`）永远轮不到。
    #[test]
    fn table_has_no_self_referential_items() {
        for &(zh, ja_list) in INDUSTRY_SYNONYMS {
            for &ja in ja_list {
                assert_ne!(ja, zh, "组 ({zh}) 含自反项 {ja}：它会占预算却不带来新写法");
            }
        }
    }

    /// 中文键必须 ≥2 字符：单字键会误命中无关词。
    ///
    /// 实例：「角」会命中「角色」「角度」「三角」，把 `ホーン` 塞进完全不相关的查询。
    #[test]
    fn table_has_no_single_char_keys() {
        for &(zh, _) in INDUSTRY_SYNONYMS {
            assert!(
                zh.chars().count() >= 2,
                "组 {zh} 是单字键，会误命中无关词（如「角」命中「角色」）"
            );
        }
    }

    /// 单字中文词不得被展开成任何日文词。
    #[test]
    fn single_char_query_expands_to_nothing() {
        // 「角」已从表中移除；即便将来有人加回，`expand` 的 ≥2 字符门槛也会挡住。
        assert!(expand("角度", 4).is_empty(), "「角度」不得展开");
        assert!(expand("角色", 4).is_empty(), "「角色」不得展开");
        // 「角色模型」含「模型」组，得到 `3Dモデル` 是**正确**的；
        // 关键是不能出现「角」组留下的 `ホーン`。
        let v = expand("角色模型", 4);
        assert!(
            !v.iter().any(|s| s.contains("ホーン")),
            "「角色模型」不得因单字「角」而展开出 ホーン，实际：{v:?}"
        );
    }

    /// 预算紧张时不能只拿到自反/无用项：真实场景回归。
    ///
    /// 实测历史缺陷：`expand("女仆装猫耳套装", 2)` 返回 `["猫耳","ねこみみ"]`，
    /// 使 `メイド服` 永远拿不到——因为 `猫耳` 组排在前且含自反项。
    #[test]
    fn tight_budget_still_yields_useful_terms() {
        let v = expand("女仆装猫耳套装", 4);
        assert!(
            v.iter().any(|s| s == "メイド服"),
            "预算内应拿到 メイド服，实际：{v:?}"
        );
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

    // ── name_hit：防止「搜出即命中」 ──────────────────────────────

    /// 名称真的含检索词 → 命中。
    #[test]
    fn name_hit_positive() {
        assert!(name_hit("猫耳", "猫耳セット", ""));
        assert!(name_hit("猫耳", "", "猫耳セット")); // 规范名命中也算
    }

    /// **假名互转不在此层**：`ねこみみ`（平假名）与 `ネコミミ`（片假名）是不同字符串，
    /// `name_hit` 是纯字面包含，不该判命中——字形互认属 `kana` 的职责，本分支不含。
    /// 本测试把这个边界钉住，避免有人误以为这里是读音层。
    #[test]
    fn name_hit_does_not_do_kana_folding() {
        assert!(!name_hit("ねこみみ", "ネコミミ風", ""));
        // 同字形则命中。
        assert!(name_hit("ネコミミ", "ネコミミ風", ""));
    }

    /// **关键负例**：名称完全不含检索词时不得判命中。
    ///
    /// 历史缺陷：同义词轮只要搜出 ≥1 条结果（任何结果）就被采用并归档。
    #[test]
    fn name_hit_rejects_unrelated() {
        assert!(!name_hit("猫耳", "メカ弾エフェクト", "メカ弾エフェクト"));
        assert!(!name_hit("铃铛", "雪女モデル", ""));
    }

    /// 大小写不敏感（ASCII 场景）。
    #[test]
    fn name_hit_case_insensitive() {
        assert!(name_hit("startiara", "StarTiara Base", ""));
        assert!(name_hit("StarTiara", "startiara", ""));
    }

    /// 日文不得因归一化被滤掉（这是 `norm` 的失效场景，故此处不用它）。
    #[test]
    fn name_hit_works_for_japanese() {
        assert!(name_hit("信濃", "信濃セット", ""));
        assert!(!name_hit("信濃", "メカ弾", ""));
    }

    /// 空查询不得命中任何东西。
    #[test]
    fn name_hit_empty_query_is_false() {
        assert!(!name_hit("", "任何名字", ""));
        assert!(!name_hit("   ", "任何名字", ""));
    }
}
