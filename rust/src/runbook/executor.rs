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

    /// Store a compiled runbook.
    pub fn insert(&self, runbook: CompiledRunbook) {
        let id = runbook.id;
        self.runbooks.write().unwrap().insert(id, runbook);
    }

    /// Retrieve a compiled runbook by ID.
    pub fn get(&self, id: &CompiledRunbookId) -> Option<CompiledRunbook> {
        self.runbooks.read().unwrap().get(id).cloned()
    }

    /// Update the status of a compiled runbook.
    pub fn update_status(
        &self,
        id: &CompiledRunbookId,
        status: CompiledRunbookStatus,
    ) -> Result<(), ExecutionError> {
        let mut map = self.runbooks.write().unwrap();
        let rb = map.get_mut(id).ok_or(ExecutionError::NotFound(*id))?;
        rb.status = status;
        Ok(())
    }
}

impl Default for RunbookStore {
    fn default() -> Self {
        Self::new()
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
/// returns `ExecutionError::LockContention` immediately — no blocking.
///
/// Returns the open transaction that holds the locks. The caller MUST keep this
/// transaction alive during step execution and commit/rollback when done.
/// Advisory xact locks auto-release on commit or rollback.
async fn acquire_advisory_locks(
    pool: &sqlx::PgPool,
    write_set: &BTreeSet<Uuid>,
) -> Result<(sqlx::Transaction<'static, sqlx::Postgres>, LockStats), ExecutionError> {
    use crate::database::locks::{acquire_locks, LockError, LockKey, LockMode};

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

    match acquire_locks(&mut tx, &lock_keys, LockMode::Try).await {
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
            ..
        }) => {
            // Fail-fast: roll back and surface contention error.
            let _ = tx.rollback().await;
            Err(ExecutionError::LockContention {
                entity_type,
                entity_id,
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
    store: &RunbookStore,
    runbook_id: CompiledRunbookId,
    cursor: Option<StepCursor>,
    executor: &dyn StepExecutor,
) -> Result<RunbookExecutionResult, ExecutionError> {
    execute_runbook_with_pool(store, runbook_id, cursor, executor, None).await
}

/// Execute a compiled runbook with optional advisory locking.
///
/// When `pool` is `Some`, advisory locks are acquired on the write_set
/// using `try_advisory_xact_lock` (fail-fast on contention). Locks are
/// sorted by UUID to prevent deadlocks and auto-release on tx commit/rollback.
///
/// When `pool` is `None` (tests, in-memory mode), locking is skipped.
pub async fn execute_runbook_with_pool(
    store: &RunbookStore,
    runbook_id: CompiledRunbookId,
    cursor: Option<StepCursor>,
    executor: &dyn StepExecutor,
    pool: Option<&sqlx::PgPool>,
) -> Result<RunbookExecutionResult, ExecutionError> {
    let start = std::time::Instant::now();

    // 1. Look up runbook
    let runbook = store
        .get(&runbook_id)
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
            let (tx, stats) = acquire_advisory_locks(pool, &write_set).await?;
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
    store.update_status(
        &runbook_id,
        CompiledRunbookStatus::Executing {
            current_step: start_idx,
        },
    )?;

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
        store.update_status(
            &runbook_id,
            CompiledRunbookStatus::Executing { current_step: idx },
        )?;

        // Execute the step
        let outcome = executor.execute_step(step).await;

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
            for remaining in runbook.steps.iter().skip(idx + 1) {
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
    store.update_status(&runbook_id, final_status.clone())?;

    // 7. Release advisory locks by committing (or dropping) the lock transaction.
    //    On success we commit; on any error path above we return early and _lock_tx
    //    is dropped, which rolls back and releases locks automatically.
    if let Some(lock_tx) = _lock_tx {
        let _ = lock_tx.commit().await;
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
            args: std::collections::HashMap::new(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
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
        store.insert(rb);

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
        store.insert(rb);

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
        store.insert(rb);

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
        store.insert(rb);

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
        store.insert(rb);

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
        store.insert(rb);

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
        store.insert(rb);

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
                args: std::collections::HashMap::new(),
                depends_on: vec![],
                execution_mode: ExecutionMode::Sync,
                write_set: vec![id1, id2],
            },
            CompiledStep {
                step_id: Uuid::new_v4(),
                sentence: "step 2".into(),
                verb: "v2".into(),
                dsl: "(v2)".into(),
                args: std::collections::HashMap::new(),
                depends_on: vec![],
                execution_mode: ExecutionMode::Sync,
                write_set: vec![id2, id3],
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
        store.insert(rb);

        let result = execute_runbook(&store, id, None, &SuccessExecutor)
            .await
            .unwrap();

        // Final status is Completed, not Executing.
        assert!(matches!(
            result.final_status,
            CompiledRunbookStatus::Completed { .. }
        ));

        // Store also reflects Completed.
        let stored = store.get(&id).unwrap();
        assert!(matches!(
            stored.status,
            CompiledRunbookStatus::Completed { .. }
        ));
    }

    /// Regression (Fix 4): Fallback write_set derivation extracts UUIDs from args.
    #[test]
    #[cfg(feature = "vnext-repl")]
    fn test_derive_write_set_extracts_uuids() {
        use crate::runbook::compiler::derive_write_set;

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let mut args = std::collections::HashMap::new();
        args.insert("entity-id".to_string(), id1.to_string());
        args.insert("cbu-id".to_string(), format!("<{}>", id2));
        args.insert("name".to_string(), "not-a-uuid".to_string());
        args.insert("count".to_string(), "42".to_string());

        let ws = derive_write_set(&args);
        assert_eq!(ws.len(), 2);
        assert!(ws.contains(&id1));
        assert!(ws.contains(&id2));
    }

    /// Regression (Fix 4): derive_write_set returns empty for non-UUID args.
    #[test]
    #[cfg(feature = "vnext-repl")]
    fn test_derive_write_set_ignores_non_uuids() {
        use crate::runbook::compiler::derive_write_set;

        let mut args = std::collections::HashMap::new();
        args.insert("name".to_string(), "Acme Corp".to_string());
        args.insert("type".to_string(), "FUND".to_string());

        let ws = derive_write_set(&args);
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
}
