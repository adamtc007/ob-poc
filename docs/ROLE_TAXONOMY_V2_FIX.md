# Role Taxonomy V2 - Comprehensive Fix Specification

## Overview

This specification addresses gaps discovered in the Role Taxonomy V2 implementation:
1. Missing custom verb handlers for `cbu-role-v2.yaml`
2. Database view column mismatches
3. Missing unique constraints for idempotency
4. Handler registration in mod.rs

---

## TASK 1: Migration Patch

Create file: `rust/migrations/202501_role_taxonomy_v2_fix.sql`

```sql
-- ═══════════════════════════════════════════════════════════════════════════
-- ROLE TAXONOMY V2 - FIX MIGRATION
-- ═══════════════════════════════════════════════════════════════════════════
-- Fixes:
--   1. View column naming (primary_layout → primary_layout_category)
--   2. Add missing primary_role_category to view
--   3. Unique constraints for idempotent upserts
-- ═══════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 1: UNIQUE CONSTRAINTS FOR IDEMPOTENCY
-- ═══════════════════════════════════════════════════════════════════════════

-- Prevent duplicate role assignments (same entity, same role, same CBU)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'uq_cbu_entity_role'
    ) THEN
        ALTER TABLE "ob-poc".cbu_entity_roles 
        ADD CONSTRAINT uq_cbu_entity_role 
        UNIQUE (cbu_id, entity_id, role_id);
    END IF;
END $$;

-- Prevent duplicate entity relationships
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'uq_entity_relationship'
    ) THEN
        ALTER TABLE "ob-poc".entity_relationships 
        ADD CONSTRAINT uq_entity_relationship 
        UNIQUE (from_entity_id, to_entity_id, relationship_type);
    END IF;
END $$;

-- Prevent duplicate UBO edges
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'uq_ubo_edge'
    ) THEN
        ALTER TABLE "ob-poc".ubo_edges 
        ADD CONSTRAINT uq_ubo_edge 
        UNIQUE (cbu_id, from_entity_id, to_entity_id, edge_type);
    END IF;
END $$;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 2: FIX v_cbu_entity_with_roles VIEW
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW "ob-poc".v_cbu_entity_with_roles AS
WITH role_data AS (
    SELECT
        cer.cbu_id,
        cer.entity_id,
        e.name AS entity_name,
        et.type_code AS entity_type,
        et.entity_category,
        COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) AS jurisdiction,
        r.name AS role_name,
        r.role_category,
        r.layout_category,
        r.display_priority,
        r.ubo_treatment,
        r.requires_percentage,
        r.kyc_obligation
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
    LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
    LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
    LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
)
SELECT
    cbu_id,
    entity_id,
    entity_name,
    entity_type,
    entity_category,
    jurisdiction,
    -- Aggregate roles
    array_agg(role_name ORDER BY display_priority DESC) AS roles,
    array_agg(DISTINCT role_category) FILTER (WHERE role_category IS NOT NULL) AS role_categories,
    array_agg(DISTINCT layout_category) FILTER (WHERE layout_category IS NOT NULL) AS layout_categories,
    -- Primary role (highest priority)
    (array_agg(role_name ORDER BY display_priority DESC))[1] AS primary_role,
    -- Primary role category (FIXED: was missing)
    (array_agg(role_category ORDER BY display_priority DESC) FILTER (WHERE role_category IS NOT NULL))[1] AS primary_role_category,
    -- Primary layout category (FIXED: renamed from primary_layout)
    (array_agg(layout_category ORDER BY display_priority DESC) FILTER (WHERE layout_category IS NOT NULL))[1] AS primary_layout_category,
    -- Max priority for sorting
    max(display_priority) AS max_role_priority,
    -- UBO treatment (most restrictive)
    CASE 
        WHEN 'ALWAYS_UBO' = ANY(array_agg(ubo_treatment)) THEN 'ALWAYS_UBO'
        WHEN 'TERMINUS' = ANY(array_agg(ubo_treatment)) THEN 'TERMINUS'
        WHEN 'CONTROL_PRONG' = ANY(array_agg(ubo_treatment)) THEN 'CONTROL_PRONG'
        WHEN 'BY_PERCENTAGE' = ANY(array_agg(ubo_treatment)) THEN 'BY_PERCENTAGE'
        WHEN 'LOOK_THROUGH' = ANY(array_agg(ubo_treatment)) THEN 'LOOK_THROUGH'
        ELSE 'NOT_APPLICABLE'
    END AS effective_ubo_treatment,
    -- KYC obligation (most stringent)
    CASE
        WHEN 'FULL_KYC' = ANY(array_agg(kyc_obligation)) THEN 'FULL_KYC'
        WHEN 'SCREEN_AND_ID' = ANY(array_agg(kyc_obligation)) THEN 'SCREEN_AND_ID'
        WHEN 'SIMPLIFIED' = ANY(array_agg(kyc_obligation)) THEN 'SIMPLIFIED'
        WHEN 'SCREEN_ONLY' = ANY(array_agg(kyc_obligation)) THEN 'SCREEN_ONLY'
        ELSE 'RECORD_ONLY'
    END AS effective_kyc_obligation
FROM role_data
GROUP BY cbu_id, entity_id, entity_name, entity_type, entity_category, jurisdiction;

COMMENT ON VIEW "ob-poc".v_cbu_entity_with_roles IS
'Aggregated view of entities with their roles, categories, and effective KYC/UBO treatment. 
Fixed in V2.1: Added primary_role_category, renamed primary_layout to primary_layout_category.';
```

---

## TASK 2: Create Custom Handlers

Create file: `rust/src/dsl_v2/custom_ops/cbu_role_ops.rs`

```rust
//! CBU Role Assignment Operations
//!
//! Custom handlers for the cbu.role domain verbs defined in cbu-role-v2.yaml.
//! These operations manage entity role assignments within CBUs with validation.
//!
//! ## Why Custom Operations?
//!
//! Role assignment requires:
//! - Entity type compatibility validation
//! - Role conflict detection
//! - Dual-write to cbu_entity_roles AND entity_relationships (for ownership/control)
//! - Transaction atomicity for related records

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

use super::CustomOperation;

// =============================================================================
// HELPER: Get role_id from role name
// =============================================================================

async fn get_role_id(pool: &PgPool, role_name: &str) -> Result<Uuid> {
    let row = sqlx::query_scalar!(
        r#"SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($1)"#,
        role_name
    )
    .fetch_optional(pool)
    .await?;

    row.ok_or_else(|| anyhow!("Role '{}' not found in taxonomy", role_name))
}

// =============================================================================
// HELPER: Validate role assignment
// =============================================================================

async fn validate_role_assignment(
    pool: &PgPool,
    entity_id: Uuid,
    role_name: &str,
    cbu_id: Uuid,
) -> Result<()> {
    let validation = sqlx::query!(
        r#"SELECT is_valid, error_code, error_message 
           FROM "ob-poc".validate_role_assignment($1, $2, $3)"#,
        entity_id,
        role_name,
        cbu_id
    )
    .fetch_one(pool)
    .await?;

    if !validation.is_valid.unwrap_or(false) {
        return Err(anyhow!(
            "{}: {}",
            validation.error_code.unwrap_or_default(),
            validation.error_message.unwrap_or_default()
        ));
    }

    Ok(())
}

// =============================================================================
// cbu.role:assign - Core role assignment
// =============================================================================

pub struct CbuRoleAssignOp;

#[async_trait]
impl CustomOperation for CbuRoleAssignOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "assign"
    }

    fn rationale(&self) -> &'static str {
        "Role assignment requires entity type validation and conflict detection"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id: Uuid = verb_call.get_arg("cbu-id")?;
        let entity_id: Uuid = verb_call.get_arg("entity-id")?;
        let role: String = verb_call.get_arg("role")?;
        let target_entity_id: Option<Uuid> = verb_call.get_arg_opt("target-entity-id")?;
        let percentage: Option<Decimal> = verb_call.get_arg_opt("percentage")?;
        let effective_from: Option<NaiveDate> = verb_call.get_arg_opt("effective-from")?;
        let effective_to: Option<NaiveDate> = verb_call.get_arg_opt("effective-to")?;
        let skip_validation: bool = verb_call.get_arg_opt("skip-validation")?.unwrap_or(false);

        // Validate unless skipped
        if !skip_validation {
            validate_role_assignment(pool, entity_id, &role, cbu_id).await?;
        }

        // Get role_id
        let role_id = get_role_id(pool, &role).await?;

        // Upsert role assignment (idempotent)
        let row = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".cbu_entity_roles 
               (cbu_id, entity_id, role_id, target_entity_id, ownership_percentage, 
                effective_from, effective_to, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   ownership_percentage = EXCLUDED.ownership_percentage,
                   effective_from = EXCLUDED.effective_from,
                   effective_to = EXCLUDED.effective_to,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
            cbu_id,
            entity_id,
            role_id,
            target_entity_id,
            percentage,
            effective_from,
            effective_to
        )
        .fetch_one(pool)
        .await?;

        // Capture result
        ctx.set_var("cbu_entity_role_id", serde_json::json!(row));

        Ok(ExecutionResult::Success {
            message: format!("Assigned role {} to entity {}", role, entity_id),
            data: Some(serde_json::json!({
                "cbu_entity_role_id": row,
                "role": role,
                "entity_id": entity_id
            })),
        })
    }
}

// =============================================================================
// cbu.role:assign-ownership - Ownership role + relationship edge
// =============================================================================

pub struct CbuRoleAssignOwnershipOp;

#[async_trait]
impl CustomOperation for CbuRoleAssignOwnershipOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "assign-ownership"
    }

    fn rationale(&self) -> &'static str {
        "Ownership creates both role assignment AND entity_relationships edge atomically"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id: Uuid = verb_call.get_arg("cbu-id")?;
        let owner_entity_id: Uuid = verb_call.get_arg("owner-entity-id")?;
        let owned_entity_id: Uuid = verb_call.get_arg("owned-entity-id")?;
        let percentage: Decimal = verb_call.get_arg("percentage")?;
        let ownership_type: String = verb_call
            .get_arg_opt("ownership-type")?
            .unwrap_or_else(|| "DIRECT".to_string());
        let role: String = verb_call
            .get_arg_opt("role")?
            .unwrap_or_else(|| "SHAREHOLDER".to_string());
        let effective_from: Option<NaiveDate> = verb_call.get_arg_opt("effective-from")?;

        // Validate
        validate_role_assignment(pool, owner_entity_id, &role, cbu_id).await?;

        let role_id = get_role_id(pool, &role).await?;

        // Transaction for atomicity
        let mut tx = pool.begin().await?;

        // 1. Upsert role assignment
        let role_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".cbu_entity_roles 
               (cbu_id, entity_id, role_id, target_entity_id, ownership_percentage, 
                effective_from, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   ownership_percentage = EXCLUDED.ownership_percentage,
                   effective_from = EXCLUDED.effective_from,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
            cbu_id,
            owner_entity_id,
            role_id,
            Some(owned_entity_id),
            Some(percentage),
            effective_from
        )
        .fetch_one(&mut *tx)
        .await?;

        // 2. Upsert entity relationship (ownership edge)
        let rel_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".entity_relationships 
               (from_entity_id, to_entity_id, relationship_type, percentage, 
                ownership_type, effective_from, created_at, updated_at)
               VALUES ($1, $2, 'ownership', $3, $4, $5, NOW(), NOW())
               ON CONFLICT (from_entity_id, to_entity_id, relationship_type) DO UPDATE SET
                   percentage = EXCLUDED.percentage,
                   ownership_type = EXCLUDED.ownership_type,
                   effective_from = EXCLUDED.effective_from,
                   updated_at = NOW()
               RETURNING relationship_id"#,
            owner_entity_id,
            owned_entity_id,
            percentage,
            ownership_type,
            effective_from
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        ctx.set_var("role_id", serde_json::json!(role_result));
        ctx.set_var("relationship_id", serde_json::json!(rel_result));

        Ok(ExecutionResult::Success {
            message: format!(
                "Assigned ownership: {} owns {}% of {}",
                owner_entity_id, percentage, owned_entity_id
            ),
            data: Some(serde_json::json!({
                "role_id": role_result,
                "relationship_id": rel_result
            })),
        })
    }
}

// =============================================================================
// cbu.role:assign-control - Control role + relationship edge
// =============================================================================

pub struct CbuRoleAssignControlOp;

#[async_trait]
impl CustomOperation for CbuRoleAssignControlOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "assign-control"
    }

    fn rationale(&self) -> &'static str {
        "Control roles create both role assignment AND control relationship edge"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id: Uuid = verb_call.get_arg("cbu-id")?;
        let controller_entity_id: Uuid = verb_call.get_arg("controller-entity-id")?;
        let controlled_entity_id: Uuid = verb_call.get_arg("controlled-entity-id")?;
        let role: String = verb_call.get_arg("role")?;
        let control_type: Option<String> = verb_call.get_arg_opt("control-type")?;
        let appointment_date: Option<NaiveDate> = verb_call.get_arg_opt("appointment-date")?;

        validate_role_assignment(pool, controller_entity_id, &role, cbu_id).await?;

        let role_id = get_role_id(pool, &role).await?;

        let mut tx = pool.begin().await?;

        // 1. Upsert role assignment
        let role_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".cbu_entity_roles 
               (cbu_id, entity_id, role_id, target_entity_id, effective_from, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   effective_from = EXCLUDED.effective_from,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
            cbu_id,
            controller_entity_id,
            role_id,
            Some(controlled_entity_id),
            appointment_date
        )
        .fetch_one(&mut *tx)
        .await?;

        // 2. Upsert control relationship
        let rel_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".entity_relationships 
               (from_entity_id, to_entity_id, relationship_type, control_type, 
                effective_from, created_at, updated_at)
               VALUES ($1, $2, 'control', $3, $4, NOW(), NOW())
               ON CONFLICT (from_entity_id, to_entity_id, relationship_type) DO UPDATE SET
                   control_type = EXCLUDED.control_type,
                   effective_from = EXCLUDED.effective_from,
                   updated_at = NOW()
               RETURNING relationship_id"#,
            controller_entity_id,
            controlled_entity_id,
            control_type,
            appointment_date
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        ctx.set_var("role_id", serde_json::json!(role_result));
        ctx.set_var("relationship_id", serde_json::json!(rel_result));

        Ok(ExecutionResult::Success {
            message: format!(
                "Assigned control: {} has {} role over {}",
                controller_entity_id, role, controlled_entity_id
            ),
            data: Some(serde_json::json!({
                "role_id": role_result,
                "relationship_id": rel_result
            })),
        })
    }
}

// =============================================================================
// cbu.role:assign-trust-role - Trust-specific roles
// =============================================================================

pub struct CbuRoleAssignTrustOp;

#[async_trait]
impl CustomOperation for CbuRoleAssignTrustOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "assign-trust-role"
    }

    fn rationale(&self) -> &'static str {
        "Trust roles have special UBO treatment and require trust relationship edges"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id: Uuid = verb_call.get_arg("cbu-id")?;
        let trust_entity_id: Uuid = verb_call.get_arg("trust-entity-id")?;
        let participant_entity_id: Uuid = verb_call.get_arg("participant-entity-id")?;
        let role: String = verb_call.get_arg("role")?;
        let interest_percentage: Option<Decimal> = verb_call.get_arg_opt("interest-percentage")?;
        let interest_type: Option<String> = verb_call.get_arg_opt("interest-type")?;
        let class_description: Option<String> = verb_call.get_arg_opt("class-description")?;

        validate_role_assignment(pool, participant_entity_id, &role, cbu_id).await?;

        let role_id = get_role_id(pool, &role).await?;

        // Map role to relationship type
        let relationship_type = match role.to_uppercase().as_str() {
            "SETTLOR" => "trust_settlor",
            "TRUSTEE" => "trust_trustee",
            "PROTECTOR" => "trust_protector",
            "BENEFICIARY_FIXED" | "BENEFICIARY_DISCRETIONARY" | "BENEFICIARY_CONTINGENT" => {
                "trust_beneficiary"
            }
            "ENFORCER" => "trust_enforcer",
            "APPOINTOR" => "trust_appointor",
            _ => "trust_role",
        };

        let mut tx = pool.begin().await?;

        // 1. Upsert role assignment
        let role_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".cbu_entity_roles 
               (cbu_id, entity_id, role_id, target_entity_id, ownership_percentage, 
                created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   ownership_percentage = EXCLUDED.ownership_percentage,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
            cbu_id,
            participant_entity_id,
            role_id,
            Some(trust_entity_id),
            interest_percentage
        )
        .fetch_one(&mut *tx)
        .await?;

        // 2. Upsert trust relationship
        let rel_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".entity_relationships 
               (from_entity_id, to_entity_id, relationship_type, percentage, 
                trust_interest_type, trust_class_description, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
               ON CONFLICT (from_entity_id, to_entity_id, relationship_type) DO UPDATE SET
                   percentage = EXCLUDED.percentage,
                   trust_interest_type = EXCLUDED.trust_interest_type,
                   trust_class_description = EXCLUDED.trust_class_description,
                   updated_at = NOW()
               RETURNING relationship_id"#,
            participant_entity_id,
            trust_entity_id,
            relationship_type,
            interest_percentage,
            interest_type,
            class_description
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        ctx.set_var("role_id", serde_json::json!(role_result));
        ctx.set_var("relationship_id", serde_json::json!(rel_result));

        Ok(ExecutionResult::Success {
            message: format!(
                "Assigned trust role: {} is {} of trust {}",
                participant_entity_id, role, trust_entity_id
            ),
            data: Some(serde_json::json!({
                "role_id": role_result,
                "relationship_id": rel_result
            })),
        })
    }
}

// =============================================================================
// cbu.role:assign-fund-role - Fund structure/management roles
// =============================================================================

pub struct CbuRoleAssignFundOp;

#[async_trait]
impl CustomOperation for CbuRoleAssignFundOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "assign-fund-role"
    }

    fn rationale(&self) -> &'static str {
        "Fund roles connect to fund_structure and may create fund hierarchy relationships"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id: Uuid = verb_call.get_arg("cbu-id")?;
        let entity_id: Uuid = verb_call.get_arg("entity-id")?;
        let fund_entity_id: Option<Uuid> = verb_call.get_arg_opt("fund-entity-id")?;
        let role: String = verb_call.get_arg("role")?;
        let investment_percentage: Option<Decimal> = verb_call.get_arg_opt("investment-percentage")?;
        let is_regulated: bool = verb_call.get_arg_opt("is-regulated")?.unwrap_or(true);
        let regulatory_jurisdiction: Option<String> = verb_call.get_arg_opt("regulatory-jurisdiction")?;

        validate_role_assignment(pool, entity_id, &role, cbu_id).await?;

        let role_id = get_role_id(pool, &role).await?;

        let mut tx = pool.begin().await?;

        // 1. Upsert role assignment
        let role_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".cbu_entity_roles 
               (cbu_id, entity_id, role_id, target_entity_id, ownership_percentage, 
                created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   ownership_percentage = EXCLUDED.ownership_percentage,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
            cbu_id,
            entity_id,
            role_id,
            fund_entity_id,
            investment_percentage
        )
        .fetch_one(&mut *tx)
        .await?;

        // 2. For fund structure roles, create fund relationship if fund_entity_id provided
        let rel_result = if let Some(fund_id) = fund_entity_id {
            let relationship_type = match role.to_uppercase().as_str() {
                "FEEDER_FUND" => "master_feeder",
                "SUB_FUND" => "umbrella_subfund",
                "PARALLEL_FUND" => "parallel",
                "FUND_INVESTOR" => "investment",
                "MANAGEMENT_COMPANY" | "INVESTMENT_MANAGER" => "management",
                _ => "fund_role",
            };

            let rel = sqlx::query_scalar!(
                r#"INSERT INTO "ob-poc".entity_relationships 
                   (from_entity_id, to_entity_id, relationship_type, percentage,
                    is_regulated, regulatory_jurisdiction, created_at, updated_at)
                   VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
                   ON CONFLICT (from_entity_id, to_entity_id, relationship_type) DO UPDATE SET
                       percentage = EXCLUDED.percentage,
                       is_regulated = EXCLUDED.is_regulated,
                       regulatory_jurisdiction = EXCLUDED.regulatory_jurisdiction,
                       updated_at = NOW()
                   RETURNING relationship_id"#,
                entity_id,
                fund_id,
                relationship_type,
                investment_percentage,
                is_regulated,
                regulatory_jurisdiction
            )
            .fetch_one(&mut *tx)
            .await?;
            Some(rel)
        } else {
            None
        };

        tx.commit().await?;

        ctx.set_var("role_id", serde_json::json!(role_result));
        if let Some(rel) = &rel_result {
            ctx.set_var("relationship_id", serde_json::json!(rel));
        }

        Ok(ExecutionResult::Success {
            message: format!("Assigned fund role {} to entity {}", role, entity_id),
            data: Some(serde_json::json!({
                "role_id": role_result,
                "relationship_id": rel_result
            })),
        })
    }
}

// =============================================================================
// cbu.role:assign-service-provider - Service provider roles
// =============================================================================

pub struct CbuRoleAssignServiceOp;

#[async_trait]
impl CustomOperation for CbuRoleAssignServiceOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "assign-service-provider"
    }

    fn rationale(&self) -> &'static str {
        "Service provider roles are flat assignments with no ownership implications"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id: Uuid = verb_call.get_arg("cbu-id")?;
        let provider_entity_id: Uuid = verb_call.get_arg("provider-entity-id")?;
        let client_entity_id: Option<Uuid> = verb_call.get_arg_opt("client-entity-id")?;
        let role: String = verb_call.get_arg("role")?;
        let service_agreement_date: Option<NaiveDate> = verb_call.get_arg_opt("service-agreement-date")?;
        let is_regulated: bool = verb_call.get_arg_opt("is-regulated")?.unwrap_or(true);

        validate_role_assignment(pool, provider_entity_id, &role, cbu_id).await?;

        let role_id = get_role_id(pool, &role).await?;

        // Upsert role assignment (service providers typically don't create relationship edges)
        let role_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".cbu_entity_roles 
               (cbu_id, entity_id, role_id, target_entity_id, effective_from, 
                created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   effective_from = EXCLUDED.effective_from,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
            cbu_id,
            provider_entity_id,
            role_id,
            client_entity_id,
            service_agreement_date
        )
        .fetch_one(pool)
        .await?;

        ctx.set_var("role_id", serde_json::json!(role_result));

        Ok(ExecutionResult::Success {
            message: format!(
                "Assigned service provider role {} to {}",
                role, provider_entity_id
            ),
            data: Some(serde_json::json!({
                "role_id": role_result
            })),
        })
    }
}

// =============================================================================
// cbu.role:assign-signatory - Trading/execution roles
// =============================================================================

pub struct CbuRoleAssignSignatoryOp;

#[async_trait]
impl CustomOperation for CbuRoleAssignSignatoryOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "assign-signatory"
    }

    fn rationale(&self) -> &'static str {
        "Signatory roles are for natural persons with operational authority"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id: Uuid = verb_call.get_arg("cbu-id")?;
        let person_entity_id: Uuid = verb_call.get_arg("person-entity-id")?;
        let for_entity_id: Option<Uuid> = verb_call.get_arg_opt("for-entity-id")?;
        let role: String = verb_call.get_arg("role")?;
        let authority_limit: Option<Decimal> = verb_call.get_arg_opt("authority-limit")?;
        let authority_currency: String = verb_call
            .get_arg_opt("authority-currency")?
            .unwrap_or_else(|| "USD".to_string());
        let requires_co_signatory: bool = verb_call.get_arg_opt("requires-co-signatory")?.unwrap_or(false);

        validate_role_assignment(pool, person_entity_id, &role, cbu_id).await?;

        let role_id = get_role_id(pool, &role).await?;

        // Upsert role assignment with authority metadata
        let role_result = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".cbu_entity_roles 
               (cbu_id, entity_id, role_id, target_entity_id, 
                authority_limit, authority_currency, requires_co_signatory,
                created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   authority_limit = EXCLUDED.authority_limit,
                   authority_currency = EXCLUDED.authority_currency,
                   requires_co_signatory = EXCLUDED.requires_co_signatory,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
            cbu_id,
            person_entity_id,
            role_id,
            for_entity_id,
            authority_limit,
            authority_currency,
            requires_co_signatory
        )
        .fetch_one(pool)
        .await?;

        ctx.set_var("role_id", serde_json::json!(role_result));

        Ok(ExecutionResult::Success {
            message: format!("Assigned signatory role {} to {}", role, person_entity_id),
            data: Some(serde_json::json!({
                "role_id": role_result
            })),
        })
    }
}

// =============================================================================
// cbu.role:validate - Validate all CBU role requirements
// =============================================================================

pub struct CbuRoleValidateAllOp;

#[async_trait]
impl CustomOperation for CbuRoleValidateAllOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "validate"
    }

    fn rationale(&self) -> &'static str {
        "Validates all role requirements for a CBU (e.g., feeder needs master)"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id: Uuid = verb_call.get_arg("cbu-id")?;

        let results = sqlx::query!(
            r#"SELECT 
                requirement_type,
                requiring_role,
                required_role,
                is_satisfied,
                message
               FROM "ob-poc".check_cbu_role_requirements($1)"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let all_satisfied = results.iter().all(|r| r.is_satisfied.unwrap_or(false));
        let unsatisfied: Vec<_> = results
            .iter()
            .filter(|r| !r.is_satisfied.unwrap_or(false))
            .collect();

        if all_satisfied {
            Ok(ExecutionResult::Success {
                message: "All role requirements satisfied".to_string(),
                data: Some(serde_json::json!({
                    "valid": true,
                    "requirements_checked": results.len()
                })),
            })
        } else {
            let messages: Vec<String> = unsatisfied
                .iter()
                .map(|r| r.message.clone().unwrap_or_default())
                .collect();

            Ok(ExecutionResult::Success {
                message: format!("Role requirements not satisfied: {}", messages.join("; ")),
                data: Some(serde_json::json!({
                    "valid": false,
                    "unsatisfied_requirements": unsatisfied.len(),
                    "messages": messages
                })),
            })
        }
    }
}
```

---

## TASK 3: Update mod.rs

Edit file: `rust/src/dsl_v2/custom_ops/mod.rs`

Add after other module declarations (around line 22):
```rust
mod cbu_role_ops;
```

Add to pub use section (around line 50):
```rust
pub use cbu_role_ops::{
    CbuRoleAssignOp, CbuRoleAssignOwnershipOp, CbuRoleAssignControlOp,
    CbuRoleAssignTrustOp, CbuRoleAssignFundOp, CbuRoleAssignServiceOp,
    CbuRoleAssignSignatoryOp, CbuRoleValidateAllOp,
};
```

Add to CustomOperationRegistry::new() (around line 200, after other registrations):
```rust
// CBU Role operations (Role Taxonomy V2)
registry.register(Arc::new(CbuRoleAssignOp));
registry.register(Arc::new(CbuRoleAssignOwnershipOp));
registry.register(Arc::new(CbuRoleAssignControlOp));
registry.register(Arc::new(CbuRoleAssignTrustOp));
registry.register(Arc::new(CbuRoleAssignFundOp));
registry.register(Arc::new(CbuRoleAssignServiceOp));
registry.register(Arc::new(CbuRoleAssignSignatoryOp));
registry.register(Arc::new(CbuRoleValidateAllOp));
```

Add to test function (around line 410):
```rust
// CBU Role operations
assert!(registry.has("cbu.role", "assign"));
assert!(registry.has("cbu.role", "assign-ownership"));
assert!(registry.has("cbu.role", "assign-control"));
assert!(registry.has("cbu.role", "assign-trust-role"));
assert!(registry.has("cbu.role", "assign-fund-role"));
assert!(registry.has("cbu.role", "assign-service-provider"));
assert!(registry.has("cbu.role", "assign-signatory"));
assert!(registry.has("cbu.role", "validate"));
```

---

## TASK 4: Add Missing Columns to cbu_entity_roles

The signatory handler uses columns that may not exist. Add to migration:

```sql
-- Add authority columns to cbu_entity_roles if missing
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles' 
                   AND column_name = 'authority_limit') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN authority_limit DECIMAL(18,2);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles' 
                   AND column_name = 'authority_currency') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN authority_currency VARCHAR(3) DEFAULT 'USD';
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles' 
                   AND column_name = 'requires_co_signatory') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN requires_co_signatory BOOLEAN DEFAULT FALSE;
    END IF;
END $$;
```

---

## TASK 5: Add Trust Columns to entity_relationships

```sql
-- Add trust-specific columns to entity_relationships if missing
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships' 
                   AND column_name = 'trust_interest_type') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN trust_interest_type VARCHAR(30);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships' 
                   AND column_name = 'trust_class_description') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN trust_class_description TEXT;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships' 
                   AND column_name = 'is_regulated') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN is_regulated BOOLEAN DEFAULT TRUE;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships' 
                   AND column_name = 'regulatory_jurisdiction') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN regulatory_jurisdiction VARCHAR(20);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships' 
                   AND column_name = 'control_type') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN control_type VARCHAR(30);
    END IF;
END $$;
```

---

## Verification

After implementation, run these tests:

1. **Migration**: `psql -f rust/migrations/202501_role_taxonomy_v2_fix.sql`

2. **Compile check**: `cargo check --features database`

3. **Test idempotency**:
```sql
-- Run twice - should not error
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
VALUES ('...', '...', '...')
ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET updated_at = NOW();
```

4. **Test view**:
```sql
SELECT entity_id, primary_role, primary_role_category, primary_layout_category
FROM "ob-poc".v_cbu_entity_with_roles
LIMIT 5;
```

5. **Test handlers via DSL**:
```
cbu.role:assign cbu-id=$cbu entity-id=$entity role="SHAREHOLDER" percentage=25.0
cbu.role:validate cbu-id=$cbu
```
