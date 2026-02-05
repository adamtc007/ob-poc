//! REPL API Routes
//!
//! Unified API endpoints for the new REPL state machine.
//! Single `/input` endpoint handles all user interactions.
//!
//! ## Endpoints
//!
//! - `POST /api/repl/session` - Create a new session
//! - `GET /api/repl/session/:id` - Get session state
//! - `POST /api/repl/session/:id/input` - Send input to session
//! - `DELETE /api/repl/session/:id` - Delete session
//!
//! ## Integration
//!
//! These routes are designed to coexist with the existing agent_routes.
//! To use them, add the ReplOrchestrator to ReplState and merge the router.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::repl::{
    DerivedState, LedgerEntry, ReplCommand, ReplOrchestrator, ReplResponse, ReplSession, ReplState,
    UserInput,
};

/// State for REPL routes
#[derive(Clone)]
pub struct ReplRouteState {
    pub orchestrator: Arc<ReplOrchestrator>,
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Request to send input to a session
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputRequest {
    /// Natural language message
    Message { content: String },

    /// User selected a verb from disambiguation options
    VerbSelection {
        option_index: usize,
        selected_verb: String,
        original_input: String,
    },

    /// User selected a scope/client group
    ScopeSelection {
        option_id: String,
        option_name: String,
    },

    /// User selected an entity to resolve a reference
    EntitySelection {
        ref_id: String,
        entity_id: Uuid,
        entity_name: String,
    },

    /// User confirmed or rejected an action
    Confirmation { confirmed: bool },

    /// User selected an intent tier option
    IntentTierSelection { tier: u32, selected_id: String },

    /// User selected a client group
    ClientGroupSelection { group_id: Uuid, group_name: String },

    /// REPL command
    Command { command: String },
}

impl From<InputRequest> for UserInput {
    fn from(req: InputRequest) -> Self {
        match req {
            InputRequest::Message { content } => UserInput::Message { content },
            InputRequest::VerbSelection {
                option_index,
                selected_verb,
                original_input,
            } => UserInput::VerbSelection {
                option_index,
                selected_verb,
                original_input,
            },
            InputRequest::ScopeSelection {
                option_id,
                option_name,
            } => UserInput::ScopeSelection {
                option_id,
                option_name,
            },
            InputRequest::EntitySelection {
                ref_id,
                entity_id,
                entity_name,
            } => UserInput::EntitySelection {
                ref_id,
                entity_id,
                entity_name,
            },
            InputRequest::Confirmation { confirmed } => UserInput::Confirmation { confirmed },
            InputRequest::IntentTierSelection { tier, selected_id } => {
                UserInput::IntentTierSelection { tier, selected_id }
            }
            InputRequest::ClientGroupSelection {
                group_id,
                group_name,
            } => UserInput::ClientGroupSelection {
                group_id,
                group_name,
            },
            InputRequest::Command { command } => {
                let cmd = match command.to_lowercase().as_str() {
                    "run" => ReplCommand::Run,
                    "undo" => ReplCommand::Undo,
                    "redo" => ReplCommand::Redo,
                    "clear" => ReplCommand::Clear,
                    "cancel" => ReplCommand::Cancel,
                    "info" => ReplCommand::Info,
                    "help" => ReplCommand::Help,
                    _ => ReplCommand::Help, // Default to help for unknown commands
                };
                UserInput::Command { command: cmd }
            }
        }
    }
}

/// Response for session creation
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub session_id: Uuid,
    pub state: ReplState,
}

/// Response for session state query
#[derive(Debug, Serialize)]
pub struct SessionStateResponse {
    pub session_id: Uuid,
    pub state: ReplState,
    pub client_group_id: Option<Uuid>,
    pub client_group_name: Option<String>,
    pub derived: DerivedState,
    pub entry_count: usize,
    pub recent_entries: Vec<LedgerEntry>,
}

impl From<ReplSession> for SessionStateResponse {
    fn from(session: ReplSession) -> Self {
        let entry_count = session.entry_count();
        // Return last 20 entries
        let recent_entries: Vec<LedgerEntry> = session
            .ledger
            .iter()
            .rev()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        Self {
            session_id: session.id,
            state: session.state,
            client_group_id: session.client_group_id,
            client_group_name: session.client_group_name,
            derived: session.derived,
            entry_count,
            recent_entries,
        }
    }
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub recoverable: bool,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// POST /api/repl/session - Create a new session
async fn create_session(
    State(state): State<ReplRouteState>,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    let session = state.orchestrator.create_session().await;

    Ok(Json(CreateSessionResponse {
        session_id: session.id,
        state: session.state,
    }))
}

/// GET /api/repl/session/:id - Get session state
async fn get_session(
    State(state): State<ReplRouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, StatusCode> {
    let session = state
        .orchestrator
        .get_session(session_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(SessionStateResponse::from(session)))
}

/// POST /api/repl/session/:id/input - Send input to session
async fn handle_input(
    State(state): State<ReplRouteState>,
    Path(session_id): Path<Uuid>,
    Json(input): Json<InputRequest>,
) -> Result<Json<ReplResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user_input: UserInput = input.into();

    match state.orchestrator.process(session_id, user_input).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            tracing::error!("REPL processing error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                    recoverable: true,
                }),
            ))
        }
    }
}

/// DELETE /api/repl/session/:id - Delete session
async fn delete_session(
    State(state): State<ReplRouteState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    // Check if session exists
    if state.orchestrator.get_session(session_id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // TODO: Add delete method to orchestrator
    // For now, just return OK (session will be cleaned up by GC)
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Router
// ============================================================================

/// Create the REPL router
///
/// This router uses `ReplRouteState` as its state type. To integrate with
/// the main application, use `.with_state(repl_state)` when nesting:
///
/// ```ignore
/// let repl_state = ReplRouteState { orchestrator: Arc::new(orchestrator) };
/// let app = Router::new()
///     .nest("/api/repl", repl_routes::router().with_state(repl_state))
///     // ... other routes
/// ```
pub fn router() -> Router<ReplRouteState> {
    Router::new()
        .route("/session", post(create_session))
        .route("/session/{id}", get(get_session).delete(delete_session))
        .route("/session/{id}/input", post(handle_input))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_request_parsing() {
        // Test message
        let json = r#"{"type": "message", "content": "hello"}"#;
        let req: InputRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequest::Message { content } if content == "hello"));

        // Test command
        let json = r#"{"type": "command", "command": "run"}"#;
        let req: InputRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequest::Command { .. }));

        // Test verb selection
        let json = r#"{"type": "verb_selection", "option_index": 0, "selected_verb": "cbu.create", "original_input": "create"}"#;
        let req: InputRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, InputRequest::VerbSelection { .. }));
    }

    #[test]
    fn test_command_conversion() {
        let req = InputRequest::Command {
            command: "RUN".to_string(),
        };
        let input: UserInput = req.into();
        assert!(matches!(
            input,
            UserInput::Command {
                command: ReplCommand::Run
            }
        ));

        let req = InputRequest::Command {
            command: "unknown".to_string(),
        };
        let input: UserInput = req.into();
        assert!(matches!(
            input,
            UserInput::Command {
                command: ReplCommand::Help
            }
        ));
    }
}
