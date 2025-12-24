# TODO: Optimistic Locking for DSL Execution

## Problem

Multiple agent sessions can operate on the same CBU/case simultaneously. Current implementation has no collision detection - two sessions can both read `current_version = 5`, both execute, and one silently overwrites the other.

## Solution: Optimistic Locking

No blocking. No waiting. Just detect stale context and fail fast.

## Implementation

### 1. Add Version Tracking to SessionContext

**File:** `src/api/session.rs`

```rust
// In SessionContext struct, add:
pub struct SessionContext {
    // ... existing fields ...
    
    /// Version of business_reference when loaded (for optimistic locking)
    #[serde(default)]
    pub loaded_dsl_version: Option<i32>,
    
    /// Business reference for this session's DSL instance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_reference: Option<String>,
}
```

### 2. Update DslRepository::save_dsl_instance()

**File:** `src/database/dsl_repository.rs`

Add `expected_version` parameter for optimistic lock check:

```rust
pub async fn save_dsl_instance(
    &self,
    business_reference: &str,
    domain_name: &str,
    dsl_content: &str,
    ast_json: Option<&serde_json::Value>,
    operation_type: &str,
    expected_version: Option<i32>,  // NEW: None = create new, Some(n) = must match
) -> Result<DslSaveResult, DslSaveError> {  // NEW: Custom error type
    let mut tx = self.pool.begin().await?;

    let existing: Option<(Uuid, i32)> = sqlx::query_as(
        r#"SELECT instance_id, current_version
           FROM "ob-poc".dsl_instances
           WHERE business_reference = $1"#,
    )
    .bind(business_reference)
    .fetch_optional(&mut *tx)
    .await?;

    let (instance_id, version) = if let Some((id, current_ver)) = existing {
        // OPTIMISTIC LOCK CHECK
        if let Some(expected) = expected_version {
            if current_ver != expected {
                return Err(DslSaveError::VersionConflict {
                    expected,
                    actual: current_ver,
                    business_reference: business_reference.to_string(),
                });
            }
        }
        
        let new_version = current_ver + 1;
        sqlx::query(
            r#"UPDATE "ob-poc".dsl_instances
               SET current_version = $1, updated_at = NOW()
               WHERE instance_id = $2 AND current_version = $3"#,  // Double-check in UPDATE
        )
        .bind(new_version)
        .bind(id)
        .bind(current_ver)
        .execute(&mut *tx)
        .await?;
        
        (id, new_version)
    } else {
        // New instance - no lock needed
        // ... existing create logic ...
    };

    // ... rest unchanged ...
}
```

### 3. Add Custom Error Type

**File:** `src/database/dsl_repository.rs`

```rust
#[derive(Debug, thiserror::Error)]
pub enum DslSaveError {
    #[error("Version conflict: expected {expected}, found {actual} for {business_reference}")]
    VersionConflict {
        expected: i32,
        actual: i32,
        business_reference: String,
    },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

### 4. Update Executor to Pass Expected Version

**File:** `src/dsl_v2/executor.rs` (or wherever execution calls save)

```rust
// When saving executed DSL, pass the session's loaded version:
let result = repo.save_dsl_instance(
    &business_reference,
    &domain,
    &dsl_content,
    Some(&ast_json),
    "EXECUTE",
    session.context.loaded_dsl_version,  // Pass expected version
).await;

match result {
    Err(DslSaveError::VersionConflict { expected, actual, .. }) => {
        // Return user-friendly error
        return Err(format!(
            "This {} has been modified by another session (version {} → {}). Please refresh and retry.",
            business_reference, expected, actual
        ));
    }
    Err(DslSaveError::Database(e)) => return Err(e.to_string()),
    Ok(save_result) => {
        // Update session with new version
        session.context.loaded_dsl_version = Some(save_result.version);
    }
}
```

### 5. Load Version When Session Activates on Entity

**File:** `src/api/agent_service.rs` (or session initialization)

When a session "loads" a CBU or case, capture the current version:

```rust
// When activating CBU in session:
if let Some(instance) = repo.get_instance_by_reference(&cbu_name).await? {
    session.context.loaded_dsl_version = Some(instance.current_version);
    session.context.business_reference = Some(cbu_name);
} else {
    // New entity - no existing version
    session.context.loaded_dsl_version = None;
    session.context.business_reference = Some(cbu_name);
}
```

## Test Cases

1. **Happy path:** Session loads v5, executes, becomes v6 ✓
2. **Collision:** Session A loads v5, Session B loads v5, A executes (→v6), B executes → CONFLICT
3. **New entity:** No existing version, creates v1 ✓
4. **Retry after conflict:** User refreshes, loads v6, executes → v7 ✓

## NOT In Scope

- Real-time push notifications ("Session B modified this")
- Pessimistic locking (blocking other sessions)
- Merge/conflict resolution UI
- Row-level locking on domain entities (cbus, cases, workstreams)

These can be added later if needed. Optimistic locking handles 95% of real-world collision cases.

## Migration

None required. This is application-level logic only - no schema changes.
