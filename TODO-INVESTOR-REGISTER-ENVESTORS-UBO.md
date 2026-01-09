# TODO: Investor Register + Envestors + BODS/GLEIF/UBO Integration

> **For:** Claude Code Implementation
> **Design Doc:** `ai-thoughts/013-investor-register-envestors-ubo-integration.md`
> **Status:** READY FOR IMPLEMENTATION
> **Created:** 2026-01-09

---

## Overview

Implement provider-agnostic investor register that:
1. Ingests investor data from **Envestors** (primary), Clearstream, or manual entry
2. Feeds holdings into **UBO domain** (holdings ≥25% = beneficial ownership)
3. Exports to **BODS 0.4** format for regulatory reporting
4. Links entities via **GLEIF LEI** spine
5. Auto-creates **KYC case workstreams** for UBO-qualified investors

---

## PHASE 1: Database Schema Updates

### TODO 1.1: Rename and Update Migration 011

**Action:** Replace `migrations/011_clearstream_investor_views.sql` with provider-agnostic version

```bash
# Delete old file
rm /Users/adamtc007/Developer/ob-poc/migrations/011_clearstream_investor_views.sql

# Create new file (content below)
```

**File:** `/Users/adamtc007/Developer/ob-poc/migrations/011_investor_register_views.sql`

**Content Requirements:**
1. Add columns to `kyc.holdings`:
   - `provider VARCHAR(50) DEFAULT 'MANUAL'` - Data source (ENVESTORS, CLEARSTREAM, MANUAL, CSV_IMPORT)
   - `provider_reference VARCHAR(100)` - External reference ID
   - `provider_sync_at TIMESTAMPTZ` - Last sync timestamp

2. Create view `kyc.v_investor_register` (renamed from v_clearstream_register):
   - Join holdings → share_classes → cbus → entities
   - Include LEI and provider identifiers
   - Calculate ownership_percentage per share class
   - Include market_value calculation

3. Create view `kyc.v_investor_movements` (renamed from v_clearstream_movements):
   - Movement audit trail with full context

4. Create view `kyc.v_holdings_ubo_qualified`:
   - Filter holdings where ownership_percentage >= 25%
   - Flag `is_ubo_qualified` boolean
   - Flag `ubo_status`: 'DIRECT_UBO' for natural persons, 'REQUIRES_TRACING' for corporates

5. Create view `kyc.v_bods_ownership_statements`:
   - UNION of holdings-based and entity_relationships-based ownership
   - Output in BODS 0.4 format

6. Create function `kyc.sync_holding_to_ubo()`:
   - Trigger function that creates/updates entity_relationships when holding ≥25%
   - Only for holdings linked to fund entities

7. Create function `kyc.create_workstream_for_ubo_holding()`:
   - Trigger that auto-creates KYC entity_workstream for UBO-qualified holdings

8. Create indexes for performance

**Verification:**
```sql
-- After running migration
SELECT table_name FROM information_schema.views WHERE table_schema = 'kyc' AND table_name LIKE 'v_%';
-- Should include: v_investor_register, v_investor_movements, v_holdings_ubo_qualified, v_bods_ownership_statements
```

---

### TODO 1.2: Update identifier.yaml with Envestors Schemes

**File:** `/Users/adamtc007/Developer/ob-poc/rust/config/verbs/identifier.yaml`

**Action:** Add to `valid_values` for scheme argument:

```yaml
valid_values:
  - LEI
  - ENVESTORS_ID          # ADD: Envestors investor ID
  - ENVESTORS_FUND_ID     # ADD: Envestors fund ID  
  - CLEARSTREAM_KV
  - CLEARSTREAM_ACCT
  - ISIN
  - company_register
  - tax_id
  - SWIFT_BIC
  - DUNS
  - VAT
  - national_id
```

Also add convenience verb `identifier.attach-envestors`:

```yaml
      attach-envestors:
        description: Attach Envestors investor or fund ID
        behavior: crud
        crud:
          operation: upsert
          table: entity_identifiers
          schema: ob-poc
          returning: identifier_id
          conflict_keys:
            - entity_id
            - scheme
            - id
          set_values:
            scheme_name: Envestors Platform Reference
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: reference
            type: string
            required: true
            maps_to: id
            description: Envestors reference ID
          - name: reference-type
            type: string
            required: false
            maps_to: scheme
            default: ENVESTORS_ID
            valid_values:
              - ENVESTORS_ID
              - ENVESTORS_FUND_ID
          - name: is-validated
            type: boolean
            required: false
            maps_to: is_validated
            default: true
        returns:
          type: uuid
          name: identifier_id
          capture: true
```

---

### TODO 1.3: Update holding.yaml with Provider Columns

**File:** `/Users/adamtc007/Developer/ob-poc/rust/config/verbs/registry/holding.yaml`

**Action:** Add provider args to `create` and `ensure` verbs:

```yaml
          - name: provider
            type: string
            required: false
            maps_to: provider
            default: MANUAL
            valid_values:
              - ENVESTORS
              - CLEARSTREAM
              - MANUAL
              - CSV_IMPORT
          - name: provider-reference
            type: string
            required: false
            maps_to: provider_reference
            description: External reference ID from provider
```

---

## PHASE 2: Envestors Module

### TODO 2.1: Create Envestors Types

**File:** `/Users/adamtc007/Developer/ob-poc/rust/src/envestors/types.rs`

```rust
//! Envestors API Types
//!
//! Types for ingesting data from Envestors platform (PE/retail investors)

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Envestors investor record (from API or CSV)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvestorsInvestor {
    pub investor_id: String,
    pub investor_type: InvestorType,
    pub name: String,
    pub email: Option<String>,
    pub country_code: Option<String>,
    pub tax_id: Option<String>,
    pub lei: Option<String>,
    pub kyc_status: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvestorType {
    Individual,
    Company,
    Nominee,
    Trust,
    Partnership,
}

/// Envestors fund/offering record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvestorsFund {
    pub fund_id: String,
    pub name: String,
    pub isin: Option<String>,
    pub currency: String,
    pub fund_type: String,
    pub status: String,
}

/// Envestors investment/holding record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvestorsInvestment {
    pub investment_id: String,
    pub investor_id: String,
    pub fund_id: String,
    pub amount: Decimal,
    pub units: Option<Decimal>,
    pub currency: String,
    pub status: String,
    pub commitment_date: Option<NaiveDate>,
    pub call_date: Option<NaiveDate>,
}

/// Result of import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub investors_imported: u64,
    pub holdings_imported: u64,
    pub errors: Vec<String>,
}

/// Result of UBO sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboSyncResult {
    pub relationships_created: u64,
    pub relationships_updated: u64,
    pub workstreams_created: u64,
}
```

---

### TODO 2.2: Create Envestors Service

**File:** `/Users/adamtc007/Developer/ob-poc/rust/src/envestors/service.rs`

**Key Methods:**
1. `import_investor(&self, investor: &EnvestorsInvestor) -> Result<Uuid>` - Create/update entity + identifiers
2. `import_fund(&self, fund: &EnvestorsFund, cbu_id: Uuid) -> Result<Uuid>` - Create share class
3. `import_investment(&self, investment: &EnvestorsInvestment, share_class_id: Uuid, investor_entity_id: Uuid) -> Result<Uuid>` - Create holding
4. `sync_holdings_to_ubo(&self, cbu_id: Uuid) -> Result<UboSyncResult>` - Bulk sync qualified holdings to entity_relationships
5. `bulk_import_csv(&self, file_path: &str, cbu_id: Uuid, dry_run: bool) -> Result<ImportResult>` - CSV import

**Implementation Notes:**
- Use existing `EntityService` for entity creation
- Use existing `BodsService` for BODS integration
- Attach `ENVESTORS_ID` identifier to all imported entities
- Map `InvestorType::Individual` → entity_type `proper_person`
- Map `InvestorType::Company` → entity_type `limited_company`

---

### TODO 2.3: Create Envestors Module Entry

**File:** `/Users/adamtc007/Developer/ob-poc/rust/src/envestors/mod.rs`

```rust
//! Envestors Integration Module
//!
//! Handles data ingestion from Envestors platform for PE/retail investors.

pub mod service;
pub mod types;

pub use service::EnvestorsService;
pub use types::*;
```

---

### TODO 2.4: Register Envestors Module in lib.rs

**File:** `/Users/adamtc007/Developer/ob-poc/rust/src/lib.rs`

**Action:** Add module declaration:

```rust
pub mod envestors;
```

---

## PHASE 3: Envestors DSL Verbs

### TODO 3.1: Create Envestors Verb YAML

**File:** `/Users/adamtc007/Developer/ob-poc/rust/config/verbs/envestors.yaml`

```yaml
domains:
  envestors:
    description: Envestors platform integration for PE/retail investor import
    verbs:
      import-investor:
        description: Import investor from Envestors platform
        behavior: plugin
        args:
          - name: envestors-id
            type: string
            required: true
            description: Envestors investor ID
          - name: name
            type: string
            required: true
          - name: investor-type
            type: string
            required: true
            valid_values: [INDIVIDUAL, COMPANY, NOMINEE, TRUST, PARTNERSHIP]
          - name: country
            type: string
            required: false
          - name: email
            type: string
            required: false
          - name: tax-id
            type: string
            required: false
          - name: lei
            type: string
            required: false
        returns:
          type: uuid
          name: entity_id
          capture: true

      import-fund:
        description: Import fund from Envestors as share class
        behavior: plugin
        args:
          - name: envestors-fund-id
            type: string
            required: true
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: name
            type: string
            required: true
          - name: isin
            type: string
            required: false
          - name: currency
            type: string
            default: EUR
          - name: fund-type
            type: string
            valid_values: [PRIVATE_EQUITY, VENTURE_CAPITAL, REAL_ESTATE, INFRASTRUCTURE, HEDGE_FUND]
        returns:
          type: uuid
          name: share_class_id
          capture: true

      import-investment:
        description: Import investment/commitment as holding
        behavior: plugin
        args:
          - name: envestors-investment-id
            type: string
            required: true
          - name: share-class-id
            type: uuid
            required: true
            lookup:
              table: share_classes
              entity_type: share_class
              schema: kyc
              search_key: name
              primary_key: id
          - name: investor-entity-id
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: amount
            type: decimal
            required: true
          - name: units
            type: decimal
            required: false
          - name: commitment-date
            type: date
            required: false
        returns:
          type: uuid
          name: holding_id
          capture: true

      sync-to-ubo:
        description: Sync all qualified holdings to UBO domain for a fund
        behavior: plugin
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

      bulk-import:
        description: Bulk import from Envestors CSV export
        behavior: plugin
        args:
          - name: file-path
            type: string
            required: true
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: dry-run
            type: boolean
            default: false
        returns:
          type: record
```

---

### TODO 3.2: Create Envestors Plugin Handlers

**File:** `/Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/custom_ops/envestors_ops.rs`

**Handlers to implement:**
1. `EnvestorsImportInvestorOp` - Create entity + ENVESTORS_ID identifier
2. `EnvestorsImportFundOp` - Create share class + ENVESTORS_FUND_ID identifier
3. `EnvestorsImportInvestmentOp` - Create holding with provider=ENVESTORS
4. `EnvestorsSyncToUboOp` - Bulk sync qualified holdings to entity_relationships
5. `EnvestorsBulkImportOp` - Parse CSV and import all records

**Registration:** Add to `custom_ops/mod.rs`:
```rust
pub mod envestors_ops;
pub use envestors_ops::*;
```

And register in executor plugin map.

---

## PHASE 4: GLEIF Integration for Corporate Investors

### TODO 4.1: Add trace-investor-ubo Verb to GLEIF

**File:** `/Users/adamtc007/Developer/ob-poc/rust/config/verbs/gleif.yaml`

**Action:** Add new verb:

```yaml
      trace-investor-ubo:
        description: Trace UBO for a corporate investor via GLEIF hierarchy
        behavior: plugin
        args:
          - name: holding-id
            type: uuid
            required: true
            description: The holding to trace UBO for
            lookup:
              table: holdings
              entity_type: holding
              schema: kyc
              search_key: id
              primary_key: id
          - name: create-relationships
            type: boolean
            default: true
            description: Create entity_relationships for discovered ownership chain
        returns:
          type: record
```

---

### TODO 4.2: Implement trace-investor-ubo Handler

**File:** `/Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/custom_ops/gleif_ops.rs`

**Action:** Add `GleifTraceInvestorUboOp` handler:

1. Get holding details including investor entity_type and LEI
2. If natural person → return early (they ARE the UBO)
3. If corporate without LEI → return error suggesting gleif.enrich first
4. Call existing `gleif.trace-ownership` with the LEI
5. If `create-relationships=true`, create entity_relationships for each link
6. Return trace result with UBO list

---

## PHASE 5: BODS Export Enhancement

### TODO 5.1: Add export-statements Verb to BODS

**File:** `/Users/adamtc007/Developer/ob-poc/rust/config/verbs/bods.yaml`

**Action:** Add new verb:

```yaml
      export-statements:
        description: Export BODS 0.4 statements for regulatory reporting
        behavior: plugin
        args:
          - name: cbu-id
            type: uuid
            required: false
            description: Filter to specific fund (optional)
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: format
            type: string
            default: json
            valid_values: [json, ndjson, csv]
          - name: include-source
            type: string
            default: ALL
            valid_values: [INVESTOR_REGISTER, ENTITY_RELATIONSHIP, ALL]
          - name: output-path
            type: string
            required: false
            description: File path for export (if not provided, returns inline)
        returns:
          type: record
```

---

### TODO 5.2: Implement export-statements Handler

**File:** `/Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/custom_ops/bods_ops.rs`

**Action:** Add `BodsExportStatementsOp` handler:

1. Query `kyc.v_bods_ownership_statements` view
2. Apply filters (cbu_id, source)
3. Format as BODS 0.4 JSON schema
4. Write to file or return inline

---

## PHASE 6: Documentation Updates

### TODO 6.1: Update CLAUDE.md

**File:** `/Users/adamtc007/Developer/ob-poc/CLAUDE.md`

**Actions:**
1. Update header stats (verb count, YAML files, migrations)
2. Replace "Clearstream" section with "Investor Register Integration"
3. Add Envestors verbs table
4. Add Holdings → UBO integration documentation
5. Add example DSL for Envestors import flow

---

## Verification Checklist

### After Phase 1 (Database):
```sql
-- Check new columns
SELECT column_name FROM information_schema.columns 
WHERE table_schema = 'kyc' AND table_name = 'holdings' 
AND column_name IN ('provider', 'provider_reference', 'provider_sync_at');

-- Check views created
SELECT table_name FROM information_schema.views 
WHERE table_schema = 'kyc' AND table_name LIKE 'v_%';

-- Check triggers
SELECT trigger_name FROM information_schema.triggers 
WHERE trigger_schema = 'kyc';
```

### After Phase 2-3 (Rust):
```bash
cd /Users/adamtc007/Developer/ob-poc/rust
cargo check --lib
cargo test envestors
```

### After Phase 4-5 (Integration):
```bash
# Run integration test
cargo test --test db_integration investor_register_ubo
```

### Full Integration Test DSL:
```clojure
;; Test: Envestors → Holdings → UBO → BODS

;; 1. Setup fund
(cbu.ensure :name "Test PE Fund" :jurisdiction "LU" :as @fund)
(entity.ensure-limited-company :name "Test PE Fund LP" :jurisdiction "LU" :as @fund-entity)

;; 2. Import via Envestors verbs
(envestors.import-fund 
  :envestors-fund-id "ENV-FUND-001" 
  :cbu-id @fund 
  :name "Class A"
  :fund-type "PRIVATE_EQUITY"
  :as @class-a)

(envestors.import-investor 
  :envestors-id "ENV-INV-001"
  :name "John Smith"
  :investor-type "INDIVIDUAL"
  :country "GB"
  :as @john)

(envestors.import-investment
  :envestors-investment-id "ENV-INV-001-001"
  :share-class-id @class-a
  :investor-entity-id @john
  :amount 100000
  :units 30000
  :commitment-date "2025-01-15")

;; 3. Verify UBO detection
(envestors.sync-to-ubo :cbu-id @fund)

;; 4. Export BODS
(bods.export-statements :cbu-id @fund :format "json")
```

---

## File Summary

| Action | File Path |
|--------|-----------|
| CREATE | `migrations/011_investor_register_views.sql` |
| CREATE | `rust/src/envestors/mod.rs` |
| CREATE | `rust/src/envestors/types.rs` |
| CREATE | `rust/src/envestors/service.rs` |
| CREATE | `rust/config/verbs/envestors.yaml` |
| CREATE | `rust/src/dsl_v2/custom_ops/envestors_ops.rs` |
| MODIFY | `rust/config/verbs/identifier.yaml` |
| MODIFY | `rust/config/verbs/registry/holding.yaml` |
| MODIFY | `rust/config/verbs/gleif.yaml` |
| MODIFY | `rust/config/verbs/bods.yaml` |
| MODIFY | `rust/src/dsl_v2/custom_ops/mod.rs` |
| MODIFY | `rust/src/dsl_v2/custom_ops/gleif_ops.rs` |
| MODIFY | `rust/src/dsl_v2/custom_ops/bods_ops.rs` |
| MODIFY | `rust/src/lib.rs` |
| MODIFY | `CLAUDE.md` |
| DELETE | `migrations/011_clearstream_investor_views.sql` |

---

## Execution Notes for Claude Code

1. **Start with Phase 1** - Database must be updated first
2. **Run migration before Rust changes** - Views must exist for service queries
3. **Use existing patterns** - Follow `gleif/` module structure for `envestors/`
4. **Register handlers in executor** - Check how existing plugin ops are registered
5. **Test incrementally** - Verify each phase before proceeding
