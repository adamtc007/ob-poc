//! Federated DSL bus runtime for ob-poc (v0.6 §T2B.9 item 35).
//!
//! Co-located inside `ob-poc-web` so the bus reuses the same Postgres
//! pool, `SemOsVerbOpRegistry`, and platform `ServiceRegistry` the
//! axum surface already constructs. A future tranche may split the
//! bus into its own bin (`ob-poc-bus-server`); tracked as tech debt
//! after T3 lands.
//!
//! Lifecycle:
//!
//! 1. `dsl_bus_storage::migrate(&pool)` applies outbox/inbox migrations.
//! 2. `BusClient::builder()` registers peer endpoints (bpmn-lite is the
//!    typical caller in v0.6 §10; ob-poc is a service domain).
//! 3. `start_sender()` spawns the §8.5 outbox-drain task.
//! 4. `BusServer::builder()` binds with [`ObPocVerbAdapter`] supplying
//!    [`VerbExecutor`] and `NoopResultDispatcher` (ob-poc never
//!    receives bus results).
//!
//! The adapter delegates verb execution to the same
//! `ObPocVerbExecutor` that drives the REPL / axum pipelines — every
//! plugin op registered in `SemOsVerbOpRegistry` is reachable.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use dsl_bus_client::BusClient;
use dsl_bus_protocol::v1::{
    typed_value::Value as ProtoTypedValueKind, ExecutionOutcomeKind, ResolvedBinding,
    TypedValue as ProtoTypedValue, Uuid as ProtoUuid,
};
use dsl_bus_server::{BusServer, ServerHandle};
use dsl_runtime::VerbExecutionPort;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use ob_poc::sem_os_runtime::verb_executor_adapter::ObPocVerbExecutor;
use ob_poc_bus_handler::{
    NoopResultDispatcher, ObPocBusHandler, VerbExecutor, VerbExecutorError, VerbOutcome,
};
use sem_os_core::principal::Principal;
use sqlx::PgPool;
use uuid::Uuid;

/// Owned bus runtime — drop or call [`shutdown`](Self::shutdown) to
/// stop both the server and the outbox sender cleanly.
pub(crate) struct BusRuntime {
    server: ServerHandle,
    sender: dsl_bus_client::SenderHandle,
}

impl BusRuntime {
    pub(crate) async fn shutdown(self) -> anyhow::Result<()> {
        let _ = self.server.shutdown().await;
        let _ = self.sender.shutdown().await;
        Ok(())
    }
}

/// Configuration plumbed in by `main`.
pub(crate) struct BusRuntimeConfig {
    pub(crate) pool: PgPool,
    pub(crate) verb_executor: Arc<ObPocVerbExecutor>,
    pub(crate) bind_addr: SocketAddr,
    /// Catalogue version this domain hosts. Mismatched incoming
    /// `InvocationRequest.catalogue_version` rejects with
    /// `RejectedVersionIncompatible` per T2B master DoD #46.
    pub(crate) catalogue_version: String,
    pub(crate) peers: Vec<(String, String)>,
}

/// Stand up the bus runtime.
pub(crate) async fn start(config: BusRuntimeConfig) -> anyhow::Result<BusRuntime> {
    dsl_bus_storage::migrate(&config.pool).await?;

    let mut builder = BusClient::builder()
        .pool(config.pool.clone())
        .local_domain("ob-poc");
    for (domain, uri) in &config.peers {
        builder = builder.add_peer(domain.clone(), uri.clone());
    }
    let client = builder.build().await?;
    let notifier = client.outbox_notifier();
    let sender = client.start_sender();

    let adapter = ObPocVerbAdapter {
        executor: config.verb_executor,
        // G5 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3 item 4): best-effort
        // shadow-decision persistence needs its own pool handle,
        // independent of the `TransactionScope` the real dispatch runs
        // under (same posture as Path A's `phase5_runtime_recheck`,
        // which spawns its shadow insert against a cloned pool, never
        // the request's own scope).
        pool: config.pool.clone(),
    };
    let handler = ObPocBusHandler::new(adapter).with_catalogue_version(config.catalogue_version);

    // A3 §3.5 — ob-poc declares all three federated services.
    // InvocationService is real (Submit + stubbed Validate);
    // EntityService and SemOsService are registered as stubs returning
    // NOT_IMPLEMENTED per A3 §6 discipline. The matching manifest
    // entries are emitted by `ob-poc-manifest-export`.
    let server = BusServer::builder()
        .pool(config.pool)
        .local_domain("ob-poc")
        .invocation_dispatcher(handler)
        .result_dispatcher(NoopResultDispatcher)
        .outbox_notifier(notifier)
        .enable_entity_service()
        .enable_sem_os_service()
        .bind(config.bind_addr)
        .build()
        .serve()
        .await?;

    tracing::info!(
        bind_addr = %server.local_addr(),
        "ob-poc bus server listening"
    );

    Ok(BusRuntime { server, sender })
}

/// Adapter that wraps [`ObPocVerbExecutor`] so the bus handler can call
/// the same execution port the REPL + axum surfaces use. Translates
/// bus-protocol bindings to/from `VerbExecutionContext` arg JSON.
struct ObPocVerbAdapter {
    executor: Arc<ObPocVerbExecutor>,
    /// G5: shadow-decision persistence pool (see the `start()` construction
    /// site comment for why this is a separate handle from the dispatch
    /// transaction scope).
    pool: PgPool,
}

#[async_trait]
impl VerbExecutor for ObPocVerbAdapter {
    async fn execute(
        &self,
        local_verb_id: &str,
        _catalogue_version: &str,
        inputs: Vec<ResolvedBinding>,
        snapshot_pin: Option<Uuid>,
    ) -> Result<VerbOutcome, VerbExecutorError> {
        let args = bindings_to_json(&inputs).map_err(VerbExecutorError::Malformed)?;
        // T6.1 (EOP-PLAN-CONTROLPLANE-001, C-034): distinct from
        // `Principal::system()` (which also carries the `admin` role) so
        // bus-originated actions are attributable in audit/telemetry as
        // coming over the federated bus, not conflated with genuine
        // system-internal actions.
        let mut ctx =
            VerbExecutionContext::new(Principal::in_process("bus-federated", vec!["bus".into()]));

        // Pre-populate symbol table with any uuid-typed bindings so
        // @reference resolution inside verb handlers sees the same
        // entities the caller passed.
        for binding in &inputs {
            if let Some(value) = binding.value.as_ref() {
                if let Some(ProtoTypedValueKind::UuidValue(uuid_msg)) = value.value.as_ref() {
                    if let Some(uuid) = uuid_from_proto(uuid_msg) {
                        ctx.bind(&binding.name, uuid);
                    }
                }
            }
        }

        // T6.1: route through the T4.1 envelope-admission entry point —
        // the bus is the first production caller. `envelope_handle: None`
        // (T8.1 widened this parameter from a bare Uuid to a typed
        // `ob_poc_types::EnvelopeHandle`) because nothing issues a sealed
        // `ExecutionEnvelope` for bus calls yet (T6.1a); with the
        // production-default empty
        // `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`, this is behaviourally
        // identical to the prior direct `execute_verb` call (`NotEnforced`)
        // for every verb — no dispatch outcome changes here. Flipping the
        // bus path to enforce-by-default (plan §0 assumption A1: "enforce
        // mode on from day one for bus, it has no legacy users to
        // shadow-compare") is NOT done by this change: bpmn-lite is a real
        // production bus caller today and nothing issues it a sealed
        // envelope, so defaulting to enforce would reject every live bus
        // verb call outright. That flip needs an explicit architect
        // decision, not a default flipped silently inside this adapter.
        // G5 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3 items 4-5): Path D
        // shadow-gate evaluation, extending the G1-G14 pipeline to the
        // bus adapter. Same bounded-and-disclosed posture as the G4 seam
        // (`dsl_v2::executor::execute_verb_in_scope`): G1 (weak "verb
        // resolved" signal) and G12 are independently substantive here
        // (zero declared predecessors); G8's input is also built but not
        // independently substantive yet -- it declares 7 predecessors in
        // `gate::GATE_DEPENDENCIES`, none wired at this adapter, so it
        // correctly reports `NotEvaluated` under collect-where-independent
        // semantics (confirmed live by the E3 matrix probe -- see the G5
        // session doc). G3/G9 are
        // the ratified NotApplicable cells (bus dispatch has no REPL pack
        // or runbook object at all — `ob_poc_control_plane::applicability`,
        // matching R:§B6's own confirmed finding for Path D). The
        // remaining gates stay unwired here for the same reason as the
        // G4 seam: `ob-poc-web` cannot reach `ob-poc`'s crate-private
        // `agent::control_plane_shadow` input builders (they are
        // `pub(crate)` to `ob-poc`, not `ob-poc-web` — a genuine
        // crate-boundary generalization gap, documented rather than
        // widening that module's pub surface to force it through in this
        // tranche). Best-effort, spawned, never blocks real dispatch.
        // No `#[cfg(feature = "database")]` gate here — unlike `ob-poc`
        // (whose `dsl_v2::executor` seam is compiled both with and
        // without the `database` feature), `ob-poc-web` declares no such
        // feature at all (`Cargo.toml` has `default = []` only); sqlx is
        // unconditionally available in this crate.
        // G6a (EOP-DESIGN-CONTROLPLANE-G6A-SNAPSHOT-PIN-CARRIER-001 §4):
        // built once, ahead of the shadow-audit spawn below, so both the
        // (unchanged) shadow evaluation AND the new real-evaluation mint
        // path (`mint_envelope_for_bus`) reuse the same
        // context construction instead of duplicating it.
        let fqn = local_verb_id.to_string();
        let entry_id = ctx.execution_id;
        let is_durable_verb = {
            use ob_poc::dsl_v2::execution::{runtime_registry, RuntimeBehavior};
            fqn.split_once('.')
                .and_then(|(d, v)| runtime_registry().get(d, v))
                .map(|rv| matches!(rv.behavior, RuntimeBehavior::Durable(_)))
                .unwrap_or(false)
        };
        let cp_ctx = ob_poc_control_plane::context::EvaluationContext {
            intent_admission: Some(ob_poc_control_plane::intent_admission::IntentAdmissionInput {
                intent_id: entry_id,
                verb_fqn: fqn.clone(),
                is_admitted: true,
                exclusion_reasons: Vec::new(),
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            stp_classifier: Some(ob_poc_control_plane::stp_classifier::StpClassifierInput {
                is_durable_verb,
                // Path D IS a legitimate durable-execution-allowed
                // context in principle (the bus is how an external
                // workflow engine reaches ob-poc), unlike Path A's
                // shadow call site -- but no attestation signal for
                // "this specific durable dispatch was authorised"
                // exists at this adapter today, so this stays
                // conservatively `false` (same "no signal means no
                // fabricated pass" posture used throughout this
                // gate stack).
                durable_execution_explicitly_allowed: false,
                has_unpinned_entities: false,
            }),
            version_pinning: Some(ob_poc_control_plane::versioning::VersionPinningInput {
                versions: ob_poc_control_plane::snapshot::PinnedVersionSet {
                    compiler_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                    ..Default::default()
                },
            }),
            ..Default::default()
        };

        // G5 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3 items 4-5): Path D
        // shadow-gate evaluation, extending the G1-G14 pipeline to the
        // bus adapter. Same bounded-and-disclosed posture as the G4 seam
        // (`dsl_v2::executor::execute_verb_in_scope`): G1 (weak "verb
        // resolved" signal) and G12 are independently substantive here
        // (zero declared predecessors); G8's input is also built but not
        // independently substantive yet -- it declares 7 predecessors in
        // `gate::GATE_DEPENDENCIES`, none wired at this adapter, so it
        // correctly reports `NotEvaluated` under collect-where-independent
        // semantics (confirmed live by the E3 matrix probe -- see the G5
        // session doc). G3/G9 are
        // the ratified NotApplicable cells (bus dispatch has no REPL pack
        // or runbook object at all — `ob_poc_control_plane::applicability`,
        // matching R:§B6's own confirmed finding for Path D). The
        // remaining gates stay unwired here for the same reason as the
        // G4 seam: `ob-poc-web` cannot reach `ob-poc`'s crate-private
        // `agent::control_plane_shadow` input builders (they are
        // `pub(crate)` to `ob-poc`, not `ob-poc-web` — a genuine
        // crate-boundary generalization gap, documented rather than
        // widening that module's pub surface to force it through in this
        // tranche). Best-effort, spawned, never blocks real dispatch.
        // No `#[cfg(feature = "database")]` gate here — unlike `ob-poc`
        // (whose `dsl_v2::executor` seam is compiled both with and
        // without the `database` feature), `ob-poc-web` declares no such
        // feature at all (`Cargo.toml` has `default = []` only); sqlx is
        // unconditionally available in this crate.
        {
            let pool = self.pool.clone();
            let fqn = fqn.clone();
            let cp_ctx = cp_ctx.clone();
            tokio::spawn(async move {
                let report = ob_poc_control_plane::evaluate_shadow(&cp_ctx);
                let report =
                    ob_poc_control_plane::applicability::apply_matrix(report, ob_poc_types::ExecutionPath::BusFederated);
                let row = ob_poc::agent::control_plane_shadow::build_shadow_decision_row(
                    Uuid::nil(),
                    entry_id,
                    &fqn,
                    &report,
                    false,
                    ob_poc_types::ExecutionPath::BusFederated,
                );
                ob_poc::agent::control_plane_shadow::insert_shadow_decision(&pool, &row).await;
            });
        }

        // G6a (EOP-DESIGN-CONTROLPLANE-G6A-SNAPSHOT-PIN-CARRIER-001 §4):
        // real (not shadow) admission-time envelope minting — replaces
        // the previously-hardcoded `None`. Short-circuits to `None`
        // itself whenever `local_verb_id` isn't enforce-gated for Path D
        // (the production default today), so this adds no behaviour
        // change and no extra DB round trip for the common case.
        let envelope_handle = self
            .executor
            .mint_envelope_for_bus(local_verb_id, &cp_ctx, snapshot_pin)
            .await;

        let result = self
            .executor
            .execute_verb_admitting_envelope(
                local_verb_id,
                args,
                &mut ctx,
                envelope_handle,
                ob_poc_types::ExecutionPath::BusFederated,
            )
            .await
            .map_err(map_executor_error)?;

        Ok(translate_result(result, ctx.execution_id))
    }
}

fn map_executor_error(err: sem_os_core::error::SemOsError) -> VerbExecutorError {
    use sem_os_core::error::SemOsError as E;
    match err {
        E::NotFound(s) => VerbExecutorError::UnknownVerb(s),
        E::Unauthorized(s) => VerbExecutorError::AuthorityDenied(s),
        E::InvalidInput(s) => VerbExecutorError::Malformed(s),
        other => VerbExecutorError::Internal(format!("{other}")),
    }
}

fn translate_result(result: dsl_runtime::VerbExecutionResult, execution_id: Uuid) -> VerbOutcome {
    let mut bindings = Vec::new();

    // Surface the outcome scalar (if any) under `result` so the caller
    // sees the verb's return value as a binding alongside `new_bindings`.
    match &result.outcome {
        VerbExecutionOutcome::Uuid(uuid) => {
            bindings.push(named_uuid_binding("result", *uuid));
        }
        VerbExecutionOutcome::Record(value) => {
            // Inline a string-encoded payload for the record; full
            // typed surface requires Phase-2 JSON-typed bindings on
            // the bus protocol.
            bindings.push(named_string_binding(
                "result",
                serde_json::to_string(value).unwrap_or_default(),
            ));
        }
        VerbExecutionOutcome::RecordSet(rows) => {
            bindings.push(named_string_binding(
                "result",
                serde_json::to_string(rows).unwrap_or_default(),
            ));
        }
        VerbExecutionOutcome::Affected(n) => {
            bindings.push(named_int_binding("affected", *n as i64));
        }
        VerbExecutionOutcome::Void => {}
    }

    for (name, uuid) in result.side_effects.new_bindings {
        bindings.push(named_uuid_binding(&name, uuid));
    }

    VerbOutcome {
        execution_id,
        kind: ExecutionOutcomeKind::Committed,
        detail: "ob-poc verb executed via SemOsVerbOpRegistry".into(),
        bindings,
    }
}

fn bindings_to_json(inputs: &[ResolvedBinding]) -> Result<serde_json::Value, String> {
    let mut map = serde_json::Map::new();
    for binding in inputs {
        let Some(value) = binding.value.as_ref() else {
            return Err(format!("binding '{}' missing value", binding.name));
        };
        let json = match value.value.as_ref() {
            Some(ProtoTypedValueKind::StringValue(s)) => serde_json::Value::String(s.clone()),
            Some(ProtoTypedValueKind::IntValue(n)) => serde_json::Value::Number((*n).into()),
            Some(ProtoTypedValueKind::DoubleValue(d)) => serde_json::Number::from_f64(*d)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Some(ProtoTypedValueKind::BoolValue(b)) => serde_json::Value::Bool(*b),
            Some(ProtoTypedValueKind::UuidValue(uuid_msg)) => uuid_from_proto(uuid_msg)
                .map(|u| serde_json::Value::String(u.to_string()))
                .ok_or_else(|| format!("binding '{}' has malformed uuid bytes", binding.name))?,
            Some(ProtoTypedValueKind::BlobValue(b)) => serde_json::Value::Array(
                b.iter()
                    .map(|byte| serde_json::Value::from(*byte))
                    .collect(),
            ),
            Some(ProtoTypedValueKind::NullValue(_)) | None => serde_json::Value::Null,
        };
        map.insert(binding.name.clone(), json);
    }
    Ok(serde_json::Value::Object(map))
}

fn uuid_from_proto(uuid_msg: &ProtoUuid) -> Option<Uuid> {
    let bytes: &[u8] = &uuid_msg.value;
    if bytes.len() != 16 {
        return None;
    }
    let mut arr = [0u8; 16];
    arr.copy_from_slice(bytes);
    Some(Uuid::from_bytes(arr))
}

fn named_uuid_binding(name: &str, uuid: Uuid) -> ResolvedBinding {
    ResolvedBinding {
        name: name.to_owned(),
        value: Some(ProtoTypedValue {
            value: Some(ProtoTypedValueKind::UuidValue(ProtoUuid {
                value: uuid.as_bytes().to_vec(),
            })),
            type_name: "uuid".to_owned(),
        }),
    }
}

fn named_int_binding(name: &str, n: i64) -> ResolvedBinding {
    ResolvedBinding {
        name: name.to_owned(),
        value: Some(ProtoTypedValue {
            value: Some(ProtoTypedValueKind::IntValue(n)),
            type_name: "i64".to_owned(),
        }),
    }
}

fn named_string_binding(name: &str, s: String) -> ResolvedBinding {
    ResolvedBinding {
        name: name.to_owned(),
        value: Some(ProtoTypedValue {
            value: Some(ProtoTypedValueKind::StringValue(s)),
            type_name: "string".to_owned(),
        }),
    }
}
