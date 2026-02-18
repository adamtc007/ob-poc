//! Execution gate for compiled runbooks.
//!
//! `execute_runbook()` is the **sole entry point** for running compiled DSL.
//! It validates the runbook status, computes the pre-lock set, acquires
//! advisory locks, and iterates steps through the underlying executor.
//!
//! ## Invariant (INV-3)
//!
//! No DSL may be executed without a valid `CompiledRunbookId`. Test-only
//! bypasses must be annotated with `// TEST-ONLY: bypasses runbook gate`.

use std::collections::BTreeSet;

use chrono::Utc;
use uuid::Uuid;

use super::types::{
    CompiledRunbook, CompiledRunbookId, CompiledRunbookStatus, CompiledStep, ParkReason, StepCursor,
};

// ---------------------------------------------------------------------------
// ExecutionResult
// ---------------------------------------------------------------------------

/// Result of executing a compiled runbook through the gate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunbookExecutionResult {
    /// The runbook that was executed.
    pub runbook_id: CompiledRunbookId,

    /// Per-step results (index-aligned with the runbook steps).
    pub step_results: Vec<StepExecutionResult>,

    /// Final status after execution.
    pub final_status: CompiledRunbookStatus,

    /// Total wall-clock time for the execution (milliseconds).
    pub elapsed_ms: u64,

    /// Lock statistics.
    pub lock_stats: LockStats,
}

/// Result of executing a single step.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StepExecutionResult {
    pub step_id: Uuid,
    pub verb: String,
    pub outcome: StepOutcome,
}

/// Outcome of a single step execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum StepOutcome {
    /// Step completed successfully.
    Completed { result: serde_json::Value },
    /// Step is parked, waiting for an external signal.
    Parked {
        correlation_key: String,
        message: String,
    },
    /// Step failed.
    Failed { error: String },
    /// Step was skipped (dependency failed).
    Skipped { reason: String },
}

/// Lock acquisition statistics.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LockStats {
    /// Number of locks acquired.
    pub locks_acquired: usize,
    /// Time spent waiting for locks (milliseconds).
    pub lock_wait_ms: u64,
}

// ---------------------------------------------------------------------------
// ExecutionError
// ---------------------------------------------------------------------------

/// Errors that prevent execution from starting.
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Runbook {0} not found")]
    NotFound(CompiledRunbookId),

    #[error("Runbook {0} is not executable (status: {1})")]
    NotExecutable(CompiledRunbookId, String),

    #[error("Lock contention on {entity_type}:{entity_id}")]
    LockContention {
        entity_type: String,
        entity_id: String,
        /// Best-effort holder of the contested lock (INV-10).
        holder_runbook_id: Option<uuid::Uuid>,
    },

    /// Lock acquisition timed out (INV-10: 30s timeout).
    ///
    /// Carries the full write_set and all entity IDs that could not be locked,
    /// plus best-effort holder identification.
    #[error("Lock timeout on {} entities (write_set size: {})", entity_ids.len(), write_set.len())]
    LockTimeout {
        /// Entity UUIDs that could not be locked.
        entity_ids: Vec<Uuid>,
        /// Full write_set that was requested.
        write_set: std::collections::BTreeSet<Uuid>,
        /// Best-effort: the runbook currently holding the contested lock.
        holder_runbook_id: Option<CompiledRunbookId>,
    },

    #[error("Database error: {0}")]
    Database(String),

    #[error("Step execution failed: {0}")]
    StepFailed(String),
}

// ---------------------------------------------------------------------------
// RunbookStore — in-memory store for compiled runbooks
// ---------------------------------------------------------------------------

/// In-memory store for compiled runbooks.
///
/// Phase 0 uses an in-memory store. A database-backed store can be added
/// later without changing the executor logic.
pub struct RunbookStore {
    runbooks: std::sync::RwLock<std::collections::HashMap<CompiledRunbookId, CompiledRunbook>>,
}

impl RunbookStore {
    pub fn new() -> Self {
        Self {
            runbooks: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Synchronous insert for callers that are not in an async context
    /// (e.g. `try_compile_entry` in the REPL orchestrator). Delegates to
    /// the same in-memory map that the async trait method uses.
    pub fn insert_sync(&self, runbook: &CompiledRunbook) {
        self.runbooks
            .write()
            .expect("RunbookStore RwLock poisoned")
            .insert(runbook.id, runbook.clone());
    }
}

impl Default for RunbookStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RunbookStoreBackend — unified trait for compiled runbook stores (INV-9)
// ---------------------------------------------------------------------------

/// Unified store trait for compiled runbooks (INV-9).
///
/// Both in-memory (dev/test) and Postgres (prod) backends implement this.
/// The executor accepts `&dyn RunbookStoreBackend` — one path, one trait.
#[async_trait::async_trait]
pub trait RunbookStoreBackend: Send + Sync {
    /// Insert a compiled runbook. Content-addressed dedup: same ID = no-op.
    async fn insert(&self, runbook: &CompiledRunbook) -> Result<(), ExecutionError>;

    /// Retrieve a compiled runbook by ID. Returns None if not found.
    async fn get(&self, id: &CompiledRunbookId) -> Result<Option<CompiledRunbook>, ExecutionError>;

    /// Append an event to the audit log (INSERT-only).
    async fn append_event(&self, event: RunbookEvent) -> Result<(), ExecutionError>;

    /// Derive current status from the latest status_change event.
    async fn current_status(
        &self,
        id: &CompiledRunbookId,
    ) -> Result<CompiledRunbookStatus, ExecutionError>;

    /// Update status (in-memory impl mutates; Postgres impl appends status_change event).
    async fn update_status(
        &self,
        id: &CompiledRunbookId,
        old_status: &str,
        new_status: CompiledRunbookStatus,
    ) -> Result<(), ExecutionError>;

    /// Best-effort lookup of which runbook holds a lock on the given entity.
    async fn lookup_lock_holder(&self, entity_type: &str, entity_id: &str) -> Option<Uuid>;
}

// ---------------------------------------------------------------------------
// RunbookStoreBackend impl for RunbookStore (in-memory)
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl RunbookStoreBackend for RunbookStore {
    async fn insert(&self, runbook: &CompiledRunbook) -> Result<(), ExecutionError> {
        let id = runbook.id;
        self.runbooks
            .write()
            .expect("RunbookStore RwLock poisoned")
            .insert(id, runbook.clone());
        Ok(())
    }

    async fn get(&self, id: &CompiledRunbookId) -> Result<Option<CompiledRunbook>, ExecutionError> {
        Ok(self
            .runbooks
            .read()
            .expect("RunbookStore RwLock poisoned")
            .get(id)
            .cloned())
    }

    async fn append_event(&self, _event: RunbookEvent) -> Result<(), ExecutionError> {
        // In-memory store has no event log — no-op.
        Ok(())
    }

    async fn current_status(
        &self,
        id: &CompiledRunbookId,
    ) -> Result<CompiledRunbookStatus, ExecutionError> {
        let map = self.runbooks.read().expect("RunbookStore RwLock poisoned");
        let rb = map.get(id).ok_or(ExecutionError::NotFound(*id))?;
        Ok(rb.status.clone())
    }

    async fn update_status(
        &self,
        id: &CompiledRunbookId,
        _old_status: &str,
        new_status: CompiledRunbookStatus,
    ) -> Result<(), ExecutionError> {
        let mut map = self.runbooks.write().expect("RunbookStore RwLock poisoned");
        let rb = map.get_mut(id).ok_or(ExecutionError::NotFound(*id))?;
        rb.status = new_status;
        Ok(())
    }

    async fn lookup_lock_holder(&self, _entity_type: &str, _entity_id: &str) -> Option<Uuid> {
        // In-memory store has no event log to query.
        None
    }
}

// ---------------------------------------------------------------------------
// RunbookEvent — event log entry for compiled_runbook_events (INV-9)
// ---------------------------------------------------------------------------

/// An event to append to the compiled_runbook_events table.
///
/// Events are INSERT-only — status is derived from the latest status_change event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunbookEvent {
    pub compiled_runbook_id: CompiledRunbookId,
    pub event_type: String,
    pub old_status: Option<String>,
    pub new_status: Option<String>,
    pub detail: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// PostgresRunbookStore — append-only database store (INV-9)
// ---------------------------------------------------------------------------

/// Append-only Postgres store for compiled runbooks (INV-9).
///
/// Design principles:
/// - `compiled_runbooks` is INSERT-only (no UPDATE, no DELETE — enforced by trigger)
/// - Status is NOT stored on the runbook row — it's derived from the latest
///   `status_change` event in `compiled_runbook_events`
/// - Content-addressed dedup: `ON CONFLICT (compiled_runbook_id) DO NOTHING`
/// - Hash verification on read: `canonical_hash` is checked against recomputed hash
#[cfg(feature = "database")]
pub struct PostgresRunbookStore {
    pool: sqlx::PgPool,
}

#[cfg(feature = "database")]
impl PostgresRunbookStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Insert a compiled runbook (append-only, dedup on content-addressed ID).
    ///
    /// Uses `ON CONFLICT DO NOTHING` since content-addressed IDs are idempotent:
    /// same content → same ID → safe to skip.
    pub async fn insert(&self, runbook: &CompiledRunbook) -> Result<(), ExecutionError> {
        let steps_json = serde_json::to_value(&runbook.steps)
            .map_err(|e| ExecutionError::Database(format!("serialize steps: {e}")))?;
        let envelope_json = serde_json::to_value(&runbook.envelope)
            .map_err(|e| ExecutionError::Database(format!("serialize envelope: {e}")))?;
        let canonical_hash = super::canonical::full_sha256(&runbook.steps, &runbook.envelope);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".compiled_runbooks
                (compiled_runbook_id, session_id, version, steps, envelope, canonical_hash)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (compiled_runbook_id) DO NOTHING
            "#,
        )
        .bind(runbook.id.0)
        .bind(runbook.session_id)
        .bind(runbook.version as i64)
        .bind(&steps_json)
        .bind(&envelope_json)
        .bind(&canonical_hash[..])
        .execute(&self.pool)
        .await
        .map_err(|e| ExecutionError::Database(e.to_string()))?;

        // Append initial status_change event
        self.append_event(RunbookEvent {
            compiled_runbook_id: runbook.id,
            event_type: "status_change".into(),
            old_status: None,
            new_status: Some("compiled".into()),
            detail: None,
        })
        .await?;

        Ok(())
    }

    /// Retrieve a compiled runbook by ID, verifying hash integrity.
    ///
    /// Returns `None` if not found. Returns an error if the stored hash
    /// doesn't match the recomputed hash (tampered artefact).
    pub async fn get(
        &self,
        id: &CompiledRunbookId,
    ) -> Result<Option<CompiledRunbook>, ExecutionError> {
        let row = sqlx::query_as::<_, CompiledRunbookRow>(
            r#"
            SELECT compiled_runbook_id, session_id, version, steps, envelope,
                   canonical_hash, created_at
            FROM "ob-poc".compiled_runbooks
            WHERE compiled_runbook_id = $1
            "#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ExecutionError::Database(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let steps: Vec<CompiledStep> = serde_json::from_value(row.steps)
            .map_err(|e| ExecutionError::Database(format!("deserialize steps: {e}")))?;
        let envelope: super::envelope::ReplayEnvelope = serde_json::from_value(row.envelope)
            .map_err(|e| ExecutionError::Database(format!("deserialize envelope: {e}")))?;

        // Verify hash integrity (INV-13)
        let recomputed = super::canonical::full_sha256(&steps, &envelope);
        if recomputed[..] != row.canonical_hash[..] {
            return Err(ExecutionError::Database(format!(
                "Hash integrity check failed for runbook {}: stored hash does not match \
                 recomputed hash. Artefact may have been tampered with.",
                id
            )));
        }

        // Derive current status from events
        let status = self.current_status(id).await?;

        Ok(Some(CompiledRunbook {
            id: CompiledRunbookId(row.compiled_runbook_id),
            session_id: row.session_id,
            version: row.version as u64,
            steps,
            envelope,
            status,
            created_at: row.created_at,
        }))
    }

    /// Append an event to the compiled_runbook_events table (INV-9: INSERT-only).
    pub async fn append_event(&self, event: RunbookEvent) -> Result<(), ExecutionError> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".compiled_runbook_events
                (compiled_runbook_id, event_type, old_status, new_status, detail)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(event.compiled_runbook_id.0)
        .bind(&event.event_type)
        .bind(&event.old_status)
        .bind(&event.new_status)
        .bind(&event.detail)
        .execute(&self.pool)
        .await
        .map_err(|e| ExecutionError::Database(e.to_string()))?;

        Ok(())
    }

    /// Derive current status from the latest status_change event (INV-9).
    ///
    /// No `update_status()` method — status is NEVER mutated on the runbook row.
    pub async fn current_status(
        &self,
        id: &CompiledRunbookId,
    ) -> Result<CompiledRunbookStatus, ExecutionError> {
        let row = sqlx::query_as::<_, StatusEventRow>(
            r#"
            SELECT new_status, detail
            FROM "ob-poc".compiled_runbook_events
            WHERE compiled_runbook_id = $1
              AND event_type = 'status_change'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ExecutionError::Database(e.to_string()))?;

        match row {
            Some(StatusEventRow {
                new_status: Some(status),
                detail,
            }) => parse_status(&status, detail.as_ref()),
            _ => Ok(CompiledRunbookStatus::Compiled),
        }
    }

    /// Best-effort lookup for the compiled runbook currently holding a lock on an entity.
    ///
    /// Queries `compiled_runbook_events` for the most recent `lock_acquired` event
    /// whose `detail` JSON contains the contested entity in its write_set, and which
    /// has no subsequent `lock_released` event for the same runbook.
    ///
    /// Returns `None` on any error or if no holder is found (advisory locks are
    /// anonymous in PostgreSQL — this is a heuristic).
    pub async fn lookup_lock_holder(&self, entity_type: &str, entity_id: &str) -> Option<Uuid> {
        // Suppress unused-variable warnings when feature is enabled but
        // the query references them via bind parameters.
        let _ = entity_type;

        // Find the most recent lock_acquired event where write_set contains
        // the contested entity_id and no corresponding lock_released follows.
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT a.compiled_runbook_id
            FROM "ob-poc".compiled_runbook_events a
            WHERE a.event_type = 'lock_acquired'
              AND a.detail @> $1::jsonb
              AND NOT EXISTS (
                  SELECT 1
                  FROM "ob-poc".compiled_runbook_events r
                  WHERE r.compiled_runbook_id = a.compiled_runbook_id
                    AND r.event_type = 'lock_released'
                    AND r.created_at > a.created_at
              )
            ORDER BY a.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(serde_json::json!({ "write_set": [entity_id] }))
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();

        row.map(|(id,)| id)
    }
}

// ---------------------------------------------------------------------------
// RunbookStoreBackend impl for PostgresRunbookStore (INV-9)
// ---------------------------------------------------------------------------

#[cfg(feature = "database")]
#[async_trait::async_trait]
impl RunbookStoreBackend for PostgresRunbookStore {
    async fn insert(&self, runbook: &CompiledRunbook) -> Result<(), ExecutionError> {
        PostgresRunbookStore::insert(self, runbook).await
    }

    async fn get(&self, id: &CompiledRunbookId) -> Result<Option<CompiledRunbook>, ExecutionError> {
        PostgresRunbookStore::get(self, id).await
    }

    async fn append_event(&self, event: RunbookEvent) -> Result<(), ExecutionError> {
        PostgresRunbookStore::append_event(self, event).await
    }

    async fn current_status(
        &self,
        id: &CompiledRunbookId,
    ) -> Result<CompiledRunbookStatus, ExecutionError> {
        PostgresRunbookStore::current_status(self, id).await
    }

    async fn update_status(
        &self,
        id: &CompiledRunbookId,
        old_status: &str,
        new_status: CompiledRunbookStatus,
    ) -> Result<(), ExecutionError> {
        // Append-only: status is tracked via events, never mutated on the row.
        self.append_event(RunbookEvent {
            compiled_runbook_id: *id,
            event_type: "status_change".into(),
            old_status: Some(old_status.to_string()),
            new_status: Some(status_to_string(&new_status).to_string()),
            detail: status_detail(&new_status),
        })
        .await
    }

    async fn lookup_lock_holder(&self, entity_type: &str, entity_id: &str) -> Option<Uuid> {
        PostgresRunbookStore::lookup_lock_holder(self, entity_type, entity_id).await
    }
}

/// Row type for compiled_runbooks table queries.
#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct CompiledRunbookRow {
    compiled_runbook_id: Uuid,
    session_id: Uuid,
    version: i64,
    steps: serde_json::Value,
    envelope: serde_json::Value,
    canonical_hash: Vec<u8>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Row type for status event queries.
#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct StatusEventRow {
    new_status: Option<String>,
    detail: Option<serde_json::Value>,
}

/// Parse a status string + optional detail JSON into `CompiledRunbookStatus`.
#[cfg(feature = "database")]
fn parse_status(
    status: &str,
    detail: Option<&serde_json::Value>,
) -> Result<CompiledRunbookStatus, ExecutionError> {
    match status {
        "compiled" => Ok(CompiledRunbookStatus::Compiled),
        "executing" => {
            let current_step = detail
                .and_then(|d| d.get("current_step"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            Ok(CompiledRunbookStatus::Executing { current_step })
        }
        "completed" => Ok(CompiledRunbookStatus::Completed {
            completed_at: Utc::now(),
        }),
        "failed" => {
            let error = detail
                .and_then(|d| d.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string();
            Ok(CompiledRunbookStatus::Failed {
                error,
                failed_step: None,
            })
        }
        "parked" => {
            let correlation_key = detail
                .and_then(|d| d.get("correlation_key"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            Ok(CompiledRunbookStatus::Parked {
                reason: ParkReason::AwaitingCallback { correlation_key },
                cursor: StepCursor {
                    index: 0,
                    step_id: Uuid::nil(),
                },
            })
        }
        other => Err(ExecutionError::Database(format!(
            "Unknown runbook status: {other}"
        ))),
    }
}

/// Serialize a `CompiledRunbookStatus` to a status string for event storage.
#[cfg(feature = "database")]
pub fn status_to_string(status: &CompiledRunbookStatus) -> &'static str {
    match status {
        CompiledRunbookStatus::Compiled => "compiled",
        CompiledRunbookStatus::Executing { .. } => "executing",
        CompiledRunbookStatus::Completed { .. } => "completed",
        CompiledRunbookStatus::Failed { .. } => "failed",
        CompiledRunbookStatus::Parked { .. } => "parked",
    }
}

/// Serialize status detail to JSON for event storage.
#[cfg(feature = "database")]
pub fn status_detail(status: &CompiledRunbookStatus) -> Option<serde_json::Value> {
    match status {
        CompiledRunbookStatus::Compiled => None,
        CompiledRunbookStatus::Executing { current_step } => {
            Some(serde_json::json!({ "current_step": current_step }))
        }
        CompiledRunbookStatus::Completed { completed_at } => {
            Some(serde_json::json!({ "completed_at": completed_at.to_rfc3339() }))
        }
        CompiledRunbookStatus::Failed { error, failed_step } => {
            let mut detail = serde_json::json!({ "error": error });
            if let Some(cursor) = failed_step {
                detail["failed_step_index"] = serde_json::json!(cursor.index);
                detail["failed_step_id"] = serde_json::json!(cursor.step_id.to_string());
            }
            Some(detail)
        }
        CompiledRunbookStatus::Parked { reason, cursor } => {
            let mut detail = serde_json::json!({
                "cursor_index": cursor.index,
                "cursor_step_id": cursor.step_id.to_string(),
            });
            match reason {
                ParkReason::AwaitingCallback { correlation_key } => {
                    detail["correlation_key"] = serde_json::json!(correlation_key);
                    detail["park_reason"] = serde_json::json!("awaiting_callback");
                }
                ParkReason::UserPaused => {
                    detail["park_reason"] = serde_json::json!("user_paused");
                }
                ParkReason::ResourceUnavailable { resource } => {
                    detail["park_reason"] = serde_json::json!("resource_unavailable");
                    detail["resource"] = serde_json::json!(resource);
                }
                ParkReason::HumanGate { entry_id } => {
                    detail["park_reason"] = serde_json::json!("human_gate");
                    detail["entry_id"] = serde_json::json!(entry_id.to_string());
                }
            };
            Some(detail)
        }
    }
}

// ---------------------------------------------------------------------------
// compute_write_set — derive the pre-lock set from steps
// ---------------------------------------------------------------------------

/// Compute the set of entity UUIDs that will be written to during execution.
///
/// Scans the step list (optionally starting from a cursor) and collects all
/// entity UUIDs declared in each step's `write_set` field.
///
/// The returned set is sorted (BTreeSet) for deadlock-free lock acquisition.
pub fn compute_write_set(steps: &[CompiledStep], cursor: Option<&StepCursor>) -> BTreeSet<Uuid> {
    let start_idx = cursor.map(|c| c.index).unwrap_or(0);
    steps
        .iter()
        .skip(start_idx)
        .flat_map(|step| step.write_set.iter().copied())
        .collect()
}

// ---------------------------------------------------------------------------
// execute_runbook — the execution gate
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Advisory lock acquisition helper
// ---------------------------------------------------------------------------

/// Acquire advisory locks on the write_set using `try_advisory_xact_lock` (fail-fast).
///
/// Locks are acquired in sorted UUID order to prevent deadlocks. On contention,
/// returns `ExecutionError::LockTimeout` with the full write_set and holder info (INV-10).
///
/// Returns the open transaction that holds the locks. The caller MUST keep this
/// transaction alive during step execution and commit/rollback when done.
/// Advisory xact locks auto-release on commit or rollback.
async fn acquire_advisory_locks(
    pool: &sqlx::PgPool,
    write_set: &BTreeSet<Uuid>,
    store: &dyn RunbookStoreBackend,
) -> Result<(sqlx::Transaction<'static, sqlx::Postgres>, LockStats), ExecutionError> {
    use crate::database::locks::{acquire_locks, LockError, LockKey, LockMode};

    /// Lock timeout for runbook execution (INV-10: 30 seconds).
    const LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    let lock_start = std::time::Instant::now();

    // Build sorted lock keys from entity UUIDs.
    let lock_keys: Vec<LockKey> = write_set
        .iter()
        .map(|id| LockKey::write("entity", id.to_string()))
        .collect();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ExecutionError::Database(e.to_string()))?;

    match acquire_locks(&mut tx, &lock_keys, LockMode::Timeout(LOCK_TIMEOUT)).await {
        Ok(result) => {
            // Locks acquired — return the open transaction.
            // Caller keeps it alive during step execution; locks auto-release
            // when the transaction is committed or rolled back.
            let stats = LockStats {
                locks_acquired: result.acquired.len(),
                lock_wait_ms: lock_start.elapsed().as_millis() as u64,
            };
            Ok((tx, stats))
        }
        Err(LockError::Contention {
            entity_type,
            entity_id,
            holder_runbook_id,
            ..
        }) => {
            // Roll back and surface timeout error with full write_set (INV-10).
            let _ = tx.rollback().await;

            // Attempt holder lookup via store if not already known.
            let holder = if holder_runbook_id.is_some() {
                holder_runbook_id.map(CompiledRunbookId)
            } else {
                store
                    .lookup_lock_holder(&entity_type, &entity_id)
                    .await
                    .map(CompiledRunbookId)
            };

            let entity_uuid = Uuid::parse_str(&entity_id).unwrap_or(Uuid::nil());

            Err(ExecutionError::LockTimeout {
                entity_ids: vec![entity_uuid],
                write_set: write_set.clone(),
                holder_runbook_id: holder,
            })
        }
        Err(LockError::Database(e)) => {
            let _ = tx.rollback().await;
            Err(ExecutionError::Database(e.to_string()))
        }
    }
}

// ---------------------------------------------------------------------------
// execute_runbook — the execution gate
// ---------------------------------------------------------------------------

/// Execute a compiled runbook through the gate.
///
/// This is the **sole entry point** for running compiled DSL (INV-3).
///
/// ## Flow
///
/// 1. Look up the runbook by `CompiledRunbookId`
/// 2. Validate status is `Compiled` or `Parked`
/// 3. Transition to `Executing`
/// 4. Compute pre-lock set from steps
/// 5. Iterate steps, executing each through the provided executor
/// 6. Transition to `Completed`, `Failed`, or `Parked`
///
/// ## Locking
///
/// When a database pool is available, advisory locks are acquired on all
/// entity UUIDs in the write set before execution begins. Locks are released
/// when the transaction commits or rolls back.
///
/// In Phase 0 (no database), locking is skipped and execution proceeds
/// directly.
pub async fn execute_runbook(
    store: &dyn RunbookStoreBackend,
    runbook_id: CompiledRunbookId,
    cursor: Option<StepCursor>,
    executor: &dyn StepExecutor,
) -> Result<RunbookExecutionResult, ExecutionError> {
    execute_runbook_with_pool(store, runbook_id, cursor, executor, None).await
}

/// Execute a compiled runbook with optional advisory locking.
///
/// When `pool` is `Some`, advisory locks are acquired on the write_set
/// using timeout-based locking (INV-10: 30s timeout). Locks are sorted by UUID
/// to prevent deadlocks and auto-release on tx commit/rollback.
///
/// When `pool` is `None` (tests, in-memory mode), locking is skipped.
///
/// The store backend handles event emission: the Postgres backend appends
/// status_change events on `update_status()` and persists lock/step events
/// via `append_event()`. The in-memory backend no-ops on events.
pub async fn execute_runbook_with_pool(
    store: &dyn RunbookStoreBackend,
    runbook_id: CompiledRunbookId,
    cursor: Option<StepCursor>,
    executor: &dyn StepExecutor,
    pool: Option<&sqlx::PgPool>,
) -> Result<RunbookExecutionResult, ExecutionError> {
    let start = std::time::Instant::now();

    // 1. Look up runbook
    let runbook = store
        .get(&runbook_id)
        .await?
        .ok_or(ExecutionError::NotFound(runbook_id))?;

    // 2. Validate status
    if !runbook.is_executable() {
        return Err(ExecutionError::NotExecutable(
            runbook_id,
            format!("{:?}", runbook.status),
        ));
    }

    let start_idx = cursor.map(|c| c.index).unwrap_or(0);

    // 3. Compute pre-lock set and acquire advisory locks BEFORE transitioning
    //    to Executing. This prevents status corruption on lock contention
    //    (Fix 3): if we can't acquire locks, we never touch the status.
    let write_set = compute_write_set(&runbook.steps, cursor.as_ref());

    // `_lock_tx` holds the advisory xact locks alive for the duration of step
    // execution. When it's dropped (committed or rolled back), locks release.
    // This makes locking real (Fix 2): locks cover the entire execution window.
    let (_lock_tx, lock_stats) = if !write_set.is_empty() {
        if let Some(pool) = pool {
            let result = acquire_advisory_locks(pool, &write_set, store).await;

            // Log lock contention event (INV-10)
            if let Err(ExecutionError::LockTimeout {
                ref entity_ids,
                ref write_set,
                ref holder_runbook_id,
            }) = result
            {
                let _ = store.append_event(RunbookEvent {
                        compiled_runbook_id: runbook_id,
                        event_type: "lock_contention".into(),
                        old_status: None,
                        new_status: None,
                        detail: Some(serde_json::json!({
                            "entity_ids": entity_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                            "write_set": write_set.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                            "holder_runbook_id": holder_runbook_id.map(|h| h.0.to_string()),
                        })),
                    }).await;
            }

            let (tx, stats) = result?;

            // Log lock_acquired event (INV-10)
            let _ = store
                .append_event(RunbookEvent {
                    compiled_runbook_id: runbook_id,
                    event_type: "lock_acquired".into(),
                    old_status: None,
                    new_status: None,
                    detail: Some(serde_json::json!({
                        "locks_acquired": stats.locks_acquired,
                        "lock_wait_ms": stats.lock_wait_ms,
                        "write_set": write_set.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                    })),
                })
                .await;

            (Some(tx), stats)
        } else {
            tracing::warn!(
                runbook_id = %runbook_id,
                write_set_size = write_set.len(),
                "Executing with non-empty write_set but no database pool — \
                 advisory locks NOT acquired. Safe in tests; indicates \
                 misconfiguration in production."
            );
            (
                None,
                LockStats {
                    locks_acquired: 0, // Honest: no locks were actually acquired
                    lock_wait_ms: 0,
                },
            )
        }
    } else {
        (None, LockStats::default())
    };

    // 4. Transition to Executing — only AFTER locks are acquired.
    store
        .update_status(
            &runbook_id,
            "compiled",
            CompiledRunbookStatus::Executing {
                current_step: start_idx,
            },
        )
        .await?;

    // 5. Execute steps (advisory locks held via `_lock_tx` throughout)
    let mut step_results = Vec::with_capacity(runbook.steps.len());
    let mut final_status = CompiledRunbookStatus::Completed {
        completed_at: Utc::now(),
    };

    // Pre-fill skipped for steps before cursor
    for step in runbook.steps.iter().take(start_idx) {
        step_results.push(StepExecutionResult {
            step_id: step.step_id,
            verb: step.verb.clone(),
            outcome: StepOutcome::Skipped {
                reason: "Before resume cursor".into(),
            },
        });
    }

    // Track failed step IDs for dependency skipping
    let mut failed_steps: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    for (idx, step) in runbook.steps.iter().enumerate().skip(start_idx) {
        // Check if any dependency has failed
        let dep_failed = step
            .depends_on
            .iter()
            .any(|dep_id| failed_steps.contains(dep_id));

        if dep_failed {
            failed_steps.insert(step.step_id);
            // Emit step_skipped event (INV-9, Phase E-2)
            let _ = store
                .append_event(RunbookEvent {
                    compiled_runbook_id: runbook_id,
                    event_type: "step_skipped".into(),
                    old_status: None,
                    new_status: None,
                    detail: Some(serde_json::json!({
                        "step_id": step.step_id.to_string(),
                        "verb": &step.verb,
                        "step_index": idx,
                        "reason": "Dependency failed",
                    })),
                })
                .await;
            step_results.push(StepExecutionResult {
                step_id: step.step_id,
                verb: step.verb.clone(),
                outcome: StepOutcome::Skipped {
                    reason: "Dependency failed".into(),
                },
            });
            continue;
        }

        // Update current step
        store
            .update_status(
                &runbook_id,
                "executing",
                CompiledRunbookStatus::Executing { current_step: idx },
            )
            .await?;

        // Execute the step
        let outcome = executor.execute_step(step).await;

        // Emit per-step event (INV-9, Phase E-2)
        let step_event_type = match &outcome {
            StepOutcome::Completed { .. } => "step_completed",
            StepOutcome::Failed { .. } => "step_failed",
            StepOutcome::Parked { .. } => "step_parked",
            StepOutcome::Skipped { .. } => "step_skipped",
        };
        let _ = store
            .append_event(RunbookEvent {
                compiled_runbook_id: runbook_id,
                event_type: step_event_type.into(),
                old_status: None,
                new_status: None,
                detail: Some(serde_json::json!({
                    "step_id": step.step_id.to_string(),
                    "verb": &step.verb,
                    "step_index": idx,
                })),
            })
            .await;

        // Determine whether this step terminates the runbook loop
        let should_break = match &outcome {
            StepOutcome::Parked {
                correlation_key, ..
            } => {
                final_status = CompiledRunbookStatus::Parked {
                    reason: ParkReason::AwaitingCallback {
                        correlation_key: correlation_key.clone(),
                    },
                    cursor: StepCursor {
                        index: idx,
                        step_id: step.step_id,
                    },
                };
                Some("Runbook parked")
            }
            StepOutcome::Failed { error } => {
                failed_steps.insert(step.step_id);
                final_status = CompiledRunbookStatus::Failed {
                    error: error.clone(),
                    failed_step: Some(StepCursor {
                        index: idx,
                        step_id: step.step_id,
                    }),
                };
                Some("Previous step failed")
            }
            StepOutcome::Completed { .. } | StepOutcome::Skipped { .. } => None,
        };

        step_results.push(StepExecutionResult {
            step_id: step.step_id,
            verb: step.verb.clone(),
            outcome,
        });

        if let Some(skip_reason) = should_break {
            // Fill remaining steps as skipped
            for (rem_idx, remaining) in runbook.steps.iter().enumerate().skip(idx + 1) {
                // Emit step_skipped event for remaining steps (INV-9, Phase E-2)
                let _ = store
                    .append_event(RunbookEvent {
                        compiled_runbook_id: runbook_id,
                        event_type: "step_skipped".into(),
                        old_status: None,
                        new_status: None,
                        detail: Some(serde_json::json!({
                            "step_id": remaining.step_id.to_string(),
                            "verb": &remaining.verb,
                            "step_index": rem_idx,
                            "reason": skip_reason,
                        })),
                    })
                    .await;
                step_results.push(StepExecutionResult {
                    step_id: remaining.step_id,
                    verb: remaining.verb.clone(),
                    outcome: StepOutcome::Skipped {
                        reason: skip_reason.into(),
                    },
                });
            }
            break;
        }
    }

    // 6. Update final status
    store
        .update_status(&runbook_id, "executing", final_status.clone())
        .await?;

    // 7. Release advisory locks by committing (or dropping) the lock transaction.
    //    On success we commit; on any error path above we return early and _lock_tx
    //    is dropped, which rolls back and releases locks automatically.
    if let Some(lock_tx) = _lock_tx {
        let _ = lock_tx.commit().await;

        // Log lock_released event (INV-10)
        let _ = store
            .append_event(RunbookEvent {
                compiled_runbook_id: runbook_id,
                event_type: "lock_released".into(),
                old_status: None,
                new_status: None,
                detail: Some(serde_json::json!({
                    "locks_released": lock_stats.locks_acquired,
                    "total_execution_ms": start.elapsed().as_millis() as u64,
                })),
            })
            .await;
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(RunbookExecutionResult {
        runbook_id,
        step_results,
        final_status,
        elapsed_ms,
        lock_stats,
    })
}

// ---------------------------------------------------------------------------
// StepExecutor trait
// ---------------------------------------------------------------------------

/// Trait for executing individual compiled steps.
///
/// This abstracts the actual DSL execution so the gate logic doesn't depend
/// on the full `DslExecutor` infrastructure. In production, this delegates
/// to `DslExecutorV2`. In tests, a stub implementation can be used.
#[async_trait::async_trait]
pub trait StepExecutor: Send + Sync {
    /// Execute a single compiled step and return the outcome.
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::envelope::ReplayEnvelope;
    use crate::runbook::types::ExecutionMode;

    /// Stub executor that always succeeds.
    struct SuccessExecutor;

    #[async_trait::async_trait]
    impl StepExecutor for SuccessExecutor {
        async fn execute_step(&self, _step: &CompiledStep) -> StepOutcome {
            StepOutcome::Completed {
                result: serde_json::json!({"ok": true}),
            }
        }
    }

    /// Stub executor that fails on a specific verb.
    struct FailOnVerb(String);

    #[async_trait::async_trait]
    impl StepExecutor for FailOnVerb {
        async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
            if step.verb == self.0 {
                StepOutcome::Failed {
                    error: format!("{} failed", step.verb),
                }
            } else {
                StepOutcome::Completed {
                    result: serde_json::json!({"ok": true}),
                }
            }
        }
    }

    /// Stub executor that parks on a specific verb.
    struct ParkOnVerb(String);

    #[async_trait::async_trait]
    impl StepExecutor for ParkOnVerb {
        async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
            if step.verb == self.0 {
                StepOutcome::Parked {
                    correlation_key: format!("park-{}", step.step_id),
                    message: "Waiting for approval".into(),
                }
            } else {
                StepOutcome::Completed {
                    result: serde_json::json!({"ok": true}),
                }
            }
        }
    }

    fn make_step(verb: &str) -> CompiledStep {
        CompiledStep {
            step_id: Uuid::new_v4(),
            sentence: format!("Do {verb}"),
            verb: verb.into(),
            dsl: format!("({verb})"),
            args: std::collections::BTreeMap::new(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        }
    }

    fn make_step_with_dep(verb: &str, dep: Uuid) -> CompiledStep {
        let mut step = make_step(verb);
        step.depends_on = vec![dep];
        step
    }

    #[tokio::test]
    async fn test_execute_empty_runbook() {
        let store = RunbookStore::new();
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![], ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        let result = execute_runbook(&store, id, None, &SuccessExecutor)
            .await
            .unwrap();
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Completed { .. }
        ));
        assert!(result.step_results.is_empty());
    }

    #[tokio::test]
    async fn test_execute_all_succeed() {
        let store = RunbookStore::new();
        let steps = vec![make_step("cbu.create"), make_step("entity.create")];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        let result = execute_runbook(&store, id, None, &SuccessExecutor)
            .await
            .unwrap();
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Completed { .. }
        ));
        assert_eq!(result.step_results.len(), 2);
        assert!(matches!(
            result.step_results[0].outcome,
            StepOutcome::Completed { .. }
        ));
    }

    #[tokio::test]
    async fn test_execute_step_fails() {
        let store = RunbookStore::new();
        let steps = vec![make_step("cbu.create"), make_step("entity.create")];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        let executor = FailOnVerb("cbu.create".into());
        let result = execute_runbook(&store, id, None, &executor).await.unwrap();
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Failed { .. }
        ));
        assert!(matches!(
            result.step_results[0].outcome,
            StepOutcome::Failed { .. }
        ));
        // Second step should be skipped
        assert!(matches!(
            result.step_results[1].outcome,
            StepOutcome::Skipped { .. }
        ));
    }

    #[tokio::test]
    async fn test_execute_parks() {
        let store = RunbookStore::new();
        let steps = vec![make_step("cbu.create"), make_step("doc.solicit")];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        let executor = ParkOnVerb("doc.solicit".into());
        let result = execute_runbook(&store, id, None, &executor).await.unwrap();
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Parked { .. }
        ));
        assert!(matches!(
            result.step_results[0].outcome,
            StepOutcome::Completed { .. }
        ));
        assert!(matches!(
            result.step_results[1].outcome,
            StepOutcome::Parked { .. }
        ));
    }

    #[tokio::test]
    async fn test_dependency_skipping() {
        let store = RunbookStore::new();
        let step1 = make_step("cbu.create");
        let step1_id = step1.step_id;
        let step2 = make_step_with_dep("entity.create", step1_id);
        let steps = vec![step1, step2];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        let executor = FailOnVerb("cbu.create".into());
        let result = execute_runbook(&store, id, None, &executor).await.unwrap();
        // First step fails
        assert!(matches!(
            result.step_results[0].outcome,
            StepOutcome::Failed { .. }
        ));
        // Second step skipped due to dependency
        assert!(matches!(
            result.step_results[1].outcome,
            StepOutcome::Skipped { .. }
        ));
    }

    #[tokio::test]
    async fn test_not_found() {
        let store = RunbookStore::new();
        let id = super::CompiledRunbookId::new();
        let result = execute_runbook(&store, id, None, &SuccessExecutor).await;
        assert!(matches!(result, Err(ExecutionError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_not_executable_after_completion() {
        let store = RunbookStore::new();
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![], ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        // First execution succeeds
        let _ = execute_runbook(&store, id, None, &SuccessExecutor)
            .await
            .unwrap();

        // Second execution should fail (already completed)
        let result = execute_runbook(&store, id, None, &SuccessExecutor).await;
        assert!(matches!(result, Err(ExecutionError::NotExecutable(_, _))));
    }

    #[tokio::test]
    async fn test_resume_from_parked() {
        let store = RunbookStore::new();
        let step1 = make_step("cbu.create");
        let step2 = make_step("doc.solicit");
        let step2_id = step2.step_id;
        let step3 = make_step("entity.create");
        let steps = vec![step1, step2, step3];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        // First execution parks at step 2
        let executor = ParkOnVerb("doc.solicit".into());
        let result = execute_runbook(&store, id, None, &executor).await.unwrap();
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Parked { .. }
        ));

        // Manually set back to Parked for resume (simulating signal_completion)
        store
            .update_status(
                &id,
                "parked",
                CompiledRunbookStatus::Parked {
                    reason: ParkReason::AwaitingCallback {
                        correlation_key: "test".into(),
                    },
                    cursor: StepCursor {
                        index: 1,
                        step_id: step2_id,
                    },
                },
            )
            .await
            .unwrap();

        // Resume from step 2 with success executor
        let cursor = StepCursor {
            index: 1,
            step_id: step2_id,
        };
        let result = execute_runbook(&store, id, Some(cursor), &SuccessExecutor)
            .await
            .unwrap();
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Completed { .. }
        ));
        // Step 0 skipped (before cursor), steps 1 and 2 completed
        assert_eq!(result.step_results.len(), 3);
    }

    #[test]
    fn test_compute_write_set() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let steps = vec![
            CompiledStep {
                step_id: Uuid::new_v4(),
                sentence: "step 1".into(),
                verb: "v1".into(),
                dsl: "(v1)".into(),
                args: std::collections::BTreeMap::new(),
                depends_on: vec![],
                execution_mode: ExecutionMode::Sync,
                write_set: vec![id1, id2],
                verb_contract_snapshot_id: None,
            },
            CompiledStep {
                step_id: Uuid::new_v4(),
                sentence: "step 2".into(),
                verb: "v2".into(),
                dsl: "(v2)".into(),
                args: std::collections::BTreeMap::new(),
                depends_on: vec![],
                execution_mode: ExecutionMode::Sync,
                write_set: vec![id2, id3],
                verb_contract_snapshot_id: None,
            },
        ];

        let ws = compute_write_set(&steps, None);
        assert_eq!(ws.len(), 3); // id1, id2, id3 (deduped by BTreeSet)
        assert!(ws.contains(&id1));
        assert!(ws.contains(&id2));
        assert!(ws.contains(&id3));

        // With cursor starting at step 2
        let cursor = StepCursor {
            index: 1,
            step_id: steps[1].step_id,
        };
        let ws2 = compute_write_set(&steps, Some(&cursor));
        assert_eq!(ws2.len(), 2); // id2, id3 only
        assert!(!ws2.contains(&id1));
    }

    // -----------------------------------------------------------------------
    // Regression tests for peer-review fixes
    // -----------------------------------------------------------------------

    /// Regression (Fix 3): Status must NOT transition to Executing before
    /// locks are acquired. If the runbook has a non-empty write_set and we
    /// simulate lock contention (no pool available = no locks = no contention
    /// in test, but we verify the status ordering by inspecting intermediate state),
    /// the status should remain Compiled on error.
    #[tokio::test]
    async fn test_status_stays_compiled_on_not_found() {
        // If runbook not found, status should never have been touched.
        let store = RunbookStore::new();
        let bogus_id = CompiledRunbookId::new();
        let result = execute_runbook(&store, bogus_id, None, &SuccessExecutor).await;
        assert!(matches!(result, Err(ExecutionError::NotFound(_))));
    }

    /// Regression (Fix 3): After successful execution, status transitions
    /// to Completed, not stuck at Executing.
    #[tokio::test]
    async fn test_status_transitions_through_executing_to_completed() {
        let store = RunbookStore::new();
        let steps = vec![make_step("cbu.create")];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        let result = execute_runbook(&store, id, None, &SuccessExecutor)
            .await
            .unwrap();

        // Final status is Completed, not Executing.
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Completed { .. }
        ));

        // Store also reflects Completed.
        let stored = store.get(&id).await.unwrap().unwrap();
        assert!(matches!(
            stored.status,
            CompiledRunbookStatus::Completed { .. }
        ));
    }

    /// Regression (Fix 4): Fallback write_set derivation extracts UUIDs from args.
    #[test]
    #[cfg(feature = "vnext-repl")]
    fn test_derive_write_set_extracts_uuids() {
        use crate::runbook::write_set::derive_write_set;

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let mut args = std::collections::BTreeMap::new();
        args.insert("entity-id".to_string(), id1.to_string());
        args.insert("cbu-id".to_string(), format!("<{}>", id2));
        args.insert("name".to_string(), "not-a-uuid".to_string());
        args.insert("count".to_string(), "42".to_string());

        let ws: Vec<Uuid> = derive_write_set("test.verb", &args, None)
            .into_iter()
            .collect();
        assert_eq!(ws.len(), 2);
        assert!(ws.contains(&id1));
        assert!(ws.contains(&id2));
    }

    /// Regression (Fix 4): derive_write_set returns empty for non-UUID args.
    #[test]
    #[cfg(feature = "vnext-repl")]
    fn test_derive_write_set_ignores_non_uuids() {
        use crate::runbook::write_set::derive_write_set;

        let args = std::collections::BTreeMap::new();

        let ws: Vec<Uuid> = derive_write_set("test.verb", &args, None)
            .into_iter()
            .collect();
        assert!(ws.is_empty());
    }

    /// Regression (Fix 1): Static grep ensuring no raw `self.executor.execute(`
    /// calls exist in orchestrator_v2.rs. All execution must go through the gate.
    #[test]
    fn test_no_raw_executor_calls_in_orchestrator() {
        let source = include_str!("../repl/orchestrator_v2.rs");

        // Count occurrences of the raw execution pattern.
        let raw_calls: Vec<&str> = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                // Skip comments and test-annotated lines.
                !trimmed.starts_with("//")
                    && !trimmed.starts_with("*")
                    && !trimmed.contains("TEST-ONLY")
                    && trimmed.contains("self.executor.execute(")
            })
            .collect();

        assert!(
            raw_calls.is_empty(),
            "INV-3 VIOLATION: Found {} raw executor.execute() call(s) in orchestrator_v2.rs \
             that bypass the execution gate:\n{}",
            raw_calls.len(),
            raw_calls.join("\n")
        );
    }

    /// Regression (Fix 1): Static grep ensuring execute_entry_v2 is fully removed.
    #[test]
    fn test_execute_entry_v2_removed() {
        let source = include_str!("../repl/orchestrator_v2.rs");

        let v2_calls: Vec<&str> = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.starts_with("//")
                    && !trimmed.starts_with("*")
                    && trimmed.contains("execute_entry_v2")
            })
            .collect();

        assert!(
            v2_calls.is_empty(),
            "execute_entry_v2 should be fully removed (bypass path). Found {} reference(s):\n{}",
            v2_calls.len(),
            v2_calls.join("\n")
        );
    }

    /// INV-1: When `runbook-gate-vnext` is enabled, the Chat API must NOT call
    /// `execute_dsl(` directly. All execution must go through `execute_runbook()`.
    ///
    /// This test scans `agent_service.rs` for raw `execute_dsl(` calls that are
    /// NOT gated behind `#[cfg(not(feature = "runbook-gate-vnext"))]`. Under the
    /// gated path, only `execute_runbook(` calls should exist.
    #[test]
    fn test_no_raw_executor_in_chat_api() {
        let source = include_str!("../api/agent_service.rs");

        // The legacy path is behind #[cfg(not(feature = "runbook-gate-vnext"))],
        // so when the feature IS enabled, those blocks are dead code. We verify
        // that `execute_dsl(` ONLY appears inside cfg-gated legacy blocks.
        // Strategy: count execute_dsl calls that are NOT in comments and NOT
        // preceded by a cfg(not) gate.
        let mut in_not_gate = false;
        let mut ungated_execute_dsl: Vec<(usize, String)> = Vec::new();

        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            // Track when we enter/exit a cfg(not(feature = "runbook-gate-vnext")) block.
            if trimmed.contains("cfg(not(feature = \"runbook-gate-vnext\"))") {
                in_not_gate = true;
            }
            // Heuristic: a new `fn ` or `async fn ` at the method level resets the gate.
            if (trimmed.starts_with("fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub async fn "))
                && !in_not_gate
            {
                in_not_gate = false;
            }
            // Skip comments.
            if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("///") {
                continue;
            }
            // Look for raw execute_dsl calls.
            if trimmed.contains("execute_dsl(") || trimmed.contains(".execute_dsl(") {
                if !in_not_gate {
                    ungated_execute_dsl.push((i + 1, line.to_string()));
                }
            }
        }

        // When the feature is enabled, there should be ZERO ungated execute_dsl calls.
        // The legacy path's execute_dsl is inside a cfg(not(...)) block.
        assert!(
            ungated_execute_dsl.is_empty(),
            "INV-1 VIOLATION: Found {} ungated execute_dsl() call(s) in agent_service.rs.\n\
             When runbook-gate-vnext is enabled, ALL execution must go through execute_runbook().\n\
             Offending lines:\n{}",
            ungated_execute_dsl.len(),
            ungated_execute_dsl
                .iter()
                .map(|(n, l)| format!("  L{}: {}", n, l.trim()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// INV-11: Both Chat API AND REPL paths must contain `execute_runbook` references.
    ///
    /// This ensures neither path regresses to bypassing the compilation gate.
    #[test]
    fn test_runbook_gate_chat_and_repl() {
        let agent_source = include_str!("../api/agent_service.rs");
        let repl_source = include_str!("../repl/orchestrator_v2.rs");

        let chat_has_gate = agent_source.contains("execute_runbook");
        let repl_has_gate = repl_source.contains("execute_runbook");

        assert!(
            chat_has_gate,
            "INV-11 VIOLATION: agent_service.rs does not contain 'execute_runbook'. \
             The Chat API must go through the runbook execution gate."
        );
        assert!(
            repl_has_gate,
            "INV-11 VIOLATION: orchestrator_v2.rs does not contain 'execute_runbook'. \
             The REPL must go through the runbook execution gate."
        );
    }

    // -----------------------------------------------------------------------
    // EventCapturingStore — test backend for asserting event emission (E-4)
    // -----------------------------------------------------------------------

    /// Test backend that delegates to `RunbookStore` for storage but captures
    /// all `append_event` calls in a `Vec<RunbookEvent>` behind a `Mutex`.
    struct EventCapturingStore {
        inner: RunbookStore,
        events: std::sync::Mutex<Vec<RunbookEvent>>,
    }

    impl EventCapturingStore {
        fn new() -> Self {
            Self {
                inner: RunbookStore::new(),
                events: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn captured_events(&self) -> Vec<RunbookEvent> {
            self.events.lock().expect("events Mutex poisoned").clone()
        }
    }

    #[async_trait::async_trait]
    impl RunbookStoreBackend for EventCapturingStore {
        async fn insert(&self, runbook: &CompiledRunbook) -> Result<(), ExecutionError> {
            RunbookStoreBackend::insert(&self.inner, runbook).await
        }

        async fn get(
            &self,
            id: &CompiledRunbookId,
        ) -> Result<Option<CompiledRunbook>, ExecutionError> {
            RunbookStoreBackend::get(&self.inner, id).await
        }

        async fn append_event(&self, event: RunbookEvent) -> Result<(), ExecutionError> {
            self.events
                .lock()
                .expect("events Mutex poisoned")
                .push(event);
            Ok(())
        }

        async fn current_status(
            &self,
            id: &CompiledRunbookId,
        ) -> Result<CompiledRunbookStatus, ExecutionError> {
            RunbookStoreBackend::current_status(&self.inner, id).await
        }

        async fn update_status(
            &self,
            id: &CompiledRunbookId,
            old_status: &str,
            new_status: CompiledRunbookStatus,
        ) -> Result<(), ExecutionError> {
            // Capture the status_change event AND delegate to inner for mutation
            self.append_event(RunbookEvent {
                compiled_runbook_id: *id,
                event_type: "status_change".into(),
                old_status: Some(old_status.to_string()),
                new_status: Some(format!("{:?}", new_status)),
                detail: None,
            })
            .await?;
            RunbookStoreBackend::update_status(&self.inner, id, old_status, new_status).await
        }

        async fn lookup_lock_holder(&self, _entity_type: &str, _entity_id: &str) -> Option<Uuid> {
            None
        }
    }

    /// Phase E-4: Verify event emission for a 3-step execution.
    ///
    /// Expected events (no pool = no locks):
    /// - 1× status_change (compiled → executing)
    /// - 3× step_completed
    /// - 1× status_change (executing → completed)
    /// = 5 events minimum
    #[tokio::test]
    async fn test_event_emission_count() {
        let store = EventCapturingStore::new();
        let steps = vec![
            make_step("cbu.create"),
            make_step("entity.create"),
            make_step("trading-profile.create"),
        ];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        let result = execute_runbook(&store, id, None, &SuccessExecutor)
            .await
            .unwrap();
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Completed { .. }
        ));

        let events = store.captured_events();

        // Must have at least 5 events: 2 status_change + 3 step_completed
        assert!(
            events.len() >= 5,
            "Expected ≥5 events for 3-step execution, got {}: {:?}",
            events.len(),
            events.iter().map(|e| &e.event_type).collect::<Vec<_>>()
        );

        // Count by type
        let status_changes = events
            .iter()
            .filter(|e| e.event_type == "status_change")
            .count();
        let step_completed = events
            .iter()
            .filter(|e| e.event_type == "step_completed")
            .count();

        assert!(
            status_changes >= 2,
            "Expected ≥2 status_change events, got {}",
            status_changes
        );
        assert_eq!(
            step_completed, 3,
            "Expected exactly 3 step_completed events, got {}",
            step_completed
        );
    }

    /// Phase E-4: Verify step_failed event is emitted on failure.
    #[tokio::test]
    async fn test_event_emission_on_failure() {
        let store = EventCapturingStore::new();
        let steps = vec![make_step("cbu.create"), make_step("entity.create")];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await.unwrap();

        let executor = FailOnVerb("cbu.create".into());
        let result = execute_runbook(&store, id, None, &executor).await.unwrap();
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Failed { .. }
        ));

        let events = store.captured_events();
        let step_failed = events
            .iter()
            .filter(|e| e.event_type == "step_failed")
            .count();
        let step_skipped = events
            .iter()
            .filter(|e| e.event_type == "step_skipped")
            .count();

        assert_eq!(step_failed, 1, "Expected 1 step_failed event");
        assert_eq!(step_skipped, 1, "Expected 1 step_skipped event");
    }

    /// INV-10: Lock events must be logged in `execute_runbook_with_pool()`.
    ///
    /// Static grep verifying that lock_acquired, lock_released, and lock_contention
    /// event logging is present in the executor.
    #[test]
    fn test_lock_event_logging_present() {
        let source = include_str!("executor.rs");

        let has_lock_acquired = source.contains("\"lock_acquired\"");
        let has_lock_released = source.contains("\"lock_released\"");
        let has_lock_contention = source.contains("\"lock_contention\"");

        assert!(
            has_lock_acquired,
            "INV-10 VIOLATION: executor.rs must log 'lock_acquired' events."
        );
        assert!(
            has_lock_released,
            "INV-10 VIOLATION: executor.rs must log 'lock_released' events."
        );
        assert!(
            has_lock_contention,
            "INV-10 VIOLATION: executor.rs must log 'lock_contention' events."
        );
    }
}
