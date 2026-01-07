//! SessionManager - Reactive session management with watch channels.
//!
//! This module provides a wrapper around the existing SessionStore that adds
//! reactive notification capabilities via tokio::sync::watch channels.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │            SessionManager                                    │
//! │  Wraps existing SessionStore + adds watch channels           │
//! ├──────────────────────────────────────────────────────────────┤
//! │  store: SessionStore (Arc<RwLock<HashMap>>)                  │
//! │  watchers: RwLock<HashMap<Uuid, WatcherEntry>>               │
//! └──────────────────────────────────────────────────────────────┘
//!                           │
//!           ┌───────────────┼───────────────┐
//!           ▼               ▼               ▼
//!     ┌──────────┐  ┌──────────┐  ┌──────────────┐
//!     │ HTTP API │  │ MCP      │  │ Graph Widget │
//!     │ routes   │  │ handlers │  │ (subscriber) │
//!     └──────────┘  └──────────┘  └──────────────┘
//! ```
//!
//! ## Key Design Decisions
//!
//! 1. **Wraps existing SessionStore** - No parallel structures
//! 2. **Watch channels per session** - Created on-demand when first subscriber appears
//! 3. **Atomic updates with notification** - `update_session()` handles lock + notify
//! 4. **Backward compatible** - Existing `sessions.read()/write()` code still works

use crate::api::session::{AgentSession, SessionStore};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use uuid::Uuid;

/// Lightweight snapshot of session state for watch channel.
///
/// We send a snapshot rather than the full session to avoid
/// holding locks while subscribers process updates.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    /// Session ID
    pub session_id: Uuid,
    /// Version number (incremented on each update)
    pub version: u64,
    /// Current scope path as string (for quick comparison)
    pub scope_path: String,
    /// Whether struct_mass has been computed
    pub has_mass: bool,
    /// Current effective view mode (if set)
    pub view_mode: Option<String>,
    /// Active CBU ID (if bound)
    pub active_cbu_id: Option<Uuid>,
    /// Timestamp of last update
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl SessionSnapshot {
    /// Create a snapshot from a session
    pub fn from_session(session: &AgentSession) -> Self {
        Self {
            session_id: session.id,
            version: session.updated_at.timestamp_millis() as u64,
            scope_path: session.context.scope_path.to_string(),
            has_mass: session.context.struct_mass.is_some(),
            view_mode: session
                .context
                .auto_view_mode
                .as_ref()
                .map(|m| m.as_str().to_string()),
            active_cbu_id: session.context.active_cbu.as_ref().map(|c| c.id),
            updated_at: session.updated_at,
        }
    }

    /// Create an empty snapshot for initialization
    pub fn empty(session_id: Uuid) -> Self {
        Self {
            session_id,
            version: 0,
            scope_path: String::new(),
            has_mass: false,
            view_mode: None,
            active_cbu_id: None,
            updated_at: chrono::Utc::now(),
        }
    }
}

/// Session watcher - a receiver that yields on every session update.
pub type SessionWatcher = watch::Receiver<SessionSnapshot>;

/// Internal watcher entry with sender and subscriber count
struct WatcherEntry {
    sender: watch::Sender<SessionSnapshot>,
    subscriber_count: usize,
}

/// Map of session ID to watcher entry
type WatcherMap = Arc<RwLock<HashMap<Uuid, WatcherEntry>>>;

/// SessionManager wraps SessionStore with reactive notification capabilities.
///
/// Use this for:
/// - Atomic session updates that notify all subscribers
/// - Subscribing to session changes without polling
/// - Coordinating REPL and Viewport state
///
/// The underlying SessionStore remains accessible for backward compatibility.
pub struct SessionManager {
    /// The underlying session store
    store: SessionStore,

    /// Watch channels per session (created on-demand)
    watchers: WatcherMap,
}

impl SessionManager {
    /// Create a new SessionManager wrapping an existing store
    pub fn new(store: SessionStore) -> Self {
        Self {
            store,
            watchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the underlying session store (for backward compatibility)
    pub fn store(&self) -> &SessionStore {
        &self.store
    }

    /// Get a session by ID (read-only clone)
    pub async fn get_session(&self, id: Uuid) -> Option<AgentSession> {
        self.store.read().await.get(&id).cloned()
    }

    /// Check if a session exists
    pub async fn exists(&self, id: Uuid) -> bool {
        self.store.read().await.contains_key(&id)
    }

    /// Insert a new session
    pub async fn insert_session(&self, session: AgentSession) {
        let id = session.id;
        let snapshot = SessionSnapshot::from_session(&session);

        // Insert into store
        self.store.write().await.insert(id, session);

        // Notify watchers if any
        let watchers = self.watchers.read().await;
        if let Some(entry) = watchers.get(&id) {
            let _ = entry.sender.send(snapshot);
        }
    }

    /// Update a session with a callback function.
    ///
    /// This is the preferred way to mutate sessions as it:
    /// 1. Acquires the write lock
    /// 2. Applies the mutation
    /// 3. Updates the session's timestamp
    /// 4. Notifies all watchers
    ///
    /// Returns `None` if the session doesn't exist.
    pub async fn update_session<F>(&self, id: Uuid, f: F) -> Option<()>
    where
        F: FnOnce(&mut AgentSession),
    {
        let snapshot = {
            let mut store = self.store.write().await;

            let session = store.get_mut(&id)?;
            f(session);
            session.updated_at = chrono::Utc::now();

            SessionSnapshot::from_session(session)
        };

        // Notify watchers if any (separate lock scope)
        let watchers = self.watchers.read().await;
        if let Some(entry) = watchers.get(&id) {
            let _ = entry.sender.send(snapshot);
        }

        Some(())
    }

    /// Update a session and return a result.
    ///
    /// Like `update_session` but allows returning a value from the callback.
    pub async fn update_session_with<F, T>(&self, id: Uuid, f: F) -> Option<T>
    where
        F: FnOnce(&mut AgentSession) -> T,
    {
        let (result, snapshot) = {
            let mut store = self.store.write().await;

            let session = store.get_mut(&id)?;
            let result = f(session);
            session.updated_at = chrono::Utc::now();

            let snapshot = SessionSnapshot::from_session(session);
            (result, snapshot)
        };

        // Notify watchers if any (separate lock scope)
        let watchers = self.watchers.read().await;
        if let Some(entry) = watchers.get(&id) {
            let _ = entry.sender.send(snapshot);
        }

        Some(result)
    }

    /// Query a session without mutating it.
    ///
    /// This acquires only a read lock.
    pub async fn query_session<F, T>(&self, id: Uuid, f: F) -> Option<T>
    where
        F: FnOnce(&AgentSession) -> T,
    {
        let store = self.store.read().await;
        store.get(&id).map(f)
    }

    /// Subscribe to session changes.
    ///
    /// Returns a watch::Receiver that yields a new snapshot on every update.
    /// The receiver can call `changed().await` to wait for the next update.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut rx = manager.subscribe(session_id).await?;
    /// while rx.changed().await.is_ok() {
    ///     let snapshot = rx.borrow();
    ///     if snapshot.scope_path != old_path {
    ///         // Handle navigation change
    ///     }
    /// }
    /// ```
    pub async fn subscribe(&self, id: Uuid) -> Option<SessionWatcher> {
        // Check if session exists
        let session = self.get_session(id).await?;

        // Get or create watcher entry
        let mut watchers = self.watchers.write().await;
        let entry = watchers.entry(id).or_insert_with(|| {
            let (tx, _rx) = watch::channel(SessionSnapshot::from_session(&session));
            WatcherEntry {
                sender: tx,
                subscriber_count: 0,
            }
        });

        entry.subscriber_count += 1;

        Some(entry.sender.subscribe())
    }

    /// Unsubscribe from session changes.
    ///
    /// Call this when a subscriber is done listening.
    /// The watch channel is cleaned up when the last subscriber leaves.
    pub async fn unsubscribe(&self, id: Uuid) {
        let mut watchers = self.watchers.write().await;
        if let Some(entry) = watchers.get_mut(&id) {
            entry.subscriber_count = entry.subscriber_count.saturating_sub(1);
            if entry.subscriber_count == 0 {
                watchers.remove(&id);
            }
        }
    }

    /// Get the number of active subscribers for a session
    pub async fn subscriber_count(&self, id: Uuid) -> usize {
        let watchers = self.watchers.read().await;
        watchers.get(&id).map(|e| e.subscriber_count).unwrap_or(0)
    }

    /// Remove a session and clean up its watchers
    pub async fn remove_session(&self, id: Uuid) -> Option<AgentSession> {
        // Remove watcher first
        {
            let mut watchers = self.watchers.write().await;
            watchers.remove(&id);
        }

        // Remove from store
        self.store.write().await.remove(&id)
    }

    /// List all active session IDs
    pub async fn list_session_ids(&self) -> Vec<Uuid> {
        self.store.read().await.keys().cloned().collect()
    }

    /// Get count of active sessions
    pub async fn session_count(&self) -> usize {
        self.store.read().await.len()
    }

    /// Force notify all watchers for a session (useful after external mutation)
    pub async fn notify(&self, id: Uuid) {
        if let Some(session) = self.get_session(id).await {
            let watchers = self.watchers.read().await;
            if let Some(entry) = watchers.get(&id) {
                let _ = entry.sender.send(SessionSnapshot::from_session(&session));
            }
        }
    }
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            watchers: self.watchers.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::session::create_session_store;

    #[tokio::test]
    async fn test_session_manager_basic() {
        let store = create_session_store();
        let manager = SessionManager::new(store);

        // Create a session
        let session = AgentSession::new();
        let id = session.id;
        manager.insert_session(session).await;

        // Verify it exists
        assert!(manager.exists(id).await);
        assert_eq!(manager.session_count().await, 1);

        // Get the session
        let retrieved = manager.get_session(id).await.unwrap();
        assert_eq!(retrieved.id, id);

        // Remove it
        let removed = manager.remove_session(id).await;
        assert!(removed.is_some());
        assert!(!manager.exists(id).await);
    }

    #[tokio::test]
    async fn test_session_update_with_callback() {
        let store = create_session_store();
        let manager = SessionManager::new(store);

        let session = AgentSession::new();
        let id = session.id;
        manager.insert_session(session).await;

        // Update the session
        manager
            .update_session(id, |s| {
                s.context.navigate_to_universe("jurisdiction");
            })
            .await;

        // Verify the update
        let updated = manager.get_session(id).await.unwrap();
        assert_eq!(updated.context.scope_path.to_string(), "/jurisdiction");
    }

    #[tokio::test]
    async fn test_subscription() {
        let store = create_session_store();
        let manager = SessionManager::new(store);

        let session = AgentSession::new();
        let id = session.id;
        manager.insert_session(session).await;

        // Subscribe
        let rx = manager.subscribe(id).await.unwrap();
        assert_eq!(manager.subscriber_count(id).await, 1);

        // Check initial value
        let snapshot = rx.borrow();
        assert_eq!(snapshot.session_id, id);
        drop(snapshot);

        // Unsubscribe
        manager.unsubscribe(id).await;
        assert_eq!(manager.subscriber_count(id).await, 0);
    }

    #[tokio::test]
    async fn test_watch_notification() {
        let store = create_session_store();
        let manager = SessionManager::new(store);

        let session = AgentSession::new();
        let id = session.id;
        manager.insert_session(session).await;

        // Subscribe
        let mut rx = manager.subscribe(id).await.unwrap();

        // Spawn a task to update the session
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            manager_clone
                .update_session(id, |s| {
                    s.context.navigate_to_universe("client_type");
                })
                .await;
        });

        // Wait for the update
        tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.changed())
            .await
            .expect("Timeout waiting for notification")
            .expect("Watch channel closed");

        // Verify the update was received
        let snapshot = rx.borrow();
        assert_eq!(snapshot.scope_path, "/client_type");
    }
}
