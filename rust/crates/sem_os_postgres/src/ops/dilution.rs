//! Capital-structure dilution verbs (8 plugin verbs) — YAML-first
//! re-implementation of `capital.dilution.*` from
//! `rust/config/verbs/capital.yaml`.
//!
//! Rewritten 2026-08-19 (EOP-PLAN share-register board design pass, Phase 3):
//! every create/list/forfeit op referenced schema that never existed live
//! on `"ob-poc".dilution_instruments` (`share_class_id`, `units_authorized`/
//! `units_outstanding`, `strike_price`, `grant_date`, `expiry_date`,
//! `warrant_series`, `safe_type`, `interest_rate`, `maturity_date` — none of
//! these columns exist; the real names are `converts_to_share_class_id`,
//! `units_granted`, `exercise_price`, `expiration_date`, and there is no
//! grant/issue-date column at all) and wrote `status = 'OUTSTANDING'`, a
//! value the live `dilution_instruments_chk_dilution_status` CHECK
//! constraint rejects outright (valid values: ACTIVE, EXERCISED, EXPIRED,
//! FORFEITED, CANCELLED) -- every create call and `forfeit` would have
//! errored on the CHECK, every time. `exercise` was the one op already
//! correct against the live schema (including the ACTIVE/EXERCISED
//! vocabulary), so it was the reference used to correct the rest, not the
//! outlier that needed fixing. The verb YAML's args
//! (`config/verbs/capital.yaml`) were already aligned to the real column
//! names; only the Rust bodies were stale.
//!
//! Ops:
//! - `grant-options` — insert STOCK_OPTION instrument with vesting schedule
//! - `issue-warrant` — insert WARRANT instrument
//! - `create-safe` — insert SAFE (cap + discount, units_granted = 0 until a
//!   priced-round conversion -- SAFEs have no defined unit count at
//!   creation; see the migration note on `create-safe`/
//!   `create-convertible-note` below)
//! - `create-convertible-note` — insert CONVERTIBLE_NOTE (units_granted = 0,
//!   same reasoning)
//! - `exercise` — atomically convert instrument → shares with
//!   FOR UPDATE lock + optimistic check + idempotency key. The retry
//!   loop present in the legacy impl is dropped: under the Sequencer
//!   scope, the surrounding transaction owns retry semantics, and a
//!   serialization conflict simply aborts the verb step (which the
//!   runbook compiler can replay).
//! - `forfeit` — reduce outstanding units (units_forfeited), terminal
//!   FORFEITED status on full consumption; the forfeiture reason is
//!   recorded on the instrument's own `notes` column -- there is no
//!   separate forfeiture-event table, and `dilution_exercise_events`'
//!   NOT NULL `units_exercised`/`shares_issued` columns don't fit a
//!   zero-conversion event without fabricating misleading rows in a table
//!   named for a different kind of event.
//! - `list` — filter by instrument_type + status
//! - `get-summary` — aggregate dilution % against supply (direct query;
//!   the `v_dilution_summary` view this used to read never existed live)

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

fn date_arg(args: &Value, arg_name: &str) -> NaiveDate {
    json_extract_string_opt(args, arg_name)
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Utc::now().date_naive())
}

fn opt_date_arg(args: &Value, arg_name: &str) -> Option<NaiveDate> {
    json_extract_string_opt(args, arg_name)
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}

fn decimal_arg(args: &Value, arg_name: &str) -> Result<rust_decimal::Decimal> {
    json_extract_string_opt(args, arg_name)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("{} is required", arg_name))
}

fn opt_decimal_arg(args: &Value, arg_name: &str) -> Option<rust_decimal::Decimal> {
    json_extract_string_opt(args, arg_name).and_then(|s| s.parse().ok())
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
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let converts_to_share_class_id =
            json_extract_uuid(args, ctx, "converts-to-share-class-id")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder-entity-id")?;
        let units = decimal_arg(args, "units")?;
        let exercise_price = decimal_arg(args, "exercise-price")?;
        let exercise_currency =
            json_extract_string_opt(args, "exercise-currency").unwrap_or_else(|| "USD".into());
        let vesting_start_date = opt_date_arg(args, "vesting-start-date");
        let vesting_end_date = opt_date_arg(args, "vesting-end-date");
        let vesting_cliff_months = json_extract_int_opt(args, "vesting-cliff-months")
            .map(|i| i as i32)
            .unwrap_or(12);
        let expiration_date = date_arg(args, "expiration-date");
        let plan_name = json_extract_string_opt(args, "plan-name");

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_instruments (
                issuer_entity_id, converts_to_share_class_id, holder_entity_id,
                instrument_type, units_granted, exercise_price, exercise_currency,
                vesting_start_date, vesting_end_date, vesting_cliff_months,
                expiration_date, plan_name
            ) VALUES ($1, $2, $3, 'STOCK_OPTION', $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING instrument_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(converts_to_share_class_id)
        .bind(holder_entity_id)
        .bind(units)
        .bind(exercise_price)
        .bind(&exercise_currency)
        .bind(vesting_start_date)
        .bind(vesting_end_date)
        .bind(vesting_cliff_months)
        .bind(expiration_date)
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
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let converts_to_share_class_id =
            json_extract_uuid(args, ctx, "converts-to-share-class-id")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder-entity-id")?;
        let units = decimal_arg(args, "units")?;
        let exercise_price = decimal_arg(args, "exercise-price")?;
        let exercisable_from = opt_date_arg(args, "exercisable-from");
        let expiration_date = date_arg(args, "expiration-date");

        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_instruments (
                issuer_entity_id, converts_to_share_class_id, holder_entity_id,
                instrument_type, units_granted, exercise_price, exercisable_from,
                expiration_date
            ) VALUES ($1, $2, $3, 'WARRANT', $4, $5, $6, $7)
            RETURNING instrument_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(converts_to_share_class_id)
        .bind(holder_entity_id)
        .bind(units)
        .bind(exercise_price)
        .bind(exercisable_from)
        .bind(expiration_date)
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
        let converts_to_share_class_id =
            json_extract_uuid_opt(args, ctx, "converts-to-share-class-id");
        let holder_entity_id = json_extract_uuid(args, ctx, "holder-entity-id")?;
        let principal_amount = decimal_arg(args, "principal-amount")?;
        let valuation_cap = opt_decimal_arg(args, "valuation-cap");
        let discount_pct = opt_decimal_arg(args, "discount-pct");

        // units_granted is NOT NULL with a CHECK (>= 0) on the live table --
        // a SAFE has no defined share count until it converts in a priced
        // round, so 0 is the correct "not yet determined" value, not a
        // placeholder. No conversion capability for SAFEs/notes exists yet
        // (out of scope this pass); `exercise` only handles instruments
        // that already carry a real units_granted (options/warrants).
        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_instruments (
                issuer_entity_id, converts_to_share_class_id, holder_entity_id,
                instrument_type, units_granted, principal_amount, valuation_cap,
                discount_pct
            ) VALUES ($1, $2, $3, 'SAFE', 0, $4, $5, $6)
            RETURNING instrument_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(converts_to_share_class_id)
        .bind(holder_entity_id)
        .bind(principal_amount)
        .bind(valuation_cap)
        .bind(discount_pct)
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
        let converts_to_share_class_id =
            json_extract_uuid_opt(args, ctx, "converts-to-share-class-id");
        let holder_entity_id = json_extract_uuid(args, ctx, "holder-entity-id")?;
        let principal_amount = decimal_arg(args, "principal-amount")?;
        let valuation_cap = opt_decimal_arg(args, "valuation-cap");
        let discount_pct = opt_decimal_arg(args, "discount-pct");
        let expiration_date = date_arg(args, "expiration-date");

        // units_granted = 0: same reasoning as create-safe above -- a
        // convertible note has no defined share count until conversion.
        let instrument_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".dilution_instruments (
                issuer_entity_id, converts_to_share_class_id, holder_entity_id,
                instrument_type, units_granted, principal_amount, valuation_cap,
                discount_pct, expiration_date
            ) VALUES ($1, $2, $3, 'CONVERTIBLE_NOTE', 0, $4, $5, $6, $7)
            RETURNING instrument_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(converts_to_share_class_id)
        .bind(holder_entity_id)
        .bind(principal_amount)
        .bind(valuation_cap)
        .bind(discount_pct)
        .bind(expiration_date)
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
        let units_to_exercise: rust_decimal::Decimal = json_extract_string_opt(args, "units")
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
        let units_to_forfeit = decimal_arg(args, "units")?;
        let forfeit_date = date_arg(args, "forfeit-date");
        let reason = json_extract_string_opt(args, "reason");

        if units_to_forfeit <= rust_decimal::Decimal::ZERO {
            return Err(anyhow!("units must be positive"));
        }

        let row = sqlx::query(
            r#"
            SELECT units_granted, units_exercised, units_forfeited, status
            FROM "ob-poc".dilution_instruments
            WHERE instrument_id = $1
            FOR UPDATE
            "#,
        )
        .bind(instrument_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Instrument {} not found", instrument_id))?;

        let units_granted: rust_decimal::Decimal = row.get("units_granted");
        let units_exercised: rust_decimal::Decimal = row.get("units_exercised");
        let units_forfeited: rust_decimal::Decimal = row.get("units_forfeited");
        let status: String = row.get("status");

        if status != "ACTIVE" {
            return Err(anyhow!(
                "Instrument {} is not active (status={})",
                instrument_id,
                status
            ));
        }

        let outstanding = units_granted - units_exercised - units_forfeited;
        if units_to_forfeit > outstanding {
            return Err(anyhow!(
                "Cannot forfeit {} units: only {} outstanding",
                units_to_forfeit,
                outstanding
            ));
        }

        let new_units_forfeited = units_forfeited + units_to_forfeit;
        let new_status = if units_exercised + new_units_forfeited >= units_granted {
            "FORFEITED"
        } else {
            "ACTIVE"
        };
        let note = format!(
            "FORFEIT {} units on {}{}",
            units_to_forfeit,
            forfeit_date,
            reason
                .map(|r| format!(": {r}"))
                .unwrap_or_default()
        );

        // No dedicated forfeiture-event table exists -- dilution_exercise_events'
        // NOT NULL units_exercised/shares_issued columns describe a real
        // conversion, not a forfeiture, so a synthetic zero-value row there
        // would misrepresent the instrument's history. The reason is
        // recorded on the instrument's own notes column instead.
        let rows = sqlx::query(
            r#"
            UPDATE "ob-poc".dilution_instruments
            SET units_forfeited = $2,
                status = $3,
                notes = COALESCE(notes || E'\n', '') || $4,
                updated_at = now()
            WHERE instrument_id = $1
              AND units_forfeited = $5
            "#,
        )
        .bind(instrument_id)
        .bind(new_units_forfeited)
        .bind(new_status)
        .bind(&note)
        .bind(units_forfeited)
        .execute(scope.executor())
        .await?
        .rows_affected();
        if rows == 0 {
            return Err(anyhow!("Concurrent modification detected"));
        }

        ctx.bind("dilution_instrument", instrument_id);
        Ok(VerbExecutionOutcome::Affected(rows))
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
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, _ctx, "issuer-entity-id")?;
        let instrument_type = json_extract_string_opt(args, "instrument-type")
            .filter(|t| t != "ALL");
        let status =
            json_extract_string_opt(args, "status").unwrap_or_else(|| "ACTIVE".to_string());

        type Row12 = (
            Uuid,
            Uuid,
            Option<Uuid>,
            Option<Uuid>,
            String,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            Option<NaiveDate>,
            String,
        );
        let instruments: Vec<Row12> = if let Some(ref itype) = instrument_type {
            sqlx::query_as(
                r#"
                SELECT instrument_id, issuer_entity_id, converts_to_share_class_id,
                       holder_entity_id, instrument_type, units_granted, units_exercised,
                       units_forfeited, exercise_price, principal_amount,
                       expiration_date, status
                FROM "ob-poc".dilution_instruments
                WHERE issuer_entity_id = $1 AND instrument_type = $2 AND status = $3
                ORDER BY created_at DESC
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
                SELECT instrument_id, issuer_entity_id, converts_to_share_class_id,
                       holder_entity_id, instrument_type, units_granted, units_exercised,
                       units_forfeited, exercise_price, principal_amount,
                       expiration_date, status
                FROM "ob-poc".dilution_instruments
                WHERE issuer_entity_id = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(issuer_entity_id)
            .fetch_all(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT instrument_id, issuer_entity_id, converts_to_share_class_id,
                       holder_entity_id, instrument_type, units_granted, units_exercised,
                       units_forfeited, exercise_price, principal_amount,
                       expiration_date, status
                FROM "ob-poc".dilution_instruments
                WHERE issuer_entity_id = $1 AND status = $2
                ORDER BY created_at DESC
                "#,
            )
            .bind(issuer_entity_id)
            .bind(&status)
            .fetch_all(scope.executor())
            .await?
        };

        let mut out: Vec<Value> = Vec::with_capacity(instruments.len());
        for i in &instruments {
            let holder_name: Option<String> = match i.3 {
                Some(holder_id) => {
                    sqlx::query_scalar(
                        r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1 AND deleted_at IS NULL"#,
                    )
                    .bind(holder_id)
                    .fetch_optional(scope.executor())
                    .await?
                }
                None => None,
            };
            let share_class_name: Option<String> = if let Some(sc_id) = i.2 {
                sqlx::query_scalar(r#"SELECT name FROM "ob-poc".share_classes WHERE id = $1"#)
                    .bind(sc_id)
                    .fetch_optional(scope.executor())
                    .await?
            } else {
                None
            };
            let units_outstanding = i.5 - i.6 - i.7;
            out.push(json!({
                "instrument_id": i.0,
                "share_class_id": i.2,
                "share_class_name": share_class_name,
                "holder_entity_id": i.3,
                "holder_name": holder_name,
                "instrument_type": i.4,
                "units_granted": i.5.to_string(),
                "units_outstanding": units_outstanding.to_string(),
                "exercise_price": i.8.map(|d| d.to_string()),
                "principal_amount": i.9.map(|d| d.to_string()),
                "expiration_date": i.10.map(|d| d.to_string()),
                "status": i.11
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
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, _ctx, "issuer-entity-id")?;

        // Direct aggregate query -- "ob-poc".v_dilution_summary never
        // existed live (the historical migration defined it under the old
        // kyc. schema, referencing share_classes.issued_shares, a column
        // that also never existed; it was never carried over the schema
        // rename).
        type SumRow = (
            String,
            Option<String>,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            Option<rust_decimal::Decimal>,
        );
        let summary: Vec<SumRow> = sqlx::query_as(
            r#"
            SELECT di.instrument_type,
                   sc.name AS share_class_name,
                   SUM(di.units_granted) AS units_granted,
                   SUM(di.units_exercised) AS units_exercised,
                   SUM(di.units_granted - di.units_exercised - di.units_forfeited) AS units_outstanding,
                   (SUM(di.exercise_price * di.units_granted) FILTER (WHERE di.exercise_price IS NOT NULL))
                     / NULLIF(SUM(di.units_granted) FILTER (WHERE di.exercise_price IS NOT NULL), 0)
                     AS weighted_avg_strike
            FROM "ob-poc".dilution_instruments di
            LEFT JOIN "ob-poc".share_classes sc ON sc.id = di.converts_to_share_class_id
            WHERE di.issuer_entity_id = $1 AND di.status = 'ACTIVE'
            GROUP BY di.instrument_type, sc.name
            ORDER BY di.instrument_type
            "#,
        )
        .bind(issuer_entity_id)
        .fetch_all(scope.executor())
        .await?;

        let mut total_outstanding = rust_decimal::Decimal::ZERO;
        let summary_data: Vec<Value> = summary
            .iter()
            .map(
                |(itype, class_name, granted, exercised, outstanding, avg_strike)| {
                    total_outstanding += outstanding;
                    json!({
                        "instrument_type": itype,
                        "share_class_name": class_name,
                        "units_granted": granted.to_string(),
                        "units_exercised": exercised.to_string(),
                        "units_outstanding": outstanding.to_string(),
                        "weighted_avg_strike": avg_strike.map(|d| d.to_string())
                    })
                },
            )
            .collect();

        let outstanding_shares: rust_decimal::Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(scs.outstanding_units), 0)
            FROM "ob-poc".share_classes sc
            LEFT JOIN LATERAL (
                SELECT outstanding_units FROM "ob-poc".share_class_supply
                WHERE share_class_id = sc.id
                ORDER BY as_of_date DESC
                LIMIT 1
            ) scs ON true
            WHERE sc.issuer_entity_id = $1 AND sc.lifecycle_status <> 'LIQUIDATED'
            "#,
        )
        .bind(issuer_entity_id)
        .fetch_one(scope.executor())
        .await?;

        let fully_diluted = outstanding_shares + total_outstanding;
        let dilution_pct = if fully_diluted > rust_decimal::Decimal::ZERO {
            (total_outstanding / fully_diluted * rust_decimal::Decimal::from(100)).round_dp(4)
        } else {
            rust_decimal::Decimal::ZERO
        };

        Ok(VerbExecutionOutcome::Record(json!({
            "issuer_entity_id": issuer_entity_id,
            "current_outstanding_shares": outstanding_shares.to_string(),
            "total_dilution_instruments_outstanding": total_outstanding.to_string(),
            "fully_diluted_shares": fully_diluted.to_string(),
            "dilution_pct": dilution_pct.to_string(),
            "by_instrument_type": summary_data
        })))
    }
}
