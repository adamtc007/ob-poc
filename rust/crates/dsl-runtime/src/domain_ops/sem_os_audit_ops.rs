//! Audit domain CustomOps (Spec §C.8 — 8 verbs).
//!
//! All verbs delegate to general SemReg MCP tools (planning / decisions /
//! evidence categories — `sem_reg_create_plan`, `sem_reg_record_decision`,
//! `sem_reg_record_observation`, etc.) via the `StewardshipDispatch`
//! trait. The ob-poc-side dispatcher (`ObPocStewardshipDispatch`)
//! cascades phase 0 → phase 1 → general `dispatch_tool`, so the audit
//! tools — none of which carry the `stew_` prefix — fall through to the
//! general arm transparently.
//!
//! Allowed in BOTH Research and Governed AgentModes.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::StewardshipDispatch;

async fn dispatch_tool(
    ctx: &VerbExecutionContext,
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<VerbExecutionOutcome> {
    let dispatcher = ctx.service::<dyn StewardshipDispatch>()?;
    let outcome = dispatcher
        .dispatch(tool_name, args, &ctx.principal)
        .await?
        .ok_or_else(|| anyhow!("Unknown audit tool: {}", tool_name))?;
    if outcome.success {
        Ok(VerbExecutionOutcome::Record(outcome.data))
    } else {
        Err(anyhow!(
            "{}",
            outcome
                .message
                .unwrap_or_else(|| format!("Audit tool {} failed", tool_name))
        ))
    }
}

macro_rules! audit_op {
    ($struct_name:ident, $verb:literal, $tool:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct_name;

        #[async_trait]
        impl CustomOperation for $struct_name {
            fn domain(&self) -> &'static str {
                "audit"
            }
            fn verb(&self) -> &'static str {
                $verb
            }
            fn rationale(&self) -> &'static str {
                $rationale
            }

            async fn execute_json(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                _pool: &PgPool,
            ) -> Result<VerbExecutionOutcome> {
                dispatch_tool(ctx, $tool, args).await
            }

            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

// ── Plan Management ───────────────────────────────────────────────

audit_op!(
    AuditCreatePlanOp,
    "create-plan",
    "sem_reg_create_plan",
    "Delegates to sem_reg_create_plan MCP tool"
);

audit_op!(
    AuditAddPlanStepOp,
    "add-plan-step",
    "sem_reg_add_plan_step",
    "Delegates to sem_reg_add_plan_step MCP tool"
);

audit_op!(
    AuditValidatePlanOp,
    "validate-plan",
    "sem_reg_validate_plan",
    "Delegates to sem_reg_validate_plan MCP tool"
);

audit_op!(
    AuditExecutePlanStepOp,
    "execute-plan-step",
    "sem_reg_execute_plan_step",
    "Delegates to sem_reg_execute_plan_step MCP tool"
);

// ── Decision & Observation Recording ──────────────────────────────

audit_op!(
    AuditRecordDecisionOp,
    "record-decision",
    "sem_reg_record_decision",
    "Delegates to sem_reg_record_decision MCP tool"
);

audit_op!(
    AuditRecordEscalationOp,
    "record-escalation",
    "sem_reg_record_escalation",
    "Delegates to sem_reg_record_escalation MCP tool"
);

audit_op!(
    AuditRecordDisambiguationOp,
    "record-disambiguation",
    "sem_reg_record_disambiguation",
    "Delegates to sem_reg_record_disambiguation MCP tool"
);

audit_op!(
    AuditRecordObservationOp,
    "record-observation",
    "sem_reg_record_observation",
    "Delegates to sem_reg_record_observation MCP tool"
);
