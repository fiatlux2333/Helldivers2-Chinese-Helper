use serde::Serialize;
use std::{
    mem::size_of,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    UI::{
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
            KEYEVENTF_KEYUP, SendInput, VK_CAPITAL, VK_CONTROL, VK_MENU, VK_SHIFT,
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GA_ROOT, GetAncestor, GetClassNameW,
            GetForegroundWindow, GetMessageW, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, IsIconic, IsWindowVisible, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
            MSG, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
            WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};

pub const DEFAULT_OVERLAY_CHAT_KEY: &str = "Enter";
pub const GAME_FOREGROUND_EVENT: &str = "game-foreground-changed";
pub const GAME_CHAT_KEY_PREPARE_EVENT: &str = "game-chat-key-prepare";
pub const GAME_CHAT_KEY_PHYSICAL_EVENT: &str = "game-chat-key-physical";
pub const GAME_CHAT_ESCAPE_PHYSICAL_EVENT: &str = "game-chat-escape-physical";
pub const GAME_CHAT_KEY_EVENT: &str = "game-chat-key-released";
const HD2_WINDOW_CLASS: &str = "stingray_window";
const ASSISTANT_WINDOW_TITLES: [&str; 3] = ["HD2CN 中文助手", "HD2CN 聊天译文", "HD2CN 中文侧栏"];
// The independent overlay no longer needs the old main-window resize delay.
// Keep only a short key-up settle so the first typed characters reach the overlay.
const CHAT_TRIGGER_SETTLE_MS: u64 = 20;
const CAPS_TOGGLE_MAX_ATTEMPTS: usize = 3;
const CAPS_TOGGLE_POLL_INTERVAL_MS: u64 = 5;
const CAPS_TOGGLE_SETTLE_TIMEOUT_MS: u64 = 50;

static CHAT_TRIGGER_ENABLED: AtomicBool = AtomicBool::new(true);
static CHAT_TRIGGER_VK: AtomicU32 = AtomicU32::new(0x0D);
static CHAT_TRIGGER_ARMED: AtomicBool = AtomicBool::new(false);
static ESCAPE_TRIGGER_ARMED: AtomicBool = AtomicBool::new(false);
static AUTO_LOCK_CAPS: AtomicBool = AtomicBool::new(true);
static CAPS_PROTECTION_FAULTED: AtomicBool = AtomicBool::new(false);
static TEXT_INJECTION_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAPS_SESSION_ORIGINAL: Mutex<Option<bool>> = Mutex::new(None);
static CAPS_TRANSITION_LOCK: Mutex<()> = Mutex::new(());

pub struct TextInjectionCapsGuard;

impl Drop for TextInjectionCapsGuard {
    fn drop(&mut self) {
        TEXT_INJECTION_ACTIVE.store(false, Ordering::Release);
        prepare_gameplay();
    }
}

pub struct GameplayCapsGuard;

impl Drop for GameplayCapsGuard {
    fn drop(&mut self) {
        prepare_gameplay();
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatTriggerSignal {
    Prepare,
    Show,
}

static CHAT_TRIGGER_TX: OnceLock<mpsc::Sender<ChatTriggerSignal>> = OnceLock::new();
static TITLE_KEYWORD: OnceLock<String> = OnceLock::new();
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameForegroundEvent {
    pub state: ForegroundState,
    pub work_area: Option<WorkArea>,
    pub scale_factor: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ForegroundState {
    Game,
    Assistant,
    Other,
}

pub fn start(app: tauri::AppHandle, title_keyword: String) {
    let _ = APP_HANDLE.set(app.clone());
    let _ = TITLE_KEYWORD.set(title_keyword);
    let (chat_tx, chat_rx) = mpsc::channel();
    let _ = CHAT_TRIGGER_TX.set(chat_tx);

    let chat_app = app.clone();
    thread::spawn(move || {
        while let Ok(signal) = chat_rx.recv() {
            if signal == ChatTriggerSignal::Prepare {
                let snapshot = foreground_snapshot();
                if snapshot.state == ForegroundState::Game {
                    append_runtime_log(
                        &chat_app,
                        "overlay.chat_key",
                        "stage=keydown_prepare state=game",
                    );
                    let _ = chat_app.emit(GAME_CHAT_KEY_PREPARE_EVENT, snapshot);
                    prepare_overlay_input();
                } else {
                    append_runtime_log(
                        &chat_app,
                        "overlay.chat_key_ignored",
                        format!("stage=keydown_prepare state={:?}", snapshot.state),
                    );
                }
                continue;
            }
            thread::sleep(Duration::from_millis(CHAT_TRIGGER_SETTLE_MS));
            let snapshot = foreground_snapshot();
            if snapshot.state != ForegroundState::Game {
                append_runtime_log(
                    &chat_app,
                    "overlay.chat_key_ignored",
                    format!("state={:?}", snapshot.state),
                );
                continue;
            }
            let caps_before = caps_lock_enabled();
            prepare_overlay_input();
            let caps_after = caps_lock_enabled();
            append_runtime_log(
                &chat_app,
                "overlay.chat_key",
                format!(
                    "state=game injected=false caps_before={caps_before} caps_after={caps_after} key_vk={}",
                    CHAT_TRIGGER_VK.load(Ordering::Acquire)
                ),
            );
            if caps_after {
                append_runtime_log(
                    &chat_app,
                    "overlay.caps_error",
                    "phase=chat_focus expected=false actual=true",
                );
            }
            let _ = chat_app.emit(GAME_CHAT_KEY_PHYSICAL_EVENT, snapshot.clone());
            let _ = chat_app.emit(GAME_CHAT_KEY_EVENT, snapshot);
        }
    });

    let monitor_app = app.clone();
    thread::spawn(move || {
        let mut previous = None;
        let mut previous_caps = None;
        loop {
            let snapshot = foreground_snapshot();
            let caps_before = caps_lock_enabled();
            apply_caps_lock_policy(snapshot.state);
            let caps_after = caps_lock_enabled();
            if previous.as_ref() != Some(&snapshot) || previous_caps != Some(caps_after) {
                let injection_active = TEXT_INJECTION_ACTIVE.load(Ordering::Acquire);
                append_runtime_log(
                    &monitor_app,
                    "overlay.foreground",
                    format!(
                        "state={:?} caps_before={caps_before} caps_after={caps_after} injection_active={injection_active} work_area={:?}",
                        snapshot.state, snapshot.work_area,
                    ),
                );
                if AUTO_LOCK_CAPS.load(Ordering::Acquire)
                    && matches!(snapshot.state, ForegroundState::Game)
                    && caps_after == injection_active
                {
                    append_runtime_log(
                        &monitor_app,
                        "overlay.caps_error",
                        format!(
                            "phase=game expected={} actual={caps_after} injection_active={injection_active}",
                            !injection_active
                        ),
                    );
                }
                if AUTO_LOCK_CAPS.load(Ordering::Acquire)
                    && matches!(snapshot.state, ForegroundState::Assistant)
                    && caps_session_active()
                    && caps_after
                {
                    append_runtime_log(
                        &monitor_app,
                        "overlay.caps_error",
                        "phase=assistant expected=false actual=true",
                    );
                }
            }
            if previous.as_ref() != Some(&snapshot) {
                let _ = monitor_app.emit(GAME_FOREGROUND_EVENT, snapshot.clone());
                previous = Some(snapshot);
            }
            previous_caps = Some(caps_after);
            thread::sleep(Duration::from_millis(250));
        }
    });

    let hook_app = app.clone();
    thread::spawn(move || run_keyboard_hook(hook_app));
}

pub fn set_overlay_enabled(enabled: bool) {
    CHAT_TRIGGER_ENABLED.store(enabled, Ordering::Release);
    if !enabled {
        CHAT_TRIGGER_ARMED.store(false, Ordering::Release);
    }
}

pub fn set_auto_lock_caps(enabled: bool) {
    AUTO_LOCK_CAPS.store(enabled, Ordering::Release);
    if enabled {
        CAPS_PROTECTION_FAULTED.store(false, Ordering::Release);
    } else {
        restore_caps_lock();
    }
}

pub fn restore_caps_lock() {
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        return;
    };
    restore_caps_lock_locked("restore");
}

pub fn prepare_overlay_input() {
    if !caps_protection_active() {
        return;
    }
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        return;
    };
    prepare_caps_lock_locked(false, "prepare_overlay");
}

pub fn prepare_gameplay() {
    if !caps_protection_active() {
        return;
    }
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        return;
    };
    prepare_caps_lock_locked(true, "prepare_gameplay");
}

pub fn suspend_caps_for_text_injection() -> TextInjectionCapsGuard {
    TEXT_INJECTION_ACTIVE.store(true, Ordering::Release);
    prepare_overlay_input();
    TextInjectionCapsGuard
}

pub fn enforce_gameplay_caps_for_text_injection() -> GameplayCapsGuard {
    prepare_gameplay();
    GameplayCapsGuard
}

pub fn set_chat_key(code: &str) -> bool {
    let Some(vk) = virtual_key_for_code(code) else {
        return false;
    };
    CHAT_TRIGGER_VK.store(vk, Ordering::Release);
    CHAT_TRIGGER_ARMED.store(false, Ordering::Release);
    true
}

pub fn is_supported_chat_key(code: &str) -> bool {
    virtual_key_for_code(code).is_some()
}

fn run_keyboard_hook(app: tauri::AppHandle) {
    let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0) }
    {
        Ok(hook) => hook,
        Err(error) => {
            append_runtime_log(
                &app,
                "overlay.keyboard_hook_failed",
                format!("error={error}"),
            );
            return;
        }
    };

    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    let _ = unsafe { UnhookWindowsHookEx(hook) };
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if event.flags.contains(LLKHF_INJECTED) {
            return unsafe { CallNextHookEx(None, code, wparam, lparam) };
        }

        if CHAT_TRIGGER_ENABLED.load(Ordering::Acquire)
            && event.vkCode == CHAT_TRIGGER_VK.load(Ordering::Acquire)
        {
            let (trigger, armed) = resolve_chat_key_trigger(
                wparam.0 as u32,
                modifier_is_down(),
                foreground_snapshot().state,
                CHAT_TRIGGER_ARMED.load(Ordering::Acquire),
            );
            let was_armed = CHAT_TRIGGER_ARMED.swap(armed, Ordering::AcqRel);
            if armed && !was_armed {
                if let Some(sender) = CHAT_TRIGGER_TX.get() {
                    let _ = sender.send(ChatTriggerSignal::Prepare);
                }
            }
            if trigger {
                if let Some(sender) = CHAT_TRIGGER_TX.get() {
                    let _ = sender.send(ChatTriggerSignal::Show);
                }
            }
        }

        if event.vkCode == 0x1B {
            let trigger = resolve_escape_trigger(
                wparam.0 as u32,
                modifier_is_down(),
                foreground_snapshot().state,
                ESCAPE_TRIGGER_ARMED.load(Ordering::Acquire),
            );
            let was_armed = ESCAPE_TRIGGER_ARMED.swap(trigger.1, Ordering::AcqRel);
            if trigger.0 && was_armed {
                if let Some(app) = APP_HANDLE.get() {
                    let snapshot = foreground_snapshot();
                    if snapshot.state == ForegroundState::Game {
                        append_runtime_log(app, "overlay.escape_key", "state=game injected=false");
                        let _ = app.emit(GAME_CHAT_ESCAPE_PHYSICAL_EVENT, snapshot);
                    }
                }
            }
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn resolve_chat_key_trigger(
    message: u32,
    modifier_down: bool,
    state: ForegroundState,
    was_armed: bool,
) -> (bool, bool) {
    if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
        return (
            false,
            !modifier_down && matches!(state, ForegroundState::Game),
        );
    }
    if message == WM_KEYUP || message == WM_SYSKEYUP {
        return (
            was_armed && !modifier_down && matches!(state, ForegroundState::Game),
            false,
        );
    }
    (false, was_armed)
}

fn resolve_escape_trigger(
    message: u32,
    modifier_down: bool,
    state: ForegroundState,
    was_armed: bool,
) -> (bool, bool) {
    if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
        return (
            false,
            !modifier_down && matches!(state, ForegroundState::Game),
        );
    }
    if message == WM_KEYUP || message == WM_SYSKEYUP {
        return (
            was_armed && !modifier_down && matches!(state, ForegroundState::Game),
            false,
        );
    }
    (false, was_armed)
}

fn modifier_is_down() -> bool {
    [VK_CONTROL, VK_MENU, VK_SHIFT]
        .into_iter()
        .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0)
}

fn apply_caps_lock_policy(state: ForegroundState) {
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        return;
    };
    if !AUTO_LOCK_CAPS.load(Ordering::Acquire) {
        restore_caps_lock_locked("disabled_restore");
        return;
    }

    match state {
        ForegroundState::Game => {
            if !caps_protection_active() {
                return;
            }
            if TEXT_INJECTION_ACTIVE.load(Ordering::Acquire) {
                prepare_caps_lock_locked(false, "monitor_game_injection");
            } else {
                prepare_caps_lock_locked(true, "monitor_game");
            }
        }
        ForegroundState::Assistant if caps_session_active() && caps_protection_active() => {
            ensure_caps_lock_locked(false, "monitor_assistant");
        }
        ForegroundState::Other => {
            restore_caps_lock_locked("monitor_other");
        }
        ForegroundState::Assistant => {}
    }
}

fn caps_protection_active() -> bool {
    AUTO_LOCK_CAPS.load(Ordering::Acquire) && !CAPS_PROTECTION_FAULTED.load(Ordering::Acquire)
}

fn prepare_caps_lock_locked(expected: bool, phase: &str) {
    if let Ok(mut original) = CAPS_SESSION_ORIGINAL.lock() {
        if original.is_none() {
            *original = Some(caps_lock_enabled());
        }
    }
    let _ = ensure_caps_lock_locked(expected, phase);
}

fn restore_caps_lock_locked(phase: &str) {
    let original_state = CAPS_SESSION_ORIGINAL
        .lock()
        .ok()
        .and_then(|original| *original);
    let Some(original_state) = original_state else {
        return;
    };
    if ensure_caps_lock_locked(original_state, phase) {
        if let Ok(mut original) = CAPS_SESSION_ORIGINAL.lock() {
            if *original == Some(original_state) {
                *original = None;
            }
        }
    }
}

fn caps_lock_enabled() -> bool {
    (unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1) == 1
}

fn caps_session_active() -> bool {
    CAPS_SESSION_ORIGINAL
        .lock()
        .map(|original| original.is_some())
        .unwrap_or(false)
}

fn ensure_caps_lock_locked(expected: bool, phase: &str) -> bool {
    for attempt in 1..=CAPS_TOGGLE_MAX_ATTEMPTS {
        let actual = caps_lock_enabled();
        append_caps_log(format!(
            "phase={phase} attempt={attempt} expected={expected} actual={actual} stage=check"
        ));
        if actual == expected {
            return true;
        }
        if unsafe { GetAsyncKeyState(VK_CAPITAL.0 as i32) } < 0 {
            append_caps_log(format!(
                "phase={phase} attempt={attempt} expected={expected} actual={actual} stage=physical_key_down"
            ));
            return false;
        }

        let inputs = [caps_lock_input(false), caps_lock_input(true)];
        let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if inserted == 1 {
            let key_up = [caps_lock_input(true)];
            let _ = unsafe { SendInput(&key_up, size_of::<INPUT>() as i32) };
        }
        append_caps_log(format!(
            "phase={phase} attempt={attempt} expected={expected} actual={actual} stage=toggle inserted={inserted}"
        ));

        let deadline = Instant::now() + Duration::from_millis(CAPS_TOGGLE_SETTLE_TIMEOUT_MS);
        while Instant::now() < deadline {
            let actual = caps_lock_enabled();
            if actual == expected {
                append_caps_log(format!(
                    "phase={phase} attempt={attempt} expected={expected} actual={actual} stage=verified"
                ));
                return true;
            }
            thread::sleep(Duration::from_millis(CAPS_TOGGLE_POLL_INTERVAL_MS));
        }
    }

    let actual = caps_lock_enabled();
    CAPS_PROTECTION_FAULTED.store(true, Ordering::Release);
    append_caps_log(format!(
        "phase={phase} attempt={CAPS_TOGGLE_MAX_ATTEMPTS} expected={expected} actual={actual} stage=faulted"
    ));
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit(
            "caps-protection-failed",
            "CapsLock 输入法保护切换失败，已暂停自动切换。请手动调整 CapsLock，或在设置中关闭后重新启用输入法保护。",
        );
    }
    false
}

fn append_caps_log(message: impl AsRef<str>) {
    if let Some(app) = APP_HANDLE.get() {
        append_runtime_log(app, "overlay.caps_transition", message);
    } else {
        eprintln!("[hd2cn][overlay.caps_transition] {}", message.as_ref());
    }
}

fn caps_lock_input(key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_CAPITAL,
                wScan: 0,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn append_runtime_log(app: &tauri::AppHandle, stage: &str, message: impl AsRef<str>) {
    let message = message.as_ref().replace(['\r', '\n'], " ");
    if let Ok(path) = app.path().app_config_dir() {
        let _ = crate::core::logging::append(&path.join("runtime.log"), stage, &message);
    }
    eprintln!("[hd2cn][{stage}] {message}");
}

fn foreground_snapshot() -> GameForegroundEvent {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() {
        return other_foreground();
    }
    let root = unsafe { GetAncestor(foreground, GA_ROOT) };
    let hwnd = if root.0.is_null() { foreground } else { root };

    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    let title = window_title(hwnd);
    if is_assistant_window(process_id, title.as_deref()) {
        return GameForegroundEvent {
            state: ForegroundState::Assistant,
            work_area: None,
            scale_factor: None,
        };
    }

    let Some(title) = title else {
        return other_foreground();
    };
    let keyword = TITLE_KEYWORD
        .get()
        .map(String::as_str)
        .unwrap_or("HELLDIVERS");
    let class = window_class(hwnd).unwrap_or_default();
    if !title
        .to_uppercase()
        .contains(&keyword.trim().to_uppercase())
        || !class.eq_ignore_ascii_case(HD2_WINDOW_CLASS)
        || !unsafe { IsWindowVisible(hwnd).as_bool() }
        || unsafe { IsIconic(hwnd).as_bool() }
    {
        return other_foreground();
    }

    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut monitor_info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let work_area = if unsafe { GetMonitorInfoW(monitor, &mut monitor_info) }.as_bool() {
        let rect = monitor_info.rcWork;
        Some(WorkArea {
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left).max(0) as u32,
            height: (rect.bottom - rect.top).max(0) as u32,
        })
    } else {
        None
    };
    let dpi = unsafe { GetDpiForWindow(hwnd) };

    GameForegroundEvent {
        state: ForegroundState::Game,
        work_area,
        scale_factor: Some(if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 }),
    }
}

fn other_foreground() -> GameForegroundEvent {
    GameForegroundEvent {
        state: ForegroundState::Other,
        work_area: None,
        scale_factor: None,
    }
}

fn is_assistant_window(process_id: u32, title: Option<&str>) -> bool {
    process_id == std::process::id()
        || (process_id == 0
            && title.is_some_and(|title| {
                ASSISTANT_WINDOW_TITLES
                    .iter()
                    .any(|expected| title.eq_ignore_ascii_case(expected))
            }))
}

fn window_title(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return None;
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if copied <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..copied as usize]))
}

fn window_class(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    let mut buffer = [0u16; 256];
    let copied = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if copied <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..copied as usize]))
}

fn virtual_key_for_code(code: &str) -> Option<u32> {
    match code {
        "Enter" => Some(0x0D),
        "Space" => Some(0x20),
        "Tab" => Some(0x09),
        "Backquote" => Some(0xC0),
        "Minus" => Some(0xBD),
        "Equal" => Some(0xBB),
        "BracketLeft" => Some(0xDB),
        "BracketRight" => Some(0xDD),
        "Backslash" => Some(0xDC),
        "Semicolon" => Some(0xBA),
        "Quote" => Some(0xDE),
        "Comma" => Some(0xBC),
        "Period" => Some(0xBE),
        "Slash" => Some(0xBF),
        value if value.len() == 4 && value.starts_with("Key") => value
            .as_bytes()
            .get(3)
            .copied()
            .filter(u8::is_ascii_uppercase)
            .map(u32::from),
        value if value.len() == 6 && value.starts_with("Digit") => value
            .as_bytes()
            .get(5)
            .copied()
            .filter(u8::is_ascii_digit)
            .map(u32::from),
        value if value.starts_with('F') => value[1..]
            .parse::<u32>()
            .ok()
            .filter(|number| (1..=12).contains(number))
            .map(|number| 0x6F + number),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_supported_dom_codes_to_windows_virtual_keys() {
        assert_eq!(virtual_key_for_code("Enter"), Some(0x0D));
        assert_eq!(virtual_key_for_code("KeyT"), Some(u32::from(b'T')));
        assert_eq!(virtual_key_for_code("Digit7"), Some(u32::from(b'7')));
        assert_eq!(virtual_key_for_code("F12"), Some(0x7B));
        assert_eq!(virtual_key_for_code("ControlLeft"), None);
    }

    #[test]
    fn recognizes_assistant_titles_when_windows_returns_no_process_id() {
        assert!(is_assistant_window(0, Some("HD2CN 中文助手")));
        assert!(is_assistant_window(0, Some("HD2CN 聊天译文")));
        assert!(is_assistant_window(0, Some("HD2CN 中文侧栏")));
        assert!(!is_assistant_window(0, Some("HELLDIVERS™ 2")));
        assert!(!is_assistant_window(1234, Some("HD2CN 中文助手")));
    }

    #[test]
    fn chat_key_release_must_start_in_game_foreground() {
        let (trigger, armed) =
            resolve_chat_key_trigger(WM_KEYUP, false, ForegroundState::Game, false);
        assert!(!trigger);
        assert!(!armed);

        let (trigger, armed) =
            resolve_chat_key_trigger(WM_KEYDOWN, false, ForegroundState::Assistant, false);
        assert!(!trigger);
        assert!(!armed);

        let (trigger, armed) =
            resolve_chat_key_trigger(WM_KEYUP, false, ForegroundState::Game, armed);
        assert!(!trigger);
        assert!(!armed);

        let (trigger, armed) =
            resolve_chat_key_trigger(WM_KEYDOWN, false, ForegroundState::Game, false);
        assert!(!trigger);
        assert!(armed);

        let (trigger, armed) =
            resolve_chat_key_trigger(WM_KEYUP, false, ForegroundState::Game, armed);
        assert!(trigger);
        assert!(!armed);
    }

    #[test]
    fn chat_key_release_is_cancelled_when_focus_leaves_game() {
        let (trigger, armed) =
            resolve_chat_key_trigger(WM_KEYDOWN, false, ForegroundState::Game, false);
        assert!(!trigger);
        assert!(armed);

        let (trigger, armed) =
            resolve_chat_key_trigger(WM_KEYUP, false, ForegroundState::Other, armed);
        assert!(!trigger);
        assert!(!armed);
    }

    #[test]
    fn escape_release_requires_game_foreground_and_unmodified_key() {
        let (trigger, armed) =
            resolve_escape_trigger(WM_KEYDOWN, false, ForegroundState::Game, false);
        assert!(!trigger);
        assert!(armed);

        let (trigger, armed) =
            resolve_escape_trigger(WM_KEYUP, false, ForegroundState::Game, armed);
        assert!(trigger);
        assert!(!armed);

        let (trigger, armed) =
            resolve_escape_trigger(WM_KEYDOWN, false, ForegroundState::Game, false);
        assert!(!trigger);
        assert!(armed);

        let (trigger, armed) =
            resolve_escape_trigger(WM_KEYUP, false, ForegroundState::Other, armed);
        assert!(!trigger);
        assert!(!armed);

        let (trigger, armed) =
            resolve_escape_trigger(WM_KEYDOWN, true, ForegroundState::Game, false);
        assert!(!trigger);
        assert!(!armed);
    }
}
