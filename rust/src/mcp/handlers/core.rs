//! MCP Tool Handlers
//!
//! Implements the business logic for each MCP tool.
//! All entity lookups go through EntityGateway (single source of truth).
//! Other database access goes through VisualizationRepository.

use anyhow::{anyhow, Result};
use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use uuid::Uuid;

use crate::api::session::SessionStore;
use crate::database::generation_log_repository::{
    CompileResult, GenerationAttempt, GenerationLogRepository, LintResult, ParseResult,
};
use crate::database::VisualizationRepository;
use crate::dsl_v2::{
    compile, gateway_resolver, parse_program, registry, DslExecutor, ExecutionContext,
};

use crate::mcp::protocol::ToolCallResult;

/// Tool handlers with database access, EntityGateway client, and UI session store
pub struct ToolHandlers {
    pool: PgPool,
    generation_log: GenerationLogRepository,
    repo: VisualizationRepository,
    /// EntityGateway client for all entity lookups (lazy-initialized)
    gateway_client: Arc<Mutex<Option<EntityGatewayClient<Channel>>>>,
    /// UI session store - shared with web server for template batch operations
    sessions: Option<SessionStore>,
}

impl ToolHandlers {
    /// Create handlers without session store (standalone MCP mode)
    pub fn new(pool: PgPool) -> Self {
        Self {
            generation_log: GenerationLogRepository::new(pool.clone()),
            repo: VisualizationRepository::new(pool.clone()),
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
            sessions: None,
        }
    }

    /// Create handlers with UI session store (integrated mode)
    pub fn with_sessions(pool: PgPool, sessions: SessionStore) -> Self {
        Self {
            generation_log: GenerationLogRepository::new(pool.clone()),
            repo: VisualizationRepository::new(pool.clone()),
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
            sessions: Some(sessions),
        }
    }

    /// Get the session store, or error if not configured
    fn require_sessions(&self) -> Result<&SessionStore> {
        self.sessions.as_ref().ok_or_else(|| {
            anyhow!("Session store not configured. Batch operations require integrated mode.")
        })
    }

    /// Get the database pool
    fn require_pool(&self) -> Result<&PgPool> {
        Ok(&self.pool)
    }

    /// Get or create EntityGateway client
    async fn get_gateway_client(&self) -> Result<EntityGatewayClient<Channel>> {
        let mut guard = self.gateway_client.lock().await;
        if let Some(client) = guard.as_ref() {
            return Ok(client.clone());
        }

        let addr = gateway_resolver::gateway_addr();
        let client = EntityGatewayClient::connect(addr.clone())
            .await
            .map_err(|e| anyhow!("Failed to connect to EntityGateway at {}: {}", addr, e))?;

        *guard = Some(client.clone());
        Ok(client)
    }

    /// Search via EntityGateway
    async fn gateway_search(
        &self,
        nickname: &str,
        search: Option<&str>,
        limit: i32,
    ) -> Result<Vec<(String, String, f32)>> {
        let mut client = self.get_gateway_client().await?;

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: search.map(|s| vec![s.to_string()]).unwrap_or_default(),
            search_key: None,
            mode: if search.is_some() {
                SearchMode::Fuzzy as i32
            } else {
                SearchMode::Exact as i32
            },
            limit: Some(limit),
            discriminators: std::collections::HashMap::new(),
        };

        let response = client
            .search(request)
            .await
            .map_err(|e| anyhow!("EntityGateway search failed: {}", e))?;

        Ok(response
            .into_inner()
            .matches
            .into_iter()
            .map(|m| (m.token, m.display, m.score))
            .collect())
    }

    /// Handle a tool call by name
    pub async fn handle(&self, name: &str, args: Value) -> ToolCallResult {
        match self.dispatch(name, args).await {
            Ok(v) => ToolCallResult::json(&v),
            Err(e) => ToolCallResult::error(e.to_string()),
        }
    }

    async fn dispatch(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            "dsl_validate" => self.dsl_validate(args).await,
            "dsl_execute" => self.dsl_execute(args).await,
            "dsl_execute_submission" => self.dsl_execute_submission(args).await,
            "dsl_bind" => self.dsl_bind(args).await,
            "dsl_plan" => self.dsl_plan(args).await,
            "dsl_generate" => self.dsl_generate(args).await,
            "cbu_get" => self.cbu_get(args).await,
            "cbu_list" => self.cbu_list(args).await,
            "entity_get" => self.entity_get(args).await,
            "verbs_list" => self.verbs_list(args),
            "schema_info" => self.schema_info(args).await,
            "dsl_lookup" => self.dsl_lookup(args).await,
            "dsl_complete" => self.dsl_complete(args),
            "dsl_signature" => self.dsl_signature(args),
            "session_context" => self.session_context(args),
            "entity_search" => self.entity_search(args).await,
            // Workflow orchestration tools
            "workflow_status" => self.workflow_status(args).await,
            "workflow_advance" => self.workflow_advance(args).await,
            "workflow_transition" => self.workflow_transition(args).await,
            "workflow_start" => self.workflow_start(args).await,
            "resolve_blocker" => self.resolve_blocker(args),
            // Template tools
            "template_list" => self.template_list(args),
            "template_get" => self.template_get(args),
            "template_expand" => self.template_expand(args),
            // Template batch execution tools
            "batch_start" => self.batch_start(args).await,
            "batch_add_entities" => self.batch_add_entities(args).await,
            "batch_confirm_keyset" => self.batch_confirm_keyset(args).await,
            "batch_set_scalar" => self.batch_set_scalar(args).await,
            "batch_get_state" => self.batch_get_state(args).await,
            "batch_expand_current" => self.batch_expand_current(args).await,
            "batch_record_result" => self.batch_record_result(args).await,
            "batch_skip_current" => self.batch_skip_current(args).await,
            "batch_cancel" => self.batch_cancel(args).await,
            // Research macro tools - LLM + web search for structured discovery
            "research_list" => self.research_list(args).await,
            "research_get" => self.research_get(args).await,
            "research_execute" => self.research_execute(args).await,
            "research_approve" => self.research_approve(args).await,
            "research_reject" => self.research_reject(args).await,
            "research_status" => self.research_status(args).await,
            // Taxonomy navigation tools
            "taxonomy_get" => self.taxonomy_get(args).await,
            "taxonomy_drill_in" => self.taxonomy_drill_in(args).await,
            "taxonomy_zoom_out" => self.taxonomy_zoom_out(args).await,
            "taxonomy_reset" => self.taxonomy_reset(args).await,
            "taxonomy_position" => self.taxonomy_position(args).await,
            "taxonomy_entities" => self.taxonomy_entities(args).await,
            // Trading matrix tools
            "trading_matrix_get" => self.trading_matrix_get(args).await,
            // Feedback inspector tools
            "feedback_analyze" => self.feedback_analyze(args).await,
            "feedback_list" => self.feedback_list(args).await,
            "feedback_get" => self.feedback_get(args).await,
            "feedback_repro" => self.feedback_repro(args).await,
            "feedback_todo" => self.feedback_todo(args).await,
            "feedback_audit" => self.feedback_audit(args).await,
            _ => Err(anyhow!("Unknown tool: {}", name)),
        }
    }

    /// Validate DSL source code with enhanced diagnostics
    ///
    /// Uses the planning facade to provide:
    /// - Structured diagnostics with resolution options
    /// - Suggested fixes for implicit creates
    /// - Reordering warnings
    async fn dsl_validate(&self, args: Value) -> Result<Value> {
        use crate::dsl_v2::config::ConfigLoader;
        use crate::dsl_v2::planning_facade::{analyse_and_plan, PlanningInput};
        use crate::dsl_v2::runtime_registry::RuntimeVerbRegistry;
        use crate::mcp::types::{
            AgentDiagnostic, ResolutionOption, SuggestedFix, ValidationOutput,
        };
        use std::sync::Arc;

        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;

        // Get session bindings if provided (for future use with known_symbols)
        let session_id = args["session_id"].as_str();
        let _binding_context = session_id.and_then(crate::mcp::session::get_session_bindings);

        // Load verb registry for planning
        let loader = ConfigLoader::from_env();
        let config = loader
            .load_verbs()
            .map_err(|e| anyhow!("Failed to load verbs config: {}", e))?;
        let registry = Arc::new(RuntimeVerbRegistry::from_config(&config));

        // Run planning facade
        let planning_input = PlanningInput::new(source, registry);
        let output = analyse_and_plan(planning_input);

        // Convert diagnostics to agent-friendly format
        let diagnostics: Vec<AgentDiagnostic> = output
            .diagnostics
            .iter()
            .map(|d| {
                let mut resolution_options = Vec::new();

                // Add resolution options based on diagnostic code
                if let Some(ref fix) = d.suggested_fix {
                    resolution_options.push(ResolutionOption {
                        description: fix.description.clone(),
                        action: "replace".to_string(),
                        replacement: Some(fix.replacement.clone()),
                    });
                }

                // Add search option for undefined symbols
                if matches!(
                    d.code,
                    crate::dsl_v2::diagnostics::DiagnosticCode::UndefinedSymbol
                ) {
                    resolution_options.push(ResolutionOption {
                        description: "Search for existing entity".to_string(),
                        action: "search".to_string(),
                        replacement: None,
                    });
                }

                AgentDiagnostic {
                    severity: crate::mcp::types::severity_to_string(d.severity),
                    message: d.message.clone(),
                    location: d.span.clone().map(|s| s.into()),
                    code: format!("{:?}", d.code),
                    resolution_options,
                }
            })
            .collect();

        // Convert synthetic steps to suggested fixes
        let suggested_fixes: Vec<SuggestedFix> = output
            .synthetic_steps
            .iter()
            .map(|step| SuggestedFix {
                description: format!(
                    "Create {} '{}' with {}",
                    step.entity_type, step.binding, step.canonical_verb
                ),
                dsl: step.suggested_dsl.clone(),
                insert_at: Some(step.insert_before_stmt as u32),
            })
            .collect();

        // Build plan summary if available
        let plan_summary = output.plan.as_ref().map(|p| p.describe());

        let has_errors = diagnostics.iter().any(|d| d.severity == "error");

        let validation_output = ValidationOutput {
            valid: !has_errors,
            diagnostics,
            plan_summary,
            suggested_fixes,
            needs_reorder: output.was_reordered,
        };

        serde_json::to_value(validation_output)
            .map_err(|e| anyhow!("Failed to serialize validation output: {}", e))
    }

    /// Execute DSL against the database
    async fn dsl_execute(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;
        let dry_run = args["dry_run"].as_bool().unwrap_or(false);

        // Extract session_id if provided - enables session state persistence
        let session_id = args["session_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok());

        // Extract user_intent if provided, otherwise use a default
        let user_intent = args["intent"].as_str().unwrap_or("MCP tool execution");

        // Start generation log
        let log_id = self
            .generation_log
            .start_log(
                user_intent,
                "mcp",
                None, // session_id
                None, // cbu_id
                None, // model
            )
            .await
            .ok();

        let start_time = std::time::Instant::now();

        // Parse
        let ast = match parse_program(source) {
            Ok(a) => a,
            Err(e) => {
                let parse_error = format!("{:?}", e);

                // Log parse failure
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(source.to_string()),
                        parse_result: ParseResult {
                            success: false,
                            error: Some(parse_error.clone()),
                        },
                        lint_result: LintResult {
                            valid: false,
                            errors: vec![],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: false,
                            error: None,
                            step_count: 0,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = self.generation_log.add_attempt(lid, &attempt).await;
                    let _ = self.generation_log.mark_failed(lid).await;
                }

                return Err(anyhow!("Parse error: {:?}", e));
            }
        };

        // CSG validation (includes dataflow)
        {
            use crate::dsl_v2::semantic_validator::SemanticValidator;
            use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};

            let validator_result = async {
                let v = SemanticValidator::new(self.pool.clone()).await?;
                v.with_csg_linter().await
            }
            .await;

            if let Ok(mut validator) = validator_result {
                let request = ValidationRequest {
                    source: source.to_string(),
                    context: ValidationContext::default(),
                };
                if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
                    validator.validate(&request).await
                {
                    let errors: Vec<String> = diagnostics
                        .iter()
                        .filter(|d| d.severity == Severity::Error)
                        .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                        .collect();
                    if !errors.is_empty() {
                        // Log validation failure
                        if let Some(lid) = log_id {
                            let attempt = GenerationAttempt {
                                attempt: 1,
                                timestamp: chrono::Utc::now(),
                                prompt_template: None,
                                prompt_text: String::new(),
                                raw_response: String::new(),
                                extracted_dsl: Some(source.to_string()),
                                parse_result: ParseResult {
                                    success: true,
                                    error: None,
                                },
                                lint_result: LintResult {
                                    valid: false,
                                    errors: errors.clone(),
                                    warnings: vec![],
                                },
                                compile_result: CompileResult {
                                    success: false,
                                    error: None,
                                    step_count: 0,
                                },
                                latency_ms: Some(start_time.elapsed().as_millis() as i32),
                                input_tokens: None,
                                output_tokens: None,
                            };
                            let _ = self.generation_log.add_attempt(lid, &attempt).await;
                            let _ = self.generation_log.mark_failed(lid).await;
                        }

                        return Err(anyhow!("Validation errors: {}", errors.join("; ")));
                    }
                }
            }
        }

        // Compile
        let plan = match compile(&ast) {
            Ok(p) => p,
            Err(e) => {
                let compile_error = format!("{:?}", e);

                // Log compile failure
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(source.to_string()),
                        parse_result: ParseResult {
                            success: true,
                            error: None,
                        },
                        lint_result: LintResult {
                            valid: false,
                            errors: vec![compile_error.clone()],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: false,
                            error: Some(compile_error.clone()),
                            step_count: 0,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = self.generation_log.add_attempt(lid, &attempt).await;
                    let _ = self.generation_log.mark_failed(lid).await;
                }

                return Err(anyhow!("Compile error: {:?}", e));
            }
        };

        if dry_run {
            // Mark as successful for dry_run (no execution needed)
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(source.to_string()),
                    parse_result: ParseResult {
                        success: true,
                        error: None,
                    },
                    lint_result: LintResult {
                        valid: true,
                        errors: vec![],
                        warnings: vec![],
                    },
                    compile_result: CompileResult {
                        success: true,
                        error: None,
                        step_count: plan.len() as i32,
                    },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = self.generation_log.add_attempt(lid, &attempt).await;
                let _ = self.generation_log.mark_success(lid, source, None).await;
            }

            let steps: Vec<_> = plan
                .steps
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    json!({
                        "index": i,
                        "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb),
                        "binding": s.bind_as
                    })
                })
                .collect();
            return Ok(json!({
                "success": true,
                "dry_run": true,
                "step_count": steps.len(),
                "steps": steps
            }));
        }

        let executor = DslExecutor::new(self.pool.clone());
        let mut ctx = ExecutionContext::new();

        match executor.execute_plan(&plan, &mut ctx).await {
            Ok(results) => {
                // Log success
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(source.to_string()),
                        parse_result: ParseResult {
                            success: true,
                            error: None,
                        },
                        lint_result: LintResult {
                            valid: true,
                            errors: vec![],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: true,
                            error: None,
                            step_count: plan.len() as i32,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = self.generation_log.add_attempt(lid, &attempt).await;
                    let _ = self.generation_log.mark_success(lid, source, None).await;
                }

                let bindings: serde_json::Map<_, _> = ctx
                    .symbols
                    .iter()
                    .map(|(k, v)| (k.clone(), json!(v.to_string())))
                    .collect();

                // Include view_state if a view.* operation produced one
                let view_state = ctx.take_pending_view_state();

                // Include viewport_state if a viewport.* operation produced one
                let viewport_state = ctx.take_pending_viewport_state();

                // Include scope_change if a session.* operation produced one
                let scope_change = ctx.take_pending_scope_change();

                // Persist to session if session_id provided and sessions available
                // This enables MCP → UI synchronization via watch channels
                if let (Some(sid), Some(sessions)) = (session_id, &self.sessions) {
                    let mut store = sessions.write().await;
                    if let Some(session) = store.get_mut(&sid) {
                        // Update bindings in session context
                        for (k, v) in ctx.symbols.iter() {
                            session.context.named_refs.insert(k.clone(), *v);
                        }

                        // Update view_state if produced
                        if let Some(ref vs) = view_state {
                            session.context.view_state = Some(vs.clone());
                        }

                        // Update viewport_state if produced
                        if let Some(ref vps) = viewport_state {
                            session.context.viewport_state = Some(vps.clone());
                        }

                        // Update scope if a session.* operation changed it
                        if let Some(ref sc) = scope_change {
                            session.context.scope =
                                crate::session::SessionScope::from_graph_scope(sc.clone());
                        }

                        // Touch updated_at to trigger watch notification
                        session.updated_at = chrono::Utc::now();

                        tracing::debug!(
                            session_id = %sid,
                            bindings_count = ctx.symbols.len(),
                            has_view_state = view_state.is_some(),
                            has_viewport_state = viewport_state.is_some(),
                            has_scope_change = scope_change.is_some(),
                            "MCP execution persisted to session"
                        );
                    }
                }

                Ok(json!({
                    "success": true,
                    "steps_executed": results.len(),
                    "bindings": bindings,
                    "view_state": view_state,
                    "viewport_state": viewport_state,
                    "scope_change": scope_change
                }))
            }
            Err(e) => {
                // Log execution failure
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(source.to_string()),
                        parse_result: ParseResult {
                            success: true,
                            error: None,
                        },
                        lint_result: LintResult {
                            valid: true,
                            errors: vec![],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: true,
                            error: None,
                            step_count: plan.len() as i32,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = self.generation_log.add_attempt(lid, &attempt).await;
                    let _ = self.generation_log.mark_failed(lid).await;
                }

                Ok(json!({
                    "success": false,
                    "error": e.to_string(),
                    "completed": ctx.symbols.len()
                }))
            }
        }
    }

    /// Execute DSL using the unified submission model
    ///
    /// Supports:
    /// - Singleton execution (one UUID per symbol)
    /// - Batch execution (multiple UUIDs for one symbol)
    /// - Draft state (unresolved symbols, can bind later)
    async fn dsl_execute_submission(&self, args: Value) -> Result<Value> {
        use crate::dsl_v2::{DomainContext, DslSubmission, SubmissionLimits, SubmissionState};

        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;

        // Parse bindings from JSON
        let bindings_json = args["bindings"].as_object();
        let confirmed = args["confirmed"].as_bool().unwrap_or(false);

        // Parse DSL
        let program = parse_program(source).map_err(|e| anyhow!("Parse error: {:?}", e))?;

        // Build submission
        let mut submission = DslSubmission::new(program.statements);

        // Parse and add bindings
        if let Some(bindings_obj) = bindings_json {
            for (symbol, value) in bindings_obj {
                let binding = Self::parse_binding_value(value)?;
                submission.set_binding(symbol, binding);
            }
        }

        let limits = SubmissionLimits::default();
        let state = submission.state(&limits);

        // Return state info for draft or warning states
        match &state {
            SubmissionState::Draft { unresolved } => {
                return Ok(json!({
                    "state": "draft",
                    "unresolved": unresolved,
                    "message": "Resolve symbols before executing"
                }));
            }
            SubmissionState::TooLarge {
                message,
                suggestion,
            } => {
                return Ok(json!({
                    "state": "too_large",
                    "message": message,
                    "suggestion": suggestion
                }));
            }
            SubmissionState::ReadyWithWarning {
                message,
                iterations,
                total_ops,
            } if !confirmed => {
                return Ok(json!({
                    "state": "warning",
                    "message": message,
                    "iterations": iterations,
                    "total_ops": total_ops,
                    "hint": "Set confirmed=true to proceed"
                }));
            }
            _ => {} // Ready or confirmed warning - proceed to execution
        }

        // Execute
        let executor = DslExecutor::new(self.pool.clone());
        let mut domain_ctx = DomainContext::new();

        match executor
            .execute_submission(&submission, &mut domain_ctx, &limits)
            .await
        {
            Ok(result) => Ok(json!({
                "success": true,
                "state": "executed",
                "is_batch": result.is_batch,
                "total_executed": result.total_executed,
                "iterations": result.iterations.iter().map(|i| json!({
                    "index": i.index,
                    "success": i.success,
                    "bindings": i.bindings.iter()
                        .map(|(k, v)| (k.clone(), json!(v.to_string())))
                        .collect::<serde_json::Map<_, _>>(),
                    "error": i.error
                })).collect::<Vec<_>>()
            })),
            Err(e) => Ok(json!({
                "success": false,
                "state": "error",
                "error": e.to_string()
            })),
        }
    }

    /// Bind symbols to UUIDs for a pending submission
    ///
    /// Used to incrementally resolve symbols in draft submissions.
    async fn dsl_bind(&self, args: Value) -> Result<Value> {
        use crate::dsl_v2::{DslSubmission, SubmissionLimits, SymbolBinding};

        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;
        let symbol = args["symbol"]
            .as_str()
            .ok_or_else(|| anyhow!("symbol required"))?;

        // Parse IDs
        let ids: Vec<Uuid> = match &args["ids"] {
            Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(Uuid::parse_str)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow!("Invalid UUID: {}", e))?,
            Value::String(s) => {
                vec![Uuid::parse_str(s).map_err(|e| anyhow!("Invalid UUID: {}", e))?]
            }
            _ => return Err(anyhow!("ids must be a string or array of strings")),
        };

        // Parse optional names
        let names: Vec<String> = args["names"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // Parse DSL and build submission
        let program = parse_program(source).map_err(|e| anyhow!("Parse error: {:?}", e))?;
        let mut submission = DslSubmission::new(program.statements);

        // Apply existing bindings if provided
        if let Some(bindings_obj) = args["bindings"].as_object() {
            for (sym, value) in bindings_obj {
                let binding = Self::parse_binding_value(value)?;
                submission.set_binding(sym, binding);
            }
        }

        // Add the new binding
        let binding = if names.is_empty() {
            SymbolBinding::multiple(ids)
        } else {
            SymbolBinding::multiple_named(ids.into_iter().zip(names).collect())
        };
        submission.set_binding(symbol, binding);

        // Return updated state
        let limits = SubmissionLimits::default();
        let state = submission.state(&limits);
        let unresolved = submission.unresolved_symbols();

        // Serialize bindings for return
        let bindings_out: serde_json::Map<_, _> = submission
            .bindings
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!({
                        "ids": v.ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                        "names": v.names,
                        "count": v.len()
                    }),
                )
            })
            .collect();

        Ok(json!({
            "state": match state {
                crate::dsl_v2::SubmissionState::Draft { .. } => "draft",
                crate::dsl_v2::SubmissionState::Ready => "ready",
                crate::dsl_v2::SubmissionState::ReadyWithWarning { .. } => "warning",
                crate::dsl_v2::SubmissionState::TooLarge { .. } => "too_large",
            },
            "unresolved": unresolved,
            "bindings": bindings_out,
            "iteration_count": submission.iteration_count(),
            "total_ops": submission.total_operations()
        }))
    }

    /// Parse a JSON value into a SymbolBinding
    fn parse_binding_value(value: &Value) -> Result<crate::dsl_v2::SymbolBinding> {
        use crate::dsl_v2::SymbolBinding;

        match value {
            Value::Null => Ok(SymbolBinding::unresolved()),
            Value::Array(arr) if arr.is_empty() => Ok(SymbolBinding::unresolved()),
            Value::String(s) => {
                let id = Uuid::parse_str(s).map_err(|e| anyhow!("Invalid UUID: {}", e))?;
                Ok(SymbolBinding::singleton(id))
            }
            Value::Array(arr) => {
                let mut ids = vec![];
                let mut names = vec![];
                for item in arr {
                    match item {
                        Value::String(s) => {
                            ids.push(
                                Uuid::parse_str(s).map_err(|e| anyhow!("Invalid UUID: {}", e))?,
                            );
                        }
                        Value::Object(obj) => {
                            if let Some(id_str) = obj.get("id").and_then(|v| v.as_str()) {
                                ids.push(
                                    Uuid::parse_str(id_str)
                                        .map_err(|e| anyhow!("Invalid UUID: {}", e))?,
                                );
                                if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                                    names.push(name.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(SymbolBinding {
                    ids,
                    names,
                    entity_type: None,
                })
            }
            _ => Ok(SymbolBinding::unresolved()),
        }
    }

    /// Show execution plan without running
    async fn dsl_plan(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;

        let ast = parse_program(source).map_err(|e| anyhow!("Parse: {:?}", e))?;
        let plan = compile(&ast).map_err(|e| anyhow!("Compile: {:?}", e))?;

        let steps: Vec<_> = plan
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "index": i,
                    "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb),
                    "binding": s.bind_as,
                    "args": s.verb_call.arguments.iter().map(|a| {
                        json!({
                            "key": a.key.clone(),
                            "value": format!("{:?}", a.value)
                        })
                    }).collect::<Vec<_>>()
                })
            })
            .collect();

        Ok(json!({
            "valid": true,
            "step_count": plan.steps.len(),
            "steps": steps
        }))
    }

    /// Generate DSL from natural language using intent extraction
    async fn dsl_generate(&self, args: Value) -> Result<Value> {
        use crate::agentic::create_llm_client;

        let instruction = args["instruction"]
            .as_str()
            .ok_or_else(|| anyhow!("instruction required"))?;
        let _domain = args["domain"].as_str();
        let execute = args["execute"].as_bool().unwrap_or(false);

        // Create LLM client (uses AGENT_BACKEND env var to select provider)
        let llm_client = create_llm_client()?;

        // Build vocabulary prompt for context
        let reg = registry();
        let vocab: Vec<_> = reg
            .all_verbs()
            .take(50) // Limit for context
            .map(|v| format!("{}: {}", v.full_name(), v.description))
            .collect();

        let system_prompt = format!(
            r#"You are a DSL generator for a KYC/AML onboarding system.
Generate valid DSL S-expressions from natural language instructions.

DSL SYNTAX:
- Format: (domain.verb :key "value" :key2 value2)
- Strings must be quoted: "text"
- Numbers are unquoted: 42
- References start with @: @symbol_name
- Use :as @name to capture results

COMMON VERBS:
{}

Respond with ONLY the DSL, no explanation. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
            vocab.join("\n")
        );

        // Call LLM API (Anthropic or OpenAI based on AGENT_BACKEND)
        let dsl = llm_client
            .chat(&system_prompt, instruction)
            .await
            .map_err(|e| anyhow!("LLM API error: {}", e))?
            .trim()
            .to_string();

        if dsl.starts_with("ERROR:") {
            return Ok(json!({
                "success": false,
                "error": dsl
            }));
        }

        // Validate the generated DSL
        let validation = match parse_program(&dsl) {
            Ok(ast) => match compile(&ast) {
                Ok(plan) => json!({
                    "valid": true,
                    "step_count": plan.steps.len()
                }),
                Err(e) => json!({
                    "valid": false,
                    "error": format!("Compile error: {:?}", e)
                }),
            },
            Err(e) => json!({
                "valid": false,
                "error": format!("Parse error: {:?}", e)
            }),
        };

        // Execute if requested and valid
        if execute && validation["valid"].as_bool().unwrap_or(false) {
            let exec_result = self
                .dsl_execute(json!({
                    "source": dsl,
                    "intent": instruction
                }))
                .await?;

            return Ok(json!({
                "success": true,
                "dsl": dsl,
                "validation": validation,
                "execution": exec_result
            }));
        }

        Ok(json!({
            "success": validation["valid"].as_bool().unwrap_or(false),
            "dsl": dsl,
            "validation": validation
        }))
    }

    /// Get CBU with all related data
    async fn cbu_get(&self, args: Value) -> Result<Value> {
        let cbu_id = Uuid::parse_str(
            args["cbu_id"]
                .as_str()
                .ok_or_else(|| anyhow!("cbu_id required"))?,
        )?;

        let cbu = self
            .repo
            .get_cbu_basic(cbu_id)
            .await?
            .ok_or_else(|| anyhow!("CBU not found"))?;

        let entities = self.repo.get_cbu_entities(cbu_id).await?;
        let roles = self.repo.get_cbu_roles(cbu_id).await?;
        let documents = self.repo.get_cbu_documents(cbu_id).await?;
        let screenings = self.repo.get_cbu_screenings(cbu_id).await?;

        Ok(json!({
            "cbu": {
                "cbu_id": cbu.cbu_id.to_string(),
                "name": cbu.name,
                "client_type": cbu.client_type,
                "jurisdiction": cbu.jurisdiction
            },
            "entities": entities.iter().map(|e| json!({
                "entity_id": e.entity_id.to_string(),
                "name": e.name,
                "entity_type": e.entity_type
            })).collect::<Vec<_>>(),
            "roles": roles.iter().map(|r| json!({
                "entity_id": r.entity_id.to_string(),
                "role": r.role_name
            })).collect::<Vec<_>>(),
            "documents": documents.iter().map(|d| json!({
                "doc_id": d.doc_id.to_string(),
                "document_type": d.document_type_code,
                "status": d.status
            })).collect::<Vec<_>>(),
            "screenings": screenings.iter().map(|s| json!({
                "screening_id": s.screening_id.to_string(),
                "entity_id": s.entity_id.to_string(),
                "screening_type": s.screening_type,
                "status": s.status,
                "result": s.result
            })).collect::<Vec<_>>(),
            "summary": {
                "entities": entities.len(),
                "roles": roles.len(),
                "documents": documents.len(),
                "screenings": screenings.len()
            }
        }))
    }

    /// List CBUs with filtering
    async fn cbu_list(&self, args: Value) -> Result<Value> {
        let limit = args["limit"].as_i64().unwrap_or(20);
        let search = args["search"].as_str();

        let cbus = self.repo.list_cbus_filtered(search, limit).await?;

        Ok(json!({
            "cbus": cbus.iter().map(|c| json!({
                "cbu_id": c.cbu_id.to_string(),
                "name": c.name,
                "client_type": c.client_type,
                "jurisdiction": c.jurisdiction
            })).collect::<Vec<_>>(),
            "total": cbus.len()
        }))
    }

    /// Get entity details
    async fn entity_get(&self, args: Value) -> Result<Value> {
        let entity_id = Uuid::parse_str(
            args["entity_id"]
                .as_str()
                .ok_or_else(|| anyhow!("entity_id required"))?,
        )?;

        let entity = self
            .repo
            .get_entity_basic(entity_id)
            .await?
            .ok_or_else(|| anyhow!("Entity not found"))?;

        let cbus = self.repo.get_entity_cbus(entity_id).await?;
        let roles = self.repo.get_entity_roles(entity_id).await?;
        let documents = self.repo.get_entity_documents(entity_id).await?;
        let screenings = self.repo.get_entity_screenings(entity_id).await?;

        Ok(json!({
            "entity": {
                "entity_id": entity.entity_id.to_string(),
                "name": entity.name,
                "entity_type": entity.type_code
            },
            "cbus": cbus.iter().map(|c| json!({
                "cbu_id": c.cbu_id.to_string(),
                "name": c.cbu_name
            })).collect::<Vec<_>>(),
            "roles": roles.iter().map(|r| json!({
                "role": r.role_name,
                "cbu_id": r.cbu_id.to_string()
            })).collect::<Vec<_>>(),
            "documents": documents.iter().map(|d| json!({
                "doc_id": d.doc_id.to_string(),
                "document_type": d.document_type_code,
                "status": d.status
            })).collect::<Vec<_>>(),
            "screenings": screenings.iter().map(|s| json!({
                "screening_id": s.screening_id.to_string(),
                "screening_type": s.screening_type,
                "status": s.status,
                "result": s.result
            })).collect::<Vec<_>>()
        }))
    }

    /// List available DSL verbs
    fn verbs_list(&self, args: Value) -> Result<Value> {
        let domain_filter = args["domain"].as_str();
        let reg = registry();

        let verbs: Vec<_> = reg
            .all_verbs()
            .filter(|v| domain_filter.is_none_or(|d| v.domain == d))
            .map(|v| {
                json!({
                    "verb": v.full_name(),
                    "domain": v.domain,
                    "description": v.description,
                    "args": v.args.iter().map(|a| json!({
                        "name": a.name,
                        "type": a.arg_type,
                        "required": a.required
                    })).collect::<Vec<_>>()
                })
            })
            .collect();

        let domains: Vec<_> = reg.domains().to_vec();

        Ok(json!({
            "domains": domains,
            "verb_count": verbs.len(),
            "verbs": verbs
        }))
    }

    /// Get entity types, roles, document types from database
    async fn schema_info(&self, args: Value) -> Result<Value> {
        let category = args["category"].as_str().unwrap_or("all");
        let mut result = json!({});

        if category == "all" || category == "entity_types" {
            let types = self.repo.get_entity_types().await?;
            result["entity_types"] = json!(types
                .iter()
                .map(|t| json!({"code": t.type_code, "name": t.name}))
                .collect::<Vec<_>>());
        }

        if category == "all" || category == "roles" {
            let roles = self.repo.get_all_roles().await?;
            result["roles"] = json!(roles
                .iter()
                .map(|r| json!({"id": r.role_id.to_string(), "name": r.name}))
                .collect::<Vec<_>>());
        }

        if category == "all" || category == "document_types" {
            let docs = self.repo.get_document_types().await?;
            result["document_types"] = json!(docs
                .iter()
                .map(|d| json!({"code": d.type_code, "name": d.display_name}))
                .collect::<Vec<_>>());
        }

        Ok(result)
    }

    /// Look up database IDs via EntityGateway - the key tool to prevent UUID hallucination
    ///
    /// All lookups go through the central EntityGateway service for consistent
    /// fuzzy matching behavior across LSP, validation, and MCP tools.
    async fn dsl_lookup(&self, args: Value) -> Result<Value> {
        let lookup_type = args["lookup_type"]
            .as_str()
            .ok_or_else(|| anyhow!("lookup_type required"))?;
        let search = args["search"].as_str();
        let limit = args["limit"].as_i64().unwrap_or(10) as i32;

        // Map lookup_type to EntityGateway nickname
        let nickname = match lookup_type {
            "cbu" => "CBU",
            "entity" => "ENTITY",
            "person" => "PERSON",
            "legal_entity" | "company" => "LEGAL_ENTITY",
            "document" => "DOCUMENT",
            "product" => "PRODUCT",
            "service" => "SERVICE",
            "role" => "ROLE",
            "jurisdiction" => "JURISDICTION",
            "currency" => "CURRENCY",
            "document_type" => "DOCUMENT_TYPE",
            "entity_type" => "ENTITY_TYPE",
            "attribute" => "ATTRIBUTE",
            "instrument_class" => "INSTRUMENT_CLASS",
            "market" => "MARKET",
            _ => {
                return Err(anyhow!(
                    "Unknown lookup_type: {}. Valid types: cbu, entity, person, legal_entity, document, product, service, role, jurisdiction, currency, document_type, entity_type, attribute, instrument_class, market",
                    lookup_type
                ));
            }
        };

        // Search via EntityGateway
        let matches = self.gateway_search(nickname, search, limit).await?;

        Ok(json!({
            "type": lookup_type,
            "count": matches.len(),
            "results": matches.iter().map(|(id, display, score)| json!({
                "id": id,
                "display": display,
                "score": score
            })).collect::<Vec<_>>()
        }))
    }

    /// Get completions for DSL - verbs, domains, products, roles
    fn dsl_complete(&self, args: Value) -> Result<Value> {
        let completion_type = args["completion_type"]
            .as_str()
            .ok_or_else(|| anyhow!("completion_type required"))?;
        let prefix = args["prefix"].as_str().unwrap_or("");
        let domain_filter = args["domain"].as_str();

        match completion_type {
            "verb" => {
                let reg = registry();
                let verbs: Vec<_> = reg
                    .all_verbs()
                    .filter(|v| domain_filter.is_none_or(|d| v.domain == d))
                    .filter(|v| {
                        prefix.is_empty()
                            || v.full_name()
                                .to_lowercase()
                                .contains(&prefix.to_lowercase())
                    })
                    .map(|v| {
                        json!({
                            "name": v.full_name(),
                            "domain": v.domain,
                            "description": v.description
                        })
                    })
                    .collect();

                Ok(json!({
                    "type": "verb",
                    "count": verbs.len(),
                    "completions": verbs
                }))
            }

            "domain" => {
                let reg = registry();
                let domains: Vec<_> = reg
                    .domains()
                    .iter()
                    .filter(|d| {
                        prefix.is_empty() || d.to_lowercase().contains(&prefix.to_lowercase())
                    })
                    .map(|d| json!({"name": d}))
                    .collect();

                Ok(json!({
                    "type": "domain",
                    "count": domains.len(),
                    "completions": domains
                }))
            }

            "product" => {
                // Return hardcoded product names from database
                let products = [
                    (
                        "Custody",
                        "Asset safekeeping, settlement, corporate actions",
                    ),
                    ("Fund Accounting", "NAV calculation, investor accounting"),
                    ("Transfer Agency", "Investor registry, subscriptions"),
                    ("Middle Office", "Position management, P&L"),
                    ("Collateral Management", "Collateral optimization"),
                    ("Markets FX", "Foreign exchange services"),
                    ("Alternatives", "Alternative investment admin"),
                ];

                let filtered: Vec<_> = products
                    .iter()
                    .filter(|(name, _)| {
                        prefix.is_empty() || name.to_lowercase().contains(&prefix.to_lowercase())
                    })
                    .map(|(name, desc)| json!({"name": name, "description": desc}))
                    .collect();

                Ok(json!({
                    "type": "product",
                    "count": filtered.len(),
                    "completions": filtered
                }))
            }

            "role" => {
                let roles = vec![
                    "DIRECTOR",
                    "SHAREHOLDER",
                    "BENEFICIAL_OWNER",
                    "PRINCIPAL",
                    "SIGNATORY",
                    "TRUSTEE",
                    "BENEFICIARY",
                    "PROTECTOR",
                    "SETTLOR",
                    "PARTNER",
                    "GENERAL_PARTNER",
                    "LIMITED_PARTNER",
                ];

                let filtered: Vec<_> = roles
                    .iter()
                    .filter(|r| {
                        prefix.is_empty() || r.to_lowercase().contains(&prefix.to_lowercase())
                    })
                    .map(|r| json!({"name": r}))
                    .collect();

                Ok(json!({
                    "type": "role",
                    "count": filtered.len(),
                    "completions": filtered
                }))
            }

            _ => Err(anyhow!(
                "Unknown completion_type: {}. Valid types: verb, domain, product, role",
                completion_type
            )),
        }
    }

    /// Get verb signature - parameters and types
    fn dsl_signature(&self, args: Value) -> Result<Value> {
        let verb_name = args["verb"]
            .as_str()
            .ok_or_else(|| anyhow!("verb required"))?;

        let reg = registry();

        // Parse domain.verb format
        let parts: Vec<&str> = verb_name.split('.').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid verb format '{}'. Expected 'domain.verb' (e.g., 'cbu.add-product')",
                verb_name
            ));
        }

        let domain = parts[0];
        let verb = parts[1];

        // Find the verb in registry
        let verb_info = reg
            .all_verbs()
            .find(|v| v.domain == domain && v.verb == verb)
            .ok_or_else(|| anyhow!("Verb '{}' not found", verb_name))?;

        Ok(json!({
            "verb": verb_info.full_name(),
            "domain": verb_info.domain,
            "description": verb_info.description,
            "parameters": verb_info.args.iter().map(|a| json!({
                "name": a.name,
                "type": a.arg_type,
                "required": a.required,
                "description": a.description
            })).collect::<Vec<_>>(),
            "behavior": format!("{:?}", verb_info.behavior),
            "example": format!(
                "({} {})",
                verb_info.full_name(),
                verb_info.args.iter()
                    .filter(|a| a.required)
                    .map(|a| format!(":{} <{}>", a.name, a.arg_type))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        }))
    }

    /// Manage conversation session state
    ///
    /// Sessions persist bindings across multiple DSL executions within a conversation.
    fn session_context(&self, args: Value) -> Result<Value> {
        use crate::mcp::session;
        use crate::mcp::types::SessionAction;

        let action: SessionAction =
            serde_json::from_value(args).map_err(|e| anyhow!("Invalid session action: {}", e))?;

        let state = session::session_context(action).map_err(|e| anyhow!("{}", e))?;

        serde_json::to_value(state).map_err(|e| anyhow!("Failed to serialize session state: {}", e))
    }

    /// Search for entities with fuzzy matching, enrichment, and smart disambiguation
    ///
    /// Returns matches enriched with context (roles, relationships, dates) and
    /// uses resolution strategy to determine whether to auto-resolve, ask user,
    /// or suggest creating a new entity.
    ///
    /// ## Features
    /// - Rich context for disambiguation (nationality, DOB, roles, ownership)
    /// - Context-aware auto-resolution (e.g., "the director" → picks entity with DIRECTOR role)
    /// - Human-readable disambiguation labels
    /// - Confidence scoring with suggested actions
    async fn entity_search(&self, args: Value) -> Result<Value> {
        use crate::mcp::enrichment::{EntityEnricher, EntityType as EnrichEntityType};
        use crate::mcp::resolution::{ConversationContext, EnrichedMatch, ResolutionStrategy};

        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow!("query required"))?;
        let entity_type_str = args["entity_type"].as_str();
        let limit = args["limit"].as_i64().unwrap_or(10) as i32;

        // Parse conversation hints for context-aware resolution
        let conversation_hints: Option<ConversationContext> = args
            .get("conversation_hints")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // Map entity_type to EntityGateway nickname
        let nickname = match entity_type_str {
            Some("cbu") => "CBU",
            Some("entity") => "ENTITY",
            Some("person") => "PERSON",
            Some("company") | Some("legal_entity") => "LEGAL_ENTITY",
            Some("document") => "DOCUMENT",
            Some("product") => "PRODUCT",
            Some("service") => "SERVICE",
            None => "ENTITY", // Default to entity search
            Some(t) => {
                return Err(anyhow!(
                    "Unknown entity_type: {}. Valid types: cbu, entity, person, company, document, product, service",
                    t
                ));
            }
        };

        // Step 1: Search via EntityGateway
        let raw_matches = self.gateway_search(nickname, Some(query), limit).await?;

        if raw_matches.is_empty() {
            let result = crate::mcp::resolution::ResolutionResult {
                confidence: crate::mcp::resolution::ResolutionConfidence::None,
                action: crate::mcp::resolution::SuggestedAction::SuggestCreate,
                prompt: Some(format!(
                    "No matches found for '{}'. Would you like to create a new entity?",
                    query
                )),
            };
            return Ok(json!({
                "matches": [],
                "resolution_confidence": result.confidence,
                "suggested_action": result.action,
                "disambiguation_prompt": result.prompt
            }));
        }

        // Step 2: Extract UUIDs for enrichment
        let ids: Vec<Uuid> = raw_matches
            .iter()
            .filter_map(|(id, _, _)| Uuid::parse_str(id).ok())
            .collect();

        // Step 3: Determine entity type for enrichment
        let enrich_type = match entity_type_str {
            Some("person") => EnrichEntityType::ProperPerson,
            Some("company") | Some("legal_entity") => EnrichEntityType::LegalEntity,
            Some("cbu") => EnrichEntityType::Cbu,
            _ => EnrichEntityType::ProperPerson, // Default
        };

        // Step 4: Enrich with context (roles, nationality, etc.)
        let enricher = EntityEnricher::new(self.pool.clone());
        let contexts = enricher.enrich(enrich_type, &ids).await.unwrap_or_default();

        // Step 5: Build enriched matches with disambiguation labels
        let enriched_matches: Vec<EnrichedMatch> = raw_matches
            .iter()
            .map(|(id, display, score)| {
                let uuid = Uuid::parse_str(id).ok();
                let context = uuid
                    .and_then(|u| contexts.get(&u).cloned())
                    .unwrap_or_default();
                let disambiguation_label = context.disambiguation_label(display, enrich_type);

                EnrichedMatch {
                    id: id.clone(),
                    display: display.clone(),
                    score: *score,
                    entity_type: entity_type_str.unwrap_or("entity").to_string(),
                    context,
                    disambiguation_label,
                }
            })
            .collect();

        // Step 6: Analyze and determine resolution strategy
        let resolution =
            ResolutionStrategy::analyze(&enriched_matches, conversation_hints.as_ref());

        // Step 7: Build response
        Ok(json!({
            "matches": enriched_matches,
            "resolution_confidence": resolution.confidence,
            "suggested_action": resolution.action,
            "disambiguation_prompt": resolution.prompt
        }))
    }

    // ==================== Workflow Orchestration Tools ====================

    /// Get workflow status for a subject
    async fn workflow_status(&self, args: Value) -> Result<Value> {
        use crate::workflow::{WorkflowEngine, WorkflowLoader};
        use std::path::Path;

        let subject_type = args["subject_type"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_type required"))?;
        let subject_id: Uuid = args["subject_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subject_id UUID"))?;
        let workflow_id = args["workflow_id"].as_str().unwrap_or("kyc_onboarding");

        // Load workflow definitions
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let workflows_path = Path::new(&config_dir).join("workflows");
        let definitions = WorkflowLoader::load_from_dir(&workflows_path)
            .map_err(|e| anyhow!("Failed to load workflows: {}", e))?;

        let engine = WorkflowEngine::new(self.pool.clone(), definitions);

        // Find or get existing workflow
        let instance = engine
            .find_or_start(workflow_id, subject_type, subject_id, None)
            .await
            .map_err(|e| anyhow!("Workflow error: {}", e))?;

        let status = engine
            .get_status(instance.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to get status: {}", e))?;

        Ok(serde_json::to_value(status)?)
    }

    /// Try to advance workflow automatically
    async fn workflow_advance(&self, args: Value) -> Result<Value> {
        use crate::workflow::{WorkflowEngine, WorkflowLoader};
        use std::path::Path;

        let subject_type = args["subject_type"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_type required"))?;
        let subject_id: Uuid = args["subject_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subject_id UUID"))?;
        let workflow_id = args["workflow_id"].as_str().unwrap_or("kyc_onboarding");

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let workflows_path = Path::new(&config_dir).join("workflows");
        let definitions = WorkflowLoader::load_from_dir(&workflows_path)
            .map_err(|e| anyhow!("Failed to load workflows: {}", e))?;

        let engine = WorkflowEngine::new(self.pool.clone(), definitions);

        // Find existing workflow
        let instance = engine
            .find_or_start(workflow_id, subject_type, subject_id, None)
            .await
            .map_err(|e| anyhow!("Workflow error: {}", e))?;

        // Try to advance
        let advanced = engine
            .try_advance(instance.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to advance: {}", e))?;

        // Get updated status
        let status = engine
            .get_status(advanced.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to get status: {}", e))?;

        Ok(json!({
            "advanced": advanced.current_state != instance.current_state,
            "previous_state": instance.current_state,
            "current_state": advanced.current_state,
            "status": status
        }))
    }

    /// Manually transition to a specific state
    async fn workflow_transition(&self, args: Value) -> Result<Value> {
        use crate::workflow::{WorkflowEngine, WorkflowLoader};
        use std::path::Path;

        let subject_type = args["subject_type"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_type required"))?;
        let subject_id: Uuid = args["subject_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subject_id UUID"))?;
        let workflow_id = args["workflow_id"].as_str().unwrap_or("kyc_onboarding");
        let to_state = args["to_state"]
            .as_str()
            .ok_or_else(|| anyhow!("to_state required"))?;
        let reason = args["reason"].as_str().map(|s| s.to_string());

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let workflows_path = Path::new(&config_dir).join("workflows");
        let definitions = WorkflowLoader::load_from_dir(&workflows_path)
            .map_err(|e| anyhow!("Failed to load workflows: {}", e))?;

        let engine = WorkflowEngine::new(self.pool.clone(), definitions);

        // Find existing workflow
        let instance = engine
            .find_or_start(workflow_id, subject_type, subject_id, None)
            .await
            .map_err(|e| anyhow!("Workflow error: {}", e))?;

        let previous_state = instance.current_state.clone();

        // Transition
        let transitioned = engine
            .transition(
                instance.instance_id,
                to_state,
                Some("mcp_tool".to_string()),
                reason,
            )
            .await
            .map_err(|e| anyhow!("Transition failed: {}", e))?;

        // Get updated status
        let status = engine
            .get_status(transitioned.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to get status: {}", e))?;

        Ok(json!({
            "success": true,
            "previous_state": previous_state,
            "current_state": transitioned.current_state,
            "status": status
        }))
    }

    /// Start a new workflow
    async fn workflow_start(&self, args: Value) -> Result<Value> {
        use crate::workflow::{WorkflowEngine, WorkflowLoader};
        use std::path::Path;

        let workflow_id = args["workflow_id"]
            .as_str()
            .ok_or_else(|| anyhow!("workflow_id required"))?;
        let subject_type = args["subject_type"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_type required"))?;
        let subject_id: Uuid = args["subject_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subject_id UUID"))?;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let workflows_path = Path::new(&config_dir).join("workflows");
        let definitions = WorkflowLoader::load_from_dir(&workflows_path)
            .map_err(|e| anyhow!("Failed to load workflows: {}", e))?;

        let engine = WorkflowEngine::new(self.pool.clone(), definitions);

        // Start new workflow
        let instance = engine
            .start_workflow(
                workflow_id,
                subject_type,
                subject_id,
                Some("mcp_tool".to_string()),
            )
            .await
            .map_err(|e| anyhow!("Failed to start workflow: {}", e))?;

        // Get status
        let status = engine
            .get_status(instance.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to get status: {}", e))?;

        Ok(json!({
            "instance_id": instance.instance_id,
            "workflow_id": workflow_id,
            "current_state": instance.current_state,
            "status": status
        }))
    }

    /// Get DSL template to resolve a blocker
    fn resolve_blocker(&self, args: Value) -> Result<Value> {
        let blocker_type = args["blocker_type"]
            .as_str()
            .ok_or_else(|| anyhow!("blocker_type required"))?;
        let context = args.get("context").cloned().unwrap_or(json!({}));

        let (verb, template, description) = match blocker_type {
            "missing_role" => {
                let role = context["role"].as_str().unwrap_or("DIRECTOR");
                let cbu_id = context["cbu_id"].as_str().unwrap_or("<cbu-id>");
                let entity_id = context["entity_id"].as_str().unwrap_or("<entity-id>");
                (
                    "cbu.assign-role",
                    format!(
                        "(cbu.assign-role :cbu-id {} :entity-id {} :role \"{}\")",
                        cbu_id, entity_id, role
                    ),
                    format!("Assign {} role to entity", role),
                )
            }
            "missing_document" => {
                let doc_type = context["document_type"].as_str().unwrap_or("PASSPORT");
                let cbu_id = context["cbu_id"].as_str().unwrap_or("<cbu-id>");
                (
                    "document.catalog",
                    format!(
                        "(document.catalog :cbu-id {} :doc-type \"{}\" :title \"<title>\")",
                        cbu_id, doc_type
                    ),
                    format!("Catalog {} document", doc_type),
                )
            }
            "pending_screening" => {
                let entity_id = context["entity_id"].as_str().unwrap_or("<entity-id>");
                let workstream_id = context["workstream_id"]
                    .as_str()
                    .unwrap_or("<workstream-id>");
                (
                    "case-screening.run",
                    format!(
                        "(case-screening.run :workstream-id {} :screening-type \"SANCTIONS\")",
                        workstream_id
                    ),
                    format!("Run screening for entity {}", entity_id),
                )
            }
            "unresolved_alert" => {
                let screening_id = context["screening_id"].as_str().unwrap_or("<screening-id>");
                (
                    "case-screening.review-hit",
                    format!(
                        "(case-screening.review-hit :screening-id {} :disposition \"FALSE_POSITIVE\" :notes \"<reason>\")",
                        screening_id
                    ),
                    "Review and resolve screening alert".to_string(),
                )
            }
            "incomplete_ownership" => {
                let owner_id = context["owner_entity_id"]
                    .as_str()
                    .unwrap_or("<owner-entity-id>");
                let owned_id = context["owned_entity_id"]
                    .as_str()
                    .unwrap_or("<owned-entity-id>");
                (
                    "ubo.add-ownership",
                    format!(
                        "(ubo.add-ownership :owner-entity-id {} :owned-entity-id {} :percentage <pct> :ownership-type \"DIRECT\")",
                        owner_id, owned_id
                    ),
                    "Add ownership relationship".to_string(),
                )
            }
            "unverified_ubo" => {
                let ubo_id = context["ubo_id"].as_str().unwrap_or("<ubo-id>");
                (
                    "ubo.verify-ubo",
                    format!(
                        "(ubo.verify-ubo :ubo-id {} :verification-status \"VERIFIED\" :risk-rating \"LOW\")",
                        ubo_id
                    ),
                    "Verify UBO".to_string(),
                )
            }
            _ => {
                return Err(anyhow!("Unknown blocker type: {}", blocker_type));
            }
        };

        Ok(json!({
            "blocker_type": blocker_type,
            "verb": verb,
            "dsl_template": template,
            "description": description
        }))
    }

    // ==================== Template Tools ====================

    /// List available templates with filtering
    fn template_list(&self, args: Value) -> Result<Value> {
        use crate::templates::TemplateRegistry;
        use std::path::Path;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        // Apply filters
        let templates: Vec<_> = if let Some(blocker) = args["blocker"].as_str() {
            registry.find_by_blocker(blocker)
        } else if let Some(tag) = args["tag"].as_str() {
            registry.find_by_tag(tag)
        } else if let (Some(workflow), Some(state)) =
            (args["workflow"].as_str(), args["state"].as_str())
        {
            registry.find_by_workflow_state(workflow, state)
        } else if let Some(search) = args["search"].as_str() {
            registry.search(search)
        } else {
            registry.list()
        };

        let results: Vec<_> = templates
            .iter()
            .map(|t| {
                json!({
                    "template_id": t.template,
                    "name": t.metadata.name,
                    "summary": t.metadata.summary,
                    "tags": t.tags,
                    "resolves_blockers": t.workflow_context.resolves_blockers,
                    "applicable_states": t.workflow_context.applicable_states
                })
            })
            .collect();

        Ok(json!({
            "count": results.len(),
            "templates": results
        }))
    }

    /// Get full template details
    fn template_get(&self, args: Value) -> Result<Value> {
        use crate::templates::TemplateRegistry;
        use std::path::Path;

        let template_id = args["template_id"]
            .as_str()
            .ok_or_else(|| anyhow!("template_id required"))?;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        let template = registry
            .get(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Build parameter info
        let params: Vec<_> = template
            .params
            .iter()
            .map(|(name, def)| {
                json!({
                    "name": name,
                    "type": def.param_type,
                    "required": def.required,
                    "source": def.source,
                    "default": def.default,
                    "prompt": def.prompt,
                    "example": def.example,
                    "validation": def.validation,
                    "enum_values": def.enum_values
                })
            })
            .collect();

        Ok(json!({
            "template_id": template.template,
            "version": template.version,
            "metadata": {
                "name": template.metadata.name,
                "summary": template.metadata.summary,
                "description": template.metadata.description,
                "when_to_use": template.metadata.when_to_use,
                "when_not_to_use": template.metadata.when_not_to_use,
                "effects": template.metadata.effects,
                "next_steps": template.metadata.next_steps
            },
            "tags": template.tags,
            "workflow_context": {
                "applicable_workflows": template.workflow_context.applicable_workflows,
                "applicable_states": template.workflow_context.applicable_states,
                "resolves_blockers": template.workflow_context.resolves_blockers
            },
            "params": params,
            "body": template.body,
            "outputs": template.outputs,
            "related_templates": template.related_templates
        }))
    }

    /// Expand a template to DSL source text
    fn template_expand(&self, args: Value) -> Result<Value> {
        use crate::templates::{ExpansionContext, TemplateExpander, TemplateRegistry};
        use std::collections::HashMap;
        use std::path::Path;

        let template_id = args["template_id"]
            .as_str()
            .ok_or_else(|| anyhow!("template_id required"))?;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        let template = registry
            .get(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Build explicit params from args
        let explicit_params: HashMap<String, String> = args["params"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Build expansion context from session info
        let mut context = ExpansionContext::new();

        if let Some(cbu_id) = args["cbu_id"].as_str() {
            if let Ok(uuid) = Uuid::parse_str(cbu_id) {
                context.current_cbu = Some(uuid);
            }
        }

        if let Some(case_id) = args["case_id"].as_str() {
            if let Ok(uuid) = Uuid::parse_str(case_id) {
                context.current_case = Some(uuid);
            }
        }

        // Expand template
        let result = TemplateExpander::expand(template, &explicit_params, &context);

        // Format missing params prompt if any
        let prompt = if result.missing_params.is_empty() {
            None
        } else {
            Some(TemplateExpander::format_missing_params_prompt(
                &result.missing_params,
            ))
        };

        Ok(json!({
            "template_id": result.template_id,
            "dsl": result.dsl,
            "complete": result.missing_params.is_empty(),
            "filled_params": result.filled_params,
            "missing_params": result.missing_params.iter().map(|p| json!({
                "name": p.name,
                "type": p.param_type,
                "prompt": p.prompt,
                "example": p.example,
                "required": p.required,
                "validation": p.validation
            })).collect::<Vec<_>>(),
            "prompt": prompt,
            "outputs": result.outputs
        }))
    }

    // =========================================================================
    // Template Batch Execution Handlers
    // =========================================================================
    //
    // These handlers operate on the UI SessionStore (self.sessions).
    // The SessionStore is the SINGLE SOURCE OF TRUTH for session state.
    // egui and all other consumers access the same store.

    /// Start a template batch execution session
    async fn batch_start(&self, args: Value) -> Result<Value> {
        use crate::api::session::{
            SessionMode, TemplateExecutionContext, TemplateParamKeySet, TemplatePhase,
        };
        use crate::templates::TemplateRegistry;
        use std::path::Path;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let template_id = args["template_id"]
            .as_str()
            .ok_or_else(|| anyhow!("template_id required"))?;

        // Load template
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("verbs/templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        let template = registry
            .get(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Extract entity dependencies from template
        let entity_deps = template.entity_dependency_summary();

        // Initialize key sets from template params
        let mut key_sets = std::collections::HashMap::new();

        for param_info in &entity_deps.batch_params {
            key_sets.insert(
                param_info.param_name.clone(),
                TemplateParamKeySet {
                    param_name: param_info.param_name.clone(),
                    entity_type: param_info.entity_type.clone(),
                    cardinality: "batch".to_string(),
                    entities: Vec::new(),
                    is_complete: false,
                    filter_description: String::new(),
                },
            );
        }

        for param_info in &entity_deps.shared_params {
            key_sets.insert(
                param_info.param_name.clone(),
                TemplateParamKeySet {
                    param_name: param_info.param_name.clone(),
                    entity_type: param_info.entity_type.clone(),
                    cardinality: "shared".to_string(),
                    entities: Vec::new(),
                    is_complete: false,
                    filter_description: String::new(),
                },
            );
        }

        // Update UI session state
        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session.context.template_execution = TemplateExecutionContext {
                template_id: Some(template_id.to_string()),
                phase: TemplatePhase::CollectingSharedParams,
                key_sets,
                scalar_params: std::collections::HashMap::new(),
                current_batch_index: 0,
                batch_results: Vec::new(),
                auto_execute: false,
            };
            session.context.mode = SessionMode::TemplateExpansion;
        }

        // Return template info and params to collect
        Ok(json!({
            "success": true,
            "template_id": template_id,
            "template_name": template.metadata.name,
            "summary": template.metadata.summary,
            "phase": "collecting_shared_params",
            "params_to_collect": {
                "batch": entity_deps.batch_params.iter().map(|p| json!({
                    "param_name": p.param_name,
                    "entity_type": p.entity_type,
                    "prompt": p.prompt,
                    "role_hint": p.role_hint
                })).collect::<Vec<_>>(),
                "shared": entity_deps.shared_params.iter().map(|p| json!({
                    "param_name": p.param_name,
                    "entity_type": p.entity_type,
                    "prompt": p.prompt,
                    "role_hint": p.role_hint
                })).collect::<Vec<_>>()
            }
        }))
    }

    /// Add entities to a parameter's key set
    async fn batch_add_entities(&self, args: Value) -> Result<Value> {
        use crate::api::session::ResolvedEntityRef;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let param_name = args["param_name"]
            .as_str()
            .ok_or_else(|| anyhow!("param_name required"))?;

        let entities = args["entities"]
            .as_array()
            .ok_or_else(|| anyhow!("entities array required"))?;

        let filter_description = args["filter_description"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Parse entities
        let resolved_entities: Vec<ResolvedEntityRef> = entities
            .iter()
            .filter_map(|e| {
                let entity_id = e["entity_id"]
                    .as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())?;
                let display_name = e["display_name"].as_str()?.to_string();
                let entity_type = e["entity_type"].as_str()?.to_string();
                let metadata = e.get("metadata").cloned().unwrap_or(json!(null));

                Some(ResolvedEntityRef {
                    entity_type,
                    display_name,
                    entity_id,
                    metadata,
                })
            })
            .collect();

        if resolved_entities.is_empty() {
            return Err(anyhow!("No valid entities provided"));
        }

        let added_count = resolved_entities.len();

        // Update UI session
        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let key_set = session
                .context
                .template_execution
                .key_sets
                .get_mut(param_name)
                .ok_or_else(|| anyhow!("Key set not found for param: {}", param_name))?;

            key_set.entities.extend(resolved_entities.clone());
            if !filter_description.is_empty() {
                key_set.filter_description = filter_description;
            }
        }

        Ok(json!({
            "success": true,
            "param_name": param_name,
            "added_count": added_count,
            "entities": resolved_entities.iter().map(|e| json!({
                "entity_id": e.entity_id.to_string(),
                "display_name": e.display_name,
                "entity_type": e.entity_type
            })).collect::<Vec<_>>()
        }))
    }

    /// Mark a key set as complete
    async fn batch_confirm_keyset(&self, args: Value) -> Result<Value> {
        use crate::api::session::TemplatePhase;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let param_name = args["param_name"]
            .as_str()
            .ok_or_else(|| anyhow!("param_name required"))?;

        let (all_complete, phase) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let ctx = &mut session.context.template_execution;

            let key_set = ctx
                .key_sets
                .get_mut(param_name)
                .ok_or_else(|| anyhow!("Key set not found for param: {}", param_name))?;

            if key_set.entities.is_empty() {
                return Err(anyhow!("Cannot confirm empty key set: {}", param_name));
            }

            key_set.is_complete = true;

            // Check if all key sets are complete
            let all_complete = ctx.key_sets.values().all(|ks| ks.is_complete);

            // Auto-advance phase if all complete
            if all_complete {
                ctx.phase = TemplatePhase::ReviewingKeySets;
            }

            (all_complete, ctx.phase.clone())
        };

        Ok(json!({
            "success": true,
            "param_name": param_name,
            "all_key_sets_complete": all_complete,
            "phase": format!("{:?}", phase).to_lowercase()
        }))
    }

    /// Set a scalar parameter value
    async fn batch_set_scalar(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let param_name = args["param_name"]
            .as_str()
            .ok_or_else(|| anyhow!("param_name required"))?;

        let value = args["value"]
            .as_str()
            .ok_or_else(|| anyhow!("value required"))?;

        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session
                .context
                .template_execution
                .scalar_params
                .insert(param_name.to_string(), value.to_string());
        }

        Ok(json!({
            "success": true,
            "param_name": param_name,
            "value": value
        }))
    }

    /// Get current template execution state
    async fn batch_get_state(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let context = {
            let sessions_guard = sessions.read().await;
            let session = sessions_guard
                .get(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session.context.template_execution.clone()
        };

        Ok(json!({
            "template_id": context.template_id,
            "phase": format!("{:?}", context.phase).to_lowercase(),
            "key_sets": context.key_sets.iter().map(|(name, ks)| {
                json!({
                    "param_name": name,
                    "entity_type": ks.entity_type,
                    "cardinality": ks.cardinality,
                    "entity_count": ks.entities.len(),
                    "is_complete": ks.is_complete,
                    "entities": ks.entities.iter().map(|e| json!({
                        "entity_id": e.entity_id.to_string(),
                        "display_name": e.display_name
                    })).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>(),
            "scalar_params": context.scalar_params,
            "current_batch_index": context.current_batch_index,
            "batch_size": context.batch_size(),
            "progress": context.progress_string(),
            "batch_results": context.batch_results.iter().map(|r| json!({
                "index": r.index,
                "source_entity": r.source_entity.display_name,
                "success": r.success,
                "created_id": r.created_id.map(|id| id.to_string()),
                "error": r.error
            })).collect::<Vec<_>>(),
            "is_active": context.is_active()
        }))
    }

    /// Expand template for current batch item
    async fn batch_expand_current(&self, args: Value) -> Result<Value> {
        use crate::templates::{ExpansionContext, TemplateExpander, TemplateRegistry};
        use std::path::Path;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        // Get template context from UI session
        let context = {
            let sessions_guard = sessions.read().await;
            let session = sessions_guard
                .get(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session.context.template_execution.clone()
        };

        let template_id = context
            .template_id
            .as_ref()
            .ok_or_else(|| anyhow!("No template set"))?;

        // Load template
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("verbs/templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        let template = registry
            .get(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Build params from context
        let mut params = std::collections::HashMap::new();

        // Add current batch entity
        if let Some(batch_entity) = context.current_batch_entity() {
            // Find the batch param name
            if let Some((param_name, _)) = context
                .key_sets
                .iter()
                .find(|(_, ks)| ks.cardinality == "batch")
            {
                params.insert(param_name.clone(), batch_entity.entity_id.to_string());
                // Also add .name for display
                params.insert(
                    format!("{}.name", param_name),
                    batch_entity.display_name.clone(),
                );
            }
        }

        // Add shared entities
        for (param_name, entity) in context.shared_entities() {
            params.insert(param_name.to_string(), entity.entity_id.to_string());
            params.insert(format!("{}.name", param_name), entity.display_name.clone());
        }

        // Add scalar params
        for (name, value) in &context.scalar_params {
            params.insert(name.clone(), value.clone());
        }

        // Expand template
        let expansion_ctx = ExpansionContext::new();
        let result = TemplateExpander::expand(template, &params, &expansion_ctx);

        Ok(json!({
            "dsl": result.dsl,
            "complete": result.missing_params.is_empty(),
            "batch_index": context.current_batch_index,
            "batch_size": context.batch_size(),
            "current_entity": context.current_batch_entity().map(|e| json!({
                "entity_id": e.entity_id.to_string(),
                "display_name": e.display_name,
                "entity_type": e.entity_type
            })),
            "missing_params": result.missing_params.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
        }))
    }

    /// Record result from executing current batch item
    async fn batch_record_result(&self, args: Value) -> Result<Value> {
        use crate::api::session::{BatchItemResult, TemplatePhase};

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let success = args["success"]
            .as_bool()
            .ok_or_else(|| anyhow!("success required"))?;

        let created_id = args["created_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok());

        let error = args["error"].as_str().map(|s| s.to_string());

        let executed_dsl = args["executed_dsl"].as_str().map(|s| s.to_string());

        let (has_more, new_index, phase) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let ctx = &mut session.context.template_execution;
            let index = ctx.current_batch_index;

            // Get the current batch entity
            let source_entity = ctx
                .current_batch_entity()
                .cloned()
                .ok_or_else(|| anyhow!("No current batch entity"))?;

            // Record result
            ctx.batch_results.push(BatchItemResult {
                index,
                source_entity,
                success,
                created_id,
                error: error.clone(),
                executed_dsl,
            });

            // Advance to next item
            let has_more = ctx.advance();

            // Update phase if complete
            if !has_more {
                ctx.phase = TemplatePhase::Complete;
            }

            (has_more, ctx.current_batch_index, ctx.phase.clone())
        };

        Ok(json!({
            "success": true,
            "recorded_success": success,
            "has_more_items": has_more,
            "next_index": new_index,
            "phase": format!("{:?}", phase).to_lowercase(),
            "created_id": created_id.map(|id| id.to_string()),
            "error": error
        }))
    }

    /// Skip current batch item
    async fn batch_skip_current(&self, args: Value) -> Result<Value> {
        use crate::api::session::{BatchItemResult, TemplatePhase};

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let reason = args["reason"].as_str().unwrap_or("User skipped");

        let (has_more, new_index, phase) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let ctx = &mut session.context.template_execution;
            let index = ctx.current_batch_index;

            // Get the current batch entity
            let source_entity = ctx
                .current_batch_entity()
                .cloned()
                .ok_or_else(|| anyhow!("No current batch entity"))?;

            // Record skip as failed result
            ctx.batch_results.push(BatchItemResult {
                index,
                source_entity,
                success: false,
                created_id: None,
                error: Some(format!("Skipped: {}", reason)),
                executed_dsl: None,
            });

            // Advance to next item
            let has_more = ctx.advance();

            // Update phase if complete
            if !has_more {
                ctx.phase = TemplatePhase::Complete;
            }

            (has_more, ctx.current_batch_index, ctx.phase.clone())
        };

        Ok(json!({
            "success": true,
            "skipped": true,
            "has_more_items": has_more,
            "next_index": new_index,
            "phase": format!("{:?}", phase).to_lowercase()
        }))
    }

    /// Cancel batch operation
    async fn batch_cancel(&self, args: Value) -> Result<Value> {
        use crate::api::session::SessionMode;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let (completed_count, failed_count, pending_count) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let ctx = &session.context.template_execution;
            let completed = ctx.batch_results.iter().filter(|r| r.success).count();
            let failed = ctx.batch_results.iter().filter(|r| !r.success).count();
            let total = ctx.batch_size();
            let pending = total.saturating_sub(completed + failed);

            // Reset template execution state
            session.context.template_execution.reset();
            session.context.mode = SessionMode::Chat;

            (completed, failed, pending)
        };

        Ok(json!({
            "success": true,
            "cancelled": true,
            "completed_count": completed_count,
            "skipped_count": failed_count,
            "abandoned_count": pending_count
        }))
    }

    // ========================================================================
    // Research Macro Handlers
    // ========================================================================

    /// List available research macros with optional filtering
    async fn research_list(&self, args: Value) -> Result<Value> {
        use crate::research::{ResearchMacroRegistry, ReviewRequirement};

        let search = args["search"].as_str();
        let tag = args["tag"].as_str();

        // Load registry from config directory
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let macros_dir = std::path::Path::new(&config_dir).join("macros/research");

        let registry = ResearchMacroRegistry::load_from_dir(&macros_dir)
            .map_err(|e| anyhow!("Failed to load research macros: {}", e))?;

        let macros: Vec<Value> = registry
            .list(search)
            .iter()
            .filter(|m| {
                // Apply tag filter
                if let Some(tag_filter) = tag {
                    if !m.tags.iter().any(|t| t.eq_ignore_ascii_case(tag_filter)) {
                        return false;
                    }
                }
                true
            })
            .map(|m| {
                json!({
                    "name": m.name,
                    "description": m.description,
                    "tags": m.tags,
                    "review_required": matches!(m.output.review, ReviewRequirement::Required),
                    "param_count": m.parameters.len()
                })
            })
            .collect();

        Ok(json!({
            "macros": macros,
            "count": macros.len()
        }))
    }

    /// Get full details of a specific research macro
    async fn research_get(&self, args: Value) -> Result<Value> {
        use crate::research::ResearchMacroRegistry;

        let macro_name = args["macro_name"]
            .as_str()
            .ok_or_else(|| anyhow!("macro_name required"))?;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let macros_dir = std::path::Path::new(&config_dir).join("macros/research");

        let registry = ResearchMacroRegistry::load_from_dir(&macros_dir)
            .map_err(|e| anyhow!("Failed to load research macros: {}", e))?;

        let macro_def = registry
            .get(macro_name)
            .ok_or_else(|| anyhow!("Research macro not found: {}", macro_name))?;

        // Build parameter descriptions
        let params: Vec<Value> = macro_def
            .parameters
            .iter()
            .map(|p| {
                json!({
                    "name": p.name,
                    "param_type": &p.param_type,
                    "required": p.required,
                    "description": p.description,
                    "default": p.default,
                    "enum_values": p.enum_values
                })
            })
            .collect();

        Ok(json!({
            "name": macro_def.name,
            "description": macro_def.description,
            "params": params,
            "output_schema": macro_def.output.schema,
            "review_requirement": format!("{:?}", macro_def.output.review),
            "suggested_verbs_template": macro_def.suggested_verbs,
            "tags": macro_def.tags
        }))
    }

    /// Execute a research macro with LLM + web search
    async fn research_execute(&self, args: Value) -> Result<Value> {
        use crate::research::{ClaudeResearchClient, ResearchExecutor, ResearchMacroRegistry};

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let macro_name = args["macro_name"]
            .as_str()
            .ok_or_else(|| anyhow!("macro_name required"))?;

        let params = args["params"].as_object().cloned().unwrap_or_default();

        // Load registry
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let macros_dir = std::path::Path::new(&config_dir).join("macros/research");

        let registry = ResearchMacroRegistry::load_from_dir(&macros_dir)
            .map_err(|e| anyhow!("Failed to load research macros: {}", e))?;

        // Convert params to HashMap
        let params_map: std::collections::HashMap<String, serde_json::Value> =
            params.into_iter().collect();

        // Create LLM client and executor
        let llm_client = ClaudeResearchClient::from_env()
            .map_err(|e| anyhow!("Failed to create LLM client: {}", e))?;
        let executor = ResearchExecutor::new(registry, llm_client);
        let result = executor
            .execute(macro_name, params_map)
            .await
            .map_err(|e| anyhow!("Research execution failed: {}", e))?;

        // Store result in session for review workflow using ResearchContext API
        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            // Use the ResearchContext.set_pending() method
            session.context.research.set_pending(result.clone());
        }

        Ok(json!({
            "success": true,
            "result_id": result.result_id,
            "macro_name": result.macro_name,
            "data": result.data,
            "schema_valid": result.schema_valid,
            "validation_errors": result.validation_errors,
            "review_required": result.review_required,
            "suggested_verbs": result.suggested_verbs,
            "search_quality": result.search_quality
        }))
    }

    /// Approve research results and get generated DSL verbs
    async fn research_approve(&self, args: Value) -> Result<Value> {
        use crate::session::ResearchState;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        // Optional edits to the research data before approval
        let edits: Option<Value> = args.get("edits").cloned();

        let (verbs, macro_name, result_id) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let research = &mut session.context.research;

            // Verify we're in the right state
            if research.state != ResearchState::PendingReview {
                return Err(anyhow!(
                    "Cannot approve: research is not pending review (state: {})",
                    research.state
                ));
            }

            // Get macro name before approval
            let macro_name = research
                .pending_macro_name()
                .unwrap_or("unknown")
                .to_string();
            let result_id = research.pending.as_ref().map(|r| r.result_id);

            // Use the ResearchContext.approve() method
            let approved = research
                .approve(edits)
                .map_err(|e| anyhow!("Approval failed: {}", e))?;

            let verbs = Some(approved.generated_verbs.clone());

            (verbs, macro_name, result_id)
        };

        Ok(json!({
            "success": true,
            "approved": true,
            "result_id": result_id,
            "macro_name": macro_name,
            "suggested_verbs": verbs,
            "message": "Research approved. Use the suggested_verbs DSL to create entities."
        }))
    }

    /// Reject research results
    async fn research_reject(&self, args: Value) -> Result<Value> {
        use crate::session::ResearchState;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let reason = args["reason"].as_str().unwrap_or("No reason provided");

        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let research = &mut session.context.research;

            // Verify we're in the right state
            if research.state != ResearchState::PendingReview {
                return Err(anyhow!(
                    "Cannot reject: research is not pending review (state: {})",
                    research.state
                ));
            }

            // Use the ResearchContext.reject() method
            research.reject();
        }

        Ok(json!({
            "success": true,
            "rejected": true,
            "reason": reason,
            "message": "Research rejected. You can re-execute with different parameters."
        }))
    }

    /// Get current research status for a session
    async fn research_status(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let status = {
            let sessions_guard = sessions.read().await;
            let session = sessions_guard
                .get(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let research = &session.context.research;

            json!({
                "state": research.state.to_string(),
                "current_macro": research.pending_macro_name(),
                "has_pending_result": research.has_pending(),
                "has_pending_verbs": research.has_verbs_ready(),
                "approved_count": research.approved_count(),
                "recent_approvals": research.approved.values()
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                    .take(5)
                    .map(|a| {
                        json!({
                            "result_id": a.result_id,
                            "approved_at": a.approved_at.to_rfc3339(),
                            "edits_made": a.edits_made
                        })
                    })
                    .collect::<Vec<_>>()
            })
        };

        Ok(json!({
            "success": true,
            "status": status
        }))
    }

    // =========================================================================
    // Taxonomy Navigation Tools
    // =========================================================================

    /// Get the entity type taxonomy tree
    async fn taxonomy_get(&self, args: Value) -> Result<Value> {
        use crate::taxonomy::{TaxonomyNode, TaxonomyService};

        let pool = self.require_pool()?;
        let include_counts = args["include_counts"].as_bool().unwrap_or(true);

        // Build the taxonomy tree from database
        let service = TaxonomyService::new(pool.clone());
        let tree = service.build_taxonomy_tree(include_counts).await?;

        // Convert to JSON representation
        fn node_to_json(node: &TaxonomyNode) -> serde_json::Value {
            json!({
                "node_id": node.id.to_string(),
                "label": node.label,
                "short_label": node.short_label,
                "node_type": format!("{:?}", node.node_type),
                "entity_count": node.descendant_count,
                "children": node.children.iter().map(node_to_json).collect::<Vec<_>>()
            })
        }

        Ok(json!({
            "success": true,
            "taxonomy": node_to_json(&tree)
        }))
    }

    /// Drill into a taxonomy node
    async fn taxonomy_drill_in(&self, args: Value) -> Result<Value> {
        use crate::taxonomy::{TaxonomyFrame, TaxonomyService};

        let sessions = self.require_sessions()?;
        let pool = self.require_pool()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let node_label = args["node_label"]
            .as_str()
            .ok_or_else(|| anyhow!("node_label required"))?
            .to_uppercase();

        // Get the taxonomy subtree for this node
        let service = TaxonomyService::new(pool.clone());
        let subtree = service.get_subtree(&node_label).await?;

        // Create a new frame for this level using the constructor
        let frame = TaxonomyFrame::from_zoom(
            subtree.id,
            node_label.clone(),
            subtree.clone(),
            None, // No parser needed for type taxonomy
        );

        // Push onto the session's taxonomy stack
        let breadcrumbs = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let _ = session.context.taxonomy_stack.push(frame);
            session.context.taxonomy_stack.breadcrumbs()
        };

        // Return the children at this level
        let children: Vec<serde_json::Value> = subtree
            .children
            .iter()
            .map(|child| {
                json!({
                    "node_id": child.id.to_string(),
                    "label": child.label,
                    "short_label": child.short_label,
                    "node_type": format!("{:?}", child.node_type),
                    "entity_count": child.descendant_count,
                    "has_children": !child.children.is_empty()
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "current_node": node_label,
            "children": children,
            "breadcrumbs": breadcrumbs
        }))
    }

    /// Zoom out one level in taxonomy
    async fn taxonomy_zoom_out(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let (success, breadcrumbs, current_label) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            if !session.context.taxonomy_stack.can_zoom_out() {
                return Ok(json!({
                    "success": false,
                    "error": "Already at root level"
                }));
            }

            session.context.taxonomy_stack.pop();
            let breadcrumbs = session.context.taxonomy_stack.breadcrumbs();
            let current_label = session
                .context
                .taxonomy_stack
                .current()
                .map(|f| f.label.clone())
                .unwrap_or_else(|| "ROOT".to_string());

            (true, breadcrumbs, current_label)
        };

        Ok(json!({
            "success": success,
            "current_node": current_label,
            "breadcrumbs": breadcrumbs
        }))
    }

    /// Reset taxonomy to root level
    async fn taxonomy_reset(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session.context.taxonomy_stack.clear();
        }

        Ok(json!({
            "success": true,
            "message": "Taxonomy reset to root level"
        }))
    }

    /// Get current taxonomy position
    async fn taxonomy_position(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let sessions_guard = sessions.read().await;
        let session = sessions_guard
            .get(&session_uuid)
            .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

        let stack = &session.context.taxonomy_stack;

        if stack.is_empty() {
            return Ok(json!({
                "success": true,
                "at_root": true,
                "breadcrumbs": [],
                "depth": 0,
                "can_zoom_out": false,
                "can_drill_in": true
            }));
        }

        let current_frame = stack.current();
        let current_node = current_frame.map(|f| {
            json!({
                "label": f.label,
                "child_count": f.tree.children.len()
            })
        });

        Ok(json!({
            "success": true,
            "at_root": false,
            "breadcrumbs": stack.breadcrumbs(),
            "depth": stack.depth(),
            "current_node": current_node,
            "can_zoom_out": stack.can_zoom_out(),
            "can_drill_in": stack.can_zoom_in()
        }))
    }

    /// List entities of the currently focused type
    async fn taxonomy_entities(&self, args: Value) -> Result<Value> {
        use crate::taxonomy::TaxonomyService;

        let sessions = self.require_sessions()?;
        let pool = self.require_pool()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let search = args["search"].as_str().map(|s| s.to_string());
        let limit = args["limit"].as_i64().unwrap_or(20);
        let offset = args["offset"].as_i64().unwrap_or(0);

        // Get current entity type from taxonomy stack
        let entity_type = {
            let sessions_guard = sessions.read().await;
            let session = sessions_guard
                .get(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session
                .context
                .taxonomy_stack
                .current()
                .map(|f| f.label.clone())
                .ok_or_else(|| anyhow!("No taxonomy node selected. Use taxonomy_drill_in first."))?
        };

        // Query entities of this type
        let service = TaxonomyService::new(pool.clone());
        let entities = service
            .list_entities_by_type(&entity_type, search.as_deref(), limit, offset)
            .await?;

        let entity_list: Vec<serde_json::Value> = entities
            .iter()
            .map(|e| {
                json!({
                    "entity_id": e.entity_id,
                    "name": e.name,
                    "entity_type": e.entity_type,
                    "created_at": e.created_at.map(|t| t.to_rfc3339())
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "entity_type": entity_type,
            "entities": entity_list,
            "count": entity_list.len(),
            "limit": limit,
            "offset": offset
        }))
    }

    // =========================================================================
    // Trading Matrix Tools
    // =========================================================================

    /// Get the trading matrix tree for a CBU
    ///
    /// Returns the hierarchical trading configuration:
    /// - Trading Universe (instrument classes → markets → currencies)
    /// - Standing Settlement Instructions (SSIs with booking rules)
    /// - Settlement Chains (multi-hop paths)
    /// - Tax Configuration (jurisdictions and statuses)
    /// - ISDA/CSA Agreements (OTC counterparties)
    async fn trading_matrix_get(&self, args: Value) -> Result<Value> {
        let cbu_id_str = args["cbu_id"]
            .as_str()
            .ok_or_else(|| anyhow!("cbu_id required"))?;
        let cbu_id = Uuid::parse_str(cbu_id_str).map_err(|_| anyhow!("Invalid cbu_id"))?;

        // Check if CBU exists
        let cbu = sqlx::query!(
            r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?;

        // Get trading profile status
        let profile = sqlx::query!(
            r#"
            SELECT profile_id, status, version, created_at, updated_at
            FROM "ob-poc".cbu_trading_profiles
            WHERE cbu_id = $1 AND status = 'ACTIVE'
            ORDER BY version DESC
            LIMIT 1
            "#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        // Count universe entries
        let universe_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.cbu_instrument_universe WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Count SSIs
        let ssi_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.cbu_ssi WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Count booking rules
        let rule_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.ssi_booking_rules WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Count settlement chains
        let chain_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.cbu_settlement_chains WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Count ISDA agreements
        let isda_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.isda_agreements WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Get instrument classes in universe
        let instrument_classes: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT ic.code
            FROM custody.cbu_instrument_universe u
            JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
            WHERE u.cbu_id = $1
            ORDER BY ic.code
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .filter_map(|s| s)
        .collect();

        // Get markets in universe
        let markets: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT m.mic
            FROM custody.cbu_instrument_universe u
            JOIN custody.markets m ON m.market_id = u.market_id
            WHERE u.cbu_id = $1 AND u.market_id IS NOT NULL
            ORDER BY m.mic
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .filter_map(|s| s)
        .collect();

        // Build summary
        let has_profile = profile.is_some();
        let profile_info = profile.map(|p| {
            json!({
                "profile_id": p.profile_id.to_string(),
                "status": p.status,
                "version": p.version,
                "updated_at": p.updated_at.map(|t| t.to_rfc3339())
            })
        });

        // Determine completeness
        let is_complete = universe_count > 0 && ssi_count > 0 && rule_count > 0;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id.to_string(),
            "cbu_name": cbu.name,
            "has_trading_profile": has_profile,
            "trading_profile": profile_info,
            "summary": {
                "universe_entries": universe_count,
                "instrument_classes": instrument_classes,
                "markets": markets,
                "ssis": ssi_count,
                "booking_rules": rule_count,
                "settlement_chains": chain_count,
                "isda_agreements": isda_count,
                "is_complete": is_complete
            },
            "endpoints": {
                "full_tree": format!("/api/cbu/{}/trading-matrix", cbu_id),
                "trading_profile_verbs": "Use verbs_list with domain='trading-profile' to see available operations"
            }
        }))
    }

    // =========================================================================
    // FEEDBACK INSPECTOR HANDLERS
    // =========================================================================

    async fn feedback_analyze(&self, args: Value) -> Result<Value> {
        use crate::feedback::{AnalysisReport, FeedbackInspector};

        #[derive(serde::Deserialize)]
        struct Args {
            since_hours: Option<i64>,
        }

        let args: Args = serde_json::from_value(args)?;
        let since_hours = args.since_hours.unwrap_or(24);

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(
            pool.clone(),
            Some(std::path::PathBuf::from("/tmp/ob-poc-events.jsonl")),
        );

        let since = chrono::Utc::now() - chrono::Duration::hours(since_hours);
        let report = inspector.analyze(Some(since)).await?;

        Ok(json!({
            "total_failures": report.events_processed,
            "unique_issues": report.failures_created,
            "updated_issues": report.failures_updated,
            "by_error_type": report.by_error_type,
            "by_remediation_path": report.by_remediation_path,
            "analyzed_at": report.analyzed_at.to_rfc3339()
        }))
    }

    async fn feedback_list(&self, args: Value) -> Result<Value> {
        use crate::feedback::{ErrorType, FeedbackInspector, IssueFilter, IssueStatus};

        #[derive(serde::Deserialize)]
        struct Args {
            status: Option<String>,
            error_type: Option<String>,
            verb: Option<String>,
            source: Option<String>,
            limit: Option<i64>,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);

        let filter = IssueFilter {
            status: args.status.and_then(|s| parse_issue_status(&s)),
            error_type: args.error_type.and_then(|s| parse_error_type(&s)),
            verb: args.verb,
            source: args.source,
            limit: args.limit,
            ..Default::default()
        };

        let issues = inspector.list_issues(filter).await?;

        Ok(json!({
            "count": issues.len(),
            "issues": issues.iter().map(|i| json!({
                "fingerprint": i.fingerprint,
                "error_type": format!("{:?}", i.error_type),
                "status": format!("{:?}", i.status),
                "verb": i.verb,
                "source": i.source,
                "message": i.error_message,
                "occurrence_count": i.occurrence_count,
                "first_seen": i.first_seen_at.to_rfc3339(),
                "last_seen": i.last_seen_at.to_rfc3339(),
                "repro_verified": i.repro_verified
            })).collect::<Vec<_>>()
        }))
    }

    async fn feedback_get(&self, args: Value) -> Result<Value> {
        use crate::feedback::FeedbackInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);

        let issue = inspector.get_issue(&args.fingerprint).await?;

        match issue {
            Some(detail) => Ok(json!({
                "found": true,
                "failure": {
                    "id": detail.failure.id,
                    "fingerprint": detail.failure.fingerprint,
                    "error_type": format!("{:?}", detail.failure.error_type),
                    "remediation_path": format!("{:?}", detail.failure.remediation_path),
                    "status": format!("{:?}", detail.failure.status),
                    "verb": detail.failure.verb,
                    "source": detail.failure.source,
                    "message": detail.failure.error_message,
                    "context": detail.failure.error_context,
                    "user_intent": detail.failure.user_intent,
                    "command_sequence": detail.failure.command_sequence,
                    "repro_type": detail.failure.repro_type,
                    "repro_path": detail.failure.repro_path,
                    "repro_verified": detail.failure.repro_verified,
                    "fix_commit": detail.failure.fix_commit,
                    "fix_notes": detail.failure.fix_notes,
                    "occurrence_count": detail.failure.occurrence_count,
                    "first_seen": detail.failure.first_seen_at.to_rfc3339(),
                    "last_seen": detail.failure.last_seen_at.to_rfc3339()
                },
                "occurrences": detail.occurrences.iter().take(10).map(|o| json!({
                    "id": o.id,
                    "event_timestamp": o.event_timestamp.to_rfc3339(),
                    "session_id": o.session_id,
                    "verb": o.verb,
                    "duration_ms": o.duration_ms,
                    "message": o.error_message
                })).collect::<Vec<_>>(),
                "audit_trail": detail.audit_trail.iter().map(|a| json!({
                    "action": format!("{:?}", a.action),
                    "actor_type": format!("{:?}", a.actor_type),
                    "actor_id": a.actor_id,
                    "details": a.details,
                    "created_at": a.created_at.to_rfc3339()
                })).collect::<Vec<_>>()
            })),
            None => Ok(json!({
                "found": false,
                "fingerprint": args.fingerprint
            })),
        }
    }

    async fn feedback_repro(&self, args: Value) -> Result<Value> {
        use crate::feedback::{FeedbackInspector, ReproGenerator};

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);
        let repro_gen = ReproGenerator::new();

        let result = repro_gen
            .generate_and_verify(&inspector, &args.fingerprint)
            .await?;

        Ok(json!({
            "fingerprint": args.fingerprint,
            "repro_type": format!("{:?}", result.repro_type),
            "path": result.path.to_string_lossy(),
            "verified": result.verified,
            "passes": result.passes,
            "output": result.output
        }))
    }

    async fn feedback_todo(&self, args: Value) -> Result<Value> {
        use crate::feedback::{FeedbackInspector, TodoGenerator};

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
            todo_number: i32,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);
        let todo_gen = TodoGenerator::new();

        let result = todo_gen
            .generate_todo(&inspector, &args.fingerprint, args.todo_number)
            .await?;

        Ok(json!({
            "fingerprint": args.fingerprint,
            "todo_number": result.todo_number,
            "path": result.todo_path.to_string_lossy(),
            "content": result.content
        }))
    }

    async fn feedback_audit(&self, args: Value) -> Result<Value> {
        use crate::feedback::FeedbackInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);

        let trail = inspector.get_audit_trail(&args.fingerprint).await?;

        Ok(json!({
            "fingerprint": args.fingerprint,
            "count": trail.len(),
            "entries": trail.iter().map(|a| json!({
                "id": a.id,
                "action": format!("{:?}", a.action),
                "actor_type": format!("{:?}", a.actor_type),
                "actor_id": a.actor_id,
                "details": a.details,
                "evidence": a.evidence,
                "previous_status": a.previous_status.map(|s| format!("{:?}", s)),
                "new_status": a.new_status.map(|s| format!("{:?}", s)),
                "created_at": a.created_at.to_rfc3339()
            })).collect::<Vec<_>>()
        }))
    }
}

// Helper functions for parsing enum strings
fn parse_issue_status(s: &str) -> Option<crate::feedback::IssueStatus> {
    use crate::feedback::IssueStatus;
    match s.to_uppercase().as_str() {
        "NEW" => Some(IssueStatus::New),
        "REPRO_GENERATED" => Some(IssueStatus::ReproGenerated),
        "REPRO_VERIFIED" => Some(IssueStatus::ReproVerified),
        "TODO_CREATED" => Some(IssueStatus::TodoCreated),
        "IN_PROGRESS" => Some(IssueStatus::InProgress),
        "FIX_COMMITTED" => Some(IssueStatus::FixCommitted),
        "RESOLVED" => Some(IssueStatus::Resolved),
        "WONT_FIX" => Some(IssueStatus::WontFix),
        _ => None,
    }
}

fn parse_error_type(s: &str) -> Option<crate::feedback::ErrorType> {
    use crate::feedback::ErrorType;
    match s.to_uppercase().as_str() {
        "TIMEOUT" => Some(ErrorType::Timeout),
        "RATE_LIMITED" => Some(ErrorType::RateLimited),
        "ENUM_DRIFT" => Some(ErrorType::EnumDrift),
        "SCHEMA_DRIFT" => Some(ErrorType::SchemaDrift),
        "HANDLER_PANIC" => Some(ErrorType::HandlerPanic),
        "HANDLER_ERROR" => Some(ErrorType::HandlerError),
        "PARSE_ERROR" => Some(ErrorType::ParseError),
        "DSL_PARSE_ERROR" => Some(ErrorType::DslParseError),
        "VALIDATION_FAILED" => Some(ErrorType::ValidationFailed),
        _ => None,
    }
}
