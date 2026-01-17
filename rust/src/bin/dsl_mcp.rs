//! DSL MCP Server Binary
//!
//! Runs the DSL MCP server for Claude integration.
//!
//! ## Usage
//!
//! ```bash
//! DATABASE_URL=postgresql://localhost/ob-poc ./target/debug/dsl_mcp
//! ```
//!
//! ## Environment Variables
//!
//! - `DATABASE_URL` (required): PostgreSQL connection string
//! - `OPENAI_API_KEY` (optional): Enables semantic search via pgvector embeddings

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

use ob_poc::agent::learning::embedder::{CachedEmbedder, OpenAIEmbedder};
use ob_poc::agent::learning::warmup::LearningWarmup;
use ob_poc::mcp::McpServer;

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");

    eprintln!("[dsl_mcp] Connecting to database...");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    eprintln!("[dsl_mcp] Connected to database");

    // Run learning warmup at startup
    eprintln!("[dsl_mcp] Running learning warmup...");
    let warmup = LearningWarmup::new(pool.clone());
    let (learned_data, stats) = warmup.warmup().await?;
    eprintln!(
        "[dsl_mcp] Warmup complete: {} aliases, {} tokens, {} phrases ({}ms)",
        stats.entity_aliases_loaded,
        stats.lexicon_tokens_loaded,
        stats.invocation_phrases_loaded,
        stats.duration_ms
    );

    // Check for OpenAI API key to enable semantic search
    let server = if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        eprintln!("[dsl_mcp] OPENAI_API_KEY found, enabling semantic search");
        let embedder = Arc::new(CachedEmbedder::new(Arc::new(OpenAIEmbedder::new(api_key))));
        McpServer::with_learned_data_and_embedder(pool, learned_data, embedder)
    } else {
        eprintln!("[dsl_mcp] No OPENAI_API_KEY, semantic search disabled");
        McpServer::with_learned_data(pool, learned_data)
    };

    server.run().await
}
