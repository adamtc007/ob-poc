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

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use sqlx::{Postgres, Transaction};
use thiserror::Error;

// Re-use the LockKey type from expansion module
pub use crate::dsl_v2::expansion::{LockAccess, LockKey, LockMode};

// =============================================================================
// LOCK KEY DERIVATION
// =============================================================================

/// Derive stable i64 lock key from entity type + UUID
///
/// Uses deterministic hashing - same input always produces same key.
/// The hash is truncated to i64 for PostgreSQL advisory lock compatibility.
///
/// # Example
/// ```ignore
/// let key = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");
/// // key is a stable i64 value
/// ```
pub fn lock_key(entity_type: &str, entity_id: &str) -> i64 {
    let mut hasher = DefaultHasher::new();
    entity_type.hash(&mut hasher);
    entity_id.hash(&mut hasher);
    hasher.finish() as i64
}

/// Derive lock key from a LockKey struct
pub fn lock_key_from_struct(lock: &LockKey) -> i64 {
    lock_key(&lock.entity_type, &lock.entity_id)
}

// =============================================================================
// LOCK ACQUISITION
// =============================================================================

/// Acquire advisory lock (blocks until available)
///
/// The lock is automatically released when the transaction ends (commit or rollback).
/// This is a transaction-level lock, NOT a session-level lock.
///
/// # Arguments
/// * `tx` - Active transaction
/// * `key` - Lock key derived from `lock_key()`
///
/// # Example
/// ```ignore
/// let key = lock_key("person", &person_id);
/// advisory_xact_lock(&mut tx, key).await?;
/// // Lock is held until tx.commit() or tx.rollback()
/// ```
pub async fn advisory_xact_lock(
    tx: &mut Transaction<'_, Postgres>,
    key: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Try to acquire advisory lock (non-blocking)
///
/// Returns `true` if lock was acquired, `false` if already held by another session.
/// Does NOT block - returns immediately.
///
/// # Arguments
/// * `tx` - Active transaction
/// * `key` - Lock key derived from `lock_key()`
///
/// # Example
/// ```ignore
/// let key = lock_key("person", &person_id);
/// if try_advisory_xact_lock(&mut tx, key).await? {
///     // Lock acquired - proceed
/// } else {
///     // Lock held by another session - handle contention
/// }
/// ```
pub async fn try_advisory_xact_lock(
    tx: &mut Transaction<'_, Postgres>,
    key: i64,
) -> Result<bool, sqlx::Error> {
    let result: (bool,) = sqlx::query_as("SELECT pg_try_advisory_xact_lock($1)")
        .bind(key)
        .fetch_one(&mut **tx)
        .await?;
    Ok(result.0)
}

// =============================================================================
// BULK LOCK ACQUISITION
// =============================================================================

/// Result of acquiring multiple locks
#[derive(Debug, Clone)]
pub struct LockAcquisitionResult {
    /// Locks successfully acquired
    pub acquired: Vec<LockKey>,
    /// Time spent waiting for locks (in milliseconds)
    pub wait_time_ms: u64,
}

/// Error during lock acquisition
#[derive(Debug, Error)]
pub enum LockError {
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
/// * `mode` - Acquisition mode (Try = fail fast, Block = wait)
///
/// # Returns
/// * `Ok(LockAcquisitionResult)` - All locks acquired successfully
/// * `Err(LockError::Contention)` - Lock held by another session (only with `LockMode::Try`)
/// * `Err(LockError::Database)` - Database error
///
/// # Example
/// ```ignore
/// let locks = vec![
///     LockKey::write("person", &person_id),
///     LockKey::write("entity", &entity_id),
/// ];
/// match acquire_locks(&mut tx, &locks, LockMode::Try).await {
///     Ok(result) => {
///         println!("Acquired {} locks in {}ms", result.acquired.len(), result.wait_time_ms);
///     }
///     Err(LockError::Contention { entity_type, entity_id, .. }) => {
///         println!("Lock contention on {}:{}", entity_type, entity_id);
///     }
///     Err(e) => return Err(e.into()),
/// }
/// ```
pub async fn acquire_locks(
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

    // For Timeout mode, set a transaction-local statement_timeout BEFORE
    // acquiring locks. `SET LOCAL` scopes it to this transaction only —
    // it does NOT leak to the connection pool. On timeout, PostgreSQL raises
    // error 57014 (query_canceled) which we catch below.
    if let LockMode::Timeout(duration) = mode {
        let ms = duration.as_millis() as i64;
        sqlx::query(&format!("SET LOCAL statement_timeout = '{ms}'"))
            .execute(&mut **tx)
            .await?;
    }

    for lock in &sorted_locks {
        let key = lock_key_from_struct(lock);

        let lock_result = match mode {
            LockMode::Try => {
                // Non-blocking — fail fast if lock unavailable
                try_advisory_xact_lock(tx, key).await
            }
            LockMode::Block | LockMode::Timeout(_) => {
                // Blocking — wait for lock (with optional statement_timeout).
                // On timeout (57014), sqlx returns Err(sqlx::Error::Database(..)).
                advisory_xact_lock(tx, key).await.map(|()| true)
            }
        };

        let lock_acquired = match lock_result {
            Ok(acquired) => acquired,
            Err(e) => {
                // Check for PostgreSQL error 57014 (query_canceled / statement_timeout).
                let is_timeout = e
                    .as_database_error()
                    .map(|db_err| db_err.code().is_some_and(|c| c == "57014"))
                    .unwrap_or(false);

                if is_timeout {
                    // Timeout mode: convert to contention error.
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
        };

        if lock_acquired {
            acquired.push(lock.clone());
            tracing::debug!(
                entity_type = %lock.entity_type,
                entity_id = %lock.entity_id,
                access = ?lock.access,
                "Acquired advisory lock"
            );
        } else {
            tracing::warn!(
                entity_type = %lock.entity_type,
                entity_id = %lock.entity_id,
                access = ?lock.access,
                "Lock contention detected"
            );
            return Err(LockError::Contention {
                entity_type: lock.entity_type.clone(),
                entity_id: lock.entity_id.clone(),
                acquired_so_far: acquired,
                holder_runbook_id: None, // Populated by caller via event store lookup
            });
        }
    }

    // Reset statement_timeout after all locks acquired (Timeout mode only).
    if matches!(mode, LockMode::Timeout(_)) {
        sqlx::query("RESET statement_timeout")
            .execute(&mut **tx)
            .await?;
    }

    let wait_time_ms = start.elapsed().as_millis() as u64;
    tracing::debug!(
        lock_count = acquired.len(),
        wait_time_ms = wait_time_ms,
        "All locks acquired"
    );

    Ok(LockAcquisitionResult {
        acquired,
        wait_time_ms,
    })
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_key_deterministic() {
        let key1 = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");
        let key2 = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");

        assert_eq!(key1, key2, "Same input should produce same key");
    }

    #[test]
    fn test_lock_key_different_for_different_entities() {
        let key1 = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");
        let key2 = lock_key("person", "660e8400-e29b-41d4-a716-446655440001");

        assert_ne!(key1, key2, "Different entities should have different keys");
    }

    #[test]
    fn test_lock_key_different_for_different_types() {
        let key1 = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");
        let key2 = lock_key("entity", "550e8400-e29b-41d4-a716-446655440000");

        assert_ne!(key1, key2, "Different types should have different keys");
    }

    #[test]
    fn test_lock_key_from_struct() {
        let lock = LockKey::write("person", "550e8400-e29b-41d4-a716-446655440000");
        let key1 = lock_key_from_struct(&lock);
        let key2 = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");

        assert_eq!(key1, key2, "Struct and direct call should produce same key");
    }
}
