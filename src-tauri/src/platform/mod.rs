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
    pub successful_events: u32,
    pub failed_batch_index: Option<usize>,
    pub partial_prefix_possible: bool,
    pub key_state_uncertain: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityDiagnostic {
    pub supported: bool,
    pub current_level: Option<String>,
    pub target_level: Option<String>,
    pub compatible: Option<bool>,
}
