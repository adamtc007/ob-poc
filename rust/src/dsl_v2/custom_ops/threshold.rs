//! Threshold Decision Matrix Operations
//!
//! These operations implement risk-based KYC threshold computation,
//! deriving document requirements and screening levels based on CBU risk factors.

use anyhow::Result;
use async_trait::async_trait;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::custom_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Derive KYC requirements based on CBU risk factors
pub struct ThresholdDeriveOp;

#[async_trait]
impl CustomOperation for ThresholdDeriveOp {
    fn domain(&self) -> &'static str {
        "threshold"
    }
    fn verb(&self) -> &'static str {
        "derive"
    }
    fn rationale(&self) -> &'static str {
        "Requires multi-table risk computation across threshold_factors, risk_bands, and threshold_requirements"
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

        // Get CBU details
        let cbu_row = sqlx::query!(
            r#"SELECT client_type, jurisdiction, source_of_funds, product_id
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        // Compute risk score
        let risk_result = sqlx::query!(
            r#"SELECT risk_score, risk_band, factors FROM "ob-poc".compute_cbu_risk_score($1)"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?;

        let (risk_score, risk_band, factors) = match risk_result {
            Some(row) => (
                row.risk_score.unwrap_or(0),
                row.risk_band.unwrap_or_else(|| "MEDIUM".to_string()),
                row.factors,
            ),
            None => (0, "MEDIUM".to_string(), None),
        };

        // Get requirements for this risk band (using document_type_code not document_type_id)
        let requirements = sqlx::query!(
            r#"SELECT tr.entity_role, tr.attribute_code, tr.is_required,
                      tr.confidence_min, tr.max_age_days, tr.must_be_authoritative,
                      array_agg(DISTINCT rad.document_type_code) FILTER (WHERE rad.document_type_code IS NOT NULL) as acceptable_docs
               FROM "ob-poc".threshold_requirements tr
               LEFT JOIN "ob-poc".requirement_acceptable_docs rad ON rad.requirement_id = tr.requirement_id
               WHERE tr.risk_band = $1
               GROUP BY tr.requirement_id, tr.entity_role, tr.attribute_code, tr.is_required,
                        tr.confidence_min, tr.max_age_days, tr.must_be_authoritative"#,
            &risk_band
        )
        .fetch_all(pool)
        .await?;

        // Get screening requirements
        let screenings = sqlx::query!(
            r#"SELECT sr.screening_type, sr.is_required, sr.frequency_months
               FROM "ob-poc".screening_requirements sr
               WHERE sr.risk_band = $1"#,
            &risk_band
        )
        .fetch_all(pool)
        .await?;

        let doc_requirements: Vec<serde_json::Value> = requirements
            .iter()
            .map(|r| {
                json!({
                    "entity_role": r.entity_role,
                    "attribute_code": r.attribute_code,
                    "is_required": r.is_required,
                    "confidence_min": r.confidence_min,
                    "max_age_days": r.max_age_days,
                    "must_be_authoritative": r.must_be_authoritative,
                    "acceptable_docs": r.acceptable_docs
                })
            })
            .collect();

        let screening_requirements: Vec<serde_json::Value> = screenings
            .iter()
            .map(|s| {
                json!({
                    "screening_type": s.screening_type,
                    "is_required": s.is_required,
                    "frequency_months": s.frequency_months
                })
            })
            .collect();

        let result = json!({
            "cbu_id": cbu_id,
            "risk_score": risk_score,
            "risk_band": risk_band,
            "factors": factors,
            "cbu_details": {
                "client_type": cbu_row.client_type,
                "jurisdiction": cbu_row.jurisdiction,
                "source_of_funds": cbu_row.source_of_funds
            },
            "document_requirements": doc_requirements,
            "screening_requirements": screening_requirements
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "risk_score": 0,
            "risk_band": "MEDIUM",
            "document_requirements": [],
            "screening_requirements": []
        })))
    }
}

/// Evaluate a specific entity against threshold requirements
pub struct ThresholdEvaluateOp;

#[async_trait]
impl CustomOperation for ThresholdEvaluateOp {
    fn domain(&self) -> &'static str {
        "threshold"
    }
    fn verb(&self) -> &'static str {
        "evaluate"
    }
    fn rationale(&self) -> &'static str {
        "Requires document count validation against role-specific requirements"
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

        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("entity-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        let role_filter = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("role"))
            .and_then(|a| a.value.as_string());

        // Get risk band
        let risk_result = sqlx::query!(
            r#"SELECT risk_band FROM "ob-poc".compute_cbu_risk_score($1)"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?;

        let risk_band = risk_result
            .and_then(|r| r.risk_band)
            .unwrap_or_else(|| "MEDIUM".to_string());

        // Get entity's roles
        let roles = sqlx::query!(
            r#"SELECT r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".roles r ON r.role_id = cer.role_id
               WHERE cer.cbu_id = $1 AND cer.entity_id = $2"#,
            cbu_id,
            entity_id
        )
        .fetch_all(pool)
        .await?;

        let role_names: Vec<String> = roles.iter().map(|r| r.role_name.clone()).collect();

        let mut evaluations = Vec::new();
        let mut all_met = true;

        for role_name in &role_names {
            if let Some(ref filter) = role_filter {
                if role_name != filter {
                    continue;
                }
            }

            let requirements = sqlx::query!(
                r#"SELECT tr.requirement_id, tr.attribute_code, tr.is_required, tr.confidence_min
                   FROM "ob-poc".threshold_requirements tr
                   WHERE tr.risk_band = $1 AND tr.entity_role = $2"#,
                &risk_band,
                role_name
            )
            .fetch_all(pool)
            .await?;

            for req in requirements {
                // Clone confidence_min before binding since BigDecimal doesn't impl Copy
                let confidence_min_for_json = req.confidence_min.clone();

                // Check observations using attribute_id (uuid) not attribute name
                let obs_count: Option<i64> = sqlx::query_scalar(
                    r#"SELECT COUNT(*) FROM "ob-poc".attribute_observations ao
                       JOIN "ob-poc".attribute_registry ar ON ar.uuid = ao.attribute_id
                       WHERE ao.entity_id = $1
                       AND ar.id = $2
                       AND ao.status = 'ACTIVE'
                       AND ao.confidence >= $3"#,
                )
                .bind(entity_id)
                .bind(&req.attribute_code)
                .bind(req.confidence_min)
                .fetch_one(pool)
                .await?;

                let count = obs_count.unwrap_or(0);
                let met = count > 0;

                if req.is_required && !met {
                    all_met = false;
                }

                evaluations.push(json!({
                    "role": role_name,
                    "attribute_code": req.attribute_code,
                    "is_required": req.is_required,
                    "confidence_min": confidence_min_for_json,
                    "observations_found": count,
                    "met": met
                }));
            }
        }

        let result = json!({
            "cbu_id": cbu_id,
            "entity_id": entity_id,
            "risk_band": risk_band,
            "roles": role_names,
            "all_requirements_met": all_met,
            "evaluations": evaluations
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "all_requirements_met": true,
            "evaluations": []
        })))
    }
}

/// Check all entities in a CBU against threshold requirements
pub struct ThresholdCheckEntityOp;

#[async_trait]
impl CustomOperation for ThresholdCheckEntityOp {
    fn domain(&self) -> &'static str {
        "threshold"
    }
    fn verb(&self) -> &'static str {
        "check-entity"
    }
    fn rationale(&self) -> &'static str {
        "Batch evaluation of entity compliance against threshold requirements"
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

        let entities = sqlx::query!(
            r#"SELECT DISTINCT cer.entity_id, e.name as entity_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
               WHERE cer.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let risk_result = sqlx::query!(
            r#"SELECT risk_band FROM "ob-poc".compute_cbu_risk_score($1)"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?;

        let risk_band = risk_result
            .and_then(|r| r.risk_band)
            .unwrap_or_else(|| "MEDIUM".to_string());

        let mut entity_results = Vec::new();
        let mut all_compliant = true;

        for entity in entities {
            let roles: Vec<String> = sqlx::query_scalar(
                r#"SELECT r.name FROM "ob-poc".cbu_entity_roles cer
                   JOIN "ob-poc".roles r ON r.role_id = cer.role_id
                   WHERE cer.cbu_id = $1 AND cer.entity_id = $2"#,
            )
            .bind(cbu_id)
            .bind(entity.entity_id)
            .fetch_all(pool)
            .await?;

            // Count missing required attributes
            let missing_count: Option<i64> = sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM "ob-poc".threshold_requirements tr
                   WHERE tr.risk_band = $1
                   AND tr.entity_role = ANY($2)
                   AND tr.is_required = true
                   AND NOT EXISTS (
                       SELECT 1 FROM "ob-poc".attribute_observations ao
                       JOIN "ob-poc".attribute_registry ar ON ar.uuid = ao.attribute_id
                       WHERE ao.entity_id = $3
                       AND ar.id = tr.attribute_code
                       AND ao.status = 'ACTIVE'
                       AND ao.confidence >= tr.confidence_min
                   )"#,
            )
            .bind(&risk_band)
            .bind(&roles)
            .bind(entity.entity_id)
            .fetch_one(pool)
            .await?;

            let missing = missing_count.unwrap_or(0);
            let compliant = missing == 0;
            if !compliant {
                all_compliant = false;
            }

            entity_results.push(json!({
                "entity_id": entity.entity_id,
                "entity_name": entity.entity_name,
                "roles": roles,
                "compliant": compliant,
                "missing_required_attributes": missing
            }));
        }

        let result = json!({
            "cbu_id": cbu_id,
            "risk_band": risk_band,
            "all_entities_compliant": all_compliant,
            "entity_count": entity_results.len(),
            "entities": entity_results
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "all_entities_compliant": true,
            "entities": []
        })))
    }
}
