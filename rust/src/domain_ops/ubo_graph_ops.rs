//! UBO Graph Operations for Convergence Model
//!
//! These operations implement the observation-based KYC convergence model where:
//! 1. Client allegations build an ownership graph (ubo.allege)
//! 2. Proofs are linked to specific edges (ubo.link-proof)
//! 3. Observations extracted from proofs are compared to allegations (ubo.verify)
//! 4. Graph converges when all edges are proven (ubo.status)
//! 5. Assertions gate progression to evaluation and decision (ubo.assert)
//!
//! See: docs/KYC-UBO-SOLUTION-OVERVIEW.md

use anyhow::Result;
use async_trait::async_trait;

use crate::dsl_v2::ast::VerbCall;
use crate::domain_ops::helpers::{extract_cbu_id, extract_entity_ref};
use crate::domain_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;
#[cfg(feature = "database")]
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// UBO LIFECYCLE OPERATIONS
// ═══════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════
// PHASE 7: UBO REMOVAL OPERATIONS
// Handles scenarios: deceased, superseded ownership, control transfer, waiver
// ═══════════════════════════════════════════════════════════════════════════

/// Mark a UBO as deceased and cascade effects
///
/// When a natural person who is a UBO dies:
/// 1. Mark the person entity as deceased
/// 2. End all ownership relationships effective as of death date
/// 3. Update verification status to require re-verification
/// 4. Flag affected CBUs for UBO redetermination
///
/// DSL: (ubo.mark-deceased :entity-id @person :date-of-death "2024-12-01" :reason "Death certificate received")
pub struct UboMarkDeceasedOp;

#[async_trait]
impl CustomOperation for UboMarkDeceasedOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "mark-deceased"
    }
    fn rationale(&self) -> &'static str {
        "Marks person as deceased and cascades effects to ownership relationships and UBO status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        // Extract entity ID (the deceased person)
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id" || a.key == "person-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Extract date of death
        let date_of_death: chrono::NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "date-of-death" || a.key == "death-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .ok_or_else(|| {
                anyhow::anyhow!("Missing or invalid date-of-death argument (format: YYYY-MM-DD)")
            })?;

        // Extract reason/notes
        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason" || a.key == "notes")
            .and_then(|a| a.value.as_string())
            .unwrap_or("Deceased - death certificate received");

        // Verify this is a natural person
        let is_natural_person: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (
                SELECT 1 FROM "ob-poc".entities e
                JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
                WHERE e.entity_id = $1 AND et.entity_category = 'PERSON'
            )"#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await?;

        if !is_natural_person {
            return Err(anyhow::anyhow!(
                "Entity {} is not a natural person. mark-deceased only applies to PERSON entities.",
                entity_id
            ));
        }

        // Get person details for audit
        let person_name: Option<String> =
            sqlx::query_scalar(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                .bind(entity_id)
                .fetch_optional(pool)
                .await?;

        // Begin transaction
        let mut tx = pool.begin().await?;

        // 1. Mark the proper_person record with date of death
        sqlx::query(
            r#"UPDATE "ob-poc".entity_proper_persons
               SET date_of_death = $1, updated_at = NOW()
               WHERE entity_id = $2"#,
        )
        .bind(date_of_death)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;

        // 2. End all active ownership relationships where this person is the owner
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
        .bind(reason)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;

        // 3. End all active control relationships where this person is the controller
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
        .bind(reason)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;

        // 4. Get all affected CBUs and mark verifications as needing review
        let affected_cbus: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT DISTINCT v.cbu_id
               FROM "ob-poc".cbu_relationship_verification v
               JOIN "ob-poc".entity_relationships r ON r.relationship_id = v.relationship_id
               WHERE r.from_entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_all(&mut *tx)
        .await?;

        // 5. Update verification status for all affected relationships
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
        .execute(&mut *tx)
        .await?;

        // 6. Update entity_workstreams.is_ubo = false for this person in all active cases
        sqlx::query(
            r#"UPDATE kyc.entity_workstreams
               SET is_ubo = false,
                   status = 'COMPLETE',
                   completed_at = NOW()
               WHERE entity_id = $1 AND is_ubo = true"#,
        )
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;

        // 7. Log audit event for each affected CBU
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
                "reason": reason
            }))
            .execute(&mut *tx)
            .await;
        }

        tx.commit().await?;

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id,
            "person_name": person_name,
            "date_of_death": date_of_death,
            "reason": reason,
            "ownership_relationships_ended": ownership_ended.rows_affected(),
            "control_relationships_ended": control_ended.rows_affected(),
            "verifications_flagged": verifications_updated.rows_affected(),
            "affected_cbus": affected_cbus,
            "message": "UBO marked deceased. All affected CBUs require UBO redetermination."
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "message": "mark-deceased executed (no-db mode)"
        })))
    }
}

/// Supersede an ownership relationship with reason and audit trail (convergence model)
///
/// Used when ownership changes hands (sale, transfer, restructure).
/// Creates new relationship and ends old one in a single transaction.
/// Note: Named convergence-supersede to distinguish from deprecated ubo.supersede-ubo verb.
///
/// DSL: (ubo.convergence-supersede :cbu @cbu :old-relationship @old :new-owner @new-person :percentage 100 :reason "Share transfer")
pub struct UboConvergenceSupersedeOp;

#[async_trait]
impl CustomOperation for UboConvergenceSupersedeOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "convergence-supersede"
    }
    fn rationale(&self) -> &'static str {
        "Atomically ends old ownership and creates new one with full audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        // Extract CBU ID
        let cbu_id = extract_cbu_id(verb_call, ctx)?;

        // Extract old relationship ID
        let old_relationship_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "old-relationship" || a.key == "relationship-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing old-relationship argument"))?;

        // Extract new owner entity
        let new_owner_id: Uuid = extract_entity_ref(verb_call, "new-owner", ctx, pool).await?;

        // Extract percentage (defaults to same as old relationship)
        let percentage: Option<rust_decimal::Decimal> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "percentage")
            .and_then(|a| a.value.as_decimal());

        // Extract effective date (defaults to today)
        let effective_date: chrono::NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "effective-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Extract reason
        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing reason argument"))?;

        // Get the old relationship details
        let old_rel = sqlx::query!(
            r#"SELECT r.from_entity_id, r.to_entity_id, r.percentage, r.relationship_type,
                      r.ownership_type, e.name as from_name
               FROM "ob-poc".entity_relationships r
               JOIN "ob-poc".entities e ON e.entity_id = r.from_entity_id
               WHERE r.relationship_id = $1"#,
            old_relationship_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Old relationship not found: {}", old_relationship_id))?;

        // Use old percentage if not specified
        let new_percentage = percentage.or_else(|| {
            old_rel
                .percentage
                .as_ref()
                .and_then(|d| rust_decimal::Decimal::from_str_exact(&d.to_string()).ok())
        });

        // Begin transaction
        let mut tx = pool.begin().await?;

        // 1. End the old relationship
        sqlx::query(
            r#"UPDATE "ob-poc".entity_relationships
               SET effective_to = $1,
                   notes = COALESCE(notes, '') || ' [Superseded: ' || $2 || ']',
                   updated_at = NOW()
               WHERE relationship_id = $3"#,
        )
        .bind(effective_date)
        .bind(reason)
        .bind(old_relationship_id)
        .execute(&mut *tx)
        .await?;

        // 2. Update old verification record
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
        .bind(reason)
        .bind(old_relationship_id)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

        // 3. Create new relationship
        let new_relationship_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entity_relationships
               (from_entity_id, to_entity_id, relationship_type, percentage, ownership_type,
                effective_from, source, notes)
               VALUES ($1, $2, $3, $4, $5, $6, 'ubo.supersede', $7)
               RETURNING relationship_id"#,
        )
        .bind(new_owner_id)
        .bind(old_rel.to_entity_id)
        .bind(&old_rel.relationship_type)
        .bind(new_percentage)
        .bind(&old_rel.ownership_type)
        .bind(effective_date)
        .bind(format!(
            "Superseded from {} - {}",
            &old_rel.from_name, reason
        ))
        .fetch_one(&mut *tx)
        .await?;

        // 4. Create new verification record with alleged status
        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_relationship_verification
               (cbu_id, relationship_id, alleged_percentage, allegation_source, status, alleged_at)
               VALUES ($1, $2, $3, 'ubo.supersede', 'alleged', NOW())"#,
        )
        .bind(cbu_id)
        .bind(new_relationship_id)
        .bind(new_percentage)
        .execute(&mut *tx)
        .await?;

        // 5. Log audit event
        let _ = sqlx::query(
            r#"INSERT INTO "ob-poc".ubo_assertion_log
               (cbu_id, assertion_type, expected_value, actual_value, passed, failure_details)
               VALUES ($1, 'ownership_superseded', true, true, true, $2)"#,
        )
        .bind(cbu_id)
        .bind(json!({
            "old_relationship_id": old_relationship_id,
            "new_relationship_id": new_relationship_id,
            "old_owner_id": old_rel.from_entity_id,
            "new_owner_id": new_owner_id,
            "percentage": new_percentage,
            "effective_date": effective_date,
            "reason": reason
        }))
        .execute(&mut *tx)
        .await;

        tx.commit().await?;

        ctx.bind("relationship", new_relationship_id);

        Ok(ExecutionResult::Record(json!({
            "old_relationship_id": old_relationship_id,
            "new_relationship_id": new_relationship_id,
            "old_owner_id": old_rel.from_entity_id,
            "old_owner_name": old_rel.from_name,
            "new_owner_id": new_owner_id,
            "to_entity_id": old_rel.to_entity_id,
            "percentage": new_percentage,
            "effective_date": effective_date,
            "reason": reason,
            "message": "Ownership superseded. New relationship requires verification."
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let new_id = uuid::Uuid::new_v4();
        ctx.bind("relationship", new_id);
        Ok(ExecutionResult::Record(serde_json::json!({
            "new_relationship_id": new_id
        })))
    }
}

/// Transfer control from one entity to another
///
/// Used when control changes hands (board resignation, new executive, voting transfer).
/// Ends old control relationship and creates new one.
///
/// DSL: (ubo.transfer-control :cbu @cbu :from @old-controller :to @new-controller :controlled-entity @entity :control-type "board_member" :reason "Board resignation")
pub struct UboTransferControlOp;

#[async_trait]
impl CustomOperation for UboTransferControlOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "transfer-control"
    }
    fn rationale(&self) -> &'static str {
        "Transfers control relationship from one entity to another with audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        // Extract CBU ID
        let cbu_id = extract_cbu_id(verb_call, ctx)?;

        // Extract from controller
        let from_controller_id: Uuid = extract_entity_ref(verb_call, "from", ctx, pool).await?;

        // Extract to controller
        let to_controller_id: Uuid = extract_entity_ref(verb_call, "to", ctx, pool).await?;

        // Extract controlled entity
        let controlled_entity_id: Uuid =
            extract_entity_ref(verb_call, "controlled-entity", ctx, pool).await?;

        // Extract control type
        let control_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "control-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing control-type argument"))?;

        // Validate control type
        let valid_control_types = [
            "board_member",
            "executive",
            "voting_rights",
            "veto_rights",
            "board_appointment",
        ];
        if !valid_control_types.contains(&control_type) {
            return Err(anyhow::anyhow!(
                "Invalid control-type: {}. Valid values: {:?}",
                control_type,
                valid_control_types
            ));
        }

        // Extract effective date
        let effective_date: chrono::NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "effective-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Extract reason
        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing reason argument"))?;

        // Begin transaction
        let mut tx = pool.begin().await?;

        // 1. Find and end existing control relationship
        let old_rel = sqlx::query!(
            r#"SELECT relationship_id FROM "ob-poc".entity_relationships
               WHERE from_entity_id = $1
                 AND to_entity_id = $2
                 AND relationship_type = 'control'
                 AND control_type = $3
                 AND (effective_to IS NULL OR effective_to > CURRENT_DATE)"#,
            from_controller_id,
            controlled_entity_id,
            control_type
        )
        .fetch_optional(&mut *tx)
        .await?;

        let old_relationship_id = old_rel.map(|r| r.relationship_id);

        // End old relationship if exists
        if let Some(old_id) = old_relationship_id {
            sqlx::query(
                r#"UPDATE "ob-poc".entity_relationships
                   SET effective_to = $1,
                       notes = COALESCE(notes, '') || ' [Control transferred: ' || $2 || ']',
                       updated_at = NOW()
                   WHERE relationship_id = $3"#,
            )
            .bind(effective_date)
            .bind(reason)
            .bind(old_id)
            .execute(&mut *tx)
            .await?;

            // Update verification status
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
            .execute(&mut *tx)
            .await?;
        }

        // 2. Create new control relationship
        let new_relationship_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entity_relationships
               (from_entity_id, to_entity_id, relationship_type, control_type,
                effective_from, source, notes)
               VALUES ($1, $2, 'control', $3, $4, 'ubo.transfer-control', $5)
               RETURNING relationship_id"#,
        )
        .bind(to_controller_id)
        .bind(controlled_entity_id)
        .bind(control_type)
        .bind(effective_date)
        .bind(reason)
        .fetch_one(&mut *tx)
        .await?;

        // 3. Create verification record for new relationship
        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_relationship_verification
               (cbu_id, relationship_id, allegation_source, status, alleged_at)
               VALUES ($1, $2, 'ubo.transfer-control', 'alleged', NOW())"#,
        )
        .bind(cbu_id)
        .bind(new_relationship_id)
        .execute(&mut *tx)
        .await?;

        // 4. Log audit event
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
            "reason": reason
        }))
        .execute(&mut *tx)
        .await;

        tx.commit().await?;

        ctx.bind("relationship", new_relationship_id);

        Ok(ExecutionResult::Record(json!({
            "old_relationship_id": old_relationship_id,
            "new_relationship_id": new_relationship_id,
            "from_controller_id": from_controller_id,
            "to_controller_id": to_controller_id,
            "controlled_entity_id": controlled_entity_id,
            "control_type": control_type,
            "effective_date": effective_date,
            "reason": reason,
            "message": "Control transferred. New relationship requires verification."
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let new_id = uuid::Uuid::new_v4();
        ctx.bind("relationship", new_id);
        Ok(ExecutionResult::Record(serde_json::json!({
            "new_relationship_id": new_id
        })))
    }
}

/// Waive verification for a relationship with justification
///
/// Used when normal verification is not possible or not required:
/// - Regulatory exemption
/// - Low-risk threshold
/// - Alternative verification method
///
/// DSL: (ubo.waive-verification :cbu @cbu :relationship @rel :reason "Regulatory exemption for listed company" :approved-by "senior.analyst@example.com")
pub struct UboWaiveVerificationOp;

#[async_trait]
impl CustomOperation for UboWaiveVerificationOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "waive-verification"
    }
    fn rationale(&self) -> &'static str {
        "Waives verification requirement with documented justification and approval"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        // Extract CBU ID
        let cbu_id = extract_cbu_id(verb_call, ctx)?;

        // Extract relationship ID
        let relationship_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| {
                a.key == "relationship"
                    || a.key == "relationship-id"
                    || a.key == "edge"
                    || a.key == "edge-id"
            })
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing relationship argument"))?;

        // Extract waiver type/category
        let waiver_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "waiver-type" || a.key == "type")
            .and_then(|a| a.value.as_string())
            .unwrap_or("other");

        // Validate waiver type
        let valid_waiver_types = [
            "regulatory_exemption",
            "low_risk",
            "listed_company",
            "government_entity",
            "alternative_verification",
            "other",
        ];
        if !valid_waiver_types.contains(&waiver_type) {
            return Err(anyhow::anyhow!(
                "Invalid waiver-type: {}. Valid values: {:?}",
                waiver_type,
                valid_waiver_types
            ));
        }

        // Extract reason (required)
        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing reason argument"))?;

        // Extract approver (required)
        let approved_by = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "approved-by" || a.key == "approver")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing approved-by argument"))?;

        // Extract expiry date (optional - waivers can expire)
        let expiry_date: Option<chrono::NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "expires" || a.key == "expiry-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        // Verify relationship exists for this CBU
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (
                SELECT 1 FROM "ob-poc".cbu_relationship_verification
                WHERE cbu_id = $1 AND relationship_id = $2
            )"#,
        )
        .bind(cbu_id)
        .bind(relationship_id)
        .fetch_one(pool)
        .await?;

        if !exists {
            return Err(anyhow::anyhow!(
                "Relationship verification not found for CBU. relationship_id={}, cbu_id={}",
                relationship_id,
                cbu_id
            ));
        }

        // Update verification status to waived
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
        .bind(approved_by)
        .bind(&waiver_notes)
        .bind(relationship_id)
        .bind(cbu_id)
        .execute(pool)
        .await?;

        // Log audit event
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
            "expiry_date": expiry_date
        }))
        .execute(pool)
        .await;

        Ok(ExecutionResult::Record(json!({
            "relationship_id": relationship_id,
            "cbu_id": cbu_id,
            "status": "waived",
            "waiver_type": waiver_type,
            "reason": reason,
            "approved_by": approved_by,
            "expiry_date": expiry_date,
            "message": "Verification requirement waived. Relationship counts as proven for convergence."
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "status": "waived"
        })))
    }
}
