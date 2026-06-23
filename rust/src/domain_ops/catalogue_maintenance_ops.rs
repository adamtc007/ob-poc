//! Product/service/resource catalogue maintenance verb implementations.
//!
//! These are narrow SQL backings for Phase 1 catalogue-governance verbs.
//! They stay in `ob-poc::domain_ops` because the legacy database services
//! live in this crate and `extend_registry()` is already the host-side
//! extension point for plugin ops not owned by `sem_os_postgres`.

use std::str::FromStr;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rust_decimal::Decimal;
use sem_os_postgres::ops::SemOsVerbOp;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid_opt,
};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

const CATALOGUE_AUTHORITY_ROLES: &[&str] = &[
    "resource_owner",
    "compliance_admin",
    "senior_compliance",
    "mlro",
    "admin",
];

fn require_catalogue_authority(ctx: &VerbExecutionContext, verb_fqn: &str) -> Result<()> {
    if CATALOGUE_AUTHORITY_ROLES
        .iter()
        .any(|role| ctx.principal.has_role(role))
    {
        return Ok(());
    }

    Err(anyhow!(
        "{verb_fqn} requires one of roles: {}",
        CATALOGUE_AUTHORITY_ROLES.join(", ")
    ))
}

fn json_arg(args: &Value, name: &str) -> Option<Value> {
    args.get(name).cloned()
}

fn string_list_arg(args: &Value, name: &str) -> Result<Option<Vec<String>>> {
    let Some(value) = args.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| anyhow!("{name} must contain only strings"))
            })
            .collect::<Result<Vec<_>>>()
            .map(Some),
        Value::String(raw) => Ok(Some(parse_pg_text_array(raw))),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) => {
            Err(anyhow!("{name} must be a string array"))
        }
    }
}

fn parse_pg_text_array(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed == "{}" || trimmed.is_empty() {
        return Vec::new();
    }
    let inner = trimmed
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(trimmed);
    inner
        .split(',')
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_matches('"').replace("\\\"", "\""))
        .collect()
}

fn decimal_arg(args: &Value, name: &str) -> Result<Option<Decimal>> {
    let Some(value) = args.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let raw = value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| value.as_i64().map(|v| v.to_string()))
        .or_else(|| value.as_u64().map(|v| v.to_string()))
        .or_else(|| value.as_f64().map(|v| v.to_string()))
        .ok_or_else(|| anyhow!("{} must be a numeric or string value", name))?;
    Decimal::from_str(&raw)
        .map(Some)
        .map_err(|err| anyhow!("invalid decimal for {}: {}", name, err))
}

fn int_arg(args: &Value, name: &str) -> Result<Option<i32>> {
    let Some(raw) = json_extract_int_opt(args, name) else {
        return Ok(None);
    };
    i32::try_from(raw)
        .map(Some)
        .map_err(|_| anyhow!("{} must fit in a 32-bit integer", name))
}

async fn lookup_product_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(product_id) = json_extract_uuid_opt(args, ctx, "product-id") {
        return Ok(product_id);
    }
    let product = json_extract_string_opt(args, "product")
        .or_else(|| json_extract_string_opt(args, "product-code"))
        .ok_or_else(|| anyhow!("requires product-id, product, or product-code"))?;

    sqlx::query_scalar(
        r#"SELECT product_id
           FROM "ob-poc".products
           WHERE product_code = $1 OR name = $1
           ORDER BY (product_code = $1) DESC
           LIMIT 1"#,
    )
    .bind(&product)
    .fetch_optional(scope.executor())
    .await?
    .ok_or_else(|| anyhow!("Unknown product: {}", product))
}

async fn lookup_service_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(service_id) = json_extract_uuid_opt(args, ctx, "service-id") {
        return Ok(service_id);
    }
    let service = json_extract_string_opt(args, "service")
        .or_else(|| json_extract_string_opt(args, "service-code"))
        .ok_or_else(|| anyhow!("requires service-id, service, or service-code"))?;

    sqlx::query_scalar(
        r#"SELECT service_id
           FROM "ob-poc".services
           WHERE service_code = $1 OR name = $1
           ORDER BY (service_code = $1) DESC
           LIMIT 1"#,
    )
    .bind(&service)
    .fetch_optional(scope.executor())
    .await?
    .ok_or_else(|| anyhow!("Unknown service: {}", service))
}

async fn lookup_resource_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(resource_id) = json_extract_uuid_opt(args, ctx, "resource-id") {
        return Ok(resource_id);
    }
    if let Some(resource_id) = json_extract_uuid_opt(args, ctx, "resource-type-id") {
        return Ok(resource_id);
    }
    let resource = json_extract_string_opt(args, "resource")
        .or_else(|| json_extract_string_opt(args, "resource-code"))
        .or_else(|| json_extract_string_opt(args, "resource-type"))
        .ok_or_else(|| anyhow!("requires resource-id, resource-code, or resource-type"))?;

    sqlx::query_scalar(
        r#"SELECT resource_id
           FROM "ob-poc".service_resource_types
           WHERE resource_code = $1 OR name = $1
           ORDER BY (resource_code = $1) DESC
           LIMIT 1"#,
    )
    .bind(&resource)
    .fetch_optional(scope.executor())
    .await?
    .ok_or_else(|| anyhow!("Unknown service resource type: {}", resource))
}

async fn lookup_capability_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(capability_id) = json_extract_uuid_opt(args, ctx, "capability-id") {
        return Ok(capability_id);
    }
    let service_id = lookup_service_id(args, ctx, scope).await?;
    let resource_id = lookup_resource_id(args, ctx, scope).await?;
    sqlx::query_scalar(
        r#"SELECT capability_id
           FROM "ob-poc".service_resource_capabilities
           WHERE service_id = $1 AND resource_id = $2"#,
    )
    .bind(service_id)
    .bind(resource_id)
    .fetch_optional(scope.executor())
    .await?
    .ok_or_else(|| anyhow!("Unknown service-resource capability"))
}

async fn lookup_owner_principal_fqn(
    args: &Value,
    scope: &mut dyn TransactionScope,
) -> Result<String> {
    if let Some(fqn) = json_extract_string_opt(args, "owner-principal-fqn") {
        return Ok(fqn);
    }
    let owner_system = json_extract_string(args, "owner-system")?;
    sqlx::query_scalar(
        r#"SELECT owner_principal_fqn
           FROM "ob-poc".resource_owner_principals
           WHERE owner_system = $1"#,
    )
    .bind(&owner_system)
    .fetch_optional(scope.executor())
    .await?
    .or_else(|| Some(format!("resource_owner:{owner_system}")))
    .ok_or_else(|| anyhow!("owner principal could not be resolved"))
}

macro_rules! op_struct {
    ($name:ident, $fqn:literal, |$args:ident, $ctx:ident, $scope:ident| $body:block) => {
        pub(super) struct $name;

        #[async_trait]
        impl SemOsVerbOp for $name {
            fn fqn(&self) -> &str {
                $fqn
            }

            async fn execute(
                &self,
                $args: &Value,
                $ctx: &mut VerbExecutionContext,
                $scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                $body
            }
        }
    };
}

op_struct!(ProductDefine, "product.define", |args, ctx, scope| {
    require_catalogue_authority(ctx, "product.define")?;
    let product_id = json_extract_uuid_opt(args, ctx, "product-id").unwrap_or_else(Uuid::new_v4);
    let name = json_extract_string(args, "name")?;
    let description = json_extract_string_opt(args, "description");
    let product_code = json_extract_string_opt(args, "product-code");
    let product_category = json_extract_string_opt(args, "product-category");
    let regulatory_framework = json_extract_string_opt(args, "regulatory-framework");
    let min_asset_requirement = decimal_arg(args, "min-asset-requirement")?;
    let is_active = json_extract_bool_opt(args, "is-active").unwrap_or(true);
    let metadata = json_arg(args, "metadata");
    let kyc_risk_rating = json_extract_string_opt(args, "kyc-risk-rating");
    let kyc_context = json_extract_string_opt(args, "kyc-context");
    let requires_kyc = json_extract_bool_opt(args, "requires-kyc").unwrap_or(true);
    let product_family = json_extract_string_opt(args, "product-family");
    let effective_from = json_extract_string_opt(args, "effective-from");
    let effective_to = json_extract_string_opt(args, "effective-to");
    let owner_principal_fqn = json_extract_string_opt(args, "owner-principal-fqn");
    let governance_status =
        json_extract_string_opt(args, "governance-status").unwrap_or_else(|| "draft".to_string());
    let created_by = Some(ctx.principal.actor_id.clone());

    sqlx::query(
        r#"INSERT INTO "ob-poc".products
           (product_id, name, description, product_code, product_category,
            regulatory_framework, min_asset_requirement, is_active, metadata,
            kyc_risk_rating, kyc_context, requires_kyc, product_family,
            effective_from, effective_to, owner_principal_fqn, governance_status,
            created_by, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                   COALESCE($14::timestamptz, NOW()), $15::timestamptz,
                   $16, $17, $18, NOW(), NOW())
           ON CONFLICT (product_id) DO UPDATE SET
             name = EXCLUDED.name,
             description = EXCLUDED.description,
             product_code = EXCLUDED.product_code,
             product_category = EXCLUDED.product_category,
             regulatory_framework = EXCLUDED.regulatory_framework,
             min_asset_requirement = EXCLUDED.min_asset_requirement,
             is_active = EXCLUDED.is_active,
             metadata = EXCLUDED.metadata,
             kyc_risk_rating = EXCLUDED.kyc_risk_rating,
             kyc_context = EXCLUDED.kyc_context,
             requires_kyc = EXCLUDED.requires_kyc,
             product_family = EXCLUDED.product_family,
             effective_from = EXCLUDED.effective_from,
             effective_to = EXCLUDED.effective_to,
             owner_principal_fqn = EXCLUDED.owner_principal_fqn,
             governance_status = EXCLUDED.governance_status,
             updated_at = NOW()"#,
    )
    .bind(product_id)
    .bind(&name)
    .bind(&description)
    .bind(&product_code)
    .bind(&product_category)
    .bind(&regulatory_framework)
    .bind(min_asset_requirement)
    .bind(is_active)
    .bind(&metadata)
    .bind(&kyc_risk_rating)
    .bind(&kyc_context)
    .bind(requires_kyc)
    .bind(&product_family)
    .bind(&effective_from)
    .bind(&effective_to)
    .bind(&owner_principal_fqn)
    .bind(&governance_status)
    .bind(&created_by)
    .execute(scope.executor())
    .await?;

    ctx.bind_typed("product", product_id, "product");
    Ok(VerbExecutionOutcome::Uuid(product_id))
});

op_struct!(ProductAmend, "product.amend", |args, ctx, scope| {
    require_catalogue_authority(ctx, "product.amend")?;
    let product_id = lookup_product_id(args, ctx, scope).await?;
    let name = json_extract_string_opt(args, "name");
    let description = json_extract_string_opt(args, "description");
    let product_code = json_extract_string_opt(args, "product-code");
    let product_category = json_extract_string_opt(args, "product-category");
    let regulatory_framework = json_extract_string_opt(args, "regulatory-framework");
    let min_asset_requirement = decimal_arg(args, "min-asset-requirement")?;
    let is_active = json_extract_bool_opt(args, "is-active");
    let metadata = json_arg(args, "metadata");
    let kyc_risk_rating = json_extract_string_opt(args, "kyc-risk-rating");
    let kyc_context = json_extract_string_opt(args, "kyc-context");
    let requires_kyc = json_extract_bool_opt(args, "requires-kyc");
    let product_family = json_extract_string_opt(args, "product-family");
    let effective_from = json_extract_string_opt(args, "effective-from");
    let effective_to = json_extract_string_opt(args, "effective-to");
    let owner_principal_fqn = json_extract_string_opt(args, "owner-principal-fqn");
    let governance_status = json_extract_string_opt(args, "governance-status");

    let affected = sqlx::query(
        r#"UPDATE "ob-poc".products SET
             name = COALESCE($2, name),
             description = COALESCE($3, description),
             product_code = COALESCE($4, product_code),
             product_category = COALESCE($5, product_category),
             regulatory_framework = COALESCE($6, regulatory_framework),
             min_asset_requirement = COALESCE($7, min_asset_requirement),
             is_active = COALESCE($8, is_active),
             metadata = COALESCE($9, metadata),
             kyc_risk_rating = COALESCE($10, kyc_risk_rating),
             kyc_context = COALESCE($11, kyc_context),
             requires_kyc = COALESCE($12, requires_kyc),
             product_family = COALESCE($13, product_family),
             effective_from = COALESCE($14::timestamptz, effective_from),
             effective_to = COALESCE($15::timestamptz, effective_to),
             owner_principal_fqn = COALESCE($16, owner_principal_fqn),
             governance_status = COALESCE($17, governance_status),
             updated_at = NOW()
           WHERE product_id = $1"#,
    )
    .bind(product_id)
    .bind(&name)
    .bind(&description)
    .bind(&product_code)
    .bind(&product_category)
    .bind(&regulatory_framework)
    .bind(min_asset_requirement)
    .bind(is_active)
    .bind(&metadata)
    .bind(&kyc_risk_rating)
    .bind(&kyc_context)
    .bind(requires_kyc)
    .bind(&product_family)
    .bind(&effective_from)
    .bind(&effective_to)
    .bind(&owner_principal_fqn)
    .bind(&governance_status)
    .execute(scope.executor())
    .await?
    .rows_affected();
    Ok(VerbExecutionOutcome::Affected(affected))
});

op_struct!(ProductRetire, "product.retire", |args, ctx, scope| {
    require_catalogue_authority(ctx, "product.retire")?;
    let product_id = lookup_product_id(args, ctx, scope).await?;
    let affected = sqlx::query(
        r#"UPDATE "ob-poc".products
           SET governance_status = 'retired',
               is_active = false,
               effective_to = COALESCE(effective_to, NOW()),
               updated_at = NOW()
           WHERE product_id = $1"#,
    )
    .bind(product_id)
    .execute(scope.executor())
    .await?
    .rows_affected();
    Ok(VerbExecutionOutcome::Affected(affected))
});

op_struct!(ServiceDefine, "service.define", |args, ctx, scope| {
    require_catalogue_authority(ctx, "service.define")?;
    let service_id = json_extract_uuid_opt(args, ctx, "service-id").unwrap_or_else(Uuid::new_v4);
    let name = json_extract_string(args, "name")?;
    let description = json_extract_string_opt(args, "description");
    let service_code = json_extract_string_opt(args, "service-code");
    let service_category = json_extract_string_opt(args, "service-category");
    let sla_definition = json_arg(args, "sla-definition");
    let is_active = json_extract_bool_opt(args, "is-active").unwrap_or(true);
    let lifecycle_tags = string_list_arg(args, "lifecycle-tags")?.unwrap_or_default();
    let lifecycle_status = json_extract_string_opt(args, "lifecycle-status")
        .unwrap_or_else(|| "ungoverned".to_string());
    let owner_principal_fqn = json_extract_string_opt(args, "owner-principal-fqn");
    let governance_status =
        json_extract_string_opt(args, "governance-status").unwrap_or_else(|| "active".to_string());
    let created_by = Some(ctx.principal.actor_id.clone());
    let created_at = json_extract_string_opt(args, "created-at");
    let updated_at = json_extract_string_opt(args, "updated-at");

    sqlx::query(
        r#"INSERT INTO "ob-poc".services
           (service_id, name, description, service_code, service_category,
            sla_definition, is_active, lifecycle_tags, lifecycle_status,
            owner_principal_fqn, governance_status, created_by, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                   COALESCE($13::timestamptz, NOW()),
                   COALESCE($14::timestamptz, NOW()))
           ON CONFLICT (service_id) DO UPDATE SET
             name = EXCLUDED.name,
             description = EXCLUDED.description,
             service_code = EXCLUDED.service_code,
             service_category = EXCLUDED.service_category,
             sla_definition = EXCLUDED.sla_definition,
             is_active = EXCLUDED.is_active,
             lifecycle_tags = EXCLUDED.lifecycle_tags,
             lifecycle_status = EXCLUDED.lifecycle_status,
             owner_principal_fqn = EXCLUDED.owner_principal_fqn,
             governance_status = EXCLUDED.governance_status,
             updated_at = EXCLUDED.updated_at"#,
    )
    .bind(service_id)
    .bind(&name)
    .bind(&description)
    .bind(&service_code)
    .bind(&service_category)
    .bind(&sla_definition)
    .bind(is_active)
    .bind(&lifecycle_tags)
    .bind(&lifecycle_status)
    .bind(&owner_principal_fqn)
    .bind(&governance_status)
    .bind(&created_by)
    .bind(&created_at)
    .bind(&updated_at)
    .execute(scope.executor())
    .await?;

    ctx.bind_typed("service", service_id, "service");
    Ok(VerbExecutionOutcome::Uuid(service_id))
});

op_struct!(
    ServiceVersionDraft,
    "service-version.draft",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "service-version.draft")?;
        let version_id = json_extract_uuid_opt(args, ctx, "version-id")
            .or_else(|| json_extract_uuid_opt(args, ctx, "id"))
            .unwrap_or_else(Uuid::new_v4);
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let version = json_extract_string(args, "version")?;
        let lifecycle_status = json_extract_string_opt(args, "lifecycle-status")
            .unwrap_or_else(|| "drafted".to_string());
        let spec = json_arg(args, "spec");
        let drafted_at = json_extract_string_opt(args, "drafted-at");
        let reviewed_at = json_extract_string_opt(args, "reviewed-at");
        let published_at = json_extract_string_opt(args, "published-at");
        let superseded_at = json_extract_string_opt(args, "superseded-at");
        let retired_at = json_extract_string_opt(args, "retired-at");
        let notes = json_extract_string_opt(args, "notes");
        let created_at = json_extract_string_opt(args, "created-at");
        let updated_at = json_extract_string_opt(args, "updated-at");

        sqlx::query(
            r#"INSERT INTO "ob-poc".service_versions
           (id, service_id, version, lifecycle_status, spec,
            drafted_at, reviewed_at, published_at, superseded_at, retired_at,
            notes, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5,
                   COALESCE($6::timestamptz, NOW()), $7::timestamptz,
                   $8::timestamptz, $9::timestamptz, $10::timestamptz,
                   $11, COALESCE($12::timestamptz, NOW()),
                   COALESCE($13::timestamptz, NOW()))
           ON CONFLICT (service_id, version) DO UPDATE SET
             id = EXCLUDED.id,
             lifecycle_status = EXCLUDED.lifecycle_status,
             spec = EXCLUDED.spec,
             drafted_at = EXCLUDED.drafted_at,
             reviewed_at = EXCLUDED.reviewed_at,
             published_at = EXCLUDED.published_at,
             superseded_at = EXCLUDED.superseded_at,
             retired_at = EXCLUDED.retired_at,
             notes = EXCLUDED.notes,
             updated_at = EXCLUDED.updated_at"#,
        )
        .bind(version_id)
        .bind(service_id)
        .bind(&version)
        .bind(&lifecycle_status)
        .bind(&spec)
        .bind(&drafted_at)
        .bind(&reviewed_at)
        .bind(&published_at)
        .bind(&superseded_at)
        .bind(&retired_at)
        .bind(&notes)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(scope.executor())
        .await?;

        ctx.bind_typed("service_version", version_id, "service_version");
        Ok(VerbExecutionOutcome::Record(json!({
            "id": version_id,
            "service_id": service_id,
            "version": version,
            "lifecycle_status": lifecycle_status
        })))
    }
);

op_struct!(
    ServiceResourceDefineType,
    "service-resource.define-type",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "service-resource.define-type")?;
        let resource_id = json_extract_uuid_opt(args, ctx, "resource-id")
            .or_else(|| json_extract_uuid_opt(args, ctx, "resource-type-id"))
            .unwrap_or_else(Uuid::new_v4);
        let name = json_extract_string(args, "name")?;
        let owner = json_extract_string(args, "owner")?;
        let description = json_extract_string_opt(args, "description");
        let dictionary_group = json_extract_string_opt(args, "dictionary-group");
        let resource_code = json_extract_string_opt(args, "resource-code");
        let resource_type = json_extract_string_opt(args, "resource-type");
        let vendor = json_extract_string_opt(args, "vendor");
        let version = json_extract_string_opt(args, "version");
        let api_endpoint = json_extract_string_opt(args, "api-endpoint");
        let api_version = json_extract_string_opt(args, "api-version");
        let authentication_method = json_extract_string_opt(args, "authentication-method");
        let is_active = json_extract_bool_opt(args, "is-active").unwrap_or(true);
        let owner_principal_fqn = json_extract_string_opt(args, "owner-principal-fqn")
            .or_else(|| Some(format!("resource_owner:{owner}")));
        let governance_status = json_extract_string_opt(args, "governance-status")
            .unwrap_or_else(|| "draft".to_string());

        sqlx::query(
            r#"INSERT INTO "ob-poc".service_resource_types
           (resource_id, name, description, owner, dictionary_group, resource_code,
            resource_type, vendor, version, api_endpoint, api_version,
            authentication_method, is_active, owner_principal_fqn, governance_status,
            created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, NOW(), NOW())
           ON CONFLICT (resource_id) DO UPDATE SET
             name = EXCLUDED.name,
             description = EXCLUDED.description,
             owner = EXCLUDED.owner,
             dictionary_group = EXCLUDED.dictionary_group,
             resource_code = EXCLUDED.resource_code,
             resource_type = EXCLUDED.resource_type,
             vendor = EXCLUDED.vendor,
             version = EXCLUDED.version,
             api_endpoint = EXCLUDED.api_endpoint,
             api_version = EXCLUDED.api_version,
             authentication_method = EXCLUDED.authentication_method,
             is_active = EXCLUDED.is_active,
             owner_principal_fqn = EXCLUDED.owner_principal_fqn,
             governance_status = EXCLUDED.governance_status,
             updated_at = NOW()"#,
        )
        .bind(resource_id)
        .bind(&name)
        .bind(&description)
        .bind(&owner)
        .bind(&dictionary_group)
        .bind(&resource_code)
        .bind(&resource_type)
        .bind(&vendor)
        .bind(&version)
        .bind(&api_endpoint)
        .bind(&api_version)
        .bind(&authentication_method)
        .bind(is_active)
        .bind(&owner_principal_fqn)
        .bind(&governance_status)
        .execute(scope.executor())
        .await?;

        ctx.bind_typed("resource", resource_id, "resource_type");
        Ok(VerbExecutionOutcome::Uuid(resource_id))
    }
);

op_struct!(
    ServiceResourceAmendType,
    "service-resource.amend-type",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "service-resource.amend-type")?;
        let resource_id = lookup_resource_id(args, ctx, scope).await?;
        let name = json_extract_string_opt(args, "name");
        let description = json_extract_string_opt(args, "description");
        let owner = json_extract_string_opt(args, "owner");
        let dictionary_group = json_extract_string_opt(args, "dictionary-group");
        let resource_code = json_extract_string_opt(args, "resource-code");
        let resource_type = json_extract_string_opt(args, "resource-type");
        let vendor = json_extract_string_opt(args, "vendor");
        let version = json_extract_string_opt(args, "version");
        let api_endpoint = json_extract_string_opt(args, "api-endpoint");
        let api_version = json_extract_string_opt(args, "api-version");
        let authentication_method = json_extract_string_opt(args, "authentication-method");
        let is_active = json_extract_bool_opt(args, "is-active");
        let owner_principal_fqn = json_extract_string_opt(args, "owner-principal-fqn");
        let governance_status = json_extract_string_opt(args, "governance-status");

        let affected = sqlx::query(
            r#"UPDATE "ob-poc".service_resource_types SET
             name = COALESCE($2, name),
             description = COALESCE($3, description),
             owner = COALESCE($4, owner),
             dictionary_group = COALESCE($5, dictionary_group),
             resource_code = COALESCE($6, resource_code),
             resource_type = COALESCE($7, resource_type),
             vendor = COALESCE($8, vendor),
             version = COALESCE($9, version),
             api_endpoint = COALESCE($10, api_endpoint),
             api_version = COALESCE($11, api_version),
             authentication_method = COALESCE($12, authentication_method),
             is_active = COALESCE($13, is_active),
             owner_principal_fqn = COALESCE($14, owner_principal_fqn),
             governance_status = COALESCE($15, governance_status),
             updated_at = NOW()
           WHERE resource_id = $1"#,
        )
        .bind(resource_id)
        .bind(&name)
        .bind(&description)
        .bind(&owner)
        .bind(&dictionary_group)
        .bind(&resource_code)
        .bind(&resource_type)
        .bind(&vendor)
        .bind(&version)
        .bind(&api_endpoint)
        .bind(&api_version)
        .bind(&authentication_method)
        .bind(is_active)
        .bind(&owner_principal_fqn)
        .bind(&governance_status)
        .execute(scope.executor())
        .await?
        .rows_affected();
        Ok(VerbExecutionOutcome::Affected(affected))
    }
);

op_struct!(
    ServiceResourceRetireType,
    "service-resource.retire-type",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "service-resource.retire-type")?;
        let resource_id = lookup_resource_id(args, ctx, scope).await?;
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".service_resource_types
           SET governance_status = 'retired',
               is_active = false,
               updated_at = NOW()
           WHERE resource_id = $1"#,
        )
        .bind(resource_id)
        .execute(scope.executor())
        .await?
        .rows_affected();
        Ok(VerbExecutionOutcome::Affected(affected))
    }
);

op_struct!(
    ProductServiceLink,
    "product-service.link",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "product-service.link")?;
        let product_id = lookup_product_id(args, ctx, scope).await?;
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let is_mandatory = json_extract_bool_opt(args, "is-mandatory").unwrap_or(false);
        let is_default = json_extract_bool_opt(args, "is-default").unwrap_or(false);
        let display_order = int_arg(args, "display-order")?;
        let configuration = json_arg(args, "configuration");
        let predicate_dsl = json_extract_string_opt(args, "predicate-dsl")
            .or_else(|| json_extract_string_opt(args, "predicate_dsl"))
            .filter(|value| !value.trim().is_empty());

        if let Some(predicate_dsl) = predicate_dsl {
            let condition_key =
                json_extract_string_opt(args, "condition-key").unwrap_or_else(|| {
                    let digest = blake3::hash(predicate_dsl.as_bytes()).to_hex();
                    format!("product-service:{product_id}:{service_id}:{digest}")
                });
            let description = json_extract_string_opt(args, "description");

            sqlx::query(
                r#"INSERT INTO "ob-poc".product_service_conditions
               (condition_key, description, predicate_dsl, lifecycle_status,
                product_id, service_id, is_mandatory, is_default, display_order, configuration)
               VALUES ($1, $2, $3, 'active', $4, $5, $6, $7, $8, COALESCE($9, '{}'::jsonb))
               ON CONFLICT (condition_key) DO UPDATE SET
                 description = EXCLUDED.description,
                 predicate_dsl = EXCLUDED.predicate_dsl,
                 lifecycle_status = 'active',
                 product_id = EXCLUDED.product_id,
                 service_id = EXCLUDED.service_id,
                 is_mandatory = EXCLUDED.is_mandatory,
                 is_default = EXCLUDED.is_default,
                 display_order = EXCLUDED.display_order,
                 configuration = EXCLUDED.configuration,
                 updated_at = NOW()"#,
            )
            .bind(&condition_key)
            .bind(&description)
            .bind(&predicate_dsl)
            .bind(product_id)
            .bind(service_id)
            .bind(is_mandatory)
            .bind(is_default)
            .bind(display_order)
            .bind(&configuration)
            .execute(scope.executor())
            .await?;
            return Ok(VerbExecutionOutcome::Record(json!({
                "product_id": product_id,
                "service_id": service_id,
                "condition_key": condition_key,
                "conditional": true
            })));
        }

        sqlx::query(
            r#"INSERT INTO "ob-poc".product_services
           (product_id, service_id, is_mandatory, is_default, display_order, configuration)
           VALUES ($1, $2, $3, $4, $5, $6)
           ON CONFLICT (product_id, service_id) DO UPDATE SET
             is_mandatory = EXCLUDED.is_mandatory,
             is_default = EXCLUDED.is_default,
             display_order = EXCLUDED.display_order,
             configuration = EXCLUDED.configuration"#,
        )
        .bind(product_id)
        .bind(service_id)
        .bind(is_mandatory)
        .bind(is_default)
        .bind(display_order)
        .bind(&configuration)
        .execute(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "product_id": product_id,
            "service_id": service_id
        })))
    }
);

op_struct!(
    ProductServiceAmend,
    "product-service.amend",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "product-service.amend")?;
        let product_id = lookup_product_id(args, ctx, scope).await?;
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let is_mandatory = json_extract_bool_opt(args, "is-mandatory");
        let is_default = json_extract_bool_opt(args, "is-default");
        let display_order = int_arg(args, "display-order")?;
        let configuration = json_arg(args, "configuration");
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".product_services SET
             is_mandatory = COALESCE($3, is_mandatory),
             is_default = COALESCE($4, is_default),
             display_order = COALESCE($5, display_order),
             configuration = COALESCE($6, configuration)
           WHERE product_id = $1 AND service_id = $2"#,
        )
        .bind(product_id)
        .bind(service_id)
        .bind(is_mandatory)
        .bind(is_default)
        .bind(display_order)
        .bind(&configuration)
        .execute(scope.executor())
        .await?
        .rows_affected();
        Ok(VerbExecutionOutcome::Affected(affected))
    }
);

op_struct!(
    ProductServiceUnlink,
    "product-service.unlink",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "product-service.unlink")?;
        let product_id = lookup_product_id(args, ctx, scope).await?;
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let affected = sqlx::query(
            r#"DELETE FROM "ob-poc".product_services
           WHERE product_id = $1 AND service_id = $2"#,
        )
        .bind(product_id)
        .bind(service_id)
        .execute(scope.executor())
        .await?
        .rows_affected();
        Ok(VerbExecutionOutcome::Affected(affected))
    }
);

op_struct!(
    ServiceResourceAddCapability,
    "service-resource.add-capability",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "service-resource.add-capability")?;
        let capability_id =
            json_extract_uuid_opt(args, ctx, "capability-id").unwrap_or_else(Uuid::new_v4);
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let resource_id = lookup_resource_id(args, ctx, scope).await?;
        let supported_options = json_arg(args, "supported-options").unwrap_or_else(|| json!({}));
        let priority = int_arg(args, "priority")?.unwrap_or(100);
        let cost_factor = decimal_arg(args, "cost-factor")?.unwrap_or_else(|| Decimal::new(10, 1));
        let performance_rating = int_arg(args, "performance-rating")?;
        let resource_config = json_arg(args, "resource-config");
        let is_active = json_extract_bool_opt(args, "is-active").unwrap_or(true);
        let is_required = json_extract_bool_opt(args, "is-required").unwrap_or(true);

        let row_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".service_resource_capabilities
           (capability_id, service_id, resource_id, supported_options, priority,
            cost_factor, performance_rating, resource_config, is_active, is_required)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           ON CONFLICT (service_id, resource_id) DO UPDATE SET
             supported_options = EXCLUDED.supported_options,
             priority = EXCLUDED.priority,
             cost_factor = EXCLUDED.cost_factor,
             performance_rating = EXCLUDED.performance_rating,
             resource_config = EXCLUDED.resource_config,
             is_active = EXCLUDED.is_active,
             is_required = EXCLUDED.is_required
           RETURNING capability_id"#,
        )
        .bind(capability_id)
        .bind(service_id)
        .bind(resource_id)
        .bind(&supported_options)
        .bind(priority)
        .bind(cost_factor)
        .bind(performance_rating)
        .bind(&resource_config)
        .bind(is_active)
        .bind(is_required)
        .fetch_one(scope.executor())
        .await?;
        ctx.bind_typed("capability", row_id, "service_resource_capability");
        Ok(VerbExecutionOutcome::Uuid(row_id))
    }
);

op_struct!(
    ServiceResourceAmendCapability,
    "service-resource.amend-capability",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "service-resource.amend-capability")?;
        let capability_id = lookup_capability_id(args, ctx, scope).await?;
        let supported_options = json_arg(args, "supported-options");
        let priority = int_arg(args, "priority")?;
        let cost_factor = decimal_arg(args, "cost-factor")?;
        let performance_rating = int_arg(args, "performance-rating")?;
        let resource_config = json_arg(args, "resource-config");
        let is_active = json_extract_bool_opt(args, "is-active");
        let is_required = json_extract_bool_opt(args, "is-required");
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".service_resource_capabilities SET
             supported_options = COALESCE($2, supported_options),
             priority = COALESCE($3, priority),
             cost_factor = COALESCE($4, cost_factor),
             performance_rating = COALESCE($5, performance_rating),
             resource_config = COALESCE($6, resource_config),
             is_active = COALESCE($7, is_active),
             is_required = COALESCE($8, is_required)
           WHERE capability_id = $1"#,
        )
        .bind(capability_id)
        .bind(&supported_options)
        .bind(priority)
        .bind(cost_factor)
        .bind(performance_rating)
        .bind(&resource_config)
        .bind(is_active)
        .bind(is_required)
        .execute(scope.executor())
        .await?
        .rows_affected();
        Ok(VerbExecutionOutcome::Affected(affected))
    }
);

op_struct!(
    ServiceResourceRemoveCapability,
    "service-resource.remove-capability",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "service-resource.remove-capability")?;
        let capability_id = lookup_capability_id(args, ctx, scope).await?;
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".service_resource_capabilities
           SET is_active = false
           WHERE capability_id = $1"#,
        )
        .bind(capability_id)
        .execute(scope.executor())
        .await?
        .rows_affected();
        Ok(VerbExecutionOutcome::Affected(affected))
    }
);

op_struct!(
    ResourceOwnerAssign,
    "resource-owner.assign",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "resource-owner.assign")?;
        let owner_system = json_extract_string(args, "owner-system")?;
        let owner_principal_fqn = json_extract_string_opt(args, "owner-principal-fqn")
            .unwrap_or_else(|| format!("resource_owner:{owner_system}"));
        let display_name =
            json_extract_string_opt(args, "display-name").unwrap_or_else(|| owner_system.clone());
        let dispatch_endpoint = json_extract_string_opt(args, "dispatch-endpoint");
        let status =
            json_extract_string_opt(args, "status").unwrap_or_else(|| "active".to_string());
        let metadata = json_arg(args, "metadata").unwrap_or_else(|| json!({}));
        let principal_capabilities =
            json_arg(args, "principal-capabilities").unwrap_or_else(|| json!(["resource_owner"]));
        let dispatch_enabled = json_extract_bool_opt(args, "dispatch-enabled").unwrap_or(true);

        sqlx::query(
            r#"INSERT INTO "ob-poc".resource_owner_principals
           (owner_principal_fqn, owner_system, display_name, dispatch_endpoint,
            status, metadata, principal_kind, principal_capabilities, dispatch_enabled,
            created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, 'resource_owner', $7, $8, NOW(), NOW())
           ON CONFLICT (owner_principal_fqn) DO UPDATE SET
             owner_system = EXCLUDED.owner_system,
             display_name = EXCLUDED.display_name,
             dispatch_endpoint = EXCLUDED.dispatch_endpoint,
             status = EXCLUDED.status,
             metadata = EXCLUDED.metadata,
             principal_capabilities = EXCLUDED.principal_capabilities,
             dispatch_enabled = EXCLUDED.dispatch_enabled,
             updated_at = NOW()"#,
        )
        .bind(&owner_principal_fqn)
        .bind(&owner_system)
        .bind(&display_name)
        .bind(&dispatch_endpoint)
        .bind(&status)
        .bind(&metadata)
        .bind(&principal_capabilities)
        .bind(dispatch_enabled)
        .execute(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "owner_principal_fqn": owner_principal_fqn,
            "owner_system": owner_system
        })))
    }
);

op_struct!(
    ResourceOwnerAmend,
    "resource-owner.amend",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "resource-owner.amend")?;
        let owner_principal_fqn = lookup_owner_principal_fqn(args, scope).await?;
        let display_name = json_extract_string_opt(args, "display-name");
        let dispatch_endpoint = json_extract_string_opt(args, "dispatch-endpoint");
        let status = json_extract_string_opt(args, "status");
        let metadata = json_arg(args, "metadata");
        let principal_capabilities = json_arg(args, "principal-capabilities");
        let dispatch_enabled = json_extract_bool_opt(args, "dispatch-enabled");
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".resource_owner_principals SET
             display_name = COALESCE($2, display_name),
             dispatch_endpoint = COALESCE($3, dispatch_endpoint),
             status = COALESCE($4, status),
             metadata = COALESCE($5, metadata),
             principal_capabilities = COALESCE($6, principal_capabilities),
             dispatch_enabled = COALESCE($7, dispatch_enabled),
             updated_at = NOW()
           WHERE owner_principal_fqn = $1"#,
        )
        .bind(&owner_principal_fqn)
        .bind(&display_name)
        .bind(&dispatch_endpoint)
        .bind(&status)
        .bind(&metadata)
        .bind(&principal_capabilities)
        .bind(dispatch_enabled)
        .execute(scope.executor())
        .await?
        .rows_affected();
        Ok(VerbExecutionOutcome::Affected(affected))
    }
);

op_struct!(
    ResourceOwnerUnassign,
    "resource-owner.unassign",
    |args, ctx, scope| {
        require_catalogue_authority(ctx, "resource-owner.unassign")?;
        let owner_principal_fqn = lookup_owner_principal_fqn(args, scope).await?;
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".resource_owner_principals
           SET status = 'retired',
               dispatch_enabled = false,
               updated_at = NOW()
           WHERE owner_principal_fqn = $1"#,
        )
        .bind(&owner_principal_fqn)
        .execute(scope.executor())
        .await?
        .rows_affected();
        Ok(VerbExecutionOutcome::Affected(affected))
    }
);
