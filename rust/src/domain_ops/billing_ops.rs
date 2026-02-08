//! Fee Billing Operations
//!
//! Operations for billing profile lifecycle, account targeting, and fee calculation.
//!
//! The fee billing system bridges commercial deals to operational billing cycles,
//! implementing a closed loop from rate cards through to invoicing.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::helpers::{
    extract_bool_opt, extract_string, extract_string_opt, extract_uuid, extract_uuid_opt,
};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// Result Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingCalculationResult {
    pub period_id: Uuid,
    pub line_count: i32,
    pub gross_amount: f64,
    pub status: String,
}

// =============================================================================
// Billing Profile Operations
// =============================================================================

/// Create a fee billing profile
#[register_custom_op]
pub struct BillingCreateProfileOp;

#[async_trait]
impl CustomOperation for BillingCreateProfileOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "create-profile"
    }
    fn rationale(&self) -> &'static str {
        "Validates rate card is AGREED and creates billing profile in PENDING status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let contract_id = extract_uuid(verb_call, ctx, "contract-id")?;
        let rate_card_id = extract_uuid(verb_call, ctx, "rate-card-id")?;
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let product_id = extract_uuid(verb_call, ctx, "product-id")?;
        let invoice_entity_id = extract_uuid(verb_call, ctx, "invoice-entity-id")?;
        let profile_name = extract_string_opt(verb_call, "profile-name");
        let billing_frequency = extract_string_opt(verb_call, "billing-frequency")
            .unwrap_or_else(|| "MONTHLY".to_string());
        let invoice_currency =
            extract_string_opt(verb_call, "invoice-currency").unwrap_or_else(|| "USD".to_string());
        let payment_method = extract_string_opt(verb_call, "payment-method");
        let payment_account_ref = extract_string_opt(verb_call, "payment-account-ref");
        let effective_from = extract_string(verb_call, "effective-from")?;

        // Validate rate card is AGREED
        let rate_card_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(pool)
        .await?;

        if rate_card_status != "AGREED" {
            return Err(anyhow!(
                "Rate card must be in AGREED status to create billing profile"
            ));
        }

        // Create billing profile
        let profile_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".fee_billing_profiles (
                deal_id, contract_id, rate_card_id, cbu_id, product_id,
                profile_name, billing_frequency, invoice_entity_id,
                invoice_currency, payment_method, payment_account_ref, effective_from
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::date)
            RETURNING profile_id
            "#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(rate_card_id)
        .bind(cbu_id)
        .bind(product_id)
        .bind(&profile_name)
        .bind(&billing_frequency)
        .bind(invoice_entity_id)
        .bind(&invoice_currency)
        .bind(&payment_method)
        .bind(&payment_account_ref)
        .bind(&effective_from)
        .fetch_one(pool)
        .await?;

        // Record deal event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value)
            VALUES ($1, 'BILLING_PROFILE_CREATED', 'BILLING_PROFILE', $2, 'PENDING')
            "#,
        )
        .bind(deal_id)
        .bind(profile_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(profile_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Activate a billing profile
#[register_custom_op]
pub struct BillingActivateProfileOp;

#[async_trait]
impl CustomOperation for BillingActivateProfileOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "activate-profile"
    }
    fn rationale(&self) -> &'static str {
        "Validates at least one account target exists before activating"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id = extract_uuid(verb_call, ctx, "profile-id")?;

        // Validate at least one account target exists
        let target_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".fee_billing_account_targets WHERE profile_id = $1 AND is_active = true"#,
        )
        .bind(profile_id)
        .fetch_one(pool)
        .await?;

        if target_count == 0 {
            return Err(anyhow!(
                "Cannot activate billing profile without account targets"
            ));
        }

        // Get deal_id for event
        let deal_id: Uuid = sqlx::query_scalar(
            r#"SELECT deal_id FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(pool)
        .await?;

        // Activate
        sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_profiles SET status = 'ACTIVE', updated_at = NOW() WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .execute(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value)
            VALUES ($1, 'BILLING_ACTIVATED', 'BILLING_PROFILE', $2, 'ACTIVE')
            "#,
        )
        .bind(deal_id)
        .bind(profile_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Suspend a billing profile
#[register_custom_op]
pub struct BillingSuspendProfileOp;

#[async_trait]
impl CustomOperation for BillingSuspendProfileOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "suspend-profile"
    }
    fn rationale(&self) -> &'static str {
        "Suspends billing and records reason"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id = extract_uuid(verb_call, ctx, "profile-id")?;
        let reason = extract_string(verb_call, "reason")?;

        // Get deal_id for event
        let deal_id: Uuid = sqlx::query_scalar(
            r#"SELECT deal_id FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(pool)
        .await?;

        // Suspend
        sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_profiles SET status = 'SUSPENDED', updated_at = NOW() WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .execute(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value, description)
            VALUES ($1, 'BILLING_SUSPENDED', 'BILLING_PROFILE', $2, 'SUSPENDED', $3)
            "#,
        )
        .bind(deal_id)
        .bind(profile_id)
        .bind(&reason)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Close a billing profile
#[register_custom_op]
pub struct BillingCloseProfileOp;

#[async_trait]
impl CustomOperation for BillingCloseProfileOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "close-profile"
    }
    fn rationale(&self) -> &'static str {
        "Sets effective_to date and closes profile"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id = extract_uuid(verb_call, ctx, "profile-id")?;
        let effective_to = extract_string(verb_call, "effective-to")?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_profiles
            SET status = 'CLOSED', effective_to = $2::date, updated_at = NOW()
            WHERE profile_id = $1
            "#,
        )
        .bind(profile_id)
        .bind(&effective_to)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// Account Target Operations
// =============================================================================

/// Add an account target to a billing profile
#[register_custom_op]
pub struct BillingAddAccountTargetOp;

#[async_trait]
impl CustomOperation for BillingAddAccountTargetOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "add-account-target"
    }
    fn rationale(&self) -> &'static str {
        "Validates resource instance belongs to profile's CBU before adding target"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id = extract_uuid(verb_call, ctx, "profile-id")?;
        let cbu_resource_instance_id = extract_uuid(verb_call, ctx, "cbu-resource-instance-id")?;
        let rate_card_line_id = extract_uuid_opt(verb_call, ctx, "rate-card-line-id");
        let resource_type = extract_string_opt(verb_call, "resource-type");
        let resource_ref = extract_string_opt(verb_call, "resource-ref");
        let activity_type = extract_string_opt(verb_call, "activity-type");
        let has_override = extract_bool_opt(verb_call, "has-override").unwrap_or(false);
        let override_rate: Option<f64> = verb_call
            .get_arg("override-rate")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let override_model = extract_string_opt(verb_call, "override-model");

        // Validate resource instance belongs to profile's CBU
        let profile_cbu_id: Uuid = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(pool)
        .await?;

        let instance_cbu_id: Uuid = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbu_resource_instances WHERE instance_id = $1"#,
        )
        .bind(cbu_resource_instance_id)
        .fetch_one(pool)
        .await?;

        if profile_cbu_id != instance_cbu_id {
            return Err(anyhow!(
                "Resource instance does not belong to the profile's CBU"
            ));
        }

        // Insert target
        let target_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".fee_billing_account_targets (
                profile_id, cbu_resource_instance_id, rate_card_line_id,
                resource_type, resource_ref, activity_type,
                has_override, override_rate, override_model
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING target_id
            "#,
        )
        .bind(profile_id)
        .bind(cbu_resource_instance_id)
        .bind(rate_card_line_id)
        .bind(&resource_type)
        .bind(&resource_ref)
        .bind(&activity_type)
        .bind(has_override)
        .bind(override_rate)
        .bind(&override_model)
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(target_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Remove an account target (soft delete)
#[register_custom_op]
pub struct BillingRemoveAccountTargetOp;

#[async_trait]
impl CustomOperation for BillingRemoveAccountTargetOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "remove-account-target"
    }
    fn rationale(&self) -> &'static str {
        "Soft-removes account target by setting is_active = false"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let target_id = extract_uuid(verb_call, ctx, "target-id")?;

        let result = sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_account_targets SET is_active = false, updated_at = NOW() WHERE target_id = $1"#,
        )
        .bind(target_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// Billing Period Operations
// =============================================================================

/// Create a billing period
#[register_custom_op]
pub struct BillingCreatePeriodOp;

#[async_trait]
impl CustomOperation for BillingCreatePeriodOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "create-period"
    }
    fn rationale(&self) -> &'static str {
        "Validates no overlapping period and profile is ACTIVE"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id = extract_uuid(verb_call, ctx, "profile-id")?;
        let period_start = extract_string(verb_call, "period-start")?;
        let period_end = extract_string(verb_call, "period-end")?;

        // Validate profile is ACTIVE
        let profile_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(pool)
        .await?;

        if profile_status != "ACTIVE" {
            return Err(anyhow!("Billing profile must be ACTIVE to create periods"));
        }

        // Check for overlapping periods
        let overlap_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM "ob-poc".fee_billing_periods
            WHERE profile_id = $1
            AND (period_start, period_end) OVERLAPS ($2::date, $3::date)
            "#,
        )
        .bind(profile_id)
        .bind(&period_start)
        .bind(&period_end)
        .fetch_one(pool)
        .await?;

        if overlap_count > 0 {
            return Err(anyhow!("Billing period overlaps with existing period"));
        }

        // Get currency from profile
        let currency_code: String = sqlx::query_scalar(
            r#"SELECT invoice_currency FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(pool)
        .await?;

        // Create period
        let period_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".fee_billing_periods (profile_id, period_start, period_end, currency_code)
            VALUES ($1, $2::date, $3::date, $4)
            RETURNING period_id
            "#,
        )
        .bind(profile_id)
        .bind(&period_start)
        .bind(&period_end)
        .bind(&currency_code)
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(period_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Calculate fees for a billing period
#[register_custom_op]
pub struct BillingCalculatePeriodOp;

#[async_trait]
impl CustomOperation for BillingCalculatePeriodOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "calculate-period"
    }
    fn rationale(&self) -> &'static str {
        "Iterates account targets, applies pricing models, and generates period lines"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let period_id = extract_uuid(verb_call, ctx, "period-id")?;

        // Update status to CALCULATING
        sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_periods SET calc_status = 'CALCULATING' WHERE period_id = $1"#,
        )
        .bind(period_id)
        .execute(pool)
        .await?;

        // Get period details
        let period_row: (Uuid, String, String) = sqlx::query_as(
            r#"SELECT profile_id, period_start::text, period_end::text FROM "ob-poc".fee_billing_periods WHERE period_id = $1"#,
        )
        .bind(period_id)
        .fetch_one(pool)
        .await?;

        let (profile_id, _period_start, _period_end) = period_row;

        // Get active account targets with rate card lines
        let targets: Vec<(
            Uuid,
            Option<Uuid>,
            Option<String>,
            Option<f64>,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT t.target_id, t.rate_card_line_id,
                   COALESCE(t.activity_type, l.fee_basis) as activity_type,
                   CASE WHEN t.has_override THEN t.override_rate ELSE l.rate_value END as rate,
                   COALESCE(t.override_model, l.pricing_model) as pricing_model
            FROM "ob-poc".fee_billing_account_targets t
            LEFT JOIN "ob-poc".deal_rate_card_lines l ON t.rate_card_line_id = l.line_id
            WHERE t.profile_id = $1 AND t.is_active = true
            "#,
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await?;

        let mut line_count = 0;
        let mut gross_amount = 0.0f64;

        for (target_id, rate_card_line_id, _activity_type, rate, pricing_model) in targets {
            // For now, use a placeholder activity volume
            // In production, this would query actual activity from resource instances
            let activity_volume = 1000000.0f64; // Placeholder: $1M AUM
            let activity_unit = "USD_AUM".to_string();

            let calculated_fee = match pricing_model.as_deref() {
                Some("BPS") => {
                    // rate is in basis points (e.g., 1.5 = 1.5 bps = 0.00015)
                    let rate_decimal = rate.unwrap_or(0.0) / 10000.0;
                    activity_volume * rate_decimal
                }
                Some("FLAT") => rate.unwrap_or(0.0),
                Some("PER_TRANSACTION") => {
                    // Placeholder: assume 100 transactions
                    rate.unwrap_or(0.0) * 100.0
                }
                _ => 0.0,
            };

            // Insert period line
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".fee_billing_period_lines (
                    period_id, target_id, rate_card_line_id,
                    activity_volume, activity_unit, applied_rate,
                    calculated_fee, adjustment, net_fee
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, 0, $7)
                "#,
            )
            .bind(period_id)
            .bind(target_id)
            .bind(rate_card_line_id)
            .bind(activity_volume)
            .bind(&activity_unit)
            .bind(rate)
            .bind(calculated_fee)
            .execute(pool)
            .await?;

            line_count += 1;
            gross_amount += calculated_fee;
        }

        // Update period totals
        sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_periods
            SET gross_amount = $2, net_amount = $2, calc_status = 'CALCULATED', calculated_at = NOW(), updated_at = NOW()
            WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .bind(gross_amount)
        .execute(pool)
        .await?;

        let result = BillingCalculationResult {
            period_id,
            line_count,
            gross_amount,
            status: "CALCULATED".to_string(),
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "period_id": Uuid::new_v4(),
            "line_count": 0,
            "gross_amount": 0.0,
            "status": "CALCULATED"
        })))
    }
}

/// Review a billing period
#[register_custom_op]
pub struct BillingReviewPeriodOp;

#[async_trait]
impl CustomOperation for BillingReviewPeriodOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "review-period"
    }
    fn rationale(&self) -> &'static str {
        "Applies adjustments and marks period as REVIEWED"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let period_id = extract_uuid(verb_call, ctx, "period-id")?;
        let reviewed_by = extract_string(verb_call, "reviewed-by")?;
        // adjustments would be JSON array - for now we skip applying them

        // Update status
        sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_periods
            SET calc_status = 'REVIEWED', reviewed_by = $2, updated_at = NOW()
            WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .bind(&reviewed_by)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Approve a billing period
#[register_custom_op]
pub struct BillingApprovePeriodOp;

#[async_trait]
impl CustomOperation for BillingApprovePeriodOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "approve-period"
    }
    fn rationale(&self) -> &'static str {
        "Marks period as APPROVED for invoicing"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let period_id = extract_uuid(verb_call, ctx, "period-id")?;
        let approved_by = extract_string(verb_call, "approved-by")?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_periods
            SET calc_status = 'APPROVED', approved_by = $2, approved_at = NOW(), updated_at = NOW()
            WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .bind(&approved_by)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Generate invoice from approved period
#[register_custom_op]
pub struct BillingGenerateInvoiceOp;

#[async_trait]
impl CustomOperation for BillingGenerateInvoiceOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "generate-invoice"
    }
    fn rationale(&self) -> &'static str {
        "Validates period is APPROVED and generates invoice record"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let period_id = extract_uuid(verb_call, ctx, "period-id")?;

        // Validate status is APPROVED
        let status: String = sqlx::query_scalar(
            r#"SELECT calc_status FROM "ob-poc".fee_billing_periods WHERE period_id = $1"#,
        )
        .bind(period_id)
        .fetch_one(pool)
        .await?;

        if status != "APPROVED" {
            return Err(anyhow!("Period must be APPROVED to generate invoice"));
        }

        // Generate invoice ID (in production, this would create an actual invoice record)
        let invoice_id = Uuid::new_v4();

        // Update period
        sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_periods
            SET calc_status = 'INVOICED', invoice_id = $2, invoiced_at = NOW(), updated_at = NOW()
            WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .bind(invoice_id)
        .execute(pool)
        .await?;

        // Get deal_id for event
        let deal_id: Uuid = sqlx::query_scalar(
            r#"
            SELECT p.deal_id
            FROM "ob-poc".fee_billing_profiles p
            JOIN "ob-poc".fee_billing_periods bp ON p.profile_id = bp.profile_id
            WHERE bp.period_id = $1
            "#,
        )
        .bind(period_id)
        .fetch_one(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'INVOICE_GENERATED', 'BILLING_PERIOD', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(period_id)
        .bind(invoice_id.to_string())
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(invoice_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Dispute a billing period
#[register_custom_op]
pub struct BillingDisputePeriodOp;

#[async_trait]
impl CustomOperation for BillingDisputePeriodOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "dispute-period"
    }
    fn rationale(&self) -> &'static str {
        "Marks period as DISPUTED and records dispute details"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let period_id = extract_uuid(verb_call, ctx, "period-id")?;
        let dispute_reason = extract_string(verb_call, "dispute-reason")?;
        // disputed_lines would be JSON array - for now we just mark the period

        sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_periods SET calc_status = 'DISPUTED', updated_at = NOW() WHERE period_id = $1"#,
        )
        .bind(period_id)
        .execute(pool)
        .await?;

        // Get deal_id for event
        let deal_id: Uuid = sqlx::query_scalar(
            r#"
            SELECT p.deal_id
            FROM "ob-poc".fee_billing_profiles p
            JOIN "ob-poc".fee_billing_periods bp ON p.profile_id = bp.profile_id
            WHERE bp.period_id = $1
            "#,
        )
        .bind(period_id)
        .fetch_one(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'BILLING_DISPUTED', 'BILLING_PERIOD', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(period_id)
        .bind(&dispute_reason)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Get billing period summary
#[register_custom_op]
pub struct BillingPeriodSummaryOp;

#[async_trait]
impl CustomOperation for BillingPeriodSummaryOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "period-summary"
    }
    fn rationale(&self) -> &'static str {
        "Returns period with line-level detail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let period_id = extract_uuid(verb_call, ctx, "period-id")?;

        // Get period
        let period: (Uuid, String, String, String, Option<f64>, Option<f64>) = sqlx::query_as(
            r#"
            SELECT period_id, period_start::text, period_end::text, calc_status, gross_amount::float8, net_amount::float8
            FROM "ob-poc".fee_billing_periods WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .fetch_one(pool)
        .await?;

        // Get lines
        let lines: Vec<(Uuid, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
            r#"
            SELECT period_line_id, activity_volume::float8, calculated_fee::float8, net_fee::float8
            FROM "ob-poc".fee_billing_period_lines WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .fetch_all(pool)
        .await?;

        let line_values: Vec<serde_json::Value> = lines
            .into_iter()
            .map(|(line_id, volume, calc_fee, net_fee)| {
                serde_json::json!({
                    "period_line_id": line_id,
                    "activity_volume": volume,
                    "calculated_fee": calc_fee,
                    "net_fee": net_fee
                })
            })
            .collect();

        let result = serde_json::json!({
            "period_id": period.0,
            "period_start": period.1,
            "period_end": period.2,
            "calc_status": period.3,
            "gross_amount": period.4,
            "net_amount": period.5,
            "lines": line_values
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({})))
    }
}

/// Revenue summary across deals/periods
#[register_custom_op]
pub struct BillingRevenueSummaryOp;

#[async_trait]
impl CustomOperation for BillingRevenueSummaryOp {
    fn domain(&self) -> &'static str {
        "billing"
    }
    fn verb(&self) -> &'static str {
        "revenue-summary"
    }
    fn rationale(&self) -> &'static str {
        "Aggregates revenue across deals, periods, and products"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid_opt(verb_call, ctx, "deal-id");
        let cbu_id = extract_uuid_opt(verb_call, ctx, "cbu-id");
        let from_date = extract_string_opt(verb_call, "from-date");
        let to_date = extract_string_opt(verb_call, "to-date");

        // Build dynamic query based on filters
        let mut conditions = Vec::new();
        let mut bind_idx = 1;

        if deal_id.is_some() {
            conditions.push(format!("p.deal_id = ${}", bind_idx));
            bind_idx += 1;
        }
        if cbu_id.is_some() {
            conditions.push(format!("p.cbu_id = ${}", bind_idx));
            bind_idx += 1;
        }
        if from_date.is_some() {
            conditions.push(format!("bp.period_start >= ${}::date", bind_idx));
            bind_idx += 1;
        }
        if to_date.is_some() {
            conditions.push(format!("bp.period_end <= ${}::date", bind_idx));
        }

        let where_clause = if conditions.is_empty() {
            "WHERE bp.calc_status = 'INVOICED'".to_string()
        } else {
            format!(
                "WHERE bp.calc_status = 'INVOICED' AND {}",
                conditions.join(" AND ")
            )
        };

        let query = format!(
            r#"
            SELECT
                COUNT(DISTINCT bp.period_id)::int8 as period_count,
                COALESCE(SUM(bp.net_amount), 0)::float8 as total_revenue,
                COUNT(DISTINCT p.deal_id)::int8 as deal_count,
                COUNT(DISTINCT p.cbu_id)::int8 as cbu_count
            FROM "ob-poc".fee_billing_profiles p
            JOIN "ob-poc".fee_billing_periods bp ON p.profile_id = bp.profile_id
            {}
            "#,
            where_clause
        );

        // Execute with dynamic bindings
        let mut query_builder = sqlx::query_as::<_, (i64, f64, i64, i64)>(&query);

        if let Some(d) = deal_id {
            query_builder = query_builder.bind(d);
        }
        if let Some(c) = cbu_id {
            query_builder = query_builder.bind(c);
        }
        if let Some(ref f) = from_date {
            query_builder = query_builder.bind(f);
        }
        if let Some(ref t) = to_date {
            query_builder = query_builder.bind(t);
        }

        let (period_count, total_revenue, deal_count, cbu_count) =
            query_builder.fetch_one(pool).await?;

        let result = serde_json::json!({
            "period_count": period_count,
            "total_revenue": total_revenue,
            "deal_count": deal_count,
            "cbu_count": cbu_count
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "period_count": 0,
            "total_revenue": 0.0,
            "deal_count": 0,
            "cbu_count": 0
        })))
    }
}
