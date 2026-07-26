pub mod commands;
pub mod core;
pub mod platform;

#[cfg(all(windows, feature = "tauri-shell"))]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_target_diagnostic,
            commands::get_diagnostic_logs,
            commands::clear_diagnostic_logs,
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
            commands::list_ocr_languages,
            commands::capture_chat_calibration_preview,
            commands::translate_chat_capture,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Helldivers 2 中文助手失败");
}
