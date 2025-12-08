//! DSL Builder - Constructs valid DSL from structured intents
//!
//! This module provides deterministic DSL generation from VerbIntent structures.
//! The LLM extracts intents, this code builds DSL - keeping LLM away from syntax.
//!
//! Mirrors the Zed/LSP pipeline: Intent → Build DSL → Validate → Feedback

use crate::api::intent::{IntentError, IntentValidation, VerbIntent};
use crate::dsl_v2::{find_unified_verb, registry};

/// Build DSL string from a single VerbIntent
pub fn build_dsl_statement(intent: &VerbIntent, binding_counter: &mut u32) -> String {
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

    // Generate binding if this verb returns something
    // Convention: use @result_N for auto-generated bindings
    if should_generate_binding(&intent.verb) {
        *binding_counter += 1;
        parts.push(format!(":as @result_{}", binding_counter));
    }

    parts.push(")".to_string());
    parts.join(" ")
}

/// Build complete DSL program from a sequence of intents
pub fn build_dsl_program(intents: &[VerbIntent]) -> String {
    let mut binding_counter = 0;
    let statements: Vec<String> = intents
        .iter()
        .map(|intent| build_dsl_statement(intent, &mut binding_counter))
        .collect();

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

    binding_verbs.iter().any(|v| verb == *v)
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
        assert!(program.contains("@result_1"));
    }
}
