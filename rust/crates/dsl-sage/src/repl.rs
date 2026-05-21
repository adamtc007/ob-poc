//! REPL integration — synchronous wrappers around the async Sage orchestrator.
//!
//! [`SageSessionStore`] holds active sessions keyed by their session ID and
//! exposes a blocking [`SageSessionStore::step_sync`] method suitable for use
//! from non-async REPL call sites.

use std::collections::HashMap;
use std::sync::Mutex;

use dsl_resolution::PackRegistry;

use crate::{
    orchestrator::{SageInput, SageOrchestrator, SageSession, SageState},
    types::SageContext,
};

// ---------------------------------------------------------------------------
// Session store
// ---------------------------------------------------------------------------

/// A thread-safe store of active Sage sessions.
pub struct SageSessionStore {
    sessions: Mutex<HashMap<String, SageSession>>,
}

impl SageSessionStore {
    /// Create an empty session store.
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Start a new session and return its ID.
    pub fn create_session(&self, context: SageContext) -> String {
        let session = SageSession::new(context);
        let id = session.session_id.clone();
        self.sessions.lock().unwrap().insert(id.clone(), session);
        id
    }

    /// Return the current state of a session, or `None` if not found.
    pub fn get_session_state(&self, session_id: &str) -> Option<SageState> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|s| s.state.clone())
    }

    /// Return the transition log of a session, or an empty `Vec` if not found.
    pub fn get_session_log(&self, session_id: &str) -> Vec<String> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|s| s.transition_log.clone())
            .unwrap_or_default()
    }

    /// Drive one step of the state machine synchronously.
    ///
    /// Suitable for REPL call sites that are not inside an async runtime.  A
    /// short-lived `tokio::runtime::Runtime` is created for each call.
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` when the session is not found or the async step
    /// propagates an error.
    pub fn step_sync(
        &self,
        session_id: &str,
        input: SageInput,
        registry: &PackRegistry,
    ) -> Result<SageState, String> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("session not found: {}", session_id))?;

        let orchestrator = SageOrchestrator::new(registry);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let new_state = rt
            .block_on(orchestrator.step(session, input))
            .map_err(|e| e.to_string())?;

        Ok(new_state.clone())
    }
}

impl Default for SageSessionStore {
    fn default() -> Self {
        Self::new()
    }
}
