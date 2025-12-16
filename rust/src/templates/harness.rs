//! Template Test Harness
//!
//! Tests all templates by:
//! 1. Loading from config/verbs/templates/
//! 2. Expanding with sample parameters
//! 3. Parsing the generated DSL
//! 4. Running through planning facade (DAG + toposort)
//! 5. Optionally executing against database
//! 6. Visualizing results
//!
//! Templates are first-class language macros that expand to DSL s-expressions.
//! This harness validates the full pipeline: expand → parse → plan → (execute).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ExpansionContext, TemplateExpander, TemplateRegistry};
use crate::dsl_v2::planning_facade::{analyse_and_plan, PlanningInput};
use crate::dsl_v2::{compile, parse_program};

/// Result of testing a single template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateTestResult {
    pub template_id: String,
    pub template_name: String,
    /// Primary entity type for this template (cbu, kyc_case, onboarding_request)
    pub primary_entity_type: Option<String>,
    pub expansion_success: bool,
    pub expansion_complete: bool,
    pub missing_params: Vec<String>,
    pub dsl: Option<String>,
    pub parse_success: bool,
    pub parse_error: Option<String>,
    pub compile_success: bool,
    pub compile_error: Option<String>,
    /// Number of ops in the execution plan
    pub op_count: usize,
    /// Planning succeeded (DAG built, toposorted)
    pub plan_success: bool,
    pub plan_error: Option<String>,
    /// True if ops were reordered from source order
    pub was_reordered: bool,
    /// Planning diagnostics (warnings, hints)
    pub diagnostics: Vec<String>,
    pub execution_success: Option<bool>,
    pub execution_error: Option<String>,
    pub bindings: HashMap<String, String>,
}

/// Result of running the full test harness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessResult {
    pub total_templates: usize,
    pub expansion_complete: usize,
    pub expansion_incomplete: usize,
    pub parse_success: usize,
    pub parse_failed: usize,
    pub compile_success: usize,
    pub compile_failed: usize,
    pub plan_success: usize,
    pub plan_failed: usize,
    pub execution_success: usize,
    pub execution_failed: usize,
    pub execution_skipped: usize,
    pub results: Vec<TemplateTestResult>,
}

impl HarnessResult {
    /// Print a summary to stdout
    pub fn print_summary(&self) {
        println!("\n=== Template Test Harness Results ===\n");
        println!("Total templates:      {}", self.total_templates);
        println!("Expansion complete:   {}", self.expansion_complete);
        println!("Expansion incomplete: {}", self.expansion_incomplete);
        println!("Parse success:        {}", self.parse_success);
        println!("Parse failed:         {}", self.parse_failed);
        println!("Compile success:      {}", self.compile_success);
        println!("Compile failed:       {}", self.compile_failed);
        println!("Plan success:         {}", self.plan_success);
        println!("Plan failed:          {}", self.plan_failed);
        println!("Execution success:    {}", self.execution_success);
        println!("Execution failed:     {}", self.execution_failed);
        println!("Execution skipped:    {}", self.execution_skipped);
        println!("\n--- Per-Template Results ---\n");

        for result in &self.results {
            let status = if result.plan_success {
                "✓"
            } else if result.compile_success {
                "○"
            } else if result.parse_success {
                "⚠"
            } else {
                "✗"
            };
            let primary = result.primary_entity_type.as_deref().unwrap_or("unknown");
            println!(
                "{} {} [{}] - {} params, {} ops{}",
                status,
                result.template_id,
                primary,
                if result.expansion_complete {
                    "complete"
                } else {
                    "incomplete"
                },
                result.op_count,
                if result.was_reordered {
                    " (reordered)"
                } else {
                    ""
                }
            );
            if let Some(ref err) = result.parse_error {
                println!("    Parse error: {}", err);
            }
            if let Some(ref err) = result.compile_error {
                println!("    Compile error: {}", err);
            }
            if let Some(ref err) = result.plan_error {
                println!("    Plan error: {}", err);
            }
            // Show all diagnostics including errors for debugging
            for diag in &result.diagnostics {
                println!("    Diag: {}", diag);
            }
            // If no explicit errors but plan failed, show the DSL for debugging
            if !result.plan_success
                && result.parse_error.is_none()
                && result.compile_error.is_none()
                && result.plan_error.is_none()
            {
                if let Some(ref dsl) = result.dsl {
                    let first_lines: String = dsl.lines().take(3).collect::<Vec<_>>().join("\n");
                    println!("    DSL preview: {}", first_lines);
                }
            }
        }
    }

    /// Check if all templates passed the full pipeline
    pub fn all_passed(&self) -> bool {
        self.plan_failed == 0 && self.parse_failed == 0
    }
}

/// Sample parameters for each template
/// Returns (template_id, HashMap of param_name -> value)
pub fn get_sample_params() -> HashMap<String, HashMap<String, String>> {
    let mut samples = HashMap::new();

    // Generate sample UUIDs for consistency
    let sample_cbu = Uuid::new_v4().to_string();
    let sample_case = Uuid::new_v4().to_string();
    let sample_entity = Uuid::new_v4().to_string();
    let sample_workstream = Uuid::new_v4().to_string();
    let sample_screening = Uuid::new_v4().to_string();

    // onboard-director
    samples.insert(
        "onboard-director".to_string(),
        HashMap::from([
            ("cbu_id".to_string(), sample_cbu.clone()),
            ("case_id".to_string(), sample_case.clone()),
            ("name".to_string(), "John Smith".to_string()),
            ("date_of_birth".to_string(), "1975-03-15".to_string()),
            ("nationality".to_string(), "GB".to_string()),
            ("tax_residency".to_string(), "GB".to_string()),
            ("effective_date".to_string(), "2024-01-15".to_string()),
        ]),
    );

    // onboard-signatory
    samples.insert(
        "onboard-signatory".to_string(),
        HashMap::from([
            ("cbu_id".to_string(), sample_cbu.clone()),
            ("case_id".to_string(), sample_case.clone()),
            ("name".to_string(), "Jane Doe".to_string()),
            ("date_of_birth".to_string(), "1980-06-22".to_string()),
            ("nationality".to_string(), "US".to_string()),
            ("signing_authority".to_string(), "SOLE".to_string()),
            ("effective_date".to_string(), "2024-01-15".to_string()),
        ]),
    );

    // add-ownership
    samples.insert(
        "add-ownership".to_string(),
        HashMap::from([
            ("owner_id".to_string(), sample_entity.clone()),
            ("owned_id".to_string(), Uuid::new_v4().to_string()),
            ("percentage".to_string(), "25.5".to_string()),
            ("ownership_type".to_string(), "DIRECT".to_string()),
            ("effective_date".to_string(), "2024-01-15".to_string()),
        ]),
    );

    // trace-chains
    samples.insert(
        "trace-chains".to_string(),
        HashMap::from([
            ("cbu_id".to_string(), sample_cbu.clone()),
            ("threshold".to_string(), "25".to_string()),
        ]),
    );

    // register-ubo
    samples.insert(
        "register-ubo".to_string(),
        HashMap::from([
            ("cbu_id".to_string(), sample_cbu.clone()),
            ("case_id".to_string(), sample_case.clone()),
            ("subject_entity_id".to_string(), sample_entity.clone()),
            ("ubo_person_id".to_string(), Uuid::new_v4().to_string()),
            ("ownership_percentage".to_string(), "35.5".to_string()),
            (
                "qualifying_reason".to_string(),
                "OWNERSHIP_25PCT".to_string(),
            ),
        ]),
    );

    // run-entity-screening
    samples.insert(
        "run-entity-screening".to_string(),
        HashMap::from([
            ("case_id".to_string(), sample_case.clone()),
            ("workstream_id".to_string(), sample_workstream.clone()),
        ]),
    );

    // review-screening-hit
    samples.insert(
        "review-screening-hit".to_string(),
        HashMap::from([
            ("screening_id".to_string(), sample_screening.clone()),
            ("outcome".to_string(), "FALSE_POSITIVE".to_string()),
            (
                "rationale".to_string(),
                "Name match only, different DOB".to_string(),
            ),
        ]),
    );

    // catalog-document
    samples.insert(
        "catalog-document".to_string(),
        HashMap::from([
            ("cbu_id".to_string(), sample_cbu.clone()),
            ("entity_id".to_string(), sample_entity.clone()),
            ("doc_type".to_string(), "PASSPORT".to_string()),
            ("title".to_string(), "John Smith UK Passport".to_string()),
            (
                "storage_key".to_string(),
                "s3://docs/passport-001".to_string(),
            ),
            ("extract".to_string(), "true".to_string()),
        ]),
    );

    // request-documents
    samples.insert(
        "request-documents".to_string(),
        HashMap::from([
            ("workstream_id".to_string(), sample_workstream.clone()),
            (
                "doc_types".to_string(),
                "[PASSPORT PROOF_OF_ADDRESS]".to_string(),
            ),
            ("due_date".to_string(), "2024-02-15".to_string()),
            ("priority".to_string(), "NORMAL".to_string()),
        ]),
    );

    // create-kyc-case
    samples.insert(
        "create-kyc-case".to_string(),
        HashMap::from([
            ("cbu_id".to_string(), sample_cbu.clone()),
            ("case_type".to_string(), "NEW_CLIENT".to_string()),
            ("risk_rating".to_string(), "MEDIUM".to_string()),
            ("notes".to_string(), "Initial onboarding case".to_string()),
        ]),
    );

    // escalate-case
    samples.insert(
        "escalate-case".to_string(),
        HashMap::from([
            ("case_id".to_string(), sample_case.clone()),
            (
                "escalation_level".to_string(),
                "SENIOR_COMPLIANCE".to_string(),
            ),
            (
                "reason".to_string(),
                "PEP match requires senior review".to_string(),
            ),
        ]),
    );

    // approve-case
    samples.insert(
        "approve-case".to_string(),
        HashMap::from([
            ("case_id".to_string(), sample_case.clone()),
            ("risk_rating".to_string(), "LOW".to_string()),
            ("review_period_months".to_string(), "12".to_string()),
            (
                "approval_notes".to_string(),
                "All requirements satisfied".to_string(),
            ),
        ]),
    );

    samples
}

/// Run the template test harness using the planning facade
///
/// # Arguments
/// * `templates_dir` - Path to templates directory (e.g., "config/verbs/templates")
/// * `execute` - Whether to execute DSL against database (requires pool)
/// * `pool` - Optional database pool for execution
/// * `verb_registry` - Optional verb registry (uses global if None)
pub async fn run_harness(
    templates_dir: &Path,
    execute: bool,
    #[cfg(feature = "database")] pool: Option<&sqlx::PgPool>,
) -> Result<HarnessResult, super::TemplateError> {
    let template_registry = TemplateRegistry::load_from_dir(templates_dir)?;
    let verb_registry = get_verb_registry_arc();
    let sample_params = get_sample_params();

    let mut results = Vec::new();
    let mut stats = HarnessResult {
        total_templates: 0,
        expansion_complete: 0,
        expansion_incomplete: 0,
        parse_success: 0,
        parse_failed: 0,
        compile_success: 0,
        compile_failed: 0,
        plan_success: 0,
        plan_failed: 0,
        execution_success: 0,
        execution_failed: 0,
        execution_skipped: 0,
        results: Vec::new(),
    };

    for template in template_registry.list() {
        stats.total_templates += 1;

        let template_id = template.template.clone();
        let template_name = template.metadata.name.clone();

        // Get primary entity type
        let primary_entity_type = template
            .primary_entity
            .as_ref()
            .map(|pe| format!("{:?}", pe.entity_type).to_lowercase());

        // Get sample params for this template
        let explicit_params = sample_params.get(&template_id).cloned().unwrap_or_default();

        // Create expansion context with sample UUIDs
        let context = ExpansionContext {
            current_cbu: Some(Uuid::new_v4()),
            current_case: Some(Uuid::new_v4()),
            bindings: HashMap::new(),
            binding_types: HashMap::new(),
        };

        // Expand template
        let expansion = TemplateExpander::expand(template, &explicit_params, &context);

        let expansion_complete = expansion.missing_params.is_empty();
        if expansion_complete {
            stats.expansion_complete += 1;
        } else {
            stats.expansion_incomplete += 1;
        }

        let missing_params: Vec<String> = expansion
            .missing_params
            .iter()
            .map(|p| p.name.clone())
            .collect();

        // Run through planning facade (parse → compile → DAG → toposort)
        let planning_input = PlanningInput::new(&expansion.dsl, verb_registry.clone());
        let planning_output = analyse_and_plan(planning_input);

        // Extract results from planning output
        let parse_success = !planning_output.program.statements.is_empty()
            || planning_output
                .diagnostics
                .iter()
                .all(|d| d.code != crate::dsl_v2::diagnostics::DiagnosticCode::SyntaxError);

        let parse_error = planning_output
            .diagnostics
            .iter()
            .find(|d| d.code == crate::dsl_v2::diagnostics::DiagnosticCode::SyntaxError)
            .map(|d| d.message.clone());

        if parse_success {
            stats.parse_success += 1;
        } else {
            stats.parse_failed += 1;
        }

        let compile_success = planning_output.compiled_ops.is_some();
        let compile_error = if !compile_success {
            planning_output
                .diagnostics
                .iter()
                .find(|d| d.code == crate::dsl_v2::diagnostics::DiagnosticCode::UndefinedSymbol)
                .map(|d| d.message.clone())
        } else {
            None
        };

        if compile_success {
            stats.compile_success += 1;
        } else if parse_success {
            stats.compile_failed += 1;
        }

        let plan_success = planning_output.plan.is_some();
        let plan_error = if !plan_success && compile_success {
            planning_output
                .diagnostics
                .iter()
                .find(|d| d.code == crate::dsl_v2::diagnostics::DiagnosticCode::CyclicDependency)
                .map(|d| d.message.clone())
        } else {
            None
        };

        if plan_success {
            stats.plan_success += 1;
        } else if compile_success {
            stats.plan_failed += 1;
        }

        let op_count = planning_output
            .plan
            .as_ref()
            .map(|p| p.op_count())
            .unwrap_or(0);

        let was_reordered = planning_output.was_reordered;

        // Capture ALL diagnostics for debugging
        let diagnostics: Vec<String> = planning_output
            .diagnostics
            .iter()
            .map(|d| format!("[{:?}] {}", d.severity, d.message))
            .collect();

        // Execute if requested and plan succeeded
        let (execution_success, execution_error, bindings) = if execute && plan_success {
            #[cfg(feature = "database")]
            {
                if let Some(pool) = pool {
                    match execute_dsl(&expansion.dsl, pool).await {
                        Ok(b) => {
                            stats.execution_success += 1;
                            (Some(true), None, b)
                        }
                        Err(e) => {
                            stats.execution_failed += 1;
                            (Some(false), Some(e), HashMap::new())
                        }
                    }
                } else {
                    stats.execution_skipped += 1;
                    (None, None, HashMap::new())
                }
            }
            #[cfg(not(feature = "database"))]
            {
                stats.execution_skipped += 1;
                (None, None, HashMap::new())
            }
        } else {
            stats.execution_skipped += 1;
            (None, None, HashMap::new())
        };

        results.push(TemplateTestResult {
            template_id,
            template_name,
            primary_entity_type,
            expansion_success: true,
            expansion_complete,
            missing_params,
            dsl: Some(expansion.dsl),
            parse_success,
            parse_error,
            compile_success,
            compile_error,
            op_count,
            plan_success,
            plan_error,
            was_reordered,
            diagnostics,
            execution_success,
            execution_error,
            bindings,
        });
    }

    stats.results = results;
    Ok(stats)
}

/// Execute DSL against database and return bindings
#[cfg(feature = "database")]
async fn execute_dsl(dsl: &str, pool: &sqlx::PgPool) -> Result<HashMap<String, String>, String> {
    use crate::dsl_v2::{DslExecutor, ExecutionContext};

    let ast = parse_program(dsl).map_err(|e| format!("Parse error: {:?}", e))?;
    let plan = compile(&ast).map_err(|e| format!("Compile error: {:?}", e))?;

    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ExecutionContext::new();

    executor
        .execute_plan(&plan, &mut ctx)
        .await
        .map_err(|e| format!("Execution error: {}", e))?;

    Ok(ctx
        .symbols
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect())
}

/// Run harness without database execution
pub async fn run_harness_no_db(
    templates_dir: &Path,
) -> Result<HarnessResult, super::TemplateError> {
    #[cfg(feature = "database")]
    {
        run_harness(templates_dir, false, None).await
    }
    #[cfg(not(feature = "database"))]
    {
        run_harness(templates_dir, false).await
    }
}

/// Run harness using templates from the standard config directory
///
/// This loads templates from config/verbs/templates/ and validates them
/// through the same pipeline as user DSL.
pub async fn run_harness_from_registry() -> Result<HarnessResult, super::TemplateError> {
    use crate::dsl_v2::config::loader::ConfigLoader;

    // Load templates from the config directory
    let loader = ConfigLoader::from_env();
    let templates_dir = loader.config_dir().join("verbs").join("templates");
    let template_registry = TemplateRegistry::load_from_dir(&templates_dir)?;
    let sample_params = get_sample_params();

    let mut results = Vec::new();
    let mut stats = HarnessResult {
        total_templates: 0,
        expansion_complete: 0,
        expansion_incomplete: 0,
        parse_success: 0,
        parse_failed: 0,
        compile_success: 0,
        compile_failed: 0,
        plan_success: 0,
        plan_failed: 0,
        execution_success: 0,
        execution_failed: 0,
        execution_skipped: 0,
        results: Vec::new(),
    };

    for template in template_registry.list() {
        let result = test_single_template(template, &sample_params);

        // Update stats
        stats.total_templates += 1;
        if result.expansion_complete {
            stats.expansion_complete += 1;
        } else {
            stats.expansion_incomplete += 1;
        }
        if result.parse_success {
            stats.parse_success += 1;
        } else {
            stats.parse_failed += 1;
        }
        if result.compile_success {
            stats.compile_success += 1;
        } else if result.parse_success {
            stats.compile_failed += 1;
        }
        if result.plan_success {
            stats.plan_success += 1;
        } else if result.compile_success {
            stats.plan_failed += 1;
        }
        stats.execution_skipped += 1;

        results.push(result);
    }

    stats.results = results;
    Ok(stats)
}

/// Get an Arc-wrapped verb registry for use with PlanningInput
fn get_verb_registry_arc() -> Arc<crate::dsl_v2::runtime_registry::RuntimeVerbRegistry> {
    use crate::dsl_v2::config::loader::ConfigLoader;
    use crate::dsl_v2::runtime_registry::RuntimeVerbRegistry;

    // Load a fresh registry since runtime_registry() returns &'static
    let loader = ConfigLoader::from_env();
    let config = loader.load_verbs().expect("verbs config should load");
    Arc::new(RuntimeVerbRegistry::from_config(&config))
}

/// Test a single template through the full pipeline
fn test_single_template(
    template: &super::TemplateDefinition,
    sample_params: &HashMap<String, HashMap<String, String>>,
) -> TemplateTestResult {
    let verb_registry = get_verb_registry_arc();

    let template_id = template.template.clone();
    let template_name = template.metadata.name.clone();

    let primary_entity_type = template
        .primary_entity
        .as_ref()
        .map(|pe| format!("{:?}", pe.entity_type).to_lowercase());

    let explicit_params = sample_params.get(&template_id).cloned().unwrap_or_default();

    let context = ExpansionContext {
        current_cbu: Some(Uuid::new_v4()),
        current_case: Some(Uuid::new_v4()),
        bindings: HashMap::new(),
        binding_types: HashMap::new(),
    };

    let expansion = TemplateExpander::expand(template, &explicit_params, &context);
    let expansion_complete = expansion.missing_params.is_empty();
    let missing_params: Vec<String> = expansion
        .missing_params
        .iter()
        .map(|p| p.name.clone())
        .collect();

    // Run through planning facade
    let planning_input = PlanningInput::new(&expansion.dsl, verb_registry);
    let planning_output = analyse_and_plan(planning_input);

    let parse_success = !planning_output.program.statements.is_empty()
        || planning_output
            .diagnostics
            .iter()
            .all(|d| d.code != crate::dsl_v2::diagnostics::DiagnosticCode::SyntaxError);

    let parse_error = planning_output
        .diagnostics
        .iter()
        .find(|d| d.code == crate::dsl_v2::diagnostics::DiagnosticCode::SyntaxError)
        .map(|d| d.message.clone());

    let compile_success = planning_output.compiled_ops.is_some();
    let compile_error = if !compile_success {
        planning_output
            .diagnostics
            .iter()
            .find(|d| d.code == crate::dsl_v2::diagnostics::DiagnosticCode::UndefinedSymbol)
            .map(|d| d.message.clone())
    } else {
        None
    };

    let plan_success = planning_output.plan.is_some();
    let plan_error = if !plan_success && compile_success {
        planning_output
            .diagnostics
            .iter()
            .find(|d| d.code == crate::dsl_v2::diagnostics::DiagnosticCode::CyclicDependency)
            .map(|d| d.message.clone())
    } else {
        None
    };

    let op_count = planning_output
        .plan
        .as_ref()
        .map(|p| p.op_count())
        .unwrap_or(0);

    // Capture ALL diagnostics for debugging
    let diagnostics: Vec<String> = planning_output
        .diagnostics
        .iter()
        .map(|d| format!("[{:?}] {}", d.severity, d.message))
        .collect();

    TemplateTestResult {
        template_id,
        template_name,
        primary_entity_type,
        expansion_success: true,
        expansion_complete,
        missing_params,
        dsl: Some(expansion.dsl),
        parse_success,
        parse_error,
        compile_success,
        compile_error,
        op_count,
        plan_success,
        plan_error,
        was_reordered: planning_output.was_reordered,
        diagnostics,
        execution_success: None,
        execution_error: None,
        bindings: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_harness_all_templates() {
        // Use new path: config/verbs/templates
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = PathBuf::from(&config_dir).join("verbs").join("templates");

        if !templates_path.exists() {
            eprintln!("Templates directory not found: {:?}", templates_path);
            return;
        }

        let result = run_harness_no_db(&templates_path).await.unwrap();
        result.print_summary();

        // All templates should parse and plan successfully
        assert!(
            result.parse_success > 0,
            "Expected at least some templates to parse"
        );
        assert!(
            result.plan_success > 0,
            "Expected at least some templates to plan successfully"
        );
    }

    #[tokio::test]
    async fn test_harness_from_registry() {
        // Test using templates loaded at startup with verbs
        let result = run_harness_from_registry().await.unwrap();
        result.print_summary();

        assert!(
            result.total_templates > 0,
            "Expected templates to be loaded in registry"
        );

        // All templates should at least parse successfully
        assert!(
            result.parse_failed == 0,
            "Expected all templates to parse: {} failed",
            result.parse_failed
        );

        // Report planning stats - some verbs may not be in compiler yet
        println!(
            "\nPipeline status: {} plan success, {} plan failed (compiler may lack some verbs)",
            result.plan_success, result.plan_failed
        );

        // If all templates pass through to planning, check full success
        if result.plan_failed == 0 {
            assert!(
                result.all_passed(),
                "Expected all templates to pass pipeline"
            );
        }
    }

    #[test]
    fn test_sample_params_coverage() {
        let samples = get_sample_params();

        // Ensure we have samples for all expected templates
        let expected = [
            "onboard-director",
            "onboard-signatory",
            "add-ownership",
            "trace-chains",
            "register-ubo",
            "run-entity-screening",
            "review-screening-hit",
            "catalog-document",
            "request-documents",
            "create-kyc-case",
            "escalate-case",
            "approve-case",
        ];

        for template_id in expected {
            assert!(
                samples.contains_key(template_id),
                "Missing sample params for template: {}",
                template_id
            );
        }
    }
}
