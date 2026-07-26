use crate::{
    core::session::TargetIdentity,
    platform::{InjectionReport, windows::target},
};
use std::{mem::size_of, thread, time::Duration};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_RETURN,
};

const ENTER_SCAN_CODE: u16 = 0x1C;
const OPEN_CHAT_KEY_HOLD_MS: u64 = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectionError {
    TargetChanged(InjectionReport),
    OpenChatFailed(InjectionReport),
    SendInputFailed(InjectionReport),
    SubmitFailed(InjectionReport),
}

pub fn inject_utf16_batches(
    expected_target: &TargetIdentity,
    batches: &[Vec<u16>],
    batch_delay_ms: u64,
    title_keyword: &str,
    open_chat: bool,
    open_chat_focus_delay_ms: u64,
    submit: bool,
) -> Result<InjectionReport, InjectionError> {
    let mut report = InjectionReport {
        attempted_batches: 0,
        successful_events: 0,
        failed_batch_index: None,
        partial_prefix_possible: false,
        key_state_uncertain: false,
        submit_attempted: false,
        submit_completed: false,
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
        let inputs = unicode_inputs(batch);
        let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        report.successful_events += inserted;
        if inserted != inputs.len() as u32 {
            report.failed_batch_index = Some(batch_index);
            report.partial_prefix_possible = inserted > 0 || batch_index > 0;
            report.key_state_uncertain = inserted % 2 != 0;
            return Err(InjectionError::SendInputFailed(report));
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
        let inputs = virtual_key_inputs(VK_RETURN);
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
    [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ]
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
    fn enter_sequence_is_a_balanced_down_up_pair() {
        let inputs = virtual_key_inputs(VK_RETURN);
        let down = unsafe { inputs[0].Anonymous.ki };
        let up = unsafe { inputs[1].Anonymous.ki };
        assert_eq!(down.wVk, VK_RETURN);
        assert_eq!(down.dwFlags, Default::default());
        assert_eq!(up.wVk, VK_RETURN);
        assert_eq!(up.dwFlags, KEYEVENTF_KEYUP);
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
}
