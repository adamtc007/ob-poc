//! Agentic DSL REST API Server
//!
//! This binary provides a REST API server for the agentic DSL system,
//! enabling HTTP access to intelligent DSL generation, entity creation,
//! role management, CBU operations, and complete workflow orchestration.
//!
//! ## Usage
//!
//! ```bash
//! # Start the server
//! DATABASE_URL=postgresql://localhost/ob-poc \
//! ANTHROPIC_API_KEY=your-key \
//! cargo run --bin agentic_server --features server
//!
//! # Open web UI
//! open http://localhost:3000
//!
//! # Test endpoints
//! curl -X POST http://localhost:3000/api/agent/generate \
//!   -H "Content-Type: application/json" \
//!   -d '{"instruction": "Create a CBU for TechCorp Ltd", "domain": "cbu"}'
//!
//! curl http://localhost:3000/api/agent/domains
//! curl http://localhost:3000/api/agent/vocabulary?domain=cbu
//! curl http://localhost:3000/api/agent/health
//! ```

use axum::response::Html;
use axum::routing::get;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use ob_poc::api::{
    create_agent_router, create_attribute_router, create_dsl_viewer_router, create_entity_router,
    create_graph_router,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("Starting Agentic DSL REST API Server");

    // Check for LLM API key
    let has_anthropic = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let has_openai = std::env::var("OPENAI_API_KEY").is_ok();

    if !has_anthropic && !has_openai {
        println!("Warning: No LLM API key found (ANTHROPIC_API_KEY or OPENAI_API_KEY)");
        println!("   DSL generation will not work without an API key");
    } else {
        println!("LLM API key configured");
    }

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    println!("Connecting to database: {}", database_url);

    // Create database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    println!("Database connection established");

    // Create routers and merge them
    let app = Router::new()
        // Serve index.html at root - redirect to WASM app
        .route("/", get(serve_index))
        // Serve static files for WASM app
        .nest_service("/pkg", ServeDir::new("crates/ob-poc-ui/pkg"))
        // API routes
        .merge(create_graph_router(pool.clone()))
        .merge(create_agent_router(pool.clone()))
        .merge(create_attribute_router(pool.clone()))
        .merge(create_entity_router(pool.clone()))
        .merge(create_dsl_viewer_router(pool))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    // Bind to address
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("\nServer running on http://{}", addr);
    println!("\nAvailable endpoints:");
    println!("  Web UI:");
    println!("    GET    http://localhost:3000/                       - CBU Graph Visualization");
    println!("\n  CBU Graph API:");
    println!("    GET    http://localhost:3000/api/cbu                - List all CBUs");
    println!("    GET    http://localhost:3000/api/cbu/:id            - Get CBU summary");
    println!("    GET    http://localhost:3000/api/cbu/:id/graph      - Get CBU graph data");
    println!("\n  Session Management:");
    println!("    POST   http://localhost:3000/api/session            - Create new session");
    println!("    GET    http://localhost:3000/api/session/:id        - Get session state");
    println!("    DELETE http://localhost:3000/api/session/:id        - Delete session");
    println!("    POST   http://localhost:3000/api/session/:id/chat   - Send chat message");
    println!("    POST   http://localhost:3000/api/session/:id/execute - Execute accumulated DSL");
    println!("\n  Agent DSL Generation:");
    println!("    POST   http://localhost:3000/api/agent/generate     - Generate DSL from natural language");
    println!(
        "    POST   http://localhost:3000/api/agent/validate     - Validate DSL syntax/semantics"
    );
    println!(
        "    GET    http://localhost:3000/api/agent/domains      - List available DSL domains"
    );
    println!("    GET    http://localhost:3000/api/agent/vocabulary   - Get vocabulary (optionally by domain)");
    println!("    GET    http://localhost:3000/api/agent/health       - Health check");
    println!("\n  Attribute Dictionary:");
    println!("    POST   http://localhost:3000/api/documents/upload");
    println!("    POST   http://localhost:3000/api/attributes/validate-dsl");
    println!("    POST   http://localhost:3000/api/attributes/validate-value");
    println!("    GET    http://localhost:3000/api/attributes/:cbu_id");
    println!("    GET    http://localhost:3000/api/attributes/document/:doc_id");
    println!("    GET    http://localhost:3000/api/attributes/health");
    println!("\n  Entity Search:");
    println!("    GET    http://localhost:3000/api/entities/search?q=<query>&types=PERSON,COMPANY");
    println!("\n  Templates:");
    println!("    GET    http://localhost:3000/api/templates                - List all templates");
    println!(
        "    GET    http://localhost:3000/api/templates/:id            - Get template details"
    );
    println!(
        "    POST   http://localhost:3000/api/templates/:id/render     - Render template to DSL"
    );
    println!("\n  DSL Viewer:");
    println!("    GET    http://localhost:3000/api/dsl/list                 - List DSL instances");
    println!(
        "    GET    http://localhost:3000/api/dsl/show/:ref            - Get latest DSL version"
    );
    println!(
        "    GET    http://localhost:3000/api/dsl/show/:ref/:ver       - Get specific version"
    );
    println!("    GET    http://localhost:3000/api/dsl/history/:ref         - Get version history");
    println!("\nPress Ctrl+C to stop\n");

    // Start server (Axum 0.7+ style)
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Serve the index.html for the WASM app
async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OB-POC Visualization</title>
    <style>
        html, body {
            margin: 0;
            padding: 0;
            width: 100%;
            height: 100%;
            overflow: hidden;
            background: #1a1a2e;
        }

        #ob_poc_canvas {
            width: 100%;
            height: 100%;
        }

        .loading {
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            color: #e94560;
            font-family: system-ui, -apple-system, sans-serif;
            font-size: 18px;
        }
    </style>
</head>
<body>
    <div class="loading" id="loading">Loading WASM app...</div>
    <canvas id="ob_poc_canvas"></canvas>
    <script type="module">
        // Cache bust: v2
        import init from './pkg/ob_poc_ui.js?v=2';

        init().then(() => {
            document.getElementById('loading').style.display = 'none';
        }).catch(err => {
            document.getElementById('loading').textContent = 'Failed to load: ' + err;
            console.error(err);
        });
    </script>
</body>
</html>"#;
