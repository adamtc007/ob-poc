//! Partnership Operations - Partnership capital and control management
//!
//! Plugin handlers for partnership.yaml verbs that require custom logic.
//! Uses kyc.partnership_capital table for partner capital tracking.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde_json::json;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::get_required_uuid;
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

// ============================================================================
// PartnershipContributionOp - Record capital contribution
// ============================================================================

/// Records a capital contribution to a partnership, updating capital accounts.
/// Updates the capital_contributed column in kyc.partnership_capital.
#[register_custom_op]
pub struct PartnershipContributionOp;

#[cfg(feature = "database")]
#[derive(Debug, sqlx::FromRow)]
struct PartnerRecord {
    id: Uuid,
    partner_type: String,
    capital_commitment: Option<rust_decimal::Decimal>,
    capital_contributed: Option<rust_decimal::Decimal>,
    capital_returned: Option<rust_decimal::Decimal>,
}

#[async_trait]
impl CustomOperation for PartnershipContributionOp {
    fn domain(&self) -> &'static str {
        "partnership"
    }

    fn verb(&self) -> &'static str {
        "record-contribution"
    }

    fn rationale(&self) -> &'static str {
        "Capital contributions require updating capital accounts and recalculating ownership percentages"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let partnership_id = get_required_uuid(verb_call, "partnership-entity-id")?;
        let partner_id = get_required_uuid(verb_call, "partner-entity-id")?;

        let amount: rust_decimal::Decimal = verb_call
            .get_arg("amount")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("amount is required"))?;

        let contribution_date = verb_call
            .get_arg("contribution-date")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

        let mut tx = pool.begin().await?;

        // Verify partner exists in partnership (use runtime query for kyc schema)
        let partner_record: Option<PartnerRecord> = sqlx::query_as(
            r#"
            SELECT id, partner_type, capital_commitment, capital_contributed, capital_returned
            FROM kyc.partnership_capital
            WHERE partnership_entity_id = $1 AND partner_entity_id = $2 AND is_active = true
            "#,
        )
        .bind(partnership_id)
        .bind(partner_id)
        .fetch_optional(&mut *tx)
        .await?;

        let partner_record =
            partner_record.ok_or_else(|| anyhow!("Partner not found in partnership"))?;

        let old_contributed = partner_record
            .capital_contributed
            .unwrap_or(rust_decimal::Decimal::ZERO);
        let commitment = partner_record
            .capital_commitment
            .unwrap_or(rust_decimal::Decimal::ZERO);
        let new_contributed = old_contributed + amount;

        // Check if contribution would exceed commitment (if commitment is set)
        if commitment > rust_decimal::Decimal::ZERO && new_contributed > commitment {
            return Err(anyhow!(
                "Contribution of {} would exceed capital commitment of {} (current contributed: {})",
                amount,
                commitment,
                old_contributed
            ));
        }

        // Update capital_contributed
        sqlx::query(
            r#"
            UPDATE kyc.partnership_capital
            SET capital_contributed = $1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(new_contributed)
        .bind(partner_record.id)
        .execute(&mut *tx)
        .await?;

        // Log the contribution event (to case_events if we have a case context, otherwise just return)
        // For now we just return the result without logging to a separate transactions table

        tx.commit().await?;

        let unfunded = commitment - new_contributed
            + partner_record
                .capital_returned
                .unwrap_or(rust_decimal::Decimal::ZERO);

        let result = json!({
            "partnership_capital_id": partner_record.id.to_string(),
            "partnership_entity_id": partnership_id.to_string(),
            "partner_entity_id": partner_id.to_string(),
            "partner_type": partner_record.partner_type,
            "contribution_amount": amount.to_string(),
            "previous_contributed": old_contributed.to_string(),
            "new_contributed": new_contributed.to_string(),
            "capital_commitment": commitment.to_string(),
            "unfunded_commitment": unfunded.to_string(),
            "contribution_date": contribution_date
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "Database feature required for partnership.record-contribution"
        ))
    }
}

// ============================================================================
// PartnershipDistributionOp - Record distribution (capital return) to partner
// ============================================================================

/// Records a capital return/distribution from partnership to partner.
/// Updates the capital_returned column in kyc.partnership_capital.
#[register_custom_op]
pub struct PartnershipDistributionOp;

#[async_trait]
impl CustomOperation for PartnershipDistributionOp {
    fn domain(&self) -> &'static str {
        "partnership"
    }

    fn verb(&self) -> &'static str {
        "record-distribution"
    }

    fn rationale(&self) -> &'static str {
        "Distributions require updating capital_returned and validating against contribution history"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let partnership_id = get_required_uuid(verb_call, "partnership-entity-id")?;
        let partner_id = get_required_uuid(verb_call, "partner-entity-id")?;

        let amount: rust_decimal::Decimal = verb_call
            .get_arg("amount")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("amount is required"))?;

        let distribution_type = verb_call
            .get_arg("distribution-type")
            .and_then(|a| a.value.as_string())
            .unwrap_or("capital_return");

        let distribution_date = verb_call
            .get_arg("distribution-date")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

        let mut tx = pool.begin().await?;

        // Get current capital state
        let partner_record: Option<PartnerRecord> = sqlx::query_as(
            r#"
            SELECT id, partner_type, capital_commitment, capital_contributed, capital_returned
            FROM kyc.partnership_capital
            WHERE partnership_entity_id = $1 AND partner_entity_id = $2 AND is_active = true
            "#,
        )
        .bind(partnership_id)
        .bind(partner_id)
        .fetch_optional(&mut *tx)
        .await?;

        let partner_record =
            partner_record.ok_or_else(|| anyhow!("Partner not found in partnership"))?;

        let contributed = partner_record
            .capital_contributed
            .unwrap_or(rust_decimal::Decimal::ZERO);
        let old_returned = partner_record
            .capital_returned
            .unwrap_or(rust_decimal::Decimal::ZERO);

        // For capital returns, validate against contributed amount
        if distribution_type == "capital_return" {
            let max_returnable = contributed - old_returned;
            if amount > max_returnable {
                return Err(anyhow!(
                    "Cannot return {} - only {} is available (contributed {} minus already returned {})",
                    amount,
                    max_returnable,
                    contributed,
                    old_returned
                ));
            }
        }

        let new_returned = old_returned + amount;

        // Update capital_returned
        sqlx::query(
            r#"
            UPDATE kyc.partnership_capital
            SET capital_returned = $1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(new_returned)
        .bind(partner_record.id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let result = json!({
            "partnership_capital_id": partner_record.id.to_string(),
            "partnership_entity_id": partnership_id.to_string(),
            "partner_entity_id": partner_id.to_string(),
            "partner_type": partner_record.partner_type,
            "distribution_type": distribution_type,
            "distribution_amount": amount.to_string(),
            "previous_returned": old_returned.to_string(),
            "new_returned": new_returned.to_string(),
            "capital_contributed": contributed.to_string(),
            "distribution_date": distribution_date
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "Database feature required for partnership.record-distribution"
        ))
    }
}

// ============================================================================
// PartnershipReconcileOp - Reconcile profit shares
// ============================================================================

/// Reconciles that profit share percentages sum to 100%.
#[register_custom_op]
pub struct PartnershipReconcileOp;

#[cfg(feature = "database")]
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

#[async_trait]
impl CustomOperation for PartnershipReconcileOp {
    fn domain(&self) -> &'static str {
        "partnership"
    }

    fn verb(&self) -> &'static str {
        "reconcile"
    }

    fn rationale(&self) -> &'static str {
        "Reconciliation validates profit shares sum to 100% and identifies discrepancies"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let partnership_id = get_required_uuid(verb_call, "partnership-entity-id")?;

        // Get all active partners with their profit shares
        let partners: Vec<PartnerSummary> = sqlx::query_as(
            r#"
            SELECT
                pc.partner_entity_id,
                e.name as partner_name,
                pc.partner_type,
                pc.profit_share_pct,
                pc.voting_pct,
                pc.capital_commitment,
                pc.capital_contributed
            FROM kyc.partnership_capital pc
            JOIN "ob-poc".entities e ON pc.partner_entity_id = e.entity_id
            WHERE pc.partnership_entity_id = $1 AND pc.is_active = true
            ORDER BY pc.partner_type, pc.profit_share_pct DESC NULLS LAST
            "#,
        )
        .bind(partnership_id)
        .fetch_all(pool)
        .await?;

        let mut total_profit_share: f64 = 0.0;
        let mut total_voting: f64 = 0.0;
        let mut total_commitment: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
        let mut total_contributed: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
        let mut gp_count = 0;
        let mut lp_count = 0;

        let mut partner_details: Vec<serde_json::Value> = Vec::new();

        for p in &partners {
            let profit_pct: f64 = p
                .profit_share_pct
                .map(|d| d.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            total_profit_share += profit_pct;

            let voting: f64 = p
                .voting_pct
                .map(|d| d.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            total_voting += voting;

            let commitment = p.capital_commitment.unwrap_or(rust_decimal::Decimal::ZERO);
            total_commitment += commitment;

            let contributed = p.capital_contributed.unwrap_or(rust_decimal::Decimal::ZERO);
            total_contributed += contributed;

            match p.partner_type.as_str() {
                "GP" | "FOUNDING_PARTNER" => gp_count += 1,
                "LP" | "SPECIAL_LP" => lp_count += 1,
                "MEMBER" => {} // LLC member, count separately if needed
                _ => {}
            }

            partner_details.push(json!({
                "partner_id": p.partner_entity_id.to_string(),
                "partner_name": p.partner_name,
                "partner_type": p.partner_type,
                "profit_share_pct": profit_pct,
                "voting_pct": voting,
                "capital_commitment": commitment.to_string(),
                "capital_contributed": contributed.to_string()
            }));
        }

        let tolerance = 0.01; // 0.01% tolerance for rounding
        let is_profit_balanced = (total_profit_share - 100.0).abs() <= tolerance;
        let is_voting_balanced = (total_voting - 100.0).abs() <= tolerance || total_voting == 0.0;

        let mut issues: Vec<String> = Vec::new();
        if !is_profit_balanced {
            issues.push(format!(
                "Profit shares sum to {:.2}%, expected 100%",
                total_profit_share
            ));
        }
        if !is_voting_balanced {
            issues.push(format!(
                "Voting percentages sum to {:.2}%, expected 100%",
                total_voting
            ));
        }
        if gp_count == 0 && lp_count > 0 {
            issues.push("No General Partner (GP) found in limited partnership".to_string());
        }

        let result = json!({
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
            "status": if issues.is_empty() { "reconciled" } else { "discrepancies_found" },
            "partners": partner_details,
            "reconciled_at": chrono::Utc::now().to_rfc3339()
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "Database feature required for partnership.reconcile"
        ))
    }
}

// ============================================================================
// PartnershipAnalyzeControlOp - Analyze control in partnership
// ============================================================================

/// Analyzes control in a partnership (GP has presumptive control).
#[register_custom_op]
pub struct PartnershipAnalyzeControlOp;

#[cfg(feature = "database")]
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

#[async_trait]
impl CustomOperation for PartnershipAnalyzeControlOp {
    fn domain(&self) -> &'static str {
        "partnership"
    }

    fn verb(&self) -> &'static str {
        "analyze-control"
    }

    fn rationale(&self) -> &'static str {
        "Partnership control analysis identifies GPs (presumptive control) and LP investors"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let partnership_id = get_required_uuid(verb_call, "partnership-entity-id")?;

        // Get all partners with control-relevant info
        let partners: Vec<PartnerControlInfo> = sqlx::query_as(
            r#"
            SELECT
                pc.partner_entity_id,
                e.name as partner_name,
                pc.partner_type,
                pc.profit_share_pct,
                pc.voting_pct,
                pc.management_rights,
                EXISTS(
                    SELECT 1 FROM "ob-poc".entity_proper_persons pp
                    WHERE pp.entity_id = pc.partner_entity_id
                ) as is_natural_person
            FROM kyc.partnership_capital pc
            JOIN "ob-poc".entities e ON pc.partner_entity_id = e.entity_id
            WHERE pc.partnership_entity_id = $1 AND pc.is_active = true
            ORDER BY pc.partner_type, pc.profit_share_pct DESC NULLS LAST
            "#,
        )
        .bind(partnership_id)
        .fetch_all(pool)
        .await?;

        let mut controllers: Vec<serde_json::Value> = Vec::new();
        let mut gps: Vec<serde_json::Value> = Vec::new();
        let mut lps: Vec<serde_json::Value> = Vec::new();

        for p in &partners {
            let profit_pct: f64 = p
                .profit_share_pct
                .map(|d| d.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            let voting_pct: f64 = p
                .voting_pct
                .map(|d| d.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);

            let partner_info = json!({
                "partner_id": p.partner_entity_id.to_string(),
                "partner_name": p.partner_name,
                "partner_type": p.partner_type,
                "profit_share_pct": profit_pct,
                "voting_pct": voting_pct,
                "management_rights": p.management_rights.unwrap_or(false),
                "is_natural_person": p.is_natural_person.unwrap_or(false)
            });

            match p.partner_type.as_str() {
                "GP" | "FOUNDING_PARTNER" => {
                    gps.push(partner_info.clone());
                    // GPs have presumptive control
                    controllers.push(json!({
                        "controller_id": p.partner_entity_id.to_string(),
                        "controller_name": p.partner_name,
                        "control_type": "general_partner",
                        "control_strength": 0.95,
                        "is_natural_person": p.is_natural_person.unwrap_or(false),
                        "needs_further_tracing": !p.is_natural_person.unwrap_or(false)
                    }));
                }
                "LP" | "SPECIAL_LP" => {
                    lps.push(partner_info.clone());
                    // LPs with management rights or >50% voting also have control
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
                            "needs_further_tracing": !p.is_natural_person.unwrap_or(false)
                        }));
                    }
                }
                "MEMBER" => {
                    // LLC members - check if they have management rights or majority voting
                    if p.management_rights.unwrap_or(false) || voting_pct > 50.0 {
                        controllers.push(json!({
                            "controller_id": p.partner_entity_id.to_string(),
                            "controller_name": p.partner_name,
                            "control_type": "managing_member",
                            "control_strength": 0.80,
                            "voting_pct": voting_pct,
                            "is_natural_person": p.is_natural_person.unwrap_or(false),
                            "needs_further_tracing": !p.is_natural_person.unwrap_or(false)
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

        let result = json!({
            "partnership_entity_id": partnership_id.to_string(),
            "control_type": control_type,
            "general_partners": gps,
            "limited_partners": lps,
            "controllers": controllers,
            "gp_count": gps.len(),
            "lp_count": lps.len(),
            "total_partners": partners.len(),
            "analysis_notes": match control_type {
                "no_gp" => "WARNING: No General Partner found - unusual partnership structure",
                "llc_member_managed" => "LLC structure - members with management rights have control",
                "single_gp" => "Single GP has presumptive control per partnership law",
                "multiple_gps" => "Multiple GPs share control - further analysis may be needed",
                _ => "Unknown control structure"
            },
            "analysis_timestamp": chrono::Utc::now().to_rfc3339()
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "Database feature required for partnership.analyze-control"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partnership_ops_metadata() {
        let contribution = PartnershipContributionOp;
        assert_eq!(contribution.domain(), "partnership");
        assert_eq!(contribution.verb(), "record-contribution");

        let distribution = PartnershipDistributionOp;
        assert_eq!(distribution.domain(), "partnership");
        assert_eq!(distribution.verb(), "record-distribution");

        let reconcile = PartnershipReconcileOp;
        assert_eq!(reconcile.domain(), "partnership");
        assert_eq!(reconcile.verb(), "reconcile");

        let analyze = PartnershipAnalyzeControlOp;
        assert_eq!(analyze.domain(), "partnership");
        assert_eq!(analyze.verb(), "analyze-control");
    }
}
