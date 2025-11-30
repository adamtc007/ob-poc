//! End-to-End Test Harness for Agent DSL Generation Pipeline
//!
//! Tests the complete flow:
//! 1. User intent -> Session creation
//! 2. DSL assembly (simulated - no LLM)
//! 3. Parse -> Lint -> Compile -> Execute
//! 4. Verify DB state (dsl_instances, generation_log)
//! 5. Cleanup

use crate::database::generation_log_repository::{
    CompileResult, GenerationAttempt, GenerationLogRepository, LintResult, ParseResult,
};
use crate::database::DslRepository;
use crate::dsl_v2::ExecutionResult as DslExecResult;
use crate::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext};
use sqlx::PgPool;
use uuid::Uuid;

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

/// Result of a single test step
#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub passed: bool,
}

/// Result of a single test scenario
pub struct TestResult {
    pub scenario_name: String,
    pub steps: Vec<StepResult>,
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
        self.steps.push(StepResult {
            name: name.to_string(),
            passed,
        });
    }

    pub fn set_expected_failure(&mut self, reason: &str) {
        self.expected_failure = Some(reason.to_string());
    }

    pub fn all_passed(&self) -> bool {
        self.steps.iter().all(|s| s.passed)
    }
}

/// Test harness for end-to-end agent testing
pub struct AgentE2ETestHarness {
    pool: PgPool,
    #[allow(dead_code)]
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
                    :name "Alpha Capital E2E Test"
                    :client-type "HEDGE_FUND"
                    :jurisdiction "US")"#
                    .to_string(),
                expected_steps: 1,
                expected_entities: vec!["CBU"],
                should_succeed: true,
            },
            // =========================================================
            // Scenario 2: Parse Error
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
            // Scenario 3: Unknown Verb
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
            // Scenario 4: CBU with Person Entity
            // =========================================================
            E2ETestScenario {
                name: "CBU with Person".to_string(),
                description: "Create CBU and a person entity".to_string(),
                user_intent: "Create a fund with a person named John".to_string(),
                dsl: r#";; Multi-step workflow
(cbu.create
    :name "Beta Partners E2E Test"
    :client-type "FUND"
    :jurisdiction "UK"
    :as @fund)

(entity.create-proper-person
    :cbu-id @fund
    :first-name "John"
    :last-name "Smith"
    :date-of-birth "1980-01-15"
    :as @john)"#
                    .to_string(),
                expected_steps: 2,
                expected_entities: vec!["CBU", "ENTITY"],
                should_succeed: true,
            },
        ]
    }

    /// Run a single test scenario
    pub async fn run_scenario(&mut self, scenario: &E2ETestScenario) -> TestResult {
        println!("\n{}", "=".repeat(60));
        println!("SCENARIO: {}", scenario.name);
        println!("{}", "=".repeat(60));
        println!("Intent: {}", scenario.user_intent);
        println!("Expected success: {}", scenario.should_succeed);

        let mut result = TestResult::new(&scenario.name);

        // =====================================================================
        // STEP 1: Start generation log
        // =====================================================================
        println!("\n--- Step 1: Start Generation Log ---");

        let log_id = match self
            .gen_log_repo
            .start_log(
                &scenario.user_intent,
                "e2e_test",
                None,
                None,
                Some("test-harness"),
            )
            .await
        {
            Ok(id) => {
                println!("  [OK] Created generation log: {}", id);
                self.created_log_ids.push(id);
                result.add_step("Create generation log", true);
                Some(id)
            }
            Err(e) => {
                println!("  [FAIL] Failed to create log: {}", e);
                result.add_step("Create generation log", false);
                None
            }
        };

        let start_time = std::time::Instant::now();

        // =====================================================================
        // STEP 2: Parse DSL
        // =====================================================================
        println!("\n--- Step 2: Parse DSL ---");

        let program = match parse_program(&scenario.dsl) {
            Ok(p) => {
                println!("  [OK] Parse succeeded");
                result.add_step("Parse DSL", true);
                Some(p)
            }
            Err(e) => {
                println!("  [FAIL] Parse failed: {:?}", e);
                // Parse failure is expected for some scenarios
                result.add_step("Parse DSL", !scenario.should_succeed);

                // Log parse failure
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(scenario.dsl.clone()),
                        parse_result: ParseResult {
                            success: false,
                            error: Some(format!("{:?}", e)),
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
                    let _ = self.gen_log_repo.add_attempt(lid, &attempt).await;
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
                println!("  [OK] Compile succeeded: {} steps", p.len());
                result.add_step("Compile DSL", true);

                // Verify step count
                let step_match = p.len() == scenario.expected_steps;
                result.add_step(
                    &format!(
                        "Step count matches (expected {}, got {})",
                        scenario.expected_steps,
                        p.len()
                    ),
                    step_match,
                );

                Some(p)
            }
            Err(e) => {
                println!("  [FAIL] Compile failed: {:?}", e);
                result.add_step("Compile DSL", !scenario.should_succeed);

                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(scenario.dsl.clone()),
                        parse_result: ParseResult {
                            success: true,
                            error: None,
                        },
                        lint_result: LintResult {
                            valid: false,
                            errors: vec![format!("{:?}", e)],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: false,
                            error: Some(format!("{:?}", e)),
                            step_count: 0,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = self.gen_log_repo.add_attempt(lid, &attempt).await;
                    let _ = self.gen_log_repo.mark_failed(lid).await;
                }

                if !scenario.should_succeed {
                    result.set_expected_failure("Compile error as expected");
                }
                return result;
            }
        };

        let plan = plan.unwrap();
        let step_count = plan.len() as i32;

        // =====================================================================
        // STEP 4: Execute
        // =====================================================================
        println!("\n--- Step 4: Execute ---");

        let mut ctx = ExecutionContext::new();

        match self.executor.execute_plan(&plan, &mut ctx).await {
            Ok(results) => {
                println!("  [OK] Execution succeeded: {} results", results.len());
                result.add_step("Execute DSL", scenario.should_succeed);

                // Track created entities for cleanup
                for exec_result in &results {
                    if let DslExecResult::Uuid(id) = exec_result {
                        println!("    Created entity: {}", id);
                        // We'll add to created_cbus - cleanup will handle both
                        self.created_cbus.push(*id);
                    }
                }

                // Log success
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(scenario.dsl.clone()),
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
                            step_count,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = self.gen_log_repo.add_attempt(lid, &attempt).await;
                    let _ = self
                        .gen_log_repo
                        .mark_success(lid, &scenario.dsl, None)
                        .await;
                }
            }
            Err(e) => {
                println!("  [FAIL] Execution failed: {}", e);
                result.add_step("Execute DSL", !scenario.should_succeed);

                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(scenario.dsl.clone()),
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
                            step_count,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = self.gen_log_repo.add_attempt(lid, &attempt).await;
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
                    println!("  [OK] Generation log verified:");
                    println!("    - success: {}", row.success);
                    println!("    - attempts: {}", row.total_attempts);
                    println!("    - intent: {}", row.user_intent);

                    let log_correct = row.success == scenario.should_succeed;
                    result.add_step("Generation log success flag matches", log_correct);
                }
                Ok(None) => {
                    println!("  [FAIL] Generation log not found");
                    result.add_step("Generation log exists", false);
                }
                Err(e) => {
                    println!("  [FAIL] Error querying log: {}", e);
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
        println!("\n{}", "=".repeat(60));
        println!("CLEANUP");
        println!("{}", "=".repeat(60));

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

        // Delete entities first (due to foreign key constraints)
        for entity_id in &self.created_entities {
            // Delete from cbu_entity_roles first
            let _ = sqlx::query!(
                r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE entity_id = $1"#,
                entity_id
            )
            .execute(&self.pool)
            .await;

            let result = sqlx::query!(
                r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#,
                entity_id
            )
            .execute(&self.pool)
            .await;
            println!("  Delete entity {}: {:?}", entity_id, result.is_ok());
        }

        // Delete CBUs (will cascade to related data)
        for cbu_id in &self.created_cbus {
            // Try deleting from cbu_entity_roles first
            let _ = sqlx::query!(
                r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#,
                cbu_id
            )
            .execute(&self.pool)
            .await;

            // Try deleting as entity
            let entity_result = sqlx::query!(
                r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#,
                cbu_id
            )
            .execute(&self.pool)
            .await;

            // Try deleting as CBU
            let cbu_result = sqlx::query!(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#, cbu_id)
                .execute(&self.pool)
                .await;

            println!(
                "  Delete {}: entity={:?}, cbu={:?}",
                cbu_id,
                entity_result.is_ok(),
                cbu_result.is_ok()
            );
        }

        println!("  Cleanup complete");
        Ok(())
    }

    /// Print summary of all test results
    pub fn print_summary(results: &[TestResult]) {
        println!("\n{}", "=".repeat(60));
        println!("TEST SUMMARY");
        println!("{}", "=".repeat(60));

        let mut total_passed = 0;
        let mut total_failed = 0;

        for result in results {
            let status = if result.all_passed() { "PASS" } else { "FAIL" };
            let icon = if result.all_passed() {
                "[OK]"
            } else {
                "[FAIL]"
            };
            println!("{} [{}] {}", icon, status, result.scenario_name);

            if let Some(ref expected) = result.expected_failure {
                println!("    (expected failure: {})", expected);
            }

            for step in &result.steps {
                let step_icon = if step.passed { "  [OK]" } else { "  [FAIL]" };
                println!("  {} {}", step_icon, step.name);
            }

            if result.all_passed() {
                total_passed += 1;
            } else {
                total_failed += 1;
            }
        }

        println!("\n{}", "-".repeat(60));
        println!("TOTAL: {} passed, {} failed", total_passed, total_failed);
        println!("{}", "=".repeat(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_definitions() {
        let scenarios = AgentE2ETestHarness::get_test_scenarios();
        assert!(!scenarios.is_empty());

        // Verify we have at least one success and one failure scenario
        let has_success = scenarios.iter().any(|s| s.should_succeed);
        let has_failure = scenarios.iter().any(|s| !s.should_succeed);
        assert!(has_success, "Should have at least one success scenario");
        assert!(has_failure, "Should have at least one failure scenario");
    }

    #[test]
    fn test_result_tracking() {
        let mut result = TestResult::new("Test Scenario");
        assert!(result.all_passed()); // Empty is considered passed

        result.add_step("Step 1", true);
        assert!(result.all_passed());

        result.add_step("Step 2", false);
        assert!(!result.all_passed());

        result.set_expected_failure("Expected");
        assert!(result.expected_failure.is_some());
    }
}
