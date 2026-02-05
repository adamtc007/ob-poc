//! Deal Taxonomy API Routes
//!
//! Provides endpoints for deal taxonomy visualization:
//! - GET /api/deal/:deal_id/graph - Get full deal graph
//! - GET /api/deal/:deal_id/products - Get deal products
//! - GET /api/deal/:deal_id/rate-cards - Get deal rate cards
//! - GET /api/deal/rate-card/:rate_card_id/lines - Get rate card lines
//! - GET /api/deal/rate-card/:rate_card_id/history - Get rate card history
//! - GET /api/deal/:deal_id/participants - Get deal participants
//! - GET /api/deal/:deal_id/contracts - Get deal contracts
//! - GET /api/deal/:deal_id/onboarding-requests - Get onboarding requests
//! - GET /api/deals - List deals with filters
//! - GET /api/session/:session_id/deal-context - Get session deal context

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::deal_types::{
    DealContractSummary, DealFilters, DealGraphResponse, DealListResponse, DealParticipantSummary,
    DealProductSummary, DealSummary, DealViewMode, OnboardingRequestSummary, RateCardDetail,
    RateCardLineSummary, RateCardSummary, SessionDealContext,
};
use crate::api::SessionStore;
use crate::database::DealRepository;
use crate::graph::DealGraphBuilder;

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct DealState {
    pub pool: PgPool,
    pub sessions: SessionStore,
}

impl DealState {
    pub fn new(pool: PgPool, sessions: SessionStore) -> Self {
        Self { pool, sessions }
    }
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for deal graph endpoint
#[derive(Debug, Deserialize)]
pub struct DealGraphQuery {
    /// View mode: COMMERCIAL, FINANCIAL, STATUS
    #[serde(default)]
    pub view_mode: Option<String>,
}

/// Query parameters for deal list endpoint
#[derive(Debug, Deserialize)]
pub struct DealListQuery {
    pub client_group_id: Option<Uuid>,
    pub deal_status: Option<String>,
    pub sales_owner: Option<String>,
    pub sales_team: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/deal/:deal_id/graph
/// Get the full deal graph for taxonomy visualization
async fn get_deal_graph(
    State(state): State<DealState>,
    Path(deal_id): Path<Uuid>,
    Query(query): Query<DealGraphQuery>,
) -> Result<Json<DealGraphResponse>, (StatusCode, Json<ErrorResponse>)> {
    let view_mode = query
        .view_mode
        .as_deref()
        .map(|s| s.parse::<DealViewMode>())
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?
        .unwrap_or_default();

    let graph = DealGraphBuilder::new(deal_id)
        .with_view_mode(view_mode)
        .build(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(graph))
}

/// GET /api/deal/:deal_id
/// Get deal summary
async fn get_deal_summary(
    State(state): State<DealState>,
    Path(deal_id): Path<Uuid>,
) -> Result<Json<DealSummary>, (StatusCode, Json<ErrorResponse>)> {
    let deal = DealRepository::get_deal_summary(&state.pool, deal_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Deal not found: {}", deal_id),
                }),
            )
        })?;

    Ok(Json(deal))
}

/// GET /api/deal/:deal_id/products
/// Get products for a deal
async fn get_deal_products(
    State(state): State<DealState>,
    Path(deal_id): Path<Uuid>,
) -> Result<Json<Vec<DealProductSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let products = DealRepository::get_deal_products(&state.pool, deal_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(products))
}

/// GET /api/deal/:deal_id/rate-cards
/// Get rate cards for a deal
async fn get_deal_rate_cards(
    State(state): State<DealState>,
    Path(deal_id): Path<Uuid>,
) -> Result<Json<Vec<RateCardSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let rate_cards = DealRepository::get_deal_rate_cards(&state.pool, deal_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(rate_cards))
}

/// GET /api/deal/:deal_id/product/:product_id/rate-cards
/// Get rate cards for a specific product in a deal
async fn get_product_rate_cards(
    State(state): State<DealState>,
    Path((deal_id, product_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<RateCardSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let rate_cards = DealRepository::get_product_rate_cards(&state.pool, deal_id, product_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(rate_cards))
}

/// GET /api/deal/rate-card/:rate_card_id/lines
/// Get lines for a rate card
async fn get_rate_card_lines(
    State(state): State<DealState>,
    Path(rate_card_id): Path<Uuid>,
) -> Result<Json<Vec<RateCardLineSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let lines = DealRepository::get_rate_card_lines(&state.pool, rate_card_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(lines))
}

/// GET /api/deal/rate-card/:rate_card_id/history
/// Get supersession history for a rate card
async fn get_rate_card_history(
    State(state): State<DealState>,
    Path(rate_card_id): Path<Uuid>,
) -> Result<Json<Vec<RateCardSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let history = DealRepository::get_rate_card_history(&state.pool, rate_card_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(history))
}

/// GET /api/deal/rate-card/:rate_card_id/detail
/// Get rate card with lines and history
async fn get_rate_card_detail(
    State(state): State<DealState>,
    Path(rate_card_id): Path<Uuid>,
) -> Result<Json<RateCardDetail>, (StatusCode, Json<ErrorResponse>)> {
    // Get rate card history (includes the card itself at position 0)
    let history = DealRepository::get_rate_card_history(&state.pool, rate_card_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let rate_card = history.first().cloned().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Rate card not found: {}", rate_card_id),
            }),
        )
    })?;

    // Get lines
    let lines = DealRepository::get_rate_card_lines(&state.pool, rate_card_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    // History excludes the current card (it's at index 0)
    let history = history.into_iter().skip(1).collect();

    Ok(Json(RateCardDetail {
        rate_card,
        lines,
        history,
    }))
}

/// GET /api/deal/:deal_id/participants
/// Get participants for a deal
async fn get_deal_participants(
    State(state): State<DealState>,
    Path(deal_id): Path<Uuid>,
) -> Result<Json<Vec<DealParticipantSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let participants = DealRepository::get_deal_participants(&state.pool, deal_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(participants))
}

/// GET /api/deal/:deal_id/contracts
/// Get contracts for a deal
async fn get_deal_contracts(
    State(state): State<DealState>,
    Path(deal_id): Path<Uuid>,
) -> Result<Json<Vec<DealContractSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let contracts = DealRepository::get_deal_contracts(&state.pool, deal_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(contracts))
}

/// GET /api/deal/:deal_id/onboarding-requests
/// Get onboarding requests for a deal
async fn get_deal_onboarding_requests(
    State(state): State<DealState>,
    Path(deal_id): Path<Uuid>,
) -> Result<Json<Vec<OnboardingRequestSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let requests = DealRepository::get_deal_onboarding_requests(&state.pool, deal_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(requests))
}

/// GET /api/deals
/// List deals with optional filters
async fn list_deals(
    State(state): State<DealState>,
    Query(query): Query<DealListQuery>,
) -> Result<Json<DealListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let filters = DealFilters {
        client_group_id: query.client_group_id,
        deal_status: query.deal_status,
        sales_owner: query.sales_owner,
        sales_team: query.sales_team,
        opened_after: None,
        opened_before: None,
        limit: query.limit,
        offset: query.offset,
    };

    let limit = filters.limit.unwrap_or(50);
    let offset = filters.offset.unwrap_or(0);

    let deals = DealRepository::list_deals(&state.pool, &filters)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let total_count = DealRepository::count_deals(&state.pool, &filters)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(DealListResponse {
        deals,
        total_count,
        offset,
        limit,
    }))
}

/// GET /api/session/:session_id/deal-context
/// Get deal context from session
async fn get_session_deal_context(
    State(state): State<DealState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Option<SessionDealContext>>, (StatusCode, Json<ErrorResponse>)> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id);

    let context = match session {
        Some(session) => {
            match (&session.context.deal_id, &session.context.deal_name) {
                (Some(deal_id), Some(deal_name)) => {
                    // Fetch additional details from DB
                    let deal = DealRepository::get_deal_summary(&state.pool, *deal_id)
                        .await
                        .ok()
                        .flatten();

                    Some(SessionDealContext {
                        deal_id: *deal_id,
                        deal_name: deal_name.clone(),
                        deal_status: deal
                            .as_ref()
                            .map(|d| d.deal_status.clone())
                            .unwrap_or_else(|| "UNKNOWN".to_string()),
                        client_group_name: deal.and_then(|d| d.client_group_name),
                    })
                }
                _ => None,
            }
        }
        None => None,
    };

    Ok(Json(context))
}

// ============================================================================
// Router
// ============================================================================

/// Create the deal routes router
pub fn create_deal_router(pool: PgPool, sessions: SessionStore) -> Router {
    let state = DealState::new(pool, sessions);

    Router::new()
        // Deal endpoints
        .route("/deals", get(list_deals))
        .route("/deal/:deal_id", get(get_deal_summary))
        .route("/deal/:deal_id/graph", get(get_deal_graph))
        .route("/deal/:deal_id/products", get(get_deal_products))
        .route("/deal/:deal_id/rate-cards", get(get_deal_rate_cards))
        .route(
            "/deal/:deal_id/product/:product_id/rate-cards",
            get(get_product_rate_cards),
        )
        .route("/deal/:deal_id/participants", get(get_deal_participants))
        .route("/deal/:deal_id/contracts", get(get_deal_contracts))
        .route(
            "/deal/:deal_id/onboarding-requests",
            get(get_deal_onboarding_requests),
        )
        // Rate card endpoints
        .route(
            "/deal/rate-card/:rate_card_id/lines",
            get(get_rate_card_lines),
        )
        .route(
            "/deal/rate-card/:rate_card_id/history",
            get(get_rate_card_history),
        )
        .route(
            "/deal/rate-card/:rate_card_id/detail",
            get(get_rate_card_detail),
        )
        // Session deal context
        .route(
            "/session/:session_id/deal-context",
            get(get_session_deal_context),
        )
        .with_state(state)
}

/// Create deal router with just pool (no sessions) for simpler use cases
pub fn create_deal_router_simple(pool: PgPool) -> Router {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Create empty sessions store
    let sessions = Arc::new(RwLock::new(std::collections::HashMap::new()));
    create_deal_router(pool, sessions)
}
