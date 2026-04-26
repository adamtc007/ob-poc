//! Verification (adversarial agent) verbs (6 plugin verbs) —
//! YAML-first re-implementation of `rust/config/verbs/verify.yaml`.
//!
//! Delegates to `dsl_runtime::verification::{PatternDetector,
//! EvasionDetector, ConfidenceCalculator, RegistryVerifier}` —
//! transitional `scope.pool()` because detectors still take
//! `&PgPool`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::verification::{
    ConfidenceCalculator, EvasionDetector, Evidence, EvidenceSource, InconsistencySeverity,
    PatternDetector, PatternSeverity, RegistryVerifier,
};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── verify.detect-patterns ────────────────────────────────────────────────────

pub struct DetectPatterns;

#[async_trait]
impl SemOsVerbOp for DetectPatterns {
    fn fqn(&self) -> &str {
        "verify.detect-patterns"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let detector = PatternDetector::new();
        let patterns = detector.detect_all(scope.pool(), cbu_id).await?;

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
            .execute(scope.executor())
            .await?;
        }

        let result: Vec<Value> = patterns
            .iter()
            .map(|p| {
                json!({
                    "pattern_type": p.pattern_type.as_str(),
                    "severity": p.severity.as_str(),
                    "description": p.description,
                    "involved_entities": p.involved_entities,
                    "evidence": p.evidence,
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::RecordSet(result))
    }
}

// ── verify.detect-evasion ─────────────────────────────────────────────────────

pub struct DetectEvasion;

#[async_trait]
impl SemOsVerbOp for DetectEvasion {
    fn fqn(&self) -> &str {
        "verify.detect-evasion"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let detector = EvasionDetector::new();
        let report = detector.analyze(scope.pool(), case_id).await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "case_id": case_id,
            "evasion_score": report.evasion_score,
            "classification": format!("{:?}", report.classification),
            "signals": report.signals.iter().map(|s| json!({
                "signal": format!("{:?}", s.signal),
                "severity": format!("{:?}", s.severity),
                "description": s.description,
                "metric_value": s.metric_value,
                "threshold": s.threshold,
            })).collect::<Vec<_>>(),
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
                "followup_response_rate": report.metrics.followup_response_rate,
            }
        })))
    }
}

// ── verify.calculate-confidence ───────────────────────────────────────────────

pub struct CalculateConfidence;

#[async_trait]
impl SemOsVerbOp for CalculateConfidence {
    fn fqn(&self) -> &str {
        "verify.calculate-confidence"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let attribute = json_extract_string_opt(args, "attribute");

        let mut query = String::from(
            r#"SELECT ao.observation_id, ao.attribute_id, ao.source_type, ao.confidence,
                      ao.is_authoritative, ao.observed_at, ar.name as attribute_name
               FROM "ob-poc".attribute_observations ao
               JOIN "ob-poc".attribute_registry ar ON ao.attribute_id = ar.uuid
               WHERE ao.entity_id = $1 AND ao.status = 'ACTIVE'"#,
        );
        if attribute.is_some() {
            query.push_str(" AND ar.name = $2");
        }

        type ObsRow = (
            Uuid,
            Uuid,
            Option<String>,
            Option<sqlx::types::BigDecimal>,
            Option<bool>,
            Option<chrono::DateTime<chrono::Utc>>,
            String,
        );

        let observations: Vec<ObsRow> = if let Some(ref attr) = attribute {
            sqlx::query_as(&query)
                .bind(entity_id)
                .bind(attr)
                .fetch_all(scope.executor())
                .await?
        } else {
            sqlx::query_as(&query)
                .bind(entity_id)
                .fetch_all(scope.executor())
                .await?
        };

        if observations.is_empty() {
            return Ok(VerbExecutionOutcome::Record(json!({
                "entity_id": entity_id,
                "score": 0.0,
                "band": "REJECTED",
                "message": "No observations found for entity",
            })));
        }

        let evidence: Vec<Evidence> = observations
            .iter()
            .map(
                |(oid, aid, src_type, confidence, is_auth, observed_at, _)| Evidence {
                    evidence_id: *oid,
                    entity_id,
                    attribute_id: *aid,
                    observed_value: json!(null),
                    source: src_type
                        .as_ref()
                        .map(|s| EvidenceSource::from(s.as_str()))
                        .unwrap_or(EvidenceSource::Allegation),
                    confidence: confidence
                        .as_ref()
                        .map(|d| d.to_string().parse().unwrap_or(0.5))
                        .unwrap_or(0.5),
                    is_authoritative: is_auth.unwrap_or(false),
                    observed_at: observed_at.unwrap_or_else(chrono::Utc::now),
                    source_document_id: None,
                    extraction_method: None,
                    effective_from: None,
                    effective_to: None,
                },
            )
            .collect();

        let pattern_rows: Vec<(String,)> = sqlx::query_as(
            r#"SELECT severity FROM "ob-poc".detected_patterns
               WHERE $1 = ANY(involved_entities) AND status = 'DETECTED'"#,
        )
        .bind(entity_id)
        .fetch_all(scope.executor())
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
        .fetch_all(scope.executor())
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

        let calculator = ConfidenceCalculator::new();
        let result = calculator.calculate(
            &evidence,
            &inconsistencies,
            &pattern_severities,
            chrono::Utc::now(),
        );

        Ok(VerbExecutionOutcome::Record(json!({
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
                "pattern_penalty": result.breakdown.pattern_penalty,
            }
        })))
    }
}

// ── verify.get-status ─────────────────────────────────────────────────────────

pub struct GetStatus;

#[async_trait]
impl SemOsVerbOp for GetStatus {
    fn fqn(&self) -> &str {
        "verify.get-status"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let pattern_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) FROM "ob-poc".detected_patterns
               WHERE cbu_id = $1 GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;
        let challenge_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) FROM "ob-poc".verification_challenges
               WHERE cbu_id = $1 GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;
        let escalation_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) FROM "ob-poc".verification_escalations
               WHERE cbu_id = $1 GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        let critical_patterns: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".detected_patterns
               WHERE cbu_id = $1 AND status = 'DETECTED' AND severity IN ('HIGH', 'CRITICAL')"#,
        )
        .bind(cbu_id)
        .fetch_one(scope.executor())
        .await
        .unwrap_or(0);
        let open_challenges: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".verification_challenges
               WHERE cbu_id = $1 AND status = 'OPEN'"#,
        )
        .bind(cbu_id)
        .fetch_one(scope.executor())
        .await
        .unwrap_or(0);
        let pending_escalations: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".verification_escalations
               WHERE cbu_id = $1 AND status = 'PENDING'"#,
        )
        .bind(cbu_id)
        .fetch_one(scope.executor())
        .await
        .unwrap_or(0);

        let overall_status = if critical_patterns > 0 || pending_escalations > 0 {
            "BLOCKED"
        } else if open_challenges > 0 {
            "PENDING_RESPONSE"
        } else {
            "CLEAR"
        };

        fn stat(stats: &[(String, i64)], key: &str) -> i64 {
            stats
                .iter()
                .find(|(s, _)| s == key)
                .map(|(_, c)| *c)
                .unwrap_or(0)
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "overall_status": overall_status,
            "patterns": {
                "detected": stat(&pattern_stats, "DETECTED"),
                "investigating": stat(&pattern_stats, "INVESTIGATING"),
                "resolved": stat(&pattern_stats, "RESOLVED"),
                "false_positive": stat(&pattern_stats, "FALSE_POSITIVE"),
                "critical_open": critical_patterns,
            },
            "challenges": {
                "open": stat(&challenge_stats, "OPEN"),
                "responded": stat(&challenge_stats, "RESPONDED"),
                "resolved": stat(&challenge_stats, "RESOLVED"),
                "escalated": stat(&challenge_stats, "ESCALATED"),
            },
            "escalations": {
                "pending": stat(&escalation_stats, "PENDING"),
                "under_review": stat(&escalation_stats, "UNDER_REVIEW"),
                "decided": stat(&escalation_stats, "DECIDED"),
            }
        })))
    }
}

// ── verify.verify-against-registry ────────────────────────────────────────────

pub struct VerifyAgainstRegistry;

#[async_trait]
impl SemOsVerbOp for VerifyAgainstRegistry {
    fn fqn(&self) -> &str {
        "verify.verify-against-registry"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
               WHERE e.entity_id = $1 AND e.deleted_at IS NULL"#,
        )
        .bind(entity_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Entity not found: {}", entity_id))?;

        let name = match &entity.company_name {
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
                return Err(anyhow!(
                    "Unsupported registry: {}. Supported: GLEIF, COMPANIES_HOUSE",
                    registry
                ));
            }
        };

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id,
            "registry": registry,
            "found": result.found,
            "matches": result.matches,
            "confidence": result.confidence,
            "field_results": result.field_results.iter().map(|f| json!({
                "field": f.field,
                "claimed": f.claimed_value,
                "registry": f.registry_value,
                "matches": f.matches,
                "match_type": format!("{:?}", f.match_type),
            })).collect::<Vec<_>>(),
            "error": result.error,
        })))
    }
}

// ── verify.assert ─────────────────────────────────────────────────────────────

pub struct Assert;

#[async_trait]
impl SemOsVerbOp for Assert {
    fn fqn(&self) -> &str {
        "verify.assert"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let min_confidence: f64 = json_extract_string(args, "min-confidence")?
            .parse()
            .unwrap_or(0.6);
        let fail_action =
            json_extract_string_opt(args, "fail-action").unwrap_or_else(|| "error".to_string());

        let query = if let Some(eid) = entity_id {
            format!(
                r#"SELECT ao.observation_id, ao.entity_id, ao.attribute_id, ao.source_type,
                          ao.confidence, ao.is_authoritative, ao.observed_at
                   FROM "ob-poc".attribute_observations ao
                   WHERE ao.entity_id = '{}' AND ao.status = 'ACTIVE'"#,
                eid
            )
        } else {
            format!(
                r#"SELECT ao.observation_id, ao.entity_id, ao.attribute_id, ao.source_type,
                          ao.confidence, ao.is_authoritative, ao.observed_at
                   FROM "ob-poc".attribute_observations ao
                   JOIN "ob-poc".cbu_entity_roles cer ON ao.entity_id = cer.entity_id
                   WHERE cer.cbu_id = '{}' AND ao.status = 'ACTIVE'"#,
                cbu_id
            )
        };

        type ObsRow = (
            Uuid,
            Uuid,
            Uuid,
            Option<String>,
            Option<sqlx::types::BigDecimal>,
            Option<bool>,
            Option<chrono::DateTime<chrono::Utc>>,
        );

        let observations: Vec<ObsRow> = sqlx::query_as(&query).fetch_all(scope.executor()).await?;

        if observations.is_empty() {
            let msg = "No observations found - cannot verify confidence";
            return match fail_action.as_str() {
                "error" => Err(anyhow!(msg)),
                "warn" => Ok(VerbExecutionOutcome::Record(json!({
                    "passed": false,
                    "score": 0.0,
                    "threshold": min_confidence,
                    "warning": msg,
                }))),
                "block" => Ok(VerbExecutionOutcome::Record(json!({
                    "passed": false,
                    "blocked": true,
                    "score": 0.0,
                    "threshold": min_confidence,
                    "reason": msg,
                }))),
                _ => Err(anyhow!(msg)),
            };
        }

        let evidence: Vec<Evidence> = observations
            .iter()
            .map(
                |(oid, eid, aid, src_type, confidence, is_auth, observed_at)| Evidence {
                    evidence_id: *oid,
                    entity_id: *eid,
                    attribute_id: *aid,
                    observed_value: json!(null),
                    source: src_type
                        .as_ref()
                        .map(|s| EvidenceSource::from(s.as_str()))
                        .unwrap_or(EvidenceSource::Allegation),
                    confidence: confidence
                        .as_ref()
                        .map(|d| d.to_string().parse().unwrap_or(0.5))
                        .unwrap_or(0.5),
                    is_authoritative: is_auth.unwrap_or(false),
                    observed_at: observed_at.unwrap_or_else(chrono::Utc::now),
                    source_document_id: None,
                    extraction_method: None,
                    effective_from: None,
                    effective_to: None,
                },
            )
            .collect();

        let calculator = ConfidenceCalculator::new();
        let result = calculator.calculate(&evidence, &[], &[], chrono::Utc::now());
        let passed = result.score >= min_confidence;

        if !passed {
            let msg = format!(
                "Confidence {} below threshold {}",
                result.score, min_confidence
            );
            return match fail_action.as_str() {
                "error" => Err(anyhow!(msg)),
                "warn" => Ok(VerbExecutionOutcome::Record(json!({
                    "passed": false,
                    "score": result.score,
                    "band": format!("{}", result.band),
                    "threshold": min_confidence,
                    "warning": msg,
                }))),
                "block" => Ok(VerbExecutionOutcome::Record(json!({
                    "passed": false,
                    "blocked": true,
                    "score": result.score,
                    "band": format!("{}", result.band),
                    "threshold": min_confidence,
                    "reason": msg,
                }))),
                _ => Err(anyhow!(msg)),
            };
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "passed": true,
            "score": result.score,
            "band": format!("{}", result.band),
            "threshold": min_confidence,
        })))
    }
}
