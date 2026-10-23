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

/// 一条商品说明文，**与商品 id 绑定**。
///
/// 为什么必须带 id：历史实现是 `DescriptionText(pub String)` 且按**位置**取
/// （`descriptions.get(idx)`）。调用方只要对 items 做过一次过滤或重排
/// （`filter(...).collect()` 之类），加成就会**静默落到另一个商品上**——
/// 分数看起来完全正常，没有任何报错。id 绑定后按商品查，位置不再参与。
///
/// 与 `Item` 分开而不是直接塞进去：`Item` 是既有三端契约的公开结构，
/// 加字段会强制所有构造点改动；且多数路径（搜索页）本来就没有说明文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionText {
    pub id: String,
    pub text: String,
}

impl DescriptionText {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

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
    word_bonus: i32,
    /// 最多计多少个命中词（防长说明文靠堆词刷分）。
    max_words: usize,
}

/// 构造 [`DescWeight`] 时的校验错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescWeightError {
    /// `word_bonus` 必须 > 0（负值会让排序反向，0 表示关闭）。
    NonPositiveBonus,
    /// `max_words` 必须在 1..=100。
    BadMaxWords,
    /// 单项加成 × 词数上限不得超过 100（等于名称命中档）。
    TooStrong,
}

impl std::fmt::Display for DescWeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DescWeightError::NonPositiveBonus => write!(f, "word_bonus 必须为正整数"),
            DescWeightError::BadMaxWords => write!(f, "max_words 必须在 1..=100"),
            DescWeightError::TooStrong => write!(
                f,
                "word_bonus × max_words 不得超过 100（等于名称命中档），否则说明文能压过标题"
            ),
        }
    }
}

impl Default for DescWeight {
    fn default() -> Self {
        // 默认关闭，且与 `OFF` **完全等价**。历史实现是 Default{0,3} ≠ OFF{0,0}：
        // 两个「关闭」状态不一样，调用方容易误判成「开启了但没效果」。
        Self::OFF
    }
}

impl DescWeight {
    /// 关闭状态（等价于历史行为）。
    pub const OFF: Self = Self {
        word_bonus: 0,
        max_words: 0,
    };

    /// 构造一个**参与评分**的权重。参数越界一律返回错误，不做静默修正。
    ///
    /// 为什么必须校验（字段此前是 `pub`，这些都能传进去）：
    ///   - `word_bonus = -50` → 排序**静默反向**；
    ///   - `max_words = 0` → `is_on()` 静默为 false，看着像开了其实没效果；
    ///   - `word_bonus = 5000` → 实测可传，远超名称命中档 +100，说明文因此
    ///     能压过标题——正是评审担心的「desc 越过名称证据」。
    pub fn new(word_bonus: i32, max_words: usize) -> Result<Self, DescWeightError> {
        if word_bonus <= 0 {
            return Err(DescWeightError::NonPositiveBonus);
        }
        if max_words == 0 || max_words > 100 {
            return Err(DescWeightError::BadMaxWords);
        }
        // 用 i64 先算上限，避免 i32 乘法溢出绕过校验。
        if (word_bonus as i64) * (max_words as i64) > 100 {
            return Err(DescWeightError::TooStrong);
        }
        Ok(Self {
            word_bonus,
            max_words,
        })
    }

    /// 单词加分数（供测试与诊断读取；字段本身不公开以免绕过校验）。
    pub fn word_bonus(&self) -> i32 {
        self.word_bonus
    }

    /// 词数上限。
    pub fn max_words(&self) -> usize {
        self.max_words
    }

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
    F: Fn(&str) -> Option<String>,
{
    // 未开启：直接走原实现，保证一分不差。
    if !weight.is_on() {
        return score_and_pick(query, items, prefer_free, canonical_name, resource_names);
    }
    // **共享缓存**：整个函数（含对 `score_and_pick` 的那次调用）只用这一份。
    // 若让 `score_and_pick` 自建缓存、下面那段再建一份，同一 id 会被解析两次
    // ——实测 ON 路径 6 次而非 3 次，等于白做。
    let memo: CanonMemo = std::cell::RefCell::new(std::collections::HashMap::new());
    let (picked, amb) = score_and_pick_memo(
        query,
        items,
        prefer_free,
        &canonical_name,
        &memo,
        resource_names,
    );
    // 单候选或名称层已否决时不介入：
    //   - 单候选不存在排序，加分只影响多候选先后；
    //   - `picked == None` 表示名称零证据，说明文**不得**把它救回来（它可能只是
    //     列表、免责声明，或「不适用于 XX」）。
    //
    // **歧义时仍要介入排序**：desc 允许改变「谁排第一」，但返回的歧义标记必须保持
    // 为真——否则 GUI 会自动采用 picked，把本该人工确认的两件定成一件。
    if picked.is_none() || items.len() <= 1 {
        return (picked, amb);
    }
    // 说明文按 **id** 建索引：位置在调用方过滤/重排后会错配，重复 id 首条胜出。
    let desc_by_id: std::collections::HashMap<&str, &str> = {
        let mut m: std::collections::HashMap<&str, &str> =
            std::collections::HashMap::with_capacity(descriptions.len());
        for d in descriptions {
            m.entry(d.id.as_str()).or_insert(d.text.as_str());
        }
        m
    };
    // 到这里：原实现的歧义与否决已定调。下面只可能在「都已有名称证据的候选」之间
    // 重排，且最终返回的 `ambiguous` 是 `原歧义 || 新歧义`——desc 只能增加歧义，
    // 永远不能消除它。
    //
    // **复用上面那份缓存**（`memo`），不新建：`score_and_pick_memo` 已把每个 id
    // 解析过一次，这里再 resol 会翻倍。
    let canonical = |id: &str| -> Option<String> {
        if let Some(hit) = memo.borrow().get(id) {
            return hit.clone();
        }
        let v = canonical_name(id);
        memo.borrow_mut().insert(id.to_string(), v.clone());
        v
    };
    // 基分走与 `score_and_pick` 完全相同的唯一实现，杜绝两处字面量漂移。
    let base_scores = weighted_scores(query, items, prefer_free, canonical);
    let mut scored: Vec<(i32, usize)> = Vec::with_capacity(items.len());
    let mut base: Vec<i32> = Vec::with_capacity(items.len());
    for (idx, bs) in base_scores.iter().enumerate() {
        base.push(bs.total);
        // 说明文加分只给**已有名称证据**的候选：名称零证据者即便说明文里堆满
        // 关键词也不加分，否则说明文会越过名称层的否决（说明可能只是列表、
        // 免责声明，或「不适用于 XX」）。权重再大也不能突破这条。
        //
        // 判据必须用 `name_hit || canon_hit`，**不能**用 `s > 0` 代替：次序分
        // `(10 - idx*2)` 让任何非首位候选都可能有正分，于是「零证据」会被误判成
        // 「有证据」，说明文就能把无关商品顶上去（实测权重 500 时确实会反超）。
        let has_name_evidence = bs.name_hit || bs.canon_hit;
        let bonus = if has_name_evidence {
            // **按商品 id 查**，不按位置：位置在调用方过滤/重排后会错配。
            // 重复 id 首条胜出，未知 id 直接忽略（不加分也不报错）。
            desc_by_id
                .get(items[idx].id.as_str())
                .map(|d| weight.word_bonus * desc_word_hits(query, d, weight.max_words) as i32)
                .unwrap_or(0)
        } else {
            0
        };
        scored.push((bs.total + bonus, idx));
    }
    // 歧义判定用**不含 desc 的基线分**：说明文不得把两个本已分不开的候选拉开距离，
    // 也不得反过来制造新的歧义。阈值语义与原实现完全一致。
    let mut base_sorted: Vec<(i32, usize)> = base.iter().copied().zip(0..).collect();
    base_sorted.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    let base_ambiguous = base_sorted.len() > 1 && (base_sorted[0].0 - base_sorted[1].0) < 30;

    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    let best_idx = scored[0].1;
    // 同名不同价判定：**两侧都必须取到规范名**才比较。取不到就压成空串的话，
    // 任意两个都取不到规范名的不同商品会被判成同名 → 假歧义。
    // 归一化必须与主判据同用 `norm_wide`，否则日文场景下同样会误判。
    let best_cn = canonical(&items[best_idx].id).map(|c| norm_wide(&c));
    let best_price = items[best_idx].price;
    // 歧义 = 原实现已判歧义 **或** 新增的两条判据。desc 永远只能让歧义增加。
    let mut ambiguous = amb || base_ambiguous;
    for (_, idx2) in &scored[1..] {
        let it2 = &items[*idx2];
        if let Some(bc) = best_cn.as_deref()
            && let Some(other) = canonical(&it2.id).map(|c| norm_wide(&c))
            && other == bc
            && it2.price != best_price
        {
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

/// 单条候选的评分明细（**不含** description 加成）。
///
/// 公开的目的：评分权重是 AGENTS.md 列明的线上行为契约，但 `score_and_pick` 只返回
/// 「选了谁」，无法断言分值——实测把 `+100` 改成 `+80`、`+20` 改成 `+5` 全量测试仍全绿。
/// 暴露明细后，golden 测试可以直接钉住每个权重常量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemScore {
    /// 总分（名称/规范名/词级/次序/免费/过长 之和）。
    pub total: i32,
    /// 商品名命中查询词（+100 那一档）。
    pub name_hit: bool,
    /// 规范名命中查询词（+100 那一档）。
    pub canon_hit: bool,
}

/// 逐项评分（唯一实现，`score_and_pick` 与 `score_and_pick_with_desc` 共用）。
///
/// 抽成单一函数的理由：此前 `score_and_pick` 与 `score_and_pick_with_desc` 各写一遍
/// 同一套字面量（`100/20/10-2·idx/5/-10`），改一处漏一处不会有任何测试报警。
pub fn weighted_scores<F>(
    query: &str,
    items: &[Item],
    prefer_free: bool,
    canonical_name: F,
) -> Vec<ItemScore>
where
    F: Fn(&str) -> Option<String>,
{
    let qn = normalized_query(query);
    items
        .iter()
        .enumerate()
        .map(|(idx, it)| {
            let name_l = it.name.to_lowercase();
            // 取不到规范名（None）时不参与 +100 与长度判据。历史实现把失败压成空串，
            // 于是「规范名命中」静默失效，同名判定还退化成 `"" == ""`（不同商品判成同名）。
            let cn = canonical_name(&it.id);
            let name_hit = !qn.is_empty() && norm_wide(&name_l).contains(&qn);
            let canon_hit = cn
                .as_deref()
                .is_some_and(|c| !qn.is_empty() && norm_wide(c).contains(&qn));
            let mut s = 0i32;
            if name_hit {
                s += 100;
            }
            if canon_hit {
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
            // 过长惩罚要求「名称与规范名**都**过长」；规范名取不到时只按名称判，
            // 不把 None 当成「长度为 0」而误判为「不惩罚」。
            let name_too_long = name_l.chars().count() as i64 > query.chars().count() as i64 * 5;
            let canon_too_long = match cn.as_deref() {
                Some(c) => c.chars().count() as i64 > query.chars().count() as i64 * 5,
                None => true,
            };
            if name_too_long && canon_too_long {
                s -= 10;
            }
            ItemScore {
                total: s,
                name_hit,
                canon_hit,
            }
        })
        .collect()
}

/// 评分选最佳。
///
/// `prefer_free`：同分候选时优先免费项（付费/免费同款场景；付费项不因此被排除）。
/// `canonical_name`：商品规范名解析器；**取不到时返回 `None`**。
/// `resource_names`：已从 zip 内 .unitypackage 提取的资源名集合（可选，仅单结果兜底用）。
///
/// **为什么返回 `Option<String>` 而非 `String`**：历史实现把网络/解析失败
/// `unwrap_or_default()` 成空串，导致「规范名命中 +100」静默失效，而 `:108` 的
/// 「同名不同价」判定退化成 `"" == ""`——两个**不同**商品会被判成同名并报歧义。
/// 改成 `Option` 后：取不到就不加分、同名判定要求两侧都 `Some`。
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
    F: Fn(&str) -> Option<String>,
{
    let memo: CanonMemo = std::cell::RefCell::new(std::collections::HashMap::new());
    score_and_pick_memo(
        query,
        items,
        prefer_free,
        &canonical_name,
        &memo,
        resource_names,
    )
}

/// 规范名缓存（对同一 id 只解析一次）。
///
/// `canonical_name` 由调用方注入，生产实现是阻塞网络 GET（`fetch_item`）。
/// 评分本身对同一 id 至少调用 2 次（加权分 + 歧义同名比较），而
/// `score_and_pick_with_desc` 还要再跑一遍完整流程——实测 N 个候选
/// legacy 6 次、ON 12 次（4N）。缓存把「每个 id 一次」变成硬保证。
type CanonMemo = std::cell::RefCell<std::collections::HashMap<String, Option<String>>>;

/// 带外部缓存的评分实现。
///
/// **必须能把缓存传进来**：否则 `score_and_pick_with_desc` 内部调
/// `score_and_pick` 时会建一份新缓存，而它自己那段又建另一份——两份互不共享，
/// 同一 id 仍被解析两次（实测 ON 路径 6 次而非 3 次）。
fn score_and_pick_memo<'a, F>(
    query: &str,
    items: &'a [Item],
    prefer_free: bool,
    canonical_name: &F,
    memo: &CanonMemo,
    resource_names: Option<&[String]>,
) -> (Option<&'a Item>, bool)
where
    F: Fn(&str) -> Option<String>,
{
    if items.is_empty() {
        return (None, false);
    }
    let canonical = |id: &str| -> Option<String> {
        if let Some(hit) = memo.borrow().get(id) {
            return hit.clone();
        }
        let v = canonical_name(id);
        memo.borrow_mut().insert(id.to_string(), v.clone());
        v
    };
    let qn = normalized_query(query);

    // 评分权重契约（+100 名称命中 / +100 规范名命中 / +20 词级 / +10-次序 / +5 免费 / -10 过长）。
    // 唯一实现在 `weighted_scores`，本函数只负责排序与判定。
    let mut scored: Vec<(i32, usize)> = weighted_scores(query, items, prefer_free, canonical)
        .into_iter()
        .enumerate()
        .map(|(idx, sc)| (sc.total, idx))
        .collect();
    // 按分数降序；同分保持输入顺序（稳定排序，`sort_by_key` 稳定）。
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));

    if scored.is_empty() || scored[0].0 <= 0 {
        // 无任何名称匹配：唯一结果也须名称命中。
        if items.len() == 1 {
            let it = &items[0];
            // 必须与主判据同用 `norm_wide`：混用会让日文标题在主路径命中、
            // 在兜底路径不命中（或反之），同一份数据两种结论。
            // 规范名取不到（None）时只比对标题，不把 None 当成空串。
            let cn_hit =
                canonical(&it.id).is_some_and(|c| !qn.is_empty() && norm_wide(&c).contains(&qn));
            if cn_hit || (!qn.is_empty() && norm_wide(&it.name).contains(&qn)) {
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
    // 不同商品误判成同名。**两侧都必须取到规范名**才比较——取不到就压成空串的话，
    // 任意两个都取不到规范名的不同商品会被判成同名 → 假歧义。
    let best_cn = canonical(&items[best_idx].id).map(|c| norm_wide(&c));
    let best_price = items[best_idx].price;
    for (_, idx2) in &scored[1..] {
        let it2 = &items[*idx2];
        if let Some(bc) = best_cn.as_deref()
            && let Some(other) = canonical(&it2.id).map(|c| norm_wide(&c))
            && other == bc
            && it2.price != best_price
        {
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
    /// 测试用规范名解析器：查表命中返回 `Some`，未命中返回 `None`
    /// （与生产实现语义一致——取不到就是 None，不是空串）。
    fn cn<'a>(table: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |id| {
            table
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, n)| n.to_string())
        }
    }

    #[test]
    fn score_empty() {
        let (best, amb) = score_and_pick("q", &[], false, |_| None, None);
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

    // ── 规范名取不到（None）的语义 ───────────────────────────────

    /// 规范名取不到时**不得**与其他取不到的候选判成同名。
    ///
    /// 历史实现用 `unwrap_or_default()` 把失败压成空串，于是 `norm("") == norm("")`
    /// 恒成立——两个**完全不同**的商品被判成同名，再配上价格不同就报**假歧义**。
    ///
    /// 用例说明：必须让两候选的**基线分差 >30**，否则会命中「分差 <30 判歧义」
    /// 那条判据，测不出同名判定的问题。故 idx1 的标题**不含查询词**（无 +100），
    /// 只有 idx0 命中。
    #[test]
    fn missing_canonical_names_are_not_same_name() {
        let items = [
            item("1111111", "StarTiara A", 0),
            item("2222222", "完全无关标题", 500),
        ];
        // 空表 ⇒ 规范名全是 None。
        let names: [(&str, &str); 0] = [];
        let (best, amb) = score_and_pick("StarTiara", &items, false, cn(&names), None);
        assert_eq!(best.map(|i| i.id.as_str()), Some("1111111"));
        assert!(
            !amb,
            "分差 >30 且规范名都取不到时不得报歧义（历史实现会因 \"\"==\"\" 触发同名不同价）"
        );
    }

    /// 规范名取不到时不得凭空加分，也不得让无关标题命中。
    ///
    /// 注意：单结果 + 名称不命中时**会**被次序分放过（`score.rs` 的既有缺口，
    /// `scored[0].0 <= 0` 因次序分恒 >0 而近乎不可达）。故本用例用**双候选**：
    /// idx0 名称命中、idx1 不命中且规范名也取不到，验证 idx1 拿不到 +100。
    #[test]
    fn missing_canonical_name_adds_no_bonus() {
        let names: [(&str, &str); 0] = [];
        let items = [
            item("1111111", "StarTiara A", 0),  // 名称命中
            item("2222222", "完全无关标题", 0), // 名称不命中、规范名 None
        ];
        let (best, _) = score_and_pick("StarTiara", &items, false, cn(&names), None);
        assert_eq!(
            best.map(|i| i.id.as_str()),
            Some("1111111"),
            "规范名取不到时 idx1 不得拿到 +100 加分而反超"
        );
    }

    /// 一侧有规范名、另一侧没有时不得判成同名。
    #[test]
    fn one_sided_canonical_name_is_not_same_name() {
        let items = [
            item("1111111", "StarTiara A", 0),
            item("2222222", "StarTiara B", 500),
        ];
        let names = [("1111111", "StarTiara A")];
        let (_, amb) = score_and_pick("StarTiara", &items, false, cn(&names), None);
        assert!(!amb, "一侧取不到规范名时不得判成同名");
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

    // ── description 加权 ──────────────────────────────────────────

    /// 把「按位置排列的说明文」转成「按 id 绑定」的列表（`items[i]` 对应 `texts[i]`）。
    fn descs_for(items: &[Item], texts: &[&str]) -> Vec<DescriptionText> {
        items
            .iter()
            .zip(texts.iter())
            .map(|(it, t)| DescriptionText::new(it.id.clone(), *t))
            .collect()
    }

    /// 安全阀：权重关闭时必须与 `score_and_pick` **完全一致**（含歧义标记）。
    /// 这是整个改动不破坏既有行为的前提。
    /// **P0-4 回归**：说明文必须按 id 对齐，调用方重排后加成不得错配。
    ///
    /// 历史实现用 `DescriptionText(pub String)` 并按位置取（`descriptions.get(idx)`）：
    /// 调用方只要对 items 做过一次 `filter`/重排，加成就会**静默落到另一个商品上**
    /// —— 分数看起来完全正常，没有任何报错。本测试把「按 id」钉死。
    #[test]
    fn description_alignment_by_id() {
        let items = [
            item("1111111", "StarTiara Base", 0),
            item("2222222", "StarTiara Variant", 0),
        ];
        let names = [
            ("1111111", "StarTiara Base"),
            ("2222222", "StarTiara Variant"),
        ];
        let w = DescWeight::new(50, 2).expect("50×2 在上限内");

        // 只有 2222222 的说明文命中查询词。
        let descs_hit_second = vec![
            DescriptionText::new("1111111", "nothing"),
            DescriptionText::new("2222222", "StarTiara 対応"),
        ];
        let a = score_and_pick_with_desc(
            "StarTiara",
            &items,
            &descs_hit_second,
            w,
            false,
            cn(&names),
            None,
        );

        // **把描述顺序颠倒**：若实现按位置取，加成会落到 1111111 上。
        let descs_reordered = vec![
            DescriptionText::new("2222222", "StarTiara 対応"),
            DescriptionText::new("1111111", "nothing"),
        ];
        let b = score_and_pick_with_desc(
            "StarTiara",
            &items,
            &descs_reordered,
            w,
            false,
            cn(&names),
            None,
        );

        assert_eq!(
            a.0.map(|i| i.id.clone()),
            b.0.map(|i| i.id.clone()),
            "描述顺序变化不得改变选优结果（按 id 而非位置对齐）"
        );
        assert_eq!(
            a.0.map(|i| i.id.as_str()),
            Some("2222222"),
            "命中说明文的那个候选应胜出"
        );
    }

    /// 未知 id 的说明文必须被忽略（不加分、不报错）。
    #[test]
    fn unknown_description_is_ignored() {
        let items = [
            item("1111111", "StarTiara Base", 0),
            item("2222222", "StarTiara Variant", 0),
        ];
        let names = [
            ("1111111", "StarTiara Base"),
            ("2222222", "StarTiara Variant"),
        ];
        let w = DescWeight::new(50, 2).expect("合法权重");
        let descs = vec![DescriptionText::new("9999999", "StarTiara StarTiara")];
        let (got, _) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        assert_eq!(
            got.map(|i| i.id.as_str()),
            Some("1111111"),
            "未知 id 不得给任何候选加分，应回落到次序分最高的首位"
        );
    }

    /// 重复 id 时首条胜出（确定性，不随实现选择变化）。
    #[test]
    fn duplicate_description_first_wins() {
        let items = [
            item("1", "StarTiara Base", 0),
            item("2", "StarTiara Var", 0),
        ];
        let names = [("1", "StarTiara Base"), ("2", "StarTiara Var")];
        let w = DescWeight::new(50, 2).expect("合法权重");
        let descs = vec![
            DescriptionText::new("2", "StarTiara 対応"),
            DescriptionText::new("2", "nothing"),
        ];
        let (got, _) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        assert_eq!(
            got.map(|i| i.id.as_str()),
            Some("2"),
            "重复 id 应以首条为准（第二条 nothing 不得覆盖）"
        );
    }

    /// **P0-5 回归**：`canonical_name` 对每个 id 只应解析一次。
    ///
    /// 生产实现是阻塞网络 GET（`fetch_item`）。历史实现里 `score_and_pick` 对同一
    /// id 至少调用 2 次（评分 + 歧义同名比较），而 `score_and_pick_with_desc` 又把
    /// 整套流程跑一遍 —— 实测 N 个候选 legacy 6 次、ON 12 次（4N）。
    ///
    /// 本测试用 `Cell` 计数，断言「调用次数 == 候选数」。
    #[test]
    fn canonical_name_is_resolved_once_per_item() {
        use std::cell::Cell;

        let items = [
            item("1111111", "Star Tiara", 0),
            item("2222222", "Star Tiara Gold", 0),
            item("3333333", "Star Tiara Deluxe", 0),
        ];
        let names = [
            ("1111111", "Star Tiara"),
            ("2222222", "Star Tiara Gold"),
            ("3333333", "Star Tiara Deluxe"),
        ];

        // legacy 路径：3 个候选 → 最多 3 次解析。
        let calls = Cell::new(0usize);
        let counting = |id: &str| -> Option<String> {
            calls.set(calls.get() + 1);
            names
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, n)| n.to_string())
        };
        let _ = score_and_pick("StarTiara", &items, false, counting, None);
        assert_eq!(
            calls.get(),
            items.len(),
            "legacy 路径：每个候选只应解析一次，实际 {}",
            calls.get()
        );

        // ON 路径：同样最多 3 次（不是 2N 或 4N）。
        let calls2 = Cell::new(0usize);
        let counting2 = |id: &str| -> Option<String> {
            calls2.set(calls2.get() + 1);
            names
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, n)| n.to_string())
        };
        let w = DescWeight::new(50, 2).expect("合法权重");
        let descs = descs_for(&items, &["nothing", "StarTiara 対応", "nothing"]);
        let _ = score_and_pick_with_desc("StarTiara", &items, &descs, w, false, counting2, None);
        assert_eq!(
            calls2.get(),
            items.len(),
            "ON 路径：每个候选只应解析一次（不是 2N/4N），实际 {}",
            calls2.get()
        );
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
        let descs = descs_for(&items, &["何もない", "Star Tiara 対応"]);
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

    /// `Default` 必须与 `OFF` **完全相等**：历史实现 Default{0,3} ≠ OFF{0,0}，
    /// 两个「关闭」状态不一致，调用方容易误判成「开启了但没效果」。
    #[test]
    fn default_equals_off() {
        assert_eq!(DescWeight::default(), DescWeight::OFF);
        assert_eq!(DescWeight::default().max_words(), 0);
        assert_eq!(DescWeight::default().word_bonus(), 0);
    }

    /// 构造器必须拒绝越界参数——不做静默修正。
    #[test]
    fn new_rejects_invalid() {
        // 负值会让排序静默反向。
        assert_eq!(
            DescWeight::new(-50, 3),
            Err(DescWeightError::NonPositiveBonus)
        );
        // 0 会让 is_on() 静默为 false。
        assert_eq!(
            DescWeight::new(0, 3),
            Err(DescWeightError::NonPositiveBonus)
        );
        // max_words = 0 同样静默失效。
        assert_eq!(DescWeight::new(10, 0), Err(DescWeightError::BadMaxWords));
        // 上限 100。
        assert_eq!(DescWeight::new(10, 101), Err(DescWeightError::BadMaxWords));
        // 加成强度不得超过名称命中档（+100）：10×11=110 被拒。
        assert_eq!(DescWeight::new(10, 11), Err(DescWeightError::TooStrong));
        // 正常值通过。
        let w = DescWeight::new(50, 2).expect("50×2=100 恰好在上限内");
        assert!(w.is_on());
        assert_eq!(w.word_bonus(), 50);
        assert_eq!(w.max_words(), 2);
    }

    /// 溢出防御：极大值不得因 i32 乘法回绕而绕过上限检查。
    #[test]
    fn new_rejects_overflow_without_wrapping() {
        assert_eq!(
            DescWeight::new(i32::MAX, 2),
            Err(DescWeightError::TooStrong),
            "i32::MAX × 2 用 i64 计算应判超限，不能回绕成负数而放行"
        );
    }

    /// **评审最担心的场景**：极大的 word_bonus 曾经可以传进去（实测 5000 可传），
    /// 让说明文压过标题。现在构造阶段就拦住。
    #[test]
    fn extreme_weight_is_unconstructible() {
        assert!(
            DescWeight::new(5000, 3).is_err(),
            "5000×3 远超 +100，必须被拒"
        );
        assert!(DescWeight::new(1000, 1).is_err());
    }

    /// 开启后，说明文命中更多的候选应被提到前面。
    #[test]
    fn desc_on_promotes_description_match() {
        // 说明文加成**只在已有名称证据的候选之间**重新排序。
        // 两候选名称都含查询词（都有 +100 证据），仅第 2 项说明文也命中。
        // 注意：这两件名称高度相似，原实现会判歧义——desc 可以改「谁排第一」，
        // 但返回的歧义标记必须保持为真（由 desc_cannot_clear_ambiguity 钉住）。
        let items = [
            item("1111111", "StarTiara Base", 0),
            item("2222222", "StarTiara Variant", 0),
        ];
        let names = [
            ("1111111", "StarTiara Base"),
            ("2222222", "StarTiara Variant"),
        ];
        let descs = descs_for(
            &items,
            &["全く関係ない説明", "この商品は StarTiara に対応しています"],
        );
        let w = DescWeight {
            word_bonus: 50,
            max_words: 3,
        };
        let (on, _) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        assert_eq!(
            on.map(|i| i.id.as_str()),
            Some("2222222"),
            "两个候选都有名称证据时，说明文命中应提升排序"
        );
    }

    /// 说明文**不得**让名称零证据的候选反超名称有证据的候选。
    ///
    /// 这是原实现最危险的漏洞：`desc_on_promotes_description_match` 曾经把
    /// 「名称都不命中、靠说明文入选」当成正向验收锁进测试。现改为反向断言。
    #[test]
    fn desc_cannot_promote_candidate_without_name_evidence() {
        let items = [
            item("1111111", "StarTiara Base", 0),
            item("2222222", "Another Item", 0),
        ];
        let names = [("1111111", "StarTiara Base"), ("2222222", "Another Item")];
        let descs = descs_for(
            &items,
            &["全く関係ない説明", "StarTiara StarTiara StarTiara 対応"],
        );
        let w = DescWeight {
            word_bonus: 50,
            max_words: 3,
        };
        let (on, _) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        assert_eq!(
            on.map(|i| i.id.as_str()),
            Some("1111111"),
            "名称有证据者必须胜出，说明文不得让无证据者反超"
        );
    }

    /// 歧义只能用「名称层基线分」判定：说明文不得把本已分不开的候选拉开。
    #[test]
    fn desc_cannot_clear_ambiguity() {
        // 两候选名称同分（Star Tiara / Star Tiara Gold 的规范名不同但都含查询词），
        // 基线分差 <30 → 原实现判歧义；desc 拉大分差后仍必须保持歧义。
        let items = [
            item("1111111", "Star Tiara", 0),
            item("2222222", "Star Tiara Gold", 0),
        ];
        let names = [("1111111", "Star Tiara"), ("2222222", "Star Tiara Gold")];
        let descs = descs_for(
            &items,
            &["nothing", "StarTiara StarTiara StarTiara StarTiara"],
        );
        let w = DescWeight {
            word_bonus: 50,
            max_words: 3,
        };
        let (_, on_amb) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        let (_, off_amb) = score_and_pick_with_desc(
            "StarTiara",
            &items,
            &descs,
            DescWeight::OFF,
            false,
            cn(&names),
            None,
        );
        assert!(off_amb, "基线本就应判歧义（前提校验）");
        assert!(on_amb, "说明文不得消除名称层的歧义");
    }

    /// desc 也不能**制造**歧义：原实现不歧义时，desc 拉不开的两件仍不歧义。
    #[test]
    fn desc_cannot_create_ambiguity() {
        // 名称分差足够大：idx0 名称命中（+100），idx1 完全不命中。
        let items = [
            item("1111111", "StarTiara Base", 0),
            item("2222222", "Unrelated", 0),
        ];
        let names = [("1111111", "StarTiara Base"), ("2222222", "Unrelated")];
        let descs = descs_for(&items, &["x", "x"]);
        let w = DescWeight {
            word_bonus: 50,
            max_words: 3,
        };
        let (_, on_amb) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        assert!(!on_amb, "名称层明确不歧义时，desc 不得制造歧义");
    }

    /// 权重极端放大的前提下，名称零证据的候选**仍不得**反超名称有证据者。
    ///
    /// 这是「名称证据门槛」的真红证：门槛若退化成 `s > 0`（次序分让非首位候选
    /// 恒有正分），本测试会失败——desc 就能把无关商品顶成 picked。
    #[test]
    fn desc_extreme_weight_cannot_beat_name_evidence() {
        let items = [
            item("1111111", "StarTiara Base", 0),
            item("2222222", "Unrelated", 0),
        ];
        let names = [("1111111", "StarTiara Base"), ("2222222", "Unrelated")];
        let descs = descs_for(
            &items,
            &["x", "StarTiara StarTiara StarTiara StarTiara StarTiara"],
        );
        // 权重远超名称命中的 +100，逼迫门槛生效。
        let w = DescWeight {
            word_bonus: 500,
            max_words: 10,
        };
        let (on, _) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        assert_eq!(
            on.map(|i| i.id.as_str()),
            Some("1111111"),
            "权重再大，名称零证据者也不得反超"
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
        // 多候选 + 全部说明文缺失：必须与 OFF 逐项一致，且不得 panic。
        // （原文用单候选并断言 `is_none() || Some("1")`——两个可能值都成立，恒真无鉴别力。）
        let items = [
            item("1111111", "Star Tiara", 0),
            item("2222222", "Star Tiara Gold", 0),
        ];
        let names = [("1111111", "Star Tiara"), ("2222222", "Star Tiara Gold")];
        let w = DescWeight {
            word_bonus: 50,
            max_words: 3,
        };
        let (on, on_amb) =
            score_and_pick_with_desc("StarTiara", &items, &[], w, false, cn(&names), None);
        let (off, off_amb) = score_and_pick_with_desc(
            "StarTiara",
            &items,
            &[],
            DescWeight::OFF,
            false,
            cn(&names),
            None,
        );
        assert_eq!(
            on.map(|i| i.id.clone()),
            off.map(|i| i.id.clone()),
            "说明文全缺时不得改变选优"
        );
        assert_eq!(on_amb, off_amb, "说明文全缺时歧义判定不得改变");
    }

    /// 单候选时 desc 必须与 OFF 结果一致。
    ///
    /// **注意**：这条不是「守卫是否生效」的验证——实测删掉 `items.len() <= 1`
    /// 守卫后（R4 变异）本测试**仍然通过**：单候选时 `scored` 只有一项，`best_idx`
    /// 必为 0，`picked` 与 `ambiguous` 都不受加分影响。也就是说该守卫是**纯优化**
    /// 而非行为保证，无法为其写出有区分力的测试。
    ///
    /// 保留本测试的价值是另一件事：它守住「单候选路径与 OFF 等价」这个**外部契约**，
    /// 将来若有人在加分分支里加入影响 picked/ambiguous 的逻辑，这里会红。
    #[test]
    fn single_item_is_untouched_by_desc() {
        let items = [item("1", "StarTiara Base", 0)];
        let names = [("1", "StarTiara Base")];
        let w = DescWeight::new(50, 2).expect("合法权重");
        let descs = vec![DescriptionText::new("1", "StarTiara StarTiara")];
        let (on, on_amb) =
            score_and_pick_with_desc("StarTiara", &items, &descs, w, false, cn(&names), None);
        let (off, off_amb) = score_and_pick_with_desc(
            "StarTiara",
            &items,
            &descs,
            DescWeight::OFF,
            false,
            cn(&names),
            None,
        );
        assert_eq!(
            on.map(|i| i.id.clone()),
            off.map(|i| i.id.clone()),
            "单候选必须与 OFF 一致（守卫被删则会不同）"
        );
        assert_eq!(on_amb, off_amb, "单候选的歧义标记也必须一致");
    }

    /// 零候选：两个入口都返回 (None, false)，不得 panic。
    #[test]
    fn empty_items_is_safe_with_desc() {
        let items: [Item; 0] = [];
        let w = DescWeight::new(50, 2).expect("合法权重");
        let (got, amb) =
            score_and_pick_with_desc("x", &items, &[], w, false, |_| Some(String::new()), None);
        assert!(got.is_none());
        assert!(!amb);
    }

    /// 加分不得翻转「名称完全不命中」的否决：权重再大也不能凭说明文入选。
    #[test]
    fn desc_cannot_override_name_mismatch() {
        // 两候选、**名称与规范名都不含查询词**（超长名触发 -10，base 分 ≤ 0），
        // 说明文里却堆满查询词。原实现会否决（picked=None），desc 不得把它救回来。
        // （原文用单候选——单候选走 `items.len() <= 1` 提前返回，根本没走到加分分支。）
        let a = "A".repeat(60);
        let b = "B".repeat(60);
        let items = [item("1", &a, 0), item("2", &b, 0)];
        let names = [("1", a.as_str()), ("2", b.as_str())];
        let descs = descs_for(
            &items,
            &[
                "Moonpiercer Moonpiercer Moonpiercer",
                "Moonpiercer Moonpiercer Moonpiercer",
            ],
        );
        let (best, amb) = score_and_pick_with_desc(
            "Moonpiercer",
            &items,
            &descs,
            DescWeight {
                word_bonus: 50,
                max_words: 3,
            },
            false,
            cn(&names),
            None,
        );
        assert!(
            best.is_none(),
            "名称零证据时说明文不得让候选入选，实际选中的是 {:?}",
            best.map(|i| &i.id)
        );
        assert!(!amb);
    }

    // ── 权重 golden（锁定线上行为契约的**精确分值**）──────────────────
    //
    // 背景：`score_and_pick` 只返回「选了谁」，光靠结果断言锁不住权重——实测把
    // `s += 100` 改成 `s += 80`、`s += 20` 改成 `s += 5`、次序步长 2 改成 0，
    // 全量测试**仍然全绿**。故这些用例直接断言 `weighted_scores` 的 `total`。

    fn ws(query: &str, items: &[Item], names: &[(&str, &str)]) -> Vec<i32> {
        weighted_scores(query, items, false, cn(names))
            .into_iter()
            .map(|s| s.total)
            .collect()
    }

    /// 名称命中 100 + 次序 10 = 110。
    #[test]
    fn golden_score_name_hit_only() {
        let items = [item("1", "StarTiara", 0)];
        // 名称命中，但 `StarTiara` 是一个词 → 词级也命中（+20）？
        // query 无分隔符 → split_query_words 只有一项 "startiara"（8 字符 ≥3）→ 也命中 +20。
        assert_eq!(
            ws("StarTiara", &items, &[("1", "Nope")]),
            vec![100 + 20 + 10]
        );
    }

    /// 名称命中 + 规范名命中 = 200（再加词级与次序）。
    #[test]
    fn golden_score_name_and_canon() {
        let items = [item("1", "StarTiara", 0)];
        assert_eq!(
            ws("StarTiara", &items, &[("1", "StarTiara")]),
            vec![100 + 100 + 20 + 10]
        );
    }

    /// 词级命中 +20：查询含空格拆成两个词，标题只含其中一个。
    #[test]
    fn golden_score_word_level_twenty() {
        // query = "abc xyzw"：词 ["abc"(3), "xyzw"(4)]；标题只含 "abc"。
        // 完整查询串不在标题里 → 无 +100；只有 "abc" 命中 → +20；次序 +10。
        let items = [item("1", "abc", 0)];
        assert_eq!(ws("abc xyzw", &items, &[("1", "nothing")]), vec![20 + 10]);
    }

    /// 次序分步长 2：第 0/1/2 项分别 10/8/6，第 5 项起封底 0。
    #[test]
    fn golden_score_order_step() {
        let items = [
            item("1", "z", 0),
            item("2", "z", 0),
            item("3", "z", 0),
            item("4", "z", 0),
            item("5", "z", 0),
            item("6", "z", 0),
            item("7", "z", 0),
        ];
        // 无任何命中，只有次序分。
        assert_eq!(
            ws("qqqq", &items, &[]),
            vec![10, 8, 6, 4, 2, 0, 0],
            "次序步长必须为 2 且封底于 0"
        );
    }

    /// 免费加成 +5：仅 prefer_free 且价格为 0 时生效。
    #[test]
    fn golden_score_free_bonus_five() {
        let items = [item("1", "z", 0)];
        let off = weighted_scores("qqqq", &items, false, cn(&[]));
        let on = weighted_scores("qqqq", &items, true, cn(&[]));
        assert_eq!(on[0].total - off[0].total, 5, "prefer_free 加成必须恰为 5");
        // 付费项不得加成
        let paid = [item("1", "z", 500)];
        let on_paid = weighted_scores("qqqq", &paid, true, cn(&[]));
        assert_eq!(on_paid[0].total, 10, "付费项不得拿免费加成");
    }

    /// 过长惩罚 -10：名称与规范名都超过查询 5 倍时才触发。
    #[test]
    fn golden_score_long_penalty_ten() {
        let long = "A".repeat(60);
        let items = [item("1", &long, 0)];
        // 名称与规范名都超长 → -10；无命中 → 次序 10 - 10 = 0
        assert_eq!(ws("Moonpiercer", &items, &[("1", long.as_str())]), vec![0]);
        // 只有名称超长、规范名很短 → 不触发惩罚
        assert_eq!(ws("Moonpiercer", &items, &[("1", "Nope")]), vec![10]);
    }

    /// 名称证据标志必须与「名称/规范名是否真命中」一致（desc 门槛依赖它）。
    #[test]
    fn golden_name_evidence_flags() {
        let items = [item("1", "StarTiara", 0), item("2", "Nope", 0)];
        let s = weighted_scores(
            "StarTiara",
            &items,
            false,
            cn(&[("1", "Nope"), ("2", "Nope")]),
        );
        assert!(s[0].name_hit, "名称命中必须置位");
        assert!(!s[0].canon_hit);
        assert!(!s[1].name_hit, "名称不命中不得置位（次序分不算证据）");
        assert!(!s[1].canon_hit);
    }
}
