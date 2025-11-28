//! Product Services Resources - DSL Generation Service
//!
//! Generates DSL that flows through DSL v2 executor instead of direct database operations.
//! This maintains the DSL-as-State architecture.

use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

/// Service for generating DSL from product/service/lifecycle-resource operations
pub struct ProductServicesResourcesDsl {
    // No database pool - we generate DSL, not execute DB operations
}

impl ProductServicesResourcesDsl {
    pub fn new() -> Self {
        Self {}
    }

    /// Generate DSL for creating a product
    pub fn generate_product_create_dsl(
        &self,
        name: &str,
        product_code: &str,
        description: Option<&str>,
        category: Option<&str>,
    ) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(
            r#":name "{}" :product-code "{}" :description "{}" :product-category "{}" product.create"#,
            name,
            product_code,
            description.unwrap_or(""),
            category.unwrap_or("")
        );
        DslGenerationResult {
            dsl,
            operation: "create_product".to_string(),
            entity_type: "Product".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for reading a product
    pub fn generate_product_read_dsl(&self, product_id: Uuid) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(r#":product-id "{}" product.read"#, product_id);
        DslGenerationResult {
            dsl,
            operation: "read_product".to_string(),
            entity_type: "Product".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for updating a product
    pub fn generate_product_update_dsl(
        &self,
        product_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> DslGenerationResult {
        let start = Instant::now();
        let mut pairs = vec![format!(r#":product-id "{}""#, product_id)];
        if let Some(n) = name {
            pairs.push(format!(r#":name "{}""#, n));
        }
        if let Some(d) = description {
            pairs.push(format!(r#":description "{}""#, d));
        }
        let dsl = format!("{} product.update", pairs.join(" "));
        DslGenerationResult {
            dsl,
            operation: "update_product".to_string(),
            entity_type: "Product".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for deleting a product
    pub fn generate_product_delete_dsl(&self, product_id: Uuid) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(r#":product-id "{}" product.delete"#, product_id);
        DslGenerationResult {
            dsl,
            operation: "delete_product".to_string(),
            entity_type: "Product".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for creating a service
    pub fn generate_service_create_dsl(
        &self,
        name: &str,
        service_code: &str,
        description: Option<&str>,
        category: Option<&str>,
    ) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(
            r#":name "{}" :service-code "{}" :description "{}" :service-category "{}" service.create"#,
            name,
            service_code,
            description.unwrap_or(""),
            category.unwrap_or("")
        );
        DslGenerationResult {
            dsl,
            operation: "create_service".to_string(),
            entity_type: "Service".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for reading a service
    pub fn generate_service_read_dsl(&self, service_id: Uuid) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(r#":service-id "{}" service.read"#, service_id);
        DslGenerationResult {
            dsl,
            operation: "read_service".to_string(),
            entity_type: "Service".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for deleting a service
    pub fn generate_service_delete_dsl(&self, service_id: Uuid) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(r#":service-id "{}" service.delete"#, service_id);
        DslGenerationResult {
            dsl,
            operation: "delete_service".to_string(),
            entity_type: "Service".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for creating a lifecycle resource
    pub fn generate_lifecycle_resource_create_dsl(
        &self,
        name: &str,
        owner: &str,
        resource_type: Option<&str>,
        description: Option<&str>,
    ) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(
            r#":name "{}" :owner "{}" :resource-type "{}" :description "{}" lifecycle-resource.create"#,
            name,
            owner,
            resource_type.unwrap_or("generic"),
            description.unwrap_or("")
        );
        DslGenerationResult {
            dsl,
            operation: "create_lifecycle_resource".to_string(),
            entity_type: "LifecycleResource".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for reading a lifecycle resource
    pub fn generate_lifecycle_resource_read_dsl(&self, resource_id: Uuid) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(r#":resource-id "{}" lifecycle-resource.read"#, resource_id);
        DslGenerationResult {
            dsl,
            operation: "read_lifecycle_resource".to_string(),
            entity_type: "LifecycleResource".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for deleting a lifecycle resource
    pub fn generate_lifecycle_resource_delete_dsl(&self, resource_id: Uuid) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(
            r#":resource-id "{}" lifecycle-resource.delete"#,
            resource_id
        );
        DslGenerationResult {
            dsl,
            operation: "delete_lifecycle_resource".to_string(),
            entity_type: "LifecycleResource".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for linking service to product
    pub fn generate_service_link_product_dsl(
        &self,
        service_id: Uuid,
        product_id: Uuid,
    ) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(
            r#":service-id "{}" :product-id "{}" service.link-product"#,
            service_id, product_id
        );
        DslGenerationResult {
            dsl,
            operation: "link_service_product".to_string(),
            entity_type: "Service".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Generate DSL for linking lifecycle resource to service
    pub fn generate_resource_link_service_dsl(
        &self,
        resource_id: Uuid,
        service_id: Uuid,
    ) -> DslGenerationResult {
        let start = Instant::now();
        let dsl = format!(
            r#":resource-id "{}" :service-id "{}" lifecycle-resource.link-service"#,
            resource_id, service_id
        );
        DslGenerationResult {
            dsl,
            operation: "link_resource_service".to_string(),
            entity_type: "LifecycleResource".to_string(),
            generation_time_ms: start.elapsed().as_millis() as u64,
        }
    }
}

impl Default for ProductServicesResourcesDsl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslGenerationResult {
    pub dsl: String,
    pub operation: String,
    pub entity_type: String,
    pub generation_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CrudResult {
    pub success: bool,
    pub operation: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub data: Option<serde_json::Value>,
    pub message: String,
    pub execution_time_ms: u64,
}
