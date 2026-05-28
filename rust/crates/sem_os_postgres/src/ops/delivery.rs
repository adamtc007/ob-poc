//! Service delivery map verbs.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime::{
    json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use serde_json::{json, Value};
use uuid::Uuid;

use super::SemOsVerbOp;

async fn lookup_product_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(id) = json_extract_uuid_opt(args, ctx, "product-id") {
        return Ok(id);
    }
    let product_code = json_extract_string_opt(args, "product")
        .ok_or_else(|| anyhow!("delivery.record requires product-id or product"))?;
    sqlx::query_scalar(r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1"#)
        .bind(&product_code)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Unknown product: {}", product_code))
}

async fn lookup_service_id(
    args: &Value,
    ctx: &VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<Uuid> {
    if let Some(id) = json_extract_uuid_opt(args, ctx, "service-id") {
        return Ok(id);
    }
    let service_code = json_extract_string_opt(args, "service")
        .ok_or_else(|| anyhow!("delivery.record requires service-id or service"))?;
    sqlx::query_scalar(r#"SELECT service_id FROM "ob-poc".services WHERE service_code = $1"#)
        .bind(&service_code)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Unknown service: {}", service_code))
}

/// Record or update a service-delivery row for a CBU/product/service tuple.
///
/// # Examples
///
/// ```rust,ignore
/// let op = sem_os_postgres::ops::delivery::Record;
/// assert_eq!(op.fqn(), "delivery.record");
/// ```
pub struct Record;

#[async_trait]
impl SemOsVerbOp for Record {
    fn fqn(&self) -> &str {
        "delivery.record"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let product_id = lookup_product_id(args, ctx, scope).await?;
        let service_id = lookup_service_id(args, ctx, scope).await?;
        let instance_id = json_extract_uuid_opt(args, ctx, "instance-id");
        let config = args.get("config").cloned().unwrap_or_else(|| json!({}));

        let delivery_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".service_delivery_map
                (cbu_id, product_id, service_id, instance_id, service_config)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (cbu_id, product_id, service_id) DO UPDATE SET
                instance_id = COALESCE(EXCLUDED.instance_id, service_delivery_map.instance_id),
                service_config = COALESCE(EXCLUDED.service_config, service_delivery_map.service_config),
                updated_at = NOW()
            RETURNING delivery_id
            "#,
        )
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(instance_id)
        .bind(config)
        .fetch_one(scope.executor())
        .await?;

        dsl_runtime::emit_pending_state_advance(
            ctx,
            cbu_id,
            "delivery:pending",
            "delivery/service",
            "delivery.record",
        );

        Ok(VerbExecutionOutcome::Uuid(delivery_id))
    }
}
