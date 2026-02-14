//! Agentic Scenario Harness — xtask runner
//!
//! Drives multi-turn YAML scenarios through the orchestrator pipeline.
//! Asserts on structured fields only (outcome kind, verb, SemReg, trace flags).
//!
//! # Usage
//! ```
//! cargo x harness list
//! cargo x harness run --all --mode stub
//! cargo x harness run --suite scenarios/suites/governance_strict.yaml
//! cargo x harness run --scenario "direct_dsl_denied_viewer"
//! cargo x harness dump --scenario "direct_dsl_denied_viewer" --out /tmp/run.json
//! ```

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use ob_poc::agent::harness::{load_all_suites, load_suite, ScenarioSuite};
use ob_poc::agent::harness::runner::{
    dump_failures, print_suite_report, run_suite, SuiteResult,
};

/// List all suites and their scenario counts.
pub fn list(scenarios_dir: &Path) -> Result<()> {
    let suites = load_all_suites(scenarios_dir)
        .context("Failed to load scenario suites")?;

    if suites.is_empty() {
        println!("No suites found in {}", scenarios_dir.display());
        return Ok(());
    }

    println!("Agentic Scenario Harness — {} suites\n", suites.len());
    let mut total_scenarios = 0;
    for suite in &suites {
        let count = suite.scenarios.len();
        total_scenarios += count;
        println!("  {:30} {:>3} scenarios  ({})", suite.name, count, suite.suite_id);
    }
    println!("\n  Total: {} scenarios across {} suites", total_scenarios, suites.len());
    Ok(())
}

/// Run scenarios with a database pool.
pub async fn run(
    pool: &sqlx::PgPool,
    scenarios_dir: &Path,
    suite_path: Option<&Path>,
    scenario_name: Option<&str>,
    all: bool,
) -> Result<bool> {
    let suites: Vec<ScenarioSuite> = if let Some(path) = suite_path {
        vec![load_suite(path).context("Failed to load suite")?]
    } else if all {
        load_all_suites(scenarios_dir).context("Failed to load suites")?
    } else if scenario_name.is_some() {
        load_all_suites(scenarios_dir).context("Failed to load suites")?
    } else {
        anyhow::bail!("Specify --suite, --scenario, or --all");
    };

    let failure_dir = PathBuf::from("target/harness_failures");
    let mut all_passed = true;
    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut suite_results: Vec<SuiteResult> = Vec::new();

    for suite in &suites {
        // If filtering by scenario name, check if this suite contains it
        if let Some(name) = scenario_name {
            if !suite.scenarios.iter().any(|s| s.name == name) {
                continue;
            }
        }

        let result = if let Some(name) = scenario_name {
            // Run only the matching scenario
            let scenario = suite.scenarios.iter().find(|s| s.name == name).unwrap();
            let sr = ob_poc::agent::harness::runner::run_scenario_stub(pool, suite, scenario).await;
            let passed = sr.passed;
            SuiteResult {
                suite_name: suite.name.clone(),
                scenarios_passed: if passed { 1 } else { 0 },
                scenarios_failed: if passed { 0 } else { 1 },
                scenarios_total: 1,
                results: vec![sr],
            }
        } else {
            run_suite(pool, suite).await
        };

        total_passed += result.scenarios_passed;
        total_failed += result.scenarios_failed;
        if !result.passed() {
            all_passed = false;
            if let Err(e) = dump_failures(&result, &failure_dir) {
                eprintln!("Warning: failed to dump artifacts: {}", e);
            }
        }
        print_suite_report(&result);
        suite_results.push(result);
    }

    println!("\n========================================");
    println!(
        "TOTAL: {}/{} scenarios passed",
        total_passed,
        total_passed + total_failed
    );
    if !all_passed {
        println!("Failure artifacts in: {}", failure_dir.display());
    }
    println!("========================================");

    Ok(all_passed)
}

/// Dump full artifacts for a specific scenario.
pub async fn dump(
    pool: &sqlx::PgPool,
    scenarios_dir: &Path,
    scenario_name: &str,
    out_path: &Path,
) -> Result<()> {
    let suites = load_all_suites(scenarios_dir)?;

    for suite in &suites {
        if let Some(scenario) = suite.scenarios.iter().find(|s| s.name == scenario_name) {
            let result = ob_poc::agent::harness::runner::run_scenario_stub(pool, suite, scenario).await;
            let json = serde_json::to_string_pretty(&result)?;
            std::fs::write(out_path, &json)?;
            println!("Dumped scenario '{}' to {}", scenario_name, out_path.display());
            return Ok(());
        }
    }

    anyhow::bail!("Scenario '{}' not found in any suite", scenario_name);
}
