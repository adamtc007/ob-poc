//! DSL Language Server - Main entry point
//!
//! Provides LSP support for the Onboarding DSL with:
//! - Syntax highlighting and error detection
//! - Smart completions with human-readable picklists
//! - Go-to-definition for @symbol references
//! - Hover documentation for verbs
//! - Signature help while typing

use std::fs::OpenOptions;
use tower_lsp::{LspService, Server};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod analysis;
mod entity_client;
mod handlers;
mod server;

use server::DslLanguageServer;

#[tokio::main]
async fn main() {
    // Setup logging to file for debugging (LSP uses stdout for protocol)
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/dsl-lsp.log")
        .expect("Failed to open log file");

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dsl_lsp=debug,ob_poc=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::sync::Mutex::new(log_file)))
        .init();

    tracing::info!("Starting DSL Language Server");
    tracing::info!("DSL_CONFIG_DIR: {:?}", std::env::var("DSL_CONFIG_DIR"));
    tracing::info!(
        "ENTITY_GATEWAY_URL: {:?}",
        std::env::var("ENTITY_GATEWAY_URL")
            .unwrap_or_else(|_| "http://[::1]:50051 (default)".to_string())
    );

    // Create LSP service
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(DslLanguageServer::new);

    // Run server
    Server::new(stdin, stdout, socket).serve(service).await;
}
