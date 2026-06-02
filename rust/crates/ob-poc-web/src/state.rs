//! Shared application state
//!
//! The `AppState` provides shared resources for the ob-poc-web server.
//! Session management and chat are handled by agent_routes in the main crate.
//! This state is used for CBU/graph endpoints only.

use sqlx::PgPool;
use std::sync::Arc;

use crate::process_registry::ProcessRegistry;

/// Shared application state for CBU/graph endpoints
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pool: PgPool,
    // Carried for future routes that need both pool and process_registry
    // via State<AppState>. Currently forms routes use a dedicated FormsState.
    #[allow(dead_code)]
    pub(crate) process_registry: Arc<ProcessRegistry>,
}

impl AppState {
    pub(crate) fn new(pool: PgPool, process_registry: Arc<ProcessRegistry>) -> Self {
        Self {
            pool,
            process_registry,
        }
    }
}
