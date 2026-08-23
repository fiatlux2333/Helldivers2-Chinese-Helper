use crate::core::session::TargetIdentity;
use serde::{Deserialize, Serialize};

#[cfg(windows)]
pub mod windows;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetDiagnostic {
    pub supported: bool,
    pub identity: Option<TargetIdentity>,
    pub title: String,
    pub title_matches: bool,
    pub is_window: bool,
    pub visible: bool,
    pub minimized: bool,
    pub cloaked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectionReport {
    pub attempted_batches: usize,
    pub requested_events: u32,
    pub successful_events: u32,
    pub text_requested_events: u32,
    pub text_successful_events: u32,
    pub last_error_code: Option<u32>,
    pub delivery_transport: String,
    pub delivery_acknowledged: bool,
    pub input_characters: usize,
    pub input_delay_ms: u64,
    pub keyboard_layout_switched: bool,
    pub keyboard_layout_before: Option<u32>,
    pub keyboard_layout_requested: Option<u32>,
    pub keyboard_layout_restored: Option<bool>,
    pub num_lock_toggled: bool,
    pub num_lock_restored: Option<bool>,
    pub failed_batch_index: Option<usize>,
    pub partial_prefix_possible: bool,
    pub key_state_uncertain: bool,
    pub submit_attempted: bool,
    pub submit_completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityDiagnostic {
    pub supported: bool,
    pub current_level: Option<String>,
    pub target_level: Option<String>,
    pub compatible: Option<bool>,
}

/// One-line system environment summary for diagnostic exports. Fan-reported
/// bugs often depend on the Windows build, installed keyboard layouts, or
/// elevation mismatch between the game and this assistant, none of which were
/// visible in exported logs before.
#[cfg(windows)]
pub fn diagnostic_environment_summary() -> String {
    windows::environment_summary()
}

#[cfg(not(windows))]
pub fn diagnostic_environment_summary() -> String {
    "unsupported_platform".to_owned()
}
