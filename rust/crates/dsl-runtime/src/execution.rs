//! Verb execution context, outcome, and result — the data-plane runtime types.
//!
//! Moved from `sem_os_core::execution` in Phase 2 of the three-plane
//! architecture refactor. Per the plan, the data-plane types live here with
//! `VerbExecutionPort`. `CrudExecutionPort` also moved here (see `port.rs`)
//! because it references `VerbExecutionContext` / `VerbExecutionOutcome`
//! and keeping it in `sem_os_core` would create a crate-graph cycle.
//!
//! # Phase-2 transitional dep
//!
//! `dsl-runtime` still depends on `sem_os_core` for `Principal`,
//! `SemOsError`, and `VerbContractBody`. A future slice inverts that
//! direction — either by moving `Principal` into a shared lower crate or
//! by introducing a dsl-runtime-local error type — but Phase 2 does not
//! gate on inversion.
//!
//! # `VerbExecutionOutcome` naming
//!
//! The enum kept its name (`VerbExecutionOutcome`) to avoid touching ~100
//! call sites. The Phase-5 plane-boundary struct of the same role lives in
//! `ob_poc_types::gated_envelope` under the name `GatedOutcome`, so the two
//! do not collide in use.

use std::collections::HashMap;
use std::sync::Arc;

use ob_poc_types::{OutboxDraft, PendingStateAdvance};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use sem_os_core::error::SemOsError;
use sem_os_core::principal::Principal;

use crate::services::ServiceRegistry;

/// Result alias over `SemOsError`. Formerly lived at
/// `sem_os_core::execution::Result`.
pub type Result<T> = std::result::Result<T, SemOsError>;

// ── Context ─────────────────────────────────────────────────────

/// Execution context passed through the verb execution port.
///
/// Contains the core execution state that SemOS understands. Platform-specific
/// state (e.g., ob-poc's pending_view_state, pending_session) lives in
/// `extensions` as opaque JSON.
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

    /// Platform service registry (trait-object injection for plugin ops).
    ///
    /// Plugin ops that need host capabilities (session state, DSL execution,
    /// stewardship, etc.) look them up via [`Self::service`]. The host
    /// (ob-poc startup) constructs the registry and threads it through the
    /// executor; tests and the default context use an empty registry.
    ///
    /// Not serialised — a live trait-object graph can't round-trip through
    /// JSON, and the registry is reconstructed at startup on every process
    /// boundary anyway.
    #[serde(skip, default = "default_service_registry")]
    pub services: Arc<ServiceRegistry>,
}

fn default_service_registry() -> Arc<ServiceRegistry> {
    Arc::new(ServiceRegistry::empty())
}

impl VerbExecutionContext {
    /// Create a new context with the given principal and a fresh execution ID.
    ///
    /// The service registry starts empty. Production callers that need to
    /// inject platform services (e.g., the ob-poc host wiring at startup)
    /// use [`Self::with_services`] or assign to [`Self::services`] directly
    /// after construction.
    pub fn new(principal: Principal) -> Self {
        Self {
            principal,
            correlation_id: Uuid::new_v4(),
            symbols: HashMap::new(),
            symbol_types: HashMap::new(),
            execution_id: Uuid::new_v4(),
            extensions: serde_json::Value::Null,
            services: default_service_registry(),
        }
    }

    /// Create a new context with a pre-populated service registry.
    pub fn with_services(principal: Principal, services: Arc<ServiceRegistry>) -> Self {
        let mut ctx = Self::new(principal);
        ctx.services = services;
        ctx
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

    /// Look up a platform service by trait.
    ///
    /// Returns `Err` when the host hasn't registered an impl for trait `T`.
    /// The error message names the missing trait so the op author can wire
    /// it at startup without guessing.
    pub fn service<T: ?Sized + Send + Sync + 'static>(&self) -> anyhow::Result<Arc<T>> {
        self.services.get::<T>().ok_or_else(|| {
            anyhow::anyhow!(
                "platform service `{trait_name}` is not registered in the \
                 `ServiceRegistry`; wire it at startup via \
                 `ServiceRegistryBuilder::register::<dyn {trait_name}>(Arc::new(impl))`",
                trait_name = std::any::type_name::<T>()
            )
        })
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
            services: default_service_registry(),
        }
    }
}

#[cfg(test)]
mod service_lookup_tests {
    use super::*;
    use crate::services::ServiceRegistryBuilder;

    trait TestGreeter: Send + Sync {
        fn greet(&self) -> &'static str;
    }

    struct GreeterImpl;
    impl TestGreeter for GreeterImpl {
        fn greet(&self) -> &'static str {
            "hello"
        }
    }

    trait Missing: Send + Sync {}

    #[test]
    fn service_lookup_hit_returns_impl() {
        let mut b = ServiceRegistryBuilder::new();
        b.register::<dyn TestGreeter>(Arc::new(GreeterImpl));
        let ctx = VerbExecutionContext::with_services(Principal::system(), Arc::new(b.build()));

        let svc = ctx.service::<dyn TestGreeter>().expect("registered");
        assert_eq!(svc.greet(), "hello");
    }

    #[test]
    fn service_lookup_miss_returns_named_error() {
        let ctx = VerbExecutionContext::default();

        let result = ctx.service::<dyn Missing>();
        let err = match result {
            Ok(_) => panic!("expected miss but got an impl"),
            Err(e) => e,
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("Missing"),
            "error should name the missing trait: {msg}"
        );
        assert!(
            msg.contains("ServiceRegistryBuilder::register"),
            "error should hint at startup wiring: {msg}"
        );
    }

    #[test]
    fn default_context_has_empty_registry() {
        let ctx = VerbExecutionContext::default();
        assert!(ctx.services.is_empty());
    }

    #[test]
    fn new_context_has_empty_registry() {
        let ctx = VerbExecutionContext::new(Principal::system());
        assert!(ctx.services.is_empty());
    }
}

// ── Outcome ─────────────────────────────────────────────────────

/// Result of executing a single verb.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    #[default]
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

/// Full result of verb execution: outcome + side effects + Phase-5 declarative
/// state advance and outbox drafts.
///
/// The `pending_state_advance` and `outbox_drafts` fields were added in
/// Phase 2 of the three-plane architecture refactor. Per the plan they are
/// *additive* — all 625 existing ops construct `VerbExecutionResult` without
/// setting them, relying on `Default` + `#[serde(default)]` to keep
/// backwards compatibility. Phase 5's Sequencer will read these fields to
/// drive stage 9a state apply + stage 9b outbox draining.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerbExecutionResult {
    /// The verb's return value.
    pub outcome: VerbExecutionOutcome,
    /// Side effects to propagate.
    #[serde(default)]
    pub side_effects: VerbSideEffects,
    /// Declarative state mutation for SemOS to apply in-txn (Phase 5 target).
    #[serde(default)]
    pub pending_state_advance: PendingStateAdvance,
    /// Post-commit effects queued for the outbox drainer (Phase 5 target).
    #[serde(default)]
    pub outbox_drafts: Vec<OutboxDraft>,
}

impl VerbExecutionResult {
    /// Create a result with just an outcome and no side effects.
    pub fn from_outcome(outcome: VerbExecutionOutcome) -> Self {
        Self {
            outcome,
            side_effects: VerbSideEffects::default(),
            pending_state_advance: PendingStateAdvance::default(),
            outbox_drafts: Vec::new(),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            ctx.symbol_types.get("fund").map(|s| s.as_str()),
            Some("cbu")
        );
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
        assert_eq!(
            back.symbol_types.get("cbu").map(|s| s.as_str()),
            Some("cbu")
        );
        assert_eq!(back.extensions["pending_view"], "universe");
        assert_eq!(back.principal.actor_id, "test-actor");
    }
}
