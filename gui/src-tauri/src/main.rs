// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
fn setup_fixed_webview2_runtime() {
    if std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_some() {
        return;
    }
    // 便携模式：探测 exe 旁的 WebView2 fixed version runtime 文件夹
    // （如 Microsoft.WebView2.FixedVersionRuntime.*.x64/），
    // 含 msedgewebview2.exe 才使用，否则回退系统 WebView2。
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
    else {
        return;
    };
    let runtime = find_fixed_runtime(&exe_dir);
    if let Some(runtime) = runtime {
        unsafe {
            std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", runtime);
        }
    }
}

/// 扫描目录下的 WebView2 fixed runtime 文件夹，返回含 msedgewebview2.exe 的最高版本路径。
#[cfg(windows)]
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
        // 取版本号段比较，多版本时用最高
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
    setup_fixed_webview2_runtime();
    gui_lib::run()
}
