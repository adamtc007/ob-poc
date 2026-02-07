//! V2 REPL API Routes — Pack-Guided Runbook Pipeline
//!
//! Coexists with `/api/repl/*` (v1 state machine routes).
//! These endpoints wrap `ReplOrchestratorV2` which uses the pack-guided
//! 7-state machine with sentence-first responses.
//!
//! ## Endpoints
//!
//! - `POST /api/repl/v2/session`           — Create session
//! - `GET  /api/repl/v2/session/:id`       — Get session state
//! - `POST /api/repl/v2/session/:id/input` — Unified input
//! - `DELETE /api/repl/v2/session/:id`      — Delete session
//! - `POST /api/repl/v2/signal`            — External system signals completion of a parked entry

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::repl::orchestrator_v2::ReplOrchestratorV2;
use crate::repl::response_v2::ReplResponseV2;
use crate::repl::session_v2::ReplSessionV2;
use crate::repl::types_v2::{ReplCommandV2, ReplStateV2, UserInputV2};

// ============================================================================
// Route State
// ============================================================================

/// Shared state for V2 REPL routes.
#[derive(Clone)]
pub struct ReplV2RouteState {
    pub orchestrator: Arc<ReplOrchestratorV2>,
}

// ============================================================================
// Request / Response Types
// ============================================================================

/// Request to send input to a V2 session.
///
/// Maps to `UserInputV2` on the backend. Uses `#[serde(tag = "type")]`
/// so the client sends `{ "type": "message", "content": "..." }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputRequestV2 {
    /// Free-text conversational input.
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
    Command { command: String },

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

    /// User approves a human-gated runbook entry.
    Approve {
        entry_id: Uuid,
        approved_by: Option<String>,
    },

    /// User rejects a human-gated runbook entry.
    RejectGate {
        entry_id: Uuid,
        reason: Option<String>,
    },
}

impl From<InputRequestV2> for UserInputV2 {
    fn from(req: InputRequestV2) -> Self {
        match req {
            InputRequestV2::Message { content } => UserInputV2::Message { content },
            InputRequestV2::Confirm => UserInputV2::Confirm,
            InputRequestV2::Reject => UserInputV2::Reject,
            InputRequestV2::Edit {
                step_id,
                field,
                value,
            } => UserInputV2::Edit {
                step_id,
                field,
                value,
            },
            InputRequestV2::Command { command } => {
                let lower = command.to_lowercase();
                let cmd = match lower.as_str() {
                    "run" => ReplCommandV2::Run,
                    "undo" => ReplCommandV2::Undo,
                    "redo" => ReplCommandV2::Redo,
                    "clear" => ReplCommandV2::Clear,
                    "cancel" => ReplCommandV2::Cancel,
                    "info" => ReplCommandV2::Info,
                    "help" => ReplCommandV2::Help,
                    "status" => ReplCommandV2::Status,
                    _ if lower.starts_with("disable ") => {
                        let id = Uuid::parse_str(lower.trim_start_matches("disable ").trim())
                            .unwrap_or_default();
                        ReplCommandV2::Disable(id)
                    }
                    _ if lower.starts_with("enable ") => {
                        let id = Uuid::parse_str(lower.trim_start_matches("enable ").trim())
                            .unwrap_or_default();
                        ReplCommandV2::Enable(id)
                    }
                    _ if lower.starts_with("toggle ") => {
                        let id = Uuid::parse_str(lower.trim_start_matches("toggle ").trim())
                            .unwrap_or_default();
                        ReplCommandV2::Toggle(id)
                    }
                    _ => ReplCommandV2::Help,
                };
                UserInputV2::Command { command: cmd }
            }
            InputRequestV2::SelectPack { pack_id } => UserInputV2::SelectPack { pack_id },
            InputRequestV2::SelectProposal { proposal_id } => {
                UserInputV2::SelectProposal { proposal_id }
            }
            InputRequestV2::SelectVerb {
                verb_fqn,
                original_input,
            } => UserInputV2::SelectVerb {
                verb_fqn,
                original_input,
            },
            InputRequestV2::SelectEntity {
                ref_id,
                entity_id,
                entity_name,
            } => UserInputV2::SelectEntity {
                ref_id,
                entity_id,
                entity_name,
            },
            InputRequestV2::SelectScope {
                group_id,
                group_name,
            } => UserInputV2::SelectScope {
                group_id,
                group_name,
            },
            InputRequestV2::Approve {
                entry_id,
                approved_by,
            } => UserInputV2::Approve {
                entry_id,
                approved_by,
            },
            InputRequestV2::RejectGate { entry_id, reason } => {
                UserInputV2::RejectGate { entry_id, reason }
            }
        }
    }
}

/// Response for session creation.
#[derive(Debug, Serialize)]
pub struct CreateSessionResponseV2 {
    pub session_id: Uuid,
    pub state: ReplStateV2,
    pub greeting: String,
}

/// Response for session state query.
#[derive(Debug, Serialize)]
pub struct SessionStateResponseV2 {
    pub session_id: Uuid,
    pub state: ReplStateV2,
    pub runbook_step_count: usize,
    pub created_at: String,
    pub last_active_at: String,
}

impl From<ReplSessionV2> for SessionStateResponseV2 {
    fn from(session: ReplSessionV2) -> Self {
        Self {
            session_id: session.id,
            state: session.state,
            runbook_step_count: session.runbook.entries.len(),
            created_at: session.created_at.to_rfc3339(),
            last_active_at: session.last_active_at.to_rfc3339(),
        }
    }
}

/// Error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponseV2 {
    pub error: String,
    pub recoverable: bool,
}

/// Request body for the signal endpoint (webhook for external systems).
///
/// External systems (workflow engines, approval portals) POST to `/api/repl/v2/signal`
/// with the `correlation_key` that was provided when the entry was parked.
#[derive(Debug, Deserialize)]
pub struct ReplSignalRequest {
    /// The correlation key linking this signal to a parked runbook entry.
    pub correlation_key: String,
    /// Signal status: `"completed"` or `"failed"`.
    pub status: String,
    /// Result payload (when status is `"completed"`).
    pub result: Option<serde_json::Value>,
    /// Error description (when status is `"failed"`).
    pub error: Option<String>,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// POST /api/repl/v2/session — Create a new V2 session.
async fn create_session_v2(
    State(state): State<ReplV2RouteState>,
) -> Result<Json<CreateSessionResponseV2>, StatusCode> {
    let session_id = state.orchestrator.create_session().await;

    let greeting = crate::repl::bootstrap::format_greeting();

    // Push the greeting as an assistant message into session history.
    {
        let sessions = state.orchestrator.sessions_for_test();
        let mut sessions_write = sessions.write().await;
        if let Some(session) = sessions_write.get_mut(&session_id) {
            session.push_message(
                crate::repl::session_v2::MessageRole::Assistant,
                greeting.clone(),
            );
        }
    }

    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CreateSessionResponseV2 {
        session_id,
        state: session.state,
        greeting,
    }))
}

/// GET /api/repl/v2/session/:id — Get V2 session state.
async fn get_session_v2(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponseV2>, StatusCode> {
    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(SessionStateResponseV2::from(session)))
}

/// POST /api/repl/v2/session/:id/input — Unified input endpoint.
async fn input_v2(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
    Json(input): Json<InputRequestV2>,
) -> Result<Json<ReplResponseV2>, (StatusCode, Json<ErrorResponseV2>)> {
    let user_input: UserInputV2 = input.into();

    match state.orchestrator.process(session_id, user_input).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            tracing::error!("REPL V2 processing error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponseV2 {
                    error: e.to_string(),
                    recoverable: true,
                }),
            ))
        }
    }
}

/// DELETE /api/repl/v2/session/:id — Delete V2 session.
async fn delete_session_v2(
    State(state): State<ReplV2RouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    if !state.orchestrator.delete_session(session_id).await {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/repl/v2/signal — External system signals completion of a parked entry.
///
/// Workflow engines, approval portals, and other external systems call this
/// endpoint with the `correlation_key` that was provided when the entry was
/// parked. The handler locates the correct session, resumes the entry, and
/// either continues execution (on success) or marks the entry as failed.
///
/// This endpoint is idempotent: signalling an already-resumed entry returns
/// 200 with `"already_resumed"` status.
async fn signal_v2(
    State(state): State<ReplV2RouteState>,
    Json(req): Json<ReplSignalRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Validate signal status.
    if req.status != "completed" && req.status != "failed" {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Find the session that owns this correlation key by scanning all sessions.
    let sessions = state.orchestrator.sessions_for_test().read().await;
    let mut found_session_id = None;
    let mut found_entry_id = None;

    for (sid, session) in sessions.iter() {
        if let Some(entry_id) = session.runbook.invocation_index.get(&req.correlation_key) {
            found_session_id = Some(*sid);
            found_entry_id = Some(*entry_id);
            break;
        }
    }
    drop(sessions);

    let session_id = found_session_id.ok_or(StatusCode::NOT_FOUND)?;
    let entry_id = found_entry_id.ok_or(StatusCode::NOT_FOUND)?;

    // Determine the result payload to pass to resume_entry.
    let signal_result = match req.status.as_str() {
        "completed" => req.result.clone(),
        "failed" => Some(serde_json::json!({
            "error": req.error.clone().unwrap_or_default()
        })),
        _ => unreachable!(), // validated above
    };

    // Resume the parked entry in the runbook.
    {
        let mut sessions = state.orchestrator.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        let resumed = session
            .runbook
            .resume_entry(&req.correlation_key, signal_result);

        if resumed.is_none() {
            // Idempotent: already resumed.
            return Ok(Json(serde_json::json!({
                "status": "already_resumed",
                "session_id": session_id
            })));
        }

        // If the signal indicates failure, mark the entry as Failed and
        // transition to RunbookEditing so the user can fix or retry.
        if req.status == "failed" {
            if let Some(entry) = session
                .runbook
                .entries
                .iter_mut()
                .find(|e| e.id == entry_id)
            {
                entry.status = crate::repl::runbook::EntryStatus::Failed;
            }
            session
                .runbook
                .set_status(crate::repl::runbook::RunbookStatus::Ready);
            session.set_state(crate::repl::types_v2::ReplStateV2::RunbookEditing);

            return Ok(Json(serde_json::json!({
                "status": "failed",
                "session_id": session_id,
                "entry_id": entry_id,
                "error": req.error.unwrap_or_else(|| "External task failed".into())
            })));
        }
    }

    // For "completed" signals, continue execution from the next entry
    // by sending a Resume command through the orchestrator.
    let input = UserInputV2::Command {
        command: ReplCommandV2::Resume(entry_id),
    };
    match state.orchestrator.process(session_id, input).await {
        Ok(response) => Ok(Json(serde_json::to_value(response).unwrap_or_else(|_| {
            serde_json::json!({
                "status": "completed",
                "session_id": session_id
            })
        }))),
        Err(e) => {
            tracing::error!(
                "REPL V2 signal processing error for session {}: {}",
                session_id,
                e
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============================================================================
// Router
// ============================================================================

/// Create the V2 REPL router.
///
/// Mount under `/api/repl/v2`:
///
/// ```ignore
/// let v2_state = ReplV2RouteState { orchestrator: Arc::new(orch) };
/// let app = Router::new()
///     .nest("/api/repl/v2", repl_routes_v2::router().with_state(v2_state))
///     // ... other routes
/// ```
pub fn router() -> Router<ReplV2RouteState> {
    Router::new()
        .route("/session", post(create_session_v2))
        .route(
            "/session/{id}",
            get(get_session_v2).delete(delete_session_v2),
        )
        .route("/session/{id}/input", post(input_v2))
        .route("/signal", post(signal_v2))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_request_v2_message_parsing() {
        let json = r#"{"type": "message", "content": "create a fund"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::Message { content } if content == "create a fund"));
    }

    #[test]
    fn test_input_request_v2_confirm() {
        let json = r#"{"type": "confirm"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::Confirm));
    }

    #[test]
    fn test_input_request_v2_reject() {
        let json = r#"{"type": "reject"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::Reject));
    }

    #[test]
    fn test_input_request_v2_command_parsing() {
        let json = r#"{"type": "command", "command": "run"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::Command {
                command: ReplCommandV2::Run
            }
        ));
    }

    #[test]
    fn test_input_request_v2_select_pack() {
        let json = r#"{"type": "select_pack", "pack_id": "onboarding-request"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(
            matches!(req, InputRequestV2::SelectPack { pack_id } if pack_id == "onboarding-request")
        );
    }

    #[test]
    fn test_input_request_v2_select_verb() {
        let json =
            r#"{"type": "select_verb", "verb_fqn": "cbu.create", "original_input": "create"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::SelectVerb { .. }));
    }

    #[test]
    fn test_input_request_v2_select_scope() {
        let json = r#"{"type": "select_scope", "group_id": "11111111-1111-1111-1111-111111111111", "group_name": "Allianz"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequestV2::SelectScope { .. }));
    }

    #[test]
    fn test_unknown_command_defaults_to_help() {
        let json = r#"{"type": "command", "command": "foobar"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::Command {
                command: ReplCommandV2::Help
            }
        ));
    }

    #[test]
    fn test_input_request_v2_approve() {
        let json = r#"{"type": "approve", "entry_id": "11111111-1111-1111-1111-111111111111", "approved_by": "admin@example.com"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(
            matches!(req, InputRequestV2::Approve { entry_id, approved_by }
                if entry_id == Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
                && approved_by == Some("admin@example.com".to_string())
            )
        );
    }

    #[test]
    fn test_input_request_v2_approve_without_approver() {
        let json = r#"{"type": "approve", "entry_id": "11111111-1111-1111-1111-111111111111"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::Approve {
                approved_by: None,
                ..
            }
        ));
    }

    #[test]
    fn test_input_request_v2_reject_gate() {
        let json = r#"{"type": "reject_gate", "entry_id": "22222222-2222-2222-2222-222222222222", "reason": "Insufficient documentation"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        assert!(
            matches!(req, InputRequestV2::RejectGate { entry_id, reason }
                if entry_id == Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()
                && reason == Some("Insufficient documentation".to_string())
            )
        );
    }

    #[test]
    fn test_input_request_v2_reject_gate_without_reason() {
        let json = r#"{"type": "reject_gate", "entry_id": "22222222-2222-2222-2222-222222222222"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::RejectGate { reason: None, .. }
        ));
    }

    #[test]
    fn test_command_status_parsing() {
        let json = r#"{"type": "command", "command": "status"}"#;
        let req: InputRequestV2 = serde_json::from_str(json).unwrap();
        let input: UserInputV2 = req.into();
        assert!(matches!(
            input,
            UserInputV2::Command {
                command: ReplCommandV2::Status
            }
        ));
    }

    #[test]
    fn test_signal_request_deserialization_completed() {
        let json =
            r#"{"correlation_key": "abc:def", "status": "completed", "result": {"doc_id": "123"}}"#;
        let req: ReplSignalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.correlation_key, "abc:def");
        assert_eq!(req.status, "completed");
        assert!(req.result.is_some());
        assert!(req.error.is_none());
    }

    #[test]
    fn test_signal_request_deserialization_failed() {
        let json = r#"{"correlation_key": "abc:def", "status": "failed", "error": "Timeout"}"#;
        let req: ReplSignalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.correlation_key, "abc:def");
        assert_eq!(req.status, "failed");
        assert!(req.result.is_none());
        assert_eq!(req.error, Some("Timeout".to_string()));
    }

    #[test]
    fn test_signal_request_deserialization_minimal() {
        let json = r#"{"correlation_key": "abc:def", "status": "completed"}"#;
        let req: ReplSignalRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.correlation_key, "abc:def");
        assert_eq!(req.status, "completed");
        assert!(req.result.is_none());
        assert!(req.error.is_none());
    }
}
