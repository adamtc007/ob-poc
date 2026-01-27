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

    let server = McpServer::new(pool, embedder);
    server.run().await
}
