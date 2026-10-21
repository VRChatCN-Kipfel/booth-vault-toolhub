//! HTTP 内容缓存：以 URL 为键，redb 单文件存储，按资源类型分 TTL。
//!
//! 为什么要有它：`search` 的「按名搜索」链路最坏会对一个文件名跑满 7 层候选，
//! 每层一次检索 + 一次 `fetch_item`；整理上百个无 ID 文件就是数百次重复联网。
//! 缓存把这些重复请求挡在本地，是后续「通用搜索 / 过滤轴 / watch 巡检」的地基。
//!
//! 刻意**不缓存**的东西（做了就是缺陷）：
//!   - 文件下载与封面图：走流式，体积大且必须逐字节到达；缓存它们只会拖慢且吃磁盘。
//!   - 无效响应：只缓存 2xx。登录页伪装、429、5xx 一律不入库，否则错误会被"冻住"。
//!
//! 失败永远是"退化为无缓存"，绝不因为缓存故障拒绝出站——这是缓存该有的姿态。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use redb::{Database, ReadableDatabase, TableDefinition};

/// 表：URL → (写入时刻 unix 秒, 响应体)。
///
/// 时间戳随条目一起存，TTL 判定在读取时做；redb 没有原生过期，也不该有——
/// 过期策略属于调用方语义，换 TTL 不该需要迁移数据。
const TABLE: TableDefinition<&str, (u64, &[u8])> = TableDefinition::new("http");

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

impl Cache {
    /// 打开或创建缓存库。失败返回 None，调用方据此退化为无缓存。
    pub fn open(path: &Path) -> Option<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // `create` 对「不存在 / 空文件 / 合法库」都可用；损坏文件会报错，
        // 此时删掉重来——缓存是可重建的派生数据，不值得为它让整个工具失败。
        let db = match Database::create(path) {
            Ok(db) => db,
            Err(_) => {
                let _ = std::fs::remove_file(path);
                Database::create(path).ok()?
            }
        };
        Some(Self { db })
    }

    /// 读取未过期的缓存条目。任何错误都当作未命中。
    pub fn get(&self, url: &str, ttl: u64) -> Option<Vec<u8>> {
        if ttl == 0 {
            return None;
        }
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(TABLE).ok()?;
        // `get` 返回的 AccessGuard 是临时值，必须绑定后再取值，否则 body 的借用悬空。
        let guard = table.get(url).ok()??;
        let (stamped, body) = guard.value();
        let fresh = now_secs().saturating_sub(stamped) < ttl;
        if !fresh {
            return None;
        }
        Some(body.to_vec())
    }

    /// 写入缓存。超过 TTL 的旧条目留着无害（读取时被 TTL 挡掉），
    /// 但同一 URL 重复写入会覆盖，不会无限增长。
    pub fn put(&self, url: &str, body: &[u8]) {
        let Ok(txn) = self.db.begin_write() else {
            return;
        };
        {
            let Ok(mut table) = txn.open_table(TABLE) else {
                return;
            };
            let _ = table.insert(url, (now_secs(), body));
        }
        let _ = txn.commit();
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
        c.put("https://booth.pm/x", b"hello");
        assert_eq!(
            c.get("https://booth.pm/x", TTL_PAGE),
            Some(b"hello".to_vec())
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn ttl_zero_disables_reads() {
        let p = tmp("ttl0");
        let c = Cache::open(&p).expect("open");
        c.put("https://booth.pm/x", b"hello");
        // ttl=0 是「绕过缓存」的表达，必须不命中。
        assert_eq!(c.get("https://booth.pm/x", 0), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn expired_entries_miss() {
        let p = tmp("expired");
        let c = Cache::open(&p).expect("open");
        c.put("https://booth.pm/x", b"hello");
        assert_eq!(
            c.get("https://booth.pm/x", TTL_PAGE),
            Some(b"hello".to_vec())
        );
        // 秒级时间戳：等满 2 秒再断言，跨过整秒边界，避免在 1.0~1.1s 之间的抖动。
        std::thread::sleep(std::time::Duration::from_millis(2100));
        assert_eq!(c.get("https://booth.pm/x", 1), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn miss_on_unknown_key() {
        let p = tmp("miss");
        let c = Cache::open(&p).expect("open");
        assert_eq!(c.get("https://booth.pm/nope", TTL_PAGE), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn clear_empties() {
        let p = tmp("clear");
        let c = Cache::open(&p).expect("open");
        c.put("https://booth.pm/x", b"hello");
        c.clear();
        assert_eq!(c.get("https://booth.pm/x", TTL_PAGE), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn corrupt_file_is_recreated() {
        let p = tmp("corrupt");
        std::fs::write(&p, b"not a redb database").unwrap();
        // 损坏的缓存是可重建的派生数据：必须能自愈，而不是让工具整体失败。
        let c = Cache::open(&p).expect("should recreate");
        c.put("https://booth.pm/x", b"ok");
        assert_eq!(c.get("https://booth.pm/x", TTL_PAGE), Some(b"ok".to_vec()));
        let _ = std::fs::remove_file(&p);
    }
}
