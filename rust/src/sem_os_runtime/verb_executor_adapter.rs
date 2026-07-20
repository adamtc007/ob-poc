//! Adapter implementing `dsl_runtime::VerbExecutionPort` over
//! the existing `DslExecutor` dispatch chain.
//!
//! This is the bridge between SemOS's execution contract and ob-poc's
//! concrete verb execution infrastructure (`SemOsVerbOpRegistry` +
//! GenericCrudExecutor). It translates:
//!
//! - `VerbExecutionContext` ↔ `dsl_v2::ExecutionContext` (30-field)
//! - `serde_json::Value` args → `VerbCall` with `Argument` list
//! - `dsl_v2::ExecutionResult` → `VerbExecutionOutcome`
//! - pending_* side-channel state → `VerbSideEffects.platform_state`

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use dsl_runtime::VerbExecutionPort;
use dsl_runtime::{
    VerbExecutionContext, VerbExecutionOutcome, VerbExecutionResult, VerbSideEffects,
};
use sem_os_core::error::SemOsError;

use crate::dsl_v2::executor::{DslExecutor, ExecutionContext, ExecutionResult};
use dsl_core::{Argument, AstNode, Literal, Span, VerbCall};

/// Adapter implementing the SemOS execution port over ob-poc's DslExecutor.
///
/// Routes verb execution based on contract behavior:
/// - **SemOS-native** → `SemOsVerbOpRegistry` lookup (Phase 5c-migrate Phase A).
///   Takes precedence over all other paths once a verb is registered here.
/// - **CRUD** → `CrudExecutionPort` when available, otherwise DslExecutor fallback.
/// - **Plugin** → DslExecutor (plugin dispatch flows through its own
///   `SemOsVerbOpRegistry` since slice #80).
/// - **GraphQuery/Durable** → DslExecutor.
pub struct ObPocVerbExecutor {
    executor: Arc<DslExecutor>,
    /// Optional SemOS-native CRUD executor. When set, CRUD verbs bypass
    /// the GenericCrudExecutor and route through the SemOS contract.
    /// Set via `with_crud_port()`. None = all verbs go through DslExecutor.
    crud_port: Option<Arc<dyn dsl_runtime::CrudExecutionPort>>,
    /// Optional SemOS-native verb op registry. Populated with re-implemented
    /// plugin ops (YAML-first, living in `sem_os_postgres::ops::*`). When
    /// present and the FQN is registered, the op executes inside a
    /// `PgTransactionScope`; otherwise the dispatcher falls through to the
    /// legacy path so unmigrated verbs keep working during the migration.
    sem_os_ops: Option<Arc<sem_os_postgres::SemOsVerbOpRegistry>>,
}

impl ObPocVerbExecutor {
    pub fn new(executor: Arc<DslExecutor>) -> Self {
        Self {
            executor,
            crud_port: None,
            sem_os_ops: None,
        }
    }

    /// Create an executor from a database pool.
    ///
    /// Constructs the underlying `DslExecutor` without a plugin registry —
    /// suitable for test harnesses. Production callers should use
    /// [`Self::from_pool_with_services`] and then [`Self::with_sem_os_ops`]
    /// so plugin dispatch resolves correctly.
    #[cfg(feature = "database")]
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self {
            executor: Arc::new(DslExecutor::new(pool)),
            crud_port: None,
            sem_os_ops: None,
        }
    }

    /// Create an executor from a database pool with a pre-built service
    /// registry. Prefer this in production — ops relocated to `dsl-runtime`
    /// that depend on platform traits (e.g. `SemanticStateService`) fail
    /// with an actionable error at runtime if their trait is not
    /// registered.
    #[cfg(feature = "database")]
    pub fn from_pool_with_services(
        pool: sqlx::PgPool,
        services: Arc<dsl_runtime::ServiceRegistry>,
    ) -> Self {
        Self {
            executor: Arc::new(DslExecutor::new(pool).with_services(services)),
            crud_port: None,
            sem_os_ops: None,
        }
    }

    /// Attach a SemOS-native CRUD executor.
    ///
    /// When set, CRUD verbs route through `CrudExecutionPort::execute_crud()`
    /// using `VerbContractBody` metadata, bypassing the legacy GenericCrudExecutor.
    pub fn with_crud_port(mut self, port: Arc<dyn dsl_runtime::CrudExecutionPort>) -> Self {
        self.crud_port = Some(port);
        self
    }

    /// Attach a SemOS-native verb op registry.
    ///
    /// When set, verb dispatch consults the registry first. If the FQN is
    /// present, the op runs inside a `PgTransactionScope` (commit on `Ok`,
    /// rollback on `Err`). Absent FQNs fall through to the legacy dispatch
    /// chain (CRUD fast-path, then `DslExecutor`).
    ///
    /// Phase A of the 5c-migrate relocation wires this with an empty
    /// registry; Phase B populates it one verb at a time.
    pub fn with_sem_os_ops(mut self, ops: Arc<sem_os_postgres::SemOsVerbOpRegistry>) -> Self {
        self.sem_os_ops = Some(ops);
        self
    }
}

#[cfg(feature = "database")]
impl ObPocVerbExecutor {
    /// T9.2 (§3, §4) → T10.2 (pin verification): admission-checks
    /// `verb_fqn` against the `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` set
    /// (read fresh from the environment on every call — matches
    /// `LifecycleGateMode::from_env()`'s existing per-call-read pattern,
    /// `dsl_v2/executor.rs`) using the caller's own `&mut PgConnection`
    /// rather than a fresh pool checkout, so the envelope consume and the
    /// verb dispatch it gates are joined to one transaction. Superseded the
    /// pool-based `admit()` (removed — its only caller,
    /// `execute_verb_admitting_envelope`, now opens one scope up front and
    /// calls this instead). A rejection here rolls the whole scope back
    /// (the caller does this), which per the design doc's rollback-retry
    /// corollary correctly leaves the envelope reconsumable rather than
    /// burning it on a rejected attempt.
    ///
    /// T10.2: returns the consumed envelope's recovered `SnapshotPins`
    /// (`None` when not applicable — not enforced, no pins were sealed, or
    /// admission was rejected) so the caller can run
    /// `verify_pins_in_scope` before dispatch — the first real consumer of
    /// `record`, per T10.1's own "T10.2 is the first consumer" note.
    async fn admit_in_scope(
        &self,
        verb_fqn: &str,
        envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
        path: ob_poc_types::ExecutionPath,
        conn: &mut sqlx::PgConnection,
    ) -> dsl_runtime::Result<Option<ob_poc_control_plane::snapshot::SnapshotPins>> {
        // T11.F.2 slice 4: G1's definitional floor, mirroring slice 2's
        // placement in `admit_plan_checked` (Path A/B/C/MCP) — the only
        // other production admission chokepoint. Unconditional, ahead of
        // and independent from `EnforcedVerbs`: an unregistered verb_fqn
        // is rejected before the enforce-mode envelope check even runs,
        // let alone before a scope-held connection touches any table.
        // Same known scope limitation as slice 2: no session id is
        // threaded into this per-verb admission check, so the audit
        // row's session_id is `Uuid::nil()`.
        if !crate::agent::control_plane_floor::g1_verb_is_registered(verb_fqn) {
            let reason = format!("{verb_fqn} is not present in the runtime verb registry");
            let row = crate::agent::control_plane_floor::FloorRejectionRow {
                session_id: Uuid::nil(),
                entry_id: Uuid::new_v4(),
                verb_fqn: verb_fqn.to_string(),
                floor_gate: "G1",
                floor_reason: reason.clone(),
            };
            let pool = self.executor.pool().clone();
            tokio::spawn(async move {
                crate::agent::control_plane_floor::insert_floor_rejection(&pool, &row).await;
            });
            return Err(SemOsError::InvalidInput(format!(
                "T11.F floor rejection [G1]: {reason}"
            )));
        }

        // EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001 §2/§3: captured
        // before `check_admission_in_scope` takes ownership of
        // `envelope_handle` -- the real G10 consume-seam call site
        // (AD-1(a): "G10 grades envelope validity at consume time").
        // `EnvelopeConsumed` is emitted below for any genuine consume
        // attempt (Admitted or RejectedConsumeFailed), same-transaction
        // via `conn` (in-scope, not a detached spawn) -- G10's provenance
        // is `ConsumeSeam` by construction (DD-3).
        //
        // No session id is threaded into this per-verb admission check
        // (same known scope limitation as the G1 floor check above), so
        // the audit row's session_id is `Uuid::nil()`, matching that
        // existing convention.
        let envelope_id_for_audit = envelope_handle.as_ref().map(|h| h.id());

        let enforced = crate::agent::control_plane_envelope_store::EnforcedVerbs::from_env()
            .map_err(|e| {
                SemOsError::Internal(anyhow::anyhow!(
                    "OB_POC_CONTROL_PLANE_ENFORCE_VERBS is unparseable — refusing to guess at \
                     enforcement state: {e}"
                ))
            })?;
        let (decision, pins) = crate::agent::control_plane_envelope_store::check_admission_in_scope(
            conn,
            &enforced,
            verb_fqn,
            path,
            envelope_handle,
        )
        .await
        .map_err(|e| SemOsError::Internal(anyhow::anyhow!("envelope admission check failed: {e}")))?;

        let emit_envelope_consumed = |outcome_kind: &'static str| {
            envelope_id_for_audit.map(|envelope_id| {
                ob_poc_control_plane::audit::AuditEvent::EnvelopeConsumed {
                    envelope_id,
                    gate_outcome: ob_poc_control_plane::audit::GateOutcomeRecord::new(
                        ob_poc_control_plane::gate::GateId::ExecutionEnvelope,
                        outcome_kind,
                    ),
                }
            })
        };

        match decision {
            crate::agent::control_plane_envelope_store::AdmissionDecision::NotEnforced => Ok(pins),
            crate::agent::control_plane_envelope_store::AdmissionDecision::Admitted => {
                if let Some(envelope_id) = envelope_id_for_audit {
                    if let Some(event) = emit_envelope_consumed("Success") {
                        crate::agent::control_plane_audit::insert_audit_event_in_scope(
                            conn,
                            envelope_id,
                            Uuid::nil(),
                            &event,
                        )
                        .await;
                    }
                }
                Ok(pins)
            }
            crate::agent::control_plane_envelope_store::AdmissionDecision::RejectedNoEnvelope => {
                Err(SemOsError::InvalidInput(format!(
                    "{verb_fqn} is enforce-mode gated (OB_POC_CONTROL_PLANE_ENFORCE_VERBS) but no sealed envelope was presented"
                )))
            }
            crate::agent::control_plane_envelope_store::AdmissionDecision::RejectedConsumeFailed(outcome) => {
                if let Some(envelope_id) = envelope_id_for_audit {
                    if let Some(event) = emit_envelope_consumed("Failure") {
                        crate::agent::control_plane_audit::insert_audit_event_in_scope(
                            conn,
                            envelope_id,
                            Uuid::nil(),
                            &event,
                        )
                        .await;
                    }
                }
                Err(SemOsError::InvalidInput(format!(
                    "{verb_fqn} envelope admission rejected: {outcome:?}"
                )))
            }
        }
    }

    /// T9.2 (§2/§3): the scope-threaded dispatch core shared by
    /// `execute_verb_admitting_envelope` — mirrors `execute_verb`'s
    /// 3-branch routing (SemOS-native ops / CRUD fast path / DslExecutor
    /// default) but every branch runs against the ONE caller-supplied
    /// scope instead of opening its own, so admission-check, pin
    /// verification (when wired — see G13's zero-production-caller note
    /// in `ob-poc-control-plane::snapshot`), and the verb's own writes all
    /// commit or roll back together (§2's one-scope-before-branching
    /// principle; closes the check_admission/dispatch TOCTOU window the
    /// architect review flagged as this tranche's BLOCKER).
    async fn execute_verb_in_open_scope(
        &self,
        verb_fqn: &str,
        args: serde_json::Value,
        ctx: &mut VerbExecutionContext,
        path: ob_poc_types::ExecutionPath,
        scope: &mut dyn dsl_runtime::TransactionScope,
    ) -> dsl_runtime::Result<VerbExecutionResult> {
        let (domain, verb) = split_fqn(verb_fqn)?;

        use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};
        let registry = runtime_registry();
        let runtime_verb = registry.get(&domain, &verb);

        let is_crud = runtime_verb
            .as_ref()
            .map(|rv| matches!(rv.behavior, RuntimeBehavior::Crud(_)))
            .unwrap_or(false);

        tracing::debug!(
            verb_fqn,
            has_crud_port = self.crud_port.is_some(),
            has_sem_os_ops = self.sem_os_ops.is_some(),
            "VerbExecutionPort: routing verb (in-scope)"
        );

        // Branch 1: SemOS-native ops — dispatch against the caller's scope
        // directly (no nested begin/commit; the outer scope owns lifecycle).
        if let Some(ref ops) = self.sem_os_ops {
            if let Some(op) = ops.get(verb_fqn) {
                ctx.services = self.executor.service_registry();
                let pool = self.executor.pool();

                let mut args = args;
                if let Some(pre_fetched) = op.pre_fetch(&args, ctx, pool).await.map_err(|e| {
                    SemOsError::Internal(anyhow::anyhow!(
                        "sem_os_ops({}) pre_fetch failed: {}",
                        verb_fqn,
                        e
                    ))
                })? {
                    if let (Some(existing_obj), serde_json::Value::Object(pf_obj)) =
                        (args.as_object_mut(), pre_fetched)
                    {
                        for (k, v) in pf_obj {
                            existing_obj.insert(k, v);
                        }
                    }
                }

                let pre_symbols = ctx.symbols.clone();
                let pre_symbol_types = ctx.symbol_types.clone();

                let outcome = op.execute(&args, ctx, scope).await.map_err(|e| {
                    SemOsError::Internal(anyhow::anyhow!("sem_os_ops({}) failed: {}", verb_fqn, e))
                })?;

                let mut new_bindings = std::collections::HashMap::new();
                let mut new_binding_types = std::collections::HashMap::new();
                for (name, uuid) in &ctx.symbols {
                    if pre_symbols.get(name) != Some(uuid) {
                        new_bindings.insert(name.clone(), *uuid);
                    }
                }
                for (name, ty) in &ctx.symbol_types {
                    if pre_symbol_types.get(name) != Some(ty) {
                        new_binding_types.insert(name.clone(), ty.clone());
                    }
                }

                return Ok(VerbExecutionResult {
                    outcome,
                    side_effects: VerbSideEffects {
                        new_bindings,
                        new_binding_types,
                        platform_state: serde_json::Value::Null,
                    },
                    ..Default::default()
                });
            }
        }

        // Branch 2: CRUD fast path — execute_crud_in_scope (T9.2 §3), no
        // pool-based fallback (CrudExecutionPort::execute_crud_in_scope has
        // no default impl per OQ2).
        if is_crud {
            if let Some(ref crud_port) = self.crud_port {
                if let Some(rv) = runtime_verb.as_ref() {
                    let contract = runtime_verb_to_contract(rv);
                    match crud_port
                        .execute_crud_in_scope(&contract, args.clone(), ctx, scope)
                        .await
                    {
                        Ok(outcome) => {
                            return Ok(VerbExecutionResult::from_outcome(outcome));
                        }
                        Err(SemOsError::InvalidInput(msg)) if msg.contains("not yet migrated") => {
                            tracing::debug!(verb_fqn, "CRUD port (in-scope): falling through to DslExecutor");
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        // Branch 3: default path — DslExecutor dispatch chain via the
        // scope-accepting sibling (T9.2 §3 Branch 3: trivial swap).
        //
        // G3 §3(e) (double-admission guard): this fallthrough reaches the
        // SAME dsl_v2 seam (`execute_verb_in_scope`) that G4 instruments
        // directly for Path B/C. Tag the converted context with the SAME
        // `path` this outer call was already admitted under, and record
        // that proof (`already_admitted_for`) so the seam's own admission
        // check recognises this dispatch already cleared `EnforcedVerbs`
        // and skips re-checking it — not a distinct "fallthrough" tag,
        // per the design doc's own reasoning (a distinct tag would
        // reintroduce, one layer down, the exact asymmetry AD-2(b) fixes
        // one layer up).
        let vc = build_verb_call(&domain, &verb, &args);
        let mut exec_ctx = to_dsl_context(ctx);
        exec_ctx.execution_path = path;
        exec_ctx.already_admitted_for = Some(path);

        let result = self
            .executor
            .execute_verb_in_scope(&vc, &mut exec_ctx, scope)
            .await
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("Verb execution failed: {e}")))?;

        let side_effects = collect_side_effects(ctx, &exec_ctx);

        for (name, uuid) in &side_effects.new_bindings {
            ctx.symbols.insert(name.clone(), *uuid);
        }
        for (name, entity_type) in &side_effects.new_binding_types {
            ctx.symbol_types.insert(name.clone(), entity_type.clone());
        }

        let outcome = to_verb_outcome(&result);

        Ok(VerbExecutionResult {
            outcome,
            side_effects,
            ..Default::default()
        })
    }

    /// G6a (`EOP-DESIGN-CONTROLPLANE-G6A-SNAPSHOT-PIN-CARRIER-001` §4):
    /// Path D's own admission-time envelope minting. Bus-federated
    /// callers (bpmn-lite) cannot mint a real `ExecutionEnvelope`
    /// themselves — they hold none of the proof-bearing gate inputs
    /// `ExecutionEnvelope::seal` requires (no `SemReg` pack registry, no
    /// compiled runbook object, no entity-lifecycle-state reader, no
    /// authority/evidence store; see the design doc's §1). So unlike
    /// [`execute_verb_admitting_envelope`](Self::execute_verb_admitting_envelope),
    /// which only *consumes* an already-sealed handle (Path A's shape),
    /// `ob-poc` is the only party in this exchange that can supply the
    /// proof inputs, so it is the only party that can seal — this method
    /// mints and persists (`status = 'sealed'`).
    ///
    /// **This method does NOT consume.** It deliberately mints/persists
    /// only; the returned handle is threaded by the caller into the
    /// ordinary `execute_verb_admitting_envelope` call, whose own
    /// `admit_in_scope` → `check_admission_in_scope` →
    /// `try_consume_in_scope_with_pins` chain performs the actual
    /// consume, inside the same transaction scope as the dispatch it
    /// gates. This preserves T9.2's rollback-together atomicity — a
    /// dispatch failure after admission rolls the consume back too, so
    /// the envelope is reconsumable, not permanently burned on a
    /// transient failure. An earlier draft of this method consumed
    /// inline; a live-DB test written against that draft
    /// (`enforced_verb_with_full_context_mints_but_does_not_consume`,
    /// found the row still `'sealed'` when the test expected `'consumed'`)
    /// is what caught that the consume-inline shape would have run the
    /// consume in a separate transaction from the dispatch, breaking that
    /// atomicity property for Path D relative to Path A — see the design
    /// doc's §4 correction note.
    ///
    /// Short-circuits to `None` whenever `verb_fqn` isn't enforce-gated
    /// for [`ob_poc_types::ExecutionPath::BusFederated`] — zero added
    /// cost (no `evaluate()` call, no DB write) on the production-default
    /// (nothing enforced) path, matching `check_admission`'s own
    /// early-return shape.
    ///
    /// `bus_pin` is the bare `Uuid` `plan_walker.rs::dispatch_callout`
    /// (bpmn-lite) populates onto the wire's `InvocationRequest.
    /// snapshot_pin` — used here purely as [`persist_sealed`](
    /// crate::agent::control_plane_envelope_store::persist_sealed)'s
    /// `entry_id` audit-correlation column, **never** as a foreign-minted
    /// handle's identity. This method never calls `try_consume`/
    /// `try_consume_by_id`/`try_consume_in_scope` at all — the content
    /// hash the LATER consume (in the caller's own admission call)
    /// checks against is always the one `envelope.handle()` computes
    /// locally from the envelope this method sealed, never anything
    /// bpmn-lite sent. See the design doc's §3 for why this does not
    /// reopen T8.1's closed id-only-consume gap.
    ///
    /// **Known limitation, disclosed not hidden (design doc §6/§7):**
    /// `ob_poc_control_plane::decision::evaluate`'s `PROOF_BEARING_GATES`
    /// check requires `GateId::PackResolution` and `GateId::RunbookProof`
    /// to succeed, and `applicability()` already confirms both are
    /// structurally inapplicable to bus dispatch (no `SemReg` pack, no
    /// compiled runbook object). Until `evaluate`/`evaluate_with_report`
    /// becomes path-aware (a separate, reviewed change — not this one;
    /// see the design doc's §7), this method's real-evaluation call can
    /// never actually reach `ApprovedStp` for a genuinely Path-D-shaped
    /// `cp_ctx` — an enforced bus verb is always rejected, honestly and
    /// fail-closed, not silently bypassed.
    pub async fn mint_envelope_for_bus(
        &self,
        verb_fqn: &str,
        cp_ctx: &ob_poc_control_plane::context::EvaluationContext,
        bus_pin: Option<Uuid>,
    ) -> Option<ob_poc_types::EnvelopeHandle> {
        let enforced =
            crate::agent::control_plane_envelope_store::EnforcedVerbs::from_env().ok()?;
        if !enforced.is_enforced(verb_fqn, ob_poc_types::ExecutionPath::BusFederated) {
            return None;
        }

        let now = chrono::Utc::now();
        let validity = ob_poc_control_plane::envelope::ValidityWindow::new(
            now,
            now + chrono::Duration::minutes(5),
        );
        let decision = ob_poc_control_plane::decision::evaluate(cp_ctx, validity);
        let ob_poc_control_plane::decision::ControlPlaneDecision::ApprovedStp(envelope) = decision
        else {
            // Rejected / RequiresHumanGate -> no envelope. Path D has no
            // human-gate UX today, so `RequiresHumanGate` degrades to the
            // same "no envelope" outcome as `Rejected` here — a future
            // Path D human-gate flow is out of this method's scope.
            return None;
        };

        let entry_id = bus_pin.unwrap_or_else(Uuid::new_v4);
        let pool = self.executor.pool();
        let persisted = crate::agent::control_plane_envelope_store::persist_sealed(
            pool,
            Uuid::nil(),
            entry_id,
            verb_fqn,
            &envelope,
        )
        .await;
        if !persisted {
            // Best-effort persist failed — handing out an id that can't
            // be found at consume time would just surface as `NotFound`
            // anyway; short-circuiting here is honest, not a behaviour
            // change (see `persist_sealed`'s own best-effort doc comment).
            return None;
        }
        Some(envelope.handle())
    }
}

#[cfg(feature = "database")]
#[async_trait]
impl VerbExecutionPort for ObPocVerbExecutor {
    async fn execute_verb(
        &self,
        verb_fqn: &str,
        args: serde_json::Value,
        ctx: &mut VerbExecutionContext,
    ) -> dsl_runtime::Result<VerbExecutionResult> {
        // 1. Split FQN into domain.verb
        let (domain, verb) = split_fqn(verb_fqn)?;

        // 2. Resolve behavior from RuntimeVerbRegistry (contract-aware routing)
        use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};
        let registry = runtime_registry();
        let runtime_verb = registry.get(&domain, &verb);

        let is_crud = runtime_verb
            .as_ref()
            .map(|rv| matches!(rv.behavior, RuntimeBehavior::Crud(_)))
            .unwrap_or(false);

        let behavior_label = match runtime_verb.as_ref().map(|rv| &rv.behavior) {
            Some(RuntimeBehavior::Crud(_)) => "crud",
            Some(RuntimeBehavior::Plugin(_)) => "plugin",
            Some(RuntimeBehavior::GraphQuery(_)) => "graph_query",
            Some(RuntimeBehavior::Durable(_)) => "durable",
            None => "unknown",
        };
        tracing::debug!(
            verb_fqn,
            behavior = behavior_label,
            has_crud_port = self.crud_port.is_some(),
            has_sem_os_ops = self.sem_os_ops.is_some(),
            "VerbExecutionPort: routing verb"
        );

        // 2.5. SemOS-native fast path — canonical plugin dispatch post-slice-#80.
        //      If the verb FQN is registered in `SemOsVerbOpRegistry`, open a
        //      `PgTransactionScope`, invoke the op, commit on Ok / rollback on Err.
        //      Unregistered FQNs fall through to the DslExecutor which has its
        //      own plugin branch (also backed by the same registry) for recursive
        //      DSL execution from template ops.
        if let Some(ref ops) = self.sem_os_ops {
            if let Some(op) = ops.get(verb_fqn) {
                use crate::sequencer_tx::PgTransactionScope;
                use dsl_runtime::TransactionScope;

                ctx.services = self.executor.service_registry();

                let pool = self.executor.pool();

                let mut args = args;
                if let Some(pre_fetched) = op.pre_fetch(&args, ctx, pool).await.map_err(|e| {
                    SemOsError::Internal(anyhow::anyhow!(
                        "sem_os_ops({}) pre_fetch failed: {}",
                        verb_fqn,
                        e
                    ))
                })? {
                    if let (Some(existing_obj), serde_json::Value::Object(pf_obj)) =
                        (args.as_object_mut(), pre_fetched)
                    {
                        for (k, v) in pf_obj {
                            existing_obj.insert(k, v);
                        }
                    }
                }

                let mut scope = PgTransactionScope::begin(pool).await.map_err(|e| {
                    SemOsError::Internal(anyhow::anyhow!(
                        "sem_os_ops({}): begin txn failed: {}",
                        verb_fqn,
                        e
                    ))
                })?;

                let pre_symbols = ctx.symbols.clone();
                let pre_symbol_types = ctx.symbol_types.clone();

                let exec_result: Result<VerbExecutionOutcome, anyhow::Error> = {
                    let scope_dyn: &mut dyn TransactionScope = &mut scope;
                    op.execute(&args, ctx, scope_dyn).await
                };

                match exec_result {
                    Ok(outcome) => {
                        scope.commit().await.map_err(|e| {
                            SemOsError::Internal(anyhow::anyhow!(
                                "sem_os_ops({}): commit failed: {}",
                                verb_fqn,
                                e
                            ))
                        })?;

                        let mut new_bindings = std::collections::HashMap::new();
                        let mut new_binding_types = std::collections::HashMap::new();
                        for (name, uuid) in &ctx.symbols {
                            if pre_symbols.get(name) != Some(uuid) {
                                new_bindings.insert(name.clone(), *uuid);
                            }
                        }
                        for (name, ty) in &ctx.symbol_types {
                            if pre_symbol_types.get(name) != Some(ty) {
                                new_binding_types.insert(name.clone(), ty.clone());
                            }
                        }

                        return Ok(VerbExecutionResult {
                            outcome,
                            side_effects: VerbSideEffects {
                                new_bindings,
                                new_binding_types,
                                platform_state: serde_json::Value::Null,
                            },
                            ..Default::default()
                        });
                    }
                    Err(e) => {
                        if let Err(rollback_err) = scope.rollback().await {
                            tracing::warn!(
                                verb_fqn,
                                %rollback_err,
                                "sem_os_ops rollback failed after op error"
                            );
                        }
                        return Err(SemOsError::Internal(anyhow::anyhow!(
                            "sem_os_ops({}) failed: {}",
                            verb_fqn,
                            e
                        )));
                    }
                }
            }
        }

        // 3. CRUD fast path — route through CrudExecutionPort when available
        if is_crud {
            if let Some(ref crud_port) = self.crud_port {
                if let Some(rv) = runtime_verb.as_ref() {
                    let contract = runtime_verb_to_contract(rv);
                    match crud_port.execute_crud(&contract, args.clone(), ctx).await {
                        Ok(outcome) => {
                            return Ok(VerbExecutionResult::from_outcome(outcome));
                        }
                        Err(SemOsError::InvalidInput(msg)) if msg.contains("not yet migrated") => {
                            // Fall through to DslExecutor for unmigrated operations
                            tracing::debug!(verb_fqn, "CRUD port: falling through to DslExecutor");
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        // 4. Default path — DslExecutor dispatch chain (plugin, graph_query, durable,
        //    or CRUD without crud_port / unmigrated operations)
        let vc = build_verb_call(&domain, &verb, &args);
        let mut exec_ctx = to_dsl_context(ctx);

        let result = self
            .executor
            .execute_verb(&vc, &mut exec_ctx)
            .await
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("Verb execution failed: {e}")))?;

        // 5. Collect side effects (new bindings + platform state)
        let side_effects = collect_side_effects(ctx, &exec_ctx);

        // 6. Propagate new bindings back to SemOS context
        for (name, uuid) in &side_effects.new_bindings {
            ctx.symbols.insert(name.clone(), *uuid);
        }
        for (name, entity_type) in &side_effects.new_binding_types {
            ctx.symbol_types.insert(name.clone(), entity_type.clone());
        }

        // 7. Convert result
        let outcome = to_verb_outcome(&result);

        Ok(VerbExecutionResult {
            outcome,
            side_effects,
            ..Default::default()
        })
    }

    /// T4.1 → T9.2: admission-checks `verb_fqn` and dispatches it inside
    /// ONE `PgTransactionScope` (§2's one-scope-before-branching
    /// principle) — the admission check (envelope consume), and the
    /// verb's own dispatch/writes, commit or roll back together. Before
    /// T9.2 these ran as two independent transactions (`admit()` against a
    /// fresh pool checkout, then `execute_verb` opening its own scope per
    /// branch), leaving a real TOCTOU window between "envelope admitted"
    /// and "verb executed" under READ COMMITTED. With the production-default
    /// empty `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`, admission is always
    /// `NotEnforced` and this path is behaviourally unchanged for
    /// everything except: CRUD verbs now execute atomically with the rest
    /// of the scope instead of autocommitting per statement (§6, an
    /// intentional correctness improvement, not a no-op — see the design
    /// doc's reframe of the original "behaviorally invisible" claim).
    async fn execute_verb_admitting_envelope(
        &self,
        verb_fqn: &str,
        args: serde_json::Value,
        ctx: &mut VerbExecutionContext,
        envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
        path: ob_poc_types::ExecutionPath,
    ) -> dsl_runtime::Result<VerbExecutionResult> {
        use crate::sequencer_tx::PgTransactionScope;
        use dsl_runtime::TransactionScope;

        let pool = self.executor.pool();
        let mut scope = PgTransactionScope::begin(pool).await.map_err(|e| {
            SemOsError::Internal(anyhow::anyhow!(
                "execute_verb_admitting_envelope({verb_fqn}): begin txn failed: {e}"
            ))
        })?;

        // EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001 §2: captured
        // before `admit_in_scope` takes ownership of `envelope_handle`, so
        // a `DispatchCommitted` audit event (G14, provenance
        // `PostDispatch`) can correlate to the same `decision_id` as this
        // dispatch's `EnvelopeConsumed` event, below.
        let envelope_id_for_commit_audit = envelope_handle.as_ref().map(|h| h.id());

        let pins = match self
            .admit_in_scope(verb_fqn, envelope_handle, path, scope.executor())
            .await
        {
            Ok(pins) => pins,
            Err(e) => {
                if let Err(rollback_err) = scope.rollback().await {
                    tracing::warn!(
                        verb_fqn,
                        %rollback_err,
                        "execute_verb_admitting_envelope: rollback failed after admission rejection"
                    );
                }
                return Err(e);
            }
        };

        // T10.2: pin verification, inside the same scope, before dispatch —
        // the locked re-read (`FOR UPDATE`, §5a) holds pinned entity rows
        // until this scope's own commit/rollback, so nothing can move them
        // between this check and the write that follows. `pins` is `None`
        // for anything not admitted-with-pins (not enforced, no snapshot
        // pins sealed, pre-T10.1 row) — matching `verify_pins_in_scope`'s
        // own "empty pins never drift" posture, not skipped as a special case.
        if let Some(pins) = &pins {
            if let Err(e) = ob_poc_boundary::toctou_recheck::verify_pins_in_scope(pins, scope.executor()).await {
                if let Err(rollback_err) = scope.rollback().await {
                    tracing::warn!(
                        verb_fqn,
                        %rollback_err,
                        "execute_verb_admitting_envelope: rollback failed after pin verification failure"
                    );
                }
                return Err(SemOsError::InvalidInput(format!(
                    "{verb_fqn} rejected: pinned entity state drifted since gating ({e})"
                )));
            }
        }

        let scope_dyn: &mut dyn dsl_runtime::TransactionScope = &mut scope;
        match self
            .execute_verb_in_open_scope(verb_fqn, args, ctx, path, scope_dyn)
            .await
        {
            Ok(result) => {
                // G2 item 2 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3, G2
                // item 2 / EOP-SESSION-CONTROLPLANE-G1-ITEM2-G2-ITEM2-IMPL-001):
                // this call site now goes through the REAL `commit_attested`
                // (not plain `commit()`), closing half of item 2's "wire
                // set_expected_write_set + commit_attested into the
                // sequencer's commit path" instruction — but
                // `set_expected_write_set` is deliberately NOT called here.
                //
                // STOP-condition finding (the plan's own named production-
                // behavior-change guard): the only existing production
                // source of a `WriteSetProof` for a verb's *declared*
                // footprint, `agent::control_plane_shadow::
                // build_write_set_input` (used today for G7's shadow
                // evaluation, the same helper the plan's own item-2 text
                // points at), always sets `allowed_columns: Vec::new()` —
                // it has no per-column knowledge, only
                // `domain_metadata.yaml`'s per-verb `writes: [table, ...]`
                // list. `write_set_attestation::attest`'s column check is
                // `write.columns.iter().all(|c| expected.allowed_columns()
                // .contains(c))` — with an empty `allowed_columns`, this is
                // `false` for ANY write reporting a nonempty column list,
                // regardless of table/entity match. `crud_executor.rs`'s
                // real `record_write` calls (T10.3, the scope-based CRUD
                // dispatch this function's Branch 2/3 actually use) always
                // report real, nonempty columns for a genuine INSERT/
                // UPDATE. Wiring `set_expected_write_set` from
                // `build_write_set_input`'s output here would therefore
                // misclassify EVERY real, legitimate CRUD write as a
                // breach and roll it back for any verb with a declared
                // write footprint — not "a real excess write gets caught,"
                // but "every write gets rejected." Proven empirically:
                // `ob-poc-control-plane::write_set_attestation::tests::
                // empty_allowed_columns_breaches_every_write_with_any_column_even_on_exact_table_and_entity_match`.
                // Per the plan's own instruction ("if implementation finds
                // any verb's behavior changing, stop and flag for
                // architect review even with green tests"), this is NOT
                // wired. `commit_attested(None, Some(verb_fqn))` with no
                // `expected_write_set` attached is provably behaviour-
                // identical to plain `commit()` (`PgTransactionScope::
                // commit_attested`'s own early-return: `let Some(expected)
                // = self.expected_write_set.clone() else { self.tx.commit
                // ().await...; return Ok(()); }` — the `attest` comparison
                // never runs, so `Breach` is structurally unreachable from
                // this call site). This closes the transport half (the
                // real function is now called, real per-commit
                // attestation-store bookkeeping exists as a mechanism) and
                // leaves the actual bound-comparison half open pending a
                // correctly column-aware `WriteSetProof` derivation — a
                // separate, reviewed follow-up, not silently forced
                // through here.
                //
                // `DispatchCommitted` (G14, provenance `PostDispatch`)
                // still records the honest degraded signal: `attested:
                // false` (no compare-and-attest genuinely ran — nothing
                // was compared, matching the STOP-condition finding above)
                // and a `NotEvaluated` gate_outcome, unchanged from the
                // prior session's V3 finding. Only emitted when a real
                // envelope was actually in play (`envelope_id_for_commit_audit`
                // is `Some`) -- a plain CRUD/legacy commit with no
                // envelope has no `decision_id` to correlate to and is out
                // of this stream's scope by construction (§2: the stream
                // is a per-decision lifecycle record, not a per-commit log).
                if let Some(envelope_id) = envelope_id_for_commit_audit {
                    let event = ob_poc_control_plane::audit::AuditEvent::DispatchCommitted {
                        attested: false,
                        gate_outcome: ob_poc_control_plane::audit::GateOutcomeRecord::new(
                            ob_poc_control_plane::gate::GateId::WriteSetAttestation,
                            "NotEvaluated",
                        ),
                    };
                    crate::agent::control_plane_audit::insert_audit_event_in_scope(
                        scope.executor(),
                        envelope_id,
                        Uuid::nil(),
                        &event,
                    )
                    .await;
                }
                scope
                    .commit_attested(None, Some(verb_fqn))
                    .await
                    .map_err(|e| {
                        SemOsError::Internal(anyhow::anyhow!(
                            "execute_verb_admitting_envelope({verb_fqn}): commit failed: {e}"
                        ))
                    })?;
                Ok(result)
            }
            Err(e) => {
                if let Err(rollback_err) = scope.rollback().await {
                    tracing::warn!(
                        verb_fqn,
                        %rollback_err,
                        "execute_verb_admitting_envelope: rollback failed after dispatch error"
                    );
                }
                Err(e)
            }
        }
    }
}

// ── Conversion helpers ──────────────────────────────────────────

/// Convert a RuntimeVerb to a minimal VerbContractBody for CRUD execution.
/// Only populates the fields needed by CrudExecutionPort.
fn runtime_verb_to_contract(
    rv: &crate::dsl_v2::runtime_registry::RuntimeVerb,
) -> sem_os_ontology::verb_contract::VerbContractBody {
    use crate::dsl_v2::runtime_registry::RuntimeBehavior;
    use sem_os_ontology::verb_contract::{
        VerbArgDef, VerbContractBody, VerbCrudMapping, VerbReturnSpec,
    };

    let crud_mapping = if let RuntimeBehavior::Crud(ref crud) = rv.behavior {
        Some(VerbCrudMapping {
            operation: format!("{:?}", crud.operation).to_lowercase(),
            table: Some(crud.table.clone()),
            schema: Some(crud.schema.clone()),
            key_column: crud.key.clone(),
            returning: crud.returning.clone(),
            conflict_keys: crud.conflict_keys.clone(),
            conflict_constraint: crud.conflict_constraint.clone(),
            junction: crud.junction.clone(),
            from_col: crud.from_col.clone(),
            to_col: crud.to_col.clone(),
            role_table: crud.role_table.clone(),
            role_col: crud.role_col.clone(),
            fk_col: crud.fk_col.clone(),
            filter_col: crud.filter_col.clone(),
            primary_table: crud.primary_table.clone(),
            join_table: crud.join_table.clone(),
            join_col: crud.join_col.clone(),
            set_values: set_values_yaml_to_json(crud.set_values.as_ref()),
        })
    } else {
        None
    };

    let args: Vec<VerbArgDef> = rv
        .args
        .iter()
        .map(|a| VerbArgDef {
            name: a.name.clone(),
            arg_type: format!("{:?}", a.arg_type).to_lowercase(),
            required: a.required,
            description: a.description.clone(),
            lookup: None, // Lookups resolved before reaching CrudExecutionPort
            valid_values: a.valid_values.clone(),
            default: None,
            maps_to: a.maps_to.clone(),
        })
        .collect();

    let returns = Some(VerbReturnSpec {
        return_type: format!("{:?}", rv.returns.return_type).to_lowercase(),
        schema: None,
    });

    VerbContractBody {
        fqn: rv.full_name.clone(),
        domain: rv.domain.clone(),
        action: rv.verb.clone(),
        description: rv.description.clone(),
        behavior: "crud".to_string(),
        args,
        returns,
        crud_mapping,
        preconditions: vec![],
        postconditions: vec![],
        produces: None,
        consumes: vec![],
        invocation_phrases: vec![],
        subject_kinds: rv.subject_kinds.clone(),
        phase_tags: vec![],
        harm_class: rv.harm_class.map(|h| match h {
            dsl_core::HarmClass::ReadOnly => sem_os_ontology::verb_contract::HarmClass::ReadOnly,
            dsl_core::HarmClass::Reversible => {
                sem_os_ontology::verb_contract::HarmClass::Reversible
            }
            dsl_core::HarmClass::Irreversible => {
                sem_os_ontology::verb_contract::HarmClass::Irreversible
            }
            dsl_core::HarmClass::Destructive => {
                sem_os_ontology::verb_contract::HarmClass::Destructive
            }
        }),
        action_class: None,
        precondition_states: vec![],
        requires_subject: true,
        produces_focus: false,
        metadata: None,
        reads_from: vec![],
        writes_to: vec![],
        outputs: vec![],
        produces_shared_facts: vec![],
    }
}

/// `RuntimeCrudConfig.set_values` is `HashMap<String, serde_yaml::Value>`
/// (the YAML-loader's native representation) but `VerbCrudMapping.set_values`
/// (the execution-plane contract `dsl-runtime::crud_executor` reads) is
/// `HashMap<String, serde_json::Value>` — this crate's boundary already
/// speaks JSON everywhere else (`VerbArgDef.default`, `json_to_sql_value`).
/// `serde_yaml::Value` implements `Serialize`, so a round-trip through
/// `serde_json::to_value` is exact for the scalar types `set_values`
/// actually carries (string/bool/integer — the only variants
/// `execute_update`/`generic_executor.rs` ever read). A conversion failure
/// (never observed for these scalar types, but not provably impossible)
/// drops that one entry rather than the whole map or panicking — an
/// honest under-report, not a silent guess.
fn set_values_yaml_to_json(
    set_values: Option<&std::collections::HashMap<String, serde_yaml::Value>>,
) -> Option<std::collections::HashMap<String, serde_json::Value>> {
    set_values.map(|sv| {
        sv.iter()
            .filter_map(|(k, v)| serde_json::to_value(v).ok().map(|jv| (k.clone(), jv)))
            .collect()
    })
}

fn split_fqn(fqn: &str) -> dsl_runtime::Result<(String, String)> {
    let parts: Vec<&str> = fqn.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(SemOsError::InvalidInput(format!(
            "Invalid verb FQN '{}': expected 'domain.verb'",
            fqn
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Public wrapper for use by the dsl_v2::executor compatibility shim.
pub(crate) fn build_verb_call_pub(domain: &str, verb: &str, args: &serde_json::Value) -> VerbCall {
    build_verb_call(domain, verb, args)
}

/// Public wrapper for use by the compatibility shim.
pub(crate) fn to_dsl_context_pub(ctx: &VerbExecutionContext) -> ExecutionContext {
    to_dsl_context(ctx)
}

/// Public wrapper for use by the compatibility shim.
pub(crate) fn to_verb_outcome_pub(result: &ExecutionResult) -> VerbExecutionOutcome {
    to_verb_outcome(result)
}

/// Unpack `VerbExecutionContext.extensions` side-channel keys back into an
/// `ExecutionContext`'s `pending_*` fields. Called by `dsl_v2::executor`
/// after dispatch to propagate session/view/agent mutations the op staged
/// on its `VerbExecutionContext`.
pub(crate) fn apply_sem_ctx_extensions_to_exec_ctx(
    sem_ctx: &VerbExecutionContext,
    exec_ctx: &mut ExecutionContext,
) {
    let obj = match sem_ctx.extensions.as_object() {
        Some(m) => m,
        None => return,
    };

    macro_rules! unpack_opt {
        ($field:ident) => {
            if let Some(v) = obj.get(stringify!($field)) {
                if !v.is_null() {
                    if let Ok(parsed) = serde_json::from_value(v.clone()) {
                        exec_ctx.$field = Some(parsed);
                    }
                }
            }
        };
    }
    macro_rules! unpack_opt_opt {
        ($field:ident) => {
            if let Some(v) = obj.get(stringify!($field)) {
                if let Some(is_set) = v.get("set").and_then(|b| b.as_bool()) {
                    if is_set {
                        let inner = v.get("value").cloned().unwrap_or(serde_json::Value::Null);
                        if inner.is_null() {
                            exec_ctx.$field = Some(None);
                        } else if let Ok(parsed) = serde_json::from_value(inner) {
                            exec_ctx.$field = Some(Some(parsed));
                        }
                    }
                }
            }
        };
    }

    unpack_opt!(pending_view_state);
    unpack_opt!(pending_viewport_state);
    unpack_opt!(pending_scope_change);
    unpack_opt!(pending_session);
    unpack_opt!(pending_session_name);
    unpack_opt!(pending_structure_id);
    unpack_opt!(pending_structure_name);
    unpack_opt!(pending_case_id);
    unpack_opt!(pending_mandate_id);
    unpack_opt_opt!(pending_deal_id);
    unpack_opt_opt!(pending_deal_name);

    if let Some(flags) = obj.get("pending_dag_flags") {
        if let Ok(parsed) = serde_json::from_value::<Vec<(String, bool)>>(flags.clone()) {
            exec_ctx.pending_dag_flags = parsed;
        }
    }
    if let Some(dirty) = obj.get("cbu_scope_dirty").and_then(|v| v.as_bool()) {
        exec_ctx.cbu_scope_dirty = dirty;
    }
    if let Some(agent_ctrl) = obj.get("pending_agent_control") {
        exec_ctx
            .json_bindings
            .insert("_agent_control".to_string(), agent_ctrl.clone());
    }
}

/// Convert a `VerbCall`'s argument list back into a JSON object — inverse
/// of [`build_verb_call`]. Used by `dsl_v2::executor` when invoking ops
/// whose execute path takes a JSON args object.
pub(crate) fn verb_call_to_json(vc: &VerbCall) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for arg in &vc.arguments {
        map.insert(arg.key.clone(), ast_node_to_json_value(&arg.value));
    }
    serde_json::Value::Object(map)
}

fn ast_node_to_json_value(node: &AstNode) -> serde_json::Value {
    match node {
        AstNode::Literal(Literal::String(s), _) => serde_json::Value::String(s.clone()),
        AstNode::Literal(Literal::Integer(i), _) => serde_json::Value::Number((*i).into()),
        AstNode::Literal(Literal::Decimal(d), _) => serde_json::Value::String(d.to_string()),
        AstNode::Literal(Literal::Boolean(b), _) => serde_json::Value::Bool(*b),
        AstNode::Literal(Literal::Uuid(u), _) => serde_json::Value::String(u.to_string()),
        AstNode::Literal(Literal::Null, _) => serde_json::Value::Null,
        AstNode::SymbolRef { name, .. } => {
            // Preserve symbol reference syntax so downstream consumers can
            // detect `@foo` by prefix.
            serde_json::Value::String(format!("@{}", name))
        }
        AstNode::EntityRef {
            resolved_key,
            value,
            ..
        } => serde_json::Value::String(resolved_key.clone().unwrap_or_else(|| value.clone())),
        AstNode::List { items, .. } => {
            serde_json::Value::Array(items.iter().map(ast_node_to_json_value).collect::<Vec<_>>())
        }
        AstNode::Map { entries, .. } => {
            let mut map = serde_json::Map::new();
            for (key, value) in entries {
                map.insert(key.clone(), ast_node_to_json_value(value));
            }
            serde_json::Value::Object(map)
        }
        // Nested calls are not valid primitive plugin arguments at this stage.
        // Preserve the legacy lossy fallback so existing error paths stay stable.
        AstNode::Nested(vc) => serde_json::Value::String(format!("{vc:?}")),
    }
}

/// Convert a `VerbExecutionOutcome` back into a legacy `ExecutionResult`.
/// Inverse of [`to_verb_outcome`].
pub(crate) fn from_verb_outcome(outcome: VerbExecutionOutcome) -> ExecutionResult {
    match outcome {
        VerbExecutionOutcome::Uuid(u) => ExecutionResult::Uuid(u),
        VerbExecutionOutcome::Record(v) => ExecutionResult::Record(v),
        VerbExecutionOutcome::RecordSet(v) => ExecutionResult::RecordSet(v),
        VerbExecutionOutcome::Affected(n) => ExecutionResult::Affected(n),
        VerbExecutionOutcome::Void => ExecutionResult::Void,
    }
}

fn build_verb_call(domain: &str, verb: &str, args: &serde_json::Value) -> VerbCall {
    let arguments = match args.as_object() {
        Some(map) => map
            .iter()
            .map(|(key, value)| Argument {
                key: key.clone(),
                value: json_value_to_ast_node(value),
                span: Span::default(),
            })
            .collect(),
        None => vec![],
    };

    VerbCall {
        domain: domain.to_string(),
        verb: verb.to_string(),
        arguments,
        lens_override: None,
        binding: None,
        span: Span::default(),
    }
}

fn json_value_to_ast_node(value: &serde_json::Value) -> AstNode {
    let span = Span::default();
    match value {
        serde_json::Value::String(s) => {
            // Check if it's a UUID
            if let Ok(uuid) = uuid::Uuid::parse_str(s) {
                AstNode::Literal(Literal::Uuid(uuid), span)
            } else {
                AstNode::Literal(Literal::String(s.clone()), span)
            }
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                AstNode::Literal(Literal::Integer(i), span)
            } else {
                AstNode::Literal(Literal::String(n.to_string()), span)
            }
        }
        serde_json::Value::Bool(b) => AstNode::Literal(Literal::Boolean(*b), span),
        serde_json::Value::Null => AstNode::Literal(Literal::Null, span),
        // Arrays and objects: serialize as string (verb handlers parse as needed)
        other => AstNode::Literal(Literal::String(other.to_string()), span),
    }
}

fn to_dsl_context(ctx: &VerbExecutionContext) -> ExecutionContext {
    let mut exec_ctx = ExecutionContext {
        symbols: ctx.symbols.clone(),
        symbol_types: ctx.symbol_types.clone(),
        execution_id: ctx.execution_id,
        ..Default::default()
    };

    // Unpack platform extensions if present
    if let Some(obj) = ctx.extensions.as_object() {
        if let Some(audit_user) = obj.get("audit_user").and_then(|v| v.as_str()) {
            exec_ctx.audit_user = Some(audit_user.to_string());
        }
        if let Some(session_id) = obj.get("session_id").and_then(|v| v.as_str()) {
            if let Ok(uuid) = Uuid::parse_str(session_id) {
                exec_ctx.session_id = Some(uuid);
            }
        }
        if let Some(group_id) = obj.get("client_group_id").and_then(|v| v.as_str()) {
            if let Ok(uuid) = Uuid::parse_str(group_id) {
                exec_ctx.client_group_id = Some(uuid);
            }
        }
        if let Some(group_name) = obj.get("client_group_name").and_then(|v| v.as_str()) {
            exec_ctx.client_group_name = Some(group_name.to_string());
        }
        if let Some(persona) = obj.get("persona").and_then(|v| v.as_str()) {
            exec_ctx.persona = Some(persona.to_string());
        }
        // Session CBU IDs
        if let Some(cbu_ids) = obj.get("session_cbu_ids").and_then(|v| v.as_array()) {
            exec_ctx.session_cbu_ids = cbu_ids
                .iter()
                .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                .collect();
        }
    }

    exec_ctx
}

fn collect_side_effects(
    original_ctx: &VerbExecutionContext,
    exec_ctx: &ExecutionContext,
) -> VerbSideEffects {
    // Find new bindings (symbols that weren't in the original context)
    let mut new_bindings = std::collections::HashMap::new();
    let mut new_binding_types = std::collections::HashMap::new();

    for (name, uuid) in &exec_ctx.symbols {
        if original_ctx.symbols.get(name) != Some(uuid) {
            new_bindings.insert(name.clone(), *uuid);
        }
    }
    for (name, entity_type) in &exec_ctx.symbol_types {
        if original_ctx.symbol_types.get(name) != Some(entity_type) {
            new_binding_types.insert(name.clone(), entity_type.clone());
        }
    }

    // Pack pending_* fields back into platform state
    let mut platform = serde_json::Map::new();

    if exec_ctx.pending_view_state.is_some() {
        platform.insert(
            "has_pending_view_state".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if exec_ctx.pending_scope_change.is_some() {
        platform.insert(
            "has_pending_scope_change".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if exec_ctx.pending_session.is_some() {
        platform.insert(
            "has_pending_session".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if !exec_ctx.pending_dag_flags.is_empty() {
        let flags: Vec<serde_json::Value> = exec_ctx
            .pending_dag_flags
            .iter()
            .map(|(k, v)| serde_json::json!({"key": k, "value": v}))
            .collect();
        platform.insert(
            "pending_dag_flags".to_string(),
            serde_json::Value::Array(flags),
        );
    }

    VerbSideEffects {
        new_bindings,
        new_binding_types,
        platform_state: serde_json::Value::Object(platform),
    }
}

fn to_verb_outcome(result: &ExecutionResult) -> VerbExecutionOutcome {
    match result {
        ExecutionResult::Uuid(id) => VerbExecutionOutcome::Uuid(*id),
        ExecutionResult::Record(v) => VerbExecutionOutcome::Record(v.clone()),
        ExecutionResult::RecordSet(v) => VerbExecutionOutcome::RecordSet(v.clone()),
        ExecutionResult::Affected(n) => VerbExecutionOutcome::Affected(*n),
        ExecutionResult::Void => VerbExecutionOutcome::Void,
        // Domain-specific result types — serialize via Debug repr until
        // these types gain Serialize derives (Phase 2 migration)
        ExecutionResult::EntityQuery(r) => VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "entity_query", "_debug": format!("{r:?}")}),
        ),
        ExecutionResult::TemplateInvoked(r) => VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "template_invoked", "_debug": format!("{r:?}")}),
        ),
        ExecutionResult::TemplateBatch(r) => VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "template_batch", "_debug": format!("{r:?}")}),
        ),
        ExecutionResult::BatchControl(r) => VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "batch_control", "_debug": format!("{r:?}")}),
        ),
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_core::principal::Principal;

    #[test]
    fn split_fqn_valid() {
        let (domain, verb) = split_fqn("cbu.create").unwrap();
        assert_eq!(domain, "cbu");
        assert_eq!(verb, "create");
    }

    #[test]
    fn split_fqn_with_hyphen() {
        let (domain, verb) = split_fqn("kyc-case.create-case").unwrap();
        assert_eq!(domain, "kyc-case");
        assert_eq!(verb, "create-case");
    }

    #[test]
    fn split_fqn_invalid() {
        assert!(split_fqn("noperiod").is_err());
    }

    #[test]
    fn build_verb_call_from_json() {
        let args = serde_json::json!({"name": "Acme Fund", "kind": "pe"});
        let vc = build_verb_call("cbu", "create", &args);

        assert_eq!(vc.domain, "cbu");
        assert_eq!(vc.verb, "create");
        assert_eq!(vc.arguments.len(), 2);
    }

    #[test]
    fn build_verb_call_empty_args() {
        let vc = build_verb_call("session", "info", &serde_json::json!({}));
        assert_eq!(vc.arguments.len(), 0);
    }

    #[test]
    fn build_verb_call_uuid_arg() {
        let id = Uuid::new_v4();
        let args = serde_json::json!({"entity-id": id.to_string()});
        let vc = build_verb_call("entity", "ghost", &args);

        assert_eq!(vc.arguments.len(), 1);
        assert!(
            matches!(&vc.arguments[0].value, AstNode::Literal(Literal::Uuid(u), _) if *u == id)
        );
    }

    #[test]
    fn to_dsl_context_copies_symbols() {
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let id = Uuid::new_v4();
        ctx.bind_typed("cbu", id, "cbu");
        ctx.execution_id = Uuid::nil();

        let exec_ctx = to_dsl_context(&ctx);
        assert_eq!(exec_ctx.symbols.get("cbu"), Some(&id));
        assert_eq!(
            exec_ctx.symbol_types.get("cbu").map(|s| s.as_str()),
            Some("cbu")
        );
        assert_eq!(exec_ctx.execution_id, Uuid::nil());
    }

    #[test]
    fn to_dsl_context_unpacks_extensions() {
        let mut ctx = VerbExecutionContext::new(Principal::system());
        ctx.extensions = serde_json::json!({
            "audit_user": "alice",
            "session_id": Uuid::nil().to_string(),
            "persona": "kyc"
        });

        let exec_ctx = to_dsl_context(&ctx);
        assert_eq!(exec_ctx.audit_user.as_deref(), Some("alice"));
        assert_eq!(exec_ctx.session_id, Some(Uuid::nil()));
        assert_eq!(exec_ctx.persona.as_deref(), Some("kyc"));
    }

    #[test]
    fn collect_side_effects_detects_new_bindings() {
        let ctx = VerbExecutionContext::new(Principal::system());
        let mut exec_ctx = ExecutionContext::default();
        let new_id = Uuid::new_v4();
        exec_ctx.symbols.insert("cbu".to_string(), new_id);
        exec_ctx
            .symbol_types
            .insert("cbu".to_string(), "cbu".to_string());

        let fx = collect_side_effects(&ctx, &exec_ctx);
        assert_eq!(fx.new_bindings.get("cbu"), Some(&new_id));
        assert_eq!(
            fx.new_binding_types.get("cbu").map(|s| s.as_str()),
            Some("cbu")
        );
    }

    #[test]
    fn collect_side_effects_ignores_unchanged_bindings() {
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let existing_id = Uuid::new_v4();
        ctx.bind("cbu", existing_id);

        let mut exec_ctx = ExecutionContext::default();
        exec_ctx.symbols.insert("cbu".to_string(), existing_id);

        let fx = collect_side_effects(&ctx, &exec_ctx);
        assert!(fx.new_bindings.is_empty());
    }

    #[test]
    fn to_verb_outcome_all_variants() {
        let id = Uuid::new_v4();
        assert!(
            matches!(to_verb_outcome(&ExecutionResult::Uuid(id)), VerbExecutionOutcome::Uuid(u) if u == id)
        );
        assert!(matches!(
            to_verb_outcome(&ExecutionResult::Record(serde_json::json!({"a":1}))),
            VerbExecutionOutcome::Record(_)
        ));
        assert!(
            matches!(to_verb_outcome(&ExecutionResult::RecordSet(vec![])), VerbExecutionOutcome::RecordSet(v) if v.is_empty())
        );
        assert!(matches!(
            to_verb_outcome(&ExecutionResult::Affected(5)),
            VerbExecutionOutcome::Affected(5)
        ));
        assert!(matches!(
            to_verb_outcome(&ExecutionResult::Void),
            VerbExecutionOutcome::Void
        ));
    }

    #[test]
    fn behavior_routing_resolves_known_verbs() {
        use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};

        let registry = runtime_registry();

        // cbu.show should be CRUD (SELECT)
        if let Some(rv) = registry.get("cbu", "show") {
            assert!(
                matches!(rv.behavior, RuntimeBehavior::Crud(_)),
                "cbu.show should be CRUD, got {:?}",
                std::mem::discriminant(&rv.behavior)
            );
        }

        // cbu.create should be Plugin
        if let Some(rv) = registry.get("cbu", "create") {
            assert!(
                matches!(rv.behavior, RuntimeBehavior::Plugin(_)),
                "cbu.create should be Plugin, got {:?}",
                std::mem::discriminant(&rv.behavior)
            );
        }
    }

    #[test]
    fn crud_port_is_optional() {
        // ObPocVerbExecutor without crud_port should still be constructable
        // (all CRUD verbs fall through to DslExecutor)
        // This just verifies the type compiles — actual execution needs a pool.
        let _has_method = ObPocVerbExecutor::with_crud_port;
    }
}

/// T4.1 exit criterion: "enforce-mode Path A green end-to-end" — proves the
/// admission mechanism itself works against a real `ObPocVerbExecutor` and
/// a real envelope row, without flipping any actual production verb into
/// the enforced set (the env var is set and unset entirely within this
/// test's own process-local scope). `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`
/// being unset is the production default (see `EnforcedVerbs::from_env`)
/// — every path stays shadow/legacy until it individually graduates
/// (§0), which this tranche does not do.
#[cfg(all(test, feature = "database"))]
mod t4_1_envelope_admission_tests {
    use super::*;
    use crate::sequencer_tx::PgTransactionScope;
    use dsl_runtime::TransactionScope;

    async fn test_pool() -> sqlx::PgPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        sqlx::PgPool::connect(&url).await.expect("connect")
    }

    /// T9.2: `admit()` (pool-based) was removed in favor of `admit_in_scope`
    /// — these tests now open their own scope per call, exactly mirroring
    /// what `execute_verb_admitting_envelope` does in production. A
    /// consume only durably persists once the scope commits, so tests
    /// asserting a prior consume is visible to a later call must commit
    /// between them.
    async fn admit_in_scope_committed(
        executor: &ObPocVerbExecutor,
        verb_fqn: &str,
        envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
        pool: &sqlx::PgPool,
    ) -> dsl_runtime::Result<Option<ob_poc_control_plane::snapshot::SnapshotPins>> {
        admit_in_scope_committed_on_path(
            executor,
            verb_fqn,
            envelope_handle,
            ob_poc_types::ExecutionPath::RunbookSequencer,
            pool,
        )
        .await
    }

    /// Path-parameterised sibling of [`admit_in_scope_committed`] — added
    /// for G3's per-path tests (§5 items 6-7 of
    /// `EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001`).
    async fn admit_in_scope_committed_on_path(
        executor: &ObPocVerbExecutor,
        verb_fqn: &str,
        envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
        path: ob_poc_types::ExecutionPath,
        pool: &sqlx::PgPool,
    ) -> dsl_runtime::Result<Option<ob_poc_control_plane::snapshot::SnapshotPins>> {
        let mut scope = PgTransactionScope::begin(pool).await.expect("begin scope");
        let result = executor
            .admit_in_scope(verb_fqn, envelope_handle, path, scope.executor())
            .await;
        match &result {
            Ok(_) => scope.commit().await.expect("commit"),
            Err(_) => scope.rollback().await.expect("rollback"),
        }
        result
    }

    /// Guards `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` mutation so these tests
    /// can't interleave with each other (env vars are process-global) and
    /// always restores the unset production default on drop.
    // T10.2: `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` is process-global — under
    // default parallel `cargo test`, two `EnvGuard`-holding tests racing
    // each other (one's `set` landing between another's `set` and its own
    // assertions) makes `shadow_default_admits_every_verb_with_no_envelope`
    // observe someone else's enforced verb, or an enforced-verb test
    // observe the default's empty set. `EnvGuard` itself only ever
    // provided cleanup-on-drop, not mutual exclusion between tests — this
    // mutex closes that gap (found while adding T10.2's pin-verification
    // tests to this module and re-running the whole suite in parallel, per
    // this session's established practice of proving fixes via repeated
    // live-DB runs; the race pre-dates this tranche, same PIR-D-004 shape:
    // fix test isolation, not product code).
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

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn shadow_default_admits_every_verb_with_no_envelope() {
        let _guard = ENV_GUARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS"); // explicit production default
        let pool = test_pool().await;
        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        admit_in_scope_committed(&executor, "cbu.confirm", None, &pool)
            .await
            .expect("unset OB_POC_CONTROL_PLANE_ENFORCE_VERBS must admit every verb, envelope or not");
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn enforced_verb_without_envelope_is_rejected() {
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;
        let executor = ObPocVerbExecutor::from_pool(pool.clone());

        let err = admit_in_scope_committed(&executor, "cbu.confirm", None, &pool)
            .await
            .expect_err("enforced verb with no envelope must be rejected");
        assert!(err.to_string().contains("no sealed envelope was presented"));
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn enforced_verb_with_consumed_envelope_admits_then_rejects_resubmission() {
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;

        let envelope_id = Uuid::new_v4();
        let content_hash: [u8; 32] = [0xAB; 32];
        let handle = ob_poc_types::EnvelopeHandle::new(envelope_id, content_hash);
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, verb_fqn,
                status, not_before, not_after
            ) VALUES ($1, $2, $3, 'cbu.confirm', 'sealed', now() - interval '1 minute', now() + interval '5 minutes')
            "#,
        )
        .bind(envelope_id)
        .bind(handle.content_hash_hex())
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect("insert sealed envelope row");

        let executor = ObPocVerbExecutor::from_pool(pool.clone());

        admit_in_scope_committed(&executor, "cbu.confirm", Some(handle), &pool)
            .await
            .expect("sealed, unconsumed envelope must admit");

        let err = admit_in_scope_committed(&executor, "cbu.confirm", Some(handle), &pool)
            .await
            .expect_err("resubmitting the same (now-consumed) envelope must be rejected");
        assert!(err.to_string().contains("AlreadyConsumed"));

        // A different, non-enforced verb is untouched by this envelope's fate.
        admit_in_scope_committed(&executor, "cbu.reject", None, &pool)
            .await
            .expect("cbu.reject is not in the enforced set — must admit regardless");
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn envelope_with_wrong_content_hash_is_rejected_loudly() {
        // T8.1 (EOP-PLAN-CONTROLPLANE-001, closes PIR-D-008/010): a handle
        // with the correct id but a content hash that does not match the
        // persisted row (e.g. minted from a different envelope, or
        // tampered with) must be rejected — this is exactly the guarantee
        // `try_consume_by_id` could not provide and `try_consume` can.
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;

        let envelope_id = Uuid::new_v4();
        let real_hash: [u8; 32] = [0x11; 32];
        let real_handle = ob_poc_types::EnvelopeHandle::new(envelope_id, real_hash);
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, verb_fqn,
                status, not_before, not_after
            ) VALUES ($1, $2, $3, 'cbu.confirm', 'sealed', now() - interval '1 minute', now() + interval '5 minutes')
            "#,
        )
        .bind(envelope_id)
        .bind(real_handle.content_hash_hex())
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect("insert sealed envelope row");

        let executor = ObPocVerbExecutor::from_pool(pool.clone());

        // Same id, wrong hash — a forged/mismatched handle.
        let wrong_hash: [u8; 32] = [0x22; 32];
        let wrong_handle = ob_poc_types::EnvelopeHandle::new(envelope_id, wrong_hash);
        let err = admit_in_scope_committed(&executor, "cbu.confirm", Some(wrong_handle), &pool)
            .await
            .expect_err("content-hash mismatch must reject, not silently admit on id match alone");
        assert!(err.to_string().contains("ContentHashMismatch"));

        // The real handle is still consumable afterward — a rejected
        // mismatched attempt must not have poisoned or consumed the row.
        admit_in_scope_committed(&executor, "cbu.confirm", Some(real_handle), &pool)
            .await
            .expect("the genuine handle must still admit after a mismatched attempt");
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn execute_verb_admitting_envelope_rolls_back_the_consume_when_dispatch_fails() {
        // T9.2's whole point, end to end: `execute_verb_admitting_envelope`
        // now runs admission-check and verb dispatch inside ONE scope
        // (§2's one-scope-before-branching principle). Before this
        // tranche, `admit()` committed its own transaction independently
        // of dispatch — a failed dispatch after a successful admission
        // permanently burned the envelope even though the verb never ran.
        // Proves the fix: dispatch a verb guaranteed to fail past
        // admission (registered — passes T11.F.2's G1 floor — but with no
        // args, so dispatch itself fails), then show the envelope is
        // STILL consumable afterward — the whole scope, including the
        // consume, rolled back.
        //
        // T11.F.2 slice 4 note: this test previously used
        // "nonexistent.verb" to force a guaranteed dispatch failure —
        // that input is now floor-rejected at `admit_in_scope`, before a
        // scope/dispatch ever runs at all, so it no longer exercises this
        // test's actual subject (rollback-together semantics for a
        // failure that occurs *inside* the open scope). Switched to a
        // real registered verb with no args instead; the new
        // `execute_verb_admitting_envelope_floor_rejects_an_unregistered_verb_before_any_scope_or_consume`
        // test below covers the floor-rejection case this one used to.
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;

        let envelope_id = Uuid::new_v4();
        let content_hash: [u8; 32] = [0x33; 32];
        let handle = ob_poc_types::EnvelopeHandle::new(envelope_id, content_hash);
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, verb_fqn,
                status, not_before, not_after
            ) VALUES ($1, $2, $3, 'cbu.confirm', 'sealed', now() - interval '1 minute', now() + interval '5 minutes')
            "#,
        )
        .bind(envelope_id)
        .bind(handle.content_hash_hex())
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect("insert sealed envelope row");

        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let mut ctx = VerbExecutionContext::new(sem_os_core::principal::Principal::system());

        let dispatch_err = executor
            .execute_verb_admitting_envelope("cbu.confirm", serde_json::json!({}), &mut ctx, Some(handle), ob_poc_types::ExecutionPath::RunbookSequencer)
            .await
            .expect_err("dispatching cbu.confirm with no args must fail");
        assert!(
            !dispatch_err.to_string().contains("envelope admission rejected")
                && !dispatch_err.to_string().contains("T11.F floor rejection"),
            "the failure must come from dispatch, not admission or the floor: {dispatch_err}"
        );

        // If the scope truly rolled back together, the envelope must still
        // be consumable — a second admission attempt with the same handle
        // succeeds exactly as if the first attempt never happened.
        admit_in_scope_committed(&executor, "cbu.confirm", Some(handle), &pool)
            .await
            .expect(
                "the envelope must still be consumable after a dispatch failure — \
                 a rolled-back scope must leave the consume undone",
            );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn execute_verb_admitting_envelope_floor_rejects_an_unregistered_verb_before_any_scope_or_consume(
    ) {
        // T11.F.2 slice 4: an unregistered verb_fqn is rejected by G1's
        // floor check inside `admit_in_scope`, unconditionally — even
        // when a validly sealed envelope is presented for it (proving
        // this is NOT an envelope/EnforcedVerbs decision at all; it fires
        // before that logic even runs). Fault-injection matrix item
        // (design doc §5): "unknown verb on Path A and Path D."
        let pool = test_pool().await;

        let envelope_id = Uuid::new_v4();
        let content_hash: [u8; 32] = [0x44; 32];
        let handle = ob_poc_types::EnvelopeHandle::new(envelope_id, content_hash);
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, verb_fqn,
                status, not_before, not_after
            ) VALUES ($1, $2, $3, 'nonexistent.verb', 'sealed', now() - interval '1 minute', now() + interval '5 minutes')
            "#,
        )
        .bind(envelope_id)
        .bind(handle.content_hash_hex())
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect("insert sealed envelope row");

        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let mut ctx = VerbExecutionContext::new(sem_os_core::principal::Principal::system());

        let err = executor
            .execute_verb_admitting_envelope("nonexistent.verb", serde_json::json!({}), &mut ctx, Some(handle), ob_poc_types::ExecutionPath::RunbookSequencer)
            .await
            .expect_err("an unregistered verb must be floor-rejected");
        assert!(
            err.to_string().contains("T11.F floor rejection [G1]"),
            "must be the floor, not some other failure: {err}"
        );

        // The floor fires before the envelope consume even runs — prove
        // it directly: the sealed row is untouched.
        let status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".control_plane_envelopes WHERE envelope_id = $1"#,
        )
        .bind(envelope_id)
        .fetch_one(&pool)
        .await
        .expect("envelope row must still exist");
        assert_eq!(
            status, "sealed",
            "the floor rejection must never have touched the envelope's consume state"
        );
    }

    /// T10.2: end-to-end proof that a sealed envelope pinning a stale
    /// `row_version` is rejected at admission — not merely that
    /// `verify_pins_in_scope` rejects in isolation (`toctou_recheck`'s own
    /// unit tests already prove that), but that
    /// `execute_verb_admitting_envelope` actually calls it, in the right
    /// place, against a real row, and rolls the whole scope back
    /// (including the envelope consume) rather than partially admitting.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn execute_verb_admitting_envelope_rejects_on_pin_drift_and_leaves_envelope_reconsumable() {
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;

        let (cbu_id, real_row_version): (Uuid, i64) =
            sqlx::query_as(r#"SELECT cbu_id, row_version FROM "ob-poc".cbus ORDER BY cbu_id LIMIT 1 OFFSET 2"#)
                .fetch_one(&pool)
                .await
                .expect("at least 3 cbu rows exist in the dev database (offsets 0/1 used by crud_executor tests)");

        let intent = ob_poc_control_plane::intent_admission::tests_support::admitted(Uuid::new_v4(), "cbu.confirm");
        let binding = ob_poc_control_plane::entity_binding::tests_support::bound(vec![cbu_id]);
        let pack = ob_poc_control_plane::pack_resolution::tests_support::resolved("ob-poc.cbu");
        let dag =
            ob_poc_control_plane::dag_proof::tests_support::legal(cbu_id, "VALIDATION_PENDING", "VALIDATED");
        let authority = ob_poc_control_plane::authority_gate::tests_support::authorised("actor-1", "compliance_officer");
        let evidence = ob_poc_control_plane::evidence_gate::tests_support::sufficient(vec!["obligation-1".into()]);
        let write_set = ob_poc_control_plane::write_set::tests_support::proof(
            vec![cbu_id],
            vec!["validation_state".into()],
            vec!["ob-poc.cbus".into()],
            vec!["status".into()],
            "idem-pin-drift",
        );
        let runbook = ob_poc_control_plane::proof::CompiledRunbookRef::new(Uuid::new_v4());
        // Deliberately stale: pin the row one version behind its real,
        // current value — exactly what a concurrent writer having moved
        // the row since gate time would produce.
        let snapshot = ob_poc_control_plane::snapshot::tests_support::pins(
            Some(Uuid::new_v4()),
            None,
            None,
            vec![(cbu_id, "cbu".to_string(), real_row_version - 1)],
        );
        let now = chrono::Utc::now();
        let envelope = ob_poc_control_plane::envelope::test_support::seal(
            intent,
            binding,
            pack,
            dag,
            authority,
            evidence,
            write_set,
            runbook,
            snapshot,
            ob_poc_control_plane::envelope::ValidityWindow::new(
                now - chrono::Duration::minutes(1),
                now + chrono::Duration::minutes(5),
            ),
        );
        let handle = envelope.handle();
        assert!(
            crate::agent::control_plane_envelope_store::persist_sealed(
                &pool,
                Uuid::new_v4(),
                Uuid::now_v7(),
                "cbu.confirm",
                &envelope
            )
            .await
        );

        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let mut ctx = VerbExecutionContext::new(sem_os_core::principal::Principal::system());

        let dispatch_err = executor
            .execute_verb_admitting_envelope(
                "cbu.confirm",
                serde_json::json!({ "cbu-id": cbu_id.to_string() }),
                &mut ctx,
                Some(handle),
                ob_poc_types::ExecutionPath::RunbookSequencer,
            )
            .await
            .expect_err("a stale pinned row_version must reject at admission, before dispatch");
        assert!(
            dispatch_err.to_string().contains("pinned entity state drifted"),
            "must fail for the pin-drift reason, not some other cause: {dispatch_err}"
        );

        // The consume must have rolled back with the rest of the scope —
        // the envelope is still consumable (same rollback-retry corollary
        // the dispatch-failure test above proves for the write path).
        admit_in_scope_committed(&executor, "cbu.confirm", Some(handle), &pool)
            .await
            .expect(
                "a pin-drift rejection must leave the envelope reconsumable — \
                 the whole scope, including the consume, rolled back together",
            );
    }
}

/// G6a (`EOP-DESIGN-CONTROLPLANE-G6A-SNAPSHOT-PIN-CARRIER-001` §8):
/// `mint_envelope_for_bus` end-to-end — proves the mechanism
/// admits/rejects for real, not just that it compiles, and proves the
/// design doc's §6/§7 disclosed limitation (today's realistic Path D
/// `EvaluationContext` can never reach `ApprovedStp`) is real and
/// reproducible, not asserted from prose alone.
#[cfg(all(test, feature = "database"))]
mod g6a_bus_envelope_mint_tests {
    use super::*;
    use dsl_runtime::TransactionScope;

    async fn test_pool() -> sqlx::PgPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        sqlx::PgPool::connect(&url).await.expect("connect")
    }

    static ENV_GUARD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);
    impl EnvGuard {
        fn set(verb_fqn: &str) -> Self {
            let guard = ENV_GUARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            std::env::set_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS", format!("{verb_fqn}:D"));
            Self(guard)
        }
        fn unset() -> Self {
            let guard = ENV_GUARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            std::env::remove_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS");
            Self(guard)
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS");
        }
    }

    /// Today's actual `bus_runtime.rs::ObPocVerbAdapter::execute`
    /// construction — only `intent_admission`/`stp_classifier`/
    /// `version_pinning` populated. Structurally cannot reach
    /// `ApprovedStp` (design doc §7): `entity_binding`, `pack_resolution`,
    /// `dag_proof`, `authority`, `evidence`, `write_set`, `snapshot` are
    /// all `None`, and a `None` field is a hard failure by
    /// `EvaluationContext`'s own contract (`context.rs`'s module doc).
    fn realistic_path_d_context(verb_fqn: &str, entry_id: Uuid) -> ob_poc_control_plane::context::EvaluationContext {
        ob_poc_control_plane::context::EvaluationContext {
            intent_admission: Some(ob_poc_control_plane::intent_admission::IntentAdmissionInput {
                intent_id: entry_id,
                verb_fqn: verb_fqn.to_string(),
                is_admitted: true,
                exclusion_reasons: Vec::new(),
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            stp_classifier: Some(ob_poc_control_plane::stp_classifier::StpClassifierInput {
                is_durable_verb: false,
                durable_execution_explicitly_allowed: false,
                has_unpinned_entities: false,
            }),
            version_pinning: Some(ob_poc_control_plane::versioning::VersionPinningInput {
                versions: ob_poc_control_plane::snapshot::PinnedVersionSet::default(),
            }),
            ..Default::default()
        }
    }

    /// Every proof-bearing gate genuinely present — mirrors
    /// `ob_poc_control_plane::decision::tests::sealable_context()`
    /// (private to that crate's own test module, so rebuilt here rather
    /// than widening that module's visibility for one cross-crate
    /// caller). Proves the *mechanism* (mint → persist → consume →
    /// dispatch) works correctly whenever all eight facts genuinely
    /// exist — the honest state this infra is ready for the moment the
    /// design doc's §7 path-awareness gap closes for Path D specifically.
    fn full_sealable_context(verb_fqn: &str, entity: Uuid) -> ob_poc_control_plane::context::EvaluationContext {
        use ob_poc_control_plane::authority_gate::{AccessDecisionKind, AuthorityInput};
        use ob_poc_control_plane::dag_proof::DagProofInput;
        use ob_poc_control_plane::entity_binding::{EntityBindingInput, EntityFacts};
        use ob_poc_control_plane::evidence_gate::EvidenceInput;
        use ob_poc_control_plane::intent_admission::IntentAdmissionInput;
        use ob_poc_control_plane::pack_resolution::PackResolutionInput;
        use ob_poc_control_plane::proof::RunbookProofInput;
        use ob_poc_control_plane::snapshot::SnapshotInput;
        use ob_poc_control_plane::stp_classifier::StpClassifierInput;
        use ob_poc_control_plane::write_set::WriteSetInput;

        ob_poc_control_plane::context::EvaluationContext {
            intent_admission: Some(IntentAdmissionInput {
                intent_id: Uuid::nil(),
                verb_fqn: verb_fqn.to_string(),
                is_admitted: true,
                exclusion_reasons: vec![],
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            entity_binding: Some(EntityBindingInput {
                entities: vec![EntityFacts {
                    entity_id: entity,
                    exists: true,
                    expected_kind: "cbu".to_string(),
                    actual_kind: "cbu".to_string(),
                    lifecycle_state_readable: true,
                    availability_blocked: false,
                    availability_reason: None,
                    in_active_pack: true,
                }],
            }),
            pack_resolution: Some(PackResolutionInput {
                candidate_pack_ids: vec!["ob-poc.cbu".to_string()],
                semreg_allowed_set_available: true,
                constraint_denies_intent: false,
            }),
            dag_proof: Some(DagProofInput {
                entity_id: entity,
                from_state: "VALIDATION_PENDING".to_string(),
                to_state: "VALIDATED".to_string(),
                blocking_violations: vec![],
                lifecycle_fail_open_class: None,
                lifecycle_gate_mode_fail_closed: false,
            }),
            authority: Some(AuthorityInput {
                actor_id: "bus-federated".to_string(),
                role: "compliance_officer".to_string(),
                access_decision: AccessDecisionKind::Allow,
                deny_reason: None,
                requires_human_approval: false,
                requires_second_line_review: false,
                segregation_of_duties_violated: false,
                toctou_drifted: false,
            }),
            evidence: Some(EvidenceInput {
                evidence_gaps: vec![],
                kyc_precondition_failures: vec![],
                satisfied_obligation_ids: vec!["obligation-1".to_string()],
                open_obligation_ids: vec![],
            }),
            write_set: Some(WriteSetInput {
                entity_ids: vec![entity],
                state_slots: vec!["validation_state".to_string()],
                tables: vec!["ob-poc.cbus".to_string()],
                allowed_columns: vec!["status".to_string()],
                idempotency_key: format!("g6a-test-{entity}"),
                contract_derived: true,
            }),
            snapshot: Some(SnapshotInput {
                sem_reg_snapshot_id: Some(Uuid::nil()),
                session_snapshot_id: None,
                kyc_manifest_hash: None,
                entity_row_versions: vec![(entity, "cbu".to_string(), 1)],
                versions: ob_poc_control_plane::snapshot::PinnedVersionSet::default(),
            }),
            stp_classifier: Some(StpClassifierInput {
                is_durable_verb: false,
                durable_execution_explicitly_allowed: false,
                has_unpinned_entities: false,
            }),
            runbook_proof: Some(RunbookProofInput {
                compiled_runbook_id: Some(Uuid::nil()),
            }),
            version_pinning: Some(ob_poc_control_plane::versioning::VersionPinningInput {
                versions: ob_poc_control_plane::snapshot::PinnedVersionSet::default(),
            }),
            write_set_attestation: None,
        }
    }

    /// Case 1: not-enforced verb → `None` regardless of `cp_ctx` content,
    /// zero DB writes (the `EnforcedVerbs::is_enforced` short-circuit
    /// fires before `evaluate()` or any query runs).
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn not_enforced_short_circuits_to_none_without_touching_the_db() {
        let _guard = EnvGuard::unset();
        let pool = test_pool().await;
        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let entity = Uuid::new_v4();
        let ctx = full_sealable_context("cbu.confirm", entity);

        // Baseline count first — this table is shared across the whole
        // test suite's live-DB history (hundreds of pre-existing rows for
        // 'cbu.confirm' from other tests), so an absolute-zero assertion
        // would be a false failure. A before/after delta is the correct
        // "this call wrote nothing" proof.
        let before: i64 = sqlx::query_scalar(
            r#"SELECT count(*) FROM "ob-poc".control_plane_envelopes WHERE verb_fqn = 'cbu.confirm'"#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let handle = executor
            .mint_envelope_for_bus("cbu.confirm", &ctx, Some(Uuid::new_v4()))
            .await;
        assert!(
            handle.is_none(),
            "an unenforced verb must never mint an envelope, even with a fully ApprovedStp-shaped context"
        );

        let after: i64 = sqlx::query_scalar(
            r#"SELECT count(*) FROM "ob-poc".control_plane_envelopes WHERE verb_fqn = 'cbu.confirm'"#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(before, after, "short-circuit must not have written anything");
    }

    /// Case 2: enforced verb + a full, genuinely-ApprovedStp-shaped
    /// context → `Some(handle)`, and a `sealed` (NOT yet `consumed`) row
    /// appears in `control_plane_envelopes` correlated by `entry_id =
    /// bus_pin`. `mint_envelope_for_bus` mints and persists only —
    /// consumption is deferred to the caller's own
    /// `execute_verb_admitting_envelope`/`admit_in_scope` call (case 4),
    /// preserving T9.2's rollback-together atomicity (design doc §4's
    /// correction note — found by this test's first draft asserting
    /// `'consumed'` here and getting a real, honest `'sealed'` back).
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn enforced_verb_with_full_context_mints_but_does_not_consume() {
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;
        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let entity = Uuid::new_v4();
        let bus_pin = Uuid::new_v4();
        let ctx = full_sealable_context("cbu.confirm", entity);

        let handle = executor
            .mint_envelope_for_bus("cbu.confirm", &ctx, Some(bus_pin))
            .await
            .expect("a fully ApprovedStp-shaped context must mint a real envelope");

        let row: (Uuid, String) = sqlx::query_as(
            r#"SELECT entry_id, status FROM "ob-poc".control_plane_envelopes WHERE envelope_id = $1"#,
        )
        .bind(handle.id())
        .fetch_one(&pool)
        .await
        .expect("the minted envelope's row must exist");
        assert_eq!(row.0, bus_pin, "entry_id must correlate to the caller-supplied bus_pin");
        assert_eq!(
            row.1, "sealed",
            "mint_envelope_for_bus must persist as sealed, not consume — consumption is the \
             caller's job via execute_verb_admitting_envelope/admit_in_scope (case 4)"
        );
    }

    /// Case 3: enforced verb + today's actual Path D-realistic context
    /// (only intent_admission/stp_classifier/version_pinning populated)
    /// → `None`. Reproduces the design doc's §6/§7 disclosed limitation
    /// as a real, run test — not an assertion made only in prose.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn enforced_verb_with_todays_realistic_context_is_rejected() {
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;
        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let entry_id = Uuid::new_v4();
        let ctx = realistic_path_d_context("cbu.confirm", entry_id);

        let handle = executor
            .mint_envelope_for_bus("cbu.confirm", &ctx, Some(Uuid::new_v4()))
            .await;
        assert!(
            handle.is_none(),
            "today's real Path D context is missing pack_resolution/runbook_proof by structural \
             necessity (design doc §7) — it can never reach ApprovedStp, and this must fail \
             closed (None), not panic or fabricate a pass"
        );
    }

    /// Case 4: end-to-end — case 2's minted-but-unconsumed handle,
    /// threaded into `admit_in_scope` (the same admission chain
    /// `execute_verb_admitting_envelope` calls before dispatch), actually
    /// admits on first use and is single-use (a second attempt with the
    /// same handle is rejected `AlreadyConsumed`). This is the
    /// RED→GREEN-provable claim: before this tranche, `bus_runtime.rs`
    /// passed a hardcoded `None` here always — with `cbu.confirm`
    /// enforced, that shape is `RejectedNoEnvelope` unconditionally. This
    /// test proves a real minted handle now reaches admission and is
    /// honoured by the already-proven T9.2/T10.2 mechanism, not a new one.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn minted_handle_gates_a_real_dispatch_and_is_single_use() {
        let _guard = EnvGuard::set("cbu.confirm");
        let pool = test_pool().await;
        let executor = ObPocVerbExecutor::from_pool(pool.clone());
        let entity = Uuid::new_v4();
        let ctx = full_sealable_context("cbu.confirm", entity);

        let handle = executor
            .mint_envelope_for_bus("cbu.confirm", &ctx, Some(Uuid::new_v4()))
            .await
            .expect("full context must mint");

        // First admission attempt: the sealed-but-unconsumed handle must
        // be admitted — proving the real handle reaches and is honoured
        // by the existing admission chain (not just persisted and
        // ignored).
        let mut scope = crate::sequencer_tx::PgTransactionScope::begin(&pool)
            .await
            .expect("begin scope");
        executor
            .admit_in_scope(
                "cbu.confirm",
                Some(handle),
                ob_poc_types::ExecutionPath::BusFederated,
                scope.executor(),
            )
            .await
            .expect("a minted, sealed, unconsumed envelope must be admitted on first use");
        scope.commit().await.expect("commit");

        // Second admission attempt against the SAME (now-consumed)
        // handle must observe AlreadyConsumed — single-use held.
        let mut scope2 = crate::sequencer_tx::PgTransactionScope::begin(&pool)
            .await
            .expect("begin scope");
        let err = executor
            .admit_in_scope(
                "cbu.confirm",
                Some(handle),
                ob_poc_types::ExecutionPath::BusFederated,
                scope2.executor(),
            )
            .await
            .expect_err("resubmitting an already-consumed handle must be rejected");
        scope2.rollback().await.expect("rollback");
        assert!(err.to_string().contains("AlreadyConsumed"));
    }
}
