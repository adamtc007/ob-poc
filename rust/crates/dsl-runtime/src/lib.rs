//! DSL runtime — the execution plane of the three-plane architecture.
//!
//! See `docs/backlog/three-plane-architecture-v0.3.md` §7.1 for the scope
//! split between this crate, `sem_os_*` (control plane), and `ob-poc`
//! (composition plane).
//!
//! # Current state
//!
//! Per `docs/backlog/three-plane-architecture-implementation-plan-v0.1.md`
//! §3 Phase 2, this crate owns:
//!
//! - `VerbExecutionPort` trait (moved from sem_os_core in Phase 1).
//! - `VerbExecutionContext`, `VerbExecutionOutcome`, `VerbSideEffects`,
//!   `VerbExecutionResult` (moved from `sem_os_core::execution` in Phase 2).
//! - `CrudExecutionPort` (moved from `sem_os_core::execution` in Phase 2 to
//!   avoid a crate-graph cycle — it references `VerbExecutionContext`).
//! - `Result<T>` alias over `SemOsError` (moved from `sem_os_core::execution`).
//!
//! `sem_os_core::execution` is now an empty placeholder module; callers
//! migrate imports:
//!
//! ```text
//! // before
//! use sem_os_core::execution::VerbExecutionContext;
//! // after
//! use dsl_runtime::VerbExecutionContext;
//! ```
//!
//! # Transitional dep
//!
//! `dsl-runtime` still depends on `sem_os_core` for `Principal`,
//! `SemOsError`, and `VerbContractBody` (the CRUD port's contract
//! metadata). A future slice inverts this by either moving `Principal`
//! into a shared lower crate or introducing a dsl-runtime-local error
//! type. Phase 2 does not gate on inversion.
//!
//! # Phase 5c-migrate slice #80 cleanup
//!
//! The former `CustomOperation` trait, `CustomOpFactory`,
//! `CustomOperationRegistry`, and the scaffold `VerbRegistrar` trait were
//! all deleted once every plugin op had migrated to
//! `sem_os_postgres::ops::SemOsVerbOp`. The proc-macro crate
//! `dsl-runtime-macros` was removed in the same slice.
//!
//! # Visibility policy
//!
//! Explicit allowlist only — no wildcard `pub use`. Every new public
//! surface is added here deliberately so the plane boundary is reviewable
//! at a glance.
#![deny(unreachable_pub)]

// dsl-runtime is now the data plane only. The 13 analyser-tier modules
// (validation, verb_registry, runtime_registry, catalogue_loader,
// entity_kind, macros, ref_resolver, gateway_resolver, lsp_validator,
// suggestions, planning_facade, stategraph, verification) live in
// `dsl-analysis`. See docs/todo/dsl-runtime-split-v1.md.
mod bods;
pub mod coordination;
pub mod cross_workspace;
mod crud_executor;
mod document_bundles;
mod document_requirements;
mod domain_ops;
mod execution;
pub mod frame;
mod placeholder;
mod port;
mod service_traits;
mod services;
mod state_reducer;
mod tx;

pub use bods::{BodsRepository, DiscoveredUbo, UboDiscoveryResult, UboDiscoveryService, UboType};
pub use cross_workspace::{
    advance_to_current, begin_replay, check_idempotency, check_staleness_for_entity,
    confirm_compensation, create_remediation_event, current_version_number, defer,
    derive_platform_dag, escalate, get_atom_by_id, get_by_id as get_remediation_by_id,
    get_by_path as get_atom_by_path, get_current_version, get_propagation_result,
    get_version_history, insert_shared_atom, insert_version, list_active, list_all,
    list_for_provider, list_for_remediation, list_open, list_shared_atoms, list_stale_refs,
    mark_deferred, mark_resolved, mark_stale, record_call, record_compensation, record_if_active,
    resolve_slot_table, revoke_deferral, set_atom_path_table_map, set_slot_state_table,
    set_table_pk_overrides, supersede_call, transition_lifecycle, upsert_from_seed, upsert_ref,
    AtomPathTableMap, CascadeAction, CascadePlanner, ChildEntityResolver, ClauseKind,
    CompensationOutcome, CompensationRecordRow, CompensationSummary, ConditionResult,
    ConsumerRefStatus, CorrectionType, CreateRemediationInput, DerivedStateEvaluator,
    DerivedStateProjection, DerivedStateProjector, DerivedStateValue, ExternalCallRow, GateChecker,
    IdempotencyAction, LifecycleTransitionResult, NoChildrenResolver, PlatformDag, PlatformEdge,
    PostgresChildEntityResolver, PostgresSlotStateProvider, PredicateResolver, PropagationResult,
    ProviderCapability, ProviderCapabilitySummary, RebuildContext, RecordCallInput,
    RecordCompensationInput, RegisterSharedAtomInput, RemediationEventRow, RemediationEventSummary,
    RemediationStatus, ReplayOutcome, ReplayResult, ReplayTrigger, SameEntityResolver,
    SharedAtomDef, SharedAtomLifecycle, SharedAtomSummary, SharedAtomValidation,
    SharedFactVersionRow, SharedFactVersionSummary, SlotStateProvider, SqlPredicateResolver,
    StaleConsumerRef, StaleSharedFactRef, WorkspaceFactRefRow,
};

pub use crud_executor::PgCrudExecutor;
pub use document_bundles::{BundleContext, DocsBundleDef, DocsBundleRegistry, DocsBundleService};
pub use document_requirements::{
    ActiveDocumentPolicyBundle, DocumentPolicyService, GovernedDocumentRequirements,
    GovernedDocumentRequirementsService, GovernedRequirementMatrix, PublishedEvidenceStrategy,
    PublishedProofObligation, PublishedRequirementProfile,
};
pub use domain_ops::{
    emit_pending_state_advance, emit_pending_state_advance_batch, json_extract_bool,
    json_extract_bool_opt, json_extract_int, json_extract_int_opt, json_extract_string,
    json_extract_string_list, json_extract_string_list_opt, json_extract_string_opt,
    json_extract_uuid, json_extract_uuid_aliased, json_extract_uuid_opt, json_get_required_uuid,
    load_affinity_graph_cached, peek_pending_state_advance, take_pending_state_advance,
    StateTransitionInput,
};
pub use execution::{
    Result, VerbExecutionContext, VerbExecutionOutcome, VerbExecutionResult, VerbSideEffects,
};
pub use placeholder::{
    CreatePlaceholderRequest, PlaceholderEntity, PlaceholderKindCount, PlaceholderResolutionResult,
    PlaceholderResolver, PlaceholderStatus, PlaceholderSummary, PlaceholderWithDetails,
    ResolvePlaceholderRequest,
};
pub use port::{CrudExecutionPort, VerbExecutionPort};
pub use service_traits::{
    AttributeDispatchOutcome, AttributeIdentityService, AttributeService, ConstellationRuntime,
    LifecycleCatalog, McpToolRegistry, McpToolSpec, PhraseService, ProcessRegistryService,
    SchemaIntrospectionAccess, SemOsChildDispatcher, SemOsContextResolver, SemanticStateService,
    ServicePipelineService, SessionService, StewardshipDispatch, StewardshipOutcome,
    TradingProfileDocument, ViewService,
};
pub use services::{ServiceRegistry, ServiceRegistryBuilder};
pub use state_reducer::{
    build_eval_scope_tx, create_override, diagnose_slot, evaluate_aggregate, evaluate_rules,
    fetch_slot_overlays, fetch_slot_overlays_tx, get_active_override, get_active_override_tx,
    handle_state_blocked_why, handle_state_check_consistency, handle_state_derive,
    handle_state_derive_all, handle_state_diagnose, handle_state_list_overrides,
    handle_state_override, handle_state_revoke_override, list_active_overrides,
    load_builtin_state_machine, load_state_machine, parse_condition_body, parse_literal,
    parse_value, reduce_slot, revoke_override, validate_state_machine, AggFn, AggResult,
    BlockReason, BlockedVerb, BlockedWhyResult, CompareOp, ConditionBody, ConditionDef,
    ConditionEvaluation, ConditionEvaluator, ConsistencyCheckDef, ConsistencyWarning,
    CreateOverrideRequest, DerivationTrace, EvalScope, Expr, FieldValue, Literal, OverlayRow,
    OverlaySourceDef, OverrideInfo, Predicate, ReducerDef, ReducerError, ReducerResult, RuleDef,
    RuleEvaluation, ScopeData, SlotField, SlotOverlayData, SlotPredicate, SlotRecord,
    SlotReduceResult, StateMachineDefinition, StateOverride, TransitionDef, ValidatedStateMachine,
    Value,
};
pub use tx::TransactionScope;

#[cfg(any(test, feature = "harness"))]
pub use cross_workspace::test_harness::{self, LiveScenarioRunner, ScenarioRunner};

#[cfg(test)]
pub use port::test_support;

#[cfg(test)]
mod integration_tests;
