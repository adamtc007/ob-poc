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
            _ => Err(anyhow!("Unknown tool: {}", name)),
        }
    }

    /// Validate DSL source code
    async fn dsl_validate(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;

        let ast = match parse_program(source) {
            Ok(ast) => ast,
            Err(e) => {
                return Ok(json!({
                    "valid": false,
                    "errors": [{"type": "parse", "message": format!("{:?}", e)}]
                }))
            }
        };

        match compile(&ast) {
            Ok(plan) => Ok(json!({
                "valid": true,
                "step_count": plan.steps.len(),
                "steps": plan.steps.iter().enumerate().map(|(i, s)| {
                    json!({
                        "index": i,
                        "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb),
                        "binding": s.bind_as
                    })
                }).collect::<Vec<_>>()
            })),
            Err(e) => Ok(json!({
                "valid": false,
                "errors": [{"type": "compile", "message": format!("{:?}", e)}]
            })),
        }
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
                let products = vec![
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
}
