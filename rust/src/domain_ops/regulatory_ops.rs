//! Regulatory Registration Operations
//!
//! Plugin operations for multi-regulator entity registration management.
//! Supports dual-regulation, passporting, and verification workflows.

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Verify a regulatory registration
///
/// Updates the verification status of an existing registration.
/// Requires: entity has registration with the specified regulator.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use chrono::NaiveDate;
        use uuid::Uuid;

        // Extract entity-id (can be @reference, UUID, or string)
        let entity_id_arg = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .ok_or_else(|| anyhow!("entity-id is required"))?;

        let entity_id: Uuid = if let Some(ref_name) = entity_id_arg.value.as_symbol() {
            ctx.resolve(ref_name)
                .ok_or_else(|| anyhow!("Unresolved reference @{}", ref_name))?
        } else if let Some(uuid_val) = entity_id_arg.value.as_uuid() {
            uuid_val
        } else if let Some(str_val) = entity_id_arg.value.as_string() {
            Uuid::parse_str(str_val)
                .map_err(|_| anyhow!("Invalid UUID format for entity-id: {}", str_val))?
        } else {
            return Err(anyhow!("entity-id must be a @reference, UUID, or string"));
        };

        let regulator = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "regulator")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("regulator is required"))?
            .to_string();

        let method = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "method")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("method is required"))?
            .to_string();

        let reference = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reference")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let expires = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "expires")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        // Update the registration
        let result = sqlx::query!(
            r#"
            UPDATE ob_kyc.entity_regulatory_registrations
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
            Some(row) => Ok(ExecutionResult::Uuid(row.registration_id)),
            None => Err(anyhow!(
                "No active registration found for entity {} with regulator {}",
                entity_id,
                regulator
            )),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

/// Check entity's overall regulatory status
///
/// Aggregates all registrations for an entity and returns a summary
/// including whether simplified DD is allowed.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        // Extract entity-id (can be @reference, UUID, or string)
        let entity_id_arg = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .ok_or_else(|| anyhow!("entity-id is required"))?;

        let entity_id: Uuid = if let Some(ref_name) = entity_id_arg.value.as_symbol() {
            ctx.resolve(ref_name)
                .ok_or_else(|| anyhow!("Unresolved reference @{}", ref_name))?
        } else if let Some(uuid_val) = entity_id_arg.value.as_uuid() {
            uuid_val
        } else if let Some(str_val) = entity_id_arg.value.as_string() {
            Uuid::parse_str(str_val)
                .map_err(|_| anyhow!("Invalid UUID format for entity-id: {}", str_val))?
        } else {
            return Err(anyhow!("entity-id must be a @reference, UUID, or string"));
        };

        // Query the summary view
        let summary = sqlx::query!(
            r#"
            SELECT
                entity_id,
                entity_name,
                registration_count,
                verified_count,
                allows_simplified_dd,
                active_regulators,
                verified_regulators,
                last_verified,
                next_expiry
            FROM ob_kyc.v_entity_regulatory_summary
            WHERE entity_id = $1
            "#,
            entity_id
        )
        .fetch_optional(pool)
        .await?;

        // Get detailed registrations
        let registrations = sqlx::query!(
            r#"
            SELECT
                r.regulator_code,
                r.registration_type,
                r.registration_number,
                r.registration_verified,
                r.verification_date,
                r.verification_expires,
                r.status,
                reg.regulator_name,
                reg.regulatory_tier
            FROM ob_kyc.entity_regulatory_registrations r
            JOIN ob_ref.regulators reg ON r.regulator_code = reg.regulator_code
            WHERE r.entity_id = $1 AND r.status = 'ACTIVE'
            ORDER BY r.registration_type
            "#,
            entity_id
        )
        .fetch_all(pool)
        .await?;

        let result = match summary {
            Some(s) => json!({
                "entity_id": entity_id,
                "entity_name": s.entity_name,
                "is_regulated": s.registration_count.unwrap_or(0) > 0,
                "registration_count": s.registration_count.unwrap_or(0),
                "verified_count": s.verified_count.unwrap_or(0),
                "allows_simplified_dd": s.allows_simplified_dd.unwrap_or(false),
                "active_regulators": s.active_regulators,
                "verified_regulators": s.verified_regulators,
                "last_verified": s.last_verified,
                "next_verification_due": s.next_expiry,
                "registrations": registrations.iter().map(|r| json!({
                    "regulator": r.regulator_code,
                    "regulator_name": r.regulator_name,
                    "type": r.registration_type,
                    "registration_number": r.registration_number,
                    "verified": r.registration_verified,
                    "verification_date": r.verification_date,
                    "tier": r.regulatory_tier,
                    "expires": r.verification_expires
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

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
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
