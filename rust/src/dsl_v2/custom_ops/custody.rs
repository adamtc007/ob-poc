//! Custody Domain Custom Operations
//!
//! Plugin handlers for custody operations that cannot be expressed as
//! data-driven verb definitions:
//! - SSI lookup for trade routing (ALERT-style matching)
//! - Booking coverage validation
//! - Sub-custodian network lookup

use anyhow::Result;
use async_trait::async_trait;

use super::{CustomOperation, ExecutionContext, ExecutionResult};
use crate::dsl_v2::ast::VerbCall;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Sub-custodian Lookup
// ============================================================================

/// Find sub-custodian for market/currency combination
///
/// Rationale: Requires date-effective lookup with fallback logic
pub struct SubcustodianLookupOp;

#[async_trait]
impl CustomOperation for SubcustodianLookupOp {
    fn domain(&self) -> &'static str {
        "subcustodian"
    }
    fn verb(&self) -> &'static str {
        "lookup"
    }
    fn rationale(&self) -> &'static str {
        "Requires date-effective lookup with primary/fallback logic"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        // Get market MIC
        let market = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("market"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing market argument"))?;

        // Get currency
        let currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("currency"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing currency argument"))?;

        // Get as-of date (default to today)
        let as_of_date: Option<chrono::NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("as-of-date"))
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        // Look up sub-custodian
        let row = sqlx::query!(
            r#"
            SELECT
                sn.network_id,
                sn.subcustodian_bic,
                sn.subcustodian_name,
                sn.local_agent_bic,
                sn.local_agent_account,
                sn.place_of_settlement_bic as pset_bic,
                sn.csd_participant_id,
                sn.is_primary
            FROM custody.subcustodian_network sn
            JOIN custody.markets m ON m.market_id = sn.market_id
            WHERE m.mic = $1
              AND sn.currency = $2
              AND sn.is_active = true
              AND sn.effective_date <= COALESCE($3, CURRENT_DATE)
              AND (sn.expiry_date IS NULL OR sn.expiry_date > COALESCE($3, CURRENT_DATE))
            ORDER BY sn.is_primary DESC, sn.effective_date DESC
            LIMIT 1
            "#,
            market,
            currency,
            as_of_date
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(ExecutionResult::Record(json!({
                "network_id": r.network_id,
                "subcustodian_bic": r.subcustodian_bic,
                "subcustodian_name": r.subcustodian_name,
                "local_agent_bic": r.local_agent_bic,
                "local_agent_account": r.local_agent_account,
                "pset_bic": r.pset_bic,
                "csd_participant_id": r.csd_participant_id,
                "is_primary": r.is_primary
            }))),
            None => Err(anyhow::anyhow!(
                "No sub-custodian found for market {} currency {}",
                market,
                currency
            )),
        }
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

// ============================================================================
// SSI Lookup for Trade (ALERT-style)
// ============================================================================

/// Find SSI for given trade characteristics using ALERT-style priority matching
///
/// Rationale: Requires complex rule matching with wildcards and priority ordering
pub struct LookupSsiForTradeOp;

#[async_trait]
impl CustomOperation for LookupSsiForTradeOp {
    fn domain(&self) -> &'static str {
        "cbu-custody"
    }
    fn verb(&self) -> &'static str {
        "lookup-ssi"
    }
    fn rationale(&self) -> &'static str {
        "Requires ALERT-style priority-based rule matching with wildcards"
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

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Get instrument class code
        let instrument_class = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("instrument-class"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing instrument-class argument"))?;

        // Get optional security type code
        let security_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("security-type"))
            .and_then(|a| a.value.as_string());

        // Get optional market MIC
        let market = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("market"))
            .and_then(|a| a.value.as_string());

        // Get currency (required)
        let currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("currency"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing currency argument"))?;

        // Get optional settlement type
        let settlement_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("settlement-type"))
            .and_then(|a| a.value.as_string());

        // Get optional counterparty BIC (we'd need to look up entity)
        let _counterparty_bic = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("counterparty-bic"))
            .and_then(|a| a.value.as_string());

        // Look up instrument class ID
        let class_id: Option<Uuid> =
            sqlx::query_scalar("SELECT class_id FROM custody.instrument_classes WHERE code = $1")
                .bind(instrument_class)
                .fetch_optional(pool)
                .await?;

        let class_id = class_id
            .ok_or_else(|| anyhow::anyhow!("Unknown instrument class: {}", instrument_class))?;

        // Look up security type ID if provided
        let security_type_id: Option<Uuid> = if let Some(st) = security_type {
            sqlx::query_scalar(
                "SELECT security_type_id FROM custody.security_types WHERE code = $1",
            )
            .bind(st)
            .fetch_optional(pool)
            .await?
        } else {
            None
        };

        // Look up market ID if provided
        let market_id: Option<Uuid> = if let Some(m) = market {
            sqlx::query_scalar("SELECT market_id FROM custody.markets WHERE mic = $1")
                .bind(m)
                .fetch_optional(pool)
                .await?
        } else {
            None
        };

        // Use the database function for matching
        let row = sqlx::query!(
            r#"
            SELECT
                ssi_id,
                ssi_name,
                rule_id,
                rule_name,
                rule_priority,
                specificity_score
            FROM custody.find_ssi_for_trade($1, $2, $3, $4, $5, $6, NULL)
            "#,
            cbu_id,
            class_id,
            security_type_id,
            market_id,
            currency,
            settlement_type
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(ExecutionResult::Record(json!({
                "ssi_id": r.ssi_id,
                "ssi_name": r.ssi_name,
                "matched_rule": r.rule_name,
                "rule_id": r.rule_id,
                "rule_priority": r.rule_priority,
                "specificity_score": r.specificity_score
            }))),
            None => Err(anyhow::anyhow!(
                "No SSI found for CBU {} with instrument class {} currency {}",
                cbu_id,
                instrument_class,
                currency
            )),
        }
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

// ============================================================================
// Validate Booking Coverage
// ============================================================================

/// Validate that all universe entries have matching booking rules
///
/// Rationale: Requires joining universe with booking rules and checking coverage
pub struct ValidateBookingCoverageOp;

#[async_trait]
impl CustomOperation for ValidateBookingCoverageOp {
    fn domain(&self) -> &'static str {
        "cbu-custody"
    }
    fn verb(&self) -> &'static str {
        "validate-booking-coverage"
    }
    fn rationale(&self) -> &'static str {
        "Requires complex join between universe and booking rules to find gaps"
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

        eprintln!("DEBUG: ValidateBookingCoverageOp::execute ENTERED");

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        eprintln!("DEBUG: cbu_id = {}", cbu_id);

        // Find universe entries without matching booking rules
        let gaps = sqlx::query!(
            r#"
            SELECT
                u.universe_id,
                ic.code as instrument_class,
                m.mic as market,
                u.currencies,
                u.settlement_types
            FROM custody.cbu_instrument_universe u
            JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
            LEFT JOIN custody.markets m ON m.market_id = u.market_id
            WHERE u.cbu_id = $1
              AND u.is_active = true
              AND NOT EXISTS (
                  SELECT 1 FROM custody.ssi_booking_rules r
                  WHERE r.cbu_id = u.cbu_id
                    AND r.is_active = true
                    AND (r.instrument_class_id IS NULL OR r.instrument_class_id = u.instrument_class_id)
                    AND (r.market_id IS NULL OR r.market_id = u.market_id)
              )
            "#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        // Find orphan rules (rules that don't match any universe entry)
        let orphans = sqlx::query!(
            r#"
            SELECT
                r.rule_id,
                r.rule_name,
                ic.code as instrument_class,
                m.mic as market
            FROM custody.ssi_booking_rules r
            LEFT JOIN custody.instrument_classes ic ON ic.class_id = r.instrument_class_id
            LEFT JOIN custody.markets m ON m.market_id = r.market_id
            WHERE r.cbu_id = $1
              AND r.is_active = true
              AND r.instrument_class_id IS NOT NULL
              AND NOT EXISTS (
                  SELECT 1 FROM custody.cbu_instrument_universe u
                  WHERE u.cbu_id = r.cbu_id
                    AND u.is_active = true
                    AND u.instrument_class_id = r.instrument_class_id
                    AND (r.market_id IS NULL OR u.market_id = r.market_id)
              )
            "#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let gap_list: Vec<serde_json::Value> = gaps
            .iter()
            .map(|g| {
                json!({
                    "universe_id": g.universe_id,
                    "instrument_class": g.instrument_class,
                    "market": g.market,
                    "currencies": g.currencies,
                    "settlement_types": g.settlement_types
                })
            })
            .collect();

        let orphan_list: Vec<serde_json::Value> = orphans
            .iter()
            .map(|o| {
                json!({
                    "rule_id": o.rule_id,
                    "rule_name": o.rule_name,
                    "instrument_class": o.instrument_class,
                    "market": o.market
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "complete": gap_list.is_empty(),
            "gaps": gap_list,
            "orphan_rules": orphan_list
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "complete": true,
            "gaps": [],
            "orphan_rules": []
        })))
    }
}

// ============================================================================
// Derive Required Coverage
// ============================================================================

/// Compare universe to booking rules and identify what coverage is needed
///
/// Rationale: Requires analyzing universe and generating coverage requirements
pub struct DeriveRequiredCoverageOp;

#[async_trait]
impl CustomOperation for DeriveRequiredCoverageOp {
    fn domain(&self) -> &'static str {
        "cbu-custody"
    }
    fn verb(&self) -> &'static str {
        "derive-required-coverage"
    }
    fn rationale(&self) -> &'static str {
        "Requires analyzing universe entries and deriving booking rule requirements"
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

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Get all universe entries with their coverage status
        let entries = sqlx::query!(
            r#"
            SELECT
                u.universe_id,
                ic.code as instrument_class,
                m.mic as market,
                u.currencies,
                u.settlement_types,
                CASE
                    WHEN EXISTS (
                        SELECT 1 FROM custody.ssi_booking_rules r
                        WHERE r.cbu_id = u.cbu_id
                          AND r.is_active = true
                          AND (r.instrument_class_id IS NULL OR r.instrument_class_id = u.instrument_class_id)
                          AND (r.market_id IS NULL OR r.market_id = u.market_id)
                    ) THEN 'COVERED'
                    ELSE 'MISSING'
                END as coverage_status
            FROM custody.cbu_instrument_universe u
            JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
            LEFT JOIN custody.markets m ON m.market_id = u.market_id
            WHERE u.cbu_id = $1
              AND u.is_active = true
            ORDER BY ic.code, m.mic
            "#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let result: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "universe_id": e.universe_id,
                    "instrument_class": e.instrument_class,
                    "market": e.market,
                    "currencies": e.currencies,
                    "settlement_types": e.settlement_types,
                    "coverage_status": e.coverage_status
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}
