//! OB-POC Web Server
//!
//! Serves the egui-based UI via WASM and provides all API endpoints
//! for DSL generation, entity search, attributes, and DSL viewer.

mod routes;
mod state;

use axum::{routing::get, Router};
use http::header::{HeaderValue, CACHE_CONTROL};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::state::AppState;

// Import API routers from main ob-poc crate
use ob_poc::api::{
    create_agent_router_with_sessions, create_attribute_router, create_dsl_viewer_router,
    create_entity_router, create_resolution_router, create_session_store,
};

// Import resolution store from services
use ob_poc::services::create_resolution_store;

// EntityGateway for entity resolution
use entity_gateway::{
    config::StartupMode,
    index::{IndexRegistry, TantivyIndex},
    proto::ob::gateway::v1::entity_gateway_server::EntityGatewayServer,
    refresh::{run_refresh_loop, RefreshPipeline},
    server::EntityGatewayService,
    GatewayConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let pool = match sqlx::PgPool::connect(&database_url).await {
        Ok(p) => {
            tracing::info!("Database connection established");
            p
        }
        Err(e) => {
            tracing::error!("Failed to connect to database at {}: {}", database_url, e);
            tracing::error!(
                "Please check DATABASE_URL environment variable and ensure PostgreSQL is running"
            );
            return Err(format!("Database connection failed: {}", e).into());
        }
    };

    // =========================================================================
    // Start embedded EntityGateway gRPC service
    // =========================================================================
    tracing::info!("Starting embedded EntityGateway...");

    let gateway_config_path = std::env::var("ENTITY_GATEWAY_CONFIG")
        .unwrap_or_else(|_| "crates/entity-gateway/config/entity_index.yaml".to_string());

    let gateway_config = match GatewayConfig::from_file(&gateway_config_path) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::warn!(
                "Failed to load EntityGateway config from {}: {}",
                gateway_config_path,
                e
            );
            tracing::warn!("Entity resolution features will not be available");
            None
        }
    };

    if let Some(gateway_config) = gateway_config {
        let configs_by_nickname: std::collections::HashMap<String, _> = gateway_config
            .entities
            .values()
            .map(|cfg| (cfg.nickname.clone(), cfg.clone()))
            .collect();
        let registry = Arc::new(IndexRegistry::new(configs_by_nickname));

        for entity_config in gateway_config.entities.values() {
            match TantivyIndex::new(entity_config.clone()) {
                Ok(index) => {
                    registry
                        .register(entity_config.nickname.clone(), Arc::new(index))
                        .await;
                    tracing::debug!("Registered index: {}", entity_config.nickname);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create index for {}: {}",
                        entity_config.nickname,
                        e
                    );
                }
            }
        }

        let refresh_registry = registry.clone();
        let refresh_config = gateway_config.clone();
        tokio::spawn(async move {
            match RefreshPipeline::new(refresh_config.clone()).await {
                Ok(pipeline) => {
                    match refresh_config.refresh.startup_mode {
                        StartupMode::Sync => {
                            tracing::info!("Performing initial index refresh (sync)...");
                            if let Err(e) = pipeline.refresh_all(&refresh_registry).await {
                                tracing::warn!("Initial refresh failed: {}", e);
                            }
                        }
                        StartupMode::Async => {
                            tracing::info!("Starting async initial refresh...");
                            let reg = refresh_registry.clone();
                            tokio::spawn(async move {
                                if let Err(e) = pipeline.refresh_all(&reg).await {
                                    tracing::error!("Async initial refresh failed: {}", e);
                                }
                            });
                        }
                    }

                    let loop_pipeline = match RefreshPipeline::new(refresh_config.clone()).await {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!("Failed to create refresh loop pipeline: {}", e);
                            return;
                        }
                    };
                    run_refresh_loop(
                        loop_pipeline,
                        refresh_registry,
                        refresh_config.refresh.interval_secs,
                    )
                    .await;
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize refresh pipeline: {}", e);
                }
            }
        });

        let grpc_service = EntityGatewayService::new(registry);
        // Default gRPC address - validated at compile time
        const DEFAULT_GRPC_ADDR: &str = "[::]:50051";
        let grpc_addr: SocketAddr = std::env::var("ENTITY_GATEWAY_ADDR")
            .unwrap_or_else(|_| DEFAULT_GRPC_ADDR.to_string())
            .parse()
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Invalid ENTITY_GATEWAY_ADDR, using default {}: {}",
                    DEFAULT_GRPC_ADDR,
                    e
                );
                DEFAULT_GRPC_ADDR
                    .parse()
                    .expect("default gRPC address is valid")
            });

        tokio::spawn(async move {
            tracing::info!("EntityGateway gRPC listening on {}", grpc_addr);
            if let Err(e) = tonic::transport::Server::builder()
                .add_service(EntityGatewayServer::new(grpc_service))
                .serve(grpc_addr)
                .await
            {
                tracing::error!("EntityGateway gRPC server error: {}", e);
            }
        });

        tracing::info!("EntityGateway started successfully");
    }

    // Create single shared session store for agent routers
    let sessions = create_session_store();

    // Create resolution store for entity reference resolution
    let resolution_store = create_resolution_store();

    // Create shared state for CBU/graph endpoints
    let state = AppState::new(pool.clone());

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

    // Build stateless API router (from main ob-poc crate) with SHARED session store
    let api_router: Router<()> = Router::new()
        .merge(create_agent_router_with_sessions(
            pool.clone(),
            sessions.clone(),
        ))
        .merge(create_attribute_router(pool.clone()))
        .merge(create_entity_router())
        .merge(create_dsl_viewer_router(pool.clone()))
        .merge(create_resolution_router(sessions, resolution_store));

    // Build main app router with state
    // Session routes (including /bind) now share the same session store via create_agent_router_with_sessions
    let app = Router::new()
        // CBU API routes (custom implementations using AppState)
        .route("/api/cbu", get(routes::api::list_cbus))
        .route("/api/cbu/:id", get(routes::api::get_cbu))
        .route("/api/cbu/:id/graph", get(routes::api::get_cbu_graph))
        // SSE streaming for agent chat
        .route("/api/chat/stream", get(routes::chat::chat_stream))
        // Static files (JS, CSS, WASM) with no-cache headers for development
        .nest_service(
            "/static",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, no-store, must-revalidate"),
                ))
                .service(ServeDir::new(&static_dir).precompressed_gzip()),
        )
        // Index.html at root (egui app)
        .route("/", get(routes::static_files::serve_index))
        // Add state
        .with_state(state)
        // Merge stateless API routes (includes session, agent, entity, dsl viewer)
        .merge(api_router)
        // Layers
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let port: u16 = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("");
    tracing::info!("===========================================");
    tracing::info!("  OB-POC Web Server running on http://{}", addr);
    tracing::info!("===========================================");
    tracing::info!("");
    tracing::info!("UI: http://localhost:{}", port);
    tracing::info!("");
    tracing::info!("API Endpoints:");
    tracing::info!("  /api/cbu              - List CBUs");
    tracing::info!("  /api/cbu/:id/graph    - Get CBU graph");
    tracing::info!("  /api/session          - Session management");
    tracing::info!("  /api/session/:id/resolution/* - Entity resolution");
    tracing::info!("  /api/agent/*          - DSL generation");
    tracing::info!("  /api/entity/search    - Entity search");
    tracing::info!("  /api/dsl/*            - DSL viewer");
    tracing::info!("");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind to {}: {}", addr, e);
            if e.kind() == std::io::ErrorKind::AddrInUse {
                tracing::error!(
                    "Port {} is already in use. Try: lsof -ti:{} | xargs kill -9",
                    port,
                    port
                );
            }
            return Err(format!("Failed to bind to {}: {}", addr, e).into());
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Server error: {}", e);
        return Err(format!("Server error: {}", e).into());
    }

    Ok(())
}
