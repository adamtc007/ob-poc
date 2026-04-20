//! Governance domain CustomOps (Spec §C.4 — 9 verbs).
//!
//! Mixed delegation: most verbs route through `StewardshipDispatch`
//! (stewardship Phase 0 tools + general SemReg MCP fallback); rollback
//! does direct SQL against `sem_reg_pub.active_snapshot_set` because it
//! manipulates the pointer itself rather than issuing a stewardship
//! tool call. The cascade in `ObPocStewardshipDispatch` covers both
//! stewardship and general SemReg MCP, so `validate` / `dry-run` (which
//! route to `sem_reg_validate_plan`) work through the same trait.
//!
//! Allowed in Governed mode only. Blocked in Research mode.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::json_extract_string;
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
        .ok_or_else(|| anyhow!("Unknown stewardship tool: {}", tool_name))?;
    if outcome.success {
        Ok(VerbExecutionOutcome::Record(outcome.data))
    } else {
        Err(anyhow!(
            "{}",
            outcome
                .message
                .unwrap_or_else(|| format!("Stewardship tool {} failed", tool_name))
        ))
    }
}

macro_rules! governance_op {
    ($struct_name:ident, $verb:literal, $tool:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct_name;

        #[async_trait]
        impl CustomOperation for $struct_name {
            fn domain(&self) -> &'static str {
                "governance"
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

governance_op!(
    GovernanceGatePrecheckOp,
    "gate-precheck",
    "stew_gate_precheck",
    "Delegates to stew_gate_precheck Phase 0 tool"
);

governance_op!(
    GovernanceSubmitForReviewOp,
    "submit-for-review",
    "stew_submit_for_review",
    "Delegates to stew_submit_for_review Phase 0 tool"
);

governance_op!(
    GovernanceRecordReviewOp,
    "record-review",
    "stew_record_review_decision",
    "Delegates to stew_record_review_decision Phase 0 tool"
);

governance_op!(
    GovernanceValidateOp,
    "validate",
    "sem_reg_validate_plan",
    "Validates changeset via CoreService validation pipeline (general SemReg MCP)"
);

governance_op!(
    GovernanceDryRunOp,
    "dry-run",
    "sem_reg_validate_plan",
    "Runs Stage 2 dry-run via CoreService (general SemReg MCP)"
);

governance_op!(
    GovernancePlanPublishOp,
    "plan-publish",
    "stew_impact_analysis",
    "Plans publish operation: diff against active snapshot set"
);

governance_op!(
    GovernancePublishOp,
    "publish",
    "stew_publish",
    "Publishes changeset via stew_publish Phase 0 tool"
);

governance_op!(
    GovernancePublishBatchOp,
    "publish-batch",
    "stew_publish",
    "Batch publishes via stew_publish with multiple IDs"
);

/// Reverts `active_snapshot_set` pointer to a previous version.
/// Direct SQL — the pointer is an ob-poc-owned table and the operation
/// is simple enough not to warrant a stewardship tool round-trip.
#[register_custom_op]
pub struct GovernanceRollbackOp;

#[async_trait]
impl CustomOperation for GovernanceRollbackOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "rollback"
    }
    fn rationale(&self) -> &'static str {
        "Reverts active_snapshot_set pointer to previous version"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let target = json_extract_string(args, "target-snapshot-set-id")?;
        sqlx::query("UPDATE sem_reg_pub.active_snapshot_set SET snapshot_set_id = $1")
            .bind(&target)
            .execute(pool)
            .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "rolled_back_to": target,
            "status": "success",
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
