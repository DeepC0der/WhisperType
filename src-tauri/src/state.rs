use serde::{Deserialize, Serialize};
use std::sync::RwLock;

/// Application lifecycle state machine for Murmur dictation & transcription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum AppState {
    #[default]
    IDLE,
    RECORDING,
    TRANSCRIBING,
    INSERTING,
    DONE,
    ERROR,
}

/// Snapshot of the state machine state for serialization over Tauri IPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateSnapshot {
    pub current_state: AppState,
    pub previous_state: Option<AppState>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct StateInner {
    current: AppState,
    previous: Option<AppState>,
    last_error: Option<String>,
}

impl Default for StateInner {
    fn default() -> Self {
        Self {
            current: AppState::IDLE,
            previous: None,
            last_error: None,
        }
    }
}

/// Thread-safe state manager for the Murmur Tauri backend.
/// Uses a single `RwLock` around the inner state to guarantee atomic snapshots and prevent torn reads.
pub struct AppStateManager {
    inner: RwLock<StateInner>,
}

impl Default for AppStateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AppStateManager {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(StateInner::default()),
        }
    }

    /// Read the current application state.
    pub fn get_state(&self) -> AppState {
        self.inner.read().expect("Lock poisoned").current
    }

    /// Retrieve an atomic snapshot of current state, previous state, and any error message.
    pub fn get_snapshot(&self) -> StateSnapshot {
        let guard = self.inner.read().expect("Lock poisoned");
        StateSnapshot {
            current_state: guard.current,
            previous_state: guard.previous,
            error_message: guard.last_error.clone(),
        }
    }

    /// Attempt a validated transition from the current state to the requested next state.
    pub fn transition_to(&self, next: AppState) -> Result<AppState, String> {
        let mut guard = self.inner.write().expect("Lock poisoned");

        let cur = guard.current;
        let is_valid = match (cur, next) {
            // Can transition to ERROR from any state
            (_, AppState::ERROR) => true,
            // Can transition to IDLE from ERROR, DONE, INSERTING, RECORDING (cancel), or TRANSCRIBING (silence / abort)
            (AppState::ERROR, AppState::IDLE) => true,
            (AppState::DONE, AppState::IDLE) => true,
            (AppState::INSERTING, AppState::IDLE) => true,
            (AppState::RECORDING, AppState::IDLE) => true,
            (AppState::TRANSCRIBING, AppState::IDLE) => true,
            // Normal forward flow
            (AppState::IDLE, AppState::RECORDING) => true,
            (AppState::RECORDING, AppState::TRANSCRIBING) => true,
            (AppState::TRANSCRIBING, AppState::INSERTING) => true,
            (AppState::TRANSCRIBING, AppState::DONE) => true, // When auto-insert is disabled
            (AppState::INSERTING, AppState::DONE) => true,
            // Idempotent re-entry
            (a, b) if a == b => true,
            _ => false,
        };

        if !is_valid {
            return Err(format!(
                "Invalid state transition from {:?} to {:?}",
                cur, next
            ));
        }

        guard.previous = Some(cur);
        guard.current = next;
        if next != AppState::ERROR {
            guard.last_error = None;
        }

        Ok(guard.current)
    }

    /// Transition to ERROR state with an explanatory message.
    pub fn set_error(&self, err_msg: String) {
        let mut guard = self.inner.write().expect("Lock poisoned");
        guard.previous = Some(guard.current);
        guard.current = AppState::ERROR;
        guard.last_error = Some(err_msg);
    }

    /// Reset back to IDLE state and clear any active error.
    pub fn reset_to_idle(&self) {
        let mut guard = self.inner.write().expect("Lock poisoned");
        guard.previous = Some(guard.current);
        guard.current = AppState::IDLE;
        guard.last_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_initial_state_is_idle() {
        let sm = AppStateManager::new();
        assert_eq!(sm.get_state(), AppState::IDLE);
        let snapshot = sm.get_snapshot();
        assert_eq!(snapshot.current_state, AppState::IDLE);
        assert_eq!(snapshot.previous_state, None);
        assert_eq!(snapshot.error_message, None);
    }

    #[test]
    fn test_valid_transitions() {
        let sm = AppStateManager::new();
        // IDLE -> RECORDING
        assert!(sm.transition_to(AppState::RECORDING).is_ok());
        assert_eq!(sm.get_state(), AppState::RECORDING);

        // RECORDING -> TRANSCRIBING
        assert!(sm.transition_to(AppState::TRANSCRIBING).is_ok());
        assert_eq!(sm.get_state(), AppState::TRANSCRIBING);

        // TRANSCRIBING -> INSERTING
        assert!(sm.transition_to(AppState::INSERTING).is_ok());
        assert_eq!(sm.get_state(), AppState::INSERTING);

        // INSERTING -> DONE
        assert!(sm.transition_to(AppState::DONE).is_ok());
        assert_eq!(sm.get_state(), AppState::DONE);

        // DONE -> IDLE
        assert!(sm.transition_to(AppState::IDLE).is_ok());
        assert_eq!(sm.get_state(), AppState::IDLE);
    }

    #[test]
    fn test_done_direct_from_transcribing() {
        let sm = AppStateManager::new();
        // IDLE -> RECORDING -> TRANSCRIBING -> DONE -> IDLE (when auto_insert is false)
        assert!(sm.transition_to(AppState::RECORDING).is_ok());
        assert!(sm.transition_to(AppState::TRANSCRIBING).is_ok());
        assert!(sm.transition_to(AppState::DONE).is_ok());
        assert_eq!(sm.get_state(), AppState::DONE);
        assert!(sm.transition_to(AppState::IDLE).is_ok());
        assert_eq!(sm.get_state(), AppState::IDLE);
    }

    #[test]
    fn test_cancellation_transitions() {
        let sm = AppStateManager::new();
        // RECORDING -> IDLE (cancel)
        assert!(sm.transition_to(AppState::RECORDING).is_ok());
        assert!(sm.transition_to(AppState::IDLE).is_ok());
        assert_eq!(sm.get_state(), AppState::IDLE);

        // TRANSCRIBING -> IDLE (silence / empty result / cancel)
        assert!(sm.transition_to(AppState::RECORDING).is_ok());
        assert!(sm.transition_to(AppState::TRANSCRIBING).is_ok());
        assert!(sm.transition_to(AppState::IDLE).is_ok());
        assert_eq!(sm.get_state(), AppState::IDLE);
    }

    #[test]
    fn test_invalid_transitions() {
        let sm = AppStateManager::new();
        // IDLE -> INSERTING (invalid)
        assert!(sm.transition_to(AppState::INSERTING).is_err());
        assert_eq!(sm.get_state(), AppState::IDLE);

        // IDLE -> TRANSCRIBING (invalid)
        assert!(sm.transition_to(AppState::TRANSCRIBING).is_err());
        assert_eq!(sm.get_state(), AppState::IDLE);

        // RECORDING -> INSERTING (invalid: must transcribe first)
        assert!(sm.transition_to(AppState::RECORDING).is_ok());
        assert!(sm.transition_to(AppState::INSERTING).is_err());

        // DONE -> RECORDING / INSERTING (invalid: must reset to IDLE first)
        assert!(sm.transition_to(AppState::TRANSCRIBING).is_ok());
        assert!(sm.transition_to(AppState::DONE).is_ok());
        assert!(sm.transition_to(AppState::RECORDING).is_err());
        assert!(sm.transition_to(AppState::INSERTING).is_err());
        assert!(sm.transition_to(AppState::TRANSCRIBING).is_err());
    }

    #[test]
    fn test_error_transition_and_reset() {
        let sm = AppStateManager::new();
        sm.transition_to(AppState::RECORDING).unwrap();
        sm.set_error("Microphone device lost".into());

        let snapshot = sm.get_snapshot();
        assert_eq!(snapshot.current_state, AppState::ERROR);
        assert_eq!(snapshot.previous_state, Some(AppState::RECORDING));
        assert_eq!(
            snapshot.error_message.as_deref(),
            Some("Microphone device lost")
        );

        sm.reset_to_idle();
        assert_eq!(sm.get_state(), AppState::IDLE);
        assert_eq!(sm.get_snapshot().error_message, None);
        assert_eq!(sm.get_snapshot().previous_state, Some(AppState::ERROR));
    }

    #[test]
    fn test_concurrent_atomic_snapshots() {
        let sm = Arc::new(AppStateManager::new());
        let mut handles = Vec::new();

        for i in 0..10 {
            let sm_clone = Arc::clone(&sm);
            handles.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    if i % 2 == 0 {
                        let _ = sm_clone.transition_to(AppState::RECORDING);
                        let _ = sm_clone.transition_to(AppState::IDLE);
                    } else {
                        sm_clone.set_error("Test error".into());
                        sm_clone.reset_to_idle();
                    }
                    let snap = sm_clone.get_snapshot();
                    // Consistency check: if in ERROR, error_message must be Some("Test error")
                    if snap.current_state == AppState::ERROR {
                        assert_eq!(snap.error_message.as_deref(), Some("Test error"));
                    }
                    // If in IDLE and previous wasn't ERROR, error_message must be None
                    if snap.current_state == AppState::IDLE {
                        assert_eq!(snap.error_message, None);
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
