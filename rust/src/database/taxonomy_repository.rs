//! Taxonomy repository for Product-Service-Resource operations
//!
//! This repository handles all taxonomy-related database operations including
//! product management, service discovery, resource allocation, and onboarding workflows.

use crate::models::taxonomy::*;
use anyhow::{anyhow, Context, Result};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct TaxonomyRepository {
    pool: PgPool,
}

impl TaxonomyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ============================================
    // Product Operations
    // ============================================

    pub async fn create_product(
        &self,
        code: &str,
        name: &str,
        category: Option<&str>,
    ) -> Result<Product> {
        let product = sqlx::query_as!(
            Product,
            r#"
            INSERT INTO "ob-poc".products 
            (product_code, name, product_category, is_active)
            VALUES ($1, $2, $3, true)
            RETURNING *
            "#,
            code,
            name,
            category
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to create product")?;

        Ok(product)
    }

    pub async fn get_product_by_code(&self, code: &str) -> Result<Option<Product>> {
        let product = sqlx::query_as!(
            Product,
            r#"SELECT * FROM "ob-poc".products WHERE product_code = $1 AND is_active = true"#,
            code
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(product)
    }

    pub async fn list_active_products(&self) -> Result<Vec<Product>> {
        let products = sqlx::query_as!(
            Product,
            r#"SELECT * FROM "ob-poc".products WHERE is_active = true ORDER BY name"#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(products)
    }

    // ============================================
    // Service Discovery
    // ============================================

    pub async fn discover_services_for_product(&self, product_id: Uuid) -> Result<Vec<Service>> {
        let services = sqlx::query_as!(
            Service,
            r#"
            SELECT s.* FROM "ob-poc".services s
            JOIN "ob-poc".product_services ps ON s.service_id = ps.service_id
            WHERE ps.product_id = $1 AND s.is_active = true
            ORDER BY ps.display_order, s.name
            "#,
            product_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(services)
    }

    pub async fn get_service_by_code(&self, code: &str) -> Result<Option<Service>> {
        let service = sqlx::query_as!(
            Service,
            r#"SELECT * FROM "ob-poc".services WHERE service_code = $1 AND is_active = true"#,
            code
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(service)
    }

    // ============================================
    // Service Options
    // ============================================

    pub async fn get_service_options(&self, service_id: Uuid) -> Result<Vec<ServiceOptionDefinition>> {
        let options = sqlx::query_as!(
            ServiceOptionDefinition,
            r#"
            SELECT * FROM "ob-poc".service_option_definitions 
            WHERE service_id = $1 
            ORDER BY display_order, option_key
            "#,
            service_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(options)
    }

    pub async fn get_option_choices(&self, option_def_id: Uuid) -> Result<Vec<ServiceOptionChoice>> {
        let choices = sqlx::query_as!(
            ServiceOptionChoice,
            r#"
            SELECT * FROM "ob-poc".service_option_choices 
            WHERE option_def_id = $1 AND is_active = true
            ORDER BY display_order, choice_value
            "#,
            option_def_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(choices)
    }

    pub async fn get_service_with_options(&self, service_id: Uuid) -> Result<ServiceWithOptions> {
        let service = sqlx::query_as!(
            Service,
            r#"SELECT * FROM "ob-poc".services WHERE service_id = $1"#,
            service_id
        )
        .fetch_one(&self.pool)
        .await?;

        let option_defs = self.get_service_options(service_id).await?;

        let mut options = Vec::new();
        for def in option_defs {
            let choices = self.get_option_choices(def.option_def_id).await?;
            options.push(ServiceOptionWithChoices {
                definition: def,
                choices,
            });
        }

        Ok(ServiceWithOptions { service, options })
    }

    // ============================================
    // Resource Management
    // ============================================

    pub async fn find_capable_resources(
        &self,
        service_id: Uuid,
        options: &serde_json::Value,
    ) -> Result<Vec<ProductionResource>> {
        let resources = sqlx::query_as!(
            ProductionResource,
            r#"
            SELECT pr.* FROM "ob-poc".prod_resources pr
            JOIN "ob-poc".service_resource_capabilities src ON pr.resource_id = src.resource_id
            WHERE src.service_id = $1 
              AND src.is_active = true
              AND pr.is_active = true
              AND src.supported_options @> $2
            ORDER BY src.priority DESC
            "#,
            service_id,
            options
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(resources)
    }

    pub async fn get_resource_attributes(&self, resource_id: Uuid) -> Result<Vec<Uuid>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            attribute_id: Uuid,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"
            SELECT attribute_id FROM "ob-poc".resource_attribute_requirements
            WHERE resource_id = $1 AND is_mandatory = true
            "#,
        )
        .bind(resource_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.attribute_id).collect())
    }

    // ============================================
    // Onboarding Request Management
    // ============================================

    pub async fn create_onboarding_request(
        &self,
        cbu_id: Uuid,
        created_by: &str,
    ) -> Result<OnboardingRequest> {
        let request = sqlx::query_as!(
            OnboardingRequest,
            r#"
            INSERT INTO "ob-poc".onboarding_requests 
            (cbu_id, request_state, created_by, dsl_version)
            VALUES ($1, 'draft', $2, 1)
            RETURNING *
            "#,
            cbu_id,
            created_by
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(request)
    }

    pub async fn get_onboarding_request(&self, request_id: Uuid) -> Result<Option<OnboardingRequest>> {
        let request = sqlx::query_as!(
            OnboardingRequest,
            r#"SELECT * FROM "ob-poc".onboarding_requests WHERE request_id = $1"#,
            request_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(request)
    }

    pub async fn add_product_to_request(&self, request_id: Uuid, product_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".onboarding_products 
            (request_id, product_id, selection_order)
            VALUES ($1, $2, (
                SELECT COALESCE(MAX(selection_order), 0) + 1 
                FROM "ob-poc".onboarding_products 
                WHERE request_id = $1
            ))
            ON CONFLICT (request_id, product_id) DO NOTHING
            "#,
            request_id,
            product_id
        )
        .execute(&mut *tx)
        .await?;

        // Update state
        sqlx::query!(
            r#"
            UPDATE "ob-poc".onboarding_requests 
            SET request_state = 'products_selected', updated_at = NOW()
            WHERE request_id = $1 AND request_state = 'draft'
            "#,
            request_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_request_products(&self, request_id: Uuid) -> Result<Vec<Product>> {
        let products = sqlx::query_as!(
            Product,
            r#"
            SELECT p.* FROM "ob-poc".products p
            JOIN "ob-poc".onboarding_products op ON p.product_id = op.product_id
            WHERE op.request_id = $1
            ORDER BY op.selection_order
            "#,
            request_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(products)
    }

    pub async fn configure_service(
        &self,
        request_id: Uuid,
        service_id: Uuid,
        options: &serde_json::Value,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".onboarding_service_configs 
            (request_id, service_id, option_selections, is_valid)
            VALUES ($1, $2, $3, true)
            ON CONFLICT (request_id, service_id)
            DO UPDATE SET 
                option_selections = $3, 
                configured_at = NOW(),
                is_valid = true
            "#,
            request_id,
            service_id,
            options
        )
        .execute(&mut *tx)
        .await?;

        // Check if all mandatory services are configured
        #[derive(sqlx::FromRow)]
        struct Count {
            count: Option<i64>,
        }

        let unconfigured = sqlx::query_as::<_, Count>(
            r#"
            SELECT COUNT(*) as count
            FROM "ob-poc".services s
            JOIN "ob-poc".product_services ps ON s.service_id = ps.service_id
            JOIN "ob-poc".onboarding_products op ON ps.product_id = op.product_id
            LEFT JOIN "ob-poc".onboarding_service_configs osc 
                ON s.service_id = osc.service_id AND osc.request_id = $1
            WHERE op.request_id = $1 
              AND ps.is_mandatory = true
              AND osc.config_id IS NULL
            "#,
        )
        .bind(request_id)
        .fetch_one(&mut *tx)
        .await?;

        if unconfigured.count.unwrap_or(1) == 0 {
            sqlx::query!(
                r#"
                UPDATE "ob-poc".onboarding_requests 
                SET request_state = 'services_configured', updated_at = NOW()
                WHERE request_id = $1
                "#,
                request_id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn allocate_resources(
        &self,
        request_id: Uuid,
        allocations: Vec<ResourceAllocationRequest>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for allocation in allocations {
            let attrs: Vec<Uuid> = allocation.required_attributes;

            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".onboarding_resource_allocations
                (request_id, service_id, resource_id, handles_options, required_attributes, allocation_status)
                VALUES ($1, $2, $3, $4, $5, 'confirmed')
                "#,
                request_id,
                allocation.service_id,
                allocation.resource_id,
                serde_json::to_value(&allocation.handles_options)?,
                &attrs
            )
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query!(
            r#"
            UPDATE "ob-poc".onboarding_requests 
            SET request_state = 'resources_allocated', updated_at = NOW()
            WHERE request_id = $1
            "#,
            request_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn complete_onboarding(&self, request_id: Uuid, final_dsl: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".onboarding_requests 
            SET request_state = 'complete', 
                dsl_draft = $2,
                completed_at = NOW(),
                updated_at = NOW()
            WHERE request_id = $1
            "#,
            request_id,
            final_dsl
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_request_state(
        &self,
        request_id: Uuid,
        new_state: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".onboarding_requests 
            SET request_state = $2, updated_at = NOW()
            WHERE request_id = $1
            "#,
            request_id,
            new_state
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
