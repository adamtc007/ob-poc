//! Confirmation state machine — drives the user through a parameter-review
//! interaction before DSL emission.
//!
//! # State transitions
//!
//! ```text
//! Pending ──Accept──────────────────────► Accepted  (terminal)
//! Pending ──EditParameter───────────────► Pending   (loop — stays editable)
//! Pending ──RejectPack──────────────────► Rejected  (terminal)
//! Pending ──Cancel──────────────────────► Cancelled (terminal)
//! ```
//!
//! [`ConfirmationSession`] is cheap to clone and can be persisted as part of a
//! REPL session.  It records every parameter edit in [`edit_history`] so that
//! provenance is preserved for audit purposes.
//!
//! # Usage
//!
//! ```rust,ignore
//! use dsl_sage::{ConfirmationRequest, ConfirmationResponse, ConfirmationSession, ConfirmationState};
//!
//! let mut session = ConfirmationSession::new(request);
//!
//! // User edits a parameter
//! session.apply_response(ConfirmationResponse::EditParameter {
//!     name: "gate-name".into(),
//!     new_value: serde_json::json!("my-gate"),
//! });
//!
//! // User accepts
//! session.apply_response(ConfirmationResponse::Accept);
//!
//! let params = session.confirmed_parameters().unwrap(); // Some(HashMap)
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::{ConfirmationRequest, ConfirmationResponse, ParameterProposal};

// ---------------------------------------------------------------------------
// State enum
// ---------------------------------------------------------------------------

/// Current state of a [`ConfirmationSession`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationState {
    /// Awaiting user decision (initial state).
    Pending,
    /// User accepted all proposed parameters — proceed to DSL emission.
    Accepted,
    /// User rejected this pack — return to pack-matching.
    Rejected,
    /// User cancelled the entire authoring flow.
    Cancelled,
}

// ---------------------------------------------------------------------------
// Edit record
// ---------------------------------------------------------------------------

/// Record of a single user-driven parameter edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterEdit {
    pub parameter_name: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
    /// RFC 3339 timestamp of the edit.
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// Stateful confirmation interaction for a single [`ConfirmationRequest`].
///
/// Create with [`ConfirmationSession::new`], drive with
/// [`ConfirmationSession::apply_response`], poll with
/// [`ConfirmationSession::state`] and [`ConfirmationSession::is_terminal`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfirmationSession {
    /// Stable identifier for this confirmation session.
    pub session_id: String,
    /// The live confirmation request (proposed parameters may be mutated by edits).
    pub request: ConfirmationRequest,
    /// Current state of the session.
    pub state: ConfirmationState,
    /// Ordered log of every parameter edit made before acceptance.
    pub edit_history: Vec<ParameterEdit>,
}

impl ConfirmationSession {
    /// Start a new confirmation session for the given request.
    pub fn new(request: ConfirmationRequest) -> Self {
        Self {
            session_id: make_session_id(),
            request,
            state: ConfirmationState::Pending,
            edit_history: vec![],
        }
    }

    /// Apply a user response and return the resulting state.
    ///
    /// Transitions:
    /// - [`ConfirmationResponse::Accept`] → [`ConfirmationState::Accepted`]
    /// - [`ConfirmationResponse::EditParameter`] → stays [`ConfirmationState::Pending`]
    /// - [`ConfirmationResponse::RejectPack`] → [`ConfirmationState::Rejected`]
    /// - [`ConfirmationResponse::Cancel`] → [`ConfirmationState::Cancelled`]
    ///
    /// Responses applied to a terminal state are ignored.
    pub fn apply_response(&mut self, response: ConfirmationResponse) -> ConfirmationState {
        if self.is_terminal() {
            return self.state.clone();
        }

        match response {
            ConfirmationResponse::Accept => {
                self.state = ConfirmationState::Accepted;
            }
            ConfirmationResponse::EditParameter { name, new_value } => {
                if let Some(prop) = self
                    .request
                    .proposed_parameters
                    .iter_mut()
                    .find(|p| p.parameter_name == name)
                {
                    self.edit_history.push(ParameterEdit {
                        parameter_name: name.clone(),
                        old_value: prop.proposed_value.clone(),
                        new_value: new_value.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });
                    prop.proposed_value = new_value;
                    // Mark as user-set: full confidence, clear rationale.
                    prop.confidence = 1.0;
                    prop.rationale = "User-edited".to_string();
                }
                // Stay Pending — user may make further edits.
            }
            ConfirmationResponse::RejectPack => {
                self.state = ConfirmationState::Rejected;
            }
            ConfirmationResponse::Cancel => {
                self.state = ConfirmationState::Cancelled;
            }
        }

        self.state.clone()
    }

    /// `true` when the session has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            ConfirmationState::Accepted
                | ConfirmationState::Rejected
                | ConfirmationState::Cancelled
        )
    }

    /// Return the confirmed parameter map if the session is in `Accepted` state,
    /// otherwise `None`.
    pub fn confirmed_parameters(&self) -> Option<HashMap<String, serde_json::Value>> {
        if self.state == ConfirmationState::Accepted {
            Some(
                self.request
                    .proposed_parameters
                    .iter()
                    .map(|p| (p.parameter_name.clone(), p.proposed_value.clone()))
                    .collect(),
            )
        } else {
            None
        }
    }

    /// Convenience accessor for the current proposed parameters.
    pub fn proposed_parameters(&self) -> &[ParameterProposal] {
        &self.request.proposed_parameters
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn make_session_id() -> String {
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    format!("sess-{}", &id[..16])
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ConfirmationRequest, ParameterProposal};

    fn make_request() -> ConfirmationRequest {
        ConfirmationRequest {
            pack_name: "conjunctive-gate".to_string(),
            pack_version: "1.0.0".to_string(),
            proposed_parameters: vec![ParameterProposal {
                parameter_name: "gate-name".to_string(),
                proposed_value: serde_json::json!("initial-gate"),
                confidence: 0.8,
                rationale: "test".to_string(),
                source_phrase: None,
            }],
            preview_dsl: String::new(),
        }
    }

    #[test]
    fn initial_state_is_pending() {
        let session = ConfirmationSession::new(make_request());
        assert_eq!(session.state, ConfirmationState::Pending);
        assert!(!session.is_terminal());
    }

    #[test]
    fn accept_transitions_to_accepted() {
        let mut session = ConfirmationSession::new(make_request());
        let state = session.apply_response(ConfirmationResponse::Accept);
        assert_eq!(state, ConfirmationState::Accepted);
        assert!(session.is_terminal());
        assert!(session.confirmed_parameters().is_some());
    }

    #[test]
    fn reject_transitions_to_rejected() {
        let mut session = ConfirmationSession::new(make_request());
        let state = session.apply_response(ConfirmationResponse::RejectPack);
        assert_eq!(state, ConfirmationState::Rejected);
        assert!(session.confirmed_parameters().is_none());
    }

    #[test]
    fn cancel_transitions_to_cancelled() {
        let mut session = ConfirmationSession::new(make_request());
        let state = session.apply_response(ConfirmationResponse::Cancel);
        assert_eq!(state, ConfirmationState::Cancelled);
        assert!(session.confirmed_parameters().is_none());
    }

    #[test]
    fn edit_stays_pending_and_records_history() {
        let mut session = ConfirmationSession::new(make_request());
        let state = session.apply_response(ConfirmationResponse::EditParameter {
            name: "gate-name".to_string(),
            new_value: serde_json::json!("my-custom-gate"),
        });
        assert_eq!(state, ConfirmationState::Pending);
        assert!(!session.is_terminal());
        assert_eq!(session.edit_history.len(), 1);
        assert_eq!(
            session.edit_history[0].old_value,
            serde_json::json!("initial-gate")
        );
        assert_eq!(
            session.edit_history[0].new_value,
            serde_json::json!("my-custom-gate")
        );
    }

    #[test]
    fn edit_then_accept_returns_edited_value() {
        let mut session = ConfirmationSession::new(make_request());
        session.apply_response(ConfirmationResponse::EditParameter {
            name: "gate-name".to_string(),
            new_value: serde_json::json!("my-custom-gate"),
        });
        session.apply_response(ConfirmationResponse::Accept);
        let params = session.confirmed_parameters().unwrap();
        assert_eq!(
            params["gate-name"],
            serde_json::Value::String("my-custom-gate".to_string())
        );
    }

    #[test]
    fn responses_ignored_after_terminal() {
        let mut session = ConfirmationSession::new(make_request());
        session.apply_response(ConfirmationResponse::Accept);
        // Applying Cancel after Accept should not change the state.
        let state = session.apply_response(ConfirmationResponse::Cancel);
        assert_eq!(state, ConfirmationState::Accepted);
    }

    #[test]
    fn session_id_is_deterministic_prefix() {
        let session = ConfirmationSession::new(make_request());
        assert!(
            session.session_id.starts_with("sess-"),
            "session_id should start with 'sess-', got: {}",
            session.session_id
        );
    }
}
