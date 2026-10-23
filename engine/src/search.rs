//! 搜索 / 店铺翻页 HTML 解析。
//!
//! BOOTH 为 SSR 页面，用正则绑定 `data-product-*` 卡片属性。
//! 注意：解析规则绑定页面结构，BOOTH 改版即可能失效；JSON 接口（fetch.rs）可作降级。

use std::sync::OnceLock;

use fancy_regex::Regex;
use reqwest::blocking::Client;

use crate::http::get;
use crate::session::default_headers;

/// 搜索结果卡片。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchItem {
    pub id: String,
    pub name: String,
    pub price: i64,
    pub price_text: String,
    pub brand: String,
    pub shop: String,
    pub category: String,
    pub category_name: String,
    pub thumbnail: String,
}

/// HTML 实体反转义（常见命名实体 + 十进制数字实体）。
pub fn unescape_html(s: &str) -> String {
    let mut out = s.to_string();
    let replacements = [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&nbsp;", " "),
    ];
    for (from, to) in replacements {
        out = out.replace(from, to);
    }
    // 数字实体：&#NNN;（十进制）
    let dec_re = Regex::new(r"&#(\d+);").expect("valid regex");
    loop {
        let caps = dec_re.captures(&out).ok().flatten();
        let Some(caps) = caps else { break };
        let Some(m) = caps.get(0) else { break };
        let Some(num) = caps.get(1) else { break };
        let Ok(codepoint) = num.as_str().parse::<u32>() else {
            break;
        };
        if let Some(ch) = char::from_u32(codepoint) {
            out = out.replacen(m.as_str(), &ch.to_string(), 1);
        } else {
            out = out.replacen(m.as_str(), "\u{FFFD}", 1);
        }
    }
    out
}

/// 模块级正则单例。
struct Re {
    id: Regex,
    name: Regex,
    price: Regex,
    brand: Regex,
    category: Regex,
    shop: Regex,
    thumb: Regex,
    cat_name: Regex,
    price_text: Regex,
    href: Regex,
    bare: Regex,
}

fn re() -> &'static Re {
    static RE: OnceLock<Re> = OnceLock::new();
    RE.get_or_init(|| Re {
        id: Regex::new(r#"data-product-id="(\d+)""#).expect("valid regex"),
        name: Regex::new(r#"data-product-name="([^"]*)""#).expect("valid regex"),
        price: Regex::new(r#"data-product-price="([^"]*)""#).expect("valid regex"),
        brand: Regex::new(r#"data-product-brand="([^"]*)""#).expect("valid regex"),
        category: Regex::new(r#"data-product-category="([^"]*)""#).expect("valid regex"),
        shop: Regex::new(r#"item-card__shop-name[^>]*>([^<]+)<"#).expect("valid regex"),
        thumb: Regex::new(r#"data-original="(https://booth\.pximg\.net/[^"]*)""#)
            .expect("valid regex"),
        cat_name: Regex::new(r#"item-card__category-anchor[^>]*>([^<]+)<"#).expect("valid regex"),
        price_text: Regex::new(r#"price[^>]*>([^<]*\d+[^<]*)<"#).expect("valid regex"),
        href: Regex::new(r#"href="https?://[^"]*/items/(\d+)""#).expect("valid regex"),
        bare: Regex::new(r"/items/(\d+)").expect("valid regex"),
    })
}

/// 取首个匹配的捕获组。
fn group(regex: &Regex, hay: &str, idx: usize) -> String {
    regex
        .captures(hay)
        .ok()
        .flatten()
        .and_then(|c| c.get(idx))
        .map(|g| g.as_str().to_string())
        .unwrap_or_default()
}

/// 从搜索/店铺 HTML 提取商品卡片列表。
///
/// 上下文窗口按**字符偏移**计算（id 前 300 / 后 4000 字符），
/// 而 `fancy_regex::Match` 的位置是**字节偏移**——含日文时两者差 2~3 倍，
/// 必须换算成字符偏移再切 ctx，否则窗口位置漂移导致字段取值不同。
pub fn parse_search_html(html: &str) -> Vec<SearchItem> {
    let r = re();
    let mut items = Vec::new();
    for m in r.id.find_iter(html).filter_map(|m| m.ok()) {
        // 字符偏移语义：start_char = max(0, char_start - 300)
        //               end_char   = min(char_len, char_end + 4000)
        let start_char = char_index_of(html, m.start()).saturating_sub(300);
        let end_char = char_index_of(html, m.end()) + 4000;
        let ctx = char_window(html, start_char, end_char);

        let id = group(&r.id, ctx, 1);
        let name = unescape_html(&group(&r.name, ctx, 1));
        let price = group(&r.price, ctx, 1);
        let brand = unescape_html(&group(&r.brand, ctx, 1));
        let category = unescape_html(&group(&r.category, ctx, 1));
        let shop = {
            let s = group(&r.shop, ctx, 1).trim().to_string();
            if s.is_empty() { brand.clone() } else { s }
        };
        let thumbnail = group(&r.thumb, ctx, 1);
        let category_name = group(&r.cat_name, ctx, 1).trim().to_string();
        let price_text = {
            let pt = group(&r.price_text, ctx, 1).trim().to_string();
            if pt.is_empty() {
                format!("¥ {price}")
            } else {
                pt
            }
        };
        let price_num = if !price.is_empty() && price.chars().all(|c| c.is_ascii_digit()) {
            price.parse().unwrap_or(-1)
        } else {
            -1
        };

        items.push(SearchItem {
            id,
            name,
            price: price_num,
            price_text,
            brand,
            shop,
            category,
            category_name,
            thumbnail,
        });
    }
    items
}

/// 字节偏移 `byte` 在 `s` 中对应的字符索引。
fn char_index_of(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().count()
}

/// 按**字符索引**区间 `[start, end)` 切 `s`（字符切片语义），自动夹紧。
fn char_window(s: &str, start: usize, end: usize) -> &str {
    let total = s.chars().count();
    let start = start.min(total);
    let end = end.min(total);
    let start_byte = s
        .char_indices()
        .nth(start)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let end_byte = s.char_indices().nth(end).map(|(i, _)| i).unwrap_or(s.len());
    &s[start_byte..end_byte]
}

/// 搜索 BOOTH：`?q=...` 返回商品列表。
pub fn search_booth(client: &Client, query: &str) -> Result<Vec<SearchItem>, String> {
    let url = format!("https://booth.pm/ja/items?q={}", urlencode(query));
    let r = get(client, &url, default_headers()).map_err(|e| format!("search request: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("search status {}", r.status()));
    }
    let html = r.text().map_err(|e| format!("search body: {e}"))?;
    Ok(parse_search_html(&html))
}

/// 一次按名解析的结果。
pub struct NamedHit {
    pub item: crate::score::Item,
    /// 命中所用的检索词（供展示与排查）。
    pub query: String,
}

/// 按名解析的整体结果。
pub struct ResolveOutcome {
    pub hit: Option<NamedHit>,
    /// 实际尝试过的检索词，按尝试顺序。
    pub tried: Vec<String>,
}

/// 按文件名候选解析出「最可能是它」的商品（**三端唯一实现**）。
///
/// 两轮**严格分离**：
///   1. 第一轮：原候选，按 `sanitize_query` 给出的优先级逐个试；
///   2. 第二轮：仅当第一轮**全部落空**时，才试 `kana` 读音变体。
///
/// 历史缺陷：两轮被压成一次 `chain`，于是「变体轮是否运行」取决于第一个候选
/// 是否返回了非空结果，而那个判据又与「名称是否真的命中」无关——触发与否
/// 实际由标题长度决定（`scored[0].0 <= 0` 因次序分恒 >0 而近乎不可达）。
///
/// 接受条件必须是「名称真的命中 **且** 不歧义」：
///   - `score_and_pick` 只返回「最优 + 歧义」，而次序分让首条恒有正分，
///     只看 `is_some()` 会把无关商品当命中并错归档；
///   - 变体轮用的是**推出来的词**，猜错代价是错归档，判据同样严格。
pub fn resolve_by_name(
    client: &Client,
    candidates: &[String],
    resource_names: Option<&[String]>,
    max_variants: usize,
    canonical: impl Fn(&str) -> String,
) -> ResolveOutcome {
    let mut tried: Vec<String> = Vec::new();
    let mut hit: Option<NamedHit> = None;

    // 第一轮：原候选。
    for q in candidates {
        tried.push(q.clone());
        if let Some(h) = pick_once(client, q, resource_names, &canonical) {
            hit = Some(h);
            break;
        }
    }

    // 第二轮：kana 读音变体。**只在第一轮全部落空后**进入。
    if hit.is_none() && max_variants > 0 {
        let variants = crate::clean::with_kana_variants(candidates.to_vec(), max_variants);
        for q in variants.into_iter().filter(|c| !candidates.contains(c)) {
            tried.push(q.clone());
            if let Some(h) = pick_once(client, &q, resource_names, &canonical) {
                hit = Some(h);
                break;
            }
        }
    }

    ResolveOutcome { hit, tried }
}

/// 对单个检索词执行「搜索 → 评分 → 名称命中校验」。
fn pick_once(
    client: &Client,
    q: &str,
    resource_names: Option<&[String]>,
    canonical: &impl Fn(&str) -> String,
) -> Option<NamedHit> {
    let results = search_booth(client, q).ok()?;
    if results.is_empty() {
        return None;
    }
    let items: Vec<crate::score::Item> = results
        .iter()
        .map(|r| crate::score::Item {
            id: r.id.clone(),
            name: r.name.clone(),
            price: r.price,
        })
        .collect();
    let (picked, ambiguous) =
        crate::score::score_and_pick(q, &items, false, canonical, resource_names);
    let p = picked?;
    if ambiguous {
        return None;
    }
    let cn = canonical(&p.id);
    // 名称命中用大小写不敏感的原串包含：`norm` 只留 ASCII，日文会被滤成空串。
    let name_ok = {
        let ql = q.trim().to_lowercase();
        !ql.is_empty() && (p.name.to_lowercase().contains(&ql) || cn.to_lowercase().contains(&ql))
    };
    // 资源名兜底：标题不命中但资源名命中时，`score_and_pick` 已据 resource_names 放行。
    let resource_ok = resource_names.is_some_and(|rs| {
        let ql = q.trim().to_lowercase();
        !ql.is_empty() && rs.iter().any(|r| r.to_lowercase().contains(&ql))
    });
    if !name_ok && !resource_ok {
        return None;
    }
    Some(NamedHit {
        item: p.clone(),
        query: q.to_string(),
    })
}

/// 店铺 `/items?page=N` 翻页收集商品 ID（店铺根有 Cloudflare 护盾，必须走 /items）。
pub fn crawl_item_ids(client: &Client, sub: &str, rate_limit: f64) -> Result<Vec<String>, String> {
    let r = re();
    let mut ids: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut page = 1u32;
    loop {
        let url = format!("https://{sub}.booth.pm/items?page={page}");
        let response = get(client, &url, default_headers()).map_err(|e| format!("crawl: {e}"))?;
        if !response.status().is_success() {
            break;
        }
        let html = response.text().map_err(|e| format!("crawl body: {e}"))?;
        let mut found: Vec<String> = r
            .href
            .find_iter(&html)
            .filter_map(|m| m.ok())
            .map(|m| group(&r.href, m.as_str(), 1))
            .filter(|s| !s.is_empty())
            .collect();
        if found.is_empty() {
            found = r
                .bare
                .find_iter(&html)
                .filter_map(|m| m.ok())
                .map(|m| group(&r.bare, m.as_str(), 1))
                .filter(|s| !s.is_empty())
                .collect();
        }
        let mut new_ids = Vec::new();
        let mut seen_page = std::collections::HashSet::new();
        for i in found {
            if seen_page.insert(i.clone()) && !seen.contains(&i) {
                seen.insert(i.clone());
                new_ids.push(i);
            }
        }
        if new_ids.is_empty() {
            break;
        }
        ids.extend(new_ids);
        page += 1;
        if rate_limit > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(rate_limit));
        }
    }
    Ok(ids)
}

/// URL 编码（UTF-8 百分比编码，保留 RFC 3986 unreserved 字符）。
fn urlencode(s: &str) -> String {
    s.as_bytes()
        .iter()
        .map(|&b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_search_html_basic() {
        let html = r#"<li data-product-id="7437723" data-product-name="Lunaria Paper Fan" data-product-price="0" data-product-brand="Studio" data-product-category="3Dモデル">
        <img data-original="https://booth.pximg.net/12345.jpg">
        <span class="item-card__shop-name">No39</span>
        <a class="item-card__category-anchor">3D模型</a>
        <span class="price">¥ 0</span>
        </li>"#;
        let items = parse_search_html(html);
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.id, "7437723");
        assert_eq!(it.name, "Lunaria Paper Fan");
        assert_eq!(it.price, 0);
        assert_eq!(it.brand, "Studio");
        assert_eq!(it.shop, "No39");
        assert_eq!(it.thumbnail, "https://booth.pximg.net/12345.jpg");
    }

    #[test]
    fn unescape_basic() {
        assert_eq!(unescape_html("a&amp;b"), "a&b");
        assert_eq!(unescape_html("&lt;tag&gt;"), "<tag>");
        assert_eq!(unescape_html("caf&#233;"), "café");
    }

    #[test]
    fn urlencode_basic() {
        assert_eq!(urlencode("abc 123"), "abc%20123");
        assert_eq!(urlencode("メカ"), "%E3%83%A1%E3%82%AB");
    }
}
