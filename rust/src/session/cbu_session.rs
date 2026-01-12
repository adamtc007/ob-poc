//! Simplified CBU Session Model
//!
//! Session state is just the set of loaded CBUs with undo/redo history.
//! Focus/camera/clusters are derived client-side from this minimal state.
//!
//! # Design Philosophy
//!
//! **Memory is truth. DB is backup.**
//!
//! Old model (20 verbs):
//! - set-galaxy, set-book, set-cbu, set-jurisdiction, set-neighborhood
//! - focus, clear-focus, add-cbu, remove-cbu, clear-cbu-set, etc.
//!
//! New model (10 verbs):
//! - load-cbu, load-jurisdiction, load-galaxy
//! - unload-cbu, clear
//! - undo, redo
//! - info, list
//!
//! Everything else (clusters, galaxies, control spheres) derived from edges on demand.
//!
//! # Performance Model
//!
//! ```text
//! HOT PATH (60fps, sync):              COLD PATH (background, async):
//! ┌────────────────────────┐           ┌────────────────────────┐
//! │ Session in MEMORY      │           │ DB persistence         │
//! │                        │           │                        │
//! │ • load_cbu()     <1µs  │──fire────▶│ • debounced save ~2s   │
//! │ • unload_cbu()   <1µs  │  and      │ • tokio::spawn         │
//! │ • undo/redo      <1µs  │  forget   │ • errors logged, ignored│
//! │ • queries        <1µs  │           │                        │
//! │                        │◀──────────│ • load on startup only │
//! │ NEVER BLOCKS RENDER    │  once     │                        │
//! └────────────────────────┘           └────────────────────────┘
//! ```
//!
//! # Graceful Degradation
//!
//! | Failure | Result |
//! |---------|--------|
//! | DB down | Session works fine, just lost on refresh |
//! | Load timeout | Fresh session, no crash |
//! | Save fails | Logged, swallowed, retry next cycle |
//! | Corrupt data | Fresh session, no crash |

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Debounce interval for background saves
const SAVE_DEBOUNCE_SECS: u64 = 2;

/// Timeout for loading session from DB
const LOAD_TIMEOUT_SECS: u64 = 2;

// =============================================================================
// SESSION STATE
// =============================================================================

/// Minimal session state - just the set of loaded CBU IDs
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CbuSessionState {
    /// The set of CBU IDs currently loaded in this session
    pub cbu_ids: HashSet<Uuid>,
}

impl CbuSessionState {
    /// Create empty state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create state with initial CBUs
    pub fn with_cbus(cbu_ids: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            cbu_ids: cbu_ids.into_iter().collect(),
        }
    }

    /// Number of CBUs loaded
    pub fn count(&self) -> usize {
        self.cbu_ids.len()
    }

    /// Check if a CBU is loaded
    pub fn contains(&self, cbu_id: Uuid) -> bool {
        self.cbu_ids.contains(&cbu_id)
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.cbu_ids.is_empty()
    }
}

// =============================================================================
// DATABASE ROW TYPES (for runtime queries)
// =============================================================================

/// Row type for loading session from DB
#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct SessionDbRow {
    id: Uuid,
    name: Option<String>,
    cbu_ids: Vec<Uuid>,
    history: serde_json::Value,
    future: serde_json::Value,
}

/// Row type for session summary list
#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct SessionSummaryRow {
    id: Uuid,
    name: Option<String>,
    cbu_count: Option<i32>,
    updated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

// =============================================================================
// SESSION WITH HISTORY
// =============================================================================

/// CBU session with undo/redo history and background persistence
#[derive(Debug)]
pub struct CbuSession {
    /// Session ID
    pub id: Uuid,

    /// Optional friendly name
    pub name: Option<String>,

    /// Current state
    pub state: CbuSessionState,

    /// Undo stack (previous states)
    history: Vec<CbuSessionState>,

    /// Redo stack (future states after undo)
    future: Vec<CbuSessionState>,

    /// Max history depth (prevents unbounded memory growth)
    max_history: usize,

    // =========================================================================
    // Persistence tracking (not persisted to DB)
    // =========================================================================
    /// Whether state has changed since last save
    dirty: bool,

    /// Last save timestamp (for debouncing)
    last_saved: Instant,
}

impl Clone for CbuSession {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            state: self.state.clone(),
            history: self.history.clone(),
            future: self.future.clone(),
            max_history: self.max_history,
            dirty: self.dirty,
            last_saved: Instant::now(), // Reset on clone
        }
    }
}

impl Default for CbuSession {
    fn default() -> Self {
        Self::new()
    }
}

impl CbuSession {
    /// Create a new empty session
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: None,
            state: CbuSessionState::new(),
            history: Vec::new(),
            future: Vec::new(),
            max_history: 50,
            dirty: false,
            last_saved: Instant::now(),
        }
    }

    /// Create session with specific ID
    pub fn with_id(id: Uuid) -> Self {
        Self { id, ..Self::new() }
    }

    /// Create session with name
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::new()
        }
    }

    /// Get session ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get session name
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Check if session has unsaved changes
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark session as dirty (needs save)
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    // =========================================================================
    // HISTORY MANAGEMENT
    // =========================================================================

    /// Push current state to history before mutation
    fn push_history(&mut self) {
        self.history.push(self.state.clone());

        // Trim history if too long
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }

        // Clear redo stack on new action
        self.future.clear();

        // Mark dirty for persistence
        self.mark_dirty();
    }

    /// Undo last action
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.future.push(self.state.clone());
            self.state = prev;
            self.mark_dirty();
            true
        } else {
            false
        }
    }

    /// Redo previously undone action
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.future.pop() {
            self.history.push(self.state.clone());
            self.state = next;
            self.mark_dirty();
            true
        } else {
            false
        }
    }

    /// Get history depth (number of undoable actions)
    pub fn history_depth(&self) -> usize {
        self.history.len()
    }

    /// Get future depth (number of redoable actions)
    pub fn future_depth(&self) -> usize {
        self.future.len()
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    // =========================================================================
    // MUTATIONS (all sync, push history first, <1µs)
    // =========================================================================

    /// Load a single CBU into the session
    /// Returns true if the CBU was newly added
    pub fn load_cbu(&mut self, cbu_id: Uuid) -> bool {
        if self.state.cbu_ids.contains(&cbu_id) {
            return false;
        }
        self.push_history();
        self.state.cbu_ids.insert(cbu_id);
        true
    }

    /// Load multiple CBUs into the session
    /// Returns the count of newly added CBUs
    pub fn load_many(&mut self, ids: impl IntoIterator<Item = Uuid>) -> usize {
        let new_ids: Vec<Uuid> = ids
            .into_iter()
            .filter(|id| !self.state.cbu_ids.contains(id))
            .collect();

        if new_ids.is_empty() {
            return 0;
        }

        self.push_history();
        let count = new_ids.len();
        self.state.cbu_ids.extend(new_ids);
        count
    }

    /// Unload a CBU from the session
    /// Returns true if the CBU was present and removed
    pub fn unload_cbu(&mut self, cbu_id: Uuid) -> bool {
        if !self.state.cbu_ids.contains(&cbu_id) {
            return false;
        }
        self.push_history();
        self.state.cbu_ids.remove(&cbu_id);
        true
    }

    /// Clear all CBUs from the session
    /// Returns the count of removed CBUs
    pub fn clear(&mut self) -> usize {
        if self.state.cbu_ids.is_empty() {
            return 0;
        }
        self.push_history();
        let count = self.state.cbu_ids.len();
        self.state.cbu_ids.clear();
        count
    }

    // =========================================================================
    // QUERIES
    // =========================================================================

    /// Get count of loaded CBUs
    pub fn count(&self) -> usize {
        self.state.count()
    }

    /// Check if a CBU is loaded
    pub fn contains(&self, cbu_id: Uuid) -> bool {
        self.state.contains(cbu_id)
    }

    /// Check if session is empty
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    /// Get all loaded CBU IDs
    pub fn cbu_ids(&self) -> impl Iterator<Item = &Uuid> {
        self.state.cbu_ids.iter()
    }

    /// Get CBU IDs as a Vec (for SQL queries)
    pub fn cbu_ids_vec(&self) -> Vec<Uuid> {
        self.state.cbu_ids.iter().copied().collect()
    }

    // =========================================================================
    // PERSISTENCE (fire-and-forget, never blocks)
    // =========================================================================

    /// Try to save session if dirty and debounce period has passed.
    ///
    /// **NEVER BLOCKS.** Spawns background task and returns immediately.
    /// Errors are logged and swallowed - session keeps working.
    #[cfg(feature = "database")]
    pub fn maybe_save(&mut self, pool: &PgPool) {
        if !self.dirty {
            return;
        }
        if self.last_saved.elapsed() < Duration::from_secs(SAVE_DEBOUNCE_SECS) {
            return;
        }

        // Snapshot current state for background save
        let snapshot = SessionSnapshot {
            id: self.id,
            name: self.name.clone(),
            cbu_ids: self.state.cbu_ids.iter().copied().collect(),
            history: self.history.clone(),
            future: self.future.clone(),
        };

        let pool = pool.clone();

        // Fire and forget - NEVER await this in hot path
        tokio::spawn(async move {
            match snapshot.persist(&pool).await {
                Ok(_) => debug!("Session {} saved", snapshot.id),
                Err(e) => warn!("Session save failed (non-fatal): {}", e),
            }
        });

        self.dirty = false;
        self.last_saved = Instant::now();
    }

    /// Force immediate save (blocking). Use sparingly - only for shutdown.
    #[cfg(feature = "database")]
    pub async fn force_save(&mut self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let snapshot = SessionSnapshot {
            id: self.id,
            name: self.name.clone(),
            cbu_ids: self.state.cbu_ids.iter().copied().collect(),
            history: self.history.clone(),
            future: self.future.clone(),
        };

        snapshot.persist(pool).await?;
        self.dirty = false;
        self.last_saved = Instant::now();
        Ok(())
    }

    /// Load session from DB, or create new if not found/error.
    ///
    /// Has timeout - never hangs. On any failure, returns fresh session.
    #[cfg(feature = "database")]
    pub async fn load_or_new(id: Option<Uuid>, pool: &PgPool) -> Self {
        if let Some(id) = id {
            match tokio::time::timeout(
                Duration::from_secs(LOAD_TIMEOUT_SECS),
                Self::load_from_db(id, pool),
            )
            .await
            {
                Ok(Ok(Some(session))) => {
                    debug!("Session {} loaded from DB", id);
                    return session;
                }
                Ok(Ok(None)) => debug!("Session {} not found, creating new", id),
                Ok(Err(e)) => warn!("Session load failed (non-fatal): {}", e),
                Err(_) => warn!("Session load timed out (non-fatal)"),
            }
        }

        // Fallback: fresh session
        Self::new()
    }

    /// Load session from database
    #[cfg(feature = "database")]
    async fn load_from_db(id: Uuid, pool: &PgPool) -> Result<Option<Self>, sqlx::Error> {
        // Use runtime query to avoid compile-time schema validation
        // (sessions table may not exist until migration 023 is run)
        let row: Option<SessionDbRow> = sqlx::query_as(
            r#"
            SELECT id, name, cbu_ids, history, future
            FROM "ob-poc".sessions
            WHERE id = $1 AND expires_at > NOW()
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| {
            let history: Vec<CbuSessionState> =
                serde_json::from_value(r.history).unwrap_or_default();
            let future: Vec<CbuSessionState> = serde_json::from_value(r.future).unwrap_or_default();

            Self {
                id: r.id,
                name: r.name,
                state: CbuSessionState {
                    cbu_ids: r.cbu_ids.into_iter().collect(),
                },
                history,
                future,
                max_history: 50,
                dirty: false,
                last_saved: Instant::now(),
            }
        }))
    }

    /// Delete session from database
    #[cfg(feature = "database")]
    pub async fn delete(id: Uuid, pool: &PgPool) -> Result<bool, sqlx::Error> {
        // Use runtime query to avoid compile-time schema validation
        let result = sqlx::query(r#"DELETE FROM "ob-poc".sessions WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List recent sessions for a user (or anonymous)
    #[cfg(feature = "database")]
    pub async fn list_recent(
        user_id: Option<Uuid>,
        limit: i64,
        pool: &PgPool,
    ) -> Result<Vec<SessionSummary>, sqlx::Error> {
        // Use runtime query to avoid compile-time schema validation
        let rows: Vec<SessionSummaryRow> = sqlx::query_as(
            r#"
            SELECT
                id,
                name,
                array_length(cbu_ids, 1) as cbu_count,
                updated_at,
                expires_at
            FROM "ob-poc".sessions
            WHERE ($1::uuid IS NULL AND user_id IS NULL) OR user_id = $1
            AND expires_at > NOW()
            ORDER BY updated_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SessionSummary {
                id: r.id,
                name: r.name,
                cbu_count: r.cbu_count.unwrap_or(0),
                updated_at: r.updated_at,
                expires_at: r.expires_at,
            })
            .collect())
    }

    /// List all sessions (regardless of user)
    #[cfg(feature = "database")]
    pub async fn list_all(pool: &PgPool, limit: usize) -> Vec<SessionSummary> {
        // Use runtime query to avoid compile-time schema validation
        let rows: Result<Vec<SessionSummaryRow>, _> = sqlx::query_as(
            r#"
            SELECT
                id,
                name,
                array_length(cbu_ids, 1) as cbu_count,
                updated_at,
                expires_at
            FROM "ob-poc".sessions
            WHERE expires_at > NOW()
            ORDER BY updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(pool)
        .await;

        match rows {
            Ok(rows) => rows
                .into_iter()
                .map(|r| SessionSummary {
                    id: r.id,
                    name: r.name,
                    cbu_count: r.cbu_count.unwrap_or(0),
                    updated_at: r.updated_at,
                    expires_at: r.expires_at,
                })
                .collect(),
            Err(e) => {
                warn!("Failed to list sessions (non-fatal): {}", e);
                vec![]
            }
        }
    }
}

// =============================================================================
// SNAPSHOT FOR BACKGROUND PERSISTENCE
// =============================================================================

/// Immutable snapshot of session state for background save
#[derive(Clone)]
struct SessionSnapshot {
    id: Uuid,
    name: Option<String>,
    cbu_ids: Vec<Uuid>,
    history: Vec<CbuSessionState>,
    future: Vec<CbuSessionState>,
}

impl SessionSnapshot {
    #[cfg(feature = "database")]
    async fn persist(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        // Use runtime query to avoid compile-time schema validation
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".sessions (id, name, cbu_ids, history, future)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                cbu_ids = EXCLUDED.cbu_ids,
                history = EXCLUDED.history,
                future = EXCLUDED.future
            "#,
        )
        .bind(self.id)
        .bind(self.name.as_deref())
        .bind(&self.cbu_ids)
        .bind(serde_json::to_value(&self.history).unwrap_or_default())
        .bind(serde_json::to_value(&self.future).unwrap_or_default())
        .execute(pool)
        .await?;
        Ok(())
    }
}

// =============================================================================
// RESULT TYPES
// =============================================================================

/// Result from loading a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadCbuResult {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub total_loaded: usize,
    pub was_new: bool,
}

/// Result from loading by jurisdiction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadJurisdictionResult {
    pub jurisdiction: String,
    pub count_added: usize,
    pub total_loaded: usize,
}

/// Result from loading a galaxy (apex entity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadGalaxyResult {
    pub apex_name: String,
    pub count_added: usize,
    pub total_loaded: usize,
}

/// Result from unloading a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnloadCbuResult {
    pub cbu_id: Uuid,
    pub name: String,
    pub total_loaded: usize,
    pub was_present: bool,
}

/// Result from clearing the session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearResult {
    pub count_removed: usize,
}

/// Result from undo/redo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryResult {
    pub success: bool,
    pub total_loaded: usize,
    pub history_depth: usize,
    pub future_depth: usize,
}

/// Session info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub name: Option<String>,
    pub total_cbus: usize,
    pub jurisdictions: Vec<JurisdictionCount>,
    pub history_depth: usize,
    pub future_depth: usize,
}

/// Jurisdiction count for session info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JurisdictionCount {
    pub jurisdiction: String,
    pub count: i64,
}

/// CBU summary for list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
}

/// Session summary for list-sessions response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub cbu_count: i32,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_cbu() {
        let mut session = CbuSession::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert!(session.load_cbu(id1));
        assert_eq!(session.count(), 1);
        assert!(session.is_dirty());

        // Duplicate load returns false
        assert!(!session.load_cbu(id1));
        assert_eq!(session.count(), 1);

        assert!(session.load_cbu(id2));
        assert_eq!(session.count(), 2);
    }

    #[test]
    fn test_undo_redo() {
        let mut session = CbuSession::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        session.load_cbu(id1);
        session.load_cbu(id2);

        assert_eq!(session.count(), 2);
        assert_eq!(session.history_depth(), 2);

        // Undo should remove id2
        assert!(session.undo());
        assert_eq!(session.count(), 1);
        assert!(session.contains(id1));
        assert!(!session.contains(id2));

        // Redo should restore id2
        assert!(session.redo());
        assert_eq!(session.count(), 2);
        assert!(session.contains(id2));
    }

    #[test]
    fn test_clear() {
        let mut session = CbuSession::new();
        session.load_many([Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()]);

        assert_eq!(session.count(), 3);

        let removed = session.clear();
        assert_eq!(removed, 3);
        assert_eq!(session.count(), 0);

        // Undo should restore all
        assert!(session.undo());
        assert_eq!(session.count(), 3);
    }

    #[test]
    fn test_load_many() {
        let mut session = CbuSession::new();
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        let added = session.load_many(ids.clone());
        assert_eq!(added, 5);

        // Loading same IDs again adds nothing
        let added2 = session.load_many(ids);
        assert_eq!(added2, 0);
        assert_eq!(session.history_depth(), 1); // Only one history entry
    }

    #[test]
    fn test_redo_cleared_on_new_action() {
        let mut session = CbuSession::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        session.load_cbu(id1);
        session.undo();

        assert!(session.can_redo());

        // New action clears redo stack
        session.load_cbu(id2);
        assert!(!session.can_redo());
    }

    #[test]
    fn test_dirty_flag() {
        let mut session = CbuSession::new();
        assert!(!session.is_dirty());

        session.load_cbu(Uuid::new_v4());
        assert!(session.is_dirty());
    }

    #[test]
    fn test_with_name() {
        let session = CbuSession::with_name("My Session");
        assert_eq!(session.name, Some("My Session".to_string()));
    }
}
