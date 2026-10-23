//! 来源证据：为商品资料生成可核验的来源指纹，并抽取「连续原文」摘录。
//!
//! 解决的问题：把 BOOTH 商品资料交给上层判断时，`description` 是**不可信外部数据**。
//! 上层若拿它当依据，必须能回答两个问题——
//!   1. 这条资料有没有被改动过？→ `source_hash`
//!   2. 引用的是不是原文里连续的一段？→ `excerpt` / `is_contiguous_quote`
//!
//! 两者都是纯函数，无网络、无 IO，可在任意层复用。

use sha2::{Digest, Sha256};

/// 参与来源指纹的字段（规范化顺序由 `canonical_json` 负责）。
///
/// 刻意**不含**展示用的派生字段（缩略图尺寸、价格文案等）：它们随展示形态变化，
/// 混进来会让「资料没变但指纹变了」，指纹就失去意义。
pub struct SourceFields<'a> {
    pub name: &'a str,
    pub category: &'a str,
    pub tags: &'a [String],
    pub description: &'a str,
}

/// 生成冻结的规范 JSON（键按字典序，字段顺序固定），供计算指纹与人工比对。
///
/// 不用 `serde_json::Map` 的插入顺序：那样同内容不同构造顺序会产生不同字节，
/// 指纹就不稳定了。
pub fn canonical_json(f: &SourceFields<'_>) -> String {
    // 手写而非用 serde：字段少且顺序必须完全确定，避免将来给结构体加字段时
    // 悄悄改变已有指纹的语义。
    let mut tags: Vec<&String> = f.tags.iter().collect();
    tags.sort();
    let mut out = String::from("{");
    out.push_str(&format!("\"category\":{},", json_str(f.category)));
    out.push_str(&format!("\"description\":{},", json_str(f.description)));
    out.push_str(&format!("\"name\":{},", json_str(f.name)));
    out.push_str("\"tags\":[");
    for (i, t) in tags.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&json_str(t));
    }
    out.push_str("]}");
    out
}

/// 最小 JSON 字符串转义（只处理必须转义的字符，不引入 serde）。
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 来源指纹：规范 JSON 的 SHA-256（小写十六进制）。
///
/// 用途：调用方与返回方各自持有一份数据时，比对指纹即可发现内容漂移，
/// 而不必搬运整段说明文。
pub fn source_hash(f: &SourceFields<'_>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_json(f).as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 抽取围绕关键词的**连续**原文片段（最多 `max_chars` 字符）。
///
/// 与「按关键词拼句子」那种伪摘录不同：返回的一定是原文的连续子串，
/// 上层引用它时能与原文逐字对上。无命中时取正文首部，并在结果中
/// 用 `hit=false` 如实告知「这不是围绕需求的证据」。
pub struct Excerpt {
    pub text: String,
    /// 是否围绕某个检索词命中。
    pub hit: bool,
    /// 原文是否长于本片段（被截断）。
    pub truncated: bool,
}

/// 小写化并记录**字符映射**：返回 `(小写串, map)`，其中 `map[i]` 是原串第 `i` 个
/// 字符在小写串中的字节起点，末尾追加一项等于小写串总字节长度。
///
/// 为什么需要映射：`to_lowercase()` 会改变字节长度——`İ`(U+0130) 小写后是
/// `i` + 组合点两个字符，`ẞ`(U+1E9E) 小写后字节数也变。直接拿小写串里的字节位置
/// 去切原串（`description[..byte_pos]`）会落在字符中间，**panic**。
/// 实测 `excerpt("İ猫が好きです", ["猫"], 20)` 会 panic：
/// `end byte index 11 is not a char boundary; it is inside '猫'`。
fn lowercase_with_map(s: &str) -> (String, Vec<usize>) {
    let mut lower = String::with_capacity(s.len());
    let mut map = Vec::with_capacity(s.chars().count() + 1);
    for c in s.chars() {
        map.push(lower.len());
        for lc in c.to_lowercase() {
            lower.push(lc);
        }
    }
    map.push(lower.len());
    (lower, map)
}

/// 把「小写串中的字节位置」换算成「原串中的字符索引」。
///
/// `map` 单调递增，故可用二分定位所属字符。
fn char_index_for(lower_byte: usize, map: &[usize]) -> usize {
    map.partition_point(|&start| start <= lower_byte)
        .saturating_sub(1)
        .min(map.len().saturating_sub(1))
}

/// 抽取摘录。`terms` 为空时等价于「取正文首部」。
pub fn excerpt(description: &str, terms: &[String], max_chars: usize) -> Excerpt {
    let total = description.chars().count();
    let truncated = total > max_chars;
    if max_chars == 0 || description.is_empty() {
        return Excerpt {
            text: String::new(),
            hit: false,
            truncated,
        };
    }
    // 用带映射的小写化定位命中，**绝不**拿小写串的字节偏移直接切原串（会 panic）。
    let (lower, map) = lowercase_with_map(description);
    let mut hit_char: Option<usize> = None;
    for t in terms {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(byte_pos) = lower.find(&t.to_lowercase()) {
            hit_char = Some(char_index_for(byte_pos, &map));
            break;
        }
    }
    let (start, hit) = match hit_char {
        // 命中时向前留一点上下文，但**同时回填尾部预算**：若命中点太靠后，
        // 单纯 saturating_sub 会让窗口越过正文末尾、摘录短于 max_chars。
        Some(pos) => {
            let back = max_chars / 4;
            let start = pos
                .saturating_sub(back)
                .min(total.saturating_sub(max_chars));
            (start, true)
        }
        None => (0, false),
    };
    let text: String = description.chars().skip(start).take(max_chars).collect();
    Excerpt {
        text,
        hit,
        truncated: truncated || start > 0,
    }
}

/// 校验一条引用是否为来源字段中的**连续原文**。
///
/// 只做「连续出现」判定，不负责语义相关性——关键词出现在说明里
/// 并不等于商品满足需求，那是调用方的判断责任。
pub fn is_contiguous_quote(field_text: &str, quote: &str) -> bool {
    let q = quote.trim();
    !q.is_empty() && field_text.contains(q)
}

/// 商品资料的可核验视图（**三端统一出口**）。
///
/// 设计立场：`description` 是**不可信外部数据**。把它交给上层（agent / 人）时，
/// 必须同时给出「这段资料是否被改动过」与「引用的是不是原文连续片段」，否则上层
/// 无法判断该不该采信。故本结构把 `source_hash` 与 `excerpt` 与正文**绑在一起**
/// 返回，而不是让调用方自己去凑。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct InspectOutput {
    pub command: String,
    pub id: String,
    pub name: String,
    /// 分类展示名（父分类/子分类中取可用者，可能为空）。
    pub category: String,
    /// 标签名（顺序按响应，指纹计算时会排序）。
    pub tags: Vec<String>,
    pub shop: String,
    /// 价格展示串（站点原文，可能是「無料」等非数字）。
    pub price_text: String,
    /// 说明文全文。**不可信外部数据**，引用时须配合下方两项核验。
    pub description: String,
    /// 来源指纹：`canonical_json` 的 SHA-256（小写十六进制）。
    /// 同一份资料重复取到应恒等；内容任何改动都会变。
    pub source_hash: String,
    /// 围绕查询词（或首部）的**连续**原文片段。
    pub excerpt: String,
    /// 摘录是否围绕某个检索词命中；false 表示「这不是围绕需求的证据」。
    pub excerpt_hit: bool,
    /// 原文是否长于摘录（被截断）。
    pub excerpt_truncated: bool,
}

/// 摘录默认长度（字符）。足够看清上下文，又不会让 agent 上下文被单条商品淹没。
pub const DEFAULT_EXCERPT_CHARS: usize = 200;

/// 由商品资料生成可核验视图。
///
/// `terms` 为空等价于「取正文首部」，此时 `excerpt_hit` 为 false —— 如实告知
/// 调用方「这段不是围绕需求的证据」，不要把它当命中。
pub fn inspect_item(
    item: &crate::fetch::ItemJson,
    terms: &[String],
    max_chars: usize,
) -> InspectOutput {
    let tag_names: Vec<String> = item.tags.iter().map(|t| t.name.clone()).collect();
    let category = if item.category.name.is_empty() {
        item.category
            .parent
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default()
    } else {
        item.category.name.clone()
    };
    let fields = SourceFields {
        name: &item.name,
        category: &category,
        tags: &tag_names,
        description: &item.description,
    };
    // 先算指纹再移出所有权：`fields` 借用了 category/tag_names，
    // 在同一作用域里把它们 move 进结构体会触发借用检查。
    let hash = source_hash(&fields);
    let ex = excerpt(&item.description, terms, max_chars);
    InspectOutput {
        command: "inspect".to_string(),
        id: item.id.clone(),
        name: item.name.clone(),
        category,
        tags: tag_names,
        shop: item.shop.name.clone(),
        price_text: item.price.to_string(),
        description: item.description.clone(),
        source_hash: hash,
        excerpt: ex.text,
        excerpt_hit: ex.hit,
        excerpt_truncated: ex.truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields<'a>(name: &'a str, tags: &'a [String], desc: &'a str) -> SourceFields<'a> {
        SourceFields {
            name,
            category: "3D衣装",
            tags,
            description: desc,
        }
    }

    #[test]
    fn canonical_json_is_order_stable() {
        let t1 = vec!["b".to_string(), "a".to_string()];
        let t2 = vec!["a".to_string(), "b".to_string()];
        assert_eq!(
            canonical_json(&fields("n", &t1, "d")),
            canonical_json(&fields("n", &t2, "d")),
            "标签顺序不得影响指纹"
        );
    }

    #[test]
    fn source_hash_changes_with_content() {
        let tags: Vec<String> = vec![];
        let a = source_hash(&fields("n", &tags, "d"));
        let b = source_hash(&fields("n", &tags, "d2"));
        assert_ne!(a, b);
        assert_eq!(a.len(), 64, "SHA-256 十六进制应为 64 字符");
    }

    #[test]
    fn source_hash_is_deterministic() {
        let tags = vec!["VRChat".to_string()];
        assert_eq!(
            source_hash(&fields("n", &tags, "d")),
            source_hash(&fields("n", &tags, "d"))
        );
    }

    #[test]
    fn excerpt_is_contiguous_with_source() {
        let desc = "対応素体：桔梗 / マヌカ。付属品：衣装一式。";
        let e = excerpt(desc, &["マヌカ".to_string()], 20);
        assert!(e.hit, "应命中检索词");
        assert!(
            desc.contains(&e.text),
            "摘录必须是原文连续子串，实际：{}",
            e.text
        );
    }

    #[test]
    fn excerpt_without_hit_is_head_and_flagged() {
        let desc = "这是一个没有任何检索词的说明文。";
        let e = excerpt(desc, &["不存在的词".to_string()], 10);
        assert!(!e.hit, "未命中必须如实标记");
        assert!(desc.starts_with(&e.text));
    }

    /// 命中时摘录**必须包含命中的词**，而不只是「原文的某个连续子串」。
    ///
    /// 这是区分「真的围着命中词取」与「无条件取正文首部」的唯一断言：后者同样
    /// 满足 `desc.contains(&e.text)`，只靠上一条测试无法区分（实测把
    /// `skip(start)` 改成恒 `skip(0)` 时上一条仍绿）。故命中词刻意放在尾部，
    /// 使两种实现的输出必然不同。
    #[test]
    fn excerpt_centers_on_the_hit_not_the_head() {
        let desc = format!("{}{}", "あ".repeat(80), "マヌカ");
        let e = excerpt(desc.as_str(), &["マヌカ".to_string()], 20);
        assert!(e.hit);
        assert!(
            e.text.contains("マヌカ"),
            "摘录必须包含命中的词，实际：{}",
            e.text
        );
        // 对照：首部 20 字里根本没有这个词，说明不是取开头。
        assert!(
            !desc.chars().take(20).collect::<String>().contains("マヌカ"),
            "测试前提：命中词必须在尾部"
        );
    }

    #[test]
    fn excerpt_respects_char_budget_for_cjk() {
        // 多字节字符下 max_chars 按字符而非字节计，否则会切坏字符。
        let desc = "あいうえおかきくけこ";
        let e = excerpt(desc, &[], 5);
        assert_eq!(e.text.chars().count(), 5);
        assert!(e.truncated);
    }

    /// 小写化改变字节长度时不得 panic（历史缺陷：直接用小写串的字节偏移切原串）。
    ///
    /// 用例字符说明——这是实测踩出来的，别看名字猜：
    ///   - `İ`(U+0130) 小写后是 `i` + 组合点，**会**改变字节长度；
    ///   - `ẞ`(U+1E9E) 小写为 `ß`，字节数变化；
    ///   - `Ω`(U+03A9) 与 ASCII `K` **不**改变长度，故不会 panic。
    #[test]
    fn excerpt_unicode_case_folding_does_not_panic() {
        for desc in ["İ猫が好きです", "ẞ猫", "İstanbul 猫", "Ω猫", "K猫", "Aẞ猫"]
        {
            let e = excerpt(desc, &["猫".to_string()], 20);
            assert!(
                desc.contains(&e.text),
                "摘录必须是原文连续子串：desc={desc:?} text={:?}",
                e.text
            );
        }
    }

    /// 命中点靠后时必须回填尾部预算，摘录不得短于 max_chars（正文足够长时）。
    #[test]
    fn excerpt_fills_budget_at_tail() {
        // 命中在最后一个字符，max_chars=5、total=10。
        let desc = "あいうえおかきく猫";
        let e = excerpt(desc, &["猫".to_string()], 5);
        assert!(e.hit);
        assert_eq!(
            e.text.chars().count(),
            5,
            "命中靠后时窗口必须前移回填，实际：{:?}",
            e.text
        );
        assert!(desc.contains(&e.text));
    }

    #[test]
    fn contiguous_quote_rejects_paraphrase() {
        let field = "対応素体：桔梗";
        assert!(is_contiguous_quote(field, "桔梗"));
        assert!(is_contiguous_quote(field, "対応素体：桔梗"));
        // 倒序拼接不是原文
        assert!(!is_contiguous_quote(field, "桔梗：対応素体"));
        assert!(!is_contiguous_quote(field, "   "), "空引用不得通过");
    }

    fn item_with(name: &str, desc: &str, tags: &[&str]) -> crate::fetch::ItemJson {
        crate::fetch::ItemJson {
            id: "1234567".to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            tags: tags
                .iter()
                .map(|t| crate::fetch::TagJson {
                    name: t.to_string(),
                    url: String::new(),
                })
                .collect(),
            ..Default::default()
        }
    }

    /// 接线死的：`inspect_item` 存在但没人调，或调了却没把指纹/摘录带出去，
    /// 上层拿到的仍是一段无从核验的正文。本测试钉住「三样东西必须一起出现」。
    #[test]
    fn inspect_output_carries_hash_and_excerpt_together() {
        let it = item_with(
            "StarTiara",
            "対応素体：桔梗。StarTiara 用の衣装です。",
            &["VRChat"],
        );
        let out = inspect_item(&it, &["桔梗".to_string()], 50);
        assert_eq!(out.command, "inspect");
        assert_eq!(out.id, "1234567");
        assert!(!out.source_hash.is_empty(), "必须带来源指纹");
        assert_eq!(out.source_hash.len(), 64, "应为 SHA-256 十六进制");
        assert!(!out.excerpt.is_empty(), "必须带摘录");
        // 摘录必须是正文的**连续**子串——这是它可被引用的前提。
        assert!(
            is_contiguous_quote(&out.description, &out.excerpt),
            "摘录必须是原文连续子串：{}",
            out.excerpt
        );
        assert!(out.excerpt_hit, "围绕 桔梗 应判命中");
        assert_eq!(out.tags, vec!["VRChat"]);
    }

    /// 不给检索词时必须**如实**说这不是围绕需求的证据，而不是悄悄当成命中。
    #[test]
    fn inspect_without_terms_reports_no_hit() {
        let it = item_with("n", "長い説明文です。これは十分に長い本文です。", &[]);
        let out = inspect_item(&it, &[], 10);
        assert!(!out.excerpt_hit, "无检索词不得报 hit");
        assert!(out.excerpt_truncated, "原文更长应报 truncated");
        assert_eq!(out.excerpt.chars().count(), 10);
    }

    /// 指纹必须对「内容」敏感、对「标签顺序」不敏感。
    #[test]
    fn inspect_hash_tracks_content_not_tag_order() {
        let a = inspect_item(&item_with("n", "d", &["b", "a"]), &[], 10);
        let b = inspect_item(&item_with("n", "d", &["a", "b"]), &[], 10);
        assert_eq!(a.source_hash, b.source_hash, "标签顺序不得影响指纹");
        let c = inspect_item(&item_with("n", "d2", &["a", "b"]), &[], 10);
        assert_ne!(a.source_hash, c.source_hash, "正文变化必须改变指纹");
    }
}
