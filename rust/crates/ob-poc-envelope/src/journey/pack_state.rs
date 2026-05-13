//! Pack lifecycle state machine.
//!
//! Each pack in a session has an associated `PackState` that tracks its
//! lifecycle. Transitions are strictly validated — there are no illegal
//! jumps between states.
//!
//! ## State Machine
//!
//! ```text
//! Dormant ──► Active ──► Completed (terminal)
//!                │  ▲
//!                ▼  │
//!            Suspended
//! ```
//!
//! - `Dormant → Active`: Pack is selected or auto-selected.
//! - `Active → Suspended`: User or system pauses the pack.
//! - `Suspended → Active`: Pack is resumed.
//! - `Active → Completed`: All stop rules / definition-of-done met.
//! - `Completed` is terminal — no further transitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PackState
// ---------------------------------------------------------------------------

/// Lifecycle state of a pack within a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PackState {
    /// Pack is loaded but not yet active. Constraints are NOT enforced.
    Dormant,

    /// Pack is active — constraints are enforced, verbs are scoped.
    Active {
        /// Progress tracking for observable signals.
        progress: PackProgress,
        /// When the pack was activated.
        activated_at: DateTime<Utc>,
    },

    /// Pack is temporarily paused. Constraints are NOT enforced.
    /// Progress is preserved for resumption.
    Suspended {
        /// Preserved progress for when the pack resumes.
        preserved_progress: PackProgress,
        /// Why the pack was suspended.
        reason: SuspendReason,
        /// When the pack was suspended.
        suspended_at: DateTime<Utc>,
    },

    /// Pack completed successfully. This is a terminal state.
    Completed {
        /// Final progress snapshot.
        final_progress: PackProgress,
        /// When the pack completed.
        completed_at: DateTime<Utc>,
    },
}

impl PackState {
    /// Create a new pack in the `Dormant` state.
    pub fn dormant() -> Self {
        Self::Dormant
    }

    /// Whether this pack is currently enforcing constraints.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
    }

    /// Whether this pack has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// Whether this pack is dormant (not yet started).
    pub fn is_dormant(&self) -> bool {
        matches!(self, Self::Dormant)
    }

    /// Transition: Dormant → Active
    pub fn activate(&self) -> Result<Self, PackTransitionError> {
        match self {
            Self::Dormant => Ok(Self::Active {
                progress: PackProgress::new(),
                activated_at: Utc::now(),
            }),
            _ => Err(PackTransitionError::InvalidTransition {
                from: self.state_name().to_string(),
                to: "active".to_string(),
            }),
        }
    }

    /// Transition: Active → Suspended
    pub fn suspend(&self, reason: SuspendReason) -> Result<Self, PackTransitionError> {
        match self {
            Self::Active { progress, .. } => Ok(Self::Suspended {
                preserved_progress: progress.clone(),
                reason,
                suspended_at: Utc::now(),
            }),
            _ => Err(PackTransitionError::InvalidTransition {
                from: self.state_name().to_string(),
                to: "suspended".to_string(),
            }),
        }
    }

    /// Transition: Suspended → Active
    pub fn resume(&self) -> Result<Self, PackTransitionError> {
        match self {
            Self::Suspended {
                preserved_progress, ..
            } => Ok(Self::Active {
                progress: preserved_progress.clone(),
                activated_at: Utc::now(),
            }),
            _ => Err(PackTransitionError::InvalidTransition {
                from: self.state_name().to_string(),
                to: "active".to_string(),
            }),
        }
    }

    /// Transition: Active → Completed
    pub fn complete(&self) -> Result<Self, PackTransitionError> {
        match self {
            Self::Active { progress, .. } => Ok(Self::Completed {
                final_progress: progress.clone(),
                completed_at: Utc::now(),
            }),
            _ => Err(PackTransitionError::InvalidTransition {
                from: self.state_name().to_string(),
                to: "completed".to_string(),
            }),
        }
    }

    /// Get the progress (if any) from the current state.
    pub fn progress(&self) -> Option<&PackProgress> {
        match self {
            Self::Active { progress, .. } => Some(progress),
            Self::Suspended {
                preserved_progress, ..
            } => Some(preserved_progress),
            Self::Completed { final_progress, .. } => Some(final_progress),
            Self::Dormant => None,
        }
    }

    /// Get a mutable reference to progress (only when active).
    pub fn progress_mut(&mut self) -> Option<&mut PackProgress> {
        match self {
            Self::Active { progress, .. } => Some(progress),
            _ => None,
        }
    }

    fn state_name(&self) -> &'static str {
        match self {
            Self::Dormant => "dormant",
            Self::Active { .. } => "active",
            Self::Suspended { .. } => "suspended",
            Self::Completed { .. } => "completed",
        }
    }
}

// ---------------------------------------------------------------------------
// PackProgress
// ---------------------------------------------------------------------------

/// Tracks observable progress within a pack.
///
/// Progress signals are emitted by the execution pipeline and recorded
/// here. They are free-text strings that match `progress_signals` from
/// the pack manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PackProgress {
    /// Signals that have been emitted (e.g., "scope_set", "funds_created").
    pub signals_emitted: Vec<String>,

    /// Verbs that have been successfully executed within this pack.
    pub executed_verbs: Vec<String>,

    /// Count of steps executed.
    pub steps_completed: u32,
}

impl PackProgress {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a signal was emitted.
    pub fn emit_signal(&mut self, signal: &str) {
        if !self.signals_emitted.contains(&signal.to_string()) {
            self.signals_emitted.push(signal.to_string());
        }
    }

    /// Record that a verb was executed.
    pub fn record_verb_execution(&mut self, verb: &str) {
        self.executed_verbs.push(verb.to_string());
        self.steps_completed += 1;
    }
}

// ---------------------------------------------------------------------------
// SuspendReason
// ---------------------------------------------------------------------------

/// Why a pack was suspended.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SuspendReason {
    /// User explicitly paused the pack.
    UserPaused,

    /// System suspended the pack (e.g., constraint violation remediation).
    SystemSuspended { explanation: String },

    /// Waiting for an external callback (e.g., document solicitation).
    AwaitingCallback { callback_type: String },
}

// ---------------------------------------------------------------------------
// PackTransitionError
// ---------------------------------------------------------------------------

/// Error when an invalid pack state transition is attempted.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PackTransitionError {
    #[error("Invalid pack transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dormant_to_active() {
        let state = PackState::dormant();
        assert!(state.is_dormant());
        assert!(!state.is_active());

        let active = state.activate().unwrap();
        assert!(active.is_active());
        assert!(!active.is_dormant());
    }

    #[test]
    fn test_active_to_suspended() {
        let active = PackState::dormant().activate().unwrap();
        let suspended = active.suspend(SuspendReason::UserPaused).unwrap();
        assert!(!suspended.is_active());
        assert!(!suspended.is_terminal());
    }

    #[test]
    fn test_suspended_to_active() {
        let active = PackState::dormant().activate().unwrap();

        // Record some progress
        let mut active = active;
        if let Some(p) = active.progress_mut() {
            p.emit_signal("scope_set");
            p.record_verb_execution("session.load-galaxy");
        }

        let suspended = active.suspend(SuspendReason::UserPaused).unwrap();

        // Resume preserves progress
        let resumed = suspended.resume().unwrap();
        assert!(resumed.is_active());
        let progress = resumed.progress().unwrap();
        assert_eq!(progress.signals_emitted, vec!["scope_set"]);
        assert_eq!(progress.steps_completed, 1);
    }

    #[test]
    fn test_active_to_completed() {
        let active = PackState::dormant().activate().unwrap();
        let completed = active.complete().unwrap();
        assert!(completed.is_terminal());
        assert!(!completed.is_active());
    }

    #[test]
    fn test_completed_is_terminal() {
        let completed = PackState::dormant().activate().unwrap().complete().unwrap();

        // Cannot transition out of completed
        assert!(completed.activate().is_err());
        assert!(completed.suspend(SuspendReason::UserPaused).is_err());
        assert!(completed.resume().is_err());
        assert!(completed.complete().is_err());
    }

    #[test]
    fn test_dormant_cannot_suspend() {
        let dormant = PackState::dormant();
        assert!(dormant.suspend(SuspendReason::UserPaused).is_err());
    }

    #[test]
    fn test_dormant_cannot_complete() {
        let dormant = PackState::dormant();
        assert!(dormant.complete().is_err());
    }

    #[test]
    fn test_progress_signal_dedup() {
        let mut progress = PackProgress::new();
        progress.emit_signal("scope_set");
        progress.emit_signal("scope_set");
        assert_eq!(progress.signals_emitted.len(), 1);
    }

    #[test]
    fn test_serde_round_trip() {
        let state = PackState::dormant().activate().unwrap();
        let json = serde_json::to_string(&state).unwrap();
        let back: PackState = serde_json::from_str(&json).unwrap();
        assert!(back.is_active());
    }
}
