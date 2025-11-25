//! REST API module for agentic operations
//!
//! This module provides HTTP endpoints for the agentic DSL system,
//! allowing external clients to interact with the system via REST API.

#[cfg(feature = "server")]
pub mod attribute_routes;

#[cfg(feature = "server")]
pub use attribute_routes::create_attribute_router;
