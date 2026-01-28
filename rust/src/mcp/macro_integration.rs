//! Macro Integration for Intent Pipeline
//!
//! Provides macro-aware verb processing. When a verb is a macro (from the operator
//! vocabulary), it's expanded to primitive DSL statements. Otherwise, processing
//! continues normally.
//!
//! ## Usage
//!
//! ```ignore
//! // In intent pipeline after verb discovery:
//! if let Some(expansion) = try_expand_macro(&verb_fqn, &args, &session, &macro_registry) {
//!     // Use expansion.statements as DSL instead of normal assembly
//! }
//! ```

use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::Result;
use tracing::{debug, info};

use crate::dsl_v2::macros::{
    expand_macro, load_macro_registry, MacroExpansionError, MacroExpansionOutput, MacroPrereq,
    MacroRegistry, MacroSchema,
};
use crate::session::unified::{DagState, PrereqCondition, UnifiedSession};

use super::intent_pipeline::{IntentArgValue, StructuredIntent};

/// Global macro registry singleton
static MACRO_REGISTRY: OnceLock<MacroRegistry> = OnceLock::new();

/// Get or initialize the global macro registry
pub fn macro_registry() -> &'static MacroRegistry {
    MACRO_REGISTRY.get_or_init(|| {
        load_macro_registry().unwrap_or_else(|e| {
            tracing::warn!("Failed to load macro registry: {}", e);
            MacroRegistry::new()
        })
    })
}

/// Initialize the global macro registry (call during server startup)
pub fn init_macro_registry() -> Result<()> {
    let registry = load_macro_registry()?;
    info!(
        "Loaded {} macros from {} files",
        registry.len(),
        registry.source_files().len()
    );
    MACRO_REGISTRY
        .set(registry)
        .map_err(|_| anyhow::anyhow!("Macro registry already initialized"))?;
    Ok(())
}

/// Check if a verb is a macro
pub fn is_macro(verb_fqn: &str) -> bool {
    macro_registry().has(verb_fqn)
}

/// Result of macro expansion attempt
#[derive(Debug)]
pub enum MacroAttemptResult {
    /// Verb is not a macro - continue normal processing
    NotAMacro,
    /// Macro expanded successfully
    Expanded(MacroExpansionOutput),
    /// Macro expansion failed
    Failed(MacroExpansionError),
}

/// Try to expand a verb as a macro
///
/// Returns `MacroAttemptResult::NotAMacro` if the verb is not in the macro registry,
/// allowing normal processing to continue.
pub fn try_expand_macro(
    verb_fqn: &str,
    args: &HashMap<String, String>,
    session: &UnifiedSession,
) -> MacroAttemptResult {
    let registry = macro_registry();

    if !registry.has(verb_fqn) {
        return MacroAttemptResult::NotAMacro;
    }

    debug!(verb = verb_fqn, "Attempting macro expansion");

    match expand_macro(verb_fqn, args, session, registry) {
        Ok(output) => {
            info!(
                verb = verb_fqn,
                statements = output.statements.len(),
                "Macro expanded successfully"
            );
            MacroAttemptResult::Expanded(output)
        }
        Err(e) => {
            tracing::warn!(verb = verb_fqn, error = %e, "Macro expansion failed");
            MacroAttemptResult::Failed(e)
        }
    }
}

/// Convert intent arguments to macro expansion args
///
/// Extracts string values from IntentArguments for macro expansion.
/// Enum values use the UI key (macro expansion handles internal mapping).
pub fn intent_args_to_macro_args(intent: &StructuredIntent) -> HashMap<String, String> {
    let mut args = HashMap::new();

    for arg in &intent.arguments {
        if let Some(value) = extract_string_value(&arg.value) {
            args.insert(arg.name.clone(), value);
        }
    }

    args
}

/// Extract string value from IntentArgValue
fn extract_string_value(value: &IntentArgValue) -> Option<String> {
    match value {
        IntentArgValue::String(s) => Some(s.clone()),
        IntentArgValue::Number(n) => Some(n.to_string()),
        IntentArgValue::Boolean(b) => Some(b.to_string()),
        IntentArgValue::Uuid(u) => Some(u.clone()),
        IntentArgValue::Reference(r) => Some(format!("@{}", r)),
        IntentArgValue::Unresolved { value, .. } => Some(value.clone()),
        IntentArgValue::Missing { .. } => None,
        IntentArgValue::List(items) => {
            // Join list items with commas
            let values: Vec<String> = items.iter().filter_map(extract_string_value).collect();
            if values.is_empty() {
                None
            } else {
                Some(values.join(","))
            }
        }
        IntentArgValue::Map(_) => None, // Maps not directly convertible
    }
}

/// Get macro schema for a verb (for UI display)
pub fn get_macro_schema(verb_fqn: &str) -> Option<&'static crate::dsl_v2::macros::MacroSchema> {
    macro_registry().get(verb_fqn)
}

/// List all available macros
pub fn list_macros() -> Vec<MacroInfo> {
    macro_registry()
        .all()
        .map(|(fqn, schema)| MacroInfo {
            fqn: fqn.clone(),
            label: schema.ui.label.clone(),
            description: schema.ui.description.clone(),
            target_label: schema.ui.target_label.clone(),
            mode_tags: schema.routing.mode_tags.clone(),
            operator_domain: schema.routing.operator_domain.clone(),
        })
        .collect()
}

/// Macro information for UI display
#[derive(Debug, Clone)]
pub struct MacroInfo {
    pub fqn: String,
    pub label: String,
    pub description: String,
    pub target_label: String,
    pub mode_tags: Vec<String>,
    pub operator_domain: Option<String>,
}

/// Get macros filtered by mode tag
pub fn macros_by_mode(mode_tag: &str) -> Vec<MacroInfo> {
    macro_registry()
        .by_mode_tag(mode_tag)
        .into_iter()
        .filter_map(|schema| {
            // Find FQN by searching registry
            macro_registry()
                .all()
                .find(|(_, s)| std::ptr::eq(*s, schema))
                .map(|(fqn, s)| MacroInfo {
                    fqn: fqn.clone(),
                    label: s.ui.label.clone(),
                    description: s.ui.description.clone(),
                    target_label: s.ui.target_label.clone(),
                    mode_tags: s.routing.mode_tags.clone(),
                    operator_domain: s.routing.operator_domain.clone(),
                })
        })
        .collect()
}

// =============================================================================
// VERB READINESS (Phase 5: DAG Navigation)
// =============================================================================

/// Verb readiness status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerbReadiness {
    /// Verb can be executed (all prereqs satisfied)
    Ready,
    /// Verb is blocked by unmet prerequisites
    Blocked { missing: Vec<String> },
}

/// Information about a verb's readiness for UI display
#[derive(Debug, Clone)]
pub struct VerbReadinessInfo {
    pub fqn: String,
    pub label: String,
    pub description: String,
    pub readiness: VerbReadiness,
    /// Verbs that this will unlock when executed
    pub unlocks: Vec<String>,
}

/// Convert macro prereq to unified PrereqCondition
fn macro_prereq_to_condition(prereq: &MacroPrereq) -> PrereqCondition {
    match prereq {
        MacroPrereq::StateExists { key } => PrereqCondition::StateExists { key: key.clone() },
        MacroPrereq::VerbCompleted { verb } => {
            PrereqCondition::VerbCompleted { verb: verb.clone() }
        }
        MacroPrereq::FactExists { predicate } => PrereqCondition::FactExists {
            predicate: predicate.clone(),
        },
        MacroPrereq::AnyOf { conditions } => {
            // Convert nested conditions - for AnyOf, we only support verb completions
            let verbs: Vec<String> = conditions
                .iter()
                .filter_map(|c| {
                    if let MacroPrereq::VerbCompleted { verb } = c {
                        Some(verb.clone())
                    } else {
                        None
                    }
                })
                .collect();
            PrereqCondition::AnyOf { verbs }
        }
    }
}

/// Check if a macro's prereqs are satisfied
pub fn check_macro_prereqs(schema: &MacroSchema, dag_state: &DagState) -> VerbReadiness {
    let mut missing = Vec::new();

    for prereq in &schema.prereqs {
        let condition = macro_prereq_to_condition(prereq);
        if !condition.is_satisfied(dag_state) {
            // Format the missing prereq for display
            let desc = match prereq {
                MacroPrereq::StateExists { key } => format!("State: {}", key),
                MacroPrereq::VerbCompleted { verb } => format!("Verb: {}", verb),
                MacroPrereq::FactExists { predicate } => format!("Fact: {}", predicate),
                MacroPrereq::AnyOf { conditions } => {
                    let items: Vec<String> = conditions
                        .iter()
                        .map(|c| match c {
                            MacroPrereq::VerbCompleted { verb } => verb.clone(),
                            MacroPrereq::StateExists { key } => key.clone(),
                            _ => "...".to_string(),
                        })
                        .collect();
                    format!("Any of: {}", items.join(" | "))
                }
            };
            missing.push(desc);
        }
    }

    if missing.is_empty() {
        VerbReadiness::Ready
    } else {
        VerbReadiness::Blocked { missing }
    }
}

/// Check if a verb (by FQN) is ready to execute
pub fn is_verb_ready(verb_fqn: &str, dag_state: &DagState) -> bool {
    if let Some(schema) = macro_registry().get(verb_fqn) {
        matches!(check_macro_prereqs(schema, dag_state), VerbReadiness::Ready)
    } else {
        // Non-macro verbs are always ready (no prereq tracking)
        true
    }
}

/// Get readiness info for a specific verb
pub fn get_verb_readiness(verb_fqn: &str, dag_state: &DagState) -> Option<VerbReadinessInfo> {
    let schema = macro_registry().get(verb_fqn)?;
    let readiness = check_macro_prereqs(schema, dag_state);

    Some(VerbReadinessInfo {
        fqn: verb_fqn.to_string(),
        label: schema.ui.label.clone(),
        description: schema.ui.description.clone(),
        readiness,
        unlocks: schema.unlocks.clone(),
    })
}

/// Get all ready verbs for a session (filtered by mode tag if provided)
pub fn get_ready_verbs(session: &UnifiedSession, mode_tag: Option<&str>) -> Vec<VerbReadinessInfo> {
    let dag_state = &session.dag_state;

    macro_registry()
        .all()
        .filter(|(_, schema)| {
            // Filter by mode tag if provided
            if let Some(tag) = mode_tag {
                schema.routing.mode_tags.contains(&tag.to_string())
            } else {
                true
            }
        })
        .filter_map(|(fqn, schema)| {
            let readiness = check_macro_prereqs(schema, dag_state);
            if matches!(readiness, VerbReadiness::Ready) {
                Some(VerbReadinessInfo {
                    fqn: fqn.clone(),
                    label: schema.ui.label.clone(),
                    description: schema.ui.description.clone(),
                    readiness,
                    unlocks: schema.unlocks.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Get all verbs with their readiness status for UI (e.g., DAG visualization)
pub fn get_all_verb_readiness(
    session: &UnifiedSession,
    mode_tag: Option<&str>,
) -> Vec<VerbReadinessInfo> {
    let dag_state = &session.dag_state;

    macro_registry()
        .all()
        .filter(|(_, schema)| {
            if let Some(tag) = mode_tag {
                schema.routing.mode_tags.contains(&tag.to_string())
            } else {
                true
            }
        })
        .map(|(fqn, schema)| {
            let readiness = check_macro_prereqs(schema, dag_state);
            VerbReadinessInfo {
                fqn: fqn.clone(),
                label: schema.ui.label.clone(),
                description: schema.ui.description.clone(),
                readiness,
                unlocks: schema.unlocks.clone(),
            }
        })
        .collect()
}

/// Update session DAG state after verb execution
///
/// This should be called after a macro executes successfully to:
/// 1. Mark the verb as completed
/// 2. Set any state flags defined in the macro's `sets_state`
pub fn update_dag_after_execution(session: &mut UnifiedSession, verb_fqn: &str) {
    // Mark verb as completed
    session.dag_state.mark_completed(verb_fqn);

    // Apply sets_state from macro schema
    if let Some(schema) = macro_registry().get(verb_fqn) {
        for set_state in &schema.sets_state {
            if let Some(b) = set_state.value.as_bool() {
                session.dag_state.set_flag(&set_state.key, b);
            } else {
                // For non-bool values, store as fact
                session
                    .dag_state
                    .set_fact(&set_state.key, set_state.value.clone());
            }
        }

        debug!(
            verb = verb_fqn,
            unlocks = ?schema.unlocks,
            "DAG state updated after verb execution"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::intent_pipeline::IntentArgument;

    #[test]
    fn test_extract_string_values() {
        assert_eq!(
            extract_string_value(&IntentArgValue::String("test".to_string())),
            Some("test".to_string())
        );

        assert_eq!(
            extract_string_value(&IntentArgValue::Number(42.0)),
            Some("42".to_string())
        );

        assert_eq!(
            extract_string_value(&IntentArgValue::Boolean(true)),
            Some("true".to_string())
        );

        assert_eq!(
            extract_string_value(&IntentArgValue::Uuid("uuid-123".to_string())),
            Some("uuid-123".to_string())
        );

        assert_eq!(
            extract_string_value(&IntentArgValue::Missing {
                arg_name: "x".to_string()
            }),
            None
        );
    }

    #[test]
    fn test_intent_args_conversion() {
        let intent = StructuredIntent {
            verb: "structure.setup".to_string(),
            arguments: vec![
                IntentArgument {
                    name: "name".to_string(),
                    value: IntentArgValue::String("Acme Fund".to_string()),
                    resolved: false,
                },
                IntentArgument {
                    name: "structure_type".to_string(),
                    value: IntentArgValue::String("pe".to_string()),
                    resolved: false,
                },
            ],
            confidence: 0.9,
            notes: vec![],
        };

        let args = intent_args_to_macro_args(&intent);
        assert_eq!(args.get("name"), Some(&"Acme Fund".to_string()));
        assert_eq!(args.get("structure_type"), Some(&"pe".to_string()));
    }
}
