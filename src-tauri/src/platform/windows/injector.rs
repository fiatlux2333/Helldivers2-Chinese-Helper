use crate::{
    core::{
        session::TargetIdentity,
        translation::{GameInputMethod, StratagemMacro, StratagemMenuMode},
    },
    platform::{InjectionReport, windows::target},
};
use encoding_rs::GBK;
use serde::Deserialize;
use std::{
    ffi::c_void,
    mem::size_of,
    thread,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, SetLastError, WIN32_ERROR, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyState, GetKeyboardLayout, GetKeyboardLayoutList, HKL, INPUT, INPUT_0,
            INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
            KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_NUMLOCK,
        },
        WindowsAndMessaging::{SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_INPUTLANGCHANGEREQUEST},
    },
};
const ENTER_SCAN_CODE: u16 = 0x1C;
const ESCAPE_SCAN_CODE: u16 = 0x01;
const ALT_SCAN_CODE: u16 = 0x38;
const OPEN_CHAT_KEY_HOLD_MS: u64 = 40;
const KEYBOARD_LAYOUT_SETTLE_MS: u64 = 120;
const KEYBOARD_LAYOUT_POLL_MS: u64 = 20;
const KEYBOARD_LAYOUT_TIMEOUT_MS: u64 = 400;
const SIMPLIFIED_CHINESE_LANGUAGE_ID: usize = 0x0804;
const NUMPAD_SCAN_CODES: [u16; 10] = [0x52, 0x4F, 0x50, 0x51, 0x4B, 0x4C, 0x4D, 0x47, 0x48, 0x49];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatPreparation {
    KeepOpen,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectionError {
    TargetChanged(InjectionReport),
    OpenChatFailed(InjectionReport),
    SendInputFailed(InjectionReport),
    SubmitFailed(InjectionReport),
    UnrepresentableCharacter(InjectionReport, char),
    KeyboardLayoutUnavailable(InjectionReport),
    UnsupportedKey(InjectionReport, String),
}

struct NumLockGuard {
    original_enabled: bool,
    input_delay_ms: u64,
    restored: bool,
}

impl NumLockGuard {
    fn new(report: &mut InjectionReport, input_delay_ms: u64) -> Option<Self> {
        let original_enabled = num_lock_enabled();
        let mut guard = Self {
            original_enabled,
            input_delay_ms,
            restored: false,
        };
        if !original_enabled {
            let inputs = virtual_key_inputs(VK_NUMLOCK);
            let inserted = send_input_with_report(&inputs, report, false);
            report.num_lock_toggled = true;
            thread::sleep(Duration::from_millis(input_delay_ms));
            if inserted != inputs.len() as u32 || !num_lock_enabled() {
                report.key_state_uncertain = inserted % 2 != 0;
                report.num_lock_restored = Some(guard.restore(report));
                return None;
            }
        }
        Some(guard)
    }

    fn restore(&mut self, report: &mut InjectionReport) -> bool {
        if self.restored {
            return true;
        }
        if num_lock_enabled() == self.original_enabled {
            self.restored = true;
            return true;
        }
        let inputs = virtual_key_inputs(VK_NUMLOCK);
        let inserted = send_input_with_report(&inputs, report, false);
        thread::sleep(Duration::from_millis(self.input_delay_ms));
        let restored =
            inserted == inputs.len() as u32 && num_lock_enabled() == self.original_enabled;
        if inserted % 2 != 0 {
            report.key_state_uncertain = true;
        }
        if !restored {
            report.key_state_uncertain = true;
        }
        self.restored = restored;
        restored
    }
}

impl Drop for NumLockGuard {
    fn drop(&mut self) {
        if !self.restored && num_lock_enabled() != self.original_enabled {
            let inputs = virtual_key_inputs(VK_NUMLOCK);
            let _ = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        }
    }
}

struct AltReleaseGuard {
    active: bool,
}

impl AltReleaseGuard {
    fn armed() -> Self {
        Self { active: true }
    }

    fn release(mut self, report: &mut InjectionReport) -> bool {
        if !self.active {
            return true;
        }
        if send_scan_code_event(ALT_SCAN_CODE, true, report) {
            self.active = false;
            true
        } else {
            false
        }
    }
}

impl Drop for AltReleaseGuard {
    fn drop(&mut self) {
        if self.active {
            let input = scan_code_input(ALT_SCAN_CODE, true);
            let _ = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
        }
    }
}

struct KeyboardLayoutGuard {
    hwnd: HWND,
    thread_id: u32,
    original: HKL,
    switched: bool,
    restored: bool,
}

impl KeyboardLayoutGuard {
    fn arm(hwnd: HWND, thread_id: u32, original: HKL, requested: HKL) -> Result<Self, bool> {
        let switched = original != requested;
        if switched {
            request_keyboard_layout(hwnd, requested);
            if !wait_for_keyboard_layout(thread_id, requested) {
                request_keyboard_layout(hwnd, original);
                return Err(wait_for_keyboard_layout(thread_id, original));
            }
            thread::sleep(Duration::from_millis(KEYBOARD_LAYOUT_SETTLE_MS));
        }
        Ok(Self {
            hwnd,
            thread_id,
            original,
            switched,
            restored: false,
        })
    }

    fn restore(&mut self) -> bool {
        if self.restored {
            return true;
        }
        self.restored = true;
        if !self.switched {
            return true;
        }
        request_keyboard_layout(self.hwnd, self.original);
        wait_for_keyboard_layout(self.thread_id, self.original)
    }
}

impl Drop for KeyboardLayoutGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InjectionFailure {
    TargetChanged,
    OpenChatFailed,
    SendInputFailed,
    SubmitFailed,
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::result_large_err)]
pub fn inject_utf16_batches(
    expected_target: &TargetIdentity,
    batches: &[Vec<u16>],
    batch_delay_ms: u64,
    title_keyword: &str,
    chat_preparation: ChatPreparation,
    open_chat_focus_delay_ms: u64,
    submit: bool,
    input_method: GameInputMethod,
    input_delay_ms: u64,
) -> Result<InjectionReport, InjectionError> {
    let mut report = InjectionReport {
        attempted_batches: 0,
        requested_events: 0,
        successful_events: 0,
        text_requested_events: 0,
        text_successful_events: 0,
        last_error_code: None,
        delivery_transport: match input_method {
            GameInputMethod::GbkAltCode => "SendInputAltCode".to_owned(),
            GameInputMethod::UnicodeSendInput => "SendInput".to_owned(),
        },
        delivery_acknowledged: true,
        input_characters: count_utf16_characters(batches),
        input_delay_ms,
        keyboard_layout_switched: false,
        keyboard_layout_before: None,
        keyboard_layout_requested: None,
        keyboard_layout_restored: None,
        num_lock_toggled: false,
        num_lock_restored: None,
        failed_batch_index: None,
        partial_prefix_possible: false,
        key_state_uncertain: false,
        submit_attempted: false,
        submit_completed: false,
    };

    let alt_code_batches = if input_method == GameInputMethod::GbkAltCode {
        Some(encode_alt_code_batches(batches).map_err(|character| {
            InjectionError::UnrepresentableCharacter(report.clone(), character)
        })?)
    } else {
        None
    };

    let mut keyboard_layout_guard = if input_method == GameInputMethod::GbkAltCode {
        let hwnd = HWND(expected_target.hwnd as usize as *mut c_void);
        let original = unsafe { GetKeyboardLayout(expected_target.thread_id) };
        if original.is_invalid() {
            return Err(InjectionError::KeyboardLayoutUnavailable(report));
        }
        report.keyboard_layout_before = Some(keyboard_layout_id(original));
        let Some(requested) = select_simplified_chinese_layout(original) else {
            return Err(InjectionError::KeyboardLayoutUnavailable(report));
        };
        report.keyboard_layout_requested = Some(keyboard_layout_id(requested));
        let guard =
            match KeyboardLayoutGuard::arm(hwnd, expected_target.thread_id, original, requested) {
                Ok(guard) => guard,
                Err(restored) => {
                    report.keyboard_layout_restored = Some(restored);
                    return Err(InjectionError::KeyboardLayoutUnavailable(report));
                }
            };
        report.keyboard_layout_switched = guard.switched;
        Some(guard)
    } else {
        None
    };

    let mut num_lock_guard = if input_method == GameInputMethod::GbkAltCode {
        match NumLockGuard::new(&mut report, input_delay_ms) {
            Some(guard) => Some(guard),
            None => {
                restore_keyboard_layout(&mut keyboard_layout_guard, &mut report);
                return Err(InjectionError::SendInputFailed(report));
            }
        }
    } else {
        None
    };

    let outcome = (|| -> Result<(), InjectionFailure> {
        for &scan_code in chat_preparation_scan_codes(chat_preparation) {
            if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
                return Err(InjectionFailure::TargetChanged);
            }
            let [key_down, key_up] = scan_code_inputs(scan_code);
            let inserted_down = send_input_with_report(&[key_down], &mut report, false);
            if inserted_down != 1 {
                return Err(InjectionFailure::OpenChatFailed);
            }
            thread::sleep(Duration::from_millis(OPEN_CHAT_KEY_HOLD_MS));
            let inserted_up = send_input_with_report(&[key_up], &mut report, false);
            if inserted_up != 1 {
                report.key_state_uncertain = !retry_key_up(key_up, &mut report);
                return Err(InjectionFailure::OpenChatFailed);
            }
            // The chat panel can become visible before its text input accepts events.
            thread::sleep(Duration::from_millis(open_chat_focus_delay_ms));
            if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
                return Err(InjectionFailure::TargetChanged);
            }
        }

        for (batch_index, batch) in batches.iter().enumerate() {
            if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
                report.failed_batch_index = Some(batch_index);
                report.partial_prefix_possible = batch_index > 0;
                return Err(InjectionFailure::TargetChanged);
            }

            report.attempted_batches += 1;
            match input_method {
                GameInputMethod::GbkAltCode => {
                    let codes =
                        &alt_code_batches.as_ref().expect("GBK batches must exist")[batch_index];
                    for (code_index, &code) in codes.iter().enumerate() {
                        if target::validate_foreground_fast(expected_target) != Ok(true) {
                            report.failed_batch_index = Some(batch_index);
                            report.partial_prefix_possible = batch_index > 0 || code_index > 0;
                            return Err(InjectionFailure::TargetChanged);
                        }
                        if !send_alt_code(code, input_delay_ms, &mut report) {
                            report.failed_batch_index = Some(batch_index);
                            report.partial_prefix_possible = true;
                            return Err(InjectionFailure::SendInputFailed);
                        }
                    }
                }
                GameInputMethod::UnicodeSendInput => {
                    for (character_index, inputs) in
                        unicode_character_inputs(batch).iter().enumerate()
                    {
                        if target::validate_foreground_fast(expected_target) != Ok(true) {
                            report.failed_batch_index = Some(batch_index);
                            report.partial_prefix_possible = batch_index > 0 || character_index > 0;
                            return Err(InjectionFailure::TargetChanged);
                        }
                        let inserted = send_input_with_report(inputs, &mut report, true);
                        if inserted != inputs.len() as u32 {
                            report.failed_batch_index = Some(batch_index);
                            report.partial_prefix_possible =
                                inserted > 0 || batch_index > 0 || character_index > 0;
                            report.key_state_uncertain = inserted % 2 != 0
                                && !retry_key_up(inputs[inserted as usize], &mut report);
                            return Err(InjectionFailure::SendInputFailed);
                        }
                        thread::sleep(Duration::from_millis(input_delay_ms));
                    }
                }
            }

            if batch_index + 1 < batches.len() {
                thread::sleep(Duration::from_millis(batch_delay_ms));
            }
        }

        if submit {
            if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
                report.partial_prefix_possible = true;
                return Err(InjectionFailure::TargetChanged);
            }
            thread::sleep(Duration::from_millis(60));
            report.submit_attempted = true;
            let inputs = scan_code_inputs(ENTER_SCAN_CODE);
            let inserted = send_input_with_report(&inputs, &mut report, false);
            if inserted != inputs.len() as u32 {
                report.partial_prefix_possible = true;
                report.key_state_uncertain =
                    inserted % 2 != 0 && !retry_key_up(inputs[inserted as usize], &mut report);
                return Err(InjectionFailure::SubmitFailed);
            }
            report.submit_completed = true;
        }

        Ok(())
    })();

    if let Some(guard) = num_lock_guard.as_mut() {
        report.num_lock_restored = Some(guard.restore(&mut report));
    }
    drop(num_lock_guard);
    restore_keyboard_layout(&mut keyboard_layout_guard, &mut report);

    match outcome {
        Ok(()) => Ok(report),
        Err(InjectionFailure::TargetChanged) => Err(InjectionError::TargetChanged(report)),
        Err(InjectionFailure::OpenChatFailed) => Err(InjectionError::OpenChatFailed(report)),
        Err(InjectionFailure::SendInputFailed) => Err(InjectionError::SendInputFailed(report)),
        Err(InjectionFailure::SubmitFailed) => Err(InjectionError::SubmitFailed(report)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StratagemKey {
    virtual_key: u16,
    scan_code: u16,
    extended: bool,
}

#[derive(Default)]
struct HeldStratagemKeys {
    keys: Vec<StratagemKey>,
}

impl HeldStratagemKeys {
    fn track(&mut self, key: StratagemKey) {
        self.keys.push(key);
    }

    fn mark_released(&mut self, key: StratagemKey) {
        if let Some(index) = self.keys.iter().rposition(|held| *held == key) {
            self.keys.remove(index);
        }
    }

    fn release_all(&mut self, report: &mut InjectionReport) -> bool {
        let mut ok = true;
        while let Some(key) = self.keys.pop() {
            ok &= send_stratagem_key_event(key, true, report);
        }
        ok
    }
}

impl Drop for HeldStratagemKeys {
    fn drop(&mut self) {
        let mut report = empty_report("SendInputStratagem", 0, 0);
        let _ = self.release_all(&mut report);
    }
}

#[allow(clippy::result_large_err)]
pub fn inject_stratagem_macro(
    expected_target: &TargetIdentity,
    macro_config: &StratagemMacro,
    title_keyword: &str,
) -> Result<InjectionReport, InjectionError> {
    let mut report = empty_report(
        "SendInputStratagem",
        macro_config.sequence.len(),
        macro_config.press_delay_ms,
    );
    let menu_key = stratagem_key(&macro_config.menu_key).ok_or_else(|| {
        InjectionError::UnsupportedKey(report.clone(), macro_config.menu_key.clone())
    })?;
    #[allow(clippy::result_large_err)]
    let sequence = macro_config
        .sequence
        .iter()
        .map(|code| {
            stratagem_key(code)
                .ok_or_else(|| InjectionError::UnsupportedKey(report.clone(), code.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut held = HeldStratagemKeys::default();

    let outcome = (|| -> Result<(), InjectionFailure> {
        if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
            return Err(InjectionFailure::TargetChanged);
        }

        if !send_stratagem_key_event(menu_key, true, &mut report) {
            return Err(InjectionFailure::SendInputFailed);
        }
        thread::sleep(Duration::from_millis(10));

        match macro_config.menu_mode {
            StratagemMenuMode::Hold => {
                if !send_stratagem_key_event(menu_key, false, &mut report) {
                    report.key_state_uncertain = true;
                    return Err(InjectionFailure::SendInputFailed);
                }
                held.track(menu_key);
            }
            StratagemMenuMode::Toggle => {
                if !tap_stratagem_key(
                    menu_key,
                    macro_config.press_delay_ms.saturating_add(20),
                    &mut report,
                ) {
                    return Err(InjectionFailure::SendInputFailed);
                }
            }
        }

        thread::sleep(Duration::from_millis(macro_config.menu_open_delay_ms));

        for (index, key) in sequence.into_iter().enumerate() {
            if target::validate_foreground_fast(expected_target) != Ok(true) {
                report.failed_batch_index = Some(index);
                report.partial_prefix_possible = index > 0;
                return Err(InjectionFailure::TargetChanged);
            }
            report.attempted_batches += 1;
            if !send_stratagem_key_event(key, false, &mut report) {
                report.failed_batch_index = Some(index);
                report.key_state_uncertain = true;
                report.partial_prefix_possible = index > 0;
                return Err(InjectionFailure::SendInputFailed);
            }
            held.track(key);
            thread::sleep(Duration::from_millis(macro_config.press_delay_ms));
            if !send_stratagem_key_event(key, true, &mut report) {
                report.failed_batch_index = Some(index);
                report.key_state_uncertain = true;
                report.partial_prefix_possible = true;
                return Err(InjectionFailure::SendInputFailed);
            }
            held.mark_released(key);
            thread::sleep(Duration::from_millis(macro_config.interval_delay_ms));
        }

        if macro_config.menu_mode == StratagemMenuMode::Hold {
            thread::sleep(Duration::from_millis(50));
            if !send_stratagem_key_event(menu_key, true, &mut report) {
                report.key_state_uncertain = true;
                return Err(InjectionFailure::SendInputFailed);
            }
            held.mark_released(menu_key);
        }

        Ok(())
    })();

    if !held.release_all(&mut report) {
        report.key_state_uncertain = true;
    }

    match outcome {
        Ok(()) => Ok(report),
        Err(InjectionFailure::TargetChanged) => Err(InjectionError::TargetChanged(report)),
        Err(InjectionFailure::SendInputFailed) => Err(InjectionError::SendInputFailed(report)),
        Err(InjectionFailure::OpenChatFailed) => Err(InjectionError::OpenChatFailed(report)),
        Err(InjectionFailure::SubmitFailed) => Err(InjectionError::SubmitFailed(report)),
    }
}

#[allow(clippy::result_large_err)]
pub fn cancel_open_chat(
    expected_target: &TargetIdentity,
    title_keyword: &str,
) -> Result<(), InjectionError> {
    let mut report = InjectionReport {
        attempted_batches: 0,
        requested_events: 0,
        successful_events: 0,
        text_requested_events: 0,
        text_successful_events: 0,
        last_error_code: None,
        delivery_transport: "SendInput".to_owned(),
        delivery_acknowledged: true,
        input_characters: 0,
        input_delay_ms: 0,
        keyboard_layout_switched: false,
        keyboard_layout_before: None,
        keyboard_layout_requested: None,
        keyboard_layout_restored: None,
        num_lock_toggled: false,
        num_lock_restored: None,
        failed_batch_index: None,
        partial_prefix_possible: false,
        key_state_uncertain: false,
        submit_attempted: false,
        submit_completed: false,
    };
    if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
        return Err(InjectionError::TargetChanged(report));
    }
    let inputs = scan_code_inputs(ESCAPE_SCAN_CODE);
    let inserted = send_input_with_report(&inputs, &mut report, false);
    if inserted != inputs.len() as u32 {
        report.key_state_uncertain =
            inserted % 2 != 0 && !retry_key_up(inputs[inserted as usize], &mut report);
        return Err(InjectionError::SendInputFailed(report));
    }
    Ok(())
}

fn empty_report(transport: &str, input_characters: usize, input_delay_ms: u64) -> InjectionReport {
    InjectionReport {
        attempted_batches: 0,
        requested_events: 0,
        successful_events: 0,
        text_requested_events: 0,
        text_successful_events: 0,
        last_error_code: None,
        delivery_transport: transport.to_owned(),
        delivery_acknowledged: true,
        input_characters,
        input_delay_ms,
        keyboard_layout_switched: false,
        keyboard_layout_before: None,
        keyboard_layout_requested: None,
        keyboard_layout_restored: None,
        num_lock_toggled: false,
        num_lock_restored: None,
        failed_batch_index: None,
        partial_prefix_possible: false,
        key_state_uncertain: false,
        submit_attempted: false,
        submit_completed: false,
    }
}

fn tap_stratagem_key(key: StratagemKey, hold_ms: u64, report: &mut InjectionReport) -> bool {
    if !send_stratagem_key_event(key, false, report) {
        report.key_state_uncertain = true;
        return false;
    }
    thread::sleep(Duration::from_millis(hold_ms));
    if !send_stratagem_key_event(key, true, report) {
        report.key_state_uncertain = !retry_key_up(
            scan_code_input_with_extended(key.scan_code, key.extended, true),
            report,
        );
        return false;
    }
    true
}

fn retry_key_up(input: INPUT, report: &mut InjectionReport) -> bool {
    let inserted = send_input_with_report(&[input], report, false);
    inserted == 1
}

fn send_stratagem_key_event(key: StratagemKey, key_up: bool, report: &mut InjectionReport) -> bool {
    let input = scan_code_input_with_extended(key.scan_code, key.extended, key_up);
    let inserted = send_input_with_report(&[input], report, false);
    inserted == 1
}

fn scan_code_input_with_extended(scan_code: u16, extended: bool, key_up: bool) -> INPUT {
    let mut flags = KEYEVENTF_SCANCODE;
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    keyboard_input(scan_code, flags)
}

fn stratagem_key(code: &str) -> Option<StratagemKey> {
    let virtual_key = virtual_key_for_input_code(code.trim())?;
    let scan_code = scan_code_for_virtual_key(virtual_key)?;
    Some(StratagemKey {
        virtual_key,
        scan_code,
        extended: is_extended_virtual_key(virtual_key),
    })
}

fn virtual_key_for_input_code(code: &str) -> Option<u16> {
    let named = match code {
        "ControlLeft" => 0xA2,
        "ControlRight" => 0xA3,
        "ShiftLeft" => 0xA0,
        "ShiftRight" => 0xA1,
        "AltLeft" => 0xA4,
        "AltRight" => 0xA5,
        "ArrowUp" | "Up" | "KeyW" | "W" => {
            return if matches!(code, "KeyW" | "W") {
                Some(u16::from(b'W'))
            } else {
                Some(0x26)
            };
        }
        "ArrowDown" | "Down" | "KeyS" | "S" => {
            return if matches!(code, "KeyS" | "S") {
                Some(u16::from(b'S'))
            } else {
                Some(0x28)
            };
        }
        "ArrowLeft" | "Left" | "KeyA" | "A" => {
            return if matches!(code, "KeyA" | "A") {
                Some(u16::from(b'A'))
            } else {
                Some(0x25)
            };
        }
        "ArrowRight" | "Right" | "KeyD" | "D" => {
            return if matches!(code, "KeyD" | "D") {
                Some(u16::from(b'D'))
            } else {
                Some(0x27)
            };
        }
        "Enter" => 0x0D,
        "NumpadEnter" => 0x0D,
        "Space" => 0x20,
        "Tab" => 0x09,
        "Escape" => 0x1B,
        "CapsLock" => 0x14,
        "Backquote" => 0xC0,
        "Minus" => 0xBD,
        "Equal" => 0xBB,
        "BracketLeft" => 0xDB,
        "BracketRight" => 0xDD,
        "Backslash" => 0xDC,
        "Semicolon" => 0xBA,
        "Quote" => 0xDE,
        "Comma" => 0xBC,
        "Period" => 0xBE,
        "Slash" => 0xBF,
        value if value.len() == 4 && value.starts_with("Key") => {
            return value
                .as_bytes()
                .get(3)
                .copied()
                .filter(u8::is_ascii_uppercase)
                .map(u16::from);
        }
        value if value.len() == 6 && value.starts_with("Digit") => {
            return value
                .as_bytes()
                .get(5)
                .copied()
                .filter(u8::is_ascii_digit)
                .map(u16::from);
        }
        value if value.starts_with('F') => {
            return value[1..]
                .parse::<u16>()
                .ok()
                .filter(|number| (1..=24).contains(number))
                .map(|number| 0x6F + number);
        }
        _ => return None,
    };
    Some(named)
}

fn scan_code_for_virtual_key(virtual_key: u16) -> Option<u16> {
    const LETTERS: [u16; 26] = [
        0x1e, 0x30, 0x2e, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31, 0x18,
        0x19, 0x10, 0x13, 0x1f, 0x14, 0x16, 0x2f, 0x11, 0x2d, 0x15, 0x2c,
    ];
    const DIGITS: [u16; 10] = [0x0b, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a];

    let scan = match virtual_key {
        0x41..=0x5a => LETTERS[usize::from(virtual_key - 0x41)],
        0x30..=0x39 => DIGITS[usize::from(virtual_key - 0x30)],
        0x70..=0x79 => 0x3b + (virtual_key - 0x70),
        0x7a => 0x57,
        0x7b => 0x58,
        0x7c..=0x86 => 0x64 + (virtual_key - 0x7c),
        0x87 => 0x76,
        0x09 => 0x0f,
        0x0d => 0x1c,
        0x14 => 0x3a,
        0x1b => 0x01,
        0x20 => 0x39,
        0x25 => 0x4b,
        0x26 => 0x48,
        0x27 => 0x4d,
        0x28 => 0x50,
        0xa0 => 0x2a,
        0xa1 => 0x36,
        0xa2 | 0xa3 => 0x1d,
        0xa4 | 0xa5 => 0x38,
        0xba => 0x27,
        0xbb => 0x0d,
        0xbc => 0x33,
        0xbd => 0x0c,
        0xbe => 0x34,
        0xbf => 0x35,
        0xc0 => 0x29,
        0xdb => 0x1a,
        0xdc => 0x2b,
        0xdd => 0x1b,
        0xde => 0x28,
        _ => return None,
    };
    Some(scan)
}

fn is_extended_virtual_key(virtual_key: u16) -> bool {
    matches!(virtual_key, 0xA3 | 0xA5 | 0x25 | 0x26 | 0x27 | 0x28)
}

fn encode_alt_code_batches(batches: &[Vec<u16>]) -> Result<Vec<Vec<u32>>, char> {
    batches
        .iter()
        .map(|batch| {
            let text = String::from_utf16(batch).map_err(|_| '\u{FFFD}')?;
            text.chars().map(gbk_alt_code).collect()
        })
        .collect()
}

fn gbk_alt_code(character: char) -> Result<u32, char> {
    let source = character.to_string();
    let (encoded, _, had_errors) = GBK.encode(&source);
    if had_errors || encoded.is_empty() || encoded.len() > 2 {
        return Err(character);
    }
    Ok(encoded
        .iter()
        .fold(0u32, |value, byte| (value << 8) | u32::from(*byte)))
}

fn num_lock_enabled() -> bool {
    (unsafe { GetKeyState(VK_NUMLOCK.0 as i32) } & 1) == 1
}

fn send_alt_code(code: u32, input_delay_ms: u64, report: &mut InjectionReport) -> bool {
    if !send_text_scan_code_event(ALT_SCAN_CODE, false, report) {
        return false;
    }
    let alt_guard = AltReleaseGuard::armed();
    thread::sleep(Duration::from_millis(input_delay_ms));

    for digit in code.to_string().bytes() {
        let value = usize::from(digit - b'0');
        let scan_code = NUMPAD_SCAN_CODES[value];
        if !send_text_scan_code_event(scan_code, false, report) {
            report.key_state_uncertain = true;
            return false;
        }
        thread::sleep(Duration::from_millis(input_delay_ms));
        if !send_text_scan_code_event(scan_code, true, report) {
            report.key_state_uncertain = true;
            return false;
        }
        thread::sleep(Duration::from_millis(input_delay_ms));
    }

    if !alt_guard.release(report) {
        report.key_state_uncertain = true;
        return false;
    }
    thread::sleep(Duration::from_millis(input_delay_ms));
    true
}

fn send_scan_code_event(scan_code: u16, key_up: bool, report: &mut InjectionReport) -> bool {
    let input = scan_code_input(scan_code, key_up);
    let inserted = send_input_with_report(&[input], report, false);
    inserted == 1
}

fn send_text_scan_code_event(scan_code: u16, key_up: bool, report: &mut InjectionReport) -> bool {
    let input = scan_code_input(scan_code, key_up);
    let inserted = send_input_with_report(&[input], report, true);
    inserted == 1
}

fn send_input_with_report(inputs: &[INPUT], report: &mut InjectionReport, text_event: bool) -> u32 {
    let requested = inputs.len() as u32;
    report.requested_events += requested;
    if text_event {
        report.text_requested_events += requested;
    }
    let inserted = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    report.successful_events += inserted;
    if text_event {
        report.text_successful_events += inserted;
    }
    if inserted != requested {
        report.delivery_acknowledged = false;
        report.last_error_code = Some(unsafe { GetLastError().0 });
    }
    inserted
}

fn restore_keyboard_layout(guard: &mut Option<KeyboardLayoutGuard>, report: &mut InjectionReport) {
    if let Some(guard) = guard.as_mut() {
        report.keyboard_layout_restored = Some(guard.restore());
    }
}

fn select_simplified_chinese_layout(current: HKL) -> Option<HKL> {
    if keyboard_layout_language_id(current) == SIMPLIFIED_CHINESE_LANGUAGE_ID {
        return Some(current);
    }

    let count = unsafe { GetKeyboardLayoutList(None) };
    if count <= 0 {
        return None;
    }
    let mut layouts = vec![HKL(std::ptr::null_mut()); count as usize];
    let copied = unsafe { GetKeyboardLayoutList(Some(&mut layouts)) };
    if copied <= 0 {
        return None;
    }
    layouts.truncate(copied as usize);
    layouts
        .into_iter()
        .find(|layout| keyboard_layout_language_id(*layout) == SIMPLIFIED_CHINESE_LANGUAGE_ID)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameplayKeyboardLayoutHandoff {
    pub before: u32,
    pub active: u32,
    pub changed: bool,
}

pub fn current_keyboard_layout(
    expected_target: &TargetIdentity,
) -> Result<GameplayKeyboardLayoutHandoff, String> {
    let current = unsafe { GetKeyboardLayout(expected_target.thread_id) };
    if current.is_invalid() {
        return Err("无法读取 HD2 当前键盘布局".to_owned());
    }
    let layout = keyboard_layout_id(current);
    Ok(GameplayKeyboardLayoutHandoff {
        before: layout,
        active: layout,
        changed: false,
    })
}

fn keyboard_layout_language_id(layout: HKL) -> usize {
    layout.0 as usize & 0xFFFF
}

fn keyboard_layout_id(layout: HKL) -> u32 {
    layout.0 as usize as u32
}

fn request_keyboard_layout(hwnd: HWND, layout: HKL) -> bool {
    let mut message_result = 0usize;
    unsafe { SetLastError(WIN32_ERROR(0)) };
    let result = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(layout.0 as isize),
            SMTO_ABORTIFHUNG,
            250,
            Some(&mut message_result),
        )
    };
    result.0 != 0 || unsafe { GetLastError().0 } == 0
}

fn wait_for_keyboard_layout(thread_id: u32, expected: HKL) -> bool {
    let deadline = Instant::now() + Duration::from_millis(KEYBOARD_LAYOUT_TIMEOUT_MS);
    while Instant::now() < deadline {
        if unsafe { GetKeyboardLayout(thread_id) } == expected {
            return true;
        }
        thread::sleep(Duration::from_millis(KEYBOARD_LAYOUT_POLL_MS));
    }
    false
}

fn count_utf16_characters(batches: &[Vec<u16>]) -> usize {
    batches
        .iter()
        .map(|batch| String::from_utf16_lossy(batch).chars().count())
        .sum()
}

fn unicode_inputs(units: &[u16]) -> Vec<INPUT> {
    let mut inputs = Vec::with_capacity(units.len() * 2);
    for &unit in units {
        inputs.push(keyboard_input(unit, KEYEVENTF_UNICODE));
        inputs.push(keyboard_input(unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    inputs
}

fn unicode_character_inputs(units: &[u16]) -> Vec<Vec<INPUT>> {
    let mut groups = Vec::new();
    let mut index = 0;
    while index < units.len() {
        let length = if (0xD800..=0xDBFF).contains(&units[index])
            && units
                .get(index + 1)
                .is_some_and(|unit| (0xDC00..=0xDFFF).contains(unit))
        {
            2
        } else {
            1
        };
        groups.push(unicode_inputs(&units[index..index + length]));
        index += length;
    }
    groups
}

fn keyboard_input(
    scan: u16,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS,
) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn virtual_key_inputs(key: VIRTUAL_KEY) -> [INPUT; 2] {
    [virtual_key_input(key, false), virtual_key_input(key, true)]
}

fn virtual_key_input(key: VIRTUAL_KEY, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
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

fn scan_code_inputs(scan_code: u16) -> [INPUT; 2] {
    [
        scan_code_input(scan_code, false),
        scan_code_input(scan_code, true),
    ]
}

fn scan_code_input(scan_code: u16, key_up: bool) -> INPUT {
    keyboard_input(
        scan_code,
        if key_up {
            KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP
        } else {
            KEYEVENTF_SCANCODE
        },
    )
}

fn chat_preparation_scan_codes(mode: ChatPreparation) -> &'static [u16] {
    match mode {
        ChatPreparation::KeepOpen => &[],
        ChatPreparation::Open => &[ENTER_SCAN_CODE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_enter_uses_a_balanced_scan_code_pair() {
        let inputs = scan_code_inputs(ENTER_SCAN_CODE);
        let down = unsafe { inputs[0].Anonymous.ki };
        let up = unsafe { inputs[1].Anonymous.ki };
        assert_eq!(down.wVk, VIRTUAL_KEY(0));
        assert_eq!(down.wScan, ENTER_SCAN_CODE);
        assert_eq!(down.dwFlags, KEYEVENTF_SCANCODE);
        assert_eq!(up.wVk, VIRTUAL_KEY(0));
        assert_eq!(up.wScan, ENTER_SCAN_CODE);
        assert_eq!(up.dwFlags, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP);
    }

    #[test]
    fn open_chat_enter_uses_a_balanced_scan_code_pair() {
        let inputs = scan_code_inputs(ENTER_SCAN_CODE);
        let down = unsafe { inputs[0].Anonymous.ki };
        let up = unsafe { inputs[1].Anonymous.ki };
        assert_eq!(down.wVk, VIRTUAL_KEY(0));
        assert_eq!(down.wScan, ENTER_SCAN_CODE);
        assert_eq!(down.dwFlags, KEYEVENTF_SCANCODE);
        assert_eq!(up.wVk, VIRTUAL_KEY(0));
        assert_eq!(up.wScan, ENTER_SCAN_CODE);
        assert_eq!(up.dwFlags, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP);
    }

    #[test]
    fn chat_preparation_modes_use_explicit_key_sequences() {
        assert_eq!(
            chat_preparation_scan_codes(ChatPreparation::KeepOpen),
            &[] as &[u16]
        );
        assert_eq!(
            chat_preparation_scan_codes(ChatPreparation::Open),
            &[ENTER_SCAN_CODE]
        );
    }

    #[test]
    fn stratagem_arrows_use_extended_scan_codes() {
        let key = stratagem_key("ArrowUp").expect("arrow key should be supported");
        assert!(key.extended);
        let input = scan_code_input_with_extended(key.scan_code, key.extended, false);
        let down = unsafe { input.Anonymous.ki };
        assert_eq!(down.wVk, VIRTUAL_KEY(0));
        assert_eq!(down.wScan, 0x48);
        assert_eq!(down.dwFlags, KEYEVENTF_SCANCODE | KEYEVENTF_EXTENDEDKEY);
    }

    #[test]
    fn stratagem_wasd_uses_layout_independent_scan_codes() {
        let key = stratagem_key("W").expect("W should be supported");
        assert!(!key.extended);
        assert_eq!(key.scan_code, 0x11);
        assert_eq!(
            stratagem_key("ControlLeft")
                .expect("left ctrl should be supported")
                .scan_code,
            0x1D
        );
    }

    #[test]
    fn unicode_character_groups_keep_surrogate_pairs_together() {
        let groups = unicode_character_inputs(&[b'A' as u16, 0xD83D, 0xDE42, b'B' as u16]);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1].len(), 4);
        assert_eq!(groups[2].len(), 2);
    }

    #[test]
    fn diagnostic_character_count_treats_surrogate_pairs_as_one_character() {
        assert_eq!(
            count_utf16_characters(&[vec![b'A' as u16, 0xD83D, 0xDE42], vec![b'B' as u16]]),
            3
        );
    }

    #[test]
    fn gbk_alt_codes_match_cp936_byte_values() {
        assert_eq!(gbk_alt_code('A'), Ok(65));
        assert_eq!(gbk_alt_code('你'), Ok(50_403));
        assert_eq!(gbk_alt_code('🙂'), Err('🙂'));
    }

    #[test]
    fn numpad_scan_codes_match_physical_digit_keys() {
        assert_eq!(NUMPAD_SCAN_CODES[0], 0x52);
        assert_eq!(NUMPAD_SCAN_CODES[1], 0x4F);
        assert_eq!(NUMPAD_SCAN_CODES[5], 0x4C);
        assert_eq!(NUMPAD_SCAN_CODES[9], 0x49);
    }

    #[test]
    fn keyboard_layout_language_id_uses_the_low_word() {
        let layout = HKL(0xE020_0804usize as *mut c_void);
        assert_eq!(
            keyboard_layout_language_id(layout),
            SIMPLIFIED_CHINESE_LANGUAGE_ID
        );
    }
}
