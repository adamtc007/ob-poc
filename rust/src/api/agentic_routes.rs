//! REST API routes for agentic DSL operations
//!
//! Provides HTTP endpoints for:
//! - Executing natural language prompts
//! - Creating entities, roles, and CBUs
//! - Retrieving CBU tree visualizations
//! - Complete workflow orchestration

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::services::agentic_complete::{
    CompleteAgenticService, CompleteExecutionResult, CompleteSetupResult, ExtendedDslParser,
};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AgenticRequest {
    pub prompt: String,
}

#[derive(Debug, Serialize)]
pub struct AgenticResponse {
    pub success: bool,
    pub message: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub data: serde_json::Value,
}

impl From<CompleteExecutionResult> for AgenticResponse {
    fn from(result: CompleteExecutionResult) -> Self {
        Self {
            success: result.success,
            message: result.message,
            entity_type: result.entity_type,
            entity_id: result.entity_id,
            data: result.data,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CompleteSetupRequest {
    pub entity_name: String,
    pub entity_type: String,
    pub role_name: String,
    pub cbu_nature: String,
    pub cbu_source: String,
}

#[derive(Debug, Serialize)]
pub struct CompleteSetupResponse {
    pub success: bool,
    pub entity_id: Uuid,
    pub role_id: Uuid,
    pub cbu_id: Uuid,
    pub connection_id: Uuid,
    pub message: String,
}

impl From<CompleteSetupResult> for CompleteSetupResponse {
    fn from(result: CompleteSetupResult) -> Self {
        Self {
            success: true,
            entity_id: result.entity_id,
            role_id: result.role_id,
            cbu_id: result.cbu_id,
            connection_id: result.connection_id,
            message: result.message,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TreeResponse {
    pub cbu_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub entities: Vec<TreeEntity>,
}

#[derive(Debug, Serialize)]
pub struct TreeEntity {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub role: Option<String>,
}

// ============================================================================
// Handler Functions
// ============================================================================

/// POST /api/agentic/execute
/// Execute a natural language prompt
async fn execute_prompt(
    State(service): State<Arc<CompleteAgenticService>>,
    Json(req): Json<AgenticRequest>,
) -> Result<Json<AgenticResponse>, StatusCode> {
    let statement = ExtendedDslParser::parse(&req.prompt).map_err(|_| StatusCode::BAD_REQUEST)?;

    let result = service
        .execute(statement)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result.into()))
}

/// POST /api/agentic/setup
/// Create a complete setup (entity + role + CBU + connection)
async fn create_complete_setup(
    State(service): State<Arc<CompleteAgenticService>>,
    Json(req): Json<CompleteSetupRequest>,
) -> Result<Json<CompleteSetupResponse>, StatusCode> {
    let result = service
        .create_complete_setup(
            &req.entity_name,
            &req.entity_type,
            &req.role_name,
            &req.cbu_nature,
            &req.cbu_source,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result.into()))
}

/// GET /api/agentic/tree/:cbu_id
/// Get CBU tree visualization data
async fn get_cbu_tree(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<TreeResponse>, StatusCode> {
    // Query CBU details
    let cbu = sqlx::query!(
        r#"
        SELECT cbu_id, name, description
        FROM "ob-poc".cbus
        WHERE cbu_id = $1
        "#,
        cbu_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Query connected entities
    let entities = sqlx::query!(
        r#"
        SELECT
            e.entity_id,
            e.name,
            et.name as entity_type_name,
            erc.role_id
        FROM "ob-poc".entity_role_connections erc
        JOIN "ob-poc".entities e ON e.entity_id = erc.entity_id
        JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        WHERE erc.cbu_id = $1
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tree_entities = entities
        .into_iter()
        .map(|e| TreeEntity {
            entity_id: e.entity_id,
            name: e.name,
            entity_type: e.entity_type_name,
            role: e.role_id.map(|id| format!("{}", id)),
        })
        .collect();

    Ok(Json(TreeResponse {
        cbu_id: cbu.cbu_id,
        name: cbu.name,
        description: cbu.description,
        entities: tree_entities,
    }))
}

/// GET /api/health
/// Health check endpoint
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "agentic-dsl-api",
        "version": "1.0.0"
    }))
}

// ============================================================================
// Router Configuration
// ============================================================================

/// Create the agentic API router
pub fn create_agentic_router(pool: PgPool) -> Router {
    let service = Arc::new(CompleteAgenticService::new(pool.clone()));

    Router::new()
        .route("/api/agentic/execute", post(execute_prompt))
        .route("/api/agentic/setup", post(create_complete_setup))
        .route("/api/agentic/tree/:cbu_id", get(get_cbu_tree))
        .route("/api/health", get(health_check))
        .with_state(service)
        .with_state(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agentic_request_deserialization() {
        let json = r#"{"prompt": "Create entity John Smith as person"}"#;
        let req: AgenticRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.prompt, "Create entity John Smith as person");
    }

    #[test]
    fn test_complete_setup_request_deserialization() {
        let json = r#"{
            "entity_name": "Alice Johnson",
            "entity_type": "PERSON",
            "role_name": "Director",
            "cbu_nature": "Private wealth",
            "cbu_source": "Investment"
        }"#;
        let req: CompleteSetupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.entity_name, "Alice Johnson");
        assert_eq!(req.entity_type, "PERSON");
    }
}
