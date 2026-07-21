use crate::platform::IntegrityDiagnostic;
use std::{ffi::c_void, mem::size_of};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, IsValidSid,
        TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel,
    },
    System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityError {
    ProcessUnavailable,
    TokenUnavailable,
    InformationUnavailable,
    InvalidSid,
}

pub fn compare_with_target(process_id: u32) -> Result<IntegrityDiagnostic, IntegrityError> {
    let current = unsafe { integrity_for_process(GetCurrentProcess()) }?;
    let target_process = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
            .map_err(|_| IntegrityError::ProcessUnavailable)?
    };
    let target = unsafe { integrity_for_process(target_process) };
    unsafe {
        let _ = CloseHandle(target_process);
    }
    let target = target?;

    Ok(IntegrityDiagnostic {
        supported: true,
        current_level: Some(level_name(current).to_owned()),
        target_level: Some(level_name(target).to_owned()),
        compatible: Some(current >= target),
    })
}

unsafe fn integrity_for_process(process: HANDLE) -> Result<u32, IntegrityError> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }
        .map_err(|_| IntegrityError::TokenUnavailable)?;

    let result = unsafe { integrity_for_token(token) };
    unsafe {
        let _ = CloseHandle(token);
    }
    result
}

unsafe fn integrity_for_token(token: HANDLE) -> Result<u32, IntegrityError> {
    let mut required = 0u32;
    let _ = unsafe { GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut required) };
    if required < size_of::<TOKEN_MANDATORY_LABEL>() as u32 {
        return Err(IntegrityError::InformationUnavailable);
    }

    let word_size = size_of::<usize>();
    let word_count = (required as usize).div_ceil(word_size);
    let mut buffer = vec![0usize; word_count];
    unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr() as *mut c_void),
            required,
            &mut required,
        )
    }
    .map_err(|_| IntegrityError::InformationUnavailable)?;

    let label = unsafe { &*(buffer.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
    let sid = label.Label.Sid;
    if sid.is_invalid() || !unsafe { IsValidSid(sid).as_bool() } {
        return Err(IntegrityError::InvalidSid);
    }

    let count_ptr = unsafe { GetSidSubAuthorityCount(sid) };
    if count_ptr.is_null() {
        return Err(IntegrityError::InvalidSid);
    }
    let count = unsafe { *count_ptr };
    if count == 0 {
        return Err(IntegrityError::InvalidSid);
    }

    let rid_ptr = unsafe { GetSidSubAuthority(sid, u32::from(count - 1)) };
    if rid_ptr.is_null() {
        return Err(IntegrityError::InvalidSid);
    }
    Ok(unsafe { *rid_ptr })
}

fn level_name(rid: u32) -> &'static str {
    match rid {
        0x0000..=0x0fff => "untrusted",
        0x1000..=0x1fff => "low",
        0x2000..=0x2fff => "medium",
        0x3000..=0x3fff => "high",
        0x4000..=0x4fff => "system",
        _ => "protected",
    }
}
