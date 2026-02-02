//! Narration Template Renderer
//!
//! Renders human-readable narration from verb execution results using
//! templates defined in verb YAML configuration.
//!
//! # Template Syntax
//!
//! Templates use `{placeholder}` syntax for variable substitution:
//! - `{arg.X}` - Value of argument X from verb call
//! - `{result.X}` - Value from execution result (e.g., `{result.id}`)
//! - `{verb}` - Full verb FQN (e.g., "cbu.create")
//! - `{domain}` - Domain name (e.g., "cbu")
//! - `{affected_count}` - Number of affected entities
//! - `{error}` - Error message (for failure templates)
//! - `{duration_ms}` - Execution time in milliseconds
//! - `{entity_names}` - Resolved entity names (comma-joined)
//!
//! # Conditional Templates
//!
//! Conditionals override base templates when conditions match:
//! ```yaml
//! conditionals:
//!   - when: "affected_count > 10"
//!     success: "Bulk created {affected_count} entities"
//! ```
//!
//! # Example
//!
//! ```ignore
//! use ob_poc::dsl_v2::narration::{render_narration, NarrationContext, NarrationOutcome};
//!
//! let template = NarrationTemplate {
//!     success: Some("Created {arg.type} '{arg.name}'".to_string()),
//!     ..Default::default()
//! };
//!
//! let context = NarrationContext {
//!     verb_fqn: "cbu.create".to_string(),
//!     domain: "cbu".to_string(),
//!     args: vec![("name".to_string(), "Acme Fund".to_string())].into_iter().collect(),
//!     ..Default::default()
//! };
//!
//! let narration = render_narration(&template, NarrationOutcome::Success, &context);
//! assert_eq!(narration, "Created FUND 'Acme Fund'");
//! ```

use dsl_core::config::types::NarrationTemplate;

#[cfg(test)]
use dsl_core::config::types::ConditionalNarration;
use std::collections::HashMap;

// =============================================================================
// TYPES
// =============================================================================

/// Outcome type for narration rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrationOutcome {
    /// Successful execution
    Success,
    /// Failed execution
    Failure,
    /// Preview/proposal mode (no execution yet)
    Preview,
}

/// Context for narration template rendering
///
/// Provides all the variables that can be substituted in templates.
#[derive(Debug, Clone, Default)]
pub struct NarrationContext {
    /// Full verb FQN (e.g., "cbu.create")
    pub verb_fqn: String,

    /// Domain name (e.g., "cbu")
    pub domain: String,

    /// Verb arguments as key-value pairs
    pub args: HashMap<String, String>,

    /// Execution result fields as key-value pairs
    pub result_fields: HashMap<String, String>,

    /// Error message (for failure outcomes)
    pub error: Option<String>,

    /// Number of affected entities
    pub affected_count: usize,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Warnings produced during execution
    pub warnings: Vec<String>,

    /// Resolved entity names (for scope/entity operations)
    pub entity_names: Vec<String>,
}

// =============================================================================
// RENDERING
// =============================================================================

/// Render narration template with context
///
/// Selects the appropriate template based on outcome and conditionals,
/// then substitutes all variables.
pub fn render_narration(
    template: &NarrationTemplate,
    outcome: NarrationOutcome,
    context: &NarrationContext,
) -> String {
    // 1. Select base template based on outcome
    let base = match outcome {
        NarrationOutcome::Success => template.success.as_deref(),
        NarrationOutcome::Failure => template.failure.as_deref(),
        NarrationOutcome::Preview => template.preview.as_deref(),
    };

    // 2. Check conditionals for override
    let selected = template
        .conditionals
        .iter()
        .find(|c| evaluate_condition(&c.condition, context))
        .and_then(|c| match outcome {
            NarrationOutcome::Success => c.success.as_deref(),
            NarrationOutcome::Failure => c.failure.as_deref(),
            NarrationOutcome::Preview => None, // Conditionals don't override preview
        })
        .or(base);

    // 3. Substitute variables or return default
    match selected {
        Some(tpl) => substitute_variables(tpl, context),
        None => default_narration(outcome, context),
    }
}

/// Substitute variables in a template string
///
/// Replaces `{placeholder}` patterns with values from context.
fn substitute_variables(template: &str, ctx: &NarrationContext) -> String {
    let mut result = template.to_string();

    // Substitute {arg.X} from verb arguments
    for (key, value) in &ctx.args {
        result = result.replace(&format!("{{arg.{}}}", key), value);
    }

    // Substitute {result.X} from execution result
    for (key, value) in &ctx.result_fields {
        result = result.replace(&format!("{{result.{}}}", key), value);
    }

    // Substitute built-in variables
    result = result.replace("{verb}", &ctx.verb_fqn);
    result = result.replace("{domain}", &ctx.domain);
    result = result.replace("{error}", ctx.error.as_deref().unwrap_or(""));
    result = result.replace("{affected_count}", &ctx.affected_count.to_string());
    result = result.replace("{duration_ms}", &ctx.duration_ms.to_string());

    // Substitute entity names (comma-joined)
    let entity_names_str = ctx.entity_names.join(", ");
    result = result.replace("{entity_names}", &entity_names_str);

    // Substitute warnings (newline-joined)
    let warnings_str = ctx.warnings.join("; ");
    result = result.replace("{warnings}", &warnings_str);

    result
}

/// Generate default narration when no template is provided
fn default_narration(outcome: NarrationOutcome, ctx: &NarrationContext) -> String {
    match outcome {
        NarrationOutcome::Success => {
            if ctx.affected_count > 0 {
                format!(
                    "{} completed successfully ({} affected)",
                    ctx.verb_fqn, ctx.affected_count
                )
            } else {
                format!("{} completed successfully", ctx.verb_fqn)
            }
        }
        NarrationOutcome::Failure => {
            format!(
                "{} failed: {}",
                ctx.verb_fqn,
                ctx.error.as_deref().unwrap_or("Unknown error")
            )
        }
        NarrationOutcome::Preview => {
            format!("Will execute {}", ctx.verb_fqn)
        }
    }
}

// =============================================================================
// CONDITION EVALUATION
// =============================================================================

/// Evaluate a condition expression against the narration context
///
/// Supports simple expressions:
/// - `"affected_count > 10"` - numeric comparison
/// - `"affected_count == 0"` - equality
/// - `"has_warnings"` - boolean check (true if warnings not empty)
/// - `"arg.type == 'FUND'"` - string equality
fn evaluate_condition(condition: &str, ctx: &NarrationContext) -> bool {
    let condition = condition.trim();

    // Boolean checks
    if condition == "has_warnings" {
        return !ctx.warnings.is_empty();
    }
    if condition == "has_error" {
        return ctx.error.is_some();
    }
    if condition == "has_entity_names" {
        return !ctx.entity_names.is_empty();
    }

    // Parse comparison expressions
    if let Some((var, op, val)) = parse_comparison(condition) {
        return evaluate_comparison(&var, &op, &val, ctx);
    }

    // Unknown condition - return false
    false
}

/// Parse a comparison expression like "affected_count > 10" or "arg.type == 'FUND'"
fn parse_comparison(condition: &str) -> Option<(String, String, String)> {
    // Try operators in order of specificity
    for op in &[">=", "<=", "==", "!=", ">", "<"] {
        if let Some(pos) = condition.find(op) {
            let var = condition[..pos].trim().to_string();
            let val = condition[pos + op.len()..].trim().to_string();
            return Some((var, op.to_string(), val));
        }
    }
    None
}

/// Evaluate a comparison expression
fn evaluate_comparison(var: &str, op: &str, val: &str, ctx: &NarrationContext) -> bool {
    // Get the actual value from context
    let actual_value = get_context_value(var, ctx);

    // Try numeric comparison first
    if let (Ok(actual_num), Ok(expected_num)) = (
        actual_value.parse::<i64>(),
        val.trim_matches('\'').trim_matches('"').parse::<i64>(),
    ) {
        return match op {
            ">" => actual_num > expected_num,
            "<" => actual_num < expected_num,
            ">=" => actual_num >= expected_num,
            "<=" => actual_num <= expected_num,
            "==" => actual_num == expected_num,
            "!=" => actual_num != expected_num,
            _ => false,
        };
    }

    // String comparison (strip quotes from expected value)
    let expected_str = val.trim_matches('\'').trim_matches('"');
    match op {
        "==" => actual_value == expected_str,
        "!=" => actual_value != expected_str,
        _ => false, // <, >, <=, >= don't make sense for strings
    }
}

/// Get a value from the narration context by variable name
fn get_context_value(var: &str, ctx: &NarrationContext) -> String {
    // Built-in variables
    match var {
        "affected_count" => return ctx.affected_count.to_string(),
        "duration_ms" => return ctx.duration_ms.to_string(),
        "verb" => return ctx.verb_fqn.clone(),
        "domain" => return ctx.domain.clone(),
        _ => {}
    }

    // Namespaced variables
    if let Some(key) = var.strip_prefix("arg.") {
        return ctx.args.get(key).cloned().unwrap_or_default();
    }
    if let Some(key) = var.strip_prefix("result.") {
        return ctx.result_fields.get(key).cloned().unwrap_or_default();
    }

    // Unknown variable
    String::new()
}

// =============================================================================
// BUILDER
// =============================================================================

impl NarrationContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the verb FQN
    pub fn with_verb(mut self, verb_fqn: impl Into<String>) -> Self {
        self.verb_fqn = verb_fqn.into();
        if let Some(dot_pos) = self.verb_fqn.find('.') {
            self.domain = self.verb_fqn[..dot_pos].to_string();
        }
        self
    }

    /// Add an argument
    pub fn with_arg(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.args.insert(key.into(), value.into());
        self
    }

    /// Add a result field
    pub fn with_result(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.result_fields.insert(key.into(), value.into());
        self
    }

    /// Set the error message
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Set the affected count
    pub fn with_affected_count(mut self, count: usize) -> Self {
        self.affected_count = count;
        self
    }

    /// Set the duration
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Add warnings
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    /// Add entity names
    pub fn with_entity_names(mut self, names: Vec<String>) -> Self {
        self.entity_names = names;
        self
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_substitution() {
        let template = NarrationTemplate {
            success: Some("Created {arg.type} '{arg.name}'".to_string()),
            ..Default::default()
        };

        let context = NarrationContext::new()
            .with_verb("cbu.create")
            .with_arg("type", "FUND")
            .with_arg("name", "Acme Fund");

        let result = render_narration(&template, NarrationOutcome::Success, &context);
        assert_eq!(result, "Created FUND 'Acme Fund'");
    }

    #[test]
    fn test_result_substitution() {
        let template = NarrationTemplate {
            success: Some("Created with ID {result.id}".to_string()),
            ..Default::default()
        };

        let context = NarrationContext::new()
            .with_verb("cbu.create")
            .with_result("id", "abc-123");

        let result = render_narration(&template, NarrationOutcome::Success, &context);
        assert_eq!(result, "Created with ID abc-123");
    }

    #[test]
    fn test_failure_template() {
        let template = NarrationTemplate {
            failure: Some("Could not create: {error}".to_string()),
            ..Default::default()
        };

        let context = NarrationContext::new()
            .with_verb("cbu.create")
            .with_error("Name already exists");

        let result = render_narration(&template, NarrationOutcome::Failure, &context);
        assert_eq!(result, "Could not create: Name already exists");
    }

    #[test]
    fn test_preview_template() {
        let template = NarrationTemplate {
            preview: Some("This will create a new {arg.type} named '{arg.name}'".to_string()),
            ..Default::default()
        };

        let context = NarrationContext::new()
            .with_verb("cbu.create")
            .with_arg("type", "FUND")
            .with_arg("name", "Acme Fund");

        let result = render_narration(&template, NarrationOutcome::Preview, &context);
        assert_eq!(result, "This will create a new FUND named 'Acme Fund'");
    }

    #[test]
    fn test_default_narration() {
        let template = NarrationTemplate::default();

        let context = NarrationContext::new()
            .with_verb("cbu.create")
            .with_affected_count(5);

        let result = render_narration(&template, NarrationOutcome::Success, &context);
        assert_eq!(result, "cbu.create completed successfully (5 affected)");
    }

    #[test]
    fn test_conditional_override() {
        let template = NarrationTemplate {
            success: Some("Created 1 entity".to_string()),
            conditionals: vec![ConditionalNarration {
                condition: "affected_count > 1".to_string(),
                success: Some("Created {affected_count} entities".to_string()),
                failure: None,
            }],
            ..Default::default()
        };

        // Single entity - uses base template
        let context1 = NarrationContext::new()
            .with_verb("cbu.create")
            .with_affected_count(1);
        let result1 = render_narration(&template, NarrationOutcome::Success, &context1);
        assert_eq!(result1, "Created 1 entity");

        // Multiple entities - uses conditional
        let context2 = NarrationContext::new()
            .with_verb("cbu.create")
            .with_affected_count(10);
        let result2 = render_narration(&template, NarrationOutcome::Success, &context2);
        assert_eq!(result2, "Created 10 entities");
    }

    #[test]
    fn test_has_warnings_condition() {
        let template = NarrationTemplate {
            success: Some("Completed".to_string()),
            conditionals: vec![ConditionalNarration {
                condition: "has_warnings".to_string(),
                success: Some("Completed with warnings: {warnings}".to_string()),
                failure: None,
            }],
            ..Default::default()
        };

        // No warnings
        let context1 = NarrationContext::new().with_verb("cbu.create");
        let result1 = render_narration(&template, NarrationOutcome::Success, &context1);
        assert_eq!(result1, "Completed");

        // With warnings
        let context2 = NarrationContext::new()
            .with_verb("cbu.create")
            .with_warnings(vec![
                "Missing field".to_string(),
                "Low confidence".to_string(),
            ]);
        let result2 = render_narration(&template, NarrationOutcome::Success, &context2);
        assert_eq!(
            result2,
            "Completed with warnings: Missing field; Low confidence"
        );
    }

    #[test]
    fn test_string_equality_condition() {
        let template = NarrationTemplate {
            success: Some("Created entity".to_string()),
            conditionals: vec![ConditionalNarration {
                condition: "arg.type == 'FUND'".to_string(),
                success: Some("Created fund".to_string()),
                failure: None,
            }],
            ..Default::default()
        };

        // Non-fund type
        let context1 = NarrationContext::new()
            .with_verb("cbu.create")
            .with_arg("type", "MANDATE");
        let result1 = render_narration(&template, NarrationOutcome::Success, &context1);
        assert_eq!(result1, "Created entity");

        // Fund type
        let context2 = NarrationContext::new()
            .with_verb("cbu.create")
            .with_arg("type", "FUND");
        let result2 = render_narration(&template, NarrationOutcome::Success, &context2);
        assert_eq!(result2, "Created fund");
    }

    #[test]
    fn test_affected_count_zero() {
        let template = NarrationTemplate {
            success: Some("Found {affected_count} results".to_string()),
            conditionals: vec![ConditionalNarration {
                condition: "affected_count == 0".to_string(),
                success: Some("No results found".to_string()),
                failure: None,
            }],
            ..Default::default()
        };

        // Zero results
        let context1 = NarrationContext::new()
            .with_verb("entity.search")
            .with_affected_count(0);
        let result1 = render_narration(&template, NarrationOutcome::Success, &context1);
        assert_eq!(result1, "No results found");

        // Some results
        let context2 = NarrationContext::new()
            .with_verb("entity.search")
            .with_affected_count(5);
        let result2 = render_narration(&template, NarrationOutcome::Success, &context2);
        assert_eq!(result2, "Found 5 results");
    }

    #[test]
    fn test_entity_names_substitution() {
        let template = NarrationTemplate {
            success: Some("Resolved: {entity_names}".to_string()),
            ..Default::default()
        };

        let context = NarrationContext::new()
            .with_verb("scope.commit")
            .with_entity_names(vec![
                "Acme Corp".to_string(),
                "Beta Ltd".to_string(),
                "Gamma Inc".to_string(),
            ]);

        let result = render_narration(&template, NarrationOutcome::Success, &context);
        assert_eq!(result, "Resolved: Acme Corp, Beta Ltd, Gamma Inc");
    }
}
