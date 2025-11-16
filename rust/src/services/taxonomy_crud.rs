//! Taxonomy CRUD Service - Main entry point for agentic operations

use crate::taxonomy::{
    crud_ast::*, crud_operations::TaxonomyCrudOperations, dsl_parser::TaxonomyDslParser,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Instant;
use uuid::Uuid;

pub struct TaxonomyCrudService {
    operations: TaxonomyCrudOperations,
    _pool: PgPool,
}

impl TaxonomyCrudService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            operations: TaxonomyCrudOperations::new(pool.clone()),
            _pool: pool,
        }
    }

    /// Execute natural language or DSL instruction
    pub async fn execute(&self, instruction: &str) -> Result<CrudResult> {
        let start = Instant::now();

        // Parse instruction to AST
        let statement = TaxonomyDslParser::parse(instruction)?;

        // Execute based on statement type
        let result = match statement {
            TaxonomyCrudStatement::CreateProduct(create) => {
                let product_id = self.operations.create_product(create.clone()).await?;
                CrudResult {
                    success: true,
                    operation: "create_product".to_string(),
                    entity_type: "product".to_string(),
                    entity_id: Some(product_id),
                    data: Some(serde_json::json!({
                        "product_id": product_id,
                        "product_code": create.product_code,
                        "name": create.name
                    })),
                    message: "Product created successfully".to_string(),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::ReadProduct(read) => {
                let product = self.operations.read_product(read.identifier).await?;
                let success = product.is_some();
                let message = if success {
                    "Product found".to_string()
                } else {
                    "Product not found".to_string()
                };
                let entity_id = product.as_ref().map(|p| p.product_id);
                let data = product.map(|p| serde_json::to_value(p).unwrap());

                CrudResult {
                    success,
                    operation: "read_product".to_string(),
                    entity_type: "product".to_string(),
                    entity_id,
                    data,
                    message,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::UpdateProduct(update) => {
                self.operations
                    .update_product(update.identifier, update.updates)
                    .await?;
                CrudResult {
                    success: true,
                    operation: "update_product".to_string(),
                    entity_type: "product".to_string(),
                    entity_id: None,
                    data: None,
                    message: "Product updated successfully".to_string(),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::DeleteProduct(delete) => {
                self.operations
                    .delete_product(delete.identifier, delete.soft_delete)
                    .await?;
                CrudResult {
                    success: true,
                    operation: "delete_product".to_string(),
                    entity_type: "product".to_string(),
                    entity_id: None,
                    data: None,
                    message: format!("Product deleted (soft: {})", delete.soft_delete),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::CreateService(create) => {
                let service_id = self.operations.create_service(create.clone()).await?;
                CrudResult {
                    success: true,
                    operation: "create_service".to_string(),
                    entity_type: "service".to_string(),
                    entity_id: Some(service_id),
                    data: Some(serde_json::json!({
                        "service_id": service_id,
                        "service_code": create.service_code,
                        "name": create.name
                    })),
                    message: "Service created successfully".to_string(),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::DiscoverServices(discover) => {
                let services = self
                    .operations
                    .discover_services(discover.product_id, discover.include_optional)
                    .await?;
                let count = services.len();
                CrudResult {
                    success: true,
                    operation: "discover_services".to_string(),
                    entity_type: "service".to_string(),
                    entity_id: None,
                    data: Some(serde_json::to_value(services)?),
                    message: format!("Discovered {} services", count),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::CreateOnboarding(create) => {
                let onboarding_id = self.operations.create_onboarding(create.clone()).await?;
                CrudResult {
                    success: true,
                    operation: "create_onboarding".to_string(),
                    entity_type: "onboarding".to_string(),
                    entity_id: Some(onboarding_id),
                    data: Some(serde_json::json!({
                        "onboarding_id": onboarding_id,
                        "cbu_id": create.cbu_id
                    })),
                    message: "Onboarding workflow created".to_string(),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::AddProductsToOnboarding(add) => {
                let product_ids = self
                    .operations
                    .add_products_to_onboarding(add.clone())
                    .await?;
                CrudResult {
                    success: true,
                    operation: "add_products".to_string(),
                    entity_type: "onboarding".to_string(),
                    entity_id: Some(add.onboarding_id),
                    data: Some(serde_json::json!({
                        "product_onboarding_ids": product_ids,
                        "count": product_ids.len()
                    })),
                    message: format!("{} products added", product_ids.len()),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::ConfigureService(config) => {
                let config_id = self.operations.configure_service(config.clone()).await?;
                CrudResult {
                    success: true,
                    operation: "configure_service".to_string(),
                    entity_type: "service_config".to_string(),
                    entity_id: Some(config_id),
                    data: Some(serde_json::json!({
                        "config_id": config_id,
                        "service_code": config.service_code
                    })),
                    message: "Service configured successfully".to_string(),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            TaxonomyCrudStatement::QueryWorkflow(query) => {
                let workflow = self
                    .operations
                    .query_workflow(query.onboarding_id, query.include_history)
                    .await?;
                CrudResult {
                    success: true,
                    operation: "query_workflow".to_string(),
                    entity_type: "onboarding".to_string(),
                    entity_id: Some(query.onboarding_id),
                    data: Some(workflow),
                    message: "Workflow queried successfully".to_string(),
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            _ => {
                return Err(anyhow!("Operation not yet implemented"));
            }
        };

        Ok(result)
    }

    /// Batch execute multiple instructions
    pub async fn execute_batch(&self, instructions: Vec<String>) -> Vec<Result<CrudResult>> {
        let mut results = Vec::new();

        for instruction in instructions {
            results.push(self.execute(&instruction).await);
        }

        results
    }
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
