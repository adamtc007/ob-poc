//! Governance domain verbs (Spec §C.4 — 9 verbs) — SemOS-side YAML-first
//! re-implementation. Mixed delegation: most route through
//! [`StewardshipDispatch`] (stewardship + general SemReg MCP cascade);
//! `rollback` does direct SQL against `sem_reg_pub.active_snapshot_set`.
//! Allowed in Governed mode only.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::domain_ops::helpers::json_extract_string;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::stewardship_helper::dispatch_stewardship_tool;
use super::SemOsVerbOp;

macro_rules! governance_op {
    ($struct:ident, $verb:literal, $tool:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("governance.", $verb)
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

governance_op!(GatePrecheck, "gate-precheck", "stew_gate_precheck");
governance_op!(SubmitForReview, "submit-for-review", "stew_submit_for_review");
governance_op!(RecordReview, "record-review", "stew_record_review_decision");
governance_op!(Validate, "validate", "sem_reg_validate_plan");
governance_op!(DryRun, "dry-run", "sem_reg_validate_plan");
governance_op!(PlanPublish, "plan-publish", "stew_impact_analysis");
governance_op!(Publish, "publish", "stew_publish");
governance_op!(PublishBatch, "publish-batch", "stew_publish");

/// `governance.rollback` — reverts the `active_snapshot_set` pointer to a
/// previous version via direct SQL (no stewardship tool equivalent).
pub struct Rollback;

#[async_trait]
impl SemOsVerbOp for Rollback {
    fn fqn(&self) -> &str {
        "governance.rollback"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let target = json_extract_string(args, "target-snapshot-set-id")?;
        sqlx::query("UPDATE sem_reg_pub.active_snapshot_set SET snapshot_set_id = $1")
            .bind(&target)
            .execute(scope.executor())
            .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "rolled_back_to": target,
            "status": "success",
        })))
    }
}
