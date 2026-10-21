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
    let lower = description.to_lowercase();
    // 命中位置按字符偏移记录，避免多字节字符把窗口切坏。
    let mut hit_char: Option<usize> = None;
    for t in terms {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(byte_pos) = lower.find(&t.to_lowercase()) {
            hit_char = Some(description[..byte_pos].chars().count());
            break;
        }
    }
    let (start, hit) = match hit_char {
        // 命中时向前留一点上下文，但不越过 0。
        Some(pos) => (pos.saturating_sub(max_chars / 4), true),
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

    #[test]
    fn excerpt_respects_char_budget_for_cjk() {
        // 多字节字符下 max_chars 按字符而非字节计，否则会切坏字符。
        let desc = "あいうえおかきくけこ";
        let e = excerpt(desc, &[], 5);
        assert_eq!(e.text.chars().count(), 5);
        assert!(e.truncated);
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
}
