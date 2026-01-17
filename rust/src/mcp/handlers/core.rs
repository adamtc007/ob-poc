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

use crate::agent::learning::warmup::SharedLearnedData;
use crate::api::cbu_session_routes::CbuSessionStore;
use crate::api::session::SessionStore;
use crate::database::generation_log_repository::{
    CompileResult, GenerationAttempt, GenerationLogRepository, LintResult, ParseResult,
};
use crate::database::VisualizationRepository;
use crate::dsl_v2::{
    compile, gateway_resolver, parse_program, registry, DslExecutor, ExecutionContext,
};
use crate::mcp::verb_search::HybridVerbSearcher;

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
    /// CBU session store for load/unload operations
    cbu_sessions: Option<CbuSessionStore>,
    /// Hybrid verb searcher (lazy-initialized)
    verb_searcher: Arc<Mutex<Option<HybridVerbSearcher>>>,
    /// Learned data from agent learning system (shared reference)
    learned_data: Option<SharedLearnedData>,
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
            cbu_sessions: None,
            verb_searcher: Arc::new(Mutex::new(None)),
            learned_data: None,
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
            cbu_sessions: None,
            verb_searcher: Arc::new(Mutex::new(None)),
            learned_data: None,
        }
    }

    /// Create handlers with both session stores (full integrated mode)
    pub fn with_all_sessions(
        pool: PgPool,
        sessions: SessionStore,
        cbu_sessions: CbuSessionStore,
    ) -> Self {
        Self {
            generation_log: GenerationLogRepository::new(pool.clone()),
            repo: VisualizationRepository::new(pool.clone()),
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
            sessions: Some(sessions),
            cbu_sessions: Some(cbu_sessions),
            verb_searcher: Arc::new(Mutex::new(None)),
            learned_data: None,
        }
    }

    /// Create handlers with learned data for semantic intent pipeline (standalone MCP mode)
    pub fn with_learned_data(pool: PgPool, learned_data: SharedLearnedData) -> Self {
        Self {
            generation_log: GenerationLogRepository::new(pool.clone()),
            repo: VisualizationRepository::new(pool.clone()),
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
            sessions: None,
            cbu_sessions: None,
            verb_searcher: Arc::new(Mutex::new(None)),
            learned_data: Some(learned_data),
        }
    }

    /// Create handlers with all session stores and learned data (full integrated mode)
    pub fn with_all_sessions_and_learned_data(
        pool: PgPool,
        sessions: SessionStore,
        cbu_sessions: CbuSessionStore,
        learned_data: SharedLearnedData,
    ) -> Self {
        Self {
            generation_log: GenerationLogRepository::new(pool.clone()),
            repo: VisualizationRepository::new(pool.clone()),
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
            sessions: Some(sessions),
            cbu_sessions: Some(cbu_sessions),
            verb_searcher: Arc::new(Mutex::new(None)),
            learned_data: Some(learned_data),
        }
    }

    /// Get the session store, or error if not configured
    fn require_sessions(&self) -> Result<&SessionStore> {
        self.sessions.as_ref().ok_or_else(|| {
            anyhow!("Session store not configured. Batch operations require integrated mode.")
        })
    }

    /// Get the CBU session store, or error if not configured
    fn require_cbu_sessions(&self) -> Result<&CbuSessionStore> {
        self.cbu_sessions.as_ref().ok_or_else(|| {
            anyhow!("CBU session store not configured. CBU operations require integrated mode.")
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

    /// Get or create HybridVerbSearcher for verb discovery
    ///
    /// Lazy-initializes on first use. Includes:
    /// - Phrase index from YAML invocation_phrases
    /// - Semantic matcher (if database available)
    /// - Learned data (if provided at construction)
    async fn get_verb_searcher(&self) -> Result<HybridVerbSearcher> {
        let mut guard = self.verb_searcher.lock().await;
        if let Some(searcher) = guard.as_ref() {
            return Ok(searcher.clone());
        }

        // Determine verbs directory
        let verbs_dir = std::env::var("DSL_CONFIG_DIR")
            .map(|d| format!("{}/verbs", d))
            .unwrap_or_else(|_| "config/verbs".to_string());

        // Create searcher based on available resources
        let searcher = if let Some(learned) = &self.learned_data {
            // Full mode with learned data
            HybridVerbSearcher::full(&verbs_dir, self.pool.clone(), Some(learned.clone()))
                .await
                .map_err(|e| anyhow!("Failed to create verb searcher: {}", e))?
        } else {
            // Phrase-only mode (no learned data yet)
            HybridVerbSearcher::phrase_only(&verbs_dir)
                .map_err(|e| anyhow!("Failed to create verb searcher: {}", e))?
        };

        *guard = Some(searcher.clone());
        Ok(searcher)
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
            tenant_id: None,
            cbu_id: None,
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
            "verb_search" => self.verb_search(args).await,
            "intent_feedback" => self.intent_feedback(args).await,
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
            // Session v2 tools - simplified CBU session management
            "session_load_cbu" => self.session_load_cbu(args).await,
            "session_load_jurisdiction" => self.session_load_jurisdiction(args).await,
            "session_load_galaxy" => self.session_load_galaxy(args).await,
            "session_unload_cbu" => self.session_unload_cbu(args).await,
            "session_clear" => self.session_clear(args).await,
            "session_undo" => self.session_undo(args).await,
            "session_redo" => self.session_redo(args).await,
            "session_info" => self.session_info(args).await,
            "session_list" => self.session_list(args).await,
            "entity_search" => self.entity_search(args).await,
            // Resolution sub-session tools
            "resolution_start" => self.resolution_start(args).await,
            "resolution_search" => self.resolution_search(args).await,
            "resolution_select" => self.resolution_select(args).await,
            "resolution_complete" => self.resolution_complete(args).await,
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
            // Agent learning tools (Loop 2 - continuous improvement)
            "intent_analyze" => self.intent_analyze(args).await,
            "intent_list" => self.intent_list(args).await,
            "intent_approve" => self.intent_approve(args).await,
            "intent_reject" => self.intent_reject(args).await,
            "intent_reload" => self.intent_reload(args).await,
            // Service resource pipeline tools
            "service_intent_create" => self.service_intent_create(args).await,
            "service_intent_list" => self.service_intent_list(args).await,
            "service_discovery_run" => self.service_discovery_run(args).await,
            "service_attributes_gaps" => self.service_attributes_gaps(args).await,
            "service_attributes_set" => self.service_attributes_set(args).await,
            "service_readiness_get" => self.service_readiness_get(args).await,
            "service_readiness_recompute" => self.service_readiness_recompute(args).await,
            "service_pipeline_run" => self.service_pipeline_run(args).await,
            "srdef_list" => self.srdef_list(args).await,
            "srdef_get" => self.srdef_get(args).await,
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
                                Some(crate::session::SessionScope::from_graph_scope(sc.clone()));
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

    /// Search for verbs matching natural language intent
    ///
    /// Uses hybrid search: learned phrases → YAML phrases → semantic embeddings
    async fn verb_search(&self, args: Value) -> Result<Value> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow!("query required"))?;
        let domain = args["domain"].as_str();
        let limit = args["limit"].as_i64().unwrap_or(5) as usize;

        // Get or create verb searcher
        let searcher = self.get_verb_searcher().await?;

        // Perform hybrid search
        let results = searcher.search(query, domain, limit).await?;

        Ok(json!({
            "query": query,
            "domain_filter": domain,
            "results": results.iter().map(|r| json!({
                "verb": r.verb,
                "score": r.score,
                "source": r.source,
                "matched_phrase": r.matched_phrase,
                "description": r.description
            })).collect::<Vec<_>>(),
            "count": results.len()
        }))
    }

    /// Generate DSL from natural language using structured intent extraction
    ///
    /// Pipeline:
    /// 1. verb_search finds matching verbs (learned → phrase → semantic)
    /// 2. LLM extracts argument values as JSON (NEVER writes DSL syntax)
    /// 3. DSL assembled deterministically from structured intent
    /// 4. Validated before return
    async fn dsl_generate(&self, args: Value) -> Result<Value> {
        use crate::mcp::intent_pipeline::IntentPipeline;

        let instruction = args["instruction"]
            .as_str()
            .ok_or_else(|| anyhow!("instruction required"))?;
        let domain = args["domain"].as_str();
        let execute = args["execute"].as_bool().unwrap_or(false);

        // Get verb searcher and create intent pipeline
        let searcher = self.get_verb_searcher().await?;
        let pipeline = IntentPipeline::new(searcher);

        // Process through structured pipeline
        let result = pipeline.process(instruction, domain).await?;

        // Build response
        let response = json!({
            "success": result.valid,
            "intent": {
                "verb": result.intent.verb,
                "arguments": result.intent.arguments.iter().map(|a| json!({
                    "name": a.name,
                    "value": a.value,
                    "resolved": a.resolved
                })).collect::<Vec<_>>(),
                "confidence": result.intent.confidence,
                "notes": result.intent.notes
            },
            "verb_candidates": result.verb_candidates.iter().map(|v| json!({
                "verb": v.verb,
                "score": v.score,
                "source": v.source,
                "matched_phrase": v.matched_phrase
            })).collect::<Vec<_>>(),
            "dsl": result.dsl,
            "valid": result.valid,
            "validation_error": result.validation_error,
            "unresolved_refs": result.unresolved_refs.iter().map(|r| json!({
                "param_name": r.param_name,
                "search_value": r.search_value,
                "entity_type": r.entity_type
            })).collect::<Vec<_>>()
        });

        // Execute if requested and valid
        if execute && result.valid {
            let exec_result = self
                .dsl_execute(json!({
                    "source": result.dsl,
                    "intent": instruction
                }))
                .await?;

            return Ok(json!({
                "success": true,
                "intent": response["intent"],
                "verb_candidates": response["verb_candidates"],
                "dsl": result.dsl,
                "valid": result.valid,
                "execution": exec_result
            }));
        }

        Ok(response)
    }

    /// Record user correction for learning loop
    ///
    /// Uses existing agent.* schema for learning candidates.
    /// Low-risk corrections (entity aliases) apply immediately.
    /// Medium-risk corrections (phrase mappings) apply after threshold.
    async fn intent_feedback(&self, args: Value) -> Result<Value> {
        let feedback_type = args["feedback_type"]
            .as_str()
            .ok_or_else(|| anyhow!("feedback_type required"))?;
        let original_input = args["original_input"]
            .as_str()
            .ok_or_else(|| anyhow!("original_input required"))?;
        let _system_choice = args["system_choice"].as_str();
        let correct_choice = args["correct_choice"]
            .as_str()
            .ok_or_else(|| anyhow!("correct_choice required"))?;
        let _user_explanation = args["user_explanation"].as_str();

        // Map feedback_type to learning parameters
        let (learning_type, risk_level, auto_applicable) = match feedback_type {
            "verb_correction" => ("invocation_phrase", "medium", false),
            "entity_correction" => ("entity_alias", "low", true),
            "phrase_mapping" => ("invocation_phrase", "medium", false),
            _ => return Err(anyhow!("Unknown feedback_type: {}", feedback_type)),
        };

        let pool = self.require_pool()?;

        // Create fingerprint for deduplication
        let fingerprint = format!(
            "{}:{}:{}",
            learning_type,
            original_input.to_lowercase().trim(),
            correct_choice.trim()
        );

        // Insert or increment learning candidate
        let row = sqlx::query_as::<_, (i64, i32, bool)>(
            r#"
            INSERT INTO agent.learning_candidates (
                fingerprint, learning_type, input_pattern, suggested_output,
                risk_level, auto_applicable
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (fingerprint) DO UPDATE SET
                occurrence_count = agent.learning_candidates.occurrence_count + 1,
                last_seen = NOW(),
                updated_at = NOW()
            RETURNING id, occurrence_count, (xmax = 0)
            "#,
        )
        .bind(&fingerprint)
        .bind(learning_type)
        .bind(original_input)
        .bind(correct_choice)
        .bind(risk_level)
        .bind(auto_applicable)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow!("Failed to record learning: {}", e))?;

        let (candidate_id, occurrence_count, was_created) = row;

        // For low-risk corrections (entity aliases), apply immediately
        let auto_applied = if risk_level == "low" {
            sqlx::query(
                r#"
                INSERT INTO agent.entity_aliases (alias, canonical_name, source)
                VALUES ($1, $2, 'explicit_feedback')
                ON CONFLICT (alias) DO UPDATE SET
                    canonical_name = $2,
                    updated_at = NOW()
                "#,
            )
            .bind(original_input.to_lowercase().trim())
            .bind(correct_choice)
            .execute(pool)
            .await
            .is_ok()
        } else {
            false
        };

        // Hot-reload into memory if learned_data available and applied
        if auto_applied {
            if let Some(learned) = &self.learned_data {
                let mut guard = learned.write().await;
                guard.entity_aliases.insert(
                    original_input.to_lowercase(),
                    (correct_choice.to_string(), None),
                );
            }
        }

        // For medium-risk (phrase mappings), check if we hit threshold (3 occurrences)
        let threshold_applied = if risk_level == "medium" && occurrence_count >= 3 {
            let applied = sqlx::query(
                r#"
                INSERT INTO agent.invocation_phrases (phrase, verb, source)
                VALUES ($1, $2, 'threshold_auto')
                ON CONFLICT (phrase, verb) DO UPDATE SET
                    occurrence_count = agent.invocation_phrases.occurrence_count + 1,
                    updated_at = NOW()
                "#,
            )
            .bind(original_input.to_lowercase().trim())
            .bind(correct_choice)
            .execute(pool)
            .await
            .is_ok();

            if applied {
                // Mark candidate as applied
                let _ = sqlx::query(
                    "UPDATE agent.learning_candidates SET status = 'applied', applied_at = NOW() WHERE id = $1"
                )
                .bind(candidate_id)
                .execute(pool)
                .await;

                // Hot-reload into memory
                if let Some(learned) = &self.learned_data {
                    let mut guard = learned.write().await;
                    guard
                        .invocation_phrases
                        .insert(original_input.to_lowercase(), correct_choice.to_string());
                }
            }
            applied
        } else {
            false
        };

        // Build user-friendly message
        let message = if auto_applied || threshold_applied {
            format!(
                "Learned: '{}' maps to '{}'. Applied immediately.",
                original_input, correct_choice
            )
        } else {
            let remaining = 3 - occurrence_count;
            format!(
                "Recorded: '{}' → '{}'. Will apply after {} more confirmation(s).",
                original_input,
                correct_choice,
                remaining.max(0)
            )
        };

        tracing::info!(
            feedback_type = feedback_type,
            input = original_input,
            correction = correct_choice,
            auto_applied = auto_applied,
            threshold_applied = threshold_applied,
            occurrence_count = occurrence_count,
            "Intent feedback recorded"
        );

        Ok(json!({
            "recorded": true,
            "candidate_id": candidate_id,
            "occurrence_count": occurrence_count,
            "was_new": was_created,
            "learning_type": learning_type,
            "risk_level": risk_level,
            "auto_applied": auto_applied,
            "threshold_applied": threshold_applied,
            "message": message,
            "what_was_learned": {
                "input": original_input,
                "maps_to": correct_choice,
                "type": feedback_type
            }
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
            "fund" => "FUND",
            "document" => "DOCUMENT",
            "product" => "PRODUCT",
            "service" => "SERVICE",
            "kyc_case" => "KYC_CASE",
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
                    "Unknown lookup_type: {}. Valid types: cbu, entity, person, legal_entity, fund, document, product, service, kyc_case, role, jurisdiction, currency, attribute, instrument_class, market",
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

    // =========================================================================
    // Session v2 Tools - Memory-first CBU session management
    // =========================================================================

    /// Load a single CBU into the session scope
    async fn session_load_cbu(&self, args: Value) -> Result<Value> {
        use crate::session::CbuSession;

        let pool = self.require_pool()?;

        // Get CBU by ID or name
        let cbu_id: Uuid = if let Some(id_str) = args["cbu_id"].as_str() {
            Uuid::parse_str(id_str).map_err(|_| anyhow!("Invalid cbu_id UUID"))?
        } else if let Some(name) = args["cbu_name"].as_str() {
            // Resolve by name
            let row: Option<(Uuid,)> =
                sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE $1 LIMIT 1"#)
                    .bind(format!("%{}%", name))
                    .fetch_optional(pool)
                    .await?;
            row.ok_or_else(|| anyhow!("CBU not found: {}", name))?.0
        } else {
            return Err(anyhow!("Either cbu_id or cbu_name required"));
        };

        // Fetch CBU details
        let cbu: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
            r#"SELECT cbu_id, name, jurisdiction FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        let (id, name, jurisdiction) = cbu.ok_or_else(|| anyhow!("CBU not found"))?;

        // Get or create session and load the CBU
        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;

        // Use default session for now (could be parameterized)
        let session = guard.entry(Uuid::nil()).or_insert_with(CbuSession::new);

        session.load_cbu(id);
        session.maybe_save(pool);

        Ok(json!({
            "loaded": true,
            "cbu_id": id,
            "name": name,
            "jurisdiction": jurisdiction,
            "scope_size": session.count()
        }))
    }

    /// Load all CBUs in a jurisdiction
    async fn session_load_jurisdiction(&self, args: Value) -> Result<Value> {
        use crate::session::CbuSession;

        let pool = self.require_pool()?;
        let jurisdiction = args["jurisdiction"]
            .as_str()
            .ok_or_else(|| anyhow!("jurisdiction required"))?;

        // Find all CBUs in jurisdiction
        let rows: Vec<(Uuid, String)> =
            sqlx::query_as(r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE jurisdiction = $1"#)
                .bind(jurisdiction)
                .fetch_all(pool)
                .await?;

        if rows.is_empty() {
            return Err(anyhow!("No CBUs found in jurisdiction: {}", jurisdiction));
        }

        let cbu_ids: Vec<Uuid> = rows.iter().map(|(id, _)| *id).collect();
        let cbu_names: Vec<String> = rows.iter().map(|(_, name)| name.clone()).collect();

        // Get or create session
        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(CbuSession::new);

        // Load all CBUs
        for id in &cbu_ids {
            session.load_cbu(*id);
        }
        session.maybe_save(pool);

        Ok(json!({
            "loaded": true,
            "jurisdiction": jurisdiction,
            "cbu_count": cbu_ids.len(),
            "cbu_ids": cbu_ids,
            "cbu_names": cbu_names
        }))
    }

    /// Load all CBUs under a commercial client (galaxy)
    async fn session_load_galaxy(&self, args: Value) -> Result<Value> {
        use crate::session::CbuSession;

        let pool = self.require_pool()?;

        // Get apex entity by ID or name
        let apex_id: Uuid = if let Some(id_str) = args["apex_entity_id"].as_str() {
            Uuid::parse_str(id_str).map_err(|_| anyhow!("Invalid apex_entity_id UUID"))?
        } else if let Some(name) = args["apex_name"].as_str() {
            // Resolve by name
            let row: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE $1 LIMIT 1"#,
            )
            .bind(format!("%{}%", name))
            .fetch_optional(pool)
            .await?;
            row.ok_or_else(|| anyhow!("Entity not found: {}", name))?.0
        } else {
            return Err(anyhow!("Either apex_entity_id or apex_name required"));
        };

        // Find all CBUs under this commercial client
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE commercial_client_entity_id = $1"#,
        )
        .bind(apex_id)
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            return Err(anyhow!("No CBUs found under commercial client"));
        }

        let cbu_ids: Vec<Uuid> = rows.iter().map(|(id, _)| *id).collect();
        let cbu_names: Vec<String> = rows.iter().map(|(_, name)| name.clone()).collect();

        // Get apex entity name for response
        let apex_name: Option<(String,)> =
            sqlx::query_as(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                .bind(apex_id)
                .fetch_optional(pool)
                .await?;

        // Get or create session
        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(CbuSession::new);

        // Load all CBUs
        for id in &cbu_ids {
            session.load_cbu(*id);
        }
        session.maybe_save(pool);

        Ok(json!({
            "loaded": true,
            "apex_entity_id": apex_id,
            "apex_name": apex_name.map(|n| n.0),
            "cbu_count": cbu_ids.len(),
            "cbu_ids": cbu_ids,
            "cbu_names": cbu_names
        }))
    }

    /// Remove a CBU from the current session scope
    async fn session_unload_cbu(&self, args: Value) -> Result<Value> {
        use crate::session::CbuSession;

        let pool = self.require_pool()?;
        let cbu_id = args["cbu_id"]
            .as_str()
            .ok_or_else(|| anyhow!("cbu_id required"))?;
        let cbu_id = Uuid::parse_str(cbu_id).map_err(|_| anyhow!("Invalid cbu_id UUID"))?;

        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(CbuSession::new);

        session.unload_cbu(cbu_id);
        session.maybe_save(pool);

        Ok(json!({
            "unloaded": true,
            "cbu_id": cbu_id,
            "scope_size": session.count()
        }))
    }

    /// Clear session scope to empty (universe view)
    async fn session_clear(&self, args: Value) -> Result<Value> {
        use crate::session::CbuSession;
        let _ = args; // unused but kept for consistent signature

        let pool = self.require_pool()?;

        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(CbuSession::new);

        session.clear();
        session.maybe_save(pool);

        Ok(json!({
            "cleared": true,
            "scope_size": 0
        }))
    }

    /// Undo the last scope change
    async fn session_undo(&self, args: Value) -> Result<Value> {
        use crate::session::CbuSession;
        let _ = args;

        let pool = self.require_pool()?;

        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(CbuSession::new);

        let success = session.undo();
        if success {
            session.maybe_save(pool);
        }

        Ok(json!({
            "success": success,
            "scope_size": session.count(),
            "history_depth": session.history_depth(),
            "future_depth": session.future_depth()
        }))
    }

    /// Redo a previously undone scope change
    async fn session_redo(&self, args: Value) -> Result<Value> {
        use crate::session::CbuSession;
        let _ = args;

        let pool = self.require_pool()?;

        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(CbuSession::new);

        let success = session.redo();
        if success {
            session.maybe_save(pool);
        }

        Ok(json!({
            "success": success,
            "scope_size": session.count(),
            "history_depth": session.history_depth(),
            "future_depth": session.future_depth()
        }))
    }

    /// Get current session state and scope
    async fn session_info(&self, args: Value) -> Result<Value> {
        let _ = args;

        let pool = self.require_pool()?;

        let sessions = self.require_cbu_sessions()?;
        let guard = sessions.read().await;
        let session = guard.get(&Uuid::nil());

        if let Some(session) = session {
            let cbu_ids = session.cbu_ids_vec();

            // Fetch CBU names if we have any
            let cbu_names: Vec<String> = if cbu_ids.is_empty() {
                vec![]
            } else {
                let rows: Vec<(String,)> =
                    sqlx::query_as(r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = ANY($1)"#)
                        .bind(&cbu_ids)
                        .fetch_all(pool)
                        .await
                        .unwrap_or_default();
                rows.into_iter().map(|(name,)| name).collect()
            };

            Ok(json!({
                "id": session.id(),
                "name": session.name(),
                "cbu_count": cbu_ids.len(),
                "cbu_ids": cbu_ids,
                "cbu_names": cbu_names,
                "history_depth": session.history_depth(),
                "future_depth": session.future_depth(),
                "dirty": session.is_dirty()
            }))
        } else {
            Ok(json!({
                "id": null,
                "name": null,
                "cbu_count": 0,
                "cbu_ids": [],
                "cbu_names": [],
                "history_depth": 0,
                "future_depth": 0,
                "dirty": false
            }))
        }
    }

    /// List saved sessions
    async fn session_list(&self, args: Value) -> Result<Value> {
        use crate::session::CbuSession;

        let pool = self.require_pool()?;
        let limit = args["limit"].as_i64().unwrap_or(20);

        let sessions = CbuSession::list_all(pool, limit as usize).await;

        Ok(json!({
            "sessions": sessions.iter().map(|s| json!({
                "id": s.id,
                "name": s.name,
                "cbu_count": s.cbu_count,
                "updated_at": s.updated_at.to_rfc3339(),
                "expires_at": s.expires_at.to_rfc3339()
            })).collect::<Vec<_>>()
        }))
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

    // ==================== Resolution Sub-Session Tools ====================

    /// Start a resolution sub-session
    async fn resolution_start(&self, args: Value) -> Result<Value> {
        use crate::api::session::{
            AgentSession, EntityMatchInfo, ResolutionSubSession, SubSessionType, UnresolvedRefInfo,
        };

        let sessions = self.require_sessions()?;

        let parent_id: Uuid = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid session_id UUID"))?;

        let parent_dsl_index = args["parent_dsl_index"].as_u64().unwrap_or(0) as usize;

        // Parse unresolved refs
        let unresolved_refs_json = args["unresolved_refs"]
            .as_array()
            .ok_or_else(|| anyhow!("unresolved_refs array required"))?;

        let unresolved_refs: Vec<UnresolvedRefInfo> = unresolved_refs_json
            .iter()
            .map(|r| {
                let initial_matches = r["initial_matches"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|m| EntityMatchInfo {
                                value: m["value"].as_str().unwrap_or("").to_string(),
                                display: m["display"].as_str().unwrap_or("").to_string(),
                                score_pct: m["score_pct"].as_u64().unwrap_or(0) as u8,
                                detail: m["detail"].as_str().map(|s| s.to_string()),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                UnresolvedRefInfo {
                    ref_id: r["ref_id"].as_str().unwrap_or("").to_string(),
                    search_value: r["search_value"].as_str().unwrap_or("").to_string(),
                    entity_type: r["entity_type"].as_str().unwrap_or("entity").to_string(),
                    context_line: r["context_line"].as_str().unwrap_or("").to_string(),
                    initial_matches,
                }
            })
            .collect();

        // Get parent session
        let parent = {
            let sessions_guard = sessions.read().await;
            sessions_guard.get(&parent_id).cloned()
        }
        .ok_or_else(|| anyhow!("Parent session {} not found", parent_id))?;

        // Create resolution sub-session
        let resolution_state = ResolutionSubSession {
            unresolved_refs: unresolved_refs.clone(),
            parent_dsl_index,
            current_ref_index: 0,
            resolutions: std::collections::HashMap::new(),
        };

        let child =
            AgentSession::new_subsession(&parent, SubSessionType::Resolution(resolution_state));
        let child_id = child.id;

        // Store child session
        {
            let mut sessions_guard = sessions.write().await;
            sessions_guard.insert(child_id, child);
        }

        // Return sub-session info
        Ok(json!({
            "subsession_id": child_id.to_string(),
            "parent_id": parent_id.to_string(),
            "unresolved_count": unresolved_refs.len(),
            "current_ref": unresolved_refs.first().map(|r| json!({
                "ref_id": r.ref_id,
                "search_value": r.search_value,
                "entity_type": r.entity_type,
                "matches": r.initial_matches.iter().map(|m| json!({
                    "value": m.value,
                    "display": m.display,
                    "score_pct": m.score_pct,
                    "detail": m.detail
                })).collect::<Vec<_>>()
            }))
        }))
    }

    /// Refine search using discriminators
    async fn resolution_search(&self, args: Value) -> Result<Value> {
        use crate::api::session::SubSessionType;

        let sessions = self.require_sessions()?;

        let subsession_id: Uuid = args["subsession_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subsession_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subsession_id UUID"))?;

        // Get sub-session
        let session = {
            let sessions_guard = sessions.read().await;
            sessions_guard.get(&subsession_id).cloned()
        }
        .ok_or_else(|| anyhow!("Sub-session {} not found", subsession_id))?;

        let SubSessionType::Resolution(resolution) = &session.sub_session_type else {
            return Err(anyhow!(
                "Session {} is not a resolution sub-session",
                subsession_id
            ));
        };

        let current_ref = resolution
            .unresolved_refs
            .get(resolution.current_ref_index)
            .ok_or_else(|| anyhow!("No current reference to resolve"))?;

        // Parse discriminators
        let discriminators = args.get("discriminators");
        let natural_language = args["natural_language"].as_str();

        // Build search query with discriminators
        let base_query = &current_ref.search_value;
        let entity_type = &current_ref.entity_type;

        // For now, re-search with the base query
        // TODO: Apply discriminators to filter results
        let nickname = match entity_type.as_str() {
            "person" => "PERSON",
            "company" | "legal_entity" => "LEGAL_ENTITY",
            "cbu" => "CBU",
            _ => "ENTITY",
        };

        let raw_matches = self.gateway_search(nickname, Some(base_query), 10).await?;

        // Apply discriminator filtering (basic implementation)
        let filtered_matches = raw_matches;

        if let Some(disc) = discriminators {
            // TODO: Implement proper discriminator filtering via EntityEnricher
            // For now, log that we received discriminators
            tracing::info!(
                "Resolution search with discriminators: {:?}, natural_language: {:?}",
                disc,
                natural_language
            );
        }

        Ok(json!({
            "ref_id": current_ref.ref_id,
            "search_value": current_ref.search_value,
            "matches": filtered_matches.iter().map(|(id, display, score)| json!({
                "value": id,
                "display": display,
                "score_pct": (score * 100.0) as u8
            })).collect::<Vec<_>>(),
            "discriminators_applied": discriminators.is_some(),
            "natural_language_parsed": natural_language.is_some()
        }))
    }

    /// Select a match to resolve current reference
    async fn resolution_select(&self, args: Value) -> Result<Value> {
        use crate::api::session::SubSessionType;

        let sessions = self.require_sessions()?;

        let subsession_id: Uuid = args["subsession_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subsession_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subsession_id UUID"))?;

        let selection = args["selection"].as_u64();
        let entity_id = args["entity_id"].as_str();

        if selection.is_none() && entity_id.is_none() {
            return Err(anyhow!("Either selection index or entity_id required"));
        }

        // Get and update sub-session
        let mut session = {
            let sessions_guard = sessions.read().await;
            sessions_guard.get(&subsession_id).cloned()
        }
        .ok_or_else(|| anyhow!("Sub-session {} not found", subsession_id))?;

        // Extract values and update resolution in a scope that ends before we move session
        let (
            ref_id,
            selected_value,
            is_complete,
            resolutions_count,
            remaining_count,
            next_ref_json,
        ) = {
            let SubSessionType::Resolution(resolution) = &mut session.sub_session_type else {
                return Err(anyhow!(
                    "Session {} is not a resolution sub-session",
                    subsession_id
                ));
            };

            let current_ref = resolution
                .unresolved_refs
                .get(resolution.current_ref_index)
                .ok_or_else(|| anyhow!("No current reference to resolve"))?;

            // Determine the selected value
            let selected_value = if let Some(idx) = selection {
                let match_info = current_ref
                    .initial_matches
                    .get(idx as usize)
                    .ok_or_else(|| anyhow!("Selection index {} out of range", idx))?;
                match_info.value.clone()
            } else if let Some(eid) = entity_id {
                eid.to_string()
            } else {
                return Err(anyhow!("No selection provided"));
            };

            // Record resolution
            let ref_id = current_ref.ref_id.clone();
            resolution
                .resolutions
                .insert(ref_id.clone(), selected_value.clone());

            // Move to next
            resolution.current_ref_index += 1;

            let is_complete = resolution.current_ref_index >= resolution.unresolved_refs.len();
            let next_ref_json = if !is_complete {
                resolution
                    .unresolved_refs
                    .get(resolution.current_ref_index)
                    .map(|r| {
                        json!({
                            "ref_id": r.ref_id,
                            "search_value": r.search_value,
                            "entity_type": r.entity_type,
                            "context_line": r.context_line,
                            "initial_matches": r.initial_matches
                        })
                    })
            } else {
                None
            };

            let resolutions_count = resolution.current_ref_index;
            let remaining_count = resolution.unresolved_refs.len() - resolution.current_ref_index;

            (
                ref_id,
                selected_value,
                is_complete,
                resolutions_count,
                remaining_count,
                next_ref_json,
            )
        };

        // Store updated session
        {
            let mut sessions_guard = sessions.write().await;
            sessions_guard.insert(subsession_id, session);
        }

        Ok(json!({
            "resolved": {
                "ref_id": ref_id,
                "value": selected_value
            },
            "is_complete": is_complete,
            "resolutions_count": resolutions_count,
            "remaining_count": remaining_count,
            "next_ref": next_ref_json
        }))
    }

    /// Complete resolution sub-session
    async fn resolution_complete(&self, args: Value) -> Result<Value> {
        use crate::api::session::{BoundEntity, SubSessionType};

        let sessions = self.require_sessions()?;

        let subsession_id: Uuid = args["subsession_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subsession_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subsession_id UUID"))?;

        let apply = args["apply"].as_bool().unwrap_or(true);

        // Remove child session
        let child = {
            let mut sessions_guard = sessions.write().await;
            sessions_guard.remove(&subsession_id)
        }
        .ok_or_else(|| anyhow!("Sub-session {} not found", subsession_id))?;

        let parent_id = child
            .parent_session_id
            .ok_or_else(|| anyhow!("Session {} has no parent", subsession_id))?;

        let SubSessionType::Resolution(resolution) = &child.sub_session_type else {
            return Err(anyhow!(
                "Session {} is not a resolution sub-session",
                subsession_id
            ));
        };

        let resolutions_count = resolution.resolutions.len();

        if apply && resolutions_count > 0 {
            // Build bound entities from resolutions
            let mut bound_entities = Vec::new();
            for unresolved in &resolution.unresolved_refs {
                if let Some(resolved_value) = resolution.resolutions.get(&unresolved.ref_id) {
                    // Find match info
                    let match_info = unresolved
                        .initial_matches
                        .iter()
                        .find(|m| &m.value == resolved_value);

                    if let Some(info) = match_info {
                        if let Ok(uuid) = Uuid::parse_str(resolved_value) {
                            bound_entities.push((
                                unresolved.ref_id.clone(),
                                BoundEntity {
                                    id: uuid,
                                    entity_type: unresolved.entity_type.clone(),
                                    display_name: info.display.clone(),
                                },
                            ));
                        }
                    }
                }
            }

            // Apply to parent session
            {
                let mut sessions_guard = sessions.write().await;
                if let Some(parent) = sessions_guard.get_mut(&parent_id) {
                    for (ref_id, bound_entity) in &bound_entities {
                        parent
                            .context
                            .bindings
                            .insert(ref_id.clone(), bound_entity.clone());
                        tracing::info!(
                            "Applied resolution: {} -> {} ({})",
                            ref_id,
                            bound_entity.id,
                            bound_entity.display_name
                        );
                    }
                }
            }
        }

        Ok(json!({
            "success": true,
            "parent_id": parent_id.to_string(),
            "resolutions_applied": if apply { resolutions_count } else { 0 },
            "message": format!(
                "Resolution complete. {} bindings {}.",
                resolutions_count,
                if apply { "applied to parent" } else { "discarded" }
            )
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
            SELECT profile_id, status, version, created_at, activated_at
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
        .await?;

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
        .await?;

        // Build summary
        let has_profile = profile.is_some();
        let profile_info = profile.map(|p| {
            json!({
                "profile_id": p.profile_id.to_string(),
                "status": p.status,
                "version": p.version,
                "activated_at": p.activated_at.map(|t| t.to_rfc3339())
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
        use crate::feedback::FeedbackInspector;

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
        use crate::feedback::{FeedbackInspector, IssueFilter};

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
        let tests_dir = std::path::PathBuf::from("tests/generated");
        let repro_gen = ReproGenerator::new(tests_dir);

        let result = repro_gen
            .generate_and_verify(&inspector, &args.fingerprint)
            .await?;

        Ok(json!({
            "fingerprint": args.fingerprint,
            "repro_type": format!("{:?}", result.repro_type),
            "path": result.repro_path.to_string_lossy(),
            "verified": result.verified,
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
        let todos_dir = std::path::PathBuf::from("todos/generated");
        let todo_gen = TodoGenerator::new(todos_dir);

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

    // =========================================================================
    // Agent Learning Tools (Loop 2 - Continuous Improvement)
    // =========================================================================

    async fn intent_analyze(&self, args: Value) -> Result<Value> {
        use crate::agent::AgentLearningInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            since_hours: Option<i64>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let since = args
            .since_hours
            .map(|h| chrono::Utc::now() - chrono::Duration::hours(h));

        let inspector = AgentLearningInspector::new(pool.clone());
        let stats = inspector.analyze(since).await?;

        Ok(json!({
            "events_processed": stats.events_processed,
            "candidates_created": stats.candidates_created,
            "candidates_updated": stats.candidates_updated,
            "auto_applied": stats.auto_applied,
            "queued_for_review": stats.queued_for_review
        }))
    }

    async fn intent_list(&self, args: Value) -> Result<Value> {
        use crate::agent::{AgentLearningInspector, LearningStatus, LearningType};

        #[derive(serde::Deserialize)]
        struct Args {
            status: Option<String>,
            learning_type: Option<String>,
            limit: Option<i64>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let status = args.status.and_then(|s| match s.as_str() {
            "pending" => Some(LearningStatus::Pending),
            "approved" => Some(LearningStatus::Approved),
            "rejected" => Some(LearningStatus::Rejected),
            "applied" => Some(LearningStatus::Applied),
            _ => None,
        });

        let learning_type = args.learning_type.and_then(|t| match t.as_str() {
            "entity_alias" => Some(LearningType::EntityAlias),
            "lexicon_token" => Some(LearningType::LexiconToken),
            "invocation_phrase" => Some(LearningType::InvocationPhrase),
            "prompt_change" => Some(LearningType::PromptChange),
            _ => None,
        });

        let inspector = AgentLearningInspector::new(pool.clone());
        let candidates = inspector
            .list_candidates(status, learning_type, args.limit.unwrap_or(20))
            .await?;

        Ok(json!({
            "count": candidates.len(),
            "candidates": candidates.iter().map(|c| json!({
                "fingerprint": c.fingerprint,
                "learning_type": c.learning_type.as_str(),
                "input_pattern": c.input_pattern,
                "suggested_output": c.suggested_output,
                "occurrence_count": c.occurrence_count,
                "risk_level": c.risk_level.as_str(),
                "status": c.status.as_str(),
                "first_seen": c.first_seen.to_rfc3339(),
                "last_seen": c.last_seen.to_rfc3339()
            })).collect::<Vec<_>>()
        }))
    }

    async fn intent_approve(&self, args: Value) -> Result<Value> {
        use crate::agent::AgentLearningInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let inspector = AgentLearningInspector::new(pool.clone());
        let applied = inspector
            .approve_candidate(&args.fingerprint, "mcp_user")
            .await?;

        Ok(json!({
            "success": true,
            "learning_type": applied.learning_type.as_str(),
            "input_pattern": applied.input_pattern,
            "output": applied.output,
            "applied_at": applied.applied_at.to_rfc3339()
        }))
    }

    async fn intent_reject(&self, args: Value) -> Result<Value> {
        use crate::agent::AgentLearningInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let inspector = AgentLearningInspector::new(pool.clone());
        inspector
            .reject_candidate(&args.fingerprint, "mcp_user")
            .await?;

        Ok(json!({
            "success": true,
            "fingerprint": args.fingerprint,
            "status": "rejected"
        }))
    }

    async fn intent_reload(&self, _args: Value) -> Result<Value> {
        use crate::agent::LearningWarmup;

        let pool = self.require_pool()?;

        let warmup = LearningWarmup::new(pool.clone());
        let (_, stats) = warmup.warmup().await?;

        Ok(json!({
            "success": true,
            "entity_aliases_loaded": stats.entity_aliases_loaded,
            "lexicon_tokens_loaded": stats.lexicon_tokens_loaded,
            "invocation_phrases_loaded": stats.invocation_phrases_loaded,
            "learnings_auto_applied": stats.learnings_auto_applied,
            "duration_ms": stats.duration_ms
        }))
    }

    // =========================================================================
    // Service Resource Pipeline Tools
    // =========================================================================

    async fn service_intent_create(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{NewServiceIntent, ServiceResourcePipelineService};

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
            product_id: String,
            service_id: String,
            options: Option<Value>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        // Resolve CBU ID (UUID or name)
        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        // Resolve product ID
        let product_id = self.resolve_product_id(&args.product_id).await?;

        // Resolve service ID
        let service_id = self.resolve_service_id(&args.service_id).await?;

        let service = ServiceResourcePipelineService::new(pool.clone());
        let input = NewServiceIntent {
            cbu_id,
            product_id,
            service_id,
            options: args.options,
            created_by: None,
        };

        let intent_id = service.create_service_intent(&input).await?;

        Ok(json!({
            "success": true,
            "intent_id": intent_id,
            "cbu_id": cbu_id,
            "product_id": product_id,
            "service_id": service_id
        }))
    }

    async fn service_intent_list(&self, args: Value) -> Result<Value> {
        use crate::service_resources::ServiceResourcePipelineService;

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let service = ServiceResourcePipelineService::new(pool.clone());
        let intents = service.get_service_intents(cbu_id).await?;

        Ok(json!({
            "success": true,
            "count": intents.len(),
            "intents": intents.iter().map(|i| json!({
                "intent_id": i.intent_id,
                "cbu_id": i.cbu_id,
                "product_id": i.product_id,
                "service_id": i.service_id,
                "options": i.options,
                "status": i.status,
                "created_at": i.created_at.map(|t| t.to_rfc3339())
            })).collect::<Vec<_>>()
        }))
    }

    async fn service_discovery_run(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{load_srdefs_from_config, run_discovery_pipeline};

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let registry = load_srdefs_from_config().unwrap_or_default();
        let result = run_discovery_pipeline(pool, &registry, cbu_id).await?;

        Ok(json!({
            "success": true,
            "cbu_id": result.cbu_id,
            "srdefs_discovered": result.srdefs_discovered,
            "attrs_rolled_up": result.attrs_rolled_up,
            "attrs_populated": result.attrs_populated,
            "attrs_missing": result.attrs_missing
        }))
    }

    async fn service_attributes_gaps(&self, args: Value) -> Result<Value> {
        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        // Query the gap view directly
        let gaps: Vec<AttrGapRow> = sqlx::query_as(
            r#"
            SELECT attr_id, attr_code, attr_name, attr_category, has_value
            FROM "ob-poc".v_cbu_attr_gaps
            WHERE cbu_id = $1 AND NOT has_value
            ORDER BY attr_category, attr_name
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "gap_count": gaps.len(),
            "gaps": gaps.iter().map(|g| json!({
                "attr_id": g.attr_id,
                "attr_code": g.attr_code,
                "attr_name": g.attr_name,
                "attr_category": g.attr_category
            })).collect::<Vec<_>>()
        }))
    }

    async fn service_attributes_set(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{
            AttributeSource, ServiceResourcePipelineService, SetCbuAttrValue,
        };

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
            attr_id: Uuid,
            value: Value,
            source: Option<String>,
            evidence_refs: Option<Vec<String>>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let source = match args.source.as_deref() {
            Some("derived") => AttributeSource::Derived,
            Some("entity") => AttributeSource::Entity,
            Some("cbu") => AttributeSource::Cbu,
            Some("document") => AttributeSource::Document,
            Some("external") => AttributeSource::External,
            _ => AttributeSource::Manual,
        };

        // Convert evidence_refs strings to EvidenceRef structs
        let evidence_refs = args.evidence_refs.map(|refs| {
            refs.into_iter()
                .map(|r| crate::service_resources::EvidenceRef {
                    ref_type: "document".to_string(),
                    id: Uuid::parse_str(&r).ok().map(|u| u.to_string()),
                    path: None,
                    details: Some(serde_json::json!({ "description": r })),
                })
                .collect()
        });

        let service = ServiceResourcePipelineService::new(pool.clone());
        let input = SetCbuAttrValue {
            cbu_id,
            attr_id: args.attr_id,
            value: args.value.clone(),
            source,
            evidence_refs,
            explain_refs: None,
        };

        service.set_cbu_attr_value(&input).await?;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "attr_id": args.attr_id,
            "value": args.value
        }))
    }

    async fn service_readiness_get(&self, args: Value) -> Result<Value> {
        use crate::service_resources::ServiceResourcePipelineService;

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let service = ServiceResourcePipelineService::new(pool.clone());
        let readiness = service.get_service_readiness(cbu_id).await?;

        let ready = readiness.iter().filter(|r| r.status == "ready").count();
        let partial = readiness.iter().filter(|r| r.status == "partial").count();
        let blocked = readiness.iter().filter(|r| r.status == "blocked").count();

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "summary": {
                "total": readiness.len(),
                "ready": ready,
                "partial": partial,
                "blocked": blocked
            },
            "services": readiness.iter().map(|r| json!({
                "service_id": r.service_id,
                "product_id": r.product_id,
                "status": r.status,
                "blocking_reasons": r.blocking_reasons
            })).collect::<Vec<_>>()
        }))
    }

    async fn service_readiness_recompute(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{load_srdefs_from_config, ReadinessEngine};

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let registry = load_srdefs_from_config().unwrap_or_default();
        let engine = ReadinessEngine::new(pool, &registry);
        let result = engine.compute_for_cbu(cbu_id).await?;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "recomputed": true,
            "total_services": result.total_services,
            "ready": result.ready,
            "partial": result.partial,
            "blocked": result.blocked
        }))
    }

    async fn service_pipeline_run(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{
            load_srdefs_from_config, run_discovery_pipeline, run_provisioning_pipeline,
        };

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
            dry_run: Option<bool>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;
        let _dry_run = args.dry_run.unwrap_or(false);

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let registry = load_srdefs_from_config().unwrap_or_default();

        // Run discovery + rollup + populate
        let discovery = run_discovery_pipeline(pool, &registry, cbu_id).await?;

        // Run provisioning + readiness
        let provisioning = run_provisioning_pipeline(pool, &registry, cbu_id).await?;

        Ok(json!({
            "success": true,
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
        }))
    }

    async fn srdef_list(&self, args: Value) -> Result<Value> {
        use crate::service_resources::load_srdefs_from_config;

        #[derive(serde::Deserialize, Default)]
        struct Args {
            domain: Option<String>,
            resource_type: Option<String>,
        }

        let args: Args = serde_json::from_value(args).unwrap_or_default();

        let registry = load_srdefs_from_config().unwrap_or_default();

        let srdefs: Vec<_> = registry
            .srdefs
            .values()
            .filter(|s| {
                args.domain
                    .as_ref()
                    .map_or(true, |d| s.code.starts_with(&format!("{}:", d)))
            })
            .filter(|s| {
                args.resource_type
                    .as_ref()
                    .map_or(true, |rt| s.resource_type.eq_ignore_ascii_case(rt))
            })
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

        Ok(json!({
            "success": true,
            "count": srdefs.len(),
            "srdefs": srdefs
        }))
    }

    async fn srdef_get(&self, args: Value) -> Result<Value> {
        use crate::service_resources::load_srdefs_from_config;

        #[derive(serde::Deserialize)]
        struct Args {
            srdef_id: String,
        }

        let args: Args = serde_json::from_value(args)?;

        let registry = load_srdefs_from_config().unwrap_or_default();

        // Try direct lookup, then with decoded colons
        let srdef_id = args.srdef_id.replace("%3A", ":").replace("%3a", ":");

        match registry.get(&srdef_id) {
            Some(srdef) => Ok(json!({
                "success": true,
                "srdef": {
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
                }
            })),
            None => Err(anyhow!("SRDEF not found: {}", srdef_id)),
        }
    }

    // Helper: resolve CBU ID from UUID string or name
    async fn resolve_cbu_id(&self, value: &str) -> Result<Uuid> {
        // Try as UUID first
        if let Ok(uuid) = Uuid::parse_str(value) {
            return Ok(uuid);
        }

        // Try as name lookup
        let pool = self.require_pool()?;
        let cbu_id: Option<Uuid> =
            sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE $1 LIMIT 1"#)
                .bind(value)
                .fetch_optional(pool)
                .await?;

        cbu_id.ok_or_else(|| anyhow!("CBU not found: {}", value))
    }

    // Helper: resolve product ID from UUID string or name
    async fn resolve_product_id(&self, value: &str) -> Result<Uuid> {
        if let Ok(uuid) = Uuid::parse_str(value) {
            return Ok(uuid);
        }

        let pool = self.require_pool()?;
        let product_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT product_id FROM "ob-poc".products WHERE name ILIKE $1 LIMIT 1"#,
        )
        .bind(value)
        .fetch_optional(pool)
        .await?;

        product_id.ok_or_else(|| anyhow!("Product not found: {}", value))
    }

    // Helper: resolve service ID from UUID string or name
    async fn resolve_service_id(&self, value: &str) -> Result<Uuid> {
        if let Ok(uuid) = Uuid::parse_str(value) {
            return Ok(uuid);
        }

        let pool = self.require_pool()?;
        let service_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT service_id FROM "ob-poc".services WHERE name ILIKE $1 LIMIT 1"#,
        )
        .bind(value)
        .fetch_optional(pool)
        .await?;

        service_id.ok_or_else(|| anyhow!("Service not found: {}", value))
    }
}

// Helper struct for attribute gap query
#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct AttrGapRow {
    attr_id: Uuid,
    attr_code: String,
    attr_name: String,
    attr_category: String,
    has_value: bool,
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
