//! Unified session input envelope types.
//!
//! These types define the single input boundary for UI->server traffic:
//! `POST /api/session/:id/input`.

use serde::{Deserialize, Serialize};

/// Structured discovery navigation input from the UI bootstrap card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverySelection {
    /// What kind of discovery item the user selected.
    pub selection_kind: DiscoverySelectionKind,
    /// Stable identifier for the selected item.
    pub selection_id: String,
    /// Human-readable label shown to the user.
    #[serde(default)]
    pub label: Option<String>,
    /// Target field for question/answer style inputs.
    #[serde(default)]
    pub maps_to: Option<String>,
    /// Value supplied for question/answer style inputs.
    #[serde(default)]
    pub value: Option<String>,
}

/// Kind of structured discovery selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySelectionKind {
    /// Select a discovery domain / work area.
    Domain,
    /// Select a constellation family.
    Family,
    /// Select a concrete constellation.
    Constellation,
    /// Answer a discovery bootstrap question.
    QuestionAnswer,
}

/// Unified request for all user input into a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionInputRequest {
    /// Natural language utterance (chat-style input).
    Utterance { message: String },
    /// Structured discovery navigation selection from the bootstrap UI.
    DiscoverySelection { selection: DiscoverySelection },
    /// Structured reply to a pending DecisionPacket.
    DecisionReply {
        packet_id: String,
        reply: crate::decision::UserReply,
    },
    /// REPL V2 input payload (forwarded to the REPL adapter).
    ///
    /// The payload shape must match `InputRequestV2`.
    ReplV2 { input: serde_json::Value },
}

/// Unified response envelope for session input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionInputResponse {
    /// Chat adapter response.
    Chat {
        response: Box<crate::chat::ChatResponse>,
    },
    /// Decision adapter response.
    Decision {
        response: crate::decision::DecisionReplyResponse,
    },
    /// REPL V2 adapter response.
    ReplV2 { response: serde_json::Value },
}
