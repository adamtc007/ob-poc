//! Edge Operations — End-and-Insert Semantics
//!
//! Implements upsert for entity relationship edges with provenance tracking.
//! Natural key: (from_entity_id, to_entity_id, relationship_type, effective_from)
//!
//! Behavior per KYC/UBO architecture spec section 2A.1:
//! - Same key + same attrs -> no-op
//! - Same key + different attrs -> end old edge, insert new
//! - New key -> insert

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use chrono::NaiveDate;
#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::{extract_string, extract_string_opt, extract_uuid, extract_uuid_opt};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// ============================================================================
// Result Type
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeUpsertResult {
    pub relationship_id: Uuid,
    pub action: String,
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub relationship_type: String,
    pub superseded_id: Option<Uuid>,
}

// ============================================================================
// EdgeUpsertOp
// ============================================================================

#[register_custom_op]
pub struct EdgeUpsertOp;

#[async_trait]
impl CustomOperation for EdgeUpsertOp {
    fn domain(&self) -> &'static str {
        "edge"
    }

    fn verb(&self) -> &'static str {
        "upsert"
    }

    fn rationale(&self) -> &'static str {
        "End-and-insert semantics require natural key lookup, attribute comparison, and conditional update/insert"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // ------------------------------------------------------------------
        // 1. Extract arguments
        // ------------------------------------------------------------------
        let from_entity_id = extract_uuid(verb_call, ctx, "from-entity-id")?;
        let to_entity_id = extract_uuid(verb_call, ctx, "to-entity-id")?;
        let relationship_type = extract_string(verb_call, "relationship-type")?;

        let percentage_str = extract_string_opt(verb_call, "percentage");
        let percentage: Option<f64> = match percentage_str {
            Some(ref s) => Some(
                s.parse::<f64>()
                    .map_err(|e| anyhow!("Invalid percentage value '{}': {}", s, e))?,
            ),
            None => None,
        };

        let ownership_type = extract_string_opt(verb_call, "ownership-type");
        let control_type = extract_string_opt(verb_call, "control-type");

        let effective_from_str = extract_string_opt(verb_call, "effective-from");
        let effective_from: Option<NaiveDate> = match effective_from_str {
            Some(ref s) => Some(NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
                anyhow!(
                    "Invalid effective-from date '{}': expected YYYY-MM-DD format: {}",
                    s,
                    e
                )
            })?),
            None => None,
        };

        let source = extract_string(verb_call, "source")?;
        let source_document_ref = extract_string_opt(verb_call, "source-document-ref");

        // Default confidence to MEDIUM if not provided
        let confidence =
            extract_string_opt(verb_call, "confidence").unwrap_or_else(|| "MEDIUM".to_string());

        let import_run_id = extract_uuid_opt(verb_call, ctx, "import-run-id");
        let evidence_hint = extract_string_opt(verb_call, "evidence-hint");

        // ------------------------------------------------------------------
        // 2. Look up existing edge by natural key
        // ------------------------------------------------------------------
        // The query shape depends on whether effective_from is provided.
        // Return tuple: (relationship_id, percentage, ownership_type, control_type, source, source_document_ref, confidence)
        let existing: Option<(
            Uuid,
            Option<rust_decimal::Decimal>,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            String,
        )> = if effective_from.is_some() {
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
            .fetch_optional(pool)
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
            .fetch_optional(pool)
            .await?
        };

        // ------------------------------------------------------------------
        // 3. Compare and act
        // ------------------------------------------------------------------
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
            // Convert incoming percentage to Decimal for comparison
            let incoming_pct: Option<rust_decimal::Decimal> = percentage
                .map(|p| rust_decimal::Decimal::try_from(p))
                .transpose()
                .map_err(|e| anyhow!("Failed to convert percentage to decimal: {}", e))?;

            // Compare attributes: percentage, ownership_type, control_type, source
            let same_pct = match (&existing_pct, &incoming_pct) {
                (None, None) => true,
                (Some(a), Some(b)) => a == b,
                _ => false,
            };
            let same_ownership = existing_ownership == ownership_type;
            let same_control = existing_control == control_type;
            let same_source = existing_source == source;

            if same_pct && same_ownership && same_control && same_source {
                // ----------------------------------------------------------
                // 3a. No change — return existing
                // ----------------------------------------------------------
                let result = EdgeUpsertResult {
                    relationship_id: existing_id,
                    action: "no_change".to_string(),
                    from_entity_id,
                    to_entity_id,
                    relationship_type,
                    superseded_id: None,
                };
                return Ok(ExecutionResult::Record(serde_json::to_value(result)?));
            }

            // ----------------------------------------------------------
            // 3b. Different attrs — end old edge, insert new
            // ----------------------------------------------------------

            // End old edge
            sqlx::query(
                r#"UPDATE "ob-poc".entity_relationships
                   SET effective_to = CURRENT_DATE, updated_at = NOW()
                   WHERE relationship_id = $1"#,
            )
            .bind(existing_id)
            .execute(pool)
            .await?;

            // Insert new edge
            let new_pct_decimal: Option<rust_decimal::Decimal> = incoming_pct;

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
            .bind(new_pct_decimal)
            .bind(&ownership_type)
            .bind(&control_type)
            .bind(effective_from)
            .bind(&source)
            .bind(&source_document_ref)
            .bind(&confidence)
            .bind(import_run_id)
            .bind(&evidence_hint)
            .fetch_one(pool)
            .await?;

            let result = EdgeUpsertResult {
                relationship_id: new_id,
                action: "replaced".to_string(),
                from_entity_id,
                to_entity_id,
                relationship_type,
                superseded_id: Some(existing_id),
            };
            return Ok(ExecutionResult::Record(serde_json::to_value(result)?));
        }

        // ------------------------------------------------------------------
        // 3c. Not found — insert new edge
        // ------------------------------------------------------------------
        let pct_decimal: Option<rust_decimal::Decimal> = percentage
            .map(|p| rust_decimal::Decimal::try_from(p))
            .transpose()
            .map_err(|e| anyhow!("Failed to convert percentage to decimal: {}", e))?;

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
        .bind(pct_decimal)
        .bind(&ownership_type)
        .bind(&control_type)
        .bind(effective_from)
        .bind(&source)
        .bind(&source_document_ref)
        .bind(&confidence)
        .bind(import_run_id)
        .bind(&evidence_hint)
        .fetch_one(pool)
        .await?;

        let result = EdgeUpsertResult {
            relationship_id: new_id,
            action: "created".to_string(),
            from_entity_id,
            to_entity_id,
            relationship_type,
            superseded_id: None,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}
