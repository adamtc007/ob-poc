//! Cross-workspace state consistency — shared atom registry, staleness propagation,
//! constellation replay, remediation lifecycle, external call idempotency,
//! platform DAG derivation, and v1.3 cross-workspace runtime mechanisms.
//!
//! See:
//!   `docs/annex-cross-workspace-state-consistency.md`
//!   `docs/backlog/catalogue-platform-refinement-v1_3.md`
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
//! use dsl_core::ConfigLoader;
//! use dsl_runtime::{
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
//! use dsl_runtime::DerivedStateEvaluator;
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
//! use dsl_runtime::{CascadePlanner, ChildEntityResolver};
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

mod compensation;
mod dag_registry;
mod derived_state;
mod derived_state_projector;
mod fact_refs;

pub use dag_registry::{DagRegistry, TransitionRef};
mod fact_versions;
mod gate_checker;
mod hierarchy_cascade;
mod idempotency;
pub(crate) mod platform_dag;
mod postgres_child_resolver;
mod providers;
mod remediation;
mod replay;
mod repository;
mod slot_state;
mod sql_predicate_resolver;
#[cfg(any(test, feature = "harness"))]
pub mod test_harness;
mod types;

pub use compensation::{
    confirm_compensation, list_for_remediation, record_compensation, CompensationOutcome,
    CompensationRecordRow, CompensationSummary, CorrectionType, RecordCompensationInput,
};
pub use derived_state::{ClauseKind, ConditionResult, DerivedStateEvaluator, DerivedStateValue};
pub use derived_state_projector::{DerivedStateProjection, DerivedStateProjector};
pub use fact_refs::{
    advance_to_current, check_staleness_for_entity, list_stale_refs, mark_deferred, mark_stale,
    upsert_ref, ConsumerRefStatus, StaleSharedFactRef, WorkspaceFactRefRow,
};
pub use fact_versions::{
    current_version_number, get_current_version, get_propagation_result, get_version_history,
    insert_version, record_if_active, PropagationResult, SharedFactVersionRow,
    SharedFactVersionSummary, StaleConsumerRef,
};
pub use gate_checker::{GateChecker, PredicateResolver, SameEntityResolver};
pub use hierarchy_cascade::{
    CascadeAction, CascadePlanner, ChildEntityResolver, NoChildrenResolver,
};
pub use idempotency::{
    check_idempotency, record_call, supersede_call, ExternalCallRow, IdempotencyAction,
    ProviderCapability, RecordCallInput,
};
pub use platform_dag::{
    derive_platform_dag, set_atom_path_table_map, AtomPathTableMap, PlatformDag, PlatformEdge,
};
pub use postgres_child_resolver::PostgresChildEntityResolver;
pub use providers::{list_all, list_for_provider, ProviderCapabilitySummary};
pub use remediation::{
    begin_replay, create_remediation_event, defer, escalate, get_by_id, list_open, mark_resolved,
    revoke_deferral, CreateRemediationInput, RemediationEventRow, RemediationEventSummary,
    RemediationStatus,
};
pub use replay::{RebuildContext, ReplayOutcome, ReplayResult, ReplayTrigger};
pub use repository::{
    get_by_id as get_atom_by_id, get_by_path, insert_shared_atom, list_active, list_shared_atoms,
    transition_lifecycle, upsert_from_seed,
};
pub use slot_state::{
    resolve_slot_table, set_slot_state_table, PostgresSlotStateProvider, SlotStateProvider,
};
pub use sql_predicate_resolver::{set_table_pk_overrides, SqlPredicateResolver};
pub use types::{
    LifecycleTransitionResult, RegisterSharedAtomInput, SharedAtomDef, SharedAtomLifecycle,
    SharedAtomSummary, SharedAtomValidation,
};
