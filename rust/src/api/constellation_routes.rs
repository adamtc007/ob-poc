//! Constellation hydration API routes.
//!
//! Endpoints for producing the server-side constellation graph payload
//! that is returned to the UI.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_os_runtime::constellation_runtime::{
    handle_constellation_hydrate, handle_constellation_summary, ConstellationSummary,
    HydratedConstellation,
};

/// Application state for constellation routes.
#[derive(Clone)]
pub struct ConstellationAppState {
    pub pool: PgPool,
}

#[derive(Debug, Deserialize)]
struct ConstellationQuery {
    #[serde(rename = "case_id")]
    case_id: Option<Uuid>,
    #[serde(rename = "map_name")]
    map_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConstellationNameQuery {
    name: String,
    #[serde(rename = "case_id")]
    case_id: Option<Uuid>,
    #[serde(rename = "map_name")]
    map_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchCbusQuery {
    name: String,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ResolvedCbu {
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
}

#[derive(Debug, Serialize)]
struct HydrateByNameResponse {
    resolved_cbu: ResolvedCbu,
    hydrated: HydratedConstellation,
}

#[derive(Debug, Serialize)]
struct CbuCaseSummary {
    case_id: Uuid,
    status: Option<String>,
    case_type: Option<String>,
    opened_at: Option<DateTime<Utc>>,
}

/// Create API routes for constellation hydration and summary lookup.
///
/// # Examples
/// ```rust,no_run
/// # #[cfg(feature = "server")]
/// # {
/// use ob_poc::api::create_constellation_router;
/// # let pool = sqlx::PgPool::connect_lazy("postgresql:///data_designer").unwrap();
/// let _router = create_constellation_router(pool);
/// # }
/// ```
pub fn create_constellation_router(pool: PgPool) -> Router {
    let state = ConstellationAppState { pool };
    Router::new()
        .route("/api/cbu/:cbu_id/constellation", get(get_constellation))
        .route("/api/cbu/:cbu_id/cases", get(get_constellation_cases))
        .route(
            "/api/cbu/:cbu_id/constellation/summary",
            get(get_constellation_summary),
        )
        .route("/api/constellation/by-name", get(get_constellation_by_name))
        .route(
            "/api/constellation/search-cbus",
            get(search_constellation_cbus),
        )
        .with_state(state)
}

async fn get_constellation(
    State(state): State<ConstellationAppState>,
    Path(cbu_id): Path<Uuid>,
    Query(query): Query<ConstellationQuery>,
) -> Result<Json<HydratedConstellation>, (StatusCode, String)> {
    let map_name = query
        .map_name
        .unwrap_or_else(|| String::from("struct.lux.ucits.sicav"));
    let hydrated = handle_constellation_hydrate(&state.pool, cbu_id, query.case_id, &map_name)
        .await
        .map_err(internal_error)?;
    Ok(Json(hydrated))
}

async fn get_constellation_summary(
    State(state): State<ConstellationAppState>,
    Path(cbu_id): Path<Uuid>,
    Query(query): Query<ConstellationQuery>,
) -> Result<Json<ConstellationSummary>, (StatusCode, String)> {
    let map_name = query
        .map_name
        .unwrap_or_else(|| String::from("struct.lux.ucits.sicav"));
    let summary = handle_constellation_summary(&state.pool, cbu_id, query.case_id, &map_name)
        .await
        .map_err(internal_error)?;
    Ok(Json(summary))
}

async fn get_constellation_cases(
    State(state): State<ConstellationAppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<Vec<CbuCaseSummary>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, Option<DateTime<Utc>>)>(
        r#"
            SELECT case_id, status, case_type, opened_at
            FROM "ob-poc".cases
            WHERE cbu_id = $1
            ORDER BY opened_at DESC NULLS LAST
            "#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(
        rows.into_iter()
            .map(|(case_id, status, case_type, opened_at)| CbuCaseSummary {
                case_id,
                status,
                case_type,
                opened_at,
            })
            .collect(),
    ))
}

async fn get_constellation_by_name(
    State(state): State<ConstellationAppState>,
    Query(query): Query<ConstellationNameQuery>,
) -> Result<Json<HydrateByNameResponse>, (StatusCode, String)> {
    let map_name = query
        .map_name
        .unwrap_or_else(|| String::from("struct.lux.ucits.sicav"));
    let resolved = resolve_cbu_by_name(&state.pool, &query.name)
        .await?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("No CBU found matching '{}'", query.name),
            )
        })?;
    let hydrated =
        handle_constellation_hydrate(&state.pool, resolved.cbu_id, query.case_id, &map_name)
            .await
            .map_err(internal_error)?;
    Ok(Json(HydrateByNameResponse {
        resolved_cbu: resolved,
        hydrated,
    }))
}

async fn search_constellation_cbus(
    State(state): State<ConstellationAppState>,
    Query(query): Query<SearchCbusQuery>,
) -> Result<Json<Vec<ResolvedCbu>>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        r#"
        SELECT cbu_id, name, jurisdiction
        FROM "ob-poc".cbus
        WHERE name ILIKE '%' || $1 || '%'
        ORDER BY
            CASE
                WHEN LOWER(name) = LOWER($1) THEN 0
                WHEN LOWER(name) LIKE LOWER($1) || '%' THEN 1
                ELSE 2
            END,
            name ASC
        LIMIT $2
        "#,
    )
    .bind(&query.name)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(
        rows.into_iter()
            .map(|(cbu_id, name, jurisdiction)| ResolvedCbu {
                cbu_id,
                name,
                jurisdiction,
            })
            .collect(),
    ))
}

async fn resolve_cbu_by_name(
    pool: &PgPool,
    name: &str,
) -> Result<Option<ResolvedCbu>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        r#"
        SELECT cbu_id, name, jurisdiction
        FROM "ob-poc".cbus
        WHERE name ILIKE '%' || $1 || '%'
        ORDER BY
            CASE
                WHEN LOWER(name) = LOWER($1) THEN 0
                WHEN LOWER(name) LIKE LOWER($1) || '%' THEN 1
                ELSE 2
            END,
            name ASC
        LIMIT 1
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?;

    Ok(row.map(|(cbu_id, name, jurisdiction)| ResolvedCbu {
        cbu_id,
        name,
        jurisdiction,
    }))
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Constellation API error: {error}"),
    )
}
