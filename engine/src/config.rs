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

/// 应用级配置（TOML）。CLI / MCP / GUI 共用这一份。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 显式代理地址；空则走环境变量/系统默认。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// 是否启用代理。`Some(false)` 强制直连（忽略 HTTPS_PROXY / 系统代理）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_enabled: Option<bool>,
    /// BOOTH 下载根目录（路径参数化，禁止硬编码）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_root: Option<String>,
    /// 网络限速区间（秒），0.5~0.8 为官方基线，三端统一不得绕过。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_secs: Option<f64>,
    /// BOOTH 登录 Cookie（仅用户目录；CLI `--cookie` / MCP 参数优先）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
}

/// GUI 设置页载荷（camelCase，与 Tauri invoke 对齐）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettings {
    pub booth_root: String,
    pub proxy: bool,
    pub proxy_url: String,
    pub cookie: String,
}

/// 用户目录配置文件名。
const CONFIG_FILENAME: &str = "config.toml";
/// 应用目录配置文件名。
const LOCAL_CONFIG_FILENAME: &str = "config.toml";

/// 是否强制直连（设置页关掉代理）。
pub fn proxy_disabled(config: &AppConfig) -> bool {
    config.proxy_enabled == Some(false)
}

/// 解析代理三态：配置文件 > 环境变量 `HTTPS_PROXY`（无缺省回退）> None。
/// `proxy_enabled = false` 时返回 None（调用方应 `no_proxy()`）。
pub fn resolve_proxy(config: &AppConfig) -> Option<String> {
    if proxy_disabled(config) {
        return None;
    }
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

/// Cookie：命令行 / MCP / GUI 传参优先，否则读配置。
pub fn resolve_cookie(arg: Option<&str>, config: &AppConfig) -> Option<String> {
    if let Some(c) = arg.map(str::trim).filter(|s| !s.is_empty()) {
        return Some(c.to_string());
    }
    config
        .cookie
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// 把已加载的配置投影成 GUI 设置页状态。
pub fn gui_settings_from_config(cfg: &AppConfig) -> GuiSettings {
    GuiSettings {
        booth_root: cfg.download_root.clone().unwrap_or_default(),
        proxy: cfg.proxy_enabled.unwrap_or(true),
        proxy_url: cfg.proxy.clone().unwrap_or_default(),
        cookie: cfg.cookie.clone().unwrap_or_default(),
    }
}

/// 用 GUI 设置覆盖配置中的对应字段（保留 rate_limit_secs 等未暴露项）。
pub fn apply_gui_settings(cfg: &mut AppConfig, g: &GuiSettings) {
    cfg.download_root = nonempty_opt(&g.booth_root);
    cfg.proxy_enabled = Some(g.proxy);
    cfg.proxy = nonempty_opt(&g.proxy_url);
    cfg.cookie = nonempty_opt(&g.cookie);
}

fn nonempty_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
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
    if file_cfg.proxy_enabled.is_some() {
        cfg.proxy_enabled = file_cfg.proxy_enabled;
    }
    if file_cfg.download_root.is_some() {
        cfg.download_root = file_cfg.download_root;
    }
    if file_cfg.rate_limit_secs.is_some() {
        cfg.rate_limit_secs = file_cfg.rate_limit_secs;
    }
    if file_cfg.cookie.is_some() {
        cfg.cookie = file_cfg.cookie;
    }
}

/// 写入指定路径（测试 / 用户目录共用）。
pub fn save_config_to(cfg: &AppConfig, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let text = toml::to_string_pretty(cfg).map_err(|e| format!("序列化配置失败: {e}"))?;
    std::fs::write(path, text).map_err(|e| format!("写入配置失败: {e}"))
}

/// 写入用户目录 `config.toml`（GUI 保存设置走这里，CLI/MCP 下次启动能读到）。
pub fn save_user_config(cfg: &AppConfig) -> Result<(), String> {
    let dir = user_config_dir().ok_or_else(|| "无法解析用户配置目录".to_string())?;
    save_config_to(cfg, &dir.join(CONFIG_FILENAME))
}

/// 默认限速间隔（0.5~0.8s，三端统一，防触发风控）。
pub fn default_rate_limit_secs() -> f64 {
    0.8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 隔离环境变量：测完必须还原，不得把调用者的代理清掉。
    fn with_env_vars(pairs: &[(&str, Option<&str>)], f: impl FnOnce()) {
        use std::sync::{Mutex, OnceLock};
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let prev: Vec<(&str, Option<String>)> = pairs
            .iter()
            .map(|(k, _)| (*k, std::env::var(k).ok()))
            .collect();
        // SAFETY: 持锁串行执行，无并发竞争。
        unsafe {
            for (k, v) in pairs {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
        f();
        // SAFETY: 还原测试前的值（有则写回，无则删除）。
        unsafe {
            for (k, v) in prev {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    fn with_env_var(key: &str, value: Option<&str>, f: impl FnOnce()) {
        with_env_vars(&[(key, value)], f);
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
        with_env_vars(&[("HTTPS_PROXY", None), ("https_proxy", None)], || {
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

    #[test]
    fn proxy_disabled_wins_over_url_and_env() {
        with_env_var("HTTPS_PROXY", Some("http://env.example:3128"), || {
            let cfg = AppConfig {
                proxy: Some("http://cfg.example:8080".to_string()),
                proxy_enabled: Some(false),
                ..AppConfig::default()
            };
            assert!(proxy_disabled(&cfg));
            assert_eq!(resolve_proxy(&cfg), None);
        });
    }

    #[test]
    fn resolve_cookie_arg_beats_config() {
        let cfg = AppConfig {
            cookie: Some("from_file=1".to_string()),
            ..AppConfig::default()
        };
        assert_eq!(
            resolve_cookie(Some("from_arg=2"), &cfg).as_deref(),
            Some("from_arg=2")
        );
        assert_eq!(
            resolve_cookie(Some("  "), &cfg).as_deref(),
            Some("from_file=1")
        );
        assert_eq!(resolve_cookie(None, &cfg).as_deref(), Some("from_file=1"));
    }

    #[test]
    fn gui_settings_roundtrip() {
        let mut cfg = AppConfig {
            rate_limit_secs: Some(0.8),
            ..AppConfig::default()
        };
        let g = GuiSettings {
            booth_root: "D:/BOOTH".to_string(),
            proxy: true,
            proxy_url: "http://127.0.0.1:7890".to_string(),
            cookie: "sid=abc".to_string(),
        };
        apply_gui_settings(&mut cfg, &g);
        assert_eq!(cfg.download_root.as_deref(), Some("D:/BOOTH"));
        assert_eq!(cfg.proxy.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(cfg.cookie.as_deref(), Some("sid=abc"));
        assert_eq!(cfg.proxy_enabled, Some(true));
        assert_eq!(cfg.rate_limit_secs, Some(0.8));
        let back = gui_settings_from_config(&cfg);
        assert_eq!(back.booth_root, "D:/BOOTH");
        assert!(back.proxy);
        assert_eq!(back.cookie, "sid=abc");
    }

    #[test]
    fn save_and_reload_preserves_cookie_and_root() {
        let dir = std::env::temp_dir().join(format!("bvt_save_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let cfg = AppConfig {
            download_root: Some("/tmp/booth".to_string()),
            cookie: Some("k=v".to_string()),
            proxy_enabled: Some(false),
            ..AppConfig::default()
        };
        save_config_to(&cfg, &path).unwrap();
        let mut loaded = AppConfig::default();
        merge_from_file(&mut loaded, &path);
        assert_eq!(loaded.download_root.as_deref(), Some("/tmp/booth"));
        assert_eq!(loaded.cookie.as_deref(), Some("k=v"));
        assert_eq!(loaded.proxy_enabled, Some(false));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
