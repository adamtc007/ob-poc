//! Dilution Instrument Operations
//!
//! Plugin handlers for options, warrants, SAFEs, convertible notes,
//! and other dilution instruments.
//!
//! ## Key Tables
//! - kyc.dilution_instruments
//! - kyc.dilution_exercise_events
//!
//! ## Key SQL Functions
//! - kyc.fn_diluted_supply_at()

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::json;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::{PgPool, Row};

use super::helpers::{extract_string_opt, extract_uuid, extract_uuid_opt};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// ============================================================================
// Option Grants
// ============================================================================

/// Grant stock options to an employee/advisor
pub struct DilutionGrantOptionsOp;

#[async_trait]
impl CustomOperation for DilutionGrantOptionsOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "dilution.grant-options"
    }

    fn rationale(&self) -> &'static str {
        "Option grants create dilution instrument with vesting schedule"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = extract_uuid(verb_call, ctx, "share-class-id")?;
        let holder_entity_id = extract_uuid(verb_call, ctx, "holder-entity-id")?;
        let units: rust_decimal::Decimal = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let strike_price: rust_decimal::Decimal = verb_call
            .get_arg("strike-price")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("strike-price is required"))?;
        let grant_date: NaiveDate = verb_call
            .get_arg("grant-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let vesting_start_date: Option<NaiveDate> = verb_call
            .get_arg("vesting-start-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let expiry_date: Option<NaiveDate> = verb_call
            .get_arg("expiry-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let vesting_months: Option<i32> = verb_call
            .get_arg("vesting-months")
            .and_then(|a| a.value.as_integer())
            .map(|i| i as i32);
        let cliff_months: Option<i32> = verb_call
            .get_arg("cliff-months")
            .and_then(|a| a.value.as_integer())
            .map(|i| i as i32);
        let plan_name = extract_string_opt(verb_call, "plan-name");

        // Get issuer from share class
        let issuer_entity_id: Uuid =
            sqlx::query_scalar(r#"SELECT issuer_entity_id FROM kyc.share_classes WHERE id = $1"#)
                .bind(share_class_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.dilution_instruments (
                issuer_entity_id, share_class_id, holder_entity_id, instrument_type,
                units_authorized, units_outstanding, strike_price, grant_date,
                vesting_start_date, expiry_date, vesting_months, cliff_months,
                status, plan_name
            ) VALUES ($1, $2, $3, 'OPTION', $4, $4, $5, $6, $7, $8, $9, $10, 'OUTSTANDING', $11)
            RETURNING instrument_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(share_class_id)
        .bind(holder_entity_id)
        .bind(units)
        .bind(strike_price)
        .bind(grant_date)
        .bind(vesting_start_date)
        .bind(expiry_date)
        .bind(vesting_months)
        .bind(cliff_months)
        .bind(&plan_name)
        .fetch_one(pool)
        .await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, instrument_id);
        }

        tracing::info!(
            "capital.dilution.grant-options: {} options to {} at strike {}",
            units,
            holder_entity_id,
            strike_price
        );

        Ok(ExecutionResult::Uuid(instrument_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// Warrants
// ============================================================================

/// Issue warrants
pub struct DilutionIssueWarrantOp;

#[async_trait]
impl CustomOperation for DilutionIssueWarrantOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "dilution.issue-warrant"
    }

    fn rationale(&self) -> &'static str {
        "Warrants are tradeable dilution instruments with strike price"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = extract_uuid(verb_call, ctx, "share-class-id")?;
        let holder_entity_id = extract_uuid(verb_call, ctx, "holder-entity-id")?;
        let units: rust_decimal::Decimal = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let strike_price: rust_decimal::Decimal = verb_call
            .get_arg("strike-price")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("strike-price is required"))?;
        let grant_date: NaiveDate = verb_call
            .get_arg("grant-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let expiry_date: Option<NaiveDate> = verb_call
            .get_arg("expiry-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let warrant_series = extract_string_opt(verb_call, "warrant-series");

        // Get issuer from share class
        let issuer_entity_id: Uuid =
            sqlx::query_scalar(r#"SELECT issuer_entity_id FROM kyc.share_classes WHERE id = $1"#)
                .bind(share_class_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.dilution_instruments (
                issuer_entity_id, share_class_id, holder_entity_id, instrument_type,
                units_authorized, units_outstanding, strike_price, grant_date,
                expiry_date, status, warrant_series
            ) VALUES ($1, $2, $3, 'WARRANT', $4, $4, $5, $6, $7, 'OUTSTANDING', $8)
            RETURNING instrument_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(share_class_id)
        .bind(holder_entity_id)
        .bind(units)
        .bind(strike_price)
        .bind(grant_date)
        .bind(expiry_date)
        .bind(&warrant_series)
        .fetch_one(pool)
        .await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, instrument_id);
        }

        tracing::info!(
            "capital.dilution.issue-warrant: {} warrants to {} at strike {}",
            units,
            holder_entity_id,
            strike_price
        );

        Ok(ExecutionResult::Uuid(instrument_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// SAFEs
// ============================================================================

/// Create a SAFE (Simple Agreement for Future Equity)
pub struct DilutionCreateSafeOp;

#[async_trait]
impl CustomOperation for DilutionCreateSafeOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "dilution.create-safe"
    }

    fn rationale(&self) -> &'static str {
        "SAFEs convert to equity at a future priced round with cap/discount"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let holder_entity_id = extract_uuid(verb_call, ctx, "holder-entity-id")?;
        let principal: rust_decimal::Decimal = verb_call
            .get_arg("principal")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("principal is required"))?;
        let valuation_cap: Option<rust_decimal::Decimal> = verb_call
            .get_arg("valuation-cap")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let discount_pct: Option<rust_decimal::Decimal> = verb_call
            .get_arg("discount-pct")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let grant_date: NaiveDate = verb_call
            .get_arg("investment-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let target_share_class_id = extract_uuid_opt(verb_call, ctx, "target-share-class-id");
        let safe_type =
            extract_string_opt(verb_call, "safe-type").unwrap_or_else(|| "POST_MONEY".to_string());

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.dilution_instruments (
                issuer_entity_id, share_class_id, holder_entity_id, instrument_type,
                principal_amount, valuation_cap, discount_pct, grant_date,
                status, safe_type
            ) VALUES ($1, $2, $3, 'SAFE', $4, $5, $6, $7, 'OUTSTANDING', $8)
            RETURNING instrument_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(target_share_class_id)
        .bind(holder_entity_id)
        .bind(principal)
        .bind(valuation_cap)
        .bind(discount_pct)
        .bind(grant_date)
        .bind(&safe_type)
        .fetch_one(pool)
        .await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, instrument_id);
        }

        tracing::info!(
            "capital.dilution.create-safe: {} from {} with cap {:?}",
            principal,
            holder_entity_id,
            valuation_cap
        );

        Ok(ExecutionResult::Uuid(instrument_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// Convertible Notes
// ============================================================================

/// Create a convertible note
pub struct DilutionCreateConvertibleNoteOp;

#[async_trait]
impl CustomOperation for DilutionCreateConvertibleNoteOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "dilution.create-convertible-note"
    }

    fn rationale(&self) -> &'static str {
        "Convertible notes are debt instruments that convert to equity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let holder_entity_id = extract_uuid(verb_call, ctx, "holder-entity-id")?;
        let principal: rust_decimal::Decimal = verb_call
            .get_arg("principal")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("principal is required"))?;
        let interest_rate: Option<rust_decimal::Decimal> = verb_call
            .get_arg("interest-rate")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let valuation_cap: Option<rust_decimal::Decimal> = verb_call
            .get_arg("valuation-cap")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let discount_pct: Option<rust_decimal::Decimal> = verb_call
            .get_arg("discount-pct")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let grant_date: NaiveDate = verb_call
            .get_arg("issue-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let maturity_date: Option<NaiveDate> = verb_call
            .get_arg("maturity-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let target_share_class_id = extract_uuid_opt(verb_call, ctx, "target-share-class-id");

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.dilution_instruments (
                issuer_entity_id, share_class_id, holder_entity_id, instrument_type,
                principal_amount, interest_rate, valuation_cap, discount_pct,
                grant_date, maturity_date, status
            ) VALUES ($1, $2, $3, 'CONVERTIBLE_NOTE', $4, $5, $6, $7, $8, $9, 'OUTSTANDING')
            RETURNING instrument_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(target_share_class_id)
        .bind(holder_entity_id)
        .bind(principal)
        .bind(interest_rate)
        .bind(valuation_cap)
        .bind(discount_pct)
        .bind(grant_date)
        .bind(maturity_date)
        .fetch_one(pool)
        .await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, instrument_id);
        }

        tracing::info!(
            "capital.dilution.create-convertible-note: {} from {} at {}% interest",
            principal,
            holder_entity_id,
            interest_rate.map(|r| r.to_string()).unwrap_or_default()
        );

        Ok(ExecutionResult::Uuid(instrument_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// Exercise & Forfeit
// ============================================================================

/// Exercise a dilution instrument (convert to shares) with full transactional safety
///
/// Safety features:
/// 1. FOR UPDATE row lock on instrument - prevents concurrent exercise of same instrument
/// 2. Optimistic check in UPDATE WHERE clause - detects concurrent modification
/// 3. Retry loop (3 attempts) for serialization conflicts
/// 4. Idempotency key - prevents double exercise on retry
/// 5. Single transaction - all-or-nothing for instrument, holding, and supply
pub struct DilutionExerciseOp;

/// Parameters for exercise operation
#[cfg(feature = "database")]
struct ExerciseParams {
    /// The instrument to exercise
    instrument_id: Uuid,
    /// Number of units to exercise
    units_to_exercise: rust_decimal::Decimal,
    /// Date of exercise (defaults to today)
    exercise_date: NaiveDate,
    /// Override the instrument's exercise price (None = use instrument's price)
    exercise_price_override: Option<rust_decimal::Decimal>,
    /// Cashless exercise withholds shares for tax
    is_cashless: bool,
    /// Idempotency key to prevent duplicate operations
    idempotency_key: String,
}

/// Internal struct for instrument data
#[cfg(feature = "database")]
struct InstrumentData {
    issuer_entity_id: Uuid,
    share_class_id: Option<Uuid>,
    holder_entity_id: Option<Uuid>,
    instrument_type: String,
    units_granted: rust_decimal::Decimal,
    units_exercised: rust_decimal::Decimal,
    units_forfeited: rust_decimal::Decimal,
    conversion_ratio: rust_decimal::Decimal,
    exercise_price: Option<rust_decimal::Decimal>,
    status: String,
}

#[async_trait]
impl CustomOperation for DilutionExerciseOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "dilution.exercise"
    }

    fn rationale(&self) -> &'static str {
        "Exercise converts dilution instrument to actual shares, updating supply"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let instrument_id = extract_uuid(verb_call, ctx, "instrument-id")?;
        let units_to_exercise: rust_decimal::Decimal = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let exercise_date: NaiveDate = verb_call
            .get_arg("exercise-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let exercise_price_override: Option<rust_decimal::Decimal> = verb_call
            .get_arg("exercise-price")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let is_cashless: bool = verb_call
            .get_arg("is-cashless")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

        if units_to_exercise <= rust_decimal::Decimal::ZERO {
            return Err(anyhow!("units must be positive"));
        }

        // Generate idempotency key
        let idempotency_key = format!(
            "exercise:{}:{}:{}",
            instrument_id, units_to_exercise, exercise_date
        );

        // Check for existing operation (idempotent)
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT exercise_id FROM kyc.dilution_exercise_events WHERE idempotency_key = $1"#,
        )
        .bind(&idempotency_key)
        .fetch_optional(pool)
        .await?;

        if let Some(exercise_id) = existing {
            tracing::info!(
                "capital.dilution.exercise: Returning existing event {} (idempotent)",
                exercise_id
            );
            if let Some(ref binding) = verb_call.binding {
                ctx.bind(binding, exercise_id);
            }
            return Ok(ExecutionResult::Uuid(exercise_id));
        }

        // Retry loop for optimistic locking conflicts
        let max_retries = 3;
        let params = ExerciseParams {
            instrument_id,
            units_to_exercise,
            exercise_date,
            exercise_price_override,
            is_cashless,
            idempotency_key,
        };

        for attempt in 0..max_retries {
            match self.try_exercise(pool, &params).await {
                Ok((exercise_id, shares_issued)) => {
                    if let Some(ref binding) = verb_call.binding {
                        ctx.bind(binding, exercise_id);
                    }
                    tracing::info!(
                        "capital.dilution.exercise: {} units of {} -> {} shares issued",
                        units_to_exercise,
                        instrument_id,
                        shares_issued
                    );
                    return Ok(ExecutionResult::Uuid(exercise_id));
                }
                Err(e) => {
                    let err_str = e.to_string();
                    let is_serialization = err_str.contains("SERIALIZATION")
                        || err_str.contains("could not serialize");
                    if is_serialization && attempt < max_retries - 1 {
                        tracing::warn!(
                            "Exercise retry {} due to serialization conflict",
                            attempt + 1
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(
                            50 * (attempt + 1) as u64,
                        ))
                        .await;
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(anyhow!("Exercise failed after {} retries", max_retries))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

#[cfg(feature = "database")]
impl DilutionExerciseOp {
    async fn try_exercise(
        &self,
        pool: &PgPool,
        params: &ExerciseParams,
    ) -> Result<(Uuid, rust_decimal::Decimal)> {
        let mut tx = pool.begin().await?;

        // 1. Lock and fetch instrument with FOR UPDATE
        let row = sqlx::query(
            r#"
            SELECT issuer_entity_id, converts_to_share_class_id, holder_entity_id,
                   instrument_type, units_granted, units_exercised, units_forfeited,
                   COALESCE(conversion_ratio, 1.0) as conversion_ratio,
                   exercise_price, status
            FROM kyc.dilution_instruments
            WHERE instrument_id = $1
            FOR UPDATE
            "#,
        )
        .bind(params.instrument_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow!("Instrument {} not found", params.instrument_id))?;

        let instrument = InstrumentData {
            issuer_entity_id: row.get("issuer_entity_id"),
            share_class_id: row.get("converts_to_share_class_id"),
            holder_entity_id: row.get("holder_entity_id"),
            instrument_type: row.get("instrument_type"),
            units_granted: row.get("units_granted"),
            units_exercised: row.get("units_exercised"),
            units_forfeited: row.get("units_forfeited"),
            conversion_ratio: row.get("conversion_ratio"),
            exercise_price: row.get("exercise_price"),
            status: row.get("status"),
        };

        // 2. Validate
        if instrument.status != "ACTIVE" {
            return Err(anyhow!(
                "Instrument {} is not active (status={})",
                params.instrument_id,
                instrument.status
            ));
        }

        let share_class_id = instrument.share_class_id.ok_or_else(|| {
            anyhow!(
                "Instrument {} has no conversion share class",
                params.instrument_id
            )
        })?;

        let holder_entity_id = instrument
            .holder_entity_id
            .ok_or_else(|| anyhow!("Instrument {} has no holder", params.instrument_id))?;

        let units_outstanding =
            instrument.units_granted - instrument.units_exercised - instrument.units_forfeited;

        if params.units_to_exercise > units_outstanding {
            return Err(anyhow!(
                "Cannot exercise {} units: only {} outstanding",
                params.units_to_exercise,
                units_outstanding
            ));
        }

        // 3. Calculate shares to issue
        let shares_to_issue = params.units_to_exercise * instrument.conversion_ratio;
        let shares_after_tax = if params.is_cashless {
            // Withhold ~40% for taxes on cashless exercise
            shares_to_issue * rust_decimal::Decimal::from_str_exact("0.6")?
        } else {
            shares_to_issue
        };
        let shares_withheld = if params.is_cashless {
            Some(shares_to_issue - shares_after_tax)
        } else {
            None
        };

        // Use override price, or instrument's exercise price
        let actual_price = params.exercise_price_override.or(instrument.exercise_price);

        // 4. Update instrument with optimistic check
        let new_units_exercised = instrument.units_exercised + params.units_to_exercise;
        let new_status =
            if new_units_exercised + instrument.units_forfeited >= instrument.units_granted {
                "EXERCISED"
            } else {
                "ACTIVE"
            };

        let rows = sqlx::query(
            r#"
            UPDATE kyc.dilution_instruments
            SET units_exercised = $2,
                status = $3,
                updated_at = now()
            WHERE instrument_id = $1
              AND units_exercised = $4  -- Optimistic check
            "#,
        )
        .bind(params.instrument_id)
        .bind(new_units_exercised)
        .bind(new_status)
        .bind(instrument.units_exercised) // expected current value
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows == 0 {
            // Concurrent modification detected
            return Err(anyhow!("Concurrent modification detected").context("SERIALIZATION"));
        }

        // 5. Create or update holding (upsert pattern)
        let holding_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.holdings (
                share_class_id, investor_entity_id, units,
                cost_basis, acquisition_date, status
            ) VALUES ($1, $2, $3, $4, $5, 'active')
            ON CONFLICT (share_class_id, investor_entity_id)
            DO UPDATE SET
                units = kyc.holdings.units + EXCLUDED.units,
                updated_at = now()
            RETURNING id
            "#,
        )
        .bind(share_class_id)
        .bind(holder_entity_id)
        .bind(shares_after_tax)
        .bind(actual_price)
        .bind(params.exercise_date)
        .fetch_one(&mut *tx)
        .await?;

        // 6. Create exercise event (audit)
        let exercise_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.dilution_exercise_events (
                instrument_id, units_exercised, exercise_date,
                exercise_price_paid, shares_issued, resulting_holding_id,
                is_cashless, shares_withheld_for_tax, idempotency_key,
                notes
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING exercise_id
            "#,
        )
        .bind(params.instrument_id)
        .bind(params.units_to_exercise)
        .bind(params.exercise_date)
        .bind(actual_price)
        .bind(shares_to_issue)
        .bind(holding_id)
        .bind(params.is_cashless)
        .bind(shares_withheld)
        .bind(&params.idempotency_key)
        .bind(format!(
            "Exercise of {} {}",
            instrument.instrument_type, params.instrument_id
        ))
        .fetch_one(&mut *tx)
        .await?;

        // 7. Update supply
        sqlx::query(
            r#"
            UPDATE kyc.share_class_supply
            SET issued_units = issued_units + $2,
                outstanding_units = outstanding_units + $2,
                updated_at = now()
            WHERE share_class_id = $1
              AND as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = $1)
            "#,
        )
        .bind(share_class_id)
        .bind(shares_after_tax)
        .execute(&mut *tx)
        .await?;

        // 8. Create issuance event for audit trail
        sqlx::query(
            r#"
            INSERT INTO kyc.issuance_events (
                share_class_id, issuer_entity_id, event_type, units_delta,
                price_per_unit, effective_date, notes, status
            ) VALUES ($1, $2, 'CONVERSION', $3, $4, $5, $6, 'EFFECTIVE')
            "#,
        )
        .bind(share_class_id)
        .bind(instrument.issuer_entity_id)
        .bind(shares_after_tax)
        .bind(actual_price)
        .bind(params.exercise_date)
        .bind(format!(
            "Exercise of {} {} ({} units -> {} shares)",
            instrument.instrument_type,
            params.instrument_id,
            params.units_to_exercise,
            shares_after_tax
        ))
        .execute(&mut *tx)
        .await?;

        // 9. Commit
        tx.commit().await?;

        Ok((exercise_id, shares_after_tax))
    }
}

/// Forfeit unvested or cancelled dilution instruments
pub struct DilutionForfeitOp;

#[async_trait]
impl CustomOperation for DilutionForfeitOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "dilution.forfeit"
    }

    fn rationale(&self) -> &'static str {
        "Forfeiture cancels unvested instruments when employee leaves"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let instrument_id = extract_uuid(verb_call, ctx, "instrument-id")?;
        let units: Option<rust_decimal::Decimal> = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let forfeit_date: NaiveDate = verb_call
            .get_arg("forfeit-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let reason = extract_string_opt(verb_call, "reason");

        // Get instrument details
        let instrument: Option<(rust_decimal::Decimal,)> = sqlx::query_as(
            r#"
            SELECT units_outstanding
            FROM kyc.dilution_instruments
            WHERE instrument_id = $1 AND status = 'OUTSTANDING'
            "#,
        )
        .bind(instrument_id)
        .fetch_optional(pool)
        .await?;

        let (outstanding,) = instrument
            .ok_or_else(|| anyhow!("Instrument {} not found or not outstanding", instrument_id))?;

        let units_to_forfeit = units.unwrap_or(outstanding);

        if units_to_forfeit > outstanding {
            return Err(anyhow!(
                "Cannot forfeit {} units: only {} outstanding",
                units_to_forfeit,
                outstanding
            ));
        }

        let mut tx = pool.begin().await?;

        // Record forfeit event
        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.dilution_exercise_events (
                instrument_id, exercise_type, units, exercise_date, notes, status
            ) VALUES ($1, 'FORFEIT', $2, $3, $4, 'COMPLETED')
            RETURNING event_id
            "#,
        )
        .bind(instrument_id)
        .bind(units_to_forfeit)
        .bind(forfeit_date)
        .bind(&reason)
        .fetch_one(&mut *tx)
        .await?;

        // Update instrument
        let remaining = outstanding - units_to_forfeit;
        let new_status = if remaining == rust_decimal::Decimal::ZERO {
            "FORFEITED"
        } else {
            "OUTSTANDING"
        };

        sqlx::query(
            r#"
            UPDATE kyc.dilution_instruments
            SET units_outstanding = $2, status = $3, updated_at = now()
            WHERE instrument_id = $1
            "#,
        )
        .bind(instrument_id)
        .bind(remaining)
        .bind(new_status)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, event_id);
        }

        tracing::info!(
            "capital.dilution.forfeit: {} units of {} forfeited",
            units_to_forfeit,
            instrument_id
        );

        Ok(ExecutionResult::Uuid(event_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// Query Operations
// ============================================================================

/// List dilution instruments for an issuer
pub struct DilutionListOp;

#[async_trait]
impl CustomOperation for DilutionListOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "dilution.list"
    }

    fn rationale(&self) -> &'static str {
        "Dilution listing with filtering by type and status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let instrument_type = extract_string_opt(verb_call, "instrument-type");
        let status =
            extract_string_opt(verb_call, "status").unwrap_or_else(|| "OUTSTANDING".to_string());

        let instruments: Vec<(
            Uuid,
            Uuid,
            Option<Uuid>,
            Uuid,
            String,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            NaiveDate,
            Option<NaiveDate>,
            String,
        )> = if let Some(ref itype) = instrument_type {
            sqlx::query_as(
                r#"
                SELECT instrument_id, issuer_entity_id, share_class_id, holder_entity_id,
                       instrument_type, units_authorized, units_outstanding,
                       strike_price, principal_amount, valuation_cap,
                       grant_date, expiry_date, status
                FROM kyc.dilution_instruments
                WHERE issuer_entity_id = $1 AND instrument_type = $2 AND status = $3
                ORDER BY grant_date DESC
                "#,
            )
            .bind(issuer_entity_id)
            .bind(itype)
            .bind(&status)
            .fetch_all(pool)
            .await?
        } else if status == "ALL" {
            sqlx::query_as(
                r#"
                SELECT instrument_id, issuer_entity_id, share_class_id, holder_entity_id,
                       instrument_type, units_authorized, units_outstanding,
                       strike_price, principal_amount, valuation_cap,
                       grant_date, expiry_date, status
                FROM kyc.dilution_instruments
                WHERE issuer_entity_id = $1
                ORDER BY grant_date DESC
                "#,
            )
            .bind(issuer_entity_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT instrument_id, issuer_entity_id, share_class_id, holder_entity_id,
                       instrument_type, units_authorized, units_outstanding,
                       strike_price, principal_amount, valuation_cap,
                       grant_date, expiry_date, status
                FROM kyc.dilution_instruments
                WHERE issuer_entity_id = $1 AND status = $2
                ORDER BY grant_date DESC
                "#,
            )
            .bind(issuer_entity_id)
            .bind(&status)
            .fetch_all(pool)
            .await?
        };

        // Get holder names
        let instrument_data: Vec<serde_json::Value> =
            futures::future::try_join_all(instruments.iter().map(|i| async {
                let holder_name: Option<String> = sqlx::query_scalar(
                    r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#,
                )
                .bind(i.3)
                .fetch_optional(pool)
                .await?;

                let share_class_name: Option<String> = if let Some(sc_id) = i.2 {
                    sqlx::query_scalar(r#"SELECT name FROM kyc.share_classes WHERE id = $1"#)
                        .bind(sc_id)
                        .fetch_optional(pool)
                        .await?
                } else {
                    None
                };

                Ok::<_, anyhow::Error>(json!({
                    "instrument_id": i.0,
                    "share_class_id": i.2,
                    "share_class_name": share_class_name,
                    "holder_entity_id": i.3,
                    "holder_name": holder_name,
                    "instrument_type": i.4,
                    "units_authorized": i.5.to_string(),
                    "units_outstanding": i.6.to_string(),
                    "strike_price": i.7.map(|d| d.to_string()),
                    "principal_amount": i.8.map(|d| d.to_string()),
                    "valuation_cap": i.9.map(|d| d.to_string()),
                    "grant_date": i.10.to_string(),
                    "expiry_date": i.11.map(|d| d.to_string()),
                    "status": i.12
                }))
            }))
            .await?;

        Ok(ExecutionResult::RecordSet(instrument_data))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Get dilution summary for an issuer
pub struct DilutionGetSummaryOp;

#[async_trait]
impl CustomOperation for DilutionGetSummaryOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "dilution.get-summary"
    }

    fn rationale(&self) -> &'static str {
        "Dilution summary aggregates across instrument types using SQL view"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let as_of: NaiveDate = verb_call
            .get_arg("as-of")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let basis =
            extract_string_opt(verb_call, "basis").unwrap_or_else(|| "EXERCISABLE".to_string());

        // Get summary from view
        let summary: Vec<(
            Uuid,
            String,
            String,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            Option<rust_decimal::Decimal>,
        )> = sqlx::query_as(
            r#"
            SELECT issuer_entity_id, share_class_name, instrument_type,
                   units_authorized, units_outstanding, units_exercised, weighted_avg_strike
            FROM kyc.v_dilution_summary
            WHERE issuer_entity_id = $1
            ORDER BY instrument_type
            "#,
        )
        .bind(issuer_entity_id)
        .fetch_all(pool)
        .await?;

        let mut total_outstanding = rust_decimal::Decimal::ZERO;
        let mut total_potential_shares = rust_decimal::Decimal::ZERO;

        let summary_data: Vec<serde_json::Value> = summary
            .iter()
            .map(
                |(_, class_name, itype, authorized, outstanding, exercised, avg_strike)| {
                    total_outstanding += outstanding;
                    total_potential_shares += outstanding;

                    json!({
                        "share_class_name": class_name,
                        "instrument_type": itype,
                        "units_authorized": authorized.to_string(),
                        "units_outstanding": outstanding.to_string(),
                        "units_exercised": exercised.to_string(),
                        "weighted_avg_strike": avg_strike.map(|d| d.to_string())
                    })
                },
            )
            .collect();

        // Get current outstanding shares for fully diluted calculation
        let outstanding_shares: rust_decimal::Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(scs.outstanding_units), 0)
            FROM kyc.share_classes sc
            LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
            WHERE sc.issuer_entity_id = $1
            "#,
        )
        .bind(issuer_entity_id)
        .fetch_one(pool)
        .await?;

        let fully_diluted = outstanding_shares + total_potential_shares;
        let dilution_pct = if fully_diluted > rust_decimal::Decimal::ZERO {
            (total_potential_shares / fully_diluted * rust_decimal::Decimal::from(100)).round_dp(4)
        } else {
            rust_decimal::Decimal::ZERO
        };

        Ok(ExecutionResult::Record(json!({
            "issuer_entity_id": issuer_entity_id,
            "as_of_date": as_of.to_string(),
            "basis": basis,
            "current_outstanding_shares": outstanding_shares.to_string(),
            "total_dilution_instruments_outstanding": total_outstanding.to_string(),
            "fully_diluted_shares": fully_diluted.to_string(),
            "dilution_pct": dilution_pct.to_string(),
            "by_instrument_type": summary_data
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// Registration function for mod.rs
// ============================================================================

pub fn register_dilution_ops(registry: &mut super::CustomOperationRegistry) {
    use std::sync::Arc;

    // Option grants
    registry.register(Arc::new(DilutionGrantOptionsOp));

    // Warrants
    registry.register(Arc::new(DilutionIssueWarrantOp));

    // SAFEs
    registry.register(Arc::new(DilutionCreateSafeOp));

    // Convertible notes
    registry.register(Arc::new(DilutionCreateConvertibleNoteOp));

    // Exercise & forfeit
    registry.register(Arc::new(DilutionExerciseOp));
    registry.register(Arc::new(DilutionForfeitOp));

    // Query operations
    registry.register(Arc::new(DilutionListOp));
    registry.register(Arc::new(DilutionGetSummaryOp));
}
