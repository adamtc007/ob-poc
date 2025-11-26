//! DSL Language Server - Main entry point
//!
//! Provides LSP support for the Onboarding DSL with:
//! - Syntax highlighting and error detection
//! - Smart completions with human-readable picklists
//! - Go-to-definition for @symbol references
//! - Hover documentation for verbs
//! - Signature help while typing

use tower_lsp::{LspService, Server};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod server;
mod handlers;
mod analysis;

use server::DslLanguageServer;

#[tokio::main]
async fn main() {
    // Setup logging to stderr (LSP uses stdout for protocol)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dsl_lsp=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting DSL Language Server");

    // Create LSP service
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(DslLanguageServer::new);

    // Run server
    Server::new(stdin, stdout, socket).serve(service).await;
}
