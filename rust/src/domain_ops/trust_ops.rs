//! Trust Operations - Trust control analysis and UBO identification
//!
//! Plugin handlers for trust.yaml verbs that require custom logic.
//!
//! Uses a combination of:
//! - `"ob-poc".trust_parties` - High-level party roles (SETTLOR, TRUSTEE, BENEFICIARY, PROTECTOR)
//! - `"ob-poc".entity_relationships` (relationship_type='trust_role') - Unified relationship tracking
//! - `"ob-poc".trust_provisions` - Granular deed provisions and powers for control analysis

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::get_required_uuid;
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

// ============================================================================
// TrustAnalyzeControlOp - Analyze control over a trust
// ============================================================================

/// Analyzes trust provisions to determine control vectors.
/// Uses "ob-poc".trust_provisions for granular power analysis.
#[register_custom_op]
pub struct TrustAnalyzeControlOp;

#[cfg(feature = "database")]
#[derive(Debug, sqlx::FromRow)]
struct TrustProvision {
    holder_entity_id: Option<Uuid>,
    holder_name: Option<String>,
    provision_type: String,
    discretion_level: Option<String>,
}

#[cfg(feature = "database")]
#[derive(Debug, sqlx::FromRow)]
struct TrustParty {
    entity_id: Uuid,
    entity_name: Option<String>,
    party_role: String,
}

#[cfg(feature = "database")]
async fn trust_analyze_control_impl(
    trust_entity_id: Uuid,
    cbu_id: Uuid,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let trust_id: Option<Uuid> =
        sqlx::query_scalar(r#"SELECT trust_id FROM "ob-poc".entity_trusts WHERE entity_id = $1"#)
            .bind(trust_entity_id)
            .fetch_optional(pool)
            .await?;

    let parties: Vec<TrustParty> = if let Some(tid) = trust_id {
        sqlx::query_as(
            r#"
            SELECT tp.entity_id, e.name as entity_name, tp.party_role
            FROM "ob-poc".trust_parties tp
            JOIN "ob-poc".entities e ON tp.entity_id = e.entity_id
            WHERE tp.trust_id = $1 AND tp.is_active = true
              AND e.deleted_at IS NULL
            ORDER BY tp.party_role
            "#,
        )
        .bind(tid)
        .fetch_all(pool)
        .await?
    } else {
        Vec::new()
    };

    let provisions: Vec<TrustProvision> = sqlx::query_as(
        r#"
        SELECT
            tp.holder_entity_id,
            e.name as holder_name,
            tp.provision_type,
            tp.discretion_level
        FROM "ob-poc".trust_provisions tp
        LEFT JOIN "ob-poc".entities e ON tp.holder_entity_id = e.entity_id
        WHERE tp.cbu_id = $1 AND tp.trust_entity_id = $2 AND tp.is_active = true
          AND (e.entity_id IS NULL OR e.deleted_at IS NULL)
        ORDER BY tp.provision_type
        "#,
    )
    .bind(cbu_id)
    .bind(trust_entity_id)
    .fetch_all(pool)
    .await?;

    let mut control_vectors: Vec<serde_json::Value> = Vec::new();
    let mut controllers: HashMap<String, Vec<String>> = HashMap::new();

    for party in &parties {
        let holder_id = party.entity_id.to_string();
        let role = party.party_role.to_uppercase();

        match role.as_str() {
            "TRUSTEE" => {
                control_vectors.push(json!({
                    "vector_type": "trustee",
                    "holder_id": holder_id,
                    "holder_name": party.entity_name,
                    "strength": 0.6,
                    "description": "Trustee with legal title and administrative powers"
                }));
                controllers
                    .entry(holder_id)
                    .or_default()
                    .push("trustee".to_string());
            }
            "PROTECTOR" => {
                control_vectors.push(json!({
                    "vector_type": "protector",
                    "holder_id": holder_id,
                    "holder_name": party.entity_name,
                    "strength": 0.7,
                    "description": "Trust protector with oversight powers"
                }));
                controllers
                    .entry(holder_id)
                    .or_default()
                    .push("protector".to_string());
            }
            "SETTLOR" => {
                control_vectors.push(json!({
                    "vector_type": "settlor",
                    "holder_id": holder_id,
                    "holder_name": party.entity_name,
                    "strength": 0.3,
                    "description": "Settlor (typically divested of control)"
                }));
            }
            "BENEFICIARY" => {
                control_vectors.push(json!({
                    "vector_type": "beneficiary",
                    "holder_id": holder_id,
                    "holder_name": party.entity_name,
                    "strength": 0.3,
                    "description": "Beneficiary with interest in trust assets"
                }));
            }
            _ => {}
        }
    }

    for prov in &provisions {
        if let Some(holder_id) = prov.holder_entity_id {
            let holder_id_str = holder_id.to_string();

            let (vector_type, strength, description) = match prov.provision_type.as_str() {
                "APPOINTOR_POWER" => ("appointor", 0.85, "Power to appoint and remove trustees"),
                "TRUSTEE_REMOVAL" => ("trustee_removal", 0.80, "Power to remove trustees"),
                "PROTECTOR_POWER" => (
                    "protector_veto",
                    0.75,
                    "Protector with veto or consent powers",
                ),
                "TRUST_VARIATION" => ("trust_variation", 0.70, "Power to vary trust terms"),
                "ADD_BENEFICIARY" | "EXCLUDE_BENEFICIARY" => (
                    "beneficiary_control",
                    0.65,
                    "Power to add or exclude beneficiaries",
                ),
                "INVESTMENT_DIRECTION" => {
                    ("investment_direction", 0.50, "Power to direct investments")
                }
                "DISTRIBUTION_DIRECTION" => (
                    "distribution_direction",
                    0.60,
                    "Power to direct distributions",
                ),
                "RESERVED_POWER" => ("reserved_power", 0.70, "Settlor reserved powers"),
                _ => continue,
            };

            control_vectors.push(json!({
                "vector_type": vector_type,
                "holder_id": holder_id_str,
                "holder_name": prov.holder_name,
                "provision_type": prov.provision_type,
                "strength": strength,
                "discretion_level": prov.discretion_level,
                "description": description
            }));

            controllers
                .entry(holder_id_str)
                .or_default()
                .push(prov.provision_type.clone());
        }
    }

    let significant_provisions = [
        "APPOINTOR_POWER",
        "TRUSTEE_REMOVAL",
        "TRUST_VARIATION",
        "RESERVED_POWER",
    ];

    let mut primary_controllers: Vec<serde_json::Value> = Vec::new();
    for (holder_id, vectors) in &controllers {
        let has_significant = vectors
            .iter()
            .any(|v| significant_provisions.contains(&v.as_str()));
        if has_significant {
            primary_controllers.push(json!({
                "controller_id": holder_id,
                "control_vectors": vectors
            }));
        }
    }

    Ok(json!({
        "trust_entity_id": trust_entity_id.to_string(),
        "cbu_id": cbu_id.to_string(),
        "party_count": parties.len(),
        "provision_count": provisions.len(),
        "control_vectors": control_vectors,
        "controllers": controllers,
        "primary_controllers": primary_controllers,
        "control_type": match primary_controllers.len() {
            0 => "diffuse",
            1 => "single",
            _ => "joint"
        },
        "analysis_timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[async_trait]
impl CustomOperation for TrustAnalyzeControlOp {
    fn domain(&self) -> &'static str {
        "trust"
    }

    fn verb(&self) -> &'static str {
        "analyze-control"
    }

    fn rationale(&self) -> &'static str {
        "Trust control analysis requires interpreting trust deed provisions and mapping them to control vectors"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_uuid;

        let trust_entity_id = json_extract_uuid(args, ctx, "trust-entity-id")?;
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let result = trust_analyze_control_impl(trust_entity_id, cbu_id, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl TrustAnalyzeControlOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let trust_entity_id = get_required_uuid(verb_call, "trust-entity-id")?;
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;
        let result = trust_analyze_control_impl(trust_entity_id, cbu_id, pool).await?;
        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "Database feature required for trust.analyze-control"
        ))
    }
}

// ============================================================================
// TrustIdentifyUbosOp - Identify UBOs from trust structure
// ============================================================================

/// Identifies beneficial owners from trust provisions using regulatory rules.
#[register_custom_op]
pub struct TrustIdentifyUbosOp;

#[cfg(feature = "database")]
#[derive(Debug, sqlx::FromRow)]
struct BeneficiaryProvision {
    holder_entity_id: Option<Uuid>,
    holder_name: Option<String>,
    provision_type: String,
    beneficiary_class: Option<String>,
    interest_percentage: Option<rust_decimal::Decimal>,
    discretion_level: Option<String>,
    is_natural_person: Option<bool>,
}

#[cfg(feature = "database")]
async fn trust_identify_ubos_impl(
    trust_entity_id: Uuid,
    cbu_id: Uuid,
    threshold: f64,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let provisions: Vec<BeneficiaryProvision> = sqlx::query_as(
        r#"
        SELECT
            tp.holder_entity_id,
            e.name as holder_name,
            tp.provision_type,
            tp.beneficiary_class,
            tp.interest_percentage,
            tp.discretion_level,
            EXISTS(
                SELECT 1 FROM "ob-poc".entity_proper_persons pp
                WHERE pp.entity_id = tp.holder_entity_id
            ) as is_natural_person
        FROM "ob-poc".trust_provisions tp
        LEFT JOIN "ob-poc".entities e ON tp.holder_entity_id = e.entity_id
        WHERE tp.cbu_id = $1 AND tp.trust_entity_id = $2 AND tp.is_active = true
          AND (e.entity_id IS NULL OR e.deleted_at IS NULL)
        "#,
    )
    .bind(cbu_id)
    .bind(trust_entity_id)
    .fetch_all(pool)
    .await?;

    let mut ubos: Vec<serde_json::Value> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut class_beneficiaries: Vec<serde_json::Value> = Vec::new();

    for prov in &provisions {
        let Some(holder_id) = prov.holder_entity_id else {
            if let Some(ref class_desc) = prov.beneficiary_class {
                class_beneficiaries.push(json!({
                    "beneficiary_class": class_desc,
                    "provision_type": prov.provision_type,
                    "interest_percentage": prov.interest_percentage.map(|p| p.to_string()),
                    "note": "Class beneficiary - requires identification of class members"
                }));
            }
            continue;
        };
        let holder_id_str = holder_id.to_string();

        if seen.contains(&holder_id_str) {
            continue;
        }

        let mut is_ubo = false;
        let mut ubo_reasons: Vec<String> = Vec::new();

        match prov.provision_type.as_str() {
            "APPOINTOR_POWER" => {
                is_ubo = true;
                ubo_reasons.push("Appointor - can appoint/remove trustees".to_string());
            }
            "TRUSTEE_REMOVAL" => {
                is_ubo = true;
                ubo_reasons.push("Power to remove trustees".to_string());
            }
            "PROTECTOR_POWER" => {
                is_ubo = true;
                ubo_reasons.push("Protector with consent/veto powers".to_string());
            }
            "TRUST_VARIATION" => {
                is_ubo = true;
                ubo_reasons.push("Power to vary trust terms".to_string());
            }
            "RESERVED_POWER" => {
                is_ubo = true;
                ubo_reasons.push("Settlor with reserved powers".to_string());
            }
            "INCOME_BENEFICIARY" | "CAPITAL_BENEFICIARY" => {
                let pct: f64 = prov
                    .interest_percentage
                    .map(|p| p.to_string().parse().unwrap_or(0.0))
                    .unwrap_or(0.0);
                if pct >= threshold {
                    is_ubo = true;
                    ubo_reasons.push(format!(
                        "Fixed interest beneficiary ({}% >= {}% threshold)",
                        pct, threshold
                    ));
                }
            }
            "DISCRETIONARY_BENEFICIARY" => match prov.discretion_level.as_deref() {
                Some("NONE") | Some("FETTERED") => {
                    is_ubo = true;
                    ubo_reasons.push(
                        "Discretionary beneficiary with limited trustee discretion".to_string(),
                    );
                }
                _ => {}
            },
            _ => {}
        }

        if is_ubo {
            seen.insert(holder_id_str.clone());
            let needs_tracing = !prov.is_natural_person.unwrap_or(false);

            ubos.push(json!({
                "entity_id": holder_id_str,
                "entity_name": prov.holder_name,
                "provision_type": prov.provision_type,
                "is_natural_person": prov.is_natural_person,
                "needs_further_tracing": needs_tracing,
                "ubo_reasons": ubo_reasons,
                "interest_percentage": prov.interest_percentage.map(|p| p.to_string()),
                "discretion_level": prov.discretion_level
            }));
        }
    }

    Ok(json!({
        "trust_entity_id": trust_entity_id.to_string(),
        "cbu_id": cbu_id.to_string(),
        "threshold_percentage": threshold,
        "ubos": ubos,
        "class_beneficiaries": class_beneficiaries,
        "ubo_count": ubos.len(),
        "natural_person_count": ubos.iter().filter(|u| u["is_natural_person"] == true).count(),
        "entities_needing_tracing": ubos.iter().filter(|u| u["needs_further_tracing"] == true).count(),
        "class_beneficiary_count": class_beneficiaries.len(),
        "identified_at": chrono::Utc::now().to_rfc3339()
    }))
}

#[async_trait]
impl CustomOperation for TrustIdentifyUbosOp {
    fn domain(&self) -> &'static str {
        "trust"
    }

    fn verb(&self) -> &'static str {
        "identify-ubos"
    }

    fn rationale(&self) -> &'static str {
        "UBO identification for trusts requires applying regulatory rules to provision holders"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};

        let trust_entity_id = json_extract_uuid(args, ctx, "trust-entity-id")?;
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let threshold = json_extract_string_opt(args, "threshold")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(25.0);
        let result = trust_identify_ubos_impl(trust_entity_id, cbu_id, threshold, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl TrustIdentifyUbosOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let trust_entity_id = get_required_uuid(verb_call, "trust-entity-id")?;
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        let threshold = verb_call
            .get_arg("threshold")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(25.0);
        let result = trust_identify_ubos_impl(trust_entity_id, cbu_id, threshold, pool).await?;
        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required for trust.identify-ubos"))
    }
}

// ============================================================================
// TrustClassifyOp - Classify trust type based on provisions
// ============================================================================

/// Classifies trust type based on its provisions (discretionary, fixed, hybrid).
#[register_custom_op]
pub struct TrustClassifyOp;

#[cfg(feature = "database")]
#[derive(Debug, sqlx::FromRow)]
struct ProvisionSummary {
    provision_type: String,
    discretion_level: Option<String>,
    count: Option<i64>,
    total_interest: Option<rust_decimal::Decimal>,
}

#[cfg(feature = "database")]
async fn trust_classify_impl(
    trust_entity_id: Uuid,
    cbu_id: Uuid,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let provisions: Vec<ProvisionSummary> = sqlx::query_as(
        r#"
        SELECT
            provision_type,
            discretion_level,
            COUNT(*) as count,
            SUM(COALESCE(interest_percentage, 0)) as total_interest
        FROM "ob-poc".trust_provisions
        WHERE cbu_id = $1 AND trust_entity_id = $2 AND is_active = true
        GROUP BY provision_type, discretion_level
        ORDER BY provision_type
        "#,
    )
    .bind(cbu_id)
    .bind(trust_entity_id)
    .fetch_all(pool)
    .await?;

    let mut has_fixed = false;
    let mut has_discretionary = false;
    let mut has_contingent = false;
    let mut total_fixed_pct: f64 = 0.0;
    let mut has_absolute_discretion = false;

    for prov in &provisions {
        match prov.provision_type.as_str() {
            "INCOME_BENEFICIARY" | "CAPITAL_BENEFICIARY" => {
                has_fixed = true;
                total_fixed_pct += prov
                    .total_interest
                    .map(|p| p.to_string().parse().unwrap_or(0.0))
                    .unwrap_or(0.0);
            }
            "DISCRETIONARY_BENEFICIARY" => {
                has_discretionary = true;
                if prov.discretion_level.as_deref() == Some("ABSOLUTE") {
                    has_absolute_discretion = true;
                }
            }
            "CONTINGENT_BENEFICIARY" | "REMAINDER_BENEFICIARY" => {
                has_contingent = true;
            }
            _ => {}
        }
    }

    let classification = if has_fixed && !has_discretionary && total_fixed_pct >= 99.0 {
        "fixed_interest"
    } else if has_discretionary && !has_fixed && has_absolute_discretion {
        "fully_discretionary"
    } else if has_discretionary && !has_fixed {
        "discretionary"
    } else if has_fixed && has_discretionary {
        "hybrid"
    } else if has_contingent && !has_fixed && !has_discretionary {
        "contingent"
    } else if provisions.is_empty() {
        "no_provisions_recorded"
    } else {
        "unclassified"
    };

    Ok(json!({
        "trust_entity_id": trust_entity_id.to_string(),
        "cbu_id": cbu_id.to_string(),
        "classification": classification,
        "has_fixed_interests": has_fixed,
        "has_discretionary_interests": has_discretionary,
        "has_contingent_interests": has_contingent,
        "has_absolute_discretion": has_absolute_discretion,
        "total_fixed_percentage": total_fixed_pct,
        "provision_breakdown": provisions.iter().map(|p| json!({
            "provision_type": p.provision_type,
            "discretion_level": p.discretion_level,
            "count": p.count,
            "total_interest_pct": p.total_interest.map(|i| i.to_string())
        })).collect::<Vec<_>>(),
        "classified_at": chrono::Utc::now().to_rfc3339()
    }))
}

#[async_trait]
impl CustomOperation for TrustClassifyOp {
    fn domain(&self) -> &'static str {
        "trust"
    }

    fn verb(&self) -> &'static str {
        "classify"
    }

    fn rationale(&self) -> &'static str {
        "Trust classification requires analyzing beneficiary interests and trustee discretion"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_uuid;

        let trust_entity_id = json_extract_uuid(args, ctx, "trust-entity-id")?;
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let result = trust_classify_impl(trust_entity_id, cbu_id, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl TrustClassifyOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let trust_entity_id = get_required_uuid(verb_call, "trust-entity-id")?;
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;
        let result = trust_classify_impl(trust_entity_id, cbu_id, pool).await?;
        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required for trust.classify"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_ops_metadata() {
        let analyze = TrustAnalyzeControlOp;
        assert_eq!(analyze.domain(), "trust");
        assert_eq!(analyze.verb(), "analyze-control");

        let identify = TrustIdentifyUbosOp;
        assert_eq!(identify.domain(), "trust");
        assert_eq!(identify.verb(), "identify-ubos");

        let classify = TrustClassifyOp;
        assert_eq!(classify.domain(), "trust");
        assert_eq!(classify.verb(), "classify");
    }
}
