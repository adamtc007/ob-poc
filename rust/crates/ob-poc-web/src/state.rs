//! Shared application state
//!
//! The `AppState` provides shared resources for the ob-poc-web server.
//! Session management and chat are handled by agent_routes in the main crate.
//! This state is used for CBU/graph endpoints only.

use sqlx::PgPool;

/// Shared application state for CBU/graph endpoints
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
