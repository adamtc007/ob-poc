//! Workflow Task Queue API endpoints
//!
//! Provides HTTP endpoints for:
//! - Task completion webhooks (external systems report results)
//! - Document creation and version management
//! - Requirement status queries
//!
//! ## Endpoints
//!
//! ### Task Queue
//! - `POST /api/workflow/task-complete` - External callback for task completion
//!
//! ### Documents
//! - `POST /api/documents` - Create a new document
//! - `POST /api/documents/:id/versions` - Upload a new version
//! - `GET /api/documents/:id` - Get document with latest version
//! - `GET /api/documents/:id/versions` - List all versions
//!
//! ### Requirements
//! - `GET /api/requirements` - List requirements (with filters)
//! - `GET /api/requirements/:id` - Get requirement status
//!
//! NOTE: All queries use runtime-checked sqlx::query() instead of compile-time
//! sqlx::query!() macros because the tables are created by migrations that may
//! not exist at compile time.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use ob_workflow::CargoRef;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use uuid::Uuid;

// ============================================================================
// State
// ============================================================================

/// Shared state for workflow routes
#[derive(Clone)]
pub struct WorkflowState {
    pub pool: PgPool,
}

// ============================================================================
// Task Completion Webhook
// ============================================================================

/// Request for task completion webhook
#[derive(Debug, Deserialize, Serialize)]
pub struct TaskCompleteRequest {
    pub task_id: Uuid,
    pub status: String,
    pub idempotency_key: String,
    pub items: Vec<TaskCompleteItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskCompleteItem {
    pub cargo_ref: String,
    pub status: Option<String>,
    pub error: Option<String>,
}

/// POST /api/workflow/task-complete
///
/// External systems call this to report task completion with bundle payload.
///
/// ## Rules
/// - All callbacks use bundle format (items array), even for single docs
/// - idempotency_key IS required and scoped to task_id
/// - Version must already exist (external creates via POST /api/documents/{id}/versions)
/// - cargo_ref uses version:// scheme (not document://)
async fn handle_task_complete(
    State(state): State<WorkflowState>,
    Json(req): Json<TaskCompleteRequest>,
) -> Result<StatusCode, WorkflowApiError> {
    // Validate task exists and is not already terminal
    let pending_row =
        sqlx::query(r#"SELECT status FROM "ob-poc".workflow_pending_tasks WHERE task_id = $1"#)
            .bind(req.task_id)
            .fetch_optional(&state.pool)
            .await?;

    match pending_row {
        None => {
            return Err(WorkflowApiError::NotFound(format!(
                "Task {} not found",
                req.task_id
            )))
        }
        Some(row) => {
            let status: String = row.get("status");
            if status == "completed" || status == "failed" || status == "cancelled" {
                // Task already terminal - accept idempotently but don't queue
                return Ok(StatusCode::OK);
            }
        }
    }

    // Validate all cargo_refs are version:// scheme and exist
    for item in &req.items {
        let cargo_ref = CargoRef::parse(&item.cargo_ref).map_err(|e| {
            WorkflowApiError::BadRequest(format!("Invalid cargo_ref '{}': {}", item.cargo_ref, e))
        })?;

        if let CargoRef::Version { id, .. } = &cargo_ref {
            let exists_row = sqlx::query(
                r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".document_versions WHERE version_id = $1) as exists"#,
            )
            .bind(id)
            .fetch_one(&state.pool)
            .await?;

            let exists: bool = exists_row.get("exists");
            if !exists {
                return Err(WorkflowApiError::BadRequest(format!(
                    "Version {} not found. Create version first via POST /api/documents/{{doc_id}}/versions",
                    id
                )));
            }
        } else {
            return Err(WorkflowApiError::BadRequest(
                "cargo_ref must use version:// scheme".to_string(),
            ));
        }
    }

    // Store raw payload for audit
    let payload = serde_json::to_value(&req).ok();

    // Insert into queue (listener will process bundle)
    // ON CONFLICT handles duplicate (task_id, idempotency_key)
    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".task_result_queue
            (task_id, status, cargo_type, payload, idempotency_key)
        VALUES ($1, $2, 'bundle', $3, $4)
        ON CONFLICT (task_id, idempotency_key) DO NOTHING
        "#,
    )
    .bind(req.task_id)
    .bind(&req.status)
    .bind(&payload)
    .bind(&req.idempotency_key)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        // Duplicate (task_id, idempotency_key) - already processed
        Ok(StatusCode::OK)
    } else {
        Ok(StatusCode::ACCEPTED)
    }
}

// ============================================================================
// Document API
// ============================================================================

/// Request to create a new document
#[derive(Debug, Deserialize)]
pub struct CreateDocumentRequest {
    pub document_type: String,
    pub subject_entity_id: Option<Uuid>,
    pub subject_cbu_id: Option<Uuid>,
    pub requirement_id: Option<Uuid>,
    pub source: String,
    pub source_ref: Option<String>,
    pub created_by: Option<String>,
}

/// Response after creating a document
#[derive(Debug, Serialize)]
pub struct CreateDocumentResponse {
    pub document_id: Uuid,
    pub document_type: String,
    pub created_at: DateTime<Utc>,
}

/// POST /api/documents
///
/// Create a new document (Layer B: logical identity).
/// External systems create the document first, then upload versions.
async fn create_document(
    State(state): State<WorkflowState>,
    Json(req): Json<CreateDocumentRequest>,
) -> Result<Json<CreateDocumentResponse>, WorkflowApiError> {
    let document_id = Uuid::now_v7();
    let created_at = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".documents
            (document_id, document_type, subject_entity_id, subject_cbu_id,
             requirement_id, source, source_ref, created_at, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(document_id)
    .bind(&req.document_type)
    .bind(req.subject_entity_id)
    .bind(req.subject_cbu_id)
    .bind(req.requirement_id)
    .bind(&req.source)
    .bind(&req.source_ref)
    .bind(created_at)
    .bind(&req.created_by)
    .execute(&state.pool)
    .await?;

    // Update requirement if linked
    if let Some(req_id) = req.requirement_id {
        sqlx::query(
            r#"
            UPDATE "ob-poc".document_requirements
            SET latest_document_id = $2, updated_at = now()
            WHERE requirement_id = $1
            "#,
        )
        .bind(req_id)
        .bind(document_id)
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(CreateDocumentResponse {
        document_id,
        document_type: req.document_type,
        created_at,
    }))
}

/// Request to upload a new document version
#[derive(Debug, Deserialize)]
pub struct CreateVersionRequest {
    pub content_type: String,
    pub structured_data: Option<serde_json::Value>,
    pub blob_ref: Option<String>,
    pub ocr_extracted: Option<serde_json::Value>,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub quality_score: Option<f64>,
    pub created_by: Option<String>,
}

/// Response after creating a version
#[derive(Debug, Serialize)]
pub struct CreateVersionResponse {
    pub version_id: Uuid,
    pub document_id: Uuid,
    pub version_no: i32,
    pub cargo_ref: String,
    pub created_at: DateTime<Utc>,
}

/// POST /api/documents/:id/versions
///
/// Upload a new document version (Layer C: immutable submission).
/// Returns cargo_ref URI for use in task completion webhook.
async fn create_version(
    State(state): State<WorkflowState>,
    Path(document_id): Path<Uuid>,
    Json(req): Json<CreateVersionRequest>,
) -> Result<Json<CreateVersionResponse>, WorkflowApiError> {
    // Validate document exists
    let exists_row = sqlx::query(
        r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".documents WHERE document_id = $1) as exists"#,
    )
    .bind(document_id)
    .fetch_one(&state.pool)
    .await?;

    let doc_exists: bool = exists_row.get("exists");
    if !doc_exists {
        return Err(WorkflowApiError::NotFound(format!(
            "Document {} not found",
            document_id
        )));
    }

    // Validate content constraint
    if req.structured_data.is_none() && req.blob_ref.is_none() {
        return Err(WorkflowApiError::BadRequest(
            "Either structured_data or blob_ref is required".to_string(),
        ));
    }

    let version_id = Uuid::now_v7();
    let created_at = Utc::now();

    // Get next version number
    let version_row = sqlx::query(r#"SELECT "ob-poc".get_next_document_version($1) as version_no"#)
        .bind(document_id)
        .fetch_one(&state.pool)
        .await?;

    let version_no: i32 = version_row.try_get("version_no").unwrap_or(1);

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_versions
            (version_id, document_id, version_no, content_type,
             structured_data, blob_ref, ocr_extracted,
             valid_from, valid_to, quality_score,
             created_at, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(version_id)
    .bind(document_id)
    .bind(version_no)
    .bind(&req.content_type)
    .bind(&req.structured_data)
    .bind(&req.blob_ref)
    .bind(&req.ocr_extracted)
    .bind(req.valid_from)
    .bind(req.valid_to)
    .bind(req.quality_score)
    .bind(created_at)
    .bind(&req.created_by)
    .execute(&state.pool)
    .await?;

    let cargo_ref = CargoRef::version(version_id);

    Ok(Json(CreateVersionResponse {
        version_id,
        document_id,
        version_no,
        cargo_ref: cargo_ref.to_uri(),
        created_at,
    }))
}

/// Document with status (from view)
#[derive(Debug, Serialize, FromRow)]
pub struct DocumentWithStatusRow {
    pub document_id: Uuid,
    pub document_type: String,
    pub subject_entity_id: Option<Uuid>,
    pub subject_cbu_id: Option<Uuid>,
    pub requirement_id: Option<Uuid>,
    pub source: String,
    pub source_ref: Option<String>,
    pub latest_version_id: Option<Uuid>,
    pub latest_version_no: Option<i32>,
    pub latest_status: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
}

/// GET /api/documents/:id
///
/// Get document with latest version status.
async fn get_document(
    State(state): State<WorkflowState>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<DocumentWithStatusRow>, WorkflowApiError> {
    let doc: DocumentWithStatusRow = sqlx::query_as(
        r#"
        SELECT
            document_id,
            document_type,
            subject_entity_id,
            subject_cbu_id,
            requirement_id,
            source,
            source_ref,
            latest_version_id,
            latest_version_no,
            latest_status,
            verified_at,
            valid_from,
            valid_to,
            created_at
        FROM "ob-poc".v_documents_with_status
        WHERE document_id = $1
        "#,
    )
    .bind(document_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| WorkflowApiError::NotFound(format!("Document {} not found", document_id)))?;

    Ok(Json(doc))
}

/// Document version row
#[derive(Debug, Serialize, FromRow)]
pub struct DocumentVersionRow {
    pub version_id: Uuid,
    pub document_id: Uuid,
    pub version_no: i32,
    pub content_type: String,
    pub structured_data: Option<serde_json::Value>,
    pub blob_ref: Option<String>,
    pub ocr_extracted: Option<serde_json::Value>,
    pub task_id: Option<Uuid>,
    pub verification_status: String,
    pub rejection_code: Option<String>,
    pub rejection_reason: Option<String>,
    pub verified_by: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub quality_score: Option<f64>,
    pub extraction_confidence: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

/// GET /api/documents/:id/versions
///
/// List all versions for a document.
async fn list_versions(
    State(state): State<WorkflowState>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<Vec<DocumentVersionRow>>, WorkflowApiError> {
    let versions: Vec<DocumentVersionRow> = sqlx::query_as(
        r#"
        SELECT
            version_id,
            document_id,
            version_no,
            content_type,
            structured_data,
            blob_ref,
            ocr_extracted,
            task_id,
            verification_status,
            rejection_code,
            rejection_reason,
            verified_by,
            verified_at,
            valid_from,
            valid_to,
            quality_score,
            extraction_confidence,
            created_at,
            created_by
        FROM "ob-poc".document_versions
        WHERE document_id = $1
        ORDER BY version_no DESC
        "#,
    )
    .bind(document_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(versions))
}

// ============================================================================
// Requirement API
// ============================================================================

/// Query params for listing requirements
#[derive(Debug, Deserialize)]
pub struct RequirementQuery {
    pub workflow_instance_id: Option<Uuid>,
    pub subject_entity_id: Option<Uuid>,
    pub status: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i32,
}

fn default_limit() -> i32 {
    50
}

/// Document requirement row
#[derive(Debug, Serialize, FromRow)]
pub struct DocumentRequirementRow {
    pub requirement_id: Uuid,
    pub workflow_instance_id: Option<Uuid>,
    pub subject_entity_id: Option<Uuid>,
    pub subject_cbu_id: Option<Uuid>,
    pub doc_type: String,
    pub required_state: String,
    pub status: String,
    pub attempt_count: i32,
    pub max_attempts: Option<i32>,
    pub current_task_id: Option<Uuid>,
    pub latest_document_id: Option<Uuid>,
    pub latest_version_id: Option<Uuid>,
    pub last_rejection_code: Option<String>,
    pub last_rejection_reason: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub satisfied_at: Option<DateTime<Utc>>,
}

/// GET /api/requirements
///
/// List requirements with optional filters.
async fn list_requirements(
    State(state): State<WorkflowState>,
    Query(query): Query<RequirementQuery>,
) -> Result<Json<Vec<DocumentRequirementRow>>, WorkflowApiError> {
    let requirements: Vec<DocumentRequirementRow> = sqlx::query_as(
        r#"
        SELECT
            requirement_id,
            workflow_instance_id,
            subject_entity_id,
            subject_cbu_id,
            doc_type,
            required_state,
            status,
            attempt_count,
            max_attempts,
            current_task_id,
            latest_document_id,
            latest_version_id,
            last_rejection_code,
            last_rejection_reason,
            due_date,
            created_at,
            updated_at,
            satisfied_at
        FROM "ob-poc".document_requirements
        WHERE ($1::uuid IS NULL OR workflow_instance_id = $1)
          AND ($2::uuid IS NULL OR subject_entity_id = $2)
          AND ($3::text IS NULL OR status = $3)
        ORDER BY created_at DESC
        LIMIT $4
        "#,
    )
    .bind(query.workflow_instance_id)
    .bind(query.subject_entity_id)
    .bind(&query.status)
    .bind(query.limit as i64)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(requirements))
}

/// GET /api/requirements/:id
///
/// Get a single requirement by ID.
async fn get_requirement(
    State(state): State<WorkflowState>,
    Path(requirement_id): Path<Uuid>,
) -> Result<Json<DocumentRequirementRow>, WorkflowApiError> {
    let requirement: DocumentRequirementRow = sqlx::query_as(
        r#"
        SELECT
            requirement_id,
            workflow_instance_id,
            subject_entity_id,
            subject_cbu_id,
            doc_type,
            required_state,
            status,
            attempt_count,
            max_attempts,
            current_task_id,
            latest_document_id,
            latest_version_id,
            last_rejection_code,
            last_rejection_reason,
            due_date,
            created_at,
            updated_at,
            satisfied_at
        FROM "ob-poc".document_requirements
        WHERE requirement_id = $1
        "#,
    )
    .bind(requirement_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        WorkflowApiError::NotFound(format!("Requirement {} not found", requirement_id))
    })?;

    Ok(Json(requirement))
}

// ============================================================================
// Verification API (QA actions)
// ============================================================================

/// Request to verify a document version
#[derive(Debug, Deserialize)]
pub struct VerifyVersionRequest {
    pub verified_by: String,
}

/// Request to reject a document version
#[derive(Debug, Deserialize)]
pub struct RejectVersionRequest {
    pub rejection_code: String,
    pub rejection_reason: Option<String>,
    pub verified_by: String,
}

/// POST /api/documents/:doc_id/versions/:version_id/verify
///
/// QA approves a document version.
async fn verify_version(
    State(state): State<WorkflowState>,
    Path((document_id, version_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<VerifyVersionRequest>,
) -> Result<StatusCode, WorkflowApiError> {
    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".document_versions
        SET verification_status = 'verified',
            verified_by = $3,
            verified_at = now()
        WHERE document_id = $1 AND version_id = $2
          AND verification_status IN ('pending', 'in_qa')
        "#,
    )
    .bind(document_id)
    .bind(version_id)
    .bind(&req.verified_by)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(WorkflowApiError::BadRequest(
            "Version not found or not in verifiable state".to_string(),
        ));
    }

    Ok(StatusCode::OK)
}

/// POST /api/documents/:doc_id/versions/:version_id/reject
///
/// QA rejects a document version with reason code.
async fn reject_version(
    State(state): State<WorkflowState>,
    Path((document_id, version_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<RejectVersionRequest>,
) -> Result<StatusCode, WorkflowApiError> {
    // Validate rejection code exists
    let code_row = sqlx::query(
        r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".rejection_reason_codes WHERE code = $1) as exists"#,
    )
    .bind(&req.rejection_code)
    .fetch_one(&state.pool)
    .await?;

    let code_exists: bool = code_row.get("exists");
    if !code_exists {
        return Err(WorkflowApiError::BadRequest(format!(
            "Unknown rejection code: {}",
            req.rejection_code
        )));
    }

    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".document_versions
        SET verification_status = 'rejected',
            rejection_code = $3,
            rejection_reason = $4,
            verified_by = $5,
            verified_at = now()
        WHERE document_id = $1 AND version_id = $2
          AND verification_status IN ('pending', 'in_qa')
        "#,
    )
    .bind(document_id)
    .bind(version_id)
    .bind(&req.rejection_code)
    .bind(&req.rejection_reason)
    .bind(&req.verified_by)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(WorkflowApiError::BadRequest(
            "Version not found or not in rejectable state".to_string(),
        ));
    }

    Ok(StatusCode::OK)
}

// ============================================================================
// Error Handling
// ============================================================================

#[derive(Debug)]
pub enum WorkflowApiError {
    Database(sqlx::Error),
    NotFound(String),
    BadRequest(String),
}

impl From<sqlx::Error> for WorkflowApiError {
    fn from(e: sqlx::Error) -> Self {
        WorkflowApiError::Database(e)
    }
}

impl axum::response::IntoResponse for WorkflowApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            WorkflowApiError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            WorkflowApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            WorkflowApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

// ============================================================================
// Router
// ============================================================================

/// Create the workflow router with all endpoints
pub fn create_workflow_router(pool: PgPool) -> Router {
    let state = WorkflowState { pool };

    Router::new()
        // Task completion webhook
        .route("/workflow/task-complete", post(handle_task_complete))
        // Document endpoints
        .route("/documents", post(create_document))
        .route("/documents/:id", get(get_document))
        .route("/documents/:id/versions", post(create_version))
        .route("/documents/:id/versions", get(list_versions))
        .route(
            "/documents/:doc_id/versions/:version_id/verify",
            post(verify_version),
        )
        .route(
            "/documents/:doc_id/versions/:version_id/reject",
            post(reject_version),
        )
        // Requirement endpoints
        .route("/requirements", get(list_requirements))
        .route("/requirements/:id", get(get_requirement))
        .with_state(state)
}

/// Create workflow router with shared state (for integration with main app)
pub fn workflow_routes(_state: WorkflowState) -> Router<WorkflowState> {
    Router::new()
        // Task completion webhook
        .route("/workflow/task-complete", post(handle_task_complete))
        // Document endpoints
        .route("/documents", post(create_document))
        .route("/documents/:id", get(get_document))
        .route("/documents/:id/versions", post(create_version))
        .route("/documents/:id/versions", get(list_versions))
        .route(
            "/documents/:doc_id/versions/:version_id/verify",
            post(verify_version),
        )
        .route(
            "/documents/:doc_id/versions/:version_id/reject",
            post(reject_version),
        )
        // Requirement endpoints
        .route("/requirements", get(list_requirements))
        .route("/requirements/:id", get(get_requirement))
}
