//! gRPC Server Implementation
//!
//! This module provides the gRPC server setup for the DSL engine,
//! including both transformation and retrieval services.

use crate::database::{DatabaseManager, PgDslInstanceRepository};
use crate::dsl_manager::DslManager;
use crate::services::{DslRetrievalServiceImpl, DslTransformServiceImpl};
use std::sync::Arc;
use tonic::transport::Server;
use tracing::{error, info};

// Generated proto services
use crate::proto::dsl_retrieval::dsl_retrieval_service_server::DslRetrievalServiceServer;
use crate::proto::dsl_transform::dsl_transform_service_server::DslTransformServiceServer;

/// Configuration for the gRPC server
#[derive(Debug, Clone)]
pub struct GrpcServerConfig {
    pub transform_port: u16,
    pub retrieval_port: u16,
    pub host: String,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            transform_port: 50051,
            retrieval_port: 50052,
            host: "0.0.0.0".to_string(),
        }
    }
}

/// gRPC Server manager that runs both DSL transformation and retrieval services
pub struct GrpcServerManager {
    config: GrpcServerConfig,
    dsl_manager: Arc<DslManager>,
    instance_repository: Arc<PgDslInstanceRepository>,
}

impl GrpcServerManager {
    /// Create a new gRPC server manager
    pub fn new(
        config: GrpcServerConfig,
        dsl_manager: Arc<DslManager>,
        instance_repository: Arc<PgDslInstanceRepository>,
    ) -> Self {
        Self {
            config,
            dsl_manager,
            instance_repository,
        }
    }

    /// Start both gRPC services concurrently
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let transform_addr = format!("{}:{}", self.config.host, self.config.transform_port)
            .parse()
            .map_err(|e| format!("Invalid transform service address: {}", e))?;

        let retrieval_addr = format!("{}:{}", self.config.host, self.config.retrieval_port)
            .parse()
            .map_err(|e| format!("Invalid retrieval service address: {}", e))?;

        info!("Starting DSL Transform Service on {}", transform_addr);
        info!("Starting DSL Retrieval Service on {}", retrieval_addr);

        // Create service implementations
        let transform_service = DslTransformServiceImpl::new(
            self.dsl_manager.clone(),
            self.instance_repository.clone(),
        );

        let retrieval_service = DslRetrievalServiceImpl::new(
            self.dsl_manager.clone(),
            self.instance_repository.clone(),
        );

        // Start both services concurrently
        let transform_server = tokio::spawn(async move {
            let result = Server::builder()
                .add_service(DslTransformServiceServer::new(transform_service))
                .serve(transform_addr)
                .await;

            if let Err(e) = result {
                error!("DSL Transform Service error: {}", e);
            }
        });

        let retrieval_server = tokio::spawn(async move {
            let result = Server::builder()
                .add_service(DslRetrievalServiceServer::new(retrieval_service))
                .serve(retrieval_addr)
                .await;

            if let Err(e) = result {
                error!("DSL Retrieval Service error: {}", e);
            }
        });

        // Wait for either service to complete (or fail)
        tokio::select! {
            _ = transform_server => {
                info!("DSL Transform Service stopped");
            }
            _ = retrieval_server => {
                info!("DSL Retrieval Service stopped");
            }
        }

        Ok(())
    }
}

/// Convenience function to start both gRPC services with default configuration
pub async fn start_grpc_services(
    database_manager: &DatabaseManager,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize DSL Manager
    let dsl_manager = Arc::new(DslManager::new_with_defaults().await?);

    // Initialize DSL Instance Repository
    let instance_repository = Arc::new(database_manager.dsl_instance_repository());

    // Create server manager with default config
    let config = GrpcServerConfig::default();
    let server_manager = GrpcServerManager::new(config, dsl_manager, instance_repository);

    // Start services
    server_manager.start().await
}

/// Start only the DSL Transform Service (for state changes)
pub async fn start_transform_service(
    database_manager: &DatabaseManager,
    port: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    let port = port.unwrap_or(50051);
    let addr = format!("0.0.0.0:{}", port).parse()?;

    info!("Starting DSL Transform Service on {}", addr);

    // Initialize dependencies
    let dsl_manager = Arc::new(DslManager::new_with_defaults().await?);
    let instance_repository = Arc::new(database_manager.dsl_instance_repository());

    // Create service
    let transform_service = DslTransformServiceImpl::new(dsl_manager, instance_repository);

    // Start server
    Server::builder()
        .add_service(DslTransformServiceServer::new(transform_service))
        .serve(addr)
        .await?;

    Ok(())
}

/// Start only the DSL Retrieval Service (for Web UI queries)
pub async fn start_retrieval_service(
    database_manager: &DatabaseManager,
    port: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    let port = port.unwrap_or(50052);
    let addr = format!("0.0.0.0:{}", port).parse()?;

    info!("Starting DSL Retrieval Service on {}", addr);

    // Initialize dependencies
    let dsl_manager = Arc::new(DslManager::new_with_defaults().await?);
    let instance_repository = Arc::new(database_manager.dsl_instance_repository());

    // Create service
    let retrieval_service = DslRetrievalServiceImpl::new(dsl_manager, instance_repository);

    // Start server
    Server::builder()
        .add_service(DslRetrievalServiceServer::new(retrieval_service))
        .serve(addr)
        .await?;

    Ok(())
}
