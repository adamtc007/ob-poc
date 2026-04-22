//! DSL MCP Server - Single path: refine → enrich → map to DSL → reject

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

use ob_poc::agent::learning::embedder::{CachedEmbedder, CandleEmbedder, Embedder};
use ob_poc::mcp::McpServer;

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");

    eprintln!("[dsl_mcp] Connecting...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    eprintln!("[dsl_mcp] Loading embedder...");
    let candle = CandleEmbedder::new()?;
    let embedder: Arc<dyn Embedder> = Arc::new(CachedEmbedder::new(Arc::new(candle)));

    // F1 fix (Slice 2.1): build the canonical SemOS plugin op registry and
    // thread it into the server so `dsl_execute` / `dsl_execute_submission`
    // can dispatch plugin verbs post-Phase-5c-migrate slice #80.
    let sem_os_ops = {
        let mut reg = sem_os_postgres::ops::build_registry();
        ob_poc::domain_ops::extend_registry(&mut reg);
        Arc::new(reg)
    };
    eprintln!(
        "[dsl_mcp] SemOsVerbOpRegistry initialised with {} plugin ops",
        sem_os_ops.len()
    );

    // F3 fix (Slice 2.2): drift-check the registry against every YAML plugin
    // verb BEFORE serving MCP traffic. Missing registrations → panic.
    {
        let missing = ob_poc::domain_ops::find_missing_plugin_ops(&sem_os_ops);
        if !missing.is_empty() {
            panic!(
                "FATAL: {} YAML plugin verb(s) have no SemOsVerbOp registered. \
                 Missing FQNs: {:?}",
                missing.len(),
                missing
            );
        }
    }

    let server = McpServer::new(pool, embedder).with_sem_os_ops(sem_os_ops);
    server.run().await
}
