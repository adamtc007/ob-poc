//! Partnership verbs (4 plugin verbs) — YAML-first
//! re-implementation of `rust/config/verbs/partnership.yaml`:
//! record-contribution, record-distribution, reconcile,
//! analyze-control.
//!
//! Legacy code opened per-op `pool.begin()` transactions; the
//! new implementation rides the Sequencer-owned scope, so all
//! writes are atomic with the rest of the dispatched verb.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    self, json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, sqlx::FromRow)]
struct PartnerRecord {
    id: Uuid,
    partner_type: String,
    capital_commitment: Option<rust_decimal::Decimal>,
    capital_contributed: Option<rust_decimal::Decimal>,
    capital_returned: Option<rust_decimal::Decimal>,
}

async fn load_partner(
    scope: &mut dyn TransactionScope,
    partnership_id: Uuid,
    partner_id: Uuid,
) -> Result<PartnerRecord> {
    let row: Option<PartnerRecord> = sqlx::query_as(
        r#"
        SELECT id, partner_type, capital_commitment, capital_contributed, capital_returned
        FROM "ob-poc".partnership_capital
        WHERE partnership_entity_id = $1 AND partner_entity_id = $2 AND is_active = true
        "#,
    )
    .bind(partnership_id)
    .bind(partner_id)
    .fetch_optional(scope.executor())
    .await?;
    row.ok_or_else(|| anyhow!("Partner not found in partnership"))
}

// ── partnership.record-contribution ───────────────────────────────────────────

pub struct RecordContribution;

#[async_trait]
impl SemOsVerbOp for RecordContribution {
    fn fqn(&self) -> &str {
        "partnership.record-contribution"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let partnership_id = json_extract_uuid(args, ctx, "partnership-entity-id")?;
        let partner_id = json_extract_uuid(args, ctx, "partner-entity-id")?;
        let amount: rust_decimal::Decimal = json_extract_string(args, "amount")?
            .parse()
            .map_err(|_| anyhow!("amount is required"))?;
        let contribution_date = json_extract_string_opt(args, "contribution-date")
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

        let partner = load_partner(scope, partnership_id, partner_id).await?;

        let old_contributed = partner.capital_contributed.unwrap_or(rust_decimal::Decimal::ZERO);
        let commitment = partner.capital_commitment.unwrap_or(rust_decimal::Decimal::ZERO);
        let new_contributed = old_contributed + amount;

        if commitment > rust_decimal::Decimal::ZERO && new_contributed > commitment {
            return Err(anyhow!(
                "Contribution of {} would exceed capital commitment of {} (current contributed: {})",
                amount,
                commitment,
                old_contributed
            ));
        }

        sqlx::query(
            r#"UPDATE "ob-poc".partnership_capital SET capital_contributed = $1, updated_at = NOW() WHERE id = $2"#,
        )
        .bind(new_contributed)
        .bind(partner.id)
        .execute(scope.executor())
        .await?;

        helpers::emit_pending_state_advance(
            ctx,
            partner_id,
            "partnership:capital_contributed",
            "partnership/capital",
            "partnership.record-contribution",
        );

        let unfunded = commitment - new_contributed
            + partner.capital_returned.unwrap_or(rust_decimal::Decimal::ZERO);

        Ok(VerbExecutionOutcome::Record(json!({
            "partnership_capital_id": partner.id.to_string(),
            "partnership_entity_id": partnership_id.to_string(),
            "partner_entity_id": partner_id.to_string(),
            "partner_type": partner.partner_type,
            "contribution_amount": amount.to_string(),
            "previous_contributed": old_contributed.to_string(),
            "new_contributed": new_contributed.to_string(),
            "capital_commitment": commitment.to_string(),
            "unfunded_commitment": unfunded.to_string(),
            "contribution_date": contribution_date,
        })))
    }
}

// ── partnership.record-distribution ───────────────────────────────────────────

pub struct RecordDistribution;

#[async_trait]
impl SemOsVerbOp for RecordDistribution {
    fn fqn(&self) -> &str {
        "partnership.record-distribution"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let partnership_id = json_extract_uuid(args, ctx, "partnership-entity-id")?;
        let partner_id = json_extract_uuid(args, ctx, "partner-entity-id")?;
        let amount: rust_decimal::Decimal = json_extract_string(args, "amount")?
            .parse()
            .map_err(|_| anyhow!("amount is required"))?;
        let distribution_type = json_extract_string_opt(args, "distribution-type")
            .unwrap_or_else(|| "capital_return".to_string());
        let distribution_date = json_extract_string_opt(args, "distribution-date")
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

        let partner = load_partner(scope, partnership_id, partner_id).await?;
        let contributed = partner.capital_contributed.unwrap_or(rust_decimal::Decimal::ZERO);
        let old_returned = partner.capital_returned.unwrap_or(rust_decimal::Decimal::ZERO);

        if distribution_type == "capital_return" {
            let max_returnable = contributed - old_returned;
            if amount > max_returnable {
                return Err(anyhow!(
                    "Cannot return {} - only {} is available (contributed {} minus already returned {})",
                    amount, max_returnable, contributed, old_returned
                ));
            }
        }

        let new_returned = old_returned + amount;
        sqlx::query(
            r#"UPDATE "ob-poc".partnership_capital SET capital_returned = $1, updated_at = NOW() WHERE id = $2"#,
        )
        .bind(new_returned)
        .bind(partner.id)
        .execute(scope.executor())
        .await?;

        helpers::emit_pending_state_advance(
            ctx,
            partner_id,
            "partnership:capital_distributed",
            "partnership/capital",
            "partnership.record-distribution",
        );

        Ok(VerbExecutionOutcome::Record(json!({
            "partnership_capital_id": partner.id.to_string(),
            "partnership_entity_id": partnership_id.to_string(),
            "partner_entity_id": partner_id.to_string(),
            "partner_type": partner.partner_type,
            "distribution_type": distribution_type,
            "distribution_amount": amount.to_string(),
            "previous_returned": old_returned.to_string(),
            "new_returned": new_returned.to_string(),
            "capital_contributed": contributed.to_string(),
            "distribution_date": distribution_date,
        })))
    }
}

// ── partnership.reconcile ─────────────────────────────────────────────────────

pub struct Reconcile;

#[async_trait]
impl SemOsVerbOp for Reconcile {
    fn fqn(&self) -> &str {
        "partnership.reconcile"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let partnership_id = json_extract_uuid(args, ctx, "partnership-entity-id")?;

        #[derive(Debug, sqlx::FromRow)]
        struct PartnerSummary {
            partner_entity_id: Uuid,
            partner_name: Option<String>,
            partner_type: String,
            profit_share_pct: Option<rust_decimal::Decimal>,
            voting_pct: Option<rust_decimal::Decimal>,
            capital_commitment: Option<rust_decimal::Decimal>,
            capital_contributed: Option<rust_decimal::Decimal>,
        }

        let partners: Vec<PartnerSummary> = sqlx::query_as(
            r#"
            SELECT pc.partner_entity_id, e.name as partner_name, pc.partner_type,
                   pc.profit_share_pct, pc.voting_pct,
                   pc.capital_commitment, pc.capital_contributed
            FROM "ob-poc".partnership_capital pc
            JOIN "ob-poc".entities e ON pc.partner_entity_id = e.entity_id
            WHERE pc.partnership_entity_id = $1 AND pc.is_active = true AND e.deleted_at IS NULL
            ORDER BY pc.partner_type, pc.profit_share_pct DESC NULLS LAST
            "#,
        )
        .bind(partnership_id)
        .fetch_all(scope.executor())
        .await?;

        let mut total_profit_share: f64 = 0.0;
        let mut total_voting: f64 = 0.0;
        let mut total_commitment = rust_decimal::Decimal::ZERO;
        let mut total_contributed = rust_decimal::Decimal::ZERO;
        let mut gp_count = 0;
        let mut lp_count = 0;
        let mut partner_details: Vec<Value> = Vec::new();

        for p in &partners {
            let profit_pct: f64 = p.profit_share_pct.map(|d| d.to_string().parse().unwrap_or(0.0)).unwrap_or(0.0);
            total_profit_share += profit_pct;
            let voting: f64 = p.voting_pct.map(|d| d.to_string().parse().unwrap_or(0.0)).unwrap_or(0.0);
            total_voting += voting;
            let commitment = p.capital_commitment.unwrap_or(rust_decimal::Decimal::ZERO);
            total_commitment += commitment;
            let contributed = p.capital_contributed.unwrap_or(rust_decimal::Decimal::ZERO);
            total_contributed += contributed;
            match p.partner_type.as_str() {
                "GP" | "FOUNDING_PARTNER" => gp_count += 1,
                "LP" | "SPECIAL_LP" => lp_count += 1,
                _ => {}
            }
            partner_details.push(json!({
                "partner_id": p.partner_entity_id.to_string(),
                "partner_name": p.partner_name,
                "partner_type": p.partner_type,
                "profit_share_pct": profit_pct,
                "voting_pct": voting,
                "capital_commitment": commitment.to_string(),
                "capital_contributed": contributed.to_string(),
            }));
        }

        let tolerance = 0.01;
        let is_profit_balanced = (total_profit_share - 100.0).abs() <= tolerance;
        let is_voting_balanced = (total_voting - 100.0).abs() <= tolerance || total_voting == 0.0;

        let mut issues: Vec<String> = Vec::new();
        if !is_profit_balanced {
            issues.push(format!("Profit shares sum to {:.2}%, expected 100%", total_profit_share));
        }
        if !is_voting_balanced {
            issues.push(format!("Voting percentages sum to {:.2}%, expected 100%", total_voting));
        }
        if gp_count == 0 && lp_count > 0 {
            issues.push("No General Partner (GP) found in limited partnership".into());
        }

        let status = if issues.is_empty() { "reconciled" } else { "discrepancies_found" };

        Ok(VerbExecutionOutcome::Record(json!({
            "partnership_entity_id": partnership_id.to_string(),
            "partner_count": partners.len(),
            "gp_count": gp_count,
            "lp_count": lp_count,
            "total_profit_share_pct": total_profit_share,
            "total_voting_pct": total_voting,
            "total_capital_commitment": total_commitment.to_string(),
            "total_capital_contributed": total_contributed.to_string(),
            "is_profit_balanced": is_profit_balanced,
            "is_voting_balanced": is_voting_balanced,
            "tolerance_pct": tolerance,
            "issues": issues,
            "status": status,
            "partners": partner_details,
            "reconciled_at": chrono::Utc::now().to_rfc3339(),
        })))
    }
}

// ── partnership.analyze-control ───────────────────────────────────────────────

pub struct AnalyzeControl;

#[async_trait]
impl SemOsVerbOp for AnalyzeControl {
    fn fqn(&self) -> &str {
        "partnership.analyze-control"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let partnership_id = json_extract_uuid(args, ctx, "partnership-entity-id")?;

        #[derive(Debug, sqlx::FromRow)]
        struct PartnerControlInfo {
            partner_entity_id: Uuid,
            partner_name: Option<String>,
            partner_type: String,
            profit_share_pct: Option<rust_decimal::Decimal>,
            voting_pct: Option<rust_decimal::Decimal>,
            management_rights: Option<bool>,
            is_natural_person: Option<bool>,
        }

        let partners: Vec<PartnerControlInfo> = sqlx::query_as(
            r#"
            SELECT
                pc.partner_entity_id, e.name as partner_name, pc.partner_type,
                pc.profit_share_pct, pc.voting_pct, pc.management_rights,
                EXISTS(
                    SELECT 1 FROM "ob-poc".entity_proper_persons pp
                    WHERE pp.entity_id = pc.partner_entity_id
                ) as is_natural_person
            FROM "ob-poc".partnership_capital pc
            JOIN "ob-poc".entities e ON pc.partner_entity_id = e.entity_id
            WHERE pc.partnership_entity_id = $1 AND pc.is_active = true AND e.deleted_at IS NULL
            ORDER BY pc.partner_type, pc.profit_share_pct DESC NULLS LAST
            "#,
        )
        .bind(partnership_id)
        .fetch_all(scope.executor())
        .await?;

        let mut controllers: Vec<Value> = Vec::new();
        let mut gps: Vec<Value> = Vec::new();
        let mut lps: Vec<Value> = Vec::new();

        for p in &partners {
            let profit_pct: f64 = p.profit_share_pct.map(|d| d.to_string().parse().unwrap_or(0.0)).unwrap_or(0.0);
            let voting_pct: f64 = p.voting_pct.map(|d| d.to_string().parse().unwrap_or(0.0)).unwrap_or(0.0);
            let partner_info = json!({
                "partner_id": p.partner_entity_id.to_string(),
                "partner_name": p.partner_name,
                "partner_type": p.partner_type,
                "profit_share_pct": profit_pct,
                "voting_pct": voting_pct,
                "management_rights": p.management_rights.unwrap_or(false),
                "is_natural_person": p.is_natural_person.unwrap_or(false),
            });

            match p.partner_type.as_str() {
                "GP" | "FOUNDING_PARTNER" => {
                    gps.push(partner_info.clone());
                    controllers.push(json!({
                        "controller_id": p.partner_entity_id.to_string(),
                        "controller_name": p.partner_name,
                        "control_type": "general_partner",
                        "control_strength": 0.95,
                        "is_natural_person": p.is_natural_person.unwrap_or(false),
                        "needs_further_tracing": !p.is_natural_person.unwrap_or(false),
                    }));
                }
                "LP" | "SPECIAL_LP" => {
                    lps.push(partner_info.clone());
                    if p.management_rights.unwrap_or(false) || voting_pct > 50.0 {
                        controllers.push(json!({
                            "controller_id": p.partner_entity_id.to_string(),
                            "controller_name": p.partner_name,
                            "control_type": if p.management_rights.unwrap_or(false) {
                                "lp_with_management_rights"
                            } else {
                                "lp_majority_voting"
                            },
                            "control_strength": if voting_pct > 50.0 { 0.85 } else { 0.70 },
                            "voting_pct": voting_pct,
                            "is_natural_person": p.is_natural_person.unwrap_or(false),
                            "needs_further_tracing": !p.is_natural_person.unwrap_or(false),
                        }));
                    }
                }
                "MEMBER" => {
                    if p.management_rights.unwrap_or(false) || voting_pct > 50.0 {
                        controllers.push(json!({
                            "controller_id": p.partner_entity_id.to_string(),
                            "controller_name": p.partner_name,
                            "control_type": "managing_member",
                            "control_strength": 0.80,
                            "voting_pct": voting_pct,
                            "is_natural_person": p.is_natural_person.unwrap_or(false),
                            "needs_further_tracing": !p.is_natural_person.unwrap_or(false),
                        }));
                    }
                }
                _ => {}
            }
        }

        let control_type = match gps.len() {
            0 => {
                if partners.iter().any(|p| p.partner_type == "MEMBER") {
                    "llc_member_managed"
                } else {
                    "no_gp"
                }
            }
            1 => "single_gp",
            _ => "multiple_gps",
        };

        let analysis_notes = match control_type {
            "no_gp" => "WARNING: No General Partner found - unusual partnership structure",
            "llc_member_managed" => "LLC structure - members with management rights have control",
            "single_gp" => "Single GP has presumptive control per partnership law",
            "multiple_gps" => "Multiple GPs share control - further analysis may be needed",
            _ => "Unknown control structure",
        };

        let gp_count = gps.len();
        let lp_count = lps.len();
        let total_partners = partners.len();

        Ok(VerbExecutionOutcome::Record(json!({
            "partnership_entity_id": partnership_id.to_string(),
            "control_type": control_type,
            "general_partners": gps,
            "limited_partners": lps,
            "controllers": controllers,
            "gp_count": gp_count,
            "lp_count": lp_count,
            "total_partners": total_partners,
            "analysis_notes": analysis_notes,
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
        })))
    }
}
