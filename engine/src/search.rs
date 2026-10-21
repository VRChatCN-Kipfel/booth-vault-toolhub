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

/// 通用检索过滤轴。
///
/// 对应 BOOTH 站内的排序 / 分类 / 价格 / 标签等参数。刻意做成结构体而不是长参数表：
/// 三端（CLI/MCP/GUI）各自构造同一结构，避免像历史上那样出现「GUI 有、CLI 没有」的漂移。
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// 检索词（已确定要查的完整词，调用方负责拆词）。
    pub query: String,
    /// 排序：`new` / `popularity` / `liked` / `price_asc` / `price_desc`。
    pub sort: Option<String>,
    /// 商品类型：`digital` / `physical`。
    pub kind: Option<String>,
    /// R-18：`include` / `exclude` / `only`。
    pub adult: Option<String>,
    /// 标签过滤（AND 语义，多次出现即多标签）。
    pub tags: Vec<String>,
    /// 分类 slug（需日语原文，如 `3D衣装`）。
    pub category: Option<String>,
    /// 价格区间（日元）。
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    /// 仅在售。
    pub in_stock: bool,
    /// 排除词（OR 组内取反，BOOTH 侧不支持，由调用方后置过滤）。
    pub exclude: Vec<String>,
    /// 页码（1 基）。
    pub page: u32,
}

impl SearchQuery {
    /// 仅按关键词检索（历史行为）。
    pub fn simple(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            page: 1,
            ..Default::default()
        }
    }
}

/// 拼装 BOOTH 检索 URL。
///
/// 规则（与站点实测行为对齐）：
///   - `popularity` 排序时**不传 page**：站点会忽略该参数，翻页永远返回第一页；
///     翻页语义由调用方改用 `new` 承担（`effective_sort`）。
///   - 查询词放在 `q`，标签用复数键 `tags[]`，价格用 `min_price`/`max_price`。
pub fn build_search_url(q: &SearchQuery) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !q.query.trim().is_empty() {
        parts.push(format!("q={}", urlencode(&q.query)));
    }
    let sort = q.sort.as_deref().unwrap_or("");
    // popularity 下 page 参数无效，直接不传，避免无意义的规范化重定向。
    let paging_ok = sort != "popularity";
    if !sort.is_empty() {
        parts.push(format!("sort={}", urlencode(sort)));
    }
    if let Some(k) = q.kind.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("type={}", urlencode(k)));
    }
    if let Some(a) = q.adult.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("adult={}", urlencode(a)));
    }
    if let Some(c) = q.category.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("category={}", urlencode(c)));
    }
    if let Some(v) = q.min_price {
        parts.push(format!("min_price={v}"));
    }
    if let Some(v) = q.max_price {
        parts.push(format!("max_price={v}"));
    }
    if q.in_stock {
        parts.push("is_sold_out=false".to_string());
    }
    for t in &q.tags {
        if !t.trim().is_empty() {
            parts.push(format!("tags[]={}", urlencode(t)));
        }
    }
    if paging_ok && q.page > 1 {
        parts.push(format!("page={}", q.page));
    }
    let base = "https://booth.pm/ja/items";
    if parts.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", parts.join("&"))
    }
}

/// 带过滤轴的搜索（走内容缓存，列表页 TTL 10 分钟）。
pub fn search_with(
    client: &Client,
    q: &SearchQuery,
    no_cache: bool,
) -> Result<Vec<SearchItem>, String> {
    let url = build_search_url(q);
    let cache = crate::cache::shared();
    let ttl = if no_cache { 0 } else { crate::cache::TTL_PAGE };
    let r = crate::http::get_cached(client, &url, default_headers(), cache.as_deref(), ttl)
        .map_err(|e| format!("search request: {e}"))?;
    if !(200..300).contains(&r.status) {
        return Err(format!("search status {}", r.status));
    }
    let html = String::from_utf8_lossy(&r.body);
    let mut items = parse_search_html(&html);
    // BOOTH 不支持关键词排除，只能本地后置过滤。
    if !q.exclude.is_empty() {
        items.retain(|it| {
            let hay = format!("{} {}", it.name, it.shop).to_lowercase();
            !q.exclude
                .iter()
                .any(|w| !w.trim().is_empty() && hay.contains(&w.to_lowercase()))
        });
    }
    Ok(items)
}

/// 搜索 BOOTH：`?q=...` 返回商品列表（历史入口，无过滤轴、不走缓存）。
///
/// 保留原签名供既有调用方（按名搜索链路）使用；新代码请用 [`search_with`]。
pub fn search_booth(client: &Client, query: &str) -> Result<Vec<SearchItem>, String> {
    let r = get(
        client,
        &format!("https://booth.pm/ja/items?q={}", urlencode(query)),
        default_headers(),
    )
    .map_err(|e| format!("search request: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("search status {}", r.status()));
    }
    let html = r.text().map_err(|e| format!("search body: {e}"))?;
    Ok(parse_search_html(&html))
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

    /// 历史行为必须逐字节保持：只传 q 时 URL 与旧实现完全一致。
    #[test]
    fn build_url_matches_legacy_for_plain_query() {
        let q = SearchQuery::simple("メカ");
        assert_eq!(
            build_search_url(&q),
            "https://booth.pm/ja/items?q=%E3%83%A1%E3%82%AB"
        );
    }

    #[test]
    fn build_url_appends_axes() {
        let q = SearchQuery {
            query: "衣装".into(),
            sort: Some("new".into()),
            adult: Some("exclude".into()),
            min_price: Some(1000),
            max_price: Some(5000),
            in_stock: true,
            tags: vec!["VRChat".into()],
            category: Some("3D衣装".into()),
            page: 2,
            ..Default::default()
        };
        let url = build_search_url(&q);
        for frag in [
            "q=%E8%A1%A3%E8%A3%85",
            "sort=new",
            "adult=exclude",
            "min_price=1000",
            "max_price=5000",
            "is_sold_out=false",
            "tags[]=VRChat",
            "page=2",
        ] {
            assert!(url.contains(frag), "缺少 {frag}：{url}");
        }
    }

    /// 站点在 popularity 下忽略 page，翻页永远回第一页——传它等于自欺。
    #[test]
    fn popularity_never_sends_page() {
        let q = SearchQuery {
            query: "x".into(),
            sort: Some("popularity".into()),
            page: 3,
            ..Default::default()
        };
        let url = build_search_url(&q);
        assert!(!url.contains("page="), "popularity 不得传 page：{url}");
    }

    #[test]
    fn empty_query_yields_bare_base() {
        let url = build_search_url(&SearchQuery::default());
        assert_eq!(url, "https://booth.pm/ja/items");
    }

    #[test]
    fn exclude_matches_name_and_shop_case_insensitively() {
        // 排除是本地后置过滤：BOOTH 侧无此参数，语义必须由我们保证。
        // 这里直接验过滤判据本身，不依赖 HTML 窗口切分（那是另一组测试的职责）。
        let keep = |name: &str, shop: &str, exclude: &[&str]| {
            let hay = format!("{name} {shop}").to_lowercase();
            !exclude
                .iter()
                .any(|w| !w.trim().is_empty() && hay.contains(&w.to_lowercase()))
        };
        assert!(!keep("VRoid 衣装", "shopA", &["vroid"]), "应命中排除词");
        assert!(keep("Kipfel 衣装", "shopA", &["vroid"]), "不该误伤");
        assert!(!keep("衣装", "vroidshop", &["VRoid"]), "店名也应参与匹配");
        assert!(keep("任意", "任意", &[""]), "空排除词不得过滤一切");
    }
}
