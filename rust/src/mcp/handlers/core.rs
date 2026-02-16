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

use crate::agent::learning::embedder::SharedEmbedder;
use crate::agent::learning::warmup::SharedLearnedData;
use crate::api::cbu_session_routes::CbuSessionStore;
use crate::api::session::SessionStore;
use crate::database::generation_log_repository::{
    CompileResult, GenerationAttempt, GenerationLogRepository, LintResult, ParseResult,
};
use crate::database::VisualizationRepository;
use crate::dsl_v2::macros::MacroRegistry;
use crate::dsl_v2::{
    compile, expand_templates_simple, gateway_resolver, parse_program, registry, runtime_registry,
    AtomicExecutionResult, BatchPolicy, BestEffortExecutionResult, DslExecutor, ExecutionContext,
};
use crate::mcp::verb_search::HybridVerbSearcher;
use crate::mcp::verb_search_factory::VerbSearcherFactory;

use crate::mcp::protocol::ToolCallResult;

// ============================================================================
// Row Structs (replacing anonymous tuples for FromRow)
// ============================================================================

/// Row struct for learning candidate upsert result
#[derive(Debug, sqlx::FromRow)]
struct LearningCandidateUpsertRow {
    id: i64,
    occurrence_count: i32,
    was_created: bool,
}

/// Outcome of MCP DSL execution - either atomic (all-or-nothing) or best-effort (partial success)
#[derive(Debug)]
enum MpcExecutionOutcome {
    /// Atomic execution result (all steps in single transaction)
    Atomic(AtomicExecutionResult),
    /// Best-effort execution result (continues on failure)
    BestEffort(BestEffortExecutionResult),
}

/// Tool handlers with database access, EntityGateway client, and UI session store
pub struct ToolHandlers {
    pub(super) pool: PgPool,
    pub(super) generation_log: GenerationLogRepository,
    pub(super) repo: VisualizationRepository,
    /// EntityGateway client for all entity lookups (lazy-initialized)
    pub(super) gateway_client: Arc<Mutex<Option<EntityGatewayClient<Channel>>>>,
    /// UI session store - shared with web server for template batch operations
    pub(super) sessions: Option<SessionStore>,
    /// CBU session store for load/unload operations
    pub(super) cbu_sessions: Option<CbuSessionStore>,
    /// Hybrid verb searcher (lazy-initialized)
    pub(super) verb_searcher: Arc<Mutex<Option<HybridVerbSearcher>>>,
    /// Learned data from agent learning system (shared reference)
    pub(super) learned_data: Option<SharedLearnedData>,
    /// Embedder for semantic operations - REQUIRED, no fallback
    pub(super) embedder: SharedEmbedder,
    /// Feedback service for learning loop
    pub(super) feedback_service: Option<Arc<ob_semantic_matcher::FeedbackService>>,
    /// Operator macro registry for business vocabulary search
    pub(super) macro_registry: Option<Arc<MacroRegistry>>,
    /// Lexicon service for fast in-memory lexical verb search (Phase A of 072)
    pub(super) lexicon: Option<crate::mcp::verb_search::SharedLexicon>,
}

impl ToolHandlers {
    /// Create handlers with embedder (REQUIRED for semantic pipeline)
    ///
    /// There is only ONE path - all tools require the Candle embedder for semantic search.
    pub fn new(pool: PgPool, embedder: SharedEmbedder) -> Self {
        ToolHandlers {
            generation_log: GenerationLogRepository::new(pool.clone()),
            repo: VisualizationRepository::new(pool.clone()),
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
            sessions: None,
            cbu_sessions: None,
            verb_searcher: Arc::new(Mutex::new(None)),
            learned_data: None,
            embedder,
            feedback_service: None,
            macro_registry: None,
            lexicon: None,
        }
    }

    /// Set the lexicon service for fast in-memory lexical verb search
    pub fn with_lexicon(mut self, lexicon: crate::mcp::verb_search::SharedLexicon) -> Self {
        self.lexicon = Some(lexicon);
        self
    }

    /// Get the session store, or error if not configured
    pub(super) fn require_sessions(&self) -> Result<&SessionStore> {
        self.sessions.as_ref().ok_or_else(|| {
            anyhow!("Session store not configured. Batch operations require integrated mode.")
        })
    }

    /// Get the CBU session store, or error if not configured
    pub(super) fn require_cbu_sessions(&self) -> Result<&CbuSessionStore> {
        self.cbu_sessions.as_ref().ok_or_else(|| {
            anyhow!("CBU session store not configured. CBU operations require integrated mode.")
        })
    }

    /// Get the database pool
    pub(super) fn require_pool(&self) -> Result<&PgPool> {
        Ok(&self.pool)
    }

    /// Get or create EntityGateway client
    pub(super) async fn get_gateway_client(&self) -> Result<EntityGatewayClient<Channel>> {
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
    /// Lazy-initializes on first use. All DB access through VerbService.
    pub(super) async fn get_verb_searcher(&self) -> Result<HybridVerbSearcher> {
        let mut guard = self.verb_searcher.lock().await;
        if let Some(searcher) = guard.as_ref() {
            return Ok(searcher.clone());
        }

        // Use factory for consistent configuration across all call sites
        let searcher = if let Some(ref macro_registry) = self.macro_registry {
            VerbSearcherFactory::build(
                &self.pool,
                self.embedder.clone(),
                self.learned_data.clone(),
                macro_registry.clone(),
                self.lexicon.clone(),
            )
        } else {
            // Fallback without macro registry (should be rare)
            VerbSearcherFactory::build(
                &self.pool,
                self.embedder.clone(),
                self.learned_data.clone(),
                Arc::new(MacroRegistry::new()),
                self.lexicon.clone(),
            )
        };

        *guard = Some(searcher.clone());
        Ok(searcher)
    }

    /// Search via EntityGateway
    pub(super) async fn gateway_search(
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
            // Learning management tools
            "intent_block" => self.intent_block(args).await,
            "learning_import" => self.learning_import(args).await,
            "learning_list" => self.learning_list(args).await,
            "learning_approve" => self.learning_approve(args).await,
            "learning_reject" => self.learning_reject(args).await,
            "learning_stats" => self.learning_stats(args).await,
            "cbu_get" => self.cbu_get(args).await,
            "cbu_list" => self.cbu_list(args).await,
            "entity_get" => self.entity_get(args).await,
            "verbs_list" => self.verbs_list(args),
            "schema_info" => self.schema_info(args).await,
            "dsl_lookup" => self.dsl_lookup(args).await,
            "dsl_complete" => self.dsl_complete(args),
            "dsl_signature" => self.dsl_signature(args),
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
            // Learning system tools
            "learning_analyze" => self.learning_analyze(args).await,
            "learning_apply" => self.learning_apply(args).await,
            "embeddings_status" => self.embeddings_status(args).await,
            // Promotion pipeline tools
            "promotion_run_cycle" => self.promotion_run_cycle(args).await,
            "promotion_candidates" => self.promotion_candidates(args).await,
            "promotion_review_queue" => self.promotion_review_queue(args).await,
            "promotion_approve" => self.promotion_approve(args).await,
            "promotion_reject" => self.promotion_reject(args).await,
            "promotion_health" => self.promotion_health(args).await,
            "promotion_pipeline_status" => self.promotion_pipeline_status(args).await,
            // Teaching tools
            "teach_phrase" => self.teach_phrase(args).await,
            "unteach_phrase" => self.unteach_phrase(args).await,
            "teaching_status" => self.teaching_status(args).await,
            // Semantic Registry tools — dispatch to sem_reg agent handlers
            name if name.starts_with("sem_reg_") => {
                use crate::sem_reg::agent::mcp_tools::{dispatch_tool, SemRegToolContext};

                let actor = crate::policy::ActorResolver::from_env();
                let ctx = SemRegToolContext {
                    pool: &self.pool,
                    actor: &actor,
                };
                let result = dispatch_tool(&ctx, name, &args).await;
                if result.success {
                    Ok(result.data)
                } else {
                    Err(anyhow!(result
                        .error
                        .unwrap_or_else(|| "sem_reg tool failed".into())))
                }
            }
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
    ///
    /// Optional parameters:
    /// - `intent_feedback_id`: Links to learning loop (from dsl_generate flow)
    pub(super) async fn dsl_execute(&self, args: Value) -> Result<Value> {
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

        // Extract intent_feedback_id if provided (links to learning loop)
        let intent_feedback_id = args["intent_feedback_id"].as_i64();

        // Start generation log with optional learning loop linkage
        let log_id = self
            .generation_log
            .start_log(
                user_intent,
                "mcp",
                None, // session_id
                None, // cbu_id
                None, // model
                intent_feedback_id,
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

        // =====================================================================
        // EXPANSION STAGE - Determine batch policy and derive locks
        // =====================================================================
        let templates = runtime_registry().templates();
        let expansion_result = expand_templates_simple(source, templates);

        let expansion_report = match expansion_result {
            Ok(output) => {
                tracing::debug!(
                    "[MCP] Expansion complete: batch_policy={:?}, locks={}, statements={}",
                    output.report.batch_policy,
                    output.report.derived_lock_set.len(),
                    output.report.expanded_statement_count
                );
                Some(output.report)
            }
            Err(e) => {
                tracing::warn!(
                    "[MCP] Expansion failed (continuing with best-effort): {}",
                    e
                );
                None
            }
        };

        // Determine batch policy from expansion report (default: BestEffort)
        let batch_policy = expansion_report
            .as_ref()
            .map(|r| r.batch_policy)
            .unwrap_or(BatchPolicy::BestEffort);

        // =====================================================================
        // EXECUTE - Route based on batch policy
        // =====================================================================
        let executor = DslExecutor::new(self.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Execute based on batch policy
        let execution_outcome = match batch_policy {
            BatchPolicy::Atomic => {
                tracing::info!(
                    "[MCP] Using atomic execution with locks (policy=atomic, locks={})",
                    expansion_report
                        .as_ref()
                        .map(|r| r.derived_lock_set.len())
                        .unwrap_or(0)
                );
                executor
                    .execute_plan_atomic_with_locks(&plan, &mut ctx, expansion_report.as_ref())
                    .await
                    .map(MpcExecutionOutcome::Atomic)
            }
            BatchPolicy::BestEffort => {
                tracing::info!("[MCP] Using best-effort execution (policy=best_effort)");
                executor
                    .execute_plan_best_effort(&plan, &mut ctx)
                    .await
                    .map(MpcExecutionOutcome::BestEffort)
            }
        };

        match execution_outcome {
            Ok(outcome) => {
                // Extract results based on outcome type
                let (steps_executed, execution_error) = match &outcome {
                    MpcExecutionOutcome::Atomic(atomic) => match atomic {
                        AtomicExecutionResult::Committed { step_results, .. } => {
                            (step_results.len(), None)
                        }
                        AtomicExecutionResult::RolledBack {
                            failed_at_step,
                            error,
                            ..
                        } => (
                            0,
                            Some(format!(
                                "Atomic execution rolled back at step {}: {}",
                                failed_at_step, error
                            )),
                        ),
                        AtomicExecutionResult::LockContention {
                            entity_type,
                            entity_id,
                            ..
                        } => (
                            0,
                            Some(format!(
                                "Lock contention on {}:{} - another session is modifying this entity",
                                entity_type, entity_id
                            )),
                        ),
                    },
                    MpcExecutionOutcome::BestEffort(best_effort) => {
                        let success_count = best_effort
                            .verb_results
                            .iter()
                            .filter(|r| r.is_some())
                            .count();
                        let error_summary = if !best_effort.errors.is_empty() {
                            Some(best_effort.errors.summary())
                        } else {
                            None
                        };
                        (success_count, error_summary)
                    }
                };

                // Check if execution succeeded
                if let Some(error_msg) = execution_error {
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

                    return Ok(json!({
                        "success": false,
                        "error": error_msg,
                        "batch_policy": format!("{:?}", batch_policy),
                        "completed": ctx.symbols.len()
                    }));
                }

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

                        // Update DAG state for executed verbs (Phase 5: context flows down)
                        // Extract executed verbs from the plan and update session's DAG
                        for step in &plan.steps {
                            let verb_fqn =
                                format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
                            crate::mcp::update_dag_after_execution(session, &verb_fqn);
                        }

                        // Touch updated_at to trigger watch notification
                        session.updated_at = chrono::Utc::now();

                        tracing::debug!(
                            session_id = %sid,
                            bindings_count = ctx.symbols.len(),
                            has_view_state = view_state.is_some(),
                            has_viewport_state = viewport_state.is_some(),
                            has_scope_change = scope_change.is_some(),
                            executed_verbs = plan.steps.len(),
                            "MCP execution persisted to session with DAG update"
                        );
                    }
                }

                Ok(json!({
                    "success": true,
                    "steps_executed": steps_executed,
                    "batch_policy": format!("{:?}", batch_policy),
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

        // Perform hybrid search (no user_id in this context)
        let results = searcher.search(query, None, domain, limit).await?;

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
        let instruction = args["instruction"]
            .as_str()
            .ok_or_else(|| anyhow!("instruction required"))?;
        let domain = args["domain"].as_str();
        let execute = args["execute"].as_bool().unwrap_or(false);
        let session_id = args["session_id"]
            .as_str()
            .and_then(|s| uuid::Uuid::parse_str(s).ok());

        // Route through unified orchestrator
        let searcher = self.get_verb_searcher().await?;
        let actor = crate::policy::ActorResolver::from_env();
        let policy_gate = std::sync::Arc::new(crate::policy::PolicyGate::from_env());
        let orch_ctx = crate::agent::orchestrator::OrchestratorContext {
            actor,
            session_id,
            case_id: None,
            dominant_entity_id: None,
            scope: None,
            pool: self.pool.clone(),
            verb_searcher: std::sync::Arc::new(searcher),
            lookup_service: None,
            policy_gate,
            source: crate::agent::orchestrator::UtteranceSource::Mcp,
        };
        let outcome = crate::agent::orchestrator::handle_utterance(&orch_ctx, instruction).await?;
        let result = outcome.pipeline_result;

        // Capture feedback for learning loop
        let _feedback_id = if let Some(ref feedback_svc) = self.feedback_service {
            let top_verb = result.verb_candidates.first();
            let match_result = top_verb.map(|v| ob_semantic_matcher::MatchResult {
                verb_name: v.verb.clone(),
                pattern_phrase: v.matched_phrase.clone(),
                similarity: v.score,
                match_method: ob_semantic_matcher::MatchMethod::Semantic,
                category: "mcp".to_string(),
                is_agent_bound: true,
            });

            match feedback_svc
                .capture_match(
                    session_id.unwrap_or_else(uuid::Uuid::new_v4),
                    instruction,
                    ob_semantic_matcher::feedback::InputSource::Command,
                    match_result.as_ref(),
                    &[], // alternatives
                    domain,
                    None, // workflow_phase
                )
                .await
            {
                Ok(interaction_id) => {
                    // Look up the feedback row ID for FK linking
                    sqlx::query_scalar::<_, i64>(
                        r#"SELECT id FROM "ob-poc".intent_feedback WHERE interaction_id = $1"#,
                    )
                    .bind(interaction_id)
                    .fetch_optional(&self.pool)
                    .await
                    .ok()
                    .flatten()
                }
                Err(e) => {
                    tracing::warn!("Failed to capture MCP feedback: {}", e);
                    None
                }
            }
        } else {
            None
        };

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
        let row = sqlx::query_as::<_, LearningCandidateUpsertRow>(
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
            RETURNING id, occurrence_count, (xmax = 0) as was_created
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

        let (candidate_id, occurrence_count, was_created) =
            (row.id, row.occurrence_count, row.was_created);

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
    // Helper: resolve CBU ID from UUID string or name
    pub(super) async fn resolve_cbu_id(&self, value: &str) -> Result<Uuid> {
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
    pub(super) async fn resolve_product_id(&self, value: &str) -> Result<Uuid> {
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
    pub(super) async fn resolve_service_id(&self, value: &str) -> Result<Uuid> {
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
