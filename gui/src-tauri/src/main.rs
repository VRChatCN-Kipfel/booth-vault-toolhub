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

/// 自注册位置：把当前 exe 目录写入用户级环境变量 BOOTHVAULT_TOOLHUB。
/// 便携版无安装器记录安装位置，靠 GUI 首次运行自报，agent 读该变量确定性发现工具目录。
/// 契约：值缺失或与当前目录不同才写；写入后广播 WM_SETTINGCHANGE；失败静默，不阻塞启动。
#[cfg(windows)]
fn register_location() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegGetValueW,
        RegOpenKeyExW, RegSetValueExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };
    use windows::core::PCWSTR;

    const VAR: &str = "BOOTHVAULT_TOOLHUB";
    const KEY: &str = "Environment";

    fn wide(s: &str) -> Vec<u16> {
        let mut v: Vec<u16> = s.encode_utf16().collect();
        v.push(0);
        v
    }
    // 兼容尾反斜杠：可能被写为 `C:\...\booth-vault-toolhub\`，归一后比较
    fn norm(v: &str) -> &str {
        v.trim_end_matches(['\\', '/'])
    }

    let Ok(exe_path) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe_path.parent() else { return };
    let current = dir.to_string_lossy().into_owned();

    let var_wide = wide(VAR);
    let key_wide = wide(KEY);
    let var_pw = PCWSTR(var_wide.as_ptr());
    let key_pw = PCWSTR(key_wide.as_ptr());

    unsafe {
        // 1) 读现有值（REG_SZ）
        let mut size = 0u32;
        let existing = if RegGetValueW(
            HKEY_CURRENT_USER,
            key_pw,
            var_pw,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
        .0 == 0
        {
            let mut buf = vec![0u16; (size as usize).div_ceil(2)];
            let mut value_type = REG_SZ;
            let mut size2 = size;
            if RegGetValueW(
                HKEY_CURRENT_USER,
                key_pw,
                var_pw,
                RRF_RT_REG_SZ,
                Some(&mut value_type),
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size2),
            )
            .0 == 0
            {
                String::from_utf16_lossy(&buf)
                    .trim_end_matches('\0')
                    .to_owned()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if norm(&existing) == norm(&current) {
            return; // 已指向自身运行目录，无需重写
        }

        // 2) 写入
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, key_pw, None, KEY_SET_VALUE, &mut hkey).0 != 0 {
            return;
        }
        let data = wide(&current);
        let bytes = std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2);
        let rc = RegSetValueExW(hkey, var_pw, None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(hkey);
        if rc.0 != 0 {
            return;
        }

        // 3) 广播环境变更，让新进程继承
        let msg_wide = wide(KEY);
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(msg_wide.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            100,
            None,
        );
    }
}

fn main() {
    #[cfg(windows)]
    setup_webview2();
    #[cfg(windows)]
    register_location();
    gui_lib::run()
}
