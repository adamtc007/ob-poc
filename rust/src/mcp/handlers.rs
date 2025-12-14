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

use crate::database::generation_log_repository::{
    CompileResult, GenerationAttempt, GenerationLogRepository, LintResult, ParseResult,
};
use crate::database::VisualizationRepository;
use crate::dsl_v2::{
    compile, gateway_resolver, parse_program, registry, DslExecutor, ExecutionContext,
};

use super::protocol::ToolCallResult;

/// Tool handlers with database access and EntityGateway client
pub struct ToolHandlers {
    pool: PgPool,
    generation_log: GenerationLogRepository,
    repo: VisualizationRepository,
    /// EntityGateway client for all entity lookups (lazy-initialized)
    gateway_client: Arc<Mutex<Option<EntityGatewayClient<Channel>>>>,
}

impl ToolHandlers {
    pub fn new(pool: PgPool) -> Self {
        Self {
            generation_log: GenerationLogRepository::new(pool.clone()),
            repo: VisualizationRepository::new(pool.clone()),
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
        }
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
        use super::types::{AgentDiagnostic, ResolutionOption, SuggestedFix, ValidationOutput};
        use crate::dsl_v2::config::ConfigLoader;
        use crate::dsl_v2::planning_facade::{analyse_and_plan, PlanningInput};
        use crate::dsl_v2::runtime_registry::RuntimeVerbRegistry;
        use std::sync::Arc;

        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;

        // Get session bindings if provided (for future use with known_symbols)
        let session_id = args["session_id"].as_str();
        let _binding_context = session_id.and_then(super::session::get_session_bindings);

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
                    severity: d.severity.into(),
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

        Ok(serde_json::to_value(validation_output).unwrap())
    }

    /// Execute DSL against the database
    async fn dsl_execute(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;
        let dry_run = args["dry_run"].as_bool().unwrap_or(false);

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

                Ok(json!({
                    "success": true,
                    "steps_executed": results.len(),
                    "bindings": bindings
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
        use super::session;
        use super::types::SessionAction;

        let action: SessionAction =
            serde_json::from_value(args).map_err(|e| anyhow!("Invalid session action: {}", e))?;

        let state = session::session_context(action).map_err(|e| anyhow!("{}", e))?;

        Ok(serde_json::to_value(state).unwrap())
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
        use super::enrichment::{EntityEnricher, EntityType as EnrichEntityType};
        use super::resolution::{ConversationContext, EnrichedMatch, ResolutionStrategy};

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
            let result = super::resolution::ResolutionResult {
                confidence: super::resolution::ResolutionConfidence::None,
                action: super::resolution::SuggestedAction::SuggestCreate,
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
}
