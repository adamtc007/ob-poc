//! Investor lifecycle verbs (13 plugin verbs) — YAML-first
//! re-implementation of `rust/config/verbs/investor.yaml`.
//!
//! State machine: ENQUIRY → PENDING_DOCUMENTS → KYC_IN_PROGRESS
//! → KYC_APPROVED → ELIGIBLE_TO_SUBSCRIBE → SUBSCRIBED →
//! ACTIVE_HOLDER → REDEEMING → OFFBOARDED. Legacy transitions
//! validated by the `trg_validate_investor_lifecycle` trigger;
//! history by `trg_log_investor_lifecycle`. sqlx::query! macros
//! rewritten as runtime queries (slice #10 pattern).

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── investor.request-documents ────────────────────────────────────────────────

pub struct RequestDocuments;

#[async_trait]
impl SemOsVerbOp for RequestDocuments {
    fn fqn(&self) -> &str {
        "investor.request-documents"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'PENDING_DOCUMENTS', lifecycle_notes = $2, updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.start-kyc ────────────────────────────────────────────────────────

pub struct StartKyc;

#[async_trait]
impl SemOsVerbOp for StartKyc {
    fn fqn(&self) -> &str {
        "investor.start-kyc"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'KYC_IN_PROGRESS',
                   kyc_status = 'IN_PROGRESS',
                   kyc_case_id = COALESCE($2, kyc_case_id),
                   lifecycle_notes = $3,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(case_id)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        // Phase C.3 rollout: investor KYC lifecycle advance.
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:kyc-in-progress",
            "investor/lifecycle",
            "investor.start-kyc — KYC_IN_PROGRESS",
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.approve-kyc ──────────────────────────────────────────────────────

pub struct ApproveKyc;

#[async_trait]
impl SemOsVerbOp for ApproveKyc {
    fn fqn(&self) -> &str {
        "investor.approve-kyc"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let risk_rating = json_extract_string_opt(args, "risk-rating");
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'KYC_APPROVED',
                   kyc_status = 'APPROVED',
                   kyc_approved_at = NOW(),
                   kyc_risk_rating = COALESCE($2, kyc_risk_rating),
                   lifecycle_notes = $3,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&risk_rating)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:kyc-approved",
            "investor/lifecycle",
            "investor.approve-kyc — KYC_APPROVED",
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.reject-kyc ───────────────────────────────────────────────────────

pub struct RejectKyc;

#[async_trait]
impl SemOsVerbOp for RejectKyc {
    fn fqn(&self) -> &str {
        "investor.reject-kyc"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let reason = json_extract_string(args, "reason")?;
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'REJECTED',
                   kyc_status = 'REJECTED',
                   rejection_reason = $2,
                   lifecycle_notes = $3,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&reason)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:rejected",
            "investor/lifecycle",
            &format!("investor.reject-kyc — REJECTED ({})", reason),
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.mark-eligible ────────────────────────────────────────────────────

pub struct MarkEligible;

#[async_trait]
impl SemOsVerbOp for MarkEligible {
    fn fqn(&self) -> &str {
        "investor.mark-eligible"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let investor_type = json_extract_string_opt(args, "investor-type");
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'ELIGIBLE_TO_SUBSCRIBE',
                   investor_type = COALESCE($2, investor_type),
                   lifecycle_notes = $3,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&investor_type)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:eligible-to-subscribe",
            "investor/lifecycle",
            "investor.mark-eligible — ELIGIBLE_TO_SUBSCRIBE",
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.record-subscription ──────────────────────────────────────────────

pub struct RecordSubscription;

#[async_trait]
impl SemOsVerbOp for RecordSubscription {
    fn fqn(&self) -> &str {
        "investor.record-subscription"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let holding_id = json_extract_uuid_opt(args, ctx, "holding-id");
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'SUBSCRIBED',
                   first_subscription_at = COALESCE(first_subscription_at, NOW()),
                   lifecycle_notes = $2,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;

        if let Some(hid) = holding_id {
            sqlx::query(
                r#"UPDATE "ob-poc".holdings SET investor_id = $1 WHERE id = $2"#,
            )
            .bind(investor_id)
            .bind(hid)
            .execute(scope.executor())
            .await?;
        }
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:subscribed",
            "investor/lifecycle",
            "investor.record-subscription — SUBSCRIBED",
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.activate ─────────────────────────────────────────────────────────

pub struct Activate;

#[async_trait]
impl SemOsVerbOp for Activate {
    fn fqn(&self) -> &str {
        "investor.activate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'ACTIVE_HOLDER',
                   lifecycle_notes = $2,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:active-holder",
            "investor/lifecycle",
            "investor.activate — ACTIVE_HOLDER",
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.start-redemption ─────────────────────────────────────────────────

pub struct StartRedemption;

#[async_trait]
impl SemOsVerbOp for StartRedemption {
    fn fqn(&self) -> &str {
        "investor.start-redemption"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let redemption_type = json_extract_string_opt(args, "redemption-type");
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'REDEEMING',
                   redemption_type = $2,
                   lifecycle_notes = $3,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&redemption_type)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:redeeming",
            "investor/lifecycle",
            "investor.start-redemption — REDEEMING",
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.complete-redemption ──────────────────────────────────────────────

pub struct CompleteRedemption;

#[async_trait]
impl SemOsVerbOp for CompleteRedemption {
    fn fqn(&self) -> &str {
        "investor.complete-redemption"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'OFFBOARDED',
                   offboarded_at = NOW(),
                   offboard_reason = 'Full redemption completed',
                   lifecycle_notes = $2,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        sqlx::query(
            r#"UPDATE "ob-poc".holdings SET status = 'closed', updated_at = NOW()
               WHERE investor_id = $1 AND status = 'active'"#,
        )
        .bind(investor_id)
        .execute(scope.executor())
        .await?;
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:offboarded",
            "investor/lifecycle",
            "investor.complete-redemption — OFFBOARDED (full redemption)",
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.offboard ─────────────────────────────────────────────────────────

pub struct Offboard;

#[async_trait]
impl SemOsVerbOp for Offboard {
    fn fqn(&self) -> &str {
        "investor.offboard"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let reason = json_extract_string(args, "reason")?;
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'OFFBOARDED',
                   offboard_reason = $2,
                   offboarded_at = NOW(),
                   lifecycle_notes = $3,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&reason)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        sqlx::query(
            r#"UPDATE "ob-poc".holdings SET status = 'closed', updated_at = NOW()
               WHERE investor_id = $1 AND status = 'active'"#,
        )
        .bind(investor_id)
        .execute(scope.executor())
        .await?;
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:offboarded",
            "investor/lifecycle",
            &format!("investor.offboard — OFFBOARDED ({})", reason),
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.suspend ──────────────────────────────────────────────────────────

pub struct Suspend;

#[async_trait]
impl SemOsVerbOp for Suspend {
    fn fqn(&self) -> &str {
        "investor.suspend"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let reason = json_extract_string(args, "reason")?;
        let notes = json_extract_string_opt(args, "notes");

        let current: String = sqlx::query_scalar(
            r#"SELECT lifecycle_state FROM "ob-poc".investors WHERE investor_id = $1"#,
        )
        .bind(investor_id)
        .fetch_one(scope.executor())
        .await
        .map_err(|e| anyhow!("Investor not found: {}", e))?;

        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = 'SUSPENDED',
                   suspended_reason = $2,
                   pre_suspension_state = $3,
                   suspended_at = NOW(),
                   lifecycle_notes = $4,
                   updated_at = NOW()
               WHERE investor_id = $1 RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&reason)
        .bind(&current)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            "investor:suspended",
            "investor/lifecycle",
            &format!("investor.suspend — {} → SUSPENDED ({})", current, reason),
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.reinstate ────────────────────────────────────────────────────────

pub struct Reinstate;

#[async_trait]
impl SemOsVerbOp for Reinstate {
    fn fqn(&self) -> &str {
        "investor.reinstate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let notes = json_extract_string_opt(args, "notes");

        let pre_state: Option<String> = sqlx::query_scalar(
            r#"SELECT pre_suspension_state FROM "ob-poc".investors WHERE investor_id = $1"#,
        )
        .bind(investor_id)
        .fetch_one(scope.executor())
        .await
        .map_err(|e| anyhow!("Investor not found: {}", e))?;
        let pre_state =
            pre_state.ok_or_else(|| anyhow!("No pre-suspension state found for investor"))?;

        let id: Uuid = sqlx::query_scalar(
            r#"UPDATE "ob-poc".investors
               SET lifecycle_state = $2,
                   suspended_reason = NULL,
                   pre_suspension_state = NULL,
                   suspended_at = NULL,
                   lifecycle_notes = $3,
                   updated_at = NOW()
               WHERE investor_id = $1 AND lifecycle_state = 'SUSPENDED'
               RETURNING investor_id"#,
        )
        .bind(investor_id)
        .bind(&pre_state)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;
        let to_node = format!("investor:{}", pre_state.to_lowercase().replace('_', "-"));
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            investor_id,
            &to_node,
            "investor/lifecycle",
            &format!("investor.reinstate — SUSPENDED → {}", pre_state),
        );
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor.count-by-state ───────────────────────────────────────────────────

pub struct CountByState;

#[async_trait]
impl SemOsVerbOp for CountByState {
    fn fqn(&self) -> &str {
        "investor.count-by-state"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT lifecycle_state, COUNT(*)::bigint
            FROM "ob-poc".investors
            WHERE owning_cbu_id = $1
            GROUP BY lifecycle_state
            ORDER BY lifecycle_state
            "#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await
        .map_err(|e| anyhow!("Failed to count investors: {}", e))?;

        let counts: HashMap<String, i64> = rows.into_iter().collect();
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(counts)?))
    }
}

