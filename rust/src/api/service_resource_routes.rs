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
