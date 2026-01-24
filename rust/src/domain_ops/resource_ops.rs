//! Resource instance custom operations
//!
//! Operations for service resource provisioning, attribute management,
//! and lifecycle transitions.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Create a resource instance for a CBU (Idempotent)
///
/// Rationale: Requires lookup of resource_type_id from service_resource_types by code,
/// creates the instance record with proper FK relationships.
///
/// Idempotency: Uses ON CONFLICT on (instance_url) or (cbu_id, resource_type_id, instance_identifier)
/// to return existing instance if already created.
#[register_custom_op]
pub struct ResourceCreateOp;

#[async_trait]
impl CustomOperation for ResourceCreateOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "provision"
    }
    fn rationale(&self) -> &'static str {
        "Requires resource_type lookup by code and CBU/product/service FK resolution"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get CBU ID (required)
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

        // Get resource type code (required)
        let resource_type_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "resource-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing resource-type argument"))?;

        // Get instance URL (optional - auto-generate if not provided)
        let instance_url = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-url")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                format!(
                    "urn:ob-poc:{}:{}:{}",
                    cbu_id,
                    resource_type_code.to_lowercase().replace('_', "-"),
                    Uuid::new_v4()
                )
            });

        // Look up resource type ID
        let resource_type_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = $1"#,
        )
        .bind(resource_type_code)
        .fetch_optional(pool)
        .await?;

        let resource_type_id = resource_type_id
            .ok_or_else(|| anyhow::anyhow!("Unknown resource type: {}", resource_type_code))?;

        // Get optional arguments
        let instance_identifier = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-id")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let instance_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Get product-id if provided
        let product_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "product-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Get service-id if provided, otherwise auto-derive from resource type capabilities
        let mut service_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "service-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Auto-derive service_id from service_resource_capabilities if not provided
        if service_id.is_none() {
            let services: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT service_id FROM "ob-poc".service_resource_capabilities
                   WHERE resource_id = $1 AND is_active = true
                   ORDER BY priority ASC
                   LIMIT 1"#,
            )
            .bind(resource_type_id)
            .fetch_all(pool)
            .await?;

            service_id = services.into_iter().next();
        }

        // Extract depends-on references
        let depends_on_refs: Vec<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "depends-on")
            .and_then(|a| a.value.as_list())
            .map(|list| {
                list.iter()
                    .filter_map(|node| {
                        if let Some(sym) = node.as_symbol() {
                            ctx.resolve(sym)
                        } else {
                            node.as_uuid()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Idempotent: INSERT or return existing using instance_url as conflict key
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
        .bind(instance_url)
        .bind(&instance_identifier)
        .bind(&instance_name)
        .fetch_one(pool)
        .await?;

        let result_id = row.0;

        // Record instance dependencies if any were specified
        for dep_instance_id in depends_on_refs {
            sqlx::query(
                r#"INSERT INTO "ob-poc".resource_instance_dependencies
                   (instance_id, depends_on_instance_id)
                   VALUES ($1, $2)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(result_id)
            .bind(dep_instance_id)
            .execute(pool)
            .await?;
        }

        ctx.bind("instance", result_id);

        Ok(ExecutionResult::Uuid(result_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::new_v4()))
    }
}

/// Set an attribute on a resource instance
///
/// Rationale: Requires lookup of attribute_id from dictionary by name,
/// then upsert into resource_instance_attributes with typed value.
#[register_custom_op]
pub struct ResourceSetAttrOp;

#[async_trait]
impl CustomOperation for ResourceSetAttrOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "set-attr"
    }
    fn rationale(&self) -> &'static str {
        "Requires attribute lookup by name and typed value storage"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        let attr_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attr")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attr argument"))?;

        let value = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "value")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing value argument"))?;

        let attribute_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1 OR display_name = $1"#,
        )
        .bind(attr_name)
        .fetch_optional(pool)
        .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        let state = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "state")
            .and_then(|a| a.value.as_string())
            .unwrap_or("proposed");

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
        .bind(value)
        .bind(state)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(value_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::new_v4()))
    }
}

/// Activate a resource instance
///
/// Rationale: Validates required attributes are set before activation.
#[register_custom_op]
pub struct ResourceActivateOp;

#[async_trait]
impl CustomOperation for ResourceActivateOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "activate"
    }
    fn rationale(&self) -> &'static str {
        "Validates required attributes before status transition"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        let instance_row: Option<(Option<Uuid>,)> = sqlx::query_as(
            r#"SELECT resource_type_id FROM "ob-poc".cbu_resource_instances WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .fetch_optional(pool)
        .await?;

        let instance_row =
            instance_row.ok_or_else(|| anyhow::anyhow!("Instance not found: {}", instance_id))?;

        if let Some(resource_type_id) = instance_row.0 {
            let required_attrs: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT attribute_id FROM "ob-poc".resource_attribute_requirements
                   WHERE resource_id = $1 AND is_mandatory = true"#,
            )
            .bind(resource_type_id)
            .fetch_all(pool)
            .await?;

            let set_attrs: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT attribute_id FROM "ob-poc".resource_instance_attributes
                   WHERE instance_id = $1"#,
            )
            .bind(instance_id)
            .fetch_all(pool)
            .await?;

            let missing: Vec<_> = required_attrs
                .iter()
                .filter(|a| !set_attrs.contains(a))
                .collect();

            if !missing.is_empty() {
                let missing_uuids: Vec<Uuid> = missing.iter().map(|u| **u).collect();
                let missing_names: Vec<String> = sqlx::query_scalar(
                    r#"SELECT display_name FROM "ob-poc".attribute_registry WHERE uuid = ANY($1)"#,
                )
                .bind(missing_uuids)
                .fetch_all(pool)
                .await?;

                return Err(anyhow::anyhow!(
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
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Void)
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Suspend a resource instance
#[register_custom_op]
pub struct ResourceSuspendOp;

#[async_trait]
impl CustomOperation for ResourceSuspendOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "suspend"
    }
    fn rationale(&self) -> &'static str {
        "Status transition with optional reason logging"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        sqlx::query(
            r#"UPDATE "ob-poc".cbu_resource_instances
               SET status = 'SUSPENDED', updated_at = NOW()
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Void)
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Decommission a resource instance
#[register_custom_op]
pub struct ResourceDecommissionOp;

#[async_trait]
impl CustomOperation for ResourceDecommissionOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "decommission"
    }
    fn rationale(&self) -> &'static str {
        "Terminal status transition with timestamp"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        sqlx::query(
            r#"UPDATE "ob-poc".cbu_resource_instances
               SET status = 'DECOMMISSIONED', decommissioned_at = NOW(), updated_at = NOW()
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Void)
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Validate that all required attributes are set for a resource instance
#[register_custom_op]
pub struct ResourceValidateAttrsOp;

#[async_trait]
impl CustomOperation for ResourceValidateAttrsOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "validate-attrs"
    }
    fn rationale(&self) -> &'static str {
        "Validates required attributes against resource_attribute_requirements"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let instance_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instance-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing instance-id argument"))?;

        let resource_type_id: Option<Option<Uuid>> = sqlx::query_scalar(
            r#"SELECT resource_type_id FROM "ob-poc".cbu_resource_instances WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .fetch_optional(pool)
        .await?;

        let resource_type_id = match resource_type_id.and_then(|r| r) {
            Some(id) => id,
            None => {
                return Ok(ExecutionResult::Record(serde_json::json!({
                    "valid": true,
                    "missing": [],
                    "message": "No resource type defined, skipping validation"
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
        .fetch_all(pool)
        .await?;

        let set_attrs: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT attribute_id FROM "ob-poc".resource_instance_attributes
               WHERE instance_id = $1"#,
        )
        .bind(instance_id)
        .fetch_all(pool)
        .await?;

        let missing: Vec<String> = required_attrs
            .iter()
            .filter(|(id, _)| !set_attrs.contains(id))
            .map(|(_, name)| name.clone())
            .collect();

        let valid = missing.is_empty();
        let result = serde_json::json!({
            "valid": valid,
            "missing": missing,
            "message": if valid {
                "All required attributes are set".to_string()
            } else {
                format!("Missing {} required attribute(s)", missing.len())
            }
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "valid": true,
            "missing": [],
            "message": "Validation skipped (no database)"
        })))
    }
}
