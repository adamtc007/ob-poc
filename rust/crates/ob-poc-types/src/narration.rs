//! Proactive narration types — goal-directed workflow guidance.
//!
//! After every state-changing action, the NarrationEngine computes the
//! constellation delta and produces a `NarrationPayload` with progress,
//! gaps, and suggested next steps. The UI renders this as a contextual
//! progress panel below the execution result.
//!
//! Design: ADR 043 (ai-thoughts/043-sage-proactive-narration.md)

use serde::{Deserialize, Serialize};

/// Proactive narration payload, computed after every state-changing action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrationPayload {
    /// Human-readable progress summary.
    /// e.g., "3 of 7 roles filled for Lux UCITS SICAV Alpha"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<String>,

    /// What changed in this turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta: Vec<SlotDelta>,

    /// Required slots still empty — these block downstream workflows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_gaps: Vec<NarrationGap>,

    /// Optional slots still empty — suggestions, not blockers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_gaps: Vec<NarrationGap>,

    /// Suggested next actions, ordered by dependency priority.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_next: Vec<SuggestedAction>,

    /// Active blockers (prereqs not met for available macros).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<NarrationBlocker>,

    /// Narration verbosity for this turn.
    pub verbosity: NarrationVerbosity,
}

/// A slot state transition observed in this turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotDelta {
    pub slot_name: String,
    pub slot_label: String,
    pub from_state: String,
    pub to_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_name: Option<String>,
}

/// A gap in the constellation — a slot that needs filling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrationGap {
    pub slot_name: String,
    pub slot_label: String,
    /// Why this slot is required, e.g., "needed for UCITS authorisation"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why_required: Option<String>,
    /// Verb FQN that would fill this slot.
    pub suggested_verb: String,
    /// Macro FQN if one wraps this verb.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_macro: Option<String>,
    /// Natural language suggestion for the operator.
    pub suggested_utterance: String,
}

/// A suggested next action, derived from the constellation dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub verb_fqn: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_fqn: Option<String>,
    pub utterance: String,
    pub priority: ActionPriority,
    pub reason: String,
}

/// Priority levels for suggested actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPriority {
    /// Required slot unfilled — blocks downstream workflows.
    Critical,
    /// Next in dependency chain — natural progression.
    Recommended,
    /// Optional but contextually relevant.
    Optional,
}

/// A blocker preventing a macro or workflow step from executing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrationBlocker {
    pub blocked_verb: String,
    pub reason: String,
    /// What needs to happen to unblock.
    pub unblock_hint: String,
}

/// Narration verbosity — controls how much context the UI shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NarrationVerbosity {
    /// Full constellation overview with all gaps.
    Full,
    /// Progress fraction + next required action.
    Medium,
    /// Acknowledge + remaining count.
    Light,
    /// No narration (read-only action or exploring).
    Silent,
}

impl NarrationPayload {
    /// Create a silent narration (no content).
    pub fn silent() -> Self {
        Self {
            progress: None,
            delta: Vec::new(),
            required_gaps: Vec::new(),
            optional_gaps: Vec::new(),
            suggested_next: Vec::new(),
            blockers: Vec::new(),
            verbosity: NarrationVerbosity::Silent,
        }
    }

    /// True if this narration has content worth rendering.
    pub fn has_content(&self) -> bool {
        self.verbosity != NarrationVerbosity::Silent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silent_has_no_content() {
        let n = NarrationPayload::silent();
        assert!(!n.has_content());
        assert_eq!(n.verbosity, NarrationVerbosity::Silent);
    }

    #[test]
    fn test_serde_roundtrip() {
        let payload = NarrationPayload {
            progress: Some("3 of 7 roles filled".into()),
            delta: vec![SlotDelta {
                slot_name: "depositary".into(),
                slot_label: "Depositary".into(),
                from_state: "empty".into(),
                to_state: "filled".into(),
                entity_name: Some("BNP Paribas".into()),
            }],
            required_gaps: vec![NarrationGap {
                slot_name: "management_company".into(),
                slot_label: "Management Company".into(),
                why_required: Some("needed for UCITS authorisation".into()),
                suggested_verb: "cbu.assign-role".into(),
                suggested_macro: Some("structure.assign-role".into()),
                suggested_utterance: "assign a Management Company".into(),
            }],
            optional_gaps: Vec::new(),
            suggested_next: vec![SuggestedAction {
                verb_fqn: "cbu.assign-role".into(),
                macro_fqn: Some("structure.assign-role".into()),
                utterance: "assign a Management Company".into(),
                priority: ActionPriority::Critical,
                reason: "required for UCITS authorisation".into(),
            }],
            blockers: Vec::new(),
            verbosity: NarrationVerbosity::Medium,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let round: NarrationPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(round.progress, payload.progress);
        assert_eq!(round.delta.len(), 1);
        assert_eq!(round.required_gaps.len(), 1);
        assert_eq!(round.suggested_next.len(), 1);
        assert_eq!(round.verbosity, NarrationVerbosity::Medium);
    }
}
