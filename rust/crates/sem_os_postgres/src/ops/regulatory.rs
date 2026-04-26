//! Regulatory registration verbs (2 plugin verbs) — SemOS-side
//! YAML-first re-implementation of the plugin subset of
//! `rust/config/verbs/kyc/regulatory.yaml`.
//!
//! `regulatory.registration.verify` toggles a registration to
//! verified + timestamp. `regulatory.status.check` aggregates
//! the `v_entity_regulatory_summary` view + current registrations
//! into one regulatory snapshot record.
//!
//! Slice #10 pattern: `sqlx::query!` macros rewritten as runtime
//! `sqlx::query_as` so we dodge the sqlx-offline cache entirely.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── regulatory.registration.verify ────────────────────────────────────────────

pub struct RegistrationVerify;

#[async_trait]
impl SemOsVerbOp for RegistrationVerify {
    fn fqn(&self) -> &str {
        "regulatory.registration.verify"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let regulator = json_extract_string(args, "regulator")?;
        let method = json_extract_string(args, "method")?;
        let reference = json_extract_string_opt(args, "reference");
        let expires_str = json_extract_string_opt(args, "expires");
        let expires = expires_str
            .as_deref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"
            UPDATE "ob-poc".entity_regulatory_registrations
            SET
                registration_verified = TRUE,
                verification_date = CURRENT_DATE,
                verification_method = $3,
                verification_reference = $4,
                verification_expires = $5,
                updated_at = NOW()
            WHERE entity_id = $1 AND regulator_code = $2 AND status = 'ACTIVE'
            RETURNING registration_id
            "#,
        )
        .bind(entity_id)
        .bind(&regulator)
        .bind(&method)
        .bind(reference.as_deref())
        .bind(expires)
        .fetch_optional(scope.executor())
        .await?;

        match row {
            Some((registration_id,)) => Ok(VerbExecutionOutcome::Uuid(registration_id)),
            None => Err(anyhow!(
                "No active registration found for entity {entity_id} with regulator {regulator}"
            )),
        }
    }
}

// ── regulatory.status.check ───────────────────────────────────────────────────

pub struct StatusCheck;

#[async_trait]
impl SemOsVerbOp for StatusCheck {
    fn fqn(&self) -> &str {
        "regulatory.status.check"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;

        type SummaryRow = (
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<bool>,
            Option<Vec<String>>,
            Option<Vec<String>>,
            Option<chrono::NaiveDate>,
            Option<chrono::NaiveDate>,
        );

        let summary: Option<SummaryRow> = sqlx::query_as(
            r#"
            SELECT
                entity_name,
                registration_count,
                verified_count,
                allows_simplified_dd,
                active_regulators,
                verified_regulators,
                last_verified,
                next_expiry
            FROM "ob-poc".v_entity_regulatory_summary
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(scope.executor())
        .await?;

        let registrations = sqlx::query(
            r#"
            SELECT
                r.regulator_code,
                r.registration_type,
                r.registration_number,
                r.registration_verified,
                r.verification_date,
                r.verification_expires,
                r.status,
                reg.name AS regulator_name,
                reg.tier AS regulatory_tier
            FROM "ob-poc".entity_regulatory_registrations r
            JOIN "ob-poc".regulators reg ON r.regulator_code = reg.regulator_code
            WHERE r.entity_id = $1 AND r.status = 'ACTIVE'
            ORDER BY r.registration_type
            "#,
        )
        .bind(entity_id)
        .fetch_all(scope.executor())
        .await?;

        let result = match summary {
            Some((
                entity_name,
                registration_count,
                verified_count,
                allows_simplified_dd,
                active_regulators,
                verified_regulators,
                last_verified,
                next_expiry,
            )) => json!({
                "entity_id": entity_id,
                "entity_name": entity_name,
                "is_regulated": registration_count.unwrap_or(0) > 0,
                "registration_count": registration_count.unwrap_or(0),
                "verified_count": verified_count.unwrap_or(0),
                "allows_simplified_dd": allows_simplified_dd.unwrap_or(false),
                "active_regulators": active_regulators,
                "verified_regulators": verified_regulators,
                "last_verified": last_verified,
                "next_verification_due": next_expiry,
                "registrations": registrations.iter().map(|r| json!({
                    "regulator": r.get::<String, _>("regulator_code"),
                    "regulator_name": r.get::<Option<String>, _>("regulator_name"),
                    "type": r.get::<Option<String>, _>("registration_type"),
                    "registration_number": r.get::<Option<String>, _>("registration_number"),
                    "verified": r.get::<Option<bool>, _>("registration_verified"),
                    "verification_date": r.get::<Option<chrono::NaiveDate>, _>("verification_date"),
                    "tier": r.get::<Option<String>, _>("regulatory_tier"),
                    "expires": r.get::<Option<chrono::NaiveDate>, _>("verification_expires"),
                })).collect::<Vec<_>>()
            }),
            None => json!({
                "entity_id": entity_id,
                "is_regulated": false,
                "registration_count": 0,
                "verified_count": 0,
                "allows_simplified_dd": false,
                "active_regulators": [],
                "verified_regulators": [],
                "registrations": [],
            }),
        };

        Ok(VerbExecutionOutcome::Record(result))
    }
}
