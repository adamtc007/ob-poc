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

use crate::api::session::SessionStore;
use crate::session::UnifiedSession;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use uuid::Uuid;

/// Lightweight snapshot of session state for watch channel.
///
/// We send a snapshot rather than the full session to avoid
/// holding locks while subscribers process updates.
#[derive(Debug, Clone)]
pub(crate) struct SessionSnapshot {
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
    /// Session scope definition (galaxy, book, cbu, jurisdiction, neighborhood)
    /// This is the primary scope from session.* verbs
    pub scope_definition: Option<crate::graph::GraphScope>,
    /// Whether scope has data loaded
    pub scope_loaded: bool,
}

impl SessionSnapshot {
    /// Create a snapshot from a session
    pub(crate) fn from_session(session: &UnifiedSession) -> Self {
        // Extract scope info from session context
        let (scope_definition, scope_loaded) = session
            .context
            .scope
            .as_ref()
            .map(|s| (Some(s.definition.clone()), s.is_fully_loaded()))
            .unwrap_or((None, false));

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
            scope_definition,
            scope_loaded,
        }
    }

}

/// Session watcher - a receiver that yields on every session update.
pub(crate) type SessionWatcher = watch::Receiver<SessionSnapshot>;

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
pub(crate) struct SessionManager {
    /// The underlying session store
    store: SessionStore,

    /// Watch channels per session (created on-demand)
    watchers: WatcherMap,
}

impl SessionManager {
    /// Create a new SessionManager wrapping an existing store
    pub(crate) fn new(store: SessionStore) -> Self {
        Self {
            store,
            watchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a session by ID (read-only clone)
    pub(crate) async fn get_session(&self, id: Uuid) -> Option<UnifiedSession> {
        self.store.read().await.get(&id).cloned()
    }

    /// Update a session and return a result.
    ///
    /// Like `update_session` but allows returning a value from the callback.
    pub(crate) async fn update_session_with<F, T>(&self, id: Uuid, f: F) -> Option<T>
    where
        F: FnOnce(&mut UnifiedSession) -> T,
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
    pub(crate) async fn subscribe(&self, id: Uuid) -> Option<SessionWatcher> {
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
    pub(crate) async fn unsubscribe(&self, id: Uuid) {
        let mut watchers = self.watchers.write().await;
        if let Some(entry) = watchers.get_mut(&id) {
            entry.subscriber_count = entry.subscriber_count.saturating_sub(1);
            if entry.subscriber_count == 0 {
                watchers.remove(&id);
            }
        }
    }

    /// Force notify all watchers for a session (useful after external mutation)
    pub(crate) async fn notify(&self, id: Uuid) {
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

// =============================================================================
// DSL Diff Tracking for Learning Loop
// =============================================================================

/// A single edit made by the user to the proposed DSL
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct DslEdit {
    /// The field/parameter that was changed
    pub field: String,
    /// Original value (from proposed DSL)
    pub from: String,
    /// New value (in final DSL)
    pub to: String,
}

/// Result of computing DSL diff
#[derive(Debug, Clone)]
pub(crate) struct DslDiff {
    /// Individual edits detected
    pub edits: Vec<DslEdit>,
    /// Whether any edits were made
    pub was_edited: bool,
}

impl SessionManager {
    // =========================================================================
    // DSL Diff Tracking Methods
    // =========================================================================

    /// Capture DSL diff and clear tracking state (called before execute)
    ///
    /// Returns the diff between proposed and current DSL, then clears the
    /// tracking fields so the next interaction starts fresh.
    pub(crate) async fn capture_dsl_diff(&self, session_id: Uuid, final_dsl: &str) -> Option<DslDiff> {
        self.update_session_with(session_id, |session| {
            let proposed = session.context.proposed_dsl.take();
            let _ = session.context.current_dsl.take(); // Clear but don't need

            match proposed {
                Some(proposed_dsl) => {
                    let edits = compute_dsl_edits(&proposed_dsl, final_dsl);
                    let was_edited = !edits.is_empty();

                    Some(DslDiff { edits, was_edited })
                }
                None => {
                    // No proposed DSL - this might be direct DSL input
                    // Still return a diff structure but with no edits
                    Some(DslDiff {
                        edits: vec![],
                        was_edited: false,
                    })
                }
            }
        })
        .await
        .flatten()
    }
}

/// Compute field-level edits between two DSL strings
///
/// This is a simple diff that looks for parameter value changes.
/// For a more sophisticated diff, we could parse both and compare ASTs.
fn compute_dsl_edits(proposed: &str, final_dsl: &str) -> Vec<DslEdit> {
    use crate::dsl_v2::parse_program;

    let mut edits = Vec::new();

    // Try to parse both DSL strings
    let proposed_ast = match parse_program(proposed) {
        Ok(ast) => ast,
        Err(_) => return edits, // Can't diff if we can't parse
    };

    let final_ast = match parse_program(final_dsl) {
        Ok(ast) => ast,
        Err(_) => return edits,
    };

    // Compare statements (simple case: same number of statements)
    for (prop_stmt, final_stmt) in proposed_ast
        .statements
        .iter()
        .zip(final_ast.statements.iter())
    {
        // Compare verb calls
        if let (
            crate::dsl_v2::Statement::VerbCall(prop_call),
            crate::dsl_v2::Statement::VerbCall(final_call),
        ) = (prop_stmt, final_stmt)
        {
            // Same verb, compare arguments
            if prop_call.domain == final_call.domain && prop_call.verb == final_call.verb {
                // Build arg maps for comparison
                let prop_args: std::collections::HashMap<_, _> = prop_call
                    .arguments
                    .iter()
                    .map(|a| (a.key.clone(), format!("{:?}", a.value)))
                    .collect();

                let final_args: std::collections::HashMap<_, _> = final_call
                    .arguments
                    .iter()
                    .map(|a| (a.key.clone(), format!("{:?}", a.value)))
                    .collect();

                // Find changed args
                for (key, final_val) in &final_args {
                    if let Some(prop_val) = prop_args.get(key) {
                        if prop_val != final_val {
                            edits.push(DslEdit {
                                field: key.clone(),
                                from: prop_val.clone(),
                                to: final_val.clone(),
                            });
                        }
                    } else {
                        // New arg added
                        edits.push(DslEdit {
                            field: key.clone(),
                            from: String::new(),
                            to: final_val.clone(),
                        });
                    }
                }

                // Find removed args
                for (key, prop_val) in &prop_args {
                    if !final_args.contains_key(key) {
                        edits.push(DslEdit {
                            field: key.clone(),
                            from: prop_val.clone(),
                            to: String::new(),
                        });
                    }
                }
            }
        }
    }

    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::session::create_session_store;

    #[tokio::test]
    async fn test_session_update_with_callback() {
        let store = create_session_store();
        let manager = SessionManager::new(store);

        let session = UnifiedSession::new();
        let id = session.id;
        manager.store.write().await.insert(id, session);

        // Update the session
        manager
            .update_session_with(id, |s| {
                s.context.business_reference = Some("Aviva Lux 9".to_string());
            })
            .await;

        // Verify the update
        let updated = manager.get_session(id).await.unwrap();
        assert_eq!(
            updated.context.business_reference,
            Some("Aviva Lux 9".to_string())
        );
    }

    #[tokio::test]
    async fn test_subscription() {
        let store = create_session_store();
        let manager = SessionManager::new(store);

        let session = UnifiedSession::new();
        let id = session.id;
        manager.store.write().await.insert(id, session);

        // Subscribe
        let rx = manager.subscribe(id).await.unwrap();

        // Check initial value
        let snapshot = rx.borrow();
        assert_eq!(snapshot.session_id, id);
        drop(snapshot);

        // Unsubscribe (no observable state beyond not panicking / cleanup)
        manager.unsubscribe(id).await;
    }

    #[tokio::test]
    async fn test_watch_notification() {
        let store = create_session_store();
        let manager = SessionManager::new(store);

        let session = UnifiedSession::new();
        let id = session.id;
        manager.store.write().await.insert(id, session);

        // Subscribe
        let mut rx = manager.subscribe(id).await.unwrap();

        // Spawn a task to update the session
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            manager_clone
                .update_session_with(id, |s| {
                    s.context.business_reference = Some("client_type".to_string());
                })
                .await;
        });

        // Wait for the update
        tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.changed())
            .await
            .expect("Timeout waiting for notification")
            .expect("Watch channel closed");

        // Verify the update was received (version increments on every update)
        let snapshot = rx.borrow();
        assert_eq!(snapshot.session_id, id);
    }
}
