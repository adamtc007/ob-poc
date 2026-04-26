//! Custody verbs (5 plugin verbs across 2 domains —
//! `subcustodian`, `cbu-custody`) — YAML-first re-implementation
//! of `rust/config/verbs/custody.yaml`.
//!
//! - `subcustodian.lookup` — date-effective sub-custodian match
//!   for a (market, currency) pair.
//! - `cbu-custody.lookup-ssi` — ALERT-style priority match for a
//!   trade's SSI routing.
//! - `cbu-custody.validate-booking-coverage` — find universe
//!   entries lacking matching booking rules and orphan rules.
//! - `cbu-custody.derive-required-coverage` — mark each universe
//!   entry as COVERED / MISSING.
//! - `cbu-custody.setup-ssi` — bulk import from SSI_ONBOARDING
//!   document (JSON schema parse + multi-table insert).
//!
//! `sqlx::query!` macros rewritten as runtime queries (slice #10).

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    self, json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── subcustodian.lookup ───────────────────────────────────────────────────────

pub struct SubcustodianLookup;

#[async_trait]
impl SemOsVerbOp for SubcustodianLookup {
    fn fqn(&self) -> &str {
        "subcustodian.lookup"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let market = json_extract_string(args, "market")?;
        let currency = json_extract_string(args, "currency")?;
        let as_of_date: Option<chrono::NaiveDate> = json_extract_string_opt(args, "as-of-date")
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

        type Row = (
            Uuid,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            bool,
        );
        let row: Option<Row> = sqlx::query_as(
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
            FROM "ob-poc".subcustodian_network sn
            JOIN "ob-poc".markets m ON m.market_id = sn.market_id
            WHERE m.mic = $1
              AND sn.currency = $2
              AND sn.is_active = true
              AND sn.effective_date <= COALESCE($3, CURRENT_DATE)
              AND (sn.expiry_date IS NULL OR sn.expiry_date > COALESCE($3, CURRENT_DATE))
            ORDER BY sn.is_primary DESC, sn.effective_date DESC
            LIMIT 1
            "#,
        )
        .bind(&market)
        .bind(&currency)
        .bind(as_of_date)
        .fetch_optional(scope.executor())
        .await?;

        match row {
            Some((
                network_id,
                subcustodian_bic,
                subcustodian_name,
                local_agent_bic,
                local_agent_account,
                pset_bic,
                csd_participant_id,
                is_primary,
            )) => Ok(VerbExecutionOutcome::Record(json!({
                "network_id": network_id,
                "subcustodian_bic": subcustodian_bic,
                "subcustodian_name": subcustodian_name,
                "local_agent_bic": local_agent_bic,
                "local_agent_account": local_agent_account,
                "pset_bic": pset_bic,
                "csd_participant_id": csd_participant_id,
                "is_primary": is_primary,
            }))),
            None => Err(anyhow!(
                "No sub-custodian found for market {} currency {}",
                market,
                currency
            )),
        }
    }
}

// ── cbu-custody.lookup-ssi ────────────────────────────────────────────────────

pub struct LookupSsiForTrade;

#[async_trait]
impl SemOsVerbOp for LookupSsiForTrade {
    fn fqn(&self) -> &str {
        "cbu-custody.lookup-ssi"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let instrument_class = json_extract_string(args, "instrument-class")?;
        let security_type = json_extract_string_opt(args, "security-type");
        let market = json_extract_string_opt(args, "market");
        let currency = json_extract_string(args, "currency")?;
        let settlement_type = json_extract_string_opt(args, "settlement-type");

        let class_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT class_id FROM "ob-poc".instrument_classes WHERE code = $1"#,
        )
        .bind(&instrument_class)
        .fetch_optional(scope.executor())
        .await?;
        let class_id =
            class_id.ok_or_else(|| anyhow!("Unknown instrument class: {}", instrument_class))?;

        let security_type_id: Option<Uuid> = if let Some(ref st) = security_type {
            sqlx::query_scalar(
                r#"SELECT security_type_id FROM "ob-poc".security_types WHERE code = $1"#,
            )
            .bind(st)
            .fetch_optional(scope.executor())
            .await?
        } else {
            None
        };
        let market_id: Option<Uuid> = if let Some(ref m) = market {
            sqlx::query_scalar(r#"SELECT market_id FROM "ob-poc".markets WHERE mic = $1"#)
                .bind(m)
                .fetch_optional(scope.executor())
                .await?
        } else {
            None
        };

        #[derive(sqlx::FromRow)]
        struct SsiMatchRow {
            ssi_id: Uuid,
            ssi_name: String,
            rule_id: Option<Uuid>,
            rule_name: Option<String>,
            rule_priority: Option<i32>,
            specificity_score: Option<rust_decimal::Decimal>,
        }

        let row: Option<SsiMatchRow> = sqlx::query_as(
            r#"SELECT ssi_id, ssi_name, rule_id, rule_name, rule_priority, specificity_score
               FROM "ob-poc".find_ssi_for_trade($1, $2, $3, $4, $5, $6, NULL)"#,
        )
        .bind(cbu_id)
        .bind(class_id)
        .bind(security_type_id)
        .bind(market_id)
        .bind(&currency)
        .bind(settlement_type.as_deref())
        .fetch_optional(scope.executor())
        .await?;

        match row {
            Some(r) => Ok(VerbExecutionOutcome::Record(json!({
                "ssi_id": r.ssi_id,
                "ssi_name": r.ssi_name,
                "matched_rule": r.rule_name,
                "rule_id": r.rule_id,
                "rule_priority": r.rule_priority,
                "specificity_score": r.specificity_score,
            }))),
            None => Err(anyhow!(
                "No SSI found for CBU {} with instrument class {} currency {}",
                cbu_id,
                instrument_class,
                currency
            )),
        }
    }
}

// ── cbu-custody.validate-booking-coverage ─────────────────────────────────────

pub struct ValidateBookingCoverage;

#[async_trait]
impl SemOsVerbOp for ValidateBookingCoverage {
    fn fqn(&self) -> &str {
        "cbu-custody.validate-booking-coverage"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        type GapRow = (
            Uuid,
            String,
            Option<String>,
            Option<Vec<String>>,
            Option<Vec<String>>,
        );
        let gaps: Vec<GapRow> = sqlx::query_as(
            r#"
            SELECT u.universe_id, ic.code, m.mic, u.currencies, u.settlement_types
            FROM "ob-poc".cbu_instrument_universe u
            JOIN "ob-poc".instrument_classes ic ON ic.class_id = u.instrument_class_id
            LEFT JOIN "ob-poc".markets m ON m.market_id = u.market_id
            WHERE u.cbu_id = $1 AND u.is_active = true
              AND NOT EXISTS (
                  SELECT 1 FROM "ob-poc".ssi_booking_rules r
                  WHERE r.cbu_id = u.cbu_id
                    AND r.is_active = true
                    AND (r.instrument_class_id IS NULL OR r.instrument_class_id = u.instrument_class_id)
                    AND (r.market_id IS NULL OR r.market_id = u.market_id)
              )
            "#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        type OrphanRow = (Uuid, String, Option<String>, Option<String>);
        let orphans: Vec<OrphanRow> = sqlx::query_as(
            r#"
            SELECT r.rule_id, r.rule_name, ic.code, m.mic
            FROM "ob-poc".ssi_booking_rules r
            LEFT JOIN "ob-poc".instrument_classes ic ON ic.class_id = r.instrument_class_id
            LEFT JOIN "ob-poc".markets m ON m.market_id = r.market_id
            WHERE r.cbu_id = $1 AND r.is_active = true AND r.instrument_class_id IS NOT NULL
              AND NOT EXISTS (
                  SELECT 1 FROM "ob-poc".cbu_instrument_universe u
                  WHERE u.cbu_id = r.cbu_id
                    AND u.is_active = true
                    AND u.instrument_class_id = r.instrument_class_id
                    AND (r.market_id IS NULL OR u.market_id = r.market_id)
              )
            "#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        let gap_list: Vec<Value> = gaps
            .iter()
            .map(|(uid, ic, market, currencies, settlement)| {
                json!({
                    "universe_id": uid,
                    "instrument_class": ic,
                    "market": market,
                    "currencies": currencies,
                    "settlement_types": settlement,
                })
            })
            .collect();
        let orphan_list: Vec<Value> = orphans
            .iter()
            .map(|(rule_id, rule_name, ic, market)| {
                json!({
                    "rule_id": rule_id,
                    "rule_name": rule_name,
                    "instrument_class": ic,
                    "market": market,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "complete": gap_list.is_empty(),
            "gaps": gap_list,
            "orphan_rules": orphan_list,
        })))
    }
}

// ── cbu-custody.derive-required-coverage ──────────────────────────────────────

pub struct DeriveRequiredCoverage;

#[async_trait]
impl SemOsVerbOp for DeriveRequiredCoverage {
    fn fqn(&self) -> &str {
        "cbu-custody.derive-required-coverage"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        type Row = (
            Uuid,
            String,
            Option<String>,
            Option<Vec<String>>,
            Option<Vec<String>>,
            String,
        );
        let entries: Vec<Row> = sqlx::query_as(
            r#"
            SELECT
                u.universe_id,
                ic.code,
                m.mic,
                u.currencies,
                u.settlement_types,
                CASE
                    WHEN EXISTS (
                        SELECT 1 FROM "ob-poc".ssi_booking_rules r
                        WHERE r.cbu_id = u.cbu_id
                          AND r.is_active = true
                          AND (r.instrument_class_id IS NULL OR r.instrument_class_id = u.instrument_class_id)
                          AND (r.market_id IS NULL OR r.market_id = u.market_id)
                    ) THEN 'COVERED'
                    ELSE 'MISSING'
                END
            FROM "ob-poc".cbu_instrument_universe u
            JOIN "ob-poc".instrument_classes ic ON ic.class_id = u.instrument_class_id
            LEFT JOIN "ob-poc".markets m ON m.market_id = u.market_id
            WHERE u.cbu_id = $1 AND u.is_active = true
            ORDER BY ic.code, m.mic
            "#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        let results: Vec<Value> = entries
            .iter()
            .map(|(uid, ic, market, currencies, settlement, status)| {
                json!({
                    "universe_id": uid,
                    "instrument_class": ic,
                    "market": market,
                    "currencies": currencies,
                    "settlement_types": settlement,
                    "coverage_status": status,
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// ── cbu-custody.setup-ssi ─────────────────────────────────────────────────────

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

pub struct SetupSsi;

#[async_trait]
impl SemOsVerbOp for SetupSsi {
    fn fqn(&self) -> &str {
        "cbu-custody.setup-ssi"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        use chrono::NaiveDate;

        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let document_id = json_extract_uuid(args, ctx, "document-id")?;
        let validation_mode = json_extract_string_opt(args, "validation-mode")
            .unwrap_or_else(|| "STRICT".to_string());

        let doc_row: Option<(Option<Value>, String)> = sqlx::query_as(
            r#"
            SELECT dc.extracted_data, dt.type_code
            FROM "ob-poc".document_catalog dc
            JOIN "ob-poc".document_types dt ON dt.type_id = dc.document_type_id
            WHERE dc.doc_id = $1
            "#,
        )
        .bind(document_id)
        .fetch_optional(scope.executor())
        .await?;
        let (extracted_data, type_code) =
            doc_row.ok_or_else(|| anyhow!("Document not found: {}", document_id))?;
        if type_code != "SSI_ONBOARDING" {
            return Err(anyhow!(
                "Document is not SSI_ONBOARDING type, got: {}",
                type_code
            ));
        }
        let extracted_data =
            extracted_data.ok_or_else(|| anyhow!("Document has no extracted_data"))?;
        let ssi_doc: SsiOnboardingDocument = serde_json::from_value(extracted_data)
            .map_err(|e| anyhow!("Failed to parse SSI document: {}", e))?;

        let mut created_ssis: Vec<Value> = Vec::new();
        let mut created_overrides = 0;
        let mut created_rules = 0;
        let mut errors: Vec<String> = Vec::new();

        for ssi in &ssi_doc.settlement_instructions {
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

            let market_id: Option<Uuid> = if let Some(mic) = &ssi.market_mic {
                sqlx::query_scalar(r#"SELECT market_id FROM "ob-poc".markets WHERE mic = $1"#)
                    .bind(mic)
                    .fetch_optional(scope.executor())
                    .await?
            } else {
                None
            };

            let effective_date = NaiveDate::parse_from_str(&ssi.effective_date, "%Y-%m-%d")
                .map_err(|e| anyhow!("Invalid effective_date for {}: {}", ssi.ssi_name, e))?;
            let expiry_date: Option<NaiveDate> = ssi
                .expiry_date
                .as_ref()
                .map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d"))
                .transpose()
                .map_err(|e| anyhow!("Invalid expiry_date for {}: {}", ssi.ssi_name, e))?;

            let ssi_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".cbu_ssi (
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
            )
            .bind(ssi_id)
            .bind(cbu_id)
            .bind(&ssi.ssi_name)
            .bind(&ssi.ssi_type)
            .bind(market_id)
            .bind(&ssi.safekeeping_account)
            .bind(&ssi.safekeeping_bic)
            .bind(&ssi.safekeeping_account_name)
            .bind(&ssi.cash_account)
            .bind(&ssi.cash_account_bic)
            .bind(&ssi.cash_currency)
            .bind(&ssi.collateral_account)
            .bind(&ssi.collateral_account_bic)
            .bind(&ssi.pset_bic)
            .bind(&ssi.receiving_agent_bic)
            .bind(&ssi.delivering_agent_bic)
            .bind(effective_date)
            .bind(expiry_date)
            .bind(&ssi.source)
            .bind(&ssi.source_reference)
            .execute(scope.executor())
            .await?;

            created_ssis.push(json!({
                "ssi_id": ssi_id,
                "ssi_name": ssi.ssi_name,
                "market": ssi.market_mic,
            }));

            for agent in &ssi.agent_overrides {
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".cbu_ssi_agent_override (
                        ssi_id, agent_role, agent_bic, agent_account, agent_name, sequence_order, reason
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                )
                .bind(ssi_id)
                .bind(&agent.agent_role)
                .bind(&agent.agent_bic)
                .bind(&agent.agent_account)
                .bind(&agent.agent_name)
                .bind(agent.sequence_order)
                .bind(&agent.reason)
                .execute(scope.executor())
                .await?;
                created_overrides += 1;
            }

            for rule in &ssi.booking_rules {
                let instrument_class_id: Option<Uuid> = if let Some(ic) = &rule.instrument_class {
                    sqlx::query_scalar(
                        r#"SELECT class_id FROM "ob-poc".instrument_classes WHERE code = $1"#,
                    )
                    .bind(ic)
                    .fetch_optional(scope.executor())
                    .await?
                } else {
                    None
                };
                let security_type_id: Option<Uuid> = if let Some(st) = &rule.security_type {
                    sqlx::query_scalar(
                        r#"SELECT security_type_id FROM "ob-poc".security_types WHERE code = $1"#,
                    )
                    .bind(st)
                    .fetch_optional(scope.executor())
                    .await?
                } else {
                    None
                };
                let rule_effective_date = rule
                    .effective_date
                    .as_ref()
                    .map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d"))
                    .transpose()
                    .map_err(|e| anyhow!("Invalid rule effective_date: {}", e))?
                    .unwrap_or(effective_date);

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".ssi_booking_rules (
                        cbu_id, ssi_id, rule_name, priority,
                        instrument_class_id, security_type_id, market_id,
                        currency, settlement_type, effective_date
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                    "#,
                )
                .bind(cbu_id)
                .bind(ssi_id)
                .bind(&rule.rule_name)
                .bind(rule.priority)
                .bind(instrument_class_id)
                .bind(security_type_id)
                .bind(market_id)
                .bind(&rule.currency)
                .bind(&rule.settlement_type)
                .bind(rule_effective_date)
                .execute(scope.executor())
                .await?;
                created_rules += 1;
            }
        }

        if errors.is_empty() && !created_ssis.is_empty() {
            helpers::emit_pending_state_advance(
                ctx,
                cbu_id,
                "custody:ssi_configured",
                "cbu/custody",
                "cbu-custody.setup-ssi",
            );
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "success": errors.is_empty(),
            "ssis_created": created_ssis.len(),
            "ssis": created_ssis,
            "agent_overrides_created": created_overrides,
            "booking_rules_created": created_rules,
            "errors": errors,
        })))
    }
}
