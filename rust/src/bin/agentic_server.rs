//! Agentic DSL REST API Server
//!
//! This binary provides a REST API server for the agentic DSL system,
//! enabling HTTP access to entity creation, role management, CBU operations,
//! and complete workflow orchestration.
//!
//! ## Usage
//!
//! ```bash
//! # Start the server
//! DATABASE_URL=postgresql://localhost/ob-poc cargo run --bin agentic_server --features server
//!
//! # Test endpoints
//! curl -X POST http://localhost:3000/api/agentic/execute \
//!   -H "Content-Type: application/json" \
//!   -d '{"prompt": "Create entity John Smith as person"}'
//!
//! curl -X POST http://localhost:3000/api/agentic/setup \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "entity_name": "Alice Johnson",
//!     "entity_type": "PERSON",
//!     "role_name": "Director",
//!     "cbu_nature": "Private wealth management",
//!     "cbu_source": "Investment portfolio"
//!   }'
//!
//! curl http://localhost:3000/api/agentic/tree/{cbu_id}
//! curl http://localhost:3000/api/health
//! ```

use axum::Server;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use ob_poc::api::create_agentic_router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("🚀 Starting Agentic DSL REST API Server");

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    println!("📊 Connecting to database: {}", database_url);

    // Create database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    println!("✅ Database connection established");

    // Create router with CORS and tracing
    let app = create_agentic_router(pool)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    // Bind to address
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("\n🌐 Server running on http://{}", addr);
    println!("\n📖 Available endpoints:");
    println!("  POST   http://localhost:3000/api/agentic/execute");
    println!("  POST   http://localhost:3000/api/agentic/setup");
    println!("  GET    http://localhost:3000/api/agentic/tree/:cbu_id");
    println!("  GET    http://localhost:3000/api/health");
    println!("\n✨ Press Ctrl+C to stop\n");

    // Start server
    Server::bind(&addr).serve(app.into_make_service()).await?;

    Ok(())
}
