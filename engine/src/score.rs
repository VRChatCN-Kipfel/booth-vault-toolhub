//! 评分选优：score_and_pick。
//!
//! 评分权重与歧义判定为**线上行为契约**（+100/+20/-10、歧义分差 <30、
//! 同名不同价必报歧义、单结果也须名称命中），改动会影响选优结果。
//! `canonical_name` 与资源名提取以依赖注入传入，保持本层无 IO 可测。

use crate::norm::norm;

/// 搜索结果条目（调用方从 BOOTH JSON 提取）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub price: i64,
}

/// description 文本的来源（随条目可选提供）。
///
/// 与 `Item` 分开而不是直接塞进 `Item`：`Item` 是既有三端契约的公开结构，
/// 加字段会让所有构造点必须改；而且多数调用路径（搜索页）本来就没有说明文。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DescriptionText(pub String);

/// description 加权配置。
///
/// **默认全 0 即行为完全不变**——这是本改动的安全阀：不显式开启就不会影响
/// 任何既有选优结果。开启后每个命中的词加 `word_bonus` 分。
///
/// 权重必须显著小于名称命中（+100）：说明文里提到一个词，远不如标题里有这个词
/// 可靠（说明可能只是列表、免责声明、或提到「不适用于 XX」）。故上限刻意压到
/// 单个名称命中的一小部分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescWeight {
    /// 说明文中每个命中词的分值（0 = 不参与评分）。
    pub word_bonus: i32,
    /// 最多计多少个命中词（防长说明文靠堆词刷分）。
    pub max_words: usize,
}

impl Default for DescWeight {
    fn default() -> Self {
        // 默认关闭。开启与否由调用方（CLI/MCP/GUI）决定，判定逻辑本身不变。
        Self {
            word_bonus: 0,
            max_words: 3,
        }
    }
}

impl DescWeight {
    /// 关闭状态（等价于历史行为）。
    pub const OFF: Self = Self {
        word_bonus: 0,
        max_words: 0,
    };

    /// 是否实际参与评分。
    pub fn is_on(&self) -> bool {
        self.word_bonus > 0 && self.max_words > 0
    }
}

/// 带 description 加权的评分选优。
///
/// 与 [`score_and_pick`] 的关系：**后者保持逐字节不变**（既有契约与 9 个单测锁着），
/// 本函数复制同一套判定并**只在开启 `DescWeight` 时**追加一项加分。
/// `weight.is_on() == false` 时两者结果必须完全一致——有测试守着。
///
/// 为什么说明文加分要单独一个入口而不是给 `score_and_pick` 加参数：
/// 加参数会强制所有既有调用点改动，且让「不传说明文」这条路径也走新分支，
/// 凭空扩大回归面。宁可多一个函数，也不动已经验证过的判据。
///
/// `descriptions[i]` 对应 `items[i]`（长度不足时按空处理）。
pub fn score_and_pick_with_desc<'a, F>(
    query: &str,
    items: &'a [Item],
    descriptions: &[DescriptionText],
    weight: DescWeight,
    prefer_free: bool,
    canonical_name: F,
    resource_names: Option<&[String]>,
) -> (Option<&'a Item>, bool)
where
    F: Fn(&str) -> String,
{
    // 未开启：直接走原实现，保证一分不差。
    if !weight.is_on() {
        return score_and_pick(query, items, prefer_free, canonical_name, resource_names);
    }
    let (picked, amb) = score_and_pick(query, items, prefer_free, &canonical_name, resource_names);
    if items.len() <= 1 {
        // 单结果与否决逻辑由原实现决定；加分只影响多候选排序，单候选不存在排序。
        return (picked, amb);
    }
    // 多候选：用带说明文加分的分数重新排序，其余判据（≤0 否决、歧义阈值、
    // 同名不同价）完全复用原实现的语义。这里刻意重算而不是改原函数内部，
    // 是为了让「旧判据」保持可独立验证。
    let qn = norm(query);
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (idx, it) in items.iter().enumerate() {
        let name_l = it.name.to_lowercase();
        let cn = canonical_name(&it.id);
        let mut s = 0i32;
        if !qn.is_empty() && norm(&name_l).contains(&qn) {
            s += 100;
        }
        if !qn.is_empty() && norm(&cn).contains(&qn) {
            s += 100;
        }
        for w in split_query_words(query) {
            if w.chars().count() >= 3 && name_l.contains(&w) {
                s += 20;
            }
        }
        s += (10 - (idx as i32) * 2).max(0);
        if prefer_free && it.price == 0 {
            s += 5;
        }
        if name_l.chars().count() as i64 > query.chars().count() as i64 * 5
            && cn.chars().count() as i64 > query.chars().count() as i64 * 5
        {
            s -= 10;
        }
        // ── 唯一的改动：说明文命中加分（受上限约束）──
        if let Some(d) = descriptions.get(idx) {
            let hits = desc_word_hits(query, &d.0, weight.max_words);
            s += weight.word_bonus * hits as i32;
        }
        scored.push((s, idx));
    }
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    if scored.is_empty() || scored[0].0 <= 0 {
        return (picked, amb);
    }
    let best_s = scored[0].0;
    let mut ambiguous = scored.len() > 1 && (best_s - scored[1].0) < 30;
    let best_idx = scored[0].1;
    let best_cn = norm(&canonical_name(&items[best_idx].id));
    let best_price = items[best_idx].price;
    for (_, idx2) in &scored[1..] {
        let it2 = &items[*idx2];
        if norm(&canonical_name(&it2.id)) == best_cn && it2.price != best_price {
            ambiguous = true;
        }
    }
    (Some(&items[best_idx]), ambiguous)
}

/// 说明文中命中的检索词个数（受 `max_words` 上限）。
///
/// 判据与词级名称匹配一致（≥3 字符），但**忽略大小写且不做归一化**：
/// 说明文是自由文本，过度匹配会引入噪声。上限防止长说明靠堆词刷分。
fn desc_word_hits(query: &str, desc: &str, max_words: usize) -> usize {
    if desc.is_empty() || max_words == 0 {
        return 0;
    }
    let d = desc.to_lowercase();
    let mut hits = 0usize;
    for w in split_query_words(query) {
        if hits >= max_words {
            break;
        }
        if w.chars().count() >= 3 && d.contains(&w) {
            hits += 1;
        }
    }
    hits
}

/// 评分选最佳。
///
/// `prefer_free`：同分候选时优先免费项（付费/免费同款场景；付费项不因此被排除）。
/// `canonical_name`：商品规范名解析器（JSON 名，含英文别名；调用方负责缓存/网络）。
/// `resource_names`：已从 zip 内 .unitypackage 提取的资源名集合（可选，仅单结果兜底用）。
///
/// 返回 `(最佳条目引用, 是否歧义)`。
pub fn score_and_pick<'a, F>(
    query: &str,
    items: &'a [Item],
    prefer_free: bool,
    canonical_name: F,
    resource_names: Option<&[String]>,
) -> (Option<&'a Item>, bool)
where
    F: Fn(&str) -> String,
{
    if items.is_empty() {
        return (None, false);
    }
    let qn = norm(query);

    // 评分权重契约（+100 名称命中 / +100 规范名命中 / +20 词级 / +10-次序 / -10 过长）。
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (idx, it) in items.iter().enumerate() {
        let name_l = it.name.to_lowercase();
        let cn = canonical_name(&it.id);
        let mut s = 0i32;
        if !qn.is_empty() && norm(&name_l).contains(&qn) {
            s += 100;
        }
        if !qn.is_empty() && norm(&cn).contains(&qn) {
            s += 100;
        }
        for w in split_query_words(query) {
            if w.chars().count() >= 3 && name_l.contains(&w) {
                s += 20;
            }
        }
        s += (10 - (idx as i32) * 2).max(0);
        if prefer_free && it.price == 0 {
            s += 5;
        }
        if name_l.chars().count() as i64 > query.chars().count() as i64 * 5
            && cn.chars().count() as i64 > query.chars().count() as i64 * 5
        {
            s -= 10;
        }
        scored.push((s, idx));
    }
    // 按分数降序；同分保持输入顺序（稳定排序，`sort_by_key` 稳定）。
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));

    if scored.is_empty() || scored[0].0 <= 0 {
        // 无任何名称匹配：唯一结果也须名称命中。
        if items.len() == 1 {
            let it = &items[0];
            let cn = norm(&canonical_name(&it.id));
            if !qn.is_empty() && (cn.contains(&qn) || norm(&it.name).contains(&qn)) {
                return (Some(it), false);
            } // 标题/规范名都不命中 → 资源名二次校验。
            if let Some(res_names) = resource_names
                && !res_names.is_empty()
            {
                for r in res_names {
                    let rn = norm(r);
                    if !qn.is_empty() && (rn.contains(&qn) || qn.contains(&rn)) {
                        return (Some(it), false);
                    }
                    for w in split_query_words(query) {
                        if w.chars().count() >= 3 && r.to_lowercase().contains(&w) {
                            return (Some(it), false);
                        }
                    }
                }
            }
            return (None, false);
        }
        return (None, false);
    }

    let best_s = scored[0].0;
    let mut ambiguous = false;
    if scored.len() > 1 && (best_s - scored[1].0) < 30 {
        ambiguous = true;
    }
    let best_idx = scored[0].1;
    let best_cn = norm(&canonical_name(&items[best_idx].id));
    let best_price = items[best_idx].price;
    for (_, idx2) in &scored[1..] {
        let it2 = &items[*idx2];
        if norm(&canonical_name(&it2.id)) == best_cn && it2.price != best_price {
            ambiguous = true;
        }
    }
    (Some(&items[best_idx]), ambiguous)
}

/// `re.split(r'[_\-\s]+', query.lower())` 等价。
fn split_query_words(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| c == '_' || c == '-' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, name: &str, price: i64) -> Item {
        Item {
            id: id.to_string(),
            name: name.to_string(),
            price,
        }
    }

    /// 规范名解析器（测试用：直接查表，带缓存语义）。
    fn cn<'a>(table: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> String + 'a {
        move |id| {
            table
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, n)| n.to_string())
                .unwrap_or_default()
        }
    }

    #[test]
    fn score_empty() {
        let (best, amb) = score_and_pick("q", &[], false, |_| String::new(), None);
        assert!(best.is_none());
        assert!(!amb);
    }

    #[test]
    fn score_single_hit() {
        let items = [item("7437723", "Lunaria Paper Fan", 0)];
        let names = [("7437723", "LunariaPaperFan")];
        let (best, amb) = score_and_pick("LunariaPaperFan", &items, false, cn(&names), None);
        assert_eq!(best.map(|i| i.id.as_str()), Some("7437723"));
        assert!(!amb);
    }

    #[test]
    fn score_ambiguous_close_scores() {
        let items = [
            item("1111111", "Star Tiara", 0),
            item("2222222", "Star Tiara Gold", 0),
        ];
        let names = [("1111111", "Star Tiara"), ("2222222", "Star Tiara Gold")];
        let (best, amb) = score_and_pick("StarTiara", &items, false, cn(&names), None);
        assert_eq!(best.map(|i| i.id.as_str()), Some("1111111"));
        assert!(amb);
    }

    #[test]
    fn score_no_match_single() {
        // 单结果也须名称命中。短名不命中时 idx+10 仍 >0 直接返回；
        // 超长名触发 -10 惩罚使 s<=0 → 进入兜底，名称不命中 → None。
        let long_name = "A".repeat(60);
        let items = [item("9999999", &long_name, 0)];
        let names = [("9999999", long_name.as_str())];
        let (best, _) = score_and_pick("Moonpiercer", &items, false, cn(&names), None);
        assert!(best.is_none());
    }

    #[test]
    fn score_long_cn_hit_single() {
        // 超长名惩罚后 s<=0，但规范名命中查询词 → 仍返回。
        let long_name = "A".repeat(60);
        let items = [item("9999999", &long_name, 0)];
        let names = [("9999999", "Moonpiercer Studio")];
        let (best, _) = score_and_pick("Moonpiercer", &items, false, cn(&names), None);
        assert_eq!(best.map(|i| i.id.as_str()), Some("9999999"));
    }

    #[test]
    fn score_single_resource_fallback() {
        let items = [item("7441550", "Agent Owl", 0)];
        let names = [("7441550", "Agent Owl")];
        let resources = vec!["Assets/Moonpiercer.prefab".to_string()];
        let (best, _) = score_and_pick("Moonpiercer", &items, false, cn(&names), Some(&resources));
        assert_eq!(best.map(|i| i.id.as_str()), Some("7441550"));
    }

    #[test]
    fn score_same_name_diff_price_ambiguous() {
        let items = [
            item("1111111", "Free Sound Pack", 0),
            item("2222222", "Free Sound Pack", 500),
        ];
        let names = [
            ("1111111", "Free Sound Pack"),
            ("2222222", "Free Sound Pack"),
        ];
        let (best, amb) = score_and_pick("FreeSoundPack", &items, false, cn(&names), None);
        assert_eq!(best.map(|i| i.id.as_str()), Some("1111111"));
        assert!(amb);
    }

    #[test]
    fn score_query_empty() {
        let items = [item("1111111", "anything", 0)];
        let names = [("1111111", "anything")];
        let (best, _) = score_and_pick("", &items, false, cn(&names), None);
        assert!(best.is_none());
    }

    #[test]
    fn score_prefer_free_tiebreak() {
        // 付费项在纯次序权重上更靠前，prefer_free 给免费项 +5 使其反超。
        let items = [
            item("2222222", "Star Tiara", 500),
            item("1111111", "Star Tiara", 0),
        ];
        let names = [("1111111", "Star Tiara"), ("2222222", "Star Tiara")];
        // 不偏好免费：idx 次序权重大，付费项（idx0 +10）胜出。
        let (best, _) = score_and_pick("StarTiara", &items, false, cn(&names), None);
        assert_eq!(best.map(|i| i.id.as_str()), Some("2222222"));
        // 偏好免费：免费项 +5 → 免费（idx1 +8+5=13）> 付费（idx0 +10）。
        let (best, _) = score_and_pick("StarTiara", &items, true, cn(&names), None);
        assert_eq!(best.map(|i| i.id.as_str()), Some("1111111"));
    }

    // ── description 加权 ──────────────────────────────────────────

    fn desc(s: &str) -> DescriptionText {
        DescriptionText(s.to_string())
    }

    /// 安全阀：权重关闭时必须与 `score_and_pick` **完全一致**（含歧义标记）。
    /// 这是整个改动不破坏既有行为的前提。
    #[test]
    fn desc_off_is_byte_identical_to_legacy() {
        let items = [
            item("1111111", "Star Tiara", 0),
            item("2222222", "Star Tiara Gold", 0),
        ];
        let names = [("1111111", "Star Tiara"), ("2222222", "Star Tiara Gold")];
        let descs = [desc("何もない"), desc("Star Tiara 対応")];
        for prefer_free in [false, true] {
            let legacy = score_and_pick("StarTiara", &items, prefer_free, cn(&names), None);
            let with = score_and_pick_with_desc(
                "StarTiara",
                &items,
                &descs,
                DescWeight::OFF,
                prefer_free,
                cn(&names),
                None,
            );
            assert_eq!(
                legacy.0.map(|i| i.id.clone()),
                with.0.map(|i| i.id.clone()),
                "关闭时最优项必须一致"
            );
            assert_eq!(legacy.1, with.1, "关闭时歧义标记必须一致");
        }
    }

    /// 默认构造即关闭：不显式开启就不该改变行为。
    #[test]
    fn default_weight_is_off() {
        assert!(!DescWeight::default().is_on());
        assert!(!DescWeight::OFF.is_on());
    }

    /// 开启后，说明文命中更多的候选应被提到前面。
    #[test]
    fn desc_on_promotes_description_match() {
        let items = [
            item("1111111", "Generic Item", 0),
            item("2222222", "Another Item", 0),
        ];
        let names = [("1111111", "Generic Item"), ("2222222", "Another Item")];
        // 只有第 2 项的说明文含目标词。
        let descs = [
            desc("全く関係ない説明"),
            desc("この商品は StarTiara に対応しています"),
        ];
        // 关闭：次序权重让 idx0 胜出。
        let (off, _) = score_and_pick_with_desc(
            "StarTiara",
            &items,
            &descs,
            DescWeight::OFF,
            false,
            cn(&names),
            None,
        );
        assert_eq!(off.map(|i| i.id.as_str()), Some("1111111"));
        // 开启且权重足够大：说明文命中的 idx1 反超。
        let w = DescWeight {
            word_bonus: 50,
            max_words: 3,
        };
        let (on, _) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        assert_eq!(
            on.map(|i| i.id.as_str()),
            Some("2222222"),
            "说明文命中应提升排序"
        );
    }

    /// 上限必须真的生效：长说明堆词不能无限加分。
    #[test]
    fn desc_hits_are_capped() {
        // query 有 3 个词，max_words=1 时最多计 1 个。
        assert_eq!(desc_word_hits("aaa bbb ccc", "aaa bbb ccc", 1), 1);
        assert_eq!(desc_word_hits("aaa bbb ccc", "aaa bbb ccc", 3), 3);
        assert_eq!(desc_word_hits("aaa bbb ccc", "aaa bbb ccc", 0), 0);
    }

    /// 少于 3 字符的词不计（与名称词级匹配同一判据），避免噪声。
    #[test]
    fn desc_short_words_ignored() {
        assert_eq!(desc_word_hits("ab", "ab ab ab", 5), 0);
        assert_eq!(desc_word_hits("abc", "abc", 5), 1);
    }

    /// 空说明文不得加分，也不得 panic。
    #[test]
    fn desc_empty_is_safe() {
        let items = [item("1", "n", 0)];
        let (best, _) = score_and_pick_with_desc(
            "StarTiara",
            &items,
            &[],
            DescWeight {
                word_bonus: 50,
                max_words: 3,
            },
            false,
            cn(&[("1", "n")]),
            None,
        );
        // 单候选走原路径，不加分也不崩。
        assert!(best.is_none() || best.map(|i| i.id.as_str()) == Some("1"));
    }

    /// 加分不得翻转「名称完全不命中」的否决：权重再大也不能凭说明文入选。
    #[test]
    fn desc_cannot_override_name_mismatch() {
        let long_a = "A".repeat(60);
        let items = [item("1", &long_a, 0)];
        let (best, _) = score_and_pick_with_desc(
            "Moonpiercer",
            &items,
            &[desc("Moonpiercer 対応")],
            DescWeight {
                word_bonus: 50,
                max_words: 3,
            },
            false,
            cn(&[("1", long_a.as_str())]),
            None,
        );
        // 单结果与否决由原实现决定，加分不介入。
        assert!(best.is_none(), "名称不命中时说明文不得单独入选");
    }
}
