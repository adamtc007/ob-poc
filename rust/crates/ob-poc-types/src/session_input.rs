//! Unified session input envelope types.
//!
//! These types define the single input boundary for UI->server traffic:
//! `POST /api/session/:id/input`.

use serde::{Deserialize, Serialize};

/// Unified request for all user input into a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionInputRequest {
    /// Natural language utterance (chat-style input).
    Utterance { message: String },
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
