use crate::{
    core::session::TargetIdentity,
    platform::{InjectionReport, windows::target},
};
use std::{mem::size_of, thread, time::Duration};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
    VIRTUAL_KEY,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectionError {
    TargetChanged(InjectionReport),
    SendInputFailed(InjectionReport),
}

pub fn inject_utf16_batches(
    expected_target: &TargetIdentity,
    batches: &[Vec<u16>],
    batch_delay_ms: u64,
    title_keyword: &str,
) -> Result<InjectionReport, InjectionError> {
    let mut report = InjectionReport {
        attempted_batches: 0,
        successful_events: 0,
        failed_batch_index: None,
        partial_prefix_possible: false,
        key_state_uncertain: false,
    };

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
