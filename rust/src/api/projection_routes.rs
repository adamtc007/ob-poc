//! Projection API routes for React Inspector UI
//!
//! ## Endpoints
//!
//! - `GET  /api/projections`                           - List recent projections
//! - `GET  /api/projections/:id`                       - Get full projection
//! - `GET  /api/projections/:id/nodes/:node_id`        - Get single node with context
//! - `GET  /api/projections/:id/nodes/:node_id/children` - Paginated children
//! - `POST /api/projections/generate`                  - Generate from snapshot
//! - `POST /api/projections/validate`                  - Validate projection structure

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use inspector_projection::{
    generator::cbu::generate_from_cbu_graph, validate, InspectorProjection, Node, NodeId, NodeKind,
    RefValue, RenderPolicy, ValidationResult as ProjectionValidation,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::database::VisualizationRepository;
use crate::graph::ConfigDrivenGraphBuilder;

// ============================================================================
// State - In-memory projection cache
// ============================================================================

/// Simple in-memory cache for projections
/// In production, this could be Redis or a more sophisticated cache
pub type ProjectionCache = Arc<RwLock<HashMap<Uuid, CachedProjection>>>;

#[derive(Clone)]
pub struct CachedProjection {
    pub projection: InspectorProjection,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub source_cbu_id: Option<Uuid>,
}

#[derive(Clone)]
pub struct ProjectionState {
    pub pool: PgPool,
    pub cache: ProjectionCache,
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Summary of a projection for list endpoint
#[derive(Debug, Serialize)]
pub struct ProjectionSummary {
    pub projection_id: Uuid,
    pub snapshot_id: String,
    pub source_hash: String,
    pub created_at: String,
    pub node_count: usize,
    pub truncated: bool,
}

/// Response for list projections endpoint
#[derive(Debug, Serialize)]
pub struct ListProjectionsResponse {
    pub projections: Vec<ProjectionSummary>,
}

/// Response for get projection endpoint
#[derive(Debug, Serialize)]
pub struct GetProjectionResponse {
    pub projection: InspectorProjection,
    pub validation: ProjectionValidationResponse,
}

/// Validation result for API response
#[derive(Debug, Serialize)]
pub struct ProjectionValidationResponse {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl From<ProjectionValidation> for ProjectionValidationResponse {
    fn from(v: ProjectionValidation) -> Self {
        Self {
            valid: v.is_valid(),
            errors: v.errors.iter().map(|e| e.to_string()).collect(),
            warnings: v.warnings.iter().map(|e| e.to_string()).collect(),
        }
    }
}

/// Response for get node endpoint
#[derive(Debug, Serialize)]
pub struct GetNodeResponse {
    pub node: Node,
    pub adjacent_refs: Vec<RefValue>,
    pub breadcrumb: Vec<RefValue>,
}

/// Query params for children endpoint
#[derive(Debug, Deserialize)]
pub struct ChildrenQuery {
    /// Opaque cursor for pagination
    pub cursor: Option<String>,
    /// Max items to return (default 50, max 100)
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// Lightweight node summary for list endpoints (per spec §2.3.3)
#[derive(Debug, Clone, Serialize)]
pub struct ApiNodeSummary {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label_short: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glyph: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_full: Option<String>,
}

/// Pagination info for API responses (per spec §2.4.1)
#[derive(Debug, Clone, Serialize)]
pub struct ApiPagingInfo {
    pub total_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_cursor: Option<String>,
}

/// Response for children endpoint
#[derive(Debug, Serialize)]
pub struct ChildrenResponse {
    pub items: Vec<ApiNodeSummary>,
    pub paging: ApiPagingInfo,
}

/// Request to generate a projection
#[derive(Debug, Deserialize)]
pub struct GenerateProjectionRequest {
    /// Source snapshot ID (typically a CBU ID)
    pub snapshot_id: Uuid,
    /// Render policy for LOD, depth, filters
    #[serde(default)]
    pub policy: RenderPolicy,
}

/// Response from generate endpoint
#[derive(Debug, Serialize)]
pub struct GenerateProjectionResponse {
    pub projection_id: Uuid,
    pub projection: InspectorProjection,
}

/// Request to validate a projection
#[derive(Debug, Deserialize)]
pub struct ValidateProjectionRequest {
    pub projection: InspectorProjection,
}

/// Response from validate endpoint
#[derive(Debug, Serialize)]
pub struct ValidateProjectionResponse {
    pub validation: ProjectionValidationResponse,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/projections - List recent projections
async fn list_projections(
    State(state): State<ProjectionState>,
) -> Result<Json<ListProjectionsResponse>, (StatusCode, String)> {
    let cache = state.cache.read().await;

    let mut projections: Vec<ProjectionSummary> = cache
        .iter()
        .map(|(id, cached)| ProjectionSummary {
            projection_id: *id,
            snapshot_id: cached.projection.snapshot.source_hash.clone(),
            source_hash: cached.projection.snapshot.source_hash.clone(),
            created_at: cached.created_at.to_rfc3339(),
            node_count: cached.projection.nodes.len(),
            truncated: false, // TODO: Track truncation in SnapshotMeta
        })
        .collect();

    // Sort by created_at descending
    projections.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Limit to 100 most recent
    projections.truncate(100);

    Ok(Json(ListProjectionsResponse { projections }))
}

/// GET /api/projections/:id - Get full projection with validation
async fn get_projection(
    State(state): State<ProjectionState>,
    Path(projection_id): Path<Uuid>,
) -> Result<Json<GetProjectionResponse>, (StatusCode, String)> {
    let cache = state.cache.read().await;

    let cached = cache.get(&projection_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Projection {} not found", projection_id),
        )
    })?;

    let validation = validate(&cached.projection);

    Ok(Json(GetProjectionResponse {
        projection: cached.projection.clone(),
        validation: validation.into(),
    }))
}

/// GET /api/projections/:id/nodes/:node_id - Get single node with context
async fn get_node(
    State(state): State<ProjectionState>,
    Path((projection_id, node_id)): Path<(Uuid, String)>,
) -> Result<Json<GetNodeResponse>, (StatusCode, String)> {
    let cache = state.cache.read().await;

    let cached = cache.get(&projection_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Projection {} not found", projection_id),
        )
    })?;

    // Parse node_id
    let node_id = NodeId::new(&node_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid node ID: {}", e)))?;

    let node = cached.projection.nodes.get(&node_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Node {} not found in projection", node_id),
        )
    })?;

    // Collect adjacent refs from branches
    let adjacent_refs: Vec<RefValue> = node
        .branches
        .values()
        .filter_map(|ref_or_list| {
            use inspector_projection::RefOrList;
            match ref_or_list {
                RefOrList::Single(r) => Some(r.clone()),
                RefOrList::List(_) => None,
            }
        })
        .collect();

    // Build breadcrumb by walking up parent refs
    let breadcrumb = build_breadcrumb(&cached.projection, &node_id);

    Ok(Json(GetNodeResponse {
        node: node.clone(),
        adjacent_refs,
        breadcrumb,
    }))
}

/// Build breadcrumb path from root to node
fn build_breadcrumb(projection: &InspectorProjection, node_id: &NodeId) -> Vec<RefValue> {
    let mut breadcrumb = Vec::new();
    let mut visited = std::collections::HashSet::new();

    // Find nodes that reference this node_id (simple parent finding)
    fn find_parent(projection: &InspectorProjection, child_id: &NodeId) -> Option<NodeId> {
        for (parent_id, parent_node) in &projection.nodes {
            for ref_or_list in parent_node.branches.values() {
                use inspector_projection::RefOrList;
                match ref_or_list {
                    RefOrList::Single(r) => {
                        if &r.target == child_id {
                            return Some(parent_id.clone());
                        }
                    }
                    RefOrList::List(list) => {
                        for item in &list.items {
                            if &item.target == child_id {
                                return Some(parent_id.clone());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    let mut current = node_id.clone();
    while let Some(parent_id) = find_parent(projection, &current) {
        if visited.contains(&parent_id) {
            break; // Cycle detected
        }
        visited.insert(parent_id.clone());
        breadcrumb.push(RefValue::new(parent_id.clone()));
        current = parent_id;
    }

    breadcrumb.reverse();
    breadcrumb
}

/// GET /api/projections/:id/nodes/:node_id/children - Paginated children
async fn get_node_children(
    State(state): State<ProjectionState>,
    Path((projection_id, node_id)): Path<(Uuid, String)>,
    Query(query): Query<ChildrenQuery>,
) -> Result<Json<ChildrenResponse>, (StatusCode, String)> {
    let cache = state.cache.read().await;

    let cached = cache.get(&projection_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Projection {} not found", projection_id),
        )
    })?;

    let node_id = NodeId::new(&node_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid node ID: {}", e)))?;

    let node = cached.projection.nodes.get(&node_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Node {} not found in projection", node_id),
        )
    })?;

    // Collect all child refs
    let mut child_refs: Vec<&RefValue> = Vec::new();
    for ref_or_list in node.branches.values() {
        use inspector_projection::RefOrList;
        match ref_or_list {
            RefOrList::Single(r) => child_refs.push(r),
            RefOrList::List(list) => child_refs.extend(list.items.iter()),
        }
    }

    // Parse cursor to get offset
    let offset = query
        .cursor
        .as_ref()
        .and_then(|c| c.parse::<usize>().ok())
        .unwrap_or(0);

    let limit = query.limit.min(100);
    let total_count = child_refs.len();

    // Slice to page
    let page_refs: Vec<_> = child_refs
        .into_iter()
        .skip(offset)
        .take(limit + 1) // +1 to detect if more exist
        .collect();

    let has_more = page_refs.len() > limit;
    let page_refs: Vec<_> = page_refs.into_iter().take(limit).collect();

    // Convert to ApiNodeSummary
    let items: Vec<ApiNodeSummary> = page_refs
        .into_iter()
        .filter_map(|r| {
            cached
                .projection
                .nodes
                .get(&r.target)
                .map(|n| ApiNodeSummary {
                    id: n.id.clone(),
                    kind: n.kind,
                    label_short: n.label_short.clone(),
                    glyph: n.glyph.clone(),
                    label_full: n.label_full.clone(),
                })
        })
        .collect();

    let next_cursor = if has_more {
        Some(format!("{}", offset + limit))
    } else {
        None
    };

    Ok(Json(ChildrenResponse {
        items,
        paging: ApiPagingInfo {
            total_count,
            next_cursor,
            prev_cursor: if offset > 0 {
                Some(format!("{}", offset.saturating_sub(limit)))
            } else {
                None
            },
        },
    }))
}

/// POST /api/projections/generate - Generate projection from snapshot
async fn generate_projection(
    State(state): State<ProjectionState>,
    Json(req): Json<GenerateProjectionRequest>,
) -> Result<Json<GenerateProjectionResponse>, (StatusCode, String)> {
    let cbu_id = req.snapshot_id;
    let policy = req.policy;

    // Build graph using config-driven builder
    let builder = ConfigDrivenGraphBuilder::new(&state.pool, cbu_id, "TRADING")
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to initialize graph builder: {}", e),
            )
        })?;

    let repo = VisualizationRepository::new(state.pool.clone());
    let graph = builder.build(&repo).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to build graph: {}", e),
        )
    })?;

    // Convert to ob_poc_types::CbuGraphResponse
    let cbu_graph_response = ob_poc_types::CbuGraphResponse {
        cbu_id: graph.cbu_id.to_string(),
        label: graph.label.clone(),
        cbu_category: graph.cbu_category.clone(),
        jurisdiction: graph.jurisdiction.clone(),
        nodes: graph
            .nodes
            .iter()
            .map(|n| ob_poc_types::GraphNode {
                id: n.id.clone(),
                node_type: format!("{:?}", n.node_type),
                layer: format!("{:?}", n.layer),
                label: n.label.clone(),
                sublabel: n.sublabel.clone(),
                status: format!("{:?}", n.status),
                roles: n.roles.clone(),
                role_categories: n.role_categories.clone(),
                primary_role: n.primary_role.clone(),
                jurisdiction: n.jurisdiction.clone(),
                ownership_pct: None,
                role_priority: n.role_priority,
                data: Some(n.data.clone()),
                x: n.x.map(|v| v as f64),
                y: n.y.map(|v| v as f64),
                importance: n.importance,
                hierarchy_depth: None,
                kyc_completion: n.kyc_completion,
                verification_summary: None,
                needs_attention: false,
                entity_category: n.entity_category.clone(),
                person_state: None,
                is_container: n.is_container,
                contains_type: n.contains_type.clone(),
                child_count: n.child_count,
                browse_nickname: n.browse_nickname.clone(),
                parent_key: n.parent_key.clone(),
                container_parent_id: n.container_parent_id.clone(),
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .map(|e| ob_poc_types::GraphEdge {
                id: e.id.clone(),
                source: e.source.clone(),
                target: e.target.clone(),
                edge_type: format!("{:?}", e.edge_type),
                label: e.label.clone(),
                weight: None,
                verification_status: None,
            })
            .collect(),
    };

    // Generate projection
    let projection = generate_from_cbu_graph(&cbu_graph_response, &policy);

    // Cache the projection
    let projection_id = Uuid::new_v4();
    {
        let mut cache = state.cache.write().await;
        cache.insert(
            projection_id,
            CachedProjection {
                projection: projection.clone(),
                created_at: chrono::Utc::now(),
                source_cbu_id: Some(cbu_id),
            },
        );

        // Limit cache size (evict oldest)
        if cache.len() > 1000 {
            if let Some(oldest_id) = cache
                .iter()
                .min_by_key(|(_, c)| c.created_at)
                .map(|(id, _)| *id)
            {
                cache.remove(&oldest_id);
            }
        }
    }

    Ok(Json(GenerateProjectionResponse {
        projection_id,
        projection,
    }))
}

/// POST /api/projections/validate - Validate projection structure
async fn validate_projection(
    Json(req): Json<ValidateProjectionRequest>,
) -> Result<Json<ValidateProjectionResponse>, (StatusCode, String)> {
    let validation = validate(&req.projection);

    Ok(Json(ValidateProjectionResponse {
        validation: validation.into(),
    }))
}

// ============================================================================
// Router
// ============================================================================

/// Create projection router with shared state
pub fn create_projection_router(pool: PgPool) -> Router {
    let state = ProjectionState {
        pool,
        cache: Arc::new(RwLock::new(HashMap::new())),
    };

    Router::new()
        .route("/api/projections", get(list_projections))
        .route("/api/projections/:id", get(get_projection))
        .route("/api/projections/:id/nodes/:node_id", get(get_node))
        .route(
            "/api/projections/:id/nodes/:node_id/children",
            get(get_node_children),
        )
        .route("/api/projections/generate", post(generate_projection))
        .route("/api/projections/validate", post(validate_projection))
        .with_state(state)
}
