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
    /// 是否售罄。
    pub is_sold_out: bool,
    /// 是否已停止销售。
    pub is_end_of_sale: bool,
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
/// 判据刻意分开关切：**价格变动只在实际数字变化时报告**（解析失败记 -1 时不比较，
/// 否则一次解析失败会被当成降价/涨价，产生假警报）。
pub fn diff(old: &Snapshot, new: &Snapshot) -> Vec<Change> {
    let mut out = Vec::new();
    if old.price >= 0 && new.price >= 0 && old.price != new.price {
        if new.price < old.price {
            out.push(Change::PriceDrop(old.price, new.price));
        } else {
            out.push(Change::PriceRise(old.price, new.price));
        }
    }
    if old.is_sold_out && !new.is_sold_out {
        out.push(Change::BackInStock);
    } else if !old.is_sold_out && new.is_sold_out {
        out.push(Change::SoldOut);
    }
    if !old.is_end_of_sale && new.is_end_of_sale {
        out.push(Change::EndOfSale);
    }
    if old.name != new.name && !new.name.is_empty() {
        out.push(Change::Renamed(old.name.clone(), new.name.clone()));
    }
    out
}

/// 整份清单（序列化形态）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Lists {
    /// 结构版本：将来改字段语义时用于拒绝/迁移旧文件。
    #[serde(default = "default_version")]
    pub version: u32,
    /// kind → (id → 快照)。BTreeMap 保证落盘稳定，便于人工 diff。
    #[serde(default)]
    pub items: BTreeMap<String, BTreeMap<String, Snapshot>>,
}

fn default_version() -> u32 {
    1
}

/// 当前结构版本。
pub const LISTS_VERSION: u32 = 1;

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

/// 读取清单。文件不存在返回空清单；损坏则返回空（并让调用方有机会提示）。
pub fn load(path: &Path) -> Lists {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Lists {
            version: LISTS_VERSION,
            ..Default::default()
        };
    };
    match serde_json::from_str::<Lists>(&text) {
        Ok(l) if l.version == LISTS_VERSION => l,
        // 版本不符或解析失败：当作空清单而不是崩掉。清单可重建，
        // 但**不静默删除**原文件——保留现场，用户可自行排查。
        _ => Lists {
            version: LISTS_VERSION,
            ..Default::default()
        },
    }
}

/// 原子写清单（临时名唯一 + rename），与 `library-index.json` 同一套理由：
/// 并发写者（CLI 与 GUI 同时改清单）共用固定临时名会交错截断，rename 上去仍是坏 JSON。
pub fn save(path: &Path, lists: &Lists) {
    let Ok(s) = serde_json::to_string_pretty(lists) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), seq));
    if std::fs::write(&tmp, &s).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::write(path, &s);
        let _ = std::fs::remove_file(&tmp);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: &str, price: i64, sold: bool) -> Snapshot {
        Snapshot {
            id: id.to_string(),
            name: "n".to_string(),
            price,
            is_sold_out: sold,
            is_end_of_sale: false,
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
        save(&p, &l);
        let back = load(&p);
        assert_eq!(back.get(Kind::Own, "42").map(|s| s.price), Some(500));
        let _ = std::fs::remove_file(&p);
    }

    /// 损坏文件不得让工具崩掉，退回空清单。
    #[test]
    fn corrupt_file_loads_empty() {
        let p = std::env::temp_dir().join(format!(
            "bvt_lists_bad_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&p, b"{not json").unwrap();
        assert!(load(&p).is_empty(Kind::Watch));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_file_loads_empty() {
        let p = std::env::temp_dir().join("bvt_lists_never_exists.json");
        let _ = std::fs::remove_file(&p);
        assert!(load(&p).is_empty(Kind::Own));
    }
}
