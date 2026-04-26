//! Catalogue Workspace REST routes — Tranche 3 Phase 3.D scaffold (2026-04-26).
//!
//! Read-only Observatory bindings for the Catalogue workspace. Provides:
//!
//!   GET /api/catalogue/proposals          List proposals (status filter)
//!   GET /api/catalogue/proposals/:id      Single proposal detail (incl. diff)
//!   GET /api/catalogue/tier-distribution  Live tier heatmap (Phase 2.G.2)
//!
//! These endpoints back the Observatory's Catalogue-workspace UX so authors
//! can see live proposal status without leaving the Observatory canvas.
//!
//! Phase 3.D full UX (egui canvas integration, diff preview rendering,
//! ABAC two-eye visualization) is Observatory Phase 8 work — out of scope
//! for Tranche 3 Phase 3.B core. This scaffold provides the data API
//! that the canvas will consume.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct CatalogueState {
    pub pool: PgPool,
}

#[derive(Debug, Deserialize)]
pub struct ProposalListQuery {
    /// pending (DRAFT|STAGED) | committed | rolled_back | all
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "pending".to_string()
}

#[derive(Debug, Serialize)]
pub struct ProposalSummary {
    pub proposal_id: Uuid,
    pub verb_fqn: String,
    pub status: String,
    pub proposed_by: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub committed_by: Option<String>,
    pub rolled_back_by: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProposalDetail {
    pub proposal_id: Uuid,
    pub verb_fqn: String,
    pub status: String,
    pub proposed_by: String,
    pub proposed_declaration: serde_json::Value,
    pub rationale: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub staged_at: Option<chrono::DateTime<chrono::Utc>>,
    pub committed_by: Option<String>,
    pub committed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub rolled_back_by: Option<String>,
    pub rolled_back_at: Option<chrono::DateTime<chrono::Utc>>,
    pub rolled_back_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TierDistribution {
    /// Per-tier counts across the catalogue.
    pub by_tier: HashMap<String, i64>,
    /// Per-domain × per-tier (for the heatmap rendering).
    pub by_domain_tier: HashMap<String, HashMap<String, i64>>,
    /// Total verbs in the catalogue.
    pub total_verbs: i64,
    /// Verbs with three_axis declared.
    pub three_axis_declared: i64,
}

async fn list_proposals(
    State(state): State<CatalogueState>,
    Query(q): Query<ProposalListQuery>,
) -> Result<Json<Vec<ProposalSummary>>, (StatusCode, String)> {
    let sql = match q.status.as_str() {
        "pending" => {
            r#"SELECT proposal_id, verb_fqn, status, proposed_by, created_at,
                      committed_by, rolled_back_by
               FROM "ob-poc".catalogue_proposals
               WHERE status IN ('DRAFT', 'STAGED')
               ORDER BY created_at DESC LIMIT 200"#
        }
        "committed" => {
            r#"SELECT proposal_id, verb_fqn, status, proposed_by,
                      committed_at AS created_at, committed_by, rolled_back_by
               FROM "ob-poc".catalogue_proposals
               WHERE status = 'COMMITTED'
               ORDER BY committed_at DESC LIMIT 200"#
        }
        "rolled_back" => {
            r#"SELECT proposal_id, verb_fqn, status, proposed_by,
                      rolled_back_at AS created_at, committed_by, rolled_back_by
               FROM "ob-poc".catalogue_proposals
               WHERE status = 'ROLLED_BACK'
               ORDER BY rolled_back_at DESC LIMIT 200"#
        }
        _ => {
            r#"SELECT proposal_id, verb_fqn, status, proposed_by, created_at,
                      committed_by, rolled_back_by
               FROM "ob-poc".catalogue_proposals
               ORDER BY created_at DESC LIMIT 200"#
        }
    };
    let rows = sqlx::query(sql)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let proposal_id: Uuid = r.try_get("proposal_id").map_err(serror)?;
        let verb_fqn: String = r.try_get("verb_fqn").map_err(serror)?;
        let status: String = r.try_get("status").map_err(serror)?;
        let proposed_by: String = r.try_get("proposed_by").map_err(serror)?;
        let created_at = r.try_get("created_at").map_err(serror)?;
        let committed_by: Option<String> = r.try_get("committed_by").ok();
        let rolled_back_by: Option<String> = r.try_get("rolled_back_by").ok();
        out.push(ProposalSummary {
            proposal_id,
            verb_fqn,
            status,
            proposed_by,
            created_at,
            committed_by,
            rolled_back_by,
        });
    }
    Ok(Json(out))
}

async fn get_proposal(
    State(state): State<CatalogueState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProposalDetail>, (StatusCode, String)> {
    let row = sqlx::query(
        r#"SELECT proposal_id, verb_fqn, status, proposed_by, proposed_declaration,
                  rationale, created_at, staged_at, committed_by, committed_at,
                  rolled_back_by, rolled_back_at, rolled_back_reason
           FROM "ob-poc".catalogue_proposals
           WHERE proposal_id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("proposal {} not found", id)))?;

    Ok(Json(ProposalDetail {
        proposal_id: row.try_get("proposal_id").map_err(serror)?,
        verb_fqn: row.try_get("verb_fqn").map_err(serror)?,
        status: row.try_get("status").map_err(serror)?,
        proposed_by: row.try_get("proposed_by").map_err(serror)?,
        proposed_declaration: row.try_get("proposed_declaration").map_err(serror)?,
        rationale: row.try_get("rationale").ok(),
        created_at: row.try_get("created_at").map_err(serror)?,
        staged_at: row.try_get("staged_at").ok(),
        committed_by: row.try_get("committed_by").ok(),
        committed_at: row.try_get("committed_at").ok(),
        rolled_back_by: row.try_get("rolled_back_by").ok(),
        rolled_back_at: row.try_get("rolled_back_at").ok(),
        rolled_back_reason: row.try_get("rolled_back_reason").ok(),
    }))
}

async fn tier_distribution(
    State(state): State<CatalogueState>,
) -> Result<Json<TierDistribution>, (StatusCode, String)> {
    // Catalogue load is the source of truth for the in-memory view.
    // For Phase 3.D scaffold, we read from catalogue_committed_verbs
    // (Stage 4 destination) when populated, else reflect the static
    // YAML estimates for the heatmap.
    let total_committed: i64 =
        sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".catalogue_committed_verbs"#)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Tier counts read from the committed-verbs projection (post-Stage-4 source of truth).
    // Until Stage 4, this is partial; the YAML still drives runtime.
    let mut by_tier: HashMap<String, i64> = HashMap::new();
    if total_committed > 0 {
        let rows = sqlx::query(
            r#"SELECT (declaration->'three_axis'->'consequence'->>'baseline') AS tier, count(*)
               FROM "ob-poc".catalogue_committed_verbs
               GROUP BY tier"#,
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for r in rows {
            let tier: Option<String> = r.try_get("tier").ok();
            let count: i64 = r.try_get("count").unwrap_or(0);
            by_tier.insert(tier.unwrap_or_else(|| "(undeclared)".to_string()), count);
        }
    }

    // Per-domain × per-tier: same query split by FQN domain prefix.
    let mut by_domain_tier: HashMap<String, HashMap<String, i64>> = HashMap::new();
    if total_committed > 0 {
        let rows = sqlx::query(
            r#"SELECT split_part(verb_fqn, '.', 1) AS domain,
                      (declaration->'three_axis'->'consequence'->>'baseline') AS tier,
                      count(*)
               FROM "ob-poc".catalogue_committed_verbs
               GROUP BY domain, tier"#,
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for r in rows {
            let domain: String = r.try_get("domain").unwrap_or_default();
            let tier: Option<String> = r.try_get("tier").ok();
            let count: i64 = r.try_get("count").unwrap_or(0);
            by_domain_tier
                .entry(domain)
                .or_default()
                .insert(tier.unwrap_or_else(|| "(undeclared)".to_string()), count);
        }
    }

    Ok(Json(TierDistribution {
        by_tier,
        by_domain_tier,
        total_verbs: total_committed,
        three_axis_declared: total_committed,
    }))
}

fn serror(e: sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

pub fn create_catalogue_router(pool: PgPool) -> Router {
    let state = CatalogueState { pool };
    Router::new()
        .route("/proposals", get(list_proposals))
        .route("/proposals/:id", get(get_proposal))
        .route("/tier-distribution", get(tier_distribution))
        .with_state(state)
}
