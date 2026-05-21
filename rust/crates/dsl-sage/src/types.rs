//! Core types for the Sage pack matcher and parameter extractor.

use serde::{Deserialize, Serialize};

/// Contextual signals available to the pack matcher.
///
/// All fields are optional — the matcher degrades gracefully with less context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SageContext {
    /// Current domain/workspace scope (e.g., `"kyc"`, `"cbu"`).
    pub domain: Option<String>,
    /// Conversation history — last N turns as plain strings.
    pub history: Vec<String>,
    /// Currently loaded process name (if any).
    pub process_name: Option<String>,
}

impl SageContext {
    /// Empty context — no domain, no history.
    pub fn empty() -> Self {
        Self {
            domain: None,
            history: vec![],
            process_name: None,
        }
    }

    /// Context with a specific domain.
    pub fn with_domain(domain: impl Into<String>) -> Self {
        Self {
            domain: Some(domain.into()),
            history: vec![],
            process_name: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tranche 2: Parameter extraction + confirmation types
// ---------------------------------------------------------------------------

/// A proposed value for a single pack parameter, produced by the extractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterProposal {
    /// The parameter name as declared in the pack (e.g., `"gate-name"`).
    pub parameter_name: String,
    /// The proposed value (may be null when extraction failed).
    pub proposed_value: serde_json::Value,
    /// Confidence score `[0, 1]`.  1.0 = user-explicitly-set.
    pub confidence: f32,
    /// Human-readable explanation of how the value was derived.
    pub rationale: String,
    /// Span from the utterance that motivated this value, if identifiable.
    pub source_phrase: Option<String>,
}

/// A confirmation request presented to the user before DSL emission.
///
/// The user can accept, edit individual parameters, reject the pack entirely,
/// or cancel the whole flow.  See [`ConfirmationResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    /// The selected pack name (e.g., `"conjunctive-gate"`).
    pub pack_name: String,
    /// The selected pack version string.
    pub pack_version: String,
    /// Proposed values for every declared parameter.
    pub proposed_parameters: Vec<ParameterProposal>,
    /// Preview DSL string with proposed parameters substituted.
    ///
    /// Tranche 3 will fill this with real DSL output.  For now it is a
    /// human-readable placeholder showing the pack name and parameter bindings.
    pub preview_dsl: String,
}

/// User response to a [`ConfirmationRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfirmationResponse {
    /// Accept all proposed parameters and proceed to DSL emission.
    Accept,
    /// Change the value of one parameter and stay in the Pending state.
    EditParameter {
        name: String,
        new_value: serde_json::Value,
    },
    /// Reject this pack; return to pack-matching (Tranche 1).
    RejectPack,
    /// Abort the whole authoring flow.
    Cancel,
}

// ---------------------------------------------------------------------------
// RankedCandidate (Tranche 1)
// ---------------------------------------------------------------------------

/// A single ranked candidate returned by the pack matcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedCandidate {
    /// The decision pack's name (e.g., `"conjunctive-gate"`).
    pub pack_name: String,
    /// The decision pack's version string.
    pub pack_version: String,
    /// Combined confidence score `[0, 1]`.
    ///
    /// Scoring function (§1.5):
    ///   `confidence = 0.5 * embedding_score + 0.5 * rank_score`
    ///   where `rank_score = 1.0 - (llm_rank - 1) / N`.
    /// In embedding-only mode `rank_score` is derived from sorted position.
    pub confidence: f32,
    /// Human-readable rationale (from LLM or embedding score fallback).
    pub rationale: String,
    /// Cosine / Jaccard similarity from the retrieval layer `[0, 1]`.
    pub embedding_score: f32,
    /// LLM ranking position (1 = best match).
    ///
    /// `Some` after an LLM ranking call, or after embedding-only ranking
    /// (position within sorted list).  `None` only if the pack was not
    /// evaluated.
    pub llm_rank: Option<usize>,
}
