//! Cross-workspace state consistency — shared atom registry, staleness propagation,
//! constellation replay, remediation lifecycle, external call idempotency,
//! platform DAG derivation, and v1.3 cross-workspace runtime mechanisms.
//!
//! See:
//!   `docs/architecture/cross-workspace-state-consistency-v0.4.md`
//!   `docs/todo/catalogue-platform-refinement-v1_3.md`
//!
//! # V1.3 runtime mechanisms (2026-04-25)
//!
//! Three blocking/projection/cascade modes from the v1.3 spec, each
//! with a runtime evaluator that takes a `DagRegistry` (build-time
//! index) plus a SlotStateProvider / PredicateResolver / ChildEntityResolver
//! (production-side data access). All evaluators are stateless and
//! thread-safe; share via `Arc`.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  V1.3 enforcement stack                                         │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │  DagRegistry  ◀── load_dag_registry() at startup                │
//! │  ───────────                                                    │
//! │  • constraints_for_transition(ws, slot, from, to)               │
//! │  • derived_states_for_slot(ws, slot)                            │
//! │  • parent_slot_for(ws, slot) / children_of(ws, slot)            │
//! │  • transitions_for_verb(verb_fqn) ← bridges runtime dispatch    │
//! │                                                                 │
//! │       │            │            │                                │
//! │       ▼            ▼            ▼                                │
//! │  ┌─────────┐  ┌──────────┐  ┌──────────────┐                    │
//! │  │ Mode A  │  │  Mode B  │  │   Mode C     │                    │
//! │  │  Gate   │  │ Aggregate│  │   Cascade    │                    │
//! │  ├─────────┤  ├──────────┤  ├──────────────┤                    │
//! │  │GateCheck│  │DerivState│  │CascadePlanner│                    │
//! │  │  er     │  │Evaluator │  │              │                    │
//! │  └─────────┘  └──────────┘  └──────────────┘                    │
//! │       │            │            │                                │
//! │       └────────────┴────────────┘                                │
//! │                    │                                             │
//! │                    ▼                                             │
//! │     SlotStateProvider (Postgres dispatch table)                  │
//! │     PredicateResolver (SqlPredicateResolver for FK eq)           │
//! │     ChildEntityResolver (caller-supplied)                        │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Wiring an end-to-end Mode A gate (KYC blocks Deal contracted)
//!
//! ```ignore
//! use std::sync::Arc;
//! use dsl_core::config::ConfigLoader;
//! use dsl_runtime::cross_workspace::{
//!     GateChecker, PostgresSlotStateProvider, SqlPredicateResolver,
//! };
//!
//! // Once at startup:
//! let registry = Arc::new(ConfigLoader::from_env().load_dag_registry()?);
//! let slot_state = Arc::new(PostgresSlotStateProvider);
//! let predicate = Arc::new(SqlPredicateResolver);
//! let gate = GateChecker::new(registry.clone(), slot_state.clone(), predicate.clone());
//!
//! // Per verb-execution (in step executor or orchestrator):
//! //   1. Use registry.transitions_for_verb(verb_fqn) to find which
//! //      transitions could fire for the current verb args.
//! //   2. For each, call gate.check_transition(...) → Vec<GateViolation>.
//! //   3. If any violations have severity == "error", reject the verb.
//! let violations = gate.check_transition(
//!     "deal", "deal", deal_id,
//!     "KYC_CLEARANCE", "CONTRACTED", &pool,
//! ).await?;
//! if violations.iter().any(|v| v.severity == "error") {
//!     return Err(anyhow::anyhow!("gate violation: {}", violations[0].message));
//! }
//! ```
//!
//! ## Wiring Mode B aggregate (CBU operationally_active tollgate)
//!
//! ```ignore
//! use dsl_runtime::cross_workspace::DerivedStateEvaluator;
//!
//! let derived_eval = DerivedStateEvaluator::new(slot_state, predicate);
//! for d in registry.derived_states_for_slot("cbu", "cbu") {
//!     let value = derived_eval.evaluate(d, cbu_id, &pool).await?;
//!     // value.satisfied is true iff the tollgate is green.
//!     // value.conditions has per-condition diagnostics.
//! }
//! ```
//!
//! ## Wiring Mode C cascade (parent CBU SUSPENDED → children SUSPENDED)
//!
//! ```ignore
//! use dsl_runtime::cross_workspace::{CascadePlanner, ChildEntityResolver};
//!
//! let child_resolver: Arc<dyn ChildEntityResolver> = Arc::new(MyResolver);
//! let planner = CascadePlanner::new(registry, child_resolver);
//!
//! // After a parent transition (e.g. cbu.suspend on parent CBU X):
//! let actions = planner.plan_cascade(
//!     "cbu", "cbu", parent_cbu_id, "suspended", &pool,
//! ).await?;
//! // For each action, the orchestrator fires the cascading transition
//! // (which itself goes through gate checking + may chain further).
//! ```
//!
//! # What's NOT here (deliberately)
//!
//! - The actual hook into `VerbExecutionPort::execute_verb` — this
//!   requires per-verb metadata (which slot, which arg = entity_id,
//!   which arg = target state). Kept out of these modules so they
//!   stay independent of the verb-dispatch architecture.
//! - Session-scope cache for derived states (OQ-2 optimisation).
//!   Each evaluation is a fresh round-trip; caching is the orchestrator's
//!   concern (per-session cache invalidated on touched-slot writes).
//! - Production ChildEntityResolver. The default `NoChildrenResolver`
//!   returns empty; a real one would parse `parent_slot.join` (via,
//!   parent_fk, child_fk) and run the corresponding SELECT.
//!
//! # Relocation history
//!
//! Relocated from ob-poc to dsl-runtime in Phase 5a composite-blocker #2
//! (2026-04-20). The ob-poc-side `crate::repl::types_v2::WorkspaceKind`
//! dependency was widened to `String` (same snake_case serde repr) to
//! keep this module plane-neutral. dsl-runtime has sqlx unconditionally,
//! so the `#[cfg(feature = "database")]` gates around DB-backed
//! submodules (legacy from ob-poc's conditional feature) are dropped.

pub mod compensation;
pub mod derived_state;
pub mod derived_state_projector;
pub mod fact_refs;
pub mod fact_versions;
pub mod gate_checker;
pub mod hierarchy_cascade;
pub mod idempotency;
pub mod platform_dag;
pub mod postgres_child_resolver;
pub mod providers;
pub mod remediation;
pub mod replay;
pub mod repository;
pub mod slot_state;
pub mod sql_predicate_resolver;
#[cfg(any(test, feature = "harness"))]
pub mod test_harness;
pub mod types;

pub use derived_state::{
    ClauseKind, ConditionResult, DerivedStateEvaluator, DerivedStateValue,
};
pub use derived_state_projector::{DerivedStateProjection, DerivedStateProjector};
pub use gate_checker::{GateChecker, GateViolation, PredicateResolver, SameEntityResolver};
pub use hierarchy_cascade::{
    CascadeAction, CascadePlanner, ChildEntityResolver, NoChildrenResolver,
};
pub use postgres_child_resolver::PostgresChildEntityResolver;
pub use slot_state::{PostgresSlotStateProvider, SlotStateProvider};
pub use sql_predicate_resolver::SqlPredicateResolver;
