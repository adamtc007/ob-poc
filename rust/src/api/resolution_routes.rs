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

use crate::api::session::SessionStore;
// Use unified session types
use crate::dsl_v2::gateway_resolver::GatewayRefResolver;
use crate::dsl_v2::semantic_validator::entity_type_to_ref_type;
use crate::session::{EntityMatchInfo, ResolutionSubSession, SubSessionType, UnresolvedRefInfo};

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

/// Entity config cache to avoid repeated gRPC calls
/// In production, this should be populated at startup or use a proper cache
use std::sync::OnceLock;
use tokio::sync::RwLock;

static ENTITY_CONFIG_CACHE: OnceLock<RwLock<HashMap<String, CachedEntityConfig>>> = OnceLock::new();

#[derive(Clone)]
struct CachedEntityConfig {
    search_keys: Vec<ob_poc_types::resolution::SearchKeyField>,
    discriminator_fields: Vec<ob_poc_types::resolution::DiscriminatorField>,
    resolution_mode: ob_poc_types::resolution::ResolutionModeHint,
    return_key_type: String,
}

fn get_config_cache() -> &'static RwLock<HashMap<String, CachedEntityConfig>> {
    ENTITY_CONFIG_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

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
        search_keys: vec![],          // Populated by enrich_with_entity_config
        discriminator_fields: vec![], // Populated by enrich_with_entity_config
        resolution_mode: ob_poc_types::resolution::ResolutionModeHint::SearchModal,
        return_key_type: None,
    }
}

/// Enrich unresolved ref with entity config (search keys, discriminators)
async fn enrich_with_entity_config(
    gateway: &mut crate::dsl_v2::gateway_resolver::GatewayRefResolver,
    unresolved: &mut UnresolvedRefResponse,
) {
    use ob_poc_types::resolution::{
        DiscriminatorField, DiscriminatorFieldType, EnumValue, ResolutionModeHint, SearchKeyField,
        SearchKeyFieldType,
    };

    // Check cache first
    let cache = get_config_cache();
    {
        let read_guard = cache.read().await;
        if let Some(cached) = read_guard.get(&unresolved.entity_type) {
            unresolved.search_keys = cached.search_keys.clone();
            unresolved.discriminator_fields = cached.discriminator_fields.clone();
            unresolved.resolution_mode = cached.resolution_mode.clone();
            unresolved.return_key_type = Some(cached.return_key_type.clone());
            return;
        }
    }

    // Fetch from gateway
    let nickname = unresolved.entity_type.to_uppercase();
    let config = match gateway.get_entity_config(&nickname).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to get entity config for {}: {}", nickname, e);
            return;
        }
    };

    // Convert proto types to API types
    let search_keys: Vec<SearchKeyField> = config
        .search_keys
        .into_iter()
        .map(|k| SearchKeyField {
            name: k.name,
            label: k.label,
            is_default: k.is_default,
            field_type: match k.field_type {
                0 => SearchKeyFieldType::Text,
                1 => SearchKeyFieldType::Enum,
                2 => SearchKeyFieldType::Uuid,
                _ => SearchKeyFieldType::Text,
            },
            enum_values: if k.enum_values.is_empty() {
                None
            } else {
                Some(
                    k.enum_values
                        .into_iter()
                        .map(|e| EnumValue {
                            code: e.code,
                            display: e.display,
                        })
                        .collect(),
                )
            },
        })
        .collect();

    let discriminator_fields: Vec<DiscriminatorField> = config
        .discriminators
        .into_iter()
        .map(|d| DiscriminatorField {
            name: d.name,
            label: d.label,
            selectivity: d.selectivity,
            field_type: match d.field_type {
                0 => DiscriminatorFieldType::String,
                1 => DiscriminatorFieldType::Date,
                2 => DiscriminatorFieldType::Enum,
                _ => DiscriminatorFieldType::String,
            },
            enum_values: if d.enum_values.is_empty() {
                None
            } else {
                Some(
                    d.enum_values
                        .into_iter()
                        .map(|e| EnumValue {
                            code: e.code,
                            display: e.display,
                        })
                        .collect(),
                )
            },
            value: None,
        })
        .collect();

    let resolution_mode = match config.resolution_mode {
        0 => ResolutionModeHint::SearchModal,
        1 => ResolutionModeHint::Autocomplete,
        _ => ResolutionModeHint::SearchModal,
    };

    // Cache for future use
    {
        let mut write_guard = cache.write().await;
        write_guard.insert(
            unresolved.entity_type.clone(),
            CachedEntityConfig {
                search_keys: search_keys.clone(),
                discriminator_fields: discriminator_fields.clone(),
                resolution_mode: resolution_mode.clone(),
                return_key_type: config.return_key_type.clone(),
            },
        );
    }

    // Set on response
    unresolved.search_keys = search_keys;
    unresolved.discriminator_fields = discriminator_fields;
    unresolved.resolution_mode = resolution_mode;
    unresolved.return_key_type = Some(config.return_key_type);
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

/// Build session response with entity config enrichment (async)
async fn build_session_response_enriched(
    session_id: Uuid,
    resolution: &ResolutionSubSession,
    gateway: &mut crate::dsl_v2::gateway_resolver::GatewayRefResolver,
) -> ResolutionSessionResponse {
    let progress = resolution.progress();

    // Build unresolved refs and enrich with entity config
    let mut unresolved: Vec<UnresolvedRefResponse> = resolution
        .unresolved_refs
        .iter()
        .filter(|r| !resolution.resolutions.contains_key(&r.ref_id))
        .map(|r| unresolved_ref_to_response(r, &resolution.resolutions))
        .collect();

    // Enrich each unresolved ref with search keys and discriminators
    for unresolved_ref in &mut unresolved {
        enrich_with_entity_config(gateway, unresolved_ref).await;
    }

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
                display: resolved_key.clone(),
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
            total_refs: progress.1,
            resolved_count: progress.0,
            warnings_count: 0,
            required_review_count: progress.1 - progress.0,
            can_commit: resolution.is_complete(),
        },
    }
}

/// Build session response from ResolutionSubSession (sync version, no entity config)
fn build_session_response(
    session_id: Uuid,
    resolution: &ResolutionSubSession,
) -> ResolutionSessionResponse {
    let progress = resolution.progress();

    // Split refs into unresolved and resolved (without entity config enrichment)
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
            total_refs: progress.1,
            resolved_count: progress.0,
            warnings_count: 0,
            required_review_count: progress.1 - progress.0,
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
    let resolution = {
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
        resolution
    };

    // Enrich response with entity config (search keys, discriminators)
    let mut gateway = state.gateway.clone();
    let response = build_session_response_enriched(session_id, &resolution, &mut gateway).await;
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
/// Supports multi-key search with filters and edge case handling.
pub async fn search_resolution(
    State(state): State<ResolutionState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ResolutionSearchRequest>,
) -> Result<Json<ResolutionSearchResponse>, (StatusCode, String)> {
    use ob_poc_types::resolution::{SearchSuggestions, SuggestedAction, SuggestedActionType};

    // Edge case 1: Empty query - return early with helpful message
    let query = body.query.trim();
    if query.is_empty() {
        return Ok(Json(ResolutionSearchResponse {
            matches: vec![],
            total: 0,
            truncated: false,
            fallback_matches: None,
            filtered_by: if body.filters.is_empty() {
                None
            } else {
                Some(body.filters.clone())
            },
            suggestions: Some(SearchSuggestions {
                message: "Enter a search term to find matches".to_string(),
                actions: vec![],
            }),
        }));
    }

    // Edge case 2: Query too long (>100 chars) - simplify suggestion
    if query.len() > 100 {
        return Ok(Json(ResolutionSearchResponse {
            matches: vec![],
            total: 0,
            truncated: false,
            fallback_matches: None,
            filtered_by: None,
            suggestions: Some(SearchSuggestions {
                message: "Search query is too long. Try a shorter search term.".to_string(),
                actions: vec![SuggestedAction {
                    label: "Simplify query".to_string(),
                    action: SuggestedActionType::SimplifyQuery,
                }],
            }),
        }));
    }

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

    // Search via gateway with multi-key support
    let mut gateway = state.gateway.clone();
    let ref_type = entity_type_to_ref_type(&entity_type);
    let limit = body.limit.unwrap_or(10);

    let (matches, total, was_filtered) = gateway
        .search_multi_key(
            ref_type,
            query,
            body.search_key.as_deref(),
            &body.filters,
            &body.discriminators,
            limit,
        )
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

    // Edge case 3 & 4: No results with filters - try fallback search without filters
    if response_matches.is_empty() && was_filtered {
        // Search again without filters to provide fallback matches
        let (fallback_matches_raw, _, _) = gateway
            .search_multi_key(
                ref_type,
                query,
                body.search_key.as_deref(),
                &HashMap::new(), // No filters
                &body.discriminators,
                limit,
            )
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

        let fallback_matches: Vec<EntityMatchResponse> = fallback_matches_raw
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

        // Build suggestion actions for clearing filters
        let mut actions: Vec<SuggestedAction> = body
            .filters
            .keys()
            .map(|k| SuggestedAction {
                label: format!("Clear {} filter", k),
                action: SuggestedActionType::ClearFilter { key: k.clone() },
            })
            .collect();

        if body.filters.len() > 1 {
            actions.insert(
                0,
                SuggestedAction {
                    label: "Clear all filters".to_string(),
                    action: SuggestedActionType::ClearFilters,
                },
            );
        }

        let message = if fallback_matches.is_empty() {
            format!(
                "No matches found for '{}'. Try a different search term.",
                query
            )
        } else {
            format!(
                "No matches in selected filters. Found {} result(s) elsewhere.",
                fallback_matches.len()
            )
        };

        return Ok(Json(ResolutionSearchResponse {
            matches: vec![],
            total: 0,
            truncated: false,
            fallback_matches: if fallback_matches.is_empty() {
                None
            } else {
                Some(fallback_matches)
            },
            filtered_by: Some(body.filters.clone()),
            suggestions: Some(SearchSuggestions { message, actions }),
        }));
    }

    // Edge case 5: No results at all (even without filters)
    if response_matches.is_empty() {
        return Ok(Json(ResolutionSearchResponse {
            matches: vec![],
            total: 0,
            truncated: false,
            fallback_matches: None,
            filtered_by: if was_filtered {
                Some(body.filters.clone())
            } else {
                None
            },
            suggestions: Some(SearchSuggestions {
                message: format!(
                    "No matches found for '{}'. Check spelling or try a different term.",
                    query
                ),
                actions: vec![SuggestedAction {
                    label: "Create new entity".to_string(),
                    action: SuggestedActionType::CreateNew,
                }],
            }),
        }));
    }

    // Normal case: results found
    Ok(Json(ResolutionSearchResponse {
        matches: response_matches,
        total,
        truncated: total > limit,
        fallback_matches: None,
        filtered_by: if was_filtered {
            Some(body.filters.clone())
        } else {
            None
        },
        suggestions: None,
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
        let progress = resolution.progress();
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot commit: only {}/{} references resolved",
                progress.0, progress.1
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
