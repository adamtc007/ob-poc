//! Federated DSL bus runtime for `bpmn-lite-server` (v0.6 §T2B.9
//! item 34, co-located with the existing 50051 BPMN gRPC service).
//!
//! Owns three things at startup:
//!
//! 1. `BusClient` — outbox writer + sender task. bpmn-lite is the
//!    *workflow* domain so most outbox rows are `submit_invocation`
//!    payloads to ob-poc / dmn-lite.
//! 2. `BusServer` — receives `DeliverResult` payloads from peer
//!    domains. The accompanying `RejectInvocationDispatcher` rejects
//!    any inbound Submit (workflow domain doesn't host verbs).
//! 3. `StoreBackedAdvancer` — the v0.6 §T2B.9 "real-store-layer"
//!    ProcessAdvancer. It deletes the matching pending row via
//!    `take_by_execution_id` and transitions the process instance's
//!    status. **It does not walk to the next BPMN node** — that lives
//!    in T3 (RIP-AND-REPLACE of the executor). A structured event is
//!    emitted so T3 can pick up the handoff.

#![cfg(feature = "postgres")]

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bpmn_lite_bus_handler::{
    BpmnLiteBusHandler, ProcessAdvanceInput, ProcessAdvancer, ProcessAdvancerError,
    RejectInvocationDispatcher,
};
use bpmn_lite_store::pending::PendingInvocationStore;
use bpmn_lite_store::process_instance::{
    BpmnProcessInstanceStore, ProcessStatus,
};
use bpmn_lite_store_postgres::{
    PostgresBpmnProcessInstanceStore, PostgresPendingInvocationStore,
};
use dsl_bus_client::BusClient;
use dsl_bus_protocol::v1::ExecutionOutcomeKind;
use dsl_bus_server::{BusServer, ServerHandle};
use sqlx::PgPool;

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
    pub(crate) bind_addr: SocketAddr,
    pub(crate) peers: Vec<(String, String)>,
}

/// Stand up the bus runtime: apply bus migrations, build the advancer
/// against the bpmn-lite-store-postgres backends, spawn the outbox
/// sender, bind the bus server. Returns a handle owning the
/// background lifecycle.
pub(crate) async fn start(config: BusRuntimeConfig) -> anyhow::Result<BusRuntime> {
    dsl_bus_storage::migrate(&config.pool).await?;

    let mut builder = BusClient::builder()
        .pool(config.pool.clone())
        .local_domain("bpmn-lite");
    for (domain, uri) in &config.peers {
        builder = builder.add_peer(domain.clone(), uri.clone());
    }
    let client = builder.build().await?;
    let notifier = client.outbox_notifier();
    let sender = client.start_sender();

    let advancer = StoreBackedAdvancer {
        pending: Arc::new(PostgresPendingInvocationStore::new(config.pool.clone())),
        instances: Arc::new(PostgresBpmnProcessInstanceStore::new(config.pool.clone())),
    };

    // A3 §3.4 / §6 — bpmn-lite is a workflow domain. It exposes
    // InvocationService (Submit rejects; Validate is the stub provided
    // by dsl-bus-server) and ResultService. No EntityService — bpmn-lite
    // doesn't host entity state. No SemOsService — process instances
    // aren't a semantic catalogue.
    let server = BusServer::builder()
        .pool(config.pool)
        .local_domain("bpmn-lite")
        .invocation_dispatcher(RejectInvocationDispatcher)
        .result_dispatcher(BpmnLiteBusHandler::new(advancer))
        .outbox_notifier(notifier)
        .bind(config.bind_addr)
        .build()
        .serve()
        .await?;

    tracing::info!(
        bind_addr = %server.local_addr(),
        "bpmn-lite bus server listening (result receiver)"
    );

    Ok(BusRuntime { server, sender })
}

/// "Real-store-layer" advancer for T2B.9.
///
/// `advance()` consumes the pending row keyed by `execution_id`,
/// transitions the matching process instance to a sensible status, and
/// records a structured event flagging the T3 executor handoff. The
/// next-node walker itself is T3 territory.
struct StoreBackedAdvancer {
    pending: Arc<PostgresPendingInvocationStore>,
    instances: Arc<PostgresBpmnProcessInstanceStore>,
}

#[async_trait]
impl ProcessAdvancer for StoreBackedAdvancer {
    async fn advance(&self, input: ProcessAdvanceInput) -> Result<(), ProcessAdvancerError> {
        let row = self
            .pending
            .take_by_execution_id(input.execution_id)
            .await
            .map_err(|e| ProcessAdvancerError::Internal(format!("take pending: {e}")))?;

        let Some(row) = row else {
            return Err(ProcessAdvancerError::UnknownExecution(input.execution_id));
        };

        let mut instance = self
            .instances
            .load(row.process_instance_id)
            .await
            .map_err(|e| ProcessAdvancerError::Internal(format!("load process instance: {e}")))?
            .ok_or_else(|| {
                ProcessAdvancerError::Internal(format!(
                    "pending row referenced unknown process instance {}",
                    row.process_instance_id
                ))
            })?;

        let now = chrono::Utc::now();
        instance.waiting_on_callout_id = None;
        instance.waiting_on_execution_id = None;
        instance.last_advanced_at = now;

        let (next_status, completed) = match input.outcome_kind {
            ExecutionOutcomeKind::Committed | ExecutionOutcomeKind::IdempotentReplayReturned => {
                // The verb completed cleanly — release the wait but leave
                // the process in `Running` for T3's executor to pick up
                // and walk the next node.
                (ProcessStatus::Running, false)
            }
            ExecutionOutcomeKind::VerbFailed
            | ExecutionOutcomeKind::AuthorityDenied
            | ExecutionOutcomeKind::TimedOut
            | ExecutionOutcomeKind::PanicRecovered
            | ExecutionOutcomeKind::RejectedByAdmission
            | ExecutionOutcomeKind::VersionMismatch
            | ExecutionOutcomeKind::Cancelled => {
                instance.completed_at = Some(now);
                instance.end_status = Some(format!("Failed:{:?}", input.outcome_kind));
                instance.failure_reason = Some(input.outcome_detail.clone());
                (ProcessStatus::Failed, true)
            }
            ExecutionOutcomeKind::OptimisticConflict | ExecutionOutcomeKind::LockTimeout => {
                // Transient — T3's retry policy will reschedule. For now
                // bounce back to Running so the next tick re-tries.
                (ProcessStatus::Running, false)
            }
            ExecutionOutcomeKind::OutcomeUnspecified => {
                return Err(ProcessAdvancerError::Malformed(
                    "ExecutionOutcomeKind::OutcomeUnspecified — peer must populate kind"
                        .to_owned(),
                ));
            }
        };
        instance.status = next_status;

        self.instances
            .update(instance)
            .await
            .map_err(|e| ProcessAdvancerError::Internal(format!("update process instance: {e}")))?;

        tracing::info!(
            execution_id = %input.execution_id,
            callout_id = %row.callout_id,
            process_instance_id = %row.process_instance_id,
            node_id = %row.node_id,
            source_domain = %input.source_domain,
            outcome = ?input.outcome_kind,
            next_status = next_status.as_str(),
            completed,
            "T3-handoff: result received, process status updated; executor walker is T3 scope"
        );
        Ok(())
    }
}
