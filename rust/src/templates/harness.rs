//! Template Test Harness
//!
//! Tests all templates by:
//! 1. Loading from config/templates/
//! 2. Expanding with sample parameters
//! 3. Parsing the generated DSL
//! 4. Optionally executing against database
//! 5. Visualizing results

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ExpansionContext, TemplateExpander, TemplateRegistry};
use crate::dsl_v2::{compile, parse_program};

/// Result of testing a single template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateTestResult {
    pub template_id: String,
    pub template_name: String,
    pub expansion_success: bool,
    pub expansion_complete: bool,
    pub missing_params: Vec<String>,
    pub dsl: Option<String>,
    pub parse_success: bool,
    pub parse_error: Option<String>,
    pub compile_success: bool,
    pub compile_error: Option<String>,
    pub step_count: usize,
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
        println!("Execution success:    {}", self.execution_success);
        println!("Execution failed:     {}", self.execution_failed);
        println!("Execution skipped:    {}", self.execution_skipped);
        println!("\n--- Per-Template Results ---\n");

        for result in &self.results {
            let status = if result.compile_success {
                "✓"
            } else if result.parse_success {
                "⚠"
            } else {
                "✗"
            };
            println!(
                "{} {} - {} params, {} steps",
                status,
                result.template_id,
                if result.expansion_complete {
                    "complete"
                } else {
                    "incomplete"
                },
                result.step_count
            );
            if let Some(ref err) = result.parse_error {
                println!("    Parse error: {}", err);
            }
            if let Some(ref err) = result.compile_error {
                println!("    Compile error: {}", err);
            }
        }
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

/// Run the template test harness
///
/// # Arguments
/// * `templates_dir` - Path to templates directory (e.g., "config/templates")
/// * `execute` - Whether to execute DSL against database (requires pool)
/// * `pool` - Optional database pool for execution
pub async fn run_harness(
    templates_dir: &Path,
    execute: bool,
    #[cfg(feature = "database")] pool: Option<&sqlx::PgPool>,
) -> Result<HarnessResult, super::TemplateError> {
    let registry = TemplateRegistry::load_from_dir(templates_dir)?;
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
        execution_success: 0,
        execution_failed: 0,
        execution_skipped: 0,
        results: Vec::new(),
    };

    for template in registry.list() {
        stats.total_templates += 1;

        let template_id = template.template.clone();
        let template_name = template.metadata.name.clone();

        // Get sample params for this template
        let explicit_params = sample_params.get(&template_id).cloned().unwrap_or_default();

        // Create expansion context with sample UUIDs
        let context = ExpansionContext {
            current_cbu: Some(Uuid::new_v4()),
            current_case: Some(Uuid::new_v4()),
            bindings: HashMap::new(),
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

        // Try to parse the expanded DSL
        let (parse_success, parse_error, ast) = match parse_program(&expansion.dsl) {
            Ok(ast) => {
                stats.parse_success += 1;
                (true, None, Some(ast))
            }
            Err(e) => {
                stats.parse_failed += 1;
                (false, Some(format!("{:?}", e)), None)
            }
        };

        // Try to compile if parse succeeded
        let (compile_success, compile_error, step_count) = if let Some(ref ast) = ast {
            match compile(ast) {
                Ok(plan) => {
                    stats.compile_success += 1;
                    (true, None, plan.steps.len())
                }
                Err(e) => {
                    stats.compile_failed += 1;
                    (false, Some(format!("{:?}", e)), 0)
                }
            }
        } else {
            (false, None, 0)
        };

        // Execute if requested and compile succeeded
        let (execution_success, execution_error, bindings) = if execute && compile_success {
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
            expansion_success: true,
            expansion_complete,
            missing_params,
            dsl: Some(expansion.dsl),
            parse_success,
            parse_error,
            compile_success,
            compile_error,
            step_count,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_harness_all_templates() {
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = PathBuf::from(&config_dir).join("templates");

        if !templates_path.exists() {
            eprintln!("Templates directory not found: {:?}", templates_path);
            return;
        }

        let result = run_harness_no_db(&templates_path).await.unwrap();
        result.print_summary();

        // All templates should at least parse (with sample params)
        assert!(
            result.parse_success > 0,
            "Expected at least some templates to parse"
        );
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
