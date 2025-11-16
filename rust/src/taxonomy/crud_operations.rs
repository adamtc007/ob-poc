//! CRUD operations for taxonomy entities

use super::crud_ast::*;
use crate::database::TaxonomyRepository;
use crate::models::taxonomy::*;
use anyhow::{anyhow, Result};
use chrono::Utc;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub struct TaxonomyCrudOperations {
    _repo: Arc<TaxonomyRepository>,
    pool: PgPool,
}

impl TaxonomyCrudOperations {
    pub fn new(pool: PgPool) -> Self {
        Self {
            _repo: Arc::new(TaxonomyRepository::new(pool.clone())),
            pool,
        }
    }

    // Product CRUD operations

    pub async fn create_product(&self, create: CreateProduct) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;

        let product_id = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".products
            (product_id, product_code, name, product_category,
             regulatory_framework, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, true, $6, $7)
            "#,
            product_id,
            &create.product_code,
            &create.name,
            create.category,
            create.regulatory_framework,
            Utc::now(),
            Utc::now()
        )
        .execute(&mut *tx)
        .await?;

        self.log_crud_operation(
            &mut tx,
            CrudLogEntry {
                operation_type: "CREATE",
                entity_type: "product",
                entity_id: Some(product_id),
                natural_language_input: &format!(
                    "create product {} ({})",
                    create.name, create.product_code
                ),
                success: true,
                error_message: None,
            },
        )
        .await?;

        tx.commit().await?;
        Ok(product_id)
    }

    pub async fn read_product(&self, identifier: ProductIdentifier) -> Result<Option<Product>> {
        match identifier {
            ProductIdentifier::Id(id) => sqlx::query_as!(
                Product,
                r#"SELECT * FROM "ob-poc".products WHERE product_id = $1"#,
                id
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into),
            ProductIdentifier::Code(code) => sqlx::query_as!(
                Product,
                r#"SELECT * FROM "ob-poc".products WHERE product_code = $1"#,
                code
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into),
        }
    }

    pub async fn update_product(
        &self,
        identifier: ProductIdentifier,
        _updates: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let product = self
            .read_product(identifier)
            .await?
            .ok_or_else(|| anyhow!("Product not found"))?;

        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            r#"UPDATE "ob-poc".products SET updated_at = $1 WHERE product_id = $2"#,
            Utc::now(),
            product.product_id
        )
        .execute(&mut *tx)
        .await?;

        self.log_crud_operation(
            &mut tx,
            CrudLogEntry {
                operation_type: "UPDATE",
                entity_type: "product",
                entity_id: Some(product.product_id),
                natural_language_input: &format!("update product {}", product.product_id),
                success: true,
                error_message: None,
            },
        )
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_product(&self, identifier: ProductIdentifier, soft: bool) -> Result<()> {
        let product = self
            .read_product(identifier)
            .await?
            .ok_or_else(|| anyhow!("Product not found"))?;

        let mut tx = self.pool.begin().await?;

        if soft {
            sqlx::query!(
                r#"UPDATE "ob-poc".products SET is_active = false WHERE product_id = $1"#,
                product.product_id
            )
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query!(
                r#"DELETE FROM "ob-poc".products WHERE product_id = $1"#,
                product.product_id
            )
            .execute(&mut *tx)
            .await?;
        }

        self.log_crud_operation(
            &mut tx,
            CrudLogEntry {
                operation_type: "DELETE",
                entity_type: "product",
                entity_id: Some(product.product_id),
                natural_language_input: &format!("delete product {} (soft: {})", product.product_id, soft),
                success: true,
                error_message: None,
            },
        )
        .await?;

        tx.commit().await?;
        Ok(())
    }

    // Service operations

    pub async fn create_service(&self, create: CreateService) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;

        let service_id = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".services
            (service_id, service_code, name, service_category,
             sla_definition, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, true, $6, $7)
            "#,
            service_id,
            &create.service_code,
            &create.name,
            create.category,
            create.sla_definition,
            Utc::now(),
            Utc::now()
        )
        .execute(&mut *tx)
        .await?;

        // Create service options
        for option in &create.options {
            let option_def_id = Uuid::new_v4();

            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".service_option_definitions
                (option_def_id, service_id, option_key, option_label,
                 option_type, is_required, validation_rules)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                option_def_id,
                service_id,
                &option.option_key,
                &option.option_key,
                &option.option_type,
                option.is_required,
                option.validation_rules
            )
            .execute(&mut *tx)
            .await?;

            // Create choices for select types
            for choice in &option.choices {
                let choice_id = Uuid::new_v4();
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".service_option_choices
                    (choice_id, option_def_id, choice_value, choice_label, is_default)
                    VALUES ($1, $2, $3, $4, false)
                    "#,
                    choice_id,
                    option_def_id,
                    choice,
                    choice
                )
                .execute(&mut *tx)
                .await?;
            }
        }

        self.log_crud_operation(
            &mut tx,
            CrudLogEntry {
                operation_type: "CREATE",
                entity_type: "service",
                entity_id: Some(service_id),
                natural_language_input: &format!("create service {} ({})", create.name, create.service_code),
                success: true,
                error_message: None,
            },
        )
        .await?;

        tx.commit().await?;
        Ok(service_id)
    }

    pub async fn discover_services(
        &self,
        product_id: Uuid,
        _include_optional: bool,
    ) -> Result<Vec<Service>> {
        // Get services linked to product via product_services table
        sqlx::query_as!(
            Service,
            r#"
            SELECT s.*
            FROM "ob-poc".services s
            JOIN "ob-poc".product_services ps ON s.service_id = ps.service_id
            WHERE ps.product_id = $1 AND s.is_active = true
            "#,
            product_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    // Onboarding operations

    pub async fn create_onboarding(&self, create: CreateOnboarding) -> Result<Uuid> {
        let request_id = Uuid::new_v4();

        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".onboarding_requests
            (request_id, cbu_id, request_state, phase_metadata, created_at, updated_at)
            VALUES ($1, $2, 'initiated', $3, $4, $5)
            "#,
            request_id,
            create.cbu_id,
            create
                .metadata
                .as_ref()
                .map(|m| serde_json::to_value(m).unwrap()),
            Utc::now(),
            Utc::now()
        )
        .execute(&mut *tx)
        .await?;

        self.log_crud_operation(
            &mut tx,
            CrudLogEntry {
                operation_type: "CREATE",
                entity_type: "onboarding",
                entity_id: Some(request_id),
                natural_language_input: &format!("create onboarding for CBU {}", create.cbu_id),
                success: true,
                error_message: None,
            },
        )
        .await?;

        tx.commit().await?;
        Ok(request_id)
    }

    pub async fn add_products_to_onboarding(
        &self,
        add: AddProductsToOnboarding,
    ) -> Result<Vec<Uuid>> {
        let mut tx = self.pool.begin().await?;
        let mut product_onboarding_ids = Vec::new();

        for product_code in &add.product_codes {
            // Get product by code
            let product = self
                .read_product(ProductIdentifier::Code(product_code.clone()))
                .await?
                .ok_or_else(|| anyhow!("Product {} not found", product_code))?;

            // Link product to onboarding
            let op_id = Uuid::new_v4();
            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".onboarding_products
                (onboarding_product_id, request_id, product_id, selected_at)
                VALUES ($1, $2, $3, $4)
                "#,
                op_id,
                add.onboarding_id,
                product.product_id,
                Utc::now()
            )
            .execute(&mut *tx)
            .await?;

            product_onboarding_ids.push(op_id);
        }

        self.log_crud_operation(
            &mut tx,
            CrudLogEntry {
                operation_type: "UPDATE",
                entity_type: "onboarding",
                entity_id: Some(add.onboarding_id),
                natural_language_input: &format!("add products {:?} to onboarding", add.product_codes),
                success: true,
                error_message: None,
            },
        )
        .await?;

        tx.commit().await?;
        Ok(product_onboarding_ids)
    }

    pub async fn configure_service(&self, config: ConfigureService) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;

        // Get service by code
        let service = sqlx::query_as!(
            Service,
            r#"SELECT * FROM "ob-poc".services WHERE service_code = $1"#,
            config.service_code
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("Service {} not found", config.service_code))?;

        // Create service configuration
        let config_id = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".onboarding_service_configs
            (config_id, request_id, service_id, option_selections, configured_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            config_id,
            config.onboarding_id,
            service.service_id,
            serde_json::to_value(&config.options)?,
            Utc::now()
        )
        .execute(&mut *tx)
        .await?;

        self.log_crud_operation(
            &mut tx,
            CrudLogEntry {
                operation_type: "UPDATE",
                entity_type: "service_config",
                entity_id: Some(config_id),
                natural_language_input: &format!("configure service {} for onboarding", config.service_code),
                success: true,
                error_message: None,
            },
        )
        .await?;

        tx.commit().await?;
        Ok(config_id)
    }

    pub async fn query_workflow(
        &self,
        onboarding_id: Uuid,
        _include_history: bool,
    ) -> Result<serde_json::Value> {
        // Get onboarding details
        let onboarding = sqlx::query!(
            r#"
            SELECT request_id, cbu_id, request_state, phase_metadata, created_at, updated_at
            FROM "ob-poc".onboarding_requests
            WHERE request_id = $1
            "#,
            onboarding_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("Onboarding not found"))?;

        // Get associated products
        let products = sqlx::query!(
            r#"
            SELECT p.product_code, p.name
            FROM "ob-poc".onboarding_products op
            JOIN "ob-poc".products p ON op.product_id = p.product_id
            WHERE op.request_id = $1
            "#,
            onboarding_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Get configured services
        let services = sqlx::query!(
            r#"
            SELECT s.service_code, s.name, osc.option_selections
            FROM "ob-poc".onboarding_service_configs osc
            JOIN "ob-poc".services s ON osc.service_id = s.service_id
            WHERE osc.request_id = $1
            "#,
            onboarding_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(serde_json::json!({
            "onboarding_id": onboarding.request_id,
            "cbu_id": onboarding.cbu_id,
            "state": onboarding.request_state,
            "created_at": onboarding.created_at,
            "products": products.iter().map(|p| serde_json::json!({
                "code": p.product_code,
                "name": p.name
            })).collect::<Vec<_>>(),
            "services": services.iter().map(|s| serde_json::json!({
                "code": s.service_code,
                "name": s.name,
                "configuration": s.option_selections
            })).collect::<Vec<_>>()
        }))
    }

    // Helper methods

    async fn log_crud_operation(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        log: CrudLogEntry<'_>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".taxonomy_crud_log
            (operation_type, entity_type, entity_id, natural_language_input,
             success, error_message, user_id, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'system', NOW())
            "#,
            log.operation_type,
            log.entity_type,
            log.entity_id,
            log.natural_language_input,
            log.success,
            log.error_message
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}

struct CrudLogEntry<'a> {
    operation_type: &'a str,
    entity_type: &'a str,
    entity_id: Option<Uuid>,
    natural_language_input: &'a str,
    success: bool,
    error_message: Option<&'a str>,
}
