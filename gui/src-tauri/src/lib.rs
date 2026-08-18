// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod commands;
pub mod portable;

use commands::{
    TaskRegistry, audit, cancel_task, download, fix_mismatch, load_app_config, mismatch_audit,
    organize, save_app_config, search, update_check, version_audit,
};
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .manage(TaskRegistry::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    StateFlags::SIZE
                        | StateFlags::POSITION
                        | StateFlags::MAXIMIZED
                        | StateFlags::FULLSCREEN,
                )
                .build(),
        );
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_plugin_liquid_glass::init());
    }
    builder
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
            if app.handle().get_webview_window("main").is_some() {
                return Ok(());
            }
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
                // Overlay + decorations:true 才会画出系统红黄绿；false 只留空位。
                builder = builder
                    .decorations(true)
                    .hidden_title(true)
                    .title_bar_style(tauri::TitleBarStyle::Overlay)
                    .transparent(true)
                    .traffic_light_position(tauri::LogicalPosition::new(14.0, 10.0));
            }
            let window = builder.build()?;
            #[cfg(target_os = "macos")]
            {
                use tauri_plugin_liquid_glass::{
                    GlassMaterialVariant, LiquidGlassConfig, LiquidGlassExt,
                };
                // macOS 26：NSGlassEffectView；更旧系统回落 NSVisualEffectView。
                if let Err(e) = app.handle().liquid_glass().set_effect(
                    &window,
                    LiquidGlassConfig {
                        enabled: true,
                        corner_radius: 10.0,
                        tint_color: None,
                        variant: GlassMaterialVariant::Regular,
                    },
                ) {
                    eprintln!("liquid glass: {e}");
                }
            }
            let _ = window;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.app_handle().exit(0);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
