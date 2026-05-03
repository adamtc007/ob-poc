//! Service-options framework verbs.

use crate::service_options::{
    hash_canonical_json, ResolvedOptionValue, ResourceFanoutPlanner, ResourceFanoutRuleRow,
    SourceKind,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use super::SemOsVerbOp;

fn json_arg(args: &Value, name: &str) -> Option<Value> {
    args.get(name).cloned()
}

fn required_json_arg(args: &Value, name: &str) -> Result<Value> {
    json_arg(args, name).ok_or_else(|| anyhow!("Missing {} argument", name))
}

fn service_version_arg(args: &Value, ctx: &VerbExecutionContext) -> Option<Uuid> {
    json_extract_uuid_opt(args, ctx, "service-version-id")
}

async fn lookup_service_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(service_id) = json_extract_uuid_opt(args, ctx, "service-id") {
        return Ok(service_id);
    }

    let service_code = json_extract_string_opt(args, "service")
        .or_else(|| json_extract_string_opt(args, "service-code"))
        .ok_or_else(|| anyhow!("requires service-id, service, or service-code"))?;

    sqlx::query_scalar(r#"SELECT service_id FROM "ob-poc".services WHERE service_code = $1"#)
        .bind(&service_code)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Unknown service: {}", service_code))
}

async fn lookup_product_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(product_id) = json_extract_uuid_opt(args, ctx, "product-id") {
        return Ok(product_id);
    }

    let product_code = json_extract_string_opt(args, "product")
        .or_else(|| json_extract_string_opt(args, "product-code"))
        .ok_or_else(|| anyhow!("requires product-id, product, or product-code"))?;

    sqlx::query_scalar(r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1"#)
        .bind(&product_code)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Unknown product: {}", product_code))
}

async fn lookup_resource_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(resource_id) = json_extract_uuid_opt(args, ctx, "resource-id") {
        return Ok(resource_id);
    }

    let resource_code = json_extract_string_opt(args, "resource")
        .or_else(|| json_extract_string_opt(args, "resource-code"))
        .ok_or_else(|| anyhow!("requires resource-id, resource, or resource-code"))?;

    sqlx::query_scalar(
        r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = $1"#,
    )
    .bind(&resource_code)
    .fetch_optional(scope.executor())
    .await?
    .ok_or_else(|| anyhow!("Unknown service resource: {}", resource_code))
}

async fn current_service_version_id(
    service_id: Uuid,
    explicit: Option<Uuid>,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(service_version_id) = explicit {
        return Ok(service_version_id);
    }

    sqlx::query_scalar(
        r#"
        SELECT id
        FROM "ob-poc".service_versions
        WHERE service_id = $1
          AND lifecycle_status = 'published'
        ORDER BY published_at DESC NULLS LAST, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(service_id)
    .fetch_optional(scope.executor())
    .await?
    .ok_or_else(|| anyhow!("no published service version for service {service_id}"))
}

async fn lookup_option_def_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(option_id) = json_extract_uuid_opt(args, ctx, "service-option-def-id") {
        return Ok(option_id);
    }
    if let Some(option_id) = json_extract_uuid_opt(args, ctx, "option-id") {
        return Ok(option_id);
    }

    let service_id = lookup_service_id(args, ctx, scope).await?;
    let version_id = current_service_version_id(service_id, service_version_arg(args, ctx), scope)
        .await
        .context("loading service version for option lookup")?;
    let option_key = json_extract_string(args, "option-key")?;

    sqlx::query_scalar(
        r#"
        SELECT service_option_def_id
        FROM "ob-poc".service_option_defs
        WHERE service_version_id = $1
          AND option_key = $2
        "#,
    )
    .bind(version_id)
    .bind(&option_key)
    .fetch_optional(scope.executor())
    .await?
    .ok_or_else(|| anyhow!("unknown service option `{}`", option_key))
}

fn record_from_row(row: sqlx::postgres::PgRow) -> Value {
    json!({
        "service_option_def_id": row.get::<Uuid, _>("service_option_def_id"),
        "service_id": row.get::<Uuid, _>("service_id"),
        "service_version_id": row.get::<Uuid, _>("service_version_id"),
        "option_key": row.get::<String, _>("option_key"),
        "option_kind": row.get::<String, _>("option_kind"),
        "allowed_values": row.get::<Option<Value>, _>("allowed_values"),
        "default_value": row.get::<Option<Value>, _>("default_value"),
        "is_required": row.get::<bool, _>("is_required"),
        "is_fanout_driver": row.get::<bool, _>("is_fanout_driver"),
        "fanout_axis": row.get::<String, _>("fanout_axis"),
        "default_source_kind": row.get::<String, _>("default_source_kind"),
        "source_path": row.get::<Option<String>, _>("source_path"),
        "fallback_policy": row.get::<Value, _>("fallback_policy"),
        "override_policy": row.get::<String, _>("override_policy"),
        "lifecycle_status": row.get::<String, _>("lifecycle_status"),
        "description": row.get::<Option<String>, _>("description")
    })
}

/// List options by service.
pub struct ListByService;

#[async_trait]
impl SemOsVerbOp for ListByService {
    fn fqn(&self) -> &str {
        "service-option.list-by-service"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let version_id =
            current_service_version_id(service_id, service_version_arg(args, ctx), scope).await?;

        let rows = sqlx::query(
            r#"
            SELECT service_option_def_id, service_id, service_version_id, option_key,
                   option_kind, allowed_values, default_value, is_required,
                   is_fanout_driver, fanout_axis, default_source_kind, source_path,
                   fallback_policy, override_policy, lifecycle_status, description
            FROM "ob-poc".service_option_defs
            WHERE service_version_id = $1
            ORDER BY option_key
            "#,
        )
        .bind(version_id)
        .fetch_all(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::RecordSet(
            rows.into_iter().map(record_from_row).collect(),
        ))
    }
}

/// Read one service option definition.
pub struct Read;

#[async_trait]
impl SemOsVerbOp for Read {
    fn fqn(&self) -> &str {
        "service-option.read"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let option_id = lookup_option_def_id(args, ctx, scope).await?;
        let row = sqlx::query(
            r#"
            SELECT service_option_def_id, service_id, service_version_id, option_key,
                   option_kind, allowed_values, default_value, is_required,
                   is_fanout_driver, fanout_axis, default_source_kind, source_path,
                   fallback_policy, override_policy, lifecycle_status, description
            FROM "ob-poc".service_option_defs
            WHERE service_option_def_id = $1
            "#,
        )
        .bind(option_id)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(record_from_row(row)))
    }
}

/// Declare or update a service option.
pub struct DeclareOption;

#[async_trait]
impl SemOsVerbOp for DeclareOption {
    fn fqn(&self) -> &str {
        "service.declare-option"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let version_id =
            current_service_version_id(service_id, service_version_arg(args, ctx), scope).await?;
        let option_key = json_extract_string(args, "option-key")?;
        let option_kind =
            json_extract_string_opt(args, "option-kind").unwrap_or_else(|| "string".to_string());
        let source_kind = json_extract_string_opt(args, "default-source-kind")
            .or_else(|| json_extract_string_opt(args, "source-kind"))
            .unwrap_or_else(|| "manual".to_string());
        let fanout_axis =
            json_extract_string_opt(args, "fanout-axis").unwrap_or_else(|| "none".to_string());
        let is_fanout_driver = fanout_axis != "none";
        let fallback_policy = args
            .get("fallback-policy")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let lifecycle_status = json_extract_string_opt(args, "lifecycle-status")
            .unwrap_or_else(|| "active".to_string());

        let option_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".service_option_defs
                (service_id, service_version_id, option_key, option_kind,
                 allowed_values, default_value, is_required, is_fanout_driver,
                 fanout_axis, default_source_kind, source_path, fallback_policy,
                 override_policy, lifecycle_status, description)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15)
            ON CONFLICT (service_version_id, option_key)
            DO UPDATE SET option_kind = EXCLUDED.option_kind,
                          allowed_values = EXCLUDED.allowed_values,
                          default_value = EXCLUDED.default_value,
                          is_required = EXCLUDED.is_required,
                          is_fanout_driver = EXCLUDED.is_fanout_driver,
                          fanout_axis = EXCLUDED.fanout_axis,
                          default_source_kind = EXCLUDED.default_source_kind,
                          source_path = EXCLUDED.source_path,
                          fallback_policy = EXCLUDED.fallback_policy,
                          override_policy = EXCLUDED.override_policy,
                          lifecycle_status = EXCLUDED.lifecycle_status,
                          description = EXCLUDED.description,
                          updated_at = now()
            RETURNING service_option_def_id
            "#,
        )
        .bind(service_id)
        .bind(version_id)
        .bind(&option_key)
        .bind(&option_kind)
        .bind(json_arg(args, "allowed-values"))
        .bind(json_arg(args, "default-value"))
        .bind(json_extract_bool_opt(args, "is-required").unwrap_or(false))
        .bind(is_fanout_driver)
        .bind(&fanout_axis)
        .bind(&source_kind)
        .bind(json_extract_string_opt(args, "source-path"))
        .bind(fallback_policy)
        .bind(
            json_extract_string_opt(args, "override-policy")
                .unwrap_or_else(|| "allowed_with_reason".to_string()),
        )
        .bind(&lifecycle_status)
        .bind(json_extract_string_opt(args, "description"))
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(option_id))
    }
}

/// Constrain option allowed values.
pub struct ConstrainOptionValues;

#[async_trait]
impl SemOsVerbOp for ConstrainOptionValues {
    fn fqn(&self) -> &str {
        "service.constrain-option-values"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let option_id = lookup_option_def_id(args, ctx, scope).await?;
        let allowed_values = required_json_arg(args, "allowed-values")?;
        let affected = sqlx::query(
            r#"
            UPDATE "ob-poc".service_option_defs
            SET allowed_values = $2,
                updated_at = now()
            WHERE service_option_def_id = $1
            "#,
        )
        .bind(option_id)
        .bind(allowed_values)
        .execute(scope.executor())
        .await?
        .rows_affected();

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}

/// Bind an option definition to a source policy.
pub struct BindOptionSource;

#[async_trait]
impl SemOsVerbOp for BindOptionSource {
    fn fqn(&self) -> &str {
        "service.bind-option-source"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let option_id = lookup_option_def_id(args, ctx, scope).await?;
        let source_kind = json_extract_string(args, "source-kind")?;
        let source_path = json_extract_string_opt(args, "source-path");
        let fallback_policy = args
            .get("fallback-policy")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let affected = sqlx::query(
            r#"
            UPDATE "ob-poc".service_option_defs
            SET default_source_kind = $2,
                source_path = $3,
                fallback_policy = $4,
                updated_at = now()
            WHERE service_option_def_id = $1
            "#,
        )
        .bind(option_id)
        .bind(&source_kind)
        .bind(source_path)
        .bind(fallback_policy)
        .execute(scope.executor())
        .await?
        .rows_affected();

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}

/// Deprecate a service option.
pub struct DeprecateOption;

#[async_trait]
impl SemOsVerbOp for DeprecateOption {
    fn fqn(&self) -> &str {
        "service.deprecate-option"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let option_id = lookup_option_def_id(args, ctx, scope).await?;
        let affected = sqlx::query(
            r#"
            UPDATE "ob-poc".service_option_defs
            SET lifecycle_status = 'deprecated',
                updated_at = now()
            WHERE service_option_def_id = $1
            "#,
        )
        .bind(option_id)
        .execute(scope.executor())
        .await?
        .rows_affected();

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}

/// Insert a product-service option override.
pub struct OverrideOption;

#[async_trait]
impl SemOsVerbOp for OverrideOption {
    fn fqn(&self) -> &str {
        "product-service.override-option"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let product_id = lookup_product_id(args, ctx, scope).await?;
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let option_id = lookup_option_def_id(args, ctx, scope).await?;

        let override_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".product_service_option_overrides
                (product_id, service_id, service_option_def_id, default_value_override,
                 allowed_values_override, is_required_override, source_precedence_override,
                 activation_condition_ref)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING override_id
            "#,
        )
        .bind(product_id)
        .bind(service_id)
        .bind(option_id)
        .bind(json_arg(args, "default-value-override"))
        .bind(json_arg(args, "allowed-values-override"))
        .bind(json_extract_bool_opt(args, "is-required-override"))
        .bind(json_arg(args, "source-precedence-override"))
        .bind(json_extract_uuid_opt(args, ctx, "activation-condition-ref"))
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(override_id))
    }
}

/// Declare resource eligibility for an option.
pub struct DeclareEligibility;

#[async_trait]
impl SemOsVerbOp for DeclareEligibility {
    fn fqn(&self) -> &str {
        "service-resource.declare-eligibility"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let resource_id = lookup_resource_id(args, ctx, scope).await?;
        let option_id = lookup_option_def_id(args, ctx, scope).await?;
        let constraint_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".service_resource_option_constraints
                (service_id, resource_id, service_option_def_id, supported_values,
                 match_operator, priority, is_required_for_coverage, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, true)
            RETURNING constraint_id
            "#,
        )
        .bind(service_id)
        .bind(resource_id)
        .bind(option_id)
        .bind(required_json_arg(args, "supported-values")?)
        .bind(
            json_extract_string_opt(args, "match-operator")
                .unwrap_or_else(|| "intersect".to_string()),
        )
        .bind(json_extract_int_opt(args, "priority").unwrap_or(100) as i32)
        .bind(json_extract_bool_opt(args, "is-required-for-coverage").unwrap_or(false))
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(constraint_id))
    }
}

/// Declare a resource fan-out rule.
pub struct DeclareFanoutRule;

#[async_trait]
impl SemOsVerbOp for DeclareFanoutRule {
    fn fqn(&self) -> &str {
        "service-resource.declare-fanout-rule"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let resource_id = lookup_resource_id(args, ctx, scope).await?;
        let option_id = json_extract_uuid_opt(args, ctx, "service-option-def-id")
            .or_else(|| json_extract_uuid_opt(args, ctx, "option-id"));
        let fanout_rule_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".service_resource_fanout_rules
                (service_id, resource_id, service_option_def_id, fanout_axis,
                 fanout_mode, group_by_policy, shared_when_null, priority, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true)
            RETURNING fanout_rule_id
            "#,
        )
        .bind(service_id)
        .bind(resource_id)
        .bind(option_id)
        .bind(json_extract_string(args, "fanout-axis")?)
        .bind(
            json_extract_string_opt(args, "fanout-mode").unwrap_or_else(|| "per_value".to_string()),
        )
        .bind(
            args.get("group-by-policy")
                .cloned()
                .unwrap_or_else(|| json!({})),
        )
        .bind(json_extract_bool_opt(args, "shared-when-null").unwrap_or(true))
        .bind(json_extract_int_opt(args, "priority").unwrap_or(100) as i32)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(fanout_rule_id))
    }
}

/// Bind current service options for a CBU/service tuple.
pub struct BindServiceOptions;

#[async_trait]
impl SemOsVerbOp for BindServiceOptions {
    fn fqn(&self) -> &str {
        "cbu.bind-service-options"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let product_id = json_extract_uuid_opt(args, ctx, "product-id");
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let version_id =
            current_service_version_id(service_id, service_version_arg(args, ctx), scope).await?;
        let options = args.get("options").and_then(Value::as_object);

        let defs = sqlx::query(
            r#"
            SELECT service_option_def_id, option_key, default_value, is_required, default_source_kind
            FROM "ob-poc".service_option_defs
            WHERE service_version_id = $1
              AND lifecycle_status = 'active'
            ORDER BY option_key
            "#,
        )
        .bind(version_id)
        .fetch_all(scope.executor())
        .await?;

        let mut inserted = Vec::new();
        for def in defs {
            let option_id = def.get::<Uuid, _>("service_option_def_id");
            let option_key = def.get::<String, _>("option_key");
            let default_value = def.get::<Option<Value>, _>("default_value");
            let is_required = def.get::<bool, _>("is_required");
            let default_source_kind = def.get::<String, _>("default_source_kind");
            let manual_value = options.and_then(|values| values.get(&option_key)).cloned();
            let value = manual_value.or(default_value);

            if value.is_none() && !is_required {
                continue;
            }
            let value =
                value.ok_or_else(|| anyhow!("required option `{}` has no value", option_key))?;
            let source_kind = if options.and_then(|values| values.get(&option_key)).is_some() {
                "manual"
            } else {
                default_source_kind.as_str()
            };
            let value_hash = hash_canonical_json(&value);

            let binding_id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO "ob-poc".cbu_service_option_bindings
                    (cbu_id, product_id, service_id, service_version_id,
                     service_option_def_id, option_key, value, source_kind,
                     source_ref, source_version, value_hash, coherence_status, activation_run_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'clean', $12)
                ON CONFLICT (cbu_id, service_id, service_option_def_id) WHERE valid_to IS NULL
                DO UPDATE SET value = EXCLUDED.value,
                              source_kind = EXCLUDED.source_kind,
                              source_ref = EXCLUDED.source_ref,
                              source_version = EXCLUDED.source_version,
                              value_hash = EXCLUDED.value_hash,
                              coherence_status = 'clean',
                              activation_run_id = EXCLUDED.activation_run_id,
                              updated_at = now()
                RETURNING binding_id
                "#,
            )
            .bind(cbu_id)
            .bind(product_id)
            .bind(service_id)
            .bind(version_id)
            .bind(option_id)
            .bind(&option_key)
            .bind(&value)
            .bind(source_kind)
            .bind(args.get("source-ref").cloned())
            .bind(json_extract_string_opt(args, "source-version"))
            .bind(value_hash)
            .bind(json_extract_uuid_opt(args, ctx, "activation-run-id"))
            .fetch_one(scope.executor())
            .await?;

            inserted.push(json!({
                "binding_id": binding_id,
                "option_key": option_key,
                "source_kind": source_kind
            }));
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "service_id": service_id,
            "bindings": inserted
        })))
    }
}

/// Override one CBU option binding.
pub struct OverrideOptionBinding;

#[async_trait]
impl SemOsVerbOp for OverrideOptionBinding {
    fn fqn(&self) -> &str {
        "cbu.override-option-binding"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let option_id = lookup_option_def_id(args, ctx, scope).await?;
        let value = required_json_arg(args, "value")?;
        let value_hash = hash_canonical_json(&value);

        sqlx::query(
            r#"
            UPDATE "ob-poc".cbu_service_option_bindings
            SET valid_to = now(),
                coherence_status = 'stale',
                updated_at = now()
            WHERE cbu_id = $1
              AND service_id = $2
              AND service_option_def_id = $3
              AND valid_to IS NULL
            "#,
        )
        .bind(cbu_id)
        .bind(service_id)
        .bind(option_id)
        .execute(scope.executor())
        .await?;

        let (version_id, option_key): (Uuid, String) = sqlx::query_as(
            r#"
            SELECT service_version_id, option_key
            FROM "ob-poc".service_option_defs
            WHERE service_option_def_id = $1
            "#,
        )
        .bind(option_id)
        .fetch_one(scope.executor())
        .await?;

        let binding_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".cbu_service_option_bindings
                (cbu_id, product_id, service_id, service_version_id,
                 service_option_def_id, option_key, value, source_kind, source_ref,
                 source_version, value_hash, coherence_status, activation_run_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'manual', $8, $9, $10, 'clean', $11)
            RETURNING binding_id
            "#,
        )
        .bind(cbu_id)
        .bind(json_extract_uuid_opt(args, ctx, "product-id"))
        .bind(service_id)
        .bind(version_id)
        .bind(option_id)
        .bind(&option_key)
        .bind(value)
        .bind(args.get("source-ref").cloned())
        .bind(json_extract_string_opt(args, "source-version"))
        .bind(value_hash)
        .bind(json_extract_uuid_opt(args, ctx, "activation-run-id"))
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(binding_id))
    }
}

/// Validate required option coverage.
pub struct ValidateOptionCoverage;

#[async_trait]
impl SemOsVerbOp for ValidateOptionCoverage {
    fn fqn(&self) -> &str {
        "cbu.validate-option-coverage"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let version_id =
            current_service_version_id(service_id, service_version_arg(args, ctx), scope).await?;

        let gaps: Vec<Value> = sqlx::query(
            r#"
            SELECT d.option_key
            FROM "ob-poc".service_option_defs d
            LEFT JOIN "ob-poc".cbu_service_option_bindings b
              ON b.cbu_id = $1
             AND b.service_id = d.service_id
             AND b.service_option_def_id = d.service_option_def_id
             AND b.valid_to IS NULL
            WHERE d.service_version_id = $2
              AND d.lifecycle_status = 'active'
              AND d.is_required
              AND (b.binding_id IS NULL OR b.value = 'null'::jsonb)
            ORDER BY d.option_key
            "#,
        )
        .bind(cbu_id)
        .bind(version_id)
        .fetch_all(scope.executor())
        .await?
        .into_iter()
        .map(|row| {
            let option_key = row.get::<String, _>("option_key");
            json!({
                "level": 1,
                "code": "missing_required_option_binding",
                "option_key": option_key
            })
        })
        .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "service_id": service_id,
            "status": if gaps.is_empty() { "clean" } else { "gapped" },
            "gaps": gaps
        })))
    }
}

/// Dirty-flag current option bindings.
pub struct DirtyFlagBindings;

#[async_trait]
impl SemOsVerbOp for DirtyFlagBindings {
    fn fqn(&self) -> &str {
        "cbu.dirty-flag-bindings"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let service_id = json_extract_uuid_opt(args, ctx, "service-id");
        let source_kind = json_extract_string_opt(args, "source-kind");
        let affected = sqlx::query(
            r#"
            UPDATE "ob-poc".cbu_service_option_bindings
            SET coherence_status = 'dirty',
                updated_at = now()
            WHERE cbu_id = $1
              AND valid_to IS NULL
              AND ($2::uuid IS NULL OR service_id = $2)
              AND ($3::text IS NULL OR source_kind = $3)
            "#,
        )
        .bind(cbu_id)
        .bind(service_id)
        .bind(source_kind)
        .execute(scope.executor())
        .await?
        .rows_affected();

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}

/// Mark dirty bindings clean after recomputation.
pub struct RecomputeBindings;

#[async_trait]
impl SemOsVerbOp for RecomputeBindings {
    fn fqn(&self) -> &str {
        "cbu.recompute-bindings"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let service_id = json_extract_uuid_opt(args, ctx, "service-id");
        let affected = sqlx::query(
            r#"
            UPDATE "ob-poc".cbu_service_option_bindings
            SET coherence_status = 'clean',
                updated_at = now()
            WHERE cbu_id = $1
              AND valid_to IS NULL
              AND coherence_status = 'dirty'
              AND ($2::uuid IS NULL OR service_id = $2)
            "#,
        )
        .bind(cbu_id)
        .bind(service_id)
        .execute(scope.executor())
        .await?
        .rows_affected();

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}

/// Compute resource fan-out plan from current bindings and active rules.
pub struct ComputeResourceFanout;

#[async_trait]
impl SemOsVerbOp for ComputeResourceFanout {
    fn fqn(&self) -> &str {
        "cbu.compute-resource-fanout"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let service_id = lookup_service_id(args, ctx, scope).await?;

        let rules = sqlx::query_as::<_, ResourceFanoutRuleRow>(
            r#"
            SELECT fanout_rule_id, service_id, resource_id, service_option_def_id,
                   fanout_axis, fanout_mode, group_by_policy, shared_when_null,
                   priority, is_active
            FROM "ob-poc".service_resource_fanout_rules
            WHERE service_id = $1
              AND is_active
            ORDER BY priority, resource_id
            "#,
        )
        .bind(service_id)
        .fetch_all(scope.executor())
        .await?;

        let bindings = sqlx::query(
            r#"
            SELECT service_option_def_id, option_key, value, source_kind, source_ref,
                   source_version, value_hash
            FROM "ob-poc".cbu_service_option_bindings
            WHERE cbu_id = $1
              AND service_id = $2
              AND valid_to IS NULL
            "#,
        )
        .bind(cbu_id)
        .bind(service_id)
        .fetch_all(scope.executor())
        .await?
        .into_iter()
        .map(|row| ResolvedOptionValue {
            service_option_def_id: row.get("service_option_def_id"),
            option_key: row.get("option_key"),
            value: row.get("value"),
            source_kind: SourceKind::try_from(row.get::<String, _>("source_kind").as_str())
                .unwrap_or(SourceKind::Manual),
            source_ref: row.get("source_ref"),
            source_version: row.get("source_version"),
            value_hash: row.get("value_hash"),
        })
        .collect::<Vec<_>>();

        let planned = ResourceFanoutPlanner::new().plan(&rules, &bindings)?;
        let records = planned
            .into_iter()
            .map(|item| {
                json!({
                    "service_id": item.service_id,
                    "resource_id": item.resource_id,
                    "fanout_axis": item.fanout_axis.as_db_str(),
                    "fanout_value": item.fanout_value
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(records))
    }
}
