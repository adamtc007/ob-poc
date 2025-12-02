//! REST API routes for attribute dictionary operations
//!
//! All database access goes through VisualizationRepository or DictionaryServiceImpl.

use crate::data_dictionary::{AttributeId, DictionaryService};
use crate::database::VisualizationRepository;
use crate::services::DictionaryServiceImpl;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct UploadDocumentRequest {
    pub cbu_id: Uuid,
    pub file_name: String,
    pub content_base64: String,
    pub document_type: String,
}

#[derive(Debug, Serialize)]
pub struct UploadDocumentResponse {
    pub doc_id: Uuid,
    pub file_hash: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ValidateDslRequest {
    pub dsl: String,
}

#[derive(Debug, Serialize)]
pub struct ValidateDslResponse {
    pub valid: bool,
    pub attribute_ids: Vec<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ValidateValueRequest {
    pub attribute_id: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ValidateValueResponse {
    pub valid: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AttributeValue {
    pub attribute_id: String,
    pub attribute_name: String,
    pub value: String,
    pub confidence: f32,
    pub source_doc_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AttributeListResponse {
    pub cbu_id: String,
    pub attributes: Vec<AttributeValue>,
    pub count: usize,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// POST /api/documents/upload
/// Upload and catalog a document for attribute extraction
/// NOTE: Stubbed - document_catalog schema differs from expected
async fn upload_document(
    State(_pool): State<PgPool>,
    Json(req): Json<UploadDocumentRequest>,
) -> Result<Json<UploadDocumentResponse>, StatusCode> {
    // Decode base64 content to calculate hash
    use base64::Engine;
    let content = base64::engine::general_purpose::STANDARD
        .decode(&req.content_base64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Calculate SHA256 hash
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let file_hash = format!("{:x}", hasher.finalize());

    // TODO: Implement proper document upload when schema is aligned
    // For now, return a mock response
    let doc_id = Uuid::new_v4();

    Ok(Json(UploadDocumentResponse {
        doc_id,
        file_hash,
        message: "Document upload stubbed - schema alignment pending".to_string(),
    }))
}

/// POST /api/attributes/validate-dsl
/// Validate DSL and extract attribute references
async fn validate_dsl(
    State(pool): State<PgPool>,
    Json(req): Json<ValidateDslRequest>,
) -> Result<Json<ValidateDslResponse>, StatusCode> {
    let dict_service = DictionaryServiceImpl::new(pool);

    match dict_service.validate_dsl_attributes(&req.dsl).await {
        Ok(attribute_ids) => {
            let ids_as_strings: Vec<String> =
                attribute_ids.iter().map(|id| id.to_string()).collect();

            Ok(Json(ValidateDslResponse {
                valid: true,
                attribute_ids: ids_as_strings,
                message: format!("Found {} valid attribute references", attribute_ids.len()),
            }))
        }
        Err(err) => Ok(Json(ValidateDslResponse {
            valid: false,
            attribute_ids: vec![],
            message: err,
        })),
    }
}

/// POST /api/attributes/validate-value
/// Validate an attribute value against its definition
async fn validate_value(
    State(pool): State<PgPool>,
    Json(req): Json<ValidateValueRequest>,
) -> Result<Json<ValidateValueResponse>, StatusCode> {
    let dict_service = DictionaryServiceImpl::new(pool);

    let attr_id = AttributeId::from_str(&req.attribute_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    match dict_service
        .validate_attribute_value(&attr_id, &req.value)
        .await
    {
        Ok(()) => Ok(Json(ValidateValueResponse {
            valid: true,
            message: "Value is valid".to_string(),
        })),
        Err(err) => Ok(Json(ValidateValueResponse {
            valid: false,
            message: err,
        })),
    }
}

/// GET /api/attributes/:entity_id
/// Get all attributes for an entity
async fn get_cbu_attributes(
    State(pool): State<PgPool>,
    Path(entity_id): Path<Uuid>,
) -> Result<Json<AttributeListResponse>, StatusCode> {
    let repo = VisualizationRepository::new(pool);

    let values = repo.get_entity_attributes(entity_id).await.map_err(|e| {
        eprintln!("Failed to fetch attributes: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let attributes: Vec<AttributeValue> = values
        .into_iter()
        .map(|v| AttributeValue {
            attribute_id: v.attribute_id.to_string(),
            attribute_name: v.attribute_name,
            value: v.value_text.unwrap_or_default(),
            confidence: 1.0,
            source_doc_id: None,
        })
        .collect();

    let count = attributes.len();

    Ok(Json(AttributeListResponse {
        cbu_id: entity_id.to_string(),
        attributes,
        count,
    }))
}

/// GET /api/attributes/document/:doc_id
/// Get all attributes extracted from a document
async fn get_document_attributes(
    State(pool): State<PgPool>,
    Path(doc_id): Path<Uuid>,
) -> Result<Json<Vec<AttributeValue>>, StatusCode> {
    let repo = VisualizationRepository::new(pool);

    let values = repo.get_document_attributes(doc_id).await.map_err(|e| {
        eprintln!("Failed to fetch document attributes: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let attributes: Vec<AttributeValue> = values
        .into_iter()
        .map(|v| AttributeValue {
            attribute_id: v.attribute_id.to_string(),
            attribute_name: v.attribute_name,
            value: serde_json::to_string(&v.value).unwrap_or_default(),
            confidence: 1.0,
            source_doc_id: Some(doc_id.to_string()),
        })
        .collect();

    Ok(Json(attributes))
}

/// GET /api/attributes/health
/// Health check endpoint
async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "service": "attribute-dictionary",
        "version": "1.0.0"
    })))
}

// ============================================================================
// Router Factory
// ============================================================================

/// Create attribute router with all endpoints
pub fn create_attribute_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/documents/upload", post(upload_document))
        .route("/api/attributes/validate-dsl", post(validate_dsl))
        .route("/api/attributes/validate-value", post(validate_value))
        .route("/api/attributes/:cbu_id", get(get_cbu_attributes))
        .route(
            "/api/attributes/document/:doc_id",
            get(get_document_attributes),
        )
        .route("/api/attributes/health", get(health_check))
        .with_state(pool)
}
