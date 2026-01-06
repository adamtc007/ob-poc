//! Universe/Galaxy Navigation API Routes
//!
//! Provides endpoints for galaxy-style navigation:
//! - GET /api/universe - All CBUs clustered by type (jurisdiction, client, risk, product)
//! - GET /api/cluster/:type/:id - Expanded cluster with CBU details
//!
//! These endpoints return shared types from ob-poc-types/galaxy.rs
//! for use by the egui WASM client's NavigationService.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use ob_poc_types::galaxy::{
    Anomaly, AnomalySeverity, CbuEdge, CbuNode, ClusterDetailGraph, ClusterNode, ClusterType,
    NavigationAction, PreviewData, PreviewItem, PreviewType, RiskRating, RiskSummary,
    UniverseGraph, UniverseStats,
};

use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// QUERY PARAMETERS
// ============================================================================

/// Query parameters for universe endpoint
#[derive(Debug, Deserialize)]
pub struct UniverseQuery {
    /// How to cluster CBUs: jurisdiction (default), client, risk, product
    #[serde(default)]
    pub cluster_by: Option<String>,
}

/// Query parameters for cluster detail endpoint
#[derive(Debug, Deserialize)]
pub struct ClusterQuery {
    /// Include shared entities between CBUs
    #[serde(default)]
    pub include_shared: Option<bool>,
}

// ============================================================================
// DATABASE ROW TYPES
// ============================================================================

/// Raw CBU row from database
#[derive(Debug, Clone, sqlx::FromRow)]
struct CbuRow {
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    client_type: Option<String>,
    commercial_client_entity_id: Option<Uuid>,
    commercial_client_name: Option<String>,
    risk_status: Option<String>,
    entity_count: Option<i64>,
    pending_kyc: Option<bool>,
}

/// Cluster aggregation row
#[derive(Debug)]
struct ClusterAggregation {
    cluster_id: String,
    label: String,
    cluster_type: ClusterType,
    cbus: Vec<CbuRow>,
}

// ============================================================================
// GET /api/universe
// ============================================================================

/// GET /api/universe?cluster_by=jurisdiction
///
/// Returns all CBUs clustered by the specified dimension.
/// Default clustering is by jurisdiction.
///
/// Response: UniverseGraph with clusters, edges, and stats
pub async fn get_universe(
    State(pool): State<PgPool>,
    Query(params): Query<UniverseQuery>,
) -> Result<Json<UniverseGraph>, (StatusCode, String)> {
    let cluster_type = parse_cluster_type(params.cluster_by.as_deref());

    // Fetch all CBUs with their clustering attributes
    let cbus = fetch_all_cbus(&pool).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch CBUs: {}", e),
        )
    })?;

    // Group CBUs into clusters based on cluster_type
    let clusters = group_cbus_into_clusters(&cbus, cluster_type);

    // Build cluster nodes with aggregated stats
    let cluster_nodes: Vec<ClusterNode> = clusters
        .iter()
        .enumerate()
        .map(|(idx, agg)| build_cluster_node(agg, idx))
        .collect();

    // Calculate cluster edges (shared ManCos, cross-ownership, etc.)
    // For now, return empty edges - will implement in Phase 2
    let cluster_edges = vec![];

    // Calculate universe stats
    let stats = calculate_universe_stats(&cbus, &cluster_nodes);

    Ok(Json(UniverseGraph {
        clusters: cluster_nodes,
        cluster_edges,
        stats,
        cluster_type,
    }))
}

// ============================================================================
// GET /api/cluster/:type/:id
// ============================================================================

/// GET /api/cluster/:type/:id
///
/// Returns expanded cluster with CBU nodes.
/// Type is one of: jurisdiction, client, risk, product
/// ID is the cluster identifier (e.g., "LU", "allianz", "HIGH")
pub async fn get_cluster_detail(
    State(pool): State<PgPool>,
    Path((cluster_type_str, cluster_id)): Path<(String, String)>,
    Query(params): Query<ClusterQuery>,
) -> Result<Json<ClusterDetailGraph>, (StatusCode, String)> {
    let cluster_type = parse_cluster_type(Some(&cluster_type_str));

    // Fetch CBUs matching this cluster
    let cbus = fetch_cluster_cbus(&pool, cluster_type, &cluster_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch cluster CBUs: {}", e),
            )
        })?;

    if cbus.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!(
                "No CBUs found for cluster {}:{}",
                cluster_type_str, cluster_id
            ),
        ));
    }

    // Build cluster node
    let agg = ClusterAggregation {
        cluster_id: format!("{}:{}", cluster_type_str, cluster_id),
        label: cluster_id.clone(),
        cluster_type,
        cbus: cbus.clone(),
    };
    let cluster = build_cluster_node(&agg, 0);

    // Build CBU nodes with positions
    let cbu_nodes: Vec<CbuNode> = cbus
        .iter()
        .enumerate()
        .map(|(idx, cbu)| build_cbu_node(cbu, idx, cbus.len()))
        .collect();

    // Calculate CBU edges (shared entities)
    let cbu_edges = if params.include_shared.unwrap_or(false) {
        calculate_cbu_edges(&pool, &cbus).await.unwrap_or_default()
    } else {
        vec![]
    };

    Ok(Json(ClusterDetailGraph {
        cluster,
        cbus: cbu_nodes,
        cbu_edges,
        shared_entities: vec![], // Will populate in later phase
    }))
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn parse_cluster_type(s: Option<&str>) -> ClusterType {
    match s.map(|s| s.to_lowercase()).as_deref() {
        Some("client") => ClusterType::Client,
        Some("risk") => ClusterType::Risk,
        Some("product") => ClusterType::Product,
        _ => ClusterType::Jurisdiction, // Default
    }
}

async fn fetch_all_cbus(pool: &PgPool) -> Result<Vec<CbuRow>, sqlx::Error> {
    sqlx::query_as::<_, CbuRow>(
        r#"
        SELECT
            c.cbu_id,
            c.name,
            c.jurisdiction,
            c.client_type,
            c.commercial_client_entity_id,
            e.name as commercial_client_name,
            c.risk_context->>'status' as risk_status,
            (SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles cer WHERE cer.cbu_id = c.cbu_id) as entity_count,
            EXISTS(
                SELECT 1 FROM kyc.cases k
                WHERE k.cbu_id = c.cbu_id
                AND k.status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN')
            ) as pending_kyc
        FROM "ob-poc".cbus c
        LEFT JOIN "ob-poc".entities e ON c.commercial_client_entity_id = e.entity_id
        ORDER BY c.name
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_cluster_cbus(
    pool: &PgPool,
    cluster_type: ClusterType,
    cluster_id: &str,
) -> Result<Vec<CbuRow>, sqlx::Error> {
    // Build WHERE clause based on cluster type
    let (where_clause, bind_value): (&str, &str) = match cluster_type {
        ClusterType::Jurisdiction => ("c.jurisdiction = $1", cluster_id),
        ClusterType::Client => {
            // cluster_id is UUID string for client clustering
            ("c.commercial_client_entity_id::text = $1", cluster_id)
        }
        ClusterType::Risk => ("c.risk_context->>'status' = $1", cluster_id),
        ClusterType::Product => ("c.client_type = $1", cluster_id),
    };

    let query = format!(
        r#"
        SELECT
            c.cbu_id,
            c.name,
            c.jurisdiction,
            c.client_type,
            c.commercial_client_entity_id,
            e.name as commercial_client_name,
            c.risk_context->>'status' as risk_status,
            (SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles cer WHERE cer.cbu_id = c.cbu_id) as entity_count,
            EXISTS(
                SELECT 1 FROM kyc.cases k
                WHERE k.cbu_id = c.cbu_id
                AND k.status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN')
            ) as pending_kyc
        FROM "ob-poc".cbus c
        LEFT JOIN "ob-poc".entities e ON c.commercial_client_entity_id = e.entity_id
        WHERE {}
        ORDER BY c.name
        "#,
        where_clause
    );

    sqlx::query_as::<_, CbuRow>(&query)
        .bind(bind_value)
        .fetch_all(pool)
        .await
}

fn group_cbus_into_clusters(cbus: &[CbuRow], cluster_type: ClusterType) -> Vec<ClusterAggregation> {
    let mut groups: HashMap<String, Vec<CbuRow>> = HashMap::new();

    for cbu in cbus {
        let key = match cluster_type {
            ClusterType::Jurisdiction => cbu
                .jurisdiction
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
            ClusterType::Client => cbu
                .commercial_client_entity_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "Unassigned".to_string()),
            ClusterType::Risk => cbu
                .risk_status
                .clone()
                .unwrap_or_else(|| "UNRATED".to_string()),
            ClusterType::Product => cbu
                .client_type
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
        };
        groups.entry(key).or_default().push(cbu.clone());
    }

    groups
        .into_iter()
        .map(|(key, cbus)| {
            let label = match cluster_type {
                ClusterType::Client => cbus
                    .first()
                    .and_then(|c| c.commercial_client_name.clone())
                    .unwrap_or_else(|| key.clone()),
                _ => key.clone(),
            };
            ClusterAggregation {
                cluster_id: format!("{}:{}", cluster_type_to_str(cluster_type), key),
                label,
                cluster_type,
                cbus,
            }
        })
        .collect()
}

fn cluster_type_to_str(ct: ClusterType) -> &'static str {
    match ct {
        ClusterType::Jurisdiction => "jurisdiction",
        ClusterType::Client => "client",
        ClusterType::Risk => "risk",
        ClusterType::Product => "product",
    }
}

fn build_cluster_node(agg: &ClusterAggregation, idx: usize) -> ClusterNode {
    let cbu_count = agg.cbus.len() as i32;
    let entity_count: i64 = agg.cbus.iter().filter_map(|c| c.entity_count).sum();

    // Calculate risk summary
    let mut risk_summary = RiskSummary::default();
    for cbu in &agg.cbus {
        match cbu.risk_status.as_deref() {
            Some("HIGH") => risk_summary.high += 1,
            Some("MEDIUM") => risk_summary.medium += 1,
            Some("LOW") => risk_summary.low += 1,
            _ => risk_summary.unrated += 1,
        }
    }

    // Detect anomalies
    let mut anomalies = vec![];
    if risk_summary.high > 0 {
        anomalies.push(Anomaly {
            id: format!("{}-high-risk", agg.cluster_id),
            anomaly_type: "high_risk".to_string(),
            severity: AnomalySeverity::High,
            message: format!("{} high-risk CBUs", risk_summary.high),
            entity_id: None,
            suggested_action: Some("Review high-risk CBUs".to_string()),
        });
    }

    let pending_kyc_count = agg
        .cbus
        .iter()
        .filter(|c| c.pending_kyc.unwrap_or(false))
        .count();
    if pending_kyc_count > 0 {
        anomalies.push(Anomaly {
            id: format!("{}-pending-kyc", agg.cluster_id),
            anomaly_type: "pending_kyc".to_string(),
            severity: AnomalySeverity::Medium,
            message: format!("{} pending KYC", pending_kyc_count),
            entity_id: None,
            suggested_action: Some("Complete KYC reviews".to_string()),
        });
    }

    // Calculate position in a circle layout
    let angle = (idx as f32) * std::f32::consts::TAU / 12.0; // Max 12 clusters in circle
    let radius = 300.0;
    let x = angle.cos() * radius;
    let y = angle.sin() * radius;

    // Size based on CBU count (logarithmic scale)
    let node_radius = 30.0 + (cbu_count as f32).ln() * 20.0;

    ClusterNode {
        id: agg.cluster_id.clone(),
        label: agg.label.clone(),
        cluster_type: agg.cluster_type,
        cbu_count,
        entity_count: entity_count as i32,
        risk_summary,
        position: Some((x, y)),
        radius: Some(node_radius),
        anomalies,
        is_expanded: false,
    }
}

fn build_cbu_node(cbu: &CbuRow, idx: usize, total: usize) -> CbuNode {
    // Calculate position in a grid or circle
    let angle = (idx as f32) * std::f32::consts::TAU / (total.max(1) as f32);
    let radius = 150.0;
    let x = angle.cos() * radius;
    let y = angle.sin() * radius;

    let risk_rating = match cbu.risk_status.as_deref() {
        Some("HIGH") => RiskRating::High,
        Some("MEDIUM") => RiskRating::Medium,
        Some("LOW") => RiskRating::Low,
        _ => RiskRating::Unrated,
    };

    // Determine KYC status string
    let kyc_status = if cbu.pending_kyc.unwrap_or(false) {
        Some("PENDING".to_string())
    } else {
        Some("COMPLETE".to_string())
    };

    CbuNode {
        id: cbu.cbu_id.to_string(),
        name: cbu.name.clone(),
        jurisdiction: cbu.jurisdiction.clone(),
        client_type: cbu.client_type.clone(),
        risk_rating,
        entity_count: cbu.entity_count.unwrap_or(0) as i32,
        position: Some((x, y)),
        kyc_status,
        anomalies: vec![],
        parent_cluster_id: None,
    }
}

fn calculate_universe_stats(cbus: &[CbuRow], clusters: &[ClusterNode]) -> UniverseStats {
    let total_entities: i64 = cbus.iter().filter_map(|c| c.entity_count).sum();
    let high_risk_count = cbus
        .iter()
        .filter(|c| c.risk_status.as_deref() == Some("HIGH"))
        .count();
    let pending_kyc_count = cbus
        .iter()
        .filter(|c| c.pending_kyc.unwrap_or(false))
        .count();
    let anomaly_count: usize = clusters.iter().map(|c| c.anomalies.len()).sum();

    UniverseStats {
        total_cbus: cbus.len() as i32,
        total_entities: total_entities as i32,
        total_clusters: clusters.len() as i32,
        high_risk_count: high_risk_count as i32,
        pending_kyc_count: pending_kyc_count as i32,
        anomaly_count: anomaly_count as i32,
    }
}

async fn calculate_cbu_edges(
    _pool: &PgPool,
    _cbus: &[CbuRow],
) -> Result<Vec<CbuEdge>, sqlx::Error> {
    // TODO: Implement shared entity detection between CBUs
    // This would query for entities that appear in multiple CBUs' cbu_entity_roles
    Ok(vec![])
}

// ============================================================================
// GET /api/node/:id/preview - Fork Presentation (Phase 3)
// ============================================================================

/// Query parameters for preview endpoint
#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    /// Node type: cluster, cbu, entity
    #[serde(default)]
    pub node_type: Option<String>,
    /// Maximum number of preview items (default 6)
    #[serde(default)]
    pub limit: Option<usize>,
}

/// GET /api/node/:id/preview
///
/// Returns preview data for a node when user loiters at a decision point.
/// This enables "branches present themselves" - showing lightweight previews
/// of what's down each navigation branch.
///
/// Node types supported:
/// - cluster:{type}:{id} - Returns CBU previews within the cluster
/// - cbu:{uuid} - Returns entity role previews within the CBU
/// - entity:{uuid} - Returns relationship/document previews
pub async fn get_node_preview(
    State(pool): State<PgPool>,
    Path(node_id): Path<String>,
    Query(params): Query<PreviewQuery>,
) -> Result<Json<PreviewData>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(6).min(12);

    // Parse node_id to determine type
    let (node_type, id_part) = if let Some(stripped) = node_id.strip_prefix("cluster:") {
        ("cluster", stripped.to_string())
    } else if let Some(stripped) = node_id.strip_prefix("cbu:") {
        ("cbu", stripped.to_string())
    } else if let Some(stripped) = node_id.strip_prefix("entity:") {
        ("entity", stripped.to_string())
    } else {
        // Try to infer from params or treat as cluster
        (
            params.node_type.as_deref().unwrap_or("cluster"),
            node_id.clone(),
        )
    };

    let items = match node_type {
        "cluster" => fetch_cluster_preview(&pool, &id_part, limit).await,
        "cbu" => fetch_cbu_preview(&pool, &id_part, limit).await,
        "entity" => fetch_entity_preview(&pool, &id_part, limit).await,
        _ => Ok(vec![]),
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch preview: {}", e),
        )
    })?;

    Ok(Json(PreviewData {
        node_id,
        items,
        complete: true,
        error: None,
    }))
}

/// Fetch preview items for a cluster (shows CBUs within)
async fn fetch_cluster_preview(
    pool: &PgPool,
    cluster_id: &str,
    limit: usize,
) -> Result<Vec<PreviewItem>, sqlx::Error> {
    // Parse cluster_id format: "jurisdiction:LU" or "client:uuid"
    let parts: Vec<&str> = cluster_id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Ok(vec![]);
    }

    let cluster_type = parse_cluster_type(Some(parts[0]));
    let cluster_value = parts[1];

    let cbus = fetch_cluster_cbus(pool, cluster_type, cluster_value).await?;

    Ok(cbus
        .into_iter()
        .take(limit)
        .map(|cbu| {
            let risk = match cbu.risk_status.as_deref() {
                Some("HIGH") => Some(RiskRating::High),
                Some("MEDIUM") => Some(RiskRating::Medium),
                Some("LOW") => Some(RiskRating::Low),
                _ => None,
            };

            PreviewItem {
                id: cbu.cbu_id.to_string(),
                label: cbu.name.clone(),
                preview_type: PreviewType::Cbu,
                count: cbu.entity_count.map(|c| c as u32),
                risk,
                description: cbu.client_type.clone(),
                visual_hint: cbu.jurisdiction.clone(),
                action: NavigationAction::DrillIntoCbu {
                    cbu_id: cbu.cbu_id.to_string(),
                },
            }
        })
        .collect())
}

/// Fetch preview items for a CBU (shows entities/roles within)
async fn fetch_cbu_preview(
    pool: &PgPool,
    cbu_id: &str,
    limit: usize,
) -> Result<Vec<PreviewItem>, sqlx::Error> {
    let cbu_uuid = match Uuid::parse_str(cbu_id) {
        Ok(u) => u,
        Err(_) => return Ok(vec![]),
    };

    // Fetch top entities by role importance
    let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
        r#"
        SELECT DISTINCT ON (e.entity_id)
            e.entity_id,
            e.name,
            r.name as role_name,
            et.name as entity_type
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
        JOIN "ob-poc".roles r ON cer.role_id = r.role_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE cer.cbu_id = $1
        ORDER BY e.entity_id,
            CASE r.name
                WHEN 'ASSET_OWNER' THEN 1
                WHEN 'MANAGEMENT_COMPANY' THEN 2
                WHEN 'INVESTMENT_MANAGER' THEN 3
                WHEN 'BENEFICIAL_OWNER' THEN 4
                WHEN 'DIRECTOR' THEN 5
                ELSE 10
            END
        LIMIT $2
        "#,
    )
    .bind(cbu_uuid)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(entity_id, name, role_name, entity_type)| PreviewItem {
            id: entity_id.to_string(),
            label: name.clone(),
            preview_type: PreviewType::Entity,
            count: None,
            risk: None,
            description: Some(role_name),
            visual_hint: entity_type,
            action: NavigationAction::DrillIntoEntity {
                entity_id: entity_id.to_string(),
            },
        })
        .collect())
}

/// Fetch preview items for an entity (shows relationships/documents)
async fn fetch_entity_preview(
    pool: &PgPool,
    entity_id: &str,
    limit: usize,
) -> Result<Vec<PreviewItem>, sqlx::Error> {
    let entity_uuid = match Uuid::parse_str(entity_id) {
        Ok(u) => u,
        Err(_) => return Ok(vec![]),
    };

    let mut items = vec![];

    // Fetch ownership relationships
    let ownership_rows = sqlx::query_as::<_, (Uuid, String, Option<rust_decimal::Decimal>)>(
        r#"
        SELECT
            er.to_entity_id,
            e.name,
            er.percentage
        FROM "ob-poc".entity_relationships er
        JOIN "ob-poc".entities e ON er.to_entity_id = e.entity_id
        WHERE er.from_entity_id = $1
            AND er.relationship_type = 'ownership'
            AND (er.effective_to IS NULL OR er.effective_to > NOW())
        ORDER BY er.percentage DESC NULLS LAST
        LIMIT $2
        "#,
    )
    .bind(entity_uuid)
    .bind((limit / 2) as i64)
    .fetch_all(pool)
    .await?;

    for (owned_id, owned_name, percentage) in ownership_rows {
        let pct_str = percentage.map(|p| format!("{}%", p));
        items.push(PreviewItem {
            id: owned_id.to_string(),
            label: owned_name.clone(),
            preview_type: PreviewType::Entity,
            count: None,
            risk: None,
            description: pct_str,
            visual_hint: Some("ownership".to_string()),
            action: NavigationAction::Select {
                node_id: owned_id.to_string(),
                node_type: "entity".to_string(),
            },
        });
    }

    // Fetch documents if space remains
    let doc_limit = limit.saturating_sub(items.len());
    if doc_limit > 0 {
        // Find documents linked to this entity via CBU
        let doc_rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
            r#"
            SELECT
                dc.doc_id,
                COALESCE(dc.document_name, dt.display_name, 'Document') as doc_name,
                dt.type_code
            FROM "ob-poc".document_catalog dc
            LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
            WHERE dc.cbu_id IN (
                SELECT cbu_id FROM "ob-poc".cbu_entity_roles WHERE entity_id = $1
            )
            ORDER BY dc.created_at DESC
            LIMIT $2
            "#,
        )
        .bind(entity_uuid)
        .bind(doc_limit as i64)
        .fetch_all(pool)
        .await?;

        for (doc_id, doc_name, type_code) in doc_rows {
            items.push(PreviewItem {
                id: doc_id.to_string(),
                label: doc_name,
                preview_type: PreviewType::Document,
                count: None,
                risk: None,
                description: type_code,
                visual_hint: Some("document".to_string()),
                action: NavigationAction::Select {
                    node_id: doc_id.to_string(),
                    node_type: "document".to_string(),
                },
            });
        }
    }

    Ok(items)
}

// ============================================================================
// ROUTER
// ============================================================================

/// Create the universe/galaxy router
pub fn create_universe_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/universe", get(get_universe))
        .route("/api/cluster/:type/:id", get(get_cluster_detail))
        .route("/api/node/:id/preview", get(get_node_preview))
        .with_state(pool)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cluster_type() {
        assert_eq!(parse_cluster_type(None), ClusterType::Jurisdiction);
        assert_eq!(
            parse_cluster_type(Some("jurisdiction")),
            ClusterType::Jurisdiction
        );
        assert_eq!(
            parse_cluster_type(Some("JURISDICTION")),
            ClusterType::Jurisdiction
        );
        assert_eq!(parse_cluster_type(Some("client")), ClusterType::Client);
        assert_eq!(parse_cluster_type(Some("risk")), ClusterType::Risk);
        assert_eq!(parse_cluster_type(Some("product")), ClusterType::Product);
        assert_eq!(
            parse_cluster_type(Some("invalid")),
            ClusterType::Jurisdiction
        );
    }

    #[test]
    fn test_risk_summary() {
        let summary = RiskSummary {
            high: 2,
            medium: 3,
            low: 5,
            unrated: 1,
        };
        assert_eq!(summary.total(), 11);
        assert_eq!(summary.dominant_rating(), RiskRating::High);
    }
}
