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
    /// T9.2 (§3, §4): admission-checks `verb_fqn` against the
    /// `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` set (read fresh from the
    /// environment on every call — matches `LifecycleGateMode::from_env()`'s
    /// existing per-call-read pattern, `dsl_v2/executor.rs`) using the
    /// caller's own `&mut PgConnection` rather than a fresh pool checkout,
    /// so the envelope consume and the verb dispatch it gates are joined
    /// to one transaction. Superseded the pool-based `admit()` (removed —
    /// its only caller, `execute_verb_admitting_envelope`, now opens one
    /// scope up front and calls this instead). A rejection here rolls the
    /// whole scope back (the caller does this), which per the design doc's
    /// rollback-retry corollary correctly leaves the envelope reconsumable
    /// rather than burning it on a rejected attempt.
    async fn admit_in_scope(
        &self,
        verb_fqn: &str,
        envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
        conn: &mut sqlx::PgConnection,
    ) -> dsl_runtime::Result<()> {
        let enforced = crate::agent::control_plane_envelope_store::EnforcedVerbs::from_env();
        let decision = crate::agent::control_plane_envelope_store::check_admission_in_scope(
            conn,
            &enforced,
            verb_fqn,
            envelope_handle,
        )
        .await
        .map_err(|e| SemOsError::Internal(anyhow::anyhow!("envelope admission check failed: {e}")))?;

        match decision {
            crate::agent::control_plane_envelope_store::AdmissionDecision::NotEnforced
            | crate::agent::control_plane_envelope_store::AdmissionDecision::Admitted => Ok(()),
            crate::agent::control_plane_envelope_store::AdmissionDecision::RejectedNoEnvelope => {
                Err(SemOsError::InvalidInput(format!(
                    "{verb_fqn} is enforce-mode gated (OB_POC_CONTROL_PLANE_ENFORCE_VERBS) but no sealed envelope was presented"
                )))
            }
            crate::agent::control_plane_envelope_store::AdmissionDecision::RejectedConsumeFailed(outcome) => {
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
                        .execute_crud_in_scope(&contract, args.clone(), ctx, scope.executor())
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
        let vc = build_verb_call(&domain, &verb, &args);
        let mut exec_ctx = to_dsl_context(ctx);

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
    ) -> dsl_runtime::Result<VerbExecutionResult> {
        use crate::sequencer_tx::PgTransactionScope;
        use dsl_runtime::TransactionScope;

        let pool = self.executor.pool();
        let mut scope = PgTransactionScope::begin(pool).await.map_err(|e| {
            SemOsError::Internal(anyhow::anyhow!(
                "execute_verb_admitting_envelope({verb_fqn}): begin txn failed: {e}"
            ))
        })?;

        if let Err(e) = self
            .admit_in_scope(verb_fqn, envelope_handle, scope.executor())
            .await
        {
            if let Err(rollback_err) = scope.rollback().await {
                tracing::warn!(
                    verb_fqn,
                    %rollback_err,
                    "execute_verb_admitting_envelope: rollback failed after admission rejection"
                );
            }
            return Err(e);
        }

        let scope_dyn: &mut dyn dsl_runtime::TransactionScope = &mut scope;
        match self
            .execute_verb_in_open_scope(verb_fqn, args, ctx, scope_dyn)
            .await
        {
            Ok(result) => {
                scope.commit().await.map_err(|e| {
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
pub fn build_verb_call_pub(domain: &str, verb: &str, args: &serde_json::Value) -> VerbCall {
    build_verb_call(domain, verb, args)
}

/// Public wrapper for use by the compatibility shim.
pub fn to_dsl_context_pub(ctx: &VerbExecutionContext) -> ExecutionContext {
    to_dsl_context(ctx)
}

/// Public wrapper for use by the compatibility shim.
pub fn to_verb_outcome_pub(result: &ExecutionResult) -> VerbExecutionOutcome {
    to_verb_outcome(result)
}

/// Unpack `VerbExecutionContext.extensions` side-channel keys back into an
/// `ExecutionContext`'s `pending_*` fields. Called by `dsl_v2::executor`
/// after dispatch to propagate session/view/agent mutations the op staged
/// on its `VerbExecutionContext`.
pub fn apply_sem_ctx_extensions_to_exec_ctx(
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
pub fn verb_call_to_json(vc: &VerbCall) -> serde_json::Value {
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
pub fn from_verb_outcome(outcome: VerbExecutionOutcome) -> ExecutionResult {
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
    ) -> dsl_runtime::Result<()> {
        let mut scope = PgTransactionScope::begin(pool).await.expect("begin scope");
        let result = executor
            .admit_in_scope(verb_fqn, envelope_handle, scope.executor())
            .await;
        match &result {
            Ok(()) => scope.commit().await.expect("commit"),
            Err(_) => scope.rollback().await.expect("rollback"),
        }
        result
    }

    /// Guards `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` mutation so these tests
    /// can't interleave with each other (env vars are process-global) and
    /// always restores the unset production default on drop.
    struct EnvGuard;
    impl EnvGuard {
        fn set(verb_fqn: &str) -> Self {
            std::env::set_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS", verb_fqn);
            Self
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
        // Proves the fix: dispatch a verb guaranteed to fail (unknown
        // FQN), then show the envelope is STILL consumable afterward —
        // the whole scope, including the consume, rolled back.
        let _guard = EnvGuard::set("nonexistent.verb");
        let pool = test_pool().await;

        let envelope_id = Uuid::new_v4();
        let content_hash: [u8; 32] = [0x33; 32];
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

        let dispatch_err = executor
            .execute_verb_admitting_envelope("nonexistent.verb", serde_json::json!({}), &mut ctx, Some(handle))
            .await
            .expect_err("dispatching an unknown FQN must fail");
        assert!(
            !dispatch_err.to_string().contains("envelope admission rejected"),
            "the failure must come from dispatch, not admission: {dispatch_err}"
        );

        // If the scope truly rolled back together, the envelope must still
        // be consumable — a second admission attempt with the same handle
        // succeeds exactly as if the first attempt never happened.
        admit_in_scope_committed(&executor, "nonexistent.verb", Some(handle), &pool)
            .await
            .expect(
                "the envelope must still be consumable after a dispatch failure — \
                 a rolled-back scope must leave the consume undone",
            );
    }
}
