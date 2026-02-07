//! Response V2 Types — Sentence-first responses
//!
//! Every response from the v2 REPL carries the current state, a human message,
//! and optionally a runbook summary. The response kinds are designed around
//! sentences rather than raw DSL.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::proposal_engine::StepProposal;
use super::types_v2::{PackCandidate, ReplStateV2, VerbCandidate};

// ---------------------------------------------------------------------------
// ReplResponseV2
// ---------------------------------------------------------------------------

/// The top-level response from the v2 REPL orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplResponseV2 {
    /// Current state of the REPL after processing the input.
    pub state: ReplStateV2,

    /// The kind of response (determines UI rendering).
    pub kind: ReplResponseKindV2,

    /// Human-readable message to display.
    pub message: String,

    /// Pack-level runbook summary (when a runbook exists).
    pub runbook_summary: Option<String>,

    /// Number of steps in the current runbook.
    pub step_count: usize,
}

// ---------------------------------------------------------------------------
// ReplResponseKindV2
// ---------------------------------------------------------------------------

/// The type of response — determines how the UI renders it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplResponseKindV2 {
    /// User needs to select a scope (client group / CBU set).
    ScopeRequired { prompt: String },

    /// Available journey packs for selection.
    JourneyOptions { packs: Vec<PackCandidate> },

    /// Pack is asking a question.
    Question {
        field: String,
        prompt: String,
        answer_kind: String,
    },

    /// Sentence playback — user should confirm or reject.
    SentencePlayback {
        sentence: String,
        verb: String,
        step_sequence: i32,
    },

    /// Full runbook summary with chapters.
    RunbookSummary {
        chapters: Vec<ChapterView>,
        summary: String,
    },

    /// Verb/entity disambiguation needed.
    Clarification {
        question: String,
        options: Vec<VerbCandidate>,
    },

    /// Execution completed (or in progress).
    Executed { results: Vec<StepResult> },

    /// Runbook is parked — one or more entries awaiting external signal or approval.
    Parked {
        results_so_far: Vec<StepResult>,
        parked_entries: Vec<ParkedEntryInfo>,
        summary: String,
    },

    /// Ranked step proposals for user review (Phase 3).
    StepProposals {
        proposals: Vec<StepProposal>,
        template_fast_path: bool,
        proposal_hash: String,
    },

    /// Informational message (status, why, options) — no action required.
    Info { detail: String },

    /// Prompt for user input (e.g. after switching journey).
    Prompt { text: String },

    /// Something went wrong.
    Error { error: String, recoverable: bool },
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// A chapter in a runbook summary — groups related steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterView {
    pub chapter: String,
    pub steps: Vec<(i32, String)>, // (sequence, sentence)
}

/// Result of executing a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub entry_id: Uuid,
    pub sequence: i32,
    pub sentence: String,
    pub success: bool,
    pub message: Option<String>,
    pub result: Option<serde_json::Value>,
}

/// Information about a parked runbook entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParkedEntryInfo {
    pub entry_id: Uuid,
    pub sequence: i32,
    pub sentence: String,
    pub gate_type: String,
    pub correlation_key: String,
    pub parked_at: DateTime<Utc>,
    pub timeout_at: Option<DateTime<Utc>>,
    pub needs_approval: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::types_v2::ExecutionProgress;
    use std::collections::HashMap;

    #[test]
    fn test_response_serialization_roundtrip() {
        let response = ReplResponseV2 {
            state: ReplStateV2::SentencePlayback {
                sentence: "Create Allianz Lux CBU".to_string(),
                verb: "cbu.create".to_string(),
                dsl: "(cbu.create :name \"Allianz Lux\")".to_string(),
                args: HashMap::from([("name".to_string(), "Allianz Lux".to_string())]),
            },
            kind: ReplResponseKindV2::SentencePlayback {
                sentence: "Create Allianz Lux CBU".to_string(),
                verb: "cbu.create".to_string(),
                step_sequence: 1,
            },
            message: "Please confirm this step.".to_string(),
            runbook_summary: None,
            step_count: 1,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ReplResponseV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.message, "Please confirm this step.");
        assert_eq!(deserialized.step_count, 1);
    }

    #[test]
    fn test_error_response() {
        let response = ReplResponseV2 {
            state: ReplStateV2::RunbookEditing,
            kind: ReplResponseKindV2::Error {
                error: "Verb not found".to_string(),
                recoverable: true,
            },
            message: "I couldn't find a matching verb.".to_string(),
            runbook_summary: None,
            step_count: 0,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"kind\":\"error\""));
        assert!(json.contains("recoverable"));
    }

    #[test]
    fn test_journey_options_response() {
        let response = ReplResponseV2 {
            state: ReplStateV2::JourneySelection {
                candidates: Some(vec![PackCandidate {
                    pack_id: "onboarding-request".to_string(),
                    pack_name: "Onboarding Request".to_string(),
                    description: "Onboard a new client".to_string(),
                    score: 0.95,
                }]),
            },
            kind: ReplResponseKindV2::JourneyOptions {
                packs: vec![PackCandidate {
                    pack_id: "onboarding-request".to_string(),
                    pack_name: "Onboarding Request".to_string(),
                    description: "Onboard a new client".to_string(),
                    score: 0.95,
                }],
            },
            message: "Which journey would you like to start?".to_string(),
            runbook_summary: None,
            step_count: 0,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("onboarding-request"));
    }

    #[test]
    fn test_executed_response() {
        let response = ReplResponseV2 {
            state: ReplStateV2::Executing {
                runbook_id: Uuid::new_v4(),
                progress: ExecutionProgress::new(2),
            },
            kind: ReplResponseKindV2::Executed {
                results: vec![StepResult {
                    entry_id: Uuid::new_v4(),
                    sequence: 1,
                    sentence: "Create Allianz Lux CBU".to_string(),
                    success: true,
                    message: Some("CBU created".to_string()),
                    result: Some(serde_json::json!({"cbu_id": "uuid-123"})),
                }],
            },
            message: "Execution complete.".to_string(),
            runbook_summary: Some("1 step completed successfully.".to_string()),
            step_count: 1,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ReplResponseV2 = serde_json::from_str(&json).unwrap();
        assert!(deserialized.runbook_summary.is_some());
    }

    #[test]
    fn test_chapter_view() {
        let chapters = vec![
            ChapterView {
                chapter: "Setup".to_string(),
                steps: vec![
                    (1, "Create Allianz Lux CBU".to_string()),
                    (2, "Assign depositary role".to_string()),
                ],
            },
            ChapterView {
                chapter: "Products".to_string(),
                steps: vec![(3, "Add IRS product".to_string())],
            },
        ];

        let json = serde_json::to_string(&chapters).unwrap();
        let deserialized: Vec<ChapterView> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized[0].steps.len(), 2);
        assert_eq!(deserialized[1].steps[0].0, 3);
    }
}
