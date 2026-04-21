//! UBO-graph lifecycle verbs (4 plugin verbs) — YAML-first
//! re-implementation of the lifecycle subset of
//! `rust/config/verbs/ubo.yaml`: mark-deceased, convergence-supersede,
//! transfer-control, waive-verification. All per-op `pool.begin()`
//! transactions replaced by the Sequencer-owned scope.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_cbu_id, json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── ubo.mark-deceased ─────────────────────────────────────────────────────────

pub struct MarkDeceased;

#[async_trait]
impl SemOsVerbOp for MarkDeceased {
    fn fqn(&self) -> &str {
        "ubo.mark-deceased"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = if args.get("entity-id").is_some() {
            json_extract_uuid(args, ctx, "entity-id")?
        } else {
            json_extract_uuid(args, ctx, "person-id")?
        };

        let date_of_death_str = json_extract_string_opt(args, "date-of-death")
            .or_else(|| json_extract_string_opt(args, "death-date"))
            .ok_or_else(|| anyhow!("Missing or invalid date-of-death argument (format: YYYY-MM-DD)"))?;
        let date_of_death = chrono::NaiveDate::parse_from_str(&date_of_death_str, "%Y-%m-%d")
            .map_err(|_| anyhow!("Missing or invalid date-of-death argument (format: YYYY-MM-DD)"))?;
        let reason = json_extract_string_opt(args, "reason")
            .or_else(|| json_extract_string_opt(args, "notes"))
            .unwrap_or_else(|| "Deceased - death certificate received".to_string());

        let is_natural_person: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (
                SELECT 1 FROM "ob-poc".entities e
                JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
                WHERE e.entity_id = $1 AND e.deleted_at IS NULL AND et.entity_category = 'PERSON'
            )"#,
        )
        .bind(entity_id)
        .fetch_one(scope.executor())
        .await?;
        if !is_natural_person {
            return Err(anyhow!(
                "Entity {} is not a natural person. mark-deceased only applies to PERSON entities.",
                entity_id
            ));
        }

        let person_name: Option<String> = sqlx::query_scalar(
            r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(entity_id)
        .fetch_optional(scope.executor())
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".entity_proper_persons
               SET date_of_death = $1, updated_at = NOW()
               WHERE entity_id = $2"#,
        )
        .bind(date_of_death)
        .bind(entity_id)
        .execute(scope.executor())
        .await?;

        let ownership_ended = sqlx::query(
            r#"UPDATE "ob-poc".entity_relationships
               SET effective_to = $1,
                   notes = COALESCE(notes, '') || ' [Ended: ' || $2 || ']',
                   updated_at = NOW()
               WHERE from_entity_id = $3
                 AND relationship_type = 'OWNERSHIP'
                 AND (effective_to IS NULL OR effective_to > $1)"#,
        )
        .bind(date_of_death)
        .bind(&reason)
        .bind(entity_id)
        .execute(scope.executor())
        .await?;

        let control_ended = sqlx::query(
            r#"UPDATE "ob-poc".entity_relationships
               SET effective_to = $1,
                   notes = COALESCE(notes, '') || ' [Ended: ' || $2 || ']',
                   updated_at = NOW()
               WHERE from_entity_id = $3
                 AND relationship_type IN ('control', 'trust_role')
                 AND (effective_to IS NULL OR effective_to > $1)"#,
        )
        .bind(date_of_death)
        .bind(&reason)
        .bind(entity_id)
        .execute(scope.executor())
        .await?;

        let affected_cbus: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT DISTINCT v.cbu_id
               FROM "ob-poc".cbu_relationship_verification v
               JOIN "ob-poc".entity_relationships r ON r.relationship_id = v.relationship_id
               WHERE r.from_entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_all(scope.executor())
        .await?;

        let verifications_updated = sqlx::query(
            r#"UPDATE "ob-poc".cbu_relationship_verification
               SET status = 'unverified',
                   discrepancy_notes = COALESCE(discrepancy_notes, '') ||
                       ' [UBO deceased ' || $1::text || ' - requires redetermination]',
                   updated_at = NOW()
               WHERE relationship_id IN (
                   SELECT relationship_id FROM "ob-poc".entity_relationships
                   WHERE from_entity_id = $2
               )"#,
        )
        .bind(date_of_death.to_string())
        .bind(entity_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".entity_workstreams
               SET is_ubo = false, status = 'COMPLETE', completed_at = NOW()
               WHERE entity_id = $1 AND is_ubo = true"#,
        )
        .bind(entity_id)
        .execute(scope.executor())
        .await?;

        for cbu_id in &affected_cbus {
            let _ = sqlx::query(
                r#"INSERT INTO "ob-poc".ubo_assertion_log
                   (cbu_id, assertion_type, expected_value, actual_value, passed, failure_details)
                   VALUES ($1, 'ubo_deceased', true, true, true, $2)"#,
            )
            .bind(cbu_id)
            .bind(json!({
                "entity_id": entity_id,
                "person_name": person_name,
                "date_of_death": date_of_death,
                "reason": reason,
            }))
            .execute(scope.executor())
            .await;
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id,
            "person_name": person_name,
            "date_of_death": date_of_death,
            "reason": reason,
            "ownership_relationships_ended": ownership_ended.rows_affected(),
            "control_relationships_ended": control_ended.rows_affected(),
            "verifications_flagged": verifications_updated.rows_affected(),
            "affected_cbus": affected_cbus,
            "message": "UBO marked deceased. All affected CBUs require UBO redetermination.",
        })))
    }
}

// ── ubo.convergence-supersede ─────────────────────────────────────────────────

pub struct ConvergenceSupersede;

#[async_trait]
impl SemOsVerbOp for ConvergenceSupersede {
    fn fqn(&self) -> &str {
        "ubo.convergence-supersede"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_cbu_id(args, ctx)?;
        let old_relationship_id = if args.get("old-relationship").is_some() {
            json_extract_uuid(args, ctx, "old-relationship")?
        } else {
            json_extract_uuid(args, ctx, "relationship-id")?
        };
        let new_owner_id = json_extract_uuid(args, ctx, "new-owner")?;
        let percentage: Option<rust_decimal::Decimal> = args
            .get("percentage")
            .and_then(|v| v.as_str().and_then(|s| s.parse().ok()));
        let effective_date = json_extract_string_opt(args, "effective-date")
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let reason = json_extract_string(args, "reason")?;

        type OldRel = (
            Uuid,
            Uuid,
            Option<rust_decimal::Decimal>,
            String,
            Option<String>,
            String,
        );
        let old_rel: OldRel = sqlx::query_as(
            r#"SELECT r.from_entity_id, r.to_entity_id, r.percentage, r.relationship_type,
                      r.ownership_type, e.name as from_name
               FROM "ob-poc".entity_relationships r
               JOIN "ob-poc".entities e ON e.entity_id = r.from_entity_id
               WHERE r.relationship_id = $1 AND e.deleted_at IS NULL"#,
        )
        .bind(old_relationship_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Old relationship not found: {}", old_relationship_id))?;

        let new_percentage = percentage.or(old_rel.2);

        sqlx::query(
            r#"UPDATE "ob-poc".entity_relationships
               SET effective_to = $1,
                   notes = COALESCE(notes, '') || ' [Superseded: ' || $2 || ']',
                   updated_at = NOW()
               WHERE relationship_id = $3"#,
        )
        .bind(effective_date)
        .bind(&reason)
        .bind(old_relationship_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".cbu_relationship_verification
               SET status = 'waived',
                   discrepancy_notes = COALESCE(discrepancy_notes, '') ||
                       ' [Superseded on ' || $1::text || ': ' || $2 || ']',
                   resolved_at = NOW(),
                   updated_at = NOW()
               WHERE relationship_id = $3 AND cbu_id = $4"#,
        )
        .bind(effective_date.to_string())
        .bind(&reason)
        .bind(old_relationship_id)
        .bind(cbu_id)
        .execute(scope.executor())
        .await?;

        let new_relationship_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entity_relationships
               (from_entity_id, to_entity_id, relationship_type, percentage, ownership_type,
                effective_from, source, confidence, notes)
               VALUES ($1, $2, $3, $4, $5, $6, 'ubo.supersede', 'MEDIUM', $7)
               RETURNING relationship_id"#,
        )
        .bind(new_owner_id)
        .bind(old_rel.1)
        .bind(&old_rel.3)
        .bind(new_percentage)
        .bind(&old_rel.4)
        .bind(effective_date)
        .bind(format!("Superseded from {} - {}", &old_rel.5, reason))
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_relationship_verification
               (cbu_id, relationship_id, alleged_percentage, allegation_source, status, alleged_at)
               VALUES ($1, $2, $3, 'ubo.supersede', 'alleged', NOW())"#,
        )
        .bind(cbu_id)
        .bind(new_relationship_id)
        .bind(new_percentage)
        .execute(scope.executor())
        .await?;

        let _ = sqlx::query(
            r#"INSERT INTO "ob-poc".ubo_assertion_log
               (cbu_id, assertion_type, expected_value, actual_value, passed, failure_details)
               VALUES ($1, 'ownership_superseded', true, true, true, $2)"#,
        )
        .bind(cbu_id)
        .bind(json!({
            "old_relationship_id": old_relationship_id,
            "new_relationship_id": new_relationship_id,
            "old_owner_id": old_rel.0,
            "new_owner_id": new_owner_id,
            "percentage": new_percentage,
            "effective_date": effective_date,
            "reason": reason,
        }))
        .execute(scope.executor())
        .await;

        ctx.bind("relationship", new_relationship_id);
        Ok(VerbExecutionOutcome::Record(json!({
            "old_relationship_id": old_relationship_id,
            "new_relationship_id": new_relationship_id,
            "old_owner_id": old_rel.0,
            "old_owner_name": old_rel.5,
            "new_owner_id": new_owner_id,
            "to_entity_id": old_rel.1,
            "percentage": new_percentage,
            "effective_date": effective_date,
            "reason": reason,
            "message": "Ownership superseded. New relationship requires verification.",
        })))
    }
}

// ── ubo.transfer-control ──────────────────────────────────────────────────────

pub struct TransferControl;

#[async_trait]
impl SemOsVerbOp for TransferControl {
    fn fqn(&self) -> &str {
        "ubo.transfer-control"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_cbu_id(args, ctx)?;
        let from_controller_id = json_extract_uuid(args, ctx, "from")?;
        let to_controller_id = json_extract_uuid(args, ctx, "to")?;
        let controlled_entity_id = json_extract_uuid(args, ctx, "controlled-entity")?;
        let control_type = json_extract_string(args, "control-type")?;
        let valid_control_types = [
            "board_member",
            "executive",
            "voting_rights",
            "veto_rights",
            "board_appointment",
        ];
        if !valid_control_types.contains(&control_type.as_str()) {
            return Err(anyhow!(
                "Invalid control-type: {}. Valid values: {:?}",
                control_type,
                valid_control_types
            ));
        }
        let effective_date = json_extract_string_opt(args, "effective-date")
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let reason = json_extract_string(args, "reason")?;

        let old_rel: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT relationship_id FROM "ob-poc".entity_relationships
               WHERE from_entity_id = $1
                 AND to_entity_id = $2
                 AND relationship_type = 'control'
                 AND control_type = $3
                 AND (effective_to IS NULL OR effective_to > CURRENT_DATE)"#,
        )
        .bind(from_controller_id)
        .bind(controlled_entity_id)
        .bind(&control_type)
        .fetch_optional(scope.executor())
        .await?;
        let old_relationship_id = old_rel.map(|r| r.0);

        if let Some(old_id) = old_relationship_id {
            sqlx::query(
                r#"UPDATE "ob-poc".entity_relationships
                   SET effective_to = $1,
                       notes = COALESCE(notes, '') || ' [Control transferred: ' || $2 || ']',
                       updated_at = NOW()
                   WHERE relationship_id = $3"#,
            )
            .bind(effective_date)
            .bind(&reason)
            .bind(old_id)
            .execute(scope.executor())
            .await?;

            sqlx::query(
                r#"UPDATE "ob-poc".cbu_relationship_verification
                   SET status = 'waived',
                       discrepancy_notes = COALESCE(discrepancy_notes, '') ||
                           ' [Control transferred on ' || $1::text || ']',
                       resolved_at = NOW(),
                       updated_at = NOW()
                   WHERE relationship_id = $2 AND cbu_id = $3"#,
            )
            .bind(effective_date.to_string())
            .bind(old_id)
            .bind(cbu_id)
            .execute(scope.executor())
            .await?;
        }

        let new_relationship_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entity_relationships
               (from_entity_id, to_entity_id, relationship_type, control_type,
                effective_from, source, confidence, notes)
               VALUES ($1, $2, 'control', $3, $4, 'ubo.transfer-control', 'MEDIUM', $5)
               RETURNING relationship_id"#,
        )
        .bind(to_controller_id)
        .bind(controlled_entity_id)
        .bind(&control_type)
        .bind(effective_date)
        .bind(&reason)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_relationship_verification
               (cbu_id, relationship_id, allegation_source, status, alleged_at)
               VALUES ($1, $2, 'ubo.transfer-control', 'alleged', NOW())"#,
        )
        .bind(cbu_id)
        .bind(new_relationship_id)
        .execute(scope.executor())
        .await?;

        let _ = sqlx::query(
            r#"INSERT INTO "ob-poc".ubo_assertion_log
               (cbu_id, assertion_type, expected_value, actual_value, passed, failure_details)
               VALUES ($1, 'control_transferred', true, true, true, $2)"#,
        )
        .bind(cbu_id)
        .bind(json!({
            "old_relationship_id": old_relationship_id,
            "new_relationship_id": new_relationship_id,
            "from_controller_id": from_controller_id,
            "to_controller_id": to_controller_id,
            "controlled_entity_id": controlled_entity_id,
            "control_type": control_type,
            "effective_date": effective_date,
            "reason": reason,
        }))
        .execute(scope.executor())
        .await;

        ctx.bind("relationship", new_relationship_id);
        Ok(VerbExecutionOutcome::Record(json!({
            "old_relationship_id": old_relationship_id,
            "new_relationship_id": new_relationship_id,
            "from_controller_id": from_controller_id,
            "to_controller_id": to_controller_id,
            "controlled_entity_id": controlled_entity_id,
            "control_type": control_type,
            "effective_date": effective_date,
            "reason": reason,
            "message": "Control transferred. New relationship requires verification.",
        })))
    }
}

// ── ubo.waive-verification ────────────────────────────────────────────────────

pub struct WaiveVerification;

#[async_trait]
impl SemOsVerbOp for WaiveVerification {
    fn fqn(&self) -> &str {
        "ubo.waive-verification"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_cbu_id(args, ctx)?;
        let relationship_id = if args.get("relationship").is_some() {
            json_extract_uuid(args, ctx, "relationship")?
        } else if args.get("relationship-id").is_some() {
            json_extract_uuid(args, ctx, "relationship-id")?
        } else if args.get("edge").is_some() {
            json_extract_uuid(args, ctx, "edge")?
        } else {
            json_extract_uuid(args, ctx, "edge-id")?
        };
        let waiver_type = json_extract_string_opt(args, "waiver-type")
            .or_else(|| json_extract_string_opt(args, "type"))
            .unwrap_or_else(|| "other".to_string());
        let valid_waiver_types = [
            "regulatory_exemption",
            "low_risk",
            "listed_company",
            "government_entity",
            "alternative_verification",
            "other",
        ];
        if !valid_waiver_types.contains(&waiver_type.as_str()) {
            return Err(anyhow!(
                "Invalid waiver-type: {}. Valid values: {:?}",
                waiver_type,
                valid_waiver_types
            ));
        }
        let reason = json_extract_string(args, "reason")?;
        let approved_by = json_extract_string_opt(args, "approved-by")
            .or_else(|| json_extract_string_opt(args, "approver"))
            .ok_or_else(|| anyhow!("Missing approved-by argument"))?;
        let expiry_date: Option<chrono::NaiveDate> = json_extract_string_opt(args, "expires")
            .or_else(|| json_extract_string_opt(args, "expiry-date"))
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (
                SELECT 1 FROM "ob-poc".cbu_relationship_verification
                WHERE cbu_id = $1 AND relationship_id = $2
            )"#,
        )
        .bind(cbu_id)
        .bind(relationship_id)
        .fetch_one(scope.executor())
        .await?;

        if !exists {
            return Err(anyhow!(
                "Relationship verification not found for CBU. relationship_id={}, cbu_id={}",
                relationship_id,
                cbu_id
            ));
        }

        let waiver_notes = format!(
            "WAIVER [{}]: {} | Approved by: {} | Expires: {}",
            waiver_type,
            reason,
            approved_by,
            expiry_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "never".to_string())
        );

        sqlx::query(
            r#"UPDATE "ob-poc".cbu_relationship_verification
               SET status = 'waived',
                   resolved_at = NOW(),
                   resolved_by = $1,
                   discrepancy_notes = COALESCE(discrepancy_notes, '') || E'\n' || $2,
                   updated_at = NOW()
               WHERE relationship_id = $3 AND cbu_id = $4"#,
        )
        .bind(&approved_by)
        .bind(&waiver_notes)
        .bind(relationship_id)
        .bind(cbu_id)
        .execute(scope.executor())
        .await?;

        let _ = sqlx::query(
            r#"INSERT INTO "ob-poc".ubo_assertion_log
               (cbu_id, assertion_type, expected_value, actual_value, passed, failure_details)
               VALUES ($1, 'verification_waived', true, true, true, $2)"#,
        )
        .bind(cbu_id)
        .bind(json!({
            "relationship_id": relationship_id,
            "waiver_type": waiver_type,
            "reason": reason,
            "approved_by": approved_by,
            "expiry_date": expiry_date,
        }))
        .execute(scope.executor())
        .await;

        Ok(VerbExecutionOutcome::Record(json!({
            "relationship_id": relationship_id,
            "cbu_id": cbu_id,
            "status": "waived",
            "waiver_type": waiver_type,
            "reason": reason,
            "approved_by": approved_by,
            "expiry_date": expiry_date,
            "message": "Verification requirement waived. Relationship counts as proven for convergence.",
        })))
    }
}
