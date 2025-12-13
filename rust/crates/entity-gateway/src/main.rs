//! Entity Gateway Server
//!
//! Main entry point for the EntityGateway gRPC service.

use std::sync::Arc;

use tonic::transport::Server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use entity_gateway::{
    config::StartupMode,
    index::{IndexRegistry, TantivyIndex},
    proto::ob::gateway::v1::entity_gateway_server::EntityGatewayServer,
    refresh::{run_refresh_loop, RefreshPipeline},
    server::EntityGatewayService,
    GatewayConfig,
};

/// Default configuration path
const DEFAULT_CONFIG_PATH: &str = "config/entity_index.yaml";

/// Default server address
const DEFAULT_ADDR: &str = "[::]:50051";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "entity_gateway=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting EntityGateway server");

    // Load configuration
    let config_path =
        std::env::var("ENTITY_GATEWAY_CONFIG").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());

    tracing::info!(path = %config_path, "Loading configuration");

    let config = GatewayConfig::from_file(&config_path)?;

    tracing::info!(
        entities = config.entities.len(),
        refresh_interval = config.refresh.interval_secs,
        "Configuration loaded"
    );

    // Create index registry - keyed by nickname field (uppercase), not YAML key
    let configs_by_nickname: std::collections::HashMap<String, _> = config
        .entities
        .values()
        .map(|cfg| (cfg.nickname.clone(), cfg.clone()))
        .collect();
    let registry = Arc::new(IndexRegistry::new(configs_by_nickname));

    // Create indexes for each entity (using nickname from config)
    for entity_config in config.entities.values() {
        tracing::info!(nickname = %entity_config.nickname, "Creating index");
        let index = TantivyIndex::new(entity_config.clone())?;
        registry
            .register(entity_config.nickname.clone(), Arc::new(index))
            .await;
    }

    // Initialize refresh pipeline
    let pipeline = RefreshPipeline::new(config.clone()).await?;

    // Initial refresh based on startup mode
    match config.refresh.startup_mode {
        StartupMode::Sync => {
            tracing::info!("Performing synchronous initial refresh");
            pipeline
                .refresh_all(&registry)
                .await
                .map_err(|e| e.to_string())?;
            tracing::info!("Initial refresh complete, all indexes ready");
        }
        StartupMode::Async => {
            tracing::info!("Starting asynchronous initial refresh");
            let reg = registry.clone();
            let cfg = config.clone();
            tokio::spawn(async move {
                let pipe = match RefreshPipeline::new(cfg).await {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to create refresh pipeline");
                        return;
                    }
                };
                if let Err(e) = pipe.refresh_all(&reg).await {
                    tracing::error!(error = %e, "Initial async refresh failed");
                } else {
                    tracing::info!("Initial async refresh complete");
                }
            });
        }
    }

    // Start background refresh loop
    let refresh_registry = registry.clone();
    let refresh_interval = config.refresh.interval_secs;
    let refresh_config = config.clone();
    tokio::spawn(async move {
        let pipeline = match RefreshPipeline::new(refresh_config).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, "Failed to create refresh pipeline for background loop");
                return;
            }
        };
        run_refresh_loop(pipeline, refresh_registry, refresh_interval).await;
    });

    // Create gRPC service
    let service = EntityGatewayService::new(registry);

    // Parse server address
    let addr = std::env::var("ENTITY_GATEWAY_ADDR")
        .unwrap_or_else(|_| DEFAULT_ADDR.to_string())
        .parse()?;

    tracing::info!(%addr, "Starting gRPC server");

    // Start server
    Server::builder()
        .add_service(EntityGatewayServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
