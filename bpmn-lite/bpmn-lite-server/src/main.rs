use std::sync::Arc;

use bpmn_lite_core::engine::BpmnLiteEngine;
use bpmn_lite_core::store::ProcessStore;
use bpmn_lite_core::store_memory::MemoryStore;
use bpmn_lite_server::grpc::proto::bpmn_lite_server::BpmnLiteServer;
use bpmn_lite_server::grpc::BpmnLiteService;
use tonic::transport::Server;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let addr = parse_bind_addr().parse()?;

    let database_url = parse_database_url();

    let store: Arc<dyn ProcessStore> = match database_url {
        #[cfg(feature = "postgres")]
        Some(url) => {
            tracing::info!("Connecting to PostgreSQL...");
            let pool = sqlx::PgPool::connect(&url).await?;
            let pg = bpmn_lite_core::store_postgres::PostgresProcessStore::new(pool);
            pg.migrate().await?;
            tracing::info!("Using PostgresProcessStore (migrations applied)");
            Arc::new(pg)
        }
        #[cfg(not(feature = "postgres"))]
        Some(_) => {
            tracing::warn!(
                "--database-url / DATABASE_URL set but postgres feature not enabled, using MemoryStore"
            );
            Arc::new(MemoryStore::new())
        }
        None => {
            tracing::info!("Using MemoryStore (no database URL configured)");
            Arc::new(MemoryStore::new())
        }
    };

    let engine = Arc::new(BpmnLiteEngine::new(store.clone()));

    // Background: reclaim stale claimed jobs (every 60s, 5min timeout)
    let reclaim_store = store.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            match reclaim_store.reclaim_stale_jobs(5 * 60 * 1000).await {
                Ok(n) if n > 0 => tracing::warn!(reclaimed = n, "Reclaimed stale jobs"),
                Err(e) => tracing::error!(error = %e, "Job reclaim failed"),
                _ => {}
            }
        }
    });

    // Background: tick all running instances (every 500ms)
    let tick_engine = engine.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if let Err(e) = tick_engine.tick_all().await {
                tracing::error!(error = %e, "tick_all failed");
            }
        }
    });

    // Background: prune dedupe cache (hourly, 24h TTL)
    let prune_store = store.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            match prune_store.prune_dedupe_cache(24 * 3600 * 1000).await {
                Ok(n) if n > 0 => tracing::info!(pruned = n, "Pruned dedupe cache"),
                Err(e) => tracing::error!(error = %e, "Dedupe prune failed"),
                _ => {}
            }
        }
    });

    tracing::info!("BPMN-Lite gRPC server listening on {}", addr);

    let service = BpmnLiteService {
        engine: engine.clone(),
    };

    Server::builder()
        .add_service(BpmnLiteServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

/// Parse database URL from `--database-url <url>` CLI arg or `DATABASE_URL` env var.
fn parse_database_url() -> Option<String> {
    // CLI arg takes precedence
    let args: Vec<String> = std::env::args().collect();
    if let Some(url) = args
        .windows(2)
        .find(|w| w[0] == "--database-url")
        .map(|w| w[1].clone())
    {
        return Some(url);
    }
    // Fall back to env var
    std::env::var("DATABASE_URL").ok()
}

/// Parse bind address from `--bind <addr>` CLI arg or `BPMN_LITE_BIND` env var.
fn parse_bind_addr() -> String {
    let args: Vec<String> = std::env::args().collect();
    if let Some(addr) = args
        .windows(2)
        .find(|w| w[0] == "--bind")
        .map(|w| w[1].clone())
    {
        return addr;
    }

    std::env::var("BPMN_LITE_BIND").unwrap_or_else(|_| "0.0.0.0:50051".to_string())
}
