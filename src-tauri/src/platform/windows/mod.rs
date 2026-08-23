pub mod capture;
pub mod game_monitor;
pub mod injector;
pub mod integrity;
pub mod key_state;
pub mod target;

use windows::Win32::{
    System::SystemInformation::{GetVersionExW, OSVERSIONINFOW},
    UI::Input::KeyboardAndMouse::{GetKeyboardLayoutList, HKL},
};

/// One-line system environment summary for diagnostic exports.
/// Captures the Windows build, installed keyboard layouts, and this process's
/// integrity level — the three factors behind most fan-reported "doesn't work"
/// cases (OS differences, missing Chinese IME, elevation mismatch).
pub fn environment_summary() -> String {
    let os = os_version();
    let layouts = layout_list();
    let integrity = integrity_level();
    format!("windows={os} layouts=[{layouts}] integrity={integrity}")
}

fn os_version() -> String {
    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: core::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // GetVersionExW is deprecated but still returns the real version when the
    // app manifest declares Win10/11 as supportedOS (Tauri's manifest does).
    if unsafe { GetVersionExW(&mut info) }.is_err() {
        return "unknown".to_owned();
    }
    format!(
        "{}.{}.{}",
        info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
    )
}

fn layout_list() -> String {
    let count = unsafe { GetKeyboardLayoutList(None) };
    if count <= 0 {
        return String::new();
    }
    let mut layouts = vec![HKL(std::ptr::null_mut()); count as usize];
    let copied = unsafe { GetKeyboardLayoutList(Some(&mut layouts)) };
    if copied <= 0 {
        return String::new();
    }
    layouts
        .iter()
        .take(copied as usize)
        .map(|handle| format!("0x{:X}", handle.0 as usize))
        .collect::<Vec<_>>()
        .join(",")
}

fn integrity_level() -> String {
    crate::platform::windows::integrity::current_process_level()
        .unwrap_or_else(|_| "unknown".to_owned())
}
