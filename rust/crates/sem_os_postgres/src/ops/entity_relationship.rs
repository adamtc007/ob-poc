//! Entity relationship graph verbs.

use crate::ops::SemOsVerbOp;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use dsl_runtime;
use dsl_runtime::TransactionScope;
use dsl_runtime::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use serde_json::{json, Value};
use uuid::Uuid;

/// Phase 1 §10.6 CAS carve-out (A7 = `entity_relationships`). A row BYPASSES the
/// version guard when it is the ManCo edge (Class 2 — `relationship_type =
/// 'management'`, maker-checker / KYC-canonical) or a `ubo_graph` recompute
/// (Class 3 — `source LIKE 'ubo.%'`, derived plane, ratification-exempt). Every
/// other operator-authored edge (Class 1) is version-CAS'd.
const A7_CAS_CARVEOUT: &str = r#"("ob-poc".entity_relationships.relationship_type = 'management'
    OR "ob-poc".entity_relationships.source LIKE 'ubo.%')"#;

/// Conflict error for a rejected stale compliance write (optimistic-concurrency
/// mismatch on the edge plane).
fn cas_conflict(expected: Option<i64>) -> anyhow::Error {
    anyhow!(
        "CAS conflict on entity_relationships: concurrent compliance write rejected — expected \
         version {:?} did not match the current edge version (re-read and retry)",
        expected
    )
}

fn parse_date_arg(args: &Value, name: &str) -> Result<Option<NaiveDate>> {
    json_extract_string_opt(args, name)
        .map(|value| {
            NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                .map_err(|err| anyhow!("{name} must be YYYY-MM-DD: {err}"))
        })
        .transpose()
}

fn parse_decimal_arg(args: &Value, name: &str) -> Result<Option<BigDecimal>> {
    json_extract_string_opt(args, name)
        .map(|value| {
            value
                .parse::<BigDecimal>()
                .map_err(|err| anyhow!("{name} must be decimal: {err}"))
        })
        .transpose()
}

/// Upsert a structural relationship between two entities.
///
/// # Examples
///
/// ```rust,ignore
/// let op = sem_os_postgres::ops::entity_relationship::Upsert;
/// assert_eq!(op.fqn(), "entity-relationship.upsert");
/// ```
pub struct Upsert;

#[async_trait]
impl SemOsVerbOp for Upsert {
    fn fqn(&self) -> &str {
        "entity-relationship.upsert"
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
        let percentage = parse_decimal_arg(args, "percentage")?;
        let ownership_type = json_extract_string_opt(args, "ownership-type");
        let control_type = json_extract_string_opt(args, "control-type");
        let trust_role = json_extract_string_opt(args, "trust-role");
        let trust_interest_type = json_extract_string_opt(args, "trust-interest-type");
        let trust_class_description = json_extract_string_opt(args, "trust-class-description");
        let is_regulated = json_extract_bool_opt(args, "is-regulated");
        let regulatory_jurisdiction = json_extract_string_opt(args, "regulatory-jurisdiction");
        let effective_from = parse_date_arg(args, "effective-from")?;
        let effective_to = parse_date_arg(args, "effective-to")?;
        let source = json_extract_string_opt(args, "source")
            .unwrap_or_else(|| "entity-relationship.upsert".to_string());
        let confidence =
            json_extract_string_opt(args, "confidence").unwrap_or_else(|| "HIGH".to_string());
        let notes = json_extract_string_opt(args, "notes");
        let created_by = json_extract_uuid_opt(args, ctx, "created-by");
        let expected_version = json_extract_int_opt(args, "expected-version");

        if relationship_type == "ownership" && percentage.is_none() {
            return Err(anyhow!(
                "entity-relationship.upsert: ownership relationship requires :percentage"
            ));
        }

        let relationship_id: Uuid = if effective_from.is_some() {
            sqlx::query_scalar(&format!(
                r#"
                INSERT INTO "ob-poc".entity_relationships
                    (from_entity_id, to_entity_id, relationship_type, percentage,
                     ownership_type, control_type, trust_role, trust_interest_type,
                     trust_class_description, is_regulated, regulatory_jurisdiction,
                     effective_from, effective_to, source, confidence, notes, created_by,
                     updated_at)
                VALUES
                    ($1, $2, $3, $4, $5, $6, $7, $8, $9, COALESCE($10, true), $11,
                     $12, $13, $14, $15, $16, $17, NOW())
                ON CONFLICT (from_entity_id, to_entity_id, relationship_type, effective_from)
                    WHERE effective_from IS NOT NULL
                DO UPDATE SET
                    percentage = EXCLUDED.percentage,
                    ownership_type = EXCLUDED.ownership_type,
                    control_type = EXCLUDED.control_type,
                    trust_role = EXCLUDED.trust_role,
                    trust_interest_type = EXCLUDED.trust_interest_type,
                    trust_class_description = EXCLUDED.trust_class_description,
                    is_regulated = EXCLUDED.is_regulated,
                    regulatory_jurisdiction = EXCLUDED.regulatory_jurisdiction,
                    effective_to = EXCLUDED.effective_to,
                    source = EXCLUDED.source,
                    confidence = EXCLUDED.confidence,
                    notes = EXCLUDED.notes,
                    version = "ob-poc".entity_relationships.version + 1,
                    updated_at = NOW()
                WHERE {carveout}
                   OR "ob-poc".entity_relationships.version
                      = COALESCE($18::bigint, "ob-poc".entity_relationships.version)
                RETURNING relationship_id
                "#,
                carveout = A7_CAS_CARVEOUT,
            ))
            .bind(from_entity_id)
            .bind(to_entity_id)
            .bind(&relationship_type)
            .bind(&percentage)
            .bind(&ownership_type)
            .bind(&control_type)
            .bind(&trust_role)
            .bind(&trust_interest_type)
            .bind(&trust_class_description)
            .bind(is_regulated)
            .bind(&regulatory_jurisdiction)
            .bind(effective_from)
            .bind(effective_to)
            .bind(&source)
            .bind(&confidence)
            .bind(&notes)
            .bind(created_by)
            .bind(expected_version)
            .fetch_optional(scope.executor())
            .await?
            .ok_or_else(|| cas_conflict(expected_version))?
        } else {
            sqlx::query_scalar(&format!(
                r#"
                INSERT INTO "ob-poc".entity_relationships
                    (from_entity_id, to_entity_id, relationship_type, percentage,
                     ownership_type, control_type, trust_role, trust_interest_type,
                     trust_class_description, is_regulated, regulatory_jurisdiction,
                     effective_from, effective_to, source, confidence, notes, created_by,
                     updated_at)
                VALUES
                    ($1, $2, $3, $4, $5, $6, $7, $8, $9, COALESCE($10, true), $11,
                     NULL, $12, $13, $14, $15, $16, NOW())
                ON CONFLICT (from_entity_id, to_entity_id, relationship_type)
                    WHERE effective_from IS NULL AND effective_to IS NULL
                DO UPDATE SET
                    percentage = EXCLUDED.percentage,
                    ownership_type = EXCLUDED.ownership_type,
                    control_type = EXCLUDED.control_type,
                    trust_role = EXCLUDED.trust_role,
                    trust_interest_type = EXCLUDED.trust_interest_type,
                    trust_class_description = EXCLUDED.trust_class_description,
                    is_regulated = EXCLUDED.is_regulated,
                    regulatory_jurisdiction = EXCLUDED.regulatory_jurisdiction,
                    source = EXCLUDED.source,
                    confidence = EXCLUDED.confidence,
                    notes = EXCLUDED.notes,
                    version = "ob-poc".entity_relationships.version + 1,
                    updated_at = NOW()
                WHERE {carveout}
                   OR "ob-poc".entity_relationships.version
                      = COALESCE($17::bigint, "ob-poc".entity_relationships.version)
                RETURNING relationship_id
                "#,
                carveout = A7_CAS_CARVEOUT,
            ))
            .bind(from_entity_id)
            .bind(to_entity_id)
            .bind(&relationship_type)
            .bind(&percentage)
            .bind(&ownership_type)
            .bind(&control_type)
            .bind(&trust_role)
            .bind(&trust_interest_type)
            .bind(&trust_class_description)
            .bind(is_regulated)
            .bind(&regulatory_jurisdiction)
            .bind(effective_to)
            .bind(&source)
            .bind(&confidence)
            .bind(&notes)
            .bind(created_by)
            .bind(expected_version)
            .fetch_optional(scope.executor())
            .await?
            .ok_or_else(|| cas_conflict(expected_version))?
        };

        ctx.bind("entity_relationship", relationship_id);
        dsl_runtime::emit_pending_state_advance(
            ctx,
            relationship_id,
            "entity-relationship:upserted",
            "entity/relationship-graph",
            "entity-relationship.upsert",
        );

        Ok(VerbExecutionOutcome::Record(json!({
            "relationship_id": relationship_id,
            "from_entity_id": from_entity_id,
            "to_entity_id": to_entity_id,
            "relationship_type": relationship_type
        })))
    }
}
