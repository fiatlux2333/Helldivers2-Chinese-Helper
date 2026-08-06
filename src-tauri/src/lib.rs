pub mod commands;
pub mod core;
pub mod platform;

#[cfg(all(windows, feature = "tauri-shell"))]
pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::AppState::default())
        .setup(|app| {
            let _ = commands::initialize_runtime_settings(app.handle());
            crate::platform::windows::game_monitor::start(
                app.handle().clone(),
                crate::core::config::AppConfig::default().title_keyword,
            );
            Ok(())
        })
        .on_window_event(|_, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                crate::platform::windows::game_monitor::set_auto_lock_caps(false);
                crate::platform::windows::game_monitor::restore_caps_lock();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_target_diagnostic,
            commands::get_diagnostic_logs,
            commands::clear_diagnostic_logs,
            commands::export_diagnostic_logs,
            commands::check_for_updates,
            commands::preview_text,
            commands::begin_probe_session,
            commands::inject_probe_text,
            commands::get_integrity_diagnostic,
            commands::get_session_state,
            commands::cancel_session,
            commands::get_translation_settings,
            commands::save_translation_settings,
            commands::test_translation_api,
            commands::translate_outgoing_text,
            commands::send_quick_shout,
            commands::cancel_overlay_chat,
            commands::list_ocr_languages,
            commands::capture_chat_calibration_preview,
            commands::translate_chat_capture,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Helldivers 2 中文助手失败");
}
