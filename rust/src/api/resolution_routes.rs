//! Resolution API Routes
//!
//! Provides endpoints for entity reference resolution workflow.
//! Uses ResolutionSubSession in session state directly - no separate service.
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
use std::collections::HashMap;
use uuid::Uuid;

use crate::api::session::{
    EntityMatchInfo, ResolutionSubSession, SessionStore, SubSessionType, UnresolvedRefInfo,
};
use crate::dsl_v2::gateway_resolver::GatewayRefResolver;
use crate::dsl_v2::semantic_validator::entity_type_to_ref_type;

/// Shared state for resolution routes
#[derive(Clone)]
pub struct ResolutionState {
    pub session_store: SessionStore,
    pub gateway: GatewayRefResolver,
}

impl ResolutionState {
    pub fn new(session_store: SessionStore, gateway: GatewayRefResolver) -> Self {
        Self {
            session_store,
            gateway,
        }
    }
}

// ============================================================================
// HELPER CONVERSIONS
// ============================================================================

/// Convert internal UnresolvedRefInfo to API response
fn unresolved_ref_to_response(
    r: &UnresolvedRefInfo,
    resolutions: &HashMap<String, String>,
) -> UnresolvedRefResponse {
    UnresolvedRefResponse {
        ref_id: r.ref_id.clone(),
        entity_type: r.entity_type.clone(),
        entity_subtype: None,
        search_value: r.search_value.clone(),
        context: RefContext {
            statement_index: 0, // Could parse from ref_id if needed
            verb: String::new(),
            arg_name: String::new(),
            dsl_snippet: Some(r.context_line.clone()),
        },
        initial_matches: r.initial_matches.iter().map(match_to_response).collect(),
        agent_suggestion: None,
        suggestion_reason: None,
        review_requirement: if resolutions.contains_key(&r.ref_id) {
            ReviewRequirement::Optional
        } else {
            ReviewRequirement::Required
        },
        discriminator_fields: vec![],
    }
}

/// Convert internal EntityMatchInfo to API response
fn match_to_response(m: &EntityMatchInfo) -> EntityMatchResponse {
    EntityMatchResponse {
        id: m.value.clone(),
        display: m.display.clone(),
        entity_type: String::new(),
        score: m.score_pct as f32 / 100.0,
        discriminators: HashMap::new(),
        status: EntityStatus::Active,
        context: m.detail.clone(),
    }
}

/// Build session response from ResolutionSubSession
fn build_session_response(
    session_id: Uuid,
    resolution: &ResolutionSubSession,
) -> ResolutionSessionResponse {
    let (resolved_count, total_count) = resolution.progress();

    // Split refs into unresolved and resolved
    let unresolved: Vec<UnresolvedRefResponse> = resolution
        .unresolved_refs
        .iter()
        .filter(|r| !resolution.resolutions.contains_key(&r.ref_id))
        .map(|r| unresolved_ref_to_response(r, &resolution.resolutions))
        .collect();

    let resolved: Vec<ResolvedRefResponse> = resolution
        .resolutions
        .iter()
        .map(|(ref_id, resolved_key)| {
            let original = resolution
                .unresolved_refs
                .iter()
                .find(|r| &r.ref_id == ref_id);
            ResolvedRefResponse {
                ref_id: ref_id.clone(),
                entity_type: original.map(|r| r.entity_type.clone()).unwrap_or_default(),
                original_search: original.map(|r| r.search_value.clone()).unwrap_or_default(),
                resolved_key: resolved_key.clone(),
                display: resolved_key.clone(), // Could enhance with gateway lookup
                discriminators: HashMap::new(),
                entity_status: EntityStatus::Active,
                warnings: vec![],
                alternative_count: 0,
                confidence: 1.0,
                reviewed: true,
                changed_from_original: false,
                resolution_method: ResolutionMethod::UserSelected,
            }
        })
        .collect();

    let state = if resolution.is_complete() {
        ResolutionStateResponse::Reviewing
    } else {
        ResolutionStateResponse::Resolving
    };

    ResolutionSessionResponse {
        id: session_id.to_string(),
        resolution_id: session_id.to_string(),
        state,
        unresolved,
        auto_resolved: vec![],
        resolved,
        summary: ResolutionSummary {
            total_refs: total_count,
            resolved_count,
            warnings_count: 0,
            required_review_count: total_count - resolved_count,
            can_commit: resolution.is_complete(),
        },
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
    let mut sessions = state.session_store.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    // Extract unresolved refs from current AST
    let resolution = ResolutionSubSession::from_statements(&session.context.ast);

    // Check if there's anything to resolve
    if resolution.unresolved_refs.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No unresolved references in DSL".to_string(),
        ));
    }

    // Store in session as sub-session
    session.sub_session_type = SubSessionType::Resolution(resolution.clone());

    let response = build_session_response(session_id, &resolution);
    Ok(Json(response))
}

/// GET /api/session/:session_id/resolution
///
/// Get the current resolution session state.
pub async fn get_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ResolutionSessionResponse>, (StatusCode, String)> {
    let sessions = state.session_store.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    let resolution = match &session.sub_session_type {
        SubSessionType::Resolution(r) => r,
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            ))
        }
    };

    let response = build_session_response(session_id, resolution);
    Ok(Json(response))
}

/// POST /api/session/:session_id/resolution/search
///
/// Search for entity matches for a specific unresolved reference.
pub async fn search_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ResolutionSearchRequest>,
) -> Result<Json<ResolutionSearchResponse>, (StatusCode, String)> {
    // Get the session to verify resolution is active and get entity type
    let sessions = state.session_store.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    let resolution = match &session.sub_session_type {
        SubSessionType::Resolution(r) => r,
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            ))
        }
    };

    // Find the ref being searched
    let ref_info = resolution
        .unresolved_refs
        .iter()
        .find(|r| r.ref_id == body.ref_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Reference not found".to_string()))?;

    // Determine entity type for gateway search
    let entity_type = ref_info.entity_type.clone();
    drop(sessions);

    // Search via gateway
    let mut gateway = state.gateway.clone();
    let ref_type = entity_type_to_ref_type(&entity_type);
    let limit = body.limit.unwrap_or(10);

    let matches = gateway
        .search_fuzzy(ref_type, &body.query, limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let response_matches: Vec<EntityMatchResponse> = matches
        .iter()
        .map(|m| EntityMatchResponse {
            id: m.value.clone(),
            display: m.display.clone(),
            entity_type: entity_type.clone(),
            score: m.score,
            discriminators: HashMap::new(),
            status: EntityStatus::Active,
            context: None,
        })
        .collect();

    Ok(Json(ResolutionSearchResponse {
        matches: response_matches.clone(),
        total: response_matches.len(),
        truncated: false,
    }))
}

/// POST /api/session/:session_id/resolution/select
///
/// Select a resolution for a specific reference.
///
/// SECURITY: This endpoint validates the provided key is a valid UUID.
pub async fn select_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<SelectResolutionRequest>,
) -> Result<Json<SelectResolutionResponse>, (StatusCode, String)> {
    // SECURE: Validate the resolved_key is a valid UUID
    let _uuid = uuid::Uuid::parse_str(&body.resolved_key).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("resolved_key '{}' is not a valid UUID", body.resolved_key),
        )
    })?;

    let mut sessions = state.session_store.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    let resolution = match &mut session.sub_session_type {
        SubSessionType::Resolution(r) => r,
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            ))
        }
    };

    // Store the selection
    resolution
        .select(&body.ref_id, &body.resolved_key)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let response = build_session_response(session_id, resolution);
    Ok(Json(SelectResolutionResponse {
        success: true,
        session: response,
    }))
}

/// POST /api/session/:session_id/resolution/confirm
///
/// Confirm (mark as reviewed) a specific resolution.
/// In the new model, selection auto-confirms, so this is mostly for compatibility.
pub async fn confirm_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ConfirmResolutionRequest>,
) -> Result<Json<ResolutionSessionResponse>, (StatusCode, String)> {
    let sessions = state.session_store.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    let resolution = match &session.sub_session_type {
        SubSessionType::Resolution(r) => r,
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            ))
        }
    };

    // Verify the ref exists and is selected
    if !resolution.resolutions.contains_key(&body.ref_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Reference {} is not selected", body.ref_id),
        ));
    }

    // Already confirmed by virtue of being selected
    let response = build_session_response(session_id, resolution);
    Ok(Json(response))
}

/// POST /api/session/:session_id/resolution/confirm-all
///
/// Confirm all resolutions above a confidence threshold.
/// In the new model, this is a no-op since selection auto-confirms.
pub async fn confirm_all_resolutions(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(_body): Json<ConfirmAllRequest>,
) -> Result<Json<ResolutionSessionResponse>, (StatusCode, String)> {
    let sessions = state.session_store.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    let resolution = match &session.sub_session_type {
        SubSessionType::Resolution(r) => r,
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            ))
        }
    };

    let response = build_session_response(session_id, resolution);
    Ok(Json(response))
}

/// POST /api/session/:session_id/resolution/commit
///
/// Commit resolutions to the AST and enable execution.
pub async fn commit_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<CommitResolutionResponse>, (StatusCode, String)> {
    let mut sessions = state.session_store.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    let resolution = match &session.sub_session_type {
        SubSessionType::Resolution(r) => r.clone(),
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            ))
        }
    };

    // Check all refs are resolved
    if !resolution.is_complete() {
        let (resolved, total) = resolution.progress();
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot commit: only {}/{} references resolved",
                resolved, total
            ),
        ));
    }

    // Apply resolutions to AST
    resolution
        .apply_to_statements(&mut session.context.ast)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Generate DSL source from resolved AST
    let dsl_source = session.context.to_dsl_source();

    // Clear the sub-session
    session.sub_session_type = SubSessionType::Root;

    Ok(Json(CommitResolutionResponse {
        success: true,
        dsl_source: Some(dsl_source),
        message: "Resolutions committed to AST".to_string(),
        errors: vec![],
    }))
}

/// POST /api/session/:session_id/resolution/cancel
///
/// Cancel the resolution session.
pub async fn cancel_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<CancelResolutionResponse>, (StatusCode, String)> {
    let mut sessions = state.session_store.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    // Verify there's an active resolution session
    match &session.sub_session_type {
        SubSessionType::Resolution(_) => {}
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "No active resolution session".to_string(),
            ))
        }
    };

    // Clear the sub-session
    session.sub_session_type = SubSessionType::Root;

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
    gateway: GatewayRefResolver,
) -> Router {
    let state = ResolutionState::new(session_store, gateway);

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
