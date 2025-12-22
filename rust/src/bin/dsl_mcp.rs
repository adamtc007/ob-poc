//! DSL MCP Server Binary
//!
//! Runs the DSL MCP server for Claude integration.
//!
//! ## Usage
//!
//! ```bash
//! DATABASE_URL=postgresql://localhost/ob-poc ./target/debug/dsl_mcp
//! ```

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;

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

    McpServer::new(pool).run().await
}
