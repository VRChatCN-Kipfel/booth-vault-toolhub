//! 便携模式支持：检测 portable.txt 标记、计算 WebView2 数据目录、授权。
//!
//! 便携版（zip 解压即用）：数据与 runtime 全在 exe 目录内，不污染系统。
//! 安装版（MSI/NSIS）：走系统默认数据目录。

use std::path::{Path, PathBuf};

/// 便携版标记文件名（zip 内 exe 旁）。
pub const PORTABLE_MARKER: &str = "portable.txt";

/// 是否便携模式：exe 目录存在 portable.txt。
pub fn is_portable() -> bool {
    current_exe_dir()
        .map(|d| d.join(PORTABLE_MARKER).is_file())
        .unwrap_or(false)
}

/// WebView2 用户数据目录（便携模式：exe 目录下 data/webview）。
pub fn portable_webview_dir() -> Option<PathBuf> {
    if !is_portable() {
        return None;
    }
    Some(current_exe_dir()?.join("data").join("webview"))
}

/// 当前 exe 所在目录。
fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.to_path_buf())
}

/// 幂等授权：确保目录 DACL 含 ALL APPLICATION PACKAGES（S-1-15-2-2 / S-1-15-2-1）。
///
/// Win10 + WebView2 fixed runtime ≥120 的 unpackaged 应用要求该权限，
/// 否则 renderer 在 AppContainer 运行失败（Win11 / 旧 runtime 不受影响）。
/// 便携目录用户可写，无需提权。自检：先 icacls 看输出，缺才授权。
pub fn ensure_webview2_acl(runtime_dir: &Path) {
    if !cfg!(windows) {
        return;
    }
    let check = std::process::Command::new("icacls")
        .arg(runtime_dir)
        .output();
    if let Ok(out) = check {
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("ALL APPLICATION PACKAGES") {
            return; // 已授权，幂等跳过
        }
    }
    for sid in ["*S-1-15-2-2", "*S-1-15-2-1"] {
        let _ = std::process::Command::new("icacls")
            .arg(runtime_dir)
            .arg("/grant")
            .arg(format!("{sid}:(OI)(CI)(RX)"))
            .arg("/q")
            .output();
    }
}
