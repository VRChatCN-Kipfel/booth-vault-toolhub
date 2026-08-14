//! ID 与链接解析：parse_discrete / extract_id / shop_subdomain / extract_shop_id_from_url。
//!
//! 注：含环视断言的正则须用 `fancy_regex`（标准 `regex` 不支持环视）。

use fancy_regex::Regex;

/// 裸 ID：5+ 位数字，前后不能是数字。
const BARE_ID_RE: &str = r"(?<!\d)\d{5,}(?!\d)";
/// 文件名中的商品 ID：7+ 位数字。
const FILE_ID_RE: &str = r"(?<!\d)(\d{7,})(?!\d)";
/// URL 商品 ID。
const URL_ITEM_RE: &str = r"/items/(\d+)";

/// 解析文本中的商品 ID：优先 URL 形式 `/items/(\d+)`，其次裸 ID（去逗号后）。
/// 去重保序。
pub fn parse_discrete(text: &str) -> Vec<String> {
    let url_ids: Vec<String> = Regex::new(URL_ITEM_RE)
        .expect("valid regex")
        .captures_iter(text)
        .filter_map(|m| m.ok())
        .filter_map(|caps| caps.get(1).map(|g| g.as_str().to_string()))
        .collect();
    let text_no_comma = text.replace(',', " ");
    let bare: Vec<String> = Regex::new(BARE_ID_RE)
        .expect("valid regex")
        .find_iter(&text_no_comma)
        .filter_map(|m| m.ok())
        .map(|m| m.as_str().to_string())
        .collect();
    let mut ids = url_ids.clone();
    for b in bare {
        if !ids.contains(&b) {
            ids.push(b);
        }
    }
    ids
}

/// 从文本提取商品 ID（7+ 位数字，首个匹配）。找不到返回空串。
pub fn extract_id(text: &str) -> String {
    Regex::new(FILE_ID_RE)
        .expect("valid regex")
        .captures(text)
        .ok()
        .flatten()
        .and_then(|caps| caps.get(1).map(|g| g.as_str().to_string()))
        .unwrap_or_default()
}

/// 提取店铺子域名：`https://(sub).booth.pm` → `sub`；否则返回清洗后的原文。
pub fn shop_subdomain(url_or_sub: &str) -> String {
    if let Ok(Some(caps)) = Regex::new(r"https?://([^./]+)\.booth\.pm")
        .expect("valid regex")
        .captures(url_or_sub)
        && let Some(g) = caps.get(1)
    {
        return g.as_str().to_string();
    }
    url_or_sub.trim().trim_matches('/').to_string()
}

/// 从 URL 提取店铺子域名（`[\w-]+`）。找不到返回空串。
pub fn extract_shop_id_from_url(url: &str) -> String {
    Regex::new(r"https?://([\w-]+)\.booth\.pm")
        .expect("valid regex")
        .captures(url)
        .ok()
        .flatten()
        .and_then(|caps| caps.get(1).map(|g| g.as_str().to_string()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_discrete_url_ids() {
        let ids =
            parse_discrete("https://booth.pm/ja/items/7437723 and https://booth.pm/items/12345");
        assert_eq!(ids, vec!["7437723", "12345"]);
    }

    #[test]
    fn parse_discrete_bare_ids() {
        let ids = parse_discrete("follow 7437723 plus 88888");
        assert_eq!(ids, vec!["7437723", "88888"]);
    }

    #[test]
    fn parse_discrete_no_partial() {
        // 整体是连续数字时贪婪匹配整段
        assert_eq!(parse_discrete("123888889"), vec!["123888889"]);
        // 不足 5 位不匹配
        assert_eq!(parse_discrete("88"), Vec::<String>::new());
        // 前后有非数字边界时可提取
        assert_eq!(parse_discrete("id88888name"), vec!["88888"]);
        assert_eq!(parse_discrete("88888"), vec!["88888"]);
    }

    #[test]
    fn parse_discrete_comma_separated() {
        assert_eq!(parse_discrete("7437723, 88888"), vec!["7437723", "88888"]);
    }

    #[test]
    fn parse_discrete_dedup() {
        assert_eq!(parse_discrete("7437723 7437723 7437723"), vec!["7437723"]);
    }

    #[test]
    fn extract_id_7plus() {
        assert_eq!(extract_id("7032906_POP_Hair.zip"), "7032906");
        assert_eq!(extract_id("no_id_here"), "");
        assert_eq!(extract_id("id1234567name"), "1234567");
    }

    #[test]
    fn shop_subdomain_from_url() {
        assert_eq!(shop_subdomain("https://muffin.booth.pm/items/1"), "muffin");
        assert_eq!(shop_subdomain("muffin.booth.pm"), "muffin.booth.pm");
        assert_eq!(shop_subdomain("  muffin/  "), "muffin");
    }

    #[test]
    fn extract_shop_id_from_url_basic() {
        assert_eq!(
            extract_shop_id_from_url("https://muffin.booth.pm/items/1"),
            "muffin"
        );
        assert_eq!(
            extract_shop_id_from_url("https://some-shop_1.booth.pm"),
            "some-shop_1"
        );
        assert_eq!(extract_shop_id_from_url("no_url"), "");
    }
}
