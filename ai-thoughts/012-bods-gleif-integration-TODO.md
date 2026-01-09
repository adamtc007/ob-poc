# BODS + GLEIF Integration - Claude Execution Guide

> **Purpose:** Step-by-step guide for Claude to implement BODS 0.4 + GLEIF deep integration
> **Migration File:** `migrations/010_bods_gleif_integration.sql`
> **Status:** ✅ IMPLEMENTED

---

## Current Capability Summary

### GLEIF DSL Verbs (15 operations)
| Verb | Purpose |
|------|--------|
| `gleif.enrich` | Enrich entity with GLEIF data |
| `gleif.search` | Search GLEIF by name/jurisdiction |
| `gleif.import-tree` | Import full corporate hierarchy |
| `gleif.refresh` | Refresh LEI data from API |
| `gleif.get-record` | Get single LEI record |
| `gleif.get-parent` | Get direct parent |
| `gleif.get-children` | Get direct children |
| `gleif.get-umbrella` | Get fund umbrella structure |
| `gleif.get-manager` | Get fund manager |
| `gleif.get-master-fund` | Get master fund for feeder |
| `gleif.trace-ownership` | Trace ownership chain |
| `gleif.get-managed-funds` | List funds managed by entity |
| `gleif.import-managed-funds` | Import all managed funds |
| `gleif.resolve-successor` | Find successor for merged/renamed LEI |
| `gleif.lookup-by-isin` | Find LEI by ISIN |

### BODS DSL Verbs (6 operations)
| Verb | Purpose |
|------|--------|
| `bods.discover-ubos` | Calculate UBOs from ownership graph |
| `bods.import` | Import BODS statement package |
| `bods.get-statement` | Get BODS statement for entity |
| `bods.find-by-lei` | Find BODS records by LEI |
| `bods.list-ownership` | List ownership interests |
| `bods.sync-from-gleif` | Sync GLEIF data into BODS format |

### Architecture
```
┌─────────────────────────────────────────────────────────────┐
│                    entity_identifiers                        │
│                  (LEI = Global Master Key)                   │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               │               ▼
┌─────────────────────┐       │     ┌─────────────────────────┐
│  gleif_relationships │       │     │   entity_relationships   │
│  (Consolidation)     │       │     │   (Beneficial Ownership) │
├─────────────────────┤       │     ├─────────────────────────┤
│ DirectParent         │       │     │ shareholding             │
│ UltimateParent       │       │     │ votingRights             │
│ DirectlyConsolidated │       │     │ boardMember              │
│ FundManager          │       │     │ trustee, settlor, ...    │
└─────────────────────┘       │     │ (23 BODS interest types) │
                              │     └─────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │    entities     │
                    │  (Hub Table)    │
                    └─────────────────┘
```

---

## PHASE 1: Database Schema (Run Migration)

### Step 1.1: Execute Migration

```bash
cd /Users/adamtc007/Developer/ob-poc
psql -d data_designer -f migrations/010_bods_gleif_integration.sql
```

### Step 1.2: Verify Migration Success

```bash
psql -d data_designer -c "
SELECT table_name FROM information_schema.tables 
WHERE table_schema = 'ob-poc' 
AND table_name IN ('entity_identifiers', 'gleif_relationships', 'bods_interest_types', 'bods_entity_types', 'person_pep_status')
ORDER BY table_name;
"
```

**Expected output:** 5 tables listed

### Step 1.3: Verify BODS Interest Types

```bash
psql -d data_designer -c "SELECT type_code, category FROM \"ob-poc\".bods_interest_types ORDER BY display_order;"
```

**Expected output:** 22 rows (shareholding, votingRights, trustee, etc.)

### Step 1.4: Verify entity_relationships Extensions

```bash
psql -d data_designer -c "
SELECT column_name, data_type 
FROM information_schema.columns 
WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships'
AND column_name IN ('interest_type', 'direct_or_indirect', 'share_minimum', 'share_maximum', 'is_component', 'component_of_relationship_id')
ORDER BY column_name;
"
```

**Expected output:** 6 new columns listed

---

## PHASE 2: Rust Types

### Step 2.1: Create BODS Types Module

**File:** `rust/src/database/bods_types.rs`

```rust
//! BODS 0.4 Types for OB-POC
//!
//! Structs matching the BODS integration tables from migration 010.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Entity identifier (LEI spine + other identifiers)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityIdentifier {
    pub identifier_id: Uuid,
    pub entity_id: Uuid,
    pub scheme: String,
    pub scheme_name: Option<String>,
    pub id: String,
    pub uri: Option<String>,
    pub is_validated: Option<bool>,
    pub validated_at: Option<DateTime<Utc>>,
    pub validation_source: Option<String>,
    pub lei_status: Option<String>,
    pub lei_next_renewal: Option<NaiveDate>,
    pub lei_managing_lou: Option<String>,
    pub effective_from: Option<NaiveDate>,
    pub effective_to: Option<NaiveDate>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// GLEIF corporate hierarchy relationship (SEPARATE from UBO)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GleifRelationship {
    pub gleif_rel_id: Uuid,
    pub parent_entity_id: Uuid,
    pub parent_lei: String,
    pub child_entity_id: Uuid,
    pub child_lei: String,
    pub relationship_type: String,
    pub relationship_status: Option<String>,
    pub ownership_percentage: Option<rust_decimal::Decimal>,
    pub accounting_standard: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub gleif_record_id: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

/// BODS interest type codelist entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BodsInterestType {
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub description: Option<String>,
    pub bods_standard: Option<bool>,
    pub requires_percentage: Option<bool>,
    pub display_order: Option<i32>,
}

/// Person PEP status (BODS-compliant)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PersonPepStatus {
    pub pep_status_id: Uuid,
    pub person_entity_id: Uuid,
    pub status: String,  // 'isPep', 'isNotPep', 'unknown'
    pub reason: Option<String>,
    pub jurisdiction: Option<String>,
    pub position_held: Option<String>,
    pub position_level: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub source_type: Option<String>,
    pub screening_id: Option<Uuid>,
    pub verified_at: Option<DateTime<Utc>>,
    pub verified_by: Option<String>,
    pub pep_risk_level: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Fields for creating a new entity identifier
#[derive(Debug, Clone)]
pub struct NewEntityIdentifier {
    pub entity_id: Uuid,
    pub scheme: String,
    pub id: String,
    pub scheme_name: Option<String>,
    pub uri: Option<String>,
}

/// Fields for creating a GLEIF relationship
#[derive(Debug, Clone)]
pub struct NewGleifRelationship {
    pub parent_entity_id: Uuid,
    pub parent_lei: String,
    pub child_entity_id: Uuid,
    pub child_lei: String,
    pub relationship_type: String,
    pub ownership_percentage: Option<rust_decimal::Decimal>,
    pub accounting_standard: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}
```

### Step 2.2: Create BODS Service

**File:** `rust/src/database/bods_service.rs`

```rust
//! BODS Service - Operations for BODS-aligned tables
//!
//! Handles entity_identifiers, gleif_relationships, person_pep_status

use anyhow::{Context, Result};
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use super::bods_types::*;

/// Service for BODS-related operations
#[derive(Clone, Debug)]
pub struct BodsService {
    pool: PgPool,
}

impl BodsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // =========================================================================
    // Entity Identifiers (LEI Spine)
    // =========================================================================

    /// Attach an identifier to an entity
    pub async fn attach_identifier(&self, fields: &NewEntityIdentifier) -> Result<Uuid> {
        let identifier_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_identifiers 
                (identifier_id, entity_id, scheme, scheme_name, id, uri, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            ON CONFLICT (entity_id, scheme, id) DO UPDATE SET
                scheme_name = EXCLUDED.scheme_name,
                uri = EXCLUDED.uri,
                updated_at = NOW()
            RETURNING identifier_id
            "#,
        )
        .bind(identifier_id)
        .bind(fields.entity_id)
        .bind(&fields.scheme)
        .bind(&fields.scheme_name)
        .bind(&fields.id)
        .bind(&fields.uri)
        .execute(&self.pool)
        .await
        .context("Failed to attach identifier")?;

        info!(
            "Attached {} identifier {} to entity {}",
            fields.scheme, fields.id, fields.entity_id
        );

        Ok(identifier_id)
    }

    /// Get entity by LEI
    pub async fn get_entity_by_lei(&self, lei: &str) -> Result<Option<Uuid>> {
        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT entity_id FROM "ob-poc".entity_identifiers
            WHERE scheme = 'LEI' AND id = $1
            "#,
        )
        .bind(lei)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get entity by LEI")?;

        Ok(result)
    }

    /// Get all identifiers for an entity
    pub async fn get_identifiers(&self, entity_id: Uuid) -> Result<Vec<EntityIdentifier>> {
        let results = sqlx::query_as::<_, EntityIdentifier>(
            r#"
            SELECT * FROM "ob-poc".entity_identifiers
            WHERE entity_id = $1
            ORDER BY scheme
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get identifiers")?;

        Ok(results)
    }

    /// Update LEI validation status (after GLEIF API call)
    pub async fn update_lei_validation(
        &self,
        entity_id: Uuid,
        lei: &str,
        lei_status: &str,
        lei_next_renewal: Option<chrono::NaiveDate>,
        managing_lou: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".entity_identifiers
            SET is_validated = true,
                validated_at = NOW(),
                validation_source = 'GLEIF_API',
                lei_status = $1,
                lei_next_renewal = $2,
                lei_managing_lou = $3,
                updated_at = NOW()
            WHERE entity_id = $4 AND scheme = 'LEI' AND id = $5
            "#,
        )
        .bind(lei_status)
        .bind(lei_next_renewal)
        .bind(managing_lou)
        .bind(entity_id)
        .bind(lei)
        .execute(&self.pool)
        .await
        .context("Failed to update LEI validation")?;

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // GLEIF Relationships (Corporate Hierarchy - SEPARATE from UBO)
    // =========================================================================

    /// Create a GLEIF relationship
    pub async fn create_gleif_relationship(&self, fields: &NewGleifRelationship) -> Result<Uuid> {
        let gleif_rel_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".gleif_relationships 
                (gleif_rel_id, parent_entity_id, parent_lei, child_entity_id, child_lei,
                 relationship_type, ownership_percentage, accounting_standard,
                 start_date, end_date, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
            ON CONFLICT (parent_lei, child_lei, relationship_type) DO UPDATE SET
                parent_entity_id = EXCLUDED.parent_entity_id,
                child_entity_id = EXCLUDED.child_entity_id,
                ownership_percentage = EXCLUDED.ownership_percentage,
                accounting_standard = EXCLUDED.accounting_standard,
                start_date = EXCLUDED.start_date,
                end_date = EXCLUDED.end_date,
                updated_at = NOW()
            "#,
        )
        .bind(gleif_rel_id)
        .bind(fields.parent_entity_id)
        .bind(&fields.parent_lei)
        .bind(fields.child_entity_id)
        .bind(&fields.child_lei)
        .bind(&fields.relationship_type)
        .bind(fields.ownership_percentage)
        .bind(&fields.accounting_standard)
        .bind(fields.start_date)
        .bind(fields.end_date)
        .execute(&self.pool)
        .await
        .context("Failed to create GLEIF relationship")?;

        info!(
            "Created GLEIF {} relationship: {} -> {}",
            fields.relationship_type, fields.parent_lei, fields.child_lei
        );

        Ok(gleif_rel_id)
    }

    /// Get GLEIF corporate hierarchy for an entity (parents)
    pub async fn get_gleif_parents(&self, child_lei: &str) -> Result<Vec<GleifRelationship>> {
        let results = sqlx::query_as::<_, GleifRelationship>(
            r#"
            SELECT * FROM "ob-poc".gleif_relationships
            WHERE child_lei = $1
              AND (relationship_status = 'ACTIVE' OR relationship_status IS NULL)
            ORDER BY relationship_type
            "#,
        )
        .bind(child_lei)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get GLEIF parents")?;

        Ok(results)
    }

    /// Get GLEIF corporate hierarchy for an entity (children)
    pub async fn get_gleif_children(&self, parent_lei: &str) -> Result<Vec<GleifRelationship>> {
        let results = sqlx::query_as::<_, GleifRelationship>(
            r#"
            SELECT * FROM "ob-poc".gleif_relationships
            WHERE parent_lei = $1
              AND (relationship_status = 'ACTIVE' OR relationship_status IS NULL)
            ORDER BY relationship_type
            "#,
        )
        .bind(parent_lei)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get GLEIF children")?;

        Ok(results)
    }

    // =========================================================================
    // BODS Interest Types
    // =========================================================================

    /// Get all BODS interest types
    pub async fn get_interest_types(&self) -> Result<Vec<BodsInterestType>> {
        let results = sqlx::query_as::<_, BodsInterestType>(
            r#"
            SELECT * FROM "ob-poc".bods_interest_types
            ORDER BY display_order
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get interest types")?;

        Ok(results)
    }

    /// Get interest types by category
    pub async fn get_interest_types_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<BodsInterestType>> {
        let results = sqlx::query_as::<_, BodsInterestType>(
            r#"
            SELECT * FROM "ob-poc".bods_interest_types
            WHERE category = $1
            ORDER BY display_order
            "#,
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get interest types by category")?;

        Ok(results)
    }

    // =========================================================================
    // Person PEP Status
    // =========================================================================

    /// Add PEP status for a person
    pub async fn add_pep_status(
        &self,
        person_entity_id: Uuid,
        status: &str,
        jurisdiction: Option<&str>,
        position_held: Option<&str>,
        source_type: Option<&str>,
    ) -> Result<Uuid> {
        let pep_status_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".person_pep_status
                (pep_status_id, person_entity_id, status, jurisdiction, position_held, source_type, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            "#,
        )
        .bind(pep_status_id)
        .bind(person_entity_id)
        .bind(status)
        .bind(jurisdiction)
        .bind(position_held)
        .bind(source_type)
        .execute(&self.pool)
        .await
        .context("Failed to add PEP status")?;

        info!("Added PEP status {} for person {}", status, person_entity_id);

        Ok(pep_status_id)
    }

    /// Get PEP status for a person
    pub async fn get_pep_status(&self, person_entity_id: Uuid) -> Result<Vec<PersonPepStatus>> {
        let results = sqlx::query_as::<_, PersonPepStatus>(
            r#"
            SELECT * FROM "ob-poc".person_pep_status
            WHERE person_entity_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(person_entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get PEP status")?;

        Ok(results)
    }

    /// Check if person is currently a PEP
    pub async fn is_pep(&self, person_entity_id: Uuid) -> Result<bool> {
        let result = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".person_pep_status
                WHERE person_entity_id = $1
                  AND status = 'isPep'
                  AND (end_date IS NULL OR end_date > CURRENT_DATE)
            )
            "#,
        )
        .bind(person_entity_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check PEP status")?;

        Ok(result)
    }
}
```

### Step 2.3: Register in mod.rs

**Edit:** `rust/src/database/mod.rs`

Add these lines:

```rust
pub mod bods_types;
pub mod bods_service;

pub use bods_types::*;
pub use bods_service::BodsService;
```

---

## PHASE 3: DSL Verb Definitions

### Step 3.1: Create GLEIF Verbs

**File:** `rust/config/verbs/gleif.yaml` (create or append)

```yaml
domains:
  gleif:
    description: GLEIF LEI operations and corporate hierarchy
    verbs:
      attach-lei:
        description: Attach an LEI to an entity and optionally validate against GLEIF API
        behavior: plugin
        plugin:
          handler: GleifAttachLeiOp
        args:
          - name: entity-id
            type: uuid
            required: true
            description: Entity to attach LEI to
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: lei
            type: string
            required: true
            description: The 20-character LEI
            validation:
              pattern: "^[A-Z0-9]{20}$"
          - name: verify
            type: boolean
            required: false
            default: true
            description: Validate LEI against GLEIF API
        returns:
          type: uuid
          name: identifier_id

      verify-lei:
        description: Verify an existing LEI against GLEIF API and update status
        behavior: plugin
        plugin:
          handler: GleifVerifyLeiOp
        args:
          - name: entity-id
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record
          description: LEI validation result

      import-hierarchy:
        description: Import GLEIF corporate hierarchy (parents/children) - SEPARATE from UBO
        behavior: plugin
        plugin:
          handler: GleifImportHierarchyOp
        args:
          - name: lei
            type: string
            required: true
            description: LEI to import hierarchy for
          - name: depth
            type: integer
            required: false
            default: 3
            description: How many levels to traverse
          - name: direction
            type: string
            required: false
            default: both
            valid_values:
              - parents
              - children
              - both
        returns:
          type: record
          description: Summary of imported entities and relationships

      list-hierarchy:
        description: List GLEIF corporate hierarchy for an entity
        behavior: crud
        crud:
          operation: select
          table: v_gleif_hierarchy
          schema: ob-poc
        args:
          - name: lei
            type: string
            required: true
        returns:
          type: record_set
```

### Step 3.2: Create BODS Verbs

**File:** `rust/config/verbs/bods.yaml` (new file)

```yaml
domains:
  bods:
    description: BODS 0.4 compliance operations
    verbs:
      validate-cbu:
        description: Validate a CBU against BODS 0.4 schema
        behavior: plugin
        plugin:
          handler: BodsValidateCbuOp
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record
          description: Validation result with errors and warnings

      export-package:
        description: Export CBU as BODS 0.4 JSON package
        behavior: plugin
        plugin:
          handler: BodsExportPackageOp
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: as-of-date
            type: date
            required: false
            description: Point-in-time date for export (defaults to today)
          - name: format
            type: string
            required: false
            default: json
            valid_values:
              - json
              - jsonl
        returns:
          type: string
          description: BODS 0.4 compliant JSON

      list-interest-types:
        description: List available BODS interest types
        behavior: crud
        crud:
          operation: select
          table: bods_interest_types
          schema: ob-poc
        args:
          - name: category
            type: string
            required: false
            description: Filter by category (ownership, control, trust, etc.)
        returns:
          type: record_set
```

### Step 3.3: Update UBO Verbs

**Edit:** `rust/config/verbs/ubo.yaml`

Update `add-ownership` to include new args:

```yaml
      # Add these args to existing add-ownership verb:
      - name: interest-type
        type: string
        required: false
        maps_to: interest_type
        default: shareholding
        description: BODS interest type code
        lookup:
          table: bods_interest_types
          entity_type: interest_type
          schema: ob-poc
          search_key: type_code
          primary_key: type_code
      - name: direct
        type: boolean
        required: false
        description: Is this direct ownership (true) or indirect (false)?
        # Handler maps to direct_or_indirect column
      - name: share-min
        type: decimal
        required: false
        maps_to: share_minimum
        description: Minimum share percentage (for ranges)
      - name: share-max
        type: decimal
        required: false
        maps_to: share_maximum
        description: Maximum share percentage (for ranges)
```

---

## PHASE 4: Plugin Handlers

### Step 4.1: GLEIF Ops Skeleton

**File:** `rust/src/dsl_v2/custom_ops/gleif_ops.rs`

```rust
//! GLEIF DSL Operations
//!
//! Handlers for gleif.attach-lei, gleif.verify-lei, gleif.import-hierarchy

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::dsl_v2::custom_ops::CustomOp;
use crate::dsl_v2::engine::ExecutionContext;

/// gleif.attach-lei - Attach LEI to entity
pub struct GleifAttachLeiOp;

#[async_trait]
impl CustomOp for GleifAttachLeiOp {
    async fn execute(&self, ctx: &mut ExecutionContext, args: Value) -> Result<Value> {
        let entity_id = args["entity-id"].as_str().unwrap();
        let lei = args["lei"].as_str().unwrap();
        let verify = args.get("verify").and_then(|v| v.as_bool()).unwrap_or(true);

        // TODO: Implement
        // 1. Validate LEI format (20 alphanumeric)
        // 2. If verify=true, call GLEIF API
        // 3. Insert into entity_identifiers
        // 4. If GLEIF returns data, update lei_status, lei_next_renewal, etc.

        todo!("Implement GleifAttachLeiOp")
    }
}

/// gleif.import-hierarchy - Import GLEIF corporate hierarchy
pub struct GleifImportHierarchyOp;

#[async_trait]
impl CustomOp for GleifImportHierarchyOp {
    async fn execute(&self, ctx: &mut ExecutionContext, args: Value) -> Result<Value> {
        let lei = args["lei"].as_str().unwrap();
        let depth = args.get("depth").and_then(|v| v.as_i64()).unwrap_or(3);
        let direction = args.get("direction").and_then(|v| v.as_str()).unwrap_or("both");

        // TODO: Implement
        // 1. Call GLEIF API for relationship data
        // 2. For each parent/child:
        //    a. Create entity if not exists (by LEI)
        //    b. Insert into gleif_relationships (NOT entity_relationships!)
        // 3. Recurse up to depth

        todo!("Implement GleifImportHierarchyOp")
    }
}
```

### Step 4.2: BODS Ops Skeleton

**File:** `rust/src/dsl_v2/custom_ops/bods_ops.rs`

```rust
//! BODS DSL Operations
//!
//! Handlers for bods.validate-cbu, bods.export-package

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::dsl_v2::custom_ops::CustomOp;
use crate::dsl_v2::engine::ExecutionContext;

/// bods.validate-cbu - Validate CBU against BODS schema
pub struct BodsValidateCbuOp;

#[async_trait]
impl CustomOp for BodsValidateCbuOp {
    async fn execute(&self, ctx: &mut ExecutionContext, args: Value) -> Result<Value> {
        let cbu_id = args["cbu-id"].as_str().unwrap();

        // TODO: Implement validation rules:
        // 1. All interests have valid interest_type (FK to bods_interest_types)
        // 2. All entities have bods_entity_type
        // 3. Indirect interests have component records
        // 4. Share ranges valid (min <= max)
        // 5. UBO flags only on person owners
        // 6. No circular ownership

        todo!("Implement BodsValidateCbuOp")
    }
}

/// bods.export-package - Export as BODS 0.4 JSON
pub struct BodsExportPackageOp;

#[async_trait]
impl CustomOp for BodsExportPackageOp {
    async fn execute(&self, ctx: &mut ExecutionContext, args: Value) -> Result<Value> {
        let cbu_id = args["cbu-id"].as_str().unwrap();
        let as_of_date = args.get("as-of-date").and_then(|v| v.as_str());

        // TODO: Implement BODS 0.4 export:
        // 1. Query all entities in CBU
        // 2. Query all relationships
        // 3. Query all identifiers
        // 4. Build BODS statement array
        // 5. Return as JSON

        todo!("Implement BodsExportPackageOp")
    }
}
```

### Step 4.3: Register Ops

**Edit:** `rust/src/dsl_v2/custom_ops/mod.rs`

Add:

```rust
pub mod gleif_ops;
pub mod bods_ops;

// In register_ops function:
registry.register("GleifAttachLeiOp", Box::new(gleif_ops::GleifAttachLeiOp));
registry.register("GleifImportHierarchyOp", Box::new(gleif_ops::GleifImportHierarchyOp));
registry.register("BodsValidateCbuOp", Box::new(bods_ops::BodsValidateCbuOp));
registry.register("BodsExportPackageOp", Box::new(bods_ops::BodsExportPackageOp));
```

---

## PHASE 5: Testing

### Step 5.1: Test Migration Views

```bash
# Test v_entities_with_lei view
psql -d data_designer -c "SELECT * FROM \"ob-poc\".v_entities_with_lei LIMIT 5;"

# Test v_ubo_interests view  
psql -d data_designer -c "SELECT * FROM \"ob-poc\".v_ubo_interests LIMIT 5;"

# Test v_gleif_hierarchy view
psql -d data_designer -c "SELECT * FROM \"ob-poc\".v_gleif_hierarchy LIMIT 5;"
```

### Step 5.2: Test Insert Operations

```bash
# Test inserting an entity identifier
psql -d data_designer -c "
INSERT INTO \"ob-poc\".entity_identifiers (entity_id, scheme, id)
SELECT entity_id, 'LEI', '529900TEST00000000XX'
FROM \"ob-poc\".entities LIMIT 1
RETURNING identifier_id, entity_id, scheme, id;
"

# Verify interest types available
psql -d data_designer -c "
SELECT type_code, display_name, category 
FROM \"ob-poc\".bods_interest_types 
WHERE category = 'ownership';
"
```

### Step 5.3: Compile Rust

```bash
cd /Users/adamtc007/Developer/ob-poc/rust
cargo check
cargo build
```

---

## Checklist Summary

### Database (Phase 1)
- [ ] Run migration 010_bods_gleif_integration.sql
- [ ] Verify 5 new tables created
- [ ] Verify 22 BODS interest types populated
- [ ] Verify 6 new columns on entity_relationships
- [ ] Verify 3 new views created

### Rust Types (Phase 2)
- [ ] Create bods_types.rs
- [ ] Create bods_service.rs
- [ ] Register in mod.rs
- [ ] Cargo check passes

### DSL Verbs (Phase 3)
- [ ] Create/update gleif.yaml
- [ ] Create bods.yaml
- [ ] Update ubo.yaml with interest-type

### Plugin Handlers (Phase 4)
- [ ] Create gleif_ops.rs skeleton
- [ ] Create bods_ops.rs skeleton
- [ ] Register in mod.rs

### Testing (Phase 5)
- [ ] Test views work
- [ ] Test insert operations
- [ ] Cargo build succeeds

---

## Key Architecture Reminder

```
GLEIF (gleif_relationships)          UBO (entity_relationships)
────────────────────────────         ────────────────────────────
Corporate hierarchy                  Beneficial ownership
Accounting consolidation             KYC/AML compliance
DirectParent/UltimateParent          shareholding/votingRights/trustee

        ↓                                    ↓
        └──────────── BOTH USE ─────────────┘
                         │
                entity_identifiers
                    (LEI spine)
```

**Never mix GLEIF and UBO semantics!**
