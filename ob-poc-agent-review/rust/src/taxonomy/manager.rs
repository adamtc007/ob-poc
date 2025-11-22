//! DSL Manager for Taxonomy Operations
//!
//! Orchestrates taxonomy operations and generates DSL fragments

use super::operations::{DslOperation, DslResult};
use crate::database::TaxonomyRepository;
use crate::models::taxonomy::*;
use anyhow::{anyhow, Context, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub struct TaxonomyDslManager {
    repo: Arc<TaxonomyRepository>,
}

impl TaxonomyDslManager {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: Arc::new(TaxonomyRepository::new(pool)),
        }
    }

    pub async fn execute(&self, operation: DslOperation) -> Result<DslResult> {
        match operation {
            DslOperation::CreateOnboarding {
                cbu_id,
                initiated_by,
            } => self.create_onboarding(cbu_id, &initiated_by).await,
            DslOperation::AddProducts {
                request_id,
                product_codes,
            } => self.add_products(request_id, product_codes).await,
            DslOperation::DiscoverServices {
                request_id,
                product_id,
            } => self.discover_services(request_id, product_id).await,
            DslOperation::ConfigureService {
                request_id,
                service_code,
                options,
            } => {
                self.configure_service(request_id, &service_code, options)
                    .await
            }
            DslOperation::AllocateResources {
                request_id,
                service_id,
            } => self.allocate_resources(request_id, service_id).await,
            DslOperation::FinalizeOnboarding { request_id } => {
                self.finalize_onboarding(request_id).await
            }
            DslOperation::GetStatus { request_id } => self.get_status(request_id).await,
        }
    }

    async fn create_onboarding(&self, cbu_id: Uuid, initiated_by: &str) -> Result<DslResult> {
        let request = self
            .repo
            .create_onboarding_request(cbu_id, initiated_by)
            .await
            .context("Failed to create onboarding request")?;

        let dsl_fragment = format!(
            r#"(onboarding.create
  :request-id "{}"
  :cbu-id "{}"
  :initiated-by "{}")"#,
            request.request_id, cbu_id, initiated_by
        );

        Ok(DslResult::success(format!(
            "Onboarding request created: {}",
            request.request_id
        ))
        .with_data(serde_json::to_value(&request)?)
        .with_next_operations(vec!["AddProducts".to_string()])
        .with_dsl_fragment(dsl_fragment)
        .with_state("draft".to_string()))
    }

    async fn add_products(
        &self,
        request_id: Uuid,
        product_codes: Vec<String>,
    ) -> Result<DslResult> {
        // Validate request exists
        let request = self
            .repo
            .get_onboarding_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Request not found"))?;

        if request.request_state != "draft" && request.request_state != "products_selected" {
            return Err(anyhow!(
                "Invalid state for adding products: {}",
                request.request_state
            ));
        }

        let mut added_products = vec![];

        for code in &product_codes {
            let product = self
                .repo
                .get_product_by_code(code)
                .await?
                .ok_or_else(|| anyhow!("Product not found: {}", code))?;

            self.repo
                .add_product_to_request(request_id, product.product_id)
                .await?;
            added_products.push(product);
        }

        let dsl_fragment = format!(
            r#"(products.add
  :request-id "{}"
  :products [{}])"#,
            request_id,
            product_codes
                .iter()
                .map(|c| format!(r#""{}""#, c))
                .collect::<Vec<_>>()
                .join(" ")
        );

        Ok(DslResult::success(format!(
            "Added {} products to request",
            added_products.len()
        ))
        .with_data(serde_json::to_value(&added_products)?)
        .with_next_operations(vec!["DiscoverServices".to_string()])
        .with_dsl_fragment(dsl_fragment)
        .with_state("products_selected".to_string()))
    }

    async fn discover_services(&self, request_id: Uuid, product_id: Uuid) -> Result<DslResult> {
        // Get all services for this product
        let services = self.repo.discover_services_for_product(product_id).await?;

        // For each service, get its options
        let mut services_with_options = vec![];

        for service in services {
            let service_with_opts = self
                .repo
                .get_service_with_options(service.service_id)
                .await?;
            services_with_options.push(service_with_opts);
        }

        let dsl_fragment = format!(
            r#"(services.discover
  :request-id "{}"
  :product-id "{}")"#,
            request_id, product_id
        );

        // Update request state
        self.repo
            .update_request_state(request_id, "services_discovered")
            .await?;

        Ok(DslResult::success(format!(
            "Discovered {} services with options",
            services_with_options.len()
        ))
        .with_data(serde_json::to_value(&services_with_options)?)
        .with_next_operations(vec!["ConfigureService".to_string()])
        .with_dsl_fragment(dsl_fragment)
        .with_state("services_discovered".to_string()))
    }

    async fn configure_service(
        &self,
        request_id: Uuid,
        service_code: &str,
        options: HashMap<String, serde_json::Value>,
    ) -> Result<DslResult> {
        // Get service by code
        let service = self
            .repo
            .get_service_by_code(service_code)
            .await?
            .ok_or_else(|| anyhow!("Service not found: {}", service_code))?;

        // Validate options
        let service_options = self.repo.get_service_options(service.service_id).await?;

        for opt_def in &service_options {
            if opt_def.is_required.unwrap_or(false) && !options.contains_key(&opt_def.option_key) {
                return Err(anyhow!("Required option missing: {}", opt_def.option_key));
            }

            // Additional validation based on option_type
            if let Some(value) = options.get(&opt_def.option_key) {
                self.validate_option_value(opt_def, value)?;
            }
        }

        // Store configuration
        let options_json = serde_json::to_value(&options)?;
        self.repo
            .configure_service(request_id, service.service_id, &options_json)
            .await?;

        let dsl_fragment = format!(
            r#"(services.configure
  :request-id "{}"
  :service "{}"
  :options {})"#,
            request_id,
            service_code,
            serde_json::to_string(&options)?
        );

        Ok(
            DslResult::success(format!("Service {} configured", service_code))
                .with_data(options_json)
                .with_next_operations(vec![
                    "ConfigureService".to_string(),
                    "AllocateResources".to_string(),
                ])
                .with_dsl_fragment(dsl_fragment)
                .with_state("services_configured".to_string()),
        )
    }

    async fn allocate_resources(&self, request_id: Uuid, service_id: Uuid) -> Result<DslResult> {
        // Get service configuration
        #[derive(sqlx::FromRow)]
        struct ConfigRow {
            option_selections: serde_json::Value,
        }

        let config = sqlx::query_as::<_, ConfigRow>(
            r#"
            SELECT option_selections
            FROM "ob-poc".onboarding_service_configs
            WHERE request_id = $1 AND service_id = $2
            "#,
        )
        .bind(request_id)
        .bind(service_id)
        .fetch_optional(self.repo.pool())
        .await?
        .ok_or_else(|| anyhow!("Service configuration not found"))?;

        // Find capable resources
        let resources = self
            .repo
            .find_capable_resources(service_id, &config.option_selections)
            .await?;

        if resources.is_empty() {
            return Err(anyhow!("No resources available for this configuration"));
        }

        // Build allocations
        let mut allocations = vec![];
        let mut all_attributes: Vec<Uuid> = vec![];

        for resource in &resources {
            let attributes = self
                .repo
                .get_resource_attributes(resource.resource_id)
                .await?;
            all_attributes.extend(&attributes);

            allocations.push(ResourceAllocationRequest {
                service_id,
                resource_id: resource.resource_id,
                handles_options: serde_json::from_value(config.option_selections.clone())?,
                required_attributes: attributes,
            });
        }

        self.repo
            .allocate_resources(request_id, allocations)
            .await?;

        let dsl_fragment = format!(
            r#"(resources.allocate
  :request-id "{}"
  :service-id "{}"
  :resources [{}])"#,
            request_id,
            service_id,
            resources
                .iter()
                .map(|r| format!(r#""{}""#, r.resource_code.as_deref().unwrap_or("unknown")))
                .collect::<Vec<_>>()
                .join(" ")
        );

        Ok(
            DslResult::success(format!("Allocated {} resources", resources.len()))
                .with_data(serde_json::json!({
                    "resources": resources,
                    "required_attributes": all_attributes,
                }))
                .with_next_operations(vec!["FinalizeOnboarding".to_string()])
                .with_dsl_fragment(dsl_fragment)
                .with_state("resources_allocated".to_string()),
        )
    }

    async fn finalize_onboarding(&self, request_id: Uuid) -> Result<DslResult> {
        // Generate complete DSL
        let complete_dsl = self.generate_complete_dsl(request_id).await?;

        // Update request
        self.repo
            .complete_onboarding(request_id, &complete_dsl)
            .await?;

        Ok(DslResult::success("Onboarding complete")
            .with_data(serde_json::json!({
                "request_id": request_id,
                "complete_dsl": complete_dsl,
            }))
            .with_next_operations(vec![])
            .with_dsl_fragment(complete_dsl.clone())
            .with_state("complete".to_string()))
    }

    async fn get_status(&self, request_id: Uuid) -> Result<DslResult> {
        let request = self
            .repo
            .get_onboarding_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Request not found"))?;

        let next_ops = match request.request_state.as_str() {
            "draft" => vec!["AddProducts"],
            "products_selected" => vec!["DiscoverServices"],
            "services_discovered" => vec!["ConfigureService"],
            "services_configured" => vec!["AllocateResources"],
            "resources_allocated" => vec!["FinalizeOnboarding"],
            _ => vec![],
        };

        Ok(
            DslResult::success(format!("Request is in {} state", request.request_state))
                .with_data(serde_json::to_value(&request)?)
                .with_next_operations(next_ops.into_iter().map(String::from).collect())
                .with_state(request.request_state),
        )
    }

    async fn generate_complete_dsl(&self, request_id: Uuid) -> Result<String> {
        // Fetch all data
        let request = self
            .repo
            .get_onboarding_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Request not found"))?;

        let products = self.repo.get_request_products(request_id).await?;

        // Build complete DSL
        let mut dsl = format!(
            r#";; Onboarding Request: {}
;; CBU: {}
;; Created: {}

(onboarding-workflow
  :request-id "{}"
  :cbu-id "{}"

  ;; Products
"#,
            request_id,
            request.cbu_id,
            request
                .created_at
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
            request_id,
            request.cbu_id
        );

        for product in products {
            dsl.push_str(&format!(
                r#"  (product "{}")"#,
                product.product_code.as_deref().unwrap_or("unknown")
            ));
            dsl.push('\n');
        }

        dsl.push_str(")\n");

        Ok(dsl)
    }

    fn validate_option_value(
        &self,
        def: &ServiceOptionDefinition,
        value: &serde_json::Value,
    ) -> Result<()> {
        let option_type = OptionType::from(def.option_type.clone());

        match option_type {
            OptionType::Boolean => {
                if !value.is_boolean() {
                    return Err(anyhow!("Option {} must be boolean", def.option_key));
                }
            }
            OptionType::Numeric => {
                if !value.is_number() {
                    return Err(anyhow!("Option {} must be numeric", def.option_key));
                }
            }
            OptionType::SingleSelect => {
                if !value.is_string() {
                    return Err(anyhow!(
                        "Option {} must be a single string value",
                        def.option_key
                    ));
                }
            }
            OptionType::MultiSelect => {
                if !value.is_array() {
                    return Err(anyhow!("Option {} must be an array", def.option_key));
                }
            }
            OptionType::Text => {
                if !value.is_string() {
                    return Err(anyhow!("Option {} must be text", def.option_key));
                }
            }
        }

        Ok(())
    }
}
