//! Shared application state
//!
//! The `AppState` provides shared resources for the ob-poc-web server.
//! Entity resolution is handled by `AgentService` which internally uses
//! EntityGateway - the same service used by the LSP for autocomplete.

use ob_poc::api::agent_service::AgentService;
use ob_poc::api::session::{create_session_store, SessionStore};
use ob_poc::database::SessionRepository;
use ob_poc::dsl_v2::DslExecutor;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Pending chat stream for SSE
#[derive(Debug, Clone)]
#[allow(dead_code)] // session_id used for debugging/logging
pub struct PendingStream {
    pub session_id: Uuid,
    pub chunks: Vec<String>,
    pub complete: bool,
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// In-memory session store (same as agentic_server)
    pub sessions: SessionStore,
    /// DSL executor
    pub dsl_executor: Arc<DslExecutor>,
    /// Session repository for persistence
    pub session_repo: Arc<SessionRepository>,
    /// Pending SSE streams by stream ID
    pub pending_streams: Arc<RwLock<HashMap<Uuid, PendingStream>>>,
    /// Centralized agent service for chat/disambiguation/entity resolution
    /// This service uses EntityGateway internally - same as LSP autocomplete
    pub agent_service: Arc<AgentService>,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            sessions: create_session_store(),
            dsl_executor: Arc::new(DslExecutor::new(pool.clone())),
            session_repo: Arc::new(SessionRepository::new(pool.clone())),
            pending_streams: Arc::new(RwLock::new(HashMap::new())),
            agent_service: Arc::new(AgentService::with_pool(pool.clone())),
            pool,
        }
    }
}
