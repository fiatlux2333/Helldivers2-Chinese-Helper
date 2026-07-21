#[cfg(any(windows, test))]
use crate::core::text::{TextError, TextPreview};
use crate::{
    core::{config::AppConfig, session::SessionMachine},
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
use std::sync::Mutex;
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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: Mutex::new(AppConfig::default()),
            session: Mutex::new(SessionMachine::default()),
            injection_gate: Mutex::new(()),
            input_state_uncertain: Mutex::new(false),
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
    state: tauri::State<'_, AppState>,
) -> Result<TargetDiagnostic, IpcError> {
    let config = state
        .config
        .lock()
        .map_err(|_| IpcError::new(IpcErrorCode::InternalState, "配置状态不可用"))?;
    target_diagnostic(&config)
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
    let generation = session.begin_probe(target);
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
