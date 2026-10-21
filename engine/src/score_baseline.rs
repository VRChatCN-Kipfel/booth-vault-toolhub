//! 评分基线：把 `score_and_pick` 的分数计算外化成可观测形态。
//!
//! 为什么要先做这件事：`score_and_pick` 只返回「最优 + 是否歧义」，**不暴露分数**。
//! 在没有 description 加权之前，无法回答「加权后分数变了多少」——只能看到最终选优
//! 结果有没有翻车。这不足以支撑「改动须重新验证」的要求。
//!
//! 本模块只做**观测**，不改判定：`baseline_scores` 的输出在未引入新权重前必须与
//! `score_and_pick` 内部完全一致，否则说明复刻走样了。

use crate::norm::norm;

/// 单条商品的分数明细（可逐项核对，而不是只给一个总数）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreBreakdown {
    pub id: String,
    /// 名称包含查询词（+100）。
    pub name_exact: bool,
    /// 规范名包含查询词（+100）。
    pub canon_exact: bool,
    /// 命中的词级项（每项 +20）。
    pub word_hits: usize,
    /// 次序微权 `max(0, 10 - idx*2)`（+0..+10）。
    pub order_bonus: i32,
    /// 免费加成（+5，仅 prefer_free）。
    pub free_bonus: i32,
    /// 过长惩罚（-10）。
    pub long_penalty: i32,
    /// 词级命中明细（供诊断「为什么这条赢」）。
    pub hit_words: Vec<String>,
    /// 最终总分。
    pub total: i32,
    /// 输入次序。
    pub idx: usize,
}

/// 词级命中：`w.chars().count() >= 3 && name_l.contains(&w)`。
fn word_hits(query: &str, name_l: &str) -> Vec<String> {
    split_query_words(query)
        .into_iter()
        .filter(|w| w.chars().count() >= 3 && name_l.contains(w))
        .collect()
}

/// `re.split(r'[_\-\s]+', query.lower())` 等价（与 `score` 内部同构）。
fn split_query_words(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| c == '_' || c == '-' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// 计算全部商品的分数明细（与 `score_and_pick` 的评分项逐条对齐）。
///
/// `canonical_name` 由调用方注入，理由同 `score_and_pick`：保持本层无 IO。
pub fn baseline_scores<F>(
    query: &str,
    items: &[(String, String, i64)],
    prefer_free: bool,
    canonical_name: F,
) -> Vec<ScoreBreakdown>
where
    F: Fn(&str) -> String,
{
    let qn = norm(query);
    items
        .iter()
        .enumerate()
        .map(|(idx, (id, name, price))| {
            let name_l = name.to_lowercase();
            let cn = canonical_name(id);
            let name_exact = !qn.is_empty() && norm(&name_l).contains(&qn);
            let canon_exact = !qn.is_empty() && norm(&cn).contains(&qn);
            let hits = word_hits(query, &name_l);
            let order_bonus = (10 - (idx as i32) * 2).max(0);
            let free_bonus = if prefer_free && *price == 0 { 5 } else { 0 };
            let long_penalty = if name_l.chars().count() as i64 > query.chars().count() as i64 * 5
                && cn.chars().count() as i64 > query.chars().count() as i64 * 5
            {
                -10
            } else {
                0
            };
            let mut total = 0i32;
            if name_exact {
                total += 100;
            }
            if canon_exact {
                total += 100;
            }
            total += 20 * hits.len() as i32;
            total += order_bonus + free_bonus + long_penalty;
            ScoreBreakdown {
                id: id.clone(),
                name_exact,
                canon_exact,
                word_hits: hits.len(),
                order_bonus,
                free_bonus,
                long_penalty,
                total,
                hit_words: hits,
                idx,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, name: &str, price: i64) -> (String, String, i64) {
        (id.to_string(), name.to_string(), price)
    }

    fn cn<'a>(table: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> String + 'a {
        move |id| {
            table
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, n)| n.to_string())
                .unwrap_or_default()
        }
    }

    /// 最重要的一条：明细总分必须与 `score_and_pick` 的判据一致。
    /// 通过「按总分排序取首位」与它返回的最优项对比来验证。
    #[test]
    fn breakdown_matches_score_and_pick_choice() {
        let cases: &[(&str, Vec<(String, String, i64)>, Vec<(&str, &str)>)] = &[
            (
                "LunariaPaperFan",
                vec![item("7437723", "Lunaria Paper Fan", 0)],
                vec![("7437723", "LunariaPaperFan")],
            ),
            (
                "StarTiara",
                vec![
                    item("1111111", "Star Tiara", 0),
                    item("2222222", "Star Tiara Gold", 0),
                ],
                vec![("1111111", "Star Tiara"), ("2222222", "Star Tiara Gold")],
            ),
            (
                "StarTiara",
                vec![
                    item("2222222", "Star Tiara", 500),
                    item("1111111", "Star Tiara", 0),
                ],
                vec![("1111111", "Star Tiara"), ("2222222", "Star Tiara")],
            ),
        ];
        for (q, items, names) in cases {
            let engine_items: Vec<crate::score::Item> = items
                .iter()
                .map(|(id, n, p)| crate::score::Item {
                    id: id.clone(),
                    name: n.clone(),
                    price: *p,
                })
                .collect();
            let (best, _) = crate::score::score_and_pick(q, &engine_items, false, cn(names), None);
            let bd = baseline_scores(q, items, false, cn(names));
            let mut sorted = bd.clone();
            sorted.sort_by_key(|b| std::cmp::Reverse(b.total));
            assert_eq!(
                sorted[0].idx,
                best.map(|b| {
                    engine_items
                        .iter()
                        .position(|i| i.id == b.id)
                        .expect("best must be in items")
                })
                .unwrap_or(usize::MAX),
                "明细排序首位必须等于 score_and_pick 的选择（query={q}）"
            );
        }
    }

    #[test]
    fn breakdown_reports_components() {
        let items = vec![item("1", "Star Tiara", 0)];
        let bd = baseline_scores("StarTiara", &items, true, cn(&[("1", "Star Tiara")]));
        let b = &bd[0];
        assert!(b.name_exact, "名称包含查询词应 +100");
        assert!(b.canon_exact, "规范名包含查询词应 +100");
        assert_eq!(b.free_bonus, 5, "prefer_free 下免费项 +5");
        assert_eq!(b.order_bonus, 10, "首项次序权 +10");
        assert!(b.total >= 215, "总分应含各项之和，实际 {}", b.total);
    }

    #[test]
    fn word_hits_require_three_chars() {
        // 词级匹配要求 ≥3 字符；「Star」4 字符命中，「Ti」2 字符不命中。
        let items = vec![item("1", "star tiara", 0)];
        let bd = baseline_scores("Star Ti", &items, false, cn(&[]));
        assert_eq!(bd[0].hit_words, vec!["star".to_string()]);
    }

    #[test]
    fn long_name_penalty_applies() {
        let long = "A".repeat(60);
        let items = vec![item("1", &long, 0)];
        let bd = baseline_scores("Moonpiercer", &items, false, cn(&[("1", long.as_str())]));
        assert_eq!(bd[0].long_penalty, -10);
    }

    #[test]
    fn empty_query_yields_no_exact_bonus() {
        let items = vec![item("1", "anything", 0)];
        let bd = baseline_scores("", &items, false, cn(&[]));
        assert!(!bd[0].name_exact);
        assert!(!bd[0].canon_exact);
    }
}
