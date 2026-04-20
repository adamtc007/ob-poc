//! `CustomOperation` trait, factory, registry, and inventory wiring.
//!
//! Moved from `ob-poc::domain_ops::mod.rs` in Phase 2.5 Slice G. The trait is
//! the single execution contract for plugin ops in the ob-poc codebase; the
//! registry owns the runtime map of `(domain, verb) → Arc<dyn CustomOperation>`;
//! `CustomOpFactory` + `inventory::collect!` drive compile-time auto-registration
//! via the `#[register_custom_op]` attribute macro in `dsl-runtime-macros`.
//!
//! # Single-path discipline
//!
//! The trait exposes one execution method: `execute_json`. Legacy
//! `execute(&VerbCall, &mut ExecutionContext, &PgPool)` moved out of the trait
//! in Slice D-quick and now lives on per-op inherent impls; Slice C-native
//! rewrites those call sites and deletes the inherent bodies altogether.
//! `execute_in_tx` dropped entirely — no op overrode it; plugins are
//! non-transactional until Slice F (VerbExecutionPort + `crate::tx::TransactionScope`).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

/// Trait for custom operations that cannot be expressed as data-driven verbs.
///
/// Implementors register via `#[register_custom_op]` (in `dsl-runtime-macros`)
/// and are auto-collected into `CustomOperationRegistry::new()` through the
/// `inventory` crate.
#[async_trait]
pub trait CustomOperation: Send + Sync {
    /// Domain this operation belongs to (e.g. `"cbu"`).
    fn domain(&self) -> &'static str;

    /// Verb name this op handles (e.g. `"create"`).
    fn verb(&self) -> &'static str;

    /// Why this operation requires custom code — documentation only.
    fn rationale(&self) -> &'static str;

    /// Execute the op against JSON args and a `VerbExecutionContext`.
    ///
    /// The single execution contract on this trait. Callers dispatch
    /// through this via `DslExecutor::dispatch_plugin_via_execute_json`
    /// or `VerbExecutionPort::execute_verb`.
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome>;

    /// True iff this op's `execute_json` body is native (does not thunk
    /// through a legacy inherent `execute` method). Retained for telemetry
    /// during the Slice C-native rip-out — removed once all ops are native.
    fn is_migrated(&self) -> bool {
        false
    }
}

/// Factory for auto-registration of custom ops via the `inventory` crate.
///
/// Each `#[register_custom_op]` invocation emits an `inventory::submit!` entry
/// carrying a `CustomOpFactory { create }` that returns `Arc<dyn CustomOperation>`.
/// `CustomOperationRegistry::new()` walks the inventory and registers each op.
pub struct CustomOpFactory {
    pub create: fn() -> Arc<dyn CustomOperation>,
}

inventory::collect!(CustomOpFactory);

/// Registry of all registered custom ops, keyed by `(domain, verb)`.
///
/// Constructed once at startup from the `inventory` collection. Exposed by
/// `ob-poc` through `DslExecutor.custom_ops` and used by the `VerbExecutionPort`
/// adapter to route plugin dispatch.
pub struct CustomOperationRegistry {
    operations: HashMap<(String, String), Arc<dyn CustomOperation>>,
}

impl CustomOperationRegistry {
    /// Build the registry from the `inventory` collection. Panics on duplicate
    /// `(domain, verb)` keys — this catches registration bugs early.
    pub fn new() -> Self {
        let mut registry = Self {
            operations: HashMap::new(),
        };

        for factory in inventory::iter::<CustomOpFactory> {
            let op = (factory.create)();
            registry.register_internal(op);
        }

        let total = registry.operations.len();
        let migrated = registry
            .operations
            .values()
            .filter(|op| op.is_migrated())
            .count();
        tracing::info!(
            "CustomOperationRegistry: {} ops from inventory ({} migrated to native execute_json, {} still thunk through legacy)",
            total,
            migrated,
            total - migrated
        );

        registry
    }

    /// Sorted `(domain, verb, is_migrated)` snapshot for diagnostics.
    pub fn manifest(&self) -> Vec<(String, String, bool)> {
        let mut ops: Vec<_> = self
            .operations
            .iter()
            .map(|((d, v), op)| (d.clone(), v.clone(), op.is_migrated()))
            .collect();
        ops.sort();
        ops
    }

    /// Number of registered ops.
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// True if no ops are registered.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Number of ops whose `execute_json` body is native.
    pub fn migrated_count(&self) -> usize {
        self.operations
            .values()
            .filter(|op| op.is_migrated())
            .count()
    }

    fn register_internal(&mut self, op: Arc<dyn CustomOperation>) {
        let key = (op.domain().to_string(), op.verb().to_string());
        if self.operations.contains_key(&key) {
            panic!(
                "Duplicate custom op registration: {}.{} — check for both \
                 `#[register_custom_op]` and manual registration.",
                key.0, key.1
            );
        }
        self.operations.insert(key, op);
    }

    /// Manual registration (allows overwrite for migration). Warns on overwrite.
    pub fn register(&mut self, op: Arc<dyn CustomOperation>) {
        let key = (op.domain().to_string(), op.verb().to_string());
        if self.operations.contains_key(&key) {
            tracing::warn!(
                "Manual registration overwriting existing op: {}.{}",
                key.0,
                key.1
            );
        }
        self.operations.insert(key, op);
    }

    /// Look up an op by `(domain, verb)`.
    pub fn get(&self, domain: &str, verb: &str) -> Option<Arc<dyn CustomOperation>> {
        self.operations
            .get(&(domain.to_string(), verb.to_string()))
            .cloned()
    }

    /// Membership check for an op.
    pub fn has(&self, domain: &str, verb: &str) -> bool {
        self.operations
            .contains_key(&(domain.to_string(), verb.to_string()))
    }

    /// Sorted `(domain, verb, rationale)` triples for introspection.
    pub fn list(&self) -> Vec<(&str, &str, &str)> {
        let mut entries: Vec<_> = self
            .operations
            .values()
            .map(|op| (op.domain(), op.verb(), op.rationale()))
            .collect();
        entries.sort_by_key(|(d, v, _)| (*d, *v));
        entries
    }
}

impl Default for CustomOperationRegistry {
    fn default() -> Self {
        Self::new()
    }
}
