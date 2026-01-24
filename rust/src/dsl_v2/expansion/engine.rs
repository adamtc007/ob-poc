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

    // Substitute parameters in template body
    let mut body = template.body.clone();

    if let Some(args_obj) = args.as_object() {
        for (key, value) in args_obj {
            let placeholder = format!("${}", key);
            let replacement = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => "".to_string(),
                // For complex values, serialize to JSON string
                _ => serde_json::to_string(value).unwrap_or_default(),
            };
            body = body.replace(&placeholder, &replacement);
        }
    }

    // Apply defaults for params not provided
    for (param_name, param_def) in &template.params {
        let placeholder = format!("${}", param_name);
        if body.contains(&placeholder) {
            if let Some(ref default_value) = param_def.default {
                body = body.replace(&placeholder, default_value);
            }
        }
    }

    // Check for unsubstituted placeholders
    let remaining_placeholders: Vec<_> = body
        .match_indices('$')
        .filter_map(|(i, _)| {
            let rest = &body[i..];
            let end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            let placeholder = &rest[..end];
            if placeholder.len() > 1 {
                Some(placeholder.to_string())
            } else {
                None
            }
        })
        .collect();

    for placeholder in remaining_placeholders {
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
        let mut locks = vec![
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
}
