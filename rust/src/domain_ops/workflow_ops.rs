//! `workflow.start-process` — Pattern B verb op that starts a BPMN process
//! instance via the `ProcessRegistryService` platform service.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime::service_traits::ProcessRegistryService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use sem_os_postgres::ops::SemOsVerbOp;

/// Start a named BPMN process and surface its initial pending human task (if any).
pub struct WorkflowStartProcess;

#[async_trait]
impl SemOsVerbOp for WorkflowStartProcess {
    fn fqn(&self) -> &str {
        "workflow.start-process"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let process_name = args
            .get("process_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("process_name required"))?;

        let initial_data = args
            .get("initial_data")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let registry = ctx.service::<dyn ProcessRegistryService>()?;
        let result = registry.start_process(process_name, initial_data).await?;

        // result = { instance_id, status, bpmn_form? }
        // The response adapter detects the bpmn_form key and promotes it to
        // ReplResponseV2.bpmn_form → ChatMessage.bpmn_form.
        Ok(VerbExecutionOutcome::Record(result))
    }
}
