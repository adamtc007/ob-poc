//! End-to-end tests for [`BusClient`] against an in-process mock gRPC
//! server.
//!
//! All tests are `#[ignore]` because they touch a real Postgres
//! (`BPMN_LITE_TEST_DATABASE_URL=postgresql://localhost/dsl_bus_test`)
//! and bind ephemeral TCP sockets.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dsl_bus_protocol::v1::invocation_service_server::{
    InvocationService, InvocationServiceServer,
};
use dsl_bus_protocol::v1::result_service_server::{ResultService, ResultServiceServer};
use dsl_bus_protocol::v1::{
    InvocationRequest, InvocationResult, ReceiptStatus, ResultAck, SubmissionAck,
    SubmissionStatus, Uuid as ProtoUuid,
};
use dsl_bus_storage::InsertOutcome;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

use crate::sender::exp_backoff_secs;
use crate::BusClient;

const DEFAULT_TEST_DATABASE_URL: &str = "postgresql://localhost/dsl_bus_test";

// ── Mock gRPC server ─────────────────────────────────────────────────

/// Configurable mock that records every Submit / DeliverResult call.
#[derive(Default)]
struct MockServiceState {
    /// 0 = accept and return new exec_id; otherwise tonic::Status::internal.
    fail_count: AtomicU32,
    invocations_received: AtomicU32,
    results_received: AtomicU32,
}

#[derive(Clone)]
struct MockService {
    state: Arc<MockServiceState>,
}

#[tonic::async_trait]
impl InvocationService for MockService {
    async fn submit(
        &self,
        _req: Request<InvocationRequest>,
    ) -> Result<Response<SubmissionAck>, Status> {
        // If `fail_count` is non-zero, decrement and fail. Lets tests
        // pre-load N failures before accepting.
        if self
            .state
            .fail_count
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                if v > 0 { Some(v - 1) } else { None }
            })
            .is_ok()
        {
            return Err(Status::internal("mock-failure"));
        }

        self.state.invocations_received.fetch_add(1, Ordering::SeqCst);
        let exec_bytes = Uuid::now_v7().as_bytes().to_vec();
        Ok(Response::new(SubmissionAck {
            execution_id: Some(ProtoUuid { value: exec_bytes }),
            status: SubmissionStatus::Accepted as i32,
            detail: String::new(),
        }))
    }
}

#[tonic::async_trait]
impl ResultService for MockService {
    async fn deliver_result(
        &self,
        _req: Request<InvocationResult>,
    ) -> Result<Response<ResultAck>, Status> {
        self.state.results_received.fetch_add(1, Ordering::SeqCst);
        Ok(Response::new(ResultAck {
            status: ReceiptStatus::Received as i32,
            detail: String::new(),
        }))
    }
}

struct MockServer {
    state: Arc<MockServiceState>,
    addr: SocketAddr,
    shutdown: oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
}

impl MockServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(MockServiceState::default());
        let service = MockService {
            state: state.clone(),
        };

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        let server = Server::builder()
            .add_service(InvocationServiceServer::new(service.clone()))
            .add_service(ResultServiceServer::new(service))
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
            });

        let join = tokio::spawn(async move {
            let _ = server.await;
        });

        // Brief settle so the listener is accepting before the first connect.
        tokio::time::sleep(Duration::from_millis(20)).await;

        Self {
            state,
            addr,
            shutdown: shutdown_tx,
            join,
        }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    async fn stop(self) {
        let _ = self.shutdown.send(());
        let _ = self.join.await;
    }
}

// ── Test helpers ─────────────────────────────────────────────────────

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

fn proto_uuid(u: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: u.as_bytes().to_vec(),
    }
}

fn sample_request(idempotency_key: Uuid) -> InvocationRequest {
    InvocationRequest {
        idempotency_key: Some(proto_uuid(idempotency_key)),
        verb_id: "cbu.create".into(),
        inputs: vec![],
        authority: None,
        source_domain: String::new(),
        catalogue_version: "v1.0.0".into(),
        snapshot_pin: None,
        result_callback_endpoint: String::new(),
        timeout_at: None,
    }
}

async fn fetch_outbox_status(pool: &PgPool, id: Uuid) -> (String, i32, Option<String>) {
    use sqlx::Row as _;
    let row = sqlx::query(
        "SELECT status, attempt_count, last_error FROM outbox WHERE idempotency_key = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    (row.get("status"), row.get("attempt_count"), row.get("last_error"))
}

// ── Tests ────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn submit_invocation_writes_outbox_row() {
    let pool = setup_pool().await;
    let client = BusClient::builder()
        .pool(pool.clone())
        .local_domain("bpmn-lite")
        .add_peer("ob-poc", "http://127.0.0.1:1") // unused; we never start sender
        .build()
        .await
        .unwrap();

    let key = Uuid::now_v7();
    let (returned_key, outcome) = client
        .submit_invocation("ob-poc", sample_request(key))
        .await
        .unwrap();
    assert_eq!(returned_key, key);
    assert_eq!(outcome, InsertOutcome::Inserted);

    let (status, attempt, _err) = fetch_outbox_status(&pool, key).await;
    assert_eq!(status, "pending");
    assert_eq!(attempt, 0);
}

#[tokio::test]
#[ignore]
async fn submit_invocation_is_idempotent_on_repeat_keys() {
    let pool = setup_pool().await;
    let client = BusClient::builder()
        .pool(pool.clone())
        .local_domain("bpmn-lite")
        .add_peer("ob-poc", "http://127.0.0.1:1")
        .build()
        .await
        .unwrap();

    let key = Uuid::now_v7();
    let first = client
        .submit_invocation("ob-poc", sample_request(key))
        .await
        .unwrap();
    let second = client
        .submit_invocation("ob-poc", sample_request(key))
        .await
        .unwrap();

    assert_eq!(first.1, InsertOutcome::Inserted);
    assert_eq!(second.1, InsertOutcome::Duplicate);
}

#[tokio::test]
#[ignore]
async fn submit_invocation_rejects_unknown_peer() {
    let pool = setup_pool().await;
    let client = BusClient::builder()
        .pool(pool.clone())
        .local_domain("bpmn-lite")
        .build()
        .await
        .unwrap();

    let key = Uuid::now_v7();
    let err = client
        .submit_invocation("ob-poc", sample_request(key))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        crate::BusClientError::UnknownPeer(d) if d == "ob-poc"
    ));
}

#[tokio::test]
#[ignore]
async fn sender_dispatches_pending_row_to_mock_server() {
    let pool = setup_pool().await;
    let server = MockServer::start().await;
    let client = BusClient::builder()
        .pool(pool.clone())
        .local_domain("bpmn-lite")
        .add_peer("ob-poc", server.url())
        .sender_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let key = Uuid::now_v7();
    client
        .submit_invocation("ob-poc", sample_request(key))
        .await
        .unwrap();

    let handle = client.start_sender();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let (status, _, _) = fetch_outbox_status(&pool, key).await;
        if status == "submitted" {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("outbox row never transitioned to submitted (got {status})");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert_eq!(server.state.invocations_received.load(Ordering::SeqCst), 1);
    let stats = handle.stats();
    assert!(stats.submitted() >= 1);

    handle.shutdown().await.unwrap();
    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn sender_retries_on_transport_failure_then_succeeds() {
    let pool = setup_pool().await;
    let server = MockServer::start().await;
    // Pre-load two failures before the mock accepts.
    server.state.fail_count.store(2, Ordering::SeqCst);

    let client = BusClient::builder()
        .pool(pool.clone())
        .local_domain("bpmn-lite")
        .add_peer("ob-poc", server.url())
        .sender_interval(Duration::from_millis(30))
        .max_backoff_secs(1) // keep tests fast: cap backoff at 1s
        .build()
        .await
        .unwrap();

    let key = Uuid::now_v7();
    client
        .submit_invocation("ob-poc", sample_request(key))
        .await
        .unwrap();

    let handle = client.start_sender();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let (status, _, _) = fetch_outbox_status(&pool, key).await;
        if status == "submitted" {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("outbox row stuck in '{status}' after retries");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let stats = handle.stats();
    assert!(
        stats.retried() >= 2,
        "expected at least 2 retries, got {}",
        stats.retried()
    );
    assert_eq!(server.state.invocations_received.load(Ordering::SeqCst), 1);

    handle.shutdown().await.unwrap();
    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn sender_dispatches_result_rows() {
    let pool = setup_pool().await;
    let server = MockServer::start().await;
    let client = BusClient::builder()
        .pool(pool.clone())
        .local_domain("ob-poc")
        .add_peer("bpmn-lite", server.url())
        .sender_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let key = Uuid::now_v7();
    let result = InvocationResult {
        execution_id: Some(proto_uuid(Uuid::now_v7())),
        idempotency_key: Some(proto_uuid(key)),
        outcome: None,
        source_domain: "ob-poc".into(),
        executed_at: None,
        plan_id: None,
        audit_reference: String::new(),
    };
    client.send_result("bpmn-lite", result).await.unwrap();

    let handle = client.start_sender();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let (status, _, _) = fetch_outbox_status(&pool, key).await;
        if status == "submitted" {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("result row never transitioned to submitted");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(server.state.results_received.load(Ordering::SeqCst), 1);

    handle.shutdown().await.unwrap();
    server.stop().await;
}

#[test]
fn backoff_grows_exponentially_and_caps() {
    assert_eq!(exp_backoff_secs(0, 60), 1);
    assert_eq!(exp_backoff_secs(1, 60), 2);
    assert_eq!(exp_backoff_secs(2, 60), 4);
    assert_eq!(exp_backoff_secs(3, 60), 8);
    assert_eq!(exp_backoff_secs(5, 60), 32);
    assert_eq!(exp_backoff_secs(6, 60), 60); // 64 capped to 60
    assert_eq!(exp_backoff_secs(30, 60), 60); // would overflow without saturation
    assert_eq!(exp_backoff_secs(40, 60), 60);
}

