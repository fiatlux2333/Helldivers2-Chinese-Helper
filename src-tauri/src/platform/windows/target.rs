use crate::{core::session::TargetIdentity, platform::TargetDiagnostic};
use std::{ffi::c_void, mem::size_of};
#[cfg(debug_assertions)]
use windows::Win32::Foundation::{SetLastError, WIN32_ERROR};
#[cfg(debug_assertions)]
use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
use windows::Win32::{
    Foundation::{CloseHandle, FILETIME, GetLastError, HWND, LPARAM},
    Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    },
    UI::WindowsAndMessaging::{
        EnumThreadWindows, GA_ROOT, GetAncestor, GetClassNameW, GetForegroundWindow,
        GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
        IsWindowVisible, SetForegroundWindow,
    },
};

/// Known HD2 executable basenames used only as a last-resort owner fallback
/// when Win32 window-to-PID APIs are blocked for stingray_window.
const HD2_PROCESS_NAMES: &[&str] = &["helldivers2.exe"];
const HD2_WINDOW_CLASS: &str = "stingray_window";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetError {
    NoForegroundWindow,
    WindowTitleUnavailable,
    WindowStateUnavailable,
    ProcessUnavailable(u32),
    ProcessTimeUnavailable(u32),
}

pub fn foreground_diagnostic(title_keyword: &str) -> Result<TargetDiagnostic, TargetError> {
    for attempt in 0..=4 {
        let foreground_hwnd = unsafe { GetForegroundWindow() };
        let hwnd = if foreground_hwnd.0.is_null() {
            foreground_hwnd
        } else {
            let root = unsafe { GetAncestor(foreground_hwnd, GA_ROOT) };
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=foreground_handles raw=0x{:X} raw_is_window={} root=0x{:X} root_is_window={}",
                foreground_hwnd.0 as usize,
                unsafe { IsWindow(Some(foreground_hwnd)).as_bool() },
                root.0 as usize,
                unsafe { IsWindow(Some(root)).as_bool() },
            );
            if root.0.is_null() {
                foreground_hwnd
            } else {
                root
            }
        };
        match hwnd.0.is_null() {
            true => {
                #[cfg(debug_assertions)]
                eprintln!("[hd2cn][target] stage=foreground hwnd=null attempt={attempt}");
                return Err(TargetError::NoForegroundWindow);
            }
            false => match foreground_diagnostic_once(hwnd, title_keyword) {
                Ok(diagnostic) => return Ok(diagnostic),
                Err(TargetError::ProcessUnavailable(0)) if attempt < 4 => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[hd2cn][target] stage=identity_retry hwnd=0x{:X} attempt={} delay_ms=15",
                        hwnd.0 as usize,
                        attempt + 1
                    );
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
                Err(error) => {
                    #[cfg(debug_assertions)]
                    if matches!(error, TargetError::ProcessUnavailable(0)) {
                        log_title_keyword_windows(title_keyword, hwnd);
                    }
                    return Err(error);
                }
            },
        }
    }

    unreachable!("foreground diagnostic retry loop must return")
}

fn foreground_diagnostic_once(
    hwnd: HWND,
    title_keyword: &str,
) -> Result<TargetDiagnostic, TargetError> {
    let identity = match identity_for(hwnd) {
        Ok(identity) => identity,
        Err(error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=identity hwnd=0x{:X} error={error:?}",
                hwnd.0 as usize
            );
            return Err(error);
        }
    };
    let title = match window_title(hwnd) {
        Ok(title) => title,
        Err(error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=window_title hwnd=0x{:X} error={error:?}",
                hwnd.0 as usize
            );
            return Err(error);
        }
    };
    let cloaked = match is_cloaked(hwnd) {
        Ok(cloaked) => cloaked,
        Err(error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=window_state hwnd=0x{:X} error={error:?}",
                hwnd.0 as usize
            );
            return Err(error);
        }
    };
    #[cfg(debug_assertions)]
    eprintln!(
        "[hd2cn][target] stage=diagnostic hwnd=0x{:X} pid={} thread={} title_len={} title_match={} visible={} minimized={} cloaked={}",
        hwnd.0 as usize,
        identity.process_id,
        identity.thread_id,
        title.chars().count(),
        title
            .to_uppercase()
            .contains(&title_keyword.trim().to_uppercase()),
        unsafe { IsWindowVisible(hwnd).as_bool() },
        unsafe { IsIconic(hwnd).as_bool() },
        cloaked
    );

    Ok(TargetDiagnostic {
        supported: true,
        identity: Some(identity),
        title_matches: title
            .to_uppercase()
            .contains(&title_keyword.trim().to_uppercase()),
        title,
        is_window: unsafe { IsWindow(Some(hwnd)).as_bool() },
        visible: unsafe { IsWindowVisible(hwnd).as_bool() },
        minimized: unsafe { IsIconic(hwnd).as_bool() },
        cloaked,
    })
}

pub fn identity_for(hwnd: HWND) -> Result<TargetIdentity, TargetError> {
    let (process_id, thread_id, source) = resolve_window_owner(hwnd)?;
    #[cfg(debug_assertions)]
    eprintln!(
        "[hd2cn][target] stage=window_owner hwnd=0x{:X} pid={} thread={} source={}",
        hwnd.0 as usize, process_id, thread_id, source
    );

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
        .map_err(|error| {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=open_process hwnd=0x{:X} pid={} thread={} win32_error={}",
                hwnd.0 as usize,
                process_id,
                thread_id,
                error.code().0
            );
            TargetError::ProcessUnavailable(error.code().0 as u32)
        })?;
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let result =
        unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
    unsafe {
        let _ = CloseHandle(process);
    }
    result.map_err(|error| TargetError::ProcessTimeUnavailable(error.code().0 as u32))?;

    Ok(TargetIdentity {
        hwnd: hwnd.0 as usize as u64,
        process_id,
        thread_id,
        process_creation_time: (u64::from(creation.dwHighDateTime) << 32)
            | u64::from(creation.dwLowDateTime),
    })
}

fn resolve_window_owner(hwnd: HWND) -> Result<(u32, u32, &'static str), TargetError> {
    let mut process_id = 0u32;
    let process_id_ptr = &mut process_id as *mut u32;
    #[cfg(debug_assertions)]
    unsafe {
        SetLastError(WIN32_ERROR(0));
    }
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(process_id_ptr)) };
    #[cfg(debug_assertions)]
    {
        let last_error = unsafe { GetLastError().0 };
        eprintln!(
            "[hd2cn][target] stage=window_identity hwnd=0x{:X} is_window={} pid={} thread={} last_error={} class={} title={}",
            hwnd.0 as usize,
            unsafe { IsWindow(Some(hwnd)).as_bool() },
            process_id,
            thread_id,
            last_error,
            window_class_debug(hwnd),
            window_title_debug(hwnd)
        );
    }
    if process_id != 0 && thread_id != 0 {
        return Ok((process_id, thread_id, "GetWindowThreadProcessId"));
    }

    // HD2 (stingray_window) can keep a valid HWND/title while
    // GetWindowThreadProcessId returns zeros. Prefer reverse owner lookup via
    // thread snapshot + EnumThreadWindows. If that is also blocked, fall back
    // to a unique helldivers2.exe process match gated by class/title.
    // Still requires OpenProcess + creation time afterward.
    match resolve_owner_by_thread_snapshot(hwnd) {
        Ok((process_id, thread_id)) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=identity_fallback hwnd=0x{:X} pid={} thread={} method=thread_snapshot",
                hwnd.0 as usize, process_id, thread_id
            );
            return Ok((process_id, thread_id, "thread_snapshot"));
        }
        Err(error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=identity_fallback hwnd=0x{:X} method=thread_snapshot error={error:?}",
                hwnd.0 as usize
            );
        }
    }

    match resolve_owner_by_process_name(hwnd) {
        Ok((process_id, thread_id)) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=identity_fallback hwnd=0x{:X} pid={} thread={} method=process_name",
                hwnd.0 as usize, process_id, thread_id
            );
            Ok((process_id, thread_id, "process_name"))
        }
        Err(error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=identity_fallback hwnd=0x{:X} method=process_name error={error:?}",
                hwnd.0 as usize
            );
            Err(error)
        }
    }
}

fn resolve_owner_by_thread_snapshot(hwnd: HWND) -> Result<(u32, u32), TargetError> {
    if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
        return Err(TargetError::ProcessUnavailable(0));
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.map_err(|error| {
        #[cfg(debug_assertions)]
        eprintln!(
            "[hd2cn][target] stage=thread_snapshot hwnd=0x{:X} create_error={}",
            hwnd.0 as usize,
            error.code().0
        );
        TargetError::ProcessUnavailable(error.code().0 as u32)
    })?;

    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };

    if unsafe { Thread32First(snapshot, &mut entry) }.is_err() {
        let code = unsafe { GetLastError().0 };
        unsafe {
            let _ = CloseHandle(snapshot);
        }
        #[cfg(debug_assertions)]
        eprintln!(
            "[hd2cn][target] stage=thread_snapshot hwnd=0x{:X} first_error={}",
            hwnd.0 as usize, code
        );
        return Err(TargetError::ProcessUnavailable(code));
    }

    loop {
        if thread_owns_window(entry.th32ThreadID, hwnd) {
            let process_id = entry.th32OwnerProcessID;
            let thread_id = entry.th32ThreadID;
            unsafe {
                let _ = CloseHandle(snapshot);
            }
            if process_id == 0 || thread_id == 0 {
                return Err(TargetError::ProcessUnavailable(0));
            }
            return Ok((process_id, thread_id));
        }

        if unsafe { Thread32Next(snapshot, &mut entry) }.is_err() {
            break;
        }
    }

    unsafe {
        let _ = CloseHandle(snapshot);
    }
    Err(TargetError::ProcessUnavailable(0))
}

fn thread_owns_window(thread_id: u32, target: HWND) -> bool {
    struct FindState {
        target: HWND,
        found: bool,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        // SAFETY: lparam points to FindState for the EnumThreadWindows call.
        let state = unsafe { &mut *(lparam.0 as *mut FindState) };
        if hwnd.0 == state.target.0 {
            state.found = true;
            return false.into();
        }
        true.into()
    }

    let mut state = FindState {
        target,
        found: false,
    };
    let _ = unsafe {
        EnumThreadWindows(
            thread_id,
            Some(enum_proc),
            LPARAM(&mut state as *mut FindState as isize),
        )
    };
    state.found
}

/// Last-resort HD2 owner fallback.
///
/// Only accepts a unique `helldivers2.exe` process when the foreground window
/// class/title already look like HD2. Never used for arbitrary windows.
fn resolve_owner_by_process_name(hwnd: HWND) -> Result<(u32, u32), TargetError> {
    if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
        return Err(TargetError::ProcessUnavailable(0));
    }

    let class = window_class_name(hwnd).unwrap_or_default();
    let title = window_title(hwnd).unwrap_or_default();
    let title_upper = title.to_uppercase();
    let looks_like_hd2 =
        class.eq_ignore_ascii_case(HD2_WINDOW_CLASS) || title_upper.contains("HELLDIVERS");
    if !looks_like_hd2 {
        #[cfg(debug_assertions)]
        eprintln!(
            "[hd2cn][target] stage=process_name_skip hwnd=0x{:X} class={} title={} reason=not_hd2_window",
            hwnd.0 as usize, class, title
        );
        return Err(TargetError::ProcessUnavailable(0));
    }

    let matches = list_process_ids_by_names(HD2_PROCESS_NAMES)?;
    #[cfg(debug_assertions)]
    eprintln!(
        "[hd2cn][target] stage=process_name_scan hwnd=0x{:X} class={} title={} match_count={} pids={:?}",
        hwnd.0 as usize,
        class,
        title,
        matches.len(),
        matches
    );

    match matches.as_slice() {
        [process_id] if *process_id != 0 => {
            // Prefer a thread that actually owns this HWND within the matched
            // process. If EnumThreadWindows is blocked, keep process_id and a
            // fixed thread_id=0 so identity equality stays stable across captures.
            let thread_id = thread_id_owning_window_in_process(*process_id, hwnd).unwrap_or(0);
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=process_name_bind hwnd=0x{:X} pid={} thread={}",
                hwnd.0 as usize, process_id, thread_id
            );
            Ok((*process_id, thread_id))
        }
        [] => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=process_name_scan hwnd=0x{:X} reason=no_helldivers2_process",
                hwnd.0 as usize
            );
            Err(TargetError::ProcessUnavailable(0))
        }
        _ => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][target] stage=process_name_scan hwnd=0x{:X} reason=ambiguous_process_count count={}",
                hwnd.0 as usize,
                matches.len()
            );
            Err(TargetError::ProcessUnavailable(0))
        }
    }
}

fn list_process_ids_by_names(names: &[&str]) -> Result<Vec<u32>, TargetError> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.map_err(|error| {
        #[cfg(debug_assertions)]
        eprintln!(
            "[hd2cn][target] stage=process_snapshot create_error={}",
            error.code().0
        );
        TargetError::ProcessUnavailable(error.code().0 as u32)
    })?;

    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut matches = Vec::new();

    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_err() {
        let code = unsafe { GetLastError().0 };
        unsafe {
            let _ = CloseHandle(snapshot);
        }
        return Err(TargetError::ProcessUnavailable(code));
    }

    loop {
        let exe = process_entry_name(&entry);
        if names.iter().any(|name| exe.eq_ignore_ascii_case(name)) && entry.th32ProcessID != 0 {
            matches.push(entry.th32ProcessID);
        }
        if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
            break;
        }
    }

    unsafe {
        let _ = CloseHandle(snapshot);
    }
    Ok(matches)
}

fn process_entry_name(entry: &PROCESSENTRY32W) -> String {
    let len = entry
        .szExeFile
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(entry.szExeFile.len());
    String::from_utf16_lossy(&entry.szExeFile[..len])
}

fn thread_id_owning_window_in_process(process_id: u32, hwnd: HWND) -> Option<u32> {
    for_each_process_thread(process_id, |thread_id| {
        if thread_owns_window(thread_id, hwnd) {
            Some(thread_id)
        } else {
            None
        }
    })
}

fn for_each_process_thread<F>(process_id: u32, mut on_thread: F) -> Option<u32>
where
    F: FnMut(u32) -> Option<u32>,
{
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.ok()?;
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };

    if unsafe { Thread32First(snapshot, &mut entry) }.is_err() {
        unsafe {
            let _ = CloseHandle(snapshot);
        }
        return None;
    }

    loop {
        if entry.th32OwnerProcessID == process_id && entry.th32ThreadID != 0 {
            if let Some(found) = on_thread(entry.th32ThreadID) {
                unsafe {
                    let _ = CloseHandle(snapshot);
                }
                return Some(found);
            }
        }
        if unsafe { Thread32Next(snapshot, &mut entry) }.is_err() {
            break;
        }
    }

    unsafe {
        let _ = CloseHandle(snapshot);
    }
    None
}

fn window_class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 256];
    let copied = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if copied == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..copied as usize]))
}

pub fn validate_foreground(
    identity: &TargetIdentity,
    title_keyword: &str,
) -> Result<bool, TargetError> {
    let diagnostic = foreground_diagnostic(title_keyword)?;
    Ok(diagnostic.identity.as_ref() == Some(identity)
        && diagnostic.title_matches
        && diagnostic.is_window
        && diagnostic.visible
        && !diagnostic.minimized
        && !diagnostic.cloaked)
}

pub fn restore_foreground(
    identity: &TargetIdentity,
    title_keyword: &str,
) -> Result<bool, TargetError> {
    if validate_foreground(identity, title_keyword) == Ok(true) {
        return Ok(true);
    }

    let hwnd = HWND(identity.hwnd as usize as *mut c_void);
    if identity_for(hwnd)? != *identity || !window_is_available(hwnd, title_keyword)? {
        return Ok(false);
    }

    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
    Ok(true)
}

fn window_is_available(hwnd: HWND, title_keyword: &str) -> Result<bool, TargetError> {
    let title = window_title(hwnd)?;
    Ok(title
        .to_uppercase()
        .contains(&title_keyword.trim().to_uppercase())
        && unsafe { IsWindow(Some(hwnd)).as_bool() }
        && unsafe { IsWindowVisible(hwnd).as_bool() }
        && !unsafe { IsIconic(hwnd).as_bool() }
        && !is_cloaked(hwnd)?)
}

fn window_title(hwnd: HWND) -> Result<String, TargetError> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length == 0 {
        return Err(TargetError::WindowTitleUnavailable);
    }

    let mut buffer = vec![0u16; length as usize + 2];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if copied == 0 {
        return Err(TargetError::WindowTitleUnavailable);
    }
    buffer.truncate(copied as usize);
    Ok(String::from_utf16_lossy(&buffer))
}

#[cfg(debug_assertions)]
fn window_title_debug(hwnd: HWND) -> String {
    window_title(hwnd).unwrap_or_else(|_| "<unavailable>".to_string())
}

#[cfg(debug_assertions)]
fn window_class_debug(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let copied = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if copied == 0 {
        return format!("<unavailable last_error={}>", unsafe { GetLastError().0 });
    }
    String::from_utf16_lossy(&buffer[..copied as usize])
}

#[cfg(debug_assertions)]
fn log_title_keyword_windows(title_keyword: &str, foreground_hwnd: HWND) {
    let keyword = title_keyword.trim().to_uppercase();
    if keyword.is_empty() {
        return;
    }

    struct EnumState {
        keyword: String,
        foreground: usize,
        count: usize,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        // SAFETY: lparam points to EnumState owned by log_title_keyword_windows
        // for the duration of EnumWindows.
        let state = unsafe { &mut *(lparam.0 as *mut EnumState) };
        if state.count >= 8 {
            return false.into();
        }

        let title = window_title(hwnd).unwrap_or_default();
        if !title.to_uppercase().contains(&state.keyword) {
            return true.into();
        }

        let mut process_id = 0u32;
        let process_id_ptr = &mut process_id as *mut u32;
        let (thread_id, last_error, is_window, visible, minimized) = unsafe {
            SetLastError(WIN32_ERROR(0));
            let thread_id = GetWindowThreadProcessId(hwnd, Some(process_id_ptr));
            let last_error = GetLastError().0;
            (
                thread_id,
                last_error,
                IsWindow(Some(hwnd)).as_bool(),
                IsWindowVisible(hwnd).as_bool(),
                IsIconic(hwnd).as_bool(),
            )
        };
        state.count += 1;
        eprintln!(
            "[hd2cn][target] stage=enum_match index={} hwnd=0x{:X} foreground_same={} is_window={} visible={} minimized={} pid={} thread={} last_error={} class={} title={}",
            state.count,
            hwnd.0 as usize,
            hwnd.0 as usize == state.foreground,
            is_window,
            visible,
            minimized,
            process_id,
            thread_id,
            last_error,
            window_class_debug(hwnd),
            title
        );
        true.into()
    }

    let mut state = EnumState {
        keyword,
        foreground: foreground_hwnd.0 as usize,
        count: 0,
    };
    let _ = unsafe {
        EnumWindows(
            Some(enum_proc),
            LPARAM(&mut state as *mut EnumState as isize),
        )
    };
    eprintln!(
        "[hd2cn][target] stage=enum_summary keyword_matches={} foreground=0x{:X}",
        state.count, state.foreground
    );
}

fn is_cloaked(hwnd: HWND) -> Result<bool, TargetError> {
    let mut cloaked = 0u32;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            size_of::<u32>() as u32,
        )
    }
    .map_err(|_| TargetError::WindowStateUnavailable)?;
    Ok(cloaked != 0)
}
