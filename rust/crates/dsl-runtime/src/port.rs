//! Verb execution port — the data-plane contract for dispatching a verb.
//!
//! Moved from `sem_os_core::execution` in Phase 1 per the three-plane
//! architecture implementation plan. Phase 2 relocated the supporting
//! `VerbExecutionContext` / `VerbExecutionResult` / `VerbExecutionOutcome`
//! types here (see `execution.rs`) so the full data-plane contract lives
//! in one crate, and also brought `CrudExecutionPort` along to break the
//! cycle. `dsl-runtime` still depends on `sem_os_core` for `Principal`,
//! `SemOsError`, and `VerbContractBody` — future slice inverts that
//! direction.
//!
//! # Plane rule
//!
//! SemOS gates, the Sequencer dispatches, the runtime executes. This
//! trait is the handoff point. Per v0.3 §8.4, the Sequencer in `ob-poc`
//! is the ONLY caller of `VerbExecutionPort::execute_verb` in production
//! code after Phase 5b; lint L2 enforces the single-dispatch-site rule.

use async_trait::async_trait;
use sem_os_core::verb_contract::VerbContractBody;

use crate::execution::{Result, VerbExecutionContext, VerbExecutionOutcome, VerbExecutionResult};

// ── Port Traits ─────────────────────────────────────────────────

/// Port trait for verb execution — implemented by the platform (ob-poc).
///
/// SemOS calls this after gating, scoping, and contract validation.
/// The implementor routes to CRUD (metadata-driven) or plugin (custom Rust)
/// execution based on the verb's behavior field.
#[async_trait]
pub trait VerbExecutionPort: Send + Sync {
    /// Execute a verb by its fully-qualified name.
    ///
    /// # Arguments
    /// - `verb_fqn` — e.g. "cbu.create"
    /// - `args` — extracted arguments as JSON object
    /// - `ctx` — mutable execution context (symbols, principal, extensions)
    ///
    /// # Returns
    /// The verb's result and any side effects (new bindings, platform state).
    async fn execute_verb(
        &self,
        verb_fqn: &str,
        args: serde_json::Value,
        ctx: &mut VerbExecutionContext,
    ) -> Result<VerbExecutionResult>;
}

/// Port trait for CRUD verb execution — driven by `VerbContractBody.crud_mapping`.
///
/// Separated from [`VerbExecutionPort`] because CRUD execution is purely
/// metadata-driven and can be implemented by `sem_os_postgres` without
/// knowing about plugin ops or the `CustomOperation` trait.
///
/// Moved from `sem_os_core::execution` in Phase 2. The trait still references
/// `VerbContractBody` (a SemOS catalogue type) because the CRUD mapping is
/// catalogue metadata.
#[async_trait]
pub trait CrudExecutionPort: Send + Sync {
    /// Execute a CRUD verb using its contract metadata.
    ///
    /// The contract's `crud_mapping` field (table, schema, operation, key_column)
    /// and `args` (with `maps_to` column mappings) drive SQL generation.
    async fn execute_crud(
        &self,
        contract: &VerbContractBody,
        args: serde_json::Value,
        ctx: &VerbExecutionContext,
    ) -> Result<VerbExecutionOutcome>;
}

// ── Test Support ────────────────────────────────────────────────

#[cfg(test)]
pub mod test_support {
    use super::*;
    use crate::execution::{VerbExecutionOutcome, VerbSideEffects};
    use sem_os_core::error::SemOsError;
    use std::collections::HashMap;

    /// Mock verb executor for testing port consumers.
    ///
    /// Pre-load results by FQN; returns `SemOsError::NotFound` for unknown verbs.
    pub struct MockVerbExecutor {
        pub results: HashMap<String, VerbExecutionResult>,
    }

    impl MockVerbExecutor {
        pub fn new() -> Self {
            Self {
                results: HashMap::new(),
            }
        }

        pub fn with_result(mut self, fqn: &str, result: VerbExecutionResult) -> Self {
            self.results.insert(fqn.to_string(), result);
            self
        }
    }

    impl Default for MockVerbExecutor {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl VerbExecutionPort for MockVerbExecutor {
        async fn execute_verb(
            &self,
            verb_fqn: &str,
            _args: serde_json::Value,
            ctx: &mut VerbExecutionContext,
        ) -> Result<VerbExecutionResult> {
            let result =
                self.results.get(verb_fqn).cloned().ok_or_else(|| {
                    SemOsError::NotFound(format!("No mock result for {verb_fqn}"))
                })?;

            // Apply side effects to context (mimics real executor behavior).
            for (name, uuid) in &result.side_effects.new_bindings {
                ctx.symbols.insert(name.clone(), *uuid);
            }
            for (name, entity_type) in &result.side_effects.new_binding_types {
                ctx.symbol_types.insert(name.clone(), entity_type.clone());
            }

            Ok(result)
        }
    }

    #[allow(dead_code)]
    fn _type_check() {
        // Compile-time assertion that we correctly re-exported the test types.
        let _: VerbExecutionOutcome = VerbExecutionOutcome::Void;
        let _: VerbSideEffects = VerbSideEffects::default();
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::test_support::MockVerbExecutor;
    use super::*;
    use crate::execution::{VerbExecutionOutcome, VerbSideEffects};
    use sem_os_core::principal::Principal;
    use uuid::Uuid;

    fn test_principal() -> Principal {
        Principal::explicit("test-actor", vec!["admin".to_string()])
    }

    #[tokio::test]
    async fn mock_executor_returns_preloaded_result() {
        let cbu_id = Uuid::new_v4();
        let executor = MockVerbExecutor::new().with_result(
            "cbu.create",
            VerbExecutionResult {
                outcome: VerbExecutionOutcome::Uuid(cbu_id),
                side_effects: VerbSideEffects {
                    new_bindings: [("cbu".to_string(), cbu_id)].into_iter().collect(),
                    new_binding_types: [("cbu".to_string(), "cbu".to_string())]
                        .into_iter()
                        .collect(),
                    platform_state: serde_json::Value::Null,
                },
                ..Default::default()
            },
        );

        let mut ctx = VerbExecutionContext::new(test_principal());
        let result = executor
            .execute_verb("cbu.create", serde_json::json!({"name": "Test"}), &mut ctx)
            .await
            .unwrap();

        assert!(matches!(result.outcome, VerbExecutionOutcome::Uuid(id) if id == cbu_id));
        assert_eq!(ctx.resolve("cbu"), Some(cbu_id));
        assert_eq!(ctx.symbol_types.get("cbu").map(|s| s.as_str()), Some("cbu"));
    }

    #[tokio::test]
    async fn mock_executor_unknown_verb_returns_error() {
        let executor = MockVerbExecutor::new();
        let mut ctx = VerbExecutionContext::default();

        let result = executor
            .execute_verb("nonexistent.verb", serde_json::json!({}), &mut ctx)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn mock_executor_symbol_propagation() {
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        let executor = MockVerbExecutor::new()
            .with_result(
                "step.a",
                VerbExecutionResult {
                    outcome: VerbExecutionOutcome::Uuid(id_a),
                    side_effects: VerbSideEffects {
                        new_bindings: [("entity_a".to_string(), id_a)].into_iter().collect(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .with_result(
                "step.b",
                VerbExecutionResult {
                    outcome: VerbExecutionOutcome::Uuid(id_b),
                    side_effects: VerbSideEffects {
                        new_bindings: [("entity_b".to_string(), id_b)].into_iter().collect(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );

        let mut ctx = VerbExecutionContext::default();

        executor
            .execute_verb("step.a", serde_json::json!({}), &mut ctx)
            .await
            .unwrap();
        assert_eq!(ctx.resolve("entity_a"), Some(id_a));
        assert_eq!(ctx.resolve("entity_b"), None);

        executor
            .execute_verb("step.b", serde_json::json!({}), &mut ctx)
            .await
            .unwrap();
        assert_eq!(ctx.resolve("entity_a"), Some(id_a));
        assert_eq!(ctx.resolve("entity_b"), Some(id_b));
    }
}
