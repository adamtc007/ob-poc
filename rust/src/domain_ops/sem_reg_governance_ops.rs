//! Governance domain CustomOps (Spec §C.4 — 9 verbs).
//!
//! Mixed delegation: stewardship tools for review/gate, direct SQL for
//! CoreService methods (validate, dry-run, plan, publish, batch, rollback).
//! Allowed in Governed mode only (pipeline). Blocked in Research mode.

use anyhow::Result;
use async_trait::async_trait;

use ob_poc_macros::register_custom_op;

use super::sem_reg_helpers::{delegate_to_stew_tool, delegate_to_tool};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Stewardship-Delegated Ops ─────────────────────────────────────

/// Gate precheck before submitting for review.
#[register_custom_op]
pub struct GovernanceGatePrecheckOp;

#[async_trait]
impl CustomOperation for GovernanceGatePrecheckOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "gate-precheck"
    }
    fn rationale(&self) -> &'static str {
        "Delegates to stew_gate_precheck Phase 0 tool"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_stew_tool(pool, ctx, verb_call, "stew_gate_precheck").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "governance.gate-precheck requires database"
        ))
    }
}

/// Submit changeset for review.
#[register_custom_op]
pub struct GovernanceSubmitForReviewOp;

#[async_trait]
impl CustomOperation for GovernanceSubmitForReviewOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "submit-for-review"
    }
    fn rationale(&self) -> &'static str {
        "Delegates to stew_submit_for_review Phase 0 tool"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_stew_tool(pool, ctx, verb_call, "stew_submit_for_review").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "governance.submit-for-review requires database"
        ))
    }
}

/// Record a review decision on a changeset.
#[register_custom_op]
pub struct GovernanceRecordReviewOp;

#[async_trait]
impl CustomOperation for GovernanceRecordReviewOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "record-review"
    }
    fn rationale(&self) -> &'static str {
        "Delegates to stew_record_review_decision Phase 0 tool"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_stew_tool(pool, ctx, verb_call, "stew_record_review_decision").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "governance.record-review requires database"
        ))
    }
}

// ── CoreService-Delegated Ops (via direct SQL/tool dispatch) ──────

/// Validate a changeset (Stage 1 artifact integrity).
#[register_custom_op]
pub struct GovernanceValidateOp;

#[async_trait]
impl CustomOperation for GovernanceValidateOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "validate"
    }
    fn rationale(&self) -> &'static str {
        "Validates changeset via CoreService validation pipeline"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "sem_reg_validate_plan").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("governance.validate requires database"))
    }
}

/// Dry-run a changeset (Stage 2 scratch schema).
#[register_custom_op]
pub struct GovernanceDryRunOp;

#[async_trait]
impl CustomOperation for GovernanceDryRunOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "dry-run"
    }
    fn rationale(&self) -> &'static str {
        "Runs Stage 2 dry-run via CoreService"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "sem_reg_validate_plan").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("governance.dry-run requires database"))
    }
}

/// Plan a publish operation (diff preview).
#[register_custom_op]
pub struct GovernancePlanPublishOp;

#[async_trait]
impl CustomOperation for GovernancePlanPublishOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "plan-publish"
    }
    fn rationale(&self) -> &'static str {
        "Plans publish operation: diff against active snapshot set"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_stew_tool(pool, ctx, verb_call, "stew_impact_analysis").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("governance.plan-publish requires database"))
    }
}

/// Publish a validated changeset to the active snapshot set.
#[register_custom_op]
pub struct GovernancePublishOp;

#[async_trait]
impl CustomOperation for GovernancePublishOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "publish"
    }
    fn rationale(&self) -> &'static str {
        "Publishes changeset via stew_publish Phase 0 tool"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_stew_tool(pool, ctx, verb_call, "stew_publish").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("governance.publish requires database"))
    }
}

/// Batch publish multiple changesets in topological order.
#[register_custom_op]
pub struct GovernancePublishBatchOp;

#[async_trait]
impl CustomOperation for GovernancePublishBatchOp {
    fn domain(&self) -> &'static str {
        "governance"
    }
    fn verb(&self) -> &'static str {
        "publish-batch"
    }
    fn rationale(&self) -> &'static str {
        "Batch publishes via stew_publish with multiple IDs"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_stew_tool(pool, ctx, verb_call, "stew_publish").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "governance.publish-batch requires database"
        ))
    }
}

/// Rollback: revert the active snapshot set pointer.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use super::sem_reg_helpers::get_string_arg;

        let target = get_string_arg(verb_call, "target-snapshot-set-id").ok_or_else(|| {
            anyhow::anyhow!("governance.rollback requires target-snapshot-set-id")
        })?;

        sqlx::query("UPDATE sem_reg_pub.active_snapshot_set SET snapshot_set_id = $1")
            .bind(&target)
            .execute(pool)
            .await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "rolled_back_to": target,
            "status": "success",
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("governance.rollback requires database"))
    }
}
