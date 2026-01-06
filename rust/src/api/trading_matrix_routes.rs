//! Trading Matrix API Routes
//!
//! Provides endpoints to fetch Trading Matrix data for CBU visualization.
//! The Trading Matrix shows a hierarchical drill-down view of trading configuration:
//!
//! CBU → Instrument Classes → Markets/Counterparties → Universe Entries → Resources
//!
//! Resources include: SSIs, Booking Rules, Settlement Chains, Tax Config, ISDA/CSA
//!
//! ## Document-First Architecture
//!
//! The Trading Matrix document IS the AST - stored in JSONB and served directly.
//! No SQL reconstruction is needed; the document is the single source of truth.
//!
//! This module uses the unified AST types from `ob_poc_types::trading_matrix`.
//! These types are shared between:
//! - Server API (this module)
//! - WASM UI client
//! - DSL execution layer

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use ob_poc_types::trading_matrix::TradingMatrixResponse;
use sqlx::PgPool;
use uuid::Uuid;

use crate::trading_profile::ast_db;

// =============================================================================
// API ENDPOINT
// =============================================================================

/// GET /api/cbu/{cbu_id}/trading-matrix
///
/// Returns the complete Trading Matrix tree for a CBU.
/// The response is a hierarchical structure suitable for drill-down visualization.
///
/// The document is loaded directly from JSONB storage - no SQL reconstruction needed.
/// If no document exists for the CBU, an empty response with CBU info is returned.
pub async fn get_trading_matrix(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<TradingMatrixResponse>, (StatusCode, String)> {
    // Get CBU info (needed for name and to verify CBU exists)
    let cbu = sqlx::query!(
        r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("CBU not found: {}", cbu_id)))?;

    // Load the active trading profile document directly from JSONB
    match ast_db::load_active_document(&pool, cbu_id).await {
        Ok(Some((_profile_id, doc))) => {
            // Convert document directly to response
            let response = TradingMatrixResponse::from(doc);
            Ok(Json(response))
        }
        Ok(None) => {
            // No trading profile exists yet - return empty tree with CBU info
            let response = TradingMatrixResponse {
                cbu_id: cbu_id.to_string(),
                cbu_name: cbu.name,
                children: Vec::new(),
                total_leaf_count: 0,
            };
            Ok(Json(response))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to load trading matrix: {}", e),
        )),
    }
}

/// Create the trading matrix router
pub fn create_trading_matrix_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/cbu/:cbu_id/trading-matrix", get(get_trading_matrix))
        .with_state(pool)
}
