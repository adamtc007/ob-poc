//! Capital Structure API Endpoints
//!
//! Provides REST endpoints for cap table queries, control analysis,
//! ownership snapshots, and reconciliation.
//!
//! These endpoints power the capital structure visualization in the egui viewport
//! and provide data for ownership analysis.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

// Import investor register types
use crate::graph::investor_register::{
    AggregateBreakdown, AggregateInvestorsNode, ControlHolderNode, InvestorFilters,
    InvestorListItem, InvestorListResponse, InvestorRegisterView, IssuerSummary, PaginationInfo,
    ThresholdConfig,
};

// =============================================================================
// QUERY PARAMETERS
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CapTableQuery {
    /// ISO date (YYYY-MM-DD), defaults to today
    pub as_of: Option<String>,
    /// Ownership basis: VOTING (default), ECONOMIC, or BOTH
    pub basis: Option<String>,
    /// Include special rights in response
    #[serde(default)]
    pub include_special_rights: bool,
}

#[derive(Debug, Deserialize)]
pub struct ReconciliationQuery {
    /// Filter by status: OPEN, RESOLVED, ESCALATED
    pub status: Option<String>,
    /// Limit number of results
    pub limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct EconomicExposureQuery {
    /// ISO date (YYYY-MM-DD), defaults to today
    pub as_of: Option<String>,
    /// Maximum traversal depth (default 6)
    pub max_depth: Option<i32>,
    /// Stop when cumulative ownership drops below this (default 0.01%)
    pub min_pct: Option<f64>,
    /// Limit result set size (default 200)
    pub max_rows: Option<i32>,
    /// Stop at holders without BO data available
    #[serde(default = "default_true")]
    pub stop_on_no_bo_data: bool,
    /// Stop at holders with NONE lookthrough policy
    #[serde(default = "default_true")]
    pub stop_on_policy_none: bool,
}

fn default_true() -> bool {
    true
}

// =============================================================================
// RESPONSE TYPES
// =============================================================================

#[derive(Debug, Serialize)]
pub struct EconomicExposureResponse {
    pub root_entity_id: Uuid,
    pub root_name: String,
    pub as_of_date: String,
    pub exposures: Vec<ExposureNode>,
    pub parameters: ExposureParameters,
}

#[derive(Debug, Serialize)]
pub struct ExposureNode {
    pub leaf_entity_id: Uuid,
    pub leaf_name: String,
    pub cumulative_pct: f64,
    pub depth: i32,
    pub path_entities: Vec<Uuid>,
    pub path_names: Vec<String>,
    pub stopped_reason: String,
}

#[derive(Debug, Serialize)]
pub struct ExposureParameters {
    pub max_depth: i32,
    pub min_pct: f64,
    pub max_rows: i32,
    pub stop_on_no_bo_data: bool,
    pub stop_on_policy_none: bool,
}

#[derive(Debug, Serialize)]
pub struct CapTableResponse {
    pub issuer_entity_id: Uuid,
    pub issuer_name: String,
    pub as_of_date: String,
    pub share_classes: Vec<ShareClassSummary>,
    pub holders: Vec<HolderPosition>,
    pub total_votes: f64,
    pub total_economic: f64,
}

#[derive(Debug, Serialize)]
pub struct ShareClassSummary {
    pub share_class_id: Uuid,
    pub name: String,
    pub instrument_kind: String,
    pub votes_per_unit: f64,
    pub issued_units: f64,
    pub total_votes: f64,
    pub voting_weight_pct: f64,
    pub identifiers: Vec<IdentifierPair>,
}

#[derive(Debug, Serialize)]
pub struct IdentifierPair {
    pub scheme: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct HolderPosition {
    pub holder_entity_id: Uuid,
    pub holder_name: String,
    pub holder_type: String,
    pub units: f64,
    pub votes: f64,
    pub economic: f64,
    pub voting_pct: f64,
    pub economic_pct: f64,
    pub has_control: bool,
    pub has_significant_influence: bool,
    pub board_seats: i32,
    pub special_rights: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SupplyResponse {
    pub issuer_entity_id: Uuid,
    pub share_classes: Vec<ShareClassSupply>,
    pub total_authorized: f64,
    pub total_issued: f64,
    pub total_outstanding: f64,
}

#[derive(Debug, Serialize)]
pub struct ShareClassSupply {
    pub share_class_id: Uuid,
    pub name: String,
    pub authorized_units: f64,
    pub issued_units: f64,
    pub outstanding_units: f64,
    pub treasury_units: f64,
    pub reserved_units: f64,
}

#[derive(Debug, Serialize)]
pub struct ControlPositionsResponse {
    pub issuer_entity_id: Uuid,
    pub as_of_date: String,
    pub basis: String,
    pub positions: Vec<ControlPosition>,
}

#[derive(Debug, Serialize)]
pub struct ControlPosition {
    pub holder_entity_id: Uuid,
    pub holder_name: String,
    pub voting_pct: f64,
    pub economic_pct: f64,
    pub has_control: bool,
    pub has_significant_influence: bool,
    pub control_flags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SpecialRightsResponse {
    pub issuer_entity_id: Uuid,
    pub rights: Vec<SpecialRight>,
}

#[derive(Debug, Serialize)]
pub struct SpecialRight {
    pub right_id: Uuid,
    pub right_type: String,
    pub holder_entity_id: Uuid,
    pub holder_name: String,
    pub share_class_id: Option<Uuid>,
    pub description: Option<String>,
    pub effective_from: Option<String>,
    pub effective_to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReconciliationRunsResponse {
    pub issuer_entity_id: Uuid,
    pub runs: Vec<ReconciliationRunSummary>,
}

#[derive(Debug, Serialize)]
pub struct ReconciliationRunSummary {
    pub run_id: Uuid,
    pub run_ts: String,
    pub snapshot_a_id: Uuid,
    pub snapshot_b_id: Uuid,
    pub total_findings: i32,
    pub open_findings: i32,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ReconciliationFindingsResponse {
    pub run_id: Uuid,
    pub findings: Vec<ReconciliationFinding>,
}

#[derive(Debug, Serialize)]
pub struct ReconciliationFinding {
    pub finding_id: Uuid,
    pub holder_entity_id: Uuid,
    pub holder_name: String,
    pub finding_type: String,
    pub severity: String,
    pub source_a_value: Option<f64>,
    pub source_b_value: Option<f64>,
    pub delta_pct: Option<f64>,
    pub status: String,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OwnershipGraphResponse {
    pub issuer_entity_id: Uuid,
    pub issuer_name: String,
    pub as_of_date: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub label: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub data: serde_json::Value,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// GET /api/capital/:issuer_id/cap-table
///
/// Returns the complete cap table for an issuer, including share classes,
/// holders, and their control positions.
async fn get_cap_table(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
    Query(query): Query<CapTableQuery>,
) -> Result<Json<CapTableResponse>, (StatusCode, String)> {
    // Parse as_of date
    let as_of = parse_as_of_date(query.as_of.as_deref())?;
    let basis = query.basis.as_deref().unwrap_or("VOTING");

    // Get issuer info
    let issuer = sqlx::query!(
        r#"SELECT entity_id, name FROM "ob-poc".entities WHERE entity_id = $1"#,
        issuer_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Issuer not found: {}", issuer_id),
        )
    })?;

    // Get share classes
    let share_classes = get_share_classes_internal(&pool, issuer_id).await?;

    // Get holder positions using fn_holder_control_position
    let holder_rows = sqlx::query(r#"SELECT * FROM kyc.fn_holder_control_position($1, $2, $3)"#)
        .bind(issuer_id)
        .bind(as_of)
        .bind(basis)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut holders = Vec::new();
    let mut total_votes = 0.0;
    let mut total_economic = 0.0;

    for row in &holder_rows {
        let votes: f64 = row.try_get("total_votes").unwrap_or(0.0);
        let economic: f64 = row.try_get("total_economic_value").unwrap_or(0.0);
        total_votes += votes;
        total_economic += economic;

        let special_rights: Vec<String> = if query.include_special_rights {
            get_holder_special_rights(
                &pool,
                issuer_id,
                row.try_get("holder_entity_id").unwrap_or(Uuid::nil()),
            )
            .await?
        } else {
            vec![]
        };

        holders.push(HolderPosition {
            holder_entity_id: row.try_get("holder_entity_id").unwrap_or(Uuid::nil()),
            holder_name: row.try_get("holder_name").unwrap_or_default(),
            holder_type: row.try_get("holder_type").unwrap_or_default(),
            units: row.try_get("total_units").unwrap_or(0.0),
            votes,
            economic,
            voting_pct: row.try_get("voting_pct").unwrap_or(0.0),
            economic_pct: row.try_get("economic_pct").unwrap_or(0.0),
            has_control: row.try_get("has_control").unwrap_or(false),
            has_significant_influence: row.try_get("has_significant_influence").unwrap_or(false),
            board_seats: row.try_get("board_seats").unwrap_or(0),
            special_rights,
        });
    }

    Ok(Json(CapTableResponse {
        issuer_entity_id: issuer_id,
        issuer_name: issuer.name,
        as_of_date: as_of.to_string(),
        share_classes,
        holders,
        total_votes,
        total_economic,
    }))
}

/// GET /api/capital/:issuer_id/share-classes
///
/// Returns share class details for an issuer.
async fn get_share_classes(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
) -> Result<Json<Vec<ShareClassSummary>>, (StatusCode, String)> {
    let classes = get_share_classes_internal(&pool, issuer_id).await?;
    Ok(Json(classes))
}

/// GET /api/capital/:issuer_id/supply
///
/// Returns supply state for all share classes of an issuer.
async fn get_supply(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
) -> Result<Json<SupplyResponse>, (StatusCode, String)> {
    // Join share_classes with share_class_supply for supply data
    let rows = sqlx::query!(
        r#"
        SELECT
            sc.id as share_class_id,
            sc.name,
            COALESCE(scs.authorized_units, 0) as "authorized_units!",
            COALESCE(scs.issued_units, 0) as "issued_units!",
            COALESCE(scs.outstanding_units, 0) as "outstanding_units!",
            COALESCE(scs.treasury_units, 0) as "treasury_units!",
            COALESCE(scs.reserved_units, 0) as "reserved_units!"
        FROM kyc.share_classes sc
        LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
            AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id)
        WHERE sc.issuer_entity_id = $1
          AND sc.status = 'active'
        ORDER BY sc.name
        "#,
        issuer_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut total_authorized = 0.0;
    let mut total_issued = 0.0;
    let mut total_outstanding = 0.0;

    let share_classes: Vec<ShareClassSupply> = rows
        .into_iter()
        .map(|row| {
            let authorized = row
                .authorized_units
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0);
            let issued = row.issued_units.to_string().parse::<f64>().unwrap_or(0.0);
            let outstanding = row
                .outstanding_units
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0);
            let treasury = row.treasury_units.to_string().parse::<f64>().unwrap_or(0.0);
            let reserved = row.reserved_units.to_string().parse::<f64>().unwrap_or(0.0);

            total_authorized += authorized;
            total_issued += issued;
            total_outstanding += outstanding;

            ShareClassSupply {
                share_class_id: row.share_class_id,
                name: row.name,
                authorized_units: authorized,
                issued_units: issued,
                outstanding_units: outstanding,
                treasury_units: treasury,
                reserved_units: reserved,
            }
        })
        .collect();

    Ok(Json(SupplyResponse {
        issuer_entity_id: issuer_id,
        share_classes,
        total_authorized,
        total_issued,
        total_outstanding,
    }))
}

/// GET /api/capital/:issuer_id/control
///
/// Returns control positions for an issuer.
async fn get_control_positions(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
    Query(query): Query<CapTableQuery>,
) -> Result<Json<ControlPositionsResponse>, (StatusCode, String)> {
    let as_of = parse_as_of_date(query.as_of.as_deref())?;
    let basis = query.basis.as_deref().unwrap_or("VOTING").to_string();

    let rows = sqlx::query(r#"SELECT * FROM kyc.fn_holder_control_position($1, $2, $3)"#)
        .bind(issuer_id)
        .bind(as_of)
        .bind(&basis)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let positions: Vec<ControlPosition> = rows
        .iter()
        .map(|row| {
            let has_control: bool = row.try_get("has_control").unwrap_or(false);
            let has_significant: bool = row.try_get("has_significant_influence").unwrap_or(false);
            let board_seats: i32 = row.try_get("board_seats").unwrap_or(0);

            let mut control_flags = Vec::new();
            if has_control {
                control_flags.push("MAJORITY_CONTROL".to_string());
            }
            if has_significant {
                control_flags.push("SIGNIFICANT_INFLUENCE".to_string());
            }
            if board_seats > 0 {
                control_flags.push(format!("BOARD_SEATS_{}", board_seats));
            }

            ControlPosition {
                holder_entity_id: row.try_get("holder_entity_id").unwrap_or(Uuid::nil()),
                holder_name: row.try_get("holder_name").unwrap_or_default(),
                voting_pct: row.try_get("voting_pct").unwrap_or(0.0),
                economic_pct: row.try_get("economic_pct").unwrap_or(0.0),
                has_control,
                has_significant_influence: has_significant,
                control_flags,
            }
        })
        .collect();

    Ok(Json(ControlPositionsResponse {
        issuer_entity_id: issuer_id,
        as_of_date: as_of.to_string(),
        basis,
        positions,
    }))
}

/// GET /api/capital/:issuer_id/special-rights
///
/// Returns special rights attached to share classes or holders for an issuer.
async fn get_special_rights(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
) -> Result<Json<SpecialRightsResponse>, (StatusCode, String)> {
    // Use dynamic query to avoid compile-time schema checks for new tables
    let rows = sqlx::query(
        r#"
        SELECT
            sr.right_id,
            sr.right_type,
            sr.holder_entity_id,
            e.name as holder_name,
            sr.share_class_id,
            sr.description,
            sr.effective_from,
            sr.effective_to
        FROM kyc.special_rights sr
        JOIN "ob-poc".entities e ON e.entity_id = sr.holder_entity_id
        WHERE sr.share_class_id IN (
            SELECT id FROM kyc.share_classes WHERE issuer_entity_id = $1
        )
        ORDER BY sr.right_type, e.name
        "#,
    )
    .bind(issuer_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rights: Vec<SpecialRight> = rows
        .iter()
        .map(|row| SpecialRight {
            right_id: row.try_get("right_id").unwrap_or(Uuid::nil()),
            right_type: row.try_get("right_type").unwrap_or_default(),
            holder_entity_id: row.try_get("holder_entity_id").unwrap_or(Uuid::nil()),
            holder_name: row.try_get("holder_name").unwrap_or_default(),
            share_class_id: row.try_get("share_class_id").ok(),
            description: row.try_get("description").ok(),
            effective_from: row
                .try_get::<NaiveDate, _>("effective_from")
                .ok()
                .map(|d| d.to_string()),
            effective_to: row
                .try_get::<NaiveDate, _>("effective_to")
                .ok()
                .map(|d| d.to_string()),
        })
        .collect();

    Ok(Json(SpecialRightsResponse {
        issuer_entity_id: issuer_id,
        rights,
    }))
}

/// GET /api/capital/:issuer_id/reconcile
///
/// Returns reconciliation runs for an issuer.
async fn get_reconciliation_runs(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
    Query(query): Query<ReconciliationQuery>,
) -> Result<Json<ReconciliationRunsResponse>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(20) as i64;

    // Use dynamic query for new tables
    let rows = sqlx::query(
        r#"
        SELECT
            rr.run_id,
            rr.run_ts,
            rr.snapshot_a_id,
            rr.snapshot_b_id,
            (SELECT COUNT(*)::int FROM kyc.ownership_reconciliation_findings rf WHERE rf.run_id = rr.run_id) as total_findings,
            (SELECT COUNT(*)::int FROM kyc.ownership_reconciliation_findings rf WHERE rf.run_id = rr.run_id AND rf.status = 'OPEN') as open_findings
        FROM kyc.ownership_reconciliation_runs rr
        JOIN kyc.ownership_snapshots os ON os.snapshot_id = rr.snapshot_a_id
        WHERE os.issuer_entity_id = $1
        ORDER BY rr.run_ts DESC
        LIMIT $2
        "#,
    )
    .bind(issuer_id)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let runs: Vec<ReconciliationRunSummary> = rows
        .iter()
        .map(|row| {
            let open: i32 = row.try_get("open_findings").unwrap_or(0);
            let total: i32 = row.try_get("total_findings").unwrap_or(0);
            let status = if open == 0 { "RESOLVED" } else { "OPEN" }.to_string();

            ReconciliationRunSummary {
                run_id: row.try_get("run_id").unwrap_or(Uuid::nil()),
                run_ts: row
                    .try_get::<chrono::DateTime<Utc>, _>("run_ts")
                    .map(|t| t.to_string())
                    .unwrap_or_default(),
                snapshot_a_id: row.try_get("snapshot_a_id").unwrap_or(Uuid::nil()),
                snapshot_b_id: row.try_get("snapshot_b_id").unwrap_or(Uuid::nil()),
                total_findings: total,
                open_findings: open,
                status,
            }
        })
        .collect();

    Ok(Json(ReconciliationRunsResponse {
        issuer_entity_id: issuer_id,
        runs,
    }))
}

/// GET /api/capital/reconciliation/:run_id/findings
///
/// Returns findings for a specific reconciliation run.
async fn get_reconciliation_findings(
    State(pool): State<PgPool>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ReconciliationFindingsResponse>, (StatusCode, String)> {
    // Use dynamic query for new tables
    let rows = sqlx::query(
        r#"
        SELECT
            rf.finding_id,
            rf.holder_entity_id,
            e.name as holder_name,
            rf.finding_type,
            rf.severity,
            rf.source_a_value,
            rf.source_b_value,
            rf.delta_pct,
            rf.status,
            rf.resolution_note
        FROM kyc.ownership_reconciliation_findings rf
        JOIN "ob-poc".entities e ON e.entity_id = rf.holder_entity_id
        WHERE rf.run_id = $1
        ORDER BY rf.severity DESC, rf.delta_pct DESC NULLS LAST
        "#,
    )
    .bind(run_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let findings: Vec<ReconciliationFinding> = rows
        .iter()
        .map(|row| ReconciliationFinding {
            finding_id: row.try_get("finding_id").unwrap_or(Uuid::nil()),
            holder_entity_id: row.try_get("holder_entity_id").unwrap_or(Uuid::nil()),
            holder_name: row.try_get("holder_name").unwrap_or_default(),
            finding_type: row.try_get("finding_type").unwrap_or_default(),
            severity: row.try_get("severity").unwrap_or_default(),
            source_a_value: row
                .try_get::<sqlx::types::BigDecimal, _>("source_a_value")
                .ok()
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
            source_b_value: row
                .try_get::<sqlx::types::BigDecimal, _>("source_b_value")
                .ok()
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
            delta_pct: row
                .try_get::<sqlx::types::BigDecimal, _>("delta_pct")
                .ok()
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
            status: row.try_get("status").unwrap_or_default(),
            resolution_note: row.try_get("resolution_note").ok(),
        })
        .collect();

    Ok(Json(ReconciliationFindingsResponse { run_id, findings }))
}

/// GET /api/capital/:issuer_id/graph
///
/// Returns graph data for ownership visualization in the egui viewport.
async fn get_ownership_graph(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
    Query(query): Query<CapTableQuery>,
) -> Result<Json<OwnershipGraphResponse>, (StatusCode, String)> {
    let as_of = parse_as_of_date(query.as_of.as_deref())?;
    let basis = query.basis.as_deref().unwrap_or("VOTING");

    // Get issuer info
    let issuer = sqlx::query!(
        r#"SELECT entity_id, name FROM "ob-poc".entities WHERE entity_id = $1"#,
        issuer_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Issuer not found: {}", issuer_id),
        )
    })?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Add issuer node
    nodes.push(GraphNode {
        id: issuer_id.to_string(),
        node_type: "Entity".to_string(),
        label: issuer.name.clone(),
        data: serde_json::json!({
            "entity_type": "ISSUER",
            "is_root": true
        }),
    });

    // Get share classes and add as nodes
    let share_classes = get_share_classes_internal(&pool, issuer_id).await?;
    for sc in &share_classes {
        nodes.push(GraphNode {
            id: sc.share_class_id.to_string(),
            node_type: "ShareClass".to_string(),
            label: sc.name.clone(),
            data: serde_json::json!({
                "instrument_kind": sc.instrument_kind,
                "votes_per_unit": sc.votes_per_unit,
                "issued_units": sc.issued_units,
                "voting_weight_pct": sc.voting_weight_pct
            }),
        });

        // Edge from share class to issuer
        edges.push(GraphEdge {
            source: sc.share_class_id.to_string(),
            target: issuer_id.to_string(),
            edge_type: "IssuedBy".to_string(),
            data: serde_json::json!({}),
        });
    }

    // Get holder positions and add as nodes with edges
    let holder_rows = sqlx::query(r#"SELECT * FROM kyc.fn_holder_control_position($1, $2, $3)"#)
        .bind(issuer_id)
        .bind(as_of)
        .bind(basis)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for row in &holder_rows {
        let holder_id: Uuid = row.try_get("holder_entity_id").unwrap_or(Uuid::nil());
        let holder_name: String = row.try_get("holder_name").unwrap_or_default();
        let holder_type: String = row.try_get("holder_type").unwrap_or_default();
        let voting_pct: f64 = row.try_get("voting_pct").unwrap_or(0.0);
        let economic_pct: f64 = row.try_get("economic_pct").unwrap_or(0.0);
        let has_control: bool = row.try_get("has_control").unwrap_or(false);

        nodes.push(GraphNode {
            id: holder_id.to_string(),
            node_type: "Entity".to_string(),
            label: holder_name,
            data: serde_json::json!({
                "entity_type": holder_type,
                "voting_pct": voting_pct,
                "economic_pct": economic_pct,
                "has_control": has_control
            }),
        });

        // Edge from holder to issuer (ownership)
        edges.push(GraphEdge {
            source: holder_id.to_string(),
            target: issuer_id.to_string(),
            edge_type: "Owns".to_string(),
            data: serde_json::json!({
                "voting_pct": voting_pct,
                "economic_pct": economic_pct,
                "has_control": has_control
            }),
        });
    }

    Ok(Json(OwnershipGraphResponse {
        issuer_entity_id: issuer_id,
        issuer_name: issuer.name,
        as_of_date: as_of.to_string(),
        nodes,
        edges,
    }))
}

/// GET /api/capital/:entity_id/economic-exposure
///
/// Returns bounded look-through economic exposure from an entity.
/// Uses recursive computation with configurable depth, min percentage,
/// and role-profile-aware stop conditions.
async fn get_economic_exposure(
    State(pool): State<PgPool>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<EconomicExposureQuery>,
) -> Result<Json<EconomicExposureResponse>, (StatusCode, String)> {
    let as_of = parse_as_of_date(query.as_of.as_deref())?;
    let max_depth = query.max_depth.unwrap_or(6);
    let min_pct = query.min_pct.unwrap_or(0.0001);
    let max_rows = query.max_rows.unwrap_or(200);

    // Get root entity name
    let root_entity = sqlx::query!(
        r#"SELECT entity_id, name FROM "ob-poc".entities WHERE entity_id = $1"#,
        entity_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Entity not found: {}", entity_id),
        )
    })?;

    // Call the economic exposure SQL function
    let rows = sqlx::query(
        r#"
        SELECT
            root_entity_id,
            leaf_entity_id,
            leaf_name,
            cumulative_pct,
            depth,
            path_entities,
            path_names,
            stopped_reason
        FROM kyc.fn_compute_economic_exposure($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(entity_id)
    .bind(as_of)
    .bind(max_depth)
    .bind(Decimal::try_from(min_pct).unwrap_or_default())
    .bind(max_rows)
    .bind(query.stop_on_no_bo_data)
    .bind(query.stop_on_policy_none)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let exposures: Vec<ExposureNode> = rows
        .iter()
        .map(|row| {
            let cumulative_pct_bd: sqlx::types::BigDecimal =
                row.try_get("cumulative_pct").unwrap_or_default();
            let cumulative_pct_decimal = bigdecimal_to_decimal(cumulative_pct_bd);

            ExposureNode {
                leaf_entity_id: row.try_get("leaf_entity_id").unwrap_or(Uuid::nil()),
                leaf_name: row.try_get("leaf_name").unwrap_or_default(),
                cumulative_pct: cumulative_pct_decimal
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0),
                depth: row.try_get("depth").unwrap_or(0),
                path_entities: row.try_get("path_entities").unwrap_or_default(),
                path_names: row.try_get("path_names").unwrap_or_default(),
                stopped_reason: row.try_get("stopped_reason").unwrap_or_default(),
            }
        })
        .collect();

    Ok(Json(EconomicExposureResponse {
        root_entity_id: entity_id,
        root_name: root_entity.name,
        as_of_date: as_of.to_string(),
        exposures,
        parameters: ExposureParameters {
            max_depth,
            min_pct,
            max_rows,
            stop_on_no_bo_data: query.stop_on_no_bo_data,
            stop_on_policy_none: query.stop_on_policy_none,
        },
    }))
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn parse_as_of_date(as_of: Option<&str>) -> Result<NaiveDate, (StatusCode, String)> {
    match as_of {
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid date format: {}. Use YYYY-MM-DD", s),
            )
        }),
        None => Ok(Utc::now().date_naive()),
    }
}

/// Convert BigDecimal to rust_decimal::Decimal via string parsing
fn bigdecimal_to_decimal(bd: sqlx::types::BigDecimal) -> Decimal {
    use std::str::FromStr;
    Decimal::from_str(&bd.to_string()).unwrap_or_default()
}

/// Convert BigDecimal to Decimal with a default value
fn bigdecimal_to_decimal_or(bd: sqlx::types::BigDecimal, default: Decimal) -> Decimal {
    use std::str::FromStr;
    Decimal::from_str(&bd.to_string()).unwrap_or(default)
}

async fn get_share_classes_internal(
    pool: &PgPool,
    issuer_id: Uuid,
) -> Result<Vec<ShareClassSummary>, (StatusCode, String)> {
    // Join with share_class_supply for issued_units
    let rows = sqlx::query!(
        r#"
        SELECT
            sc.id as share_class_id,
            sc.name,
            COALESCE(sc.instrument_kind, 'FUND_UNIT') as "instrument_kind!",
            COALESCE(sc.votes_per_unit, 1.0) as "votes_per_unit!",
            COALESCE(scs.issued_units, 0) as "issued_units!"
        FROM kyc.share_classes sc
        LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
            AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id)
        WHERE sc.issuer_entity_id = $1
          AND sc.status = 'active'
        ORDER BY sc.name
        "#,
        issuer_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Calculate total votes for weight calculation
    let total_all_votes: f64 = rows
        .iter()
        .map(|r| {
            let issued: f64 = r.issued_units.to_string().parse().unwrap_or(0.0);
            let votes_per: f64 = r.votes_per_unit.to_string().parse().unwrap_or(1.0);
            issued * votes_per
        })
        .sum();

    let mut classes = Vec::new();
    for row in rows {
        let issued_units: f64 = row.issued_units.to_string().parse().unwrap_or(0.0);
        let votes_per_unit: f64 = row.votes_per_unit.to_string().parse().unwrap_or(1.0);
        let total_votes = issued_units * votes_per_unit;

        let voting_weight_pct = if total_all_votes > 0.0 {
            (total_votes / total_all_votes) * 100.0
        } else {
            0.0
        };

        // Get identifiers for this share class
        let identifiers = get_share_class_identifiers(pool, row.share_class_id).await?;

        classes.push(ShareClassSummary {
            share_class_id: row.share_class_id,
            name: row.name,
            instrument_kind: row.instrument_kind,
            votes_per_unit,
            issued_units,
            total_votes,
            voting_weight_pct,
            identifiers,
        });
    }

    Ok(classes)
}

async fn get_share_class_identifiers(
    pool: &PgPool,
    share_class_id: Uuid,
) -> Result<Vec<IdentifierPair>, (StatusCode, String)> {
    let rows = sqlx::query!(
        r#"
        SELECT scheme_code as scheme, identifier_value as value
        FROM kyc.share_class_identifiers
        WHERE share_class_id = $1
        "#,
        share_class_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| IdentifierPair {
            scheme: r.scheme,
            value: r.value,
        })
        .collect())
}

async fn get_holder_special_rights(
    pool: &PgPool,
    issuer_id: Uuid,
    holder_id: Uuid,
) -> Result<Vec<String>, (StatusCode, String)> {
    // Use dynamic query for new table
    let rows = sqlx::query(
        r#"
        SELECT sr.right_type
        FROM kyc.special_rights sr
        WHERE sr.holder_entity_id = $2
          AND sr.share_class_id IN (SELECT id FROM kyc.share_classes WHERE issuer_entity_id = $1)
          AND (sr.effective_to IS NULL OR sr.effective_to > CURRENT_DATE)
        "#,
    )
    .bind(issuer_id)
    .bind(holder_id)
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(rows
        .iter()
        .filter_map(|r| r.try_get::<String, _>("right_type").ok())
        .collect())
}

// =============================================================================
// INVESTOR REGISTER VISUALIZATION ENDPOINTS
// =============================================================================

/// Query parameters for investor register view (from URL query string)
#[derive(Debug, Deserialize)]
pub struct InvestorRegisterUrlQuery {
    /// Filter to specific share class
    pub share_class_id: Option<Uuid>,
    /// As-of date (YYYY-MM-DD)
    pub as_of: Option<String>,
    /// Include dilution instruments
    #[serde(default)]
    pub include_dilution: bool,
    /// Control basis: VOTES or ECONOMIC
    pub basis: Option<String>,
}

/// Query parameters for investor list (paginated drill-down)
#[derive(Debug, Deserialize)]
pub struct InvestorListUrlQuery {
    /// Filter to specific share class
    pub share_class_id: Option<Uuid>,
    /// As-of date
    pub as_of: Option<String>,
    /// Page number (1-indexed)
    pub page: Option<i32>,
    /// Page size (default 50, max 200)
    pub page_size: Option<i32>,
    /// Filter by investor type
    pub investor_type: Option<String>,
    /// Filter by KYC status
    pub kyc_status: Option<String>,
    /// Filter by jurisdiction
    pub jurisdiction: Option<String>,
    /// Search by name
    pub search: Option<String>,
    /// Minimum units
    pub min_units: Option<f64>,
    /// Sort field
    pub sort_by: Option<String>,
    /// Sort direction: asc or desc
    pub sort_dir: Option<String>,
}

/// GET /api/capital/:issuer_id/investors
///
/// Returns investor register view with:
/// - Control holders as individual nodes (above threshold or with rights)
/// - Aggregate node for remaining investors (below threshold)
/// - Breakdown data for drill-down
async fn get_investor_register(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
    Query(params): Query<InvestorRegisterUrlQuery>,
) -> Result<Json<InvestorRegisterView>, (StatusCode, String)> {
    let as_of = parse_as_of_date(params.as_of.as_deref())?;
    let basis = params.basis.as_deref().unwrap_or("VOTES");

    // 1. Get issuer info
    let issuer = get_issuer_summary(&pool, issuer_id).await?;

    // 2. Get threshold config (from issuer_control_config or defaults)
    let thresholds = get_threshold_config(&pool, issuer_id, as_of).await?;

    // 3. Compute all holder positions
    let all_positions = get_all_holder_positions(&pool, issuer_id, as_of, basis).await?;

    // 4. Partition by threshold: control holders vs aggregate
    let (control_holders, aggregate_positions) =
        partition_by_threshold(&all_positions, &thresholds, &pool, issuer_id).await?;

    // 5. Build aggregate node if there are aggregate investors
    let aggregate = if !aggregate_positions.is_empty() {
        Some(build_aggregate_node(&aggregate_positions, &thresholds))
    } else {
        None
    };

    // 6. Get total supply for context
    let total_issued = get_total_issued_units(&pool, issuer_id, params.share_class_id).await?;

    // 7. Check for dilution data
    let has_dilution = check_has_dilution_data(&pool, issuer_id).await?;

    Ok(Json(InvestorRegisterView {
        issuer,
        share_class_filter: params.share_class_id,
        as_of_date: as_of,
        thresholds,
        control_holders,
        aggregate,
        total_investor_count: all_positions.len() as i32,
        total_issued_units: total_issued,
        has_dilution_data: has_dilution,
    }))
}

/// GET /api/capital/:issuer_id/investors/list
///
/// Returns paginated list of investors for drill-down view.
async fn get_investor_list(
    State(pool): State<PgPool>,
    Path(issuer_id): Path<Uuid>,
    Query(params): Query<InvestorListUrlQuery>,
) -> Result<Json<InvestorListResponse>, (StatusCode, String)> {
    let as_of = parse_as_of_date(params.as_of.as_deref())?;
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * page_size;

    // Build filter conditions
    let filters = InvestorFilters {
        investor_type: params.investor_type.clone(),
        kyc_status: params.kyc_status.clone(),
        jurisdiction: params.jurisdiction.clone(),
        search: params.search.clone(),
        min_units: params
            .min_units
            .map(|f| Decimal::try_from(f).unwrap_or_default()),
    };

    // Build sort clause
    let sort_field = params.sort_by.as_deref().unwrap_or("name");
    let sort_dir = params.sort_dir.as_deref().unwrap_or("asc");
    let order_clause = match sort_field {
        "units" => format!("h.units {}", sort_dir),
        "pct" | "economic_pct" => format!("economic_pct {}", sort_dir),
        "kyc_status" => format!("i.kyc_status {}", sort_dir),
        "acquisition_date" => format!("h.acquisition_date {} NULLS LAST", sort_dir),
        _ => format!("e.name {}", sort_dir),
    };

    // Query for total count (with filters)
    let (count_query, count_params) = build_investor_count_query(&filters);
    let mut count_q = sqlx::query_scalar(&count_query)
        .bind(issuer_id)
        .bind(as_of);
    for p in &count_params {
        count_q = count_q.bind(p.as_str());
    }
    let total_items: i64 = count_q
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total_pages = ((total_items as f64) / (page_size as f64)).ceil() as i32;

    // Query for page items
    let (items_query, list_params) = build_investor_list_query(&filters, &order_clause, page_size, offset);
    let mut items_q = sqlx::query(&items_query)
        .bind(issuer_id)
        .bind(as_of)
        .bind(page_size as i64)
        .bind(offset as i64);
    for p in &list_params {
        items_q = items_q.bind(p.as_str());
    }
    let rows = items_q
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<InvestorListItem> = rows
        .iter()
        .map(|row| InvestorListItem {
            entity_id: row.try_get("entity_id").unwrap_or(Uuid::nil()),
            name: row.try_get("name").unwrap_or_default(),
            entity_type: row.try_get("entity_type").unwrap_or_default(),
            investor_type: row.try_get("investor_type").ok(),
            units: row
                .try_get::<sqlx::types::BigDecimal, _>("units")
                .map(bigdecimal_to_decimal)
                .unwrap_or_default(),
            economic_pct: row
                .try_get::<sqlx::types::BigDecimal, _>("economic_pct")
                .map(bigdecimal_to_decimal)
                .unwrap_or_default(),
            voting_pct: row
                .try_get::<sqlx::types::BigDecimal, _>("voting_pct")
                .map(bigdecimal_to_decimal)
                .unwrap_or_default(),
            kyc_status: row.try_get("kyc_status").unwrap_or_default(),
            jurisdiction: row.try_get("jurisdiction").ok(),
            acquisition_date: row.try_get("acquisition_date").ok(),
        })
        .collect();

    Ok(Json(InvestorListResponse {
        items,
        pagination: PaginationInfo {
            page,
            page_size,
            total_items: total_items as i32,
            total_pages,
        },
        filters,
    }))
}

// =============================================================================
// INVESTOR REGISTER HELPER FUNCTIONS
// =============================================================================

/// Get issuer summary for investor register view
async fn get_issuer_summary(
    pool: &PgPool,
    issuer_id: Uuid,
) -> Result<IssuerSummary, (StatusCode, String)> {
    let row = sqlx::query!(
        r#"
        SELECT
            e.entity_id,
            e.name,
            et.type_code as entity_type,
            elc.jurisdiction as "jurisdiction?",
            ei.identifier_value as "lei?"
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
        LEFT JOIN "ob-poc".entity_identifiers ei ON ei.entity_id = e.entity_id AND ei.identifier_type = 'LEI'
        WHERE e.entity_id = $1
        "#,
        issuer_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Issuer not found: {}", issuer_id),
        )
    })?;

    Ok(IssuerSummary {
        entity_id: row.entity_id,
        name: row.name,
        entity_type: row.entity_type.unwrap_or_default(),
        jurisdiction: row.jurisdiction,
        lei: row.lei,
    })
}

/// Get threshold config from issuer_control_config or use defaults
async fn get_threshold_config(
    pool: &PgPool,
    issuer_id: Uuid,
    as_of: NaiveDate,
) -> Result<ThresholdConfig, (StatusCode, String)> {
    let row = sqlx::query(
        r#"
        SELECT
            disclosure_threshold_pct,
            material_threshold_pct,
            significant_threshold_pct,
            control_threshold_pct,
            control_basis
        FROM kyc.issuer_control_config
        WHERE issuer_entity_id = $1
          AND effective_from <= $2
          AND (effective_to IS NULL OR effective_to > $2)
        ORDER BY effective_from DESC
        LIMIT 1
        "#,
    )
    .bind(issuer_id)
    .bind(as_of)
    .fetch_optional(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok(ThresholdConfig {
            disclosure_pct: r
                .try_get::<sqlx::types::BigDecimal, _>("disclosure_threshold_pct")
                .map(|d| bigdecimal_to_decimal_or(d, Decimal::new(5, 0)))
                .unwrap_or(Decimal::new(5, 0)),
            material_pct: r
                .try_get::<sqlx::types::BigDecimal, _>("material_threshold_pct")
                .map(|d| bigdecimal_to_decimal_or(d, Decimal::new(10, 0)))
                .unwrap_or(Decimal::new(10, 0)),
            significant_pct: r
                .try_get::<sqlx::types::BigDecimal, _>("significant_threshold_pct")
                .map(|d| bigdecimal_to_decimal_or(d, Decimal::new(25, 0)))
                .unwrap_or(Decimal::new(25, 0)),
            control_pct: r
                .try_get::<sqlx::types::BigDecimal, _>("control_threshold_pct")
                .map(|d| bigdecimal_to_decimal_or(d, Decimal::new(50, 0)))
                .unwrap_or(Decimal::new(50, 0)),
            control_basis: r.try_get("control_basis").unwrap_or("VOTES".to_string()),
        }),
        None => Ok(ThresholdConfig::default()),
    }
}

/// Internal holder position for partitioning
#[derive(Debug, Clone)]
struct HolderPositionInternal {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    investor_type: Option<String>,
    units: Decimal,
    voting_pct: Decimal,
    economic_pct: Decimal,
    kyc_status: String,
    jurisdiction: Option<String>,
}

/// Get all holder positions for an issuer
async fn get_all_holder_positions(
    pool: &PgPool,
    issuer_id: Uuid,
    as_of: NaiveDate,
    basis: &str,
) -> Result<Vec<HolderPositionInternal>, (StatusCode, String)> {
    let rows = sqlx::query(
        r#"
        SELECT
            e.entity_id,
            e.name,
            et.type_code as entity_type,
            i.investor_type,
            COALESCE(SUM(h.units), 0) as units,
            COALESCE(hcp.voting_pct, 0) as voting_pct,
            COALESCE(hcp.economic_pct, 0) as economic_pct,
            COALESCE(i.kyc_status, 'UNKNOWN') as kyc_status,
            i.tax_jurisdiction as jurisdiction
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
        JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        LEFT JOIN kyc.investors i ON i.entity_id = h.investor_entity_id
        LEFT JOIN LATERAL (
            SELECT voting_pct, economic_pct
            FROM kyc.fn_holder_control_position($1, $2, $3)
            WHERE holder_entity_id = h.investor_entity_id
        ) hcp ON true
        WHERE sc.issuer_entity_id = $1
          AND h.status = 'active'
        GROUP BY e.entity_id, e.name, et.type_code, i.investor_type,
                 hcp.voting_pct, hcp.economic_pct, i.kyc_status, i.tax_jurisdiction
        ORDER BY hcp.voting_pct DESC NULLS LAST
        "#,
    )
    .bind(issuer_id)
    .bind(as_of)
    .bind(basis)
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(rows
        .iter()
        .map(|row| HolderPositionInternal {
            entity_id: row.try_get("entity_id").unwrap_or(Uuid::nil()),
            name: row.try_get("name").unwrap_or_default(),
            entity_type: row.try_get("entity_type").unwrap_or_default(),
            investor_type: row.try_get("investor_type").ok(),
            units: row
                .try_get::<sqlx::types::BigDecimal, _>("units")
                .map(bigdecimal_to_decimal)
                .unwrap_or_default(),
            voting_pct: row
                .try_get::<sqlx::types::BigDecimal, _>("voting_pct")
                .map(bigdecimal_to_decimal)
                .unwrap_or_default(),
            economic_pct: row
                .try_get::<sqlx::types::BigDecimal, _>("economic_pct")
                .map(bigdecimal_to_decimal)
                .unwrap_or_default(),
            kyc_status: row.try_get("kyc_status").unwrap_or_default(),
            jurisdiction: row.try_get("jurisdiction").ok(),
        })
        .collect())
}

/// Partition holders by threshold into control holders and aggregate
async fn partition_by_threshold(
    positions: &[HolderPositionInternal],
    thresholds: &ThresholdConfig,
    pool: &PgPool,
    issuer_id: Uuid,
) -> Result<(Vec<ControlHolderNode>, Vec<HolderPositionInternal>), (StatusCode, String)> {
    let mut control_holders = Vec::new();
    let mut aggregate_positions = Vec::new();

    for pos in positions {
        // Check if above disclosure threshold
        let pct_to_check = if thresholds.control_basis == "ECONOMIC" {
            pos.economic_pct
        } else {
            pos.voting_pct
        };

        let above_disclosure = pct_to_check >= thresholds.disclosure_pct;
        let has_significant = pct_to_check >= thresholds.significant_pct;
        let has_control = pct_to_check >= thresholds.control_pct;

        // Check for special rights
        let (board_seats, veto_rights, other_rights) =
            get_holder_rights(pool, issuer_id, pos.entity_id).await?;

        let has_special_rights = board_seats > 0 || !veto_rights.is_empty();

        if above_disclosure || has_special_rights {
            let mut node = ControlHolderNode {
                entity_id: pos.entity_id,
                name: pos.name.clone(),
                entity_type: pos.entity_type.clone(),
                investor_type: pos.investor_type.clone(),
                units: pos.units,
                voting_pct: pos.voting_pct,
                economic_pct: pos.economic_pct,
                has_control,
                has_significant_influence: has_significant,
                above_disclosure,
                board_seats,
                veto_rights,
                other_rights,
                inclusion_reason: String::new(),
                kyc_status: pos.kyc_status.clone(),
                hierarchy_depth: 0,
            };
            node.inclusion_reason = node.compute_inclusion_reason(thresholds);
            control_holders.push(node);
        } else {
            aggregate_positions.push(pos.clone());
        }
    }

    Ok((control_holders, aggregate_positions))
}

/// Get special rights for a holder
async fn get_holder_rights(
    pool: &PgPool,
    issuer_id: Uuid,
    holder_id: Uuid,
) -> Result<(i32, Vec<String>, Vec<String>), (StatusCode, String)> {
    let rows = sqlx::query(
        r#"
        SELECT right_type, board_seats
        FROM kyc.special_rights
        WHERE issuer_entity_id = $1
          AND holder_entity_id = $2
          AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
        "#,
    )
    .bind(issuer_id)
    .bind(holder_id)
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut board_seats = 0;
    let mut veto_rights = Vec::new();
    let mut other_rights = Vec::new();

    for row in rows {
        let right_type: String = row.try_get("right_type").unwrap_or_default();
        let seats: i32 = row.try_get("board_seats").unwrap_or(0);

        if right_type == "BOARD_APPOINTMENT" {
            board_seats += seats.max(1);
        } else if right_type.starts_with("VETO_") {
            veto_rights.push(right_type);
        } else {
            other_rights.push(right_type);
        }
    }

    Ok((board_seats, veto_rights, other_rights))
}

/// Build aggregate node from below-threshold positions
fn build_aggregate_node(
    positions: &[HolderPositionInternal],
    _thresholds: &ThresholdConfig,
) -> AggregateInvestorsNode {
    let investor_count = positions.len() as i32;
    let total_units: Decimal = positions.iter().map(|p| p.units).sum();
    let voting_pct: Decimal = positions.iter().map(|p| p.voting_pct).sum();
    let economic_pct: Decimal = positions.iter().map(|p| p.economic_pct).sum();

    // Build breakdown by investor type
    let by_type = build_breakdown_by_field(positions, |p| {
        p.investor_type.clone().unwrap_or("UNKNOWN".to_string())
    });

    // Build breakdown by KYC status
    let by_kyc_status = build_breakdown_by_field(positions, |p| p.kyc_status.clone());

    // Build breakdown by jurisdiction (top 10)
    let mut by_jurisdiction = build_breakdown_by_field(positions, |p| {
        p.jurisdiction.clone().unwrap_or("--".to_string())
    });
    by_jurisdiction.truncate(10);

    let display_label = AggregateInvestorsNode::make_display_label(investor_count, economic_pct);

    AggregateInvestorsNode {
        investor_count,
        total_units,
        voting_pct,
        economic_pct,
        by_type,
        by_kyc_status,
        by_jurisdiction,
        can_drill_down: investor_count <= 10000,
        page_size: 50,
        display_label,
    }
}

/// Build breakdown by a field extractor
fn build_breakdown_by_field<F>(
    positions: &[HolderPositionInternal],
    field_fn: F,
) -> Vec<AggregateBreakdown>
where
    F: Fn(&HolderPositionInternal) -> String,
{
    use std::collections::HashMap;

    let mut groups: HashMap<String, (i32, Decimal, Decimal)> = HashMap::new();

    for pos in positions {
        let key = field_fn(pos);
        let entry = groups
            .entry(key)
            .or_insert((0, Decimal::ZERO, Decimal::ZERO));
        entry.0 += 1;
        entry.1 += pos.units;
        entry.2 += pos.economic_pct;
    }

    let mut breakdowns: Vec<AggregateBreakdown> = groups
        .into_iter()
        .map(|(key, (count, units, pct))| AggregateBreakdown {
            key: key.clone(),
            label: format_breakdown_label(&key),
            count,
            units,
            pct,
        })
        .collect();

    // Sort by count descending
    breakdowns.sort_by(|a, b| b.count.cmp(&a.count));
    breakdowns
}

/// Format breakdown label for display
fn format_breakdown_label(key: &str) -> String {
    match key {
        "INSTITUTIONAL" => "Institutional".to_string(),
        "PROFESSIONAL" => "Professional".to_string(),
        "RETAIL" => "Retail".to_string(),
        "NOMINEE" => "Nominee".to_string(),
        "APPROVED" => "KYC Approved".to_string(),
        "PENDING" => "KYC Pending".to_string(),
        "REJECTED" => "KYC Rejected".to_string(),
        "EXPIRED" => "KYC Expired".to_string(),
        "UNKNOWN" => "Unknown".to_string(),
        "--" => "No Jurisdiction".to_string(),
        other => other.to_string(),
    }
}

/// Get total issued units for an issuer
async fn get_total_issued_units(
    pool: &PgPool,
    issuer_id: Uuid,
    share_class_filter: Option<Uuid>,
) -> Result<Decimal, (StatusCode, String)> {
    let query = if share_class_filter.is_some() {
        r#"
        SELECT COALESCE(SUM(scs.issued_units), 0) as total
        FROM kyc.share_class_supply scs
        JOIN kyc.share_classes sc ON sc.id = scs.share_class_id
        WHERE sc.issuer_entity_id = $1
          AND scs.share_class_id = $2
          AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = scs.share_class_id)
        "#
    } else {
        r#"
        SELECT COALESCE(SUM(scs.issued_units), 0) as total
        FROM kyc.share_class_supply scs
        JOIN kyc.share_classes sc ON sc.id = scs.share_class_id
        WHERE sc.issuer_entity_id = $1
          AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = scs.share_class_id)
        "#
    };

    let row = if let Some(class_id) = share_class_filter {
        sqlx::query(query)
            .bind(issuer_id)
            .bind(class_id)
            .fetch_one(pool)
            .await
    } else {
        sqlx::query(query).bind(issuer_id).fetch_one(pool).await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(row
        .try_get::<sqlx::types::BigDecimal, _>("total")
        .map(bigdecimal_to_decimal)
        .unwrap_or_default())
}

/// Check if issuer has dilution instruments
async fn check_has_dilution_data(
    pool: &PgPool,
    issuer_id: Uuid,
) -> Result<bool, (StatusCode, String)> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM kyc.dilution_instruments
        WHERE issuer_entity_id = $1 AND status = 'ACTIVE'
        "#,
    )
    .bind(issuer_id)
    .fetch_one(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(count > 0)
}

/// Build count query with parameterized filters.
///
/// Returns `(sql, filter_values)` where filter_values should be bound
/// after the base parameters ($1=issuer_id, $2=as_of).
fn build_investor_count_query(filters: &InvestorFilters) -> (String, Vec<String>) {
    let mut query = String::from(
        r#"
        SELECT COUNT(DISTINCT h.investor_entity_id)
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
        LEFT JOIN kyc.investors i ON i.entity_id = h.investor_entity_id
        WHERE sc.issuer_entity_id = $1
          AND h.status = 'active'
        "#,
    );

    let mut params: Vec<String> = Vec::new();
    // $1=issuer_id, $2=as_of are bound at the call site
    let mut idx = 3u32;

    if let Some(ref v) = filters.investor_type {
        query.push_str(&format!(" AND i.investor_type = ${idx}"));
        params.push(v.clone());
        idx += 1;
    }
    if let Some(ref v) = filters.kyc_status {
        query.push_str(&format!(" AND i.kyc_status = ${idx}"));
        params.push(v.clone());
        idx += 1;
    }
    if let Some(ref v) = filters.jurisdiction {
        query.push_str(&format!(" AND i.tax_jurisdiction = ${idx}"));
        params.push(v.clone());
        idx += 1;
    }
    if let Some(ref v) = filters.search {
        query.push_str(&format!(" AND e.name ILIKE ${idx}"));
        params.push(format!("%{v}%"));
        let _ = idx; // suppress unused warning
    }

    (query, params)
}

/// Build list query with parameterized filters, ordering, and pagination.
///
/// Returns `(sql, filter_values)` where filter_values should be bound
/// after the base parameters ($1=issuer_id, $2=as_of, $3=limit, $4=offset).
fn build_investor_list_query(
    filters: &InvestorFilters,
    order_clause: &str,
    _page_size: i32,
    _offset: i32,
) -> (String, Vec<String>) {
    let mut query = String::from(
        r#"
        SELECT
            e.entity_id,
            e.name,
            et.type_code as entity_type,
            i.investor_type,
            SUM(h.units) as units,
            0::numeric as economic_pct,
            0::numeric as voting_pct,
            COALESCE(i.kyc_status, 'UNKNOWN') as kyc_status,
            i.tax_jurisdiction as jurisdiction,
            MIN(h.acquisition_date) as acquisition_date
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
        JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        LEFT JOIN kyc.investors i ON i.entity_id = h.investor_entity_id
        WHERE sc.issuer_entity_id = $1
          AND h.status = 'active'
        "#,
    );

    let mut params: Vec<String> = Vec::new();
    // $1=issuer_id, $2=as_of, $3=limit, $4=offset are bound at call site
    let mut idx = 5u32;

    if let Some(ref v) = filters.investor_type {
        query.push_str(&format!(" AND i.investor_type = ${idx}"));
        params.push(v.clone());
        idx += 1;
    }
    if let Some(ref v) = filters.kyc_status {
        query.push_str(&format!(" AND i.kyc_status = ${idx}"));
        params.push(v.clone());
        idx += 1;
    }
    if let Some(ref v) = filters.jurisdiction {
        query.push_str(&format!(" AND i.tax_jurisdiction = ${idx}"));
        params.push(v.clone());
        idx += 1;
    }
    if let Some(ref v) = filters.search {
        query.push_str(&format!(" AND e.name ILIKE ${idx}"));
        params.push(format!("%{v}%"));
        let _ = idx;
    }

    query.push_str(" GROUP BY e.entity_id, e.name, et.type_code, i.investor_type, i.kyc_status, i.tax_jurisdiction");
    query.push_str(&format!(" ORDER BY {}", order_clause));
    query.push_str(" LIMIT $3 OFFSET $4");

    (query, params)
}

// =============================================================================
// ROUTER
// =============================================================================

/// Create the capital structure router
pub fn create_capital_router(pool: PgPool) -> Router {
    Router::new()
        // Cap table endpoints
        .route("/api/capital/:issuer_id/cap-table", get(get_cap_table))
        .route(
            "/api/capital/:issuer_id/share-classes",
            get(get_share_classes),
        )
        .route("/api/capital/:issuer_id/supply", get(get_supply))
        // Control analysis
        .route(
            "/api/capital/:issuer_id/control",
            get(get_control_positions),
        )
        .route(
            "/api/capital/:issuer_id/special-rights",
            get(get_special_rights),
        )
        // Investor register visualization
        .route(
            "/api/capital/:issuer_id/investors",
            get(get_investor_register),
        )
        .route(
            "/api/capital/:issuer_id/investors/list",
            get(get_investor_list),
        )
        // Reconciliation
        .route(
            "/api/capital/:issuer_id/reconcile",
            get(get_reconciliation_runs),
        )
        .route(
            "/api/capital/reconciliation/:run_id/findings",
            get(get_reconciliation_findings),
        )
        // Graph data (for viewport)
        .route("/api/capital/:issuer_id/graph", get(get_ownership_graph))
        // Economic exposure look-through
        .route(
            "/api/capital/:entity_id/economic-exposure",
            get(get_economic_exposure),
        )
        .with_state(pool)
}
