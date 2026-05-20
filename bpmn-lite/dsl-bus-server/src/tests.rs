//! End-to-end coverage for [`BusServer`].
//!
//! Each test stands up a real tonic gRPC server on an ephemeral port,
//! connects with the protocol's generated client, and verifies that
//! the v0.6 §8.6 receive flow (idempotency, atomic inbox + outbox,
//! status mapping) holds.

use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dsl_bus_protocol::v1::invocation_service_client::InvocationServiceClient;
use dsl_bus_protocol::v1::result_service_client::ResultServiceClient;
use dsl_bus_protocol::v1::{
    ExecutionOutcome, ExecutionOutcomeKind, InvocationRequest, InvocationResult, ReceiptStatus,
    ResolvedBinding, SubmissionStatus, Uuid as ProtoUuid,
};
use dsl_bus_storage::{lookup_inbox, BusEndpoint};
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{
    InvocationContext, InvocationDispatcher, InvocationOutcome, ResultContext, ResultDispatcher,
};
use crate::{BusServer, BusServerError};

const DEFAULT_TEST_DATABASE_URL: &str = "postgresql://localhost/dsl_bus_test";

// ── Test harness ─────────────────────────────────────────────────────

async fn setup_pool() -> PgPool {
    let url = std::env::var("BPMN_LITE_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| DEFAULT_TEST_DATABASE_URL.to_owned());
    let pool = PgPool::connect(&url).await.expect("connect");
    sqlx::migrate!("../dsl-bus-storage/migrations")
        .run(&pool)
        .await
        .expect("migrations");
    sqlx::query("TRUNCATE outbox").execute(&pool).await.unwrap();
    sqlx::query("TRUNCATE inbox").execute(&pool).await.unwrap();
    pool
}

fn ephemeral_addr() -> SocketAddr {
    // Bind / drop a std listener to learn a free port; tonic will rebind it.
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    drop(l);
    addr
}

/// Connect with a short retry loop — `BusServer::serve` spawns the tonic
/// listener on a background task, and the first connect can land before
/// it's accepting. Five attempts at 30 ms is more than enough.
async fn connect_invocation_client(
    addr: SocketAddr,
) -> InvocationServiceClient<tonic::transport::Channel> {
    let url = format!("http://{addr}");
    for _ in 0..20 {
        if let Ok(client) = InvocationServiceClient::connect(url.clone()).await {
            return client;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    InvocationServiceClient::connect(url)
        .await
        .expect("connect to bus server")
}

async fn connect_result_client(
    addr: SocketAddr,
) -> ResultServiceClient<tonic::transport::Channel> {
    let url = format!("http://{addr}");
    for _ in 0..20 {
        if let Ok(client) = ResultServiceClient::connect(url.clone()).await {
            return client;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    ResultServiceClient::connect(url)
        .await
        .expect("connect to bus server")
}

fn proto_uuid(u: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: u.as_bytes().to_vec(),
    }
}

fn sample_request(idempotency_key: Uuid, verb_id: &str) -> InvocationRequest {
    InvocationRequest {
        idempotency_key: Some(proto_uuid(idempotency_key)),
        verb_id: verb_id.into(),
        inputs: vec![ResolvedBinding {
            name: "name".into(),
            value: Some(dsl_bus_protocol::v1::TypedValue {
                value: Some(dsl_bus_protocol::v1::typed_value::Value::StringValue(
                    "Allianz".into(),
                )),
                type_name: "String".into(),
            }),
        }],
        authority: None,
        source_domain: "bpmn-lite".into(),
        catalogue_version: "v1.0.0".into(),
        snapshot_pin: None,
        result_callback_endpoint: "http://bpmn-lite/result".into(),
        timeout_at: None,
    }
}

// ── Dispatchers ──────────────────────────────────────────────────────

struct AcceptingInvocationDispatcher {
    calls: Arc<AtomicU32>,
    /// Optional override that lets a test inject typed failures.
    next_error: Arc<Mutex<Option<BusServerError>>>,
    /// Optional override of the catalogue_version expected.
    require_version: Option<String>,
    seen_contexts: Arc<Mutex<Vec<InvocationContext>>>,
}

impl AcceptingInvocationDispatcher {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicU32::new(0)),
            next_error: Arc::new(Mutex::new(None)),
            require_version: None,
            seen_contexts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl InvocationDispatcher for AcceptingInvocationDispatcher {
    async fn dispatch(
        &self,
        ctx: InvocationContext,
        _inputs: Vec<ResolvedBinding>,
    ) -> Result<InvocationOutcome, BusServerError> {
        if let Some(required) = &self.require_version
            && &ctx.catalogue_version != required
        {
            return Err(BusServerError::VersionIncompatible(format!(
                "expected {required}, got {}",
                ctx.catalogue_version
            )));
        }
        if let Some(err) = self.next_error.lock().unwrap().take() {
            return Err(err);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen_contexts.lock().unwrap().push(ctx);

        Ok(InvocationOutcome {
            execution_id: Uuid::now_v7(),
            outcome: ExecutionOutcome {
                kind: ExecutionOutcomeKind::Committed as i32,
                detail: "ok".into(),
                bindings: vec![],
            },
        })
    }
}

struct RecordingResultDispatcher {
    calls: Arc<AtomicU32>,
}

impl RecordingResultDispatcher {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[async_trait]
impl ResultDispatcher for RecordingResultDispatcher {
    async fn dispatch(
        &self,
        _ctx: ResultContext,
        _outcome: ExecutionOutcome,
    ) -> Result<(), BusServerError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn invocation_round_trip_inserts_inbox_and_enqueues_result_outbox() {
    let pool = setup_pool().await;
    let inv = AcceptingInvocationDispatcher::new();
    let inv_calls = inv.calls.clone();
    let seen = inv.seen_contexts.clone();
    let res = RecordingResultDispatcher::new();

    let server = BusServer::builder()
        .pool(pool.clone())
        .local_domain("ob-poc")
        .invocation_dispatcher(inv)
        .result_dispatcher(res)
        .bind(ephemeral_addr())
        .build();
    let handle = server.serve().await.unwrap();

    let mut client = connect_invocation_client(handle.local_addr()).await;

    let key = Uuid::now_v7();
    let resp = client
        .submit(sample_request(key, "ob-poc:cbu.create"))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.status, SubmissionStatus::Accepted as i32);
    assert!(resp.execution_id.is_some());
    assert_eq!(inv_calls.load(Ordering::SeqCst), 1);

    // Dispatcher saw the stripped verb id and the carried source domain.
    let ctx = seen.lock().unwrap().first().unwrap().clone();
    assert_eq!(ctx.local_verb_id, "cbu.create");
    assert_eq!(ctx.source_domain, "bpmn-lite");

    // Inbox row exists; outbox carries a result for the caller.
    let inbox_row = lookup_inbox(&pool, key).await.unwrap().expect("inbox row");
    assert_eq!(inbox_row.endpoint, BusEndpoint::Invocation);
    assert!(inbox_row.execution_id.is_some());

    let outbox_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM outbox WHERE idempotency_key = $1 AND target_endpoint = 'result'",
    )
    .bind(key)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outbox_count, 1);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn duplicate_invocation_returns_cached_execution_id_without_redispatch() {
    let pool = setup_pool().await;
    let inv = AcceptingInvocationDispatcher::new();
    let inv_calls = inv.calls.clone();
    let res = RecordingResultDispatcher::new();

    let server = BusServer::builder()
        .pool(pool.clone())
        .local_domain("ob-poc")
        .invocation_dispatcher(inv)
        .result_dispatcher(res)
        .bind(ephemeral_addr())
        .build();
    let handle = server.serve().await.unwrap();
    let mut client = connect_invocation_client(handle.local_addr()).await;

    let key = Uuid::now_v7();
    let first = client
        .submit(sample_request(key, "ob-poc:cbu.create"))
        .await
        .unwrap()
        .into_inner();
    let second = client
        .submit(sample_request(key, "ob-poc:cbu.create"))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(first.status, SubmissionStatus::Accepted as i32);
    assert_eq!(second.status, SubmissionStatus::Duplicate as i32);
    assert_eq!(first.execution_id, second.execution_id);
    assert_eq!(inv_calls.load(Ordering::SeqCst), 1, "dispatcher must not run twice");

    handle.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn unknown_verb_returns_rejected_verb_unknown() {
    let pool = setup_pool().await;
    let inv = AcceptingInvocationDispatcher::new();
    *inv.next_error.lock().unwrap() = Some(BusServerError::UnknownVerb("no such verb".into()));
    let res = RecordingResultDispatcher::new();

    let server = BusServer::builder()
        .pool(pool.clone())
        .local_domain("ob-poc")
        .invocation_dispatcher(inv)
        .result_dispatcher(res)
        .bind(ephemeral_addr())
        .build();
    let handle = server.serve().await.unwrap();
    let mut client = connect_invocation_client(handle.local_addr()).await;

    let resp = client
        .submit(sample_request(Uuid::now_v7(), "ob-poc:cbu.nope"))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.status, SubmissionStatus::RejectedVerbUnknown as i32);
    assert!(resp.detail.contains("no such verb"));

    handle.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn version_mismatch_returns_rejected_version_incompatible() {
    let pool = setup_pool().await;
    let mut inv = AcceptingInvocationDispatcher::new();
    inv.require_version = Some("v9.9.9".into());
    let res = RecordingResultDispatcher::new();

    let server = BusServer::builder()
        .pool(pool.clone())
        .local_domain("ob-poc")
        .invocation_dispatcher(inv)
        .result_dispatcher(res)
        .bind(ephemeral_addr())
        .build();
    let handle = server.serve().await.unwrap();
    let mut client = connect_invocation_client(handle.local_addr()).await;

    let resp = client
        .submit(sample_request(Uuid::now_v7(), "ob-poc:cbu.create"))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        resp.status,
        SubmissionStatus::RejectedVersionIncompatible as i32
    );

    handle.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn missing_idempotency_key_returns_rejected_malformed() {
    let pool = setup_pool().await;
    let inv = AcceptingInvocationDispatcher::new();
    let res = RecordingResultDispatcher::new();

    let server = BusServer::builder()
        .pool(pool.clone())
        .local_domain("ob-poc")
        .invocation_dispatcher(inv)
        .result_dispatcher(res)
        .bind(ephemeral_addr())
        .build();
    let handle = server.serve().await.unwrap();
    let mut client = connect_invocation_client(handle.local_addr()).await;

    let mut req = sample_request(Uuid::now_v7(), "ob-poc:cbu.create");
    req.idempotency_key = None;

    let resp = client.submit(req).await.unwrap().into_inner();
    assert_eq!(resp.status, SubmissionStatus::RejectedMalformed as i32);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn deliver_result_invokes_dispatcher_and_records_inbox() {
    let pool = setup_pool().await;
    let inv = AcceptingInvocationDispatcher::new();
    let res = RecordingResultDispatcher::new();
    let res_calls = res.calls.clone();

    let server = BusServer::builder()
        .pool(pool.clone())
        .local_domain("bpmn-lite")
        .invocation_dispatcher(inv)
        .result_dispatcher(res)
        .bind(ephemeral_addr())
        .build();
    let handle = server.serve().await.unwrap();
    let mut client = connect_result_client(handle.local_addr()).await;

    let key = Uuid::now_v7();
    let exec = Uuid::now_v7();
    let req = InvocationResult {
        execution_id: Some(proto_uuid(exec)),
        idempotency_key: Some(proto_uuid(key)),
        outcome: Some(ExecutionOutcome {
            kind: ExecutionOutcomeKind::Committed as i32,
            detail: "ok".into(),
            bindings: vec![],
        }),
        source_domain: "ob-poc".into(),
        executed_at: None,
        plan_id: None,
        audit_reference: "audit://ob-poc/run".into(),
    };

    let resp = client.deliver_result(req).await.unwrap().into_inner();
    assert_eq!(resp.status, ReceiptStatus::Received as i32);
    assert_eq!(res_calls.load(Ordering::SeqCst), 1);

    let inbox_row = lookup_inbox(&pool, key).await.unwrap().expect("inbox row");
    assert_eq!(inbox_row.endpoint, BusEndpoint::Result);
    assert_eq!(inbox_row.execution_id, Some(exec));

    handle.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn duplicate_deliver_result_returns_duplicate_ignored() {
    let pool = setup_pool().await;
    let inv = AcceptingInvocationDispatcher::new();
    let res = RecordingResultDispatcher::new();
    let res_calls = res.calls.clone();

    let server = BusServer::builder()
        .pool(pool.clone())
        .local_domain("bpmn-lite")
        .invocation_dispatcher(inv)
        .result_dispatcher(res)
        .bind(ephemeral_addr())
        .build();
    let handle = server.serve().await.unwrap();
    let mut client = connect_result_client(handle.local_addr()).await;

    let key = Uuid::now_v7();
    let exec = Uuid::now_v7();
    let make_req = || InvocationResult {
        execution_id: Some(proto_uuid(exec)),
        idempotency_key: Some(proto_uuid(key)),
        outcome: Some(ExecutionOutcome {
            kind: ExecutionOutcomeKind::Committed as i32,
            detail: "ok".into(),
            bindings: vec![],
        }),
        source_domain: "ob-poc".into(),
        executed_at: None,
        plan_id: None,
        audit_reference: String::new(),
    };

    let _ = client.deliver_result(make_req()).await.unwrap();
    let dup = client.deliver_result(make_req()).await.unwrap().into_inner();
    assert_eq!(dup.status, ReceiptStatus::DuplicateIgnored as i32);
    assert_eq!(res_calls.load(Ordering::SeqCst), 1, "dispatcher must not re-run");

    handle.shutdown().await.unwrap();
}

#[test]
fn strip_domain_prefix_unit() {
    use crate::services::strip_domain_prefix;
    assert_eq!(strip_domain_prefix("ob-poc:cbu.create"), "cbu.create");
    assert_eq!(strip_domain_prefix("cbu.create"), "cbu.create");
    assert_eq!(
        strip_domain_prefix("dmn-lite:cbu_type_routing"),
        "cbu_type_routing"
    );
}
