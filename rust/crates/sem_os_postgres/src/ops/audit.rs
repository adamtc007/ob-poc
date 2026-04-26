//! Audit domain verbs (Spec §C.8 — 8 verbs) — SemOS-side YAML-first
//! re-implementation. All delegate to general SemReg MCP tools
//! (`sem_reg_*`) via [`StewardshipDispatch`]. Allowed in BOTH Research
//! and Governed AgentModes.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::stewardship_helper::dispatch_stewardship_tool;
use super::SemOsVerbOp;

macro_rules! audit_op {
    ($struct:ident, $verb:literal, $tool:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("audit.", $verb)
            }
            async fn execute(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                _scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                dispatch_stewardship_tool(ctx, $tool, args).await
            }
        }
    };
}

audit_op!(CreatePlan, "create-plan", "sem_reg_create_plan");
audit_op!(AddPlanStep, "add-plan-step", "sem_reg_add_plan_step");
audit_op!(ValidatePlan, "validate-plan", "sem_reg_validate_plan");
audit_op!(
    ExecutePlanStep,
    "execute-plan-step",
    "sem_reg_execute_plan_step"
);
audit_op!(RecordDecision, "record-decision", "sem_reg_record_decision");
audit_op!(
    RecordEscalation,
    "record-escalation",
    "sem_reg_record_escalation"
);
audit_op!(
    RecordDisambiguation,
    "record-disambiguation",
    "sem_reg_record_disambiguation"
);
audit_op!(
    RecordObservation,
    "record-observation",
    "sem_reg_record_observation"
);
