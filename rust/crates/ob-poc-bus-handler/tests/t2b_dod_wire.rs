//! T2B master DoD wire-only scenarios (#43 idempotency + #46 version
//! mismatch).
//!
//! These exercise the **full** sender-side loop — `BusClient::submit_invocation`
//! → outbox row → sender task drain → tonic gRPC → `BusServer` →
//! `ObPocBusHandler` → recording `VerbExecutor`. The existing
//! `dsl-bus-server::tests` cover the same semantics by talking to the
//! gRPC server directly with a tonic client; here we go through
//! `BusClient` so the outbox + sender + retry-protocol is exercised
//! end-to-end before T3 builds the executor on top.
//!
//! Single-process by design — these are wire-protocol tests, not the
//! three-process deployment tests (which arrive with T2B-DoD-full +
//! T3). All marked `#[ignore]` because they need Postgres.

use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dsl_bus_client::BusClient;
use dsl_bus_protocol::v1::{
    ExecutionOutcomeKind, InvocationRequest, ResolvedBinding, TypedValue, Uuid as ProtoUuid,
};
use dsl_bus_server::BusServer;
use dsl_bus_storage::{lookup_inbox, InsertOutcome};
use ob_poc_bus_handler::{
    NoopResultDispatcher, ObPocBusHandler, VerbExecutor, VerbExecutorError, VerbOutcome,
};
use sqlx::PgPool;
use uuid::Uuid;

const DEFAULT_TEST_DATABASE_URL: &str = "postgresql://localhost/dsl_bus_test";
const TEST_CATALOGUE_VERSION: &str = "v1.0.0";

// ── Test harness ────────────────────────────────────────────────────

async fn setup_pool() -> PgPool {
    // Note: NO `TRUNCATE` here. Tests rely on UUIDv7 idempotency_keys
    // for isolation so they can run in parallel against the shared
    // `dsl_bus_test` database without wiping each other's rows.
    let url = std::env::var("BPMN_LITE_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| DEFAULT_TEST_DATABASE_URL.to_owned());
    let pool = PgPool::connect(&url).await.expect("connect");
    // Use the migrator exported by dsl-bus-storage — the migrations ship
    // inside the published crate, not at a fixed relative path on disk.
    dsl_bus_storage::migrate(&pool).await.expect("migrations");
    pool
}

fn ephemeral_addr() -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    drop(l);
    addr
}

/// Records every `execute()` call so the idempotency test can assert
/// the verb runs exactly once across two duplicate submissions.
struct RecordingExecutor {
    calls: Arc<AtomicU32>,
}

impl RecordingExecutor {
    fn new() -> (Self, Arc<AtomicU32>) {
        let calls = Arc::new(AtomicU32::new(0));
        (
            Self {
                calls: calls.clone(),
            },
            calls,
        )
    }
}

#[async_trait]
impl VerbExecutor for RecordingExecutor {
    async fn execute(
        &self,
        _local_verb_id: &str,
        _catalogue_version: &str,
        _inputs: Vec<ResolvedBinding>,
        _snapshot_pin: Option<Uuid>,
    ) -> Result<VerbOutcome, VerbExecutorError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(VerbOutcome {
            execution_id: Uuid::now_v7(),
            kind: ExecutionOutcomeKind::Committed,
            detail: "ok".into(),
            bindings: Vec::new(),
        })
    }
}

async fn spawn_server(
    pool: PgPool,
    catalogue_version: &str,
) -> (dsl_bus_server::ServerHandle, Arc<AtomicU32>, BusClient) {
    let (executor, calls) = RecordingExecutor::new();
    let handler = ObPocBusHandler::new(executor).with_catalogue_version(catalogue_version);
    let addr = ephemeral_addr();
    let server_url = format!("http://{addr}");

    // BusClient first — its OutboxNotifier is required by the server.
    let client = BusClient::builder()
        .pool(pool.clone())
        .local_domain("test-bpmn-lite")
        .add_peer("ob-poc", server_url.clone())
        .build()
        .await
        .expect("build BusClient");
    let notifier = client.outbox_notifier();

    let server = BusServer::builder()
        .pool(pool)
        .local_domain("ob-poc")
        .invocation_dispatcher(handler)
        .result_dispatcher(NoopResultDispatcher)
        .outbox_notifier(notifier)
        .bind(addr)
        .build()
        .serve()
        .await
        .expect("serve BusServer");

    (server, calls, client)
}

fn sample_request(idempotency_key: Uuid, catalogue_version: &str) -> InvocationRequest {
    InvocationRequest {
        idempotency_key: Some(ProtoUuid {
            value: idempotency_key.as_bytes().to_vec(),
        }),
        verb_id: "ob-poc:cbu.create".into(),
        inputs: vec![ResolvedBinding {
            name: "name".into(),
            value: Some(TypedValue {
                value: Some(dsl_bus_protocol::v1::typed_value::Value::StringValue(
                    "Allianz".into(),
                )),
                type_name: "String".into(),
            }),
        }],
        authority: None,
        source_domain: "test-bpmn-lite".into(),
        catalogue_version: catalogue_version.into(),
        snapshot_pin: None,
        result_callback_endpoint: format!("http://{}/result", ephemeral_addr()),
        timeout_at: None,
    }
}

/// Wait up to `timeout` for `pred()` to return `true`. The sender task
/// drains asynchronously; tests check observable state with a short
/// poll loop rather than a fixed sleep.
async fn wait_until<F, Fut>(timeout: Duration, mut pred: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if pred().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    pred().await
}

// ── #43 idempotency ─────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn dod_43_idempotency_same_key_submitted_twice_executes_verb_once() {
    let pool = setup_pool().await;
    let (server, exec_calls, client) = spawn_server(pool.clone(), TEST_CATALOGUE_VERSION).await;
    let sender = client.start_sender();

    let key = Uuid::now_v7();
    let req = sample_request(key, TEST_CATALOGUE_VERSION);

    let (first_key, first_outcome) = client
        .submit_invocation("ob-poc", req.clone(), "test-authority".to_string())
        .await
        .expect("first submit");
    assert_eq!(first_key, key);
    assert_eq!(first_outcome, InsertOutcome::Inserted);

    // Wait for the receiver to record the invocation in its inbox.
    let inbox_seen = wait_until(Duration::from_secs(3), || {
        let pool = pool.clone();
        async move {
            lookup_inbox(&pool, key)
                .await
                .map(|opt| opt.is_some())
                .unwrap_or(false)
        }
    })
    .await;
    assert!(
        inbox_seen,
        "first submit must reach the receiver and write inbox"
    );

    // Second submit with same key — the outbox UNIQUE constraint
    // surfaces Duplicate, no second row is enqueued.
    let (second_key, second_outcome) = client
        .submit_invocation("ob-poc", req, "test-authority".to_string())
        .await
        .expect("second submit");
    assert_eq!(second_key, key);
    assert_eq!(
        second_outcome,
        InsertOutcome::Duplicate,
        "outbox sender-side dedupe must surface Duplicate on resubmission"
    );

    // Give any in-flight redispatch a moment; the receiver-side inbox
    // dedupe would still catch it, but the sender shouldn't re-send.
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        exec_calls.load(Ordering::SeqCst),
        1,
        "VerbExecutor must be invoked exactly once across two duplicate submits"
    );

    let _ = sender.shutdown().await;
    server.shutdown().await.unwrap();
}

// ── #46 version mismatch ────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn dod_46_version_mismatch_rejects_with_rejected_version_incompatible() {
    // The handler is configured for "v1.0.0"; the client submits with
    // "v999.0.0". The dispatch path inside `ObPocBusHandler` must
    // reject with `VersionIncompatible`, which the bus server maps to
    // `SubmissionStatus::RejectedVersionIncompatible` on the wire.
    let pool = setup_pool().await;
    let (server, exec_calls, client) = spawn_server(pool.clone(), TEST_CATALOGUE_VERSION).await;
    let sender = client.start_sender();

    let key = Uuid::now_v7();
    let req = sample_request(key, "v999.0.0");

    let (_returned_key, outcome) = client
        .submit_invocation("ob-poc", req, "test-authority".to_string())
        .await
        .expect("submit accepted into outbox");
    assert_eq!(outcome, InsertOutcome::Inserted);

    // The sender will drain the row; the receiver rejects;
    // `mark_outbox_retry` records the failure. The outbox row's last
    // `attempt_count` should rise without ever transitioning to
    // `submitted`. Verify by watching the row.
    let saw_rejection = wait_until(Duration::from_secs(3), || {
        let pool = pool.clone();
        async move {
            let row: Option<(String, i32, Option<String>)> = sqlx::query_as(
                "SELECT status, attempt_count, last_error \
                   FROM outbox WHERE idempotency_key = $1",
            )
            .bind(key)
            .fetch_optional(&pool)
            .await
            .unwrap();
            matches!(
                row,
                Some((ref status, _, Some(ref err)))
                    if status == "pending"
                        && (err.contains("version") || err.contains("Version"))
            )
        }
    })
    .await;
    assert!(
        saw_rejection,
        "outbox row must record the version-mismatch rejection in last_error"
    );
    assert_eq!(
        exec_calls.load(Ordering::SeqCst),
        0,
        "VerbExecutor must NOT be invoked when version is rejected"
    );

    let _ = sender.shutdown().await;
    server.shutdown().await.unwrap();
}
