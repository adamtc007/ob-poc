//! Service-resource domain verbs (6 plugin verbs) — SemOS-side
//! YAML-first re-implementation of the plugin subset of
//! `rust/config/verbs/service-resource.yaml`.
//!
//! Resource instance lifecycle: provision (create) → set-attr →
//! activate → suspend / decommission. `validate-attrs` is a
//! diagnostic gate checking `resource_attribute_requirements`.
//!
//! Attribute identity resolution routes through the existing
//! `AttributeIdentityService` service trait (same as
//! observation_ops).

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::service_traits::AttributeIdentityService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── service-resource.provision ────────────────────────────────────────────────

pub struct Provision;

#[async_trait]
impl SemOsVerbOp for Provision {
    fn fqn(&self) -> &str {
        "service-resource.provision"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let resource_type_code = json_extract_string(args, "resource-type")?;
        let instance_url = json_extract_string_opt(args, "instance-url").unwrap_or_else(|| {
            format!(
                "urn:ob-poc:{}:{}:{}",
                cbu_id,
                resource_type_code.to_lowercase().replace('_', "-"),
                Uuid::new_v4()
            )
        });
        let instance_identifier = json_extract_string_opt(args, "instance-id");
        let instance_name = json_extract_string_opt(args, "instance-name");
        let product_id = json_extract_uuid_opt(args, ctx, "product-id");
        let mut service_id = json_extract_uuid_opt(args, ctx, "service-id");
        let depends_on_refs: Vec<Uuid> = args
            .get("depends-on")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        item.as_str().and_then(|s| {
                            if let Some(sym) = s.strip_prefix('@') {
                                ctx.resolve(sym)
                            } else {
                                Uuid::parse_str(s).ok()
                            }
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let resource_type_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = $1"#,
        )
        .bind(&resource_type_code)
        .fetch_optional(scope.executor())
        .await?;
        let resource_type_id = resource_type_id
            .ok_or_else(|| anyhow!("Unknown resource type: {}", resource_type_code))?;

        if service_id.is_none() {
            let services: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT service_id FROM "ob-poc".service_resource_capabilities
                   WHERE resource_id = $1 AND is_active = true
                   ORDER BY priority ASC
                   LIMIT 1"#,
            )
            .bind(resource_type_id)
            .fetch_all(scope.executor())
            .await?;
            service_id = services.into_iter().next();
        }

        let instance_id = Uuid::new_v4();
        let row: (Uuid,) = sqlx::query_as(
            r#"WITH ins AS (
                INSERT INTO "ob-poc".cbu_resource_instances
                (instance_id, cbu_id, product_id, service_id, resource_type_id,
                 instance_url, instance_identifier, instance_name, status)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'PENDING')
                ON CONFLICT (instance_url) DO NOTHING
                RETURNING instance_id
            )
            SELECT instance_id FROM ins
            UNION ALL
            SELECT instance_id FROM "ob-poc".cbu_resource_instances
            WHERE instance_url = $6
            AND NOT EXISTS (SELECT 1 FROM ins)
            LIMIT 1"#,
        )
        .bind(instance_id)
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(resource_type_id)
        .bind(&instance_url)
        .bind(&instance_identifier)
        .bind(&instance_name)
        .fetch_one(scope.executor())
        .await?;

        let result_id = row.0;
        for dep in depends_on_refs {
            sqlx::query(
                r#"INSERT INTO "ob-poc".resource_instance_dependencies
                   (instance_id, depends_on_instance_id)
                   VALUES ($1, $2)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(result_id)
            .bind(dep)
            .execute(scope.executor())
            .await?;
        }

        ctx.bind("instance", result_id);
        Ok(VerbExecutionOutcome::Uuid(result_id))
    }
}

// ── service-resource.set-attr ─────────────────────────────────────────────────

pub struct SetAttr;

#[async_trait]
impl SemOsVerbOp for SetAttr {
    fn fqn(&self) -> &str {
        "service-resource.set-attr"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instance_id = json_extract_uuid(args, ctx, "instance-id")?;
        let attr_name = json_extract_string(args, "attr")?;
        let value = json_extract_string(args, "value")?;
        let state =
            json_extract_string_opt(args, "state").unwrap_or_else(|| "proposed".to_string());

        let attribute_id = ctx
            .service::<dyn AttributeIdentityService>()?
            .resolve_runtime_uuid(&attr_name)
            .await?
            .ok_or_else(|| anyhow!("Unknown attribute: {}", attr_name))?;

        let value_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".resource_instance_attributes
               (value_id, instance_id, attribute_id, value_text, state, observed_at)
               VALUES ($1, $2, $3, $4, $5, NOW())
               ON CONFLICT (instance_id, attribute_id) DO UPDATE SET
                   value_text = EXCLUDED.value_text,
                   state = EXCLUDED.state,
                   observed_at = NOW()"#,
        )
        .bind(value_id)
        .bind(instance_id)
        .bind(attribute_id)
        .bind(&value)
        .bind(&state)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(value_id))
    }
}

// ── service-resource.activate ─────────────────────────────────────────────────

pub struct Activate;

#[async_trait]
impl SemOsVerbOp for Activate {
    fn fqn(&self) -> &str {
        "service-resource.activate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instance_id = json_extract_uuid(args, ctx, "instance-id")?;

        let instance_row: Option<(Option<Uuid>,)> = sqlx::query_as(
            r#"SELECT resource_type_id FROM "ob-poc".cbu_resource_instances WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .fetch_optional(scope.executor())
        .await?;
        let instance_row =
            instance_row.ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        if let Some(resource_type_id) = instance_row.0 {
            let required_attrs: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT attribute_id FROM "ob-poc".resource_attribute_requirements
                   WHERE resource_id = $1 AND is_mandatory = true"#,
            )
            .bind(resource_type_id)
            .fetch_all(scope.executor())
            .await?;

            let set_attrs: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT attribute_id FROM "ob-poc".resource_instance_attributes
                   WHERE instance_id = $1"#,
            )
            .bind(instance_id)
            .fetch_all(scope.executor())
            .await?;

            let missing: Vec<Uuid> = required_attrs
                .into_iter()
                .filter(|a| !set_attrs.contains(a))
                .collect();

            if !missing.is_empty() {
                let missing_names: Vec<String> = sqlx::query_scalar(
                    r#"SELECT display_name FROM "ob-poc".attribute_registry WHERE uuid = ANY($1)"#,
                )
                .bind(&missing)
                .fetch_all(scope.executor())
                .await?;

                return Err(anyhow!(
                    "Cannot activate: missing required attributes: {}",
                    missing_names.join(", ")
                ));
            }
        }

        sqlx::query(
            r#"UPDATE "ob-poc".cbu_resource_instances
               SET status = 'ACTIVE', activated_at = NOW(), updated_at = NOW()
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Void)
    }
}

// ── service-resource.suspend ──────────────────────────────────────────────────

pub struct Suspend;

#[async_trait]
impl SemOsVerbOp for Suspend {
    fn fqn(&self) -> &str {
        "service-resource.suspend"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instance_id = json_extract_uuid(args, ctx, "instance-id")?;
        sqlx::query(
            r#"UPDATE "ob-poc".cbu_resource_instances
               SET status = 'SUSPENDED', updated_at = NOW()
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .execute(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::Void)
    }
}

// ── service-resource.decommission ─────────────────────────────────────────────

pub struct Decommission;

#[async_trait]
impl SemOsVerbOp for Decommission {
    fn fqn(&self) -> &str {
        "service-resource.decommission"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instance_id = json_extract_uuid(args, ctx, "instance-id")?;
        sqlx::query(
            r#"UPDATE "ob-poc".cbu_resource_instances
               SET status = 'DECOMMISSIONED', decommissioned_at = NOW(), updated_at = NOW()
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .execute(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::Void)
    }
}

// ── service-resource.validate-attrs ───────────────────────────────────────────

pub struct ValidateAttrs;

#[async_trait]
impl SemOsVerbOp for ValidateAttrs {
    fn fqn(&self) -> &str {
        "service-resource.validate-attrs"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instance_id = json_extract_uuid(args, ctx, "instance-id")?;

        let resource_type_id: Option<Option<Uuid>> = sqlx::query_scalar(
            r#"SELECT resource_type_id FROM "ob-poc".cbu_resource_instances WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .fetch_optional(scope.executor())
        .await?;

        let resource_type_id = match resource_type_id.and_then(|r| r) {
            Some(id) => id,
            None => {
                return Ok(VerbExecutionOutcome::Record(json!({
                    "valid": true,
                    "missing": [],
                    "message": "No resource type defined, skipping validation",
                })));
            }
        };

        let required_attrs: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT rar.attribute_id, ar.display_name
               FROM "ob-poc".resource_attribute_requirements rar
               JOIN "ob-poc".attribute_registry ar ON rar.attribute_id = ar.uuid
               WHERE rar.resource_id = $1 AND rar.is_mandatory = true
               ORDER BY rar.display_order"#,
        )
        .bind(resource_type_id)
        .fetch_all(scope.executor())
        .await?;

        let set_attrs: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT attribute_id FROM "ob-poc".resource_instance_attributes
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .fetch_all(scope.executor())
        .await?;

        let missing: Vec<String> = required_attrs
            .iter()
            .filter(|(id, _)| !set_attrs.contains(id))
            .map(|(_, name)| name.clone())
            .collect();

        let valid = missing.is_empty();
        Ok(VerbExecutionOutcome::Record(json!({
            "valid": valid,
            "missing": missing,
            "message": if valid {
                "All required attributes are set".to_string()
            } else {
                format!("Missing {} required attribute(s)", missing.len())
            },
        })))
    }
}
