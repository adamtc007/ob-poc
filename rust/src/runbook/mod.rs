//! Compiled runbook module — the sole executable truth.
//!
//! This module defines the **two public surfaces** of the runbook architecture:
//!
//! 1. **`compile_invocation()`** — the compile surface. Takes a resolved verb
//!    and args, runs classification → macro expansion → constraint checking →
//!    DAG ordering, and returns an `OrchestratorResponse`.
//!
//! 2. **`execute_runbook()`** — the execution gate. Takes a
//!    `CompiledRunbookId`, validates status, acquires advisory locks on the
//!    pre-computed write set, and iterates steps through the underlying
//!    executor.
//!
//! ## Invariants
//!
//! - **INV-1a**: A `CompiledRunbook` is immutable once created. Status
//!   transitions do not mutate steps or envelope.
//! - **INV-2**: Given the same `ReplayEnvelope`, re-execution produces the
//!   same verb call sequence.
//! - **INV-3**: `execute_runbook()` is the only path to the underlying
//!   executor. No raw DSL execution is permitted without a
//!   `CompiledRunbookId`.
//!
//! ## Phase 0 Scope
//!
//! Phase 0 establishes the type system and execution gate. The compile
//! surface (`compile_invocation`) is a thin wrapper that delegates to the
//! existing orchestrator pipeline. Full macro/pack/constraint integration
//! is wired in Phases 1-3.

// Phase 3 slice 2c.2 (2026-05-12): relocated to ob-poc-boundary.
pub(crate) use ob_poc_boundary::approval_token;
pub(crate) mod canonical;
pub(crate) mod compiler;
pub(crate) mod constraint_gate;
// Phase 3 slice 2c.2 (2026-05-12): relocated to ob-poc-boundary.
pub(crate) use ob_poc_boundary::dsl_drafter;
pub(crate) mod envelope;
pub(crate) mod errors;
pub(crate) mod executor;
// Phase 3 slice 2d-prep (2026-05-12): relocated to ob-poc-boundary.
pub(crate) use ob_poc_boundary::kyc_dry_run;
// Phase 3 slice 2d-prep (2026-05-12): relocated to ob-poc-boundary.
pub(crate) use ob_poc_boundary::language_pack;
pub(crate) mod llm_draft_adapter;
// Phase 3 slice 2c.2 (2026-05-12): relocated to ob-poc-boundary.
pub(crate) use ob_poc_boundary::mutation_preflight;
pub(crate) mod narration;
pub(crate) mod plan_compiler;
pub(crate) mod plan_executor;
pub(crate) mod plan_types;
pub(crate) mod response;
pub(crate) mod restricted_mutation;
pub(crate) mod sem_os_filter;
pub(crate) mod step_executor_bridge;
pub(crate) mod types;
pub(crate) mod verb_classifier;
// Phase 3 slice 2c (2026-05-12): workbook relocated to ob-poc-boundary; alias for crate use.
pub(crate) use ob_poc_boundary::workbook;
// Phase 3 slice 2d-prep (2026-05-12): relocated to ob-poc-boundary.
pub(crate) use ob_poc_boundary::workbook_diagnostics;
pub(crate) use ob_poc_boundary::workbook_revision;
pub(crate) mod write_set;

// Re-export key types at module boundary
pub use approval_token::{
    compute_approval_token_id, create_approval_token_for_workbook,
    validate_restricted_mutation_approval, ApprovalTokenId, ApprovalTokenValidationError,
    MutationApprovalToken, MutationApprovalTokenCore, MutationApprovalTokenStatus,
    ObservedMutationAnchors, RestrictedMutationApprovalCheck,
};
pub(crate) use canonical::{
    canonical_bytes_for_envelope, canonical_bytes_for_envelope_core, canonical_bytes_for_step,
    canonical_bytes_for_steps, content_addressed_id, full_sha256,
};
pub(crate) use compiler::compile_verb;
pub(crate) use constraint_gate::check_pack_constraints;
pub use dsl_drafter::{
    validate_workbook_for_dry_run, DslDrafterDryRunResult, DslDrafterExecutionMode,
    DslDrafterRefusalCode, DslDrafterValidationError, DslDrafterValidationStep,
    DslDrafterValidationStepStatus,
};
pub use envelope::{ReplayEnvelope};
pub(crate) use envelope::{EnvelopeCore};
pub(crate) use errors::{CompilationError, CompilationErrorKind};
#[cfg(feature = "database")]
pub(crate) use executor::PostgresRunbookStore;
pub use executor::{acquire_advisory_locks_on_scope, execute_runbook, execute_runbook_in_scope, RunbookStore, RunbookStoreBackend, StepOutcome};
pub(crate) use executor::{compute_write_set, ExecutionError, LockStats, RunbookEvent, RunbookExecutionResult, StepExecutionResult, StepExecutor, UnlockedExecutionToken};
// T0.3 (EOP-PLAN-CONTROLPLANE-001, closes C-022): test-only bypass for
// `execute_runbook` with a non-empty write_set and no pool. Re-exported
// unconditionally (not `#[cfg(test)]`) because `mod.rs` itself has no test
// cfg to gate on here; the symbol is only constructible/callable at all
// where `executor::execute_runbook_unlocked_for_tests` is compiled, i.e.
// under `#[cfg(test)]` in the defining module.
#[cfg(test)]
pub use executor::execute_runbook_unlocked_for_tests;
pub use kyc_dry_run::{
    build_kyc_update_status_dry_run, build_kyc_update_status_dry_run_with_manifest,
    KycUpdateStatusDryRunInput, KycUpdateStatusDryRunOutput, KycUpdateStatusDryRunRefusal,
};
pub use language_pack::{
    build_kyc_update_status_language_pack, build_update_status_language_pack,
    transition_language_pack_readiness, transition_language_pack_readiness_report, BlockedVerb,
    CanonicalMicroPattern, EvidencePolicySummary, KycLanguagePackRequest, LanguagePackArg,
    LanguagePackError, LanguagePackSubject, LanguagePackTransition, LanguagePackVerb,
    SemOsLanguagePack, TransitionEffect, TransitionLanguagePackReadiness,
    UpdateStatusLanguagePackRequest, UuidBindingRequirement,
};
pub use llm_draft_adapter::{run_kyc_update_status_llm_draft_loop, run_kyc_update_status_llm_draft_loop_with_prompt_pack};
pub(crate) use llm_draft_adapter::{LlmDraftAdapterRefusal, LlmDraftLoopOutcome, KYC_UPDATE_STATUS_LLM_DRAFT_PROMPT_TEMPLATE_VERSION};
pub use mutation_preflight::{
    prepare_restricted_mutation_preflight, MutationExecutor, MutationSemanticDiff,
    RestrictedMutationPreflight, RestrictedMutationPreflightError,
};
pub(crate) use response::{
    ClarificationContext, ClarificationRequest, CompiledRunbookSummary, ConstraintViolationDetail,
    MissingField, OrchestratorResponse, Remediation, StepPreview,
};
pub(crate) use restricted_mutation::{
    compile_restricted_mutation_preflight, record_restricted_mutation_execution_receipt,
    RestrictedMutationExecutionReceipt, RestrictedMutationRunbookCompilation,
    RestrictedMutationRunbookCompilationError,
};
pub(crate) use sem_os_filter::{filter_verbs_against_allowed_set, SemOsDeniedVerb, SemOsFilterResult};
pub use step_executor_bridge::{DslStepExecutor, GatePipeline, HashMapVerbTransitionLookup};
pub(crate) use step_executor_bridge::{DslExecutorV2StepExecutor, VerbExecutionPortStepExecutor, VerbTransitionLookup};
pub use types::{CompiledRunbook, CompiledRunbookId, CompiledRunbookStatus, CompiledStep, ExecutionMode};
pub(crate) use types::{ParkReason, StepCursor};
pub(crate) use verb_classifier::{classify_verb, VerbClassification};
pub use workbook::{
    compute_workbook_id, EvidenceRef, ExecutionWorkbook, ExecutionWorkbookCore,
    ExecutionWorkbookId, ExecutionWorkbookValidationError, LlmTraceRef, StaleWorkbookPolicy,
    WorkbookActor, WorkbookCheck, WorkbookCheckStatus, WorkbookExecutionMode, WorkbookSubject,
};
pub use workbook_diagnostics::{
    diagnostic_from_state_simulation, diagnostic_from_workbook_validation,
    diagnostics_from_dry_run_refusal, WorkbookDiagnostic,
};
pub use workbook_revision::{
    run_kyc_update_status_revision_loop, validate_kyc_update_status_draft_without_revision,
    KycUpdateStatusWorkbookDraft, LanguageAcquisitionMetrics, LanguageLoopTraceEvent,
    StructuredWorkbookRefusal, WorkbookDraftAttempt, WorkbookRevisionOutcome,
    MAX_WORKBOOK_REVISIONS,
};
pub(crate) use write_set::{derive_write_set, derive_write_set_heuristic};

// Re-export compile_invocation (defined below)
// Callers: `use crate::runbook::compile_invocation;`

// ---------------------------------------------------------------------------
// compile_invocation — compile surface
// ---------------------------------------------------------------------------

/// Compile surface: classify a resolved verb and compile it into a `CompiledRunbook`.
///
/// This function sits **after** verb discovery and arg extraction — those
/// stages are handled by the REPL orchestrator's `match_verb_for_input()`.
/// `compile_invocation()` handles the remaining steps:
///
/// ```text
/// verb_fqn + args (already resolved by REPL orchestrator)
///   → classify_verb (Primitive | Macro | Unknown)
///   → compile_verb (expand macros → constraint gate → assemble plan → freeze)
///   → OrchestratorResponse
/// ```
///
/// # Arguments
///
/// * `session_id` — session that owns this compilation
/// * `verb_fqn` — fully-qualified verb name (already resolved by verb search)
/// * `args` — extracted arguments (name → value)
/// * `session` — current session state (for macro context/autofill)
/// * `macro_registry` — for verb classification and macro expansion
/// * `verb_config_index` — for verb classification (primitive lookup)
/// * `constraints` — effective constraints from active pack
/// * `runbook_version` — monotonic version within the session
#[allow(clippy::too_many_arguments)]
pub(crate) fn compile_invocation(
    session_id: uuid::Uuid,
    verb_fqn: &str,
    args: &std::collections::BTreeMap<String, String>,
    session: &crate::session::unified::UnifiedSession,
    macro_registry: &crate::dsl_v2::macros::MacroRegistry,
    verb_config_index: &crate::repl::verb_config_index::VerbConfigIndex,
    constraints: &crate::journey::pack_manager::EffectiveConstraints,
    runbook_version: u64,
    sem_reg_allowed_verbs: Option<&std::collections::HashSet<String>>,
    verb_snapshot_pins: Option<&std::collections::HashMap<String, (uuid::Uuid, uuid::Uuid)>>,
) -> OrchestratorResponse {
    let classification = classify_verb(verb_fqn, verb_config_index, macro_registry);
    compile_verb(
        session_id,
        &classification,
        args,
        session,
        macro_registry,
        runbook_version,
        constraints,
        sem_reg_allowed_verbs,
        verb_snapshot_pins,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod invariant_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_invocation_unknown_verb_returns_clarification() {
        use crate::dsl_v2::macros::MacroRegistry;
        use crate::journey::pack_manager::EffectiveConstraints;
        use crate::repl::verb_config_index::VerbConfigIndex;
        use crate::session::unified::UnifiedSession;

        let session = UnifiedSession::new();
        let macro_reg = MacroRegistry::new();
        let verb_index = VerbConfigIndex::empty();
        let constraints = EffectiveConstraints::unconstrained();

        let resp = compile_invocation(
            uuid::Uuid::new_v4(),
            "nonexistent.verb",
            &std::collections::BTreeMap::new(),
            &session,
            &macro_reg,
            &verb_index,
            &constraints,
            1,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
        );
        assert!(
            matches!(resp, OrchestratorResponse::Clarification(_)),
            "Unknown verb should return Clarification, got {:?}",
            resp
        );
    }

    #[test]
    fn compile_invocation_delegates_classify_then_compile() {
        // Verify the real pipeline: classify → compile round-trip.
        // With empty registries, any verb is Unknown → Clarification.
        // Primitive and Macro paths are exercised in compiler.rs tests.
        use crate::dsl_v2::macros::MacroRegistry;
        use crate::journey::pack_manager::EffectiveConstraints;
        use crate::repl::verb_config_index::VerbConfigIndex;
        use crate::session::unified::UnifiedSession;

        let resp = compile_invocation(
            uuid::Uuid::new_v4(),
            "cbu.create",
            &std::collections::BTreeMap::new(),
            &UnifiedSession::new(),
            &MacroRegistry::new(),
            &VerbConfigIndex::empty(),
            &EffectiveConstraints::unconstrained(),
            1,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
        );
        // cbu.create is not in empty VerbConfigIndex → Unknown → Clarification
        assert!(matches!(resp, OrchestratorResponse::Clarification(_)));
    }

    #[test]
    fn re_exports_are_accessible() {
        // Verify that key types are re-exported at module boundary
        let _id = CompiledRunbookId::new();
        let _env = ReplayEnvelope::empty();
        let _store = RunbookStore::new();
    }
}
