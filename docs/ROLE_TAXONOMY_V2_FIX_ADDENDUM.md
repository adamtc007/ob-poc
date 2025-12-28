# Role Taxonomy V2 - Fix Addendum: Remove Handlers

## Context

The UBO lifecycle requires removal of role connections during:
- Ownership structure changes (shareholding sold, diluted)
- Control changes (director resigned, removed)
- Trust modifications (beneficiary removed, trustee replaced)
- Entity supersession (company dissolved, merged)
- Death/incapacity of natural persons

The YAML defines a `remove` verb as CRUD, but it can't do the role name → role_id lookup. We need custom handlers that mirror the assign handlers with dual-delete for relationship edges.

---

## TASK 6: Add Remove Handlers to cbu_role_ops.rs

Append to `rust/src/dsl_v2/custom_ops/cbu_role_ops.rs`:

```rust
// =============================================================================
// cbu.role:remove - Remove a role assignment by name
// =============================================================================

/// Remove a role assignment from an entity within a CBU
pub struct CbuRoleRemoveOp;

#[async_trait]
impl CustomOperation for CbuRoleRemoveOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "remove"
    }

    fn rationale(&self) -> &'static str {
        "Role removal requires name-to-id lookup before delete"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
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

        let role: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!("Missing role argument"))?;

        let result = sqlx::query!(
            r#"DELETE FROM "ob-poc".cbu_entity_roles
               WHERE cbu_id = $1 
               AND entity_id = $2 
               AND role_id = (SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($3))"#,
            cbu_id,
            entity_id,
            role
        )
        .execute(pool)
        .await?;

        let affected = result.rows_affected();

        Ok(ExecutionResult::Record(serde_json::json!({
            "affected": affected,
            "role": role,
            "entity_id": entity_id,
            "message": if affected > 0 {
                format!("Removed role {} from entity {}", role, entity_id)
            } else {
                format!("Role {} not found on entity {}", role, entity_id)
            }
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
// cbu.role:remove-ownership - Remove ownership role AND relationship edge
// =============================================================================

/// Remove an ownership role and its corresponding entity_relationships edge
/// 
/// This is the inverse of assign-ownership - removes both the role and the
/// ownership relationship atomically.
pub struct CbuRoleRemoveOwnershipOp;

#[async_trait]
impl CustomOperation for CbuRoleRemoveOwnershipOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "remove-ownership"
    }

    fn rationale(&self) -> &'static str {
        "Ownership removal must delete both role assignment AND relationship edge atomically"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
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

        // Optional: specific role to remove (defaults to any ownership role)
        let role: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase());

        let mut tx = pool.begin().await?;

        // 1. Delete role assignment(s)
        let role_affected = if let Some(ref role_name) = role {
            sqlx::query!(
                r#"DELETE FROM "ob-poc".cbu_entity_roles
                   WHERE cbu_id = $1 
                   AND entity_id = $2
                   AND target_entity_id = $3
                   AND role_id = (SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($4))"#,
                cbu_id,
                owner_entity_id,
                owned_entity_id,
                role_name
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
        } else {
            // Remove all ownership roles for this relationship
            sqlx::query!(
                r#"DELETE FROM "ob-poc".cbu_entity_roles
                   WHERE cbu_id = $1 
                   AND entity_id = $2
                   AND target_entity_id = $3
                   AND role_id IN (
                       SELECT role_id FROM "ob-poc".roles 
                       WHERE role_category = 'OWNERSHIP_CHAIN'
                   )"#,
                cbu_id,
                owner_entity_id,
                owned_entity_id
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
        };

        // 2. Delete ownership relationship edge
        let rel_affected = sqlx::query!(
            r#"DELETE FROM "ob-poc".entity_relationships
               WHERE from_entity_id = $1 
               AND to_entity_id = $2 
               AND relationship_type = 'ownership'"#,
            owner_entity_id,
            owned_entity_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "roles_removed": role_affected,
            "relationships_removed": rel_affected,
            "owner_entity_id": owner_entity_id,
            "owned_entity_id": owned_entity_id,
            "message": format!(
                "Removed ownership: {} roles, {} relationship edges",
                role_affected, rel_affected
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
// cbu.role:remove-control - Remove control role AND relationship edge
// =============================================================================

/// Remove a control role and its corresponding relationship edge
pub struct CbuRoleRemoveControlOp;

#[async_trait]
impl CustomOperation for CbuRoleRemoveControlOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "remove-control"
    }

    fn rationale(&self) -> &'static str {
        "Control removal must delete both role assignment AND relationship edge atomically"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
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

        let role: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase());

        let mut tx = pool.begin().await?;

        // 1. Delete role assignment(s)
        let role_affected = if let Some(ref role_name) = role {
            sqlx::query!(
                r#"DELETE FROM "ob-poc".cbu_entity_roles
                   WHERE cbu_id = $1 
                   AND entity_id = $2
                   AND target_entity_id = $3
                   AND role_id = (SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($4))"#,
                cbu_id,
                controller_entity_id,
                controlled_entity_id,
                role_name
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
        } else {
            sqlx::query!(
                r#"DELETE FROM "ob-poc".cbu_entity_roles
                   WHERE cbu_id = $1 
                   AND entity_id = $2
                   AND target_entity_id = $3
                   AND role_id IN (
                       SELECT role_id FROM "ob-poc".roles 
                       WHERE role_category = 'CONTROL_CHAIN'
                   )"#,
                cbu_id,
                controller_entity_id,
                controlled_entity_id
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
        };

        // 2. Delete control relationship edge
        let rel_affected = sqlx::query!(
            r#"DELETE FROM "ob-poc".entity_relationships
               WHERE from_entity_id = $1 
               AND to_entity_id = $2 
               AND relationship_type = 'control'"#,
            controller_entity_id,
            controlled_entity_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "roles_removed": role_affected,
            "relationships_removed": rel_affected,
            "controller_entity_id": controller_entity_id,
            "controlled_entity_id": controlled_entity_id,
            "message": format!(
                "Removed control: {} roles, {} relationship edges",
                role_affected, rel_affected
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
// cbu.role:remove-trust-role - Remove trust role AND relationship edge
// =============================================================================

/// Remove a trust role and its corresponding relationship edge
pub struct CbuRoleRemoveTrustOp;

#[async_trait]
impl CustomOperation for CbuRoleRemoveTrustOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "remove-trust-role"
    }

    fn rationale(&self) -> &'static str {
        "Trust role removal must delete both role assignment AND relationship edge atomically"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
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

        let role: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_uppercase());

        let mut tx = pool.begin().await?;

        // 1. Delete role assignment(s)
        let role_affected = if let Some(ref role_name) = role {
            sqlx::query!(
                r#"DELETE FROM "ob-poc".cbu_entity_roles
                   WHERE cbu_id = $1 
                   AND entity_id = $2
                   AND target_entity_id = $3
                   AND role_id = (SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($4))"#,
                cbu_id,
                participant_entity_id,
                trust_entity_id,
                role_name
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
        } else {
            sqlx::query!(
                r#"DELETE FROM "ob-poc".cbu_entity_roles
                   WHERE cbu_id = $1 
                   AND entity_id = $2
                   AND target_entity_id = $3
                   AND role_id IN (
                       SELECT role_id FROM "ob-poc".roles 
                       WHERE role_category = 'TRUST_ROLES'
                   )"#,
                cbu_id,
                participant_entity_id,
                trust_entity_id
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
        };

        // 2. Delete trust relationship edge (any trust_* type)
        let rel_affected = sqlx::query!(
            r#"DELETE FROM "ob-poc".entity_relationships
               WHERE from_entity_id = $1 
               AND to_entity_id = $2 
               AND relationship_type LIKE 'trust_%'"#,
            participant_entity_id,
            trust_entity_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "roles_removed": role_affected,
            "relationships_removed": rel_affected,
            "participant_entity_id": participant_entity_id,
            "trust_entity_id": trust_entity_id,
            "message": format!(
                "Removed trust role: {} roles, {} relationship edges",
                role_affected, rel_affected
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
// cbu.role:end-role - Soft-delete by setting effective_to date
// =============================================================================

/// End a role by setting its effective_to date (preserves audit trail)
/// 
/// This is preferred over hard delete when you need to maintain history
/// for compliance/audit purposes.
pub struct CbuRoleEndOp;

#[async_trait]
impl CustomOperation for CbuRoleEndOp {
    fn domain(&self) -> &'static str {
        "cbu.role"
    }

    fn verb(&self) -> &'static str {
        "end-role"
    }

    fn rationale(&self) -> &'static str {
        "Soft-delete preserves audit trail by setting effective_to instead of deleting"
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

        // Default to today if not specified
        let effective_to: NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "effective-to")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let result = sqlx::query!(
            r#"UPDATE "ob-poc".cbu_entity_roles
               SET effective_to = $4, updated_at = NOW()
               WHERE cbu_id = $1 
               AND entity_id = $2 
               AND role_id = (SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($3))
               AND effective_to IS NULL"#,
            cbu_id,
            entity_id,
            role,
            effective_to
        )
        .execute(pool)
        .await?;

        let affected = result.rows_affected();

        Ok(ExecutionResult::Record(serde_json::json!({
            "affected": affected,
            "role": role,
            "entity_id": entity_id,
            "effective_to": effective_to.to_string(),
            "message": if affected > 0 {
                format!("Ended role {} for entity {} effective {}", role, entity_id, effective_to)
            } else {
                format!("No active role {} found on entity {}", role, entity_id)
            }
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
```

---

## TASK 7: Update mod.rs with Remove Handlers

Add to pub use section:
```rust
pub use cbu_role_ops::{
    // ... existing ...
    CbuRoleRemoveOp, CbuRoleRemoveOwnershipOp, CbuRoleRemoveControlOp,
    CbuRoleRemoveTrustOp, CbuRoleEndOp,
};
```

Add to registry:
```rust
// CBU Role remove operations
registry.register(Arc::new(CbuRoleRemoveOp));
registry.register(Arc::new(CbuRoleRemoveOwnershipOp));
registry.register(Arc::new(CbuRoleRemoveControlOp));
registry.register(Arc::new(CbuRoleRemoveTrustOp));
registry.register(Arc::new(CbuRoleEndOp));
```

---

## TASK 8: Update YAML to use plugin behavior

Change `cbu-role-v2.yaml` remove verb from CRUD to plugin:

```yaml
remove:
  description: "Remove a role assignment from entity within CBU"
  behavior: plugin
  custom_handler: cbu_role_remove

  args:
    - name: cbu-id
      type: uuid
      required: true

    - name: entity-id
      type: uuid
      required: true

    - name: role
      type: string
      required: true
      description: "Role to remove (by name)"

  returns:
    type: affected
```

Add new remove verbs:

```yaml
remove-ownership:
  description: "Remove ownership role and relationship edge atomically"
  behavior: plugin
  custom_handler: cbu_role_remove_ownership

  args:
    - name: cbu-id
      type: uuid
      required: true

    - name: owner-entity-id
      type: uuid
      required: true

    - name: owned-entity-id
      type: uuid
      required: true

    - name: role
      type: string
      required: false
      description: "Specific ownership role to remove (optional - removes all if not specified)"

  returns:
    type: record

remove-control:
  description: "Remove control role and relationship edge atomically"
  behavior: plugin
  custom_handler: cbu_role_remove_control

  args:
    - name: cbu-id
      type: uuid
      required: true

    - name: controller-entity-id
      type: uuid
      required: true

    - name: controlled-entity-id
      type: uuid
      required: true

    - name: role
      type: string
      required: false

  returns:
    type: record

remove-trust-role:
  description: "Remove trust role and relationship edge atomically"
  behavior: plugin
  custom_handler: cbu_role_remove_trust

  args:
    - name: cbu-id
      type: uuid
      required: true

    - name: trust-entity-id
      type: uuid
      required: true

    - name: participant-entity-id
      type: uuid
      required: true

    - name: role
      type: string
      required: false

  returns:
    type: record

end-role:
  description: "Soft-end a role by setting effective_to date (preserves audit trail)"
  behavior: plugin
  custom_handler: cbu_role_end

  args:
    - name: cbu-id
      type: uuid
      required: true

    - name: entity-id
      type: uuid
      required: true

    - name: role
      type: string
      required: true

    - name: effective-to
      type: date
      required: false
      description: "End date (defaults to today)"

  returns:
    type: record
```

---

## Summary: Complete Verb Set

| Verb | Purpose | Deletes Edge? |
|------|---------|---------------|
| `remove` | Simple role removal by name | No |
| `remove-ownership` | Remove ownership role + edge | Yes |
| `remove-control` | Remove control role + edge | Yes |
| `remove-trust-role` | Remove trust role + edge | Yes |
| `end-role` | Soft-delete (set effective_to) | No |

## UBO Lifecycle Usage Examples

```clojure
;; Shareholder sold stake
(cbu.role:remove-ownership 
  :cbu-id @cbu 
  :owner-entity-id @old_shareholder 
  :owned-entity-id @company)

;; Director resigned
(cbu.role:remove-control 
  :cbu-id @cbu 
  :controller-entity-id @director 
  :controlled-entity-id @company 
  :role "DIRECTOR")

;; Beneficiary removed from trust (soft-delete for audit)
(cbu.role:end-role 
  :cbu-id @cbu 
  :entity-id @beneficiary 
  :role "BENEFICIARY_DISCRETIONARY" 
  :effective-to "2025-01-15")

;; Ownership superseded - old UBO no longer qualifies
(cbu.role:remove 
  :cbu-id @cbu 
  :entity-id @old_ubo 
  :role "ULTIMATE_BENEFICIAL_OWNER")
```
