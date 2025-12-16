//! Resolution API Routes
//!
//! Provides endpoints for entity reference resolution workflow.
//!
//! ## Endpoints
//!
//! | Endpoint | Method | Description |
//! |----------|--------|-------------|
//! | `/api/session/:id/resolution/start` | POST | Start resolution session |
//! | `/api/session/:id/resolution` | GET | Get current resolution state |
//! | `/api/session/:id/resolution/search` | POST | Search for entity matches |
//! | `/api/session/:id/resolution/select` | POST | Select a resolution |
//! | `/api/session/:id/resolution/confirm` | POST | Confirm a resolution |
//! | `/api/session/:id/resolution/confirm-all` | POST | Confirm all high-confidence |
//! | `/api/session/:id/resolution/commit` | POST | Commit resolutions to AST |
//! | `/api/session/:id/resolution/cancel` | POST | Cancel resolution session |

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use ob_poc_types::resolution::*;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::session::SessionStore;
use crate::services::resolution_service::{
    EntityMatchInternal, ResolutionService, ResolutionStore,
};

/// Shared state for resolution routes
#[derive(Clone)]
pub struct ResolutionState {
    pub session_store: SessionStore,
    pub resolution_store: ResolutionStore,
    pub resolution_service: Arc<ResolutionService>,
}

impl ResolutionState {
    pub fn new(session_store: SessionStore, resolution_store: ResolutionStore) -> Self {
        let resolution_service = Arc::new(ResolutionService::new(resolution_store.clone()));
        Self {
            session_store,
            resolution_store,
            resolution_service,
        }
    }
}

// ============================================================================
// ROUTE HANDLERS
// ============================================================================

/// POST /api/session/:session_id/resolution/start
///
/// Start a resolution session for the given session's current DSL.
/// Extracts unresolved EntityRefs and prepares them for resolution.
pub async fn start_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(_body): Json<StartResolutionRequest>,
) -> Result<Json<ResolutionSessionResponse>, (StatusCode, String)> {
    // Get the session
    let sessions = state.session_store.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    // Get the AST from the session context
    let ast = session.context.ast.clone();
    drop(sessions);

    // Start resolution
    let resolution_session = state
        .resolution_service
        .start_resolution(session_id, &ast)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(resolution_session.to_response()))
}

/// GET /api/session/:session_id/resolution
///
/// Get the current resolution session state.
pub async fn get_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ResolutionSessionResponse>, (StatusCode, String)> {
    // Find the resolution session for this session
    let store = state.resolution_store.read().await;

    // Find resolution session by session_id
    let resolution_session = store
        .values()
        .find(|r| r.session_id == session_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            )
        })?;

    Ok(Json(resolution_session.to_response()))
}

/// POST /api/session/:session_id/resolution/search
///
/// Search for entity matches for a specific unresolved reference.
pub async fn search_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ResolutionSearchRequest>,
) -> Result<Json<ResolutionSearchResponse>, (StatusCode, String)> {
    // Find the resolution session
    let store = state.resolution_store.read().await;
    let resolution_session = store
        .values()
        .find(|r| r.session_id == session_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            )
        })?;
    let resolution_id = resolution_session.id;
    drop(store);

    // Perform search
    let matches = state
        .resolution_service
        .search(
            resolution_id,
            &body.ref_id,
            &body.query,
            &body.discriminators,
            body.limit,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ResolutionSearchResponse {
        matches: matches.iter().map(|m| m.into()).collect(),
        total: matches.len(),
        truncated: false,
    }))
}

/// POST /api/session/:session_id/resolution/select
///
/// Select a resolution for a specific reference.
pub async fn select_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<SelectResolutionRequest>,
) -> Result<Json<SelectResolutionResponse>, (StatusCode, String)> {
    // Find the resolution session
    let store = state.resolution_store.read().await;
    let resolution_session = store
        .values()
        .find(|r| r.session_id == session_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            )
        })?;

    // Get the unresolved ref to get entity_type
    let unresolved = resolution_session
        .unresolved
        .iter()
        .find(|r| r.ref_id == body.ref_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Reference not found".to_string()))?;

    let entity_type = unresolved.entity_type.clone();
    let search_value = unresolved.search_value.clone();
    let resolution_id = resolution_session.id;
    drop(store);

    // Parse the resolved key
    let resolved_key = Uuid::parse_str(&body.resolved_key)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid UUID".to_string()))?;

    // Create the entity match (simplified - in production would fetch from EntityGateway)
    let entity_match = EntityMatchInternal {
        id: resolved_key,
        display: search_value.clone(),
        entity_type: entity_type.clone(),
        score: 1.0,
        discriminators: std::collections::HashMap::new(),
        status: EntityStatus::Active,
        context: None,
    };

    // Select the resolution
    let updated_session = state
        .resolution_service
        .select(resolution_id, &body.ref_id, entity_match)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(SelectResolutionResponse {
        success: true,
        session: updated_session.to_response(),
    }))
}

/// POST /api/session/:session_id/resolution/confirm
///
/// Confirm (mark as reviewed) a specific resolution.
pub async fn confirm_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ConfirmResolutionRequest>,
) -> Result<Json<ResolutionSessionResponse>, (StatusCode, String)> {
    // Find the resolution session
    let store = state.resolution_store.read().await;
    let resolution_session = store
        .values()
        .find(|r| r.session_id == session_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            )
        })?;
    let resolution_id = resolution_session.id;
    drop(store);

    // Confirm the resolution
    let updated_session = state
        .resolution_service
        .confirm(resolution_id, &body.ref_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(updated_session.to_response()))
}

/// POST /api/session/:session_id/resolution/confirm-all
///
/// Confirm all resolutions above a confidence threshold.
pub async fn confirm_all_resolutions(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ConfirmAllRequest>,
) -> Result<Json<ResolutionSessionResponse>, (StatusCode, String)> {
    // Find the resolution session
    let store = state.resolution_store.read().await;
    let resolution_session = store
        .values()
        .find(|r| r.session_id == session_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            )
        })?;
    let resolution_id = resolution_session.id;
    drop(store);

    // Confirm all
    let updated_session = state
        .resolution_service
        .confirm_all(resolution_id, body.min_confidence)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(updated_session.to_response()))
}

/// POST /api/session/:session_id/resolution/commit
///
/// Commit resolutions to the AST and enable execution.
pub async fn commit_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<CommitResolutionResponse>, (StatusCode, String)> {
    // Find the resolution session
    let store = state.resolution_store.read().await;
    let resolution_session = store
        .values()
        .find(|r| r.session_id == session_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            )
        })?;
    let resolution_id = resolution_session.id;
    drop(store);

    // Commit the resolutions
    let resolved_ast = state
        .resolution_service
        .commit(resolution_id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Cannot commit: {}", e)))?;

    // Update the session's AST with resolved refs
    let mut sessions = state.session_store.write().await;
    if let Some(session) = sessions.get_mut(&session_id) {
        // Clear existing AST and add resolved statements
        session.context.ast.clear();
        session.context.add_statements(resolved_ast.clone());

        // Generate DSL source from resolved AST
        let dsl_source = session.context.to_dsl_source();

        Ok(Json(CommitResolutionResponse {
            success: true,
            dsl_source: Some(dsl_source),
            message: "Resolutions committed to AST".to_string(),
            errors: vec![],
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Session not found".to_string()))
    }
}

/// POST /api/session/:session_id/resolution/cancel
///
/// Cancel the resolution session.
pub async fn cancel_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<CancelResolutionResponse>, (StatusCode, String)> {
    // Find the resolution session
    let store = state.resolution_store.read().await;
    let resolution_session = store
        .values()
        .find(|r| r.session_id == session_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            )
        })?;
    let resolution_id = resolution_session.id;
    drop(store);

    // Cancel
    state
        .resolution_service
        .cancel(resolution_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(CancelResolutionResponse {
        success: true,
        message: "Resolution session cancelled".to_string(),
    }))
}

// ============================================================================
// ROUTER
// ============================================================================

/// Create the resolution router
///
/// All routes are under /api/session/:session_id/resolution
pub fn create_resolution_router(
    session_store: SessionStore,
    resolution_store: ResolutionStore,
) -> Router {
    let state = ResolutionState::new(session_store, resolution_store);

    Router::new()
        .route(
            "/api/session/:session_id/resolution/start",
            post(start_resolution),
        )
        .route("/api/session/:session_id/resolution", get(get_resolution))
        .route(
            "/api/session/:session_id/resolution/search",
            post(search_resolution),
        )
        .route(
            "/api/session/:session_id/resolution/select",
            post(select_resolution),
        )
        .route(
            "/api/session/:session_id/resolution/confirm",
            post(confirm_resolution),
        )
        .route(
            "/api/session/:session_id/resolution/confirm-all",
            post(confirm_all_resolutions),
        )
        .route(
            "/api/session/:session_id/resolution/commit",
            post(commit_resolution),
        )
        .route(
            "/api/session/:session_id/resolution/cancel",
            post(cancel_resolution),
        )
        .with_state(state)
}
