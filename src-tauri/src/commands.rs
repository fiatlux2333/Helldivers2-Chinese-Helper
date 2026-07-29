#[cfg(any(windows, test))]
use crate::core::text::{TextError, TextPreview};
use crate::{
    core::{
        config::AppConfig,
        session::SessionMachine,
        translation::{
            ChatLineTracker, NormalizedRegion, QuickShout, TranslationError, TranslationSettings,
            TranslationSettingsView, clamp_quick_shout_focus_delay_ms,
            default_quick_shout_focus_delay_ms,
        },
    },
    platform::InjectionReport,
};
#[cfg(windows)]
use crate::{
    core::{
        session::{SessionError, SessionSnapshot},
        text::preview_text as build_preview,
    },
    platform::{IntegrityDiagnostic, TargetDiagnostic},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};
#[cfg(windows)]
use std::{
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub session: Mutex<SessionMachine>,
    pub injection_gate: Mutex<()>,
    pub input_state_uncertain: Mutex<bool>,
    pub chat_line_tracker: Mutex<ChatLineTracker>,
    /// Last successfully resolved HD2 window. Survives session phase changes so
    /// global quick-shout hotkeys still know where to inject.
    pub last_target: Mutex<Option<crate::core::session::TargetIdentity>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: Mutex::new(AppConfig::default()),
            session: Mutex::new(SessionMachine::default()),
            injection_gate: Mutex::new(()),
            input_state_uncertain: Mutex::new(false),
            chat_line_tracker: Mutex::new(ChatLineTracker::default()),
            last_target: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpcErrorCode {
    UnsupportedPlatform,
    TargetNotMatched,
    TargetChanged,
    WindowUnavailable,
    WindowNotVisible,
    WindowMinimized,
    WindowCloaked,
    IntegrityIncompatible,
    TextEmpty,
    TextTooLong,
    SendInputPartial,
    FinalSubmitFailed,
    ApiConfiguration,
    ApiRequestFailed,
    ApiResponseInvalid,
    SettingsStorageFailed,
    CaptureFailed,
    OcrUnavailable,
    OcrEmpty,
    InvalidCaptureRegion,
    InvalidSession,
    SubmitKeyStillDown,
    InputStateUncertain,
    InternalState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
    pub partial_prefix_possible: bool,
    pub report: Option<InjectionReport>,
}

impl IpcError {
    fn new(code: IpcErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            partial_prefix_possible: false,
            report: None,
        }
    }

    #[cfg(windows)]
    fn partial(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::SendInputPartial,
            message: message.into(),
            partial_prefix_possible: true,
            report: None,
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeSession {
    #[serde(with = "crate::core::session::u64_string")]
    pub generation: u64,
    pub diagnostic: TargetDiagnostic,
    pub integrity: IntegrityDiagnostic,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSettingsUpdate {
    pub api_url: String,
    pub proxy_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub ocr_language: String,
    pub capture_hotkey: String,
    pub chat_region: Option<NormalizedRegion>,
    pub incoming_prompt: String,
    pub outgoing_prompt: String,
    #[serde(default = "default_quick_shout_focus_delay_ms")]
    pub quick_shout_focus_delay_ms: u64,
    pub quick_shouts: Vec<QuickShout>,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTranslationResult {
    pub lines: Vec<crate::core::translation::TranslatedChatLine>,
    pub message_ocr_language: String,
    pub speaker_ocr_language: String,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticLogsView {
    pub path: String,
    pub content: String,
}

#[cfg(windows)]
fn translation_settings_path(app: &tauri::AppHandle) -> Result<PathBuf, IpcError> {
    use tauri::Manager;
    app.path()
        .app_config_dir()
        .map(|directory| directory.join("translation-settings.json"))
        .map_err(|error| {
            IpcError::new(
                IpcErrorCode::SettingsStorageFailed,
                format!("无法定位翻译设置目录：{error}"),
            )
        })
}

#[cfg(windows)]
fn diagnostic_log_path(app: &tauri::AppHandle) -> Result<PathBuf, IpcError> {
    use tauri::Manager;
    app.path()
        .app_config_dir()
        .map(|directory| directory.join("runtime.log"))
        .map_err(|error| {
            IpcError::new(
                IpcErrorCode::SettingsStorageFailed,
                format!("无法定位运行日志目录：{error}"),
            )
        })
}

#[cfg(windows)]
fn record_log(app: &tauri::AppHandle, stage: &str, message: impl AsRef<str>) {
    let message = message.as_ref().replace(['\r', '\n'], " ");
    if let Ok(path) = diagnostic_log_path(app) {
        let _ = crate::core::logging::append(&path, stage, &message);
    }
    eprintln!("[hd2cn][{stage}] {message}");
}

#[cfg(windows)]
#[tauri::command]
pub fn get_diagnostic_logs(app: tauri::AppHandle) -> Result<DiagnosticLogsView, IpcError> {
    let path = diagnostic_log_path(&app)?;
    let content = crate::core::logging::read(&path).map_err(|error| {
        IpcError::new(
            IpcErrorCode::SettingsStorageFailed,
            format!("读取运行日志失败：{error}"),
        )
    })?;
    Ok(DiagnosticLogsView {
        path: path.to_string_lossy().into_owned(),
        content,
    })
}

#[cfg(windows)]
#[tauri::command]
pub fn clear_diagnostic_logs(app: tauri::AppHandle) -> Result<DiagnosticLogsView, IpcError> {
    let path = diagnostic_log_path(&app)?;
    crate::core::logging::clear(&path).map_err(|error| {
        IpcError::new(
            IpcErrorCode::SettingsStorageFailed,
            format!("清空运行日志失败：{error}"),
        )
    })?;
    Ok(DiagnosticLogsView {
        path: path.to_string_lossy().into_owned(),
        content: String::new(),
    })
}

#[cfg(windows)]
fn load_translation_settings(app: &tauri::AppHandle) -> Result<TranslationSettings, IpcError> {
    let path = translation_settings_path(app)?;
    crate::core::translation::load_settings(&path).map_err(map_translation_error)
}

#[cfg(windows)]
fn session_target(
    state: &tauri::State<'_, AppState>,
    generation: &str,
) -> Result<crate::core::session::TargetIdentity, IpcError> {
    let generation = generation
        .parse::<u64>()
        .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "会话编号无效"))?;
    let snapshot = state
        .session
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态锁异常"))?
        .snapshot();
    if snapshot.generation != generation {
        return Err(IpcError::new(
            IpcErrorCode::InvalidSession,
            "目标会话已经失效",
        ));
    }
    snapshot
        .target
        .ok_or_else(|| IpcError::new(IpcErrorCode::InvalidSession, "目标会话不存在"))
}

#[cfg(windows)]
fn map_translation_error(error: TranslationError) -> IpcError {
    match error {
        TranslationError::MissingApiUrl => {
            IpcError::new(IpcErrorCode::ApiConfiguration, "请先填写翻译 API 地址")
        }
        TranslationError::InvalidApiUrl => IpcError::new(
            IpcErrorCode::ApiConfiguration,
            "翻译 API 地址必须使用 http:// 或 https://",
        ),
        TranslationError::InvalidProxy => IpcError::new(
            IpcErrorCode::ApiConfiguration,
            "网络代理地址无效，请填写 http:// 或 socks5:// 地址",
        ),
        TranslationError::MissingModel => {
            IpcError::new(IpcErrorCode::ApiConfiguration, "请先填写翻译模型名")
        }
        TranslationError::InvalidRegion => IpcError::new(
            IpcErrorCode::InvalidCaptureRegion,
            "聊天截图区域无效，请重新校准",
        ),
        TranslationError::Request(message) => IpcError::new(
            IpcErrorCode::ApiRequestFailed,
            format!("翻译接口请求失败：{message}"),
        ),
        TranslationError::Response(message) if message.contains("未识别到文字") => {
            IpcError::new(IpcErrorCode::OcrEmpty, message)
        }
        TranslationError::Response(message) if message.contains("语言") => {
            IpcError::new(IpcErrorCode::OcrUnavailable, message)
        }
        TranslationError::Response(message) => {
            IpcError::new(IpcErrorCode::ApiResponseInvalid, message)
        }
        TranslationError::Storage(message) => IpcError::new(
            IpcErrorCode::SettingsStorageFailed,
            format!("翻译设置读写失败：{message}"),
        ),
    }
}

#[cfg(windows)]
#[tauri::command]
pub fn get_translation_settings(
    app: tauri::AppHandle,
) -> Result<TranslationSettingsView, IpcError> {
    let settings = load_translation_settings(&app)?;
    Ok(TranslationSettingsView::from(&settings))
}

#[cfg(windows)]
#[tauri::command]
pub fn save_translation_settings(
    settings: TranslationSettingsUpdate,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<TranslationSettingsView, IpcError> {
    let current = load_translation_settings(&app)?;
    let next_ocr_language = settings.ocr_language.trim().to_owned();
    let reset_chat_tracker =
        current.ocr_language != next_ocr_language || current.chat_region != settings.chat_region;
    let next = TranslationSettings {
        api_url: settings.api_url.trim().to_owned(),
        proxy_url: settings.proxy_url.trim().to_owned(),
        api_key: settings.api_key.unwrap_or(current.api_key),
        model: settings.model.trim().to_owned(),
        ocr_language: next_ocr_language,
        capture_hotkey: settings.capture_hotkey.trim().to_owned(),
        chat_region: settings.chat_region,
        incoming_prompt: settings.incoming_prompt.trim().to_owned(),
        outgoing_prompt: settings.outgoing_prompt.trim().to_owned(),
        quick_shout_focus_delay_ms: clamp_quick_shout_focus_delay_ms(
            settings.quick_shout_focus_delay_ms,
        ),
        quick_shouts: settings
            .quick_shouts
            .into_iter()
            .take(12)
            .filter_map(|shout| {
                let label = shout.label.trim().chars().take(24).collect::<String>();
                let message = shout.message.trim().chars().take(100).collect::<String>();
                if label.is_empty() || message.is_empty() {
                    return None;
                }
                Some(QuickShout {
                    label,
                    message,
                    hotkey: shout.hotkey.trim().to_owned(),
                })
            })
            .collect(),
    };
    if let Some(region) = next.chat_region {
        region.validate().map_err(map_translation_error)?;
    }
    let path = translation_settings_path(&app)?;
    crate::core::translation::save_settings(&path, &next).map_err(map_translation_error)?;
    if reset_chat_tracker {
        state
            .chat_line_tracker
            .lock()
            .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "聊天增量状态不可用"))?
            .reset();
    }
    Ok(TranslationSettingsView::from(&next))
}

#[cfg(windows)]
#[tauri::command]
pub async fn test_translation_api(app: tauri::AppHandle) -> Result<String, IpcError> {
    let settings = load_translation_settings(&app)?;
    crate::core::translation::translate_connection_test(&settings)
        .await
        .map_err(map_translation_error)
}

#[cfg(windows)]
#[tauri::command]
pub async fn translate_outgoing_text(
    text: String,
    app: tauri::AppHandle,
) -> Result<String, IpcError> {
    if text.trim().is_empty() {
        return Err(IpcError::new(
            IpcErrorCode::TextEmpty,
            "没有可翻译的中文内容",
        ));
    }
    let settings = load_translation_settings(&app)?;
    crate::core::translation::translate_outgoing_message(&settings, text.trim())
        .await
        .map_err(map_translation_error)
}

#[cfg(windows)]
#[tauri::command]
pub fn send_quick_shout(
    text: String,
    generation: Option<String>,
    open_chat: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<crate::platform::InjectionReport, IpcError> {
    record_log(
        &app,
        "quick_shout.start",
        format!(
            "text_chars={} generation={} open_chat={}",
            text.chars().count(),
            generation.as_deref().unwrap_or("none"),
            open_chat
        ),
    );
    let _injection_gate = state
        .injection_gate
        .try_lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "已有输入事务正在进行"))?;
    if *state
        .input_state_uncertain
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "输入安全状态不可用"))?
    {
        record_log(&app, "quick_shout.reject", "input_state_uncertain");
        return Err(IpcError::new(
            IpcErrorCode::InputStateUncertain,
            "上次 SendInput 返回了未配对事件；请重启助手后再试",
        ));
    }

    let config = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态不可用"))?
        .clone();
    let quick_shout_focus_delay_ms = load_translation_settings(&app)?.quick_shout_focus_delay_ms;
    let preview = map_text_result(build_preview(
        &text,
        config.character_limit,
        config.batch_size,
    ))?;

    let expected_target = resolve_quick_shout_target(&app, &state, &config, generation.as_deref())?;
    remember_target(&state, expected_target.clone());
    record_log(
        &app,
        "quick_shout.target_ready",
        format!(
            "pid={} hwnd=0x{:X} thread={}",
            expected_target.process_id, expected_target.hwnd, expected_target.thread_id
        ),
    );

    if !wait_for_input_release(Duration::from_millis(900)) {
        record_log(&app, "quick_shout.reject", "input_keys_still_down");
        return Err(IpcError::new(
            IpcErrorCode::SubmitKeyStillDown,
            "喊话快捷键尚未稳定释放，请松开按键后重试",
        ));
    }

    // One more restore attempt after key release; Windows often ignores earlier
    // SetForegroundWindow while modifiers are still down.
    let restored = crate::platform::windows::target::restore_foreground(
        &expected_target,
        &config.title_keyword,
    )
    .map_err(|error| {
        record_log(
            &app,
            "quick_shout.reject",
            format!("final_restore_error={error:?}"),
        );
        IpcError::new(IpcErrorCode::WindowUnavailable, "无法恢复 HD2 窗口")
    })?;
    if !restored {
        record_log(&app, "quick_shout.reject", "final_restore_failed");
        return Err(IpcError::new(
            IpcErrorCode::TargetChanged,
            "HD2 未能稳定恢复前台；请先切到游戏窗口，或在助手里重新捕获目标",
        ));
    }

    let deadline = Instant::now() + Duration::from_millis(2_000);
    while crate::platform::windows::target::validate_foreground(
        &expected_target,
        &config.title_keyword,
    ) != Ok(true)
    {
        record_log(
            &app,
            "quick_shout.wait",
            "target_not_stable_before_injection",
        );
        if Instant::now() >= deadline {
            record_log(&app, "quick_shout.reject", "target_not_stable_timeout");
            return Err(IpcError::new(
                IpcErrorCode::TargetChanged,
                "HD2 未能稳定恢复前台；请确认游戏未最小化，并在助手里重新捕获一次",
            ));
        }
        let _ = crate::platform::windows::target::restore_foreground(
            &expected_target,
            &config.title_keyword,
        );
        thread::sleep(Duration::from_millis(25));
    }

    record_log(
        &app,
        "quick_shout.inject",
        format!("begin open_chat={open_chat} focus_delay_ms={quick_shout_focus_delay_ms}"),
    );
    match crate::platform::windows::injector::inject_utf16_batches(
        &expected_target,
        &preview.utf16_batches,
        config.batch_delay_ms,
        &config.title_keyword,
        open_chat,
        quick_shout_focus_delay_ms,
        true,
    ) {
        Ok(report) => {
            record_log(&app, "quick_shout.success", format!("report={report:?}"));
            Ok(report)
        }
        Err(crate::platform::windows::injector::InjectionError::TargetChanged(report)) => {
            record_log(
                &app,
                "quick_shout.failure",
                format!("target_changed report={report:?}"),
            );
            let mut error = IpcError::new(IpcErrorCode::TargetChanged, "喊话过程中 HD2 窗口已变化");
            error.partial_prefix_possible = report.partial_prefix_possible;
            error.report = Some(report);
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::OpenChatFailed(report)) => {
            record_log(
                &app,
                "quick_shout.failure",
                format!("open_chat_failed report={report:?}"),
            );
            if report.key_state_uncertain {
                if let Ok(mut uncertain) = state.input_state_uncertain.lock() {
                    *uncertain = true;
                }
            }
            let mut error = IpcError::new(
                if report.key_state_uncertain {
                    IpcErrorCode::InputStateUncertain
                } else {
                    IpcErrorCode::SendInputPartial
                },
                "无法打开游戏聊天框",
            );
            error.report = Some(report);
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::SendInputFailed(report)) => {
            record_log(
                &app,
                "quick_shout.failure",
                format!("text_injection_failed report={report:?}"),
            );
            if report.key_state_uncertain {
                if let Ok(mut uncertain) = state.input_state_uncertain.lock() {
                    *uncertain = true;
                }
            }
            let mut error = IpcError::partial("喊话文字可能只填入了部分前缀，请检查游戏聊天框");
            error.report = Some(report);
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::SubmitFailed(report)) => {
            record_log(
                &app,
                "quick_shout.failure",
                format!("submit_failed report={report:?}"),
            );
            if report.key_state_uncertain {
                if let Ok(mut uncertain) = state.input_state_uncertain.lock() {
                    *uncertain = true;
                }
            }
            let mut error = IpcError::new(
                IpcErrorCode::FinalSubmitFailed,
                "喊话文字已填入，但最终 Enter 未完整注入",
            );
            error.partial_prefix_possible = true;
            error.report = Some(report);
            Err(error)
        }
    }
}

#[cfg(windows)]
fn remember_target(
    state: &tauri::State<'_, AppState>,
    target: crate::core::session::TargetIdentity,
) {
    if let Ok(mut last_target) = state.last_target.lock() {
        *last_target = Some(target);
    }
}

#[cfg(windows)]
fn resolve_quick_shout_target(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
    config: &AppConfig,
    generation: Option<&str>,
) -> Result<crate::core::session::TargetIdentity, IpcError> {
    if let Some(generation) = generation {
        record_log(app, "quick_shout.target", "using_captured_generation");
        let target = session_target(state, generation)?;
        let restored =
            crate::platform::windows::target::restore_foreground(&target, &config.title_keyword)
                .map_err(|error| {
                    record_log(
                        app,
                        "quick_shout.target",
                        format!("generation_restore_error={error:?}"),
                    );
                    IpcError::new(IpcErrorCode::WindowUnavailable, "无法恢复 HD2 窗口")
                })?;
        if restored {
            return Ok(target);
        }
        record_log(
            app,
            "quick_shout.target",
            "generation_restore_failed_try_fallbacks",
        );
    }

    let candidates: Vec<(String, crate::core::session::TargetIdentity)> = {
        let mut list = Vec::new();
        if let Ok(last_target) = state.last_target.lock() {
            if let Some(target) = last_target.clone() {
                list.push(("last_target".to_owned(), target));
            }
        }
        if let Ok(session) = state.session.lock() {
            if let Some(target) = session.snapshot().target {
                if list.iter().all(|(_, existing)| {
                    !crate::platform::windows::target::identities_compatible(existing, &target)
                }) {
                    list.push(("session_target".to_owned(), target));
                }
            }
        }
        list
    };

    for (source, candidate) in candidates {
        record_log(
            app,
            "quick_shout.target",
            format!(
                "trying_{source} hwnd=0x{:X} pid={}",
                candidate.hwnd, candidate.process_id
            ),
        );
        match crate::platform::windows::target::restore_foreground(
            &candidate,
            &config.title_keyword,
        ) {
            Ok(true) => {
                ensure_injection_integrity(candidate.process_id)?;
                return Ok(candidate);
            }
            Ok(false) => {
                record_log(app, "quick_shout.target", format!("{source}_not_restored"));
            }
            Err(error) => {
                record_log(
                    app,
                    "quick_shout.target",
                    format!("{source}_restore_error={error:?}"),
                );
            }
        }
    }

    record_log(app, "quick_shout.target", "fallback_foreground_diagnostic");
    let diagnostic = target_diagnostic(config)?;
    validate_target(&diagnostic)?;
    let target = diagnostic
        .identity
        .ok_or_else(|| IpcError::new(IpcErrorCode::WindowUnavailable, "目标窗口身份不可用"))?;
    ensure_injection_integrity(target.process_id)?;
    let restored =
        crate::platform::windows::target::restore_foreground(&target, &config.title_keyword)
            .unwrap_or(false);
    if !restored {
        record_log(app, "quick_shout.target", "foreground_target_not_confirmed");
    }
    Ok(target)
}

#[cfg(windows)]
fn ensure_injection_integrity(process_id: u32) -> Result<(), IpcError> {
    let integrity = crate::platform::windows::integrity::compare_with_target(process_id)
        .map_err(|_| IpcError::new(IpcErrorCode::WindowUnavailable, "无法读取进程完整性级别"))?;
    if integrity.compatible != Some(true) {
        return Err(IpcError::new(
            IpcErrorCode::IntegrityIncompatible,
            "无法确认工具具备向游戏注入输入的权限",
        ));
    }
    Ok(())
}

#[cfg(windows)]
#[tauri::command]
pub async fn list_ocr_languages()
-> Result<Vec<crate::platform::windows::capture::OcrLanguage>, IpcError> {
    tauri::async_runtime::spawn_blocking(crate::platform::windows::capture::list_ocr_languages)
        .await
        .map_err(|error| IpcError::new(IpcErrorCode::OcrUnavailable, error.to_string()))?
        .map_err(map_translation_error)
}

#[cfg(windows)]
#[tauri::command]
pub fn capture_chat_calibration_preview(
    generation: String,
    state: tauri::State<'_, AppState>,
) -> Result<crate::platform::windows::capture::CalibrationPreview, IpcError> {
    let target = session_target(&state, &generation)?;
    let title_keyword = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态锁异常"))?
        .title_keyword
        .clone();
    crate::platform::windows::capture::capture_client(&target, &title_keyword)
        .and_then(|image| image.calibration_preview())
        .map_err(map_translation_error)
}

#[cfg(windows)]
#[tauri::command]
pub async fn translate_chat_capture(
    generation: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ChatTranslationResult, IpcError> {
    let generation_number = generation
        .parse::<u64>()
        .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "会话编号无效"))?;
    let target = session_target(&state, &generation)?;
    let title_keyword = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态锁异常"))?
        .title_keyword
        .clone();
    let settings = load_translation_settings(&app)?;
    let region = settings
        .chat_region
        .ok_or_else(|| IpcError::new(IpcErrorCode::InvalidCaptureRegion, "请先校准游戏聊天区域"))?;
    let image = crate::platform::windows::capture::capture_client(&target, &title_keyword)
        .and_then(|image| image.crop(region))
        .map_err(map_translation_error)?;
    let fallback_language = settings.ocr_language.clone();
    let (chinese_reading, english_reading, mixed_ocr_language) =
        tauri::async_runtime::spawn_blocking(move || {
            let languages = crate::platform::windows::capture::list_ocr_languages()?;
            let chinese_language =
                crate::platform::windows::capture::preferred_chinese_ocr_language(&languages);
            let english_language =
                crate::platform::windows::capture::preferred_english_ocr_language(&languages);
            let chinese = match chinese_language {
                Some(language) => image.recognize_allow_empty(&language)?,
                None => image
                    .recognize_allow_empty(&fallback_language)
                    .map_err(|error| {
                        TranslationError::Response(format!(
                            "未安装可用的中文 Windows OCR 语言，回退识别也失败：{error:?}"
                        ))
                    })?,
            };
            let (english, mixed_ocr_language) = match english_language {
                Some(language) if chinese.language.eq_ignore_ascii_case(&language) => (
                    crate::platform::windows::capture::OcrReading {
                        language: language.clone(),
                        lines: Vec::new(),
                    },
                    format!("{}（中英混合）", chinese.language),
                ),
                Some(language) => {
                    let english = image.recognize_allow_empty(&language)?;
                    let label = format!("{} + {}", chinese.language, english.language);
                    (english, label)
                }
                None => (
                    crate::platform::windows::capture::OcrReading {
                        language: "未安装英文 OCR，使用中文引擎兼容拉丁字符".to_owned(),
                        lines: Vec::new(),
                    },
                    format!("{}（中英混合兼容模式）", chinese.language),
                ),
            };
            if chinese.lines.is_empty() && english.lines.is_empty() {
                return Err(TranslationError::Response(
                    "聊天区域未识别到文字".to_owned(),
                ));
            }
            Ok::<_, TranslationError>((chinese, english, mixed_ocr_language))
        })
        .await
        .map_err(|error| IpcError::new(IpcErrorCode::OcrUnavailable, error.to_string()))?
        .map_err(map_translation_error)?;
    let parsed_lines = crate::core::translation::merge_bilingual_ocr_lines(
        &chinese_reading.lines,
        &english_reading.lines,
    );
    let pending_lines = state
        .chat_line_tracker
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "聊天增量状态不可用"))?
        .pending_lines(generation_number, &parsed_lines);
    #[cfg(debug_assertions)]
    eprintln!(
        "[hd2cn][ocr] chinese_language={} chinese_lines={} english_language={} english_lines={} parsed_lines={} pending_lines={}",
        chinese_reading.language,
        chinese_reading.lines.len(),
        english_reading.language,
        english_reading.lines.len(),
        parsed_lines.len(),
        pending_lines.len()
    );
    if pending_lines.is_empty() {
        state
            .chat_line_tracker
            .lock()
            .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "聊天增量状态不可用"))?
            .commit(generation_number, &parsed_lines);
        return Ok(ChatTranslationResult {
            lines: Vec::new(),
            message_ocr_language: mixed_ocr_language,
            speaker_ocr_language: chinese_reading.language,
        });
    }
    let translated_lines =
        crate::core::translation::translate_chat_lines(&settings, &pending_lines)
            .await
            .map_err(map_translation_error)?;
    state
        .chat_line_tracker
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "聊天增量状态不可用"))?
        .commit(generation_number, &parsed_lines);
    Ok(ChatTranslationResult {
        lines: translated_lines,
        message_ocr_language: mixed_ocr_language,
        speaker_ocr_language: chinese_reading.language,
    })
}

#[cfg(windows)]
fn target_diagnostic(config: &AppConfig) -> Result<TargetDiagnostic, IpcError> {
    use crate::platform::windows::target::TargetError;

    crate::platform::windows::target::foreground_diagnostic(&config.title_keyword).map_err(
        |error| {
            let message = match error {
                TargetError::NoForegroundWindow => "系统当前没有可读取的前台窗口",
                TargetError::WindowTitleUnavailable => "无法读取当前前台窗口标题",
                TargetError::WindowStateUnavailable => "无法读取当前前台窗口的显示状态",
                TargetError::ProcessUnavailable(code) => {
                    return IpcError::new(
                        IpcErrorCode::WindowUnavailable,
                        format!("无法读取当前前台窗口的进程身份（Win32 错误码 {code}）"),
                    );
                }
                TargetError::ProcessTimeUnavailable(code) => {
                    return IpcError::new(
                        IpcErrorCode::WindowUnavailable,
                        format!("无法读取当前前台进程的创建时间（Win32 错误码 {code}）"),
                    );
                }
            };
            IpcError::new(IpcErrorCode::WindowUnavailable, message)
        },
    )
}

#[cfg(windows)]
#[tauri::command]
pub fn get_target_diagnostic(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<TargetDiagnostic, IpcError> {
    let config = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态不可用"))?;
    match target_diagnostic(&config) {
        Ok(diagnostic) => {
            record_log(
                &app,
                "target.diagnostic",
                format!(
                    "title={:?} matches={} visible={} minimized={} cloaked={}",
                    diagnostic.title,
                    diagnostic.title_matches,
                    diagnostic.visible,
                    diagnostic.minimized,
                    diagnostic.cloaked
                ),
            );
            if diagnostic.title_matches {
                if let Some(identity) = diagnostic.identity.clone() {
                    remember_target(&state, identity);
                }
            }
            Ok(diagnostic)
        }
        Err(error) => {
            record_log(&app, "target.diagnostic_failure", error.message.clone());
            Err(error)
        }
    }
}

#[cfg(windows)]
#[tauri::command]
pub fn preview_text(
    text: String,
    state: tauri::State<'_, AppState>,
) -> Result<TextPreview, IpcError> {
    let config = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态不可用"))?;
    map_text_result(build_preview(
        &text,
        config.character_limit,
        config.batch_size,
    ))
}

#[cfg(windows)]
#[tauri::command]
pub fn begin_probe_session(state: tauri::State<'_, AppState>) -> Result<ProbeSession, IpcError> {
    let config = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态不可用"))?
        .clone();
    let diagnostic = target_diagnostic(&config)?;
    validate_target(&diagnostic)?;
    let target = diagnostic
        .identity
        .clone()
        .ok_or_else(|| IpcError::new(IpcErrorCode::WindowUnavailable, "目标窗口身份不可用"))?;
    let integrity = crate::platform::windows::integrity::compare_with_target(target.process_id)
        .map_err(|_| IpcError::new(IpcErrorCode::WindowUnavailable, "无法读取进程完整性级别"))?;
    if integrity.compatible != Some(true) {
        return Err(IpcError::new(
            IpcErrorCode::IntegrityIncompatible,
            "无法确认工具具备向游戏注入输入的权限",
        ));
    }
    if *state
        .input_state_uncertain
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "输入安全状态不可用"))?
    {
        return Err(IpcError::new(
            IpcErrorCode::InputStateUncertain,
            "上次 SendInput 返回了未配对事件；请重启助手后再试",
        ));
    }
    let _gate = state
        .injection_gate
        .try_lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "已有填字事务正在进行"))?;

    let mut session = state
        .session
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))?;
    let generation = session.begin_probe(target.clone());
    drop(session);
    remember_target(&state, target);
    state
        .chat_line_tracker
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "聊天增量状态不可用"))?
        .reset();
    Ok(ProbeSession {
        generation,
        diagnostic,
        integrity,
    })
}

#[cfg(windows)]
#[tauri::command]
pub fn inject_probe_text(
    generation: String,
    text: String,
    submit: bool,
    state: tauri::State<'_, AppState>,
) -> Result<crate::platform::InjectionReport, IpcError> {
    let generation = generation
        .parse::<u64>()
        .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "会话编号无效"))?;
    let _injection_gate = state
        .injection_gate
        .try_lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "已有填字事务正在进行"))?;
    if *state
        .input_state_uncertain
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "输入安全状态不可用"))?
    {
        return Err(IpcError::new(
            IpcErrorCode::InputStateUncertain,
            "上次 SendInput 返回了未配对事件；请重启助手后再试",
        ));
    }

    let config = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态不可用"))?
        .clone();
    let preview = map_text_result(build_preview(
        &text,
        config.character_limit,
        config.batch_size,
    ))?;
    let expected_target = {
        let mut session = state
            .session
            .lock()
            .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))?;
        session
            .request_submit(generation, preview.cleaned_text.clone())
            .map_err(map_session_error)?;
        session
            .snapshot()
            .target
            .ok_or_else(|| IpcError::new(IpcErrorCode::InvalidSession, "会话目标不存在"))?
    };

    if !wait_for_input_release(Duration::from_millis(500)) {
        fail_session(&state, generation, "提交键或修饰键未稳定释放");
        return Err(IpcError::new(
            IpcErrorCode::SubmitKeyStillDown,
            "Enter、Ctrl、Alt、Shift 或 Win 尚未稳定释放",
        ));
    }

    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))?;
        let released = session
            .observe_submit_key(
                generation,
                crate::platform::windows::key_state::sample_keys(),
            )
            .map_err(map_session_error)?;
        if !released {
            let _ = session.fail_session(generation, "提交键或修饰键重新进入按下状态");
            return Err(IpcError::new(
                IpcErrorCode::SubmitKeyStillDown,
                "提交键或修饰键重新进入按下状态",
            ));
        }
    }

    let restored = match crate::platform::windows::target::restore_foreground(
        &expected_target,
        &config.title_keyword,
    ) {
        Ok(restored) => restored,
        Err(_) => {
            fail_session(&state, generation, "无法恢复目标窗口");
            return Err(IpcError::new(
                IpcErrorCode::WindowUnavailable,
                "无法恢复目标窗口",
            ));
        }
    };
    if !restored {
        fail_session(&state, generation, "目标窗口已变化或系统拒绝恢复前台");
        return Err(IpcError::new(
            IpcErrorCode::TargetChanged,
            "目标窗口已变化或系统拒绝恢复前台",
        ));
    }

    let deadline = Instant::now() + Duration::from_millis(1_500);
    let current_target = loop {
        match crate::platform::windows::target::validate_foreground(
            &expected_target,
            &config.title_keyword,
        ) {
            Ok(true) => break expected_target.clone(),
            _ if Instant::now() >= deadline => {
                fail_session(&state, generation, "目标窗口未能稳定恢复前台");
                return Err(IpcError::new(
                    IpcErrorCode::TargetChanged,
                    "目标窗口未能稳定恢复前台",
                ));
            }
            _ => thread::sleep(Duration::from_millis(25)),
        }
    };

    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))?;
        session
            .begin_injection(generation, &current_target)
            .map_err(map_session_error)?;
    }

    let result = crate::platform::windows::injector::inject_utf16_batches(
        &expected_target,
        &preview.utf16_batches,
        config.batch_delay_ms,
        &config.title_keyword,
        false,
        0,
        submit,
    );
    let mut session = state
        .session
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))?;
    match result {
        Ok(report) => {
            session
                .complete_injection(generation)
                .map_err(map_session_error)?;
            Ok(report)
        }
        Err(crate::platform::windows::injector::InjectionError::TargetChanged(report)) => {
            let _ = session.fail_injection(generation, "发送过程中目标窗口已变化");
            let mut error = IpcError::new(IpcErrorCode::TargetChanged, "发送过程中目标窗口已变化");
            error.partial_prefix_possible = report.partial_prefix_possible;
            error.report = Some(report);
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::OpenChatFailed(report)) => {
            let _ = session.fail_injection(generation, "意外进入打开聊天失败分支");
            let mut error = IpcError::new(
                IpcErrorCode::SendInputPartial,
                "输入流程未能稳定启动，请重新捕获游戏窗口",
            );
            error.report = Some(report);
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::SendInputFailed(report)) => {
            let _ = session.fail_injection(generation, "SendInput 未完整发送本批事件");
            let mut error = if report.key_state_uncertain {
                if let Ok(mut uncertain) = state.input_state_uncertain.lock() {
                    *uncertain = true;
                }
                IpcError::new(
                    IpcErrorCode::InputStateUncertain,
                    "SendInput 返回了未配对事件；请检查游戏输入框并重启助手",
                )
            } else {
                IpcError::partial("文字可能只发送了部分前缀，请检查游戏输入框")
            };
            error.partial_prefix_possible = report.partial_prefix_possible;
            error.report = Some(report);
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::SubmitFailed(report)) => {
            let _ = session.fail_injection(generation, "最终 Enter 未完整注入");
            if report.key_state_uncertain {
                if let Ok(mut uncertain) = state.input_state_uncertain.lock() {
                    *uncertain = true;
                }
            }
            let mut error = IpcError::new(
                if report.key_state_uncertain {
                    IpcErrorCode::InputStateUncertain
                } else {
                    IpcErrorCode::FinalSubmitFailed
                },
                "文字已填入，但最终 Enter 未完整注入；请检查游戏聊天框，切勿直接重发整段",
            );
            error.partial_prefix_possible = true;
            error.report = Some(report);
            Err(error)
        }
    }
}

#[cfg(windows)]
#[tauri::command]
pub fn get_integrity_diagnostic(
    state: tauri::State<'_, AppState>,
) -> Result<IntegrityDiagnostic, IpcError> {
    let config = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态不可用"))?
        .clone();
    let diagnostic = target_diagnostic(&config)?;
    validate_target(&diagnostic)?;
    let target = diagnostic
        .identity
        .ok_or_else(|| IpcError::new(IpcErrorCode::WindowUnavailable, "目标窗口身份不可用"))?;
    crate::platform::windows::integrity::compare_with_target(target.process_id)
        .map_err(|_| IpcError::new(IpcErrorCode::WindowUnavailable, "无法读取进程完整性级别"))
}

#[cfg(windows)]
#[tauri::command]
pub fn get_session_state(state: tauri::State<'_, AppState>) -> Result<SessionSnapshot, IpcError> {
    state
        .session
        .lock()
        .map(|session| session.snapshot())
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))
}

#[cfg(windows)]
#[tauri::command]
pub fn cancel_session(
    generation: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<SessionSnapshot, IpcError> {
    let _gate = state.injection_gate.try_lock().map_err(|_| {
        IpcError::new(
            IpcErrorCode::InvalidSession,
            "填字事务已经开始，无法取消已发送的事件",
        )
    })?;
    let mut session = state
        .session
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))?;
    if let Some(generation) = generation {
        let generation = generation
            .parse::<u64>()
            .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "会话编号无效"))?;
        if session.snapshot().generation != generation {
            return Err(IpcError::new(
                IpcErrorCode::InvalidSession,
                "取消请求对应的会话已经失效",
            ));
        }
    }
    session.cancel();
    state
        .chat_line_tracker
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "聊天增量状态不可用"))?
        .reset();
    Ok(session.snapshot())
}

#[cfg(windows)]
fn wait_for_input_release(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let mut stable_samples = 0;

    while Instant::now() < deadline {
        if crate::platform::windows::key_state::sample_keys().all_released() {
            stable_samples += 1;
            if stable_samples >= 3 {
                return true;
            }
        } else {
            stable_samples = 0;
        }
        thread::sleep(Duration::from_millis(15));
    }

    false
}

#[cfg(windows)]
fn fail_session(state: &tauri::State<'_, AppState>, generation: u64, message: &str) {
    if let Ok(mut session) = state.session.lock() {
        let _ = session.fail_session(generation, message);
    }
}

#[cfg(not(windows))]
pub fn unsupported_platform() -> IpcError {
    IpcError::new(
        IpcErrorCode::UnsupportedPlatform,
        "当前平台不支持 Windows 输入探针",
    )
}

#[cfg(any(windows, test))]
fn map_text_result(result: Result<TextPreview, TextError>) -> Result<TextPreview, IpcError> {
    result.map_err(|error| match error {
        TextError::Empty => IpcError::new(IpcErrorCode::TextEmpty, "清理后的文本为空"),
        TextError::TooLong { actual, limit } => IpcError::new(
            IpcErrorCode::TextTooLong,
            format!("文本包含 {actual} 个字符，超过上限 {limit}"),
        ),
        TextError::InvalidBatchSize => {
            IpcError::new(IpcErrorCode::InternalState, "每批字符数配置无效")
        }
    })
}

#[cfg(windows)]
fn validate_target(diagnostic: &TargetDiagnostic) -> Result<(), IpcError> {
    if !diagnostic.title_matches {
        return Err(IpcError::new(
            IpcErrorCode::TargetNotMatched,
            "当前前台窗口标题不匹配 HELLDIVERS",
        ));
    }
    if !diagnostic.is_window || !diagnostic.visible {
        return Err(IpcError::new(
            IpcErrorCode::WindowNotVisible,
            "目标窗口不可见",
        ));
    }
    if diagnostic.minimized {
        return Err(IpcError::new(
            IpcErrorCode::WindowMinimized,
            "目标窗口已最小化",
        ));
    }
    if diagnostic.cloaked {
        return Err(IpcError::new(
            IpcErrorCode::WindowCloaked,
            "目标窗口被系统隐藏",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn map_session_error(error: SessionError) -> IpcError {
    match error {
        SessionError::StaleGeneration | SessionError::InvalidPhase => {
            IpcError::new(IpcErrorCode::InvalidSession, "会话已失效或状态不允许此操作")
        }
        SessionError::TargetChanged => IpcError::new(IpcErrorCode::TargetChanged, "目标窗口已变化"),
        SessionError::SubmitKeyStillDown => {
            IpcError::new(IpcErrorCode::SubmitKeyStillDown, "提交键尚未释放")
        }
        SessionError::InjectionInProgress => {
            IpcError::new(IpcErrorCode::InvalidSession, "已有发送事务正在进行")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_error_is_explicit() {
        #[cfg(not(windows))]
        assert_eq!(
            unsupported_platform().code,
            IpcErrorCode::UnsupportedPlatform
        );
    }

    #[test]
    fn text_errors_use_stable_codes() {
        assert_eq!(
            map_text_result(Err(TextError::Empty)).unwrap_err().code,
            IpcErrorCode::TextEmpty
        );
    }
}
