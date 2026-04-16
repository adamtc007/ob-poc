//! Verb execution port — the contract SemOS defines for executing verbs.
//!
//! SemOS owns verb visibility, scoping, gating, and contract metadata.
//! The execution port defines HOW verbs execute, implemented by the platform
//! (ob-poc) as an adapter over its domain_ops / GenericCrudExecutor.
//!
//! ## Design
//!
//! - `VerbExecutionPort` — async trait, the single entry point for verb execution
//! - `VerbExecutionContext` — execution state (symbols, principal, correlation)
//! - `VerbExecutionOutcome` — result of executing a single verb
//! - `VerbSideEffects` — bindings and platform state produced by execution
//!
//! The context carries an opaque `extensions` field (JSON) for platform-specific
//! state (e.g., ob-poc's pending_view_state, pending_session). SemOS never
//! inspects this; the platform adapter serializes/deserializes it.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::SemOsError;
use crate::principal::Principal;
use crate::verb_contract::VerbContractBody;

pub type Result<T> = std::result::Result<T, SemOsError>;

// ── Port Trait ──────────────────────────────────────────────────

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
/// Separated from `VerbExecutionPort` because CRUD execution is purely
/// metadata-driven and can be implemented by sem_os_postgres without
/// knowing about plugin ops or the CustomOperation trait.
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

// ── Context ─────────────────────────────────────────────────────

/// Execution context passed through the verb execution port.
///
/// Contains the core execution state that SemOS understands. Platform-specific
/// state (e.g., REPL pending_* fields) lives in `extensions` as opaque JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbExecutionContext {
    /// Identity of the actor executing the verb.
    pub principal: Principal,

    /// Correlation ID for distributed tracing.
    pub correlation_id: Uuid,

    /// Symbol table for @reference resolution.
    /// Key: binding name (e.g., "cbu"), Value: entity UUID.
    pub symbols: HashMap<String, Uuid>,

    /// Symbol type annotations.
    /// Key: binding name, Value: entity type (e.g., "cbu", "entity").
    pub symbol_types: HashMap<String, String>,

    /// Unique execution ID (for idempotency tracking).
    pub execution_id: Uuid,

    /// Opaque platform extensions.
    ///
    /// The platform adapter serializes its own state here (e.g., ob-poc's
    /// pending_view_state, pending_session, client_group_id). SemOS never
    /// inspects this — it round-trips through execution unchanged except
    /// for side effects produced by the verb handler.
    #[serde(default)]
    pub extensions: serde_json::Value,
}

impl VerbExecutionContext {
    /// Create a new context with the given principal and a fresh execution ID.
    pub fn new(principal: Principal) -> Self {
        Self {
            principal,
            correlation_id: Uuid::new_v4(),
            symbols: HashMap::new(),
            symbol_types: HashMap::new(),
            execution_id: Uuid::new_v4(),
            extensions: serde_json::Value::Null,
        }
    }

    /// Bind a symbol to a UUID value.
    pub fn bind(&mut self, name: &str, value: Uuid) {
        self.symbols.insert(name.to_string(), value);
    }

    /// Bind a symbol with its entity type.
    pub fn bind_typed(&mut self, name: &str, value: Uuid, entity_type: &str) {
        self.symbols.insert(name.to_string(), value);
        self.symbol_types
            .insert(name.to_string(), entity_type.to_string());
    }

    /// Resolve a symbol reference.
    pub fn resolve(&self, name: &str) -> Option<Uuid> {
        self.symbols.get(name).copied()
    }

    /// Check if a symbol exists.
    pub fn has(&self, name: &str) -> bool {
        self.symbols.contains_key(name)
    }
}

impl Default for VerbExecutionContext {
    fn default() -> Self {
        Self {
            principal: Principal::system(),
            correlation_id: Uuid::new_v4(),
            symbols: HashMap::new(),
            symbol_types: HashMap::new(),
            execution_id: Uuid::new_v4(),
            extensions: serde_json::Value::Null,
        }
    }
}

// ── Outcome ─────────────────────────────────────────────────────

/// Result of executing a single verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum VerbExecutionOutcome {
    /// A single UUID (e.g., newly created entity ID).
    Uuid(Uuid),
    /// A single JSON record.
    Record(serde_json::Value),
    /// Multiple JSON records.
    RecordSet(Vec<serde_json::Value>),
    /// Number of rows affected.
    Affected(u64),
    /// No return value.
    Void,
}

// ── Side Effects ────────────────────────────────────────────────

/// Side effects produced by verb execution.
///
/// Returned alongside the outcome so the caller can propagate
/// symbol bindings and platform state changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerbSideEffects {
    /// New symbol bindings produced by the verb (e.g., "cbu" → UUID).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub new_bindings: HashMap<String, Uuid>,

    /// Type annotations for new bindings.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub new_binding_types: HashMap<String, String>,

    /// Platform-specific state changes (e.g., ob-poc pending_* fields).
    /// Opaque to SemOS — the adapter knows how to unpack this.
    #[serde(default)]
    pub platform_state: serde_json::Value,
}

// ── Combined Result ─────────────────────────────────────────────

/// Full result of verb execution: outcome + side effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbExecutionResult {
    /// The verb's return value.
    pub outcome: VerbExecutionOutcome,
    /// Side effects to propagate.
    #[serde(default)]
    pub side_effects: VerbSideEffects,
}

impl VerbExecutionResult {
    /// Create a result with just an outcome and no side effects.
    pub fn from_outcome(outcome: VerbExecutionOutcome) -> Self {
        Self {
            outcome,
            side_effects: VerbSideEffects::default(),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
pub mod test_support {
    use super::*;

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

    #[async_trait]
    impl VerbExecutionPort for MockVerbExecutor {
        async fn execute_verb(
            &self,
            verb_fqn: &str,
            _args: serde_json::Value,
            ctx: &mut VerbExecutionContext,
        ) -> Result<VerbExecutionResult> {
            let result = self
                .results
                .get(verb_fqn)
                .cloned()
                .ok_or_else(|| SemOsError::NotFound(format!("No mock result for {verb_fqn}")))?;

            // Apply side effects to context (mimics real executor behavior)
            for (name, uuid) in &result.side_effects.new_bindings {
                ctx.symbols.insert(name.clone(), *uuid);
            }
            for (name, entity_type) in &result.side_effects.new_binding_types {
                ctx.symbol_types.insert(name.clone(), entity_type.clone());
            }

            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::test_support::MockVerbExecutor;

    fn test_principal() -> Principal {
        Principal::explicit("test-actor", vec!["admin".to_string()])
    }

    #[test]
    fn context_new_has_fresh_ids() {
        let ctx = VerbExecutionContext::new(test_principal());
        assert!(!ctx.execution_id.is_nil());
        assert!(!ctx.correlation_id.is_nil());
        assert!(ctx.symbols.is_empty());
        assert_eq!(ctx.principal.actor_id, "test-actor");
    }

    #[test]
    fn context_bind_and_resolve() {
        let mut ctx = VerbExecutionContext::default();
        let id = Uuid::new_v4();

        ctx.bind("cbu", id);
        assert_eq!(ctx.resolve("cbu"), Some(id));
        assert_eq!(ctx.resolve("nonexistent"), None);
    }

    #[test]
    fn context_bind_typed() {
        let mut ctx = VerbExecutionContext::default();
        let id = Uuid::new_v4();

        ctx.bind_typed("fund", id, "cbu");
        assert_eq!(ctx.resolve("fund"), Some(id));
        assert_eq!(ctx.symbol_types.get("fund").map(|s| s.as_str()), Some("cbu"));
    }

    #[test]
    fn context_has() {
        let mut ctx = VerbExecutionContext::default();
        ctx.bind("x", Uuid::new_v4());
        assert!(ctx.has("x"));
        assert!(!ctx.has("y"));
    }

    #[test]
    fn outcome_serde_round_trip_uuid() {
        let outcome = VerbExecutionOutcome::Uuid(Uuid::nil());
        let json = serde_json::to_value(&outcome).unwrap();
        let back: VerbExecutionOutcome = serde_json::from_value(json).unwrap();
        assert!(matches!(back, VerbExecutionOutcome::Uuid(id) if id.is_nil()));
    }

    #[test]
    fn outcome_serde_round_trip_record() {
        let outcome = VerbExecutionOutcome::Record(serde_json::json!({"name": "Test"}));
        let json = serde_json::to_value(&outcome).unwrap();
        let back: VerbExecutionOutcome = serde_json::from_value(json).unwrap();
        assert!(matches!(back, VerbExecutionOutcome::Record(v) if v["name"] == "Test"));
    }

    #[test]
    fn outcome_serde_round_trip_record_set() {
        let outcome = VerbExecutionOutcome::RecordSet(vec![serde_json::json!({"a": 1})]);
        let json = serde_json::to_value(&outcome).unwrap();
        let back: VerbExecutionOutcome = serde_json::from_value(json).unwrap();
        assert!(matches!(back, VerbExecutionOutcome::RecordSet(v) if v.len() == 1));
    }

    #[test]
    fn outcome_serde_round_trip_affected() {
        let outcome = VerbExecutionOutcome::Affected(42);
        let json = serde_json::to_value(&outcome).unwrap();
        let back: VerbExecutionOutcome = serde_json::from_value(json).unwrap();
        assert!(matches!(back, VerbExecutionOutcome::Affected(42)));
    }

    #[test]
    fn outcome_serde_round_trip_void() {
        let outcome = VerbExecutionOutcome::Void;
        let json = serde_json::to_value(&outcome).unwrap();
        let back: VerbExecutionOutcome = serde_json::from_value(json).unwrap();
        assert!(matches!(back, VerbExecutionOutcome::Void));
    }

    #[test]
    fn side_effects_default_is_empty() {
        let fx = VerbSideEffects::default();
        assert!(fx.new_bindings.is_empty());
        assert!(fx.new_binding_types.is_empty());
        assert!(fx.platform_state.is_null());
    }

    #[test]
    fn result_from_outcome() {
        let result = VerbExecutionResult::from_outcome(VerbExecutionOutcome::Void);
        assert!(matches!(result.outcome, VerbExecutionOutcome::Void));
        assert!(result.side_effects.new_bindings.is_empty());
    }

    #[test]
    fn context_serde_round_trip() {
        let mut ctx = VerbExecutionContext::new(test_principal());
        let id = Uuid::new_v4();
        ctx.bind_typed("cbu", id, "cbu");
        ctx.extensions = serde_json::json!({"pending_view": "universe"});

        let json = serde_json::to_value(&ctx).unwrap();
        let back: VerbExecutionContext = serde_json::from_value(json).unwrap();

        assert_eq!(back.resolve("cbu"), Some(id));
        assert_eq!(back.symbol_types.get("cbu").map(|s| s.as_str()), Some("cbu"));
        assert_eq!(back.extensions["pending_view"], "universe");
        assert_eq!(back.principal.actor_id, "test-actor");
    }

    #[tokio::test]
    async fn mock_executor_returns_preloaded_result() {
        let cbu_id = Uuid::new_v4();
        let executor = MockVerbExecutor::new()
            .with_result("cbu.create", VerbExecutionResult {
                outcome: VerbExecutionOutcome::Uuid(cbu_id),
                side_effects: VerbSideEffects {
                    new_bindings: [("cbu".to_string(), cbu_id)].into_iter().collect(),
                    new_binding_types: [("cbu".to_string(), "cbu".to_string())].into_iter().collect(),
                    platform_state: serde_json::Value::Null,
                },
            });

        let mut ctx = VerbExecutionContext::default();
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
            .with_result("step.a", VerbExecutionResult {
                outcome: VerbExecutionOutcome::Uuid(id_a),
                side_effects: VerbSideEffects {
                    new_bindings: [("entity_a".to_string(), id_a)].into_iter().collect(),
                    ..Default::default()
                },
            })
            .with_result("step.b", VerbExecutionResult {
                outcome: VerbExecutionOutcome::Uuid(id_b),
                side_effects: VerbSideEffects {
                    new_bindings: [("entity_b".to_string(), id_b)].into_iter().collect(),
                    ..Default::default()
                },
            });

        let mut ctx = VerbExecutionContext::default();

        // Execute step A
        executor.execute_verb("step.a", serde_json::json!({}), &mut ctx).await.unwrap();
        assert_eq!(ctx.resolve("entity_a"), Some(id_a));
        assert_eq!(ctx.resolve("entity_b"), None);

        // Execute step B — context accumulates bindings
        executor.execute_verb("step.b", serde_json::json!({}), &mut ctx).await.unwrap();
        assert_eq!(ctx.resolve("entity_a"), Some(id_a));
        assert_eq!(ctx.resolve("entity_b"), Some(id_b));
    }
}
