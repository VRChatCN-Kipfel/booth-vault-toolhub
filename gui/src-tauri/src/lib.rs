// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod commands;
pub mod portable;

use commands::{
    TaskRegistry, audit, cancel_task, download, fix_mismatch, load_app_config, mismatch_audit,
    organize, save_app_config, search, update_check, version_audit,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(TaskRegistry::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            download,
            organize,
            search,
            audit,
            version_audit,
            mismatch_audit,
            fix_mismatch,
            update_check,
            cancel_task,
            load_app_config,
            save_app_config,
        ])
        .setup(|app| {
            // 主窗口手建（config create:false）。
            // 便携模式：数据目录锚定 exe 目录内（data/webview），
            // 避免污染 %LOCALAPPDATA% 且跨机器可携带。
            let mut builder = tauri::WebviewWindowBuilder::from_config(
                app.handle(),
                &app.config().app.windows[0],
            )?;
            if let Some(data_dir) = portable::portable_webview_dir() {
                std::fs::create_dir_all(&data_dir).ok();
                builder = builder.data_directory(data_dir);
            }
            #[cfg(target_os = "macos")]
            {
                builder = builder
                    .hidden_title(true)
                    .title_bar_style(tauri::TitleBarStyle::Overlay);
            }
            builder.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
