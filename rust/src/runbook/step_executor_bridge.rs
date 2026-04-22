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
use crate::sequencer::{DslExecutionOutcome, DslExecutor, DslExecutorV2};

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

    /// Phase B.2b-δ (2026-04-22): routes step execution through the
    /// caller-owned scope so the runbook executor's outer scope (B.2b-ε)
    /// is shared across every step.
    async fn execute_step_in_scope(
        &self,
        step: &CompiledStep,
        scope: &mut dyn dsl_runtime::tx::TransactionScope,
    ) -> StepOutcome {
        match self.executor.execute_in_scope(&step.dsl, scope).await {
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
    session_stack: Option<ob_poc_types::session_stack::SessionStackState>,
}

impl DslExecutorV2StepExecutor {
    pub fn new(
        executor: Arc<dyn DslExecutorV2>,
        runbook_id: Uuid,
        session_stack: Option<ob_poc_types::session_stack::SessionStackState>,
    ) -> Self {
        Self {
            executor,
            runbook_id,
            session_stack,
        }
    }
}

#[async_trait::async_trait]
impl super::executor::StepExecutor for DslExecutorV2StepExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        match self
            .executor
            .execute_v2(
                &step.dsl,
                step.step_id,
                self.runbook_id,
                self.session_stack.clone(),
            )
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
// VerbExecutionPortStepExecutor — SemOS execution port bridge
// ---------------------------------------------------------------------------

/// Bridge from `VerbExecutionPort` (SemOS execution contract) to `StepExecutor`.
///
/// This adapter translates each `CompiledStep` into a `VerbExecutionPort::execute_verb()`
/// call, converting the step's verb FQN and args to JSON, and mapping the
/// `VerbExecutionOutcome` back to `StepOutcome`.
pub struct VerbExecutionPortStepExecutor {
    port: Arc<dyn dsl_runtime::VerbExecutionPort>,
    /// Principal used for all executions in this runbook.
    principal: sem_os_core::principal::Principal,
    /// Session ID for correlation.
    session_id: Option<Uuid>,
}

impl VerbExecutionPortStepExecutor {
    pub fn new(
        port: Arc<dyn dsl_runtime::VerbExecutionPort>,
        principal: sem_os_core::principal::Principal,
        session_id: Option<Uuid>,
    ) -> Self {
        Self {
            port,
            principal,
            session_id,
        }
    }
}

#[async_trait::async_trait]
impl super::executor::StepExecutor for VerbExecutionPortStepExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        // Build execution context
        let mut ctx = dsl_runtime::VerbExecutionContext::new(self.principal.clone());
        if let Some(sid) = self.session_id {
            ctx.extensions = serde_json::json!({"session_id": sid.to_string()});
        }

        // Convert step args (BTreeMap<String, String>) to JSON object
        let args: serde_json::Value = step
            .args
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        // Execute through the SemOS port
        match self.port.execute_verb(&step.verb, args, &mut ctx).await {
            Ok(result) => {
                let json = match &result.outcome {
                    dsl_runtime::VerbExecutionOutcome::Uuid(id) => {
                        serde_json::json!({"type": "uuid", "value": id.to_string()})
                    }
                    dsl_runtime::VerbExecutionOutcome::Record(v) => {
                        serde_json::json!({"type": "record", "value": v})
                    }
                    dsl_runtime::VerbExecutionOutcome::RecordSet(v) => {
                        serde_json::json!({"type": "record_set", "value": v})
                    }
                    dsl_runtime::VerbExecutionOutcome::Affected(n) => {
                        serde_json::json!({"type": "affected", "value": n})
                    }
                    dsl_runtime::VerbExecutionOutcome::Void => {
                        serde_json::json!({"type": "void"})
                    }
                };
                StepOutcome::Completed { result: json }
            }
            Err(e) => StepOutcome::Failed {
                error: e.to_string(),
            },
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
            _session_stack: Option<ob_poc_types::session_stack::SessionStackState>,
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
            verb_contract_snapshot_id: None,
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
        let executor =
            DslExecutorV2StepExecutor::new(Arc::new(ParkingExecutor), Uuid::new_v4(), None);
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
