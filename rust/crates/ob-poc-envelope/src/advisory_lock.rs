//! Transport-neutral PostgreSQL advisory lock primitives.
//!
//! This is the typeless slice of the locks surface — `lock_key`,
//! `advisory_xact_lock`, `try_advisory_xact_lock`. They take only `&str`
//! and `i64` and so have zero coupling to expansion-tier `LockKey` /
//! `LockMode` types in `dsl_v2::expansion`. The richer `acquire_locks` +
//! `LockError` helpers stay in `crate::database::locks` in src/ for now
//! (they need the expansion types) and re-export the helpers below.
//!
//! ## Lock Key Derivation
//!
//! Lock keys are derived from `(entity_type, entity_id)` pairs using
//! deterministic hashing. Same entity → same key, stable across restarts,
//! truncated to `i64` for PostgreSQL advisory-lock compatibility.
//!
//! ## Lifetime
//!
//! `advisory_xact_lock` and `try_advisory_xact_lock` acquire **transaction
//! level** locks (`pg_advisory_xact_lock` / `pg_try_advisory_xact_lock`).
//! The lock is released automatically when the transaction ends.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use sqlx::{Postgres, Transaction};

/// Derive stable `i64` lock key from `(entity_type, entity_id)`.
///
/// Deterministic — same input always produces same key.
pub fn lock_key(entity_type: &str, entity_id: &str) -> i64 {
    let mut hasher = DefaultHasher::new();
    entity_type.hash(&mut hasher);
    entity_id.hash(&mut hasher);
    hasher.finish() as i64
}

/// Acquire transaction-level advisory lock (blocks until available).
///
/// The lock is automatically released when the transaction ends
/// (commit or rollback).
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

/// Try to acquire transaction-level advisory lock (non-blocking).
///
/// Returns `true` if lock was acquired, `false` if already held by another
/// session. Does NOT block — returns immediately.
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
}
