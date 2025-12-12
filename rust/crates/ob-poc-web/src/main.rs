//! OB-POC Hybrid Web Server
//!
//! Serves the hybrid UI architecture:
//! - HTML/TypeScript for Chat, DSL, AST panels
//! - WASM/egui for CBU graph visualization
//!
//! This replaces the monolithic egui UI with a more debuggable setup.

mod routes;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::state::AppState;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ob_poc_web=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting OB-POC Hybrid Web Server");

    // Database connection
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    tracing::info!("Database connection established");

    // Create shared state
    let state = AppState::new(pool);

    // Static file serving - point to our static directory
    // Use manifest dir at compile time, or STATIC_DIR env var at runtime
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| {
        // Try to find static dir relative to the crate
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        format!("{}/static", manifest_dir)
    });
    tracing::info!("Serving static files from: {}", static_dir);

    // CORS for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        // Session API routes
        .route("/api/session", post(routes::api::create_session))
        .route("/api/session/:id", get(routes::api::get_session))
        .route("/api/session/:id/chat", post(routes::api::chat))
        .route("/api/session/:id/chat/v2", post(routes::api::chat_v2))
        .route("/api/session/:id/execute", post(routes::api::execute))
        .route("/api/session/:id/dsl", get(routes::api::get_session_dsl))
        .route("/api/session/:id/ast", get(routes::api::get_session_ast))
        // CBU API routes
        .route("/api/cbu", get(routes::api::list_cbus))
        .route("/api/cbu/:id", get(routes::api::get_cbu))
        .route("/api/cbu/:id/graph", get(routes::api::get_cbu_graph))
        // SSE streaming for agent chat
        .route("/api/chat/stream", get(routes::chat::chat_stream))
        // Static files (JS, CSS, WASM)
        .nest_service("/static", ServeDir::new(&static_dir))
        // Index.html at root
        .route("/", get(routes::static_files::serve_index))
        // Layers
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001)); // Port 3001 to avoid conflict
    tracing::info!("Server running on http://{}", addr);
    tracing::info!("");
    tracing::info!("Hybrid UI Architecture:");
    tracing::info!("  - HTML/TS: Chat, DSL, AST panels");
    tracing::info!("  - WASM/egui: CBU graph visualization");
    tracing::info!("");
    tracing::info!("Open http://localhost:3001 in your browser");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
