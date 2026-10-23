//! 评分选优：score_and_pick。
//!
//! 评分权重与歧义判定为**线上行为契约**（+100/+20/-10、歧义分差 <30、
//! 同名不同价必报歧义、单结果也须名称命中），改动会影响选优结果。
//! `canonical_name` 与资源名提取以依赖注入传入，保持本层无 IO 可测。

use crate::norm::norm_wide;

/// 搜索结果条目（调用方从 BOOTH JSON 提取）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub price: i64,
}

/// 诊断用：返回查询词归一化后的串（与 `score_and_pick` 内部完全同一个调用）。
///
/// 存在的理由：`+100`（名称命中）与 `+20`（词级命中）在同一标题上**必然同时出现**
/// ——整串命中必然包含词级命中。因此**无法通过选优结果反推 `qn` 是否为空**：
/// 撤销 `norm_wide`（`qn` 变成空串）后，词级 +20 仍单独成立，选优结果可能不变。
/// 实测：把主判据改回 `norm` 后，232 个测试全绿。要真正守住日文评分，必须
/// 直接断言归一化结果本身。
pub fn normalized_query(query: &str) -> String {
    norm_wide(query)
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
    let qn = normalized_query(query);

    // 评分权重契约（+100 名称命中 / +100 规范名命中 / +20 词级 / +10-次序 / -10 过长）。
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (idx, it) in items.iter().enumerate() {
        let name_l = it.name.to_lowercase();
        let cn = canonical_name(&it.id);
        let mut s = 0i32;
        if !qn.is_empty() && norm_wide(&name_l).contains(&qn) {
            s += 100;
        }
        if !qn.is_empty() && norm_wide(&cn).contains(&qn) {
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
            // 必须与主判据同用 `norm_wide`：混用会让日文标题在主路径命中、
            // 在兜底路径不命中（或反之），同一份数据两种结论。
            let cn = norm_wide(&canonical_name(&it.id));
            if !qn.is_empty() && (cn.contains(&qn) || norm_wide(&it.name).contains(&qn)) {
                return (Some(it), false);
            } // 标题/规范名都不命中 → 资源名二次校验。
            if let Some(res_names) = resource_names
                && !res_names.is_empty()
            {
                for r in res_names {
                    let rn = norm_wide(r);
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
    // 同理：同名判定必须与主判据同一归一化，否则日文场景下 `"" == ""` 会把
    // 不同商品误判成同名。
    let best_cn = norm_wide(&canonical_name(&items[best_idx].id));
    let best_price = items[best_idx].price;
    for (_, idx2) in &scored[1..] {
        let it2 = &items[*idx2];
        if norm_wide(&canonical_name(&it2.id)) == best_cn && it2.price != best_price {
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

    // ── 日文名称命中（跨 score_and_pick 的集成防线）──────────────────
    //
    // 背景：`norm` 只保留 ASCII，任何 CJK 查询被归一成空串，`!qn.is_empty()`
    // 前置检查因此短路掉全部名称命中判定。修复引入 `norm_wide`（保留 CJK）。
    //
    // **这组测试存在的唯一目的**：实测把 `norm_wide(query)` 改回 `norm(query)`
    // （即撤销整个修复）时，此前 227 个测试**全绿、一条都不红**。没有这些用例，
    // 修复处于无保护状态，任何人改回去 CI 都不会响。

    /// **唯一能抓住「撤销 norm_wide」的测试**。
    ///
    /// 为什么不用选优结果断言：`+100` 与 `+20` 在同一标题上必然同时出现（整串
    /// 命中必然含词级命中），撤销 `norm_wide` 后 `+20` 仍单独成立，选优结果可能
    /// 完全不变——实测把主判据改回 `norm`，全量 232 个测试仍全绿。
    ///
    /// 故直接断言归一化结果本身：`query` 过了这一层，后面的 `!qn.is_empty()`
    /// 才不会短路掉 `+100`。撤销修复时本测试立即变红。
    #[test]
    fn japanese_query_survives_normalization() {
        assert_eq!(
            normalized_query("信濃セット"),
            "信濃セット",
            "日文查询不得被归一成空串（否则 +100 档全部短路）"
        );
        assert_eq!(normalized_query("メカ弾エフェクト"), "メカ弾エフェクト");
        assert_eq!(normalized_query("ねこみみ"), "ねこみみ");
        // 对照：既有 `norm` 对这些确实是空串，证明这不是同义反复。
        assert_eq!(crate::norm::norm("信濃セット"), "");
        // 标注：ASCII 不受影响。
        assert_eq!(normalized_query("Star-Tiara_v1.0"), "startiarav10");
    }

    /// 日文查询应能命中日文标题。
    ///
    /// 注意本用例**不能**用来证明 `norm_wide` 生效（见上一条的说明），
    /// 它只验证端到端行为正确——名称命中的候选能压过首位次序分。
    #[test]
    fn japanese_query_hits_japanese_title() {
        let items = [
            item("1111111", "メカ弾エフェクト", 0),
            item("2222222", "信濃セット", 0),
        ];
        let names = [("1111111", "メカ弾エフェクト"), ("2222222", "信濃セット")];
        let (best, _) = score_and_pick("信濃セット", &items, false, cn(&names), None);
        assert_eq!(
            best.map(|i| i.id.as_str()),
            Some("2222222"),
            "日文名称命中必须压过首位次序分；若选到 1111111 说明 query 被归一成空串"
        );
    }

    /// 假名查询命中汉字标题（`norm_wide` 保留双方字符，故 contains 成立）。
    #[test]
    fn kana_query_hits_kanji_title() {
        // 「シナノ」是「信濃」的读音，但字符串不同——此用例只验证 norm_wide
        // 不会把任一方滤成空串而导致 +100 全灭；真正的字形互认是 kana 的职责。
        let items = [item("1111111", "信濃セット 雪女", 0)];
        let names = [("1111111", "信濃セット 雪女")];
        let (best, _) = score_and_pick("信濃セット 雪女", &items, false, cn(&names), None);
        assert!(best.is_some(), "含假名与汉字的标题必须能被本身命中");
    }

    /// 反向：日文查询**不得**因为归一化而命中完全无关的标题。
    ///
    /// 注意本用例的真实结论：**它命中了**，但根因不是 `norm_wide`，而是
    /// `score.rs` 的既有缺陷——次序分 `(10 - idx*2)` 让任何非空结果集的首条恒得
    /// 10 分，`scored[0].0 <= 0` 的否决分支因此近乎不可达，于是「单结果 + 名称
    /// 不命中」会被放过。这是 AGENTS「单结果也须名称命中」契约的实际缺口。
    ///
    /// 本测试**记录现状**并把这个缺口钉成已知行为，避免它被误认为是 norm_wide
    /// 的回归；真正的修法是「先判名称命中、再加次序分」，属独立改动。
    #[test]
    fn single_unrelated_result_is_known_gap() {
        let items = [item("1111111", "メカ弾エフェクト", 0)];
        let names = [("1111111", "メカ弾エフェクト")];
        let (best, _) = score_and_pick("信濃セット", &items, false, cn(&names), None);
        // 名称与规范名都不含查询词，却因次序分 > 0 被放过。
        assert_eq!(
            best.map(|i| i.id.as_str()),
            Some("1111111"),
            "已知缺口：次序分让单结果绕过名称命中检查（若此处变为 None 说明已修好，请更新本测试）"
        );
    }

    /// 反过来钉住：`norm_wide` 本身不得让不相关的**中文/ASCII** 标题命中。
    ///
    /// 这条与上一条互补：上一条是既有次序分缺口，这一条确认归一化没有额外放大。
    #[test]
    fn norm_wide_does_not_fabricate_match() {
        let q = crate::norm::norm_wide("信濃セット");
        assert_eq!(q, "信濃セット");
        for title in ["メカ弾エフェクト", "Lunaria Paper Fan", "雪女"] {
            assert!(
                !crate::norm::norm_wide(title).contains(&q),
                "归一化不得让 {title} 含查询词"
            );
        }
    }

    /// 长名兜底分支也必须用 `norm_wide`（与主判据同一归一化）。
    ///
    /// 修复前该分支用 `norm`，日文标题在这里会被归一成空串而判定"命中"，
    /// 与主路径结论相反。
    #[test]
    fn single_result_fallback_uses_same_normalization() {
        let long = format!("{}信濃セット", "A".repeat(50));
        let items = [item("1111111", &long, 0)];
        let names = [("1111111", long.as_str())];
        // 查询与长标题前段不同、但整串里含「信濃セット」→ 名称命中应成立。
        let (best, _) = score_and_pick("信濃セット", &items, false, cn(&names), None);
        assert_eq!(
            best.map(|i| i.id.as_str()),
            Some("1111111"),
            "兜底分支必须与主判据同用 norm_wide，否则同一数据两种结论"
        );
    }
}
