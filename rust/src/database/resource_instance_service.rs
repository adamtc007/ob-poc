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
pub(crate) struct ResourceInstanceRow {
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
pub(crate) struct ResourceInstanceAttributeRow {
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
pub(crate) struct ServiceDeliveryRow {
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
pub(crate) struct NewResourceInstance {
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
pub(crate) struct SetInstanceAttribute {
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
pub(crate) struct ResourceInstanceService {
    pool: PgPool,
}

impl ResourceInstanceService {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    // -------------------------------------------------------------------------
    // Resource Instance CRUD
    // -------------------------------------------------------------------------


    pub(crate) async fn get_instance(&self, instance_id: Uuid) -> Result<Option<ResourceInstanceRow>> {
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



    pub(crate) async fn update_status(&self, instance_id: Uuid, status: &str) -> Result<bool> {
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


    pub(crate) async fn get_instance_attributes(
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

    pub(crate) async fn get_attribute_value(
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




    // -------------------------------------------------------------------------
    // Validation Helpers
    // -------------------------------------------------------------------------

    pub(crate) async fn get_required_attributes(&self, resource_type_id: Uuid) -> Result<Vec<Uuid>> {
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


    // -------------------------------------------------------------------------
    // Lookup Helpers
    // -------------------------------------------------------------------------




}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_resource_instance_builder() {
        let input = NewResourceInstance {
            cbu_id: Uuid::new_v4(),
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
            instance_id: Uuid::new_v4(),
            attribute_id: Uuid::new_v4(),
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
