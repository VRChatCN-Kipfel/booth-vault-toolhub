// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod commands;
pub mod portable;

use commands::TaskRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(TaskRegistry::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::download,
            commands::organize,
            commands::search,
            commands::audit,
            commands::version_audit,
            commands::mismatch_audit,
            commands::fix_mismatch,
            commands::update_check,
            commands::cancel_task,
            commands::load_app_config,
            commands::save_app_config,
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
