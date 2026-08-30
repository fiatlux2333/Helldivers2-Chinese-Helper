use crate::core::session::KeySnapshot;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL,
    VK_RETURN, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};

const VK_ESCAPE_CODE: u16 = 0x1B;

pub fn sample_keys() -> KeySnapshot {
    KeySnapshot {
        enter_down: high_bit(VK_RETURN.0),
        ctrl_down: high_bit(VK_CONTROL.0),
        alt_down: high_bit(VK_MENU.0),
        shift_down: high_bit(VK_SHIFT.0),
        win_down: high_bit(VK_LWIN.0) || high_bit(VK_RWIN.0),
    }
}

fn high_bit(virtual_key: u16) -> bool {
    unsafe { (GetAsyncKeyState(i32::from(virtual_key)) as u16 & 0x8000) != 0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyReleaseReport {
    pub requested_events: u32,
    pub inserted_events: u32,
    pub last_error_code: Option<u32>,
}

pub fn release_possible_stuck_keys() -> KeyReleaseReport {
    const KEYS: [VIRTUAL_KEY; 13] = [
        VK_RETURN,
        VIRTUAL_KEY(VK_ESCAPE_CODE),
        VK_CONTROL,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_MENU,
        VK_LMENU,
        VK_RMENU,
        VK_SHIFT,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_LWIN,
        VK_RWIN,
    ];
    let inputs = KEYS.map(key_up_input);
    let inserted_events = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    KeyReleaseReport {
        requested_events: inputs.len() as u32,
        inserted_events,
        last_error_code: (inserted_events != inputs.len() as u32)
            .then(|| unsafe { GetLastError().0 }),
    }
}

fn key_up_input(key: VIRTUAL_KEY) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                // Marked so the chat-key hook recognizes these as our own
                // injections instead of third-party (remote) input.
                dwExtraInfo: crate::platform::windows::game_monitor::SELF_INJECTED_EXTRA_INFO,
            },
        },
    }
}
