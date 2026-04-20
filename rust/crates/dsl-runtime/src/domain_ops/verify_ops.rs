//! Verification Operations (Adversarial Agent Model)
//!
//! Plugin handlers for the verify.* domain verbs that implement
//! game-theoretic "Distrust And Verify" model for KYC.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

async fn verify_calculate_confidence_impl(
    entity_id: uuid::Uuid,
    attribute: Option<String>,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    use crate::verification::{
        ConfidenceCalculator, Evidence, EvidenceSource, InconsistencySeverity, PatternSeverity,
    };
    use chrono::Utc;
    use uuid::Uuid;

    let mut query = String::from(
        r#"SELECT
            ao.observation_id,
            ao.attribute_id,
            ao.source_type,
            ao.confidence,
            ao.is_authoritative,
            ao.observed_at,
            ar.name as attribute_name
        FROM "ob-poc".attribute_observations ao
        JOIN "ob-poc".attribute_registry ar ON ao.attribute_id = ar.uuid
        WHERE ao.entity_id = $1 AND ao.status = 'ACTIVE'"#,
    );

    if attribute.is_some() {
        query.push_str(" AND ar.name = $2");
    }

    #[derive(sqlx::FromRow)]
    struct ObservationRow {
        observation_id: Uuid,
        attribute_id: Uuid,
        source_type: Option<String>,
        confidence: Option<sqlx::types::BigDecimal>,
        is_authoritative: Option<bool>,
        observed_at: Option<chrono::DateTime<chrono::Utc>>,
        #[allow(dead_code)]
        attribute_name: String,
    }

    let observations: Vec<ObservationRow> = if let Some(ref attr) = attribute {
        sqlx::query_as(&query)
            .bind(entity_id)
            .bind(attr)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as(&query)
            .bind(entity_id)
            .fetch_all(pool)
            .await?
    };

    if observations.is_empty() {
        return Ok(serde_json::json!({
            "entity_id": entity_id,
            "score": 0.0,
            "band": "REJECTED",
            "message": "No observations found for entity"
        }));
    }

    let evidence: Vec<Evidence> = observations
        .iter()
        .map(|o| {
            let source: EvidenceSource = o
                .source_type
                .as_ref()
                .map(|s| EvidenceSource::from(s.as_str()))
                .unwrap_or(EvidenceSource::Allegation);

            Evidence {
                evidence_id: o.observation_id,
                entity_id,
                attribute_id: o.attribute_id,
                observed_value: serde_json::json!(null),
                source,
                confidence: o
                    .confidence
                    .as_ref()
                    .map(|d| d.to_string().parse().unwrap_or(0.5))
                    .unwrap_or(0.5),
                is_authoritative: o.is_authoritative.unwrap_or(false),
                observed_at: o.observed_at.unwrap_or_else(chrono::Utc::now),
                source_document_id: None,
                extraction_method: None,
                effective_from: None,
                effective_to: None,
            }
        })
        .collect();

    let calculator = ConfidenceCalculator::new();

    let pattern_rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT severity FROM "ob-poc".detected_patterns
           WHERE $1 = ANY(involved_entities) AND status = 'DETECTED'"#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let pattern_severities: Vec<PatternSeverity> = pattern_rows
        .iter()
        .map(|(s,)| match s.to_uppercase().as_str() {
            "LOW" => PatternSeverity::Low,
            "MEDIUM" => PatternSeverity::Medium,
            "HIGH" => PatternSeverity::High,
            "CRITICAL" => PatternSeverity::Critical,
            _ => PatternSeverity::Low,
        })
        .collect();

    let inconsistency_rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT severity FROM "ob-poc".observation_discrepancies
           WHERE entity_id = $1 AND resolution_status = 'OPEN'"#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let inconsistencies: Vec<(InconsistencySeverity, f64)> = inconsistency_rows
        .iter()
        .map(|(s,)| {
            let severity = match s.to_uppercase().as_str() {
                "INFO" => InconsistencySeverity::Info,
                "LOW" => InconsistencySeverity::Low,
                "MEDIUM" => InconsistencySeverity::Medium,
                "HIGH" => InconsistencySeverity::High,
                "CRITICAL" => InconsistencySeverity::Critical,
                _ => InconsistencySeverity::Low,
            };
            (severity, 1.0)
        })
        .collect();

    let result = calculator.calculate(&evidence, &inconsistencies, &pattern_severities, Utc::now());

    Ok(serde_json::json!({
        "entity_id": entity_id,
        "score": result.score,
        "band": format!("{}", result.band),
        "observation_count": evidence.len(),
        "pattern_count": pattern_severities.len(),
        "inconsistency_count": inconsistencies.len(),
        "breakdown": {
            "base_score": result.breakdown.base_score,
            "authoritative_bonus": result.breakdown.authoritative_bonus,
            "recency_factor": result.breakdown.recency_factor,
            "corroboration_bonus": result.breakdown.corroboration_bonus,
            "inconsistency_penalty": result.breakdown.inconsistency_penalty,
            "pattern_penalty": result.breakdown.pattern_penalty
        }
    }))
}

async fn verify_assert_impl(
    cbu_id: uuid::Uuid,
    entity_id: Option<uuid::Uuid>,
    min_confidence: f64,
    fail_action: &str,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    use crate::verification::{
        ConfidenceCalculator, Evidence, EvidenceSource, InconsistencySeverity, PatternSeverity,
    };
    use chrono::Utc;
    use uuid::Uuid;

    let query = if let Some(eid) = entity_id {
        format!(
            r#"SELECT ao.observation_id, ao.entity_id, ao.attribute_id, ao.source_type, ao.confidence, ao.is_authoritative, ao.observed_at
               FROM "ob-poc".attribute_observations ao
               WHERE ao.entity_id = '{}' AND ao.status = 'ACTIVE'"#,
            eid
        )
    } else {
        format!(
            r#"SELECT ao.observation_id, ao.entity_id, ao.attribute_id, ao.source_type, ao.confidence, ao.is_authoritative, ao.observed_at
               FROM "ob-poc".attribute_observations ao
               JOIN "ob-poc".cbu_entity_roles cer ON ao.entity_id = cer.entity_id
               WHERE cer.cbu_id = '{}' AND ao.status = 'ACTIVE'"#,
            cbu_id
        )
    };

    #[derive(sqlx::FromRow)]
    struct ObsRow {
        observation_id: Uuid,
        entity_id: Uuid,
        attribute_id: Uuid,
        source_type: Option<String>,
        confidence: Option<sqlx::types::BigDecimal>,
        is_authoritative: Option<bool>,
        observed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let observations: Vec<ObsRow> = sqlx::query_as(&query).fetch_all(pool).await?;

    if observations.is_empty() {
        let msg = "No observations found - cannot verify confidence";
        return match fail_action {
            "error" => Err(anyhow::anyhow!(msg)),
            "warn" => Ok(serde_json::json!({
                "passed": false,
                "score": 0.0,
                "threshold": min_confidence,
                "warning": msg
            })),
            "block" => Ok(serde_json::json!({
                "passed": false,
                "blocked": true,
                "score": 0.0,
                "threshold": min_confidence,
                "reason": msg
            })),
            _ => Err(anyhow::anyhow!(msg)),
        };
    }

    let evidence: Vec<Evidence> = observations
        .iter()
        .map(|o| Evidence {
            evidence_id: o.observation_id,
            entity_id: o.entity_id,
            attribute_id: o.attribute_id,
            observed_value: serde_json::json!(null),
            source: o
                .source_type
                .as_ref()
                .map(|s| EvidenceSource::from(s.as_str()))
                .unwrap_or(EvidenceSource::Allegation),
            confidence: o
                .confidence
                .as_ref()
                .map(|d| d.to_string().parse().unwrap_or(0.5))
                .unwrap_or(0.5),
            is_authoritative: o.is_authoritative.unwrap_or(false),
            observed_at: o.observed_at.unwrap_or_else(chrono::Utc::now),
            source_document_id: None,
            extraction_method: None,
            effective_from: None,
            effective_to: None,
        })
        .collect();

    let calculator = ConfidenceCalculator::new();
    let empty_inconsistencies: Vec<(InconsistencySeverity, f64)> = vec![];
    let empty_patterns: Vec<PatternSeverity> = vec![];
    let result = calculator.calculate(
        &evidence,
        &empty_inconsistencies,
        &empty_patterns,
        Utc::now(),
    );
    let passed = result.score >= min_confidence;

    if !passed {
        let msg = format!(
            "Confidence {} below threshold {}",
            result.score, min_confidence
        );
        return match fail_action {
            "error" => Err(anyhow::anyhow!(msg)),
            "warn" => Ok(serde_json::json!({
                "passed": false,
                "score": result.score,
                "band": format!("{}", result.band),
                "threshold": min_confidence,
                "warning": msg
            })),
            "block" => Ok(serde_json::json!({
                "passed": false,
                "blocked": true,
                "score": result.score,
                "band": format!("{}", result.band),
                "threshold": min_confidence,
                "reason": msg
            })),
            _ => Err(anyhow::anyhow!(msg)),
        };
    }

    Ok(serde_json::json!({
        "passed": true,
        "score": result.score,
        "band": format!("{}", result.band),
        "threshold": min_confidence
    }))
}

#[register_custom_op]
pub struct VerifyDetectPatternsOp;

#[async_trait]
impl CustomOperation for VerifyDetectPatternsOp {
    fn domain(&self) -> &'static str {
        "verify"
    }
    fn verb(&self) -> &'static str {
        "detect-patterns"
    }
    fn rationale(&self) -> &'static str {
        "Requires graph traversal for circular ownership, layering, and nominee detection"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::verification::PatternDetector;
        use uuid::Uuid;

        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let detector = PatternDetector::new();
        let patterns = detector.detect_all(pool, cbu_id).await?;

        for pattern in &patterns {
            let evidence_json = serde_json::to_value(&pattern.evidence)?;
            let involved: Vec<Uuid> = pattern.involved_entities.clone();

            sqlx::query(
                r#"INSERT INTO "ob-poc".detected_patterns
                   (cbu_id, case_id, pattern_type, severity, description, involved_entities, evidence)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(cbu_id)
            .bind(case_id)
            .bind(pattern.pattern_type.as_str())
            .bind(pattern.severity.as_str())
            .bind(&pattern.description)
            .bind(&involved)
            .bind(&evidence_json)
            .execute(pool)
            .await?;
        }

        let result: Vec<serde_json::Value> = patterns
            .iter()
            .map(|p| {
                serde_json::json!({
                    "pattern_type": p.pattern_type.as_str(),
                    "severity": p.severity.as_str(),
                    "description": p.description,
                    "involved_entities": p.involved_entities,
                    "evidence": p.evidence
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct VerifyDetectEvasionOp;

#[async_trait]
impl CustomOperation for VerifyDetectEvasionOp {
    fn domain(&self) -> &'static str {
        "verify"
    }
    fn verb(&self) -> &'static str {
        "detect-evasion"
    }
    fn rationale(&self) -> &'static str {
        "Requires behavioral analysis of document request history"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::verification::EvasionDetector;

        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let detector = EvasionDetector::new();
        let report = detector.analyze(pool, case_id).await?;
        let result = serde_json::json!({
            "case_id": case_id,
            "evasion_score": report.evasion_score,
            "classification": format!("{:?}", report.classification),
            "signals": report.signals.iter().map(|s| {
                serde_json::json!({
                    "signal": format!("{:?}", s.signal),
                    "severity": format!("{:?}", s.severity),
                    "description": s.description,
                    "metric_value": s.metric_value,
                    "threshold": s.threshold
                })
            }).collect::<Vec<_>>(),
            "recommendation": format!("{:?}", report.recommendation),
            "metrics": {
                "total_requests": report.metrics.total_requests,
                "fulfilled_requests": report.metrics.fulfilled_requests,
                "pending_requests": report.metrics.pending_requests,
                "rejected_requests": report.metrics.rejected_requests,
                "waived_requests": report.metrics.waived_requests,
                "avg_response_days": report.metrics.avg_response_days,
                "max_response_days": report.metrics.max_response_days,
                "extension_count": report.metrics.extension_count,
                "rejection_rate": report.metrics.rejection_rate,
                "completion_rate": report.metrics.completion_rate,
                "followup_response_rate": report.metrics.followup_response_rate
            }
        });
        Ok(VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct VerifyCalculateConfidenceOp;

#[async_trait]
impl CustomOperation for VerifyCalculateConfidenceOp {
    fn domain(&self) -> &'static str {
        "verify"
    }
    fn verb(&self) -> &'static str {
        "calculate-confidence"
    }
    fn rationale(&self) -> &'static str {
        "Requires weighted aggregation with source, recency, and corroboration modifiers"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let attribute = json_extract_string_opt(args, "attribute");
        let result = verify_calculate_confidence_impl(entity_id, attribute, pool).await?;
        Ok(VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct VerifyGetStatusOp;

#[async_trait]
impl CustomOperation for VerifyGetStatusOp {
    fn domain(&self) -> &'static str {
        "verify"
    }
    fn verb(&self) -> &'static str {
        "get-status"
    }
    fn rationale(&self) -> &'static str {
        "Aggregates patterns, challenges, escalations, and confidence into single report"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let pattern_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) as count
               FROM "ob-poc".detected_patterns
               WHERE cbu_id = $1
               GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;
        let challenge_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) as count
               FROM "ob-poc".verification_challenges
               WHERE cbu_id = $1
               GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;
        let escalation_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) as count
               FROM "ob-poc".verification_escalations
               WHERE cbu_id = $1
               GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;
        let critical_patterns: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".detected_patterns
               WHERE cbu_id = $1 AND status = 'DETECTED' AND severity IN ('HIGH', 'CRITICAL')"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let open_challenges: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".verification_challenges
               WHERE cbu_id = $1 AND status = 'OPEN'"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let pending_escalations: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".verification_escalations
               WHERE cbu_id = $1 AND status = 'PENDING'"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let overall_status = if critical_patterns > 0 || pending_escalations > 0 {
            "BLOCKED"
        } else if open_challenges > 0 {
            "PENDING_RESPONSE"
        } else {
            "CLEAR"
        };
        let result = serde_json::json!({
            "cbu_id": cbu_id,
            "overall_status": overall_status,
            "patterns": {
                "detected": pattern_stats.iter().find(|(s, _)| s == "DETECTED").map(|(_, c)| c).unwrap_or(&0),
                "investigating": pattern_stats.iter().find(|(s, _)| s == "INVESTIGATING").map(|(_, c)| c).unwrap_or(&0),
                "resolved": pattern_stats.iter().find(|(s, _)| s == "RESOLVED").map(|(_, c)| c).unwrap_or(&0),
                "false_positive": pattern_stats.iter().find(|(s, _)| s == "FALSE_POSITIVE").map(|(_, c)| c).unwrap_or(&0),
                "critical_open": critical_patterns
            },
            "challenges": {
                "open": challenge_stats.iter().find(|(s, _)| s == "OPEN").map(|(_, c)| c).unwrap_or(&0),
                "responded": challenge_stats.iter().find(|(s, _)| s == "RESPONDED").map(|(_, c)| c).unwrap_or(&0),
                "resolved": challenge_stats.iter().find(|(s, _)| s == "RESOLVED").map(|(_, c)| c).unwrap_or(&0),
                "escalated": challenge_stats.iter().find(|(s, _)| s == "ESCALATED").map(|(_, c)| c).unwrap_or(&0)
            },
            "escalations": {
                "pending": escalation_stats.iter().find(|(s, _)| s == "PENDING").map(|(_, c)| c).unwrap_or(&0),
                "under_review": escalation_stats.iter().find(|(s, _)| s == "UNDER_REVIEW").map(|(_, c)| c).unwrap_or(&0),
                "decided": escalation_stats.iter().find(|(s, _)| s == "DECIDED").map(|(_, c)| c).unwrap_or(&0)
            }
        });
        Ok(VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct VerifyAgainstRegistryOp;

#[async_trait]
impl CustomOperation for VerifyAgainstRegistryOp {
    fn domain(&self) -> &'static str {
        "verify"
    }
    fn verb(&self) -> &'static str {
        "verify-against-registry"
    }
    fn rationale(&self) -> &'static str {
        "Requires external registry API calls and field comparison logic"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::verification::RegistryVerifier;

        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let registry = json_extract_string(args, "registry")?;

        #[derive(sqlx::FromRow)]
        struct EntityRow {
            name: String,
            company_name: Option<String>,
            registration_number: Option<String>,
            lc_jurisdiction: Option<String>,
        }

        let entity: EntityRow = sqlx::query_as(
            r#"SELECT e.name,
                      lc.company_name, lc.registration_number, lc.jurisdiction as lc_jurisdiction
               FROM "ob-poc".entities e
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               WHERE e.entity_id = $1
                 AND e.deleted_at IS NULL"#,
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", entity_id))?;

        let name: String = match &entity.company_name {
            Some(cn) if !cn.is_empty() => cn.clone(),
            _ => entity.name.clone(),
        };
        let registration_number = entity.registration_number.clone();
        let jurisdiction = entity.lc_jurisdiction.clone();
        let verifier = RegistryVerifier::new();
        let result = match registry.to_uppercase().as_str() {
            "GLEIF" => {
                let lei = registration_number.as_deref().unwrap_or("");
                verifier
                    .verify_gleif_by_lei(entity_id, lei, Some(&name), jurisdiction.as_deref())
                    .await
            }
            "COMPANIES_HOUSE" => {
                let reg_num = registration_number.as_deref().unwrap_or("");
                verifier
                    .verify_companies_house_uk(entity_id, reg_num, Some(&name))
                    .await
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported registry: {}. Supported: GLEIF, COMPANIES_HOUSE",
                    registry
                ));
            }
        };
        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "entity_id": entity_id,
            "registry": registry,
            "found": result.found,
            "matches": result.matches,
            "confidence": result.confidence,
            "field_results": result.field_results.iter().map(|f| {
                serde_json::json!({
                    "field": f.field,
                    "claimed": f.claimed_value,
                    "registry": f.registry_value,
                    "matches": f.matches,
                    "match_type": format!("{:?}", f.match_type)
                })
            }).collect::<Vec<_>>(),
            "error": result.error
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct VerifyAssertOp;

#[async_trait]
impl CustomOperation for VerifyAssertOp {
    fn domain(&self) -> &'static str {
        "verify"
    }
    fn verb(&self) -> &'static str {
        "assert"
    }
    fn rationale(&self) -> &'static str {
        "Gate operation that blocks workflow if confidence below threshold"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let min_confidence: f64 = json_extract_string(args, "min-confidence")?
            .parse()
            .unwrap_or(0.6);
        let fail_action =
            json_extract_string_opt(args, "fail-action").unwrap_or_else(|| "error".to_string());
        let result =
            verify_assert_impl(cbu_id, entity_id, min_confidence, &fail_action, pool).await?;
        Ok(VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
