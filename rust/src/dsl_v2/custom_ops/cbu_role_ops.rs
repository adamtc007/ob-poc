//! CBU Role Assignment Operations (Role Taxonomy V2)
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

use anyhow::Result;
use async_trait::async_trait;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::custom_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// HELPER: Get role_id from role name
// =============================================================================

#[cfg(feature = "database")]
async fn get_role_id(pool: &PgPool, role_name: &str) -> Result<uuid::Uuid> {
    let row = sqlx::query_scalar!(
        r#"SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($1)"#,
        role_name
    )
    .fetch_optional(pool)
    .await?;

    row.ok_or_else(|| anyhow::anyhow!("Role '{}' not found in taxonomy", role_name))
}

// =============================================================================
// cbu.role:assign - Core role assignment
// =============================================================================

/// Assign a role to an entity within a CBU context
///
/// This is the base role assignment operation. For ownership, control, and trust
/// roles, use the specialized variants that also create relationship edges.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use chrono::NaiveDate;
        use sqlx::types::BigDecimal;
        use uuid::Uuid;

        // Extract required arguments
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

        let role: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!("Missing role argument"))?;

        // Optional arguments
        let target_entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "target-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let percentage: Option<BigDecimal> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "percentage")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse::<BigDecimal>().unwrap_or_default());

        let effective_from: Option<NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "effective-from")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let effective_to: Option<NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "effective-to")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        // Get role_id
        let role_id = get_role_id(pool, &role).await?;

        // Upsert role assignment (idempotent via ON CONFLICT)
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

        // Capture result for :as binding
        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, row);
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "cbu_entity_role_id": row,
            "role": role,
            "entity_id": entity_id,
            "message": format!("Assigned role {} to entity {}", role, entity_id)
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// =============================================================================
// cbu.role:assign-ownership - Ownership role + relationship edge
// =============================================================================

/// Assign an ownership role and create the corresponding entity_relationships edge
///
/// This creates both:
/// 1. cbu_entity_roles entry (role assignment)
/// 2. entity_relationships entry (ownership edge for UBO traversal)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use chrono::NaiveDate;
        use sqlx::types::BigDecimal;
        use uuid::Uuid;

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

        let owner_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "owner-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing owner-entity-id argument"))?;

        let owned_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "owned-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing owned-entity-id argument"))?;

        let percentage: BigDecimal = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "percentage")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse::<BigDecimal>().unwrap_or_default())
            .ok_or_else(|| anyhow::anyhow!("Missing percentage argument"))?;

        let ownership_type: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "ownership-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "DIRECT".to_string());

        let role: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "SHAREHOLDER".to_string());

        let effective_from: Option<NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "effective-from")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

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
            Some(percentage.clone()),
            effective_from
        )
        .fetch_one(&mut *tx)
        .await?;

        // 2. Upsert entity relationship (ownership edge)
        let percentage_display = percentage.to_string();
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
            Some(percentage),
            ownership_type,
            effective_from
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, role_result);
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "role_id": role_result,
            "relationship_id": rel_result,
            "message": format!(
                "Assigned ownership: {} owns {}% of {}",
                owner_entity_id, percentage_display, owned_entity_id
            )
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// =============================================================================
// cbu.role:assign-control - Control role + relationship edge
// =============================================================================

/// Assign a control role and create the corresponding entity_relationships edge
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use chrono::NaiveDate;
        use uuid::Uuid;

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

        let controller_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "controller-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing controller-entity-id argument"))?;

        let controlled_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "controlled-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing controlled-entity-id argument"))?;

        let role: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!("Missing role argument"))?;

        let control_type: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "control-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase());

        let appointment_date: Option<NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "appointment-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

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

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, role_result);
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "role_id": role_result,
            "relationship_id": rel_result,
            "message": format!(
                "Assigned control: {} has {} role over {}",
                controller_entity_id, role, controlled_entity_id
            )
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// =============================================================================
// cbu.role:assign-trust-role - Trust-specific roles
// =============================================================================

/// Assign a trust role (settlor, trustee, beneficiary, protector, etc.)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use sqlx::types::BigDecimal;
        use uuid::Uuid;

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

        let trust_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "trust-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing trust-entity-id argument"))?;

        let participant_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "participant-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing participant-entity-id argument"))?;

        let role: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!("Missing role argument"))?;

        let interest_percentage: Option<BigDecimal> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "interest-percentage")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse::<BigDecimal>().unwrap_or_default());

        let interest_type: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "interest-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let class_description: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "class-description")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let role_id = get_role_id(pool, &role).await?;

        // Map role to relationship type
        let relationship_type = match role.as_str() {
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

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, role_result);
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "role_id": role_result,
            "relationship_id": rel_result,
            "message": format!(
                "Assigned trust role: {} is {} of trust {}",
                participant_entity_id, role, trust_entity_id
            )
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// =============================================================================
// cbu.role:assign-fund-role - Fund structure/management roles
// =============================================================================

/// Assign a fund-related role (ManCo, IM, fund investor, etc.)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use sqlx::types::BigDecimal;
        use uuid::Uuid;

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

        let fund_entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "fund-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let role: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!("Missing role argument"))?;

        let investment_percentage: Option<BigDecimal> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "investment-percentage")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse::<BigDecimal>().unwrap_or_default());

        let is_regulated: bool = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "is-regulated")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let regulatory_jurisdiction: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "regulatory-jurisdiction")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

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
            let relationship_type = match role.as_str() {
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

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, role_result);
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "role_id": role_result,
            "relationship_id": rel_result,
            "message": format!("Assigned fund role {} to entity {}", role, entity_id)
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// =============================================================================
// cbu.role:assign-service-provider - Service provider roles
// =============================================================================

/// Assign a service provider role (admin, custodian, auditor, etc.)
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use chrono::NaiveDate;
        use uuid::Uuid;

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

        let provider_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "provider-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing provider-entity-id argument"))?;

        let client_entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "client-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let role: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!("Missing role argument"))?;

        let service_agreement_date: Option<NaiveDate> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "service-agreement-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

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

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, role_result);
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "role_id": role_result,
            "message": format!(
                "Assigned service provider role {} to {}",
                role, provider_entity_id
            )
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// =============================================================================
// cbu.role:assign-signatory - Trading/execution roles
// =============================================================================

/// Assign a signatory role with authority limits
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use sqlx::types::BigDecimal;
        use uuid::Uuid;

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

        let person_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "person-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing person-entity-id argument"))?;

        let for_entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "for-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let role: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!("Missing role argument"))?;

        let authority_limit: Option<BigDecimal> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "authority-limit")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse::<BigDecimal>().unwrap_or_default());

        let authority_currency: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "authority-currency")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "USD".to_string());

        let requires_co_signatory: bool = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "requires-co-signatory")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

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

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, role_result);
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "role_id": role_result,
            "message": format!("Assigned signatory role {} to {}", role, person_entity_id)
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// =============================================================================
// cbu.role:validate - Validate all CBU role requirements
// =============================================================================

/// Validate that all required roles are assigned and relationships are complete
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Check for basic role requirements based on CBU category
        let cbu = sqlx::query!(
            r#"SELECT cbu_category, client_type FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        let mut issues: Vec<String> = Vec::new();

        // Check for at least one DIRECTOR or equivalent
        let has_director = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                SELECT 1 FROM "ob-poc".cbu_entity_roles cer
                JOIN "ob-poc".roles r ON cer.role_id = r.role_id
                WHERE cer.cbu_id = $1 AND r.role_category = 'CONTROL'
            ) as "exists!""#,
            cbu_id
        )
        .fetch_one(pool)
        .await?;

        if !has_director {
            issues.push("No control role (DIRECTOR, MANAGER, etc.) assigned".to_string());
        }

        // For funds, check for MANAGEMENT_COMPANY
        if cbu.client_type.as_deref() == Some("FUND") {
            let has_manco = sqlx::query_scalar!(
                r#"SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".cbu_entity_roles cer
                    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
                    WHERE cer.cbu_id = $1 AND r.name IN ('MANAGEMENT_COMPANY', 'INVESTMENT_MANAGER')
                ) as "exists!""#,
                cbu_id
            )
            .fetch_one(pool)
            .await?;

            if !has_manco {
                issues
                    .push("Fund CBU requires MANAGEMENT_COMPANY or INVESTMENT_MANAGER".to_string());
            }
        }

        // Check that ownership relationships have matching roles
        let orphan_ownerships = sqlx::query!(
            r#"SELECT COUNT(*) as "count!"
               FROM "ob-poc".entity_relationships er
               WHERE er.relationship_type = 'ownership'
               AND NOT EXISTS (
                   SELECT 1 FROM "ob-poc".cbu_entity_roles cer
                   WHERE cer.entity_id = er.from_entity_id
                   AND cer.cbu_id = $1
               )"#,
            cbu_id
        )
        .fetch_one(pool)
        .await?;

        if orphan_ownerships.count > 0 {
            issues.push(format!(
                "{} ownership relationships without corresponding role assignments",
                orphan_ownerships.count
            ));
        }

        let is_valid = issues.is_empty();

        Ok(ExecutionResult::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "valid": is_valid,
            "issues": issues,
            "cbu_category": cbu.cbu_category,
            "client_type": cbu.client_type
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "valid": true,
            "issues": []
        })))
    }
}
