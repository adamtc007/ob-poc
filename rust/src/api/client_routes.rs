//! Client Portal REST API routes
//!
//! These routes are for the client-facing portal, allowing clients to:
//! - View their onboarding status
//! - See outstanding requests with WHY context
//! - Submit documents and information
//! - Add notes and request clarification
//! - Escalate to relationship manager
//!
//! All routes are scoped to the authenticated client's accessible CBUs.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Extension, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::api::agent_service::{AgentChatResponse, AgentService};
use crate::api::session::{AgentSession, SessionStore};

// ============================================================================
// Client State
// ============================================================================

/// Authenticated client extracted from JWT/token
#[derive(Debug, Clone)]
pub struct AuthenticatedClient {
    pub client_id: Uuid,
    pub client_name: String,
    pub client_email: String,
    pub accessible_cbus: Vec<Uuid>,
}

/// Client session state (separate from internal agent sessions)
pub type ClientSessionStore = Arc<RwLock<std::collections::HashMap<Uuid, ClientSession>>>;

#[derive(Debug, Clone)]
pub struct ClientSession {
    pub client_id: Uuid,
    pub active_cbu_id: Option<Uuid>,
    pub collection_state: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

/// State for client routes
#[derive(Clone)]
pub struct ClientState {
    pub pool: PgPool,
    pub sessions: ClientSessionStore,
    pub agent_sessions: SessionStore,
}

impl ClientState {
    pub fn new(pool: PgPool, agent_sessions: SessionStore) -> Self {
        Self {
            pool,
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            agent_sessions,
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub client_id: Uuid,
    pub credential: String,
}

/// Login response with token
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub client_name: String,
    pub accessible_cbus: Vec<CbuSummary>,
    pub expires_at: DateTime<Utc>,
}

/// CBU summary for client
#[derive(Debug, Serialize)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    /// Template discriminator: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, etc.
    pub cbu_category: Option<String>,
}

/// Onboarding status response
#[derive(Debug, Serialize)]
pub struct OnboardingStatusResponse {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub overall_progress_percent: i32,
    pub current_stage: String,
    pub completed_stages: Vec<String>,
    pub outstanding_count: i32,
    pub blockers: Vec<String>,
}

/// Outstanding request with WHY context
#[derive(Debug, Serialize)]
pub struct ClientOutstandingRequest {
    pub request_id: Uuid,
    pub entity_id: Option<Uuid>,
    pub entity_name: Option<String>,
    pub request_type: String,
    pub request_subtype: Option<String>,
    pub reason_for_request: Option<String>,
    pub compliance_context: Option<String>,
    pub acceptable_alternatives: Option<Vec<String>>,
    pub status: String,
    pub due_date: Option<chrono::NaiveDate>,
    pub client_notes: Option<String>,
    pub submission_count: i64,
}

/// Request detail with full context
#[derive(Debug, Serialize)]
pub struct RequestDetailResponse {
    pub request: ClientOutstandingRequest,
    pub common_questions: Vec<CommonQuestion>,
    pub partial_progress: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CommonQuestion {
    pub question: String,
    pub answer: String,
}

/// Document submission request
#[derive(Debug, Deserialize)]
pub struct SubmitDocumentRequest {
    pub request_id: Uuid,
    pub document_type: String,
    pub file_reference: String,
    pub file_name: Option<String>,
    pub notes: Option<String>,
}

/// Info submission request
#[derive(Debug, Deserialize)]
pub struct ProvideInfoRequest {
    pub request_id: Uuid,
    pub info_type: String,
    pub data: serde_json::Value,
    pub notes: Option<String>,
}

/// Add note request
#[derive(Debug, Deserialize)]
pub struct AddNoteRequest {
    pub request_id: Uuid,
    pub note: String,
    pub expected_date: Option<chrono::NaiveDate>,
}

/// Escalation request
#[derive(Debug, Deserialize)]
pub struct EscalateRequest {
    pub reason: Option<String>,
    pub preferred_contact: Option<String>,
}

/// Generic success response
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
}

/// Chat request for client
#[derive(Debug, Deserialize)]
pub struct ClientChatRequest {
    pub message: String,
    pub cbu_id: Option<Uuid>,
}

// ============================================================================
// Router Creation
// ============================================================================

/// Create the client portal router
pub fn create_client_router(pool: PgPool, agent_sessions: SessionStore) -> Router {
    let state = ClientState::new(pool, agent_sessions);

    Router::new()
        // Public routes (no auth required)
        .route("/api/client/auth/login", post(login))
        // Protected routes (require client auth)
        .route("/api/client/chat", post(client_chat))
        .route("/api/client/status", get(get_status))
        .route("/api/client/outstanding", get(get_outstanding))
        .route(
            "/api/client/outstanding/:request_id",
            get(get_request_detail),
        )
        .route("/api/client/submit-document", post(submit_document))
        .route("/api/client/provide-info", post(provide_info))
        .route("/api/client/add-note", post(add_note))
        .route("/api/client/escalate", post(escalate))
        .with_state(state)
    // Note: In production, add auth middleware layer here
    // .layer(middleware::from_fn(verify_client_token))
}

// ============================================================================
// Route Handlers
// ============================================================================

/// Client login
async fn login(
    State(state): State<ClientState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    // Look up client
    let client = sqlx::query!(
        r#"
        SELECT c.client_id, c.name, c.email, c.accessible_cbus, cr.credential_hash
        FROM client_portal.clients c
        JOIN client_portal.credentials cr ON c.client_id = cr.client_id
        WHERE c.client_id = $1 AND c.is_active = true AND cr.is_active = true
        "#,
        request.client_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Login query error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?
    .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()))?;

    // Verify password (in production, use proper bcrypt verification)
    // For now, we'll do a simple check - REPLACE WITH PROPER BCRYPT
    // let valid = bcrypt::verify(&request.credential, &client.credential_hash)?;
    // For development, accept any non-empty credential
    if request.credential.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()));
    }

    // Get CBU details
    let cbus = sqlx::query_as!(
        CbuSummary,
        r#"
        SELECT cbu_id, name, jurisdiction, client_type, cbu_category
        FROM "ob-poc".cbus
        WHERE cbu_id = ANY($1)
        ORDER BY name
        "#,
        &client.accessible_cbus
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("CBU query error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?;

    // Generate token (in production, use proper JWT)
    // For development, use a simple base64 encoded client_id
    let token = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        client.client_id.to_string(),
    );

    // Update last login
    let _ = sqlx::query!(
        "UPDATE client_portal.clients SET last_login_at = now() WHERE client_id = $1",
        client.client_id
    )
    .execute(&state.pool)
    .await;

    Ok(Json(LoginResponse {
        token,
        client_name: client.name,
        accessible_cbus: cbus,
        expires_at: Utc::now() + chrono::Duration::hours(24),
    }))
}

/// Client chat endpoint
async fn client_chat(
    State(state): State<ClientState>,
    Extension(client): Extension<AuthenticatedClient>,
    Json(request): Json<ClientChatRequest>,
) -> Result<Json<AgentChatResponse>, (StatusCode, String)> {
    // Validate CBU access if specified
    if let Some(cbu_id) = request.cbu_id {
        if !client.accessible_cbus.contains(&cbu_id) {
            return Err((
                StatusCode::FORBIDDEN,
                "Access denied to this CBU".to_string(),
            ));
        }
    }

    // Create client-scoped agent service
    let _service = AgentService::for_client_named(
        state.pool.clone(),
        client.client_id,
        client.accessible_cbus.clone(),
        client.client_name.clone(),
    );

    // Get or create agent session for this client
    let session_id = client.client_id; // Use client_id as session_id for simplicity
    let mut sessions = state.agent_sessions.write().await;
    let _session = sessions
        .entry(session_id)
        .or_insert_with(|| AgentSession::new(Some("client".to_string())));

    // Process chat (same pipeline, but client-scoped)
    // Note: This would need an LLM client - for now return a placeholder
    // In production, inject the LLM client via state
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "Chat requires LLM client - use with full server setup".to_string(),
    ))
}

/// Get onboarding status for client's CBUs
async fn get_status(
    State(state): State<ClientState>,
    Extension(client): Extension<AuthenticatedClient>,
) -> Result<Json<Vec<OnboardingStatusResponse>>, (StatusCode, String)> {
    let statuses = sqlx::query!(
        r#"
        SELECT
            c.cbu_id,
            c.name as cbu_name,
            COALESCE(
                (SELECT COUNT(*) FILTER (WHERE r.status = 'FULFILLED') * 100 /
                 NULLIF(COUNT(*), 0)
                 FROM kyc.outstanding_requests r WHERE r.cbu_id = c.cbu_id),
                0
            )::int as progress_percent,
            COALESCE(
                (SELECT status FROM kyc.cases WHERE cbu_id = c.cbu_id
                 ORDER BY created_at DESC LIMIT 1),
                'NOT_STARTED'
            ) as current_stage,
            (SELECT COUNT(*) FROM kyc.outstanding_requests r
             WHERE r.cbu_id = c.cbu_id
               AND r.status NOT IN ('FULFILLED', 'CANCELLED', 'WAIVED')
               AND r.client_visible = true
            )::int as outstanding_count
        FROM "ob-poc".cbus c
        WHERE c.cbu_id = ANY($1)
        ORDER BY c.name
        "#,
        &client.accessible_cbus
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Status query error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?;

    let responses: Vec<OnboardingStatusResponse> = statuses
        .into_iter()
        .map(|s| OnboardingStatusResponse {
            cbu_id: s.cbu_id,
            cbu_name: s.cbu_name,
            overall_progress_percent: s.progress_percent.unwrap_or(0),
            current_stage: s.current_stage.unwrap_or_else(|| "NOT_STARTED".to_string()),
            completed_stages: vec![], // Would need more complex query
            outstanding_count: s.outstanding_count.unwrap_or(0),
            blockers: vec![], // Would need more complex query
        })
        .collect();

    Ok(Json(responses))
}

/// Get outstanding requests with WHY context
async fn get_outstanding(
    State(state): State<ClientState>,
    Extension(client): Extension<AuthenticatedClient>,
) -> Result<Json<Vec<ClientOutstandingRequest>>, (StatusCode, String)> {
    let requests = sqlx::query!(
        r#"
        SELECT
            r.request_id,
            r.entity_id,
            e.name as entity_name,
            r.request_type,
            r.request_subtype,
            r.reason_for_request,
            r.compliance_context,
            r.acceptable_alternatives,
            r.status,
            r.due_date,
            r.client_notes,
            (SELECT COUNT(*) FROM client_portal.submissions s
             WHERE s.request_id = r.request_id) as submission_count
        FROM kyc.outstanding_requests r
        LEFT JOIN "ob-poc".entities e ON r.entity_id = e.entity_id
        WHERE r.cbu_id = ANY($1)
          AND r.client_visible = true
          AND r.status NOT IN ('FULFILLED', 'CANCELLED', 'WAIVED')
        ORDER BY r.due_date NULLS LAST, r.created_at
        "#,
        &client.accessible_cbus
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Outstanding query error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?;

    let responses: Vec<ClientOutstandingRequest> = requests
        .into_iter()
        .map(|r| ClientOutstandingRequest {
            request_id: r.request_id,
            entity_id: r.entity_id,
            entity_name: Some(r.entity_name),
            request_type: r.request_type,
            request_subtype: Some(r.request_subtype),
            reason_for_request: r.reason_for_request,
            compliance_context: r.compliance_context,
            acceptable_alternatives: r.acceptable_alternatives,
            status: r.status.unwrap_or_else(|| "PENDING".to_string()),
            due_date: r.due_date,
            client_notes: r.client_notes,
            submission_count: r.submission_count.unwrap_or(0),
        })
        .collect();

    Ok(Json(responses))
}

/// Get full detail for a specific request
async fn get_request_detail(
    State(state): State<ClientState>,
    Extension(client): Extension<AuthenticatedClient>,
    Path(request_id): Path<Uuid>,
) -> Result<Json<RequestDetailResponse>, (StatusCode, String)> {
    // Verify client has access to this request
    let request = sqlx::query!(
        r#"
        SELECT
            r.request_id,
            r.cbu_id,
            r.entity_id,
            e.name as entity_name,
            r.request_type,
            r.request_subtype,
            r.reason_for_request,
            r.compliance_context,
            r.acceptable_alternatives,
            r.status,
            r.due_date,
            r.client_notes,
            (SELECT COUNT(*) FROM client_portal.submissions s
             WHERE s.request_id = r.request_id) as submission_count
        FROM kyc.outstanding_requests r
        LEFT JOIN "ob-poc".entities e ON r.entity_id = e.entity_id
        WHERE r.request_id = $1
          AND r.client_visible = true
        "#,
        request_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Request detail query error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    // Check CBU access - cbu_id may be null
    let cbu_id = request.cbu_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Request has no associated CBU".to_string(),
        )
    })?;
    if !client.accessible_cbus.contains(&cbu_id) {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    Ok(Json(RequestDetailResponse {
        request: ClientOutstandingRequest {
            request_id: request.request_id,
            entity_id: request.entity_id,
            entity_name: Some(request.entity_name),
            request_type: request.request_type,
            request_subtype: Some(request.request_subtype),
            reason_for_request: request.reason_for_request,
            compliance_context: request.compliance_context,
            acceptable_alternatives: request.acceptable_alternatives,
            status: request.status.unwrap_or_else(|| "PENDING".to_string()),
            due_date: request.due_date,
            client_notes: request.client_notes,
            submission_count: request.submission_count.unwrap_or(0),
        },
        common_questions: vec![], // Would need lookup table
        partial_progress: None,   // Would need session state
    }))
}

/// Submit a document
async fn submit_document(
    State(state): State<ClientState>,
    Extension(client): Extension<AuthenticatedClient>,
    Json(request): Json<SubmitDocumentRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, String)> {
    // Verify client has access to this request
    let outstanding = sqlx::query!(
        "SELECT cbu_id FROM kyc.outstanding_requests WHERE request_id = $1",
        request.request_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Request lookup error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    // Check access - cbu_id may be null
    let cbu_id = outstanding.cbu_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Request has no associated CBU".to_string(),
        )
    })?;
    if !client.accessible_cbus.contains(&cbu_id) {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    // Create submission
    let submission_id = sqlx::query_scalar!(
        r#"
        INSERT INTO client_portal.submissions
            (client_id, request_id, submission_type, document_type, file_reference, file_name)
        VALUES ($1, $2, 'DOCUMENT', $3, $4, $5)
        RETURNING submission_id
        "#,
        client.client_id,
        request.request_id,
        request.document_type,
        request.file_reference,
        request.file_name
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Submission insert error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create submission".to_string(),
        )
    })?;

    // Update request status to indicate submission received
    let _ = sqlx::query!(
        r#"
        UPDATE kyc.outstanding_requests
        SET status = CASE WHEN status = 'PENDING' THEN 'SUBMITTED' ELSE status END,
            updated_at = now()
        WHERE request_id = $1
        "#,
        request.request_id
    )
    .execute(&state.pool)
    .await;

    Ok(Json(SuccessResponse {
        success: true,
        message: "Document submitted successfully".to_string(),
        id: Some(submission_id),
    }))
}

/// Provide information
async fn provide_info(
    State(state): State<ClientState>,
    Extension(client): Extension<AuthenticatedClient>,
    Json(request): Json<ProvideInfoRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, String)> {
    // Verify client has access
    let outstanding = sqlx::query!(
        "SELECT cbu_id FROM kyc.outstanding_requests WHERE request_id = $1",
        request.request_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Request lookup error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    // Check access - cbu_id may be null
    let cbu_id = outstanding.cbu_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Request has no associated CBU".to_string(),
        )
    })?;
    if !client.accessible_cbus.contains(&cbu_id) {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    // Create info submission
    let submission_id = sqlx::query_scalar!(
        r#"
        INSERT INTO client_portal.submissions
            (client_id, request_id, submission_type, info_type, info_data, note_text)
        VALUES ($1, $2, 'INFORMATION', $3, $4, $5)
        RETURNING submission_id
        "#,
        client.client_id,
        request.request_id,
        request.info_type,
        request.data,
        request.notes
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Info submission insert error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create submission".to_string(),
        )
    })?;

    Ok(Json(SuccessResponse {
        success: true,
        message: "Information submitted successfully".to_string(),
        id: Some(submission_id),
    }))
}

/// Add a note to a request
async fn add_note(
    State(state): State<ClientState>,
    Extension(client): Extension<AuthenticatedClient>,
    Json(request): Json<AddNoteRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, String)> {
    // Verify access
    let outstanding = sqlx::query!(
        "SELECT cbu_id FROM kyc.outstanding_requests WHERE request_id = $1",
        request.request_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Request lookup error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    // Check access - cbu_id may be null
    let cbu_id = outstanding.cbu_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Request has no associated CBU".to_string(),
        )
    })?;
    if !client.accessible_cbus.contains(&cbu_id) {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    // Update client_notes on request
    sqlx::query!(
        r#"
        UPDATE kyc.outstanding_requests
        SET client_notes = COALESCE(client_notes || E'\n', '') || $2,
            updated_at = now()
        WHERE request_id = $1
        "#,
        request.request_id,
        format!(
            "[{}] {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M"),
            request.note
        )
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Note update error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to add note".to_string(),
        )
    })?;

    // Create commitment if expected_date provided
    if let Some(expected_date) = request.expected_date {
        // Calculate reminder date as 1 day before expected date
        let reminder_date = expected_date - chrono::Duration::days(1);
        let _ = sqlx::query!(
            r#"
            INSERT INTO client_portal.commitments
                (client_id, request_id, commitment_text, expected_date, reminder_date)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            client.client_id,
            request.request_id,
            request.note,
            expected_date,
            reminder_date
        )
        .execute(&state.pool)
        .await;
    }

    Ok(Json(SuccessResponse {
        success: true,
        message: "Note added successfully".to_string(),
        id: None,
    }))
}

/// Escalate to relationship manager
async fn escalate(
    State(state): State<ClientState>,
    Extension(client): Extension<AuthenticatedClient>,
    Json(request): Json<EscalateRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, String)> {
    // Create escalation
    let escalation_id = sqlx::query_scalar!(
        r#"
        INSERT INTO client_portal.escalations
            (client_id, reason, preferred_contact, conversation_context)
        VALUES ($1, $2, $3, $4)
        RETURNING escalation_id
        "#,
        client.client_id,
        request.reason,
        request.preferred_contact,
        serde_json::json!({
            "client_name": client.client_name,
            "client_email": client.client_email,
            "accessible_cbus": client.accessible_cbus,
            "escalated_at": chrono::Utc::now().to_rfc3339()
        })
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Escalation insert error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create escalation".to_string(),
        )
    })?;

    // In production, would notify RM here

    Ok(Json(SuccessResponse {
        success: true,
        message: "Escalation created. Your relationship manager will contact you shortly."
            .to_string(),
        id: Some(escalation_id),
    }))
}
