//! Service Resource Pipeline API Routes
//!
//! Endpoints for:
//! - Service intent management
//! - Resource discovery
//! - Attribute rollup and population
//! - Provisioning
//! - Readiness queries

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::service_resources::{
    discovery::{run_discovery_pipeline, AttributeRollupEngine, PopulationEngine},
    provisioning::{run_provisioning_pipeline, ReadinessEngine},
    service::ServiceResourcePipelineService,
    srdef_loader::{load_srdefs_from_config, SrdefRegistry},
    types::*,
};

// =============================================================================
// STATE
// =============================================================================

/// Shared state for service resource routes
pub struct ServiceResourceState {
    pub pool: PgPool,
    pub registry: SrdefRegistry,
}

impl ServiceResourceState {
    pub fn new(pool: PgPool) -> Self {
        let registry = load_srdefs_from_config().unwrap_or_default();
        Self { pool, registry }
    }
}

// =============================================================================
// ROUTER
// =============================================================================

/// Create the service resource API router
pub fn service_resource_router(pool: PgPool) -> Router {
    let state = Arc::new(ServiceResourceState::new(pool));

    Router::new()
        // Service Taxonomy (hierarchical view for UI)
        .route("/cbu/:cbu_id/service-taxonomy", get(get_service_taxonomy))
        // Service Intents
        .route("/cbu/:cbu_id/service-intents", post(create_service_intent))
        .route("/cbu/:cbu_id/service-intents", get(list_service_intents))
        .route(
            "/cbu/:cbu_id/service-intents/:intent_id",
            get(get_service_intent),
        )
        // Discovery
        .route("/cbu/:cbu_id/resource-discover", post(run_discovery))
        // Attributes
        .route("/cbu/:cbu_id/attributes/rollup", post(run_rollup))
        .route("/cbu/:cbu_id/attributes/populate", post(run_population))
        .route(
            "/cbu/:cbu_id/attributes/requirements",
            get(get_attr_requirements),
        )
        .route("/cbu/:cbu_id/attributes/values", get(get_attr_values))
        .route("/cbu/:cbu_id/attributes/values", post(set_attr_value))
        .route("/cbu/:cbu_id/attributes/gaps", get(get_attr_gaps))
        // Provisioning
        .route("/cbu/:cbu_id/resources/provision", post(run_provisioning))
        .route(
            "/cbu/:cbu_id/provisioning-requests",
            get(list_provisioning_requests),
        )
        // Readiness
        .route("/cbu/:cbu_id/readiness", get(get_readiness))
        .route(
            "/cbu/:cbu_id/readiness/recompute",
            post(recompute_readiness),
        )
        // Full Pipeline
        .route("/cbu/:cbu_id/pipeline/full", post(run_full_pipeline))
        // SRDEF registry
        .route("/srdefs", get(list_srdefs))
        .route("/srdefs/:srdef_id", get(get_srdef))
        .with_state(state)
}

// =============================================================================
// SERVICE INTENT HANDLERS
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateServiceIntentRequest {
    pub product_id: Uuid,
    pub service_id: Uuid,
    pub options: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
pub struct ServiceIntentResponseDto {
    pub intent_id: Uuid,
    pub cbu_id: Uuid,
    pub product_id: Uuid,
    pub service_id: Uuid,
    pub options: JsonValue,
    pub status: String,
}

async fn create_service_intent(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
    Json(req): Json<CreateServiceIntentRequest>,
) -> impl IntoResponse {
    let service = ServiceResourcePipelineService::new(state.pool.clone());

    let input = NewServiceIntent {
        cbu_id,
        product_id: req.product_id,
        service_id: req.service_id,
        options: req.options,
        created_by: None,
    };

    match service.create_service_intent(&input).await {
        Ok(intent_id) => (
            StatusCode::CREATED,
            Json(json!({
                "intent_id": intent_id,
                "cbu_id": cbu_id,
                "product_id": req.product_id,
                "service_id": req.service_id,
                "status": "active"
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn list_service_intents(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    let service = ServiceResourcePipelineService::new(state.pool.clone());

    match service.get_service_intents(cbu_id).await {
        Ok(intents) => {
            let dtos: Vec<ServiceIntentResponseDto> = intents
                .into_iter()
                .map(|i| ServiceIntentResponseDto {
                    intent_id: i.intent_id,
                    cbu_id: i.cbu_id,
                    product_id: i.product_id,
                    service_id: i.service_id,
                    options: i.options,
                    status: i.status,
                })
                .collect();
            (StatusCode::OK, Json(json!({ "intents": dtos })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn get_service_intent(
    State(state): State<Arc<ServiceResourceState>>,
    Path((cbu_id, intent_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    let service = ServiceResourcePipelineService::new(state.pool.clone());

    match service.get_service_intent(intent_id).await {
        Ok(Some(intent)) if intent.cbu_id == cbu_id => (
            StatusCode::OK,
            Json(json!({
                "intent_id": intent.intent_id,
                "cbu_id": intent.cbu_id,
                "product_id": intent.product_id,
                "service_id": intent.service_id,
                "options": intent.options,
                "status": intent.status
            })),
        ),
        Ok(Some(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Intent not found for this CBU" })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Intent not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

// =============================================================================
// DISCOVERY HANDLERS
// =============================================================================

async fn run_discovery(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    match run_discovery_pipeline(&state.pool, &state.registry, cbu_id).await {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "cbu_id": result.cbu_id,
                "srdefs_discovered": result.srdefs_discovered,
                "attrs_rolled_up": result.attrs_rolled_up,
                "attrs_populated": result.attrs_populated,
                "attrs_missing": result.attrs_missing
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

// =============================================================================
// ATTRIBUTE HANDLERS
// =============================================================================

async fn run_rollup(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    let engine = AttributeRollupEngine::new(&state.pool);

    match engine.rollup_for_cbu(cbu_id).await {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "total_attributes": result.total_attributes,
                "required_count": result.required_count,
                "optional_count": result.optional_count,
                "conflict_count": result.conflict_count
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn run_population(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    let engine = PopulationEngine::new(&state.pool);

    match engine.populate_for_cbu(cbu_id).await {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "populated": result.populated,
                "already_populated": result.already_populated,
                "still_missing": result.still_missing
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn get_attr_requirements(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    let service = ServiceResourcePipelineService::new(state.pool.clone());

    match service.get_unified_attr_requirements(cbu_id).await {
        Ok(requirements) => (
            StatusCode::OK,
            Json(json!({ "requirements": requirements })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn get_attr_values(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    let service = ServiceResourcePipelineService::new(state.pool.clone());

    match service.get_cbu_attr_values(cbu_id).await {
        Ok(values) => (StatusCode::OK, Json(json!({ "values": values }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetAttrValueRequest {
    pub attr_id: Uuid,
    pub value: JsonValue,
    pub source: Option<String>,
    pub evidence_refs: Option<Vec<EvidenceRef>>,
}

async fn set_attr_value(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
    Json(req): Json<SetAttrValueRequest>,
) -> impl IntoResponse {
    let service = ServiceResourcePipelineService::new(state.pool.clone());

    let source = match req.source.as_deref() {
        Some("derived") => AttributeSource::Derived,
        Some("entity") => AttributeSource::Entity,
        Some("cbu") => AttributeSource::Cbu,
        Some("document") => AttributeSource::Document,
        Some("external") => AttributeSource::External,
        _ => AttributeSource::Manual,
    };

    let input = SetCbuAttrValue {
        cbu_id,
        attr_id: req.attr_id,
        value: req.value,
        source,
        evidence_refs: req.evidence_refs,
        explain_refs: None,
    };

    match service.set_cbu_attr_value(&input).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn get_attr_gaps(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    // Query the view directly
    let rows: Result<Vec<AttrGapRow>, _> = sqlx::query_as(
        r#"
        SELECT attr_id, attr_code, attr_name, attr_category, has_value
        FROM "ob-poc".v_cbu_attr_gaps
        WHERE cbu_id = $1
        ORDER BY attr_category, attr_name
        "#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await;

    match rows {
        Ok(gaps) => {
            let missing: Vec<_> = gaps.iter().filter(|g| !g.has_value).collect();
            let populated: Vec<_> = gaps.iter().filter(|g| g.has_value).collect();

            (
                StatusCode::OK,
                Json(json!({
                    "total_required": gaps.len(),
                    "populated": populated.len(),
                    "missing": missing.len(),
                    "missing_attrs": missing
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, sqlx::FromRow, Serialize)]
struct AttrGapRow {
    attr_id: Uuid,
    attr_code: String,
    attr_name: String,
    attr_category: String,
    has_value: bool,
}

// =============================================================================
// PROVISIONING HANDLERS
// =============================================================================

async fn run_provisioning(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    match run_provisioning_pipeline(&state.pool, &state.registry, cbu_id).await {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "cbu_id": result.cbu_id,
                "requests_created": result.requests_created,
                "already_active": result.already_active,
                "not_ready": result.not_ready,
                "services_ready": result.services_ready,
                "services_partial": result.services_partial,
                "services_blocked": result.services_blocked
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn list_provisioning_requests(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    let service = ServiceResourcePipelineService::new(state.pool.clone());

    match service.get_provisioning_requests(cbu_id).await {
        Ok(requests) => (StatusCode::OK, Json(json!({ "requests": requests }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

// =============================================================================
// READINESS HANDLERS
// =============================================================================

async fn get_readiness(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    let service = ServiceResourcePipelineService::new(state.pool.clone());

    match service.get_service_readiness(cbu_id).await {
        Ok(readiness) => {
            let ready = readiness.iter().filter(|r| r.status == "ready").count();
            let partial = readiness.iter().filter(|r| r.status == "partial").count();
            let blocked = readiness.iter().filter(|r| r.status == "blocked").count();

            (
                StatusCode::OK,
                Json(json!({
                    "cbu_id": cbu_id,
                    "summary": {
                        "total": readiness.len(),
                        "ready": ready,
                        "partial": partial,
                        "blocked": blocked
                    },
                    "services": readiness
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn recompute_readiness(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    let engine = ReadinessEngine::new(&state.pool, &state.registry);

    match engine.compute_for_cbu(cbu_id).await {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "total_services": result.total_services,
                "ready": result.ready,
                "partial": result.partial,
                "blocked": result.blocked
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

// =============================================================================
// FULL PIPELINE HANDLER
// =============================================================================

async fn run_full_pipeline(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    // Run discovery + rollup + populate
    let discovery_result = run_discovery_pipeline(&state.pool, &state.registry, cbu_id).await;

    let discovery = match discovery_result {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Discovery failed: {}", e) })),
            )
        }
    };

    // Run provisioning + readiness
    let provisioning_result = run_provisioning_pipeline(&state.pool, &state.registry, cbu_id).await;

    let provisioning = match provisioning_result {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Provisioning failed: {}", e) })),
            )
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "cbu_id": cbu_id,
            "discovery": {
                "srdefs_discovered": discovery.srdefs_discovered,
                "attrs_rolled_up": discovery.attrs_rolled_up,
                "attrs_populated": discovery.attrs_populated,
                "attrs_missing": discovery.attrs_missing
            },
            "provisioning": {
                "requests_created": provisioning.requests_created,
                "already_active": provisioning.already_active,
                "not_ready": provisioning.not_ready
            },
            "readiness": {
                "services_ready": provisioning.services_ready,
                "services_partial": provisioning.services_partial,
                "services_blocked": provisioning.services_blocked
            }
        })),
    )
}

// =============================================================================
// SERVICE TAXONOMY HANDLER
// =============================================================================

/// Response types for service taxonomy (mirrors ob-poc-graph types)
mod taxonomy_types {
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceTaxonomyResponse {
        pub root: ServiceTaxonomyNode,
        pub cbu_id: Uuid,
        pub cbu_name: String,
        pub stats: ServiceTaxonomyStats,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceTaxonomyStats {
        pub product_count: usize,
        pub service_count: usize,
        pub intent_count: usize,
        pub resource_count: usize,
        pub attribute_progress: (usize, usize),
        pub services_ready: usize,
        pub services_partial: usize,
        pub services_blocked: usize,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceTaxonomyNode {
        pub id: Vec<String>,
        pub node_type: ServiceTaxonomyNodeType,
        pub label: String,
        pub sublabel: Option<String>,
        pub status: String,
        pub children: Vec<ServiceTaxonomyNode>,
        pub leaf_count: usize,
        pub blocking_reasons: Vec<String>,
        pub attr_progress: Option<(usize, usize)>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum ServiceTaxonomyNodeType {
        Root {
            cbu_id: Uuid,
        },
        Product {
            product_id: Uuid,
        },
        Service {
            service_id: Uuid,
            product_id: Uuid,
        },
        ServiceIntent {
            intent_id: Uuid,
        },
        Resource {
            srdef_id: String,
            resource_type: String,
        },
        AttributeCategory {
            category: String,
        },
        Attribute {
            attr_id: Uuid,
            satisfied: bool,
        },
        AttributeValue {
            source: String,
        },
    }

    impl ServiceTaxonomyNode {
        pub fn new(id: Vec<String>, node_type: ServiceTaxonomyNodeType, label: String) -> Self {
            Self {
                id,
                node_type,
                label,
                sublabel: None,
                status: "pending".to_string(),
                children: Vec::new(),
                leaf_count: 0,
                blocking_reasons: Vec::new(),
                attr_progress: None,
            }
        }

        pub fn compute_leaf_counts(&mut self) -> usize {
            if self.children.is_empty() {
                self.leaf_count = 1;
            } else {
                self.leaf_count = self
                    .children
                    .iter_mut()
                    .map(|c| c.compute_leaf_counts())
                    .sum();
            }
            self.leaf_count
        }
    }
}

use taxonomy_types::*;

/// Query result types for building taxonomy
#[derive(Debug, sqlx::FromRow)]
struct ProductRow {
    product_id: Uuid,
    name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ServiceRow {
    service_id: Uuid,
    product_id: Uuid,
    name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct IntentRow {
    intent_id: Uuid,
    product_id: Uuid,
    service_id: Uuid,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct DiscoveryRow {
    srdef_id: String,
    triggered_by_intents: JsonValue,
}

#[derive(Debug, sqlx::FromRow)]
struct AttrRequirementRow {
    attr_id: Uuid,
    requirement_strength: String,
    required_by_srdefs: JsonValue,
}

#[derive(Debug, sqlx::FromRow)]
struct AttrValueRow {
    attr_id: Uuid,
    source: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ReadinessRow {
    product_id: Uuid,
    service_id: Uuid,
    status: String,
    blocking_reasons: JsonValue,
}

#[derive(Debug, sqlx::FromRow)]
struct CbuNameRow {
    name: String,
}

/// Build the hierarchical service taxonomy for a CBU
async fn get_service_taxonomy(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> impl IntoResponse {
    // Get CBU name
    let cbu_name: String = match sqlx::query_as::<_, CbuNameRow>(
        r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(row)) => row.name,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "CBU not found" })),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    };

    // Get active service intents for this CBU
    let intents: Vec<IntentRow> = match sqlx::query_as(
        r#"
        SELECT intent_id, product_id, service_id, status
        FROM "ob-poc".service_intents
        WHERE cbu_id = $1 AND status = 'active'
        ORDER BY product_id, service_id
        "#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    };

    // Collect unique product IDs from intents
    let product_ids: Vec<Uuid> = intents
        .iter()
        .map(|i| i.product_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Get product details
    let products: Vec<ProductRow> = if product_ids.is_empty() {
        Vec::new()
    } else {
        match sqlx::query_as(
            r#"
            SELECT product_id, name
            FROM "ob-poc".products
            WHERE product_id = ANY($1)
            ORDER BY name
            "#,
        )
        .bind(&product_ids)
        .fetch_all(&state.pool)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                )
            }
        }
    };

    // Collect unique service IDs from intents
    let service_ids: Vec<Uuid> = intents
        .iter()
        .map(|i| i.service_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Get service details with product mapping
    let services: Vec<ServiceRow> = if service_ids.is_empty() {
        Vec::new()
    } else {
        match sqlx::query_as(
            r#"
            SELECT s.service_id, ps.product_id, s.name
            FROM "ob-poc".services s
            JOIN "ob-poc".product_services ps ON s.service_id = ps.service_id
            WHERE s.service_id = ANY($1) AND ps.product_id = ANY($2)
            ORDER BY s.name
            "#,
        )
        .bind(&service_ids)
        .bind(&product_ids)
        .fetch_all(&state.pool)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                )
            }
        }
    };

    // Get discovered SRDEFs for this CBU
    let discoveries: Vec<DiscoveryRow> = match sqlx::query_as(
        r#"
        SELECT srdef_id, triggered_by_intents
        FROM "ob-poc".srdef_discovery_reasons
        WHERE cbu_id = $1 AND superseded_at IS NULL
        ORDER BY srdef_id
        "#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    };

    // Get attribute requirements
    let attr_requirements: Vec<AttrRequirementRow> = match sqlx::query_as(
        r#"
        SELECT attr_id, requirement_strength, required_by_srdefs
        FROM "ob-poc".cbu_unified_attr_requirements
        WHERE cbu_id = $1
        ORDER BY requirement_strength, attr_id
        "#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    };

    // Get attribute values (what's been satisfied)
    let attr_values: Vec<AttrValueRow> = match sqlx::query_as(
        r#"
        SELECT attr_id, source
        FROM "ob-poc".cbu_attr_values
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    };

    // Get service readiness
    let readiness: Vec<ReadinessRow> = match sqlx::query_as(
        r#"
        SELECT product_id, service_id, status, blocking_reasons
        FROM "ob-poc".cbu_service_readiness
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    };

    // Build maps for efficient lookup
    let satisfied_attrs: std::collections::HashSet<Uuid> =
        attr_values.iter().map(|v| v.attr_id).collect();

    let readiness_map: std::collections::HashMap<(Uuid, Uuid), &ReadinessRow> = readiness
        .iter()
        .map(|r| ((r.product_id, r.service_id), r))
        .collect();

    // Build the tree
    let mut root = ServiceTaxonomyNode::new(
        vec![],
        ServiceTaxonomyNodeType::Root { cbu_id },
        cbu_name.clone(),
    );
    root.status = "ready".to_string();

    let mut stats = ServiceTaxonomyStats::default();

    // For each product
    for product in &products {
        let product_key = product.product_id.to_string();
        let mut product_node = ServiceTaxonomyNode::new(
            vec![product_key.clone()],
            ServiceTaxonomyNodeType::Product {
                product_id: product.product_id,
            },
            product.name.clone(),
        );
        stats.product_count += 1;

        // Find services for this product
        let product_services: Vec<&ServiceRow> = services
            .iter()
            .filter(|s| s.product_id == product.product_id)
            .collect();

        for service in product_services {
            let service_key = service.service_id.to_string();
            let mut service_node = ServiceTaxonomyNode::new(
                vec![product_key.clone(), service_key.clone()],
                ServiceTaxonomyNodeType::Service {
                    service_id: service.service_id,
                    product_id: product.product_id,
                },
                service.name.clone(),
            );
            stats.service_count += 1;

            // Get readiness status for this service
            if let Some(r) = readiness_map.get(&(product.product_id, service.service_id)) {
                service_node.status = r.status.clone();
                if let Some(reasons) = r.blocking_reasons.as_array() {
                    service_node.blocking_reasons = reasons
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                }

                match r.status.as_str() {
                    "ready" => stats.services_ready += 1,
                    "partial" => stats.services_partial += 1,
                    "blocked" => stats.services_blocked += 1,
                    _ => {}
                }
            }

            // Find intents for this product/service
            let service_intents: Vec<&IntentRow> = intents
                .iter()
                .filter(|i| {
                    i.product_id == product.product_id && i.service_id == service.service_id
                })
                .collect();

            for intent in service_intents {
                let intent_key = intent.intent_id.to_string();
                let mut intent_node = ServiceTaxonomyNode::new(
                    vec![product_key.clone(), service_key.clone(), intent_key.clone()],
                    ServiceTaxonomyNodeType::ServiceIntent {
                        intent_id: intent.intent_id,
                    },
                    format!("Intent: {}", &intent.intent_id.to_string()[..8]),
                );
                intent_node.sublabel = Some(intent.status.clone());
                intent_node.status = if intent.status == "active" {
                    "ready".to_string()
                } else {
                    "pending".to_string()
                };
                stats.intent_count += 1;

                // Find SRDEFs triggered by this intent
                let intent_srdefs: Vec<&DiscoveryRow> = discoveries
                    .iter()
                    .filter(|d| {
                        if let Some(arr) = d.triggered_by_intents.as_array() {
                            arr.iter().any(|v| {
                                v.as_str()
                                    .map(|s| s == intent.intent_id.to_string())
                                    .unwrap_or(false)
                            })
                        } else {
                            false
                        }
                    })
                    .collect();

                for discovery in intent_srdefs {
                    let srdef_key = discovery.srdef_id.clone();
                    let resource_type = discovery
                        .srdef_id
                        .split(':')
                        .nth(1)
                        .unwrap_or("resource")
                        .to_string();

                    let mut resource_node = ServiceTaxonomyNode::new(
                        vec![
                            product_key.clone(),
                            service_key.clone(),
                            intent_key.clone(),
                            srdef_key.clone(),
                        ],
                        ServiceTaxonomyNodeType::Resource {
                            srdef_id: discovery.srdef_id.clone(),
                            resource_type: resource_type.clone(),
                        },
                        discovery.srdef_id.clone(),
                    );
                    resource_node.sublabel = Some(resource_type);
                    stats.resource_count += 1;

                    // Find attributes required by this SRDEF
                    let srdef_attrs: Vec<&AttrRequirementRow> = attr_requirements
                        .iter()
                        .filter(|a| {
                            if let Some(arr) = a.required_by_srdefs.as_array() {
                                arr.iter().any(|v| {
                                    v.as_str().map(|s| s == discovery.srdef_id).unwrap_or(false)
                                })
                            } else {
                                false
                            }
                        })
                        .collect();

                    let mut satisfied_count = 0;
                    let total_count = srdef_attrs.len();

                    for attr in srdef_attrs {
                        let satisfied = satisfied_attrs.contains(&attr.attr_id);
                        if satisfied {
                            satisfied_count += 1;
                        }

                        let attr_node = ServiceTaxonomyNode::new(
                            vec![
                                product_key.clone(),
                                service_key.clone(),
                                intent_key.clone(),
                                srdef_key.clone(),
                                attr.attr_id.to_string(),
                            ],
                            ServiceTaxonomyNodeType::Attribute {
                                attr_id: attr.attr_id,
                                satisfied,
                            },
                            format!(
                                "{} {}",
                                if satisfied { "✓" } else { "○" },
                                attr.attr_id.to_string()[..8].to_string()
                            ),
                        );

                        resource_node.children.push(attr_node);
                        stats.attribute_progress.1 += 1;
                        if satisfied {
                            stats.attribute_progress.0 += 1;
                        }
                    }

                    resource_node.attr_progress = Some((satisfied_count, total_count));
                    resource_node.status = if total_count == 0 {
                        "ready".to_string()
                    } else if satisfied_count == total_count {
                        "ready".to_string()
                    } else if satisfied_count > 0 {
                        "partial".to_string()
                    } else {
                        "blocked".to_string()
                    };

                    intent_node.children.push(resource_node);
                }

                service_node.children.push(intent_node);
            }

            product_node.children.push(service_node);
        }

        root.children.push(product_node);
    }

    // Compute leaf counts
    root.compute_leaf_counts();

    let response = ServiceTaxonomyResponse {
        root,
        cbu_id,
        cbu_name,
        stats,
    };

    (StatusCode::OK, Json(json!(response)))
}

// =============================================================================
// SRDEF HANDLERS
// =============================================================================

async fn list_srdefs(State(state): State<Arc<ServiceResourceState>>) -> impl IntoResponse {
    let srdefs: Vec<_> = state
        .registry
        .srdefs
        .values()
        .map(|s| {
            json!({
                "srdef_id": s.srdef_id,
                "code": s.code,
                "name": s.name,
                "resource_type": s.resource_type,
                "owner": s.owner,
                "provisioning_strategy": s.provisioning_strategy,
                "triggered_by_services": s.triggered_by_services,
                "attribute_count": s.attributes.len(),
                "depends_on": s.depends_on
            })
        })
        .collect();

    (StatusCode::OK, Json(json!({ "srdefs": srdefs })))
}

async fn get_srdef(
    State(state): State<Arc<ServiceResourceState>>,
    Path(srdef_id): Path<String>,
) -> impl IntoResponse {
    // Try direct lookup first, then with URL-decoded colons
    let srdef_id = srdef_id.replace("%3A", ":").replace("%3a", ":");

    match state.registry.get(&srdef_id) {
        Some(srdef) => (
            StatusCode::OK,
            Json(json!({
                "srdef_id": srdef.srdef_id,
                "code": srdef.code,
                "name": srdef.name,
                "resource_type": srdef.resource_type,
                "purpose": srdef.purpose,
                "owner": srdef.owner,
                "provisioning_strategy": srdef.provisioning_strategy,
                "triggered_by_services": srdef.triggered_by_services,
                "attributes": srdef.attributes.iter().map(|a| json!({
                    "attr_id": a.attr_id,
                    "requirement": a.requirement,
                    "source_policy": a.source_policy,
                    "constraints": a.constraints,
                    "description": a.description
                })).collect::<Vec<_>>(),
                "depends_on": srdef.depends_on,
                "per_market": srdef.per_market,
                "per_currency": srdef.per_currency,
                "per_counterparty": srdef.per_counterparty
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "SRDEF not found" })),
        ),
    }
}
