//! Macro Integration for Intent Pipeline
//!
//! Provides macro-aware verb processing. When a verb is a macro (from the operator
//! vocabulary), it's expanded to primitive DSL statements. Otherwise, processing
//! continues normally.
//!
//! Macro expansion itself is dispatched through `dsl_v2::macros::expander::expand_macro`
//! (see `dsl_v2::macros::expander::check_prereqs` for the live prereq-checking path).

use std::sync::OnceLock;

use anyhow::Result;
use tracing::{debug, info};

use crate::dsl_v2::macros::{load_macro_registry, MacroRegistry};
use crate::session::unified::UnifiedSession;

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

/// Update session DAG state after verb execution
///
/// This should be called after a macro executes successfully to:
/// 1. Mark the verb as completed
/// 2. Set any state flags defined in the macro's `sets_state`
pub(crate) fn update_dag_after_execution(session: &mut UnifiedSession, verb_fqn: &str) {
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

