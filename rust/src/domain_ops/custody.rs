//! Custody Domain Custom Operations
//!
//! Plugin handlers for custody operations that cannot be expressed as
//! data-driven verb definitions:
//! - SSI lookup for trade routing (ALERT-style matching)
//! - Booking coverage validation
//! - Sub-custodian network lookup

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

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
#[register_custom_op]
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
            .find(|a| a.key == "market")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing market argument"))?;

        // Get currency
        let currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "currency")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing currency argument"))?;

        // Get as-of date (default to today)
        let as_of_date: Option<chrono::NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "as-of-date")
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
#[register_custom_op]
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
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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
            .find(|a| a.key == "instrument-class")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing instrument-class argument"))?;

        // Get optional security type code
        let security_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "security-type")
            .and_then(|a| a.value.as_string());

        // Get optional market MIC
        let market = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "market")
            .and_then(|a| a.value.as_string());

        // Get currency (required)
        let currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "currency")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing currency argument"))?;

        // Get optional settlement type
        let settlement_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "settlement-type")
            .and_then(|a| a.value.as_string());

        // Get optional counterparty BIC (we'd need to look up entity)
        let _counterparty_bic = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "counterparty-bic")
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
#[register_custom_op]
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

        tracing::debug!("ValidateBookingCoverageOp::execute ENTERED");

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        tracing::debug!("cbu_id = {}", cbu_id);

        // Find universe entries without matching booking rules
        let gaps = sqlx::query!(
            r#"
            SELECT
                u.universe_id,
                ic.code as instrument_class,
                m.mic as "market?",
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
                ic.code as "instrument_class?",
                m.mic as "market?"
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
#[register_custom_op]
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
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
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
                m.mic as "market?",
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

// ============================================================================
// Setup SSI from Document (Bulk Import)
// ============================================================================

/// Bulk import SSIs from SSI_ONBOARDING document
///
/// Rationale: Requires parsing JSON document, validating BICs, and creating
/// multiple related records (SSIs, agent overrides, booking rules) in a transaction.
#[register_custom_op]
pub struct SetupSsiFromDocumentOp;

#[async_trait]
impl CustomOperation for SetupSsiFromDocumentOp {
    fn domain(&self) -> &'static str {
        "cbu-custody"
    }
    fn verb(&self) -> &'static str {
        "setup-ssi"
    }
    fn rationale(&self) -> &'static str {
        "Requires JSON document parsing, BIC validation, and multi-table transaction"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use chrono::NaiveDate;
        use serde::Deserialize;
        use serde_json::json;
        use uuid::Uuid;

        // SSI Onboarding Document schema structs
        #[derive(Debug, Deserialize)]
        struct SsiOnboardingDocument {
            settlement_instructions: Vec<SettlementInstruction>,
        }

        #[derive(Debug, Deserialize)]
        struct SettlementInstruction {
            ssi_name: String,
            ssi_type: String,
            market_mic: Option<String>,
            safekeeping_account: Option<String>,
            safekeeping_bic: Option<String>,
            safekeeping_account_name: Option<String>,
            cash_account: Option<String>,
            cash_account_bic: Option<String>,
            cash_currency: Option<String>,
            collateral_account: Option<String>,
            collateral_account_bic: Option<String>,
            pset_bic: Option<String>,
            receiving_agent_bic: Option<String>,
            delivering_agent_bic: Option<String>,
            effective_date: String,
            expiry_date: Option<String>,
            source: Option<String>,
            source_reference: Option<String>,
            #[serde(default)]
            agent_overrides: Vec<AgentOverride>,
            #[serde(default)]
            booking_rules: Vec<BookingRule>,
        }

        #[derive(Debug, Deserialize)]
        struct AgentOverride {
            agent_role: String,
            agent_bic: String,
            agent_account: Option<String>,
            agent_name: Option<String>,
            sequence_order: i32,
            reason: Option<String>,
        }

        #[derive(Debug, Deserialize)]
        struct BookingRule {
            rule_name: String,
            priority: i32,
            instrument_class: Option<String>,
            security_type: Option<String>,
            currency: Option<String>,
            settlement_type: Option<String>,
            effective_date: Option<String>,
        }

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Get document ID
        let document_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing document-id argument"))?;

        // Get validation mode
        let validation_mode = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "validation-mode")
            .and_then(|a| a.value.as_string())
            .unwrap_or("STRICT");

        // Fetch document and verify it's SSI_ONBOARDING type
        let doc_row = sqlx::query!(
            r#"
            SELECT dc.extracted_data, dt.type_code
            FROM "ob-poc".document_catalog dc
            JOIN "ob-poc".document_types dt ON dt.type_id = dc.document_type_id
            WHERE dc.doc_id = $1
            "#,
            document_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Document not found: {}", document_id))?;

        if doc_row.type_code != "SSI_ONBOARDING" {
            return Err(anyhow::anyhow!(
                "Document is not SSI_ONBOARDING type, got: {}",
                doc_row.type_code
            ));
        }

        let extracted_data = doc_row
            .extracted_data
            .ok_or_else(|| anyhow::anyhow!("Document has no extracted_data"))?;

        // Parse the SSI onboarding document
        let ssi_doc: SsiOnboardingDocument = serde_json::from_value(extracted_data)
            .map_err(|e| anyhow::anyhow!("Failed to parse SSI document: {}", e))?;

        let mut created_ssis: Vec<serde_json::Value> = Vec::new();
        let mut created_overrides = 0;
        let mut created_rules = 0;
        let mut errors: Vec<String> = Vec::new();

        // Process each settlement instruction
        for ssi in &ssi_doc.settlement_instructions {
            // Validate BICs in STRICT mode
            if validation_mode == "STRICT" {
                if let Some(bic) = &ssi.safekeeping_bic {
                    if bic.len() != 8 && bic.len() != 11 {
                        errors.push(format!(
                            "Invalid safekeeping_bic length for {}: {}",
                            ssi.ssi_name, bic
                        ));
                        continue;
                    }
                }
                if let Some(bic) = &ssi.pset_bic {
                    if bic.len() != 8 && bic.len() != 11 {
                        errors.push(format!(
                            "Invalid pset_bic length for {}: {}",
                            ssi.ssi_name, bic
                        ));
                        continue;
                    }
                }
            }

            // Look up market_id if market_mic provided
            let market_id: Option<Uuid> = if let Some(mic) = &ssi.market_mic {
                sqlx::query_scalar("SELECT market_id FROM custody.markets WHERE mic = $1")
                    .bind(mic)
                    .fetch_optional(pool)
                    .await?
            } else {
                None
            };

            // Parse effective date
            let effective_date = NaiveDate::parse_from_str(&ssi.effective_date, "%Y-%m-%d")
                .map_err(|e| {
                    anyhow::anyhow!("Invalid effective_date for {}: {}", ssi.ssi_name, e)
                })?;

            // Parse expiry date if present
            let expiry_date: Option<NaiveDate> = ssi
                .expiry_date
                .as_ref()
                .map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d"))
                .transpose()
                .map_err(|e| anyhow::anyhow!("Invalid expiry_date for {}: {}", ssi.ssi_name, e))?;

            // Insert SSI
            let ssi_id = Uuid::now_v7();
            sqlx::query!(
                r#"
                INSERT INTO custody.cbu_ssi (
                    ssi_id, cbu_id, ssi_name, ssi_type, market_id,
                    safekeeping_account, safekeeping_bic, safekeeping_account_name,
                    cash_account, cash_account_bic, cash_currency,
                    collateral_account, collateral_account_bic,
                    pset_bic, receiving_agent_bic, delivering_agent_bic,
                    effective_date, expiry_date, status, source, source_reference
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, 'PENDING', $19, $20
                )
                "#,
                ssi_id,
                cbu_id,
                ssi.ssi_name,
                ssi.ssi_type,
                market_id,
                ssi.safekeeping_account,
                ssi.safekeeping_bic,
                ssi.safekeeping_account_name,
                ssi.cash_account,
                ssi.cash_account_bic,
                ssi.cash_currency,
                ssi.collateral_account,
                ssi.collateral_account_bic,
                ssi.pset_bic,
                ssi.receiving_agent_bic,
                ssi.delivering_agent_bic,
                effective_date,
                expiry_date,
                ssi.source,
                ssi.source_reference
            )
            .execute(pool)
            .await?;

            created_ssis.push(json!({
                "ssi_id": ssi_id,
                "ssi_name": ssi.ssi_name,
                "market": ssi.market_mic
            }));

            // Insert agent overrides
            for agent in &ssi.agent_overrides {
                sqlx::query!(
                    r#"
                    INSERT INTO custody.cbu_ssi_agent_override (
                        ssi_id, agent_role, agent_bic, agent_account, agent_name, sequence_order, reason
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                    ssi_id,
                    agent.agent_role,
                    agent.agent_bic,
                    agent.agent_account,
                    agent.agent_name,
                    agent.sequence_order,
                    agent.reason
                )
                .execute(pool)
                .await?;
                created_overrides += 1;
            }

            // Insert booking rules
            for rule in &ssi.booking_rules {
                // Look up instrument_class_id if provided
                let instrument_class_id: Option<Uuid> = if let Some(ic) = &rule.instrument_class {
                    sqlx::query_scalar(
                        "SELECT class_id FROM custody.instrument_classes WHERE code = $1",
                    )
                    .bind(ic)
                    .fetch_optional(pool)
                    .await?
                } else {
                    None
                };

                // Look up security_type_id if provided
                let security_type_id: Option<Uuid> = if let Some(st) = &rule.security_type {
                    sqlx::query_scalar(
                        "SELECT security_type_id FROM custody.security_types WHERE code = $1",
                    )
                    .bind(st)
                    .fetch_optional(pool)
                    .await?
                } else {
                    None
                };

                let rule_effective_date = rule
                    .effective_date
                    .as_ref()
                    .map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d"))
                    .transpose()
                    .map_err(|e| anyhow::anyhow!("Invalid rule effective_date: {}", e))?
                    .unwrap_or(effective_date);

                sqlx::query!(
                    r#"
                    INSERT INTO custody.ssi_booking_rules (
                        cbu_id, ssi_id, rule_name, priority,
                        instrument_class_id, security_type_id, market_id,
                        currency, settlement_type, effective_date
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                    "#,
                    cbu_id,
                    ssi_id,
                    rule.rule_name,
                    rule.priority,
                    instrument_class_id,
                    security_type_id,
                    market_id,
                    rule.currency,
                    rule.settlement_type,
                    rule_effective_date
                )
                .execute(pool)
                .await?;
                created_rules += 1;
            }
        }

        Ok(ExecutionResult::Record(json!({
            "success": errors.is_empty(),
            "ssis_created": created_ssis.len(),
            "ssis": created_ssis,
            "agent_overrides_created": created_overrides,
            "booking_rules_created": created_rules,
            "errors": errors
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "success": true,
            "ssis_created": 0,
            "ssis": [],
            "agent_overrides_created": 0,
            "booking_rules_created": 0,
            "errors": []
        })))
    }
}
