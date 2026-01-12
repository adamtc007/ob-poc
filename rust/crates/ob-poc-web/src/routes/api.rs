//! REST API endpoints for CBU and Graph data
//!
//! Session management and chat endpoints are handled by agent_routes in the main crate.
//! This module only contains CBU listing and graph visualization endpoints.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use ob_poc_types::CbuSummary;

use crate::state::AppState;

/// Query parameters for CBU search
#[derive(Debug, serde::Deserialize)]
pub struct CbuSearchQuery {
    /// Search query (matches against name, case-insensitive)
    pub q: Option<String>,
    /// Maximum results to return (default 20)
    pub limit: Option<i64>,
}

// =============================================================================
// CBU
// =============================================================================

/// Search CBUs by name (case-insensitive, trigram similarity)
pub async fn search_cbus(
    State(state): State<AppState>,
    Query(params): Query<CbuSearchQuery>,
) -> Result<Json<Vec<CbuSummary>>, StatusCode> {
    let query = params.q.unwrap_or_default();
    let limit = params.limit.unwrap_or(20).min(100);

    let rows = if query.is_empty() {
        // No query - return recent/popular CBUs
        sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
            r#"SELECT cbu_id, name, jurisdiction, client_type
               FROM "ob-poc".cbus
               ORDER BY updated_at DESC NULLS LAST, name
               LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(&state.pool)
        .await
    } else {
        // Search by name using ILIKE for prefix/contains match
        // Uses pg_trgm index if available for better performance
        sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
            r#"SELECT cbu_id, name, jurisdiction, client_type
               FROM "ob-poc".cbus
               WHERE name ILIKE $1
               ORDER BY
                   CASE WHEN name ILIKE $2 THEN 0 ELSE 1 END,  -- Prefix matches first
                   name
               LIMIT $3"#,
        )
        .bind(format!("%{}%", query)) // Contains match
        .bind(format!("{}%", query)) // Prefix match (for ordering)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
    }
    .map_err(|e| {
        tracing::error!("CBU search error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let cbus: Vec<CbuSummary> = rows
        .into_iter()
        .map(|(cbu_id, name, jurisdiction, client_type)| CbuSummary {
            cbu_id: cbu_id.to_string(),
            name,
            jurisdiction,
            client_type,
            cbu_category: None,
        })
        .collect();

    Ok(Json(cbus))
}
