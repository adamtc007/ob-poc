//! REST API module for DSL v2 operations
//!
//! This module provides HTTP endpoints for the DSL v2 system,
//! allowing external clients to interact with the system via REST API.

#[cfg(feature = "server")]
pub mod attribute_routes;

#[cfg(feature = "server")]
pub mod agent_routes;

#[cfg(feature = "server")]
pub mod intent;

#[cfg(feature = "server")]
pub mod dsl_builder;

#[cfg(feature = "server")]
pub mod session;

#[cfg(feature = "server")]
pub mod dsl_session_file;

#[cfg(feature = "server")]
pub mod agent_service;

#[cfg(feature = "server")]
pub mod entity_routes;

#[cfg(feature = "server")]
pub mod dsl_viewer_routes;

#[cfg(feature = "server")]
pub mod graph_routes;

#[cfg(feature = "server")]
pub use attribute_routes::create_attribute_router;

#[cfg(feature = "server")]
pub use entity_routes::create_entity_router;

#[cfg(feature = "server")]
pub use agent_routes::{create_agent_router, create_agent_router_with_sessions};

#[cfg(feature = "server")]
pub use dsl_viewer_routes::create_dsl_viewer_router;

#[cfg(feature = "server")]
pub use graph_routes::create_graph_router;

#[cfg(feature = "server")]
pub use session::{create_session_store, SessionStore};

#[cfg(feature = "server")]
pub use intent::{AssembledDsl, IntentSequence, VerbIntent};

#[cfg(feature = "server")]
pub use agent_service::{AgentChatRequest, AgentChatResponse, AgentCommand, AgentService};
