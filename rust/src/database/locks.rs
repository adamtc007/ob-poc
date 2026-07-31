//! Advisory Lock Management for DSL Execution
//!
//! Provides PostgreSQL advisory locks to prevent concurrent modification
//! of entities during batch execution.
//!
//! ## Why Advisory Locks?
//!
//! Without locking, a concurrent session could delete an entity mid-batch:
//! ```text
//! Session A: LINK person_123 → 50 CBUs (batch)
//!   T0-T20: Verbs 1-20 succeed (edges created)
//!   T21: Session B deletes person_123
//!   T22-T50: Verbs 21-50 fail (person deleted)
//!   Result: 20/50 partial state — inconsistent
//! ```
//!
//! With advisory locks, Session B's delete blocks until Session A completes.
//!
//! ## Lock Key Derivation
//!
//! Lock keys are derived from (entity_type, entity_id) pairs using deterministic
//! hashing. This ensures:
//! - Same entity always gets same lock key
//! - Different entities get different keys (with high probability)
//! - Keys are stable across restarts
//!
//! ## Deadlock Prevention
//!
//! Locks MUST be acquired in sorted order (by entity_type, then entity_id).
//! The `acquire_locks` function enforces this, but callers should pre-sort
//! for efficiency.

use sqlx::{Postgres, Transaction};
use thiserror::Error;

// Lock type definitions (LockKey, LockMode, LockAccess) live here.
// They previously lived in dsl_v2::expansion; when the template-expansion
// stage was deleted (atomic-path removal, 2026-07-31) the types were
// relocated because their surviving consumers (acquire_locks below, the
// runbook executor) are all in the database/runbook tier. The typeless
// SQL primitives (lock_key, advisory_xact_lock, try_advisory_xact_lock)
// were lifted to ob-poc-boundary in Phase 3 slice 2p so the boundary
// tier can call them directly without re-entering src/.
pub use ob_poc_derived_attributes::advisory_lock::{
    advisory_xact_lock, lock_key, try_advisory_xact_lock,
};

/// Lock access type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LockAccess {
    /// Read lock - allows concurrent readers, blocks writers
    Read,
    /// Write lock - exclusive access
    Write,
}

/// Lock acquisition mode
///
/// The non-blocking `Try` variant was deleted with the atomic execution
/// path (2026-07-31) — its only production constructor. All surviving
/// callers (runbook executor) acquire with a bounded blocking wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LockMode {
    /// Blocking with timeout: wait up to `duration`, then fail with contention error.
    ///
    /// Implementation: `SET LOCAL statement_timeout = '<ms>'` before
    /// `pg_advisory_xact_lock()`, then `RESET statement_timeout` after.
    /// `SET LOCAL` scopes the timeout to the current transaction only —
    /// it does NOT leak to the connection pool. If the lock is not acquired
    /// within the duration, PostgreSQL raises error 57014 (query_canceled),
    /// which is caught and converted to `LockError::Contention`.
    Timeout(std::time::Duration),
}

/// A concrete lock key for an entity
///
/// Lock keys are sorted before acquisition to prevent deadlocks.
/// The sort order is: (entity_type, entity_id, access).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct LockKey {
    /// Entity type (e.g., "person", "entity", "cbu")
    pub entity_type: String,
    /// Entity UUID as string
    pub entity_id: String,
    /// Access type
    pub access: LockAccess,
}

impl LockKey {
    /// Create a new lock key
    pub(crate) fn new(
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        access: LockAccess,
    ) -> Self {
        Self {
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            access,
        }
    }

    /// Create a write lock key
    pub(crate) fn write(entity_type: impl Into<String>, entity_id: impl Into<String>) -> Self {
        Self::new(entity_type, entity_id, LockAccess::Write)
    }
}

impl PartialOrd for LockKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LockKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.entity_type, &self.entity_id, &self.access).cmp(&(
            &other.entity_type,
            &other.entity_id,
            &other.access,
        ))
    }
}

// =============================================================================
// LOCK KEY DERIVATION (struct overload)
// =============================================================================

/// Derive lock key from a LockKey struct
pub(crate) fn lock_key_from_struct(lock: &LockKey) -> i64 {
    lock_key(&lock.entity_type, &lock.entity_id)
}

// =============================================================================
// BULK LOCK ACQUISITION
// =============================================================================

/// Result of acquiring multiple locks
#[derive(Debug, Clone)]
pub(crate) struct LockAcquisitionResult {
    /// Locks successfully acquired
    pub acquired: Vec<LockKey>,
}

/// Error during lock acquisition
#[derive(Debug, Error)]
pub(crate) enum LockError {
    /// Lock contention - another session holds the lock
    #[error("Lock contention on {entity_type}:{entity_id}")]
    Contention {
        entity_type: String,
        entity_id: String,
        /// Locks that were acquired before contention occurred
        acquired_so_far: Vec<LockKey>,
        /// Best-effort: the compiled runbook ID currently holding the lock (INV-10).
        /// Populated by querying `compiled_runbook_events` for the most recent
        /// `lock_acquired` event on the contested entity. `None` if lookup fails
        /// or no holder is found (advisory locks are anonymous in PostgreSQL).
        holder_runbook_id: Option<uuid::Uuid>,
    },

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Acquire multiple locks in sorted order (deadlock prevention)
///
/// Locks are acquired in sorted order by (entity_type, entity_id, access) to prevent
/// deadlocks. The caller can pre-sort for efficiency, but this function ensures
/// correct ordering regardless.
///
/// # Arguments
/// * `tx` - Active transaction
/// * `locks` - Lock keys to acquire
/// * `mode` - Acquisition mode (bounded blocking wait)
///
/// # Returns
/// * `Ok(LockAcquisitionResult)` - All locks acquired successfully
/// * `Err(LockError::Contention)` - Lock held by another session past the timeout
/// * `Err(LockError::Database)` - Database error
///
/// # Example
/// ```ignore
/// let locks = vec![
///     LockKey::write("person", &person_id),
///     LockKey::write("entity", &entity_id),
/// ];
/// match acquire_locks(&mut tx, &locks, LockMode::Timeout(Duration::from_secs(30))).await {
///     Ok(result) => {
///         println!("Acquired {} locks", result.acquired.len());
///     }
///     Err(LockError::Contention { entity_type, entity_id, .. }) => {
///         println!("Lock contention on {}:{}", entity_type, entity_id);
///     }
///     Err(e) => return Err(e.into()),
/// }
/// ```
pub(crate) async fn acquire_locks(
    tx: &mut Transaction<'_, Postgres>,
    locks: &[LockKey],
    mode: LockMode,
) -> Result<LockAcquisitionResult, LockError> {
    let start = std::time::Instant::now();
    let mut acquired = Vec::with_capacity(locks.len());

    // Sort locks to prevent deadlocks
    // Caller may have pre-sorted, but we ensure correctness here
    let mut sorted_locks = locks.to_vec();
    sorted_locks.sort();

    // Deduplicate - same lock shouldn't be acquired twice
    sorted_locks.dedup();

    // Set a transaction-local statement_timeout BEFORE acquiring locks.
    // `SET LOCAL` scopes it to this transaction only — it does NOT leak to
    // the connection pool. On timeout, PostgreSQL raises error 57014
    // (query_canceled) which we catch below.
    let LockMode::Timeout(duration) = mode;
    let ms = duration.as_millis() as i64;
    sqlx::query(&format!("SET LOCAL statement_timeout = '{ms}'"))
        .execute(&mut **tx)
        .await?;

    for lock in &sorted_locks {
        let key = lock_key_from_struct(lock);

        // Blocking — wait for lock, bounded by statement_timeout.
        // On timeout (57014), sqlx returns Err(sqlx::Error::Database(..)).
        match advisory_xact_lock(tx, key).await {
            Ok(()) => {
                acquired.push(lock.clone());
                tracing::debug!(
                    entity_type = %lock.entity_type,
                    entity_id = %lock.entity_id,
                    access = ?lock.access,
                    "Acquired advisory lock"
                );
            }
            Err(e) => {
                // Check for PostgreSQL error 57014 (query_canceled / statement_timeout).
                let is_timeout = e
                    .as_database_error()
                    .map(|db_err| db_err.code().is_some_and(|c| c == "57014"))
                    .unwrap_or(false);

                if is_timeout {
                    // Timed out waiting: convert to contention error.
                    tracing::warn!(
                        entity_type = %lock.entity_type,
                        entity_id = %lock.entity_id,
                        "Lock acquisition timed out (statement_timeout)"
                    );
                    return Err(LockError::Contention {
                        entity_type: lock.entity_type.clone(),
                        entity_id: lock.entity_id.clone(),
                        acquired_so_far: acquired,
                        holder_runbook_id: None, // Populated by caller via event store lookup
                    });
                }
                return Err(LockError::Database(e));
            }
        }
    }

    // Reset statement_timeout after all locks acquired.
    sqlx::query("RESET statement_timeout")
        .execute(&mut **tx)
        .await?;

    tracing::debug!(
        lock_count = acquired.len(),
        wait_time_ms = start.elapsed().as_millis() as u64,
        "All locks acquired"
    );

    Ok(LockAcquisitionResult { acquired })
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // lock_key determinism + collision tests live in
    // ob-poc-boundary::advisory_lock — this test covers the LockKey
    // struct overload that stays in src/.
    #[test]
    fn test_lock_key_from_struct() {
        let lock = LockKey::write("person", "550e8400-e29b-41d4-a716-446655440000");
        let key1 = lock_key_from_struct(&lock);
        let key2 = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");

        assert_eq!(key1, key2, "Struct and direct call should produce same key");
    }

    #[test]
    fn test_lock_key_ordering() {
        let mut keys = [
            LockKey::write("person", "uuid-3"),
            LockKey::write("cbu", "uuid-1"),
            LockKey::new("person", "uuid-2", LockAccess::Read),
            LockKey::write("person", "uuid-2"),
        ];

        keys.sort();

        // Should be sorted by (entity_type, entity_id, access); Read < Write
        assert_eq!(keys[0].entity_type, "cbu");
        assert_eq!(keys[1].entity_type, "person");
        assert_eq!(keys[1].entity_id, "uuid-2");
        assert_eq!(keys[1].access, LockAccess::Read);
        assert_eq!(keys[2].entity_type, "person");
        assert_eq!(keys[2].entity_id, "uuid-2");
        assert_eq!(keys[2].access, LockAccess::Write);
        assert_eq!(keys[3].entity_type, "person");
        assert_eq!(keys[3].entity_id, "uuid-3");
    }

    #[test]
    fn test_lock_key_deduplication() {
        let mut keys = vec![
            LockKey::write("person", "uuid-1"),
            LockKey::write("person", "uuid-1"),
            LockKey::write("person", "uuid-1"),
        ];

        keys.sort();
        keys.dedup();

        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn test_lock_key_constructors() {
        let write_key = LockKey::write("person", "uuid-123");
        assert_eq!(write_key.access, LockAccess::Write);
    }
}
