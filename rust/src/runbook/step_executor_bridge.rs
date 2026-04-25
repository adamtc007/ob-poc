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
        scope: &mut dyn dsl_runtime::tx::TransactionScope,
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
    pub registry: Arc<dsl_core::config::DagRegistry>,
    pub gate_checker: Arc<dsl_runtime::cross_workspace::GateChecker>,
    pub verb_metadata: Arc<dyn VerbTransitionLookup>,
    pub pool: Arc<sqlx::PgPool>,
    /// V1.3-3 cascade planner. Optional — when set, post-dispatch
    /// hook plans + executes single-level cascades (e.g. parent CBU
    /// suspended → all child CBUs suspended).
    pub cascade_planner: Option<Arc<dsl_runtime::cross_workspace::CascadePlanner>>,
}

/// Resolves a verb FQN to its v1.3 `transition_args` metadata. Caller
/// implements this via a HashMap pre-populated from VerbsConfig at
/// startup.
pub trait VerbTransitionLookup: Send + Sync {
    fn lookup(&self, verb_fqn: &str) -> Option<dsl_core::config::types::TransitionArgs>;
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
        }
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
    async fn pre_dispatch_gate_check(
        &self,
        step: &CompiledStep,
    ) -> Result<(), String> {
        let Some(pipe) = &self.gate_pipeline else {
            return Ok(());
        };
        let Some(meta) = pipe.verb_metadata.lookup(&step.verb) else {
            // Verb has no transition_args — no gate check applicable.
            return Ok(());
        };

        // Resolve entity_id from args.
        let entity_str = step.args.get(&meta.entity_id_arg).ok_or_else(|| {
            format!(
                "gate-check: verb '{}' transition_args.entity_id_arg='{}' \
                 not found in step args",
                step.verb, meta.entity_id_arg
            )
        })?;
        let entity_id = uuid::Uuid::parse_str(entity_str).map_err(|e| {
            format!(
                "gate-check: verb '{}' arg '{}' value '{}' is not a UUID: {}",
                step.verb, meta.entity_id_arg, entity_str, e
            )
        })?;

        // Resolve target state from args (optional).
        let target_state_opt: Option<&String> = meta
            .target_state_arg
            .as_ref()
            .and_then(|arg| step.args.get(arg));

        // Resolve target workspace + slot.
        let target_workspace = meta
            .target_workspace
            .as_deref()
            .or_else(|| step.verb.split('.').next())
            .ok_or_else(|| {
                format!("gate-check: cannot infer target_workspace for '{}'", step.verb)
            })?;
        let target_slot = meta.target_slot.as_deref().unwrap_or(target_workspace);

        // Cross-reference DAG transitions for this verb to determine
        // candidate (from, to) pairs. We accept either:
        //   * The transition matching `target_state` if declared
        //   * Any matching from→to pair otherwise (fail-conservative
        //     by checking each)
        let transitions = pipe.registry.transitions_for_verb(&step.verb);
        let candidates: Vec<&dsl_core::config::dag_registry::TransitionRef> = if let Some(ts) = target_state_opt {
            transitions.iter().filter(|t| t.to_state.eq_ignore_ascii_case(ts)).collect()
        } else {
            transitions.iter().collect()
        };

        if candidates.is_empty() {
            // Verb is gate-metadata-aware but DAG has no matching
            // transition. This is expected for verbs that operate on
            // non-state aspects (e.g. updates that don't transition).
            return Ok(());
        }

        for cand in candidates {
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
                    return Err(format!(
                        "v1.3 gate violation [{}]: {}",
                        v.constraint_id, v.message
                    ));
                }
            }
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
                tos.into_iter().next().unwrap().to_string()
            }
        };

        let actions = match planner
            .plan_cascade(target_workspace, target_slot, entity_id, &to_state, &pipe.pool)
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
    action: &dsl_runtime::cross_workspace::CascadeAction,
    pool: &sqlx::PgPool,
) -> anyhow::Result<()> {
    use dsl_runtime::cross_workspace::slot_state::resolve_slot_table;
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

impl CascadeActionExt for dsl_runtime::cross_workspace::CascadeAction {
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

        // Execute through the SemOS port
        let outcome = match self.port.execute_verb(&step.verb, args, &mut ctx).await {
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
    map: std::collections::HashMap<String, dsl_core::config::types::TransitionArgs>,
}

impl HashMapVerbTransitionLookup {
    pub fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
        }
    }

    pub fn from_verbs_config(cfg: &dsl_core::config::types::VerbsConfig) -> Self {
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
    fn lookup(&self, verb_fqn: &str) -> Option<dsl_core::config::types::TransitionArgs> {
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
        use dsl_core::config::types::{
            DomainConfig, TransitionArgs, VerbConfig, VerbBehavior, VerbsConfig,
        };
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
}
