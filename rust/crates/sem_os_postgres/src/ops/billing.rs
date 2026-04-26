//! Fee billing verbs (14 plugin verbs) — YAML-first
//! re-implementation of `billing.*` from
//! `rust/config/verbs/billing.yaml`.
//!
//! Profile lifecycle: create-profile → activate-profile →
//! suspend/close-profile. Account targets: add/remove-account-target.
//! Period lifecycle: create-period → calculate-period → review-period
//! → approve-period → generate-invoice (or dispute-period). Reads:
//! period-summary + revenue-summary.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingCalculationResult {
    pub period_id: Uuid,
    pub line_count: i32,
    pub gross_amount: f64,
    pub status: String,
}

// ---------------------------------------------------------------------------
// Profile lifecycle
// ---------------------------------------------------------------------------

pub struct CreateProfile;

#[async_trait]
impl SemOsVerbOp for CreateProfile {
    fn fqn(&self) -> &str {
        "billing.create-profile"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let contract_id = json_extract_uuid(args, ctx, "contract-id")?;
        let rate_card_id = json_extract_uuid(args, ctx, "rate-card-id")?;
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let product_id = json_extract_uuid(args, ctx, "product-id")?;
        let invoice_entity_id = json_extract_uuid(args, ctx, "invoice-entity-id")?;
        let profile_name = json_extract_string_opt(args, "profile-name");
        let billing_frequency = json_extract_string_opt(args, "billing-frequency")
            .unwrap_or_else(|| "MONTHLY".to_string());
        let invoice_currency =
            json_extract_string_opt(args, "invoice-currency").unwrap_or_else(|| "USD".to_string());
        let payment_method = json_extract_string_opt(args, "payment-method");
        let payment_account_ref = json_extract_string_opt(args, "payment-account-ref");
        let effective_from = json_extract_string(args, "effective-from")?;

        let rate_card_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(scope.executor())
        .await?;
        if rate_card_status != "AGREED" {
            return Err(anyhow!(
                "Rate card must be in AGREED status to create billing profile"
            ));
        }

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
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value)
            VALUES ($1, 'BILLING_PROFILE_CREATED', 'BILLING_PROFILE', $2, 'PENDING')
            "#,
        )
        .bind(deal_id)
        .bind(profile_id)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(profile_id))
    }
}

pub struct ActivateProfile;

#[async_trait]
impl SemOsVerbOp for ActivateProfile {
    fn fqn(&self) -> &str {
        "billing.activate-profile"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;

        let target_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".fee_billing_account_targets WHERE profile_id = $1 AND is_active = true"#,
        )
        .bind(profile_id)
        .fetch_one(scope.executor())
        .await?;
        if target_count == 0 {
            return Err(anyhow!(
                "Cannot activate billing profile without account targets"
            ));
        }

        let deal_id: Uuid = sqlx::query_scalar(
            r#"SELECT deal_id FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_profiles SET status = 'ACTIVE', updated_at = NOW() WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value)
            VALUES ($1, 'BILLING_ACTIVATED', 'BILLING_PROFILE', $2, 'ACTIVE')
            "#,
        )
        .bind(deal_id)
        .bind(profile_id)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: billing profile activated (DRAFT → ACTIVE).
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            profile_id,
            "billing-profile:active",
            "billing/profile",
            &format!(
                "billing.activate-profile — profile {} ACTIVE (deal {})",
                profile_id, deal_id
            ),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct SuspendProfile;

#[async_trait]
impl SemOsVerbOp for SuspendProfile {
    fn fqn(&self) -> &str {
        "billing.suspend-profile"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let reason = json_extract_string(args, "reason")?;

        let deal_id: Uuid = sqlx::query_scalar(
            r#"SELECT deal_id FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_profiles SET status = 'SUSPENDED', updated_at = NOW() WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value, description)
            VALUES ($1, 'BILLING_SUSPENDED', 'BILLING_PROFILE', $2, 'SUSPENDED', $3)
            "#,
        )
        .bind(deal_id)
        .bind(profile_id)
        .bind(&reason)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: billing profile suspended.
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            profile_id,
            "billing-profile:suspended",
            "billing/profile",
            &format!(
                "billing.suspend-profile — profile {} SUSPENDED ({})",
                profile_id, reason
            ),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct CloseProfile;

#[async_trait]
impl SemOsVerbOp for CloseProfile {
    fn fqn(&self) -> &str {
        "billing.close-profile"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let effective_to = json_extract_string(args, "effective-to")?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_profiles
            SET status = 'CLOSED', effective_to = $2::date, updated_at = NOW()
            WHERE profile_id = $1
            "#,
        )
        .bind(profile_id)
        .bind(&effective_to)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: billing profile closed (terminal).
        if result.rows_affected() > 0 {
            dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
                ctx,
                profile_id,
                "billing-profile:closed",
                "billing/profile",
                &format!(
                    "billing.close-profile — profile {} CLOSED effective {}",
                    profile_id, effective_to
                ),
            );
        }
        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// ---------------------------------------------------------------------------
// Account targets
// ---------------------------------------------------------------------------

pub struct AddAccountTarget;

#[async_trait]
impl SemOsVerbOp for AddAccountTarget {
    fn fqn(&self) -> &str {
        "billing.add-account-target"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let cbu_resource_instance_id = json_extract_uuid(args, ctx, "cbu-resource-instance-id")?;
        let rate_card_line_id = json_extract_uuid_opt(args, ctx, "rate-card-line-id");
        let resource_type = json_extract_string_opt(args, "resource-type");
        let resource_ref = json_extract_string_opt(args, "resource-ref");
        let activity_type = json_extract_string_opt(args, "activity-type");
        let has_override = json_extract_bool_opt(args, "has-override").unwrap_or(false);
        let override_rate: Option<f64> =
            json_extract_string_opt(args, "override-rate").and_then(|s| s.parse().ok());
        let override_model = json_extract_string_opt(args, "override-model");

        let profile_cbu_id: Uuid = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(scope.executor())
        .await?;
        let instance_cbu_id: Uuid = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbu_resource_instances WHERE instance_id = $1"#,
        )
        .bind(cbu_resource_instance_id)
        .fetch_one(scope.executor())
        .await?;
        if profile_cbu_id != instance_cbu_id {
            return Err(anyhow!(
                "Resource instance does not belong to the profile's CBU"
            ));
        }

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
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(target_id))
    }
}

pub struct RemoveAccountTarget;

#[async_trait]
impl SemOsVerbOp for RemoveAccountTarget {
    fn fqn(&self) -> &str {
        "billing.remove-account-target"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let target_id = json_extract_uuid(args, ctx, "target-id")?;
        let result = sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_account_targets SET is_active = false, updated_at = NOW() WHERE target_id = $1"#,
        )
        .bind(target_id)
        .execute(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// ---------------------------------------------------------------------------
// Period lifecycle
// ---------------------------------------------------------------------------

pub struct CreatePeriod;

#[async_trait]
impl SemOsVerbOp for CreatePeriod {
    fn fqn(&self) -> &str {
        "billing.create-period"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let period_start = json_extract_string(args, "period-start")?;
        let period_end = json_extract_string(args, "period-end")?;

        let profile_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(scope.executor())
        .await?;
        if profile_status != "ACTIVE" {
            return Err(anyhow!("Billing profile must be ACTIVE to create periods"));
        }

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
        .fetch_one(scope.executor())
        .await?;
        if overlap_count > 0 {
            return Err(anyhow!("Billing period overlaps with existing period"));
        }

        let currency_code: String = sqlx::query_scalar(
            r#"SELECT invoice_currency FROM "ob-poc".fee_billing_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(scope.executor())
        .await?;

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
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(period_id))
    }
}

pub struct CalculatePeriod;

#[async_trait]
impl SemOsVerbOp for CalculatePeriod {
    fn fqn(&self) -> &str {
        "billing.calculate-period"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let period_id = json_extract_uuid(args, ctx, "period-id")?;

        sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_periods SET calc_status = 'CALCULATING' WHERE period_id = $1"#,
        )
        .bind(period_id)
        .execute(scope.executor())
        .await?;

        let (profile_id, _period_start, _period_end): (Uuid, String, String) = sqlx::query_as(
            r#"SELECT profile_id, period_start::text, period_end::text FROM "ob-poc".fee_billing_periods WHERE period_id = $1"#,
        )
        .bind(period_id)
        .fetch_one(scope.executor())
        .await?;

        type TargetRow = (
            Uuid,
            Option<Uuid>,
            Option<String>,
            Option<f64>,
            Option<String>,
        );
        let targets: Vec<TargetRow> = sqlx::query_as(
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
        .fetch_all(scope.executor())
        .await?;

        let mut line_count = 0;
        let mut gross_amount = 0.0f64;

        for (target_id, rate_card_line_id, _activity_type, rate, pricing_model) in targets {
            let activity_volume = 1_000_000.0f64;
            let activity_unit = "USD_AUM".to_string();

            let calculated_fee = match pricing_model.as_deref() {
                Some("BPS") => {
                    let rate_decimal = rate.unwrap_or(0.0) / 10_000.0;
                    activity_volume * rate_decimal
                }
                Some("FLAT") => rate.unwrap_or(0.0),
                Some("PER_TRANSACTION") => rate.unwrap_or(0.0) * 100.0,
                _ => 0.0,
            };

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
            .execute(scope.executor())
            .await?;

            line_count += 1;
            gross_amount += calculated_fee;
        }

        sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_periods
            SET gross_amount = $2, net_amount = $2, calc_status = 'CALCULATED',
                calculated_at = NOW(), updated_at = NOW()
            WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .bind(gross_amount)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: billing period calculated.
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            period_id,
            "billing-period:calculated",
            "billing/period",
            &format!(
                "billing.calculate-period — {} lines calculated, gross {:.2}",
                line_count, gross_amount
            ),
        );

        let result = BillingCalculationResult {
            period_id,
            line_count,
            gross_amount,
            status: "CALCULATED".to_string(),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

pub struct ReviewPeriod;

#[async_trait]
impl SemOsVerbOp for ReviewPeriod {
    fn fqn(&self) -> &str {
        "billing.review-period"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let period_id = json_extract_uuid(args, ctx, "period-id")?;
        let reviewed_by = json_extract_string(args, "reviewed-by")?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_periods
            SET calc_status = 'REVIEWED', reviewed_by = $2, updated_at = NOW()
            WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .bind(&reviewed_by)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: period CALCULATED → REVIEWED.
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            period_id,
            "billing-period:reviewed",
            "billing/period",
            &format!("billing.review-period — reviewed by {}", reviewed_by),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct ApprovePeriod;

#[async_trait]
impl SemOsVerbOp for ApprovePeriod {
    fn fqn(&self) -> &str {
        "billing.approve-period"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let period_id = json_extract_uuid(args, ctx, "period-id")?;
        let approved_by = json_extract_string(args, "approved-by")?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_periods
            SET calc_status = 'APPROVED', approved_by = $2, approved_at = NOW(), updated_at = NOW()
            WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .bind(&approved_by)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: period REVIEWED → APPROVED (gate before invoice).
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            period_id,
            "billing-period:approved",
            "billing/period",
            &format!("billing.approve-period — approved by {}", approved_by),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct GenerateInvoice;

#[async_trait]
impl SemOsVerbOp for GenerateInvoice {
    fn fqn(&self) -> &str {
        "billing.generate-invoice"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let period_id = json_extract_uuid(args, ctx, "period-id")?;

        let status: String = sqlx::query_scalar(
            r#"SELECT calc_status FROM "ob-poc".fee_billing_periods WHERE period_id = $1"#,
        )
        .bind(period_id)
        .fetch_one(scope.executor())
        .await?;
        if status != "APPROVED" {
            return Err(anyhow!("Period must be APPROVED to generate invoice"));
        }

        let invoice_id = Uuid::new_v4();
        sqlx::query(
            r#"
            UPDATE "ob-poc".fee_billing_periods
            SET calc_status = 'INVOICED', invoice_id = $2, invoiced_at = NOW(), updated_at = NOW()
            WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .bind(invoice_id)
        .execute(scope.executor())
        .await?;

        let deal_id: Uuid = sqlx::query_scalar(
            r#"
            SELECT p.deal_id
            FROM "ob-poc".fee_billing_profiles p
            JOIN "ob-poc".fee_billing_periods bp ON p.profile_id = bp.profile_id
            WHERE bp.period_id = $1
            "#,
        )
        .bind(period_id)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'INVOICE_GENERATED', 'BILLING_PERIOD', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(period_id)
        .bind(invoice_id.to_string())
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: period APPROVED → INVOICED (terminal for the
        // period's lifecycle; invoice is a separate downstream entity).
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            period_id,
            "billing-period:invoiced",
            "billing/period",
            &format!("billing.generate-invoice — invoice {} issued", invoice_id),
        );
        Ok(VerbExecutionOutcome::Uuid(invoice_id))
    }
}

pub struct DisputePeriod;

#[async_trait]
impl SemOsVerbOp for DisputePeriod {
    fn fqn(&self) -> &str {
        "billing.dispute-period"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let period_id = json_extract_uuid(args, ctx, "period-id")?;
        let dispute_reason = json_extract_string(args, "dispute-reason")?;

        sqlx::query(
            r#"UPDATE "ob-poc".fee_billing_periods SET calc_status = 'DISPUTED', updated_at = NOW() WHERE period_id = $1"#,
        )
        .bind(period_id)
        .execute(scope.executor())
        .await?;

        let deal_id: Uuid = sqlx::query_scalar(
            r#"
            SELECT p.deal_id
            FROM "ob-poc".fee_billing_profiles p
            JOIN "ob-poc".fee_billing_periods bp ON p.profile_id = bp.profile_id
            WHERE bp.period_id = $1
            "#,
        )
        .bind(period_id)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'BILLING_DISPUTED', 'BILLING_PERIOD', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(period_id)
        .bind(&dispute_reason)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: period transitions to DISPUTED (reason captures
        // the dispute cause for operator review).
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            period_id,
            "billing-period:disputed",
            "billing/period",
            &format!("billing.dispute-period — DISPUTED ({})", dispute_reason),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct PeriodSummary;

#[async_trait]
impl SemOsVerbOp for PeriodSummary {
    fn fqn(&self) -> &str {
        "billing.period-summary"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let period_id = json_extract_uuid(args, ctx, "period-id")?;

        let period: (Uuid, String, String, String, Option<f64>, Option<f64>) = sqlx::query_as(
            r#"
            SELECT period_id, period_start::text, period_end::text, calc_status, gross_amount::float8, net_amount::float8
            FROM "ob-poc".fee_billing_periods WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .fetch_one(scope.executor())
        .await?;

        let lines: Vec<(Uuid, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
            r#"
            SELECT period_line_id, activity_volume::float8, calculated_fee::float8, net_fee::float8
            FROM "ob-poc".fee_billing_period_lines WHERE period_id = $1
            "#,
        )
        .bind(period_id)
        .fetch_all(scope.executor())
        .await?;

        let line_values: Vec<Value> = lines
            .into_iter()
            .map(|(line_id, volume, calc_fee, net_fee)| {
                json!({
                    "period_line_id": line_id,
                    "activity_volume": volume,
                    "calculated_fee": calc_fee,
                    "net_fee": net_fee
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "period_id": period.0,
            "period_start": period.1,
            "period_end": period.2,
            "calc_status": period.3,
            "gross_amount": period.4,
            "net_amount": period.5,
            "lines": line_values
        })))
    }
}

pub struct RevenueSummary;

#[async_trait]
impl SemOsVerbOp for RevenueSummary {
    fn fqn(&self) -> &str {
        "billing.revenue-summary"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid_opt(args, ctx, "deal-id");
        let cbu_id = json_extract_uuid_opt(args, ctx, "cbu-id");
        let from_date = json_extract_string_opt(args, "from-date");
        let to_date = json_extract_string_opt(args, "to-date");

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

        let mut q = sqlx::query_as::<_, (i64, f64, i64, i64)>(&query);
        if let Some(d) = deal_id {
            q = q.bind(d);
        }
        if let Some(c) = cbu_id {
            q = q.bind(c);
        }
        if let Some(ref f) = from_date {
            q = q.bind(f);
        }
        if let Some(ref t) = to_date {
            q = q.bind(t);
        }

        let (period_count, total_revenue, deal_count, cbu_count) =
            q.fetch_one(scope.executor()).await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "period_count": period_count,
            "total_revenue": total_revenue,
            "deal_count": deal_count,
            "cbu_count": cbu_count
        })))
    }
}
