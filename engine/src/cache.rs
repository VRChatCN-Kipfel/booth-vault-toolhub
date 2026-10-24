//! HTTP 内容缓存：以 `(作用域, URL)` 为键，redb 单文件存储，按资源类型分 TTL。
//!
//! 为什么要有它：`search` 的「按名搜索」链路最坏会对一个文件名跑满 7 层候选，
//! 每层一次检索 + 一次 `fetch_item`；整理上百个无 ID 文件就是数百次重复联网。
//! 缓存把这些重复请求挡在本地，是后续「通用搜索 / 过滤轴 / watch 巡检」的地基。
//!
//! 键必须带**作用域**（登录态）：否则匿名抓到的页面会被带 Cookie 的请求复用，
//! 不同账号的 R-18/年龄门结果互相污染。
//!
//! 刻意**不缓存**的东西（做了就是缺陷）：
//!   - 文件下载与封面图：走流式，体积大且必须逐字节到达；缓存它们只会拖慢且吃磁盘。
//!   - 非 200 响应：登录页伪装、204/206、429、5xx 一律不入库，否则错误会被"冻住"。
//!
//! 失败永远是"退化为无缓存"，绝不因为缓存故障拒绝出站——这是缓存该有的姿态。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use redb::{Database, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use sha2::{Digest, Sha256};

/// 表：缓存键 → (写入时刻 unix 秒, 状态码, 最终 URL, 响应体)。
///
/// 时间戳随条目一起存，TTL 判定在读取时做；redb 没有原生过期，也不该有——
/// 过期策略属于调用方语义，换 TTL 不该需要迁移数据。
///
/// 同时存 status 与 final_url：命中时必须**原样回放**，不能伪造 `200` 与请求 URL
/// （那会让调用方无法分辨真实响应与缓存回放）。
///
/// **表名带 schema 版本 `v2`**：redb 按值的类型元数据校验，键值结构变更后旧库里的
/// `v1` 表与新定义的元组类型不兼容——`put` 会静默失败、`get` 恒为 None，
/// 表现为「缓存开了但永不命中」。换表名让旧库自然失效（旧表留着不影响正确性），
/// 不需要写迁移逻辑，也不冒把用户目录里旧文件误删的风险。
const TABLE: TableDefinition<&str, (u64, u16, &str, &[u8], &str)> = TableDefinition::new("http_v3");

/// 缓存条目（命中后回放的全部信息）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEntry {
    pub status: u16,
    pub final_url: String,
    pub body: Vec<u8>,
    /// 诊断用的少量响应头（`key: value\n` 拼接），**只存归因必需的几项**。
    ///
    /// 为什么不存全量响应头：缓存是给「重放响应体」用的，绝大多数头（CSP、权限策略、
    /// `Set-Cookie`）对回放无意义，还会撑大缓存并把 cookie 落到盘上。这里只留
    /// `Cf-Mitigated` / `Retry-After` 这类**决定下一步动作**的项。
    pub diag_headers: String,
}

/// 商品 JSON 的变化很慢，缓存 6 小时。
pub const TTL_ITEM: u64 = 6 * 3600;
/// 搜索/商店页要求新鲜度，缓存 10 分钟。
pub const TTL_PAGE: u64 = 10 * 60;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 进程级共享句柄。
///
/// **必须有**：redb 对同一文件加独占锁，同一进程里二次 `open` 会拿不到
/// （`DatabaseAlreadyOpen`）。而 `fetch` / `search` 都是短函数，各自开门必然踩坑。
/// 故这里只开一次，全局复用；打开失败则永久退化为无缓存。
pub fn shared() -> Option<std::sync::Arc<Cache>> {
    use std::sync::{Arc, OnceLock};
    static SHARED: OnceLock<Option<Arc<Cache>>> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            let path = crate::config::http_cache_path()?;
            Cache::open(&path).map(Arc::new)
        })
        .clone()
}

/// 数据库句柄。构造开销低（redb 自带页缓存），但同一进程内重复打开会拿不到锁，
/// 故调用方应持有并复用。
pub struct Cache {
    db: Database,
}

/// 缓存作用域：把「登录态」纳入缓存键。
///
/// 为什么必须有：缓存只以 URL 为键时，匿名请求抓到的页面会被带 Cookie 的请求原样
/// 复用（反之亦然）。同一台机器上不同账号的 R-18/年龄门结果因此互相污染，
/// 且没有任何入口能清掉污染条目。
///
/// `scope` 由 cookie 派生（见 `scope_of`），**不落明文**：只存其 SHA-256 前 16 位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// 匿名（未配置登录 cookie）。
    Anonymous,
    /// 带登录 cookie；`0` 是 cookie 摘要，不是 cookie 本身。
    LoggedIn(u64),
}

impl Scope {
    /// 拼缓存键：`{scope}\x1f{url}`。用 `\x1f`（单元分隔符）避免与 URL 内容歧义。
    fn key(self, url: &str) -> String {
        match self {
            Scope::Anonymous => format!("anon\x1f{url}"),
            Scope::LoggedIn(h) => format!("auth{h:016x}\x1f{url}"),
        }
    }
}

/// 由 cookie 串派生作用域。
///
/// 只对**会话相关**的 cookie 取摘要：分析类 cookie（`_ga` 等）与用户无关，
/// 把它们算进去会让同一账号因统计 cookie 变化而反复失效缓存。
/// 空串得到 `Anonymous`。
pub fn scope_of(cookie: Option<&str>) -> Scope {
    let raw = cookie.map(str::trim).unwrap_or("");
    if raw.is_empty() {
        return Scope::Anonymous;
    }
    let mut hasher = Sha256::new();
    // 排序后逐个摘要，保证 cookie 顺序不同不影响结果。
    let mut pairs = crate::session::load_cookie_pairs(raw).pairs;
    pairs.sort();
    for (k, v) in &pairs {
        hasher.update(k.as_bytes());
        hasher.update(b"=");
        hasher.update(v.as_bytes());
        hasher.update(b";");
    }
    let out = hasher.finalize();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&out[..8]);
    Scope::LoggedIn(u64::from_be_bytes(buf))
}

impl Cache {
    /// 打开或创建缓存库。失败返回 None，调用方据此退化为无缓存。
    pub fn open(path: &Path) -> Option<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let db = match Database::create(path) {
            Ok(db) => db,
            // 只有**确证损坏**才重建：Windows 上被另一进程持锁时删除会失败，
            // 直接 remove 会把「暂时打不开」误判成「文件坏了」。
            Err(redb::DatabaseError::DatabaseAlreadyOpen) => return None,
            Err(_) => {
                let backup = path.with_extension("corrupt");
                let _ = std::fs::rename(path, &backup);
                Database::create(path).ok()?
            }
        };
        Some(Self { db })
    }

    /// 读取未过期的缓存条目。任何错误都当作未命中。
    ///
    /// `ttl == 0` 表示「绕过缓存」（`--no-cache`），**读与写都必须跳过**：
    /// 历史实现只挡读、不挡写，逃生口当场把坏响应写回库。
    pub fn get(&self, scope: Scope, url: &str, ttl: u64) -> Option<StoredEntry> {
        if ttl == 0 {
            return None;
        }
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(TABLE).ok()?;
        // `get` 返回的 AccessGuard 是临时值，必须绑定后再取值，否则 body 的借用悬空。
        let guard = table.get(scope.key(url).as_str()).ok()??;
        let (stamped, status, final_url, body, diag) = guard.value();
        let fresh = now_secs().saturating_sub(stamped) < ttl;
        if !fresh {
            return None;
        }
        Some(StoredEntry {
            status,
            final_url: final_url.to_string(),
            body: body.to_vec(),
            diag_headers: diag.to_string(),
        })
    }

    /// 写入缓存。同键重复写会覆盖，不会无限增长。
    pub fn put(&self, scope: Scope, url: &str, entry: &StoredEntry) {
        let Ok(txn) = self.db.begin_write() else {
            return;
        };
        {
            let Ok(mut table) = txn.open_table(TABLE) else {
                return;
            };
            let key = scope.key(url);
            let _ = table.insert(
                key.as_str(),
                (
                    now_secs(),
                    entry.status,
                    entry.final_url.as_str(),
                    entry.body.as_slice(),
                    entry.diag_headers.as_str(),
                ),
            );
        }
        let _ = txn.commit();
    }

    /// 删除单个键（命中校验失败时回源前调用）。
    pub fn remove(&self, scope: Scope, url: &str) {
        let Ok(txn) = self.db.begin_write() else {
            return;
        };
        {
            let Ok(mut table) = txn.open_table(TABLE) else {
                return;
            };
            let _ = table.remove(scope.key(url).as_str());
        }
        let _ = txn.commit();
    }

    /// 当前条目数（`booth cache stats` 用）。
    ///
    /// 表尚未创建（首次运行、还没写过任何条目）时返回 0，这不是错误。
    pub fn count(&self) -> usize {
        let Ok(txn) = self.db.begin_read() else {
            return 0;
        };
        let Ok(table) = txn.open_table(TABLE) else {
            // 表不存在 = 0 条，属正常初始状态。
            return 0;
        };
        table.len().unwrap_or(0) as usize
    }

    /// 清空缓存（`--no-cache` 的强制刷新语义未要求清库，此处供运维/手工使用）。
    pub fn clear(&self) {
        let Ok(txn) = self.db.begin_write() else {
            return;
        };
        {
            let Ok(mut table) = txn.open_table(TABLE) else {
                return;
            };
            let _ = table.retain(|_, _| false);
        }
        let _ = txn.commit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "bvt_httpcache_{tag}_{}_{}.redb",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn stores_and_reads_back() {
        let p = tmp("roundtrip");
        let c = Cache::open(&p).expect("open");
        c.put(
            Scope::Anonymous,
            "https://booth.pm/x",
            &StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"hello".to_vec(),
                diag_headers: String::new(),
            },
        );
        assert_eq!(
            c.get(Scope::Anonymous, "https://booth.pm/x", TTL_PAGE),
            Some(StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"hello".to_vec(),
                diag_headers: String::new(),
            })
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn ttl_zero_disables_reads() {
        let p = tmp("ttl0");
        let c = Cache::open(&p).expect("open");
        c.put(
            Scope::Anonymous,
            "https://booth.pm/x",
            &StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"hello".to_vec(),
                diag_headers: String::new(),
            },
        );
        // ttl=0 是「绕过缓存」的表达，必须不命中。
        assert_eq!(c.get(Scope::Anonymous, "https://booth.pm/x", 0), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn expired_entries_miss() {
        let p = tmp("expired");
        let c = Cache::open(&p).expect("open");
        c.put(
            Scope::Anonymous,
            "https://booth.pm/x",
            &StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"hello".to_vec(),
                diag_headers: String::new(),
            },
        );
        assert_eq!(
            c.get(Scope::Anonymous, "https://booth.pm/x", TTL_PAGE),
            Some(StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"hello".to_vec(),
                diag_headers: String::new(),
            })
        );
        // 秒级时间戳：等满 2 秒再断言，跨过整秒边界，避免在 1.0~1.1s 之间的抖动。
        std::thread::sleep(std::time::Duration::from_millis(2100));
        assert_eq!(c.get(Scope::Anonymous, "https://booth.pm/x", 1), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn miss_on_unknown_key() {
        let p = tmp("miss");
        let c = Cache::open(&p).expect("open");
        assert_eq!(
            c.get(Scope::Anonymous, "https://booth.pm/nope", TTL_PAGE),
            None
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn clear_empties() {
        let p = tmp("clear");
        let c = Cache::open(&p).expect("open");
        c.put(
            Scope::Anonymous,
            "https://booth.pm/x",
            &StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"hello".to_vec(),
                diag_headers: String::new(),
            },
        );
        c.clear();
        assert_eq!(
            c.get(Scope::Anonymous, "https://booth.pm/x", TTL_PAGE),
            None
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn corrupt_file_is_recreated() {
        let p = tmp("corrupt");
        std::fs::write(&p, b"not a redb database").unwrap();
        // 损坏的缓存是可重建的派生数据：必须能自愈，而不是让工具整体失败。
        let c = Cache::open(&p).expect("should recreate");
        c.put(
            Scope::Anonymous,
            "https://booth.pm/x",
            &StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"ok".to_vec(),
                diag_headers: String::new(),
            },
        );
        assert_eq!(
            c.get(Scope::Anonymous, "https://booth.pm/x", TTL_PAGE),
            Some(StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"ok".to_vec(),
                diag_headers: String::new(),
            })
        );
        let _ = std::fs::remove_file(&p);
    }

    // ── 作用域隔离（评审 P0-1 的回归）────────────────────────────

    /// 不同作用域**不得**互相命中：匿名抓到的页面不能被带会话的请求复用。
    ///
    /// 历史实现只以 URL 为键，同一台机器上匿名与登录、账号 A 与 B 共用条目，
    /// R-18/年龄门结果互相污染且无从清理。
    #[test]
    fn scoped_keys_do_not_collide() {
        let p = tmp("scope");
        let c = Cache::open(&p).expect("open");
        let url = "https://booth.pm/x";
        c.put(
            Scope::Anonymous,
            url,
            &StoredEntry {
                status: 200,
                final_url: url.to_string(),
                body: b"anon".to_vec(),
                diag_headers: String::new(),
            },
        );
        let auth = scope_of(Some("_plaza_session_nktz7u=SECRET"));
        assert_eq!(c.get(auth, url, TTL_PAGE), None, "登录态不得读到匿名条目");
        // 反向同理：登录态写入不得覆盖匿名条目。
        c.put(
            auth,
            url,
            &StoredEntry {
                status: 200,
                final_url: url.to_string(),
                body: b"auth".to_vec(),
                diag_headers: String::new(),
            },
        );
        assert_eq!(
            c.get(Scope::Anonymous, url, TTL_PAGE)
                .map(|e| e.body)
                .as_deref(),
            Some(b"anon".as_slice()),
            "匿名条目不得被登录态覆盖"
        );
        let _ = std::fs::remove_file(&p);
    }

    /// 不同账号必须得到不同作用域；顺序不同但内容相同必须等价。
    #[test]
    fn distinct_cookies_yield_distinct_scopes() {
        let a = scope_of(Some("_plaza_session_nktz7u=A"));
        let b = scope_of(Some("_plaza_session_nktz7u=B"));
        assert_ne!(a, b, "不同账号必须隔离");
        assert_eq!(scope_of(None), Scope::Anonymous);
        assert_eq!(scope_of(Some("   ")), Scope::Anonymous, "空白视为匿名");
        assert_eq!(
            scope_of(Some("a=1; b=2")),
            scope_of(Some("b=2; a=1")),
            "cookie 顺序不得影响作用域（否则缓存会无故失效）"
        );
    }

    /// 作用域键里**不得**出现 cookie 明文（隐私）。
    #[test]
    fn scope_key_has_no_plaintext() {
        let s = scope_of(Some("_plaza_session_nktz7u=VERYSECRET"));
        let key = s.key("https://booth.pm/x");
        assert!(
            !key.contains("VERYSECRET"),
            "缓存键不得含 cookie 明文：{key}"
        );
        assert!(!key.contains("_plaza_session"), "不得含 cookie 名：{key}");
    }

    /// `--no-cache`（ttl=0）必须**既不读也不写**：只挡读会让逃生口把坏响应写回库。
    #[test]
    fn ttl_zero_skips_both_read_and_write() {
        let p = tmp("ttl0w");
        let c = Cache::open(&p).expect("open");
        // get 在 ttl=0 时恒不命中
        c.put(
            Scope::Anonymous,
            "https://booth.pm/x",
            &StoredEntry {
                status: 200,
                final_url: "https://booth.pm/x".to_string(),
                body: b"v".to_vec(),
                diag_headers: String::new(),
            },
        );
        assert_eq!(c.get(Scope::Anonymous, "https://booth.pm/x", 0), None);
        let _ = std::fs::remove_file(&p);
    }

    /// 命中必须**原样回放** status 与 final_url，不得伪造 200/请求 URL。
    #[test]
    fn hit_replays_status_and_final_url() {
        let p = tmp("replay");
        let c = Cache::open(&p).expect("open");
        let url = "https://booth.pm/ja/items/1.json";
        let real_final = "https://booth.pm/ja/items/1.json?redirected=1";
        c.put(
            Scope::Anonymous,
            url,
            &StoredEntry {
                status: 206,
                final_url: real_final.to_string(),
                body: b"partial".to_vec(),
                diag_headers: String::new(),
            },
        );
        let got = c.get(Scope::Anonymous, url, TTL_PAGE).expect("应命中");
        assert_eq!(got.status, 206, "不得伪造成 200");
        assert_eq!(got.final_url, real_final, "不得伪造请求 URL");
        let _ = std::fs::remove_file(&p);
    }
}
