//! Sage-primary utterance routing types.

use serde::{Deserialize, Serialize};

use super::drafter_result::DraftResult;
use super::outcome::{EntityRef, OutcomeAction, OutcomeIntent};

/// The single routing decision after Sage classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UtteranceDisposition {
    /// Sage serves directly with a facts-only path.
    Serve(ServeIntent),
    /// Drafter resolves a mutation candidate that requires confirmation.
    Delegate(Box<DelegateIntent>),
}

/// Read-only intent served without exposing DSL to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServeIntent {
    pub summary: String,
    pub domain: String,
    pub action: OutcomeAction,
    pub subject: Option<EntityRef>,
}

/// Mutation intent delegated to the Drafter for confirmation-first execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateIntent {
    pub summary: String,
    pub outcome: OutcomeIntent,
}

/// Mutation staged between confirmation prompt and execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMutation {
    pub confirmation_text: String,
    pub change_summary: Vec<String>,
    pub drafter_result: DraftResult,
    pub intent: OutcomeIntent,
}
