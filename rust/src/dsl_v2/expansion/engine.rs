//! Expansion Engine
//!
//! Deterministic template expansion with audit trail generation.
//!
//! Key principles:
//! - Expansion is PURE: No database calls, no side effects
//! - Same input always produces identical output
//! - Lock set is sorted to prevent deadlocks
//! - All expansion details captured in ExpansionReport

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::types::{
    BatchPolicy, DiagnosticLevel, ExpansionDiagnostic, ExpansionReport, LockKey, LockingPolicy,
    PerItemOrigin, SpanRef, TemplateDigest, TemplateInvocationReport, TemplatePolicy,
};
use crate::templates::{TemplateDefinition, TemplateRegistry};

// =============================================================================
// PUBLIC API
// =============================================================================

/// Output from template expansion
#[derive(Debug)]
pub struct ExpansionOutput {
    /// The expanded DSL source (all templates replaced with verb calls)
    pub expanded_dsl: String,
    /// Audit report with full expansion details
    pub report: ExpansionReport,
}

/// Errors that can occur during expansion
#[derive(Debug, thiserror::Error)]
pub enum ExpansionError {
    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("Missing lock argument: {0}")]
    MissingLockArg(String),

    #[error("Invalid lock argument {arg}: {value} is not a valid UUID")]
    InvalidLockArg { arg: String, value: String },

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Missing required parameter: {param} for template {template}")]
    MissingParam { template: String, param: String },

    #[error("Template expansion failed: {0}")]
    ExpansionFailed(String),
}

/// Expand templates in DSL source deterministically (PURE - no DB calls)
///
/// This function:
/// 1. Identifies template invocations in the source
/// 2. Expands each template with provided args
/// 3. Derives lock keys from policy + runtime args
/// 4. Produces deterministic output and audit report
///
/// # Arguments
///
/// * `source_dsl` - The DSL source text (may contain template invocations)
/// * `template_registry` - Registry of available templates
/// * `template_args` - Arguments for template invocations (template_name → args)
///
/// # Returns
///
/// `ExpansionOutput` containing expanded DSL and audit report
pub fn expand_templates(
    source_dsl: &str,
    template_registry: &TemplateRegistry,
    template_args: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<ExpansionOutput, ExpansionError> {
    let expansion_id = Uuid::new_v4();
    let source_digest = hash_canonical(source_dsl);

    let mut template_digests = Vec::new();
    let mut invocations = Vec::new();
    let mut derived_locks = Vec::new();
    let mut batch_policy = BatchPolicy::BestEffort;
    let mut diagnostics = Vec::new();

    // Parse source to identify template invocations
    let parsed = parse_for_expansion(source_dsl)?;

    let mut expanded_statements = Vec::new();

    for node in parsed.nodes {
        match node {
            ParsedNode::TemplateInvocation(invocation) => {
                // Look up template
                let template = template_registry
                    .get(&invocation.name)
                    .ok_or_else(|| ExpansionError::TemplateNotFound(invocation.name.clone()))?;

                // Record template digest (for audit)
                if !template_digests
                    .iter()
                    .any(|d: &TemplateDigest| d.name == template.template)
                {
                    template_digests.push(TemplateDigest {
                        name: template.template.clone(),
                        version: template.version.to_string(),
                        digest: hash_canonical(&template.body),
                    });
                }

                // Get args for this invocation
                let args = template_args
                    .get(&invocation.name)
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                // Expand template with provided args
                let start_index = expanded_statements.len();
                let (expanded, expansion_diagnostics) =
                    expand_single_template(template, &args, &invocation.name)?;

                diagnostics.extend(expansion_diagnostics);

                // Track per-item origins
                let per_item_origins: Vec<_> = expanded
                    .iter()
                    .enumerate()
                    .map(|(i, _)| PerItemOrigin {
                        expanded_statement_index: start_index + i,
                        template_item_index: i,
                    })
                    .collect();

                expanded_statements.extend(expanded);

                // Build template policy
                let policy = build_template_policy(template);

                // Record invocation
                invocations.push(TemplateInvocationReport {
                    name: invocation.name.clone(),
                    args_json: args.clone(),
                    policy: policy.clone(),
                    origin_span: invocation.span.clone(),
                    expanded_range: super::types::ExpandedRange {
                        start_index,
                        end_index_exclusive: expanded_statements.len(),
                    },
                    per_item_origins,
                });

                // Derive locks from template policy + args
                if let Some(ref locking) = policy.locking {
                    match derive_locks_from_policy(locking, &args) {
                        Ok(locks) => derived_locks.extend(locks),
                        Err(e) => {
                            diagnostics.push(ExpansionDiagnostic {
                                level: DiagnosticLevel::Warning,
                                message: format!("Could not derive locks: {}", e),
                                path: format!("template.{}", invocation.name),
                            });
                        }
                    }
                }

                // Escalate to atomic if any template requests it
                if matches!(policy.batch_policy, BatchPolicy::Atomic) {
                    batch_policy = BatchPolicy::Atomic;
                }
            }
            ParsedNode::AtomicStatement(stmt) => {
                // Pass through unchanged
                expanded_statements.push(stmt);
            }
        }
    }

    // Sort locks to prevent deadlocks (CRITICAL)
    derived_locks.sort();
    derived_locks.dedup();

    // Build expanded DSL string
    let expanded_dsl = statements_to_dsl(&expanded_statements);
    let expanded_dsl_digest = hash_canonical(&expanded_dsl);

    Ok(ExpansionOutput {
        expanded_dsl,
        report: ExpansionReport {
            expansion_id,
            source_digest,
            template_digests,
            invocations,
            expanded_statement_count: expanded_statements.len(),
            expanded_dsl_digest,
            derived_lock_set: derived_locks,
            batch_policy,
            diagnostics,
            expanded_at: chrono::Utc::now(),
        },
    })
}

/// Expand templates without template arguments (uses empty args for all)
///
/// Convenience function for DSL that doesn't use templates or has templates
/// with all defaults/session-derived params.
pub fn expand_templates_simple(
    source_dsl: &str,
    template_registry: &TemplateRegistry,
) -> Result<ExpansionOutput, ExpansionError> {
    expand_templates(
        source_dsl,
        template_registry,
        &std::collections::HashMap::new(),
    )
}

// =============================================================================
// INTERNAL PARSING
// =============================================================================

/// Parsed representation of DSL for expansion
struct ParsedForExpansion {
    nodes: Vec<ParsedNode>,
}

/// A node in the parsed DSL
#[allow(dead_code)] // TemplateInvocation will be used when inline template syntax is implemented
enum ParsedNode {
    /// A template invocation: @template_name { args }
    TemplateInvocation(TemplateInvocation),
    /// An atomic statement (regular verb call)
    AtomicStatement(String),
}

/// A template invocation found in DSL
#[allow(dead_code)] // Will be used when inline template syntax is implemented
struct TemplateInvocation {
    /// Template name
    name: String,
    /// Source span (if available)
    span: Option<SpanRef>,
}

/// Parse DSL source to identify template invocations
///
/// Templates are invoked with syntax: @template_name or @template_name { args }
///
/// For now, this is a simple implementation that passes through all DSL
/// unchanged since templates are expanded via separate parameter passing.
fn parse_for_expansion(source_dsl: &str) -> Result<ParsedForExpansion, ExpansionError> {
    // For the current implementation, templates are invoked via the template
    // registry and args are passed separately. The DSL source itself contains
    // verb calls, not template invocations.
    //
    // If we need to support inline template syntax like @template_name { args },
    // we would parse it here. For now, treat entire source as atomic statements.

    let nodes = if source_dsl.trim().is_empty() {
        Vec::new()
    } else {
        // Split by semicolons or newlines to get individual statements
        // For now, treat entire source as one block
        vec![ParsedNode::AtomicStatement(source_dsl.to_string())]
    };

    Ok(ParsedForExpansion { nodes })
}

// =============================================================================
// TEMPLATE EXPANSION
// =============================================================================

/// Expand a single template with given arguments
///
/// Returns (expanded_statements, diagnostics)
fn expand_single_template(
    template: &TemplateDefinition,
    args: &serde_json::Value,
    template_name: &str,
) -> Result<(Vec<String>, Vec<ExpansionDiagnostic>), ExpansionError> {
    let mut diagnostics = Vec::new();

    // Check for missing required parameters
    for (param_name, param_def) in &template.params {
        if param_def.required && param_def.source.is_none() && param_def.default.is_none() {
            // This param needs to be provided
            if !args
                .as_object()
                .map(|o| o.contains_key(param_name))
                .unwrap_or(false)
            {
                diagnostics.push(ExpansionDiagnostic {
                    level: DiagnosticLevel::Warning,
                    message: format!("Required parameter '{}' not provided", param_name),
                    path: format!("template.{}.params.{}", template_name, param_name),
                });
            }
        }
    }

    // Substitute variables in template body using the new variable grammar
    let (body, substitution_diagnostics) =
        substitute_variables(&template.body, args, template_name);
    diagnostics.extend(substitution_diagnostics);

    // Apply defaults for params not provided (simple $param format only)
    let mut body = body;
    for (param_name, param_def) in &template.params {
        let placeholder = format!("${}", param_name);
        if body.contains(&placeholder) {
            if let Some(ref default_value) = param_def.default {
                body = body.replace(&placeholder, default_value);
            }
        }
    }

    // Check for unsubstituted placeholders
    let remaining = find_remaining_placeholders(&body);
    for placeholder in remaining {
        diagnostics.push(ExpansionDiagnostic {
            level: DiagnosticLevel::Warning,
            message: format!("Unsubstituted placeholder: {}", placeholder),
            path: format!("template.{}.body", template_name),
        });
    }

    // Split body into individual statements (by line, filtering empty)
    let statements: Vec<String> = body
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with(';'))
        .map(|l| l.to_string())
        .collect();

    Ok((statements, diagnostics))
}

// =============================================================================
// VARIABLE SUBSTITUTION
// =============================================================================

/// Substitute variables in a template string
///
/// Supports the following variable grammar (per architecture doc):
/// - `${arg.X}` - Direct arg value
/// - `${arg.X.internal}` - Internal value for enum args (key → internal mapping)
/// - `${scope.X}` - Scope context value (client_id, etc.)
/// - `${session.X}` - Session field value
///
/// For enum args, the value can be either:
/// - A simple string (the key, e.g., "pe")
/// - An object with `key` and `internal` fields
///
/// Returns (substituted_string, diagnostics)
fn substitute_variables(
    template: &str,
    args: &serde_json::Value,
    template_name: &str,
) -> (String, Vec<ExpansionDiagnostic>) {
    let mut result = template.to_string();
    let mut diagnostics = Vec::new();

    // Regex would be cleaner, but let's use simple string scanning for now
    // Pattern: ${namespace.path} or ${namespace.path.modifier}

    // Find all ${...} patterns
    let patterns = find_variable_patterns(&result);

    for pattern in patterns.into_iter().rev() {
        // Process in reverse order to preserve indices
        let var_content = &pattern.content;
        let replacement = resolve_variable(var_content, args, &mut diagnostics, template_name);

        result.replace_range(pattern.start..pattern.end, &replacement);
    }

    (result, diagnostics)
}

/// A variable pattern found in the template
struct VariablePattern {
    start: usize,
    end: usize,
    content: String, // The content inside ${...}
}

/// Find all ${...} patterns in a string
fn find_variable_patterns(s: &str) -> Vec<VariablePattern> {
    let mut patterns = Vec::new();
    let mut i = 0;
    let bytes = s.as_bytes();

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
            // Found start of pattern
            let start = i;
            i += 2;
            let content_start = i;

            // Find closing brace
            let mut depth = 1;
            while i < bytes.len() && depth > 0 {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    i += 1;
                }
            }

            if depth == 0 {
                let content = s[content_start..i].to_string();
                patterns.push(VariablePattern {
                    start,
                    end: i + 1, // Include closing brace
                    content,
                });
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    patterns
}

/// Resolve a variable reference to its value
fn resolve_variable(
    var_content: &str,
    args: &serde_json::Value,
    diagnostics: &mut Vec<ExpansionDiagnostic>,
    template_name: &str,
) -> String {
    let parts: Vec<&str> = var_content.split('.').collect();

    if parts.is_empty() {
        diagnostics.push(ExpansionDiagnostic {
            level: DiagnosticLevel::Warning,
            message: format!("Empty variable reference: ${{{}}}", var_content),
            path: format!("template.{}", template_name),
        });
        return format!("${{{}}}", var_content);
    }

    match parts[0] {
        "arg" => resolve_arg_variable(&parts[1..], args, diagnostics, template_name, var_content),
        "scope" => {
            // Scope variables are passed as args with "scope." prefix
            let scope_key = format!("scope.{}", parts[1..].join("."));
            if let Some(value) = args.get(&scope_key) {
                value_to_string(value)
            } else if let Some(value) = args.get(parts.get(1).unwrap_or(&"")) {
                // Fallback: try direct key
                value_to_string(value)
            } else {
                diagnostics.push(ExpansionDiagnostic {
                    level: DiagnosticLevel::Warning,
                    message: format!("Scope variable not found: {}", var_content),
                    path: format!("template.{}", template_name),
                });
                format!("${{{}}}", var_content)
            }
        }
        "session" => {
            // Session variables are passed as args with "session." prefix
            let session_key = format!("session.{}", parts[1..].join("."));
            if let Some(value) = args.get(&session_key) {
                value_to_string(value)
            } else {
                diagnostics.push(ExpansionDiagnostic {
                    level: DiagnosticLevel::Warning,
                    message: format!("Session variable not found: {}", var_content),
                    path: format!("template.{}", template_name),
                });
                format!("${{{}}}", var_content)
            }
        }
        _ => {
            // Treat as simple arg reference (backwards compatibility)
            resolve_arg_variable(&parts, args, diagnostics, template_name, var_content)
        }
    }
}

/// Resolve an arg.* variable reference
fn resolve_arg_variable(
    path: &[&str],
    args: &serde_json::Value,
    diagnostics: &mut Vec<ExpansionDiagnostic>,
    template_name: &str,
    full_var: &str,
) -> String {
    if path.is_empty() {
        diagnostics.push(ExpansionDiagnostic {
            level: DiagnosticLevel::Warning,
            message: format!("Empty arg path: ${{{}}}", full_var),
            path: format!("template.{}", template_name),
        });
        return format!("${{{}}}", full_var);
    }

    let arg_name = path[0];
    let modifier = path.get(1).copied();

    // Get the arg value
    let arg_value = match args.get(arg_name) {
        Some(v) => v,
        None => {
            // Not found - leave placeholder for default handling
            return format!("${{{}}}", full_var);
        }
    };

    match modifier {
        Some("internal") => {
            // For enum args: extract the internal value
            // The arg can be:
            // - A string (the key itself, resolve via enum definition - but we don't have that here)
            // - An object with { key, internal } fields
            match arg_value {
                serde_json::Value::Object(obj) => {
                    if let Some(internal) = obj.get("internal") {
                        value_to_string(internal)
                    } else {
                        diagnostics.push(ExpansionDiagnostic {
                            level: DiagnosticLevel::Warning,
                            message: format!(
                                "Arg '{}' has no 'internal' field for .internal access",
                                arg_name
                            ),
                            path: format!("template.{}.args.{}", template_name, arg_name),
                        });
                        value_to_string(arg_value)
                    }
                }
                serde_json::Value::String(s) => {
                    // String value - assume it's already the internal value
                    // In a full implementation, we'd look up the enum definition
                    s.clone()
                }
                _ => {
                    diagnostics.push(ExpansionDiagnostic {
                        level: DiagnosticLevel::Warning,
                        message: format!(
                            "Arg '{}' is not an enum (cannot use .internal)",
                            arg_name
                        ),
                        path: format!("template.{}.args.{}", template_name, arg_name),
                    });
                    value_to_string(arg_value)
                }
            }
        }
        Some("key") => {
            // For enum args: extract the key value
            match arg_value {
                serde_json::Value::Object(obj) => {
                    if let Some(key) = obj.get("key") {
                        value_to_string(key)
                    } else {
                        value_to_string(arg_value)
                    }
                }
                _ => value_to_string(arg_value),
            }
        }
        Some("label") => {
            // For enum args: extract the label value
            match arg_value {
                serde_json::Value::Object(obj) => {
                    if let Some(label) = obj.get("label") {
                        value_to_string(label)
                    } else {
                        value_to_string(arg_value)
                    }
                }
                _ => value_to_string(arg_value),
            }
        }
        Some(other) => {
            // Unknown modifier - try to access as object property
            match arg_value {
                serde_json::Value::Object(obj) => {
                    if let Some(v) = obj.get(other) {
                        value_to_string(v)
                    } else {
                        diagnostics.push(ExpansionDiagnostic {
                            level: DiagnosticLevel::Warning,
                            message: format!("Unknown property '{}' on arg '{}'", other, arg_name),
                            path: format!("template.{}.args.{}", template_name, arg_name),
                        });
                        format!("${{{}}}", full_var)
                    }
                }
                _ => {
                    diagnostics.push(ExpansionDiagnostic {
                        level: DiagnosticLevel::Warning,
                        message: format!(
                            "Cannot access property '{}' on non-object arg '{}'",
                            other, arg_name
                        ),
                        path: format!("template.{}.args.{}", template_name, arg_name),
                    });
                    format!("${{{}}}", full_var)
                }
            }
        }
        None => {
            // No modifier - return the value directly
            // For objects with 'key', return the key for backwards compatibility
            match arg_value {
                serde_json::Value::Object(obj) => {
                    if let Some(key) = obj.get("key") {
                        value_to_string(key)
                    } else {
                        value_to_string(arg_value)
                    }
                }
                _ => value_to_string(arg_value),
            }
        }
    }
}

/// Convert a JSON value to a string for substitution
fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        // For arrays, join with comma
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        // For objects, serialize to JSON
        serde_json::Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

/// Find remaining unsubstituted placeholders
fn find_remaining_placeholders(body: &str) -> Vec<String> {
    let mut placeholders = Vec::new();

    // Find ${...} patterns that weren't substituted
    for pattern in find_variable_patterns(body) {
        placeholders.push(format!("${{{}}}", pattern.content));
    }

    // Also find simple $param patterns
    let mut i = 0;
    let chars: Vec<char> = body.chars().collect();
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] != '{' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let placeholder: String = chars[start..i].iter().collect();
            if placeholder.len() > 1 {
                placeholders.push(placeholder);
            }
        } else {
            i += 1;
        }
    }

    placeholders
}

/// Build template policy from template definition
fn build_template_policy(_template: &TemplateDefinition) -> TemplatePolicy {
    // For now, use default policy
    // In Phase 2.2, we'll add policy metadata to TemplateDefinition
    TemplatePolicy {
        batch_policy: BatchPolicy::BestEffort,
        locking: None,
    }
}

// =============================================================================
// LOCK DERIVATION
// =============================================================================

/// Derive concrete lock keys from policy + runtime args
fn derive_locks_from_policy(
    policy: &LockingPolicy,
    args: &serde_json::Value,
) -> Result<Vec<LockKey>, ExpansionError> {
    let mut locks = Vec::new();

    let args_obj = args.as_object();

    for target in &policy.targets {
        let entity_id = args_obj
            .and_then(|o| o.get(&target.arg))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExpansionError::MissingLockArg(target.arg.clone()))?;

        // Validate it looks like a UUID
        if Uuid::parse_str(entity_id).is_err() {
            return Err(ExpansionError::InvalidLockArg {
                arg: target.arg.clone(),
                value: entity_id.to_string(),
            });
        }

        locks.push(LockKey {
            entity_type: target.entity_type.clone(),
            entity_id: entity_id.to_string(),
            access: target.access,
        });
    }

    Ok(locks)
}

// =============================================================================
// HASHING
// =============================================================================

/// Hash content canonically (stable across runs)
///
/// Canonicalization:
/// - Collapse all whitespace to single spaces
/// - Trim leading/trailing whitespace
fn hash_canonical(content: &str) -> String {
    let canonical = canonicalize_whitespace(content);
    let hash = Sha256::digest(canonical.as_bytes());
    hex::encode(hash)
}

/// Canonicalize whitespace for deterministic hashing
fn canonicalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// =============================================================================
// OUTPUT
// =============================================================================

/// Convert expanded statements back to DSL source string
fn statements_to_dsl(statements: &[String]) -> String {
    statements.join("\n")
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_canonical_determinism() {
        let input1 = "(verb.call :arg1 \"value\" :arg2 123)";
        let input2 = "(verb.call  :arg1  \"value\"  :arg2  123)";
        let input3 = "(verb.call\n  :arg1 \"value\"\n  :arg2 123)";

        let hash1 = hash_canonical(input1);
        let hash2 = hash_canonical(input2);
        let hash3 = hash_canonical(input3);

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn test_canonicalize_whitespace() {
        let input = "  hello   world\n\tfoo  ";
        let output = canonicalize_whitespace(input);
        assert_eq!(output, "hello world foo");
    }

    #[test]
    fn test_lock_key_sorting() {
        let mut locks = [
            LockKey::write("person", "uuid-3"),
            LockKey::write("cbu", "uuid-1"),
            LockKey::read("person", "uuid-2"),
        ];

        locks.sort();

        assert_eq!(locks[0].entity_type, "cbu");
        assert_eq!(locks[1].entity_type, "person");
        assert_eq!(locks[1].entity_id, "uuid-2");
        assert_eq!(locks[2].entity_type, "person");
        assert_eq!(locks[2].entity_id, "uuid-3");
    }

    #[test]
    fn test_derive_locks_from_policy() {
        use super::super::types::{LockAccess, LockTarget};

        let policy = LockingPolicy {
            mode: super::super::types::LockMode::Try,
            timeout_ms: None,
            targets: vec![
                LockTarget {
                    arg: "person-id".to_string(),
                    entity_type: "person".to_string(),
                    access: LockAccess::Write,
                },
                LockTarget {
                    arg: "cbu-id".to_string(),
                    entity_type: "cbu".to_string(),
                    access: LockAccess::Read,
                },
            ],
        };

        let args = serde_json::json!({
            "person-id": "550e8400-e29b-41d4-a716-446655440000",
            "cbu-id": "550e8400-e29b-41d4-a716-446655440001"
        });

        let locks = derive_locks_from_policy(&policy, &args).unwrap();
        assert_eq!(locks.len(), 2);

        assert_eq!(locks[0].entity_type, "person");
        assert_eq!(locks[0].access, LockAccess::Write);

        assert_eq!(locks[1].entity_type, "cbu");
        assert_eq!(locks[1].access, LockAccess::Read);
    }

    #[test]
    fn test_derive_locks_missing_arg() {
        use super::super::types::{LockAccess, LockTarget};

        let policy = LockingPolicy {
            mode: super::super::types::LockMode::Try,
            timeout_ms: None,
            targets: vec![LockTarget {
                arg: "missing-arg".to_string(),
                entity_type: "entity".to_string(),
                access: LockAccess::Write,
            }],
        };

        let args = serde_json::json!({});

        let result = derive_locks_from_policy(&policy, &args);
        assert!(matches!(result, Err(ExpansionError::MissingLockArg(_))));
    }

    #[test]
    fn test_derive_locks_invalid_uuid() {
        use super::super::types::{LockAccess, LockTarget};

        let policy = LockingPolicy {
            mode: super::super::types::LockMode::Try,
            timeout_ms: None,
            targets: vec![LockTarget {
                arg: "entity-id".to_string(),
                entity_type: "entity".to_string(),
                access: LockAccess::Write,
            }],
        };

        let args = serde_json::json!({
            "entity-id": "not-a-uuid"
        });

        let result = derive_locks_from_policy(&policy, &args);
        assert!(matches!(result, Err(ExpansionError::InvalidLockArg { .. })));
    }

    #[test]
    fn test_expand_simple_passthrough() {
        let registry = TemplateRegistry::new();
        let source = "(cbu.create :name \"Test\" :as @cbu)";

        let result = expand_templates_simple(source, &registry).unwrap();

        // Should pass through unchanged since no templates
        assert_eq!(result.expanded_dsl, source);
        assert!(result.report.template_digests.is_empty());
        assert!(result.report.invocations.is_empty());
        assert_eq!(result.report.batch_policy, BatchPolicy::BestEffort);
    }

    #[test]
    fn test_expansion_report_determinism() {
        let registry = TemplateRegistry::new();
        let source = "(cbu.create :name \"Test\" :as @cbu)";

        let result1 = expand_templates_simple(source, &registry).unwrap();
        let result2 = expand_templates_simple(source, &registry).unwrap();

        // Same source digest
        assert_eq!(result1.report.source_digest, result2.report.source_digest);
        // Same expanded digest
        assert_eq!(
            result1.report.expanded_dsl_digest,
            result2.report.expanded_dsl_digest
        );
        // Same expanded DSL
        assert_eq!(result1.expanded_dsl, result2.expanded_dsl);
    }

    // =========================================================================
    // Variable Substitution Tests
    // =========================================================================

    #[test]
    fn test_find_variable_patterns_basic() {
        let patterns = find_variable_patterns("Hello ${arg.name} world");
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].content, "arg.name");
        assert_eq!(patterns[0].start, 6);
        assert_eq!(patterns[0].end, 17); // end is exclusive, points after closing }
    }

    #[test]
    fn test_find_variable_patterns_multiple() {
        let patterns = find_variable_patterns("${arg.x} and ${scope.y} and ${session.z}");
        assert_eq!(patterns.len(), 3);
        assert_eq!(patterns[0].content, "arg.x");
        assert_eq!(patterns[1].content, "scope.y");
        assert_eq!(patterns[2].content, "session.z");
    }

    #[test]
    fn test_find_variable_patterns_with_modifier() {
        let patterns = find_variable_patterns("${arg.structure_type.internal}");
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].content, "arg.structure_type.internal");
    }

    #[test]
    fn test_find_variable_patterns_none() {
        let patterns = find_variable_patterns("No variables here");
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_resolve_arg_variable_simple() {
        let args = serde_json::json!({
            "name": "Acme Fund"
        });
        let mut diagnostics = Vec::new();

        let result =
            resolve_arg_variable(&["name"], &args, &mut diagnostics, "test.verb", "arg.name");

        assert_eq!(result, "Acme Fund");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_resolve_arg_variable_internal_modifier() {
        let args = serde_json::json!({
            "structure_type": {
                "key": "pe",
                "internal": "private-equity",
                "label": "Private Equity"
            }
        });
        let mut diagnostics = Vec::new();

        let result = resolve_arg_variable(
            &["structure_type", "internal"],
            &args,
            &mut diagnostics,
            "test.verb",
            "arg.structure_type.internal",
        );

        assert_eq!(result, "private-equity");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_resolve_arg_variable_key_modifier() {
        let args = serde_json::json!({
            "structure_type": {
                "key": "pe",
                "internal": "private-equity",
                "label": "Private Equity"
            }
        });
        let mut diagnostics = Vec::new();

        let result = resolve_arg_variable(
            &["structure_type", "key"],
            &args,
            &mut diagnostics,
            "test.verb",
            "arg.structure_type.key",
        );

        assert_eq!(result, "pe");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_resolve_arg_variable_label_modifier() {
        let args = serde_json::json!({
            "structure_type": {
                "key": "pe",
                "internal": "private-equity",
                "label": "Private Equity"
            }
        });
        let mut diagnostics = Vec::new();

        let result = resolve_arg_variable(
            &["structure_type", "label"],
            &args,
            &mut diagnostics,
            "test.verb",
            "arg.structure_type.label",
        );

        assert_eq!(result, "Private Equity");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_resolve_arg_variable_missing() {
        let args = serde_json::json!({});
        let mut diagnostics = Vec::new();

        let result = resolve_arg_variable(
            &["missing"],
            &args,
            &mut diagnostics,
            "test.verb",
            "arg.missing",
        );

        // Missing args return placeholder without diagnostic (allows default handling)
        assert_eq!(result, "${arg.missing}");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_substitute_variables_full() {
        let args = serde_json::json!({
            "name": "Test Fund",
            "structure_type": {
                "key": "pe",
                "internal": "private-equity",
                "label": "Private Equity"
            }
        });

        let template =
            "(cbu.create :name \"${arg.name}\" :kind \"${arg.structure_type.internal}\")";
        let (result, diagnostics) = substitute_variables(template, &args, "test.macro");

        assert_eq!(
            result,
            "(cbu.create :name \"Test Fund\" :kind \"private-equity\")"
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_substitute_variables_preserves_non_variables() {
        let args = serde_json::json!({
            "name": "Test"
        });

        let template = "Hello ${arg.name}, welcome to $regular_var and {braces}";
        let (result, _) = substitute_variables(template, &args, "test.macro");

        assert_eq!(result, "Hello Test, welcome to $regular_var and {braces}");
    }

    #[test]
    fn test_value_to_string_types() {
        assert_eq!(value_to_string(&serde_json::json!("hello")), "hello");
        assert_eq!(value_to_string(&serde_json::json!(123)), "123");
        assert_eq!(value_to_string(&serde_json::json!(true)), "true");
        assert_eq!(value_to_string(&serde_json::json!(null)), ""); // null → empty string
        assert_eq!(value_to_string(&serde_json::json!(12.5)), "12.5");
    }

    #[test]
    fn test_find_remaining_placeholders() {
        let body = "(verb :a \"${arg.x}\" :b \"${scope.y}\" :c \"value\")";
        let remaining = find_remaining_placeholders(body);

        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&"${arg.x}".to_string()));
        assert!(remaining.contains(&"${scope.y}".to_string()));
    }

    #[test]
    fn test_find_remaining_placeholders_none() {
        let body = "(verb :a \"hello\" :b 123)";
        let remaining = find_remaining_placeholders(body);

        assert!(remaining.is_empty());
    }
}
