//! 应用配置：配置文件加载 + 代理三态裁决。
//!
//! 代理优先级（AGENTS.md 网络规则，禁止硬编码个人地址）：
//!   配置文件 `proxy`（用户目录 > 应用目录）> 环境变量 `HTTPS_PROXY` > 系统默认。
//!
//! 配置文件为 TOML，两处支持：
//!
//!   - 用户目录：`{dirs::config_dir()}/booth-vault-toolhub/config.toml`
//!   - 应用目录：`{current_exe 同目录或 CWD}/config.toml`
//!
//! 两处同时存在时用户目录优先；字段级合并。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 应用级配置（TOML）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 显式代理地址；空则走环境变量/系统默认。
    pub proxy: Option<String>,
    /// BOOTH 下载根目录（路径参数化，禁止硬编码）。
    pub download_root: Option<String>,
    /// 网络限速区间（秒），0.5~0.8 为官方基线，三端统一不得绕过。
    pub rate_limit_secs: Option<f64>,
}

/// 用户目录配置文件名。
const CONFIG_FILENAME: &str = "config.toml";
/// 应用目录配置文件名。
const LOCAL_CONFIG_FILENAME: &str = "config.toml";

/// 解析代理三态：配置文件 > 环境变量 `HTTPS_PROXY`（无缺省回退）> None。
pub fn resolve_proxy(config: &AppConfig) -> Option<String> {
    if let Some(p) = config.proxy.as_ref() {
        let p = p.trim();
        if !p.is_empty() {
            return Some(p.to_string());
        }
    }
    let env = std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .unwrap_or_default();
    let env = env.trim();
    if !env.is_empty() {
        Some(env.to_string())
    } else {
        None
    }
}

/// 加载配置：用户目录优先，应用目录兜底，字段级合并。
pub fn load_config() -> AppConfig {
    let mut cfg = AppConfig::default();
    if let Some(dir) = local_config_dir() {
        let path = dir.join(LOCAL_CONFIG_FILENAME);
        merge_from_file(&mut cfg, &path);
    }
    if let Some(dir) = user_config_dir() {
        let path = dir.join(CONFIG_FILENAME);
        merge_from_file(&mut cfg, &path);
    }
    cfg
}

/// 用户级配置目录（`dirs::config_dir()/booth-vault-toolhub`）。
fn user_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("booth-vault-toolhub"))
}

/// 应用级配置目录：优先当前可执行文件所在目录，回退工作目录。
fn local_config_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .or_else(|| std::env::current_dir().ok())
}

/// 若文件存在则解析 TOML 并字段级合并（覆盖已设置的字段）。
fn merge_from_file(cfg: &mut AppConfig, path: &Path) {
    if !path.is_file() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(file_cfg) = toml::from_str::<AppConfig>(&text) else {
        return;
    };
    if file_cfg.proxy.is_some() {
        cfg.proxy = file_cfg.proxy;
    }
    if file_cfg.download_root.is_some() {
        cfg.download_root = file_cfg.download_root;
    }
    if file_cfg.rate_limit_secs.is_some() {
        cfg.rate_limit_secs = file_cfg.rate_limit_secs;
    }
}

/// 默认限速间隔（0.5~0.8s，三端统一，防触发风控）。
pub fn default_rate_limit_secs() -> f64 {
    0.8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 隔离环境变量操作（Rust 2024 中 set_var/remove_var 为 unsafe）。
    /// 用全局互斥锁串行化 env 测试，避免并行测试互相污染。
    fn with_env_var(key: &str, value: Option<&str>, f: impl FnOnce()) {
        use std::sync::{Mutex, OnceLock};
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        // SAFETY: 持锁串行执行，无并发竞争。
        unsafe {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        f();
        // SAFETY: 同上，测试结束后清理恢复。
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn proxy_from_config_wins() {
        let cfg = AppConfig {
            proxy: Some("http://cfg.example:8080".to_string()),
            ..AppConfig::default()
        };
        assert_eq!(
            resolve_proxy(&cfg),
            Some("http://cfg.example:8080".to_string())
        );
    }

    #[test]
    fn proxy_empty_config_falls_to_env() {
        with_env_var("HTTPS_PROXY", Some("http://env.example:3128"), || {
            let cfg = AppConfig::default();
            assert_eq!(
                resolve_proxy(&cfg),
                Some("http://env.example:3128".to_string())
            );
        });
    }

    #[test]
    fn proxy_none_when_no_source() {
        with_env_var("HTTPS_PROXY", None, || {
            let cfg = AppConfig::default();
            assert_eq!(resolve_proxy(&cfg), None);
        });
    }

    #[test]
    fn config_file_merge() {
        let mut cfg = AppConfig::default();
        let dir = std::env::temp_dir().join(format!("bvt_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "proxy = \"http://file.example:9090\"\n").unwrap();
        merge_from_file(&mut cfg, &path);
        assert_eq!(cfg.proxy.as_deref(), Some("http://file.example:9090"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_config_overrides_local_config() {
        let dir = std::env::temp_dir().join(format!("bvt_config_order_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let local = dir.join("local.toml");
        let user = dir.join("user.toml");
        std::fs::write(&local, "proxy = \"http://local.example:8080\"\n").unwrap();
        std::fs::write(&user, "proxy = \"http://user.example:8080\"\n").unwrap();

        let mut cfg = AppConfig::default();
        merge_from_file(&mut cfg, &local);
        merge_from_file(&mut cfg, &user);

        assert_eq!(cfg.proxy.as_deref(), Some("http://user.example:8080"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_config_is_noop() {
        let mut cfg = AppConfig::default();
        merge_from_file(&mut cfg, Path::new("nonexistent_config.toml"));
        assert!(cfg.proxy.is_none());
        assert!(cfg.download_root.is_none());
    }
}
