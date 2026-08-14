//! 评分选优：score_and_pick。
//!
//! 行为逐数字复刻 Python（血泪坑 #8.2.11：评分权重 +100/+20/-10、歧义分差 <30、
//! 同名不同价必报歧义、单结果也须名称命中）。`canonical_name` 与资源名提取以
//! 依赖注入传入，保持本层无 IO 可测。

use crate::norm::norm;

/// 搜索结果条目（调用方从 BOOTH JSON 提取）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub price: i64,
}

/// 评分选最佳。
///
/// `canonical_name`：商品规范名解析器（JSON 名，含英文别名；调用方负责缓存/网络）。
/// `resource_names`：已从 zip 内 .unitypackage 提取的资源名集合（可选，仅单结果兜底用）。
///
/// 返回 `(最佳条目引用, 是否歧义)`。
pub fn score_and_pick<'a, F>(
    query: &str,
    items: &'a [Item],
    _prefer_free: bool,
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

    // 评分（Python 权重逐字复刻）。
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
        if name_l.chars().count() as i64 > query.chars().count() as i64 * 5
            && cn.chars().count() as i64 > query.chars().count() as i64 * 5
        {
            s -= 10;
        }
        scored.push((s, idx));
    }
    // 按分数降序；分数相同时保持原序（Python sorted 稳定）。
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
                && !res_names.is_empty() {
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
        // 血泪坑 #8.2.13：单结果也须名称命中。短名不命中时 idx+10 仍 >0 直接返回；
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
}
