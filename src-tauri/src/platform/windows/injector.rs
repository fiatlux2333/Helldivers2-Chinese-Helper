use crate::{
    core::{session::TargetIdentity, translation::GameInputMethod},
    platform::{InjectionReport, windows::target},
};
use encoding_rs::GBK;
use std::{mem::size_of, thread, time::Duration};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_MENU, VK_NUMLOCK, VK_NUMPAD0, keybd_event,
};

const ENTER_SCAN_CODE: u16 = 0x1C;
const ESCAPE_SCAN_CODE: u16 = 0x01;
const ALT_SCAN_CODE: u8 = 0x38;
const OPEN_CHAT_KEY_HOLD_MS: u64 = 40;
const ALT_CODE_KEY_DELAY_MS: u64 = 5;
const NUMPAD_SCAN_CODES: [u8; 10] = [0x52, 0x4F, 0x50, 0x51, 0x4B, 0x4C, 0x4D, 0x47, 0x48, 0x49];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectionError {
    TargetChanged(InjectionReport),
    OpenChatFailed(InjectionReport),
    SendInputFailed(InjectionReport),
    SubmitFailed(InjectionReport),
    UnrepresentableCharacter(InjectionReport, char),
}

struct NumLockGuard {
    original_enabled: bool,
}

impl NumLockGuard {
    fn new(report: &mut InjectionReport) -> Option<Self> {
        let original_enabled = num_lock_enabled();
        if !original_enabled {
            let inputs = virtual_key_inputs(VK_NUMLOCK);
            let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
            report.successful_events += inserted;
            report.num_lock_toggled = true;
            thread::sleep(Duration::from_millis(ALT_CODE_KEY_DELAY_MS));
            if inserted != inputs.len() as u32 || !num_lock_enabled() {
                report.key_state_uncertain = inserted % 2 != 0;
                return None;
            }
        }
        Some(Self { original_enabled })
    }
}

impl Drop for NumLockGuard {
    fn drop(&mut self) {
        if num_lock_enabled() != self.original_enabled {
            let inputs = virtual_key_inputs(VK_NUMLOCK);
            let _ = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        }
    }
}

struct LegacyAltReleaseGuard {
    active: bool,
}

impl LegacyAltReleaseGuard {
    fn armed() -> Self {
        Self { active: true }
    }

    fn release(mut self, report: &mut InjectionReport) {
        if self.active {
            send_legacy_key_event(VK_MENU, ALT_SCAN_CODE, true, report);
            self.active = false;
        }
    }
}

impl Drop for LegacyAltReleaseGuard {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                keybd_event(VK_MENU.0 as u8, ALT_SCAN_CODE, KEYEVENTF_KEYUP, 0);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn inject_utf16_batches(
    expected_target: &TargetIdentity,
    batches: &[Vec<u16>],
    batch_delay_ms: u64,
    title_keyword: &str,
    open_chat: bool,
    open_chat_focus_delay_ms: u64,
    submit: bool,
    input_method: GameInputMethod,
) -> Result<InjectionReport, InjectionError> {
    let mut report = InjectionReport {
        attempted_batches: 0,
        successful_events: 0,
        delivery_transport: match input_method {
            GameInputMethod::GbkAltCode => "keybd_event".to_owned(),
            GameInputMethod::UnicodeSendInput => "SendInput".to_owned(),
        },
        delivery_acknowledged: input_method == GameInputMethod::UnicodeSendInput,
        keyboard_layout_switched: false,
        num_lock_toggled: false,
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
    let _num_lock_guard = if input_method == GameInputMethod::GbkAltCode {
        match NumLockGuard::new(&mut report) {
            Some(guard) => Some(guard),
            None => return Err(InjectionError::SendInputFailed(report)),
        }
    } else {
        None
    };

    if open_chat {
        if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
            return Err(InjectionError::TargetChanged(report));
        }
        let [key_down, key_up] = scan_code_inputs(ENTER_SCAN_CODE);
        let inserted_down = unsafe { SendInput(&[key_down], size_of::<INPUT>() as i32) };
        report.successful_events += inserted_down;
        if inserted_down != 1 {
            return Err(InjectionError::OpenChatFailed(report));
        }
        thread::sleep(Duration::from_millis(OPEN_CHAT_KEY_HOLD_MS));
        let inserted_up = unsafe { SendInput(&[key_up], size_of::<INPUT>() as i32) };
        report.successful_events += inserted_up;
        if inserted_up != 1 {
            report.key_state_uncertain = true;
            return Err(InjectionError::OpenChatFailed(report));
        }
        // The chat panel can become visible before its text input accepts events.
        thread::sleep(Duration::from_millis(open_chat_focus_delay_ms));
        if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
            return Err(InjectionError::TargetChanged(report));
        }
    }

    for (batch_index, batch) in batches.iter().enumerate() {
        if target::validate_foreground(expected_target, title_keyword) != Ok(true) {
            report.failed_batch_index = Some(batch_index);
            report.partial_prefix_possible = report.successful_events > 0;
            return Err(InjectionError::TargetChanged(report));
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
                        return Err(InjectionError::TargetChanged(report));
                    }
                    send_alt_code(code, &mut report);
                }
            }
            GameInputMethod::UnicodeSendInput => {
                let inputs = unicode_inputs(batch);
                let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
                report.successful_events += inserted;
                if inserted != inputs.len() as u32 {
                    report.failed_batch_index = Some(batch_index);
                    report.partial_prefix_possible = inserted > 0 || batch_index > 0;
                    report.key_state_uncertain = inserted % 2 != 0;
                    return Err(InjectionError::SendInputFailed(report));
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
            return Err(InjectionError::TargetChanged(report));
        }
        // Give HD2 chat a brief settle window after unicode injection.
        thread::sleep(Duration::from_millis(60));
        report.submit_attempted = true;
        let inputs = scan_code_inputs(ENTER_SCAN_CODE);
        let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        report.successful_events += inserted;
        if inserted != inputs.len() as u32 {
            report.partial_prefix_possible = true;
            report.key_state_uncertain = inserted % 2 != 0;
            return Err(InjectionError::SubmitFailed(report));
        }
        report.submit_completed = true;
    }

    Ok(report)
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
        keyboard_layout_switched: false,
        num_lock_toggled: false,
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

fn send_alt_code(code: u32, report: &mut InjectionReport) {
    send_legacy_key_event(VK_MENU, ALT_SCAN_CODE, false, report);
    let alt_guard = LegacyAltReleaseGuard::armed();
    thread::sleep(Duration::from_millis(ALT_CODE_KEY_DELAY_MS));

    for digit in code.to_string().bytes() {
        let value = usize::from(digit - b'0');
        let key = VIRTUAL_KEY(VK_NUMPAD0.0 + value as u16);
        let scan_code = NUMPAD_SCAN_CODES[value];
        send_legacy_key_event(key, scan_code, false, report);
        thread::sleep(Duration::from_millis(ALT_CODE_KEY_DELAY_MS));
        send_legacy_key_event(key, scan_code, true, report);
        thread::sleep(Duration::from_millis(ALT_CODE_KEY_DELAY_MS));
    }

    alt_guard.release(report);
    thread::sleep(Duration::from_millis(ALT_CODE_KEY_DELAY_MS));
}

fn send_legacy_key_event(
    key: VIRTUAL_KEY,
    scan_code: u8,
    key_up: bool,
    report: &mut InjectionReport,
) {
    unsafe {
        keybd_event(
            key.0 as u8,
            scan_code,
            if key_up {
                KEYEVENTF_KEYUP
            } else {
                Default::default()
            },
            0,
        );
    }
    report.successful_events += 1;
}

fn unicode_inputs(units: &[u16]) -> Vec<INPUT> {
    let mut inputs = Vec::with_capacity(units.len() * 2);
    for &unit in units {
        inputs.push(keyboard_input(unit, KEYEVENTF_UNICODE));
        inputs.push(keyboard_input(unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    inputs
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
        keyboard_input(scan_code, KEYEVENTF_SCANCODE),
        keyboard_input(scan_code, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP),
    ]
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
}
