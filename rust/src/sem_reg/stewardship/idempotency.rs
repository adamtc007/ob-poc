//! Idempotency Service — ensures mutating tools are at-most-once.
//!
//! Spec §6.2: All mutating tools accept `client_request_id: Option<Uuid>`.
//! If present, we check `stewardship.idempotency_keys` before executing.
//! If a cached result exists, return it. Otherwise execute and cache.

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::store::StewardshipStore;

/// Result of an idempotency check.
pub enum IdempotencyCheck {
    /// First time seeing this request — proceed with execution.
    New,
    /// Cached result from a previous execution of the same request.
    Cached(serde_json::Value),
}

/// Check whether a client_request_id has been processed before.
/// Returns `New` if no prior result exists, or `Cached(result)` if it does.
pub async fn check_idempotency(
    pool: &PgPool,
    client_request_id: Option<Uuid>,
) -> Result<IdempotencyCheck> {
    match client_request_id {
        None => Ok(IdempotencyCheck::New),
        Some(id) => match StewardshipStore::check_idempotency(pool, id).await? {
            Some(cached) => Ok(IdempotencyCheck::Cached(cached)),
            None => Ok(IdempotencyCheck::New),
        },
    }
}

/// Record the result of a tool execution for idempotency.
/// No-ops if `client_request_id` is None.
pub async fn record_idempotency(
    pool: &PgPool,
    client_request_id: Option<Uuid>,
    tool_name: &str,
    result: &serde_json::Value,
) -> Result<()> {
    if let Some(id) = client_request_id {
        StewardshipStore::record_idempotency(pool, id, tool_name, result).await?;
    }
    Ok(())
}

/// Helper to wrap tool execution with idempotency check + record.
/// `execute_fn` is called only if no cached result exists.
pub async fn with_idempotency<F, Fut>(
    pool: &PgPool,
    client_request_id: Option<Uuid>,
    tool_name: &str,
    execute_fn: F,
) -> Result<serde_json::Value>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<serde_json::Value>>,
{
    // Check for cached result
    match check_idempotency(pool, client_request_id).await? {
        IdempotencyCheck::Cached(cached) => Ok(cached),
        IdempotencyCheck::New => {
            // Execute the tool
            let result = execute_fn().await?;

            // Cache the result
            record_idempotency(pool, client_request_id, tool_name, &result).await?;

            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idempotency_check_none_is_new() {
        // Without a client_request_id, check should always return New
        // This is a sync test for the None branch
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // We can't test with a real pool here, but we can verify the None path
            // The None path doesn't hit the database
            let result = check_idempotency(
                // We need a pool for the function signature but None path doesn't use it
                // This test just verifies the logic, not DB interaction
                &PgPool::connect_lazy("postgres://localhost/nonexistent").unwrap(),
                None,
            )
            .await
            .unwrap();
            assert!(matches!(result, IdempotencyCheck::New));
        });
    }
}
