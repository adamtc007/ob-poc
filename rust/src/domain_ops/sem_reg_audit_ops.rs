//! Audit domain CustomOps (Spec §C.8 — 8 verbs).
//!
//! All verbs delegate to existing sem_reg MCP tools via `dispatch_tool()`.
//! Allowed in BOTH Research and Governed AgentModes.

use anyhow::Result;
use async_trait::async_trait;

use ob_poc_macros::register_custom_op;

use super::sem_reg_helpers::delegate_to_tool;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Macro to reduce boilerplate for simple delegation ops ──────────

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

            #[cfg(feature = "database")]
            async fn execute(
                &self,
                verb_call: &VerbCall,
                ctx: &mut ExecutionContext,
                pool: &PgPool,
            ) -> Result<ExecutionResult> {
                delegate_to_tool(pool, ctx, verb_call, $tool).await
            }

            #[cfg(not(feature = "database"))]
            async fn execute(
                &self,
                _verb_call: &VerbCall,
                _ctx: &mut ExecutionContext,
            ) -> Result<ExecutionResult> {
                Err(anyhow::anyhow!("audit.{} requires database", $verb))
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
