//! DSL Language Server Library
//!
//! This crate provides the LSP implementation for the Onboarding DSL,
//! including entity resolution via EntityGateway.

pub mod analysis;
pub mod entity_client;
pub mod handlers;
pub mod server;

pub use entity_client::{EntityLookupClient, EntityMatch};
pub use server::DslLanguageServer;
