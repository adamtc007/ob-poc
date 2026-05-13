//! Motivated Sage `GoalFrame` — the typed goal state the agent
//! tracks across a session.
//!
//! Phase 3.1 (C-01 / C-02 / C-03) promotes `GoalFrame` from an
//! inline planning-loop scratch value into a first-class concept:
//!
//! - **C-01** — typed [`GoalFrame`] with explicit lifecycle.
//! - **C-02** — session binding (Phase 3.1b adds the store).
//! - **C-03** — lifecycle transitions ([`GoalFrameStatus`]).
//!
//! The seed shape Phase 2 introduced (utterance, pack anchor,
//! workspace, optional intent summary) stays load-bearing; Phase 3.1
//! layers on the status / lifecycle / transition history that the
//! Motivated Sage planning loop needs to iterate across prompts.
//!
//! ## Status FSM
//!
//! ```text
//!         seed
//!           │
//!           ▼
//!        Proposed ──── refuse ──► Refused
//!           │
//!         confirm
//!           │
//!           ▼
//!        Confirmed ─── start ──► InProgress ── complete ──► Completed
//! ```
//!
//! - **Proposed** — a draft has been emitted but the user hasn't
//!   confirmed yet. The Phase 2 spike's frames are all `Proposed`.
//! - **Refused** — the user (or the agent itself via
//!   constrained-composition refusal) declined the draft.
//! - **Confirmed** — the user accepted the draft; ready for
//!   `validate-and-execute`.
//! - **InProgress** — execution started. Compiler / executor owns
//!   the frame at this point; the agent is in observer mode.
//! - **Completed** — execution finished (success or non-recoverable
//!   failure). Audit record is sealed.
//!
//! Phase 3.6 will wire the transition methods to the approval gates
//! V&S §6.4 references.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::index::SessionIndex;

/// Lifecycle status of a [`GoalFrame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalFrameStatus {
    /// Initial state when the frame is constructed from an
    /// utterance. Phase 2 spike frames terminate here.
    Proposed,
    /// User (or constrained-composition guard) refused the draft.
    Refused,
    /// User accepted the draft; awaits execution.
    Confirmed,
    /// Execution started; agent is in observer mode.
    InProgress,
    /// Execution finished (success or non-recoverable failure).
    Completed,
}

impl GoalFrameStatus {
    /// Whether the planning loop may still mutate the frame. Once
    /// the user has confirmed or execution has started, the frame
    /// is read-only from the agent's perspective.
    pub fn is_mutable(self) -> bool {
        matches!(self, Self::Proposed)
    }

    /// Whether the frame has reached a terminal state and can be
    /// pruned from the per-session store.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Refused | Self::Completed)
    }
}

/// Motivated Sage goal frame.
///
/// Seed fields from Phase 2 are preserved verbatim; Phase 3.1 adds
/// lifecycle (`status`, `updated_at`) and Phase 3.2+ will add
/// constellation hydration / frontier / blockers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalFrame {
    /// Stable id the audit record correlates against (`gf-<uuid>`).
    pub id: String,
    /// Raw utterance the user typed in the editor.
    pub utterance: String,
    /// Pack the session is anchored to.
    pub pack_id: String,
    /// Pack manifest hash (SHA-256 of raw YAML) — captured for
    /// replay-grade audit.
    pub pack_hash: String,
    /// Workspace the session targets (seed: copied from
    /// [`SessionIndex::workspace`]).
    pub workspace: String,
    /// Optional intent summary the planning loop or the LLM
    /// recorded for the round-trip. Phase 2 leaves this `None`;
    /// Phase 3.5 (motivation prompt template) fills it.
    pub intent_summary: Option<String>,
    /// When the frame was constructed.
    pub created_at: DateTime<Utc>,
    /// When the frame last transitioned. Equals `created_at` for
    /// freshly-seeded frames.
    pub updated_at: DateTime<Utc>,
    /// Current lifecycle status.
    pub status: GoalFrameStatus,
}

impl GoalFrame {
    /// Seed constructor — captures the utterance + anchors against
    /// the session index. Phase 3.2+ will introduce richer
    /// constructors that thread constellation hydration + frontier
    /// state in.
    pub fn seed_for_spike(utterance: &str, index: &SessionIndex) -> Self {
        let now = Utc::now();
        Self {
            id: format!("gf-{}", uuid::Uuid::new_v4()),
            utterance: utterance.to_string(),
            pack_id: index.pack.id.clone(),
            pack_hash: index.pack_hash.clone(),
            workspace: workspace_tag(&index.workspace),
            intent_summary: None,
            created_at: now,
            updated_at: now,
            status: GoalFrameStatus::Proposed,
        }
    }

    /// Mark the frame as refused. Idempotent: refusing an already-
    /// terminal frame is a no-op.
    pub fn refuse(&mut self) {
        if self.status.is_terminal() {
            return;
        }
        self.status = GoalFrameStatus::Refused;
        self.updated_at = Utc::now();
    }

    /// Transition Proposed → Confirmed. Returns `Err` if the frame
    /// is not in `Proposed`.
    pub fn confirm(&mut self) -> Result<(), GoalFrameTransitionError> {
        if self.status != GoalFrameStatus::Proposed {
            return Err(GoalFrameTransitionError::InvalidFrom(self.status));
        }
        self.status = GoalFrameStatus::Confirmed;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Transition Confirmed → InProgress.
    pub fn start_execution(&mut self) -> Result<(), GoalFrameTransitionError> {
        if self.status != GoalFrameStatus::Confirmed {
            return Err(GoalFrameTransitionError::InvalidFrom(self.status));
        }
        self.status = GoalFrameStatus::InProgress;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Transition InProgress → Completed.
    pub fn complete(&mut self) -> Result<(), GoalFrameTransitionError> {
        if self.status != GoalFrameStatus::InProgress {
            return Err(GoalFrameTransitionError::InvalidFrom(self.status));
        }
        self.status = GoalFrameStatus::Completed;
        self.updated_at = Utc::now();
        Ok(())
    }
}

/// Errors produced by [`GoalFrame`] state transitions.
#[derive(Debug, thiserror::Error)]
pub enum GoalFrameTransitionError {
    #[error("invalid transition from status {0:?}")]
    InvalidFrom(GoalFrameStatus),
}

/// Stable workspace tag — picks the serde rename when present (e.g.
/// `OnBoarding -> "onboarding_request"`) so the audit-shape value
/// matches everything else in the system.
fn workspace_tag(workspace: &ob_poc_types::session::kinds::WorkspaceKind) -> String {
    serde_json::to_value(workspace)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| format!("{workspace:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ob_poc_journey::pack::load_pack_from_bytes;
    use ob_poc_types::session::kinds::WorkspaceKind;

    fn manifest_yaml() -> &'static [u8] {
        br#"
id: book-setup
name: Book Setup
version: "0.1"
description: GoalFrame test fixture
invocation_phrases: []
required_context: []
optional_context: []
workspaces:
  - cbu
allowed_verbs:
  - cbu.create
forbidden_verbs: []
required_questions: []
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done: []
progress_signals: []
"#
    }

    fn make_index() -> SessionIndex {
        let (pack, pack_hash) = load_pack_from_bytes(manifest_yaml()).unwrap();
        SessionIndex {
            pack,
            pack_hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        }
    }

    #[test]
    fn seed_starts_in_proposed_status() {
        let frame = GoalFrame::seed_for_spike("set up a book", &make_index());
        assert_eq!(frame.status, GoalFrameStatus::Proposed);
        assert!(frame.status.is_mutable());
        assert!(!frame.status.is_terminal());
        assert_eq!(frame.created_at, frame.updated_at);
    }

    #[test]
    fn confirm_advances_status_and_bumps_updated_at() {
        let mut frame = GoalFrame::seed_for_spike("draft", &make_index());
        let before = frame.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(2));
        frame.confirm().unwrap();
        assert_eq!(frame.status, GoalFrameStatus::Confirmed);
        assert!(frame.updated_at > before);
    }

    #[test]
    fn refuse_is_idempotent_from_terminal_state() {
        let mut frame = GoalFrame::seed_for_spike("draft", &make_index());
        frame.refuse();
        let snapshot = frame.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(2));
        frame.refuse(); // No-op
        assert_eq!(frame.status, GoalFrameStatus::Refused);
        assert_eq!(frame.updated_at, snapshot, "no-op must not bump updated_at");
    }

    #[test]
    fn confirm_from_non_proposed_state_errors() {
        let mut frame = GoalFrame::seed_for_spike("draft", &make_index());
        frame.refuse();
        let err = frame.confirm().expect_err("must reject");
        match err {
            GoalFrameTransitionError::InvalidFrom(status) => {
                assert_eq!(status, GoalFrameStatus::Refused);
            }
        }
    }

    #[test]
    fn full_happy_path_lifecycle() {
        let mut frame = GoalFrame::seed_for_spike("seed", &make_index());
        frame.confirm().unwrap();
        frame.start_execution().unwrap();
        frame.complete().unwrap();
        assert_eq!(frame.status, GoalFrameStatus::Completed);
        assert!(frame.status.is_terminal());
        assert!(!frame.status.is_mutable());
    }
}
