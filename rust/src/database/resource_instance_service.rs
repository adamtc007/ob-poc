//! Resource Instance Service - CRUD for CBU Resource Instances
//!
//! Manages the actual delivered artifacts (accounts, connections, platform access)
//! that are created when a CBU is onboarded to a product.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use std::collections::HashSet;
use tracing::info;
use uuid::Uuid;

// =============================================================================
// Row Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ResourceInstanceRow {
    pub instance_id: Uuid,
    pub cbu_id: Uuid,
    pub product_id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    pub resource_type_id: Option<Uuid>,
    pub instance_url: String,
    pub instance_identifier: Option<String>,
    pub instance_name: Option<String>,
    pub instance_config: Option<JsonValue>,
    pub status: String,
    pub requested_at: Option<DateTime<Utc>>,
    pub provisioned_at: Option<DateTime<Utc>>,
    pub activated_at: Option<DateTime<Utc>>,
    pub decommissioned_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ResourceInstanceAttributeRow {
    pub value_id: Uuid,
    pub instance_id: Uuid,
    pub attribute_id: Uuid,
    pub value_text: Option<String>,
    pub value_number: Option<Decimal>,
    pub value_boolean: Option<bool>,
    pub value_date: Option<NaiveDate>,
    pub value_timestamp: Option<DateTime<Utc>>,
    pub value_json: Option<JsonValue>,
    pub state: Option<String>,
    pub source: Option<JsonValue>,
    pub observed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceDeliveryRow {
    pub delivery_id: Uuid,
    pub cbu_id: Uuid,
    pub product_id: Uuid,
    pub service_id: Uuid,
    pub instance_id: Option<Uuid>,
    pub service_config: Option<JsonValue>,
    pub delivery_status: String,
    pub requested_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub failure_reason: Option<String>,
}

// =============================================================================
// Input Types
// =============================================================================

#[derive(Debug, Clone)]
pub struct NewResourceInstance {
    pub cbu_id: Uuid,
    pub product_id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    pub resource_type_id: Option<Uuid>,
    pub instance_url: String,
    pub instance_identifier: Option<String>,
    pub instance_name: Option<String>,
    pub instance_config: Option<JsonValue>,
}

#[derive(Debug, Clone)]
pub struct SetInstanceAttribute {
    pub instance_id: Uuid,
    pub attribute_id: Uuid,
    pub value_text: Option<String>,
    pub value_number: Option<Decimal>,
    pub value_boolean: Option<bool>,
    pub value_date: Option<NaiveDate>,
    pub value_json: Option<JsonValue>,
    pub state: Option<String>,
    pub source: Option<JsonValue>,
}

// =============================================================================
// Service Implementation
// =============================================================================

#[derive(Clone, Debug)]
pub struct ResourceInstanceService {
    pool: PgPool,
}

impl ResourceInstanceService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // -------------------------------------------------------------------------
    // Resource Instance CRUD
    // -------------------------------------------------------------------------

    pub async fn create_instance(&self, input: &NewResourceInstance) -> Result<Uuid> {
        let instance_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_resource_instances
                (instance_id, cbu_id, product_id, service_id, resource_type_id,
                 instance_url, instance_identifier, instance_name, instance_config, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'PENDING')
        "#,
        )
        .bind(instance_id)
        .bind(input.cbu_id)
        .bind(input.product_id)
        .bind(input.service_id)
        .bind(input.resource_type_id)
        .bind(&input.instance_url)
        .bind(&input.instance_identifier)
        .bind(&input.instance_name)
        .bind(&input.instance_config)
        .execute(&self.pool)
        .await
        .context("Failed to create resource instance")?;

        info!(
            "Created resource instance {} for CBU {}",
            instance_id, input.cbu_id
        );
        Ok(instance_id)
    }

    pub async fn get_instance(&self, instance_id: Uuid) -> Result<Option<ResourceInstanceRow>> {
        sqlx::query_as::<_, ResourceInstanceRow>(
            r#"
            SELECT instance_id, cbu_id, product_id, service_id, resource_type_id,
                   instance_url, instance_identifier, instance_name, instance_config,
                   status, requested_at, provisioned_at, activated_at, decommissioned_at,
                   created_at, updated_at
            FROM "ob-poc".cbu_resource_instances
            WHERE instance_id = $1
        "#,
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get resource instance")
    }

    pub async fn get_instance_by_url(&self, url: &str) -> Result<Option<ResourceInstanceRow>> {
        sqlx::query_as::<_, ResourceInstanceRow>(
            r#"
            SELECT instance_id, cbu_id, product_id, service_id, resource_type_id,
                   instance_url, instance_identifier, instance_name, instance_config,
                   status, requested_at, provisioned_at, activated_at, decommissioned_at,
                   created_at, updated_at
            FROM "ob-poc".cbu_resource_instances
            WHERE instance_url = $1
        "#,
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get resource instance by URL")
    }

    pub async fn list_instances_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<ResourceInstanceRow>> {
        sqlx::query_as::<_, ResourceInstanceRow>(
            r#"
            SELECT instance_id, cbu_id, product_id, service_id, resource_type_id,
                   instance_url, instance_identifier, instance_name, instance_config,
                   status, requested_at, provisioned_at, activated_at, decommissioned_at,
                   created_at, updated_at
            FROM "ob-poc".cbu_resource_instances
            WHERE cbu_id = $1
            ORDER BY created_at DESC
        "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list resource instances for CBU")
    }

    pub async fn update_status(&self, instance_id: Uuid, status: &str) -> Result<bool> {
        let result = match status {
            "PROVISIONING" => {
                sqlx::query(
                    r#"UPDATE "ob-poc".cbu_resource_instances
                       SET status = $1, provisioned_at = NOW(), updated_at = NOW()
                       WHERE instance_id = $2"#,
                )
                .bind(status)
                .bind(instance_id)
                .execute(&self.pool)
                .await
            }
            "ACTIVE" => {
                sqlx::query(
                    r#"UPDATE "ob-poc".cbu_resource_instances
                       SET status = $1, activated_at = NOW(), updated_at = NOW()
                       WHERE instance_id = $2"#,
                )
                .bind(status)
                .bind(instance_id)
                .execute(&self.pool)
                .await
            }
            "DECOMMISSIONED" => {
                sqlx::query(
                    r#"UPDATE "ob-poc".cbu_resource_instances
                       SET status = $1, decommissioned_at = NOW(), updated_at = NOW()
                       WHERE instance_id = $2"#,
                )
                .bind(status)
                .bind(instance_id)
                .execute(&self.pool)
                .await
            }
            _ => {
                sqlx::query(
                    r#"UPDATE "ob-poc".cbu_resource_instances
                       SET status = $1, updated_at = NOW()
                       WHERE instance_id = $2"#,
                )
                .bind(status)
                .bind(instance_id)
                .execute(&self.pool)
                .await
            }
        }
        .context("Failed to update instance status")?;

        if result.rows_affected() > 0 {
            info!("Updated instance {} status to {}", instance_id, status);
        }
        Ok(result.rows_affected() > 0)
    }

    // -------------------------------------------------------------------------
    // Instance Attributes
    // -------------------------------------------------------------------------

    pub async fn set_attribute(&self, input: &SetInstanceAttribute) -> Result<Uuid> {
        let value_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".resource_instance_attributes
                (value_id, instance_id, attribute_id, value_text, value_number,
                 value_boolean, value_date, value_json, state, source, observed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())
            ON CONFLICT (instance_id, attribute_id) DO UPDATE SET
                value_text = EXCLUDED.value_text,
                value_number = EXCLUDED.value_number,
                value_boolean = EXCLUDED.value_boolean,
                value_date = EXCLUDED.value_date,
                value_json = EXCLUDED.value_json,
                state = EXCLUDED.state,
                source = EXCLUDED.source,
                observed_at = NOW()
        "#,
        )
        .bind(value_id)
        .bind(input.instance_id)
        .bind(input.attribute_id)
        .bind(&input.value_text)
        .bind(input.value_number)
        .bind(input.value_boolean)
        .bind(input.value_date)
        .bind(&input.value_json)
        .bind(input.state.as_deref().unwrap_or("proposed"))
        .bind(&input.source)
        .execute(&self.pool)
        .await
        .context("Failed to set instance attribute")?;

        info!(
            "Set attribute {} on instance {}",
            input.attribute_id, input.instance_id
        );
        Ok(value_id)
    }

    pub async fn get_instance_attributes(
        &self,
        instance_id: Uuid,
    ) -> Result<Vec<ResourceInstanceAttributeRow>> {
        sqlx::query_as::<_, ResourceInstanceAttributeRow>(
            r#"
            SELECT value_id, instance_id, attribute_id, value_text, value_number,
                   value_boolean, value_date, value_timestamp, value_json,
                   state, source, observed_at
            FROM "ob-poc".resource_instance_attributes
            WHERE instance_id = $1
            ORDER BY attribute_id
        "#,
        )
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get instance attributes")
    }

    pub async fn get_attribute_value(
        &self,
        instance_id: Uuid,
        attribute_id: Uuid,
    ) -> Result<Option<ResourceInstanceAttributeRow>> {
        sqlx::query_as::<_, ResourceInstanceAttributeRow>(
            r#"
            SELECT value_id, instance_id, attribute_id, value_text, value_number,
                   value_boolean, value_date, value_timestamp, value_json,
                   state, source, observed_at
            FROM "ob-poc".resource_instance_attributes
            WHERE instance_id = $1 AND attribute_id = $2
        "#,
        )
        .bind(instance_id)
        .bind(attribute_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get attribute value")
    }

    // -------------------------------------------------------------------------
    // Service Delivery Map
    // -------------------------------------------------------------------------

    pub async fn record_delivery(
        &self,
        cbu_id: Uuid,
        product_id: Uuid,
        service_id: Uuid,
        instance_id: Option<Uuid>,
        service_config: Option<JsonValue>,
    ) -> Result<Uuid> {
        let delivery_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".service_delivery_map
                (delivery_id, cbu_id, product_id, service_id, instance_id,
                 service_config, delivery_status)
            VALUES ($1, $2, $3, $4, $5, $6, 'PENDING')
            ON CONFLICT (cbu_id, product_id, service_id) DO UPDATE SET
                instance_id = EXCLUDED.instance_id,
                service_config = EXCLUDED.service_config,
                updated_at = NOW()
        "#,
        )
        .bind(delivery_id)
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(instance_id)
        .bind(&service_config)
        .execute(&self.pool)
        .await
        .context("Failed to record service delivery")?;

        info!(
            "Recorded delivery {} for CBU {} / product {} / service {}",
            delivery_id, cbu_id, product_id, service_id
        );
        Ok(delivery_id)
    }

    pub async fn update_delivery_status(
        &self,
        cbu_id: Uuid,
        product_id: Uuid,
        service_id: Uuid,
        status: &str,
        failure_reason: Option<&str>,
    ) -> Result<bool> {
        let result = match status {
            "IN_PROGRESS" => {
                sqlx::query(
                    r#"UPDATE "ob-poc".service_delivery_map
                       SET delivery_status = $1, started_at = NOW(), failure_reason = $2, updated_at = NOW()
                       WHERE cbu_id = $3 AND product_id = $4 AND service_id = $5"#,
                )
                .bind(status)
                .bind(failure_reason)
                .bind(cbu_id)
                .bind(product_id)
                .bind(service_id)
                .execute(&self.pool)
                .await
            }
            "DELIVERED" => {
                sqlx::query(
                    r#"UPDATE "ob-poc".service_delivery_map
                       SET delivery_status = $1, delivered_at = NOW(), failure_reason = $2, updated_at = NOW()
                       WHERE cbu_id = $3 AND product_id = $4 AND service_id = $5"#,
                )
                .bind(status)
                .bind(failure_reason)
                .bind(cbu_id)
                .bind(product_id)
                .bind(service_id)
                .execute(&self.pool)
                .await
            }
            "FAILED" => {
                sqlx::query(
                    r#"UPDATE "ob-poc".service_delivery_map
                       SET delivery_status = $1, failed_at = NOW(), failure_reason = $2, updated_at = NOW()
                       WHERE cbu_id = $3 AND product_id = $4 AND service_id = $5"#,
                )
                .bind(status)
                .bind(failure_reason)
                .bind(cbu_id)
                .bind(product_id)
                .bind(service_id)
                .execute(&self.pool)
                .await
            }
            _ => {
                sqlx::query(
                    r#"UPDATE "ob-poc".service_delivery_map
                       SET delivery_status = $1, failure_reason = $2, updated_at = NOW()
                       WHERE cbu_id = $3 AND product_id = $4 AND service_id = $5"#,
                )
                .bind(status)
                .bind(failure_reason)
                .bind(cbu_id)
                .bind(product_id)
                .bind(service_id)
                .execute(&self.pool)
                .await
            }
        }
        .context("Failed to update delivery status")?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_cbu_deliveries(&self, cbu_id: Uuid) -> Result<Vec<ServiceDeliveryRow>> {
        sqlx::query_as::<_, ServiceDeliveryRow>(
            r#"
            SELECT delivery_id, cbu_id, product_id, service_id, instance_id,
                   service_config, delivery_status, requested_at, started_at,
                   delivered_at, failed_at, failure_reason
            FROM "ob-poc".service_delivery_map
            WHERE cbu_id = $1
            ORDER BY requested_at DESC
        "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CBU deliveries")
    }

    // -------------------------------------------------------------------------
    // Validation Helpers
    // -------------------------------------------------------------------------

    pub async fn get_required_attributes(&self, resource_type_id: Uuid) -> Result<Vec<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".resource_attribute_requirements
            WHERE resource_id = $1 AND is_mandatory = true
            ORDER BY display_order
        "#,
        )
        .bind(resource_type_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get required attributes for resource type")
    }

    pub async fn validate_instance_attributes(&self, instance_id: Uuid) -> Result<Vec<String>> {
        // Get instance with resource type
        let instance = self.get_instance(instance_id).await?;
        let Some(instance) = instance else {
            return Ok(vec!["Instance not found".to_string()]);
        };

        let Some(resource_type_id) = instance.resource_type_id else {
            return Ok(vec![]); // No resource type = no validation
        };

        // Get required attributes
        let required = self.get_required_attributes(resource_type_id).await?;

        // Get set attributes
        let set_attrs = self.get_instance_attributes(instance_id).await?;
        let set_ids: HashSet<Uuid> = set_attrs.iter().map(|a| a.attribute_id).collect();

        // Find missing
        let mut missing = Vec::new();
        for attr_id in required {
            if !set_ids.contains(&attr_id) {
                missing.push(format!("Missing required attribute: {}", attr_id));
            }
        }

        Ok(missing)
    }

    // -------------------------------------------------------------------------
    // Lookup Helpers
    // -------------------------------------------------------------------------

    pub async fn lookup_resource_type_by_code(&self, code: &str) -> Result<Option<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = $1"#,
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to lookup resource type by code")
    }

    pub async fn lookup_attribute_by_name(&self, name: &str) -> Result<Option<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE name = $1"#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to lookup attribute by name")
    }

    pub async fn lookup_product_by_code(&self, code: &str) -> Result<Option<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1"#,
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to lookup product by code")
    }

    pub async fn lookup_service_by_code(&self, code: &str) -> Result<Option<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT service_id FROM "ob-poc".services WHERE service_code = $1"#,
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to lookup service by code")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_resource_instance_builder() {
        let input = NewResourceInstance {
            cbu_id: Uuid::now_v7(),
            product_id: None,
            service_id: None,
            resource_type_id: None,
            instance_url: "https://test.example.com/account/123".to_string(),
            instance_identifier: Some("ACC-123".to_string()),
            instance_name: Some("Test Account".to_string()),
            instance_config: None,
        };

        assert_eq!(input.instance_url, "https://test.example.com/account/123");
        assert_eq!(input.instance_identifier, Some("ACC-123".to_string()));
    }

    #[test]
    fn test_set_instance_attribute_builder() {
        let input = SetInstanceAttribute {
            instance_id: Uuid::now_v7(),
            attribute_id: Uuid::now_v7(),
            value_text: Some("DTC-789456".to_string()),
            value_number: None,
            value_boolean: None,
            value_date: None,
            value_json: None,
            state: Some("confirmed".to_string()),
            source: None,
        };

        assert_eq!(input.value_text, Some("DTC-789456".to_string()));
        assert_eq!(input.state, Some("confirmed".to_string()));
    }
}
