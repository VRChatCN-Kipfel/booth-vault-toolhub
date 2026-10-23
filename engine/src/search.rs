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

/// 决定实际发送给站点的排序，并在必要时给出说明。
///
/// **核心规则**：`popularity` 下站点忽略 `page`——实测 page=1/2/3 返回的首条商品
/// ID 完全相同（2280136），而 `new` 下 p1/p2 首条不同。因此翻页时（`page > 1`）
/// 必须切到 `new`，否则翻页永远返回第一页（这是「以为在翻页、其实没翻」的经典
/// 静默错误）。第一页保留 `popularity`，尊重用户的人气排序意图。
///
/// 返回 `(实际 sort, 说明)`；说明为 `Some` 时调用方应告知用户「已改用新着排序」。
pub fn effective_sort(sort: &str, page: u32) -> (String, Option<String>) {
    if sort == "popularity" && page > 1 {
        (
            "new".to_string(),
            Some("人气排序下站点忽略翻页，已改用新着排序取第 2 页起".to_string()),
        )
    } else {
        (sort.to_string(), None)
    }
}

/// 检索结果的**统一输出契约**（CLI / MCP / GUI 三端共用）。
///
/// 存在的理由：三端曾各自拼 JSON，字段数不一致（CLI 9 个、MCP 6 个），而
/// `skills/booth/SKILL.md` 却声称「三端完全一致」——agent 按文档取字段会拿到 null。
/// 现在字段在本结构里单点定义，两端只做 `serde_json::to_string_pretty`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SearchRow {
    pub id: String,
    pub name: String,
    /// 价格（日元整数；解析失败为 -1）。
    pub price: i64,
    /// 价格展示串（含货币符号与千分位，来自站点原文）。
    pub price_text: String,
    pub shop: String,
    pub brand: String,
    /// 分类原始值（站点内部 id 或 slug）。
    pub category: String,
    /// 分类展示名（日语原文）。
    pub category_name: String,
    pub thumbnail: String,
}

impl SearchRow {
    /// 从解析结果构造（单一实现，避免两端各写一份映射）。
    pub fn from_item(it: &SearchItem) -> Self {
        Self {
            id: it.id.clone(),
            name: it.name.clone(),
            price: it.price,
            price_text: it.price_text.clone(),
            shop: it.shop.clone(),
            brand: it.brand.clone(),
            category: it.category.clone(),
            category_name: it.category_name.clone(),
            thumbnail: it.thumbnail.clone(),
        }
    }
}

/// 检索响应信封（统一契约）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchOutput {
    pub command: String,
    pub mode: String,
    pub query: String,
    /// 实际请求的 URL（便于排查参数是否生效）。
    pub url: String,
    /// 本页解析到的条数（已应用本地 exclude 过滤）。
    pub total: usize,
    /// 实际返回给调用方的条数。
    pub shown: usize,
    /// 用户请求的页码。
    pub page: u32,
    /// 是否还有下一页（站点是否给出更靠后页的迹象）。
    pub has_next: bool,
    /// 人气排序翻页被迫改新着时的说明。
    pub sort_note: Option<String>,
    pub items: Vec<SearchRow>,
}

/// 组装统一输出。
///
/// `parsed_count` 是**本地 exclude 过滤之前**站点本页返回的条数。`has_next` 必须基于它：
/// 历史实现用过滤后的 `items.len() >= 60` 判断，于是「整页都被 exclude 排除」时会
/// 报 `total=0, has_next=false`，谎称没有下一页。
pub fn into_output(
    q: &SearchQuery,
    items: &[SearchItem],
    parsed_count: usize,
    limit: usize,
) -> SearchOutput {
    let u = build_search_url(q);
    let (_sort, note) = effective_sort(q.sort.as_deref().unwrap_or(""), q.page);
    SearchOutput {
        command: "search".to_string(),
        mode: "query".to_string(),
        query: q.query.clone(),
        url: u,
        total: items.len(),
        shown: items.len().min(limit),
        page: q.page,
        // 站点单页固定返回 60 条；达到上限说明后面还有。
        has_next: parsed_count >= 60,
        sort_note: note,
        items: items.iter().take(limit).map(SearchRow::from_item).collect(),
    }
}

/// 拼装 BOOTH 检索 URL。
///
/// 规则均来自 2026-10 的实网对照实验（不是推断）：
///   - **仅在售**用 `in_stock=true`。`is_sold_out=false` **完全无效**：实测它与
///     无意义参数 `fake_param=zzz` 的结果总数完全相同（4058），而 `in_stock=true`
///     会真实收窄（4038）。
///   - **翻页排序要切换**：`popularity` 下站点忽略 `page`（实测 page=1/2/3 首条 ID
///     恒为 2280136），必须改按 `new` 取页并发 `page`，否则翻页永远返回第一页。
///     由 [`effective_sort`] 决定实际发送的 sort，本函数只负责拼。
///   - 查询词放在 `q`，标签用复数键 `tags[]`，价格用 `min_price`/`max_price`。
pub fn build_search_url(q: &SearchQuery) -> String {
    let mut parts: Vec<String> = Vec::new();
    let (sort, sort_note) = effective_sort(q.sort.as_deref().unwrap_or(""), q.page);
    if !sort.is_empty() {
        parts.push(format!("sort={}", urlencode(&sort)));
    }
    let _ = sort_note;
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
        // 实测：in_stock=true 生效；is_sold_out=false 与假参数同效（被忽略）。
        parts.push("in_stock=true".to_string());
    }
    for t in &q.tags {
        if !t.trim().is_empty() {
            parts.push(format!("tags[]={}", urlencode(t)));
        }
    }
    if q.page > 1 {
        parts.push(format!("page={}", q.page));
    }
    // **端点必须是 `/ja/search/<查询词>`（词写在路径里）**，不能用 `/ja/items?q=`。
    //
    // 实测（2026-10）：
    //   - `/ja/items?q=X&page=2` → 302 到 `/ja/search/X`，**page 被丢弃**，
    //     page=2/3 返回的首条 ID 完全相同（9024927），即永远停在第一页；
    //   - `/ja/search/X?page=2` → 首条 ID 变为 5479202，**真正翻页**；
    //   - `/ja/search?q=X` → **404**（该端点不接受 `q` 参数，词必须在路径里）。
    // 用错端点的后果是「以为在翻页、其实没有」——且若同时给出 sort_note 说明，
    // 就变成带说明的欺骗，比静默失效更糟。
    let base = if q.query.trim().is_empty() {
        "https://booth.pm/ja/search".to_string()
    } else {
        format!("https://booth.pm/ja/search/{}", urlencode(&q.query))
    };
    if parts.is_empty() {
        base
    } else {
        format!("{base}?{}", parts.join("&"))
    }
}

/// 判断响应体是否为「未登录」伪装页。
///
/// 未登录时 BOOTH 会返回 **HTTP 200** 的登录页——状态码检查拦不住它。
/// 识别依据（任一）：
///   - 跳转/表单指向 `/users/sign_in`；
///   - 含 `_plaza_session` 缺失提示类文案（保守起见不依赖文案，只用上方 URL 特征）。
///
/// **不能**把「解析出 0 条」当作登录页：合法的零结果搜索同样解析出 0 条，
/// 误判会把正常空结果报成错误。
fn looks_like_login_page(body: &str) -> bool {
    let head: String = body.chars().take(4096).collect();
    head.contains("/users/sign_in") || head.contains("users/sign_in")
}

/// 带过滤轴的搜索（走内容缓存，列表页 TTL 10 分钟）。
pub fn search_with(
    client: &Client,
    q: &SearchQuery,
    no_cache: bool,
) -> Result<Vec<SearchItem>, String> {
    let url = build_search_url(q);
    let cache = crate::cache::shared();
    let scope = crate::cache::scope_of(crate::session::session_cookie().as_deref());
    let ttl = if no_cache { 0 } else { crate::cache::TTL_PAGE };
    let r = crate::http::get_cached(
        client,
        &url,
        default_headers(),
        cache.as_deref(),
        scope,
        ttl,
    )
    .map_err(|e| format!("search request: {e}"))?;
    if !(200..300).contains(&r.status) {
        return Err(format!("search status {}", r.status));
    }
    let html = String::from_utf8_lossy(&r.body);
    // 登录页伪装成 200，必须在**解析与入库之前**拦住。
    if looks_like_login_page(&html) {
        // 若这条是从缓存读出来的（历史版本写入的登录页），驱逐它，
        // 否则用户会一直读到登录页直到 TTL 过期——这正是评审指出的危害。
        if let Some(c) = cache.as_deref() {
            c.remove(scope, &url);
        }
        return Err("search: 返回登录页而非搜索结果，请到设置页填写有效 Cookie".to_string());
    }
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

/// 带过滤轴的搜索，返回**原始条数**（exclude 过滤前）。
///
/// `into_output` 的 `has_next` 需要它：过滤后的条数无法反映站点是否还有下一页。
pub fn search_with_count(
    client: &Client,
    q: &SearchQuery,
    no_cache: bool,
) -> Result<(Vec<SearchItem>, usize), String> {
    let url = build_search_url(q);
    let cache = crate::cache::shared();
    let scope = crate::cache::scope_of(crate::session::session_cookie().as_deref());
    let ttl = if no_cache { 0 } else { crate::cache::TTL_PAGE };
    let r = crate::http::get_cached(
        client,
        &url,
        default_headers(),
        cache.as_deref(),
        scope,
        ttl,
    )
    .map_err(|e| format!("search request: {e}"))?;
    if !(200..300).contains(&r.status) {
        return Err(format!("search status {}", r.status));
    }
    let html = String::from_utf8_lossy(&r.body);
    if looks_like_login_page(&html) {
        if let Some(c) = cache.as_deref() {
            c.remove(scope, &url);
        }
        return Err("search: 返回登录页而非搜索结果，请到设置页填写有效 Cookie".to_string());
    }
    let parsed = parse_search_html(&html);
    let parsed_count = parsed.len();
    let mut items = parsed;
    if !q.exclude.is_empty() {
        items.retain(|it| {
            let hay = format!("{} {}", it.name, it.shop).to_lowercase();
            !q.exclude
                .iter()
                .any(|w| !w.trim().is_empty() && hay.contains(&w.to_lowercase()))
        });
    }
    Ok((items, parsed_count))
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

/// 商品的规范名与说明文。
///
/// **必须一起返回**：两者同源于一次 `fetch_item`（阻塞 GET）。拆成两个闭包会让同一
/// id 被抓两次；而说明文只在 `DescWeight` 开启时参与评分，等到那时再补抓，等于把
/// 选优成本翻倍。
#[derive(Debug, Clone, Default)]
pub struct CanonInfo {
    pub name: String,
    pub description: String,
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
    info: impl Fn(&str) -> CanonInfo,
    weight: crate::score::DescWeight,
) -> ResolveOutcome {
    let mut tried: Vec<String> = Vec::new();
    let mut hit: Option<NamedHit> = None;

    // 第一轮：原候选。
    for q in candidates {
        tried.push(q.clone());
        if let Some(h) = pick_once(client, q, resource_names, &info, weight) {
            hit = Some(h);
            break;
        }
    }

    // 第二轮：kana 读音变体。**只在第一轮全部落空后**进入。
    if hit.is_none() && max_variants > 0 {
        let variants = crate::clean::with_kana_variants(candidates.to_vec(), max_variants);
        for q in variants.into_iter().filter(|c| !candidates.contains(c)) {
            tried.push(q.clone());
            if let Some(h) = pick_once(client, &q, resource_names, &info, weight) {
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
    info: &impl Fn(&str) -> CanonInfo,
    weight: crate::score::DescWeight,
) -> Option<NamedHit> {
    let results = search_booth(client, q).ok()?;
    if results.is_empty() {
        return None;
    }
    pick_from_results(q, &results, resource_names, info, weight)
}

/// 评分决策本身——与取数（`search_booth` / `fetch_item`）分开，好让这层能用假数据
/// 直接测：否则「开关开了但说明文没接进去」这种失效只能靠真实网络才能暴露。
fn pick_from_results(
    q: &str,
    results: &[SearchItem],
    resource_names: Option<&[String]>,
    info: &impl Fn(&str) -> CanonInfo,
    weight: crate::score::DescWeight,
) -> Option<NamedHit> {
    let items: Vec<crate::score::Item> = results
        .iter()
        .map(|r| crate::score::Item {
            id: r.id.clone(),
            name: r.name.clone(),
            price: r.price,
        })
        .collect();
    // 本轮的规范名/说明文缓存：`info` 可能是非缓存闭包（三端各自注入），
    // 而开启 desc 时同一 id 会先取说明文、再被评分取规范名——不在这里收拢，
    // 一次选优就会对每个候选抓两遍。
    let memo: std::cell::RefCell<std::collections::HashMap<String, CanonInfo>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    let cached = |id: &str| -> CanonInfo {
        if let Some(hit) = memo.borrow().get(id) {
            return hit.clone();
        }
        let v = info(id);
        memo.borrow_mut().insert(id.to_string(), v.clone());
        v
    };
    let canonical = |id: &str| cached(id).name;
    let (picked, ambiguous) = if weight.is_on() {
        let descs: Vec<crate::score::DescriptionText> = items
            .iter()
            .map(|it| crate::score::DescriptionText::new(it.id.clone(), cached(&it.id).description))
            .collect();
        crate::score::score_and_pick_with_desc(
            q,
            &items,
            &descs,
            weight,
            false,
            canonical,
            resource_names,
        )
    } else {
        crate::score::score_and_pick(q, &items, false, canonical, resource_names)
    };
    let p = picked?;
    if ambiguous {
        return None;
    }
    let cn = cached(&p.id).name;
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

    /// 历史行为必须逐字节保持：只传 q 时 URL 与旧实现完全一致。
    #[test]
    fn build_url_matches_legacy_for_plain_query() {
        let q = SearchQuery::simple("メカ");
        // 端点必须是 /ja/search/<词>（词在路径里）——/ja/items?q= 会丢 page，
        // /ja/search?q= 则直接 404。见 build_search_url 的实测注释。
        assert_eq!(
            build_search_url(&q),
            "https://booth.pm/ja/search/%E3%83%A1%E3%82%AB"
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
            // 查询词在**路径**里，不是 q 参数。
            "/ja/search/%E8%A1%A3%E8%A3%85",
            "sort=new",
            "adult=exclude",
            "min_price=1000",
            "max_price=5000",
            "in_stock=true",
            "tags[]=VRChat",
            "page=2",
        ] {
            assert!(url.contains(frag), "缺少 {frag}：{url}");
        }
        assert!(
            !url.contains("q="),
            "查询词必须走路径，q= 参数形式在 /ja/search 上会 404：{url}"
        );
        // 对照：这个参数名**无效**（实测与假参数同效），不得出现在 URL 里。
        assert!(
            !url.contains("is_sold_out"),
            "in_stock 必须用 in_stock=true，is_sold_out=false 被站点忽略：{url}"
        );
    }

    /// 实网实测（2026-10）：`popularity` 下 page=1/2/3 首条商品 ID 恒为同一个，
    /// 说明站点忽略 page；而 `new` 下 p1/p2 首条不同。故翻页时必须切到 `new`。
    #[test]
    fn popularity_paging_switches_to_new() {
        let q = SearchQuery {
            query: "x".into(),
            sort: Some("popularity".into()),
            page: 3,
            ..Default::default()
        };
        let url = build_search_url(&q);
        assert!(
            url.contains("sort=new"),
            "popularity 翻页必须切到 new，否则永远返回第一页：{url}"
        );
        assert!(url.contains("page=3"), "切换排序后必须真的发 page：{url}");
        // 端点必须是能翻页的那一个：用 /ja/items?q= 时 page 会被站点丢弃
        // （实测 page=2/3 首条 ID 相同），翻页形同虚设。
        assert!(
            url.starts_with("https://booth.pm/ja/search/"),
            "必须用 /ja/search/<词> 端点，否则 page 被丢弃：{url}"
        );
        // 第一页保留用户的人气排序意图。
        let q1 = SearchQuery {
            query: "x".into(),
            sort: Some("popularity".into()),
            page: 1,
            ..Default::default()
        };
        let url1 = build_search_url(&q1);
        assert!(
            url1.contains("sort=popularity"),
            "第一页应保留 popularity：{url1}"
        );
    }

    #[test]
    fn effective_sort_notes_the_switch() {
        let (s, note) = effective_sort("popularity", 2);
        assert_eq!(s, "new");
        assert!(note.is_some(), "切换排序必须给出说明");
        let (s1, n1) = effective_sort("popularity", 1);
        assert_eq!(s1, "popularity");
        assert!(n1.is_none(), "第一页无需说明");
        let (s2, n2) = effective_sort("new", 5);
        assert_eq!(s2, "new");
        assert!(n2.is_none());
    }

    #[test]
    fn empty_query_yields_bare_base() {
        let url = build_search_url(&SearchQuery::default());
        // 无查询词时是搜索首页（词在路径里时才是 /ja/search/<词>）。
        assert_eq!(url, "https://booth.pm/ja/search");
    }

    /// 登录页伪装成 200，必须能被识别出来（状态码检查拦不住它）。
    ///
    /// 这是评审 P0-2 的原始危害：登录页 200 会被当作正常响应解析（0 条）并入库，
    /// 之后同一 URL 在 TTL 内一直返回登录页。
    #[test]
    fn detects_login_page_disguised_as_200() {
        let login = r#"<html><body><a href="/users/sign_in">sign in</a></body></html>"#;
        assert!(looks_like_login_page(login));
        // 正常搜索页不得误判。
        let ok = r#"<li data-product-id="1" data-product-name="x" data-product-price="0"></li>"#;
        assert!(!looks_like_login_page(ok));
        // **合法零结果不得误判成登录页**：这是保守判据的关键。
        let empty =
            r#"<html><body><div class="no-result">該当する商品がありません</div></body></html>"#;
        assert!(!looks_like_login_page(empty), "零结果不得被当成登录页");
    }

    /// `has_next` 必须用**过滤前**的原始条数：整页被 exclude 排除时
    /// 过滤后为 0，但站点确实还有下一页。
    #[test]
    fn has_next_uses_prefilter_count() {
        let q = SearchQuery {
            query: "x".into(),
            exclude: vec!["everything".into()],
            ..Default::default()
        };
        // 解析到 60 条但全部被 exclude 掉。
        let items: Vec<SearchItem> = Vec::new();
        let out = into_output(&q, &items, 60, 20);
        assert_eq!(out.total, 0, "过滤后应为 0");
        assert!(
            out.has_next,
            "站点本页给了 60 条（到上限），必须报还有下一页"
        );
        // 对照：站点只给 10 条 → 没有下一页。
        let out2 = into_output(&q, &items, 10, 20);
        assert!(!out2.has_next);
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

    fn si(id: &str, name: &str, price: i64) -> SearchItem {
        SearchItem {
            id: id.to_string(),
            name: name.to_string(),
            price,
            price_text: String::new(),
            brand: String::new(),
            shop: String::new(),
            category: String::new(),
            category_name: String::new(),
            thumbnail: String::new(),
        }
    }

    /// 接线死的：开关打开但说明文没接进去，用户看到的仍是「只按标题」的结果。
    ///
    /// 分值构造（`star tiara`，两词各 ≥3 字符）：
    ///   首位「Star Tiara」        名称命中 +100、词级 +40、次序 +10        = 150
    ///   次位「Star Tiara Gold」   名称命中 +100、词级 +40、次序 +8、
    ///                             规范名命中 +100                          = 248
    /// 基线分差 98（≥30，不判歧义），基线赢家是次位；只有把说明文接进去，首位才能
    /// 靠 +100 加成反超（250 > 248）。故本测试同时钉住「开关生效」与「生效方向」。
    #[test]
    fn desc_weight_actually_changes_the_winner() {
        let results = [
            si("1111111", "Star Tiara", 0),
            si("2222222", "Star Tiara Gold", 0),
        ];
        let info = |id: &str| match id {
            // 规范名不含查询词 → 无规范名命中；说明文命中两个词 → +100。
            "1111111" => CanonInfo {
                name: "Other Item".to_string(),
                description: "star tiara".to_string(),
            },
            _ => CanonInfo {
                name: "Star Tiara Gold".to_string(),
                description: String::new(),
            },
        };
        let q = "star tiara";

        let off = pick_from_results(q, &results, None, &info, crate::score::DescWeight::OFF)
            .expect("关闭时基线赢家应命中");
        assert_eq!(off.item.id, "2222222", "关闭时说明文不得参与");

        let on_weight = crate::score::DescWeight::new(50, 2).expect("50×2 在上限内");
        let on = pick_from_results(q, &results, None, &info, on_weight).expect("开启时也应命中");
        assert_eq!(
            on.item.id, "1111111",
            "说明文已接进评分时首位应反超；仍是 2222222 说明开关没接线"
        );
    }

    /// 每个候选只解析一次：说明文与规范名同源，分两次取就会把选优成本翻倍
    /// （#71 的 memo 就是为这个问题加的，接线时最先会退化的也是这里）。
    #[test]
    fn desc_weight_does_not_double_canonical_lookups() {
        use std::cell::Cell;
        let results = [
            si("1111111", "StarTiara Base", 0),
            si("2222222", "StarTiara Variant", 0),
            si("3333333", "StarTiara Deluxe", 0),
        ];
        let calls = Cell::new(0usize);
        let info = |id: &str| {
            calls.set(calls.get() + 1);
            CanonInfo {
                name: match id {
                    "1111111" => "StarTiara Base",
                    "2222222" => "StarTiara Variant",
                    _ => "StarTiara Deluxe",
                }
                .to_string(),
                description: "StarTiara 対応".to_string(),
            }
        };
        let w = crate::score::DescWeight::new(50, 2).expect("50×2 在上限内");
        let _ = pick_from_results("StarTiara", &results, None, &info, w);
        assert_eq!(
            calls.get(),
            results.len(),
            "开启说明文加权后每个候选仍只应解析一次，实际 {}",
            calls.get()
        );
    }

    /// 关闭时必须与「完全没有说明文」的旧路径逐位一致。
    #[test]
    fn desc_off_matches_plain_scoring() {
        let results = [
            si("1111111", "Star Tiara", 0),
            si("2222222", "Star Tiara Gold", 0),
        ];
        let info = |id: &str| CanonInfo {
            name: match id {
                "1111111" => "Other Item",
                _ => "Star Tiara Gold",
            }
            .to_string(),
            description: "star tiara".to_string(),
        };
        let q = "star tiara";
        let items: Vec<crate::score::Item> = results
            .iter()
            .map(|r| crate::score::Item {
                id: r.id.clone(),
                name: r.name.clone(),
                price: r.price,
            })
            .collect();
        let plain = crate::score::score_and_pick(q, &items, false, |id| info(id).name, None);
        assert!(!plain.1, "本用例的基线不应歧义，否则测试前提不成立");

        let got = pick_from_results(q, &results, None, &info, crate::score::DescWeight::OFF);
        assert_eq!(
            got.map(|h| h.item.id),
            plain.0.map(|i| i.id.clone()),
            "关闭时不得改变任何选优结果"
        );
    }
}
