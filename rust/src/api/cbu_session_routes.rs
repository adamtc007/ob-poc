//! CBU Session API Routes
//!
//! REST endpoints for CBU session management - scope navigation with undo/redo.
//!
//! ## Endpoints
//!
//! - `POST /api/cbu-session` - Create new session
//! - `GET /api/cbu-session/:id` - Get session info
//! - `GET /api/cbu-session` - List all sessions
//! - `POST /api/cbu-session/:id/load-cbu` - Load a CBU into scope
//! - `POST /api/cbu-session/:id/load-jurisdiction` - Load all CBUs in jurisdiction
//! - `POST /api/cbu-session/:id/load-galaxy` - Load all CBUs under commercial client
//! - `POST /api/cbu-session/:id/unload-cbu` - Remove CBU from scope
//! - `POST /api/cbu-session/:id/clear` - Clear all CBUs from scope
//! - `POST /api/cbu-session/:id/undo` - Undo last scope change
//! - `POST /api/cbu-session/:id/redo` - Redo scope change
//! - `DELETE /api/cbu-session/:id` - Delete session

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::session::CbuSession;

/// Shared state for CBU session endpoints
pub type CbuSessionStore = Arc<RwLock<HashMap<Uuid, CbuSession>>>;

/// Create a new CBU session store
pub fn new_cbu_session_store() -> CbuSessionStore {
    Arc::new(RwLock::new(HashMap::new()))
}

/// State for CBU session routes
#[derive(Clone)]
pub struct CbuSessionState {
    pub pool: PgPool,
    pub sessions: CbuSessionStore,
}

/// Get session from memory or load from DB if not present (read-only)
async fn get_or_load_session(session_id: Uuid, state: &CbuSessionState) -> Option<CbuSession> {
    // Check memory first
    {
        let sessions = state.sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            return Some(session.clone());
        }
    }

    // Try to load from DB
    let session = CbuSession::load_or_new(Some(session_id), &state.pool).await;

    // If loaded successfully (has the right ID), cache it
    if session.id() == session_id {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session.clone());
        Some(session)
    } else {
        // load_or_new returned a new session with different ID (not found in DB)
        None
    }
}

/// Ensure session exists in memory (load from DB if needed), then return write lock
/// Creates new session if not found anywhere
async fn ensure_session_in_store(session_id: Uuid, state: &CbuSessionState) {
    // Check if already in memory
    {
        let sessions = state.sessions.read().await;
        if sessions.contains_key(&session_id) {
            return;
        }
    }

    // Try to load from DB, or create new
    let session = CbuSession::load_or_new(Some(session_id), &state.pool).await;

    // Insert into store (even if new - will have session_id set)
    let mut sessions = state.sessions.write().await;
    sessions.entry(session_id).or_insert_with(|| {
        let mut s = session;
        s.id = session_id; // Ensure ID matches
        s
    });
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: Uuid,
    pub name: Option<String>,
    pub cbu_count: usize,
    pub cbu_ids: Vec<Uuid>,
    pub history_depth: usize,
    pub future_depth: usize,
}

#[derive(Debug, Deserialize)]
pub struct LoadCbuRequest {
    pub cbu_id: Option<Uuid>,
    pub cbu_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoadJurisdictionRequest {
    pub jurisdiction: String,
}

#[derive(Debug, Deserialize)]
pub struct LoadGalaxyRequest {
    pub apex_entity_id: Option<Uuid>,
    pub apex_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UnloadCbuRequest {
    pub cbu_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct LoadResult {
    pub loaded: bool,
    pub count: usize,
    pub scope_size: usize,
}

#[derive(Debug, Serialize)]
pub struct HistoryResult {
    pub success: bool,
    pub scope_size: usize,
    pub history_depth: usize,
    pub future_depth: usize,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
}

// =============================================================================
// ROUTE HANDLERS
// =============================================================================

/// POST /api/cbu-session - Create new session
async fn create_session(
    State(state): State<CbuSessionState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, (StatusCode, String)> {
    let mut session = CbuSession::new();
    if let Some(name) = req.name {
        session.name = Some(name);
    }

    let id = session.id();
    let response = SessionResponse {
        id,
        name: session.name.clone(),
        cbu_count: session.count(),
        cbu_ids: session.cbu_ids_vec(),
        history_depth: session.history_depth(),
        future_depth: session.future_depth(),
    };

    // Save to store
    state.sessions.write().await.insert(id, session);

    Ok(Json(response))
}

/// GET /api/cbu-session/:id - Get session info
async fn get_session(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
) -> Result<Json<SessionResponse>, (StatusCode, String)> {
    let session = get_or_load_session(session_id, &state)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Session {} not found", session_id),
            )
        })?;

    Ok(Json(SessionResponse {
        id: session.id(),
        name: session.name.clone(),
        cbu_count: session.count(),
        cbu_ids: session.cbu_ids_vec(),
        history_depth: session.history_depth(),
        future_depth: session.future_depth(),
    }))
}

/// GET /api/cbu-session - List all sessions
async fn list_sessions(
    Query(query): Query<ListQuery>,
    State(state): State<CbuSessionState>,
) -> Result<Json<Vec<crate::session::SessionSummary>>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(20) as usize;
    let summaries = CbuSession::list_all(&state.pool, limit).await;
    Ok(Json(summaries))
}

/// POST /api/cbu-session/:id/load-cbu - Load a CBU into scope
async fn load_cbu(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
    Json(req): Json<LoadCbuRequest>,
) -> Result<Json<LoadResult>, (StatusCode, String)> {
    // Resolve CBU ID
    let cbu_id = if let Some(id) = req.cbu_id {
        id
    } else if let Some(name) = req.cbu_name {
        let row: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE $1 LIMIT 1"#)
                .bind(format!("%{}%", name))
                .fetch_optional(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        row.ok_or_else(|| (StatusCode::NOT_FOUND, format!("CBU not found: {}", name)))?
            .0
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Either cbu_id or cbu_name required".to_string(),
        ));
    };

    // Ensure session exists (load from DB if needed)
    ensure_session_in_store(session_id, &state).await;

    // Update session
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).expect("session just ensured");

    let was_new = session.load_cbu(cbu_id);
    session.maybe_save(&state.pool);

    Ok(Json(LoadResult {
        loaded: was_new,
        count: if was_new { 1 } else { 0 },
        scope_size: session.count(),
    }))
}

/// POST /api/cbu-session/:id/load-jurisdiction - Load all CBUs in jurisdiction
async fn load_jurisdiction(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
    Json(req): Json<LoadJurisdictionRequest>,
) -> Result<Json<LoadResult>, (StatusCode, String)> {
    // Find all CBUs in jurisdiction
    let rows: Vec<(Uuid,)> =
        sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE jurisdiction = $1"#)
            .bind(&req.jurisdiction)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if rows.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("No CBUs found in jurisdiction: {}", req.jurisdiction),
        ));
    }

    let cbu_ids: Vec<Uuid> = rows.into_iter().map(|(id,)| id).collect();

    // Ensure session exists (load from DB if needed)
    ensure_session_in_store(session_id, &state).await;

    // Update session
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).expect("session just ensured");

    let count = session.load_many(cbu_ids);
    session.maybe_save(&state.pool);

    Ok(Json(LoadResult {
        loaded: count > 0,
        count,
        scope_size: session.count(),
    }))
}

/// POST /api/cbu-session/:id/load-galaxy - Load all CBUs under commercial client
async fn load_galaxy(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
    Json(req): Json<LoadGalaxyRequest>,
) -> Result<Json<LoadResult>, (StatusCode, String)> {
    // Resolve apex entity
    let apex_id = if let Some(id) = req.apex_entity_id {
        id
    } else if let Some(name) = req.apex_name {
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE $1 LIMIT 1"#,
        )
        .bind(format!("%{}%", name))
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        row.ok_or_else(|| (StatusCode::NOT_FOUND, format!("Entity not found: {}", name)))?
            .0
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Either apex_entity_id or apex_name required".to_string(),
        ));
    };

    // Find all CBUs under commercial client
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT cbu_id FROM "ob-poc".cbus WHERE commercial_client_entity_id = $1"#,
    )
    .bind(apex_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if rows.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            "No CBUs found under commercial client".to_string(),
        ));
    }

    let cbu_ids: Vec<Uuid> = rows.into_iter().map(|(id,)| id).collect();

    // Ensure session exists (load from DB if needed)
    ensure_session_in_store(session_id, &state).await;

    // Update session
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).expect("session just ensured");

    let count = session.load_many(cbu_ids);
    session.maybe_save(&state.pool);

    Ok(Json(LoadResult {
        loaded: count > 0,
        count,
        scope_size: session.count(),
    }))
}

/// POST /api/cbu-session/:id/unload-cbu - Remove CBU from scope
async fn unload_cbu(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
    Json(req): Json<UnloadCbuRequest>,
) -> Result<Json<LoadResult>, (StatusCode, String)> {
    // Ensure session exists (load from DB if needed)
    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).expect("session just ensured");

    let was_present = session.unload_cbu(req.cbu_id);
    session.maybe_save(&state.pool);

    Ok(Json(LoadResult {
        loaded: false,
        count: if was_present { 1 } else { 0 },
        scope_size: session.count(),
    }))
}

/// POST /api/cbu-session/:id/clear - Clear all CBUs from scope
async fn clear_session(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
) -> Result<Json<LoadResult>, (StatusCode, String)> {
    // Ensure session exists (load from DB if needed)
    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).expect("session just ensured");

    let count = session.clear();
    session.maybe_save(&state.pool);

    Ok(Json(LoadResult {
        loaded: false,
        count,
        scope_size: 0,
    }))
}

/// POST /api/cbu-session/:id/undo - Undo last scope change
async fn undo(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
) -> Result<Json<HistoryResult>, (StatusCode, String)> {
    // Ensure session exists (load from DB if needed)
    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).expect("session just ensured");

    let success = session.undo();
    if success {
        session.maybe_save(&state.pool);
    }

    Ok(Json(HistoryResult {
        success,
        scope_size: session.count(),
        history_depth: session.history_depth(),
        future_depth: session.future_depth(),
    }))
}

/// POST /api/cbu-session/:id/redo - Redo scope change
async fn redo(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
) -> Result<Json<HistoryResult>, (StatusCode, String)> {
    // Ensure session exists (load from DB if needed)
    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).expect("session just ensured");

    let success = session.redo();
    if success {
        session.maybe_save(&state.pool);
    }

    Ok(Json(HistoryResult {
        success,
        scope_size: session.count(),
        history_depth: session.history_depth(),
        future_depth: session.future_depth(),
    }))
}

/// DELETE /api/cbu-session/:id - Delete session
async fn delete_session(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Remove from memory
    state.sessions.write().await.remove(&session_id);

    // Remove from DB
    let deleted = CbuSession::delete(session_id, &state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "deleted": deleted,
        "session_id": session_id
    })))
}

// =============================================================================
// SHEET EXECUTION ENDPOINTS
// =============================================================================

/// Request to set scope DSL
#[derive(Debug, Deserialize)]
pub struct SetScopeRequest {
    /// DSL commands that define the scope
    pub scope_dsl: Vec<String>,
}

/// Request to set template DSL
#[derive(Debug, Deserialize)]
pub struct SetTemplateRequest {
    /// Template DSL (may contain @cbu placeholder)
    pub template_dsl: String,
    /// Target entity type (e.g., "cbu")
    pub target_entity_type: String,
}

/// Request to confirm intent
#[derive(Debug, Deserialize)]
pub struct ConfirmIntentRequest {
    /// Must be true to confirm
    pub confirm: bool,
}

/// Request to submit sheet for execution
#[derive(Debug, Deserialize)]
pub struct SubmitSheetRequest {
    /// Must be true to execute
    pub confirm: bool,
}

/// POST /api/cbu-session/:id/scope - Set scope DSL
async fn set_scope(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
    Json(request): Json<SetScopeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    session
        .set_scope(request.scope_dsl)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Persist to DB
    session
        .force_save(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "state": format!("{:?}", session.repl_state),
        "scope_dsl_count": session.scope_dsl.len(),
    })))
}

/// POST /api/cbu-session/:id/template - Set template DSL
async fn set_template(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
    Json(request): Json<SetTemplateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    session
        .set_template(request.template_dsl, request.target_entity_type)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Persist to DB
    session
        .force_save(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "state": format!("{:?}", session.repl_state),
        "template_set": true,
    })))
}

/// POST /api/cbu-session/:id/confirm-intent - Confirm template intent
async fn confirm_intent(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
    Json(request): Json<ConfirmIntentRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !request.confirm {
        return Err((
            StatusCode::BAD_REQUEST,
            "Must set confirm: true".to_string(),
        ));
    }

    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    session
        .confirm_intent()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Persist to DB
    session
        .force_save(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "state": format!("{:?}", session.repl_state),
        "intent_confirmed": true,
    })))
}

/// POST /api/cbu-session/:id/sheet/generate - Generate DSL sheet from template
async fn generate_sheet(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
) -> Result<Json<crate::session::dsl_sheet::DslSheet>, (StatusCode, String)> {
    use crate::session::dsl_sheet::{DslSheet, SessionDslStatement, StatementStatus};

    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    // Validate state - must be Templated with confirmed=true
    if !session.intent_confirmed {
        return Err((
            StatusCode::BAD_REQUEST,
            "Intent not confirmed. Call /confirm-intent first.".to_string(),
        ));
    }

    let template = session
        .template_dsl
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No template DSL set".to_string()))?;

    // Get entities from current scope
    let entity_ids: Vec<Uuid> = session.cbu_ids().copied().collect();
    if entity_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No entities in scope. Load CBUs first.".to_string(),
        ));
    }

    // Generate statements by expanding template for each entity
    let mut statements = Vec::with_capacity(entity_ids.len());
    for (idx, entity_id) in entity_ids.iter().enumerate() {
        // Replace @cbu placeholder with actual UUID
        let populated = template.replace("@cbu", &format!("\"{}\"", entity_id));
        statements.push(SessionDslStatement {
            index: idx,
            source: populated,
            dag_depth: 0, // Will be computed after DAG analysis
            produces: None,
            consumes: vec!["cbu".to_string()],
            resolved_args: std::collections::HashMap::new(),
            returned_pk: None,
            status: StatementStatus::Pending,
        });
    }

    let sheet = DslSheet::with_statements(session_id, statements);

    // Update session state
    session
        .set_generated(sheet.clone())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Persist to DB
    session
        .force_save(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(sheet))
}

/// POST /api/cbu-session/:id/sheet/submit - Submit sheet for execution
async fn submit_sheet(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
    Json(request): Json<SubmitSheetRequest>,
) -> Result<Json<crate::session::dsl_sheet::SheetExecutionResult>, (StatusCode, String)> {
    use crate::dsl_v2::SheetExecutor;
    use crate::session::dsl_sheet::SheetStatus;

    if !request.confirm {
        return Err((
            StatusCode::BAD_REQUEST,
            "Must set confirm: true".to_string(),
        ));
    }

    ensure_session_in_store(session_id, &state).await;

    // Get session and validate state
    let (mut sheet, phases, scope_dsl, template_dsl) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

        // Validate state - must be Ready
        if !matches!(
            session.repl_state,
            crate::session::ReplSessionState::Ready
                | crate::session::ReplSessionState::Generated
                | crate::session::ReplSessionState::Parsed
        ) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Session not ready for execution (state: {:?})",
                    session.repl_state
                ),
            ));
        }

        let sheet = session.sheet.clone().ok_or((
            StatusCode::BAD_REQUEST,
            "No sheet generated. Call /sheet/generate first.".to_string(),
        ))?;

        let phases = sheet.phases.clone();
        let scope_dsl = session.scope_dsl.clone();
        let template_dsl = session.template_dsl.clone();

        (sheet, phases, scope_dsl, template_dsl)
    };

    // Execute the sheet
    let executor = SheetExecutor::new(&state.pool, None);
    let result = executor
        .execute_phased(&mut sheet, &phases)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Persist audit trail
    let _ = executor
        .persist_audit(
            &sheet,
            &result,
            &scope_dsl,
            template_dsl.as_deref(),
            None, // TODO: Get user from auth context
        )
        .await;

    // Update session state
    {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            let success = result.overall_status == SheetStatus::Success;
            let _ = session.mark_executed(success);

            // Persist to DB
            let _ = session.force_save(&state.pool).await;
        }
    }

    Ok(Json(result))
}

/// GET /api/cbu-session/:id/state - Get REPL state machine info
async fn get_repl_state(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let session = get_or_load_session(session_id, &state)
        .await
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "repl_state": format!("{:?}", session.repl_state),
        "scope_dsl_count": session.scope_dsl.len(),
        "template_set": session.template_dsl.is_some(),
        "target_entity_type": session.target_entity_type,
        "intent_confirmed": session.intent_confirmed,
        "sheet": session.sheet.as_ref().map(|s| serde_json::json!({
            "id": s.id,
            "statement_count": s.statement_count(),
            "phase_count": s.phase_count(),
        })),
        "cbu_count": session.count(),
    })))
}

/// POST /api/cbu-session/:id/reset - Reset to scoped state
async fn reset_session(
    Path(session_id): Path<Uuid>,
    State(state): State<CbuSessionState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    ensure_session_in_store(session_id, &state).await;

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    session
        .reset_to_scoped()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Persist to DB
    session
        .force_save(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "state": format!("{:?}", session.repl_state),
        "reset": true,
    })))
}

// =============================================================================
// ROUTER
// =============================================================================

/// Create the CBU session router
pub fn create_cbu_session_router(state: CbuSessionState) -> Router {
    Router::new()
        // Basic session operations
        .route("/", post(create_session))
        .route("/", get(list_sessions))
        .route("/{id}", get(get_session))
        .route("/{id}", delete(delete_session))
        // Scope navigation
        .route("/{id}/load-cbu", post(load_cbu))
        .route("/{id}/load-jurisdiction", post(load_jurisdiction))
        .route("/{id}/load-galaxy", post(load_galaxy))
        .route("/{id}/unload-cbu", post(unload_cbu))
        .route("/{id}/clear", post(clear_session))
        .route("/{id}/undo", post(undo))
        .route("/{id}/redo", post(redo))
        // REPL state machine
        .route("/{id}/state", get(get_repl_state))
        .route("/{id}/scope", post(set_scope))
        .route("/{id}/template", post(set_template))
        .route("/{id}/confirm-intent", post(confirm_intent))
        .route("/{id}/reset", post(reset_session))
        // Sheet execution
        .route("/{id}/sheet/generate", post(generate_sheet))
        .route("/{id}/sheet/submit", post(submit_sheet))
        .with_state(state)
}

/// Create the CBU session router with a pool (convenience function)
pub fn create_cbu_session_router_with_pool(pool: PgPool) -> Router {
    let state = CbuSessionState {
        pool,
        sessions: new_cbu_session_store(),
    };
    create_cbu_session_router(state)
}
