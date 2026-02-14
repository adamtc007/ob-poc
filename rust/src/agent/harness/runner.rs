//! Multi-turn scenario runner.
//!
//! Drives the orchestrator through scenario steps, tracking run_sheet deltas,
//! asserting structured invariants, and collecting artifacts for debugging.

use serde::Serialize;
use std::path::Path;

use super::assertions::{check_step, AssertionFailure};
use super::{Scenario, ScenarioSuite, StepExpectation};
use crate::session::unified::UnifiedSession;

/// Result of running a single scenario.
#[derive(Debug, Serialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub suite_name: String,
    pub passed: bool,
    pub steps_passed: usize,
    pub steps_failed: usize,
    pub steps_total: usize,
    pub step_results: Vec<StepResult>,
}

/// Result of a single step within a scenario.
#[derive(Debug, Serialize)]
pub struct StepResult {
    pub step_index: usize,
    pub utterance: String,
    pub passed: bool,
    pub actual_outcome: String,
    pub failures: Vec<AssertionFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_json: Option<String>,
}

/// Result of running an entire suite.
#[derive(Debug, Serialize)]
pub struct SuiteResult {
    pub suite_name: String,
    pub scenarios_passed: usize,
    pub scenarios_failed: usize,
    pub scenarios_total: usize,
    pub results: Vec<ScenarioResult>,
}

impl SuiteResult {
    pub fn passed(&self) -> bool {
        self.scenarios_failed == 0
    }
}

/// Run a single scenario against the orchestrator in stub mode.
#[cfg(feature = "database")]
pub async fn run_scenario_stub(
    pool: &sqlx::PgPool,
    suite: &ScenarioSuite,
    scenario: &Scenario,
) -> ScenarioResult {
    use crate::agent::harness::stub::build_stub_context;
    use crate::agent::orchestrator;
    use crate::mcp::intent_pipeline::PipelineOutcome;

    let session = UnifiedSession::new();
    let ctx = build_stub_context(pool, session.id, &suite.session_seed, &suite.mode_expectations);

    let mut step_results = Vec::new();
    let mut steps_passed = 0;
    let mut steps_failed = 0;

    for (idx, step) in scenario.steps.iter().enumerate() {
        let prev_entry_count = session.run_sheet.entries.len();

        // Call the orchestrator
        let outcome = match orchestrator::handle_utterance(&ctx, &step.user).await {
            Ok(o) => o,
            Err(e) => {
                step_results.push(StepResult {
                    step_index: idx,
                    utterance: step.user.clone(),
                    passed: false,
                    actual_outcome: format!("ERROR: {}", e),
                    failures: vec![AssertionFailure {
                        field: "orchestrator_error".into(),
                        expected: "Ok(...)".into(),
                        actual: format!("{}", e),
                    }],
                    trace_json: None,
                });
                steps_failed += 1;
                continue;
            }
        };

        let actual_outcome = format!("{:?}", outcome.pipeline_result.outcome);
        let trace_json = serde_json::to_string_pretty(&outcome.trace).ok();

        // Check assertions
        let failures = check_step(&step.expect, &outcome, &session, prev_entry_count);
        let passed = failures.is_empty();

        if passed {
            steps_passed += 1;
        } else {
            steps_failed += 1;
        }

        step_results.push(StepResult {
            step_index: idx,
            utterance: step.user.clone(),
            passed,
            actual_outcome: actual_outcome.clone(),
            failures,
            trace_json,
        });

        // Handle interactive outcomes with on_outcome handlers
        if let Some(ref handlers) = step.on_outcome {
            let outcome_key = match &outcome.pipeline_result.outcome {
                PipelineOutcome::NeedsClarification => "ClarifyVerb",
                PipelineOutcome::NeedsUserInput => "ClarifyArgs",
                PipelineOutcome::ScopeCandidates => "ScopeClarify",
                _ => continue,
            };

            if let Some(_handler) = handlers.get(outcome_key) {
                // In stub mode with minimal searcher, interactive outcomes need
                // forced verb path. For now, log that we'd handle it.
                tracing::debug!(
                    outcome = outcome_key,
                    scenario = %scenario.name,
                    step = idx,
                    "Interactive outcome handler registered (stub mode)"
                );
                // In live mode, this would call handle_utterance_with_forced_verb
            }
        }
    }

    let all_passed = steps_failed == 0;

    ScenarioResult {
        scenario_name: scenario.name.clone(),
        suite_name: suite.name.clone(),
        passed: all_passed,
        steps_passed,
        steps_failed,
        steps_total: scenario.steps.len(),
        step_results,
    }
}

/// Run all scenarios in a suite.
#[cfg(feature = "database")]
pub async fn run_suite(pool: &sqlx::PgPool, suite: &ScenarioSuite) -> SuiteResult {
    let mut results = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    for scenario in &suite.scenarios {
        let result = run_scenario_stub(pool, suite, scenario).await;
        if result.passed {
            passed += 1;
        } else {
            failed += 1;
        }
        results.push(result);
    }

    SuiteResult {
        suite_name: suite.name.clone(),
        scenarios_passed: passed,
        scenarios_failed: failed,
        scenarios_total: suite.scenarios.len(),
        results,
    }
}

/// Dump failure artifacts to disk for debugging.
pub fn dump_failures(suite_result: &SuiteResult, base_dir: &Path) -> std::io::Result<()> {
    for scenario in &suite_result.results {
        if scenario.passed {
            continue;
        }
        for step in &scenario.step_results {
            if step.passed {
                continue;
            }
            let dir = base_dir
                .join(&suite_result.suite_name)
                .join(&scenario.scenario_name);
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("step_{}.json", step.step_index));
            let json = serde_json::to_string_pretty(step)
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e));
            std::fs::write(path, json)?;
        }
    }
    Ok(())
}

/// Print a summary report for a suite run.
pub fn print_suite_report(result: &SuiteResult) {
    let status = if result.passed() { "PASS" } else { "FAIL" };
    println!(
        "\n{} {} — {}/{} scenarios passed",
        status, result.suite_name, result.scenarios_passed, result.scenarios_total
    );

    for scenario in &result.results {
        let icon = if scenario.passed { "  +" } else { "  -" };
        println!(
            "{} {} ({}/{} steps)",
            icon, scenario.scenario_name, scenario.steps_passed, scenario.steps_total
        );

        if !scenario.passed {
            for step in &scenario.step_results {
                if !step.passed {
                    println!("      Step {}: \"{}\"", step.step_index, step.utterance);
                    for f in &step.failures {
                        println!("        {} {}", "x", f);
                    }
                }
            }
        }
    }
}
