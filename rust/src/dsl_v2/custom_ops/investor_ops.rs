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

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract a required UUID argument from verb call
#[cfg(feature = "database")]
fn get_required_uuid(
    verb_call: &VerbCall,
    key: &str,
    ctx: &ExecutionContext,
) -> Result<uuid::Uuid> {
    use uuid::Uuid;

    let arg = verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))?;

    // Try as symbol reference first
    if let Some(ref_name) = arg.value.as_symbol() {
        let resolved = ctx
            .resolve(ref_name)
            .ok_or_else(|| anyhow::anyhow!("Unresolved reference @{}", ref_name))?;
        return Ok(resolved);
    }

    // Try as UUID directly
    if let Some(uuid_val) = arg.value.as_uuid() {
        return Ok(uuid_val);
    }

    // Try as string (may be UUID string)
    if let Some(str_val) = arg.value.as_string() {
        return Uuid::parse_str(str_val)
            .map_err(|e| anyhow::anyhow!("Invalid UUID for :{}: {}", key, e));
    }

    Err(anyhow::anyhow!(":{} must be a UUID or @reference", key))
}

/// Extract an optional string argument from verb call
#[cfg(feature = "database")]
fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

/// Extract a required string argument from verb call
#[cfg(feature = "database")]
fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    get_optional_string(verb_call, key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

/// Extract an optional UUID argument from verb call
#[cfg(feature = "database")]
fn get_optional_uuid(
    verb_call: &VerbCall,
    key: &str,
    ctx: &ExecutionContext,
) -> Option<uuid::Uuid> {
    get_required_uuid(verb_call, key, ctx).ok()
}

// ============================================================================
// Lifecycle Transition Operations
// ============================================================================

/// Request documents from an investor (ENQUIRY → PENDING_DOCUMENTS)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let notes = get_optional_string(verb_call, "notes");

        // Update lifecycle state (trigger handles validation and history logging)
        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
            SET lifecycle_state = 'PENDING_DOCUMENTS',
                lifecycle_notes = $2,
                updated_at = NOW()
            WHERE investor_id = $1
            RETURNING investor_id
            "#,
            investor_id,
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Start KYC process for an investor (PENDING_DOCUMENTS → KYC_IN_PROGRESS)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let case_id = get_optional_uuid(verb_call, "case-id", ctx);
        let notes = get_optional_string(verb_call, "notes");

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
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
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Approve KYC for an investor (KYC_IN_PROGRESS → KYC_APPROVED)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let risk_rating = get_optional_string(verb_call, "risk-rating");
        let notes = get_optional_string(verb_call, "notes");

        // Note: No kyc_approved_by column exists in schema - removed from query
        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
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
            risk_rating.as_deref(),
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Reject KYC for an investor (KYC_IN_PROGRESS → REJECTED)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let reason = get_required_string(verb_call, "reason")?;
        let notes = get_optional_string(verb_call, "notes");

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
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
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Mark investor as eligible to subscribe (KYC_APPROVED → ELIGIBLE_TO_SUBSCRIBE)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let investor_type = get_optional_string(verb_call, "investor-type");
        let notes = get_optional_string(verb_call, "notes");

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
            SET lifecycle_state = 'ELIGIBLE_TO_SUBSCRIBE',
                investor_type = COALESCE($2, investor_type),
                lifecycle_notes = $3,
                updated_at = NOW()
            WHERE investor_id = $1
            RETURNING investor_id
            "#,
            investor_id,
            investor_type.as_deref(),
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Record subscription for investor (ELIGIBLE_TO_SUBSCRIBE → SUBSCRIBED)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let holding_id = get_optional_uuid(verb_call, "holding-id", ctx);
        let notes = get_optional_string(verb_call, "notes");

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
            SET lifecycle_state = 'SUBSCRIBED',
                first_subscription_at = COALESCE(first_subscription_at, NOW()),
                lifecycle_notes = $2,
                updated_at = NOW()
            WHERE investor_id = $1
            RETURNING investor_id
            "#,
            investor_id,
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        // Link holding if provided
        if let Some(hid) = holding_id {
            sqlx::query!(
                r#"
                UPDATE kyc.holdings
                SET investor_id = $1
                WHERE id = $2
                "#,
                investor_id,
                hid
            )
            .execute(pool)
            .await?;
        }

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Activate investor as holder (SUBSCRIBED → ACTIVE_HOLDER)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let notes = get_optional_string(verb_call, "notes");

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
            SET lifecycle_state = 'ACTIVE_HOLDER',
                lifecycle_notes = $2,
                updated_at = NOW()
            WHERE investor_id = $1
            RETURNING investor_id
            "#,
            investor_id,
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Start redemption process (ACTIVE_HOLDER → REDEEMING)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let redemption_type = get_optional_string(verb_call, "redemption-type");
        let notes = get_optional_string(verb_call, "notes");

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
            SET lifecycle_state = 'REDEEMING',
                redemption_type = $2,
                lifecycle_notes = $3,
                updated_at = NOW()
            WHERE investor_id = $1
            RETURNING investor_id
            "#,
            investor_id,
            redemption_type.as_deref(),
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Complete redemption and offboard (REDEEMING → OFFBOARDED)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let notes = get_optional_string(verb_call, "notes");

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
            SET lifecycle_state = 'OFFBOARDED',
                offboarded_at = NOW(),
                offboard_reason = 'Full redemption completed',
                lifecycle_notes = $2,
                updated_at = NOW()
            WHERE investor_id = $1
            RETURNING investor_id
            "#,
            investor_id,
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        // Close any associated holdings
        sqlx::query!(
            r#"
            UPDATE kyc.holdings
            SET status = 'closed',
                updated_at = NOW()
            WHERE investor_id = $1 AND status = 'active'
            "#,
            investor_id
        )
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Offboard investor directly (any active state → OFFBOARDED)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let reason = get_required_string(verb_call, "reason")?;
        let notes = get_optional_string(verb_call, "notes");

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
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
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update investor: {}", e))?;

        // Close any associated holdings
        sqlx::query!(
            r#"
            UPDATE kyc.holdings
            SET status = 'closed',
                updated_at = NOW()
            WHERE investor_id = $1 AND status = 'active'
            "#,
            investor_id
        )
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Suspend an investor (any active state → SUSPENDED)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let reason = get_required_string(verb_call, "reason")?;
        let notes = get_optional_string(verb_call, "notes");

        // Get current state to store for reinstatement
        let current = sqlx::query_scalar!(
            r#"SELECT lifecycle_state FROM kyc.investors WHERE investor_id = $1"#,
            investor_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Investor not found: {}", e))?;

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
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
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to suspend investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

/// Reinstate a suspended investor (SUSPENDED → previous state)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let investor_id = get_required_uuid(verb_call, "investor-id", ctx)?;
        let notes = get_optional_string(verb_call, "notes");

        // Get the pre-suspension state
        let pre_state = sqlx::query_scalar!(
            r#"SELECT pre_suspension_state FROM kyc.investors WHERE investor_id = $1"#,
            investor_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Investor not found: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No pre-suspension state found for investor"))?;

        let row = sqlx::query!(
            r#"
            UPDATE kyc.investors
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
            notes.as_deref()
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to reinstate investor: {}", e))?;

        Ok(ExecutionResult::Uuid(row.investor_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

// ============================================================================
// Query Operations
// ============================================================================

/// Count investors by lifecycle state for a CBU
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id", ctx)?;

        let rows = sqlx::query!(
            r#"
            SELECT lifecycle_state, COUNT(*) as count
            FROM kyc.investors
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

        Ok(ExecutionResult::Record(serde_json::to_value(counts)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for investor operations"
        ))
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register all investor operations with the registry
pub fn register_investor_ops(registry: &mut crate::dsl_v2::custom_ops::CustomOperationRegistry) {
    use std::sync::Arc;

    registry.register(Arc::new(InvestorRequestDocumentsOp));
    registry.register(Arc::new(InvestorStartKycOp));
    registry.register(Arc::new(InvestorApproveKycOp));
    registry.register(Arc::new(InvestorRejectKycOp));
    registry.register(Arc::new(InvestorMarkEligibleOp));
    registry.register(Arc::new(InvestorRecordSubscriptionOp));
    registry.register(Arc::new(InvestorActivateOp));
    registry.register(Arc::new(InvestorStartRedemptionOp));
    registry.register(Arc::new(InvestorCompleteRedemptionOp));
    registry.register(Arc::new(InvestorOffboardOp));
    registry.register(Arc::new(InvestorSuspendOp));
    registry.register(Arc::new(InvestorReinstateOp));
    registry.register(Arc::new(InvestorCountByStateOp));
}
