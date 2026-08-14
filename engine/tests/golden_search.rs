//! 与 Python 旧实现输出 diff 对拍（golden tests）：搜索 HTML 解析。
//!
//! 黄金数据由 legacy `booth_name_search.py::search_booth` 实机运行生成（2026-08-15），
//! 输入为 `fixtures/search.html`（与 Rust 完全同一文件，保证字节一致）。
//! 注意：Python 的 ctx 窗口（id 前 300 / 后 4000 字符）在短 HTML 下互相重叠，
//! 导致跨卡片字段污染——此为 legacy 真实行为（血泪坑 #14），必须原样复刻。

use engine::search::parse_search_html;

fn fixture() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/search.html"
    ))
    .expect("fixture exists")
}

#[test]
fn golden_search_parse() {
    let items = parse_search_html(&fixture());
    assert_eq!(items.len(), 2);

    // 卡片 1（自己的字段）
    assert_eq!(items[0].id, "7437723");
    assert_eq!(items[0].name, "Lunaria Paper Fan & More");
    assert_eq!(items[0].price, 0);
    assert_eq!(items[0].price_text, "¥ 0");
    assert_eq!(items[0].brand, "No39 Studio");
    assert_eq!(items[0].shop, "No39");
    assert_eq!(items[0].category, "3Dモデル");
    assert_eq!(items[0].category_name, "3Dモデル");
    assert_eq!(items[0].thumbnail, "https://booth.pximg.net/a1b2c3_100.jpg");

    // 卡片 2：id/name/price 为本卡片，其余字段被 ctx 窗口捕获的卡片 1 覆盖。
    assert_eq!(items[1].id, "8888888");
    assert_eq!(items[1].name, "媭女エフェクト"); // &#23213; 数字实体解码
    assert_eq!(items[1].price, 800);
    assert_eq!(items[1].price_text, "¥ 0"); // 污染：来自卡片 1
    assert_eq!(items[1].brand, "No39 Studio"); // 污染
    assert_eq!(items[1].shop, "No39"); // 污染
    assert_eq!(items[1].category, "3Dモデル"); // 污染
    assert_eq!(items[1].category_name, "3Dモデル"); // 污染
    assert_eq!(items[1].thumbnail, "https://booth.pximg.net/a1b2c3_100.jpg"); // 污染
}
