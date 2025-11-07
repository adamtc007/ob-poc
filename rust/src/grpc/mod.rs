//! Minimal gRPC service implementations

pub mod parser_service;

// Re-export service implementations
pub use parser_service::ParserServiceImpl;

use std::net::SocketAddr;
use tonic::transport::Server;

/// Minimal gRPC server that hosts just the parser service
pub struct DSLGrpcServer {
    parser_service: ParserServiceImpl,
}

impl DSLGrpcServer {
    pub fn new() -> Self {
        Self {
            parser_service: ParserServiceImpl::new(),
        }
    }

    /// Start the gRPC server on the given address
    pub async fn serve(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Starting minimal DSL gRPC server on {}", addr);

        use crate::proto::ob_poc::parser::tonic::parser_service_server::ParserServiceServer;

        Server::builder()
            .add_service(ParserServiceServer::new(self.parser_service))
            .serve(addr)
            .await?;

        Ok(())
    }
}

impl Default for DSLGrpcServer {
    fn default() -> Self {
        Self::new()
    }
}
