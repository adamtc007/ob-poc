//! Narration Template Linting
//!
//! Validates narration templates at startup to catch errors like:
//! - Unknown placeholders (not in whitelist)
//! - Unknown param placeholders ({arg.X} with no arg X)
//! - Scope placeholders without scope slot
//! - Malformed placeholders (nested braces, function calls)
//!
//! # Error Codes
//!
//! | Code | Description |
//! |------|-------------|
//! | `NARR001` | Unknown placeholder (not in whitelist) |
//! | `NARR002` | Unknown param placeholder (`{arg.X}` with no arg X) |
//! | `NARR003` | Scope placeholder without scope slot |
//! | `NARR004` | Snapshot placeholder without entity_scope type |
//! | `NARR005` | Effects placeholder without effects metadata |
//! | `NARR006` | Malformed placeholder (nested braces, function call) |
//!
//! # Usage
//!
//! ```ignore
//! use ob_poc::dsl_v2::narration_lint::{lint_narration_template, LintStatus};
//!
//! let report = lint_narration_template("cbu.create", &verb_config);
//! if report.status == LintStatus::Error {
//!     panic!("Narration lint failed: {:?}", report.errors);
//! }
//! ```

use dsl_core::config::types::VerbConfig;
use thiserror::Error;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Narration lint error
#[derive(Debug, Error, Clone)]
pub enum NarrationLintError {
    #[error("NARR001: Unknown placeholder '{placeholder}' in {field}{}",
        suggestion.as_ref().map(|s| format!(". Did you mean {}?", s)).unwrap_or_default()
    )]
    UnknownPlaceholder {
        verb_id: String,
        field: String,
        placeholder: String,
        suggestion: Option<String>,
    },

    #[error("NARR002: Unknown param placeholder '{{arg.{param}}}' - verb has no arg '{param}'")]
    UnknownParamPlaceholder {
        verb_id: String,
        field: String,
        param: String,
        available_params: Vec<String>,
    },

    #[error("NARR003: Scope placeholder used but verb has no scope/entity argument")]
    ScopePlaceholderWithoutSlot {
        verb_id: String,
        field: String,
        placeholder: String,
    },

    #[error("NARR004: Snapshot placeholder '{placeholder}' requires entity_scope typed slot")]
    SnapshotPlaceholderWithoutScopeSlot {
        verb_id: String,
        field: String,
        placeholder: String,
    },

    #[error("NARR005: Effects placeholder '{placeholder}' requires effects metadata in verb")]
    EffectsPlaceholderMissingMetadata {
        verb_id: String,
        field: String,
        placeholder: String,
    },

    #[error("NARR006: Malformed placeholder '{placeholder}' - {reason}")]
    MalformedPlaceholder {
        verb_id: String,
        field: String,
        placeholder: String,
        reason: String,
    },
}

/// Narration lint warning
#[derive(Debug, Clone)]
pub struct NarrationLintWarning {
    pub code: String,
    pub verb_id: String,
    pub field: String,
    pub message: String,
}

// =============================================================================
// LINT REPORT
// =============================================================================

/// Result of linting a verb's narration templates
#[derive(Debug, Clone)]
pub struct NarrationLintReport {
    pub verb_id: String,
    pub status: LintStatus,
    pub errors: Vec<NarrationLintError>,
    pub warnings: Vec<NarrationLintWarning>,
}

/// Overall lint status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintStatus {
    Ok,
    Warning,
    Error,
}

impl NarrationLintReport {
    /// Create a passing report
    pub fn ok(verb_id: impl Into<String>) -> Self {
        Self {
            verb_id: verb_id.into(),
            status: LintStatus::Ok,
            errors: vec![],
            warnings: vec![],
        }
    }

    /// Check if report has any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if report has any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

// =============================================================================
// LINTING
// =============================================================================

/// Lint all narration templates in a verb definition
pub fn lint_narration_template(verb_id: &str, verb_def: &VerbConfig) -> NarrationLintReport {
    let mut errors = vec![];
    let mut warnings = vec![];

    if let Some(template) = &verb_def.narration_template {
        // Lint each template field
        if let Some(success) = &template.success {
            lint_template_string(
                verb_id,
                "success",
                success,
                verb_def,
                &mut errors,
                &mut warnings,
            );
        }
        if let Some(failure) = &template.failure {
            lint_template_string(
                verb_id,
                "failure",
                failure,
                verb_def,
                &mut errors,
                &mut warnings,
            );
        }
        if let Some(preview) = &template.preview {
            lint_template_string(
                verb_id,
                "preview",
                preview,
                verb_def,
                &mut errors,
                &mut warnings,
            );
        }
        if let Some(hint) = &template.training_hint {
            lint_template_string(
                verb_id,
                "training_hint",
                hint,
                verb_def,
                &mut errors,
                &mut warnings,
            );
        }

        // Lint conditionals
        for (i, cond) in template.conditionals.iter().enumerate() {
            if let Some(s) = &cond.success {
                lint_template_string(
                    verb_id,
                    &format!("conditionals[{}].success", i),
                    s,
                    verb_def,
                    &mut errors,
                    &mut warnings,
                );
            }
            if let Some(f) = &cond.failure {
                lint_template_string(
                    verb_id,
                    &format!("conditionals[{}].failure", i),
                    f,
                    verb_def,
                    &mut errors,
                    &mut warnings,
                );
            }
        }
    }

    let status = if !errors.is_empty() {
        LintStatus::Error
    } else if !warnings.is_empty() {
        LintStatus::Warning
    } else {
        LintStatus::Ok
    };

    NarrationLintReport {
        verb_id: verb_id.to_string(),
        status,
        errors,
        warnings,
    }
}

/// Lint a single template string
fn lint_template_string(
    verb_id: &str,
    field: &str,
    template: &str,
    verb_def: &VerbConfig,
    errors: &mut Vec<NarrationLintError>,
    warnings: &mut Vec<NarrationLintWarning>,
) {
    // Extract all placeholders
    let placeholders = extract_placeholders(template);

    for placeholder in placeholders {
        // Check for malformed placeholders
        if placeholder.contains('{') || placeholder.contains('}') {
            errors.push(NarrationLintError::MalformedPlaceholder {
                verb_id: verb_id.to_string(),
                field: field.to_string(),
                placeholder: placeholder.clone(),
                reason: "nested braces not allowed".to_string(),
            });
            continue;
        }

        if placeholder.contains('(') {
            errors.push(NarrationLintError::MalformedPlaceholder {
                verb_id: verb_id.to_string(),
                field: field.to_string(),
                placeholder: placeholder.clone(),
                reason: "function calls not allowed".to_string(),
            });
            continue;
        }

        // Validate against whitelist
        if !validate_placeholder(&placeholder, verb_def, verb_id, field, errors, warnings) {
            // Error already pushed by validate_placeholder
        }
    }
}

/// Extract all placeholders from a template string
fn extract_placeholders(template: &str) -> Vec<String> {
    let mut placeholders = vec![];
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut placeholder = String::new();
            while let Some(&next) = chars.peek() {
                if next == '}' {
                    chars.next();
                    break;
                }
                placeholder.push(chars.next().unwrap());
            }
            if !placeholder.is_empty() {
                placeholders.push(placeholder);
            }
        }
    }

    placeholders
}

/// Validate a placeholder against the whitelist
fn validate_placeholder(
    placeholder: &str,
    verb_def: &VerbConfig,
    verb_id: &str,
    field: &str,
    errors: &mut Vec<NarrationLintError>,
    _warnings: &mut Vec<NarrationLintWarning>,
) -> bool {
    // Built-in placeholders
    let builtins = [
        "error",
        "affected_count",
        "duration_ms",
        "warnings",
        "entity_names",
    ];
    if builtins.contains(&placeholder) {
        return true;
    }

    // Namespaced placeholders
    let parts: Vec<&str> = placeholder.split('.').collect();
    match parts.as_slice() {
        ["verb"] | ["verb", "id" | "domain" | "action"] => true,
        ["domain"] => true,
        ["group", "alias" | "id"] => true,
        ["entity", "label" | "id"] => true,
        ["result", _] => true, // Any result field allowed

        ["scope", key] => {
            // Check verb has scope slot
            let has_scope_slot = verb_def
                .args
                .iter()
                .any(|a| a.name == "scope" || a.name == "entity-id" || a.name == "entity-ids");
            if !has_scope_slot {
                errors.push(NarrationLintError::ScopePlaceholderWithoutSlot {
                    verb_id: verb_id.to_string(),
                    field: field.to_string(),
                    placeholder: placeholder.to_string(),
                });
                return false;
            }

            // Snapshot-only check - requires entity set slot type
            if *key == "count" || *key == "snapshot" {
                use dsl_core::config::types::SlotType;
                let has_entity_set_type = verb_def.args.iter().any(|a| {
                    matches!(
                        a.slot_type,
                        Some(SlotType::EntitySetRef) | Some(SlotType::CbuSetRef)
                    )
                });
                if !has_entity_set_type {
                    errors.push(NarrationLintError::SnapshotPlaceholderWithoutScopeSlot {
                        verb_id: verb_id.to_string(),
                        field: field.to_string(),
                        placeholder: placeholder.to_string(),
                    });
                    return false;
                }
            }
            true
        }

        ["effects", _key] => {
            // Effects placeholders not yet supported - would require effects metadata
            errors.push(NarrationLintError::EffectsPlaceholderMissingMetadata {
                verb_id: verb_id.to_string(),
                field: field.to_string(),
                placeholder: placeholder.to_string(),
            });
            false
        }

        ["arg", param_name] => {
            // Check param exists in verb args
            let param_exists = verb_def.args.iter().any(|a| a.name == *param_name);
            if !param_exists {
                let available: Vec<String> = verb_def.args.iter().map(|a| a.name.clone()).collect();
                errors.push(NarrationLintError::UnknownParamPlaceholder {
                    verb_id: verb_id.to_string(),
                    field: field.to_string(),
                    param: param_name.to_string(),
                    available_params: available,
                });
                return false;
            }
            true
        }

        _ => {
            // Unknown placeholder
            let suggestion = suggest_placeholder(placeholder);
            errors.push(NarrationLintError::UnknownPlaceholder {
                verb_id: verb_id.to_string(),
                field: field.to_string(),
                placeholder: placeholder.to_string(),
                suggestion,
            });
            false
        }
    }
}

/// Suggest a similar placeholder using Levenshtein distance
fn suggest_placeholder(unknown: &str) -> Option<String> {
    let known = [
        "verb",
        "verb.id",
        "verb.domain",
        "domain",
        "group.alias",
        "group.id",
        "scope.desc",
        "scope.count",
        "entity.label",
        "entity.id",
        "effects.mode",
        "effects.writes",
        "error",
        "affected_count",
        "duration_ms",
        "warnings",
        "entity_names",
    ];

    known
        .iter()
        .min_by_key(|k| levenshtein(unknown, k))
        .filter(|k| levenshtein(unknown, k) <= 3)
        .map(|s| s.to_string())
}

/// Simple Levenshtein distance implementation
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}

// =============================================================================
// BATCH LINTING
// =============================================================================

/// Lint all verbs in a config and return combined results
pub fn lint_all_narration_templates(verbs: &[(String, VerbConfig)]) -> Vec<NarrationLintReport> {
    verbs
        .iter()
        .map(|(id, config)| lint_narration_template(id, config))
        .collect()
}

/// Check if any lint reports have errors
pub fn has_lint_errors(reports: &[NarrationLintReport]) -> bool {
    reports.iter().any(|r| r.status == LintStatus::Error)
}

/// Format lint errors as a summary string
pub fn format_lint_errors(reports: &[NarrationLintReport]) -> String {
    let errors: Vec<String> = reports
        .iter()
        .filter(|r| r.status == LintStatus::Error)
        .flat_map(|r| r.errors.iter())
        .map(|e| e.to_string())
        .collect();

    errors.join("\n")
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::config::types::{
        ArgConfig, ArgType, ConditionalNarration, NarrationTemplate, VerbBehavior,
    };

    fn make_test_verb_config(args: Vec<ArgConfig>) -> VerbConfig {
        VerbConfig {
            description: "Test verb".to_string(),
            behavior: VerbBehavior::Plugin,
            crud: None,
            handler: Some("test".to_string()),
            graph_query: None,
            args,
            returns: None,
            produces: None,
            consumes: vec![],
            lifecycle: None,
            metadata: None,
            invocation_phrases: vec![],
            policy: None,
            narration_template: None,
        }
    }

    #[test]
    fn test_valid_builtin_placeholders() {
        let mut config = make_test_verb_config(vec![]);
        config.narration_template = Some(NarrationTemplate {
            success: Some(
                "{verb} completed with {affected_count} affected in {duration_ms}ms".to_string(),
            ),
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Ok);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_valid_arg_placeholder() {
        let mut config = make_test_verb_config(vec![ArgConfig {
            name: "name".to_string(),
            arg_type: ArgType::String,
            required: true,
            maps_to: None,
            lookup: None,
            valid_values: None,
            default: None,
            description: None,
            validation: None,
            fuzzy_check: None,
            slot_type: None,
            preferred_roles: vec![],
        }]);
        config.narration_template = Some(NarrationTemplate {
            success: Some("Created '{arg.name}'".to_string()),
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Ok);
    }

    #[test]
    fn test_unknown_arg_placeholder() {
        let mut config = make_test_verb_config(vec![]);
        config.narration_template = Some(NarrationTemplate {
            success: Some("Created '{arg.name}'".to_string()),
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Error);
        assert!(matches!(
            &report.errors[0],
            NarrationLintError::UnknownParamPlaceholder { param, .. } if param == "name"
        ));
    }

    #[test]
    fn test_unknown_placeholder_with_suggestion() {
        let mut config = make_test_verb_config(vec![]);
        config.narration_template = Some(NarrationTemplate {
            success: Some("{affcted_count} affected".to_string()), // typo
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Error);
        assert!(matches!(
            &report.errors[0],
            NarrationLintError::UnknownPlaceholder { suggestion: Some(s), .. } if s == "affected_count"
        ));
    }

    #[test]
    fn test_malformed_nested_braces() {
        let mut config = make_test_verb_config(vec![]);
        config.narration_template = Some(NarrationTemplate {
            success: Some("{{nested}}".to_string()),
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Error);
        assert!(matches!(
            &report.errors[0],
            NarrationLintError::MalformedPlaceholder { reason, .. } if reason.contains("nested")
        ));
    }

    #[test]
    fn test_malformed_function_call() {
        let mut config = make_test_verb_config(vec![]);
        config.narration_template = Some(NarrationTemplate {
            success: Some("{arg.name.toUpper()}".to_string()),
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Error);
        assert!(matches!(
            &report.errors[0],
            NarrationLintError::MalformedPlaceholder { reason, .. } if reason.contains("function")
        ));
    }

    #[test]
    fn test_scope_placeholder_without_scope_arg() {
        let mut config = make_test_verb_config(vec![]);
        config.narration_template = Some(NarrationTemplate {
            success: Some("Found {scope.count} entities".to_string()),
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Error);
        assert!(matches!(
            &report.errors[0],
            NarrationLintError::ScopePlaceholderWithoutSlot { .. }
        ));
    }

    #[test]
    fn test_result_placeholder_allowed() {
        let mut config = make_test_verb_config(vec![]);
        config.narration_template = Some(NarrationTemplate {
            success: Some("Created with ID {result.id}".to_string()),
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Ok);
    }

    #[test]
    fn test_conditional_templates_linted() {
        let mut config = make_test_verb_config(vec![]);
        config.narration_template = Some(NarrationTemplate {
            success: Some("OK".to_string()),
            conditionals: vec![ConditionalNarration {
                condition: "affected_count > 10".to_string(),
                success: Some("{arg.missing_arg} items".to_string()), // Invalid
                failure: None,
            }],
            ..Default::default()
        });

        let report = lint_narration_template("test.verb", &config);
        assert_eq!(report.status, LintStatus::Error);
        assert!(report.errors[0].to_string().contains("missing_arg"));
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", "abd"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }
}
