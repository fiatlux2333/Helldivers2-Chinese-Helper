use crate::core::session::KeySnapshot;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RETURN, VK_RWIN, VK_SHIFT,
};

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
