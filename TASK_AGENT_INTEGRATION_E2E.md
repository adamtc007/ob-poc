# TASK: Agent Generation Integration & End-to-End Test Harness

## Goal

1. Wire `GenerationLogRepository` into the agent DSL generation flow
2. Create an end-to-end test harness that validates the complete pipeline:
   - User intent → DSL generation → Parse → Lint → Compile → Execute → Persist → Log

---

## Part 1: Integration Points

### 1.1 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         AGENT FLOW                                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  User Intent ──→ [Session/Chat] ──→ DSL Generation ──→ Validation       │
│       │                                    │               │             │
│       │                                    ▼               ▼             │
│       │                            ┌──────────────┐  ┌──────────┐       │
│       │                            │ Parse        │  │ Lint     │       │
│       │                            └──────────────┘  └──────────┘       │
│       │                                    │               │             │
│       │                                    ▼               ▼             │
│       │                            ┌──────────────┐  ┌──────────┐       │
│       │                            │ Compile      │  │ Execute  │       │
│       │                            └──────────────┘  └──────────┘       │
│       │                                                    │             │
│       ▼                                                    ▼             │
│  ┌─────────────────┐                              ┌──────────────┐      │
│  │ generation_log  │◄─────── LOG ALL STEPS ──────│ dsl_instances│      │
│  └─────────────────┘                              └──────────────┘      │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Files to Modify

| File | Change |
|------|--------|
| `api/agent_routes.rs` | Add `GenerationLogRepository` to `AgentState`, wire logging into `execute_session_dsl` |
| `mcp/handlers.rs` | Wire logging into `dsl_execute` tool |
| `database/mod.rs` | Export generation log repository (if not already) |

---

## Part 2: Agent Routes Integration

### 2.1 Update AgentState

**File:** `rust/src/api/agent_routes.rs`

```rust
use crate::database::generation_log_repository::{
    GenerationLogRepository, GenerationAttempt, ParseResult, LintResult, CompileResult
};

#[derive(Clone)]
pub struct AgentState {
    pub pool: PgPool,
    pub dsl_v2_executor: Arc<DslExecutor>,
    pub sessions: SessionStore,
    pub generation_log: Arc<GenerationLogRepository>,  // ADD THIS
}

impl AgentState {
    pub fn new(pool: PgPool) -> Self {
        let dsl_v2_executor = Arc::new(DslExecutor::new(pool.clone()));
        let sessions = create_session_store();
        let generation_log = Arc::new(GenerationLogRepository::new(pool.clone()));  // ADD THIS
        Self {
            pool,
            dsl_v2_executor,
            sessions,
            generation_log,  // ADD THIS
        }
    }
}
```

### 2.2 Update execute_session_dsl Handler

**File:** `rust/src/api/agent_routes.rs`

Modify the `execute_session_dsl` function to log the generation:

```rust
/// POST /api/session/:id/execute - Execute accumulated DSL
async fn execute_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    // Get session and DSL
    let (dsl, context, current_state, user_intent) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        
        if session.assembled_dsl.is_empty() {
            return Ok(Json(ExecuteResponse {
                success: false,
                results: Vec::new(),
                errors: vec!["No DSL to execute".to_string()],
                new_state: session.state.clone(),
            }));
        }
        
        // Extract user intent from last user message
        let user_intent = session.messages.iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "Unknown intent".to_string());
        
        (
            session.combined_dsl(),
            session.context.clone(),
            session.state.clone(),
            user_intent,
        )
    };

    // =========================================================================
    // START GENERATION LOG
    // =========================================================================
    let log_id = state.generation_log.start_log(
        &user_intent,
        "session",  // domain_name - could extract from DSL
        Some(session_id),
        context.last_cbu_id,
        None,  // model_used - not applicable for direct DSL
    ).await.ok();

    // Build execution context
    let mut exec_ctx = ExecutionContext::new();
    if let Some(id) = context.last_cbu_id {
        exec_ctx.bind("last_cbu", id);
    }
    if let Some(id) = context.last_entity_id {
        exec_ctx.bind("last_entity", id);
    }

    // =========================================================================
    // PARSE
    // =========================================================================
    let start_time = std::time::Instant::now();
    let program = match parse_program(&dsl) {
        Ok(p) => p,
        Err(e) => {
            let parse_error = format!("Parse error: {}", e);
            
            // Log failed attempt
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(dsl.clone()),
                    parse_result: ParseResult { success: false, error: Some(parse_error.clone()) },
                    lint_result: LintResult { valid: false, errors: vec![], warnings: vec![] },
                    compile_result: CompileResult { success: false, error: None, step_count: 0 },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = state.generation_log.add_attempt(lid, &attempt).await;
                let _ = state.generation_log.mark_failed(lid).await;
            }
            
            return Ok(Json(ExecuteResponse {
                success: false,
                results: Vec::new(),
                errors: vec![parse_error],
                new_state: current_state,
            }));
        }
    };

    // =========================================================================
    // COMPILE (includes lint)
    // =========================================================================
    let plan = match compile(&program) {
        Ok(p) => p,
        Err(e) => {
            let compile_error = format!("Compile error: {}", e);
            
            // Log failed attempt
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(dsl.clone()),
                    parse_result: ParseResult { success: true, error: None },
                    lint_result: LintResult { valid: false, errors: vec![compile_error.clone()], warnings: vec![] },
                    compile_result: CompileResult { success: false, error: Some(compile_error.clone()), step_count: 0 },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = state.generation_log.add_attempt(lid, &attempt).await;
                let _ = state.generation_log.mark_failed(lid).await;
            }
            
            return Ok(Json(ExecuteResponse {
                success: false,
                results: Vec::new(),
                errors: vec![compile_error],
                new_state: current_state,
            }));
        }
    };

    // =========================================================================
    // EXECUTE
    // =========================================================================
    let mut results = Vec::new();
    let mut all_success = true;
    let mut errors = Vec::new();

    match state.dsl_v2_executor.execute_plan(&plan, &mut exec_ctx).await {
        Ok(exec_results) => {
            for (idx, exec_result) in exec_results.iter().enumerate() {
                let mut entity_id: Option<Uuid> = None;
                if let DslV2Result::Uuid(uuid) = exec_result {
                    entity_id = Some(*uuid);
                }

                results.push(ExecutionResult {
                    statement_index: idx,
                    dsl: dsl.clone(),
                    success: true,
                    message: "Executed successfully".to_string(),
                    entity_id,
                    entity_type: None,
                });
            }
            
            // =========================================================================
            // LOG SUCCESS
            // =========================================================================
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(dsl.clone()),
                    parse_result: ParseResult { success: true, error: None },
                    lint_result: LintResult { valid: true, errors: vec![], warnings: vec![] },
                    compile_result: CompileResult { 
                        success: true, 
                        error: None, 
                        step_count: plan.len() as i32 
                    },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = state.generation_log.add_attempt(lid, &attempt).await;
                let _ = state.generation_log.mark_success(lid, &dsl, None).await;
            }
        }
        Err(e) => {
            all_success = false;
            let error_msg = format!("Execution error: {}", e);
            errors.push(error_msg.clone());
            
            // Log execution failure
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(dsl.clone()),
                    parse_result: ParseResult { success: true, error: None },
                    lint_result: LintResult { valid: true, errors: vec![], warnings: vec![] },
                    compile_result: CompileResult { 
                        success: true, 
                        error: None, 
                        step_count: plan.len() as i32 
                    },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = state.generation_log.add_attempt(lid, &attempt).await;
                let _ = state.generation_log.mark_failed(lid).await;
            }
            
            results.push(ExecutionResult {
                statement_index: 0,
                dsl: dsl.clone(),
                success: false,
                message: error_msg,
                entity_id: None,
                entity_type: None,
            });
        }
    }

    // Update session state (existing code)
    // ...

    Ok(Json(ExecuteResponse {
        success: all_success,
        results,
        errors,
        new_state: SessionState::Executed,
    }))
}
```

### 2.3 Update MCP Handler

**File:** `rust/src/mcp/handlers.rs`

Similar integration for the `dsl_execute` tool:

```rust
use crate::database::generation_log_repository::{
    GenerationLogRepository, GenerationAttempt, ParseResult, LintResult, CompileResult
};

pub struct ToolHandlers {
    pool: PgPool,
    generation_log: GenerationLogRepository,  // ADD THIS
}

impl ToolHandlers {
    pub fn new(pool: PgPool) -> Self {
        Self {
            generation_log: GenerationLogRepository::new(pool.clone()),  // ADD THIS
            pool,
        }
    }

    /// Execute DSL against the database
    async fn dsl_execute(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;
        let dry_run = args["dry_run"].as_bool().unwrap_or(false);
        
        // Extract user_intent if provided, otherwise use source as intent
        let user_intent = args["intent"]
            .as_str()
            .unwrap_or("MCP tool execution");

        // Start generation log
        let log_id = self.generation_log.start_log(
            user_intent,
            "mcp",
            None,  // session_id
            None,  // cbu_id
            None,  // model
        ).await.ok();

        let start_time = std::time::Instant::now();

        // Parse
        let ast = match parse_program(source) {
            Ok(a) => a,
            Err(e) => {
                if let Some(lid) = log_id {
                    // Log parse failure
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(source.to_string()),
                        parse_result: ParseResult { 
                            success: false, 
                            error: Some(format!("{:?}", e)) 
                        },
                        lint_result: LintResult { valid: false, errors: vec![], warnings: vec![] },
                        compile_result: CompileResult { success: false, error: None, step_count: 0 },
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

        // Compile
        let plan = match compile(&ast) {
            Ok(p) => p,
            Err(e) => {
                if let Some(lid) = log_id {
                    // Log compile failure
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(source.to_string()),
                        parse_result: ParseResult { success: true, error: None },
                        lint_result: LintResult { 
                            valid: false, 
                            errors: vec![format!("{:?}", e)], 
                            warnings: vec![] 
                        },
                        compile_result: CompileResult { 
                            success: false, 
                            error: Some(format!("{:?}", e)), 
                            step_count: 0 
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
            // ... existing dry_run code ...
            return Ok(json!({ "success": true, "dry_run": true, ... }));
        }

        // Execute
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
                        parse_result: ParseResult { success: true, error: None },
                        lint_result: LintResult { valid: true, errors: vec![], warnings: vec![] },
                        compile_result: CompileResult { 
                            success: true, 
                            error: None, 
                            step_count: plan.len() as i32 
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
                    let _ = self.generation_log.mark_failed(lid).await;
                }
                
                Ok(json!({
                    "success": false,
                    "error": e.to_string()
                }))
            }
        }
    }
}
```

---

## Part 3: End-to-End Test Harness

### 3.1 Test Harness Structure

**File:** `rust/src/services/agent_e2e_test_harness.rs`

```rust
//! End-to-End Test Harness for Agent DSL Generation Pipeline
//!
//! Tests the complete flow:
//! 1. User intent → Session creation
//! 2. DSL assembly (simulated - no LLM)
//! 3. Parse → Lint → Compile → Execute
//! 4. Verify DB state (dsl_instances, generation_log)
//! 5. Cleanup

use crate::api::agent_routes::AgentState;
use crate::api::session::{AgentSession, SessionState, ExecutionResult};
use crate::database::dsl_repository::DslRepository;
use crate::database::generation_log_repository::GenerationLogRepository;
use crate::dsl_v2::{parse_program, compile, DslExecutor, ExecutionContext};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

/// End-to-end test scenario
#[derive(Debug, Clone)]
pub struct E2ETestScenario {
    pub name: String,
    pub description: String,
    /// The simulated user intent (natural language)
    pub user_intent: String,
    /// The DSL that would be generated (we skip LLM, provide directly)
    pub dsl: String,
    /// Expected number of execution steps
    pub expected_steps: usize,
    /// Expected entity types created
    pub expected_entities: Vec<&'static str>,
    /// Whether execution should succeed
    pub should_succeed: bool,
}

/// Test harness for end-to-end agent testing
pub struct AgentE2ETestHarness {
    pool: PgPool,
    dsl_repo: DslRepository,
    gen_log_repo: GenerationLogRepository,
    executor: DslExecutor,
    /// Track created resources for cleanup
    created_cbus: Vec<Uuid>,
    created_entities: Vec<Uuid>,
    created_log_ids: Vec<Uuid>,
}

impl AgentE2ETestHarness {
    pub fn new(pool: PgPool) -> Self {
        Self {
            dsl_repo: DslRepository::new(pool.clone()),
            gen_log_repo: GenerationLogRepository::new(pool.clone()),
            executor: DslExecutor::new(pool.clone()),
            pool,
            created_cbus: Vec::new(),
            created_entities: Vec::new(),
            created_log_ids: Vec::new(),
        }
    }

    /// Get predefined test scenarios
    pub fn get_test_scenarios() -> Vec<E2ETestScenario> {
        vec![
            // =========================================================
            // Scenario 1: Simple CBU Creation
            // =========================================================
            E2ETestScenario {
                name: "Simple CBU Creation".to_string(),
                description: "Create a basic hedge fund CBU".to_string(),
                user_intent: "Create a hedge fund called Alpha Capital".to_string(),
                dsl: r#"(cbu.create 
                    :name "Alpha Capital"
                    :client-type "HEDGE_FUND"
                    :jurisdiction "US")"#.to_string(),
                expected_steps: 1,
                expected_entities: vec!["CBU"],
                should_succeed: true,
            },

            // =========================================================
            // Scenario 2: CBU with Entity and Role
            // =========================================================
            E2ETestScenario {
                name: "CBU with Director".to_string(),
                description: "Create CBU and assign a director".to_string(),
                user_intent: "Create a fund called Beta Partners with John Smith as director".to_string(),
                dsl: r#"(cbu.create 
                    :name "Beta Partners"
                    :client-type "FUND"
                    :jurisdiction "UK"
                    :roles [
                        (cbu.assign-role 
                            :entity-id "11111111-1111-1111-1111-111111111111"
                            :role "Director")
                    ])"#.to_string(),
                expected_steps: 2,  // create + assign-role
                expected_entities: vec!["CBU"],
                should_succeed: false,  // Will fail - entity doesn't exist
            },

            // =========================================================
            // Scenario 3: Entity Creation
            // =========================================================
            E2ETestScenario {
                name: "Create Person Entity".to_string(),
                description: "Create a proper person entity".to_string(),
                user_intent: "Create a person named Jane Doe".to_string(),
                dsl: r#"(entity.create-proper-person
                    :first-name "Jane"
                    :last-name "Doe"
                    :date-of-birth "1985-06-15")"#.to_string(),
                expected_steps: 1,
                expected_entities: vec!["ENTITY"],
                should_succeed: true,
            },

            // =========================================================
            // Scenario 4: Parse Error
            // =========================================================
            E2ETestScenario {
                name: "Invalid DSL Syntax".to_string(),
                description: "Test parse error handling".to_string(),
                user_intent: "Create something broken".to_string(),
                dsl: r#"(cbu.create :name "Test" :missing-close-paren"#.to_string(),
                expected_steps: 0,
                expected_entities: vec![],
                should_succeed: false,
            },

            // =========================================================
            // Scenario 5: Unknown Verb
            // =========================================================
            E2ETestScenario {
                name: "Unknown Verb".to_string(),
                description: "Test compile error for unknown verb".to_string(),
                user_intent: "Do something impossible".to_string(),
                dsl: r#"(fake.unknown-verb :param "value")"#.to_string(),
                expected_steps: 0,
                expected_entities: vec![],
                should_succeed: false,
            },

            // =========================================================
            // Scenario 6: Multi-step Workflow
            // =========================================================
            E2ETestScenario {
                name: "Complete Onboarding Flow".to_string(),
                description: "Full onboarding with CBU, entity, document".to_string(),
                user_intent: "Onboard Gamma Holdings with CEO Mike Johnson and upload passport".to_string(),
                dsl: r#";; Complete onboarding workflow
(cbu.create 
    :name "Gamma Holdings"
    :client-type "COMPANY"
    :jurisdiction "DE"
    :as @gamma)

(entity.create-proper-person
    :first-name "Mike"
    :last-name "Johnson"
    :date-of-birth "1970-03-20"
    :as @mike)

(cbu.assign-role
    :cbu-id @gamma
    :entity-id @mike
    :role "CEO")"#.to_string(),
                expected_steps: 3,
                expected_entities: vec!["CBU", "ENTITY"],
                should_succeed: true,
            },
        ]
    }

    /// Run a single test scenario
    pub async fn run_scenario(&mut self, scenario: &E2ETestScenario) -> TestResult {
        println!("\n{'='*60}");
        println!("SCENARIO: {}", scenario.name);
        println!("{'='*60}");
        println!("Intent: {}", scenario.user_intent);
        println!("Expected success: {}", scenario.should_succeed);

        let mut result = TestResult::new(&scenario.name);

        // =====================================================================
        // STEP 1: Start generation log
        // =====================================================================
        println!("\n--- Step 1: Start Generation Log ---");
        
        let log_id = match self.gen_log_repo.start_log(
            &scenario.user_intent,
            "e2e_test",
            None,
            None,
            Some("test-harness"),
        ).await {
            Ok(id) => {
                println!("  ✓ Created generation log: {}", id);
                self.created_log_ids.push(id);
                result.add_step("Create generation log", true);
                Some(id)
            }
            Err(e) => {
                println!("  ✗ Failed to create log: {}", e);
                result.add_step("Create generation log", false);
                None
            }
        };

        // =====================================================================
        // STEP 2: Parse DSL
        // =====================================================================
        println!("\n--- Step 2: Parse DSL ---");
        
        let program = match parse_program(&scenario.dsl) {
            Ok(p) => {
                println!("  ✓ Parse succeeded");
                result.add_step("Parse DSL", true);
                Some(p)
            }
            Err(e) => {
                println!("  ✗ Parse failed: {:?}", e);
                result.add_step("Parse DSL", scenario.should_succeed == false);
                
                // Log parse failure
                if let Some(lid) = log_id {
                    let _ = self.gen_log_repo.mark_failed(lid).await;
                }
                
                if !scenario.should_succeed {
                    result.set_expected_failure("Parse error as expected");
                }
                return result;
            }
        };

        // =====================================================================
        // STEP 3: Compile
        // =====================================================================
        println!("\n--- Step 3: Compile ---");
        
        let plan = match compile(&program.unwrap()) {
            Ok(p) => {
                println!("  ✓ Compile succeeded: {} steps", p.len());
                result.add_step("Compile DSL", true);
                
                // Verify step count
                let step_match = p.len() == scenario.expected_steps;
                result.add_step(
                    &format!("Step count matches (expected {}, got {})", 
                             scenario.expected_steps, p.len()),
                    step_match
                );
                
                Some(p)
            }
            Err(e) => {
                println!("  ✗ Compile failed: {:?}", e);
                result.add_step("Compile DSL", scenario.should_succeed == false);
                
                if let Some(lid) = log_id {
                    let _ = self.gen_log_repo.mark_failed(lid).await;
                }
                
                if !scenario.should_succeed {
                    result.set_expected_failure("Compile error as expected");
                }
                return result;
            }
        };

        // =====================================================================
        // STEP 4: Execute
        // =====================================================================
        println!("\n--- Step 4: Execute ---");
        
        let mut ctx = ExecutionContext::new();
        
        match self.executor.execute_plan(&plan.unwrap(), &mut ctx).await {
            Ok(results) => {
                println!("  ✓ Execution succeeded: {} results", results.len());
                result.add_step("Execute DSL", scenario.should_succeed);
                
                // Track created entities for cleanup
                for exec_result in &results {
                    if let crate::dsl_v2::ExecutionResult::Uuid(id) = exec_result {
                        println!("    Created entity: {}", id);
                        // TODO: Determine if CBU or entity and add to appropriate list
                        self.created_cbus.push(*id);
                    }
                }
                
                // Log success
                if let Some(lid) = log_id {
                    let _ = self.gen_log_repo.mark_success(lid, &scenario.dsl, None).await;
                }
            }
            Err(e) => {
                println!("  ✗ Execution failed: {}", e);
                result.add_step("Execute DSL", !scenario.should_succeed);
                
                if let Some(lid) = log_id {
                    let _ = self.gen_log_repo.mark_failed(lid).await;
                }
                
                if !scenario.should_succeed {
                    result.set_expected_failure("Execution error as expected");
                }
            }
        }

        // =====================================================================
        // STEP 5: Verify generation log
        // =====================================================================
        println!("\n--- Step 5: Verify Generation Log ---");
        
        if let Some(lid) = log_id {
            // Query the log to verify it was written
            match sqlx::query!(
                r#"SELECT success, total_attempts, user_intent 
                   FROM "ob-poc".dsl_generation_log 
                   WHERE log_id = $1"#,
                lid
            )
            .fetch_optional(&self.pool)
            .await
            {
                Ok(Some(row)) => {
                    println!("  ✓ Generation log verified:");
                    println!("    - success: {}", row.success);
                    println!("    - attempts: {}", row.total_attempts);
                    println!("    - intent: {}", row.user_intent);
                    
                    let log_correct = row.success == scenario.should_succeed;
                    result.add_step("Generation log success flag matches", log_correct);
                }
                Ok(None) => {
                    println!("  ✗ Generation log not found");
                    result.add_step("Generation log exists", false);
                }
                Err(e) => {
                    println!("  ✗ Error querying log: {}", e);
                    result.add_step("Query generation log", false);
                }
            }
        }

        result
    }

    /// Run all test scenarios
    pub async fn run_all_scenarios(&mut self) -> Vec<TestResult> {
        let scenarios = Self::get_test_scenarios();
        let mut results = Vec::new();

        for scenario in &scenarios {
            let result = self.run_scenario(scenario).await;
            results.push(result);
        }

        results
    }

    /// Cleanup test data
    pub async fn cleanup(&self) -> Result<(), String> {
        println!("\n{'='*60}");
        println!("CLEANUP");
        println!("{'='*60}");

        // Delete generation logs
        for log_id in &self.created_log_ids {
            let result = sqlx::query!(
                r#"DELETE FROM "ob-poc".dsl_generation_log WHERE log_id = $1"#,
                log_id
            )
            .execute(&self.pool)
            .await;
            println!("  Delete log {}: {:?}", log_id, result.is_ok());
        }

        // Delete CBUs (will cascade to related data)
        for cbu_id in &self.created_cbus {
            let result = sqlx::query!(
                r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                cbu_id
            )
            .execute(&self.pool)
            .await;
            println!("  Delete CBU {}: {:?}", cbu_id, result.is_ok());
        }

        // Delete entities
        for entity_id in &self.created_entities {
            let result = sqlx::query!(
                r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#,
                entity_id
            )
            .execute(&self.pool)
            .await;
            println!("  Delete entity {}: {:?}", entity_id, result.is_ok());
        }

        println!("  Cleanup complete");
        Ok(())
    }

    /// Print summary of all test results
    pub fn print_summary(results: &[TestResult]) {
        println!("\n{'='*60}");
        println!("TEST SUMMARY");
        println!("{'='*60}");

        let mut total_passed = 0;
        let mut total_failed = 0;

        for result in results {
            let status = if result.all_passed() { "PASS" } else { "FAIL" };
            let icon = if result.all_passed() { "✓" } else { "✗" };
            println!("{} [{}] {}", icon, status, result.scenario_name);
            
            if result.expected_failure.is_some() {
                println!("    (expected failure: {})", result.expected_failure.as_ref().unwrap());
            }

            for (step_name, passed) in &result.steps {
                let step_icon = if *passed { "  ✓" } else { "  ✗" };
                println!("  {} {}", step_icon, step_name);
            }

            if result.all_passed() {
                total_passed += 1;
            } else {
                total_failed += 1;
            }
        }

        println!("\n{'-'*60}");
        println!("TOTAL: {} passed, {} failed", total_passed, total_failed);
        println!("{'='*60}");
    }
}

/// Result of a single test scenario
pub struct TestResult {
    pub scenario_name: String,
    pub steps: Vec<(String, bool)>,
    pub expected_failure: Option<String>,
}

impl TestResult {
    pub fn new(name: &str) -> Self {
        Self {
            scenario_name: name.to_string(),
            steps: Vec::new(),
            expected_failure: None,
        }
    }

    pub fn add_step(&mut self, name: &str, passed: bool) {
        self.steps.push((name.to_string(), passed));
    }

    pub fn set_expected_failure(&mut self, reason: &str) {
        self.expected_failure = Some(reason.to_string());
    }

    pub fn all_passed(&self) -> bool {
        self.steps.iter().all(|(_, passed)| *passed)
    }
}
```

### 3.2 Wire Into Services Module

**File:** `rust/src/services/mod.rs`

```rust
pub mod agent_e2e_test_harness;
pub use agent_e2e_test_harness::{AgentE2ETestHarness, E2ETestScenario, TestResult};
```

### 3.3 CLI Runner

**File:** `rust/src/bin/run_e2e_tests.rs`

```rust
//! End-to-End Test Runner
//!
//! Run with: cargo run --bin run_e2e_tests

use ob_poc::services::AgentE2ETestHarness;
use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenvy::dotenv().ok();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║     Agent E2E Test Harness                                 ║");
    println!("║     Testing: Intent → DSL → Parse → Compile → Execute → DB ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    // Connect to database
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    println!("\n✓ Connected to database");

    // Create harness
    let mut harness = AgentE2ETestHarness::new(pool);

    // Run all scenarios
    let results = harness.run_all_scenarios().await;

    // Print summary
    AgentE2ETestHarness::print_summary(&results);

    // Cleanup
    println!("\nRunning cleanup...");
    harness.cleanup().await?;

    // Exit with appropriate code
    let all_passed = results.iter().all(|r| r.all_passed());
    if all_passed {
        println!("\n✓ All tests passed!");
        Ok(())
    } else {
        println!("\n✗ Some tests failed!");
        std::process::exit(1);
    }
}
```

### 3.4 Add Binary to Cargo.toml

**File:** `rust/Cargo.toml`

Add under `[[bin]]`:

```toml
[[bin]]
name = "run_e2e_tests"
path = "src/bin/run_e2e_tests.rs"
```

---

## Part 4: Implementation Order

### Phase A: Repository Integration
1. [ ] Ensure `GenerationLogRepository` is in `database/mod.rs` exports
2. [ ] Update `AgentState` in `agent_routes.rs` to include `generation_log`
3. [ ] Update `execute_session_dsl` to log all outcomes
4. [ ] Update `ToolHandlers` in `mcp/handlers.rs` to log all outcomes

### Phase B: Test Harness
1. [ ] Create `services/agent_e2e_test_harness.rs`
2. [ ] Add to `services/mod.rs`
3. [ ] Create `bin/run_e2e_tests.rs`
4. [ ] Add binary to `Cargo.toml`

### Phase C: Run Tests
1. [ ] `cargo build --bin run_e2e_tests`
2. [ ] `cargo run --bin run_e2e_tests`
3. [ ] Verify generation_log table has entries
4. [ ] Verify cleanup works

---

## Part 5: Testing Commands

```bash
# Build the test runner
cargo build --bin run_e2e_tests

# Run tests
cargo run --bin run_e2e_tests

# Check generation logs were created
psql $DATABASE_URL -c "SELECT log_id, user_intent, success, total_attempts FROM \"ob-poc\".dsl_generation_log ORDER BY created_at DESC LIMIT 10;"

# Run with verbose output
RUST_LOG=debug cargo run --bin run_e2e_tests
```

---

## Part 6: Expected Output

```
╔════════════════════════════════════════════════════════════╗
║     Agent E2E Test Harness                                 ║
║     Testing: Intent → DSL → Parse → Compile → Execute → DB ║
╚════════════════════════════════════════════════════════════╝

✓ Connected to database

============================================================
SCENARIO: Simple CBU Creation
============================================================
Intent: Create a hedge fund called Alpha Capital
Expected success: true

--- Step 1: Start Generation Log ---
  ✓ Created generation log: abc123...

--- Step 2: Parse DSL ---
  ✓ Parse succeeded

--- Step 3: Compile ---
  ✓ Compile succeeded: 1 steps

--- Step 4: Execute ---
  ✓ Execution succeeded: 1 results
    Created entity: def456...

--- Step 5: Verify Generation Log ---
  ✓ Generation log verified:
    - success: true
    - attempts: 1
    - intent: Create a hedge fund called Alpha Capital

... more scenarios ...

============================================================
TEST SUMMARY
============================================================
✓ [PASS] Simple CBU Creation
  ✓ Create generation log
  ✓ Parse DSL
  ✓ Compile DSL
  ✓ Step count matches (expected 1, got 1)
  ✓ Execute DSL
  ✓ Generation log success flag matches
✓ [PASS] Invalid DSL Syntax
    (expected failure: Parse error as expected)
  ✓ Create generation log
  ✓ Parse DSL
... 

------------------------------------------------------------
TOTAL: 6 passed, 0 failed
============================================================

Running cleanup...
  Delete log abc123...: true
  Delete CBU def456...: true
  Cleanup complete

✓ All tests passed!
```

---

## Notes

- The test harness bypasses actual LLM calls — DSL is provided directly in scenarios
- This tests the "last mile": parse → compile → execute → persist → log
- For full agent testing (with LLM), create separate integration tests that call the LLM
- Cleanup is important — test data should not persist in the database
- The harness can be extended with more scenarios as verbs are added
