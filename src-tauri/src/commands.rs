#[cfg(any(windows, test))]
use crate::core::text::{TextError, TextPreview};
use crate::{
    core::{config::AppConfig, session::SessionMachine, translation::ChatLineTracker},
    platform::InjectionReport,
};
#[cfg(windows)]
use crate::{
    core::{
        session::{SessionError, SessionSnapshot},
        text::preview_text as build_preview,
        translation::{
            GameInputMethod, IncomingTranslationDisplayMode, NormalizedPosition, NormalizedRegion,
            QuickShout, StratagemDirectionInputMode, StratagemMacro, TranslationError,
            TranslationSettings, TranslationSettingsView, clamp_game_input_delay_ms,
            clamp_quick_shout_focus_delay_ms, clamp_stratagem_delay_ms,
            default_game_input_delay_ms, default_quick_shout_focus_delay_ms,
            default_stratagem_allow_bare_number_hotkeys, default_stratagem_direction_input_mode,
            normalize_game_input_method, normalize_stratagem_direction_code,
            stratagem_direction_input_code,
        },
    },
    platform::{IntegrityDiagnostic, TargetDiagnostic},
};
#[cfg(any(windows, test))]
use semver::Version;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
#[cfg(windows)]
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
const OVERLAY_CANCEL_FOCUS_DELAY_MS: u64 = 160;
#[cfg(windows)]
const OVERLAY_CANCEL_FOREGROUND_TIMEOUT_MS: u64 = 1_500;

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
    TextEncodingUnsupported,
    KeyboardLayoutUnavailable,
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
    pub report: Option<Box<InjectionReport>>,
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
    #[serde(default = "crate::core::translation::default_incoming_translation_display_mode")]
    pub incoming_translation_display_mode: IncomingTranslationDisplayMode,
    #[serde(default)]
    pub translation_hud_position: Option<NormalizedPosition>,
    #[serde(default = "crate::core::translation::default_game_overlay_enabled")]
    pub game_overlay_enabled: bool,
    #[serde(default = "crate::core::translation::default_overlay_chat_key")]
    pub overlay_chat_key: String,
    #[serde(default = "crate::core::translation::default_auto_lock_caps")]
    pub auto_lock_caps: bool,
    #[serde(default)]
    pub game_input_method: GameInputMethod,
    #[serde(default = "default_game_input_delay_ms")]
    pub game_input_delay_ms: u64,
    #[serde(default = "default_quick_shout_focus_delay_ms")]
    pub quick_shout_focus_delay_ms: u64,
    pub quick_shouts: Vec<QuickShout>,
    #[serde(default)]
    pub stratagem_macros: Vec<StratagemMacro>,
    #[serde(default = "default_stratagem_direction_input_mode")]
    pub stratagem_direction_input_mode: StratagemDirectionInputMode,
    #[serde(default = "default_stratagem_allow_bare_number_hotkeys")]
    pub stratagem_allow_bare_number_hotkeys: bool,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckView {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
struct GitHubLatestRelease {
    tag_name: String,
    html_url: String,
}

#[cfg(windows)]
const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/fiatlux2333/Helldivers2-Chinese-Helper/releases/latest";
#[cfg(windows)]
const RELEASE_URL_PREFIX: &str =
    "https://github.com/fiatlux2333/Helldivers2-Chinese-Helper/releases/";

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
#[tauri::command]
pub fn record_client_diagnostic(
    app: tauri::AppHandle,
    stage: String,
    message: String,
) -> Result<(), IpcError> {
    let stage = stage.trim();
    if stage.is_empty()
        || stage.len() > 64
        || !stage
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(IpcError::new(
            IpcErrorCode::InternalState,
            "前端诊断阶段名称无效",
        ));
    }
    record_log(&app, &format!("client.{stage}"), message);
    Ok(())
}

#[cfg(windows)]
#[tauri::command]
pub fn export_diagnostic_logs(app: tauri::AppHandle) -> Result<String, IpcError> {
    record_log(
        &app,
        "diagnostic.export",
        format!("app_version={}", env!("CARGO_PKG_VERSION")),
    );
    let source = diagnostic_log_path(&app)?;
    let executable = std::env::current_exe().map_err(|error| {
        IpcError::new(
            IpcErrorCode::SettingsStorageFailed,
            format!("无法定位软件安装目录：{error}"),
        )
    })?;
    let directory = executable.parent().ok_or_else(|| {
        IpcError::new(IpcErrorCode::SettingsStorageFailed, "无法定位软件安装目录")
    })?;
    let destination = directory.join("HD2CN-diagnostic.log");
    crate::core::logging::export(&source, &destination).map_err(|error| {
        IpcError::new(
            IpcErrorCode::SettingsStorageFailed,
            format!("无法将诊断日志写入安装目录：{error}"),
        )
    })?;
    Ok(destination.to_string_lossy().into_owned())
}

#[cfg(windows)]
#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateCheckView, IpcError> {
    let settings = load_translation_settings(&app)?;
    let mut client_builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .http1_only();
    if settings.proxy_url.trim().is_empty() {
        client_builder = client_builder.no_proxy();
    } else {
        let proxy = reqwest::Proxy::all(settings.proxy_url.trim())
            .map_err(|_| IpcError::new(IpcErrorCode::ApiConfiguration, "更新检查代理地址无效"))?;
        client_builder = client_builder.proxy(proxy);
    }
    let client = client_builder.build().map_err(|error| {
        IpcError::new(
            IpcErrorCode::ApiRequestFailed,
            format!("无法创建更新检查连接：{error}"),
        )
    })?;
    let response = client
        .get(LATEST_RELEASE_API)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(
            reqwest::header::USER_AGENT,
            concat!("helldivers2-cn-helper/", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .map_err(|error| {
            IpcError::new(
                IpcErrorCode::ApiRequestFailed,
                format!("检查更新失败：{error}"),
            )
        })?;
    if !response.status().is_success() {
        return Err(IpcError::new(
            IpcErrorCode::ApiRequestFailed,
            format!("GitHub 更新接口返回状态 {}", response.status()),
        ));
    }
    let release = response
        .json::<GitHubLatestRelease>()
        .await
        .map_err(|error| {
            IpcError::new(
                IpcErrorCode::ApiResponseInvalid,
                format!("无法解析 GitHub 最新版本：{error}"),
            )
        })?;
    if !release.html_url.starts_with(RELEASE_URL_PREFIX) {
        return Err(IpcError::new(
            IpcErrorCode::ApiResponseInvalid,
            "GitHub 返回了非预期的下载地址",
        ));
    }

    let current = parse_release_version(env!("CARGO_PKG_VERSION")).map_err(|message| {
        IpcError::new(
            IpcErrorCode::InternalState,
            format!("当前版本无效：{message}"),
        )
    })?;
    let latest = parse_release_version(&release.tag_name).map_err(|message| {
        IpcError::new(
            IpcErrorCode::ApiResponseInvalid,
            format!("GitHub 版本号无效：{message}"),
        )
    })?;

    Ok(UpdateCheckView {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        update_available: latest > current,
        release_url: release.html_url,
    })
}

#[cfg(any(windows, test))]
fn parse_release_version(value: &str) -> Result<Version, String> {
    Version::parse(value.trim().trim_start_matches(['v', 'V'])).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn load_translation_settings(app: &tauri::AppHandle) -> Result<TranslationSettings, IpcError> {
    let path = translation_settings_path(app)?;
    let mut settings =
        crate::core::translation::load_settings(&path).map_err(map_translation_error)?;
    if !crate::platform::windows::game_monitor::is_supported_chat_key(&settings.overlay_chat_key) {
        settings.overlay_chat_key = crate::core::translation::DEFAULT_OVERLAY_CHAT_KEY.to_owned();
    }
    Ok(settings)
}

#[cfg(windows)]
pub fn initialize_runtime_settings(app: &tauri::AppHandle) -> TranslationSettings {
    let settings = load_translation_settings(app).unwrap_or_default();
    crate::platform::windows::game_monitor::set_overlay_enabled(settings.game_overlay_enabled);
    crate::platform::windows::game_monitor::set_auto_lock_caps(settings.auto_lock_caps);
    let _ = crate::platform::windows::game_monitor::set_chat_key(&settings.overlay_chat_key);
    settings
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
        TranslationError::InvalidHudPosition => {
            IpcError::new(IpcErrorCode::ApiConfiguration, "HUD 位置无效，请重置后重试")
        }
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
        incoming_translation_display_mode: settings.incoming_translation_display_mode,
        translation_hud_position: settings.translation_hud_position,
        game_overlay_enabled: settings.game_overlay_enabled,
        overlay_chat_key: if crate::platform::windows::game_monitor::is_supported_chat_key(
            settings.overlay_chat_key.trim(),
        ) {
            settings.overlay_chat_key.trim().to_owned()
        } else {
            current.overlay_chat_key
        },
        auto_lock_caps: settings.auto_lock_caps,
        game_input_method: normalize_game_input_method(settings.game_input_method),
        game_input_delay_ms: clamp_game_input_delay_ms(settings.game_input_delay_ms),
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
        stratagem_macros: settings
            .stratagem_macros
            .into_iter()
            .take(12)
            .filter_map(|macro_config| {
                let label = macro_config
                    .label
                    .trim()
                    .chars()
                    .take(24)
                    .collect::<String>();
                let sequence = macro_config
                    .sequence
                    .into_iter()
                    .map(|code| normalize_stratagem_direction_code(&code))
                    .filter(|code| !code.is_empty())
                    .take(16)
                    .collect::<Vec<_>>();
                if label.is_empty() || sequence.is_empty() {
                    return None;
                }
                let menu_key = macro_config
                    .menu_key
                    .trim()
                    .chars()
                    .take(32)
                    .collect::<String>();
                Some(StratagemMacro {
                    label,
                    hotkey: macro_config.hotkey.trim().to_owned(),
                    menu_key: if menu_key.is_empty() {
                        "ControlLeft".to_owned()
                    } else {
                        menu_key
                    },
                    menu_mode: macro_config.menu_mode,
                    sequence,
                    menu_open_delay_ms: clamp_stratagem_delay_ms(macro_config.menu_open_delay_ms),
                    press_delay_ms: clamp_stratagem_delay_ms(macro_config.press_delay_ms),
                    interval_delay_ms: clamp_stratagem_delay_ms(macro_config.interval_delay_ms),
                })
            })
            .collect(),
        stratagem_direction_input_mode: settings.stratagem_direction_input_mode,
        stratagem_allow_bare_number_hotkeys: settings.stratagem_allow_bare_number_hotkeys,
    };
    if let Some(region) = next.chat_region {
        region.validate().map_err(map_translation_error)?;
    }
    if let Some(position) = next.translation_hud_position {
        position.validate().map_err(map_translation_error)?;
    }
    let path = translation_settings_path(&app)?;
    crate::core::translation::save_settings(&path, &next).map_err(map_translation_error)?;
    crate::platform::windows::game_monitor::set_overlay_enabled(next.game_overlay_enabled);
    crate::platform::windows::game_monitor::set_auto_lock_caps(next.auto_lock_caps);
    let _ = crate::platform::windows::game_monitor::set_chat_key(&next.overlay_chat_key);
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
    chat_preparation: crate::platform::windows::injector::ChatPreparation,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<crate::platform::InjectionReport, IpcError> {
    record_log(
        &app,
        "quick_shout.start",
        format!(
            "text_chars={} generation={} chat_preparation={chat_preparation:?}",
            text.chars().count(),
            generation.as_deref().unwrap_or("none"),
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
    let settings = load_translation_settings(&app)?;
    let quick_shout_focus_delay_ms = settings.quick_shout_focus_delay_ms;
    let game_input_delay_ms = settings.game_input_delay_ms;
    let preview = map_text_result(build_preview(
        &text,
        config.character_limit,
        config.batch_size,
    ))?;

    let _caps_guard = matches!(settings.game_input_method, GameInputMethod::GbkAltCode)
        .then(crate::platform::windows::game_monitor::suspend_caps_for_text_injection);
    let _gameplay_caps_guard = matches!(
        settings.game_input_method,
        GameInputMethod::UnicodeSendInput
    )
    .then(crate::platform::windows::game_monitor::enforce_gameplay_caps_for_text_injection);

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

    let input_release_started = Instant::now();
    if !wait_for_input_release(Duration::from_millis(900)) {
        record_log(&app, "quick_shout.reject", "input_keys_still_down");
        return Err(IpcError::new(
            IpcErrorCode::SubmitKeyStillDown,
            "喊话快捷键尚未稳定释放，请松开按键后重试",
        ));
    }
    record_log(
        &app,
        "quick_shout.wait",
        format!(
            "input_keys_released elapsed_ms={}",
            input_release_started.elapsed().as_millis()
        ),
    );

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

    if matches!(
        chat_preparation,
        crate::platform::windows::injector::ChatPreparation::KeepOpen
    ) {
        thread::sleep(Duration::from_millis(quick_shout_focus_delay_ms));
    }

    record_log(
        &app,
        "quick_shout.inject",
        format!(
            "begin chat_preparation={chat_preparation:?} focus_delay_ms={quick_shout_focus_delay_ms} input_method={:?} input_chars={} input_delay_ms={game_input_delay_ms}",
            settings.game_input_method,
            preview.cleaned_text.chars().count(),
        ),
    );
    match crate::platform::windows::injector::inject_utf16_batches(
        &expected_target,
        &preview.utf16_batches,
        config.batch_delay_ms,
        &config.title_keyword,
        chat_preparation,
        quick_shout_focus_delay_ms,
        true,
        settings.game_input_method,
        game_input_delay_ms,
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
            error.report = Some(Box::new(report));
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
                "无法准备游戏聊天框",
            );
            error.report = Some(Box::new(report));
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
            error.report = Some(Box::new(report));
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
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::UnrepresentableCharacter(
            report,
            character,
        )) => {
            record_log(
                &app,
                "quick_shout.failure",
                format!("gbk_unrepresentable character={character:?}"),
            );
            let mut error = IpcError::new(
                IpcErrorCode::TextEncodingUnsupported,
                format!(
                    "字符“{character}”无法用 GBK 发送；请删除该字符或在设置中切换到 Unicode 稳定模式"
                ),
            );
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::KeyboardLayoutUnavailable(
            report,
        )) => {
            record_log(
                &app,
                "quick_shout.failure",
                format!("gbk_keyboard_layout_unavailable report={report:?}"),
            );
            let mut error = IpcError::new(
                IpcErrorCode::KeyboardLayoutUnavailable,
                "GBK 兼容模式无法切换到简体中文键盘布局；请安装中文输入法或改用 Unicode 稳定模式",
            );
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::UnsupportedKey(report, key)) => {
            let mut error = IpcError::new(
                IpcErrorCode::ApiConfiguration,
                format!("不支持的输入按键：{key}"),
            );
            error.report = Some(Box::new(report));
            Err(error)
        }
    }
}

#[cfg(windows)]
#[tauri::command]
pub fn send_stratagem_macro(
    mut macro_config: StratagemMacro,
    direction_input_mode: Option<StratagemDirectionInputMode>,
    generation: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<crate::platform::InjectionReport, IpcError> {
    let direction_input_mode = direction_input_mode.unwrap_or_default();
    macro_config.sequence = macro_config
        .sequence
        .into_iter()
        .map(|code| stratagem_direction_input_code(&code, direction_input_mode))
        .collect();
    record_log(
        &app,
        "stratagem.start",
        format!(
            "label={} sequence_len={} direction_input_mode={:?} generation={}",
            macro_config.label.trim(),
            macro_config.sequence.len(),
            direction_input_mode,
            generation.as_deref().unwrap_or("none"),
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
        record_log(&app, "stratagem.reject", "input_state_uncertain");
        return Err(IpcError::new(
            IpcErrorCode::InputStateUncertain,
            "上次 SendInput 返回了未配对事件；请重启助手后再试",
        ));
    }
    if macro_config.sequence.is_empty() {
        return Err(IpcError::new(
            IpcErrorCode::ApiConfiguration,
            "战备方向序列为空，请先配置方向键",
        ));
    }

    let config = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态不可用"))?
        .clone();
    let _gameplay_caps_guard =
        crate::platform::windows::game_monitor::enforce_gameplay_caps_for_text_injection();
    let expected_target = resolve_quick_shout_target(&app, &state, &config, generation.as_deref())?;
    remember_target(&state, expected_target.clone());
    record_log(
        &app,
        "stratagem.target_ready",
        format!(
            "pid={} hwnd=0x{:X} thread={}",
            expected_target.process_id, expected_target.hwnd, expected_target.thread_id
        ),
    );

    let input_release_started = Instant::now();
    if !wait_for_input_release(Duration::from_millis(900)) {
        record_log(&app, "stratagem.reject", "input_keys_still_down");
        return Err(IpcError::new(
            IpcErrorCode::SubmitKeyStillDown,
            "战备快捷键尚未稳定释放，请松开按键后重试",
        ));
    }
    record_log(
        &app,
        "stratagem.wait",
        format!(
            "input_keys_released elapsed_ms={}",
            input_release_started.elapsed().as_millis()
        ),
    );

    let restored = crate::platform::windows::target::restore_foreground(
        &expected_target,
        &config.title_keyword,
    )
    .map_err(|error| {
        record_log(
            &app,
            "stratagem.reject",
            format!("final_restore_error={error:?}"),
        );
        IpcError::new(IpcErrorCode::WindowUnavailable, "无法恢复 HD2 窗口")
    })?;
    if !restored {
        record_log(&app, "stratagem.reject", "final_restore_failed");
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
        record_log(&app, "stratagem.wait", "target_not_stable_before_injection");
        if Instant::now() >= deadline {
            record_log(&app, "stratagem.reject", "target_not_stable_timeout");
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
        "stratagem.inject",
        format!(
            "begin menu_key={} menu_mode={:?} sequence_len={} menu_delay_ms={} press_delay_ms={} interval_delay_ms={}",
            macro_config.menu_key,
            macro_config.menu_mode,
            macro_config.sequence.len(),
            macro_config.menu_open_delay_ms,
            macro_config.press_delay_ms,
            macro_config.interval_delay_ms,
        ),
    );
    match crate::platform::windows::injector::inject_stratagem_macro(
        &expected_target,
        &macro_config,
        &config.title_keyword,
    ) {
        Ok(report) => {
            record_log(&app, "stratagem.success", format!("report={report:?}"));
            Ok(report)
        }
        Err(crate::platform::windows::injector::InjectionError::TargetChanged(report)) => {
            record_log(
                &app,
                "stratagem.failure",
                format!("target_changed report={report:?}"),
            );
            let mut error =
                IpcError::new(IpcErrorCode::TargetChanged, "战备输入过程中 HD2 窗口已变化");
            error.partial_prefix_possible = report.partial_prefix_possible;
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::SendInputFailed(report)) => {
            record_log(
                &app,
                "stratagem.failure",
                format!("send_input_failed report={report:?}"),
            );
            if report.key_state_uncertain {
                if let Ok(mut uncertain) = state.input_state_uncertain.lock() {
                    *uncertain = true;
                }
            }
            let mut error = IpcError::partial("战备按键可能只输入了部分序列，请检查游戏状态");
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::UnsupportedKey(report, key)) => {
            record_log(
                &app,
                "stratagem.failure",
                format!("unsupported_key={key} report={report:?}"),
            );
            let mut error = IpcError::new(
                IpcErrorCode::ApiConfiguration,
                format!("不支持的战备按键：{key}"),
            );
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(error) => {
            record_log(
                &app,
                "stratagem.failure",
                format!("unexpected_error={error:?}"),
            );
            Err(IpcError::new(
                IpcErrorCode::SendInputPartial,
                "战备输入失败",
            ))
        }
    }
}

#[cfg(windows)]
#[tauri::command]
pub fn cancel_overlay_chat(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), IpcError> {
    let _injection_gate = state
        .injection_gate
        .try_lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "已有输入事务正在进行"))?;
    if *state
        .input_state_uncertain
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "输入安全状态不可用"))?
    {
        record_log(&app, "overlay.cancel_reject", "input_state_uncertain");
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
    crate::platform::windows::game_monitor::prepare_gameplay();
    let expected_target = resolve_quick_shout_target(&app, &state, &config, None)?;
    if !wait_for_input_release(Duration::from_millis(500)) {
        return Err(IpcError::new(
            IpcErrorCode::SubmitKeyStillDown,
            "取消键或修饰键尚未稳定释放",
        ));
    }
    let restored = crate::platform::windows::target::restore_foreground(
        &expected_target,
        &config.title_keyword,
    )
    .map_err(|_| IpcError::new(IpcErrorCode::WindowUnavailable, "无法恢复 HD2 窗口"))?;
    if !restored {
        return Err(IpcError::new(
            IpcErrorCode::TargetChanged,
            "HD2 未能稳定恢复前台",
        ));
    }
    let deadline = Instant::now() + Duration::from_millis(OVERLAY_CANCEL_FOREGROUND_TIMEOUT_MS);
    while crate::platform::windows::target::validate_foreground(
        &expected_target,
        &config.title_keyword,
    ) != Ok(true)
    {
        if Instant::now() >= deadline {
            record_log(&app, "overlay.cancel_failure", "target_not_stable_timeout");
            return Err(IpcError::new(
                IpcErrorCode::TargetChanged,
                "HD2 未能稳定恢复前台，无法发送 Esc",
            ));
        }
        let _ = crate::platform::windows::target::restore_foreground(
            &expected_target,
            &config.title_keyword,
        );
        thread::sleep(Duration::from_millis(25));
    }
    thread::sleep(Duration::from_millis(OVERLAY_CANCEL_FOCUS_DELAY_MS));
    match crate::platform::windows::injector::cancel_open_chat(
        &expected_target,
        &config.title_keyword,
    ) {
        Ok(()) => {
            record_log(&app, "overlay.cancel", "escape_injected");
            Ok(())
        }
        Err(crate::platform::windows::injector::InjectionError::TargetChanged(report)) => {
            record_log(
                &app,
                "overlay.cancel_failure",
                format!("target_changed report={report:?}"),
            );
            let mut error = IpcError::new(IpcErrorCode::TargetChanged, "取消输入前 HD2 窗口已变化");
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::SendInputFailed(report)) => {
            record_log(
                &app,
                "overlay.cancel_failure",
                format!("send_input_failed report={report:?}"),
            );
            let mut error =
                IpcError::new(IpcErrorCode::SendInputPartial, "无法向游戏聊天框发送 Esc");
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(error) => {
            record_log(
                &app,
                "overlay.cancel_failure",
                format!("unexpected_inject_error={error:?}"),
            );
            Err(IpcError::new(
                IpcErrorCode::SendInputPartial,
                "无法向游戏聊天框发送 Esc",
            ))
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
            ensure_injection_integrity(target.process_id)?;
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
            "Windows 阻止低权限助手控制高权限游戏。请关闭游戏和助手后重新打开，优先都普通运行；如果游戏必须管理员运行，助手也必须管理员运行",
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
    record_log(
        &app,
        "ocr.start",
        format!(
            "generation={generation_number} hwnd=0x{:X} region=({:.4},{:.4},{:.4},{:.4}) configured_language={}",
            target.hwnd, region.x, region.y, region.width, region.height, settings.ocr_language
        ),
    );
    prepare_chat_capture_target(&target, &title_keyword).map_err(|error| {
        record_log(
            &app,
            "ocr.failure",
            format!("stage=prepare error={error:?}"),
        );
        error
    })?;
    let captured = crate::platform::windows::capture::capture_client(&target, &title_keyword)
        .map_err(|error| {
            record_log(
                &app,
                "ocr.failure",
                format!("stage=capture error={error:?}"),
            );
            map_translation_error(error)
        })?;
    let brightness = captured.sampled_brightness_range();
    record_log(
        &app,
        "ocr.capture",
        format!(
            "full_size={}x{} brightness={brightness:?}",
            captured.width, captured.height
        ),
    );
    let image = captured.crop(region).map_err(|error| {
        record_log(&app, "ocr.failure", format!("stage=crop error={error:?}"));
        map_translation_error(error)
    })?;
    record_log(
        &app,
        "ocr.capture",
        format!(
            "cropped_size={}x{} brightness={:?}",
            image.width,
            image.height,
            image.sampled_brightness_range()
        ),
    );
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
        .map_err(|error| {
            record_log(
                &app,
                "ocr.failure",
                format!("stage=recognize error={error:?}"),
            );
            map_translation_error(error)
        })?;
    let parsed_lines = crate::core::translation::merge_bilingual_ocr_lines(
        &chinese_reading.lines,
        &english_reading.lines,
    );
    let pending_lines = state
        .chat_line_tracker
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "聊天增量状态不可用"))?
        .pending_lines(generation_number, &parsed_lines);
    record_log(
        &app,
        "ocr.result",
        format!(
            "chinese_language={} chinese_lines={} english_language={} english_lines={} parsed_lines={} pending_lines={}",
            chinese_reading.language,
            chinese_reading.lines.len(),
            english_reading.language,
            english_reading.lines.len(),
            parsed_lines.len(),
            pending_lines.len()
        ),
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
            .map_err(|error| {
                record_log(
                    &app,
                    "ocr.failure",
                    format!("stage=translate error={error:?}"),
                );
                map_translation_error(error)
            })?;
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
fn prepare_chat_capture_target(
    expected_target: &crate::core::session::TargetIdentity,
    title_keyword: &str,
) -> Result<(), IpcError> {
    if !wait_for_input_release(Duration::from_millis(900)) {
        return Err(IpcError::new(
            IpcErrorCode::SubmitKeyStillDown,
            "截图翻译快捷键尚未稳定释放，请松开按键后重试",
        ));
    }

    let restored =
        crate::platform::windows::target::restore_foreground(expected_target, title_keyword)
            .map_err(|_| IpcError::new(IpcErrorCode::WindowUnavailable, "无法恢复 HD2 窗口"))?;
    if !restored {
        return Err(IpcError::new(
            IpcErrorCode::TargetChanged,
            "HD2 未能恢复前台；请确认游戏未最小化，或重新捕获目标",
        ));
    }

    let deadline = Instant::now() + Duration::from_millis(1_500);
    while crate::platform::windows::target::validate_foreground(expected_target, title_keyword)
        != Ok(true)
    {
        if Instant::now() >= deadline {
            return Err(IpcError::new(
                IpcErrorCode::TargetChanged,
                "HD2 未能稳定恢复前台；请确认游戏未最小化",
            ));
        }
        let _ =
            crate::platform::windows::target::restore_foreground(expected_target, title_keyword);
        thread::sleep(Duration::from_millis(25));
    }
    thread::sleep(Duration::from_millis(100));
    Ok(())
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
            "Windows 阻止低权限助手控制高权限游戏。请关闭游戏和助手后重新打开，优先都普通运行；如果游戏必须管理员运行，助手也必须管理员运行",
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
    chat_preparation: crate::platform::windows::injector::ChatPreparation,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<crate::platform::InjectionReport, IpcError> {
    let generation = generation
        .parse::<u64>()
        .map_err(|_| IpcError::new(IpcErrorCode::InvalidSession, "会话编号无效"))?;
    record_log(
        &app,
        "probe_injection.start",
        format!(
            "generation={generation} text_chars={} submit={submit} chat_preparation={chat_preparation:?}",
            text.chars().count()
        ),
    );
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
    let input_settings = load_translation_settings(&app)?;
    let input_method = input_settings.game_input_method;
    let game_input_delay_ms = input_settings.game_input_delay_ms;
    let chat_focus_delay_ms = input_settings.quick_shout_focus_delay_ms;
    let _caps_guard = matches!(input_method, GameInputMethod::GbkAltCode)
        .then(crate::platform::windows::game_monitor::suspend_caps_for_text_injection);
    let _gameplay_caps_guard = matches!(input_method, GameInputMethod::UnicodeSendInput)
        .then(crate::platform::windows::game_monitor::enforce_gameplay_caps_for_text_injection);
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

    let input_release_started = Instant::now();
    if !wait_for_input_release(Duration::from_millis(500)) {
        abort_submit(&state, generation, "提交键或修饰键未稳定释放");
        return Err(IpcError::new(
            IpcErrorCode::SubmitKeyStillDown,
            "Enter、Ctrl、Alt、Shift 或 Win 尚未稳定释放",
        ));
    }
    record_log(
        &app,
        "probe_injection.wait",
        format!(
            "input_keys_released elapsed_ms={}",
            input_release_started.elapsed().as_millis()
        ),
    );

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
            let _ = session.abort_submit(generation, "提交键或修饰键重新进入按下状态");
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
        abort_submit(&state, generation, "系统暂时拒绝恢复目标窗口前台");
        return Err(IpcError::new(
            IpcErrorCode::TargetChanged,
            "Windows 暂时未把前台交还 HD2，请回到游戏后重试；目标锁定仍然保留",
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
                abort_submit(&state, generation, "目标窗口未能稳定恢复前台");
                return Err(IpcError::new(
                    IpcErrorCode::TargetChanged,
                    "HD2 前台状态暂时不稳定，请回到游戏后重试；目标锁定仍然保留",
                ));
            }
            _ => thread::sleep(Duration::from_millis(25)),
        }
    };
    record_log(
        &app,
        "probe_injection.foreground",
        format!(
            "stage=verified generation={generation} hwnd=0x{:X} pid={}",
            current_target.hwnd, current_target.process_id
        ),
    );

    if matches!(
        chat_preparation,
        crate::platform::windows::injector::ChatPreparation::KeepOpen
    ) {
        thread::sleep(Duration::from_millis(chat_focus_delay_ms));
    }

    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))?;
        session
            .begin_injection(generation, &current_target)
            .map_err(map_session_error)?;
    }

    record_log(
        &app,
        "probe_injection.inject",
        format!(
            "stage=begin generation={generation} input_method={input_method:?} chat_preparation={chat_preparation:?} input_chars={} input_delay_ms={game_input_delay_ms} focus_delay_ms={chat_focus_delay_ms}",
            preview.cleaned_text.chars().count()
        ),
    );
    let result = crate::platform::windows::injector::inject_utf16_batches(
        &expected_target,
        &preview.utf16_batches,
        config.batch_delay_ms,
        &config.title_keyword,
        chat_preparation,
        chat_focus_delay_ms,
        submit,
        input_method,
        game_input_delay_ms,
    );
    let mut session = state
        .session
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "会话状态不可用"))?;
    match result {
        Ok(report) => {
            record_log(
                &app,
                "probe_injection.success",
                format!("generation={generation} report={report:?}"),
            );
            session
                .complete_injection(generation)
                .map_err(map_session_error)?;
            Ok(report)
        }
        Err(crate::platform::windows::injector::InjectionError::TargetChanged(report)) => {
            record_log(
                &app,
                "probe_injection.failure",
                format!("generation={generation} stage=target_changed report={report:?}"),
            );
            let _ = session.fail_injection(generation, "发送过程中目标窗口已变化");
            let mut error = IpcError::new(IpcErrorCode::TargetChanged, "发送过程中目标窗口已变化");
            error.partial_prefix_possible = report.partial_prefix_possible;
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::OpenChatFailed(report)) => {
            record_log(
                &app,
                "probe_injection.failure",
                format!("generation={generation} stage=open_chat_failed report={report:?}"),
            );
            let _ = session.fail_injection(generation, "未能稳定打开游戏聊天框");
            let mut error = IpcError::new(
                IpcErrorCode::SendInputPartial,
                "未能稳定打开游戏聊天框，请确认助手里的游戏聊天键和 HD2 设置一致后重试",
            );
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::SendInputFailed(report)) => {
            record_log(
                &app,
                "probe_injection.failure",
                format!("generation={generation} stage=text report={report:?}"),
            );
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
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::SubmitFailed(report)) => {
            record_log(
                &app,
                "probe_injection.failure",
                format!("generation={generation} stage=submit report={report:?}"),
            );
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
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::UnrepresentableCharacter(
            report,
            character,
        )) => {
            let _ = session.fail_injection(generation, "文本包含 GBK 无法表示的字符");
            let mut error = IpcError::new(
                IpcErrorCode::TextEncodingUnsupported,
                format!(
                    "字符“{character}”无法用 GBK 发送；请删除该字符或在设置中切换到 Unicode 稳定模式"
                ),
            );
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::KeyboardLayoutUnavailable(
            report,
        )) => {
            let _ = session.fail_injection(generation, "无法切换到简体中文键盘布局");
            let mut error = IpcError::new(
                IpcErrorCode::KeyboardLayoutUnavailable,
                "GBK 兼容模式无法切换到简体中文键盘布局；请安装中文输入法或改用 Unicode 稳定模式",
            );
            error.report = Some(Box::new(report));
            Err(error)
        }
        Err(crate::platform::windows::injector::InjectionError::UnsupportedKey(report, key)) => {
            let _ = session.fail_injection(generation, "输入按键配置不受支持");
            let mut error = IpcError::new(
                IpcErrorCode::ApiConfiguration,
                format!("不支持的输入按键：{key}"),
            );
            error.report = Some(Box::new(report));
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

#[cfg(windows)]
fn abort_submit(state: &tauri::State<'_, AppState>, generation: u64, message: &str) {
    if let Ok(mut session) = state.session.lock() {
        let _ = session.abort_submit(generation, message);
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

    #[test]
    fn release_versions_are_compared_semantically() {
        let current = parse_release_version("v0.9.0").unwrap();
        let latest = parse_release_version("0.10.0").unwrap();
        assert!(latest > current);
        assert_eq!(
            parse_release_version("V1.2.3").unwrap(),
            Version::new(1, 2, 3)
        );
        assert!(parse_release_version("latest").is_err());
    }
}
