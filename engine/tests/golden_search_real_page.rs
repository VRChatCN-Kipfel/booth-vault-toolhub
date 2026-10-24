//! 真实 BOOTH 搜索结果页的解析回归（fixture 取自线上，非手写）。
//!
//! **存在的理由**：其余解析测试用的是内联手写 HTML。那能证明代码逻辑自洽，却证明不了
//! 「真实页面能解析」——`data-product-*` 是绑定站点 DOM 的正则，BOOTH 一改版就静默失效，
//! 而手写 fixture 会跟着一起改，测试**永远绿**。AGENTS.md 把它列为血泪坑 #11。
//!
//! 本测试用**线上抓取的原始 HTML**（只裁掉多余卡片，DOM 结构一字未改）作为输入，
//! 断言关键字段取到预期值。BOOTH 若改版，这条会红——这正是它存在的目的。
//!
//! fixture 来源：`https://booth.pm/ja/search/ウィッグ`，2026-10 抓取，原文 60 卡片 /
//! 279KB；保存前 4 个完整卡片（14.4KB）。裁剪已用逐字段比对验证与原文前 4 条完全一致
//! （id / name / price / brand / category 全等）。
//!
//! 与 `golden_search.rs` 的分工：那边锁「与 Python 旧实现逐字节一致」（含 ctx 窗口
//! 重叠导致的跨卡片污染，短 HTML 下必然发生）；这边锁「真实长页面下字段不串」——
//! 真实页面卡片间距 ~3.3KB，远超 ctx 的 300 字符前置窗口，所以**不应有污染**。
//!
//! 更新 fixture 的判据：只有确认 BOOTH 合法改版时才更新，并同步改断言。
//! 不要为了让测试变绿而改 fixture。

use engine::search::{SearchQuery, build_search_url, into_output, parse_search_html};

fn real_page() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/search_real.html"
    ))
    .expect("fixture search_real.html 应存在")
}

#[test]
fn real_page_yields_expected_card_count() {
    let items = parse_search_html(&real_page());
    assert_eq!(items.len(), 4, "fixture 内应为 4 个完整卡片");
}

/// 逐字段断言前 3 条。数值取自线上原文，不是手填的假数据。
#[test]
fn real_page_fields_are_parsed_without_cross_card_bleed() {
    let items = parse_search_html(&real_page());

    // 卡片 1
    assert_eq!(items[0].id, "5603856");
    assert_eq!(items[0].name, "White Wig World（初級編）");
    assert_eq!(items[0].price, 1000);
    assert_eq!(items[0].price_text, "¥ 1,000~");
    assert_eq!(items[0].brand, "whitewigorder");
    assert_eq!(items[0].category, "52");
    assert_eq!(items[0].category_name, "コスプレ小道具");
    assert!(
        items[0].thumbnail.starts_with("https://booth.pximg.net/"),
        "缩略图应取 data-original，实际：{}",
        items[0].thumbnail
    );

    // 卡片 2：**关键断言——不得等于卡片 1 的值**。
    // 短 HTML 下 ctx 窗口重叠会把卡片 1 的字段污染到卡片 2（golden_search.rs 记录的
    // legacy 行为）；真实长页面间距足够，必须没有污染。
    assert_eq!(items[1].id, "6474171");
    assert_eq!(items[1].brand, "aminico");
    assert_ne!(
        items[1].brand, items[0].brand,
        "字段串到上一张卡片了：ctx 窗口重叠回归"
    );
    assert_eq!(items[1].category, "54");
    assert_ne!(items[1].category, items[0].category);

    // 卡片 3
    assert_eq!(items[2].id, "7034550");
    assert_eq!(items[2].price, 7800);
    assert_eq!(items[2].brand, "limdolls");
    assert_eq!(items[2].category, "198");
}

/// 每张卡片的 id 必须唯一且非空——解析器若在真实页面上漏切/重切，这里会暴露。
#[test]
fn real_page_ids_are_unique_and_nonempty() {
    let items = parse_search_html(&real_page());
    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    for id in &ids {
        assert!(!id.is_empty(), "出现空 id");
        assert!(
            id.len() == 7 && id.chars().all(|c| c.is_ascii_digit()),
            "id 应为 7 位数字，实际：{id}"
        );
    }
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "出现重复 id：{ids:?}");
}

/// 真实页面解析出的条目经 `into_output` 后应满足统一契约：
/// 字段齐全、`has_next` 依据过滤前条数、`shown` 受 limit 约束。
#[test]
fn real_page_into_output_matches_contract() {
    let items = parse_search_html(&real_page());
    let q = SearchQuery {
        query: "ウィッグ".into(),
        // `Default` 的 page 是 0（结构体默认值），真实调用方由 CLI/MCP 显式传 1。
        page: 1,
        ..Default::default()
    };
    // 真实站点单页固定 60 条；这里只有 4 条 fixture，故 has_next 为 false。
    let out = into_output(&q, &items, items.len(), 2);

    assert_eq!(out.command, "search");
    assert_eq!(out.mode, "query");
    assert_eq!(out.query, "ウィッグ");
    assert_eq!(out.total, 4);
    assert_eq!(out.shown, 2, "shown 应受 limit 约束");
    assert_eq!(out.page, 1);
    assert!(!out.has_next, "解析 4 条 < 60，不应报还有下一页");
    assert!(out.sort_note.is_none(), "未指定 sort 不应有说明");
    assert_eq!(out.items.len(), 2);

    // 真实检索词必须走路径，不是 q 参数（否则站点丢 page 或 404）。
    assert!(
        out.url.starts_with("https://booth.pm/ja/search/"),
        "端点必须是 /ja/search/<词>：{}",
        out.url
    );
    assert!(!out.url.contains("q="), "不得用 q= 参数形式：{}", out.url);

    // 契约字段名与文档（SKILL.md）必须一致，防两处漂移。
    let json = serde_json::to_value(&out).expect("可序列化");
    let keys: Vec<&str> = json
        .as_object()
        .expect("应为对象")
        .keys()
        .map(|s| s.as_str())
        .collect();
    for f in [
        "command",
        "mode",
        "query",
        "url",
        "total",
        "shown",
        "page",
        "has_next",
        "sort_note",
        "items",
    ] {
        assert!(
            keys.contains(&f),
            "SearchOutput 缺少字段 {f}，实际：{keys:?}"
        );
    }
    assert_eq!(keys.len(), 10, "SearchOutput 字段数应为 10：{keys:?}");
}

/// 端点构造与真实页面对照：真实页面 URL 就是 `/ja/search/<词>`。
#[test]
fn real_page_url_shape_matches_site() {
    let q = SearchQuery::simple("ウィッグ");
    let url = build_search_url(&q);
    assert_eq!(
        url, "https://booth.pm/ja/search/%E3%82%A6%E3%82%A3%E3%83%83%E3%82%B0",
        "抓取 fixture 时用的正是这个 URL"
    );
}
