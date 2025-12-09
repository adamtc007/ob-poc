//! DSL Builder - Constructs valid DSL from structured intents
//!
//! This module provides deterministic DSL generation from VerbIntent structures.
//! The LLM extracts intents, this code builds DSL - keeping LLM away from syntax.
//!
//! Mirrors the Zed/LSP pipeline: Intent → Build DSL → Validate → Feedback

use crate::api::intent::{IntentError, IntentValidation, ParamValue, VerbIntent};
use crate::dsl_v2::{find_unified_verb, registry};

/// Result of building a DSL statement, includes binding metadata
#[derive(Debug, Clone)]
pub struct DslBuildResult {
    /// The DSL statement string
    pub statement: String,
    /// The binding name if one was generated (e.g., "aviva_lux_9")
    pub binding_name: Option<String>,
    /// The display name for the entity (e.g., "Aviva Lux 9")
    pub display_name: Option<String>,
    /// The entity type (e.g., "cbu", "entity")
    pub entity_type: Option<String>,
}

/// Build DSL string from a single VerbIntent, returning binding metadata
pub fn build_dsl_statement_with_metadata(intent: &VerbIntent) -> DslBuildResult {
    let mut parts = vec![format!("({}", intent.verb)];

    // Add parameters in sorted order for determinism
    let mut param_names: Vec<_> = intent.params.keys().collect();
    param_names.sort();

    for name in param_names {
        if let Some(value) = intent.params.get(name) {
            parts.push(format!(":{} {}", name, value.to_dsl_string()));
        }
    }

    // Add references (e.g., :cbu-id @cbu)
    let mut ref_names: Vec<_> = intent.refs.keys().collect();
    ref_names.sort();

    for name in ref_names {
        if let Some(ref_name) = intent.refs.get(name) {
            parts.push(format!(":{} {}", name, ref_name));
        }
    }

    // Generate semantic binding if this verb returns something
    let (binding_name, display_name, entity_type) = if should_generate_binding(&intent.verb) {
        let (name, display, etype) = generate_semantic_binding(intent);
        parts.push(format!(":as @{}", name));
        (Some(name), Some(display), Some(etype))
    } else {
        (None, None, None)
    };

    parts.push(")".to_string());

    DslBuildResult {
        statement: parts.join(" "),
        binding_name,
        display_name,
        entity_type,
    }
}

/// Build DSL string from a single VerbIntent (legacy interface)
pub fn build_dsl_statement(intent: &VerbIntent, _binding_counter: &mut u32) -> String {
    build_dsl_statement_with_metadata(intent).statement
}

/// Generate a semantic binding name from the intent's parameters
/// Returns (binding_name, display_name, entity_type)
fn generate_semantic_binding(intent: &VerbIntent) -> (String, String, String) {
    let entity_type = get_entity_type_from_verb(&intent.verb);

    // Try to get a name from common parameter names
    let display_name = intent
        .params
        .get("name")
        .or_else(|| intent.params.get("cbu-name"))
        .or_else(|| intent.params.get("first-name"))
        .or_else(|| intent.params.get("company-name"))
        .or_else(|| intent.params.get("trust-name"))
        .or_else(|| intent.params.get("partnership-name"))
        .and_then(|v| match v {
            ParamValue::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_else(|| entity_type.clone());

    // Convert display name to valid binding name (lowercase, underscores)
    let binding_name = display_name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    // Ensure binding name is not empty
    let binding_name = if binding_name.is_empty() {
        entity_type.clone()
    } else {
        binding_name
    };

    (binding_name, display_name, entity_type)
}

/// Get the entity type from the verb name
fn get_entity_type_from_verb(verb: &str) -> String {
    match verb {
        v if v.starts_with("cbu.") => "cbu".to_string(),
        v if v.starts_with("entity.") => "entity".to_string(),
        v if v.starts_with("kyc-case.") => "case".to_string(),
        v if v.starts_with("entity-workstream.") => "workstream".to_string(),
        v if v.starts_with("share-class.") => "share_class".to_string(),
        v if v.starts_with("holding.") => "holding".to_string(),
        v if v.starts_with("service-resource.") => "resource".to_string(),
        v if v.starts_with("document.") => "document".to_string(),
        _ => "entity".to_string(),
    }
}

/// Build complete DSL program from a sequence of intents
/// Tracks bindings generated for each statement and replaces @result_N references
pub fn build_dsl_program(intents: &[VerbIntent]) -> String {
    let mut statements = Vec::new();
    // Map from @result_N -> actual semantic binding name
    let mut result_bindings: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for (idx, intent) in intents.iter().enumerate() {
        // Build the statement and get binding metadata
        let build_result = build_dsl_statement_with_metadata(intent);
        let mut statement = build_result.statement;

        // Replace any @result_N references with actual binding names
        for (result_ref, actual_binding) in &result_bindings {
            statement = statement.replace(result_ref, &format!("@{}", actual_binding));
        }

        statements.push(statement);

        // Track this statement's binding for future references
        // AI uses @result_1 for first statement, @result_2 for second, etc.
        if let Some(binding_name) = build_result.binding_name {
            let result_ref = format!("@result_{}", idx + 1);
            result_bindings.insert(result_ref, binding_name);
        }
    }

    statements.join("\n")
}

/// Check if a verb typically returns a value that should be bound
fn should_generate_binding(verb: &str) -> bool {
    // Verbs that create entities/resources typically return IDs
    let binding_verbs = [
        "cbu.ensure",
        "cbu.create",
        "entity.create-proper-person",
        "entity.create-limited-company",
        "entity.create-partnership-limited",
        "entity.create-trust-discretionary",
        "document.catalog",
        "kyc-case.create",
        "entity-workstream.create",
        "share-class.create",
        "holding.create",
        "service-resource.provision",
    ];

    binding_verbs.contains(&verb)
}

/// Validate a VerbIntent against the verb registry
/// Uses the same registry as the LSP for consistency
pub fn validate_intent(intent: &VerbIntent) -> IntentValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Parse domain.verb
    let parts: Vec<&str> = intent.verb.split('.').collect();
    if parts.len() != 2 {
        errors.push(IntentError {
            code: "INVALID_VERB_FORMAT".to_string(),
            message: format!(
                "Invalid verb format '{}', expected 'domain.verb'",
                intent.verb
            ),
            param: None,
        });
        return IntentValidation {
            valid: false,
            intent: intent.clone(),
            errors,
            warnings,
        };
    }

    let domain = parts[0];
    let verb_name = parts[1];

    // Check verb exists using the same registry as LSP
    let verb = match find_unified_verb(domain, verb_name) {
        Some(v) => v,
        None => {
            // Suggest similar verbs (same logic as LSP diagnostics)
            let reg = registry();
            let suggestions: Vec<String> = reg
                .all_verbs()
                .filter(|v| {
                    v.domain == domain
                        || v.verb.contains(verb_name)
                        || v.full_name().contains(&intent.verb)
                })
                .take(3)
                .map(|v| v.full_name())
                .collect();

            let message = if suggestions.is_empty() {
                format!("Unknown verb '{}'", intent.verb)
            } else {
                format!(
                    "Unknown verb '{}'. Did you mean: {}?",
                    intent.verb,
                    suggestions.join(", ")
                )
            };

            errors.push(IntentError {
                code: "UNKNOWN_VERB".to_string(),
                message,
                param: None,
            });

            return IntentValidation {
                valid: false,
                intent: intent.clone(),
                errors,
                warnings,
            };
        }
    };

    // Get all known args for this verb
    let required_args = verb.required_arg_names();
    let optional_args = verb.optional_arg_names();
    let all_known_args: Vec<&str> = required_args
        .iter()
        .chain(optional_args.iter())
        .copied()
        .collect();

    // Combine params and refs for checking
    let provided: std::collections::HashSet<&str> = intent
        .params
        .keys()
        .map(|s| s.as_str())
        .chain(intent.refs.keys().map(|s| s.as_str()))
        .collect();

    // Check for missing required arguments
    for required_arg in &required_args {
        if !provided.contains(*required_arg) {
            errors.push(IntentError {
                code: "MISSING_REQUIRED_ARG".to_string(),
                message: format!(
                    "Missing required argument '{}' for '{}'",
                    required_arg, intent.verb
                ),
                param: Some(required_arg.to_string()),
            });
        }
    }

    // Check for unknown arguments
    for param_name in intent.params.keys() {
        if !all_known_args.contains(&param_name.as_str()) && param_name != "as" {
            warnings.push(format!(
                "Unknown argument '{}' for verb '{}'",
                param_name, intent.verb
            ));
        }
    }

    for ref_name in intent.refs.keys() {
        if !all_known_args.contains(&ref_name.as_str()) {
            warnings.push(format!(
                "Unknown reference '{}' for verb '{}'",
                ref_name, intent.verb
            ));
        }
    }

    // Note: Product code validation is handled by the executor which looks up
    // against the database. The prompt instructs Claude to use uppercase codes.

    IntentValidation {
        valid: errors.is_empty(),
        intent: intent.clone(),
        errors,
        warnings,
    }
}

/// Format validation errors for feedback to the agent
pub fn format_feedback(errors: &[String]) -> String {
    if errors.is_empty() {
        return String::new();
    }

    let mut feedback = String::from("The DSL has the following errors:\n");
    for (i, error) in errors.iter().enumerate() {
        feedback.push_str(&format!("{}. {}\n", i + 1, error));
    }
    feedback.push_str("\nPlease fix these issues in your intent.");
    feedback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_statement() {
        let mut counter = 0;
        let intent = VerbIntent {
            verb: "cbu.ensure".to_string(),
            params: {
                let mut m = HashMap::new();
                m.insert(
                    "name".to_string(),
                    ParamValue::String("Test Fund".to_string()),
                );
                m.insert(
                    "jurisdiction".to_string(),
                    ParamValue::String("LU".to_string()),
                );
                m
            },
            refs: HashMap::new(),
            sequence: None,
        };

        let dsl = build_dsl_statement(&intent, &mut counter);
        assert!(dsl.contains("cbu.ensure"));
        assert!(dsl.contains(":name \"Test Fund\""));
        assert!(dsl.contains(":jurisdiction \"LU\""));
        assert!(dsl.contains(":as @result_1"));
    }

    #[test]
    fn test_build_with_refs() {
        let mut counter = 0;
        let intent = VerbIntent {
            verb: "cbu.assign-role".to_string(),
            params: {
                let mut m = HashMap::new();
                m.insert(
                    "role".to_string(),
                    ParamValue::String("DIRECTOR".to_string()),
                );
                m
            },
            refs: {
                let mut m = HashMap::new();
                m.insert("cbu-id".to_string(), "@cbu".to_string());
                m.insert("entity-id".to_string(), "@person".to_string());
                m
            },
            sequence: None,
        };

        let dsl = build_dsl_statement(&intent, &mut counter);
        assert!(dsl.contains(":cbu-id @cbu"));
        assert!(dsl.contains(":entity-id @person"));
        assert!(dsl.contains(":role \"DIRECTOR\""));
    }

    #[test]
    fn test_build_program() {
        let intents = vec![
            VerbIntent {
                verb: "cbu.ensure".to_string(),
                params: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), ParamValue::String("Test".to_string()));
                    m
                },
                refs: HashMap::new(),
                sequence: None,
            },
            VerbIntent {
                verb: "cbu.add-product".to_string(),
                params: {
                    let mut m = HashMap::new();
                    m.insert(
                        "product".to_string(),
                        ParamValue::String("Custody".to_string()),
                    );
                    m
                },
                refs: {
                    let mut m = HashMap::new();
                    m.insert("cbu-id".to_string(), "@result_1".to_string());
                    m
                },
                sequence: None,
            },
        ];

        let program = build_dsl_program(&intents);
        assert!(program.contains("cbu.ensure"));
        assert!(program.contains("cbu.add-product"));
        // @result_1 should be replaced with the semantic binding from cbu.ensure
        assert!(
            !program.contains("@result_1"),
            "Expected @result_1 to be replaced with semantic binding"
        );
        // The cbu.ensure generates binding @test from :name "Test"
        assert!(
            program.contains(":cbu-id @test"),
            "Expected @result_1 to be replaced with @test"
        );
    }
}
