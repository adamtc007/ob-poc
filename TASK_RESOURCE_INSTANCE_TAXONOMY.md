# TASK: Resource Instance Taxonomy Implementation

## Overview

Implement the Production Resource Instance layer that represents the **actual delivered artifacts** when a CBU is onboarded to a Product. This completes the Product → Service → Resource Instance taxonomy.

**Key Concept:** A Resource Instance is the unique, configured "thing" created for a specific CBU — it has a URL, unique identifier, and its own attribute values.

## Current State

```
products                    → Definition: "Prime Brokerage"
    ↓ (product_services)
services                    → Definition: "Trade Settlement"  
    ↓ (service_resource_capabilities)
lifecycle_resources         → Type Definition: "DTCC Settlement System"
    ↓
??? (MISSING)               → Instance: "Acme Fund's DTCC Account #ABC123"
```

## Target State

```
CBU: "Acme Fund"
    ↓ subscribes to
Product: "Prime Brokerage"
    ↓ bundles
Service: "Trade Settlement" (config: markets=US_EQUITY, speed=T1)
    ↓ implemented by
Resource Type: "DTCC Settlement System"
    ↓ instantiated as
Resource Instance: 
    - URL: https://dtcc.com/accounts/acme-hf-001
    - Identifier: ACME-HF-001
    - Attributes: account_number=DTC-789456, bic_code=DTCYUS33
    ↓ tracked in
Service Delivery Map:
    - CBU → Product → Service → Instance
    - delivery_status: DELIVERED
```

---

## Task 1: Database Schema Migration

Create file: `sql/migrations/027_resource_instance_taxonomy.sql`

```sql
-- ============================================
-- Resource Instance Taxonomy Migration
-- Purpose: Add instance-level resource tracking for CBU onboarding
-- ============================================

BEGIN;

-- =============================================================================
-- 1. CBU RESOURCE INSTANCES - The actual "things" created for a CBU
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_resource_instances (
    instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Ownership & Lineage
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id UUID REFERENCES "ob-poc".products(product_id),
    service_id UUID REFERENCES "ob-poc".services(service_id),
    resource_type_id UUID REFERENCES "ob-poc".lifecycle_resources(resource_id),
    
    -- THE UNIQUE THING (the URL)
    instance_url VARCHAR(1024) NOT NULL,
    instance_identifier VARCHAR(255),
    instance_name VARCHAR(255),
    
    -- Configuration
    instance_config JSONB DEFAULT '{}',
    
    -- Lifecycle
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'PROVISIONING', 'ACTIVE', 'SUSPENDED', 'DECOMMISSIONED')),
    
    -- Audit
    requested_at TIMESTAMPTZ DEFAULT NOW(),
    provisioned_at TIMESTAMPTZ,
    activated_at TIMESTAMPTZ,
    decommissioned_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Constraints
    UNIQUE(instance_url),
    UNIQUE(cbu_id, resource_type_id, instance_identifier)
);

COMMENT ON TABLE "ob-poc".cbu_resource_instances IS 
'Production resource instances - the actual delivered artifacts for a CBU (accounts, connections, platform access)';

COMMENT ON COLUMN "ob-poc".cbu_resource_instances.instance_url IS 
'Unique URL/endpoint for this resource instance (e.g., https://custody.bank.com/accounts/ABC123)';

-- =============================================================================
-- 2. RESOURCE INSTANCE ATTRIBUTES - Dense table, no sparse matrix
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".resource_instance_attributes (
    value_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    
    -- Typed values (use one based on dictionary.value_type)
    value_text VARCHAR,
    value_number NUMERIC,
    value_boolean BOOLEAN,
    value_date DATE,
    value_timestamp TIMESTAMPTZ,
    value_json JSONB,
    
    -- Provenance
    state VARCHAR(50) DEFAULT 'proposed'
        CHECK (state IN ('proposed', 'confirmed', 'derived', 'system')),
    source JSONB,
    
    observed_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(instance_id, attribute_id)
);

COMMENT ON TABLE "ob-poc".resource_instance_attributes IS 
'Attribute values for resource instances - dense storage (row exists = value set)';

-- =============================================================================
-- 3. SERVICE DELIVERY MAP - The persisted "what was delivered" record
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".service_delivery_map (
    delivery_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id),
    instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    
    -- Service Configuration (options selected during onboarding)
    service_config JSONB DEFAULT '{}',
    
    -- Status
    delivery_status VARCHAR(50) DEFAULT 'PENDING'
        CHECK (delivery_status IN ('PENDING', 'IN_PROGRESS', 'DELIVERED', 'FAILED', 'CANCELLED')),
    
    -- Audit
    requested_at TIMESTAMPTZ DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    failure_reason TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(cbu_id, product_id, service_id)
);

COMMENT ON TABLE "ob-poc".service_delivery_map IS 
'Tracks service delivery for CBU onboarding - links CBU → Product → Service → Instance';

-- =============================================================================
-- 4. RESOURCE TYPE ATTRIBUTES - What attributes does each resource TYPE require?
-- =============================================================================
-- Note: resource_attribute_requirements already exists, but let's ensure it's complete

-- Add missing columns if needed
ALTER TABLE "ob-poc".resource_attribute_requirements
    ADD COLUMN IF NOT EXISTS default_value TEXT,
    ADD COLUMN IF NOT EXISTS display_order INTEGER DEFAULT 0;

-- =============================================================================
-- 5. INDEXES
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_cri_cbu ON "ob-poc".cbu_resource_instances(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cri_status ON "ob-poc".cbu_resource_instances(status);
CREATE INDEX IF NOT EXISTS idx_cri_resource_type ON "ob-poc".cbu_resource_instances(resource_type_id);
CREATE INDEX IF NOT EXISTS idx_cri_url ON "ob-poc".cbu_resource_instances(instance_url);

CREATE INDEX IF NOT EXISTS idx_ria_instance ON "ob-poc".resource_instance_attributes(instance_id);
CREATE INDEX IF NOT EXISTS idx_ria_attribute ON "ob-poc".resource_instance_attributes(attribute_id);

CREATE INDEX IF NOT EXISTS idx_sdm_cbu ON "ob-poc".service_delivery_map(cbu_id);
CREATE INDEX IF NOT EXISTS idx_sdm_product ON "ob-poc".service_delivery_map(product_id);
CREATE INDEX IF NOT EXISTS idx_sdm_service ON "ob-poc".service_delivery_map(service_id);
CREATE INDEX IF NOT EXISTS idx_sdm_status ON "ob-poc".service_delivery_map(delivery_status);

-- =============================================================================
-- 6. UPDATE TRIGGER for updated_at
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_cri_updated ON "ob-poc".cbu_resource_instances;
CREATE TRIGGER trg_cri_updated
    BEFORE UPDATE ON "ob-poc".cbu_resource_instances
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();

DROP TRIGGER IF EXISTS trg_sdm_updated ON "ob-poc".service_delivery_map;
CREATE TRIGGER trg_sdm_updated
    BEFORE UPDATE ON "ob-poc".service_delivery_map
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();

COMMIT;

-- =============================================================================
-- VERIFICATION
-- =============================================================================

SELECT 'cbu_resource_instances' as table_name, COUNT(*) as columns 
FROM information_schema.columns 
WHERE table_schema = 'ob-poc' AND table_name = 'cbu_resource_instances'
UNION ALL
SELECT 'resource_instance_attributes', COUNT(*) 
FROM information_schema.columns 
WHERE table_schema = 'ob-poc' AND table_name = 'resource_instance_attributes'
UNION ALL
SELECT 'service_delivery_map', COUNT(*) 
FROM information_schema.columns 
WHERE table_schema = 'ob-poc' AND table_name = 'service_delivery_map';
```

---

## Task 2: Seed Data for Resource Types

Create file: `sql/migrations/028_seed_resource_type_attributes.sql`

```sql
-- ============================================
-- Seed Resource Type Attribute Requirements
-- ============================================

BEGIN;

-- Get or create dictionary entries for resource attributes
INSERT INTO "ob-poc".dictionary (attribute_id, attribute_name, display_name, value_type, category)
VALUES
    (gen_random_uuid(), 'account_number', 'Account Number', 'string', 'resource.account'),
    (gen_random_uuid(), 'bic_code', 'BIC/SWIFT Code', 'string', 'resource.routing'),
    (gen_random_uuid(), 'settlement_currency', 'Settlement Currency', 'string', 'resource.account'),
    (gen_random_uuid(), 'api_key', 'API Key', 'string', 'resource.connection'),
    (gen_random_uuid(), 'api_secret', 'API Secret', 'string', 'resource.connection'),
    (gen_random_uuid(), 'platform_user_id', 'Platform User ID', 'string', 'resource.access'),
    (gen_random_uuid(), 'access_level', 'Access Level', 'string', 'resource.access'),
    (gen_random_uuid(), 'routing_number', 'Routing Number', 'string', 'resource.routing'),
    (gen_random_uuid(), 'iban', 'IBAN', 'string', 'resource.account'),
    (gen_random_uuid(), 'custodian_code', 'Custodian Code', 'string', 'resource.custody')
ON CONFLICT (attribute_name) DO UPDATE 
SET display_name = EXCLUDED.display_name,
    category = EXCLUDED.category;

-- Link attributes to DTCC resource type
WITH dtcc AS (SELECT resource_id FROM "ob-poc".lifecycle_resources WHERE resource_code = 'DTCC_SETTLE'),
     attrs AS (SELECT attribute_id, attribute_name FROM "ob-poc".dictionary 
               WHERE attribute_name IN ('account_number', 'bic_code', 'settlement_currency'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT dtcc.resource_id, attrs.attribute_id,
       CASE attrs.attribute_name
           WHEN 'account_number' THEN true
           WHEN 'bic_code' THEN true
           WHEN 'settlement_currency' THEN false
       END,
       CASE attrs.attribute_name
           WHEN 'account_number' THEN 1
           WHEN 'bic_code' THEN 2
           WHEN 'settlement_currency' THEN 3
       END
FROM dtcc, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE
SET is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

-- Link attributes to Euroclear resource type
WITH euro AS (SELECT resource_id FROM "ob-poc".lifecycle_resources WHERE resource_code = 'EUROCLEAR'),
     attrs AS (SELECT attribute_id, attribute_name FROM "ob-poc".dictionary 
               WHERE attribute_name IN ('account_number', 'iban', 'settlement_currency'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT euro.resource_id, attrs.attribute_id,
       CASE attrs.attribute_name
           WHEN 'account_number' THEN true
           WHEN 'iban' THEN true
           WHEN 'settlement_currency' THEN false
       END,
       CASE attrs.attribute_name
           WHEN 'account_number' THEN 1
           WHEN 'iban' THEN 2
           WHEN 'settlement_currency' THEN 3
       END
FROM euro, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE
SET is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

COMMIT;
```

---

## Task 3: Rust Database Service

Create file: `rust/src/database/resource_instance_service.rs`

```rust
//! Resource Instance Service - CRUD for CBU Resource Instances
//!
//! Manages the actual delivered artifacts (accounts, connections, platform access)
//! that are created when a CBU is onboarded to a product.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
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
    pub value_number: Option<rust_decimal::Decimal>,
    pub value_boolean: Option<bool>,
    pub value_date: Option<chrono::NaiveDate>,
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
    pub value_number: Option<rust_decimal::Decimal>,
    pub value_boolean: Option<bool>,
    pub value_date: Option<chrono::NaiveDate>,
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
        let instance_id = Uuid::new_v4();
        
        sqlx::query(r#"
            INSERT INTO "ob-poc".cbu_resource_instances 
                (instance_id, cbu_id, product_id, service_id, resource_type_id,
                 instance_url, instance_identifier, instance_name, instance_config, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'PENDING')
        "#)
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

        info!("Created resource instance {} for CBU {}", instance_id, input.cbu_id);
        Ok(instance_id)
    }

    pub async fn get_instance(&self, instance_id: Uuid) -> Result<Option<ResourceInstanceRow>> {
        sqlx::query_as::<_, ResourceInstanceRow>(r#"
            SELECT instance_id, cbu_id, product_id, service_id, resource_type_id,
                   instance_url, instance_identifier, instance_name, instance_config,
                   status, requested_at, provisioned_at, activated_at, decommissioned_at,
                   created_at, updated_at
            FROM "ob-poc".cbu_resource_instances
            WHERE instance_id = $1
        "#)
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get resource instance")
    }

    pub async fn get_instance_by_url(&self, url: &str) -> Result<Option<ResourceInstanceRow>> {
        sqlx::query_as::<_, ResourceInstanceRow>(r#"
            SELECT instance_id, cbu_id, product_id, service_id, resource_type_id,
                   instance_url, instance_identifier, instance_name, instance_config,
                   status, requested_at, provisioned_at, activated_at, decommissioned_at,
                   created_at, updated_at
            FROM "ob-poc".cbu_resource_instances
            WHERE instance_url = $1
        "#)
        .bind(url)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get resource instance by URL")
    }

    pub async fn list_instances_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<ResourceInstanceRow>> {
        sqlx::query_as::<_, ResourceInstanceRow>(r#"
            SELECT instance_id, cbu_id, product_id, service_id, resource_type_id,
                   instance_url, instance_identifier, instance_name, instance_config,
                   status, requested_at, provisioned_at, activated_at, decommissioned_at,
                   created_at, updated_at
            FROM "ob-poc".cbu_resource_instances
            WHERE cbu_id = $1
            ORDER BY created_at DESC
        "#)
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list resource instances for CBU")
    }

    pub async fn update_status(&self, instance_id: Uuid, status: &str) -> Result<bool> {
        let timestamp_field = match status {
            "PROVISIONING" => Some("provisioned_at"),
            "ACTIVE" => Some("activated_at"),
            "DECOMMISSIONED" => Some("decommissioned_at"),
            _ => None,
        };

        let query = if let Some(field) = timestamp_field {
            format!(
                r#"UPDATE "ob-poc".cbu_resource_instances 
                   SET status = $1, {} = NOW(), updated_at = NOW() 
                   WHERE instance_id = $2"#,
                field
            )
        } else {
            r#"UPDATE "ob-poc".cbu_resource_instances 
               SET status = $1, updated_at = NOW() 
               WHERE instance_id = $2"#.to_string()
        };

        let result = sqlx::query(&query)
            .bind(status)
            .bind(instance_id)
            .execute(&self.pool)
            .await
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
        let value_id = Uuid::new_v4();
        
        sqlx::query(r#"
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
        "#)
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

        info!("Set attribute {} on instance {}", input.attribute_id, input.instance_id);
        Ok(value_id)
    }

    pub async fn get_instance_attributes(&self, instance_id: Uuid) -> Result<Vec<ResourceInstanceAttributeRow>> {
        sqlx::query_as::<_, ResourceInstanceAttributeRow>(r#"
            SELECT value_id, instance_id, attribute_id, value_text, value_number,
                   value_boolean, value_date, value_timestamp, value_json, 
                   state, source, observed_at
            FROM "ob-poc".resource_instance_attributes
            WHERE instance_id = $1
            ORDER BY attribute_id
        "#)
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get instance attributes")
    }

    pub async fn get_attribute_value(&self, instance_id: Uuid, attribute_id: Uuid) -> Result<Option<ResourceInstanceAttributeRow>> {
        sqlx::query_as::<_, ResourceInstanceAttributeRow>(r#"
            SELECT value_id, instance_id, attribute_id, value_text, value_number,
                   value_boolean, value_date, value_timestamp, value_json,
                   state, source, observed_at
            FROM "ob-poc".resource_instance_attributes
            WHERE instance_id = $1 AND attribute_id = $2
        "#)
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
        let delivery_id = Uuid::new_v4();
        
        sqlx::query(r#"
            INSERT INTO "ob-poc".service_delivery_map
                (delivery_id, cbu_id, product_id, service_id, instance_id, 
                 service_config, delivery_status)
            VALUES ($1, $2, $3, $4, $5, $6, 'PENDING')
            ON CONFLICT (cbu_id, product_id, service_id) DO UPDATE SET
                instance_id = EXCLUDED.instance_id,
                service_config = EXCLUDED.service_config,
                updated_at = NOW()
        "#)
        .bind(delivery_id)
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .bind(instance_id)
        .bind(&service_config)
        .execute(&self.pool)
        .await
        .context("Failed to record service delivery")?;

        info!("Recorded delivery {} for CBU {} / product {} / service {}", 
              delivery_id, cbu_id, product_id, service_id);
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
        let timestamp_field = match status {
            "IN_PROGRESS" => "started_at",
            "DELIVERED" => "delivered_at",
            "FAILED" => "failed_at",
            _ => "updated_at",
        };

        let result = sqlx::query(&format!(
            r#"UPDATE "ob-poc".service_delivery_map 
               SET delivery_status = $1, {} = NOW(), failure_reason = $2, updated_at = NOW()
               WHERE cbu_id = $3 AND product_id = $4 AND service_id = $5"#,
            timestamp_field
        ))
        .bind(status)
        .bind(failure_reason)
        .bind(cbu_id)
        .bind(product_id)
        .bind(service_id)
        .execute(&self.pool)
        .await
        .context("Failed to update delivery status")?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_cbu_deliveries(&self, cbu_id: Uuid) -> Result<Vec<ServiceDeliveryRow>> {
        sqlx::query_as::<_, ServiceDeliveryRow>(r#"
            SELECT delivery_id, cbu_id, product_id, service_id, instance_id,
                   service_config, delivery_status, requested_at, started_at,
                   delivered_at, failed_at, failure_reason
            FROM "ob-poc".service_delivery_map
            WHERE cbu_id = $1
            ORDER BY requested_at DESC
        "#)
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CBU deliveries")
    }

    // -------------------------------------------------------------------------
    // Validation Helpers
    // -------------------------------------------------------------------------

    pub async fn get_required_attributes(&self, resource_type_id: Uuid) -> Result<Vec<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(r#"
            SELECT attribute_id 
            FROM "ob-poc".resource_attribute_requirements
            WHERE resource_id = $1 AND is_mandatory = true
            ORDER BY display_order
        "#)
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
        let set_ids: std::collections::HashSet<Uuid> = set_attrs.iter().map(|a| a.attribute_id).collect();
        
        // Find missing
        let mut missing = Vec::new();
        for attr_id in required {
            if !set_ids.contains(&attr_id) {
                missing.push(format!("Missing required attribute: {}", attr_id));
            }
        }
        
        Ok(missing)
    }
}
```

---

## Task 4: Update Database Module

Update file: `rust/src/database/mod.rs`

Add the new module:

```rust
// Add to existing mod.rs
pub mod resource_instance_service;
pub use resource_instance_service::{
    ResourceInstanceService, ResourceInstanceRow, ResourceInstanceAttributeRow,
    ServiceDeliveryRow, NewResourceInstance, SetInstanceAttribute,
};
```

---

## Task 5: DSL Verb Definitions

Add to verb registry in `rust/src/dsl_v2/verb_registry.rs`:

```rust
// Resource Instance Domain
VerbDef {
    domain: "resource",
    verb: "create",
    description: "Create a resource instance for a CBU",
    required_args: vec!["cbu-id", "resource-type", "instance-url"],
    optional_args: vec!["product-id", "service-id", "instance-id", "instance-name", "config"],
    produces_binding: true,
    category: VerbCategory::Create,
},
VerbDef {
    domain: "resource",
    verb: "set-attr",
    description: "Set an attribute value on a resource instance",
    required_args: vec!["instance-id", "attr", "value"],
    optional_args: vec!["state", "source"],
    produces_binding: false,
    category: VerbCategory::Update,
},
VerbDef {
    domain: "resource",
    verb: "activate",
    description: "Activate a resource instance (PENDING → ACTIVE)",
    required_args: vec!["instance-id"],
    optional_args: vec![],
    produces_binding: false,
    category: VerbCategory::Update,
},
VerbDef {
    domain: "resource",
    verb: "suspend",
    description: "Suspend a resource instance",
    required_args: vec!["instance-id"],
    optional_args: vec!["reason"],
    produces_binding: false,
    category: VerbCategory::Update,
},
VerbDef {
    domain: "resource",
    verb: "decommission",
    description: "Decommission a resource instance",
    required_args: vec!["instance-id"],
    optional_args: vec!["reason"],
    produces_binding: false,
    category: VerbCategory::Delete,
},

// Service Delivery Domain
VerbDef {
    domain: "delivery",
    verb: "record",
    description: "Record a service delivery for a CBU",
    required_args: vec!["cbu-id", "product", "service"],
    optional_args: vec!["instance-id", "config"],
    produces_binding: true,
    category: VerbCategory::Create,
},
VerbDef {
    domain: "delivery",
    verb: "complete",
    description: "Mark a service delivery as complete",
    required_args: vec!["cbu-id", "product", "service"],
    optional_args: vec!["instance-id"],
    produces_binding: false,
    category: VerbCategory::Update,
},
VerbDef {
    domain: "delivery",
    verb: "fail",
    description: "Mark a service delivery as failed",
    required_args: vec!["cbu-id", "product", "service", "reason"],
    optional_args: vec![],
    produces_binding: false,
    category: VerbCategory::Update,
},
```

---

## Task 6: DSL Mappings

Add to `rust/src/dsl_v2/mappings.rs`:

```rust
// Resource Instance mappings
("resource", "create") => TableMapping {
    table: "cbu_resource_instances",
    columns: hashmap! {
        "cbu-id" => "cbu_id",
        "product-id" => "product_id",
        "service-id" => "service_id",
        "resource-type" => "resource_type_id",  // Lookup from lifecycle_resources.resource_code
        "instance-url" => "instance_url",
        "instance-id" => "instance_identifier",
        "instance-name" => "instance_name",
        "config" => "instance_config",
    },
    pk_column: "instance_id",
},

// Service Delivery mappings
("delivery", "record") => TableMapping {
    table: "service_delivery_map",
    columns: hashmap! {
        "cbu-id" => "cbu_id",
        "product" => "product_id",    // Lookup from products.product_code
        "service" => "service_id",    // Lookup from services.service_code
        "instance-id" => "instance_id",
        "config" => "service_config",
    },
    pk_column: "delivery_id",
},
```

---

## Task 7: Executor Operations

Add to `rust/src/dsl_v2/executor.rs` or create `rust/src/dsl_v2/custom_ops/resource_ops.rs`:

```rust
//! Resource Instance Operations

use crate::database::ResourceInstanceService;
use anyhow::Result;
use uuid::Uuid;

pub async fn execute_resource_create(
    service: &ResourceInstanceService,
    cbu_id: Uuid,
    resource_type_code: &str,
    instance_url: &str,
    instance_identifier: Option<&str>,
    instance_name: Option<&str>,
    product_id: Option<Uuid>,
    service_id: Option<Uuid>,
) -> Result<Uuid> {
    // Lookup resource_type_id from code
    let resource_type_id = lookup_resource_type_by_code(resource_type_code).await?;
    
    let input = NewResourceInstance {
        cbu_id,
        product_id,
        service_id,
        resource_type_id: Some(resource_type_id),
        instance_url: instance_url.to_string(),
        instance_identifier: instance_identifier.map(|s| s.to_string()),
        instance_name: instance_name.map(|s| s.to_string()),
        instance_config: None,
    };
    
    service.create_instance(&input).await
}

pub async fn execute_resource_set_attr(
    service: &ResourceInstanceService,
    instance_id: Uuid,
    attr_name: &str,
    value: &str,
    state: Option<&str>,
) -> Result<Uuid> {
    // Lookup attribute_id from name
    let attribute_id = lookup_attribute_by_name(attr_name).await?;
    
    let input = SetInstanceAttribute {
        instance_id,
        attribute_id,
        value_text: Some(value.to_string()),
        value_number: None,
        value_boolean: None,
        value_date: None,
        value_json: None,
        state: state.map(|s| s.to_string()),
        source: None,
    };
    
    service.set_attribute(&input).await
}

pub async fn execute_resource_activate(
    service: &ResourceInstanceService,
    instance_id: Uuid,
) -> Result<bool> {
    // Validate all required attributes are set
    let missing = service.validate_instance_attributes(instance_id).await?;
    if !missing.is_empty() {
        anyhow::bail!("Cannot activate: {}", missing.join(", "));
    }
    
    service.update_status(instance_id, "ACTIVE").await
}
```

---

## Task 8: Update DATABASE_SCHEMA.md

Add to `docs/DATABASE_SCHEMA.md`:

```markdown
## Resource Instance Tables

### cbu_resource_instances
Production resource instances - the actual delivered artifacts for a CBU.

| Column | Type | Description |
|--------|------|-------------|
| instance_id | uuid | Primary key |
| cbu_id | uuid | FK to cbus |
| product_id | uuid | FK to products |
| service_id | uuid | FK to services |
| resource_type_id | uuid | FK to lifecycle_resources |
| instance_url | varchar | Unique URL/endpoint for this instance |
| instance_identifier | varchar | Account #, User ID, BIC code |
| instance_name | varchar | Human-readable name |
| instance_config | jsonb | Instance-specific settings |
| status | varchar | PENDING, PROVISIONING, ACTIVE, SUSPENDED, DECOMMISSIONED |

### resource_instance_attributes
Attribute values for resource instances (dense storage).

| Column | Type | Description |
|--------|------|-------------|
| value_id | uuid | Primary key |
| instance_id | uuid | FK to cbu_resource_instances |
| attribute_id | uuid | FK to dictionary |
| value_text | varchar | String value |
| value_number | numeric | Numeric value |
| value_boolean | boolean | Boolean value |
| value_date | date | Date value |
| value_json | jsonb | JSON value |
| state | varchar | proposed, confirmed, derived, system |

### service_delivery_map
Tracks service delivery for CBU onboarding.

| Column | Type | Description |
|--------|------|-------------|
| delivery_id | uuid | Primary key |
| cbu_id | uuid | FK to cbus |
| product_id | uuid | FK to products |
| service_id | uuid | FK to services |
| instance_id | uuid | FK to cbu_resource_instances |
| service_config | jsonb | Service configuration options |
| delivery_status | varchar | PENDING, IN_PROGRESS, DELIVERED, FAILED |
```

---

## DSL Usage Examples

```clojure
;; Onboard Acme Fund to Prime Brokerage
(cbu.ensure :name "Acme Hedge Fund" :jurisdiction "US" :as @fund)

;; Create DTCC settlement account for the fund
(resource.create 
    :cbu-id @fund 
    :resource-type "DTCC_SETTLE"
    :instance-url "https://dtcc.com/accounts/acme-hf-001"
    :instance-id "ACME-HF-001"
    :instance-name "Acme Fund DTCC Account"
    :as @dtcc-acct)

;; Set required attributes
(resource.set-attr :instance-id @dtcc-acct :attr "account_number" :value "DTC-789456")
(resource.set-attr :instance-id @dtcc-acct :attr "bic_code" :value "DTCYUS33")
(resource.set-attr :instance-id @dtcc-acct :attr "settlement_currency" :value "USD")

;; Activate the account (validates required attrs are set)
(resource.activate :instance-id @dtcc-acct)

;; Record the service delivery
(delivery.record 
    :cbu-id @fund 
    :product "PRIME_BROKER" 
    :service "SETTLEMENT"
    :instance-id @dtcc-acct
    :config {:markets ["US_EQUITY"] :speed "T1"})

;; Mark delivery complete
(delivery.complete :cbu-id @fund :product "PRIME_BROKER" :service "SETTLEMENT")
```

---

## Verification Checklist

1. [ ] Migration `027_resource_instance_taxonomy.sql` runs without errors
2. [ ] Migration `028_seed_resource_type_attributes.sql` runs without errors
3. [ ] `ResourceInstanceService` compiles and passes unit tests
4. [ ] DSL verbs registered in verb registry
5. [ ] DSL mappings configured
6. [ ] Executor operations implemented
7. [ ] E2E test: Create CBU → Create Instance → Set Attrs → Activate → Record Delivery
8. [ ] DATABASE_SCHEMA.md updated
9. [ ] All existing tests still pass

---

## Test Scenario

```sql
-- Verify the taxonomy flow
SELECT 
    c.name as cbu_name,
    p.product_code,
    s.service_code,
    lr.resource_code as resource_type,
    cri.instance_url,
    cri.instance_identifier,
    cri.status as instance_status,
    sdm.delivery_status
FROM "ob-poc".cbus c
JOIN "ob-poc".service_delivery_map sdm ON c.cbu_id = sdm.cbu_id
JOIN "ob-poc".products p ON sdm.product_id = p.product_id
JOIN "ob-poc".services s ON sdm.service_id = s.service_id
LEFT JOIN "ob-poc".cbu_resource_instances cri ON sdm.instance_id = cri.instance_id
LEFT JOIN "ob-poc".lifecycle_resources lr ON cri.resource_type_id = lr.resource_id
WHERE c.name = 'Acme Hedge Fund';
```

Expected output:
```
cbu_name          | product_code  | service_code | resource_type | instance_url                         | instance_identifier | instance_status | delivery_status
------------------+---------------+--------------+---------------+--------------------------------------+---------------------+-----------------+----------------
Acme Hedge Fund   | PRIME_BROKER  | SETTLEMENT   | DTCC_SETTLE   | https://dtcc.com/accounts/acme-hf-001| ACME-HF-001        | ACTIVE          | DELIVERED
```
