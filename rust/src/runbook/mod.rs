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

pub mod canonical;
#[cfg(feature = "vnext-repl")]
pub mod compiler;
#[cfg(feature = "vnext-repl")]
pub mod constraint_gate;
pub mod envelope;
pub mod errors;
pub mod executor;
pub mod response;
#[cfg(feature = "vnext-repl")]
pub mod sem_reg_filter;
#[cfg(feature = "vnext-repl")]
pub mod step_executor_bridge;
pub mod types;
#[cfg(feature = "vnext-repl")]
pub mod verb_classifier;
#[cfg(feature = "vnext-repl")]
pub mod write_set;

// Re-export key types at module boundary
pub use canonical::{
    canonical_bytes_for_envelope, canonical_bytes_for_envelope_core, canonical_bytes_for_step,
    canonical_bytes_for_steps, content_addressed_id, full_sha256,
};
#[cfg(feature = "vnext-repl")]
pub use compiler::compile_verb;
#[cfg(feature = "vnext-repl")]
pub use constraint_gate::check_pack_constraints;
pub use envelope::{EnvelopeCore, ReplayEnvelope};
pub use errors::{CompilationError, CompilationErrorKind};
#[cfg(feature = "database")]
pub use executor::PostgresRunbookStore;
pub use executor::{
    execute_runbook, ExecutionError, LockStats, RunbookEvent, RunbookExecutionResult, RunbookStore,
    RunbookStoreBackend, StepExecutionResult, StepExecutor, StepOutcome,
};
pub use response::{
    ClarificationContext, ClarificationRequest, CompiledRunbookSummary, ConstraintViolationDetail,
    MissingField, OrchestratorResponse, Remediation, StepPreview,
};
#[cfg(feature = "vnext-repl")]
pub use sem_reg_filter::{filter_verbs_against_allowed_set, DeniedVerb, SemRegFilterResult};
pub use types::{
    CompiledRunbook, CompiledRunbookId, CompiledRunbookStatus, CompiledStep, ExecutionMode,
    ParkReason, StepCursor,
};
#[cfg(feature = "vnext-repl")]
pub use verb_classifier::{classify_verb, VerbClassification};

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
#[cfg(feature = "vnext-repl")]
#[allow(clippy::too_many_arguments)]
pub fn compile_invocation(
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
mod tests {
    use super::*;

    #[cfg(feature = "vnext-repl")]
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

    #[cfg(feature = "vnext-repl")]
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
