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

    let addr = "0.0.0.0:50051".parse()?;

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

    let engine = Arc::new(BpmnLiteEngine::new(store));

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
