// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

/// 便携模式 WebView2 初始化：
/// 1. 探测 exe 旁的 fixed runtime 文件夹（含 msedgewebview2.exe 取最高版），设 env var
/// 2. 若为 Win10 + runtime ≥120，幂等授权（icacls）
fn setup_webview2() {
    if std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_some() {
        return; // Tauri fixedRuntime 模式会自己设置，避免覆盖
    }
    // 便携模式：探测 exe 旁的 WebView2 fixed runtime。
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
    else {
        return;
    };
    if let Some(runtime) = find_fixed_runtime(&exe_dir) {
        unsafe {
            std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", &runtime);
        }
        // 授权（仅 Win10 + 新 runtime 需要；幂等）
        gui_lib::portable::ensure_webview2_acl(&runtime);
    }
}

/// 扫描目录下的 WebView2 fixed runtime 文件夹，返回含 msedgewebview2.exe 的最高版本路径。
fn find_fixed_runtime(dir: &PathBuf) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(String, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        if !name.starts_with("Microsoft.WebView2.FixedVersionRuntime.") {
            continue;
        }
        if !path.join("msedgewebview2.exe").is_file() {
            continue;
        }
        let Some(ver) = name.strip_prefix("Microsoft.WebView2.FixedVersionRuntime.") else {
            continue;
        };
        if best.as_ref().is_none_or(|(b, _)| ver > b.as_str()) {
            best = Some((ver.to_string(), path));
        }
    }
    best.map(|(_, p)| p)
}

fn main() {
    #[cfg(windows)]
    setup_webview2();
    gui_lib::run()
}
