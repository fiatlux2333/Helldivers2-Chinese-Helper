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
    Foundation::{GetLastError, LPARAM, LRESULT, SetLastError, WIN32_ERROR, WPARAM},
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
            GetWindowThreadProcessId, IsIconic, IsWindowVisible, KBDLLHOOKSTRUCT, LLKHF_EXTENDED,
            LLKHF_INJECTED, MSG, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
            WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
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
const CAPS_BACKGROUND_FAILURE_LIMIT: u32 = 3;

// --- First-key buffer ------------------------------------------------------
// The HD2 chat-key flash (~1.1s after opening, 0.98–1.24s in fan logs) briefly
// drags the foreground back to the game while the user is already typing into
// the sidebar. Those keystrokes land in the game chat box and are lost. While
// a buffer session is armed, the low-level hook swallows plain typing keys
// (letters/digits/space/punctuation/Backspace — never Enter, Esc, modifiers,
// or combos) while the GAME owns the foreground, and replays them into the
// sidebar once JS confirms the composer has focus again.
//
// Degradation contract: the watchdog (covers the measured flash window) and
// capacity overflow STOP swallowing but RETAIN what was captured for the JS
// replay; contents are dropped only when the sidebar session ends (game-side
// Escape) or the next session arms. Replay itself re-checks the foreground
// and defers instead of firing into a foreign window.

/// Upper bound on buffered key events (down+up pairs). 64 keystrokes is far
/// beyond any realistic burst inside the flash window; overflow stops
/// swallowing but keeps what was captured.
const KEY_BUFFER_CAPACITY: usize = 128;
/// Watchdog: stop swallowing once this window after arming has passed. Must
/// exceed the measured flash (0.98–1.24s) so buffered keys survive until the
/// JS replay fires; on timeout the captured keys are retained, not dropped.
const KEY_BUFFER_WATCHDOG_MS: u64 = 1_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferedKeyEvent {
    vk_code: u32,
    scan_code: u16,
    extended: bool,
    key_up: bool,
}

static KEY_BUFFER_ARMED: AtomicBool = AtomicBool::new(false);
static KEY_BUFFER: Mutex<(Vec<BufferedKeyEvent>, Option<Instant>)> = Mutex::new((Vec::new(), None));

/// Arm a first-key buffer session. Called when the sidebar opens (chat key
/// pressed while the game is foreground). There is no watchdog thread: the
/// hook checks the elapsed budget lazily on each candidate key, and the JS
/// replay (or the next session) drains or clears whatever was captured.
pub fn arm_first_key_buffer() {
    if let Ok(mut buffer) = KEY_BUFFER.lock() {
        buffer.0.clear();
        buffer.1 = Some(Instant::now());
    }
    KEY_BUFFER_ARMED.store(true, Ordering::Release);
}

/// Stop swallowing new keys but keep what was captured for a later replay.
fn stop_first_key_buffer_swallow() {
    KEY_BUFFER_ARMED.store(false, Ordering::Release);
}

/// Stop swallowing and drop everything (sidebar session is over).
pub fn discard_first_key_buffer() {
    stop_first_key_buffer_swallow();
    if let Ok(mut buffer) = KEY_BUFFER.lock() {
        buffer.0.clear();
        buffer.1 = None;
    }
}

/// Stop swallowing ONLY when the capture window has expired. A replay call
/// that lands mid-flash (foreground left the assistant but the watchdog has
/// not run out) must keep capturing that flash's keys — disarming here would
/// kill the very capture the retry is for. Expired or timestamp-less sessions
/// disarm (stale armed state must not outlive its purpose).
pub fn stop_first_key_buffer_swallow_if_expired() {
    let expired = KEY_BUFFER
        .lock()
        .ok()
        .and_then(|buffer| {
            buffer
                .1
                .map(|started| started.elapsed() > Duration::from_millis(KEY_BUFFER_WATCHDOG_MS))
        })
        // Lock poisoned / missing timestamp: treat as expired (stop swallowing;
        // the hook-side watchdog self-limit no longer applies once disarmed).
        .unwrap_or(true);
    if expired {
        stop_first_key_buffer_swallow();
    }
}

/// Renew the watchdog start of an ARMED capture session after a settled
/// replay drained the buffer: the game re-asserts itself in bursts, so the
/// next flicker of the burst gets a fresh capture window instead of hitting
/// a disarmed buffer. Disarming stays with the session ends; the hook only
/// ever swallows while the game owns the foreground.
///
/// The armed check and the lock are deliberately NOT atomic: a racing disarm
/// (stop/discard between them) only leaves a stale timestamp behind while
/// ARMED stays false — the hook gates on the atomic so nothing is swallowed,
/// the next arm overwrites the timestamp, and the expired-only stop reading
/// the newer timestamp is at worst a no-op. Harmless; do not "fix".
pub fn renew_first_key_buffer_window() {
    if !first_key_buffer_armed() {
        return;
    }
    if let Ok(mut buffer) = KEY_BUFFER.lock() {
        buffer.1 = Some(Instant::now());
    }
}

fn first_key_buffer_armed() -> bool {
    KEY_BUFFER_ARMED.load(Ordering::Acquire)
}

/// Hook-path gate: cheap checks only (armed atomic, message kind, modifier,
/// plain-typing VK). Returns true when the event should be swallowed; the
/// caller then runs the ONE expensive foreground snapshot and calls
/// `try_buffer_event`. Nothing here may stall: try_lock fails OPEN.
fn buffer_event_if_armed(event: &KBDLLHOOKSTRUCT, wparam_message: u32) -> Option<bool> {
    if !first_key_buffer_armed() {
        return None;
    }
    let message = wparam_message;
    let key_up = message == WM_KEYUP || message == WM_SYSKEYUP;
    let key_down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
    if !key_up && !key_down {
        return None;
    }
    if modifier_is_down() {
        // Combos (Ctrl+C etc.) are gameplay shortcuts, never sidebar text.
        return None;
    }
    if !is_plain_typing_vk(event.vkCode) {
        return None;
    }
    Some(key_up)
}

/// Swallow the (already gated) candidate into the buffer. `foreground_game`
/// comes from the caller's snapshot. Lock contention fails OPEN; watchdog
/// and capacity overflow STOP swallowing but RETAIN what was captured —
/// the JS replay owns dropping, never the hook.
fn try_buffer_event(event: &KBDLLHOOKSTRUCT, key_up: bool, foreground_game: bool) -> bool {
    let Ok(mut buffer) = KEY_BUFFER.try_lock() else {
        // Fail open: cannot stall the system input path on lock contention.
        return false;
    };
    let Some(started) = buffer.1 else {
        return false;
    };
    if started.elapsed() > Duration::from_millis(KEY_BUFFER_WATCHDOG_MS) {
        // Watchdog fired inside the hook: stop swallowing, KEEP the captured
        // keys for the JS replay (dropping here would lose them twice over).
        stop_first_key_buffer_swallow();
        return false;
    }
    if !foreground_game {
        return false;
    }
    if buffer.0.len() >= KEY_BUFFER_CAPACITY {
        // Overflow: stop swallowing, keep what we hold. Fallback for the
        // unbuffered remainder is today's behavior (keys pass through).
        stop_first_key_buffer_swallow();
        return false;
    }
    buffer.0.push(BufferedKeyEvent {
        vk_code: event.vkCode,
        scan_code: (event.scanCode & 0xFFFF) as u16,
        extended: event.flags.contains(LLKHF_EXTENDED),
        key_up,
    });
    true
}

/// Plain typing material for the sidebar composer. Deliberately excludes
/// Enter (submit), Escape (cancel), Tab, modifiers, and everything the game
/// owns during gameplay (F-keys etc. are not typed text).
fn is_plain_typing_vk(vk: u32) -> bool {
    matches!(vk, 0x30..=0x39 /* digits */ | 0x41..=0x5A /* A-Z */ | 0x20 /* space */)
        || matches!(vk, 0xBA..=0xC0 | 0xDB..=0xDF /* OEM punctuation */)
        || vk == 0x08 /* backspace */
}

/// Drain the buffered events WITHOUT disarming. An empty drain is the COMMON
/// case at initial focus (~0.2s, before the 0.98–1.24s flash), so disarming
/// here would silently switch the whole buffer off before the flash it exists
/// for. The replay command stops swallowing explicitly on a settled replay;
/// every session end (Escape, injection commands, sidebar fallback, user
/// leaving, next arm, watchdog) owns its own disarm. Replay runs OUTSIDE any
/// lock (SendInput with the SELF marker so the hook ignores its own replay).
pub fn take_buffered_keys() -> Vec<BufferedKeyEvent> {
    KEY_BUFFER
        .lock()
        .map(|mut buffer| std::mem::take(&mut buffer.0))
        .unwrap_or_default()
}

/// Number of events currently held (diagnostics / deferred-replay logging).
pub fn buffered_key_count() -> usize {
    KEY_BUFFER.lock().map(|buffer| buffer.0.len()).unwrap_or(0)
}

/// Put replay events back after SendInput rejected them, preserving replay
/// order. Swallowing stays OFF: re-arming on top of a failing input path
/// would eat even more gameplay keys. The next focus-recovery replay retries
/// the same buffer, and the session-end discard remains the final drop point.
pub fn restore_buffered_keys(events: &[BufferedKeyEvent]) {
    if events.is_empty() {
        return;
    }
    if let Ok(mut buffer) = KEY_BUFFER.lock() {
        // Anything already held was captured EARLIER (residue is empty in
        // practice), so restored events append behind it in time order.
        buffer.0.extend_from_slice(events);
    }
}

pub fn buffered_key_input(
    event: BufferedKeyEvent,
) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        KEYEVENTF_SCANCODE, VIRTUAL_KEY,
    };
    let mut flags = KEYEVENTF_SCANCODE;
    if event.extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if event.key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: event.scan_code,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SELF_INJECTED_EXTRA_INFO,
            },
        },
    }
}

static CHAT_TRIGGER_ENABLED: AtomicBool = AtomicBool::new(true);
static CHAT_TRIGGER_VK: AtomicU32 = AtomicU32::new(0x0D);
// When the chat key is Enter (VK 0x0D), the numpad-Enter-only mode makes the
// hook react exclusively to the extended-key Enter (numpad side) and ignore
// the main-row Enter; false keeps both Enters triggering exactly as before.
static CHAT_TRIGGER_NUMPAD_ENTER_ONLY: AtomicBool = AtomicBool::new(false);
static CHAT_TRIGGER_ARMED: AtomicBool = AtomicBool::new(false);
// Extended-key flag of the most recent VK_RETURN the hook saw (main-row vs
// numpad Enter). Diagnostic only: the chat thread stamps it into the
// `overlay.chat_key` log line so numpad-only-mode reports can tell WHICH
// Enter was involved. Racy by design (a newer event overwrites an older
// one); the hook stores it cheaply before the extended-flag gate so BOTH
// Enters are observed, not just the accepted one.
static CHAT_TRIGGER_LAST_ENTER_EXTENDED: AtomicBool = AtomicBool::new(false);
static ESCAPE_TRIGGER_ARMED: AtomicBool = AtomicBool::new(false);

// Marker written into dwExtraInfo of every SendInput event this app emits
// ("HD2C" in ASCII). The low-level keyboard hook ignores injected events
// carrying it — those are our own Enter/Esc/unicode injections that must
// never re-trigger the chat-key hook (recursive sidebar opens) — while
// injected events WITHOUT it are treated as user input from third-party
// software (remote control) and go through the normal foreground/chat-key/
// modifier gates. The blanket LLKHF_INJECTED drop it replaces is why
// remote-control sessions could not open the sidebar at all.
pub const SELF_INJECTED_EXTRA_INFO: usize = 0x4844_3243;

/// Where a low-level keyboard event came from, decided purely from the hook
/// struct so tests can pin the policy without a live hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookEventSource {
    Physical,
    RemoteInjected,
    SelfInjected,
}

fn classify_hook_event(injected: bool, extra_info: usize) -> HookEventSource {
    if !injected {
        return HookEventSource::Physical;
    }
    if extra_info == SELF_INJECTED_EXTRA_INFO {
        HookEventSource::SelfInjected
    } else {
        HookEventSource::RemoteInjected
    }
}
static AUTO_LOCK_CAPS: AtomicBool = AtomicBool::new(true);
static CAPS_PROTECTION_FAULTED: AtomicBool = AtomicBool::new(false);

/// Read-only accessor for the degraded-guard flag: callers that do not
/// depend on CapsLock injection (Esc-to-close) use it to pass through when a
/// prepare attempt ITSELF triggered the fault (the early-exit above only
/// covers an already-faulted guard).
#[cfg(windows)]
pub fn caps_protection_faulted() -> bool {
    CAPS_PROTECTION_FAULTED.load(Ordering::Acquire)
}
static CAPS_BACKGROUND_FAILURES: AtomicU32 = AtomicU32::new(0);
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
// Where a chat-key trigger came from. Carried inside the signal instead of a
// global: a global would be overwritten by any other keyboard event the hook
// sees between the trigger and the chat thread reading it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatTriggerSource {
    Physical,
    RemoteInjected,
}

impl ChatTriggerSource {
    fn as_str(self) -> &'static str {
        match self {
            ChatTriggerSource::Physical => "physical",
            ChatTriggerSource::RemoteInjected => "remote_injected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatTriggerSignal {
    Prepare(ChatTriggerSource),
    Show(ChatTriggerSource),
    // Escape side effects (foreground re-check, log, Tauri event) run on this
    // worker thread: the hook must not touch disk or IPC, or a stall past
    // LowLevelHooksTimeout gets the hook silently removed by Windows and no
    // chat key — physical or remote — opens the sidebar until restart.
    Escape(ChatTriggerSource),
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
            if let ChatTriggerSignal::Escape(source) = signal {
                let snapshot = foreground_snapshot();
                if snapshot.state == ForegroundState::Game {
                    // The sidebar session is ending: buffered keys would never
                    // be replayed into it, so drop them now.
                    discard_first_key_buffer();
                    append_runtime_log(
                        &chat_app,
                        "overlay.escape_key",
                        format!("state=game source={}", source.as_str()),
                    );
                    let _ = chat_app.emit(GAME_CHAT_ESCAPE_PHYSICAL_EVENT, snapshot);
                }
                continue;
            }
            let (is_prepare, source) = match signal {
                ChatTriggerSignal::Prepare(source) => (true, source),
                ChatTriggerSignal::Show(source) => (false, source),
                ChatTriggerSignal::Escape(_) => continue,
            };
            if is_prepare {
                let snapshot = foreground_snapshot();
                if snapshot.state == ForegroundState::Game {
                    append_runtime_log(
                        &chat_app,
                        "overlay.chat_key",
                        format!(
                            "stage=keydown_prepare state=game source={}",
                            source.as_str()
                        ),
                    );
                    let _ = chat_app.emit(GAME_CHAT_KEY_PREPARE_EVENT, snapshot);
                    prepare_overlay_input();
                } else {
                    append_runtime_log(
                        &chat_app,
                        "overlay.chat_key_ignored",
                        format!(
                            "stage=keydown_prepare state={:?} source={}",
                            snapshot.state,
                            source.as_str()
                        ),
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
                    format!("state={:?} source={}", snapshot.state, source.as_str()),
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
                    "state=game source={} caps_before={caps_before} caps_after={caps_after} key_vk={} extended={}",
                    source.as_str(),
                    CHAT_TRIGGER_VK.load(Ordering::Acquire),
                    CHAT_TRIGGER_LAST_ENTER_EXTENDED.load(Ordering::Relaxed)
                ),
            );
            if caps_after {
                append_runtime_log(
                    &chat_app,
                    "overlay.caps_error",
                    "phase=chat_focus expected=false actual=true",
                );
            }
            // Arm the first-key buffer: the sidebar is about to take focus,
            // but the game chat flash (~1.1s) may steal it back mid-typing.
            // Plain keys typed while the game owns the foreground are held
            // here until JS confirms composer focus and asks for a replay.
            arm_first_key_buffer();
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

pub fn set_auto_lock_caps(enabled: bool) -> bool {
    AUTO_LOCK_CAPS.store(enabled, Ordering::Release);
    CAPS_BACKGROUND_FAILURES.store(0, Ordering::Release);
    if enabled {
        CAPS_PROTECTION_FAULTED.store(false, Ordering::Release);
        true
    } else {
        restore_caps_lock()
    }
}

pub fn restore_caps_lock() -> bool {
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        return false;
    };
    restore_caps_lock_locked("restore")
        && !caps_session_active()
        && !CAPS_PROTECTION_FAULTED.load(Ordering::Acquire)
}

/// Reconcile CapsLock with the window that is actually in the foreground.
/// Manual recovery often runs while the assistant is focused, so restoring the
/// original game-session state unconditionally can immediately conflict with
/// the assistant-side protection policy.
pub fn recover_caps_for_current_foreground() -> bool {
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        return false;
    };
    CAPS_PROTECTION_FAULTED.store(false, Ordering::Release);
    let state = foreground_snapshot().state;
    let restored = match state {
        ForegroundState::Game => {
            prepare_caps_lock_locked(true, "recovery_game") && caps_lock_enabled()
        }
        ForegroundState::Assistant => {
            prepare_caps_lock_locked(false, "recovery_assistant") && !caps_lock_enabled()
        }
        ForegroundState::Other => {
            restore_caps_lock_locked("recovery_other") && !caps_session_active()
        }
    };
    if !restored {
        CAPS_PROTECTION_FAULTED.store(true, Ordering::Release);
    }
    append_caps_log(format!(
        "phase=recovery state={state:?} expected={} actual={} result={restored}",
        matches!(state, ForegroundState::Game),
        caps_lock_enabled(),
    ));
    restored
}

pub fn prepare_overlay_input() -> bool {
    if !AUTO_LOCK_CAPS.load(Ordering::Acquire) {
        return true;
    }
    if CAPS_PROTECTION_FAULTED.load(Ordering::Acquire) {
        // Same degraded rationale as verify_text_injection_caps: a faulted
        // toggle must not gate the chat-key overlay flow, cancel, or the
        // gameplay handoff — callers treat false as "refuse the flow", which
        // used to block cancel and error every handoff for the whole faulted
        // session. Protection re-arms through the monitor's Other-foreground
        // restore and the manual recovery button, not through this gate.
        append_caps_log("phase=prepare_overlay stage=faulted_degraded_allowed");
        return true;
    }
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        return false;
    };
    prepare_caps_lock_locked(false, "prepare_overlay")
}

pub fn prepare_gameplay() -> bool {
    if !AUTO_LOCK_CAPS.load(Ordering::Acquire) {
        return true;
    }
    if CAPS_PROTECTION_FAULTED.load(Ordering::Acquire) {
        append_caps_log("phase=prepare_gameplay stage=faulted_degraded_allowed");
        return true;
    }
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        return false;
    };
    prepare_caps_lock_locked(true, "prepare_gameplay")
}

pub fn verify_text_injection_caps(expected: bool, phase: &str) -> bool {
    if !AUTO_LOCK_CAPS.load(Ordering::Acquire) {
        append_caps_log(format!(
            "phase={phase} expected={expected} actual={} stage=disabled",
            caps_lock_enabled()
        ));
        return true;
    }
    let Ok(_transition) = CAPS_TRANSITION_LOCK.lock() else {
        append_caps_log(format!(
            "phase={phase} expected={expected} actual=unknown stage=lock_failed"
        ));
        return false;
    };
    if CAPS_PROTECTION_FAULTED.load(Ordering::Acquire) {
        // Degraded pass-through: a faulted CapsLock guard only means the
        // automatic toggle is unavailable (e.g. UIPI when the game runs
        // elevated). Blocking every send on it regresses the pre-guard
        // behavior where Unicode injection still worked (02.log) and hides
        // the real failure behind a secondary CapsLock error. The injection
        // layer's own report stays authoritative for delivery.
        append_caps_log(format!(
            "phase={phase} expected={expected} actual={} stage=faulted_degraded_send_allowed",
            caps_lock_enabled()
        ));
        return true;
    }
    prepare_caps_lock_locked(expected, phase);
    let actual = caps_lock_enabled();
    // The prepare we just ran may itself be what faulted (SendInput blocked on
    // this very preflight). Without this re-check the FIRST send after the
    // fault is still rejected via actual != expected, and only the second one
    // reaches the degraded early return above.
    if actual != expected && CAPS_PROTECTION_FAULTED.load(Ordering::Acquire) {
        append_caps_log(format!(
            "phase={phase} expected={expected} actual={actual} stage=faulted_degraded_send_allowed"
        ));
        return true;
    }
    // Faulted is intentionally absent here: the faulted cases return early
    // above with a degraded pass-through, so this tail only decides whether
    // the state matches. Do not reintroduce a faulted term without also
    // revisiting the early return, or sends get hard-blocked again.
    let verified = actual == expected;
    append_caps_log(format!(
        "phase={phase} expected={expected} actual={actual} stage=preflight_verified result={verified}"
    ));
    verified
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

pub fn set_chat_key_numpad_enter_only(enabled: bool) {
    CHAT_TRIGGER_NUMPAD_ENTER_ONLY.store(enabled, Ordering::Release);
    // Re-arm cleanly so a mid-press mode flip cannot fire a stale trigger.
    CHAT_TRIGGER_ARMED.store(false, Ordering::Release);
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
        // No disk logging anywhere in this hook: it runs on the system input
        // path, and if append_runtime_log stalls past LowLevelHooksTimeout
        // (slow disk, antivirus), Windows silently removes the hook and no
        // chat key — physical or remote — opens the sidebar until restart.
        let chat_source =
            match classify_hook_event(event.flags.contains(LLKHF_INJECTED), event.dwExtraInfo) {
                // Our own Enter/Esc/unicode injections must not re-trigger the
                // chat-key hook (recursive sidebar opens); the marker itself
                // identifies them, drop as fast as possible.
                HookEventSource::SelfInjected => {
                    return unsafe { CallNextHookEx(None, code, wparam, lparam) };
                }
                // Injected by third-party software (remote control): treated as
                // user input, subject to the same gates as physical keys.
                HookEventSource::RemoteInjected => ChatTriggerSource::RemoteInjected,
                HookEventSource::Physical => ChatTriggerSource::Physical,
            };

        if CHAT_TRIGGER_ENABLED.load(Ordering::Acquire)
            && event.vkCode == CHAT_TRIGGER_VK.load(Ordering::Acquire)
        {
            // Observe BOTH Enters before the numpad-only gate: the rejected
            // main-row Enter is exactly what numpad-mode bug reports need to
            // see. Relaxed is fine — diagnostic bookkeeping, no ordering.
            CHAT_TRIGGER_LAST_ENTER_EXTENDED
                .store(event.flags.contains(LLKHF_EXTENDED), Ordering::Relaxed);
            if chat_key_matches_extended_flag(event.flags.contains(LLKHF_EXTENDED)) {
                let (trigger, armed) = resolve_chat_key_trigger(
                    wparam.0 as u32,
                    modifier_is_down(),
                    foreground_snapshot().state,
                    CHAT_TRIGGER_ARMED.load(Ordering::Acquire),
                );
                let was_armed = CHAT_TRIGGER_ARMED.swap(armed, Ordering::AcqRel);
                if armed && !was_armed {
                    if let Some(sender) = CHAT_TRIGGER_TX.get() {
                        let _ = sender.send(ChatTriggerSignal::Prepare(chat_source));
                    }
                }
                if trigger {
                    if let Some(sender) = CHAT_TRIGGER_TX.get() {
                        let _ = sender.send(ChatTriggerSignal::Show(chat_source));
                    }
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
                if let Some(sender) = CHAT_TRIGGER_TX.get() {
                    let _ = sender.send(ChatTriggerSignal::Escape(chat_source));
                }
            }
        }

        // First-key buffering: while a sidebar session is opening and the game
        // still owns the foreground (the ~1.1s flash window), swallow plain
        // typing keys for later replay into the sidebar. The armed gate is a
        // single atomic and candidate gating is cheap, so the common case
        // (nothing armed, non-typing keys) costs nothing on the system input
        // path; the ONE expensive foreground snapshot runs only for a plain
        // unmodified typing key during a live session.
        // Swallowing = return 1 without calling CallNextHookEx.
        if first_key_buffer_armed() {
            if let Some(key_up) = buffer_event_if_armed(event, wparam.0 as u32) {
                let foreground_game = foreground_snapshot().state == ForegroundState::Game;
                if try_buffer_event(event, key_up, foreground_game) {
                    return LRESULT(1);
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

// VK_RETURN is shared by the main-row and numpad Enter; the LLKHF_EXTENDED
// flag distinguishes them (numpad side sets it). The numpad-only mode gates
// the shared-VK comparison on that flag; other chat keys never enter here.
fn chat_key_matches_extended_flag(event_is_extended: bool) -> bool {
    if !CHAT_TRIGGER_NUMPAD_ENTER_ONLY.load(Ordering::Acquire) {
        return true;
    }
    let trigger_vk = CHAT_TRIGGER_VK.load(Ordering::Acquire);
    if trigger_vk == 0x0D {
        event_is_extended
    } else {
        true
    }
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
            let expected = caps_session_original_state().unwrap_or_else(caps_lock_enabled);
            if restore_caps_lock_locked("monitor_other") {
                CAPS_BACKGROUND_FAILURES.store(0, Ordering::Release);
                CAPS_PROTECTION_FAULTED.store(false, Ordering::Release);
            } else {
                let failures = CAPS_BACKGROUND_FAILURES.fetch_add(1, Ordering::AcqRel) + 1;
                append_caps_log(format!(
                    "phase=monitor_other failures={failures} limit={CAPS_BACKGROUND_FAILURE_LIMIT} stage=retryable_failure"
                ));
                if failures >= CAPS_BACKGROUND_FAILURE_LIMIT {
                    mark_caps_protection_fault("monitor_other", expected);
                }
            }
        }
        ForegroundState::Assistant => {}
    }
}

fn caps_protection_active() -> bool {
    AUTO_LOCK_CAPS.load(Ordering::Acquire) && !CAPS_PROTECTION_FAULTED.load(Ordering::Acquire)
}

fn prepare_caps_lock_locked(expected: bool, phase: &str) -> bool {
    if let Ok(mut original) = CAPS_SESSION_ORIGINAL.lock() {
        if original.is_none() {
            *original = Some(caps_lock_enabled());
        }
    }
    ensure_caps_lock_locked(expected, phase)
}

fn restore_caps_lock_locked(phase: &str) -> bool {
    let original_state = caps_session_original_state();
    let Some(original_state) = original_state else {
        return true;
    };
    if ensure_caps_lock_locked(original_state, phase) {
        if let Ok(mut original) = CAPS_SESSION_ORIGINAL.lock() {
            if *original == Some(original_state) {
                *original = None;
            }
        }
        return true;
    }
    false
}

fn caps_session_original_state() -> Option<bool> {
    CAPS_SESSION_ORIGINAL
        .lock()
        .ok()
        .and_then(|original| *original)
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
        if actual == expected {
            // No log for the no-change confirmation: the 250ms monitor polls
            // land here whenever the state is already correct, and those
            // check lines were 25-47% of every diagnostic export. Everything
            // that carries signal still logs below (mismatch, physical key
            // down, toggle, verify, retryable failure, fault).
            return true;
        }
        append_caps_log(format!(
            "phase={phase} attempt={attempt} expected={expected} actual={actual} stage=check"
        ));
        if unsafe { GetAsyncKeyState(VK_CAPITAL.0 as i32) } < 0 {
            append_caps_log(format!(
                "phase={phase} attempt={attempt} expected={expected} actual={actual} stage=physical_key_down"
            ));
            return false;
        }

        let inputs = [caps_lock_input(false), caps_lock_input(true)];
        // Zero out the last error so the code logged below is this call's,
        // not a stale value from an unrelated earlier API (SendInput does not
        // reliably set it on partial success).
        unsafe { SetLastError(WIN32_ERROR(0)) };
        let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if inserted == 1 {
            let key_up = [caps_lock_input(true)];
            let _ = unsafe { SendInput(&key_up, size_of::<INPUT>() as i32) };
        }
        append_caps_log(format!(
            "phase={phase} attempt={attempt} expected={expected} actual={actual} stage=toggle inserted={inserted} last_error={}",
            unsafe { GetLastError().0 }
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
    if phase == "monitor_other" {
        append_caps_log(format!(
            "phase={phase} attempt={CAPS_TOGGLE_MAX_ATTEMPTS} expected={expected} actual={actual} stage=retryable_failure"
        ));
    } else {
        mark_caps_protection_fault(phase, expected);
    }
    false
}

fn mark_caps_protection_fault(phase: &str, expected: bool) {
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
                dwExtraInfo: SELF_INJECTED_EXTRA_INFO,
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

// Classify whoever currently owns the OS foreground *right now*, without
// waiting for the next monitor tick. The chat overlay focus guard probes this
// within milliseconds of a focus loss to tell "the game stole the composer"
// apart from an Alt+Tab. Names match the serialized GameForegroundEvent state
// contract used over the event bridge.
pub fn foreground_state_name() -> &'static str {
    match foreground_snapshot().state {
        ForegroundState::Game => "game",
        ForegroundState::Assistant => "assistant",
        ForegroundState::Other => "other",
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
        "Enter" | "NumpadEnter" => Some(0x0D),
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

    /// The first-key buffer is process-global state; tests exercising it must
    /// hold this lock so parallel test threads cannot race each other.
    static KEY_BUFFER_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn hook_event_classification_pins_the_injection_policy() {
        // Physical keys and third-party injections (remote control) both reach
        // the trigger gates; only our own marked injections are skipped —
        // dropping ALL injected events is what blocked remote-control users
        // from opening the sidebar.
        assert_eq!(classify_hook_event(false, 0), HookEventSource::Physical);
        assert_eq!(
            classify_hook_event(false, SELF_INJECTED_EXTRA_INFO),
            HookEventSource::Physical
        );
        assert_eq!(
            classify_hook_event(true, 0),
            HookEventSource::RemoteInjected
        );
        assert_eq!(
            classify_hook_event(true, SELF_INJECTED_EXTRA_INFO),
            HookEventSource::SelfInjected
        );
        // The marker must never collide with a plain 0 extraInfo.
        assert_ne!(SELF_INJECTED_EXTRA_INFO, 0);
    }

    #[test]
    fn maps_supported_dom_codes_to_windows_virtual_keys() {
        assert_eq!(virtual_key_for_code("Enter"), Some(0x0D));
        assert_eq!(virtual_key_for_code("NumpadEnter"), Some(0x0D));
        assert_eq!(virtual_key_for_code("KeyT"), Some(u32::from(b'T')));
        assert_eq!(virtual_key_for_code("Digit7"), Some(u32::from(b'7')));
        assert_eq!(virtual_key_for_code("F12"), Some(0x7B));
        assert_eq!(virtual_key_for_code("ControlLeft"), None);
    }

    #[test]
    fn plain_typing_vk_scope_pins_the_first_key_buffer_policy() {
        // Bufferable: letters, digits, space, OEM punctuation, backspace.
        assert!(is_plain_typing_vk(0x41)); // A
        assert!(is_plain_typing_vk(0x5A)); // Z
        assert!(is_plain_typing_vk(0x30)); // 0
        assert!(is_plain_typing_vk(0x39)); // 9
        assert!(is_plain_typing_vk(0x20)); // space
        assert!(is_plain_typing_vk(0xBA)); // ; :
        assert!(is_plain_typing_vk(0xBF)); // / ?
        assert!(is_plain_typing_vk(0x08)); // backspace
        // Never bufferable: Enter (submit), Escape (cancel), Tab, modifiers,
        // F-keys (game owns them during the flash window).
        assert!(!is_plain_typing_vk(0x0D));
        assert!(!is_plain_typing_vk(0x1B));
        assert!(!is_plain_typing_vk(0x09));
        assert!(!is_plain_typing_vk(0x10)); // shift
        assert!(!is_plain_typing_vk(0x11)); // ctrl
        assert!(!is_plain_typing_vk(0x70)); // F1
    }

    #[test]
    fn first_key_buffer_lifecycle_round_trip() {
        // The first-key buffer is process-global state: serialize the tests
        // that exercise it (cargo runs tests in parallel threads).
        let _serial = KEY_BUFFER_TEST_LOCK.lock();
        // Clean slate (other tests may have armed/disarmed).
        discard_first_key_buffer();

        // Without arming, the gate declines before anything expensive runs.
        assert!(!first_key_buffer_armed());
        let event = KBDLLHOOKSTRUCT {
            vkCode: 0x48,
            scanCode: 0x23,
            flags: Default::default(),
            time: 0,
            dwExtraInfo: 0,
        };
        assert_eq!(buffer_event_if_armed(&event, WM_KEYDOWN), None);

        arm_first_key_buffer();
        assert!(first_key_buffer_armed());

        // A plain unmodified letter is a swallow candidate; Enter is not.
        assert_eq!(buffer_event_if_armed(&event, WM_KEYDOWN), Some(false));
        assert_eq!(buffer_event_if_armed(&event, WM_KEYUP), Some(true));
        let enter = KBDLLHOOKSTRUCT {
            vkCode: 0x0D,
            scanCode: 0x1C,
            flags: Default::default(),
            time: 0,
            dwExtraInfo: 0,
        };
        assert_eq!(buffer_event_if_armed(&enter, WM_KEYDOWN), None);

        // With the game confirmed on foreground, the candidate is buffered in
        // order; drain returns it and KEEPS the session armed — an empty
        // later drain (initial focus precedes the flash) must not switch the
        // buffer off. Disarming is the replay-settle/session-end's job.
        assert!(try_buffer_event(&event, false, true));
        let drained = take_buffered_keys();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].vk_code, 0x48);
        assert_eq!(drained[0].scan_code, 0x23);
        assert!(!drained[0].key_up);
        assert!(first_key_buffer_armed());

        // A second drain after disarm-by-explicit-discard is empty, and
        // re-arming clears residue.
        discard_first_key_buffer();
        assert!(!first_key_buffer_armed());
        arm_first_key_buffer();
        let drained_again = take_buffered_keys();
        assert!(drained_again.is_empty());
        assert!(first_key_buffer_armed());
        discard_first_key_buffer();
    }

    #[test]
    fn restore_buffered_keys_preserves_order_for_retry() {
        let _serial = KEY_BUFFER_TEST_LOCK.lock();
        discard_first_key_buffer();

        // Simulate the replay command failing wholesale: the drained down+up
        // pair goes back and a later take returns the exact same sequence.
        let drained = vec![
            BufferedKeyEvent {
                vk_code: 0x48,
                scan_code: 0,
                extended: false,
                key_up: false,
            },
            BufferedKeyEvent {
                vk_code: 0x48,
                scan_code: 0,
                extended: false,
                key_up: true,
            },
        ];
        restore_buffered_keys(&drained);
        assert_eq!(buffered_key_count(), 2);

        let re_drained = take_buffered_keys();
        assert_eq!(re_drained.len(), 2);
        assert_eq!(re_drained[0].vk_code, 0x48);
        assert!(!re_drained[0].key_up);
        assert_eq!(re_drained[1].vk_code, 0x48);
        assert!(re_drained[1].key_up);

        // Empty restore is a no-op.
        restore_buffered_keys(&[]);
        assert_eq!(buffered_key_count(), 0);
    }

    #[test]
    fn renew_and_expired_stop_pin_the_burst_capture_contract() {
        let _serial = KEY_BUFFER_TEST_LOCK.lock();
        discard_first_key_buffer();

        // Not armed: renewal is a no-op and must not fabricate a live window.
        renew_first_key_buffer_window();
        assert!(!first_key_buffer_armed());

        // Armed with a FRESH window (a settled replay just renewed it): the
        // expired-only stop must keep the capture alive so the NEXT flicker
        // of the burst is still swallowed and replayed.
        arm_first_key_buffer();
        stop_first_key_buffer_swallow_if_expired();
        assert!(first_key_buffer_armed());

        // Rewind the watchdog start beyond the budget: the same stop must
        // now disarm (stale armed state must not eat gameplay keys forever).
        if let Ok(mut buffer) = KEY_BUFFER.lock() {
            buffer.1 = Some(Instant::now() - Duration::from_millis(KEY_BUFFER_WATCHDOG_MS + 1));
        }
        stop_first_key_buffer_swallow_if_expired();
        assert!(!first_key_buffer_armed());

        // The expired-only stop never re-arms a dead session.
        stop_first_key_buffer_swallow_if_expired();
        assert!(!first_key_buffer_armed());
        renew_first_key_buffer_window();
        assert!(!first_key_buffer_armed());
        discard_first_key_buffer();
    }

    #[test]
    fn first_key_buffer_overflow_retains_captured_keys() {
        let _serial = KEY_BUFFER_TEST_LOCK.lock();
        discard_first_key_buffer();

        // Overflow policy: once the capacity is hit, swallowing stops but the
        // captured keys are RETAINED for the JS replay (never dropped here).
        arm_first_key_buffer();
        let event = KBDLLHOOKSTRUCT {
            vkCode: 0x48,
            scanCode: 0x23,
            flags: Default::default(),
            time: 0,
            dwExtraInfo: 0,
        };
        // Fill exactly to capacity: 64 down+up pairs = 128 events.
        for _ in 0..KEY_BUFFER_CAPACITY / 2 {
            assert!(try_buffer_event(&event, false, true));
            assert!(try_buffer_event(&event, true, true));
        }
        assert_eq!(buffered_key_count(), KEY_BUFFER_CAPACITY);
        // The next candidate sees the overflow: not swallowed...
        assert!(!try_buffer_event(&event, false, true));
        // ...swallowing stopped, but everything captured is still there.
        assert!(!first_key_buffer_armed());
        assert_eq!(buffered_key_count(), KEY_BUFFER_CAPACITY);

        // Re-arming clears the retained keys (new session, fresh buffer).
        arm_first_key_buffer();
        assert_eq!(buffered_key_count(), 0);

        discard_first_key_buffer();
    }

    #[test]
    fn numpad_enter_only_mode_gates_the_extended_key_flag() {
        // Feature off: both physical Enters (extended flag either way) match.
        CHAT_TRIGGER_NUMPAD_ENTER_ONLY.store(false, Ordering::Release);
        CHAT_TRIGGER_VK.store(0x0D, Ordering::Release);
        assert!(chat_key_matches_extended_flag(true));
        assert!(chat_key_matches_extended_flag(false));

        // Feature on with Enter as the chat key: only the numpad (extended)
        // Enter matches; the main-row Enter must not trigger.
        CHAT_TRIGGER_NUMPAD_ENTER_ONLY.store(true, Ordering::Release);
        assert!(chat_key_matches_extended_flag(true));
        assert!(!chat_key_matches_extended_flag(false));

        // The flag must not affect any other chat key (non-Enter VK).
        CHAT_TRIGGER_VK.store(0x20, Ordering::Release);
        assert!(chat_key_matches_extended_flag(false));
        assert!(chat_key_matches_extended_flag(true));

        CHAT_TRIGGER_NUMPAD_ENTER_ONLY.store(false, Ordering::Release);
        CHAT_TRIGGER_VK.store(0x0D, Ordering::Release);
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
    fn faulted_caps_protection_degrades_prepare_gates() {
        // Degrade pass-through mirrors verify_text_injection_caps: a faulted
        // toggle must keep cancel/handoff flows open instead of refusing them
        // for the rest of the session. Both branches below exit before the
        // transition lock, so no real SendInput/CapsLock toggle can fire here.
        let auto_lock_default = AUTO_LOCK_CAPS.load(Ordering::Acquire);
        AUTO_LOCK_CAPS.store(true, Ordering::Release);
        CAPS_PROTECTION_FAULTED.store(true, Ordering::Release);
        assert!(prepare_overlay_input());
        assert!(prepare_gameplay());

        // Protection disabled: both gates stay open regardless of fault state.
        AUTO_LOCK_CAPS.store(false, Ordering::Release);
        assert!(prepare_overlay_input());
        assert!(prepare_gameplay());

        // Restore the process-wide defaults so parallel tests are unaffected.
        AUTO_LOCK_CAPS.store(auto_lock_default, Ordering::Release);
        CAPS_PROTECTION_FAULTED.store(false, Ordering::Release);
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
