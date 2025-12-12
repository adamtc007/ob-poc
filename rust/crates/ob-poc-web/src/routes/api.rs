//! REST API endpoints
//!
//! These endpoints serve the HTML/TypeScript panels with data.
//! They delegate to the centralized AgentService for all agent chat operations.
//!
//! IMPORTANT: All API types are defined in `ob-poc-types` crate.
//! Do NOT define inline structs here - import from the shared crate.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use ob_poc::api::agent_service::{AgentChatRequest, AgentCommand as ServiceAgentCommand};
use ob_poc::api::session::{AgentSession, SessionState};
use ob_poc::dsl_v2::{compile, parse_program, ExecutionContext};
use uuid::Uuid;

// Import shared types from ob-poc-types (single source of truth)
use ob_poc_types::{
    AgentCommand, AstResponse, CbuSummary, ChatPayload, ChatRequest, ChatResponse, ChatResponseV2,
    CreateSessionRequest, CreateSessionResponse, DisambiguationItem, DisambiguationRequest,
    DslResponse, EntityMatch, ExecuteRequest, ExecuteResponse, ExecuteResult, SessionStateResponse,
};

use crate::state::AppState;

// =============================================================================
// CONVERSION HELPERS
// =============================================================================

/// Convert service AgentCommand to types AgentCommand
fn convert_command(cmd: &ServiceAgentCommand) -> AgentCommand {
    match cmd {
        ServiceAgentCommand::ShowCbu { cbu_id } => AgentCommand::ShowCbu {
            cbu_id: cbu_id.clone(),
        },
        ServiceAgentCommand::HighlightEntity { entity_id } => AgentCommand::HighlightEntity {
            entity_id: entity_id.clone(),
        },
        ServiceAgentCommand::NavigateDsl { line } => AgentCommand::NavigateDsl { line: *line },
    }
}

/// Convert service DisambiguationItem to types DisambiguationItem
fn convert_disambig_item(item: &ob_poc::api::session::DisambiguationItem) -> DisambiguationItem {
    match item {
        ob_poc::api::session::DisambiguationItem::EntityMatch {
            param,
            search_text,
            matches,
        } => DisambiguationItem::EntityMatch {
            param: param.clone(),
            search_text: search_text.clone(),
            matches: matches
                .iter()
                .map(|m| EntityMatch {
                    entity_id: m.entity_id.to_string(),
                    name: m.name.clone(),
                    entity_type: m.entity_type.clone(),
                    jurisdiction: m.jurisdiction.clone(),
                    context: m.context.clone(),
                    score: m.score.map(|s| s as f64),
                })
                .collect(),
        },
        ob_poc::api::session::DisambiguationItem::InterpretationChoice { text, options } => {
            DisambiguationItem::InterpretationChoice {
                text: text.clone(),
                options: options
                    .iter()
                    .map(|opt| ob_poc_types::Interpretation {
                        id: opt.id.clone(),
                        label: opt.label.clone(),
                        description: opt.description.clone(),
                        effect: opt.effect.clone(),
                    })
                    .collect(),
            }
        }
    }
}

// =============================================================================
// SESSION MANAGEMENT
// =============================================================================

pub async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Json<CreateSessionResponse> {
    let session = AgentSession::new(req.domain_hint);
    let session_id = session.id;
    let created_at = session.created_at.to_rfc3339();

    // Store in memory
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session);
    }

    // Persist to database asynchronously
    let session_repo = state.session_repo.clone();
    tokio::spawn(async move {
        if let Err(e) = session_repo
            .create_session_with_id(session_id, None, None)
            .await
        {
            tracing::error!("Failed to persist session {}: {}", session_id, e);
        }
    });

    Json(CreateSessionResponse {
        session_id: session_id.to_string(),
        state: "new".to_string(),
        created_at,
    })
}

pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, StatusCode> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(SessionStateResponse {
        session_id: session_id.to_string(),
        state: format!("{:?}", session.state),
        message_count: session.messages.len(),
        can_execute: session.can_execute(),
        dsl_source: if session.assembled_dsl.is_empty() {
            None
        } else {
            Some(session.assembled_dsl.join("\n"))
        },
    }))
}

// =============================================================================
// CHAT
// =============================================================================

/// Chat endpoint - generates DSL from natural language
///
/// Delegates to centralized AgentService which handles:
/// - Intent extraction via LLM
/// - Entity resolution via EntityGateway (same as LSP autocomplete)
/// - DSL generation with validation
/// - Retry loop with linter feedback
pub async fn chat(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    use ob_poc::agentic::create_llm_client;

    // Get session (mutable borrow for agent service)
    let mut session = {
        let sessions = state.sessions.read().await;
        sessions
            .get(&session_id)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)?
    };

    // Create LLM client
    let llm_client = create_llm_client().map_err(|e| {
        tracing::error!("LLM client error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Build request for agent service
    let agent_request = AgentChatRequest {
        message: req.message.clone(),
        cbu_id: None,
        disambiguation_response: None,
    };

    // Delegate to AgentService
    let response = state
        .agent_service
        .process_chat(&mut session, &agent_request, llm_client)
        .await
        .map_err(|e| {
            tracing::error!("AgentService error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Persist session changes back
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session);
    }

    // Convert AST to shared types
    let ast: Option<Vec<ob_poc_types::AstStatement>> = response.ast.as_ref().map(|stmts| {
        stmts
            .iter()
            .filter_map(|s| {
                serde_json::to_value(s)
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())
            })
            .collect()
    });

    // Convert commands
    let commands: Option<Vec<AgentCommand>> = response
        .commands
        .as_ref()
        .map(|cmds| cmds.iter().map(convert_command).collect());

    Ok(Json(ChatResponse {
        message: response.message,
        can_execute: response.can_execute,
        dsl_source: response.dsl_source,
        ast,
        session_state: format!("{:?}", response.session_state),
        commands,
    }))
}

// =============================================================================
// CHAT V2 (with disambiguation)
// =============================================================================

/// Chat endpoint V2 - with entity disambiguation support
///
/// Delegates to centralized AgentService which handles:
/// - Intent extraction via LLM (constrained to verb registry)
/// - Entity resolution via EntityGateway (same service as LSP autocomplete)
/// - Disambiguation when multiple entities match
/// - DSL generation with semantic validation
/// - Retry loop with linter feedback
///
/// This is the preferred chat endpoint - it returns structured payloads
/// that distinguish between ready-to-execute DSL and disambiguation requests.
pub async fn chat_v2(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponseV2>, StatusCode> {
    use ob_poc::agentic::create_llm_client;

    // Get session (clone for agent service to mutate)
    let mut session = {
        let sessions = state.sessions.read().await;
        sessions
            .get(&session_id)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)?
    };

    // Create LLM client
    let llm_client = create_llm_client().map_err(|e| {
        tracing::error!("LLM client error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Build request for agent service
    let agent_request = AgentChatRequest {
        message: req.message.clone(),
        cbu_id: None,
        disambiguation_response: None,
    };

    // Delegate to AgentService
    let response = state
        .agent_service
        .process_chat(&mut session, &agent_request, llm_client)
        .await
        .map_err(|e| {
            tracing::error!("AgentService error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Persist session changes back
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session);
    }

    // Check if disambiguation is needed
    if let Some(disambig) = response.disambiguation {
        // Convert disambiguation items
        let items: Vec<DisambiguationItem> =
            disambig.items.iter().map(convert_disambig_item).collect();

        return Ok(Json(ChatResponseV2 {
            message: response.message,
            payload: ChatPayload::NeedsDisambiguation {
                disambiguation: DisambiguationRequest {
                    request_id: disambig.request_id.to_string(),
                    items,
                    prompt: disambig.prompt,
                },
            },
            session_state: format!("{:?}", response.session_state),
        }));
    }

    // Check if we have commands (e.g., ShowCbu)
    if let Some(ref cmds) = response.commands {
        if !cmds.is_empty() {
            let commands: Vec<AgentCommand> = cmds.iter().map(convert_command).collect();
            return Ok(Json(ChatResponseV2 {
                message: response.message,
                payload: ChatPayload::Message {
                    commands: Some(commands),
                },
                session_state: format!("{:?}", response.session_state),
            }));
        }
    }

    // Convert AST to shared types
    let ast: Option<Vec<ob_poc_types::AstStatement>> = response.ast.as_ref().map(|stmts| {
        stmts
            .iter()
            .filter_map(|s| {
                serde_json::to_value(s)
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())
            })
            .collect()
    });

    // Ready payload with DSL
    Ok(Json(ChatResponseV2 {
        message: response.message,
        payload: ChatPayload::Ready {
            dsl_source: response.dsl_source.unwrap_or_default(),
            ast,
            can_execute: response.can_execute,
            commands: None,
        },
        session_state: format!("{:?}", response.session_state),
    }))
}

// =============================================================================
// EXECUTE
// =============================================================================

pub async fn execute(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    // Get DSL to execute
    let (dsl, mut context) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        let dsl = req
            .dsl
            .clone()
            .unwrap_or_else(|| session.assembled_dsl.join("\n"));

        (dsl, session.context.clone())
    };

    if dsl.is_empty() {
        return Ok(Json(ExecuteResponse {
            success: false,
            results: vec![],
            errors: vec!["No DSL to execute".to_string()],
            bindings: None,
        }));
    }

    // Parse
    let program = match parse_program(&dsl) {
        Ok(p) => p,
        Err(e) => {
            return Ok(Json(ExecuteResponse {
                success: false,
                results: vec![],
                errors: vec![format!("Parse error: {}", e)],
                bindings: None,
            }));
        }
    };

    // Compile
    let plan = match compile(&program) {
        Ok(p) => p,
        Err(e) => {
            return Ok(Json(ExecuteResponse {
                success: false,
                results: vec![],
                errors: vec![format!("Compile error: {}", e)],
                bindings: None,
            }));
        }
    };

    // Execute
    let mut exec_ctx = ExecutionContext::new().with_audit_user(&format!("session-{}", session_id));

    // Pre-bind symbols
    for (name, id) in &context.named_refs {
        exec_ctx.bind(name, *id);
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut all_success = true;

    match state.dsl_executor.execute_plan(&plan, &mut exec_ctx).await {
        Ok(exec_results) => {
            for (idx, exec_result) in exec_results.iter().enumerate() {
                let entity_id = match exec_result {
                    ob_poc::dsl_v2::ExecutionResult::Uuid(id) => Some(id.to_string()),
                    _ => None,
                };

                results.push(ExecuteResult {
                    statement_index: idx,
                    success: true,
                    message: "Executed successfully".to_string(),
                    entity_id,
                });
            }

            // Persist symbols
            for (name, id) in &exec_ctx.symbols {
                context.named_refs.insert(name.clone(), *id);
            }
        }
        Err(e) => {
            all_success = false;
            errors.push(format!("Execution error: {}", e));
        }
    }

    // Update session
    {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.context = context.clone();
            session.state = SessionState::Executed;
            session.assembled_dsl.clear();
        }
    }

    // Convert bindings to String UUIDs for TypeScript
    let bindings: std::collections::HashMap<String, String> = exec_ctx
        .symbols
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect();

    Ok(Json(ExecuteResponse {
        success: all_success,
        results,
        errors,
        bindings: Some(bindings),
    }))
}

// =============================================================================
// CBU
// =============================================================================

pub async fn list_cbus(State(state): State<AppState>) -> Result<Json<Vec<CbuSummary>>, StatusCode> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
        r#"SELECT cbu_id, name, jurisdiction, client_type FROM "ob-poc".cbus ORDER BY name LIMIT 100"#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let cbus: Vec<CbuSummary> = rows
        .into_iter()
        .map(|(cbu_id, name, jurisdiction, client_type)| CbuSummary {
            cbu_id: cbu_id.to_string(),
            name,
            jurisdiction,
            client_type,
        })
        .collect();

    Ok(Json(cbus))
}

pub async fn get_cbu(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<CbuSummary>, StatusCode> {
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
        r#"SELECT cbu_id, name, jurisdiction, client_type FROM "ob-poc".cbus WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(CbuSummary {
        cbu_id: row.0.to_string(),
        name: row.1,
        jurisdiction: row.2,
        client_type: row.3,
    }))
}

// =============================================================================
// GRAPH
// =============================================================================

/// Query parameters for graph endpoint
#[derive(Debug, serde::Deserialize)]
pub struct GraphQuery {
    /// View mode: KYC_UBO (default), SERVICE_DELIVERY, or CUSTODY
    pub view_mode: Option<String>,
    /// Layout orientation: VERTICAL (default) or HORIZONTAL
    pub orientation: Option<String>,
}

pub async fn get_cbu_graph(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<GraphQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use ob_poc::database::VisualizationRepository;
    use ob_poc::graph::{CbuGraphBuilder, LayoutEngine, Orientation, ViewMode};

    let repo = VisualizationRepository::new(state.pool.clone());
    let view_mode = ViewMode::parse(params.view_mode.as_deref().unwrap_or("KYC_UBO"));
    let orientation = Orientation::parse(params.orientation.as_deref().unwrap_or("VERTICAL"));

    // Build graph with all layers
    let mut graph = CbuGraphBuilder::new(cbu_id)
        .with_custody(true)
        .with_kyc(true)
        .with_ubo(true)
        .with_services(true)
        .build(&repo)
        .await
        .map_err(|e| {
            tracing::error!("Graph build error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Apply server-side layout (computes x/y positions)
    let layout_engine = LayoutEngine::with_orientation(view_mode, orientation);
    layout_engine.layout(&mut graph);

    Ok(Json(serde_json::to_value(graph).unwrap_or_default()))
}

// =============================================================================
// DSL
// =============================================================================

pub async fn get_session_dsl(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<DslResponse>, StatusCode> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    let source = if session.assembled_dsl.is_empty() {
        session.context.to_dsl_source()
    } else {
        session.assembled_dsl.join("\n")
    };

    Ok(Json(DslResponse {
        source,
        session_id: Some(session_id.to_string()),
    }))
}

// =============================================================================
// AST
// =============================================================================

pub async fn get_session_ast(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<AstResponse>, StatusCode> {
    use ob_poc_types::AstStatement;

    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Get AST from pending or context, convert through JSON to shared types
    let statements: Vec<AstStatement> = if let Some(ref pending) = session.pending {
        pending
            .ast
            .iter()
            .filter_map(|s| {
                serde_json::to_value(s)
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())
            })
            .collect()
    } else {
        session
            .context
            .ast
            .iter()
            .filter_map(|s| {
                serde_json::to_value(s)
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())
            })
            .collect()
    };

    Ok(Json(AstResponse { statements }))
}
