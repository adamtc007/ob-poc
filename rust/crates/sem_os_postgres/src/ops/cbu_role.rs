//! CBU specialist role assignment (7 plugin verbs) — YAML-first
//! re-implementation of `cbu.assign-*` + `cbu.validate-roles` from
//! `rust/config/verbs/cbu-specialist-roles.yaml`.
//!
//! Each `assign-*` verb writes to `cbu_entity_roles` and (where
//! applicable) a matching `entity_relationships` edge for UBO/control
//! traversal. The Sequencer-owned scope replaces the legacy
//! `pool.begin()`/`tx.commit()` pairs — every write uses
//! `scope.executor()` so the surrounding transaction commits atomically.
//!
//! `validate-roles` is a pure read — it inspects CBU classification
//! and checks for required role coverage + orphaned ownership edges.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::types::BigDecimal;
use uuid::Uuid;

use dsl_runtime::SemOsChildDispatcher;
use dsl_runtime::TransactionScope;
use dsl_runtime::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

async fn get_role_id(scope: &mut dyn TransactionScope, role_name: &str) -> Result<Uuid> {
    let row: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($1)"#)
            .bind(role_name)
            .fetch_optional(scope.executor())
            .await?;
    row.map(|(id,)| id)
        .ok_or_else(|| anyhow!("Role '{}' not found in taxonomy", role_name))
}

async fn upsert_entity_relationship(
    parent_fqn: &str,
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    let dispatcher = ctx.service::<dyn SemOsChildDispatcher>()?;
    let outcome = dispatcher
        .dispatch_child(parent_fqn, "entity-relationship.upsert", args, ctx, scope)
        .await?;
    let VerbExecutionOutcome::Record(record) = outcome else {
        return Err(anyhow!(
            "entity-relationship.upsert returned non-record outcome for {parent_fqn}"
        ));
    };
    let relationship_id = record
        .get("relationship_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("entity-relationship.upsert did not return relationship_id"))?;
    relationship_id.parse::<Uuid>().map_err(|err| {
        anyhow!("entity-relationship.upsert returned invalid relationship_id: {err}")
    })
}

pub struct AssignOwnership;

#[async_trait]
impl SemOsVerbOp for AssignOwnership {
    fn fqn(&self) -> &str {
        "cbu.assign-ownership"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let owner_entity_id = json_extract_uuid(args, ctx, "owner-entity-id")?;
        let owned_entity_id = json_extract_uuid(args, ctx, "owned-entity-id")?;
        let percentage: BigDecimal = json_extract_string(args, "percentage")?
            .parse::<BigDecimal>()
            .unwrap_or_default();
        let ownership_type = json_extract_string_opt(args, "ownership-type")
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "DIRECT".to_string());
        let role = json_extract_string_opt(args, "role")
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "SHAREHOLDER".to_string());
        let effective_from = json_extract_string_opt(args, "effective-from")
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let role_id = get_role_id(scope, &role).await?;

        let role_result: Uuid = sqlx::query_scalar(
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
        )
        .bind(cbu_id)
        .bind(owner_entity_id)
        .bind(role_id)
        .bind(Some(owned_entity_id))
        .bind(Some(&percentage))
        .bind(effective_from)
        .fetch_one(scope.executor())
        .await?;

        let percentage_display = percentage.to_string();
        let rel_args = json!({
            "from-entity-id": owner_entity_id,
            "to-entity-id": owned_entity_id,
            "relationship-type": "ownership",
            "percentage": percentage_display,
            "ownership-type": ownership_type,
            "effective-from": effective_from.map(|date| date.to_string()),
            "source": "cbu.assign-ownership",
            "confidence": "HIGH"
        });
        let rel_result = upsert_entity_relationship(self.fqn(), &rel_args, ctx, scope).await?;

        ctx.bind("cbu_entity_role", role_result);
        // Phase C.3 rollout: ownership edge recorded. Subject is the
        // CBU whose structure changed.
        dsl_runtime::emit_pending_state_advance(
            ctx,
            cbu_id,
            "cbu-role:ownership-assigned",
            "cbu/role-graph",
            &format!(
                "cbu.assign-ownership — {} owns {}% of {} (under CBU {})",
                owner_entity_id, percentage_display, owned_entity_id, cbu_id
            ),
        );
        Ok(VerbExecutionOutcome::Record(json!({
            "role_id": role_result,
            "relationship_id": rel_result,
            "message": format!(
                "Assigned ownership: {} owns {}% of {}",
                owner_entity_id, percentage_display, owned_entity_id
            )
        })))
    }
}

pub struct AssignControl;

#[async_trait]
impl SemOsVerbOp for AssignControl {
    fn fqn(&self) -> &str {
        "cbu.assign-control"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let controller_entity_id = json_extract_uuid(args, ctx, "controller-entity-id")?;
        let controlled_entity_id = json_extract_uuid(args, ctx, "controlled-entity-id")?;
        let role = json_extract_string(args, "role")?.to_uppercase();
        let control_type = json_extract_string_opt(args, "control-type").map(|s| s.to_uppercase());
        let appointment_date = json_extract_string_opt(args, "appointment-date")
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let role_id = get_role_id(scope, &role).await?;

        let role_result: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".cbu_entity_roles
               (cbu_id, entity_id, role_id, target_entity_id, effective_from, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   effective_from = EXCLUDED.effective_from,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
        )
        .bind(cbu_id)
        .bind(controller_entity_id)
        .bind(role_id)
        .bind(Some(controlled_entity_id))
        .bind(appointment_date)
        .fetch_one(scope.executor())
        .await?;

        let rel_args = json!({
            "from-entity-id": controller_entity_id,
            "to-entity-id": controlled_entity_id,
            "relationship-type": "control",
            "control-type": control_type,
            "effective-from": appointment_date.map(|date| date.to_string()),
            "source": "cbu.assign-control",
            "confidence": "HIGH"
        });
        let rel_result = upsert_entity_relationship(self.fqn(), &rel_args, ctx, scope).await?;

        ctx.bind("cbu_entity_role", role_result);
        dsl_runtime::emit_pending_state_advance(
            ctx,
            cbu_id,
            "cbu-role:control-assigned",
            "cbu/role-graph",
            &format!(
                "cbu.assign-control — {} has '{}' role over {} (under CBU {})",
                controller_entity_id, role, controlled_entity_id, cbu_id
            ),
        );
        Ok(VerbExecutionOutcome::Record(json!({
            "role_id": role_result,
            "relationship_id": rel_result,
            "message": format!(
                "Assigned control: {} has {} role over {}",
                controller_entity_id, role, controlled_entity_id
            )
        })))
    }
}

pub struct AssignTrustRole;

#[async_trait]
impl SemOsVerbOp for AssignTrustRole {
    fn fqn(&self) -> &str {
        "cbu.assign-trust-role"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let trust_entity_id = json_extract_uuid(args, ctx, "trust-entity-id")?;
        let participant_entity_id = json_extract_uuid(args, ctx, "participant-entity-id")?;
        let role = json_extract_string(args, "role")?.to_uppercase();
        let interest_percentage = json_extract_string_opt(args, "interest-percentage")
            .map(|d| d.parse::<BigDecimal>().unwrap_or_default());
        let interest_type = json_extract_string_opt(args, "interest-type");
        let class_description = json_extract_string_opt(args, "class-description");

        let role_id = get_role_id(scope, &role).await?;
        let trust_role = match role.as_str() {
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

        let role_result: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".cbu_entity_roles
               (cbu_id, entity_id, role_id, target_entity_id, ownership_percentage,
                created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   ownership_percentage = EXCLUDED.ownership_percentage,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
        )
        .bind(cbu_id)
        .bind(participant_entity_id)
        .bind(role_id)
        .bind(Some(trust_entity_id))
        .bind(&interest_percentage)
        .fetch_one(scope.executor())
        .await?;

        let rel_args = json!({
            "from-entity-id": participant_entity_id,
            "to-entity-id": trust_entity_id,
            "relationship-type": "trust_role",
            "percentage": interest_percentage.as_ref().map(ToString::to_string),
            "trust-role": trust_role,
            "trust-interest-type": interest_type,
            "trust-class-description": class_description,
            "source": "cbu.assign-trust-role",
            "confidence": "HIGH"
        });
        let rel_result = upsert_entity_relationship(self.fqn(), &rel_args, ctx, scope).await?;

        ctx.bind("cbu_entity_role", role_result);
        dsl_runtime::emit_pending_state_advance(
            ctx,
            cbu_id,
            "cbu-role:trust-role-assigned",
            "cbu/role-graph",
            &format!(
                "cbu.assign-trust-role — {} is '{}' of trust {} (under CBU {})",
                participant_entity_id, role, trust_entity_id, cbu_id
            ),
        );
        Ok(VerbExecutionOutcome::Record(json!({
            "role_id": role_result,
            "relationship_id": rel_result,
            "message": format!(
                "Assigned trust role: {} is {} of trust {}",
                participant_entity_id, role, trust_entity_id
            )
        })))
    }
}

pub struct AssignFundRole;

#[async_trait]
impl SemOsVerbOp for AssignFundRole {
    fn fqn(&self) -> &str {
        "cbu.assign-fund-role"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let fund_entity_id = json_extract_uuid_opt(args, ctx, "fund-entity-id");
        let role = json_extract_string(args, "role")?.to_uppercase();
        let investment_percentage = json_extract_string_opt(args, "investment-percentage")
            .map(|d| d.parse::<BigDecimal>().unwrap_or_default());
        let is_regulated = json_extract_bool_opt(args, "is-regulated").unwrap_or(true);
        let regulatory_jurisdiction = json_extract_string_opt(args, "regulatory-jurisdiction");

        let role_id = get_role_id(scope, &role).await?;

        let role_result: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".cbu_entity_roles
               (cbu_id, entity_id, role_id, target_entity_id, ownership_percentage,
                created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   ownership_percentage = EXCLUDED.ownership_percentage,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .bind(role_id)
        .bind(fund_entity_id)
        .bind(&investment_percentage)
        .fetch_one(scope.executor())
        .await?;

        let rel_result: Option<Uuid> = if let Some(fund_id) = fund_entity_id {
            let relationship_type = match role.as_str() {
                "FEEDER_FUND" => "master_feeder",
                "SUB_FUND" => "umbrella_subfund",
                "PARALLEL_FUND" => "parallel",
                "FUND_INVESTOR" => "investment",
                "MANAGEMENT_COMPANY" | "INVESTMENT_MANAGER" => "management",
                _ => "fund_role",
            };
            let rel_args = json!({
                "from-entity-id": entity_id,
                "to-entity-id": fund_id,
                "relationship-type": relationship_type,
                "percentage": investment_percentage.as_ref().map(ToString::to_string),
                "is-regulated": is_regulated,
                "regulatory-jurisdiction": regulatory_jurisdiction,
                "source": "cbu.assign-fund-role",
                "confidence": "HIGH"
            });
            let rel = upsert_entity_relationship(self.fqn(), &rel_args, ctx, scope).await?;
            Some(rel)
        } else {
            None
        };

        ctx.bind("cbu_entity_role", role_result);
        dsl_runtime::emit_pending_state_advance(
            ctx,
            cbu_id,
            "cbu-role:fund-role-assigned",
            "cbu/role-graph",
            &format!(
                "cbu.assign-fund-role — entity {} assigned '{}' on CBU {}",
                entity_id, role, cbu_id
            ),
        );
        Ok(VerbExecutionOutcome::Record(json!({
            "role_id": role_result,
            "relationship_id": rel_result,
            "message": format!("Assigned fund role {} to entity {}", role, entity_id)
        })))
    }
}

pub struct AssignServiceProvider;

#[async_trait]
impl SemOsVerbOp for AssignServiceProvider {
    fn fqn(&self) -> &str {
        "cbu.assign-service-provider"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let provider_entity_id = json_extract_uuid(args, ctx, "provider-entity-id")?;
        let client_entity_id = json_extract_uuid_opt(args, ctx, "client-entity-id");
        let role = json_extract_string(args, "role")?.to_uppercase();
        let service_agreement_date = json_extract_string_opt(args, "service-agreement-date")
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let role_id = get_role_id(scope, &role).await?;

        let role_result: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".cbu_entity_roles
               (cbu_id, entity_id, role_id, target_entity_id, effective_from,
                created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               ON CONFLICT (cbu_id, entity_id, role_id) DO UPDATE SET
                   target_entity_id = EXCLUDED.target_entity_id,
                   effective_from = EXCLUDED.effective_from,
                   updated_at = NOW()
               RETURNING cbu_entity_role_id"#,
        )
        .bind(cbu_id)
        .bind(provider_entity_id)
        .bind(role_id)
        .bind(client_entity_id)
        .bind(service_agreement_date)
        .fetch_one(scope.executor())
        .await?;

        ctx.bind("cbu_entity_role", role_result);
        dsl_runtime::emit_pending_state_advance(
            ctx,
            cbu_id,
            "cbu-role:service-provider-assigned",
            "cbu/role-graph",
            &format!(
                "cbu.assign-service-provider — {} serves as '{}' (under CBU {})",
                provider_entity_id, role, cbu_id
            ),
        );
        Ok(VerbExecutionOutcome::Record(json!({
            "role_id": role_result,
            "message": format!(
                "Assigned service provider role {} to {}",
                role, provider_entity_id
            )
        })))
    }
}

pub struct AssignSignatory;

#[async_trait]
impl SemOsVerbOp for AssignSignatory {
    fn fqn(&self) -> &str {
        "cbu.assign-signatory"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let person_entity_id = json_extract_uuid(args, ctx, "person-entity-id")?;
        let for_entity_id = json_extract_uuid_opt(args, ctx, "for-entity-id");
        let role = json_extract_string(args, "role")?.to_uppercase();
        let authority_limit = json_extract_string_opt(args, "authority-limit")
            .map(|d| d.parse::<BigDecimal>().unwrap_or_default());
        let authority_currency = json_extract_string_opt(args, "authority-currency")
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "USD".to_string());
        let requires_co_signatory =
            json_extract_bool_opt(args, "requires-co-signatory").unwrap_or(false);

        let role_id = get_role_id(scope, &role).await?;

        let role_result: Uuid = sqlx::query_scalar(
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
        )
        .bind(cbu_id)
        .bind(person_entity_id)
        .bind(role_id)
        .bind(for_entity_id)
        .bind(&authority_limit)
        .bind(&authority_currency)
        .bind(requires_co_signatory)
        .fetch_one(scope.executor())
        .await?;

        ctx.bind("cbu_entity_role", role_result);
        dsl_runtime::emit_pending_state_advance(
            ctx,
            cbu_id,
            "cbu-role:signatory-assigned",
            "cbu/role-graph",
            &format!(
                "cbu.assign-signatory — {} as '{}' (authority {} {}; co-sig={}; CBU {})",
                person_entity_id,
                role,
                authority_limit
                    .as_ref()
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "—".to_string()),
                authority_currency,
                requires_co_signatory,
                cbu_id
            ),
        );
        Ok(VerbExecutionOutcome::Record(json!({
            "role_id": role_result,
            "message": format!("Assigned signatory role {} to {}", role, person_entity_id)
        })))
    }
}

pub struct Terminate;

#[async_trait]
impl SemOsVerbOp for Terminate {
    fn fqn(&self) -> &str {
        "cbu-role.terminate"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let hard_delete = json_extract_bool_opt(args, "hard-delete").unwrap_or(false);
        let affected = if hard_delete {
            sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(scope.executor())
                .await?
                .rows_affected()
        } else {
            sqlx::query(
                r#"
                UPDATE "ob-poc".cbu_entity_roles
                SET effective_to = CURRENT_DATE,
                    updated_at = NOW()
                WHERE cbu_id = $1
                  AND effective_to IS NULL
                "#,
            )
            .bind(cbu_id)
            .execute(scope.executor())
            .await?
            .rows_affected()
        };

        if affected > 0 {
            dsl_runtime::emit_pending_state_advance(
                ctx,
                cbu_id,
                "cbu-role:terminated",
                "cbu/role-graph",
                "cbu-role.terminate",
            );
        }

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}

pub struct ValidateRoles;

#[async_trait]
impl SemOsVerbOp for ValidateRoles {
    fn fqn(&self) -> &str {
        "cbu.validate-roles"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id: Uuid = json_extract_string(args, "cbu-id")?.parse()?;

        let cbu: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            r#"SELECT cbu_category, client_type FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
        .await?;
        let (cbu_category, client_type) =
            cbu.ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?;

        let mut issues: Vec<String> = Vec::new();

        let has_director: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                SELECT 1 FROM "ob-poc".cbu_entity_roles cer
                JOIN "ob-poc".roles r ON cer.role_id = r.role_id
                WHERE cer.cbu_id = $1 AND r.role_category = 'CONTROL'
            )"#,
        )
        .bind(cbu_id)
        .fetch_one(scope.executor())
        .await?;
        if !has_director {
            issues.push("No control role (DIRECTOR, MANAGER, etc.) assigned".to_string());
        }

        if client_type.as_deref() == Some("FUND") {
            let has_manco: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".cbu_entity_roles cer
                    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
                    WHERE cer.cbu_id = $1 AND r.name IN ('MANAGEMENT_COMPANY', 'INVESTMENT_MANAGER')
                )"#,
            )
            .bind(cbu_id)
            .fetch_one(scope.executor())
            .await?;
            if !has_manco {
                issues
                    .push("Fund CBU requires MANAGEMENT_COMPANY or INVESTMENT_MANAGER".to_string());
            }
        }

        let (orphan_count,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*)
               FROM "ob-poc".entity_relationships er
               WHERE er.relationship_type = 'ownership'
               AND NOT EXISTS (
                   SELECT 1 FROM "ob-poc".cbu_entity_roles cer
                   WHERE cer.entity_id = er.from_entity_id
                   AND cer.cbu_id = $1
               )"#,
        )
        .bind(cbu_id)
        .fetch_one(scope.executor())
        .await?;
        if orphan_count > 0 {
            issues.push(format!(
                "{} ownership relationships without corresponding role assignments",
                orphan_count
            ));
        }

        let is_valid = issues.is_empty();
        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "valid": is_valid,
            "issues": issues,
            "cbu_category": cbu_category,
            "client_type": client_type,
        })))
    }
}
