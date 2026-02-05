//! REPL Response types
//!
//! Unified response format for all REPL interactions.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{
    ClarifyingState, ClientGroupOption, IntentTierOption, LedgerEntry, ReplState, ScopeCandidate,
    UnresolvedRef, VerbCandidate,
};

/// Unified response from the REPL orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplResponse {
    /// Current REPL state after processing
    pub state: ReplState,

    /// Response kind with payload
    pub kind: ReplResponseKind,

    /// Message to display to user
    pub message: String,

    /// Entry ID for this interaction (for correlation)
    pub entry_id: Uuid,

    /// Ledger entries for this turn (may include multiple for selections)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<LedgerEntry>,
}

/// Response kinds with associated payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReplResponseKind {
    /// Simple acknowledgment (no action needed)
    Ack,

    /// DSL ready for execution
    DslReady {
        dsl: String,
        verb: String,
        summary: String,
        can_auto_execute: bool,
    },

    /// DSL executed successfully
    Executed {
        dsl: String,
        result_message: String,
        affected_cbu_ids: Vec<Uuid>,
        bindings: Vec<(String, Uuid)>,
    },

    /// Need verb disambiguation
    VerbDisambiguation {
        options: Vec<VerbCandidate>,
        original_input: String,
        margin: f32,
    },

    /// Need scope/client selection
    ScopeSelection {
        options: Vec<ScopeCandidate>,
        original_input: String,
    },

    /// Need entity resolution
    EntityResolution {
        unresolved_refs: Vec<UnresolvedRef>,
        partial_dsl: String,
    },

    /// Need confirmation for action
    Confirmation {
        dsl: String,
        verb: String,
        summary: String,
    },

    /// Need intent tier selection
    IntentTierSelection {
        tier_number: u32,
        options: Vec<IntentTierOption>,
        original_input: String,
        prompt: String,
    },

    /// Need client group selection
    ClientGroupSelection {
        options: Vec<ClientGroupOption>,
        prompt: String,
    },

    /// Error response
    Error { error: String, recoverable: bool },

    /// No match found
    NoMatch {
        reason: String,
        suggestions: Vec<String>,
    },
}

impl ReplResponse {
    /// Create an acknowledgment response
    pub fn ack(state: ReplState, message: impl Into<String>) -> Self {
        Self {
            state,
            kind: ReplResponseKind::Ack,
            message: message.into(),
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create a DSL ready response
    pub fn dsl_ready(
        dsl: impl Into<String>,
        verb: impl Into<String>,
        summary: impl Into<String>,
        can_auto_execute: bool,
    ) -> Self {
        let dsl = dsl.into();
        let verb = verb.into();
        let summary_str = summary.into();

        Self {
            state: ReplState::DslReady {
                dsl: dsl.clone(),
                verb: verb.clone(),
                can_auto_execute,
            },
            kind: ReplResponseKind::DslReady {
                dsl,
                verb,
                summary: summary_str.clone(),
                can_auto_execute,
            },
            message: summary_str,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create an execution result response
    pub fn executed(
        dsl: impl Into<String>,
        result_message: impl Into<String>,
        affected_cbu_ids: Vec<Uuid>,
        bindings: Vec<(String, Uuid)>,
    ) -> Self {
        let result_msg = result_message.into();
        Self {
            state: ReplState::Idle,
            kind: ReplResponseKind::Executed {
                dsl: dsl.into(),
                result_message: result_msg.clone(),
                affected_cbu_ids,
                bindings,
            },
            message: result_msg,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create a verb disambiguation response
    pub fn verb_disambiguation(
        options: Vec<VerbCandidate>,
        original_input: String,
        margin: f32,
    ) -> Self {
        let message = format!(
            "I found {} possible actions. Please select the one you meant:",
            options.len()
        );

        Self {
            state: ReplState::Clarifying(ClarifyingState::VerbSelection {
                options: options.clone(),
                original_input: original_input.clone(),
                margin,
            }),
            kind: ReplResponseKind::VerbDisambiguation {
                options,
                original_input,
                margin,
            },
            message,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create a scope selection response
    pub fn scope_selection(options: Vec<ScopeCandidate>, original_input: String) -> Self {
        let message = format!(
            "Found {} matching scopes. Please select one:",
            options.len()
        );

        Self {
            state: ReplState::Clarifying(ClarifyingState::ScopeSelection {
                options: options.clone(),
                original_input: original_input.clone(),
            }),
            kind: ReplResponseKind::ScopeSelection {
                options,
                original_input,
            },
            message,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create an entity resolution response
    pub fn entity_resolution(unresolved_refs: Vec<UnresolvedRef>, partial_dsl: String) -> Self {
        let ref_count = unresolved_refs.len();
        let message = format!(
            "I need help resolving {} entity reference{}:",
            ref_count,
            if ref_count == 1 { "" } else { "s" }
        );

        Self {
            state: ReplState::Clarifying(ClarifyingState::EntityResolution {
                unresolved_refs: unresolved_refs.clone(),
                partial_dsl: partial_dsl.clone(),
            }),
            kind: ReplResponseKind::EntityResolution {
                unresolved_refs,
                partial_dsl,
            },
            message,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create a confirmation response
    pub fn confirmation(dsl: String, verb: String, summary: String) -> Self {
        Self {
            state: ReplState::Clarifying(ClarifyingState::Confirmation {
                dsl: dsl.clone(),
                verb: verb.clone(),
                summary: summary.clone(),
            }),
            kind: ReplResponseKind::Confirmation {
                dsl,
                verb,
                summary: summary.clone(),
            },
            message: summary,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create an intent tier selection response
    pub fn intent_tier_selection(
        tier_number: u32,
        options: Vec<IntentTierOption>,
        original_input: String,
        prompt: String,
    ) -> Self {
        Self {
            state: ReplState::Clarifying(ClarifyingState::IntentTier {
                tier_number,
                options: options.clone(),
                original_input: original_input.clone(),
            }),
            kind: ReplResponseKind::IntentTierSelection {
                tier_number,
                options,
                original_input,
                prompt: prompt.clone(),
            },
            message: prompt,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create a client group selection response
    pub fn client_group_selection(options: Vec<ClientGroupOption>, prompt: String) -> Self {
        Self {
            state: ReplState::Clarifying(ClarifyingState::ClientGroupSelection {
                options: options.clone(),
                prompt: prompt.clone(),
            }),
            kind: ReplResponseKind::ClientGroupSelection {
                options,
                prompt: prompt.clone(),
            },
            message: prompt,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create an error response
    pub fn error(error: impl Into<String>, recoverable: bool) -> Self {
        let error_str = error.into();
        Self {
            state: ReplState::Idle,
            kind: ReplResponseKind::Error {
                error: error_str.clone(),
                recoverable,
            },
            message: error_str,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Create a "no match" response
    pub fn no_match(reason: impl Into<String>, suggestions: Vec<String>) -> Self {
        let reason_str = reason.into();
        let message = if suggestions.is_empty() {
            reason_str.clone()
        } else {
            format!("{}. Did you mean: {}?", reason_str, suggestions.join(", "))
        };

        Self {
            state: ReplState::Idle,
            kind: ReplResponseKind::NoMatch {
                reason: reason_str,
                suggestions,
            },
            message,
            entry_id: Uuid::new_v4(),
            entries: vec![],
        }
    }

    /// Set the entry ID
    pub fn with_entry_id(mut self, entry_id: Uuid) -> Self {
        self.entry_id = entry_id;
        self
    }

    /// Add ledger entries
    pub fn with_entries(mut self, entries: Vec<LedgerEntry>) -> Self {
        self.entries = entries;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_serialization() {
        let response = ReplResponse::dsl_ready(
            "(cbu.create :name \"Test\")",
            "cbu.create",
            "Create a new CBU named 'Test'",
            false,
        );

        let json = serde_json::to_string(&response).unwrap();
        let parsed: ReplResponse = serde_json::from_str(&json).unwrap();

        assert!(matches!(parsed.kind, ReplResponseKind::DslReady { .. }));
        assert!(matches!(parsed.state, ReplState::DslReady { .. }));
    }

    #[test]
    fn test_verb_disambiguation_response() {
        let options = vec![
            VerbCandidate {
                verb_fqn: "session.load-galaxy".to_string(),
                description: "Load CBUs by apex".to_string(),
                score: 0.85,
                example: None,
                domain: Some("session".to_string()),
            },
            VerbCandidate {
                verb_fqn: "session.load-cbu".to_string(),
                description: "Load single CBU".to_string(),
                score: 0.82,
                example: None,
                domain: Some("session".to_string()),
            },
        ];

        let response = ReplResponse::verb_disambiguation(options, "load".to_string(), 0.03);

        assert!(response.message.contains("2 possible actions"));
        assert!(matches!(
            response.kind,
            ReplResponseKind::VerbDisambiguation { .. }
        ));
    }

    #[test]
    fn test_error_response() {
        let response = ReplResponse::error("Something went wrong", true);

        assert_eq!(response.message, "Something went wrong");
        assert!(matches!(
            response.kind,
            ReplResponseKind::Error {
                recoverable: true,
                ..
            }
        ));
    }
}
