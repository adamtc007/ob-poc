//! Lightweight DSL API server for test harness integration.
//!
//! Endpoints:
//! - GET  /health              - Health check
//! - GET  /verbs               - List available verbs
//! - POST /validate            - Validate DSL (parse + compile)
//! - POST /execute             - Execute DSL
//! - GET  /query/cbus          - List CBUs
//! - GET  /query/cbus/:id      - Get CBU with full details
//! - GET  /query/kyc/cases/:id - Get KYC case with details
//! - DELETE /cleanup/cbu/:id   - Delete CBU and cascade

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use ob_poc::dsl_v2::{
    compile, parse_program, verb_registry::registry, DslExecutor,
    ExecutionContext, ExecutionResult as DslResult,
};

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    executor: Arc<DslExecutor>,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    verb_count: usize,
}

#[derive(Serialize)]
struct VerbResponse {
    domain: String,
    name: String,
    full_name: String,
    description: String,
    required_args: Vec<String>,
    optional_args: Vec<String>,
}

#[derive(Serialize)]
struct VerbsResponse {
    verbs: Vec<VerbResponse>,
    total: usize,
}

#[derive(Deserialize)]
struct ValidateRequest {
    dsl: String,
}

#[derive(Serialize)]
struct ValidationError {
    message: String,
}

#[derive(Serialize)]
struct ValidateResponse {
    valid: bool,
    errors: Vec<ValidationError>,
}

#[derive(Deserialize)]
struct ExecuteRequest {
    dsl: String,
}

#[derive(Serialize)]
struct ExecuteResultItem {
    statement_index: usize,
    success: bool,
    message: String,
    entity_id: Option<Uuid>,
}

#[derive(Serialize)]
struct ExecuteResponse {
    success: bool,
    results: Vec<ExecuteResultItem>,
    bindings: std::collections::HashMap<String, Uuid>,
    errors: Vec<String>,
}

// ============================================================================
// Handlers
// ============================================================================

async fn health() -> Json<HealthResponse> {
    let reg = registry();
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        verb_count: reg.len(),
    })
}

async fn list_verbs() -> Json<VerbsResponse> {
    let reg = registry();
    let mut verbs = Vec::new();

    for domain in reg.domains() {
        for verb in reg.verbs_for_domain(&domain) {
            verbs.push(VerbResponse {
                domain: verb.domain.to_string(),
                name: verb.verb.to_string(),
                full_name: format!("{}.{}", verb.domain, verb.verb),
                description: verb.description.to_string(),
                required_args: verb.required_arg_names().iter().map(|s| s.to_string()).collect(),
                optional_args: verb.optional_arg_names().iter().map(|s| s.to_string()).collect(),
            });
        }
    }

    let total = verbs.len();
    Json(VerbsResponse { verbs, total })
}

async fn validate(Json(req): Json<ValidateRequest>) -> Json<ValidateResponse> {
    // Parse
    let program = match parse_program(&req.dsl) {
        Ok(p) => p,
        Err(e) => {
            return Json(ValidateResponse {
                valid: false,
                errors: vec![ValidationError {
                    message: format!("Parse error: {}", e),
                }],
            });
        }
    };

    // Compile (includes validation)
    match compile(&program) {
        Ok(_) => Json(ValidateResponse {
            valid: true,
            errors: vec![],
        }),
        Err(e) => Json(ValidateResponse {
            valid: false,
            errors: vec![ValidationError {
                message: format!("Compile error: {}", e),
            }],
        }),
    }
}

async fn execute(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, (StatusCode, String)> {
    // Parse
    let program = parse_program(&req.dsl).map_err(|e| {
        (StatusCode::BAD_REQUEST, format!("Parse error: {}", e))
    })?;

    // Compile
    let plan = compile(&program).map_err(|e| {
        (StatusCode::BAD_REQUEST, format!("Compile error: {}", e))
    })?;

    // Execute
    let mut ctx = ExecutionContext::new().with_audit_user("dsl_api");

    match state.executor.execute_plan(&plan, &mut ctx).await {
        Ok(results) => {
            let items: Vec<ExecuteResultItem> = results
                .iter()
                .enumerate()
                .map(|(idx, r)| {
                    let entity_id = match r {
                        DslResult::Uuid(id) => Some(*id),
                        _ => None,
                    };
                    ExecuteResultItem {
                        statement_index: idx,
                        success: true,
                        message: format!("{:?}", r),
                        entity_id,
                    }
                })
                .collect();

            Ok(Json(ExecuteResponse {
                success: true,
                results: items,
                bindings: ctx.symbols.clone(),
                errors: vec![],
            }))
        }
        Err(e) => Ok(Json(ExecuteResponse {
            success: false,
            results: vec![],
            bindings: ctx.symbols.clone(),
            errors: vec![e.to_string()],
        })),
    }
}

// ============================================================================
// Query Handlers
// ============================================================================

#[derive(Serialize)]
struct CbuSummary {
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    client_type: Option<String>,
}

async fn list_cbus(
    State(state): State<AppState>,
) -> Result<Json<Vec<CbuSummary>>, (StatusCode, String)> {
    let rows = sqlx::query_as!(
        CbuSummary,
        r#"SELECT cbu_id, name, jurisdiction, client_type FROM "ob-poc".cbus ORDER BY created_at DESC"#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

async fn get_cbu(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let cbu = sqlx::query!(
        r#"SELECT cbu_id, name, jurisdiction, client_type, description, 
                  created_at, updated_at
           FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "CBU not found".to_string()))?;

    let entities = sqlx::query!(
        r#"SELECT e.entity_id, e.name, et.name as entity_type, 
                  r.name as role_name
           FROM "ob-poc".cbu_entity_roles cer
           JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
           JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
           JOIN "ob-poc".roles r ON cer.role_id = r.role_id
           WHERE cer.cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = serde_json::json!({
        "cbu_id": cbu.cbu_id,
        "name": cbu.name,
        "jurisdiction": cbu.jurisdiction,
        "client_type": cbu.client_type,
        "description": cbu.description,
        "created_at": cbu.created_at,
        "updated_at": cbu.updated_at,
        "entities": entities.iter().map(|e| serde_json::json!({
            "entity_id": e.entity_id,
            "name": e.name,
            "entity_type": e.entity_type,
            "role": e.role_name
        })).collect::<Vec<_>>()
    });

    Ok(Json(result))
}

async fn get_kyc_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let case_row = sqlx::query!(
        r#"SELECT case_id, cbu_id, status, case_type, risk_rating,
                  opened_at, closed_at
           FROM kyc.cases WHERE case_id = $1"#,
        case_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Case not found".to_string()))?;

    let workstreams = sqlx::query!(
        r#"SELECT workstream_id, entity_id, status, is_ubo, risk_rating
           FROM kyc.entity_workstreams WHERE case_id = $1"#,
        case_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let flags = sqlx::query!(
        r#"SELECT red_flag_id, flag_type, severity, status, description
           FROM kyc.red_flags WHERE case_id = $1"#,
        case_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = serde_json::json!({
        "case_id": case_row.case_id,
        "cbu_id": case_row.cbu_id,
        "status": case_row.status,
        "case_type": case_row.case_type,
        "risk_rating": case_row.risk_rating,
        "opened_at": case_row.opened_at,
        "closed_at": case_row.closed_at,
        "workstreams": workstreams.iter().map(|w| serde_json::json!({
            "workstream_id": w.workstream_id,
            "entity_id": w.entity_id,
            "status": w.status,
            "is_ubo": w.is_ubo,
            "risk_rating": w.risk_rating
        })).collect::<Vec<_>>(),
        "red_flags": flags.iter().map(|f| serde_json::json!({
            "red_flag_id": f.red_flag_id,
            "flag_type": f.flag_type,
            "severity": f.severity,
            "status": f.status,
            "description": f.description
        })).collect::<Vec<_>>()
    });

    Ok(Json(result))
}

// ============================================================================
// Cleanup Handler
// ============================================================================

async fn cleanup_cbu(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut tx = state.pool.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // KYC data
    let _ = sqlx::query(r#"DELETE FROM kyc.red_flags WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM kyc.screenings WHERE workstream_id IN (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM kyc.doc_requests WHERE workstream_id IN (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM kyc.cases WHERE cbu_id = $1"#)
        .bind(cbu_id).execute(&mut *tx).await;

    // Core data
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE cbu_id = $1"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".cbu_resource_instances WHERE cbu_id = $1"#)
        .bind(cbu_id).execute(&mut *tx).await;
    
    let result = sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
        .bind(cbu_id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "deleted": result.rows_affected() > 0,
        "cbu_id": cbu_id
    })))
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///data_designer".to_string());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    let state = AppState {
        pool: pool.clone(),
        executor: Arc::new(DslExecutor::new(pool)),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .route("/verbs", get(list_verbs))
        .route("/validate", post(validate))
        .route("/execute", post(execute))
        .route("/query/cbus", get(list_cbus))
        .route("/query/cbus/{id}", get(get_cbu))
        .route("/query/kyc/cases/{id}", get(get_kyc_case))
        .route("/cleanup/cbu/{id}", delete(cleanup_cbu))
        .layer(cors)
        .with_state(state);

    let addr = "0.0.0.0:3001";
    println!("dsl_api listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
