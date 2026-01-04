//! Taxonomy Navigation API Routes
//!
//! Provides endpoints for fractal taxonomy navigation using TaxonomyStack.
//! These endpoints are used by the egui UI to:
//! - Zoom into a type (push a new frame onto the stack)
//! - Zoom out (pop the current frame)
//! - Jump to a breadcrumb level (back-to)
//! - Get current breadcrumbs
//!
//! The taxonomy stack is session-scoped, stored in UnifiedSessionContext.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::SessionStore;

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct TaxonomyState {
    pub pool: PgPool,
    pub sessions: SessionStore,
}

impl TaxonomyState {
    pub fn new(pool: PgPool, sessions: SessionStore) -> Self {
        Self { pool, sessions }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Request to zoom into a type
#[derive(Debug, Deserialize)]
pub struct ZoomInRequest {
    /// Type code to zoom into
    pub type_code: String,
}

/// Request to jump to a breadcrumb level
#[derive(Debug, Deserialize)]
pub struct BackToRequest {
    /// Index of the breadcrumb level to jump to (0-based)
    pub level_index: usize,
}

/// A single breadcrumb in the navigation trail
#[derive(Debug, Clone, Serialize)]
pub struct Breadcrumb {
    /// Display label for this level
    pub label: String,
    /// Type code for this level
    pub type_code: String,
    /// Index in the breadcrumb trail
    pub index: usize,
}

/// Response containing current breadcrumbs
#[derive(Debug, Serialize)]
pub struct BreadcrumbsResponse {
    /// List of breadcrumbs from root to current level
    pub breadcrumbs: Vec<Breadcrumb>,
    /// Current depth (0 = root)
    pub depth: usize,
    /// Whether we can zoom out
    pub can_zoom_out: bool,
}

/// Response for zoom operations
#[derive(Debug, Serialize)]
pub struct ZoomResponse {
    /// Whether the operation succeeded
    pub success: bool,
    /// Updated breadcrumbs after the operation
    pub breadcrumbs: Vec<Breadcrumb>,
    /// Current depth after operation
    pub depth: usize,
    /// Error message if operation failed
    pub error: Option<String>,
}

// ============================================================================
// Router
// ============================================================================

pub fn create_taxonomy_router(pool: PgPool, sessions: SessionStore) -> Router {
    let state = TaxonomyState::new(pool, sessions);

    Router::new()
        // Get current breadcrumbs for a session
        .route(
            "/api/session/:session_id/taxonomy/breadcrumbs",
            get(get_breadcrumbs),
        )
        // Zoom into a type
        .route("/api/session/:session_id/taxonomy/zoom-in", post(zoom_in))
        // Zoom out one level
        .route("/api/session/:session_id/taxonomy/zoom-out", post(zoom_out))
        // Jump to a breadcrumb level
        .route("/api/session/:session_id/taxonomy/back-to", post(back_to))
        // Reset to root level
        .route("/api/session/:session_id/taxonomy/reset", post(reset))
        .with_state(state)
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/session/:session_id/taxonomy/breadcrumbs
///
/// Get the current taxonomy navigation breadcrumbs for a session.
async fn get_breadcrumbs(
    State(state): State<TaxonomyState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<BreadcrumbsResponse>, (StatusCode, String)> {
    let sessions = state.sessions.read().await;

    let session = sessions.get(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Session {} not found", session_id),
        )
    })?;

    // Get breadcrumbs from the session's taxonomy stack
    let breadcrumbs = session
        .context
        .taxonomy_stack
        .breadcrumbs_with_codes()
        .iter()
        .enumerate()
        .map(|(i, (label, type_code))| Breadcrumb {
            label: label.clone(),
            type_code: type_code.clone(),
            index: i,
        })
        .collect::<Vec<_>>();

    let depth = session.context.taxonomy_stack.depth();

    Ok(Json(BreadcrumbsResponse {
        breadcrumbs,
        depth,
        can_zoom_out: depth > 0,
    }))
}

/// POST /api/session/:session_id/taxonomy/zoom-in
///
/// Zoom into a type, pushing a new frame onto the taxonomy stack.
async fn zoom_in(
    State(state): State<TaxonomyState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<ZoomInRequest>,
) -> Result<Json<ZoomResponse>, (StatusCode, String)> {
    let mut sessions = state.sessions.write().await;

    let session = sessions.get_mut(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Session {} not found", session_id),
        )
    })?;

    // Push a loading frame with the type code as label
    // In a full implementation, this would use the TaxonomyParser to expand the type
    let frame = crate::taxonomy::TaxonomyFrame::loading(
        Uuid::new_v4(), // Focus node ID (placeholder)
        request.type_code.clone(),
    );

    if let Err(e) = session.context.taxonomy_stack.push(frame) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Failed to push taxonomy frame: {}", e),
        ));
    }

    // Get updated breadcrumbs
    let breadcrumbs = session
        .context
        .taxonomy_stack
        .breadcrumbs_with_codes()
        .iter()
        .enumerate()
        .map(|(i, (label, type_code))| Breadcrumb {
            label: label.clone(),
            type_code: type_code.clone(),
            index: i,
        })
        .collect();

    let depth = session.context.taxonomy_stack.depth();

    Ok(Json(ZoomResponse {
        success: true,
        breadcrumbs,
        depth,
        error: None,
    }))
}

/// POST /api/session/:session_id/taxonomy/zoom-out
///
/// Zoom out one level by popping the current frame from the stack.
async fn zoom_out(
    State(state): State<TaxonomyState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ZoomResponse>, (StatusCode, String)> {
    let mut sessions = state.sessions.write().await;

    let session = sessions.get_mut(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Session {} not found", session_id),
        )
    })?;

    // Pop the current frame
    let popped = session.context.taxonomy_stack.pop();

    if popped.is_none() {
        return Ok(Json(ZoomResponse {
            success: false,
            breadcrumbs: Vec::new(),
            depth: 0,
            error: Some("Already at root level".to_string()),
        }));
    }

    // Get updated breadcrumbs
    let breadcrumbs = session
        .context
        .taxonomy_stack
        .breadcrumbs_with_codes()
        .iter()
        .enumerate()
        .map(|(i, (label, type_code))| Breadcrumb {
            label: label.clone(),
            type_code: type_code.clone(),
            index: i,
        })
        .collect();

    let depth = session.context.taxonomy_stack.depth();

    Ok(Json(ZoomResponse {
        success: true,
        breadcrumbs,
        depth,
        error: None,
    }))
}

/// POST /api/session/:session_id/taxonomy/back-to
///
/// Jump to a specific breadcrumb level, popping all frames above it.
async fn back_to(
    State(state): State<TaxonomyState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<BackToRequest>,
) -> Result<Json<ZoomResponse>, (StatusCode, String)> {
    let mut sessions = state.sessions.write().await;

    let session = sessions.get_mut(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Session {} not found", session_id),
        )
    })?;

    // Pop frames until we reach the target level
    let current_depth = session.context.taxonomy_stack.depth();
    if request.level_index >= current_depth {
        return Ok(Json(ZoomResponse {
            success: false,
            breadcrumbs: Vec::new(),
            depth: current_depth,
            error: Some(format!(
                "Level {} is at or beyond current depth {}",
                request.level_index, current_depth
            )),
        }));
    }

    // Pop frames until we reach target level
    // level_index 0 means go to root (pop all frames)
    // level_index 1 means keep 1 frame, pop the rest
    let frames_to_pop = current_depth - request.level_index - 1;
    for _ in 0..frames_to_pop {
        session.context.taxonomy_stack.pop();
    }

    // Get updated breadcrumbs
    let breadcrumbs = session
        .context
        .taxonomy_stack
        .breadcrumbs_with_codes()
        .iter()
        .enumerate()
        .map(|(i, (label, type_code))| Breadcrumb {
            label: label.clone(),
            type_code: type_code.clone(),
            index: i,
        })
        .collect();

    let depth = session.context.taxonomy_stack.depth();

    Ok(Json(ZoomResponse {
        success: true,
        breadcrumbs,
        depth,
        error: None,
    }))
}

/// POST /api/session/:session_id/taxonomy/reset
///
/// Reset taxonomy navigation to root level by clearing the stack.
async fn reset(
    State(state): State<TaxonomyState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ZoomResponse>, (StatusCode, String)> {
    let mut sessions = state.sessions.write().await;

    let session = sessions.get_mut(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Session {} not found", session_id),
        )
    })?;

    // Clear the taxonomy stack
    session.context.taxonomy_stack.clear();

    Ok(Json(ZoomResponse {
        success: true,
        breadcrumbs: Vec::new(),
        depth: 0,
        error: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breadcrumb_serialization() {
        let breadcrumb = Breadcrumb {
            label: "Entity".to_string(),
            type_code: "ENTITY".to_string(),
            index: 0,
        };

        let json = serde_json::to_string(&breadcrumb).unwrap();
        assert!(json.contains("Entity"));
        assert!(json.contains("ENTITY"));
    }
}
