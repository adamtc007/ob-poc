#![allow(unreachable_pub)]
//! Bridge adapters from REPL executor traits to `StepExecutor`.
//!
//! Two bridges are provided:
//!
//! 1. **`DslStepExecutor`** — wraps `Arc<dyn DslExecutor>` (sync-only path).
//!    Maps `Ok(json)` → `StepOutcome::Completed`, `Err(s)` → `StepOutcome::Failed`.
//!
//! 2. **`DslExecutorV2StepExecutor`** — wraps `Arc<dyn DslExecutorV2>` (durable/BPMN path).
//!    Maps `DslExecutionOutcome::Parked` → `StepOutcome::Parked` in addition to
//!    Completed/Failed.
//!
//! Both adapters extract the raw DSL string from `CompiledStep.dsl` — the same
//! string that was previously passed directly to the executor.

use std::sync::Arc;

use uuid::Uuid;

use dsl_runtime::{CascadeAction, CascadePlanner, GateChecker, TransactionScope};

use super::executor::StepOutcome;
use super::types::CompiledStep;
use crate::sequencer::{DslExecutionOutcome, DslExecutor, DslExecutorV2};

// ---------------------------------------------------------------------------
// DslStepExecutor — sync-only bridge
// ---------------------------------------------------------------------------

/// Bridge from `DslExecutor` (REPL's raw DSL executor) to `StepExecutor`.
///
/// Used for the standard sync execution path where parking is not possible.
pub struct DslStepExecutor {
    executor: Arc<dyn DslExecutor>,
}

impl DslStepExecutor {
    pub fn new(executor: Arc<dyn DslExecutor>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl super::executor::StepExecutor for DslStepExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        match self.executor.execute(&step.dsl).await {
            Ok(result) => StepOutcome::Completed { result },
            Err(error) => StepOutcome::Failed { error },
        }
    }

    /// Phase B.2b-δ (2026-04-22): routes step execution through the
    /// caller-owned scope so the runbook executor's outer scope (B.2b-ε)
    /// is shared across every step.
    async fn execute_step_in_scope(
        &self,
        step: &CompiledStep,
        scope: &mut dyn TransactionScope,
    ) -> StepOutcome {
        match self.executor.execute_in_scope(&step.dsl, scope).await {
            Ok(result) => StepOutcome::Completed { result },
            Err(error) => StepOutcome::Failed { error },
        }
    }
}

// ---------------------------------------------------------------------------
// DslExecutorV2StepExecutor — durable/BPMN bridge
// ---------------------------------------------------------------------------

/// Bridge from `DslExecutorV2` (WorkflowDispatcher path) to `StepExecutor`.
///
/// This adapter handles the `Parked` outcome from `DslExecutorV2`, mapping it
/// to `StepOutcome::Parked` so the execution gate can suspend the runbook
/// and record the cursor for later resumption.
pub struct DslExecutorV2StepExecutor {
    executor: Arc<dyn DslExecutorV2>,
    /// Runbook ID passed through to `execute_v2` for correlation.
    runbook_id: Uuid,
    session_stack: Option<ob_poc_types::session_stack::SessionStackState>,
}

impl DslExecutorV2StepExecutor {
    pub fn new(
        executor: Arc<dyn DslExecutorV2>,
        runbook_id: Uuid,
        session_stack: Option<ob_poc_types::session_stack::SessionStackState>,
    ) -> Self {
        Self {
            executor,
            runbook_id,
            session_stack,
        }
    }
}

#[async_trait::async_trait]
impl super::executor::StepExecutor for DslExecutorV2StepExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        match self
            .executor
            .execute_v2(
                &step.dsl,
                step.step_id,
                self.runbook_id,
                self.session_stack.clone(),
            )
            .await
        {
            DslExecutionOutcome::Completed(result) => StepOutcome::Completed { result },
            DslExecutionOutcome::Parked {
                correlation_key,
                message,
                ..
            } => StepOutcome::Parked {
                correlation_key,
                message,
            },
            DslExecutionOutcome::Failed(error) => StepOutcome::Failed { error },
        }
    }
}

// ---------------------------------------------------------------------------
// VerbExecutionPortStepExecutor — SemOS execution port bridge
// ---------------------------------------------------------------------------

/// Bridge from `VerbExecutionPort` (SemOS execution contract) to `StepExecutor`.
///
/// This adapter translates each `CompiledStep` into a `VerbExecutionPort::execute_verb()`
/// call, converting the step's verb FQN and args to JSON, and mapping the
/// `VerbExecutionOutcome` back to `StepOutcome`.
///
/// Optionally hooks the v1.3 cross-workspace gate checker (per
/// catalogue-platform-refinement-v1_3 §3.3 — runtime impact). When a
/// `GatePipeline` is attached, every step is gate-checked against the
/// loaded DAG taxonomies before dispatch.
pub struct VerbExecutionPortStepExecutor {
    port: Arc<dyn dsl_runtime::VerbExecutionPort>,
    /// Principal used for all executions in this runbook.
    principal: sem_os_core::principal::Principal,
    /// Session ID for correlation.
    session_id: Option<Uuid>,
    /// Optional v1.3 gate-check pipeline. When `Some`, every step is
    /// gate-checked against cross-workspace constraints before dispatch.
    gate_pipeline: Option<GatePipeline>,
    /// G1 item 2 (EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001 §2, §3):
    /// pool used for `lookup_sealed_handle` at the consume site. `None`
    /// when the caller has no pool available (the two in-crate unit-test
    /// constructors below) — `execute_step` then falls back to the prior
    /// hardcoded `None` handle, unchanged behaviour for those tests.
    pool: Option<sqlx::PgPool>,
}

/// Wiring for the v1.3 cross-workspace gate hook.
///
/// Held by VerbExecutionPortStepExecutor and consulted around every
/// step dispatch. Construction at orchestrator startup (one set of
/// shared `Arc`s reused across all step executors).
///
/// Components:
///   - `gate_checker` runs V1.3-1 blocking constraint checks BEFORE
///     dispatch.
///   - `cascade_planner` plans V1.3-3 hierarchy cascades AFTER
///     successful dispatch. Optional: if absent, no cascades are
///     planned.
#[derive(Clone)]
pub struct GatePipeline {
    pub registry: Arc<dsl_runtime::cross_workspace::DagRegistry>,
    pub gate_checker: Arc<GateChecker>,
    pub verb_metadata: Arc<dyn VerbTransitionLookup>,
    pub pool: Arc<sqlx::PgPool>,
    /// V1.3-3 cascade planner. Optional — when set, post-dispatch
    /// hook plans + executes single-level cascades (e.g. parent CBU
    /// suspended → all child CBUs suspended).
    pub cascade_planner: Option<Arc<CascadePlanner>>,
}

/// Resolves a verb FQN to its v1.3 `transition_args` metadata. Caller
/// implements this via a HashMap pre-populated from VerbsConfig at
/// startup.
pub trait VerbTransitionLookup: Send + Sync {
    fn lookup(&self, verb_fqn: &str) -> Option<dsl_core::TransitionArgs>;
}

/// A resolved DAG transition candidate for a verb (T9.1b, EOP-PLAN-CONTROLPLANE-001
/// Addendum B): the entity, its from/to state, and any blocking (severity=error)
/// violation found. `blocking_violations` has at most one entry — resolution
/// short-circuits on the first blocking violation found, exactly mirroring
/// `pre_dispatch_gate_check`'s original control flow (see [`resolve_transition_probe`]).
/// When empty, `from_state`/`to_state` are the first candidate `TransitionRef`'s
/// values — representative only when a verb has multiple candidate transitions and
/// no `target_state_arg` narrows them to one; `blocking_violations` (what callers
/// actually gate on) is always exact.
pub(crate) struct DagTransitionProbe {
    pub entity_id: uuid::Uuid,
    pub from_state: String,
    pub to_state: String,
    pub blocking_violations: Vec<String>,
}

/// Resolves a verb's `transition_args` metadata via `get_arg`, cross-references the
/// DAG's declared transitions, and runs `GateChecker::check_transition` against each
/// candidate — extracted from `pre_dispatch_gate_check`'s original inline body so a
/// shadow-only caller (T9.1b's `control_plane_shadow::build_dag_proof_input`) can
/// build a full `DagProofInput` from the same real mechanism the actual v1.3 gate
/// uses, without re-deriving it.
///
/// Control flow is byte-for-byte the same as the original inline version: the same
/// candidate `TransitionRef`s in the same order, the same `GateChecker` calls, and
/// the same short-circuit on the first found blocking violation — a later
/// candidate's `check_transition` is never invoked once an earlier one already
/// found a blocking violation, exactly as before. `pre_dispatch_gate_check` below
/// is now a thin wrapper reproducing its original `Result<(), String>` contract
/// from this function's `DagTransitionProbe`.
///
/// Returns `Ok(None)` when the verb has no `transition_args` declared, or when it
/// does but the DAG has no matching transition (both legitimate — most verbs are
/// not state transitions at all). Returns `Err` only for genuine resolution
/// failures (missing/invalid entity_id arg, unresolvable workspace) — the same
/// error shape `pre_dispatch_gate_check` always produced for these.
pub(crate) async fn resolve_transition_probe<'a>(
    pipe: &GatePipeline,
    verb_fqn: &str,
    get_arg: impl Fn(&str) -> Option<&'a str>,
) -> Result<Option<DagTransitionProbe>, String> {
    let Some(meta) = pipe.verb_metadata.lookup(verb_fqn) else {
        // Verb has no transition_args — no gate check applicable.
        return Ok(None);
    };

    // Resolve entity_id from args.
    let entity_str = get_arg(&meta.entity_id_arg).ok_or_else(|| {
        format!(
            "gate-check: verb '{}' transition_args.entity_id_arg='{}' \
             not found in step args",
            verb_fqn, meta.entity_id_arg
        )
    })?;
    let entity_id = uuid::Uuid::parse_str(entity_str).map_err(|e| {
        format!(
            "gate-check: verb '{}' arg '{}' value '{}' is not a UUID: {}",
            verb_fqn, meta.entity_id_arg, entity_str, e
        )
    })?;

    // Resolve target state from args (optional).
    let target_state_opt: Option<&str> = meta
        .target_state_arg
        .as_ref()
        .and_then(|arg| get_arg(arg));

    // Resolve target workspace + slot.
    let target_workspace = meta
        .target_workspace
        .as_deref()
        .or_else(|| verb_fqn.split('.').next())
        .ok_or_else(|| format!("gate-check: cannot infer target_workspace for '{}'", verb_fqn))?;
    let target_slot = meta.target_slot.as_deref().unwrap_or(target_workspace);

    // Cross-reference DAG transitions for this verb to determine
    // candidate (from, to) pairs. We accept either:
    //   * The transition matching `target_state` if declared
    //   * Any matching from→to pair otherwise (fail-conservative
    //     by checking each)
    let transitions = pipe.registry.transitions_for_verb(verb_fqn);
    let candidates: Vec<&dsl_runtime::cross_workspace::TransitionRef> = if let Some(ts) = target_state_opt {
        transitions
            .iter()
            .filter(|t| t.to_state.eq_ignore_ascii_case(ts))
            .collect()
    } else {
        transitions.iter().collect()
    };

    if candidates.is_empty() {
        // Verb is gate-metadata-aware but DAG has no matching
        // transition. This is expected for verbs that operate on
        // non-state aspects (e.g. updates that don't transition).
        return Ok(None);
    }

    for cand in &candidates {
        let violations = pipe
            .gate_checker
            .check_transition(
                target_workspace,
                target_slot,
                entity_id,
                &cand.from_state,
                &cand.to_state,
                &pipe.pool,
            )
            .await
            .map_err(|e| format!("gate-check error: {e}"))?;
        for v in &violations {
            if v.severity == "error" {
                return Ok(Some(DagTransitionProbe {
                    entity_id,
                    from_state: cand.from_state.clone(),
                    to_state: cand.to_state.clone(),
                    blocking_violations: vec![format!(
                        "v1.3 gate violation [{}]: {}",
                        v.constraint_id, v.message
                    )],
                }));
            }
        }
    }

    let first = candidates[0];
    Ok(Some(DagTransitionProbe {
        entity_id,
        from_state: first.from_state.clone(),
        to_state: first.to_state.clone(),
        blocking_violations: Vec::new(),
    }))
}

impl VerbExecutionPortStepExecutor {
    pub fn new(
        port: Arc<dyn dsl_runtime::VerbExecutionPort>,
        principal: sem_os_core::principal::Principal,
        session_id: Option<Uuid>,
    ) -> Self {
        Self {
            port,
            principal,
            session_id,
            gate_pipeline: None,
            pool: None,
        }
    }

    /// G1 item 2 (EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001 §2, §3):
    /// attach the pool `execute_step` uses to look up the sealed envelope
    /// `phase5_runtime_recheck` (or its `HumanGate` re-seal sibling)
    /// persisted for this step's `entry_id` — replaces the hardcoded
    /// `None` handle at the consume call site.
    pub fn with_pool(mut self, pool: sqlx::PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Attach a v1.3 gate-check pipeline. When set, each `execute_step`
    /// call invokes the GateChecker against the verb's declared
    /// transitions before dispatching to the port.
    pub fn with_gate_pipeline(mut self, pipeline: GatePipeline) -> Self {
        self.gate_pipeline = Some(pipeline);
        self
    }

    /// Run V1.3-1 gate checks for the given step. Returns Ok(()) if no
    /// errors were found, Err with a violation message otherwise.
    /// Returns Ok(()) when no GatePipeline is attached.
    async fn pre_dispatch_gate_check(&self, step: &CompiledStep) -> Result<(), String> {
        let Some(pipe) = &self.gate_pipeline else {
            return Ok(());
        };
        let probe = resolve_transition_probe(pipe, &step.verb, |arg| {
            step.args.get(arg).map(|s| s.as_str())
        })
        .await?;
        let Some(probe) = probe else {
            return Ok(());
        };
        if let Some(v) = probe.blocking_violations.first() {
            return Err(v.clone());
        }
        Ok(())
    }

    /// Run V1.3-3 hierarchy cascade planning + execution after a
    /// successful step. Returns the count of cascade actions executed
    /// (informational; surfaced via tracing).
    ///
    /// Single-level execution: applies state writes directly via
    /// SlotStateProvider's table mapping. Does NOT recursively go
    /// through verb dispatch — full recursive cascade with gate-checks
    /// per level is a follow-up. The current impl handles the common
    /// case (parent CBU suspended → child CBUs suspended) and logs
    /// any cascade-of-cascades for ops follow-up.
    async fn post_dispatch_cascade(&self, step: &CompiledStep) {
        let Some(pipe) = &self.gate_pipeline else {
            return;
        };
        let Some(planner) = &pipe.cascade_planner else {
            return;
        };
        let Some(meta) = pipe.verb_metadata.lookup(&step.verb) else {
            return;
        };
        let Some(entity_str) = step.args.get(&meta.entity_id_arg) else {
            return;
        };
        let Ok(entity_id) = uuid::Uuid::parse_str(entity_str) else {
            return;
        };
        let Some(target_workspace) = meta
            .target_workspace
            .as_deref()
            .or_else(|| step.verb.split('.').next())
        else {
            return;
        };
        let target_slot = meta.target_slot.as_deref().unwrap_or(target_workspace);

        // The transition has just succeeded. We need to know what
        // state the parent moved INTO, then plan cascades for that
        // parent_new_state. Resolve via the DAG: pick the transition
        // matching this verb whose `to_state` matches the
        // target_state arg (if declared) or the unique to_state
        // (otherwise).
        let target_state = meta
            .target_state_arg
            .as_ref()
            .and_then(|arg| step.args.get(arg).cloned());
        let transitions = pipe.registry.transitions_for_verb(&step.verb);
        let to_state = match target_state {
            Some(ts) => ts,
            None => {
                // No target_state arg — the verb's transitions in the
                // registry should agree on a single to_state.
                let tos: std::collections::HashSet<&str> =
                    transitions.iter().map(|t| t.to_state.as_str()).collect();
                if tos.len() != 1 {
                    return; // ambiguous; skip cascade for safety
                }
                // Length checked above (== 1) — `next()` is Some by construction.
                let Some(only) = tos.into_iter().next() else {
                    return;
                };
                only.to_string()
            }
        };

        let actions = match planner
            .plan_cascade(
                target_workspace,
                target_slot,
                entity_id,
                &to_state,
                &pipe.pool,
            )
            .await
        {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(
                    verb = %step.verb,
                    parent_id = %entity_id,
                    "v1.3 cascade planning failed: {e}"
                );
                return;
            }
        };

        if actions.is_empty() {
            return;
        }

        tracing::info!(
            verb = %step.verb,
            parent_id = %entity_id,
            parent_new_state = %to_state,
            cascade_count = actions.len(),
            "v1.3 cascade fired"
        );

        // Single-level execution via direct state-column write.
        // Resolve table + state column via the SlotStateProvider's
        // dispatch table (re-using the same mapping).
        for action in &actions {
            if let Err(e) = apply_cascade_state_write(action, &pipe.pool).await {
                tracing::warn!(
                    constraint = %action.constraint_id_or_unspecified(),
                    child_workspace = %action.child_workspace,
                    child_slot = %action.child_slot,
                    child_entity_id = %action.child_entity_id,
                    target_state = %action.target_state,
                    "v1.3 cascade write failed: {e}"
                );
            }
        }
    }
}

/// Apply a single cascade action by writing the target state directly
/// to the child entity's state column.
///
/// Bypasses the verb dispatch path — no recursive gate check, no
/// further cascades. This is a deliberate scope limit; full recursive
/// dispatch is a follow-up.
async fn apply_cascade_state_write(
    action: &CascadeAction,
    pool: &sqlx::PgPool,
) -> anyhow::Result<()> {
    use dsl_runtime::resolve_slot_table;
    let (table, col, pk) = resolve_slot_table(&action.child_workspace, &action.child_slot)?;
    let sql = format!(
        r#"UPDATE "ob-poc".{tbl} SET {col} = $1 WHERE {pk} = $2"#,
        tbl = table,
        col = col,
        pk = pk,
    );
    sqlx::query(&sql)
        .bind(&action.target_state)
        .bind(action.child_entity_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Convenience trait extension — CascadeAction doesn't carry the
/// constraint id, but we surface useful identification in logs.
trait CascadeActionExt {
    fn constraint_id_or_unspecified(&self) -> &str;
}

impl CascadeActionExt for CascadeAction {
    fn constraint_id_or_unspecified(&self) -> &str {
        // CascadeAction carries rule_parent_state which uniquely keys
        // the rule; surface it as a constraint identifier proxy.
        self.rule_parent_state.as_str()
    }
}

#[async_trait::async_trait]
impl super::executor::StepExecutor for VerbExecutionPortStepExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        // V1.3 cross-workspace gate check (no-op when pipeline absent).
        if let Err(error) = self.pre_dispatch_gate_check(step).await {
            return StepOutcome::Failed { error };
        }

        // Build execution context
        let mut ctx = dsl_runtime::VerbExecutionContext::new(self.principal.clone());
        if let Some(sid) = self.session_id {
            ctx.extensions = serde_json::json!({"session_id": sid.to_string()});
        }

        // Convert step args (BTreeMap<String, String>) to JSON object
        let args: serde_json::Value = step
            .args
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        // Execute through the SemOS port.
        //
        // T6.1-style admission wiring (EOP-PLAN-CONTROLPLANE-001, PIR-D-002):
        // routes through the T4.1 envelope-admission entry point instead of
        // the bare `execute_verb`, mirroring the bus adapter's change
        // (`ob-poc-web/src/bus_runtime.rs`). With the production-default
        // empty `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` this remains
        // behaviourally identical to the prior direct `execute_verb` call
        // (`NotEnforced`) for every verb dispatched through the runbook step
        // executor (Path A) — zero dispatch-outcome change. This closes the
        // "Path A never reaches the admission mechanism at all" gap
        // (independent adversarial review, docs/research/control-plane-pir-001.md,
        // PIR-D-002) and satisfies the graduation runbook's Path A Step 0
        // precondition (docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md
        // §4) — Path A's shadow-evaluation window for any gate wired here
        // still starts fresh from this commit per the runbook's §1
        // graduation-window rule, not before.
        //
        // G1 item 2 (EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001 §2, §3):
        // the envelope handle is no longer hardcoded `None` — it's looked
        // up from `control_plane_envelopes` by `(session_id, step.step_id)`
        // (`step.step_id` already equals the `RunbookEntry.id`/`entry_id`
        // that `phase5_runtime_recheck` sealed under, `runbook/types.rs`'s
        // own doc comment). `None` (no pool attached, no session id, or
        // nothing sealed for this entry) degrades to the exact same
        // pre-G1 behaviour this comment above already documents as
        // dispatch-outcome-neutral while `ENFORCE_VERBS` stays empty.
        let envelope_handle = match (&self.pool, self.session_id) {
            (Some(pool), Some(session_id)) => {
                match crate::agent::control_plane_envelope_store::lookup_sealed_handle(
                    pool,
                    session_id,
                    step.step_id,
                )
                .await
                {
                    Ok(handle) => handle,
                    Err(err) => {
                        tracing::warn!(
                            step_id = %step.step_id,
                            verb = %step.verb,
                            error = %err,
                            "lookup_sealed_handle failed (degrading to no-envelope, matching pre-G1 NotEnforced behaviour)"
                        );
                        None
                    }
                }
            }
            _ => None,
        };

        let outcome = match self
            .port
            .execute_verb_admitting_envelope(&step.verb, args, &mut ctx, envelope_handle)
            .await
        {
            Ok(result) => {
                let json = match &result.outcome {
                    dsl_runtime::VerbExecutionOutcome::Uuid(id) => {
                        serde_json::json!({"type": "uuid", "value": id.to_string()})
                    }
                    dsl_runtime::VerbExecutionOutcome::Record(v) => {
                        serde_json::json!({"type": "record", "value": v})
                    }
                    dsl_runtime::VerbExecutionOutcome::RecordSet(v) => {
                        serde_json::json!({"type": "record_set", "value": v})
                    }
                    dsl_runtime::VerbExecutionOutcome::Affected(n) => {
                        serde_json::json!({"type": "affected", "value": n})
                    }
                    dsl_runtime::VerbExecutionOutcome::Void => {
                        serde_json::json!({"type": "void"})
                    }
                };
                StepOutcome::Completed { result: json }
            }
            Err(e) => StepOutcome::Failed {
                error: e.to_string(),
            },
        };

        // V1.3-3 cascade hook (post-success only). No-op when no
        // pipeline / no cascade_planner / verb has no transition_args.
        if matches!(outcome, StepOutcome::Completed { .. }) {
            self.post_dispatch_cascade(step).await;
        }

        outcome
    }
}

// ---------------------------------------------------------------------------
// HashMapVerbTransitionLookup — production helper
// ---------------------------------------------------------------------------

/// Default `VerbTransitionLookup` implementation backed by a HashMap.
///
/// Production usage: build once at startup from a loaded `VerbsConfig`
/// using [`HashMapVerbTransitionLookup::from_verbs_config`], wrap in
/// `Arc`, share into the `GatePipeline`.
pub struct HashMapVerbTransitionLookup {
    map: std::collections::HashMap<String, dsl_core::TransitionArgs>,
}

impl HashMapVerbTransitionLookup {
    pub fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
        }
    }

    pub fn from_verbs_config(cfg: &dsl_core::VerbsConfig) -> Self {
        let mut map = std::collections::HashMap::new();
        for (domain_name, domain) in &cfg.domains {
            for (verb_name, verb) in &domain.verbs {
                if let Some(ta) = &verb.transition_args {
                    map.insert(format!("{domain_name}.{verb_name}"), ta.clone());
                }
            }
        }
        Self { map }
    }
}

impl Default for HashMapVerbTransitionLookup {
    fn default() -> Self {
        Self::new()
    }
}

impl VerbTransitionLookup for HashMapVerbTransitionLookup {
    fn lookup(&self, verb_fqn: &str) -> Option<dsl_core::TransitionArgs> {
        self.map.get(verb_fqn).cloned()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::executor::StepExecutor;
    use crate::runbook::types::ExecutionMode;

    /// Stub DslExecutor that returns success.
    struct SuccessExecutor;

    #[async_trait::async_trait]
    impl DslExecutor for SuccessExecutor {
        async fn execute(&self, _dsl: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"status": "ok"}))
        }
    }

    /// Stub DslExecutor that returns failure.
    struct FailureExecutor;

    #[async_trait::async_trait]
    impl DslExecutor for FailureExecutor {
        async fn execute(&self, _dsl: &str) -> Result<serde_json::Value, String> {
            Err("execution failed".into())
        }
    }

    // Build a HashMapVerbTransitionLookup from a hand-rolled VerbsConfig
    // so the gate-pipeline construction can be smoke-tested without
    // loading real YAML files.
    #[test]
    fn hashmap_verb_transition_lookup_construction() {
        use dsl_core::{DomainConfig, TransitionArgs, VerbBehavior, VerbConfig, VerbsConfig};
        use std::collections::HashMap;

        let mut domain = DomainConfig {
            description: "test".into(),
            verbs: HashMap::new(),
            dynamic_verbs: vec![],
            invocation_hints: vec![],
        };
        domain.verbs.insert(
            "update-status".into(),
            VerbConfig {
                description: "test".into(),
                behavior: VerbBehavior::Plugin,
                crud: None,
                handler: None,
                graph_query: None,
                durable: None,
                args: vec![],
                returns: None,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                metadata: None,
                invocation_phrases: vec![],
                policy: None,
                sentences: None,
                confirm_policy: None,
                outputs: vec![],
                three_axis: None,
                transition_args: Some(TransitionArgs {
                    entity_id_arg: "deal-id".into(),
                    target_state_arg: Some("new-status".into()),
                    target_workspace: Some("deal".into()),
                    target_slot: Some("deal".into()),
                }),
                ..Default::default()
            },
        );
        let mut domains = HashMap::new();
        domains.insert("deal".into(), domain);
        let cfg = VerbsConfig {
            version: "1.0".into(),
            domains,
        };
        let lookup = HashMapVerbTransitionLookup::from_verbs_config(&cfg);
        let ta = lookup.lookup("deal.update-status").expect("found");
        assert_eq!(ta.entity_id_arg, "deal-id");
        assert_eq!(ta.target_state_arg.as_deref(), Some("new-status"));
        assert_eq!(ta.target_workspace.as_deref(), Some("deal"));
        // Verb without transition_args returns None.
        assert!(lookup.lookup("nonexistent.verb").is_none());
    }

    /// Stub DslExecutorV2 that returns Parked.
    struct ParkingExecutor;

    #[async_trait::async_trait]
    impl DslExecutorV2 for ParkingExecutor {
        async fn execute_v2(
            &self,
            _dsl: &str,
            _entry_id: Uuid,
            _runbook_id: Uuid,
            _session_stack: Option<ob_poc_types::session_stack::SessionStackState>,
        ) -> DslExecutionOutcome {
            DslExecutionOutcome::Parked {
                task_id: Uuid::nil(),
                correlation_key: "test-corr-key".into(),
                timeout: None,
                message: "Awaiting callback".into(),
            }
        }
    }

    fn test_step() -> CompiledStep {
        CompiledStep {
            step_id: Uuid::new_v4(),
            sentence: "test step".into(),
            verb: "test.verb".into(),
            dsl: "(test.verb :arg1 \"value\")".into(),
            args: std::collections::BTreeMap::new(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        }
    }

    #[tokio::test]
    async fn test_dsl_step_executor_success() {
        let executor = DslStepExecutor::new(Arc::new(SuccessExecutor));
        let step = test_step();
        let outcome = executor.execute_step(&step).await;

        match outcome {
            StepOutcome::Completed { result } => {
                assert_eq!(result["status"], "ok");
            }
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_dsl_step_executor_failure() {
        let executor = DslStepExecutor::new(Arc::new(FailureExecutor));
        let step = test_step();
        let outcome = executor.execute_step(&step).await;

        match outcome {
            StepOutcome::Failed { error } => {
                assert_eq!(error, "execution failed");
            }
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_dsl_executor_v2_step_executor_parked() {
        let executor =
            DslExecutorV2StepExecutor::new(Arc::new(ParkingExecutor), Uuid::new_v4(), None);
        let step = test_step();
        let outcome = executor.execute_step(&step).await;

        match outcome {
            StepOutcome::Parked {
                correlation_key,
                message,
            } => {
                assert_eq!(correlation_key, "test-corr-key");
                assert_eq!(message, "Awaiting callback");
            }
            other => panic!("Expected Parked, got {:?}", other),
        }
    }

    // ── T9.1b: resolve_transition_probe extraction equivalence ─────────
    //
    // Proves `pre_dispatch_gate_check`'s production behavior is unchanged
    // by the extraction: same Ok/Err outcomes, same error message text,
    // for the same fail-conservative short-circuit-on-first-violation
    // control flow the original inline body had.

    /// In-memory `SlotStateProvider` — no DB, no `harness` feature needed.
    #[derive(Default)]
    struct FixedSlotState {
        states: std::collections::HashMap<(String, String, Uuid), Option<String>>,
    }

    impl FixedSlotState {
        fn set(&mut self, workspace: &str, slot: &str, entity_id: Uuid, state: Option<&str>) {
            self.states.insert(
                (workspace.to_string(), slot.to_string(), entity_id),
                state.map(String::from),
            );
        }
    }

    #[async_trait::async_trait]
    impl dsl_runtime::cross_workspace::SlotStateProvider for FixedSlotState {
        async fn read_slot_state(
            &self,
            workspace: &str,
            slot: &str,
            entity_id: Uuid,
            _pool: &sqlx::PgPool,
        ) -> anyhow::Result<Option<String>> {
            Ok(self
                .states
                .get(&(workspace.to_string(), slot.to_string(), entity_id))
                .cloned()
                .unwrap_or(None))
        }
    }

    struct FixedLookup(Option<dsl_core::TransitionArgs>);

    impl VerbTransitionLookup for FixedLookup {
        fn lookup(&self, _verb_fqn: &str) -> Option<dsl_core::TransitionArgs> {
            self.0.clone()
        }
    }

    /// Never actually invoked by these tests — `pre_dispatch_gate_check`
    /// gate-checks only, it never dispatches. Exists so
    /// `VerbExecutionPortStepExecutor::new` has a port to construct with.
    struct UnusedPort;

    #[async_trait::async_trait]
    impl dsl_runtime::VerbExecutionPort for UnusedPort {
        async fn execute_verb(
            &self,
            _verb_fqn: &str,
            _args: serde_json::Value,
            _ctx: &mut dsl_runtime::VerbExecutionContext,
        ) -> dsl_runtime::Result<dsl_runtime::VerbExecutionResult> {
            unreachable!("pre_dispatch_gate_check tests never dispatch")
        }
    }

    /// One workspace, one slot with a single FROM -> TO transition declared
    /// via `test.transition-verb`, and one cross-workspace constraint
    /// gating that exact transition on `gate_slot` reading `READY`.
    const TEST_DAG_YAML: &str = r#"
workspace: testws
dag_id: test_dag
slots:
  - id: testslot
    stateless: false
    state_machine:
      id: sm
      states: [{ id: FROM, entry: true }, { id: TO }]
      transitions:
        - from: FROM
          to: TO
          via: test.transition-verb
cross_workspace_constraints:
  - id: test_gate
    source_workspace: testws
    source_slot: gate_slot
    source_state: [READY]
    target_workspace: testws
    target_slot: testslot
    target_transition: "FROM -> TO"
    severity: error
"#;

    fn test_gate_pipeline(entity_id: Uuid, gate_state: Option<&str>) -> GatePipeline {
        let dir = std::env::temp_dir().join(format!("t91b_test_dag_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.yaml"), TEST_DAG_YAML).unwrap();
        let registry = Arc::new(dsl_runtime::cross_workspace::DagRegistry::from_dir(&dir).unwrap());
        std::fs::remove_dir_all(&dir).ok();

        let mut states = FixedSlotState::default();
        states.set("testws", "gate_slot", entity_id, gate_state);

        let gate_checker = Arc::new(GateChecker::new(
            registry.clone(),
            Arc::new(states),
            Arc::new(dsl_runtime::cross_workspace::SameEntityResolver),
        ));
        let verb_metadata: Arc<dyn VerbTransitionLookup> = Arc::new(FixedLookup(Some(dsl_core::TransitionArgs {
            entity_id_arg: "entity-id".into(),
            target_state_arg: None,
            target_workspace: Some("testws".into()),
            target_slot: Some("testslot".into()),
        })));
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://harness-mock-never-connects")
            .expect("connect_lazy with a valid-shaped URL never fails");

        GatePipeline {
            registry,
            gate_checker,
            verb_metadata,
            pool: Arc::new(pool),
            cascade_planner: None,
        }
    }

    fn test_step_with_entity(entity_id: Uuid) -> CompiledStep {
        let mut step = test_step();
        step.verb = "test.transition-verb".to_string();
        step.args.insert("entity-id".to_string(), entity_id.to_string());
        step
    }

    #[tokio::test]
    async fn resolve_transition_probe_legal_transition_has_no_blocking_violations() {
        let entity_id = Uuid::new_v4();
        let pipe = test_gate_pipeline(entity_id, Some("READY"));
        let args: std::collections::BTreeMap<String, String> =
            [("entity-id".to_string(), entity_id.to_string())].into();

        let probe = resolve_transition_probe(&pipe, "test.transition-verb", |a| {
            args.get(a).map(|s| s.as_str())
        })
        .await
        .expect("resolution succeeds")
        .expect("verb has transition_args and a matching DAG transition");

        assert_eq!(probe.entity_id, entity_id);
        assert_eq!(probe.from_state, "FROM");
        assert_eq!(probe.to_state, "TO");
        assert!(probe.blocking_violations.is_empty());
    }

    #[tokio::test]
    async fn resolve_transition_probe_violating_transition_short_circuits_with_one_violation() {
        let entity_id = Uuid::new_v4();
        // gate_slot is NOT "READY" -> the constraint is violated.
        let pipe = test_gate_pipeline(entity_id, Some("NOT_READY"));
        let args: std::collections::BTreeMap<String, String> =
            [("entity-id".to_string(), entity_id.to_string())].into();

        let probe = resolve_transition_probe(&pipe, "test.transition-verb", |a| {
            args.get(a).map(|s| s.as_str())
        })
        .await
        .expect("resolution succeeds")
        .expect("verb has transition_args and a matching DAG transition");

        assert_eq!(probe.blocking_violations.len(), 1);
        assert!(probe.blocking_violations[0].contains("test_gate"));
        assert!(probe.blocking_violations[0].starts_with("v1.3 gate violation ["));
    }

    #[tokio::test]
    async fn pre_dispatch_gate_check_equivalence_legal_and_violating() {
        // The whole point of the extraction: pre_dispatch_gate_check's
        // Ok/Err contract (and Err message text) must be byte-for-byte
        // identical to the pre-extraction inline version.
        let legal_entity = Uuid::new_v4();
        let legal_pipe = test_gate_pipeline(legal_entity, Some("READY"));
        let legal_executor = VerbExecutionPortStepExecutor::new(
            Arc::new(UnusedPort),
            sem_os_core::principal::Principal::system(),
            None,
        )
        .with_gate_pipeline(legal_pipe);
        let legal_step = test_step_with_entity(legal_entity);
        assert!(legal_executor.pre_dispatch_gate_check(&legal_step).await.is_ok());

        let bad_entity = Uuid::new_v4();
        let bad_pipe = test_gate_pipeline(bad_entity, Some("NOT_READY"));
        let bad_executor = VerbExecutionPortStepExecutor::new(
            Arc::new(UnusedPort),
            sem_os_core::principal::Principal::system(),
            None,
        )
        .with_gate_pipeline(bad_pipe);
        let bad_step = test_step_with_entity(bad_entity);
        let err = bad_executor
            .pre_dispatch_gate_check(&bad_step)
            .await
            .expect_err("violating transition must be rejected");
        assert!(err.starts_with("v1.3 gate violation [test_gate]:"));
    }
}

/// G1 item 2/3 (EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001,
/// EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3 G1 item 3): the `t4_1`
/// property set proven from the REAL Path A call site —
/// `VerbExecutionPortStepExecutor::execute_step`, via its real
/// `lookup_sealed_handle` wiring — not just from the adapter's own direct
/// `execute_verb_admitting_envelope`/`admit_in_scope` tests (which prove
/// the mechanism works when handed a handle directly, not that Path A
/// actually threads one to it).
#[cfg(all(test, feature = "database"))]
mod g1_item2_path_a_tests {
    use super::*;
    use crate::runbook::executor::StepExecutor;
    use crate::runbook::types::ExecutionMode;
    use crate::sem_os_runtime::verb_executor_adapter::ObPocVerbExecutor;

    async fn test_pool() -> sqlx::PgPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        sqlx::PgPool::connect(&url).await.expect("connect")
    }

    // Same process-global-env-var guard pattern as
    // `sem_os_runtime::verb_executor_adapter::t4_1_envelope_admission_tests::EnvGuard`
    // — duplicated locally (module-private there) rather than widened to
    // cross-module visibility for one shared test helper.
    static ENV_GUARD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    struct EnvGuard(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);
    impl EnvGuard {
        fn set(verb_fqn: &str) -> Self {
            let guard = ENV_GUARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            std::env::set_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS", verb_fqn);
            Self(guard)
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS");
        }
    }

    fn path_a_step(step_id: Uuid, verb: &str, args: &[(&str, String)]) -> CompiledStep {
        CompiledStep {
            step_id,
            sentence: "g1 item 2/3 path a test".into(),
            verb: verb.to_string(),
            dsl: format!("({verb})"),
            args: args.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        }
    }

    /// G1 item 3, assertion 2 / item 4: an enforced verb with NOTHING
    /// sealed for this step's entry_id is rejected at the real Path A
    /// call site — `lookup_sealed_handle` genuinely finds no row (not a
    /// stubbed `None`), and the rejection carries the same triage-
    /// classifiable message `admit_in_scope` produces
    /// (`verb_executor_adapter.rs`'s own `"{verb_fqn} is enforce-mode
    /// gated... but no sealed envelope was presented"`), not a bare
    /// dispatch error indistinguishable from an unrelated failure. Proves
    /// the outage framing (G1 design doc §0) is real from Path A's own
    /// call pattern, not hypothetical, and stays real until a verb
    /// graduates.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn no_sealed_envelope_for_this_entry_is_rejected_with_triage_classification() {
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;
        let session_id = Uuid::now_v7();
        let entry_id = Uuid::now_v7();

        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let bridge = VerbExecutionPortStepExecutor::new(
            std::sync::Arc::new(executor),
            sem_os_core::principal::Principal::system(),
            Some(session_id),
        )
        .with_pool(pool.clone());

        let step = path_a_step(entry_id, "cbu.confirm", &[]);
        let outcome = bridge.execute_step(&step).await;

        match outcome {
            crate::runbook::executor::StepOutcome::Failed { error } => {
                assert!(
                    error.contains("is enforce-mode gated") && error.contains("no sealed envelope was presented"),
                    "expected the real admit_in_scope RejectedNoEnvelope message, got: {error}"
                );
            }
            other => panic!("expected Failed (no envelope sealed for this entry), got {other:?}"),
        }
    }

    /// G1 item 3, assertions 1 and 3: end-to-end admit-and-consume from
    /// the real Path A call site, then single-use — a second `execute_step`
    /// call against the SAME compiled step (simulating a caller bug: no
    /// intervening re-seal) finds the row `lookup_sealed_handle` would
    /// return already consumed (excluded by its own `status = 'sealed'`
    /// filter), so the *system's* behaviour is "the stale handle is
    /// simply not found again," matching §5's "a retry naturally consumes
    /// its own fresh envelope, never a stale one" — Path A never
    /// manufactures a raw resubmission of the same handle by construction
    /// (that raw-resubmission property is already proven at the adapter
    /// level, `t4_1_envelope_admission_tests`; this test is about Path
    /// A's actual call pattern).
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn admits_consumes_once_from_path_a_then_a_bare_retry_finds_nothing_sealed() {
        let _guard = EnvGuard::set("cbu.confirm");
        // `enforce_requires_states_precondition_with_mode` fail-closes on
        // `NoSlotMapping` when no `SlotStateProvider` is wired (only the
        // real server startup path wires one) — set fail-open so this
        // isolated unit test can exercise a real dispatch; unrelated to
        // G1/G2's own subject, this is the pre-existing lifecycle-gate
        // mechanism, matching its own doc's documented escape hatch.
        std::env::set_var("OB_POC_LIFECYCLE_GATE_MODE", "fail-open");
        let pool = test_pool().await;
        let session_id = Uuid::now_v7();
        let entry_id = Uuid::now_v7();

        // A real, currently-VALIDATION_PENDING cbu row — test setup
        // against the dev database, matching this file's neighbouring
        // live-DB tests' own convention (e.g. T10.2's pin-drift test
        // reads real `cbus` rows directly).
        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("at least one cbu row exists in the dev database");
        sqlx::query(r#"UPDATE "ob-poc".cbus SET status = 'VALIDATION_PENDING' WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&pool)
            .await
            .expect("reset fixture cbu to VALIDATION_PENDING");

        // Seal a real envelope for this entry_id — the same shape
        // `persist_sealed` writes (raw INSERT, matching this crate's
        // existing t4_1-style live-DB test convention rather than
        // reaching across the module boundary for a pub(crate) helper).
        let envelope_id = Uuid::now_v7();
        let content_hash_hex = "0".repeat(64); // 32 zero bytes, hex-encoded — matches EnvelopeHandle::content_hash_hex's format
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, entry_id, verb_fqn,
                status, not_before, not_after
            ) VALUES ($1, $2, $3, $4, 'cbu.confirm', 'sealed', now() - interval '1 minute', now() + interval '5 minutes')
            "#,
        )
        .bind(envelope_id)
        .bind(&content_hash_hex)
        .bind(session_id)
        .bind(entry_id)
        .execute(&pool)
        .await
        .expect("insert sealed envelope row");

        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let bridge = VerbExecutionPortStepExecutor::new(
            std::sync::Arc::new(executor),
            sem_os_core::principal::Principal::system(),
            Some(session_id),
        )
        .with_pool(pool.clone());

        let step = path_a_step(entry_id, "cbu.confirm", &[("cbu-id", cbu_id.to_string())]);

        // First attempt: lookup_sealed_handle finds the sealed row and
        // threads it into execute_verb_admitting_envelope, which must
        // consume it AND actually dispatch cbu.confirm successfully.
        let outcome = bridge.execute_step(&step).await;
        assert!(
            matches!(outcome, crate::runbook::executor::StepOutcome::Completed { .. }),
            "expected the first Path A dispatch to admit, consume, and complete: {outcome:?}"
        );

        let status: String =
            sqlx::query_scalar(r#"SELECT status FROM "ob-poc".control_plane_envelopes WHERE envelope_id = $1"#)
                .bind(envelope_id)
                .fetch_one(&pool)
                .await
                .expect("envelope row still exists");
        assert_eq!(status, "consumed", "the real Path A consume must have durably transitioned the row");

        // Second attempt: same compiled step, no re-seal in between.
        // lookup_sealed_handle's own `WHERE status = 'sealed'` filter
        // means it finds nothing for this entry_id now — not a raw
        // resubmission of the consumed handle, but the honest system
        // behaviour Path A actually exhibits.
        let outcome2 = bridge.execute_step(&step).await;
        match outcome2 {
            crate::runbook::executor::StepOutcome::Failed { error } => {
                assert!(
                    error.contains("no sealed envelope was presented"),
                    "expected RejectedNoEnvelope (nothing sealed for this entry anymore), got: {error}"
                );
            }
            other => panic!("expected the bare retry to be rejected, got {other:?}"),
        }

        std::env::remove_var("OB_POC_LIFECYCLE_GATE_MODE");
    }
}
