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
pub mod session_manager;

#[cfg(feature = "server")]
pub mod dsl_session_file;

#[cfg(feature = "server")]
pub mod agent_service;

#[cfg(feature = "server")]
pub mod client_group_adapter;

#[cfg(feature = "server")]
pub mod entity_routes;

#[cfg(feature = "server")]
pub mod dsl_viewer_routes;

#[cfg(feature = "server")]
pub mod graph_routes;

#[cfg(feature = "server")]
pub mod resolution_routes;

#[cfg(feature = "server")]
pub mod client_routes;

#[cfg(feature = "server")]
pub mod client_auth;

#[cfg(feature = "server")]
pub mod verb_discovery_routes;

#[cfg(feature = "server")]
pub mod trading_matrix_routes;

#[cfg(feature = "server")]
pub mod taxonomy_routes;

#[cfg(feature = "server")]
pub mod universe_routes;

#[cfg(feature = "server")]
pub mod capital_routes;

#[cfg(feature = "server")]
pub mod control_routes;

#[cfg(feature = "server")]
pub mod cbu_session_routes;

#[cfg(feature = "server")]
pub mod service_resource_routes;

#[cfg(feature = "server")]
pub mod workflow_routes;

#[cfg(feature = "server")]
pub use attribute_routes::create_attribute_router;

#[cfg(feature = "server")]
pub use entity_routes::{create_entity_router, create_scoped_entity_router};

#[cfg(feature = "server")]
pub use agent_routes::create_agent_router_with_semantic;

#[cfg(feature = "server")]
pub use dsl_viewer_routes::create_dsl_viewer_router;

#[cfg(feature = "server")]
pub use graph_routes::{create_graph_router, create_session_graph_router};

#[cfg(feature = "server")]
pub use session::{create_session_store, SessionStore};

#[cfg(feature = "server")]
pub use session_manager::{SessionManager, SessionSnapshot, SessionWatcher};

#[cfg(feature = "server")]
pub use intent::{AssembledDsl, IntentSequence, VerbIntent};

#[cfg(feature = "server")]
pub use agent_service::{AgentChatResponse, AgentCommand, AgentService, ChatRequest, ClientScope};

#[cfg(feature = "server")]
pub use resolution_routes::create_resolution_router;

#[cfg(feature = "server")]
pub use client_routes::{create_client_router, AuthenticatedClient, ClientState};

#[cfg(feature = "server")]
pub use verb_discovery_routes::create_verb_discovery_router;

#[cfg(feature = "server")]
pub use trading_matrix_routes::create_trading_matrix_router;

#[cfg(feature = "server")]
pub use taxonomy_routes::create_taxonomy_router;

#[cfg(feature = "server")]
pub use universe_routes::create_universe_router;

#[cfg(feature = "server")]
pub use capital_routes::create_capital_router;

#[cfg(feature = "server")]
pub use control_routes::control_routes;

#[cfg(feature = "server")]
pub use cbu_session_routes::{
    create_cbu_session_router, create_cbu_session_router_with_pool, CbuSessionState,
    CbuSessionStore,
};

#[cfg(feature = "server")]
pub use service_resource_routes::{service_resource_router, ServiceResourceState};

#[cfg(feature = "server")]
pub use workflow_routes::{create_workflow_router, WorkflowState};
