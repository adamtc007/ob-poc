//! Investor lifecycle plugin operations for TA KYC-as-a-Service.
//!
//! This module implements the investor lifecycle state machine transitions:
//! ENQUIRY → PENDING_DOCUMENTS → KYC_IN_PROGRESS → KYC_APPROVED → ELIGIBLE_TO_SUBSCRIBE
//! → SUBSCRIBED → ACTIVE_HOLDER → REDEEMING → OFFBOARDED
//!
//! State transitions are validated by the `trg_validate_investor_lifecycle` database trigger.
//! History is automatically logged by the `trg_log_investor_lifecycle` trigger.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

// ── Shared _impl functions ──────────────────────────────────────────────

async fn investor_request_documents_impl(
    investor_id: uuid::Uuid,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'PENDING_DOCUMENTS',
            lifecycle_notes = $2,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;
    Ok(row.investor_id)
}

async fn investor_start_kyc_impl(
    investor_id: uuid::Uuid,
    case_id: Option<uuid::Uuid>,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'KYC_IN_PROGRESS',
            kyc_status = 'IN_PROGRESS',
            kyc_case_id = COALESCE($2, kyc_case_id),
            lifecycle_notes = $3,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        case_id,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;
    Ok(row.investor_id)
}

async fn investor_approve_kyc_impl(
    investor_id: uuid::Uuid,
    risk_rating: Option<&str>,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'KYC_APPROVED',
            kyc_status = 'APPROVED',
            kyc_approved_at = NOW(),
            kyc_risk_rating = COALESCE($2, kyc_risk_rating),
            lifecycle_notes = $3,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        risk_rating,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;
    Ok(row.investor_id)
}

async fn investor_reject_kyc_impl(
    investor_id: uuid::Uuid,
    reason: &str,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'REJECTED',
            kyc_status = 'REJECTED',
            rejection_reason = $2,
            lifecycle_notes = $3,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        reason,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;
    Ok(row.investor_id)
}

async fn investor_mark_eligible_impl(
    investor_id: uuid::Uuid,
    investor_type: Option<&str>,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'ELIGIBLE_TO_SUBSCRIBE',
            investor_type = COALESCE($2, investor_type),
            lifecycle_notes = $3,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        investor_type,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;
    Ok(row.investor_id)
}

async fn investor_record_subscription_impl(
    investor_id: uuid::Uuid,
    holding_id: Option<uuid::Uuid>,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'SUBSCRIBED',
            first_subscription_at = COALESCE(first_subscription_at, NOW()),
            lifecycle_notes = $2,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

    if let Some(hid) = holding_id {
        sqlx::query!(
            r#"UPDATE "ob-poc".holdings SET investor_id = $1 WHERE id = $2"#,
            investor_id,
            hid
        )
        .execute(pool)
        .await?;
    }

    Ok(row.investor_id)
}

async fn investor_activate_impl(
    investor_id: uuid::Uuid,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'ACTIVE_HOLDER',
            lifecycle_notes = $2,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;
    Ok(row.investor_id)
}

async fn investor_start_redemption_impl(
    investor_id: uuid::Uuid,
    redemption_type: Option<&str>,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'REDEEMING',
            redemption_type = $2,
            lifecycle_notes = $3,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        redemption_type,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;
    Ok(row.investor_id)
}

async fn investor_complete_redemption_impl(
    investor_id: uuid::Uuid,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'OFFBOARDED',
            offboarded_at = NOW(),
            offboard_reason = 'Full redemption completed',
            lifecycle_notes = $2,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

    sqlx::query!(
        r#"UPDATE "ob-poc".holdings SET status = 'closed', updated_at = NOW() WHERE investor_id = $1 AND status = 'active'"#,
        investor_id
    )
    .execute(pool)
    .await?;

    Ok(row.investor_id)
}

async fn investor_offboard_impl(
    investor_id: uuid::Uuid,
    reason: &str,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'OFFBOARDED',
            offboard_reason = $2,
            offboarded_at = NOW(),
            lifecycle_notes = $3,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        reason,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

    sqlx::query!(
        r#"UPDATE "ob-poc".holdings SET status = 'closed', updated_at = NOW() WHERE investor_id = $1 AND status = 'active'"#,
        investor_id
    )
    .execute(pool)
    .await?;

    Ok(row.investor_id)
}

async fn investor_suspend_impl(
    investor_id: uuid::Uuid,
    reason: &str,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let current = sqlx::query_scalar!(
        r#"SELECT lifecycle_state FROM "ob-poc".investors WHERE investor_id = $1"#,
        investor_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Investor not found: {}", e))?;

    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = 'SUSPENDED',
            suspended_reason = $2,
            pre_suspension_state = $3,
            suspended_at = NOW(),
            lifecycle_notes = $4,
            updated_at = NOW()
        WHERE investor_id = $1
        RETURNING investor_id
        "#,
        investor_id,
        reason,
        current,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to suspend investor: {}", e))?;

    Ok(row.investor_id)
}

async fn investor_reinstate_impl(
    investor_id: uuid::Uuid,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let pre_state = sqlx::query_scalar!(
        r#"SELECT pre_suspension_state FROM "ob-poc".investors WHERE investor_id = $1"#,
        investor_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Investor not found: {}", e))?
    .ok_or_else(|| anyhow::anyhow!("No pre-suspension state found for investor"))?;

    let row = sqlx::query!(
        r#"
        UPDATE "ob-poc".investors
        SET lifecycle_state = $2,
            suspended_reason = NULL,
            pre_suspension_state = NULL,
            suspended_at = NULL,
            lifecycle_notes = $3,
            updated_at = NOW()
        WHERE investor_id = $1 AND lifecycle_state = 'SUSPENDED'
        RETURNING investor_id
        "#,
        investor_id,
        pre_state,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to reinstate investor: {}", e))?;

    Ok(row.investor_id)
}

async fn investor_count_by_state_impl(
    cbu_id: uuid::Uuid,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let rows = sqlx::query!(
        r#"
        SELECT lifecycle_state, COUNT(*) as count
        FROM "ob-poc".investors
        WHERE owning_cbu_id = $1
        GROUP BY lifecycle_state
        ORDER BY lifecycle_state
        "#,
        cbu_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to count investors: {}", e))?;

    let counts: std::collections::HashMap<String, i64> = rows
        .into_iter()
        .map(|r| (r.lifecycle_state, r.count.unwrap_or(0)))
        .collect();

    serde_json::to_value(counts).map_err(|e| anyhow::anyhow!("{}", e))
}

// ============================================================================
// Lifecycle Transition Operations
// ============================================================================

/// Request documents from an investor (ENQUIRY → PENDING_DOCUMENTS)
#[register_custom_op]
pub struct InvestorRequestDocumentsOp;

#[async_trait]
impl CustomOperation for InvestorRequestDocumentsOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "request-documents"
    }
    fn rationale(&self) -> &'static str {
        "Transitions investor from ENQUIRY to PENDING_DOCUMENTS state"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_request_documents_impl(investor_id, notes.as_deref(), pool).await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Start KYC process for an investor (PENDING_DOCUMENTS → KYC_IN_PROGRESS)
#[register_custom_op]
pub struct InvestorStartKycOp;

#[async_trait]
impl CustomOperation for InvestorStartKycOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "start-kyc"
    }
    fn rationale(&self) -> &'static str {
        "Transitions investor from PENDING_DOCUMENTS to KYC_IN_PROGRESS"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_start_kyc_impl(investor_id, case_id, notes.as_deref(), pool).await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Approve KYC for an investor (KYC_IN_PROGRESS → KYC_APPROVED)
#[register_custom_op]
pub struct InvestorApproveKycOp;

#[async_trait]
impl CustomOperation for InvestorApproveKycOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "approve-kyc"
    }
    fn rationale(&self) -> &'static str {
        "Transitions investor from KYC_IN_PROGRESS to KYC_APPROVED"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let risk_rating = json_extract_string_opt(args, "risk-rating");
        let notes = json_extract_string_opt(args, "notes");
        let id =
            investor_approve_kyc_impl(investor_id, risk_rating.as_deref(), notes.as_deref(), pool)
                .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Reject KYC for an investor (KYC_IN_PROGRESS → REJECTED)
#[register_custom_op]
pub struct InvestorRejectKycOp;

#[async_trait]
impl CustomOperation for InvestorRejectKycOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "reject-kyc"
    }
    fn rationale(&self) -> &'static str {
        "Transitions investor from KYC_IN_PROGRESS to REJECTED state"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let reason = json_extract_string(args, "reason")?;
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_reject_kyc_impl(investor_id, &reason, notes.as_deref(), pool).await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Mark investor as eligible to subscribe (KYC_APPROVED → ELIGIBLE_TO_SUBSCRIBE)
#[register_custom_op]
pub struct InvestorMarkEligibleOp;

#[async_trait]
impl CustomOperation for InvestorMarkEligibleOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "mark-eligible"
    }
    fn rationale(&self) -> &'static str {
        "Transitions investor from KYC_APPROVED to ELIGIBLE_TO_SUBSCRIBE"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let investor_type = json_extract_string_opt(args, "investor-type");
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_mark_eligible_impl(
            investor_id,
            investor_type.as_deref(),
            notes.as_deref(),
            pool,
        )
        .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Record subscription for investor (ELIGIBLE_TO_SUBSCRIBE → SUBSCRIBED)
#[register_custom_op]
pub struct InvestorRecordSubscriptionOp;

#[async_trait]
impl CustomOperation for InvestorRecordSubscriptionOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "record-subscription"
    }
    fn rationale(&self) -> &'static str {
        "Records initial subscription, transitions to SUBSCRIBED"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let holding_id = json_extract_uuid_opt(args, ctx, "holding-id");
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_record_subscription_impl(investor_id, holding_id, notes.as_deref(), pool)
            .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Activate investor as holder (SUBSCRIBED → ACTIVE_HOLDER)
#[register_custom_op]
pub struct InvestorActivateOp;

#[async_trait]
impl CustomOperation for InvestorActivateOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "activate"
    }
    fn rationale(&self) -> &'static str {
        "Transitions investor from SUBSCRIBED to ACTIVE_HOLDER"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_activate_impl(investor_id, notes.as_deref(), pool).await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Start redemption process (ACTIVE_HOLDER → REDEEMING)
#[register_custom_op]
pub struct InvestorStartRedemptionOp;

#[async_trait]
impl CustomOperation for InvestorStartRedemptionOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "start-redemption"
    }
    fn rationale(&self) -> &'static str {
        "Transitions investor from ACTIVE_HOLDER to REDEEMING"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let redemption_type = json_extract_string_opt(args, "redemption-type");
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_start_redemption_impl(
            investor_id,
            redemption_type.as_deref(),
            notes.as_deref(),
            pool,
        )
        .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Complete redemption and offboard (REDEEMING → OFFBOARDED)
#[register_custom_op]
pub struct InvestorCompleteRedemptionOp;

#[async_trait]
impl CustomOperation for InvestorCompleteRedemptionOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "complete-redemption"
    }
    fn rationale(&self) -> &'static str {
        "Completes full redemption, transitions to OFFBOARDED"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_complete_redemption_impl(investor_id, notes.as_deref(), pool).await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Offboard investor directly (any active state → OFFBOARDED)
#[register_custom_op]
pub struct InvestorOffboardOp;

#[async_trait]
impl CustomOperation for InvestorOffboardOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "offboard"
    }
    fn rationale(&self) -> &'static str {
        "Immediately offboards an investor with reason"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let reason = json_extract_string(args, "reason")?;
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_offboard_impl(investor_id, &reason, notes.as_deref(), pool).await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Suspend an investor (any active state → SUSPENDED)
#[register_custom_op]
pub struct InvestorSuspendOp;

#[async_trait]
impl CustomOperation for InvestorSuspendOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "suspend"
    }
    fn rationale(&self) -> &'static str {
        "Suspends an investor, blocking transactions until reinstated"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let reason = json_extract_string(args, "reason")?;
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_suspend_impl(investor_id, &reason, notes.as_deref(), pool).await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Reinstate a suspended investor (SUSPENDED → previous state)
#[register_custom_op]
pub struct InvestorReinstateOp;

#[async_trait]
impl CustomOperation for InvestorReinstateOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "reinstate"
    }
    fn rationale(&self) -> &'static str {
        "Reinstates a suspended investor to their previous state"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let investor_id = json_extract_uuid(args, ctx, "investor-id")?;
        let notes = json_extract_string_opt(args, "notes");
        let id = investor_reinstate_impl(investor_id, notes.as_deref(), pool).await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// Query Operations
// ============================================================================

/// Count investors by lifecycle state for a CBU
#[register_custom_op]
pub struct InvestorCountByStateOp;

#[async_trait]
impl CustomOperation for InvestorCountByStateOp {
    fn domain(&self) -> &'static str {
        "investor"
    }
    fn verb(&self) -> &'static str {
        "count-by-state"
    }
    fn rationale(&self) -> &'static str {
        "Returns investor counts by lifecycle state for reporting"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let result = investor_count_by_state_impl(cbu_id, pool).await?;
        Ok(VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
