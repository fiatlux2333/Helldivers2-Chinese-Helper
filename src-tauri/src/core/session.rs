use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetIdentity {
    #[serde(with = "u64_string")]
    pub hwnd: u64,
    pub process_id: u32,
    pub thread_id: u32,
    #[serde(with = "u64_string")]
    pub process_creation_time: u64,
}

pub(crate) mod u64_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySnapshot {
    pub enter_down: bool,
    pub ctrl_down: bool,
    pub alt_down: bool,
    pub shift_down: bool,
    pub win_down: bool,
}

impl KeySnapshot {
    pub fn modifiers_down(self) -> bool {
        self.ctrl_down || self.alt_down || self.shift_down || self.win_down
    }

    pub fn all_released(self) -> bool {
        !self.enter_down && !self.modifiers_down()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionPhase {
    Idle,
    WaitingGameEnterRelease,
    OpeningPanel,
    Editing,
    WaitingSubmitKeyRelease,
    RestoringTarget,
    Injecting,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    #[serde(with = "u64_string")]
    pub generation: u64,
    pub phase: SessionPhase,
    pub target: Option<TargetIdentity>,
    pub draft: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnterSequence {
    NeedInitialRelease,
    Armed,
    Pressed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    StaleGeneration,
    InvalidPhase,
    TargetChanged,
    SubmitKeyStillDown,
    InjectionInProgress,
}

#[derive(Debug)]
pub struct SessionMachine {
    snapshot: SessionSnapshot,
    next_generation: u64,
    enter_sequence: EnterSequence,
}

impl Default for SessionMachine {
    fn default() -> Self {
        Self {
            snapshot: SessionSnapshot {
                generation: 0,
                phase: SessionPhase::Idle,
                target: None,
                draft: String::new(),
                last_error: None,
            },
            next_generation: 1,
            enter_sequence: EnterSequence::NeedInitialRelease,
        }
    }
}

impl SessionMachine {
    pub fn snapshot(&self) -> SessionSnapshot {
        self.snapshot.clone()
    }

    pub fn arm_game_enter(&mut self, target: TargetIdentity) -> u64 {
        let generation = self.allocate_generation();
        self.snapshot = SessionSnapshot {
            generation,
            phase: SessionPhase::WaitingGameEnterRelease,
            target: Some(target),
            draft: String::new(),
            last_error: None,
        };
        self.enter_sequence = EnterSequence::NeedInitialRelease;
        generation
    }

    pub fn observe_game_enter(
        &mut self,
        generation: u64,
        keys: KeySnapshot,
    ) -> Result<bool, SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.phase != SessionPhase::WaitingGameEnterRelease {
            return Err(SessionError::InvalidPhase);
        }

        if keys.modifiers_down() {
            self.enter_sequence = EnterSequence::NeedInitialRelease;
            return Ok(false);
        }

        match (self.enter_sequence, keys.enter_down) {
            (EnterSequence::NeedInitialRelease, false) => {
                self.enter_sequence = EnterSequence::Armed;
            }
            (EnterSequence::Armed, true) => {
                self.enter_sequence = EnterSequence::Pressed;
            }
            (EnterSequence::Pressed, false) => {
                self.snapshot.phase = SessionPhase::OpeningPanel;
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    pub fn begin_probe(&mut self, target: TargetIdentity) -> u64 {
        let generation = self.allocate_generation();
        self.snapshot = SessionSnapshot {
            generation,
            phase: SessionPhase::Editing,
            target: Some(target),
            draft: String::new(),
            last_error: None,
        };
        generation
    }

    pub fn panel_opened(&mut self, generation: u64) -> Result<(), SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.phase != SessionPhase::OpeningPanel {
            return Err(SessionError::InvalidPhase);
        }
        self.snapshot.phase = SessionPhase::Editing;
        Ok(())
    }

    pub fn request_submit(&mut self, generation: u64, draft: String) -> Result<(), SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.phase == SessionPhase::Injecting {
            return Err(SessionError::InjectionInProgress);
        }
        if self.snapshot.phase != SessionPhase::Editing {
            return Err(SessionError::InvalidPhase);
        }
        self.snapshot.draft = draft;
        self.snapshot.phase = SessionPhase::WaitingSubmitKeyRelease;
        Ok(())
    }

    pub fn observe_submit_key(
        &mut self,
        generation: u64,
        keys: KeySnapshot,
    ) -> Result<bool, SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.phase != SessionPhase::WaitingSubmitKeyRelease {
            return Err(SessionError::InvalidPhase);
        }
        if !keys.all_released() {
            return Ok(false);
        }
        self.snapshot.phase = SessionPhase::RestoringTarget;
        Ok(true)
    }

    pub fn begin_injection(
        &mut self,
        generation: u64,
        current_target: &TargetIdentity,
    ) -> Result<String, SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.phase == SessionPhase::Injecting {
            return Err(SessionError::InjectionInProgress);
        }
        if self.snapshot.phase == SessionPhase::WaitingSubmitKeyRelease {
            return Err(SessionError::SubmitKeyStillDown);
        }
        if self.snapshot.phase != SessionPhase::RestoringTarget {
            return Err(SessionError::InvalidPhase);
        }
        if self.snapshot.target.as_ref() != Some(current_target) {
            self.fail("目标窗口已变化");
            return Err(SessionError::TargetChanged);
        }

        self.snapshot.phase = SessionPhase::Injecting;
        Ok(self.snapshot.draft.clone())
    }

    pub fn complete_injection(&mut self, generation: u64) -> Result<(), SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.phase != SessionPhase::Injecting {
            return Err(SessionError::InvalidPhase);
        }
        // Keep the same locked target so the user can send multiple messages
        // without recapturing. Each inject still revalidates the foreground.
        self.snapshot.phase = SessionPhase::Editing;
        self.snapshot.draft.clear();
        self.snapshot.last_error = None;
        Ok(())
    }

    pub fn fail_injection(
        &mut self,
        generation: u64,
        message: impl Into<String>,
    ) -> Result<(), SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.phase != SessionPhase::Injecting {
            return Err(SessionError::InvalidPhase);
        }
        self.fail(message);
        Ok(())
    }

    pub fn invalidate_target(
        &mut self,
        generation: u64,
        current_target: &TargetIdentity,
    ) -> Result<(), SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.target.as_ref() != Some(current_target) {
            self.fail("目标窗口已变化");
            return Err(SessionError::TargetChanged);
        }
        Ok(())
    }

    pub fn fail_session(
        &mut self,
        generation: u64,
        message: impl Into<String>,
    ) -> Result<(), SessionError> {
        self.require_generation(generation)?;
        if self.snapshot.phase == SessionPhase::Idle {
            return Err(SessionError::InvalidPhase);
        }
        self.fail(message);
        Ok(())
    }

    pub fn cancel(&mut self) -> u64 {
        let generation = self.allocate_generation();
        self.snapshot = SessionSnapshot {
            generation,
            phase: SessionPhase::Idle,
            target: None,
            draft: String::new(),
            last_error: None,
        };
        generation
    }

    fn require_generation(&self, generation: u64) -> Result<(), SessionError> {
        if self.snapshot.generation == generation {
            Ok(())
        } else {
            Err(SessionError::StaleGeneration)
        }
    }

    fn allocate_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("session generation exhausted");
        generation
    }

    fn fail(&mut self, message: impl Into<String>) {
        self.snapshot.phase = SessionPhase::Failed;
        self.snapshot.last_error = Some(message.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(hwnd: u64) -> TargetIdentity {
        TargetIdentity {
            hwnd,
            process_id: 10,
            thread_id: 20,
            process_creation_time: 30,
        }
    }

    fn keys(enter_down: bool) -> KeySnapshot {
        KeySnapshot {
            enter_down,
            ctrl_down: false,
            alt_down: false,
            shift_down: false,
            win_down: false,
        }
    }

    #[test]
    fn serializes_large_identity_values_as_strings() {
        let identity = TargetIdentity {
            hwnd: u64::MAX,
            process_id: 10,
            thread_id: 20,
            process_creation_time: u64::MAX - 1,
        };

        let json = serde_json::to_value(&identity).unwrap();
        assert_eq!(json["hwnd"], u64::MAX.to_string());
        assert_eq!(json["processCreationTime"], (u64::MAX - 1).to_string());
        assert_eq!(
            serde_json::from_value::<TargetIdentity>(json).unwrap(),
            identity
        );
    }

    #[test]
    fn enter_requires_stable_up_down_up_sequence() {
        let mut machine = SessionMachine::default();
        let generation = machine.arm_game_enter(target(1));

        assert!(!machine.observe_game_enter(generation, keys(true)).unwrap());
        assert!(!machine.observe_game_enter(generation, keys(true)).unwrap());
        assert!(!machine.observe_game_enter(generation, keys(false)).unwrap());
        assert!(!machine.observe_game_enter(generation, keys(true)).unwrap());
        assert!(!machine.observe_game_enter(generation, keys(true)).unwrap());
        assert!(machine.observe_game_enter(generation, keys(false)).unwrap());
        assert_eq!(machine.snapshot().phase, SessionPhase::OpeningPanel);
    }

    #[test]
    fn modified_enter_does_not_trigger() {
        let mut machine = SessionMachine::default();
        let generation = machine.arm_game_enter(target(1));
        machine.observe_game_enter(generation, keys(false)).unwrap();
        let mut modified = keys(true);
        modified.ctrl_down = true;
        assert!(!machine.observe_game_enter(generation, modified).unwrap());
        assert!(!machine.observe_game_enter(generation, keys(false)).unwrap());
        assert_eq!(
            machine.snapshot().phase,
            SessionPhase::WaitingGameEnterRelease
        );
    }

    #[test]
    fn modified_enter_requires_a_fresh_unmodified_press() {
        let mut machine = SessionMachine::default();
        let generation = machine.arm_game_enter(target(1));
        machine.observe_game_enter(generation, keys(false)).unwrap();

        let mut modified_down = keys(true);
        modified_down.ctrl_down = true;
        assert!(
            !machine
                .observe_game_enter(generation, modified_down)
                .unwrap()
        );
        assert!(!machine.observe_game_enter(generation, keys(true)).unwrap());
        assert!(!machine.observe_game_enter(generation, keys(false)).unwrap());
        assert!(!machine.observe_game_enter(generation, keys(true)).unwrap());
        assert!(machine.observe_game_enter(generation, keys(false)).unwrap());
    }

    #[test]
    fn stale_generation_cannot_mutate_new_session() {
        let mut machine = SessionMachine::default();
        let old = machine.begin_probe(target(1));
        let current = machine.begin_probe(target(2));
        assert_ne!(old, current);
        assert_eq!(
            machine.request_submit(old, "迟到文本".to_owned()),
            Err(SessionError::StaleGeneration)
        );
        assert_eq!(machine.snapshot().target, Some(target(2)));
    }

    #[test]
    fn target_change_invalidates_session() {
        let mut machine = SessionMachine::default();
        let generation = machine.begin_probe(target(1));
        assert_eq!(
            machine.invalidate_target(generation, &target(2)),
            Err(SessionError::TargetChanged)
        );
        assert_eq!(machine.snapshot().phase, SessionPhase::Failed);
    }

    #[test]
    fn submit_release_gates_injection_and_transaction_is_single() {
        let mut machine = SessionMachine::default();
        let target = target(1);
        let generation = machine.begin_probe(target.clone());
        machine
            .request_submit(generation, "草稿".to_owned())
            .unwrap();

        assert!(!machine.observe_submit_key(generation, keys(true)).unwrap());
        assert_eq!(
            machine.begin_injection(generation, &target),
            Err(SessionError::SubmitKeyStillDown)
        );
        assert!(machine.observe_submit_key(generation, keys(false)).unwrap());
        assert_eq!(
            machine.begin_injection(generation, &target).unwrap(),
            "草稿"
        );
        assert_eq!(
            machine.begin_injection(generation, &target),
            Err(SessionError::InjectionInProgress)
        );
    }

    #[test]
    fn failure_preserves_complete_draft_without_retry_transition() {
        let mut machine = SessionMachine::default();
        let target = target(1);
        let generation = machine.begin_probe(target.clone());
        machine
            .request_submit(generation, "完整草稿🙂".to_owned())
            .unwrap();
        machine.observe_submit_key(generation, keys(false)).unwrap();
        machine.begin_injection(generation, &target).unwrap();
        machine.fail_injection(generation, "部分发送失败").unwrap();

        let snapshot = machine.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::Failed);
        assert_eq!(snapshot.draft, "完整草稿🙂");
        assert_eq!(snapshot.last_error.as_deref(), Some("部分发送失败"));
    }

    #[test]
    fn successful_injection_keeps_target_for_next_message() {
        let mut machine = SessionMachine::default();
        let target = target(1);
        let generation = machine.begin_probe(target.clone());

        machine
            .request_submit(generation, "第一句".to_owned())
            .unwrap();
        machine.observe_submit_key(generation, keys(false)).unwrap();
        machine.begin_injection(generation, &target).unwrap();
        machine.complete_injection(generation).unwrap();

        let after_first = machine.snapshot();
        assert_eq!(after_first.phase, SessionPhase::Editing);
        assert_eq!(after_first.target, Some(target.clone()));
        assert!(after_first.draft.is_empty());
        assert_eq!(after_first.generation, generation);

        machine
            .request_submit(generation, "第二句".to_owned())
            .unwrap();
        machine.observe_submit_key(generation, keys(false)).unwrap();
        assert_eq!(
            machine.begin_injection(generation, &target).unwrap(),
            "第二句"
        );
        machine.complete_injection(generation).unwrap();
        assert_eq!(machine.snapshot().phase, SessionPhase::Editing);
        assert_eq!(machine.snapshot().target, Some(target));
    }
}
