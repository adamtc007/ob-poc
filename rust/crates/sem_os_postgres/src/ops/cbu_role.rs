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

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

async fn get_role_id(scope: &mut dyn TransactionScope, role_name: &str) -> Result<Uuid> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT role_id FROM "ob-poc".roles WHERE name = UPPER($1)"#,
    )
    .bind(role_name)
    .fetch_optional(scope.executor())
    .await?;
    row.map(|(id,)| id)
        .ok_or_else(|| anyhow!("Role '{}' not found in taxonomy", role_name))
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
        let rel_result: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entity_relationships
               (from_entity_id, to_entity_id, relationship_type, percentage,
                ownership_type, effective_from, source, confidence, created_at, updated_at)
               VALUES ($1, $2, 'ownership', $3, $4, $5, 'cbu.assign-ownership', 'HIGH', NOW(), NOW())
               ON CONFLICT (from_entity_id, to_entity_id, relationship_type, effective_from)
                   WHERE effective_from IS NOT NULL
               DO UPDATE SET
                   percentage = EXCLUDED.percentage,
                   ownership_type = EXCLUDED.ownership_type,
                   updated_at = NOW()
               RETURNING relationship_id"#,
        )
        .bind(owner_entity_id)
        .bind(owned_entity_id)
        .bind(Some(&percentage))
        .bind(&ownership_type)
        .bind(effective_from)
        .fetch_one(scope.executor())
        .await?;

        ctx.bind("cbu_entity_role", role_result);
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

        let rel_result: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entity_relationships
               (from_entity_id, to_entity_id, relationship_type, control_type,
                effective_from, source, confidence, created_at, updated_at)
               VALUES ($1, $2, 'control', $3, $4, 'cbu.assign-control', 'HIGH', NOW(), NOW())
               ON CONFLICT (from_entity_id, to_entity_id, relationship_type, effective_from)
                   WHERE effective_from IS NOT NULL
               DO UPDATE SET
                   control_type = EXCLUDED.control_type,
                   updated_at = NOW()
               RETURNING relationship_id"#,
        )
        .bind(controller_entity_id)
        .bind(controlled_entity_id)
        .bind(&control_type)
        .bind(appointment_date)
        .fetch_one(scope.executor())
        .await?;

        ctx.bind("cbu_entity_role", role_result);
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

        let rel_result: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entity_relationships
               (from_entity_id, to_entity_id, relationship_type, percentage,
                trust_interest_type, trust_class_description, source, confidence,
                created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, 'cbu.assign-trust-role', 'HIGH', NOW(), NOW())
               ON CONFLICT (from_entity_id, to_entity_id, relationship_type)
                   WHERE effective_from IS NULL AND effective_to IS NULL
               DO UPDATE SET
                   percentage = EXCLUDED.percentage,
                   trust_interest_type = EXCLUDED.trust_interest_type,
                   trust_class_description = EXCLUDED.trust_class_description,
                   updated_at = NOW()
               RETURNING relationship_id"#,
        )
        .bind(participant_entity_id)
        .bind(trust_entity_id)
        .bind(relationship_type)
        .bind(&interest_percentage)
        .bind(interest_type)
        .bind(class_description)
        .fetch_one(scope.executor())
        .await?;

        ctx.bind("cbu_entity_role", role_result);
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
            let rel: Uuid = sqlx::query_scalar(
                r#"INSERT INTO "ob-poc".entity_relationships
                   (from_entity_id, to_entity_id, relationship_type, percentage,
                    is_regulated, regulatory_jurisdiction, source, confidence,
                    created_at, updated_at)
                   VALUES ($1, $2, $3, $4, $5, $6, 'cbu.assign-fund-role', 'HIGH', NOW(), NOW())
                   ON CONFLICT (from_entity_id, to_entity_id, relationship_type)
                       WHERE effective_from IS NULL AND effective_to IS NULL
                   DO UPDATE SET
                       percentage = EXCLUDED.percentage,
                       is_regulated = EXCLUDED.is_regulated,
                       regulatory_jurisdiction = EXCLUDED.regulatory_jurisdiction,
                       updated_at = NOW()
                   RETURNING relationship_id"#,
            )
            .bind(entity_id)
            .bind(fund_id)
            .bind(relationship_type)
            .bind(&investment_percentage)
            .bind(is_regulated)
            .bind(regulatory_jurisdiction)
            .fetch_one(scope.executor())
            .await?;
            Some(rel)
        } else {
            None
        };

        ctx.bind("cbu_entity_role", role_result);
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
        Ok(VerbExecutionOutcome::Record(json!({
            "role_id": role_result,
            "message": format!("Assigned signatory role {} to {}", role, person_entity_id)
        })))
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
                issues.push(
                    "Fund CBU requires MANAGEMENT_COMPANY or INVESTMENT_MANAGER".to_string(),
                );
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
