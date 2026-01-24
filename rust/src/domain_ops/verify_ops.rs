//! Verification Operations (Adversarial Agent Model)
//!
//! Plugin handlers for the verify.* domain verbs that implement
//! game-theoretic "Distrust And Verify" model for KYC.
//!
//! ## Operations
//! - verify.detect-patterns - Run adversarial pattern detection
//! - verify.detect-evasion - Analyze doc_request history for evasion signals
//! - verify.calculate-confidence - Aggregate confidence scores
//! - verify.get-status - Comprehensive verification status report
//! - verify.verify-against-registry - Check against external registries
//! - verify.assert - Declarative confidence gate

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Pattern Detection
// ============================================================================

/// Run adversarial pattern detection on CBU
///
/// Rationale: Requires graph traversal for circular ownership, layering detection,
/// and cross-entity analysis for nominee patterns and opacity jurisdictions.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::verification::PatternDetector;
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

        // Get optional case ID
        let case_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "case-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Create detector with default config
        let detector = PatternDetector::new();

        // Run detection
        let patterns = detector.detect_all(pool, cbu_id).await?;

        // Persist detected patterns
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

        // Convert to JSON for result
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
// Evasion Detection
// ============================================================================

/// Analyze doc_request history for evasion signals
///
/// Rationale: Requires behavioral analysis of doc_request timeline patterns.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::verification::EvasionDetector;
        use uuid::Uuid;

        // Get case ID
        let case_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "case-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing case-id argument"))?;

        // Run detection
        let detector = EvasionDetector::new();
        let report = detector.analyze(pool, case_id).await?;

        // Convert to JSON
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

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "signals": [],
            "severity": "NONE",
            "recommendation": "No analysis performed"
        })))
    }
}

// ============================================================================
// Confidence Calculation
// ============================================================================

/// Calculate aggregate confidence for entity/attribute
///
/// Rationale: Requires complex weighted aggregation with multiple modifiers.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::verification::{
            ConfidenceCalculator, Evidence, EvidenceSource, InconsistencySeverity, PatternSeverity,
        };
        use chrono::Utc;
        use uuid::Uuid;

        // Get entity ID
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Get optional attribute filter
        let attribute: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Fetch observations for entity
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
            return Ok(ExecutionResult::Record(serde_json::json!({
                "entity_id": entity_id,
                "score": 0.0,
                "band": "REJECTED",
                "message": "No observations found for entity"
            })));
        }

        // Convert to Evidence structs for calculator
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

        // Calculate confidence
        let calculator = ConfidenceCalculator::new();

        // Count patterns for this entity and build severities
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

        // Count inconsistencies for this entity
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
                (severity, 1.0) // weight of 1.0 for each
            })
            .collect();

        let result =
            calculator.calculate(&evidence, &inconsistencies, &pattern_severities, Utc::now());

        Ok(ExecutionResult::Record(serde_json::json!({
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
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "score": 0.0,
            "band": "REJECTED",
            "message": "Database not available"
        })))
    }
}

// ============================================================================
// Verification Status
// ============================================================================

/// Get comprehensive verification status for CBU
///
/// Rationale: Aggregates data from multiple tables for holistic view.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
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

        // Count patterns by status
        let pattern_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) as count
               FROM "ob-poc".detected_patterns
               WHERE cbu_id = $1
               GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        // Count challenges by status
        let challenge_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) as count
               FROM "ob-poc".verification_challenges
               WHERE cbu_id = $1
               GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        // Count escalations by status
        let escalation_stats: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT status, COUNT(*) as count
               FROM "ob-poc".verification_escalations
               WHERE cbu_id = $1
               GROUP BY status"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        // Get open high/critical patterns
        let critical_patterns: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".detected_patterns
               WHERE cbu_id = $1 AND status = 'DETECTED' AND severity IN ('HIGH', 'CRITICAL')"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        // Get open challenges
        let open_challenges: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".verification_challenges
               WHERE cbu_id = $1 AND status = 'OPEN'"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        // Get pending escalations
        let pending_escalations: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".verification_escalations
               WHERE cbu_id = $1 AND status = 'PENDING'"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        // Determine overall status
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

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "overall_status": "UNKNOWN",
            "message": "Database not available"
        })))
    }
}

// ============================================================================
// Registry Verification
// ============================================================================

/// Verify entity against external registry
///
/// Rationale: Requires external API calls to GLEIF, Companies House, etc.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::verification::RegistryVerifier;
        use uuid::Uuid;

        // Get entity ID
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Get registry type
        let registry = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "registry")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing registry argument"))?;

        // Fetch entity data
        let entity = sqlx::query!(
            r#"SELECT e.entity_id, e.name,
                      lc.company_name, lc.registration_number, lc.jurisdiction as lc_jurisdiction
               FROM "ob-poc".entities e
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               WHERE e.entity_id = $1"#,
            entity_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", entity_id))?;

        // Use company_name if non-empty, otherwise fall back to entity name
        let name: String = if entity.company_name.is_empty() {
            entity.name.clone()
        } else {
            entity.company_name.clone()
        };
        let registration_number = entity.registration_number.clone();
        let jurisdiction = entity.lc_jurisdiction.clone();

        // Verify against registry
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

        Ok(ExecutionResult::Record(serde_json::json!({
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

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "found": false,
            "message": "Database not available"
        })))
    }
}

// ============================================================================
// Assertion Gate
// ============================================================================

/// Declarative confidence assertion gate
///
/// Rationale: Combines confidence calculation with gate logic for workflow control.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::verification::{
            ConfidenceCalculator, Evidence, EvidenceSource, InconsistencySeverity, PatternSeverity,
        };
        use chrono::Utc;
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

        // Get optional entity ID (if omitted, aggregate across all CBU entities)
        let entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Get min confidence threshold
        let min_confidence: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "min-confidence")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.6))
            .ok_or_else(|| anyhow::anyhow!("Missing min-confidence argument"))?;

        // Get fail action (default: error)
        let fail_action = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "fail-action")
            .and_then(|a| a.value.as_string())
            .unwrap_or("error");

        // Fetch observations
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
                "warn" => Ok(ExecutionResult::Record(serde_json::json!({
                    "passed": false,
                    "score": 0.0,
                    "threshold": min_confidence,
                    "warning": msg
                }))),
                "block" => Ok(ExecutionResult::Record(serde_json::json!({
                    "passed": false,
                    "blocked": true,
                    "score": 0.0,
                    "threshold": min_confidence,
                    "reason": msg
                }))),
                _ => Err(anyhow::anyhow!(msg)),
            };
        }

        // Convert to evidence
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

        // Calculate confidence (no patterns/inconsistencies for basic assert)
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
                "warn" => Ok(ExecutionResult::Record(serde_json::json!({
                    "passed": false,
                    "score": result.score,
                    "band": format!("{}", result.band),
                    "threshold": min_confidence,
                    "warning": msg
                }))),
                "block" => Ok(ExecutionResult::Record(serde_json::json!({
                    "passed": false,
                    "blocked": true,
                    "score": result.score,
                    "band": format!("{}", result.band),
                    "threshold": min_confidence,
                    "reason": msg
                }))),
                _ => Err(anyhow::anyhow!(msg)),
            };
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "passed": true,
            "score": result.score,
            "band": format!("{}", result.band),
            "threshold": min_confidence
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "passed": false,
            "message": "Database not available"
        })))
    }
}
