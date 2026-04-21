//! Capital-structure dilution verbs (8 plugin verbs) — YAML-first
//! re-implementation of `capital.dilution.*` from
//! `rust/config/verbs/capital.yaml`.
//!
//! Ops:
//! - `grant-options` — insert OPTION instrument with vesting schedule
//! - `issue-warrant` — insert WARRANT instrument
//! - `create-safe` — insert SAFE (cap + discount)
//! - `create-convertible-note` — insert CONVERTIBLE_NOTE
//! - `exercise` — atomically convert instrument → shares with
//!   FOR UPDATE lock + optimistic check + idempotency key. The retry
//!   loop present in the legacy impl is dropped: under the Sequencer
//!   scope, the surrounding transaction owns retry semantics, and a
//!   serialization conflict simply aborts the verb step (which the
//!   runbook compiler can replay).
//! - `forfeit` — cancel unvested units (audit event + supply update)
//! - `list` — filter by instrument_type + status
//! - `get-summary` — aggregate dilution % against supply

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

fn date_arg(args: &Value, arg_name: &str) -> NaiveDate {
    json_extract_string_opt(args, arg_name)
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Utc::now().date_naive())
}

pub struct GrantOptions;

#[async_trait]
impl SemOsVerbOp for GrantOptions {
    fn fqn(&self) -> &str {
        "capital.dilution.grant-options"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_extract_uuid(args, ctx, "share-class-id")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder-entity-id")?;
        let units: rust_decimal::Decimal = json_extract_string_opt(args, "units")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let strike_price: rust_decimal::Decimal =
            json_extract_string_opt(args, "strike-price")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| anyhow!("strike-price is required"))?;
        let grant_date = date_arg(args, "grant-date");
        let vesting_start_date = json_extract_string_opt(args, "vesting-start-date")
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let expiry_date = json_extract_string_opt(args, "expiry-date")
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let vesting_months = json_extract_int_opt(args, "vesting-months").map(|i| i as i32);
        let cliff_months = json_extract_int_opt(args, "cliff-months").map(|i| i as i32);
        let plan_name = json_extract_string_opt(args, "plan-name");

        let issuer_entity_id: Uuid = sqlx::query_scalar(
            r#"SELECT issuer_entity_id FROM "ob-poc".share_classes WHERE id = $1"#,
        )
        .bind(share_class_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_instruments (
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
        .fetch_one(scope.executor())
        .await?;
        ctx.bind("dilution_instrument", instrument_id);
        Ok(VerbExecutionOutcome::Uuid(instrument_id))
    }
}

pub struct IssueWarrant;

#[async_trait]
impl SemOsVerbOp for IssueWarrant {
    fn fqn(&self) -> &str {
        "capital.dilution.issue-warrant"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_extract_uuid(args, ctx, "share-class-id")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder-entity-id")?;
        let units: rust_decimal::Decimal = json_extract_string_opt(args, "units")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let strike_price: rust_decimal::Decimal =
            json_extract_string_opt(args, "strike-price")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| anyhow!("strike-price is required"))?;
        let grant_date = date_arg(args, "grant-date");
        let expiry_date = json_extract_string_opt(args, "expiry-date")
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let warrant_series = json_extract_string_opt(args, "warrant-series");

        let issuer_entity_id: Uuid = sqlx::query_scalar(
            r#"SELECT issuer_entity_id FROM "ob-poc".share_classes WHERE id = $1"#,
        )
        .bind(share_class_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_instruments (
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
        .fetch_one(scope.executor())
        .await?;
        ctx.bind("dilution_instrument", instrument_id);
        Ok(VerbExecutionOutcome::Uuid(instrument_id))
    }
}

pub struct CreateSafe;

#[async_trait]
impl SemOsVerbOp for CreateSafe {
    fn fqn(&self) -> &str {
        "capital.dilution.create-safe"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder-entity-id")?;
        let principal: rust_decimal::Decimal = json_extract_string_opt(args, "principal")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("principal is required"))?;
        let valuation_cap: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "valuation-cap").and_then(|s| s.parse().ok());
        let discount_pct: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "discount-pct").and_then(|s| s.parse().ok());
        let grant_date = date_arg(args, "investment-date");
        let target_share_class_id = json_extract_uuid_opt(args, ctx, "target-share-class-id");
        let safe_type = json_extract_string_opt(args, "safe-type")
            .unwrap_or_else(|| "POST_MONEY".to_string());

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_instruments (
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
        .fetch_one(scope.executor())
        .await?;
        ctx.bind("dilution_instrument", instrument_id);
        Ok(VerbExecutionOutcome::Uuid(instrument_id))
    }
}

pub struct CreateConvertibleNote;

#[async_trait]
impl SemOsVerbOp for CreateConvertibleNote {
    fn fqn(&self) -> &str {
        "capital.dilution.create-convertible-note"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder-entity-id")?;
        let principal: rust_decimal::Decimal = json_extract_string_opt(args, "principal")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("principal is required"))?;
        let interest_rate: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "interest-rate").and_then(|s| s.parse().ok());
        let valuation_cap: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "valuation-cap").and_then(|s| s.parse().ok());
        let discount_pct: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "discount-pct").and_then(|s| s.parse().ok());
        let grant_date = date_arg(args, "issue-date");
        let maturity_date = json_extract_string_opt(args, "maturity-date")
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let target_share_class_id = json_extract_uuid_opt(args, ctx, "target-share-class-id");

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_instruments (
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
        .fetch_one(scope.executor())
        .await?;
        ctx.bind("dilution_instrument", instrument_id);
        Ok(VerbExecutionOutcome::Uuid(instrument_id))
    }
}

pub struct Exercise;

#[async_trait]
impl SemOsVerbOp for Exercise {
    fn fqn(&self) -> &str {
        "capital.dilution.exercise"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instrument_id = json_extract_uuid(args, ctx, "instrument-id")?;
        let units_to_exercise: rust_decimal::Decimal =
            json_extract_string_opt(args, "units")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| anyhow!("units is required"))?;
        let exercise_date = date_arg(args, "exercise-date");
        let exercise_price_override: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "exercise-price").and_then(|s| s.parse().ok());
        let is_cashless = json_extract_bool_opt(args, "is-cashless").unwrap_or(false);

        if units_to_exercise <= rust_decimal::Decimal::ZERO {
            return Err(anyhow!("units must be positive"));
        }
        let idempotency_key = format!(
            "exercise:{}:{}:{}",
            instrument_id, units_to_exercise, exercise_date
        );
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT exercise_id FROM "ob-poc".dilution_exercise_events WHERE idempotency_key = $1"#,
        )
        .bind(&idempotency_key)
        .fetch_optional(scope.executor())
        .await?;
        if let Some(exercise_id) = existing {
            ctx.bind("dilution_exercise", exercise_id);
            return Ok(VerbExecutionOutcome::Uuid(exercise_id));
        }

        let row = sqlx::query(
            r#"
            SELECT issuer_entity_id, converts_to_share_class_id, holder_entity_id,
                   instrument_type, units_granted, units_exercised, units_forfeited,
                   COALESCE(conversion_ratio, 1.0) as conversion_ratio,
                   exercise_price, status
            FROM "ob-poc".dilution_instruments
            WHERE instrument_id = $1
            FOR UPDATE
            "#,
        )
        .bind(instrument_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Instrument {} not found", instrument_id))?;

        let issuer_entity_id: Uuid = row.get("issuer_entity_id");
        let share_class_id: Option<Uuid> = row.get("converts_to_share_class_id");
        let holder_entity_id: Option<Uuid> = row.get("holder_entity_id");
        let instrument_type: String = row.get("instrument_type");
        let units_granted: rust_decimal::Decimal = row.get("units_granted");
        let units_exercised: rust_decimal::Decimal = row.get("units_exercised");
        let units_forfeited: rust_decimal::Decimal = row.get("units_forfeited");
        let conversion_ratio: rust_decimal::Decimal = row.get("conversion_ratio");
        let exercise_price: Option<rust_decimal::Decimal> = row.get("exercise_price");
        let status: String = row.get("status");

        if status != "ACTIVE" {
            return Err(anyhow!(
                "Instrument {} is not active (status={})",
                instrument_id,
                status
            ));
        }
        let share_class_id = share_class_id
            .ok_or_else(|| anyhow!("Instrument {} has no conversion share class", instrument_id))?;
        let holder_entity_id = holder_entity_id
            .ok_or_else(|| anyhow!("Instrument {} has no holder", instrument_id))?;

        let units_outstanding = units_granted - units_exercised - units_forfeited;
        if units_to_exercise > units_outstanding {
            return Err(anyhow!(
                "Cannot exercise {} units: only {} outstanding",
                units_to_exercise,
                units_outstanding
            ));
        }

        let shares_to_issue = units_to_exercise * conversion_ratio;
        let shares_after_tax = if is_cashless {
            shares_to_issue * rust_decimal::Decimal::from_str_exact("0.6")?
        } else {
            shares_to_issue
        };
        let shares_withheld = if is_cashless {
            Some(shares_to_issue - shares_after_tax)
        } else {
            None
        };
        let actual_price = exercise_price_override.or(exercise_price);

        let new_units_exercised = units_exercised + units_to_exercise;
        let new_status = if new_units_exercised + units_forfeited >= units_granted {
            "EXERCISED"
        } else {
            "ACTIVE"
        };

        let rows = sqlx::query(
            r#"
            UPDATE "ob-poc".dilution_instruments
            SET units_exercised = $2,
                status = $3,
                updated_at = now()
            WHERE instrument_id = $1
              AND units_exercised = $4
            "#,
        )
        .bind(instrument_id)
        .bind(new_units_exercised)
        .bind(new_status)
        .bind(units_exercised)
        .execute(scope.executor())
        .await?
        .rows_affected();
        if rows == 0 {
            return Err(anyhow!("Concurrent modification detected"));
        }

        let holding_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".holdings (
                share_class_id, investor_entity_id, units,
                cost_basis, acquisition_date, status
            ) VALUES ($1, $2, $3, $4, $5, 'active')
            ON CONFLICT (share_class_id, investor_entity_id)
            DO UPDATE SET
                units = "ob-poc".holdings.units + EXCLUDED.units,
                updated_at = now()
            RETURNING id
            "#,
        )
        .bind(share_class_id)
        .bind(holder_entity_id)
        .bind(shares_after_tax)
        .bind(actual_price)
        .bind(exercise_date)
        .fetch_one(scope.executor())
        .await?;

        let exercise_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_exercise_events (
                instrument_id, units_exercised, exercise_date,
                exercise_price_paid, shares_issued, resulting_holding_id,
                is_cashless, shares_withheld_for_tax, idempotency_key,
                notes
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING exercise_id
            "#,
        )
        .bind(instrument_id)
        .bind(units_to_exercise)
        .bind(exercise_date)
        .bind(actual_price)
        .bind(shares_to_issue)
        .bind(holding_id)
        .bind(is_cashless)
        .bind(shares_withheld)
        .bind(&idempotency_key)
        .bind(format!("Exercise of {} {}", instrument_type, instrument_id))
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".share_class_supply
            SET issued_units = issued_units + $2,
                outstanding_units = outstanding_units + $2,
                updated_at = now()
            WHERE share_class_id = $1
              AND as_of_date = (SELECT MAX(as_of_date) FROM "ob-poc".share_class_supply WHERE share_class_id = $1)
            "#,
        )
        .bind(share_class_id)
        .bind(shares_after_tax)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".issuance_events (
                share_class_id, issuer_entity_id, event_type, units_delta,
                price_per_unit, effective_date, notes, status
            ) VALUES ($1, $2, 'CONVERSION', $3, $4, $5, $6, 'EFFECTIVE')
            "#,
        )
        .bind(share_class_id)
        .bind(issuer_entity_id)
        .bind(shares_after_tax)
        .bind(actual_price)
        .bind(exercise_date)
        .bind(format!(
            "Exercise of {} {} ({} units -> {} shares)",
            instrument_type, instrument_id, units_to_exercise, shares_after_tax
        ))
        .execute(scope.executor())
        .await?;

        ctx.bind("dilution_exercise", exercise_id);
        Ok(VerbExecutionOutcome::Uuid(exercise_id))
    }
}

pub struct Forfeit;

#[async_trait]
impl SemOsVerbOp for Forfeit {
    fn fqn(&self) -> &str {
        "capital.dilution.forfeit"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instrument_id = json_extract_uuid(args, ctx, "instrument-id")?;
        let units: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "units").and_then(|s| s.parse().ok());
        let forfeit_date = date_arg(args, "forfeit-date");
        let reason = json_extract_string_opt(args, "reason");

        let instrument: Option<(rust_decimal::Decimal,)> = sqlx::query_as(
            r#"
            SELECT units_outstanding
            FROM "ob-poc".dilution_instruments
            WHERE instrument_id = $1 AND status = 'OUTSTANDING'
            "#,
        )
        .bind(instrument_id)
        .fetch_optional(scope.executor())
        .await?;
        let (outstanding,) = instrument.ok_or_else(|| {
            anyhow!(
                "Instrument {} not found or not outstanding",
                instrument_id
            )
        })?;
        let units_to_forfeit = units.unwrap_or(outstanding);
        if units_to_forfeit > outstanding {
            return Err(anyhow!(
                "Cannot forfeit {} units: only {} outstanding",
                units_to_forfeit,
                outstanding
            ));
        }

        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_exercise_events (
                instrument_id, exercise_type, units, exercise_date, notes, status
            ) VALUES ($1, 'FORFEIT', $2, $3, $4, 'COMPLETED')
            RETURNING event_id
            "#,
        )
        .bind(instrument_id)
        .bind(units_to_forfeit)
        .bind(forfeit_date)
        .bind(&reason)
        .fetch_one(scope.executor())
        .await?;

        let remaining = outstanding - units_to_forfeit;
        let new_status = if remaining == rust_decimal::Decimal::ZERO {
            "FORFEITED"
        } else {
            "OUTSTANDING"
        };
        sqlx::query(
            r#"
            UPDATE "ob-poc".dilution_instruments
            SET units_outstanding = $2, status = $3, updated_at = now()
            WHERE instrument_id = $1
            "#,
        )
        .bind(instrument_id)
        .bind(remaining)
        .bind(new_status)
        .execute(scope.executor())
        .await?;

        ctx.bind("dilution_exercise", event_id);
        Ok(VerbExecutionOutcome::Uuid(event_id))
    }
}

pub struct List;

#[async_trait]
impl SemOsVerbOp for List {
    fn fqn(&self) -> &str {
        "capital.dilution.list"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let instrument_type = json_extract_string_opt(args, "instrument-type");
        let status = json_extract_string_opt(args, "status").unwrap_or_else(|| "OUTSTANDING".to_string());

        type Row13 = (
            Uuid, Uuid, Option<Uuid>, Uuid, String,
            rust_decimal::Decimal, rust_decimal::Decimal,
            Option<rust_decimal::Decimal>, Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            NaiveDate, Option<NaiveDate>, String,
        );
        let instruments: Vec<Row13> = if let Some(ref itype) = instrument_type {
            sqlx::query_as(
                r#"
                SELECT instrument_id, issuer_entity_id, share_class_id, holder_entity_id,
                       instrument_type, units_authorized, units_outstanding,
                       strike_price, principal_amount, valuation_cap,
                       grant_date, expiry_date, status
                FROM "ob-poc".dilution_instruments
                WHERE issuer_entity_id = $1 AND instrument_type = $2 AND status = $3
                ORDER BY grant_date DESC
                "#,
            )
            .bind(issuer_entity_id)
            .bind(itype)
            .bind(&status)
            .fetch_all(scope.executor())
            .await?
        } else if status == "ALL" {
            sqlx::query_as(
                r#"
                SELECT instrument_id, issuer_entity_id, share_class_id, holder_entity_id,
                       instrument_type, units_authorized, units_outstanding,
                       strike_price, principal_amount, valuation_cap,
                       grant_date, expiry_date, status
                FROM "ob-poc".dilution_instruments
                WHERE issuer_entity_id = $1
                ORDER BY grant_date DESC
                "#,
            )
            .bind(issuer_entity_id)
            .fetch_all(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT instrument_id, issuer_entity_id, share_class_id, holder_entity_id,
                       instrument_type, units_authorized, units_outstanding,
                       strike_price, principal_amount, valuation_cap,
                       grant_date, expiry_date, status
                FROM "ob-poc".dilution_instruments
                WHERE issuer_entity_id = $1 AND status = $2
                ORDER BY grant_date DESC
                "#,
            )
            .bind(issuer_entity_id)
            .bind(&status)
            .fetch_all(scope.executor())
            .await?
        };

        let mut out: Vec<Value> = Vec::with_capacity(instruments.len());
        for i in &instruments {
            let holder_name: Option<String> = sqlx::query_scalar(
                r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1 AND deleted_at IS NULL"#,
            )
            .bind(i.3)
            .fetch_optional(scope.executor())
            .await?;
            let share_class_name: Option<String> = if let Some(sc_id) = i.2 {
                sqlx::query_scalar(r#"SELECT name FROM "ob-poc".share_classes WHERE id = $1"#)
                    .bind(sc_id)
                    .fetch_optional(scope.executor())
                    .await?
            } else {
                None
            };
            out.push(json!({
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
            }));
        }
        Ok(VerbExecutionOutcome::RecordSet(out))
    }
}

pub struct GetSummary;

#[async_trait]
impl SemOsVerbOp for GetSummary {
    fn fqn(&self) -> &str {
        "capital.dilution.get-summary"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of = date_arg(args, "as-of");
        let basis = json_extract_string_opt(args, "basis")
            .unwrap_or_else(|| "EXERCISABLE".to_string());

        type SumRow = (
            Uuid, String, String,
            rust_decimal::Decimal, rust_decimal::Decimal, rust_decimal::Decimal,
            Option<rust_decimal::Decimal>,
        );
        let summary: Vec<SumRow> = sqlx::query_as(
            r#"
            SELECT issuer_entity_id, share_class_name, instrument_type,
                   units_authorized, units_outstanding, units_exercised, weighted_avg_strike
            FROM "ob-poc".v_dilution_summary
            WHERE issuer_entity_id = $1
            ORDER BY instrument_type
            "#,
        )
        .bind(issuer_entity_id)
        .fetch_all(scope.executor())
        .await?;

        let mut total_outstanding = rust_decimal::Decimal::ZERO;
        let mut total_potential_shares = rust_decimal::Decimal::ZERO;
        let summary_data: Vec<Value> = summary
            .iter()
            .map(|(_, class_name, itype, authorized, outstanding, exercised, avg_strike)| {
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
            })
            .collect();

        let outstanding_shares: rust_decimal::Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(scs.outstanding_units), 0)
            FROM "ob-poc".share_classes sc
            LEFT JOIN "ob-poc".share_class_supply scs ON scs.share_class_id = sc.id
            WHERE sc.issuer_entity_id = $1
            "#,
        )
        .bind(issuer_entity_id)
        .fetch_one(scope.executor())
        .await?;

        let fully_diluted = outstanding_shares + total_potential_shares;
        let dilution_pct = if fully_diluted > rust_decimal::Decimal::ZERO {
            (total_potential_shares / fully_diluted * rust_decimal::Decimal::from(100)).round_dp(4)
        } else {
            rust_decimal::Decimal::ZERO
        };

        Ok(VerbExecutionOutcome::Record(json!({
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
}
