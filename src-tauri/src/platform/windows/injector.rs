use crate::{
    core::{session::TargetIdentity, translation::GameInputMethod},
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
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyState, GetKeyboardLayout, GetKeyboardLayoutList, HKL, INPUT, INPUT_0,
            INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE,
            SendInput, VIRTUAL_KEY, VK_NUMLOCK,
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
            let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
            report.successful_events += inserted;
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
        let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        report.successful_events += inserted;
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
        successful_events: 0,
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
            let inserted_down = unsafe { SendInput(&[key_down], size_of::<INPUT>() as i32) };
            report.successful_events += inserted_down;
            if inserted_down != 1 {
                return Err(InjectionFailure::OpenChatFailed);
            }
            thread::sleep(Duration::from_millis(OPEN_CHAT_KEY_HOLD_MS));
            let inserted_up = unsafe { SendInput(&[key_up], size_of::<INPUT>() as i32) };
            report.successful_events += inserted_up;
            if inserted_up != 1 {
                report.key_state_uncertain = true;
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
                        let inserted = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
                        report.successful_events += inserted;
                        if inserted != inputs.len() as u32 {
                            report.failed_batch_index = Some(batch_index);
                            report.partial_prefix_possible =
                                inserted > 0 || batch_index > 0 || character_index > 0;
                            report.key_state_uncertain = inserted % 2 != 0;
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
            let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
            report.successful_events += inserted;
            if inserted != inputs.len() as u32 {
                report.partial_prefix_possible = true;
                report.key_state_uncertain = inserted % 2 != 0;
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

pub fn cancel_open_chat(
    expected_target: &TargetIdentity,
    title_keyword: &str,
) -> Result<(), InjectionError> {
    let mut report = InjectionReport {
        attempted_batches: 0,
        successful_events: 0,
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
    let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    report.successful_events = inserted;
    if inserted != inputs.len() as u32 {
        report.key_state_uncertain = inserted % 2 != 0;
        return Err(InjectionError::SendInputFailed(report));
    }
    Ok(())
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
    if !send_scan_code_event(ALT_SCAN_CODE, false, report) {
        return false;
    }
    let alt_guard = AltReleaseGuard::armed();
    thread::sleep(Duration::from_millis(input_delay_ms));

    for digit in code.to_string().bytes() {
        let value = usize::from(digit - b'0');
        let scan_code = NUMPAD_SCAN_CODES[value];
        if !send_scan_code_event(scan_code, false, report) {
            report.key_state_uncertain = true;
            return false;
        }
        thread::sleep(Duration::from_millis(input_delay_ms));
        if !send_scan_code_event(scan_code, true, report) {
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
    let inserted = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
    report.successful_events += inserted;
    inserted == 1
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

fn keyboard_layout_language_id(layout: HKL) -> usize {
    layout.0 as usize & 0xFFFF
}

fn keyboard_layout_id(layout: HKL) -> u32 {
    layout.0 as usize as u32
}

fn request_keyboard_layout(hwnd: HWND, layout: HKL) {
    let mut message_result = 0usize;
    let _ = unsafe {
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
