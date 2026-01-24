//! Integration tests for advisory lock contention
//!
//! These tests verify that:
//! 1. Advisory locks prevent concurrent modification
//! 2. Lock contention is properly detected and reported
//! 3. Locks are released on transaction commit/rollback
//!
//! Requires: DATABASE_URL environment variable and `database` feature

#![cfg(feature = "database")]

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::Barrier;
use uuid::Uuid;

use ob_poc::database::locks::{
    acquire_locks, advisory_xact_lock, lock_key, try_advisory_xact_lock, LockError,
};
use ob_poc::dsl_v2::expansion::{LockKey, LockMode};

/// Helper to get test database pool
async fn get_test_pool() -> PgPool {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}

#[tokio::test]
async fn test_lock_key_is_stable() {
    // Same input should always produce same key
    let key1 = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");
    let key2 = lock_key("person", "550e8400-e29b-41d4-a716-446655440000");

    assert_eq!(key1, key2);
}

#[tokio::test]
async fn test_advisory_lock_acquired_and_released() {
    let pool = get_test_pool().await;

    let entity_id = Uuid::new_v4().to_string();
    let key = lock_key("test_entity", &entity_id);

    // Start transaction and acquire lock
    let mut tx = pool.begin().await.expect("Failed to begin transaction");
    advisory_xact_lock(&mut tx, key)
        .await
        .expect("Failed to acquire lock");

    // Lock should be held - try from another connection should fail
    let mut tx2 = pool.begin().await.expect("Failed to begin transaction 2");
    let acquired = try_advisory_xact_lock(&mut tx2, key)
        .await
        .expect("Failed to try lock");
    assert!(!acquired, "Should not acquire lock held by another session");
    tx2.rollback().await.expect("Failed to rollback tx2");

    // Commit first transaction - releases lock
    tx.commit().await.expect("Failed to commit");

    // Now lock should be available
    let mut tx3 = pool.begin().await.expect("Failed to begin transaction 3");
    let acquired = try_advisory_xact_lock(&mut tx3, key)
        .await
        .expect("Failed to try lock");
    assert!(acquired, "Should acquire lock after previous tx committed");
    tx3.rollback().await.expect("Failed to rollback tx3");
}

#[tokio::test]
async fn test_try_lock_returns_false_on_contention() {
    let pool = get_test_pool().await;

    let entity_id = Uuid::new_v4().to_string();
    let key = lock_key("test_entity", &entity_id);

    // Session A acquires lock
    let mut tx_a = pool.begin().await.expect("Failed to begin tx_a");
    let acquired_a = try_advisory_xact_lock(&mut tx_a, key)
        .await
        .expect("Failed to try lock A");
    assert!(acquired_a, "Session A should acquire lock");

    // Session B tries to acquire same lock - should fail
    let mut tx_b = pool.begin().await.expect("Failed to begin tx_b");
    let acquired_b = try_advisory_xact_lock(&mut tx_b, key)
        .await
        .expect("Failed to try lock B");
    assert!(!acquired_b, "Session B should NOT acquire lock");

    // Cleanup
    tx_a.rollback().await.expect("Failed to rollback tx_a");
    tx_b.rollback().await.expect("Failed to rollback tx_b");
}

#[tokio::test]
async fn test_acquire_locks_sorted_prevents_deadlock() {
    let pool = get_test_pool().await;

    // Create two locks in unsorted order
    let locks = vec![
        LockKey::write("zebra", "uuid-2"),
        LockKey::write("alpha", "uuid-1"),
    ];

    let mut tx = pool.begin().await.expect("Failed to begin transaction");

    // acquire_locks should sort internally and acquire in order
    let result = acquire_locks(&mut tx, &locks, LockMode::Try)
        .await
        .expect("Failed to acquire locks");

    // Should have acquired both locks
    assert_eq!(result.acquired.len(), 2);

    // First lock acquired should be alpha (sorted first)
    assert_eq!(result.acquired[0].entity_type, "alpha");
    assert_eq!(result.acquired[1].entity_type, "zebra");

    tx.rollback().await.expect("Failed to rollback");
}

#[tokio::test]
async fn test_acquire_locks_deduplicates() {
    let pool = get_test_pool().await;

    // Same lock specified twice
    let locks = vec![
        LockKey::write("entity", "uuid-1"),
        LockKey::write("entity", "uuid-1"), // Duplicate
    ];

    let mut tx = pool.begin().await.expect("Failed to begin transaction");

    let result = acquire_locks(&mut tx, &locks, LockMode::Try)
        .await
        .expect("Failed to acquire locks");

    // Should only have one lock (deduplicated)
    assert_eq!(result.acquired.len(), 1);

    tx.rollback().await.expect("Failed to rollback");
}

#[tokio::test]
async fn test_acquire_locks_contention_returns_partial() {
    let pool = get_test_pool().await;

    // Use deterministic UUIDs where entity_1 sorts BEFORE entity_2 alphabetically
    // This ensures entity_1 lock is acquired first, then we hit contention on entity_2
    let entity_id_1 = "00000000-0000-0000-0000-000000000001".to_string(); // Sorts first
    let entity_id_2 = "ffffffff-ffff-ffff-ffff-ffffffffffff".to_string(); // Sorts second

    // Session A acquires lock on entity_2 (sorted second)
    let mut tx_a = pool.begin().await.expect("Failed to begin tx_a");
    let key_2 = lock_key("entity", &entity_id_2);
    advisory_xact_lock(&mut tx_a, key_2)
        .await
        .expect("Failed to acquire lock");

    // Session B tries to acquire both locks
    let locks = vec![
        LockKey::write("entity", &entity_id_1),
        LockKey::write("entity", &entity_id_2), // This one is held by A
    ];

    let mut tx_b = pool.begin().await.expect("Failed to begin tx_b");

    let result = acquire_locks(&mut tx_b, &locks, LockMode::Try).await;

    match result {
        Err(LockError::Contention {
            entity_type,
            entity_id,
            acquired_so_far,
        }) => {
            assert_eq!(entity_type, "entity");
            assert_eq!(entity_id, entity_id_2);
            // Should have acquired entity_1 before hitting contention on entity_2
            assert_eq!(acquired_so_far.len(), 1);
            assert_eq!(acquired_so_far[0].entity_id, entity_id_1);
        }
        Ok(_) => panic!("Should have returned contention error"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }

    // Cleanup
    tx_a.rollback().await.expect("Failed to rollback tx_a");
    tx_b.rollback().await.expect("Failed to rollback tx_b");
}

#[tokio::test]
async fn test_concurrent_sessions_with_locking() {
    let pool = Arc::new(get_test_pool().await);
    let barrier = Arc::new(Barrier::new(2));

    let entity_id = Uuid::new_v4().to_string();
    let locks = vec![LockKey::write("person", &entity_id)];

    // Session A: Acquires lock, holds it for 200ms, then releases
    let pool_a = pool.clone();
    let barrier_a = barrier.clone();
    let locks_a = locks.clone();
    let session_a = tokio::spawn(async move {
        let mut tx = pool_a.begin().await.expect("Failed to begin tx_a");

        let result = acquire_locks(&mut tx, &locks_a, LockMode::Try).await;
        assert!(result.is_ok(), "Session A should acquire lock");

        // Signal that we have the lock
        barrier_a.wait().await;

        // Hold lock for a bit
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Commit releases lock
        tx.commit().await.expect("Failed to commit tx_a");

        "Session A: committed"
    });

    // Session B: Waits for A to acquire lock, then tries to acquire
    let pool_b = pool.clone();
    let barrier_b = barrier.clone();
    let locks_b = locks.clone();
    let session_b = tokio::spawn(async move {
        // Wait for Session A to acquire lock
        barrier_b.wait().await;

        // Small delay to ensure A is holding the lock
        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut tx = pool_b.begin().await.expect("Failed to begin tx_b");

        let result = acquire_locks(&mut tx, &locks_b, LockMode::Try).await;

        // Should fail due to contention
        match result {
            Err(LockError::Contention { .. }) => {
                tx.rollback().await.expect("Failed to rollback tx_b");
                "Session B: contention detected"
            }
            Ok(_) => {
                tx.rollback().await.expect("Failed to rollback tx_b");
                panic!("Session B should NOT have acquired lock while A holds it");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    });

    let (result_a, result_b) = tokio::join!(session_a, session_b);

    assert_eq!(result_a.unwrap(), "Session A: committed");
    assert_eq!(result_b.unwrap(), "Session B: contention detected");
}

#[tokio::test]
async fn test_lock_released_on_rollback() {
    let pool = get_test_pool().await;

    let entity_id = Uuid::new_v4().to_string();
    let key = lock_key("entity", &entity_id);

    // Session A acquires lock then rolls back
    let mut tx_a = pool.begin().await.expect("Failed to begin tx_a");
    advisory_xact_lock(&mut tx_a, key)
        .await
        .expect("Failed to acquire lock");
    tx_a.rollback().await.expect("Failed to rollback tx_a");

    // Session B should now be able to acquire
    let mut tx_b = pool.begin().await.expect("Failed to begin tx_b");
    let acquired = try_advisory_xact_lock(&mut tx_b, key)
        .await
        .expect("Failed to try lock");
    assert!(
        acquired,
        "Should acquire lock after previous session rolled back"
    );

    tx_b.rollback().await.expect("Failed to rollback tx_b");
}
