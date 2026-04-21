//! Observation-domain verbs (5 plugin verbs across
//! `observation` + `document` domains) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/observation.yaml` + the one straggler
//! from `rust/config/verbs/document.yaml` (extract-to-observations).
//!
//! Attribute identity resolution routes through
//! `AttributeIdentityService` (slice-8 pattern). Rest is direct
//! sqlx against `"ob-poc".attribute_observations`,
//! `client_allegations`, `observation_discrepancies`,
//! `document_attribute_links`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::service_traits::AttributeIdentityService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── observation.record-from-document ──────────────────────────────────────────

pub struct RecordFromDocument;

#[async_trait]
impl SemOsVerbOp for RecordFromDocument {
    fn fqn(&self) -> &str {
        "observation.record-from-document"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let document_id = json_extract_uuid(args, ctx, "document-id")?;
        let attr_name = json_extract_string(args, "attribute")?;
        let attribute_id = ctx
            .service::<dyn AttributeIdentityService>()?
            .resolve_runtime_uuid(&attr_name)
            .await?
            .ok_or_else(|| anyhow!("Unknown attribute: {}", attr_name))?;

        let value = json_extract_string(args, "value")?;
        let extraction_method = json_extract_string_opt(args, "extraction-method");
        let confidence: f64 = json_extract_string_opt(args, "confidence")
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.80);

        let is_authoritative: bool = sqlx::query_scalar(
            r#"SELECT COALESCE(dal.is_authoritative, FALSE)
               FROM "ob-poc".document_catalog dc
               LEFT JOIN "ob-poc".document_attribute_links dal
                 ON dal.document_type_id = dc.document_type_id
                 AND dal.attribute_id = $2
                 AND dal.direction IN ('SOURCE', 'BOTH')
               WHERE dc.doc_id = $1
               LIMIT 1"#,
        )
        .bind(document_id)
        .bind(attribute_id)
        .fetch_optional(scope.executor())
        .await?
        .unwrap_or(false);

        let observation_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".attribute_observations
               (observation_id, entity_id, attribute_id, value_text, source_type,
                source_document_id, confidence, is_authoritative, extraction_method,
                observed_at, status)
               VALUES ($1, $2, $3, $4, 'DOCUMENT', $5, $6, $7, $8, NOW(), 'ACTIVE')"#,
        )
        .bind(observation_id)
        .bind(entity_id)
        .bind(attribute_id)
        .bind(&value)
        .bind(document_id)
        .bind(confidence)
        .bind(is_authoritative)
        .bind(extraction_method.as_deref())
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(observation_id))
    }
}

// ── observation.get-current ───────────────────────────────────────────────────

pub struct GetCurrent;

#[async_trait]
impl SemOsVerbOp for GetCurrent {
    fn fqn(&self) -> &str {
        "observation.get-current"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let attr_name = json_extract_string(args, "attribute")?;
        let attribute_id = ctx
            .service::<dyn AttributeIdentityService>()?
            .resolve_runtime_uuid(&attr_name)
            .await?
            .ok_or_else(|| anyhow!("Unknown attribute: {}", attr_name))?;

        type Row = (
            Uuid,
            Option<String>,
            Option<rust_decimal::Decimal>,
            Option<bool>,
            Option<chrono::NaiveDate>,
            String,
            Option<rust_decimal::Decimal>,
            bool,
        );

        let result: Option<Row> = sqlx::query_as(
            r#"SELECT observation_id, value_text, value_number, value_boolean, value_date,
                      source_type, confidence, is_authoritative
               FROM "ob-poc".v_attribute_current
               WHERE entity_id = $1 AND attribute_id = $2"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .fetch_optional(scope.executor())
        .await?;

        Ok(match result {
            Some((obs_id, v_text, v_num, v_bool, v_date, src_type, conf, is_auth)) => {
                VerbExecutionOutcome::Record(json!({
                    "observation_id": obs_id,
                    "value_text": v_text,
                    "value_number": v_num,
                    "value_boolean": v_bool,
                    "value_date": v_date,
                    "source_type": src_type,
                    "confidence": conf,
                    "is_authoritative": is_auth,
                }))
            }
            None => VerbExecutionOutcome::Record(json!({
                "found": false,
                "message": "No active observation found for this attribute",
            })),
        })
    }
}

// ── observation.reconcile ─────────────────────────────────────────────────────

pub struct Reconcile;

#[async_trait]
impl SemOsVerbOp for Reconcile {
    fn fqn(&self) -> &str {
        "observation.reconcile"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let attr_name = json_extract_string(args, "attribute")?;
        let attribute_id = ctx
            .service::<dyn AttributeIdentityService>()?
            .resolve_runtime_uuid(&attr_name)
            .await?
            .ok_or_else(|| anyhow!("Unknown attribute: {}", attr_name))?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let auto_create = json_extract_bool_opt(args, "auto-create-discrepancies").unwrap_or(true);

        type ObsRow = (
            Uuid,
            Option<String>,
            String,
            Option<rust_decimal::Decimal>,
            bool,
        );

        let observations: Vec<ObsRow> = sqlx::query_as(
            r#"SELECT observation_id, value_text, source_type, confidence, is_authoritative
               FROM "ob-poc".attribute_observations
               WHERE entity_id = $1 AND attribute_id = $2 AND status = 'ACTIVE'
               ORDER BY is_authoritative DESC, confidence DESC NULLS LAST, observed_at DESC"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .fetch_all(scope.executor())
        .await?;

        if observations.len() < 2 {
            return Ok(VerbExecutionOutcome::Record(json!({
                "status": "no_conflict",
                "observation_count": observations.len(),
                "discrepancies_created": 0,
            })));
        }

        let first = &observations[0];
        let mut discrepancies_created = 0;
        for other in observations.iter().skip(1) {
            if first.1 != other.1 && auto_create {
                let discrepancy_id = Uuid::new_v4();
                sqlx::query(
                    r#"INSERT INTO "ob-poc".observation_discrepancies
                       (discrepancy_id, entity_id, attribute_id, observation_1_id, observation_2_id,
                        discrepancy_type, severity, description, case_id, resolution_status)
                       VALUES ($1, $2, $3, $4, $5, 'VALUE_MISMATCH', 'MEDIUM',
                               'Different values observed for same attribute', $6, 'OPEN')"#,
                )
                .bind(discrepancy_id)
                .bind(entity_id)
                .bind(attribute_id)
                .bind(first.0)
                .bind(other.0)
                .bind(case_id)
                .execute(scope.executor())
                .await?;
                discrepancies_created += 1;
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "status": if discrepancies_created > 0 { "conflicts_found" } else { "no_conflict" },
            "observation_count": observations.len(),
            "discrepancies_created": discrepancies_created,
        })))
    }
}

// ── observation.verify-allegations ────────────────────────────────────────────

pub struct VerifyAllegations;

#[async_trait]
impl SemOsVerbOp for VerifyAllegations {
    fn fqn(&self) -> &str {
        "observation.verify-allegations"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;

        let allegations: Vec<(Uuid, Uuid, Value, Option<String>)> = sqlx::query_as(
            r#"SELECT allegation_id, attribute_id, alleged_value, alleged_value_display
               FROM "ob-poc".client_allegations
               WHERE cbu_id = $1 AND entity_id = $2 AND verification_status = 'PENDING'"#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .fetch_all(scope.executor())
        .await?;

        let mut verified = 0;
        let mut contradicted = 0;
        let mut no_observation = 0;

        for (allegation_id, attribute_id, alleged_value, alleged_display) in allegations {
            let current: Option<(Uuid, Option<String>)> = sqlx::query_as(
                r#"SELECT observation_id, value_text
                   FROM "ob-poc".v_attribute_current
                   WHERE entity_id = $1 AND attribute_id = $2"#,
            )
            .bind(entity_id)
            .bind(attribute_id)
            .fetch_optional(scope.executor())
            .await?;

            match current {
                Some((observation_id, obs_value_text)) => {
                    let alleged_str = alleged_display
                        .or_else(|| alleged_value.as_str().map(String::from))
                        .unwrap_or_default();
                    let matches = obs_value_text
                        .as_ref()
                        .map(|v| v.to_lowercase() == alleged_str.to_lowercase())
                        .unwrap_or(false);

                    if matches {
                        sqlx::query(
                            r#"UPDATE "ob-poc".client_allegations
                               SET verification_status = 'VERIFIED',
                                   verified_by_observation_id = $2,
                                   verified_at = NOW()
                               WHERE allegation_id = $1"#,
                        )
                        .bind(allegation_id)
                        .bind(observation_id)
                        .execute(scope.executor())
                        .await?;
                        verified += 1;
                    } else {
                        sqlx::query(
                            r#"UPDATE "ob-poc".client_allegations
                               SET verification_status = 'CONTRADICTED',
                                   verified_by_observation_id = $2,
                                   verified_at = NOW(),
                                   verification_notes = 'Value does not match observation'
                               WHERE allegation_id = $1"#,
                        )
                        .bind(allegation_id)
                        .bind(observation_id)
                        .execute(scope.executor())
                        .await?;
                        contradicted += 1;
                    }
                }
                None => no_observation += 1,
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "verified": verified,
            "contradicted": contradicted,
            "no_observation": no_observation,
            "total_processed": verified + contradicted + no_observation,
        })))
    }
}

// ── document.extract-to-observations ──────────────────────────────────────────

pub struct ExtractToObservations;

#[async_trait]
impl SemOsVerbOp for ExtractToObservations {
    fn fqn(&self) -> &str {
        "document.extract-to-observations"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let document_id = json_extract_uuid(args, ctx, "document-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let auto_verify = json_extract_bool_opt(args, "auto-verify-allegations").unwrap_or(true);

        let doc_type: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT document_type_id FROM "ob-poc".document_catalog WHERE doc_id = $1"#,
        )
        .bind(document_id)
        .fetch_optional(scope.executor())
        .await?;
        let doc_type_id = doc_type.ok_or_else(|| anyhow!("Document not found: {}", document_id))?;

        type ExtractableRow = (
            Uuid,
            String,
            Option<String>,
            Option<sqlx::types::BigDecimal>,
            bool,
        );

        let extractable: Vec<ExtractableRow> = sqlx::query_as(
            r#"SELECT dal.attribute_id, ar.id, dal.extraction_method,
                      dal.extraction_confidence_default, dal.is_authoritative
               FROM "ob-poc".document_attribute_links dal
               JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
               WHERE dal.document_type_id = $1 AND dal.direction = 'SOURCE' AND dal.is_active = TRUE"#,
        )
        .bind(doc_type_id)
        .fetch_all(scope.executor())
        .await?;

        let mut observations_created = 0;
        for (attribute_id, attr_name, extraction_method, confidence, is_authoritative) in
            &extractable
        {
            sqlx::query(
                r#"INSERT INTO "ob-poc".attribute_observations
                   (observation_id, entity_id, attribute_id, value_text, source_type,
                    source_document_id, extraction_method, confidence, is_authoritative, status)
                   VALUES ($1, $2, $3, $4, 'DOCUMENT', $5, $6, $7, $8, 'ACTIVE')
                   ON CONFLICT (entity_id, attribute_id, source_type, source_document_id)
                   WHERE status = 'ACTIVE'
                   DO NOTHING"#,
            )
            .bind(Uuid::new_v4())
            .bind(entity_id)
            .bind(attribute_id)
            .bind(format!("[Extracted from document - {}]", attr_name))
            .bind(document_id)
            .bind(extraction_method.as_deref().unwrap_or("MANUAL"))
            .bind(confidence.clone())
            .bind(is_authoritative)
            .execute(scope.executor())
            .await?;
            observations_created += 1;
        }

        let mut allegations_verified = 0_i32;
        if auto_verify && observations_created > 0 {
            let cbu_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT cbu_id FROM "ob-poc".document_catalog WHERE doc_id = $1"#,
            )
            .bind(document_id)
            .fetch_optional(scope.executor())
            .await?;

            if let Some(cbu_id) = cbu_id {
                let result = sqlx::query(
                    r#"UPDATE "ob-poc".client_allegations ca
                       SET verification_status = 'VERIFIED',
                           verified_at = NOW(),
                           verification_notes = 'Auto-verified by document extraction'
                       WHERE ca.cbu_id = $1 AND ca.entity_id = $2
                         AND ca.verification_status = 'PENDING'
                         AND EXISTS (
                           SELECT 1 FROM "ob-poc".attribute_observations ao
                           WHERE ao.entity_id = ca.entity_id
                             AND ao.attribute_id = ca.attribute_id
                             AND ao.source_document_id = $3
                             AND ao.status = 'ACTIVE'
                         )"#,
                )
                .bind(cbu_id)
                .bind(entity_id)
                .bind(document_id)
                .execute(scope.executor())
                .await?;
                allegations_verified = result.rows_affected() as i32;
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "observations_created": observations_created,
            "attributes_extractable": extractable.len(),
            "allegations_verified": allegations_verified,
        })))
    }
}
