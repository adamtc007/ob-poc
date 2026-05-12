//! DSL Language Server Library
//!
//! This crate provides the LSP implementation for the Onboarding DSL,
//! including entity resolution via EntityGateway.

pub mod analysis;
mod encoding;
mod entity_client;
pub mod handlers;
mod server;

pub use handlers::diagnostics::analyze_document;
