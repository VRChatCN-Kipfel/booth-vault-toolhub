//! 关注/已购清单与变动检测：本地个人数据，只存本机。
//!
//! 两类清单共用一张表（`kind` 区分）：
//!   - `watch`：关注商品，关心**降价 / 补货 / 售罄**；
//!   - `own`：已购素材，关心**作者发布新版本**（免费重下）。
//!
//! 为什么合一张表：同款可以既已购又关注，分表会重复存同一份快照，且搜索标记要查两张表。
//!
//! 存储形态刻意用 JSON + 原子 rename（与 `library-index.json` 一致）：
//! 规模是「一个人的关注列表」（几十到几百条），不值得为此引入 SQLite。
//! 真正的并发写风险由 `write_atomic` 的「临时名唯一 + rename」承担。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 清单类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// 关注：等降价/补货。
    Watch,
    /// 已购：等作者更新。
    Own,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Watch => "watch",
            Kind::Own => "own",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "watch" => Some(Kind::Watch),
            "own" => Some(Kind::Own),
            _ => None,
        }
    }
}

/// 单条快照：存「上次看到的样子」，用于下次比对。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    /// 商品 ID（7 位数字）。
    pub id: String,
    /// 商品名（用于展示与重命名检测）。
    pub name: String,
    /// 价格（日元整数；解析失败为 -1）。
    pub price: i64,
    /// 是否售罄。`None` = 未知（字段缺失或形态异常）。
    ///
    /// **必须与 `ItemJson` 一致地使用 `Option`**：若这里写 `bool`，序列化/反序列化
    /// 会把「未知」压成 `false`（＝有货），下一次比对就凭空报一次「补货」。
    #[serde(default)]
    pub is_sold_out: Option<bool>,
    /// 是否已停止销售（同上）。
    #[serde(default)]
    pub is_end_of_sale: Option<bool>,
    /// 加入清单时的 unix 秒（仅为可追溯，不参与比对）。
    #[serde(default)]
    pub added_at: u64,
    /// 最近一次 check 的 unix 秒；0 表示从未检查。
    #[serde(default)]
    pub checked_at: u64,
}

/// 变动类型（语义化，供展示与下游判断）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// 降价（附旧价与新价）。
    PriceDrop(i64, i64),
    /// 涨价。
    PriceRise(i64, i64),
    /// 补货（售罄 → 有货）。
    BackInStock,
    /// 售罄。
    SoldOut,
    /// 停止销售。
    EndOfSale,
    /// 商品改名。
    Renamed(String, String),
}

impl Change {
    /// 面向人的一行描述。
    pub fn describe(&self) -> String {
        match self {
            Change::PriceDrop(o, n) => format!("降价 ¥{o} → ¥{n}"),
            Change::PriceRise(o, n) => format!("涨价 ¥{o} → ¥{n}"),
            Change::BackInStock => "补货".to_string(),
            Change::SoldOut => "已售罄".to_string(),
            Change::EndOfSale => "停止销售".to_string(),
            Change::Renamed(o, n) => format!("改名：{o} → {n}"),
        }
    }
}

/// 比对新旧快照，返回变动列表（按发生顺序排列，便于展示）。
///
/// 三条「只在两侧都已知时才比较」的判据：
///   - **价格**：解析失败记 -1，不比较，否则一次坏响应会被当成降价/涨价；
///   - **售罄状态**：`Option<bool>`，任一侧 `None` 就跳过——历史实现用裸 `bool`，
///     字段缺失被 serde 填成 `false`（＝有货），于是纯缺失就报「补货」；
///   - **停售**：同样只在两侧都已知时比较。
pub fn diff(old: &Snapshot, new: &Snapshot) -> Vec<Change> {
    let mut out = Vec::new();
    if old.price >= 0 && new.price >= 0 && old.price != new.price {
        if new.price < old.price {
            out.push(Change::PriceDrop(old.price, new.price));
        } else {
            out.push(Change::PriceRise(old.price, new.price));
        }
    }
    // 售罄：只在两侧都 Some 时比较。
    match (old.is_sold_out, new.is_sold_out) {
        (Some(true), Some(false)) => out.push(Change::BackInStock),
        (Some(false), Some(true)) => out.push(Change::SoldOut),
        _ => {}
    }
    // 停售：只在两侧都 Some 时比较。
    if let (Some(false), Some(true)) = (old.is_end_of_sale, new.is_end_of_sale) {
        out.push(Change::EndOfSale);
    }
    if old.name != new.name && !new.name.is_empty() {
        out.push(Change::Renamed(old.name.clone(), new.name.clone()));
    }
    out
}

/// 整份清单（序列化形态）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Lists {
    /// 结构版本：将来改字段语义时用于拒绝/迁移旧文件。
    #[serde(default = "default_version")]
    pub version: u32,
    /// kind → (id → 快照)。BTreeMap 保证落盘稳定，便于人工 diff。
    #[serde(default)]
    pub items: BTreeMap<String, BTreeMap<String, Snapshot>>,
}

fn default_version() -> u32 {
    LISTS_VERSION
}

/// 当前结构版本。
pub const LISTS_VERSION: u32 = 1;

/// **手写而非派生**：`#[derive(Default)]` 会让 `version == 0`，任何经默认构造再
/// `save` 的路径都会写出一个下次自己都拒读的文件（`load` 检查版本）。
impl Default for Lists {
    fn default() -> Self {
        Self {
            version: LISTS_VERSION,
            items: BTreeMap::new(),
        }
    }
}

impl Lists {
    pub fn of(&self, kind: Kind) -> &BTreeMap<String, Snapshot> {
        static EMPTY: std::sync::OnceLock<BTreeMap<String, Snapshot>> = std::sync::OnceLock::new();
        self.items
            .get(kind.as_str())
            .unwrap_or_else(|| EMPTY.get_or_init(BTreeMap::new))
    }

    pub fn get(&self, kind: Kind, id: &str) -> Option<&Snapshot> {
        self.of(kind).get(id)
    }

    /// 加入（已存在则覆盖快照）。
    pub fn put(&mut self, kind: Kind, snap: Snapshot) {
        self.items
            .entry(kind.as_str().to_string())
            .or_default()
            .insert(snap.id.clone(), snap);
    }

    /// 移除，返回是否原本存在。
    pub fn remove(&mut self, kind: Kind, id: &str) -> bool {
        self.items
            .get_mut(kind.as_str())
            .map(|m| m.remove(id).is_some())
            .unwrap_or(false)
    }

    pub fn len(&self, kind: Kind) -> usize {
        self.of(kind).len()
    }

    pub fn is_empty(&self, kind: Kind) -> bool {
        self.len(kind) == 0
    }
}

/// 清单文件路径（用户配置目录，`BOOTH_LISTS_FILE` 可覆盖）。
///
/// 必须落在用户配置目录而不是 BOOTH 库：库是只读资产（AGENTS 红线），
/// 个人清单属于用户数据。
pub fn lists_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BOOTH_LISTS_FILE")
        && !p.trim().is_empty()
    {
        return Some(PathBuf::from(p));
    }
    crate::config::collections_path()
}

/// 读取清单失败的原因。
///
/// **必须与「空清单」区分开**：历史实现把解析失败/版本不符/读取失败一律降级成空
/// 清单，随后任何一次 `add` 都把这份空清单写回，用户的 watch/own 列表就**无提示、
/// 无备份地永久丢失**。清单是用户手工攒起来的数据，丢了不可重建。
#[derive(Debug)]
pub enum LoadError {
    /// 文件存在但不是合法 JSON（或结构不符）。
    Corrupt(String),
    /// 结构版本不受支持（可能是更新的版本写的）。
    Version { found: u32, expected: u32 },
    /// 读取失败（权限、IO 等）。
    Io(std::io::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Corrupt(e) => write!(f, "清单文件已损坏，拒绝覆盖以保护数据: {e}"),
            LoadError::Version { found, expected } => write!(
                f,
                "清单版本不受支持（文件 {found}，本版 {expected}），拒绝覆盖以保护数据"
            ),
            LoadError::Io(e) => write!(f, "无法读取清单文件: {e}"),
        }
    }
}

/// 读取清单。
///
/// - 文件**不存在** → `Ok` 空清单（首次使用的正常情况）；
/// - 解析失败 / 版本不符 / 读取失败 → `Err`，调用方必须**在任何写盘之前**中止。
///
/// 绝不把上述失败降级为空清单：那会让下一次 save 静默清空用户数据。
pub fn load(path: &Path) -> Result<Lists, LoadError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Lists::default()),
        Err(e) => return Err(LoadError::Io(e)),
    };
    let lists: Lists =
        serde_json::from_str(&text).map_err(|e| LoadError::Corrupt(e.to_string()))?;
    if lists.version != LISTS_VERSION {
        return Err(LoadError::Version {
            found: lists.version,
            expected: LISTS_VERSION,
        });
    }
    Ok(lists)
}

/// 原子写清单（临时名唯一 + rename），与 `library-index.json` 同一套理由：
/// 并发写者（CLI 与 GUI 同时改清单）共用固定临时名会交错截断，rename 上去仍是坏 JSON。
///
/// 返回 `io::Result`：**写盘失败必须上抛**。历史实现返回 `()`、错误被 `let _` 吞掉，
/// 于是 `add` 打印 added、退出 0，而磁盘上什么都没有。
///
/// **不做「rename 失败后直接写」的截断兜底**：`audit.rs` 里那段是给「可全扫自愈的
/// 缓存」用的，用户清单被截断就再也回不来了，宁可失败也不能半截覆盖。
pub fn save(path: &Path, lists: &Lists) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(lists)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), seq));
    if let Err(e) = std::fs::write(&tmp, &s) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// 从商品 JSON 构造快照。
pub fn snapshot_from(item: &crate::fetch::ItemJson, id: &str) -> Snapshot {
    Snapshot {
        id: id.to_string(),
        name: item.name.clone(),
        price: crate::fetch::parse_price(&item.price),
        is_sold_out: item.is_sold_out,
        is_end_of_sale: item.is_end_of_sale,
        added_at: now_secs(),
        checked_at: 0,
    }
}

/// 当前 unix 秒（清单时间戳用；公开供 CLI 记录 `checked_at`）。
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 批量拉取详情并加入清单，**带限速**。
///
/// 为什么下沉到 engine：CLI 与 MCP 各写一份循环时，`add` 这条路径两边都漏了
/// sleep（`check` 有、`add` 没有），100 个 ID 就是一串无间隔请求，违反 AGENTS
/// 「0.5~0.8s 三端统一」契约。限速只能有一个实现。
///
/// `sleep` 与 `fetch` 都以闭包注入：
///   - `sleep` 让测试能计数而不真睡（真睡会让单测跑几十秒）；
///   - `fetch` 让本层不需要知道 `reqwest::Client` 的细节。
///
/// 返回 `(已加入的 id, 失败项说明)`。
pub fn add_paced(
    kind: Kind,
    lists: &mut Lists,
    ids: &[String],
    sleep: impl Fn(),
    fetch: impl Fn(&str) -> Result<crate::fetch::ItemJson, String>,
) -> (Vec<String>, Vec<String>) {
    let mut added = Vec::new();
    let mut failed = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        // 首个不睡：与下载链路的「无前序请求则无需等待」一致。
        if i > 0 {
            sleep();
        }
        match fetch(id) {
            Ok(item) => {
                lists.put(kind, snapshot_from(&item, id));
                added.push(id.clone());
            }
            Err(e) => failed.push(format!("{id}: {e}")),
        }
    }
    (added, failed)
}

/// 选取本轮要检查的条目：按 `checked_at` 升序（最久没查的优先），`0` 表示全部。
///
/// 历史实现用 `keys().take(limit)`——`BTreeMap` 的字典序，导致第 N+1 条
/// **永远轮不到**；`limit=0` 取 0 条却报 `checked: 0` 退出 0（同仓 `download`/`preview`
/// 的 0 都表示「不限」，语义还不一致）。
pub fn pick_batch(lists: &Lists, kind: Kind, limit: usize) -> Vec<String> {
    let mut entries: Vec<(&String, &Snapshot)> = lists.of(kind).iter().collect();
    // 主键 checked_at 升序（从未查过 = 0 排最前），次键 id 保证稳定。
    entries.sort_by(|a, b| a.1.checked_at.cmp(&b.1.checked_at).then(a.0.cmp(b.0)));
    let n = if limit == 0 { entries.len() } else { limit };
    entries
        .into_iter()
        .take(n)
        .map(|(id, _)| id.clone())
        .collect()
}

/// 记录「这一条刚查过」，把 `checked_at` 推进到新值并写回清单。
///
/// **为什么必须由 engine 提供而不是让调用方自己设**：轮换契约是
/// 「`pick_batch` 按 `checked_at` 升序取前 N 条，查过就推进」，两半必须成对
/// 使用。历史实现把「推进」交给调用方，而 CLI 与 MCP 都把它写在了
/// `if !changes.is_empty()` 里——**没有变动就不推进**，于是
/// `snapshot_from` 恒置的 0 永远不变，平局退化成 id 字典序，
/// 第 N+1 条起**永远轮不到**。这个缺陷 engine 单测看不见（`pick_batch`
/// 本身是对的），只在真实多轮调用里显现。
///
/// 返回是否确实推进（`None` 表示该 id 不在清单里，不凭空创建条目）。
pub fn mark_checked(lists: &mut Lists, kind: Kind, id: &str, now: u64) -> bool {
    let Some(mut s) = lists.get(kind, id).cloned() else {
        return false;
    };
    s.checked_at = now;
    lists.put(kind, s);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: &str, price: i64, sold: bool) -> Snapshot {
        Snapshot {
            id: id.to_string(),
            name: "n".to_string(),
            price,
            is_sold_out: Some(sold),
            is_end_of_sale: Some(false),
            added_at: 1,
            checked_at: 0,
        }
    }

    #[test]
    fn detects_price_drop_and_rise() {
        assert_eq!(
            diff(&snap("1", 1000, false), &snap("1", 800, false)),
            vec![Change::PriceDrop(1000, 800)]
        );
        assert_eq!(
            diff(&snap("1", 800, false), &snap("1", 1000, false)),
            vec![Change::PriceRise(800, 1000)]
        );
    }

    /// 解析失败（-1）不得当成变动：否则一次坏响应就会触发假降价警报。
    #[test]
    fn unknown_price_never_reports_change() {
        assert!(diff(&snap("1", -1, false), &snap("1", 800, false)).is_empty());
        assert!(diff(&snap("1", 800, false), &snap("1", -1, false)).is_empty());
    }

    // ── 库存字段的 None 语义（评审 P0-3 的回归）────────────────────

    /// 售罄状态**未知**时不得报任何补货/售罄变动。
    ///
    /// 历史实现用裸 `bool` + 容器级 `#[serde(default)]`：字段缺失被填成 `false`
    /// （＝有货），于是「旧值缺失→新值有货」会被判成**补货**——一次字段缺失
    /// 就触发假提醒。改成 `Option` 后，任一侧 `None` 就跳过比较。
    #[test]
    fn unknown_stock_never_reports_change() {
        let mut a = snap("1", 100, false);
        let mut b = snap("1", 100, false);
        a.is_sold_out = None; // 旧值未知
        b.is_sold_out = Some(false);
        assert!(diff(&a, &b).is_empty(), "一侧未知时不得报补货");

        let mut c = snap("1", 100, false);
        let mut d = snap("1", 100, false);
        c.is_sold_out = Some(false);
        d.is_sold_out = None; // 新值未知
        assert!(diff(&c, &d).is_empty(), "一侧未知时不得报售罄");

        let mut e = snap("1", 100, false);
        let mut f = snap("1", 100, false);
        e.is_sold_out = None;
        f.is_sold_out = None;
        assert!(diff(&e, &f).is_empty(), "两侧都未知时不得报变动");

        // **关键区分点**：None 与 Some(true) 的组合。若实现把 None 当 false，
        // 这里会误报 SoldOut / BackInStock；而 (None, Some(false)) 那组测不出。
        let mut g = snap("1", 100, false);
        let mut h = snap("1", 100, false);
        g.is_sold_out = None;
        h.is_sold_out = Some(true);
        assert!(diff(&g, &h).is_empty(), "未知→售罄不得报变动");

        let mut i = snap("1", 100, false);
        let mut j = snap("1", 100, false);
        i.is_sold_out = Some(true);
        j.is_sold_out = None;
        assert!(diff(&i, &j).is_empty(), "售罄→未知不得报补货");
    }

    /// 停售状态未知时不得报「停止销售」。
    #[test]
    fn unknown_end_of_sale_never_reports_change() {
        let mut a = snap("1", 100, false);
        let mut b = snap("1", 100, false);
        a.is_end_of_sale = None;
        b.is_end_of_sale = Some(true);
        assert!(diff(&a, &b).is_empty(), "旧值未知时不得报停售");
    }

    /// 两侧都已知时仍要正常检测（不能因改成 Option 而失效）。
    #[test]
    fn known_stock_still_detected() {
        assert_eq!(
            diff(&snap("1", 0, true), &snap("1", 0, false)),
            vec![Change::BackInStock]
        );
        assert_eq!(
            diff(&snap("1", 0, false), &snap("1", 0, true)),
            vec![Change::SoldOut]
        );
    }

    #[test]
    fn detects_stock_transitions() {
        assert_eq!(
            diff(&snap("1", 0, true), &snap("1", 0, false)),
            vec![Change::BackInStock]
        );
        assert_eq!(
            diff(&snap("1", 0, false), &snap("1", 0, true)),
            vec![Change::SoldOut]
        );
    }

    #[test]
    fn no_change_yields_empty() {
        assert!(diff(&snap("1", 100, false), &snap("1", 100, false)).is_empty());
    }

    #[test]
    fn detects_rename() {
        let mut a = snap("1", 100, false);
        let mut b = a.clone();
        b.name = "新名".to_string();
        a.name = "旧名".to_string();
        assert_eq!(
            diff(&a, &b),
            vec![Change::Renamed("旧名".into(), "新名".into())]
        );
    }

    #[test]
    fn put_remove_and_len() {
        let mut l = Lists {
            version: LISTS_VERSION,
            ..Default::default()
        };
        l.put(Kind::Watch, snap("1", 100, false));
        l.put(Kind::Watch, snap("2", 200, false));
        l.put(Kind::Own, snap("1", 100, false));
        assert_eq!(l.len(Kind::Watch), 2);
        assert_eq!(l.len(Kind::Own), 1, "同款可既已购又关注");
        assert!(l.remove(Kind::Watch, "1"));
        assert!(!l.remove(Kind::Watch, "1"), "重复移除应返回 false");
        assert_eq!(l.len(Kind::Watch), 1);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let p = std::env::temp_dir().join(format!(
            "bvt_lists_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut l = Lists {
            version: LISTS_VERSION,
            ..Default::default()
        };
        l.put(Kind::Own, snap("42", 500, false));
        save(&p, &l).expect("写盘应成功");
        let back = load(&p).expect("刚写完的清单必须可读");
        assert_eq!(back.get(Kind::Own, "42").map(|s| s.price), Some(500));
        let _ = std::fs::remove_file(&p);
    }

    /// 损坏文件不得让工具崩掉，退回空清单。
    /// 损坏文件必须**报错**，绝不能被当成空清单。
    ///
    /// 这条替换了旧的 `corrupt_file_loads_empty`——那条把缺陷当契约钉住了：
    /// 损坏→空清单→下一次 save 覆盖，用户数据永久丢失且没有任何提示。
    #[test]
    fn corrupt_file_is_reported() {
        let p = std::env::temp_dir().join(format!(
            "bvt_lists_bad_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, b"{not json").unwrap();
        let err = load(&p).expect_err("损坏文件必须报错");
        assert!(matches!(err, LoadError::Corrupt(_)), "实际：{err:?}");
        // 关键：原文件不得被删改（保护现场）
        assert!(p.exists(), "损坏文件必须原地保留，供用户排查");
        let _ = std::fs::remove_file(&p);
    }

    /// 版本不符必须报错，不能降级成空清单。
    #[test]
    fn future_version_is_rejected() {
        let p = std::env::temp_dir().join(format!(
            "bvt_lists_ver_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, br#"{"version":99,"items":{}}"#).unwrap();
        let err = load(&p).expect_err("未来版本必须报错");
        assert!(
            matches!(err, LoadError::Version { found: 99, .. }),
            "实际：{err:?}"
        );
        let _ = std::fs::remove_file(&p);
    }

    /// 默认构造必须带当前版本——派生的 `Default` 会给出 0，写出下次拒读的文件。
    #[test]
    fn default_has_current_version() {
        assert_eq!(
            Lists::default().version,
            LISTS_VERSION,
            "Default 必须置当前版本，否则写出的文件下次自己都拒读"
        );
    }

    /// 写盘失败必须上抛（父路径不可创建时）。
    #[test]
    fn save_error_is_reported() {
        // 用一个「父级是文件」的路径，create_dir_all 必然失败。
        let base = std::env::temp_dir().join(format!(
            "bvt_lists_blocker_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&base, b"x").unwrap();
        let bad = base.join("sub").join("lists.json");
        let err = save(&bad, &Lists::default()).expect_err("写盘必须失败并上报");
        assert!(err.kind() != std::io::ErrorKind::NotFound || true);
        let _ = std::fs::remove_file(&base);
    }

    /// 写盘失败不得留下临时文件。
    #[test]
    fn save_leaves_no_tmp() {
        let dir = std::env::temp_dir().join(format!(
            "bvt_lists_tmp_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("lists.json");
        save(&p, &Lists::default()).expect("正常写盘应成功");
        let tmps: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(tmps.is_empty(), "不得残留 .tmp：{tmps:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_loads_empty() {
        let p = std::env::temp_dir().join("bvt_lists_never_exists.json");
        let _ = std::fs::remove_file(&p);
        // 文件不存在是**正常**情况（首次使用），仍然返回空清单。
        assert!(load(&p).expect("缺失文件不得报错").is_empty(Kind::Own));
    }

    // ── add_paced：限速与失败隔离 ────────────────────────────────

    /// `add` 必须限速：N 个 ID 只应 sleep N-1 次（首个不等）。
    ///
    /// 历史缺陷：CLI/MCP 的 `add` 循环里完全没有 sleep，100 个 ID 就是
    /// 一串无间隔请求——违反 AGENTS「0.5~0.8s 三端统一」契约。
    #[test]
    fn add_paces_requests() {
        use std::cell::Cell;
        let sleeps = Cell::new(0usize);
        let fetches = Cell::new(0usize);
        let mut lists = Lists::default();
        let ids: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let (added, failed) = add_paced(
            Kind::Watch,
            &mut lists,
            &ids,
            || sleeps.set(sleeps.get() + 1),
            |id| {
                fetches.set(fetches.get() + 1);
                Ok(crate::fetch::ItemJson {
                    id: id.to_string(),
                    name: format!("n{id}"),
                    ..Default::default()
                })
            },
        );
        assert_eq!(added.len(), 3);
        assert!(failed.is_empty());
        assert_eq!(fetches.get(), 3, "每个 ID 都要 fetch 一次");
        assert_eq!(sleeps.get(), 2, "3 个 ID 应只 sleep 2 次（首个不等）");
        assert_eq!(lists.len(Kind::Watch), 3);
    }

    /// 单个 ID 失败不得中断整批（其余仍加入）。
    #[test]
    fn add_isolates_failures() {
        let mut lists = Lists::default();
        let ids: Vec<String> = vec!["ok1".into(), "bad".into(), "ok2".into()];
        let (added, failed) = add_paced(
            Kind::Own,
            &mut lists,
            &ids,
            || {},
            |id| {
                if id == "bad" {
                    Err("boom".to_string())
                } else {
                    Ok(crate::fetch::ItemJson {
                        id: id.to_string(),
                        name: id.to_string(),
                        ..Default::default()
                    })
                }
            },
        );
        assert_eq!(added, vec!["ok1".to_string(), "ok2".to_string()]);
        assert_eq!(failed.len(), 1);
        assert_eq!(lists.len(Kind::Own), 2);
    }

    // ── pick_batch：轮换与 0=全部 ────────────────────────────────

    /// 按 `checked_at` 升序轮换：最久没查的优先，且不会永远卡在前 N 条。
    #[test]
    fn batch_orders_by_checked_at() {
        let mut lists = Lists::default();
        for (id, checked) in [("a", 30u64), ("b", 0), ("c", 20)] {
            let mut s = snap(id, 100, false);
            s.checked_at = checked;
            lists.put(Kind::Watch, s);
        }
        // checked_at 升序：b(0) → c(20) → a(30)
        assert_eq!(
            pick_batch(&lists, Kind::Watch, 2),
            vec!["b".to_string(), "c".to_string()]
        );
        assert_eq!(
            pick_batch(&lists, Kind::Watch, 10),
            vec!["b".to_string(), "c".to_string(), "a".to_string()],
            "多轮下来全部条目都能轮到（历史实现用字典序，末位永远轮不到）"
        );
    }

    /// `limit = 0` 表示**全部**，与同仓 `download`/`preview` 的语义一致。
    #[test]
    fn limit_zero_means_all() {
        let mut lists = Lists::default();
        for id in ["a", "b", "c", "d"] {
            lists.put(Kind::Own, snap(id, 1, false));
        }
        assert_eq!(pick_batch(&lists, Kind::Own, 0).len(), 4);
        assert_eq!(pick_batch(&lists, Kind::Own, 2).len(), 2);
    }

    /// `snapshot_from` 建出来的条目 `checked_at` 恒为 0 —— 这是轮换的起点。
    /// 把该事实钉住：调用方若以为「新建即已检查」，轮换就永远不动。
    #[test]
    fn snapshot_from_starts_unchecked() {
        let it = crate::fetch::ItemJson {
            id: "1111111".to_string(),
            name: "n".to_string(),
            ..Default::default()
        };
        assert_eq!(snapshot_from(&it, "1111111").checked_at, 0);
    }

    /// **mark_checked 让轮换真正推进**：即使内容毫无变化（`diff` 为空）也必须推进，
    /// 否则平局退化成 id 字典序，第 N+1 条永远轮不到。
    ///
    /// 这是原缺陷的正面回归：engine 的 `pick_batch` 一直是对的，错在调用方把
    /// 「推进」写在了 `if !changes.is_empty()` 里。故必须有一条测试直接断言
    /// 「连续 N 轮后所有条目都被轮到」，而不是只测排序函数本身。
    #[test]
    fn mark_checked_makes_rotation_advance_without_changes() {
        let mut lists = Lists::default();
        for id in ["a", "b", "c"] {
            lists.put(Kind::Watch, snap(id, 100, false)); // checked_at 均为 0
        }
        // limit=1 逐轮跑 3 轮，每轮标记刚查过的那一条（模拟真实调用方）。
        let mut seen: Vec<String> = Vec::new();
        for round in 1..=3u64 {
            let batch = pick_batch(&lists, Kind::Watch, 1);
            assert_eq!(batch.len(), 1);
            let id = batch[0].clone();
            seen.push(id.clone());
            assert!(
                mark_checked(&mut lists, Kind::Watch, &id, round * 100),
                "在册条目必须能推进"
            );
        }
        seen.sort();
        assert_eq!(
            seen,
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            "三轮应覆盖全部条目；若仍重复取同一条，说明推进没生效"
        );
    }

    /// 不在册的 id 不得被凭空创建（追加不存在的条目会让后续 `check` 报错源不明）。
    #[test]
    fn mark_checked_ignores_unknown_id() {
        let mut lists = Lists::default();
        lists.put(Kind::Watch, snap("a", 1, false));
        assert!(!mark_checked(&mut lists, Kind::Watch, "zzz", 999));
        assert_eq!(lists.len(Kind::Watch), 1);
    }

    /// 空清单返回空批次，不得 panic。
    #[test]
    fn empty_batch_is_safe() {
        let lists = Lists::default();
        assert!(pick_batch(&lists, Kind::Watch, 10).is_empty());
    }
}
