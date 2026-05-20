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
use dsl_runtime::execution::{VerbExecutionContext, VerbExecutionOutcome};
use dsl_runtime::port::VerbExecutionPort;
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
    };

    // A3 §3.5 — ob-poc declares all three federated services.
    // InvocationService is real (Submit + stubbed Validate);
    // EntityService and SemOsService are registered as stubs returning
    // NOT_IMPLEMENTED per A3 §6 discipline. The matching manifest
    // entries are emitted by `ob-poc-manifest-export`.
    let server = BusServer::builder()
        .pool(config.pool)
        .local_domain("ob-poc")
        .invocation_dispatcher(ObPocBusHandler::new(adapter))
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
}

#[async_trait]
impl VerbExecutor for ObPocVerbAdapter {
    async fn execute(
        &self,
        local_verb_id: &str,
        _catalogue_version: &str,
        inputs: Vec<ResolvedBinding>,
    ) -> Result<VerbOutcome, VerbExecutorError> {
        let args = bindings_to_json(&inputs).map_err(VerbExecutorError::Malformed)?;
        let mut ctx = VerbExecutionContext::new(Principal::system());

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

        let result = self
            .executor
            .execute_verb(local_verb_id, args, &mut ctx)
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

fn translate_result(
    result: dsl_runtime::execution::VerbExecutionResult,
    execution_id: Uuid,
) -> VerbOutcome {
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
            Some(ProtoTypedValueKind::StringValue(s)) => {
                serde_json::Value::String(s.clone())
            }
            Some(ProtoTypedValueKind::IntValue(n)) => {
                serde_json::Value::Number((*n).into())
            }
            Some(ProtoTypedValueKind::DoubleValue(d)) => serde_json::Number::from_f64(*d)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Some(ProtoTypedValueKind::BoolValue(b)) => serde_json::Value::Bool(*b),
            Some(ProtoTypedValueKind::UuidValue(uuid_msg)) => uuid_from_proto(uuid_msg)
                .map(|u| serde_json::Value::String(u.to_string()))
                .ok_or_else(|| format!("binding '{}' has malformed uuid bytes", binding.name))?,
            Some(ProtoTypedValueKind::BlobValue(b)) => serde_json::Value::Array(
                b.iter().map(|byte| serde_json::Value::from(*byte)).collect(),
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
