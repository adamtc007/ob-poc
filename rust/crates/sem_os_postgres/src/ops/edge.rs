//! Edge (entity relationship) verbs (1 plugin verb) — SemOS-side
//! YAML-first re-implementation of the plugin subset of
//! `rust/config/verbs/edge.yaml`.
//!
//! `edge.upsert` implements end-and-insert semantics for
//! `entity_relationships`:
//! - same natural key + same attrs → no-op
//! - same natural key + different attrs → end old + insert new
//! - new natural key → insert
//!
//! Natural key: `(from_entity_id, to_entity_id, relationship_type,
//! effective_from)`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EdgeUpsertResult {
    relationship_id: Uuid,
    action: String,
    from_entity_id: Uuid,
    to_entity_id: Uuid,
    relationship_type: String,
    superseded_id: Option<Uuid>,
}

pub struct Upsert;

#[async_trait]
impl SemOsVerbOp for Upsert {
    fn fqn(&self) -> &str {
        "edge.upsert"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let from_entity_id = json_extract_uuid(args, ctx, "from-entity-id")?;
        let to_entity_id = json_extract_uuid(args, ctx, "to-entity-id")?;
        let relationship_type = json_extract_string(args, "relationship-type")?;

        let percentage: Option<f64> = json_extract_string_opt(args, "percentage")
            .as_deref()
            .map(|s| {
                s.parse::<f64>()
                    .map_err(|e| anyhow!("Invalid percentage value '{}': {}", s, e))
            })
            .transpose()?;
        let incoming_pct: Option<rust_decimal::Decimal> = percentage
            .map(rust_decimal::Decimal::try_from)
            .transpose()
            .map_err(|e| anyhow!("Failed to convert percentage to decimal: {}", e))?;

        let ownership_type = json_extract_string_opt(args, "ownership-type");
        let control_type = json_extract_string_opt(args, "control-type");
        let effective_from: Option<NaiveDate> = json_extract_string_opt(args, "effective-from")
            .as_deref()
            .map(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
                    anyhow!(
                        "Invalid effective-from date '{}': expected YYYY-MM-DD format: {}",
                        s,
                        e
                    )
                })
            })
            .transpose()?;
        let source = json_extract_string(args, "source")?;
        let source_document_ref = json_extract_string_opt(args, "source-document-ref");
        let confidence =
            json_extract_string_opt(args, "confidence").unwrap_or_else(|| "MEDIUM".to_string());
        let import_run_id = json_extract_uuid_opt(args, ctx, "import-run-id");
        let evidence_hint = json_extract_string_opt(args, "evidence-hint");

        type ExistingRow = (
            Uuid,
            Option<rust_decimal::Decimal>,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            String,
        );

        let existing: Option<ExistingRow> = if effective_from.is_some() {
            sqlx::query_as(
                r#"SELECT relationship_id, percentage, ownership_type, control_type,
                          source, source_document_ref, confidence
                   FROM "ob-poc".entity_relationships
                   WHERE from_entity_id = $1
                     AND to_entity_id = $2
                     AND relationship_type = $3
                     AND effective_from = $4
                     AND effective_to IS NULL"#,
            )
            .bind(from_entity_id)
            .bind(to_entity_id)
            .bind(&relationship_type)
            .bind(effective_from)
            .fetch_optional(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                r#"SELECT relationship_id, percentage, ownership_type, control_type,
                          source, source_document_ref, confidence
                   FROM "ob-poc".entity_relationships
                   WHERE from_entity_id = $1
                     AND to_entity_id = $2
                     AND relationship_type = $3
                     AND effective_from IS NULL
                     AND effective_to IS NULL"#,
            )
            .bind(from_entity_id)
            .bind(to_entity_id)
            .bind(&relationship_type)
            .fetch_optional(scope.executor())
            .await?
        };

        if let Some((
            existing_id,
            existing_pct,
            existing_ownership,
            existing_control,
            existing_source,
            _existing_source_doc_ref,
            _existing_confidence,
        )) = existing
        {
            let same_pct = match (&existing_pct, &incoming_pct) {
                (None, None) => true,
                (Some(a), Some(b)) => a == b,
                _ => false,
            };
            let same_ownership = existing_ownership == ownership_type;
            let same_control = existing_control == control_type;
            let same_source = existing_source == source;

            if same_pct && same_ownership && same_control && same_source {
                let result = EdgeUpsertResult {
                    relationship_id: existing_id,
                    action: "no_change".to_string(),
                    from_entity_id,
                    to_entity_id,
                    relationship_type,
                    superseded_id: None,
                };
                return Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?));
            }

            sqlx::query(
                r#"UPDATE "ob-poc".entity_relationships
                   SET effective_to = CURRENT_DATE, updated_at = NOW()
                   WHERE relationship_id = $1"#,
            )
            .bind(existing_id)
            .execute(scope.executor())
            .await?;

            let new_id: Uuid = sqlx::query_scalar(
                r#"INSERT INTO "ob-poc".entity_relationships (
                       from_entity_id, to_entity_id, relationship_type, percentage,
                       ownership_type, control_type, effective_from,
                       source, source_document_ref, confidence, import_run_id, evidence_hint
                   ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                   RETURNING relationship_id"#,
            )
            .bind(from_entity_id)
            .bind(to_entity_id)
            .bind(&relationship_type)
            .bind(incoming_pct)
            .bind(&ownership_type)
            .bind(&control_type)
            .bind(effective_from)
            .bind(&source)
            .bind(&source_document_ref)
            .bind(&confidence)
            .bind(import_run_id)
            .bind(&evidence_hint)
            .fetch_one(scope.executor())
            .await?;

            let result = EdgeUpsertResult {
                relationship_id: new_id,
                action: "replaced".to_string(),
                from_entity_id,
                to_entity_id,
                relationship_type,
                superseded_id: Some(existing_id),
            };
            return Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?));
        }

        let new_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entity_relationships (
                   from_entity_id, to_entity_id, relationship_type, percentage,
                   ownership_type, control_type, effective_from,
                   source, source_document_ref, confidence, import_run_id, evidence_hint
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               RETURNING relationship_id"#,
        )
        .bind(from_entity_id)
        .bind(to_entity_id)
        .bind(&relationship_type)
        .bind(incoming_pct)
        .bind(&ownership_type)
        .bind(&control_type)
        .bind(effective_from)
        .bind(&source)
        .bind(&source_document_ref)
        .bind(&confidence)
        .bind(import_run_id)
        .bind(&evidence_hint)
        .fetch_one(scope.executor())
        .await?;

        let result = EdgeUpsertResult {
            relationship_id: new_id,
            action: "created".to_string(),
            from_entity_id,
            to_entity_id,
            relationship_type,
            superseded_id: None,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}
