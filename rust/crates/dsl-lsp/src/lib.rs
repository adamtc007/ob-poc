//! DSL Language Server Library
//!
//! This crate provides the LSP implementation for the Onboarding DSL,
//! including entity resolution via EntityGateway.

mod analysis;
mod encoding;
mod entity_client;
mod handlers;
mod server;

pub use encoding::{offset_to_position, position_to_offset, span_to_range, PositionEncoding};
pub use entity_client::{EntityLookupClient, EntityMatch};
pub use handlers::diagnostics::analyze_document;
pub use server::DslLanguageServer;
