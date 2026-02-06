//! V2 State Machine Types — 7-State REPL
//!
//! Defines the new 7-state machine from spec §8.3:
//! ScopeGate → JourneySelection → InPack → Clarifying →
//! SentencePlayback → RunbookEditing → Executing
//!
//! Also defines `UserInputV2` — the conversational input model.
//! All answers are free-text `Message` input; no picker/form gates.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ReplStateV2 — 7-state machine
// ---------------------------------------------------------------------------

/// The seven states of the v2 REPL pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReplStateV2 {
    /// Waiting for client/scope selection before any pack can start.
    ScopeGate { pending_input: Option<String> },

    /// User has scope, now choosing a journey pack.
    JourneySelection {
        candidates: Option<Vec<PackCandidate>>,
    },

    /// Inside an active pack — asking questions, matching verbs, building runbook.
    InPack {
        pack_id: String,
        required_slots_remaining: Vec<String>,
        last_proposal_id: Option<Uuid>,
    },

    /// Waiting for user to disambiguate a verb or entity.
    Clarifying {
        question: String,
        candidates: Vec<VerbCandidate>,
        original_input: String,
    },

    /// Showing a sentence for user to confirm or reject.
    SentencePlayback {
        sentence: String,
        verb: String,
        dsl: String,
        args: HashMap<String, String>,
    },

    /// Runbook exists and user is reviewing / editing it.
    RunbookEditing,

    /// Runbook is executing.
    Executing {
        runbook_id: Uuid,
        progress: ExecutionProgress,
    },
}

// ---------------------------------------------------------------------------
// Supporting types for state variants
// ---------------------------------------------------------------------------

/// A candidate pack for journey selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackCandidate {
    pub pack_id: String,
    pub pack_name: String,
    pub description: String,
    pub score: f32,
}

/// A candidate verb for clarification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidate {
    pub verb_fqn: String,
    pub description: String,
    pub score: f32,
}

/// Progress of runbook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionProgress {
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub parked_steps: usize,
    pub current_step: Option<Uuid>,
    pub parked_entry_id: Option<Uuid>,
}

impl ExecutionProgress {
    pub fn new(total_steps: usize) -> Self {
        Self {
            total_steps,
            completed_steps: 0,
            failed_steps: 0,
            parked_steps: 0,
            current_step: None,
            parked_entry_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// UserInputV2 — conversational model, not typed forms
// ---------------------------------------------------------------------------

/// All input variants the v2 REPL accepts.
///
/// Design rule: conversation-first. All answers are accepted as free-text
/// `Message` input. Structured variants exist only for explicit UI actions
/// (button clicks, picker selections), never as correctness gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserInputV2 {
    /// Free-text conversational input — the primary input mode.
    Message { content: String },

    /// User confirmed a sentence or runbook.
    Confirm,

    /// User rejected a proposed sentence.
    Reject,

    /// User edited a specific field on a runbook entry.
    Edit {
        step_id: Uuid,
        field: String,
        value: String,
    },

    /// Explicit REPL command.
    Command { command: ReplCommandV2 },

    /// User explicitly selected a pack by ID.
    SelectPack { pack_id: String },

    /// User selected a verb from disambiguation options.
    SelectVerb {
        verb_fqn: String,
        original_input: String,
    },

    /// User selected a proposal from the ranked list (Phase 3).
    SelectProposal { proposal_id: Uuid },

    /// User selected an entity to resolve an ambiguous reference.
    SelectEntity {
        ref_id: String,
        entity_id: Uuid,
        entity_name: String,
    },

    /// User selected a scope (client group / CBU set).
    SelectScope { group_id: Uuid, group_name: String },

    /// User approves a human-gated entry.
    Approve {
        entry_id: Uuid,
        approved_by: Option<String>,
    },

    /// User rejects a human-gated entry.
    RejectGate {
        entry_id: Uuid,
        reason: Option<String>,
    },
}

/// REPL commands available to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplCommandV2 {
    /// Execute the runbook.
    Run,
    /// Undo the last action.
    Undo,
    /// Redo the last undone action.
    Redo,
    /// Clear the runbook.
    Clear,
    /// Cancel the current operation.
    Cancel,
    /// Show session info.
    Info,
    /// Show help.
    Help,
    /// Remove a specific runbook entry.
    Remove(Uuid),
    /// Reorder runbook entries.
    Reorder(Vec<Uuid>),
    /// Disable a specific runbook entry (skip during execution).
    Disable(Uuid),
    /// Enable a previously disabled entry.
    Enable(Uuid),
    /// Toggle disabled state on an entry.
    Toggle(Uuid),
    /// Show status of parked entries.
    Status,
    /// Resume a parked entry (by entry_id) — for internal use after signal.
    Resume(Uuid),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_serialization_roundtrip() {
        let state = ReplStateV2::InPack {
            pack_id: "onboarding-request".to_string(),
            required_slots_remaining: vec!["products".to_string(), "jurisdiction".to_string()],
            last_proposal_id: Some(Uuid::new_v4()),
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: ReplStateV2 = serde_json::from_str(&json).unwrap();

        match deserialized {
            ReplStateV2::InPack {
                pack_id,
                required_slots_remaining,
                ..
            } => {
                assert_eq!(pack_id, "onboarding-request");
                assert_eq!(required_slots_remaining.len(), 2);
            }
            _ => panic!("Wrong state variant"),
        }
    }

    #[test]
    fn test_input_message_serialization() {
        let input = UserInputV2::Message {
            content: "Add IRS product".to_string(),
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"message\""));
        assert!(json.contains("Add IRS product"));
    }

    #[test]
    fn test_input_confirm_serialization() {
        let input = UserInputV2::Confirm;
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"confirm\""));
    }

    #[test]
    fn test_input_select_pack_serialization() {
        let input = UserInputV2::SelectPack {
            pack_id: "onboarding-request".to_string(),
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"select_pack\""));
        assert!(json.contains("onboarding-request"));
    }

    #[test]
    fn test_input_command_run() {
        let input = UserInputV2::Command {
            command: ReplCommandV2::Run,
        };
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: UserInputV2 = serde_json::from_str(&json).unwrap();
        match deserialized {
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            } => {}
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_input_command_remove() {
        let id = Uuid::new_v4();
        let input = UserInputV2::Command {
            command: ReplCommandV2::Remove(id),
        };
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: UserInputV2 = serde_json::from_str(&json).unwrap();
        match deserialized {
            UserInputV2::Command {
                command: ReplCommandV2::Remove(rid),
            } => assert_eq!(rid, id),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_execution_progress() {
        let mut progress = ExecutionProgress::new(5);
        assert_eq!(progress.total_steps, 5);
        assert_eq!(progress.completed_steps, 0);

        progress.completed_steps = 3;
        progress.current_step = Some(Uuid::new_v4());
        assert_eq!(progress.completed_steps, 3);
    }

    #[test]
    fn test_all_state_variants_serialize() {
        let states: Vec<ReplStateV2> = vec![
            ReplStateV2::ScopeGate {
                pending_input: Some("allianz".to_string()),
            },
            ReplStateV2::JourneySelection {
                candidates: Some(vec![PackCandidate {
                    pack_id: "test".to_string(),
                    pack_name: "Test".to_string(),
                    description: "desc".to_string(),
                    score: 0.9,
                }]),
            },
            ReplStateV2::InPack {
                pack_id: "test".to_string(),
                required_slots_remaining: vec![],
                last_proposal_id: None,
            },
            ReplStateV2::Clarifying {
                question: "Which verb?".to_string(),
                candidates: vec![],
                original_input: "load".to_string(),
            },
            ReplStateV2::SentencePlayback {
                sentence: "Create Allianz Lux CBU".to_string(),
                verb: "cbu.create".to_string(),
                dsl: "(cbu.create :name \"Allianz Lux\")".to_string(),
                args: HashMap::new(),
            },
            ReplStateV2::RunbookEditing,
            ReplStateV2::Executing {
                runbook_id: Uuid::new_v4(),
                progress: ExecutionProgress::new(3),
            },
        ];

        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let _: ReplStateV2 = serde_json::from_str(&json).unwrap();
        }
    }
}
