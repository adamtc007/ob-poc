//! Regulatory Registration Operations
//!
//! Plugin operations for multi-regulator entity registration management.
//! Supports dual-regulation, passporting, and verification workflows.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{json_extract_string, json_extract_string_opt, json_extract_uuid};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

// ── Shared _impl functions ──

async fn registration_verify_impl(
    entity_id: uuid::Uuid,
    regulator: &str,
    method: &str,
    reference: Option<&str>,
    expires: Option<chrono::NaiveDate>,
    pool: &PgPool,
) -> Result<uuid::Uuid> {
    let result = sqlx::query!(
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
        entity_id,
        regulator,
        method,
        reference,
        expires
    )
    .fetch_optional(pool)
    .await?;

    match result {
        Some(row) => Ok(row.registration_id),
        None => Err(anyhow!(
            "No active registration found for entity {} with regulator {}",
            entity_id,
            regulator
        )),
    }
}

async fn regulatory_status_check_impl(
    entity_id: uuid::Uuid,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    use serde_json::json;
    use sqlx::Row;

    let summary: Option<(
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<bool>,
        Option<Vec<String>>,
        Option<Vec<String>>,
        Option<chrono::NaiveDate>,
        Option<chrono::NaiveDate>,
    )> = sqlx::query_as(
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
    .fetch_optional(pool)
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
    .fetch_all(pool)
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
                "expires": r.get::<Option<chrono::NaiveDate>, _>("verification_expires")
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
            "registrations": []
        }),
    };

    Ok(result)
}

/// Verify a regulatory registration
///
/// Updates the verification status of an existing registration.
/// Requires: entity has registration with the specified regulator.
#[register_custom_op]
pub struct RegistrationVerifyOp;

#[async_trait]
impl CustomOperation for RegistrationVerifyOp {
    fn domain(&self) -> &'static str {
        "regulatory.registration"
    }

    fn verb(&self) -> &'static str {
        "verify"
    }

    fn rationale(&self) -> &'static str {
        "Updates multiple columns atomically and validates registration exists"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let regulator = json_extract_string(args, "regulator")?;
        let method = json_extract_string(args, "method")?;
        let reference = json_extract_string_opt(args, "reference");
        let expires_str = json_extract_string_opt(args, "expires");
        let expires = expires_str
            .as_deref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let registration_id = registration_verify_impl(
            entity_id,
            &regulator,
            &method,
            reference.as_deref(),
            expires,
            pool,
        )
        .await?;

        Ok(VerbExecutionOutcome::Uuid(registration_id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Check entity's overall regulatory status
///
/// Aggregates all registrations for an entity and returns a summary
/// including whether simplified DD is allowed.
#[register_custom_op]
pub struct RegulatoryStatusCheckOp;

#[async_trait]
impl CustomOperation for RegulatoryStatusCheckOp {
    fn domain(&self) -> &'static str {
        "regulatory.status"
    }

    fn verb(&self) -> &'static str {
        "check"
    }

    fn rationale(&self) -> &'static str {
        "Aggregates multiple registrations and computes derived properties"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let result = regulatory_status_check_impl(entity_id, pool).await?;
        Ok(VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registration_verify_op_metadata() {
        let op = RegistrationVerifyOp;
        assert_eq!(op.domain(), "regulatory.registration");
        assert_eq!(op.verb(), "verify");
    }

    #[test]
    fn test_regulatory_status_check_op_metadata() {
        let op = RegulatoryStatusCheckOp;
        assert_eq!(op.domain(), "regulatory.status");
        assert_eq!(op.verb(), "check");
    }
}
