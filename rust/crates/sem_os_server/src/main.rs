//! sem_os_server — standalone REST server for Semantic OS.
//!
//! Reads config from env vars:
//!   SEM_OS_DATABASE_URL — Postgres connection string (required)
//!   SEM_OS_JWT_SECRET   — JWT HMAC secret (required)
//!   SEM_OS_BIND_ADDR    — listen address (default: 0.0.0.0:4100)

use std::sync::Arc;
use std::time::Duration;

use sem_os_core::service::CoreServiceImpl;
use sem_os_postgres::PgStores;
use sem_os_server::dispatcher::OutboxDispatcher;
use sem_os_server::middleware::jwt::JwtConfig;
use sem_os_server::router::build_router;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sem_os_server=debug".into()),
        )
        .init();

    // Read config from environment
    let database_url =
        std::env::var("SEM_OS_DATABASE_URL").expect("SEM_OS_DATABASE_URL must be set");
    let jwt_secret = std::env::var("SEM_OS_JWT_SECRET").expect("SEM_OS_JWT_SECRET must be set");
    let bind_addr = std::env::var("SEM_OS_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:4100".into());

    // Create PgPool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    tracing::info!("Connected to database");

    // Build port implementations — Arc-wrap outbox + projections so they're
    // shared between the CoreService and the OutboxDispatcher.
    let stores = PgStores::new(pool.clone());

    let outbox: Arc<dyn sem_os_core::ports::OutboxStore> = Arc::new(stores.outbox);
    let projections: Arc<dyn sem_os_core::ports::ProjectionWriter> = Arc::new(stores.projections);

    // Build core service (with changeset store wired)
    let service: Arc<dyn sem_os_core::service::CoreService> = Arc::new(
        CoreServiceImpl::new(
            Arc::new(stores.snapshots),
            Arc::new(stores.objects),
            Arc::new(stores.audit),
            Arc::clone(&outbox),
            Arc::new(stores.evidence),
            Arc::clone(&projections),
        )
        .with_changesets(Arc::new(stores.changesets)),
    );

    // Start outbox dispatcher as background task
    let dispatcher_interval_ms: u64 = std::env::var("SEM_OS_DISPATCHER_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);
    let dispatcher_max_fails: u32 = std::env::var("SEM_OS_DISPATCHER_MAX_FAILS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    let dispatcher = OutboxDispatcher::new(
        Arc::clone(&outbox),
        Arc::clone(&projections),
        Duration::from_millis(dispatcher_interval_ms),
        dispatcher_max_fails,
    );

    tokio::spawn(async move {
        dispatcher.run().await;
    });
    tracing::info!(
        "OutboxDispatcher spawned (interval={}ms, max_fails={})",
        dispatcher_interval_ms,
        dispatcher_max_fails
    );

    // Build JWT config
    let jwt_config = JwtConfig::from_secret(jwt_secret.as_bytes());

    // Build router
    let app = build_router(service, pool, jwt_config);

    // Bind and serve
    let listener = TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind to {bind_addr}: {e}"));
    tracing::info!("sem_os_server listening on {bind_addr}");

    axum::serve(listener, app).await.expect("server error");
}
