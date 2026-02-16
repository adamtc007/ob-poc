//! Bridge adapters from REPL executor traits to `StepExecutor`.
//!
//! Two bridges are provided:
//!
//! 1. **`DslStepExecutor`** — wraps `Arc<dyn DslExecutor>` (sync-only path).
//!    Maps `Ok(json)` → `StepOutcome::Completed`, `Err(s)` → `StepOutcome::Failed`.
//!
//! 2. **`DslExecutorV2StepExecutor`** — wraps `Arc<dyn DslExecutorV2>` (durable/BPMN path).
//!    Maps `DslExecutionOutcome::Parked` → `StepOutcome::Parked` in addition to
//!    Completed/Failed.
//!
//! Both adapters extract the raw DSL string from `CompiledStep.dsl` — the same
//! string that was previously passed directly to the executor.

use std::sync::Arc;

use uuid::Uuid;

use super::executor::StepOutcome;
use super::types::CompiledStep;
use crate::repl::orchestrator_v2::{DslExecutionOutcome, DslExecutor, DslExecutorV2};

// ---------------------------------------------------------------------------
// DslStepExecutor — sync-only bridge
// ---------------------------------------------------------------------------

/// Bridge from `DslExecutor` (REPL's raw DSL executor) to `StepExecutor`.
///
/// Used for the standard sync execution path where parking is not possible.
pub struct DslStepExecutor {
    executor: Arc<dyn DslExecutor>,
}

impl DslStepExecutor {
    pub fn new(executor: Arc<dyn DslExecutor>) -> Self {
        Self { executor }
    }
}

#[async_trait::async_trait]
impl super::executor::StepExecutor for DslStepExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        match self.executor.execute(&step.dsl).await {
            Ok(result) => StepOutcome::Completed { result },
            Err(error) => StepOutcome::Failed { error },
        }
    }
}

// ---------------------------------------------------------------------------
// DslExecutorV2StepExecutor — durable/BPMN bridge
// ---------------------------------------------------------------------------

/// Bridge from `DslExecutorV2` (WorkflowDispatcher path) to `StepExecutor`.
///
/// This adapter handles the `Parked` outcome from `DslExecutorV2`, mapping it
/// to `StepOutcome::Parked` so the execution gate can suspend the runbook
/// and record the cursor for later resumption.
pub struct DslExecutorV2StepExecutor {
    executor: Arc<dyn DslExecutorV2>,
    /// Runbook ID passed through to `execute_v2` for correlation.
    runbook_id: Uuid,
}

impl DslExecutorV2StepExecutor {
    pub fn new(executor: Arc<dyn DslExecutorV2>, runbook_id: Uuid) -> Self {
        Self {
            executor,
            runbook_id,
        }
    }
}

#[async_trait::async_trait]
impl super::executor::StepExecutor for DslExecutorV2StepExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        match self
            .executor
            .execute_v2(&step.dsl, step.step_id, self.runbook_id)
            .await
        {
            DslExecutionOutcome::Completed(result) => StepOutcome::Completed { result },
            DslExecutionOutcome::Parked {
                correlation_key,
                message,
                ..
            } => StepOutcome::Parked {
                correlation_key,
                message,
            },
            DslExecutionOutcome::Failed(error) => StepOutcome::Failed { error },
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::executor::StepExecutor;
    use crate::runbook::types::ExecutionMode;
    use std::collections::HashMap;

    /// Stub DslExecutor that returns success.
    struct SuccessExecutor;

    #[async_trait::async_trait]
    impl DslExecutor for SuccessExecutor {
        async fn execute(&self, _dsl: &str) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"status": "ok"}))
        }
    }

    /// Stub DslExecutor that returns failure.
    struct FailureExecutor;

    #[async_trait::async_trait]
    impl DslExecutor for FailureExecutor {
        async fn execute(&self, _dsl: &str) -> Result<serde_json::Value, String> {
            Err("execution failed".into())
        }
    }

    /// Stub DslExecutorV2 that returns Parked.
    struct ParkingExecutor;

    #[async_trait::async_trait]
    impl DslExecutorV2 for ParkingExecutor {
        async fn execute_v2(
            &self,
            _dsl: &str,
            _entry_id: Uuid,
            _runbook_id: Uuid,
        ) -> DslExecutionOutcome {
            DslExecutionOutcome::Parked {
                task_id: Uuid::nil(),
                correlation_key: "test-corr-key".into(),
                timeout: None,
                message: "Awaiting callback".into(),
            }
        }
    }

    fn test_step() -> CompiledStep {
        CompiledStep {
            step_id: Uuid::new_v4(),
            sentence: "test step".into(),
            verb: "test.verb".into(),
            dsl: "(test.verb :arg1 \"value\")".into(),
            args: std::collections::BTreeMap::new(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
        }
    }

    #[tokio::test]
    async fn test_dsl_step_executor_success() {
        let executor = DslStepExecutor::new(Arc::new(SuccessExecutor));
        let step = test_step();
        let outcome = executor.execute_step(&step).await;

        match outcome {
            StepOutcome::Completed { result } => {
                assert_eq!(result["status"], "ok");
            }
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_dsl_step_executor_failure() {
        let executor = DslStepExecutor::new(Arc::new(FailureExecutor));
        let step = test_step();
        let outcome = executor.execute_step(&step).await;

        match outcome {
            StepOutcome::Failed { error } => {
                assert_eq!(error, "execution failed");
            }
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_dsl_executor_v2_step_executor_parked() {
        let executor = DslExecutorV2StepExecutor::new(Arc::new(ParkingExecutor), Uuid::new_v4());
        let step = test_step();
        let outcome = executor.execute_step(&step).await;

        match outcome {
            StepOutcome::Parked {
                correlation_key,
                message,
            } => {
                assert_eq!(correlation_key, "test-corr-key");
                assert_eq!(message, "Awaiting callback");
            }
            other => panic!("Expected Parked, got {:?}", other),
        }
    }
}
