pub mod commands;
pub mod core;
pub mod platform;

#[cfg(all(windows, feature = "tauri-shell"))]
const SINGLE_INSTANCE_RESTORE_EVENT: &str = "single-instance-restore";

#[cfg(all(windows, feature = "tauri-shell"))]
fn cleanup_before_exit() {
    crate::platform::windows::game_monitor::set_auto_lock_caps(false);
    let _ = crate::platform::windows::game_monitor::restore_caps_lock();
}

#[cfg(all(windows, feature = "tauri-shell"))]
fn start_exit_fallback() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(1_500));
        std::process::exit(0);
    });
}

#[cfg(all(windows, feature = "tauri-shell"))]
fn restore_main_window_for_second_instance(app: &tauri::AppHandle) {
    use tauri::{Emitter, Manager};

    if let Some(window) = app.get_webview_window("chat-overlay") {
        let _ = window.set_focusable(false);
        let _ = window.set_skip_taskbar(true);
        let _ = window.hide();
    }
    if let Some(window) = app.get_webview_window("translation-hud") {
        let _ = window.set_focusable(false);
        let _ = window.set_skip_taskbar(true);
        let _ = window.hide();
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focusable(true);
        let _ = window.set_skip_taskbar(false);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let _ = app.emit(SINGLE_INSTANCE_RESTORE_EVENT, ());
}

#[cfg(all(windows, feature = "tauri-shell"))]
pub fn run() {
    use tauri::Manager;

    static EXIT_CLEANUP_STARTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    static EXIT_FALLBACK_STARTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            restore_main_window_for_second_instance(app);
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
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if !EXIT_CLEANUP_STARTED.swap(true, std::sync::atomic::Ordering::AcqRel) {
                        cleanup_before_exit();
                    }
                    window.app_handle().exit(0);
                    if !EXIT_FALLBACK_STARTED.swap(true, std::sync::atomic::Ordering::AcqRel) {
                        start_exit_fallback();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_target_diagnostic,
            commands::get_diagnostic_logs,
            commands::clear_diagnostic_logs,
            commands::record_client_diagnostic,
            commands::export_diagnostic_logs,
            commands::check_for_updates,
            commands::preview_text,
            commands::begin_probe_session,
            commands::inject_probe_text,
            commands::get_integrity_diagnostic,
            commands::get_session_state,
            commands::cancel_session,
            commands::reset_input_state,
            commands::get_translation_settings,
            commands::save_translation_settings,
            commands::test_translation_api,
            commands::translate_outgoing_text,
            commands::send_quick_shout,
            commands::handoff_gameplay_input,
            commands::send_stratagem_macro,
            commands::cancel_overlay_chat,
            commands::cancel_injection,
            commands::discard_pending_injection_cancel,
            commands::discard_first_key_buffer,
            commands::replay_buffered_keys,
            commands::probe_foreground_state,
            commands::list_ocr_languages,
            commands::capture_chat_calibration_preview,
            commands::translate_chat_capture,
        ])
        .build(tauri::generate_context!())
        .expect("启动 Helldivers 2 中文助手失败");

    app.run(|_, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            cleanup_before_exit();
        }
    });
}
