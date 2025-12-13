//! Lightweight DSL API server for test harness integration.
//!
//! Endpoints:
//! - GET  /health              - Health check
//! - GET  /verbs               - List available verbs
//! - POST /validate            - Validate DSL (parse + compile)
//! - POST /execute             - Execute DSL
//! - GET  /query/cbus          - List CBUs
//! - GET  /query/cbus/:id      - Get CBU with full details
//! - GET  /query/kyc/cases/:id - Get KYC case with details
//! - DELETE /cleanup/cbu/:id   - Delete CBU and cascade

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use ob_poc::dsl_v2::{
    compile, parse_program, verb_registry::registry, DslExecutor, ExecutionContext,
    ExecutionResult as DslResult,
};

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    executor: Arc<DslExecutor>,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    verb_count: usize,
}

#[derive(Serialize)]
struct VerbResponse {
    domain: String,
    name: String,
    full_name: String,
    description: String,
    required_args: Vec<String>,
    optional_args: Vec<String>,
}

#[derive(Serialize)]
struct VerbsResponse {
    verbs: Vec<VerbResponse>,
    total: usize,
}

#[derive(Deserialize)]
struct ValidateRequest {
    dsl: String,
}

#[derive(Serialize)]
struct ValidationError {
    message: String,
}

#[derive(Serialize)]
struct ValidateResponse {
    valid: bool,
    errors: Vec<ValidationError>,
}

#[derive(Deserialize)]
struct ExecuteRequest {
    dsl: String,
    #[serde(default)]
    bindings: Option<std::collections::HashMap<String, Uuid>>,
}

#[derive(Serialize)]
struct ExecuteResultItem {
    statement_index: usize,
    success: bool,
    message: String,
    entity_id: Option<Uuid>,
}

#[derive(Serialize)]
struct ExecuteResponse {
    success: bool,
    results: Vec<ExecuteResultItem>,
    bindings: std::collections::HashMap<String, Uuid>,
    errors: Vec<String>,
}

#[derive(Deserialize)]
struct AnalyzeErrorRequest {
    dsl: String,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct ErrorSuggestion {
    error_type: String,
    original_value: Option<String>,
    suggested_value: Option<String>,
    available_values: Vec<String>,
    fix_description: String,
}

#[derive(Serialize)]
struct AnalyzeErrorResponse {
    suggestions: Vec<ErrorSuggestion>,
    corrected_dsl: Option<String>,
}

// Request/Response for validate-with-fixes
#[derive(Deserialize)]
struct ValidateWithFixesRequest {
    dsl: String,
}

/// Threshold for auto-fixing without asking user
const AUTO_FIX_THRESHOLD: f32 = 0.90;
/// Threshold for suggesting (below this, don't even suggest)
const SUGGEST_THRESHOLD: f32 = 0.60;

#[derive(Serialize, Clone)]
struct LookupCorrection {
    line: usize,
    arg_name: String,
    current_value: String,
    suggested_value: String,
    available_values: Vec<String>,
    confidence: f32, // 0.0 - 1.0, based on string similarity
    /// "auto_fixed" = applied automatically, "needs_confirmation" = ask user
    action: String,
}

#[derive(Serialize, Clone)]
struct VerbCorrection {
    line: usize,
    current_verb: String,
    suggested_verb: String,
    available_verbs: Vec<String>,
    confidence: f32,
    /// "auto_fixed" = applied automatically, "needs_confirmation" = ask user
    action: String,
}

#[derive(Serialize)]
struct ValidateWithFixesResponse {
    valid: bool,
    parse_error: Option<String>,
    compile_error: Option<String>,
    lookup_corrections: Vec<LookupCorrection>,
    verb_corrections: Vec<VerbCorrection>,
    corrected_dsl: Option<String>,
    /// "valid" = no issues, "auto_fixed" = fixed automatically,
    /// "needs_confirmation" = user must choose, "unfixable" = no good suggestions
    status: String,
    /// Human-readable message for the agent to relay to user
    message: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

async fn health() -> Json<HealthResponse> {
    let reg = registry();
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        verb_count: reg.len(),
    })
}

async fn list_verbs() -> Json<VerbsResponse> {
    let reg = registry();
    let mut verbs = Vec::new();

    for domain in reg.domains() {
        for verb in reg.verbs_for_domain(domain) {
            verbs.push(VerbResponse {
                domain: verb.domain.to_string(),
                name: verb.verb.to_string(),
                full_name: format!("{}.{}", verb.domain, verb.verb),
                description: verb.description.to_string(),
                required_args: verb
                    .required_arg_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                optional_args: verb
                    .optional_arg_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            });
        }
    }

    let total = verbs.len();
    Json(VerbsResponse { verbs, total })
}

async fn validate(Json(req): Json<ValidateRequest>) -> Json<ValidateResponse> {
    // Parse
    let program = match parse_program(&req.dsl) {
        Ok(p) => p,
        Err(e) => {
            return Json(ValidateResponse {
                valid: false,
                errors: vec![ValidationError {
                    message: format!("Parse error: {}", e),
                }],
            });
        }
    };

    // Compile (includes validation)
    match compile(&program) {
        Ok(_) => Json(ValidateResponse {
            valid: true,
            errors: vec![],
        }),
        Err(e) => Json(ValidateResponse {
            valid: false,
            errors: vec![ValidationError {
                message: format!("Compile error: {}", e),
            }],
        }),
    }
}

async fn execute(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    // Parse
    let program = match parse_program(&req.dsl) {
        Ok(p) => p,
        Err(e) => {
            return Json(ExecuteResponse {
                success: false,
                results: vec![],
                bindings: std::collections::HashMap::new(),
                errors: vec![format!("Parse error: {}", e)],
            });
        }
    };

    // Compile
    let plan = match compile(&program) {
        Ok(p) => p,
        Err(e) => {
            return Json(ExecuteResponse {
                success: false,
                results: vec![],
                bindings: std::collections::HashMap::new(),
                errors: vec![format!("Compile error: {}", e)],
            });
        }
    };

    // Execute
    let mut ctx = ExecutionContext::new().with_audit_user("dsl_api");

    // Pre-bind any symbols passed from previous executions
    if let Some(bindings) = &req.bindings {
        for (name, id) in bindings {
            ctx.bind(name, *id);
        }
    }

    match state.executor.execute_plan(&plan, &mut ctx).await {
        Ok(results) => {
            let items: Vec<ExecuteResultItem> = results
                .iter()
                .enumerate()
                .map(|(idx, r)| {
                    let entity_id = match r {
                        DslResult::Uuid(id) => Some(*id),
                        _ => None,
                    };
                    ExecuteResultItem {
                        statement_index: idx,
                        success: true,
                        message: format!("{:?}", r),
                        entity_id,
                    }
                })
                .collect();

            Json(ExecuteResponse {
                success: true,
                results: items,
                bindings: ctx.symbols.clone(),
                errors: vec![],
            })
        }
        Err(e) => Json(ExecuteResponse {
            success: false,
            results: vec![],
            bindings: ctx.symbols.clone(),
            errors: vec![e.to_string()],
        }),
    }
}

/// Analyze DSL errors and provide structured suggestions for fixes.
/// Parses error messages to extract "did you mean?" suggestions and
/// can optionally produce a corrected DSL with the suggested fixes applied.
async fn analyze_errors(Json(req): Json<AnalyzeErrorRequest>) -> Json<AnalyzeErrorResponse> {
    use regex::Regex;

    let mut suggestions = Vec::new();
    let mut corrected_dsl = req.dsl.clone();

    // Pattern: "Lookup failed: no {table} with {column} = '{value}' in {schema}.{table}\n  Did you mean: {suggestion}?\n  Available values: {values}"
    let lookup_pattern = Regex::new(
        r"Lookup failed: no (\w+) with (\w+) = '([^']+)' in [^.]+\.\w+\n\s+Did you mean: ([^?]+)\?\n\s+Available values: (.+)"
    ).unwrap();

    // Pattern for unknown verb
    let verb_pattern = Regex::new(r"Unknown verb: ([a-z-]+\.[a-z-]+)").unwrap();

    // Pattern for undefined symbol
    let symbol_pattern = Regex::new(r"Unresolved reference: @(\w+)").unwrap();

    for error in &req.errors {
        // Check for lookup failure with suggestions
        if let Some(caps) = lookup_pattern.captures(error) {
            let table = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let original = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let suggested = caps.get(4).map(|m| m.as_str()).unwrap_or("");
            let available = caps.get(5).map(|m| m.as_str()).unwrap_or("");

            let available_values: Vec<String> = available
                .split(", ")
                .map(|s| s.trim().to_string())
                .collect();

            suggestions.push(ErrorSuggestion {
                error_type: "lookup_failed".to_string(),
                original_value: Some(original.to_string()),
                suggested_value: Some(suggested.to_string()),
                available_values: available_values.clone(),
                fix_description: format!(
                    "Replace '{}' with '{}' (valid {} values: {})",
                    original, suggested, table, available
                ),
            });

            // Apply fix to DSL
            corrected_dsl =
                corrected_dsl.replace(&format!("\"{}\"", original), &format!("\"{}\"", suggested));
        }
        // Check for unknown verb
        else if let Some(caps) = verb_pattern.captures(error) {
            let verb = caps.get(1).map(|m| m.as_str()).unwrap_or("");

            suggestions.push(ErrorSuggestion {
                error_type: "unknown_verb".to_string(),
                original_value: Some(verb.to_string()),
                suggested_value: None,
                available_values: vec![],
                fix_description: format!(
                    "Verb '{}' does not exist. Check /verbs endpoint for available verbs.",
                    verb
                ),
            });
        }
        // Check for undefined symbol
        else if let Some(caps) = symbol_pattern.captures(error) {
            let symbol = caps.get(1).map(|m| m.as_str()).unwrap_or("");

            suggestions.push(ErrorSuggestion {
                error_type: "undefined_symbol".to_string(),
                original_value: Some(format!("@{}", symbol)),
                suggested_value: None,
                available_values: vec![],
                fix_description: format!(
                    "Symbol '@{}' is not defined. Make sure it's created with ':as @{}' before being used.",
                    symbol, symbol
                ),
            });
        }
        // Generic error - no suggestions
        else {
            suggestions.push(ErrorSuggestion {
                error_type: "other".to_string(),
                original_value: None,
                suggested_value: None,
                available_values: vec![],
                fix_description: error.clone(),
            });
        }
    }

    // Only return corrected DSL if we made changes
    let corrected = if corrected_dsl != req.dsl {
        Some(corrected_dsl)
    } else {
        None
    };

    Json(AnalyzeErrorResponse {
        suggestions,
        corrected_dsl: corrected,
    })
}

/// Validate DSL and provide auto-corrections for lookup values.
/// This is a proactive validation that checks lookup args against the database
/// BEFORE execution, and returns a corrected DSL if possible.
///
/// Hybrid approach:
/// - confidence >= 0.90: auto-fix without asking
/// - confidence 0.60-0.90: ask user to confirm
/// - confidence < 0.60: no suggestion (unfixable)
async fn validate_with_fixes(
    State(state): State<AppState>,
    Json(req): Json<ValidateWithFixesRequest>,
) -> Json<ValidateWithFixesResponse> {
    use ob_poc::dsl_v2::runtime_registry::runtime_registry;

    // Step 1: Parse
    let program = match parse_program(&req.dsl) {
        Ok(p) => p,
        Err(e) => {
            return Json(ValidateWithFixesResponse {
                valid: false,
                parse_error: Some(e.to_string()),
                compile_error: None,
                lookup_corrections: vec![],
                verb_corrections: vec![],
                corrected_dsl: None,
                status: "unfixable".to_string(),
                message: Some(format!("Parse error: {}", e)),
            });
        }
    };

    // Step 2: Check for unknown verbs and collect lookup args
    let reg = registry();
    let runtime_reg = runtime_registry();
    let mut verb_corrections = Vec::new();
    let mut lookup_corrections = Vec::new();

    for (line_num, stmt) in program.statements.iter().enumerate() {
        let line_num = line_num + 1; // 1-indexed
        if let ob_poc::dsl_v2::ast::Statement::VerbCall(vc) = stmt {
            let full_verb = format!("{}.{}", vc.domain, vc.verb);

            // Check if verb exists
            if reg.get(&vc.domain, &vc.verb).is_none() {
                // Get suggestions for unknown verb
                let available: Vec<String> = reg
                    .verbs_for_domain(&vc.domain)
                    .iter()
                    .map(|v| format!("{}.{}", v.domain, v.verb))
                    .collect();

                let (suggested, confidence) = if !available.is_empty() {
                    let verb_names: Vec<&str> = available
                        .iter()
                        .filter_map(|s| s.split('.').nth(1))
                        .collect();
                    find_closest_with_score(
                        &vc.verb,
                        &verb_names.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    )
                    .map(|(s, c)| (format!("{}.{}", vc.domain, s), c))
                    .unwrap_or_default()
                } else {
                    (String::new(), 0.0)
                };

                let action = if confidence >= AUTO_FIX_THRESHOLD {
                    "auto_fixed"
                } else if confidence >= SUGGEST_THRESHOLD {
                    "needs_confirmation"
                } else {
                    "unfixable"
                };

                verb_corrections.push(VerbCorrection {
                    line: line_num,
                    current_verb: full_verb,
                    suggested_verb: suggested,
                    available_verbs: available,
                    confidence,
                    action: action.to_string(),
                });
                continue;
            }

            // Get verb definition to find lookup args
            if let Some(runtime_verb) = runtime_reg.get(&vc.domain, &vc.verb) {
                for arg in &runtime_verb.args {
                    // Check if this arg has a lookup configuration
                    if let Some(lookup_config) = &arg.lookup {
                        // Find this arg in the verb call
                        if let Some(call_arg) = vc.arguments.iter().find(|a| a.key == arg.name) {
                            if let Some(value) = call_arg.value.as_string() {
                                // Query DB for valid values
                                let valid_values = get_lookup_values(
                                    &state.pool,
                                    lookup_config.schema.as_deref().unwrap_or("ob-poc"),
                                    &lookup_config.table,
                                    lookup_config.search_key.primary_column(),
                                )
                                .await;

                                // Check if value is valid
                                if !valid_values.iter().any(|v| v.eq_ignore_ascii_case(value)) {
                                    // Find closest match
                                    if let Some((suggested, confidence)) =
                                        find_closest_with_score(value, &valid_values)
                                    {
                                        let action = if confidence >= AUTO_FIX_THRESHOLD {
                                            "auto_fixed"
                                        } else if confidence >= SUGGEST_THRESHOLD {
                                            "needs_confirmation"
                                        } else {
                                            "unfixable"
                                        };

                                        lookup_corrections.push(LookupCorrection {
                                            line: line_num,
                                            arg_name: arg.name.clone(),
                                            current_value: value.to_string(),
                                            suggested_value: suggested.clone(),
                                            available_values: valid_values.clone(),
                                            confidence,
                                            action: action.to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Step 3: Determine status and build corrected DSL
    // Only apply auto-fixes (confidence >= 0.90)
    let auto_fixed_lookups: Vec<&LookupCorrection> = lookup_corrections
        .iter()
        .filter(|c| c.action == "auto_fixed")
        .collect();
    let auto_fixed_verbs: Vec<&VerbCorrection> = verb_corrections
        .iter()
        .filter(|c| c.action == "auto_fixed")
        .collect();

    let needs_confirmation_lookups: Vec<&LookupCorrection> = lookup_corrections
        .iter()
        .filter(|c| c.action == "needs_confirmation")
        .collect();
    let needs_confirmation_verbs: Vec<&VerbCorrection> = verb_corrections
        .iter()
        .filter(|c| c.action == "needs_confirmation")
        .collect();

    let unfixable_lookups: Vec<&LookupCorrection> = lookup_corrections
        .iter()
        .filter(|c| c.action == "unfixable")
        .collect();
    let unfixable_verbs: Vec<&VerbCorrection> = verb_corrections
        .iter()
        .filter(|c| c.action == "unfixable")
        .collect();

    // Build corrected DSL with only auto-fixes applied
    let mut corrected_dsl = req.dsl.clone();
    for correction in &auto_fixed_lookups {
        corrected_dsl = corrected_dsl.replace(
            &format!("\"{}\"", correction.current_value),
            &format!("\"{}\"", correction.suggested_value),
        );
    }
    for correction in &auto_fixed_verbs {
        if !correction.suggested_verb.is_empty() {
            corrected_dsl = corrected_dsl.replace(
                &format!("({}", correction.current_verb),
                &format!("({}", correction.suggested_verb),
            );
        }
    }

    // Determine overall status
    let has_unfixable = !unfixable_lookups.is_empty() || !unfixable_verbs.is_empty();
    let has_needs_confirmation =
        !needs_confirmation_lookups.is_empty() || !needs_confirmation_verbs.is_empty();
    let has_auto_fixed = !auto_fixed_lookups.is_empty() || !auto_fixed_verbs.is_empty();
    let has_any_issues = !lookup_corrections.is_empty() || !verb_corrections.is_empty();

    let (status, message) = if !has_any_issues {
        // No issues found - DSL is valid
        ("valid".to_string(), None)
    } else if has_unfixable {
        // Some issues can't be fixed
        let unfixable_items: Vec<String> = unfixable_lookups
            .iter()
            .map(|c| format!("'{}' (no good match found)", c.current_value))
            .chain(
                unfixable_verbs
                    .iter()
                    .map(|c| format!("verb '{}' (unknown)", c.current_verb)),
            )
            .collect();
        (
            "unfixable".to_string(),
            Some(format!(
                "Cannot fix: {}. Please check the available values.",
                unfixable_items.join(", ")
            )),
        )
    } else if has_needs_confirmation {
        // Some fixes need user confirmation
        let mut suggestions: Vec<String> = Vec::new();
        for c in &needs_confirmation_lookups {
            suggestions.push(format!(
                "'{}' → '{}' ({:.0}% match)",
                c.current_value,
                c.suggested_value,
                c.confidence * 100.0
            ));
        }
        for c in &needs_confirmation_verbs {
            suggestions.push(format!(
                "verb '{}' → '{}' ({:.0}% match)",
                c.current_verb,
                c.suggested_verb,
                c.confidence * 100.0
            ));
        }
        (
            "needs_confirmation".to_string(),
            Some(format!(
                "Please confirm these changes: {}",
                suggestions.join("; ")
            )),
        )
    } else if has_auto_fixed {
        // All fixes were high-confidence auto-fixes
        let fixes: Vec<String> = auto_fixed_lookups
            .iter()
            .map(|c| format!("'{}' → '{}'", c.current_value, c.suggested_value))
            .chain(
                auto_fixed_verbs
                    .iter()
                    .map(|c| format!("'{}' → '{}'", c.current_verb, c.suggested_verb)),
            )
            .collect();
        (
            "auto_fixed".to_string(),
            Some(format!("Auto-corrected: {}", fixes.join(", "))),
        )
    } else {
        ("valid".to_string(), None)
    };

    // Verify corrected DSL compiles (only if we made auto-fixes)
    let final_valid = if has_auto_fixed && !has_needs_confirmation && !has_unfixable {
        match parse_program(&corrected_dsl) {
            Ok(p) => compile(&p).is_ok(),
            Err(_) => false,
        }
    } else if !has_any_issues {
        // Original DSL was valid
        compile(&program).is_ok()
    } else {
        false
    };

    Json(ValidateWithFixesResponse {
        valid: final_valid,
        parse_error: None,
        compile_error: if !verb_corrections.is_empty()
            && verb_corrections.iter().any(|v| v.action == "unfixable")
        {
            Some(format!(
                "Unknown verb(s): {}",
                verb_corrections
                    .iter()
                    .filter(|v| v.action == "unfixable")
                    .map(|v| v.current_verb.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        } else {
            None
        },
        lookup_corrections,
        verb_corrections,
        corrected_dsl: if has_any_issues {
            Some(corrected_dsl)
        } else {
            None
        },
        status,
        message,
    })
}

/// Query database for valid lookup values (search_key only, for simple lookups)
async fn get_lookup_values(
    pool: &PgPool,
    schema: &str,
    table: &str,
    code_column: &str,
) -> Vec<String> {
    let sql = format!(
        r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" IS NOT NULL ORDER BY "{}""#,
        code_column, schema, table, code_column, code_column
    );

    sqlx::query_scalar::<_, String>(&sql)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
}

/// Query database for fuzzy matches using pg_trgm (for large datasets)
/// Returns top matches with similarity scores
#[allow(dead_code)]
async fn get_fuzzy_matches_from_db(
    pool: &PgPool,
    schema: &str,
    table: &str,
    code_column: &str,
    target: &str,
    limit: i32,
) -> Vec<(String, f32)> {
    // Use pg_trgm similarity function for DB-side fuzzy matching
    // This is efficient for large datasets (1000+ rows)
    let sql = format!(
        r#"SELECT "{}", similarity("{}", $1) as score
           FROM "{}"."{}"
           WHERE "{}" % $1
           ORDER BY score DESC
           LIMIT $2"#,
        code_column, code_column, schema, table, code_column
    );

    let results: Vec<(String, f32)> = sqlx::query_as(&sql)
        .bind(target)
        .bind(limit)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    results
}

/// Minimum similarity threshold for suggestions (0.0 - 1.0)
const SIMILARITY_THRESHOLD: f64 = 0.6;

/// Maximum candidates for in-memory fuzzy matching
/// For larger sets, we use DB-side trigram matching instead
#[allow(dead_code)]
const MAX_FUZZY_CANDIDATES: usize = 500;

/// Find the closest matching string using Jaro-Winkler similarity
#[allow(dead_code)]
fn find_closest_match(target: &str, candidates: &[&str]) -> Option<String> {
    find_closest_with_score(
        target,
        &candidates.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
    .map(|(s, _)| s)
}

/// Find closest match with confidence score (0.0 - 1.0) using Jaro-Winkler
/// Returns None if no candidate meets the similarity threshold
fn find_closest_with_score(target: &str, candidates: &[String]) -> Option<(String, f32)> {
    if candidates.is_empty() {
        return None;
    }

    let target_lower = target.to_lowercase();

    // Score all candidates using Jaro-Winkler (optimized for short strings)
    let mut scored: Vec<(String, f64)> = candidates
        .iter()
        .map(|candidate| {
            let score = strsim::jaro_winkler(&target_lower, &candidate.to_lowercase());
            (candidate.clone(), score)
        })
        .filter(|(_, score)| *score >= SIMILARITY_THRESHOLD)
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored.first().map(|(s, score)| (s.clone(), *score as f32))
}

/// Get top N suggestions with scores, filtered by threshold
#[allow(dead_code)]
fn get_ranked_suggestions(target: &str, candidates: &[String], top_n: usize) -> Vec<(String, f32)> {
    if candidates.is_empty() {
        return vec![];
    }

    let target_lower = target.to_lowercase();

    let mut scored: Vec<(String, f64)> = candidates
        .iter()
        .map(|candidate| {
            let score = strsim::jaro_winkler(&target_lower, &candidate.to_lowercase());
            (candidate.clone(), score)
        })
        .filter(|(_, score)| *score >= SIMILARITY_THRESHOLD)
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .take(top_n)
        .map(|(s, score)| (s, score as f32))
        .collect()
}

// ============================================================================
// Query Handlers
// ============================================================================

#[derive(Serialize)]
struct CbuSummary {
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    client_type: Option<String>,
}

async fn list_cbus(
    State(state): State<AppState>,
) -> Result<Json<Vec<CbuSummary>>, (StatusCode, String)> {
    let rows = sqlx::query_as!(
        CbuSummary,
        r#"SELECT cbu_id, name, jurisdiction, client_type FROM "ob-poc".cbus ORDER BY created_at DESC"#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

async fn get_cbu(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let cbu = sqlx::query!(
        r#"SELECT cbu_id, name, jurisdiction, client_type, description,
                  created_at, updated_at
           FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "CBU not found".to_string()))?;

    let entities = sqlx::query!(
        r#"SELECT e.entity_id, e.name, et.name as entity_type,
                  r.name as role_name
           FROM "ob-poc".cbu_entity_roles cer
           JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
           JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
           JOIN "ob-poc".roles r ON cer.role_id = r.role_id
           WHERE cer.cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = serde_json::json!({
        "cbu_id": cbu.cbu_id,
        "name": cbu.name,
        "jurisdiction": cbu.jurisdiction,
        "client_type": cbu.client_type,
        "description": cbu.description,
        "created_at": cbu.created_at,
        "updated_at": cbu.updated_at,
        "entities": entities.iter().map(|e| serde_json::json!({
            "entity_id": e.entity_id,
            "name": e.name,
            "entity_type": e.entity_type,
            "role": e.role_name
        })).collect::<Vec<_>>()
    });

    Ok(Json(result))
}

async fn get_kyc_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let case_row = sqlx::query!(
        r#"SELECT case_id, cbu_id, status, case_type, risk_rating,
                  opened_at, closed_at
           FROM kyc.cases WHERE case_id = $1"#,
        case_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Case not found".to_string()))?;

    let workstreams = sqlx::query!(
        r#"SELECT workstream_id, entity_id, status, is_ubo, risk_rating
           FROM kyc.entity_workstreams WHERE case_id = $1"#,
        case_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let flags = sqlx::query!(
        r#"SELECT red_flag_id, flag_type, severity, status, description
           FROM kyc.red_flags WHERE case_id = $1"#,
        case_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = serde_json::json!({
        "case_id": case_row.case_id,
        "cbu_id": case_row.cbu_id,
        "status": case_row.status,
        "case_type": case_row.case_type,
        "risk_rating": case_row.risk_rating,
        "opened_at": case_row.opened_at,
        "closed_at": case_row.closed_at,
        "workstreams": workstreams.iter().map(|w| serde_json::json!({
            "workstream_id": w.workstream_id,
            "entity_id": w.entity_id,
            "status": w.status,
            "is_ubo": w.is_ubo,
            "risk_rating": w.risk_rating
        })).collect::<Vec<_>>(),
        "red_flags": flags.iter().map(|f| serde_json::json!({
            "red_flag_id": f.red_flag_id,
            "flag_type": f.flag_type,
            "severity": f.severity,
            "status": f.status,
            "description": f.description
        })).collect::<Vec<_>>()
    });

    Ok(Json(result))
}

// ============================================================================
// Cleanup Handler
// ============================================================================

async fn cleanup_cbu(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // KYC data
    let _ = sqlx::query(r#"DELETE FROM kyc.red_flags WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM kyc.screenings WHERE workstream_id IN (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM kyc.doc_requests WHERE workstream_id IN (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await;
    let _ = sqlx::query(r#"DELETE FROM kyc.cases WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await;

    // Core data
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".cbu_resource_instances WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await;

    let result = sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "deleted": result.rows_affected() > 0,
        "cbu_id": cbu_id
    })))
}

// ============================================================================
// Entity Search
// ============================================================================

#[derive(Deserialize)]
struct EntitySearchRequest {
    query: String,
    #[serde(default)]
    limit: Option<i64>,
    /// Optional jurisdiction filter (e.g., "US", "LU", "GB")
    #[serde(default)]
    jurisdiction: Option<String>,
    /// Optional entity type filter (e.g., "LIMITED_COMPANY_PRIVATE", "PROPER_PERSON_NATURAL")
    #[serde(default)]
    entity_type: Option<String>,
}

#[derive(Serialize)]
struct EntitySearchResult {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    entity_type_code: Option<String>,
    jurisdiction: Option<String>,
    similarity: f32,
}

#[derive(Serialize)]
struct EntitySearchResponse {
    results: Vec<EntitySearchResult>,
    query: String,
    create_option: String,
}

/// Type alias for entity search query result row
type EntitySearchRow = (Uuid, String, String, Option<String>, Option<String>, f32);

/// Search for existing entities by name (fuzzy match)
/// Uses pg_trgm for fast fuzzy search with optional jurisdiction/type filtering
async fn search_entities(
    State(state): State<AppState>,
    Json(req): Json<EntitySearchRequest>,
) -> Json<EntitySearchResponse> {
    let limit = req.limit.unwrap_or(10).min(50);
    let query = req.query.trim();

    if query.is_empty() {
        return Json(EntitySearchResponse {
            results: vec![],
            query: query.to_string(),
            create_option: "Create new entity".to_string(),
        });
    }

    // Build dynamic query with optional filters
    let mut sql = String::from(
        r#"
        SELECT
            e.entity_id,
            e.name,
            et.name as entity_type,
            et.type_code as entity_type_code,
            COALESCE(
                lc.jurisdiction,
                p.jurisdiction,
                t.jurisdiction,
                pp.nationality
            ) as jurisdiction,
            similarity(e.name, $1) as sim
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
        LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
        LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
        LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
        WHERE (e.name % $1 OR similarity(e.name, $1) > 0.15)
        "#,
    );

    // Add jurisdiction filter if provided
    let mut param_idx = 2;
    if req.jurisdiction.is_some() {
        sql.push_str(&format!(
            " AND COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) = ${}",
            param_idx
        ));
        param_idx += 1;
    }

    // Add entity type filter if provided
    if req.entity_type.is_some() {
        sql.push_str(&format!(" AND et.type_code = ${}", param_idx));
        param_idx += 1;
    }

    sql.push_str(&format!(
        " ORDER BY similarity(e.name, $1) DESC LIMIT ${}",
        param_idx
    ));

    // Execute query with appropriate bindings
    let rows: Vec<EntitySearchRow> = match (&req.jurisdiction, &req.entity_type) {
        (Some(j), Some(t)) => {
            sqlx::query_as(&sql)
                .bind(query)
                .bind(j)
                .bind(t)
                .bind(limit)
                .fetch_all(&state.pool)
                .await
        }
        (Some(j), None) => {
            sqlx::query_as(&sql)
                .bind(query)
                .bind(j)
                .bind(limit)
                .fetch_all(&state.pool)
                .await
        }
        (None, Some(t)) => {
            sqlx::query_as(&sql)
                .bind(query)
                .bind(t)
                .bind(limit)
                .fetch_all(&state.pool)
                .await
        }
        (None, None) => {
            sqlx::query_as(&sql)
                .bind(query)
                .bind(limit)
                .fetch_all(&state.pool)
                .await
        }
    }
    .unwrap_or_default();

    // Re-rank with Jaro-Winkler for better accuracy
    let query_lower = query.to_lowercase();
    let mut results: Vec<EntitySearchResult> = rows
        .into_iter()
        .map(
            |(entity_id, name, entity_type, entity_type_code, jurisdiction, _pg_sim)| {
                let jw_score = strsim::jaro_winkler(&query_lower, &name.to_lowercase()) as f32;
                EntitySearchResult {
                    entity_id,
                    name,
                    entity_type,
                    entity_type_code,
                    jurisdiction,
                    similarity: jw_score,
                }
            },
        )
        .collect();

    // Sort by Jaro-Winkler score
    results.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Build create option with filters context
    let create_option = match (&req.jurisdiction, &req.entity_type) {
        (Some(j), Some(t)) => format!("Create new {} \"{}\" in {}", t, query, j),
        (Some(j), None) => format!("Create new entity \"{}\" in {}", query, j),
        (None, Some(t)) => format!("Create new {} \"{}\"", t, query),
        (None, None) => format!("Create new entity \"{}\"", query),
    };

    Json(EntitySearchResponse {
        results,
        query: query.to_string(),
        create_option,
    })
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    let state = AppState {
        pool: pool.clone(),
        executor: Arc::new(DslExecutor::new(pool)),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .route("/verbs", get(list_verbs))
        .route("/validate", post(validate))
        .route("/validate-with-fixes", post(validate_with_fixes))
        .route("/execute", post(execute))
        .route("/analyze-errors", post(analyze_errors))
        .route("/query/cbus", get(list_cbus))
        .route("/query/cbus/{id}", get(get_cbu))
        .route("/query/kyc/cases/{id}", get(get_kyc_case))
        .route("/query/entities/search", post(search_entities))
        .route("/cleanup/cbu/{id}", delete(cleanup_cbu))
        .layer(cors)
        .with_state(state);

    let addr = "0.0.0.0:3001";
    println!("dsl_api listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
