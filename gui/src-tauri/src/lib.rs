// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod commands;

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
            commands::cancel_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
