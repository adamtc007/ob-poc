# GLEIF API Deep Dive & Entity Schema Gap Analysis

**Date:** 2025-01-01  
**Purpose:** Comprehensive analysis of GLEIF API data fields and mapping to ob-poc entity schemas

---

## Executive Summary

GLEIF (Global Legal Entity Identifier Foundation) provides **free, structured, API access** to legal entity reference data. This document details:

1. **All available GLEIF data fields** (Level 1 + Level 2)
2. **Current ob-poc entity schema coverage**
3. **Gap analysis** - what GLEIF provides that ob-poc doesn't capture
4. **Implementation recommendations**

---

## GLEIF API Overview

### Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /lei-records` | Search/list entities |
| `GET /lei-records/{lei}` | Get single entity record |
| `GET /lei-records/{lei}/direct-parent-relationship` | Direct parent |
| `GET /lei-records/{lei}/ultimate-parent-relationship` | Ultimate parent |
| `GET /lei-records/{lei}/direct-children` | Direct children |
| `GET /lei-records/{lei}/ultimate-children` | Ultimate children |
| `GET /lei-records/{lei}/fund-manager` | Fund manager relationship |
| `GET /lei-records/{lei}/managed-funds` | Funds managed by entity |
| `GET /lei-records/{lei}/umbrella-fund` | Umbrella fund relationship |
| `GET /lei-records/{lei}/sub-funds` | Sub-funds of umbrella |
| `GET /lei-records/{lei}/master-fund` | Master fund relationship |
| `GET /lei-records/{lei}/feeder-funds` | Feeder funds of master |

### Rate Limits

- **60 requests per minute** per IP
- No authentication required
- No usage fees

---

## Level 1 Data: Entity Reference ("Who is Who")

### Complete Field Reference


```json
{
  "data": {
    "type": "lei-records",
    "id": "529900K9B0N5BT694847",
    "attributes": {
      "lei": "529900K9B0N5BT694847",
      
      "entity": {
        // === CORE IDENTIFICATION ===
        "legalName": {
          "name": "Allianz SE",
          "language": "de"
        },
        "otherNames": [
          {
            "name": "ALLIANZ SOCIETAS EUROPAEA",
            "type": "TRADING_OR_OPERATING_NAME",
            "language": "en"
          },
          {
            "name": "アリアンツ SE",
            "type": "TRANSLITERATED_NAME",
            "language": "ja"
          }
        ],
        "transliteratedOtherNames": [
          {
            "name": "Allianz SE",
            "type": "AUTO_ASCII_TRANSLITERATED_LEGAL_NAME",
            "language": "de"
          }
        ],
        
        // === LEGAL STRUCTURE ===
        "legalForm": {
          "id": "MWBW",                    // ELF (Entity Legal Form) code
          "other": null                     // Free text if no standard code
        },
        "jurisdiction": "DE",               // ISO 3166-1/2 code
        "category": null,                   // FUND, BRANCH, SOLE_PROPRIETOR, etc.
        "subCategory": null,                // More specific classification
        "creationDate": "1890-01-01",       // Entity creation date (not LEI registration)
        
        // === STATUS ===
        "status": "ACTIVE",                 // ACTIVE, INACTIVE
        "expirationDate": null,             // If INACTIVE
        "expirationReason": null,           // CORPORATE_ACTION, DISSOLVED, etc.
        
        // === ADDRESSES ===
        "legalAddress": {
          "language": "de",
          "addressLines": ["Königinstr. 28"],
          "addressNumber": null,
          "addressNumberWithinBuilding": null,
          "mailRouting": null,
          "city": "München",
          "region": "DE-BY",                // ISO 3166-2 region
          "country": "DE",
          "postalCode": "80802"
        },
        "headquartersAddress": {
          "language": "de",
          "addressLines": ["Königinstr. 28"],
          "city": "München",
          "region": "DE-BY",
          "country": "DE",
          "postalCode": "80802"
        },
        "otherAddresses": [
          {
            "type": "ALTERNATIVE_LANGUAGE_LEGAL_ADDRESS",
            "language": "en",
            "addressLines": ["Koeniginstrasse 28"],
            "city": "Munich",
            "country": "DE",
            "postalCode": "80802"
          }
        ],
        
        // === REGISTRATION AUTHORITY ===
        "registeredAt": {
          "id": "RA000304",                 // Registration Authority ID
          "other": null                      // Free text if no standard code
        },
        "registeredAs": "HRB 164232",       // Registration number
        
        // === SUCCESSOR (for defunct entities) ===
        "successorEntities": [
          {
            "lei": "5493001KJTIIGC8Y1R12",
            "name": "Successor Company Name"
          }
        ],
        
        // === CORPORATE EVENTS ===
        "eventGroups": [
          {
            "groupType": "CHANGE_LEGAL_NAME",
            "events": [
              {
                "type": "CHANGE_LEGAL_NAME",
                "status": "COMPLETED",
                "effectiveDate": "2006-02-14",
                "recordedDate": "2020-03-15",
                "validationDocuments": "SUPPORTING_DOCUMENTS",
                "validationReference": "https://...",
                "affectedFields": [
                  {
                    "xpath": "/lei:LEIRecord/lei:Entity/lei:LegalName",
                    "value": "Allianz AG"
                  }
                ]
              }
            ]
          }
        ]
      },
      
      "registration": {
        // === LEI REGISTRATION STATUS ===
        "initialRegistrationDate": "2012-12-03T00:00:00+00:00",
        "lastUpdateDate": "2024-10-24T11:45:45+00:00",
        "status": "ISSUED",                 // ISSUED, PENDING, LAPSED, etc.
        "nextRenewalDate": "2025-12-04T23:00:00+00:00",
        "managingLou": "5299000J2N45DDNE4Y28",  // Local Operating Unit that manages
        "corroborationLevel": "FULLY_CORROBORATED",  // Validation level
        "validatedAt": {
          "id": "RA000304",
          "other": null
        },
        "validatedAs": "HRB 164232"
      },
      
      // === CONFORMITY FLAGS ===
      "conformityFlag": "CONFORMING"        // CONFORMING or NON_CONFORMING
    },
    
    "relationships": {
      // Links to related endpoints
      "managing-lou": { ... },
      "lei-issuer": { ... },
      "direct-parent": { ... },
      "ultimate-parent": { ... },
      "direct-children": { ... },
      "ultimate-children": { ... },
      "fund-manager": { ... },
      "managed-funds": { ... },
      "umbrella-fund": { ... },
      "sub-funds": { ... },
      "master-fund": { ... },
      "feeder-funds": { ... },
      "successor-entities": { ... },
      "isins": { ... },
      "bics": { ... }
    }
  }
}
```

---

## Level 2 Data: Relationships ("Who Owns Whom")

### Relationship Types

| Type | Description |
|------|-------------|
| `IS_DIRECTLY_CONSOLIDATED_BY` | Direct accounting consolidation parent |
| `IS_ULTIMATELY_CONSOLIDATED_BY` | Ultimate parent in ownership chain |
| `IS_INTERNATIONAL_BRANCH_OF` | Branch office of another entity |
| `IS_FUND-MANAGED_BY` | Fund → Manager relationship |
| `IS_SUBFUND_OF` | Sub-fund → Umbrella fund |
| `IS_FEEDER_TO` | Feeder fund → Master fund |

### Relationship Record Structure

```json
{
  "data": {
    "type": "rr-relationship-records",
    "id": "relationship-uuid",
    "attributes": {
      "relationship": {
        "startNode": {
          "nodeID": "529900CHILD_LEI_HERE",
          "nodeIDType": "LEI"
        },
        "endNode": {
          "nodeID": "529900PARENT_LEI_HERE",
          "nodeIDType": "LEI"
        },
        "relationshipType": "IS_DIRECTLY_CONSOLIDATED_BY",
        "relationshipPeriods": [
          {
            "startDate": "2015-01-01",
            "endDate": null,
            "periodType": "RELATIONSHIP_PERIOD"
          }
        ],
        "relationshipStatus": "ACTIVE",
        "relationshipQualifiers": [
          {
            "qualifierDimension": "ACCOUNTING_STANDARD",
            "qualifierCategory": "IFRS"
          }
        ]
      },
      "registration": {
        "initialRegistrationDate": "2017-05-10",
        "lastUpdateDate": "2024-06-15",
        "status": "PUBLISHED",
        "validationSources": "FULLY_CORROBORATED",
        "validationDocuments": "ACCOUNTS_FILING",
        "validationReference": "Annual Report 2023"
      }
    }
  }
}
```

---

## Mapped Identifiers

GLEIF provides cross-references to other identifier systems:

| Identifier | Source | Example |
|------------|--------|---------|
| **BIC** | SWIFT | ALLIDEMM |
| **ISIN** | Securities | DE0008404005 |
| **CIK** | SEC EDGAR | 0001127508 |
| **MIC** | Market Identifier | XFRA |

### BIC Mapping Endpoint

`GET /lei-records/{lei}/bics`

```json
{
  "data": [
    {
      "type": "lei-bic-mappings",
      "attributes": {
        "bic": "ALLIDEMM",
        "lei": "529900K9B0N5BT694847"
      }
    }
  ]
}
```

---

## Current ob-poc Entity Schema Mapping

### What We Currently Capture

| GLEIF Field | ob-poc Table | ob-poc Column | Status |
|-------------|--------------|---------------|--------|
| `lei` | `entities` | `external_id` | ⚠️ Partial (not typed) |
| `legalName` | `entity_limited_companies` | `company_name` | ✅ |
| `jurisdiction` | `entity_limited_companies` | `jurisdiction` | ✅ |
| `registeredAs` | `entity_limited_companies` | `registration_number` | ✅ |
| `incorporationDate` | `entity_limited_companies` | `incorporation_date` | ✅ |
| `legalAddress` | `entity_limited_companies` | `registered_address` | ⚠️ Single text field |
| `status` | - | - | ❌ Missing |
| `category` | - | - | ❌ Missing |
| `legalForm.id` | - | - | ❌ Missing |
| `otherNames` | - | - | ❌ Missing |
| `headquartersAddress` | - | - | ❌ Missing |
| `creationDate` | - | - | ❌ Missing |
| `directParent` | - | - | ❌ Missing (in schema) |
| `ultimateParent` | - | - | ❌ Missing (in schema) |
| `nextRenewalDate` | - | - | ❌ Missing |
| `corroborationLevel` | - | - | ❌ Missing |
| `BIC codes` | - | - | ❌ Missing |
| `eventGroups` | - | - | ❌ Missing |


---

## Gap Analysis

### Critical Missing Fields (High Priority)

| GLEIF Field | Why It Matters | Recommendation |
|-------------|----------------|----------------|
| **LEI as typed identifier** | Need to distinguish LEI from other external_ids | Add `lei` column to entities table |
| **Entity Status** | Track ACTIVE vs INACTIVE/DISSOLVED | Add `gleif_status` column |
| **Entity Category** | FUND vs BRANCH vs SOLE_PROPRIETOR matters for KYC | Add `gleif_category` column |
| **Legal Form Code** | Standard ELF codes for cross-jurisdictional comparison | Add `legal_form_code` column |
| **Direct Parent LEI** | UBO chain tracking | Add relationship table or column |
| **Ultimate Parent LEI** | UBO chain terminus | Add relationship table or column |
| **Corroboration Level** | Data quality indicator | Add `gleif_validation_level` column |

### Address Structure Gap

**Current:** Single `registered_address` TEXT field  
**GLEIF provides:**
- Structured address lines
- City, Region, Country, Postal Code separately
- Multiple address types (Legal, HQ, Branch)
- Language variants

**Recommendation:** Create `entity_addresses` table:

```sql
CREATE TABLE "ob-poc".entity_addresses (
    address_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    address_type VARCHAR(50) NOT NULL,  -- LEGAL, HEADQUARTERS, BRANCH, etc.
    language VARCHAR(10),
    address_lines TEXT[],
    city VARCHAR(200),
    region VARCHAR(50),           -- ISO 3166-2
    country VARCHAR(3) NOT NULL,  -- ISO 3166-1 alpha-2
    postal_code VARCHAR(50),
    is_primary BOOLEAN DEFAULT FALSE,
    source VARCHAR(50),           -- GLEIF, USER_INPUT, DOCUMENT
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Alternative Names Gap

**Current:** Only `company_name` captured  
**GLEIF provides:**
- Trading names
- Transliterated names
- Historical names
- Language variants

**Recommendation:** Create `entity_names` table:

```sql
CREATE TABLE "ob-poc".entity_names (
    name_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    name_type VARCHAR(50) NOT NULL,  -- LEGAL, TRADING, TRANSLITERATED, HISTORICAL
    name TEXT NOT NULL,
    language VARCHAR(10),
    is_primary BOOLEAN DEFAULT FALSE,
    effective_from DATE,
    effective_to DATE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Parent Relationship Gap

**Current:** No direct parent tracking in schema  
**Existing:** `ALLIANZ-DATA-ACQUISITION.md` scraper captures relationships  
**Gap:** No persistent storage for relationships

**Recommendation:** Enhance relationship tracking:

```sql
-- Add GLEIF-specific columns to entities
ALTER TABLE "ob-poc".entity_limited_companies ADD COLUMN IF NOT EXISTS
    lei VARCHAR(20) UNIQUE,
    gleif_status VARCHAR(20),
    gleif_category VARCHAR(50),
    legal_form_code VARCHAR(10),
    gleif_validation_level VARCHAR(30),
    gleif_last_update TIMESTAMPTZ,
    gleif_next_renewal DATE,
    direct_parent_lei VARCHAR(20),
    ultimate_parent_lei VARCHAR(20),
    created_date DATE;  -- Entity creation date (not record creation)

-- Create parent relationship table for full chain tracking
CREATE TABLE "ob-poc".entity_parent_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    parent_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    parent_lei VARCHAR(20),  -- Store even if parent not in our system
    relationship_type VARCHAR(50) NOT NULL,  -- DIRECT_PARENT, ULTIMATE_PARENT
    accounting_standard VARCHAR(20),  -- IFRS, US_GAAP, etc.
    relationship_start DATE,
    relationship_end DATE,
    validation_source VARCHAR(50),
    validation_reference TEXT,
    source VARCHAR(50) DEFAULT 'GLEIF',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Identifier Mapping Gap

### Current State

External identifiers stored in `entities.external_id` without typing.

### GLEIF Provides

- LEI (20-char)
- BIC codes (8 or 11 char)
- ISIN references
- CIK (SEC)
- Registration Authority IDs

**Recommendation:** Create typed identifier table:

```sql
CREATE TABLE "ob-poc".entity_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    identifier_type VARCHAR(30) NOT NULL,  -- LEI, BIC, ISIN, CIK, REG_NUM
    identifier_value VARCHAR(50) NOT NULL,
    issuing_authority VARCHAR(100),
    is_primary BOOLEAN DEFAULT FALSE,
    valid_from DATE,
    valid_until DATE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(entity_id, identifier_type, identifier_value)
);

CREATE INDEX idx_entity_identifiers_type_value 
ON "ob-poc".entity_identifiers(identifier_type, identifier_value);
```

---

## Corporate Events Gap

GLEIF tracks entity lifecycle events:
- Name changes
- Mergers/acquisitions
- Jurisdiction changes
- Status changes
- Successor entities

**Current:** Not captured  
**Recommendation:** Create events table:

```sql
CREATE TABLE "ob-poc".entity_lifecycle_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    event_type VARCHAR(50) NOT NULL,  -- CHANGE_LEGAL_NAME, MERGER, DISSOLUTION, etc.
    event_status VARCHAR(30),         -- PENDING, COMPLETED
    effective_date DATE,
    recorded_date DATE,
    affected_fields JSONB,            -- What changed
    old_values JSONB,
    new_values JSONB,
    successor_lei VARCHAR(20),
    validation_documents VARCHAR(50),
    validation_reference TEXT,
    source VARCHAR(50) DEFAULT 'GLEIF',
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Fund-Specific Relationships

GLEIF Level 2 provides fund structure data:

| Relationship | Description |
|--------------|-------------|
| Fund → Fund Manager | IS_FUND-MANAGED_BY |
| Sub-fund → Umbrella | IS_SUBFUND_OF |
| Feeder → Master | IS_FEEDER_TO |

**Current:** Not captured in schema  
**Recommendation:** Add fund relationship columns:

```sql
ALTER TABLE "ob-poc".entity_limited_companies ADD COLUMN IF NOT EXISTS
    fund_manager_lei VARCHAR(20),
    umbrella_fund_lei VARCHAR(20),
    master_fund_lei VARCHAR(20),
    is_fund BOOLEAN DEFAULT FALSE,
    fund_type VARCHAR(30);  -- UCITS, AIF, ETF, etc.
```

---

## Schema Enhancement Migration

### Full Migration Script

```sql
-- GLEIF Entity Enhancement Migration
-- Run after backing up existing data

BEGIN;

-- 1. Add LEI and GLEIF-specific columns to entity_limited_companies
ALTER TABLE "ob-poc".entity_limited_companies 
ADD COLUMN IF NOT EXISTS lei VARCHAR(20) UNIQUE,
ADD COLUMN IF NOT EXISTS gleif_status VARCHAR(20),
ADD COLUMN IF NOT EXISTS gleif_category VARCHAR(50),
ADD COLUMN IF NOT EXISTS gleif_subcategory VARCHAR(50),
ADD COLUMN IF NOT EXISTS legal_form_code VARCHAR(10),
ADD COLUMN IF NOT EXISTS legal_form_text VARCHAR(200),
ADD COLUMN IF NOT EXISTS gleif_validation_level VARCHAR(30),
ADD COLUMN IF NOT EXISTS gleif_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS gleif_next_renewal DATE,
ADD COLUMN IF NOT EXISTS direct_parent_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS ultimate_parent_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS entity_creation_date DATE,
ADD COLUMN IF NOT EXISTS headquarters_address TEXT,
ADD COLUMN IF NOT EXISTS headquarters_city VARCHAR(200),
ADD COLUMN IF NOT EXISTS headquarters_country VARCHAR(3),
ADD COLUMN IF NOT EXISTS fund_manager_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS umbrella_fund_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS master_fund_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS is_fund BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS fund_type VARCHAR(30);

-- 2. Create entity_names table
CREATE TABLE IF NOT EXISTS "ob-poc".entity_names (
    name_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    name_type VARCHAR(50) NOT NULL,
    name TEXT NOT NULL,
    language VARCHAR(10),
    is_primary BOOLEAN DEFAULT FALSE,
    effective_from DATE,
    effective_to DATE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_entity_names_entity 
ON "ob-poc".entity_names(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_names_search 
ON "ob-poc".entity_names USING gin(to_tsvector('english', name));

-- 3. Create entity_addresses table
CREATE TABLE IF NOT EXISTS "ob-poc".entity_addresses (
    address_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    address_type VARCHAR(50) NOT NULL,
    language VARCHAR(10),
    address_lines TEXT[],
    city VARCHAR(200),
    region VARCHAR(50),
    country VARCHAR(3) NOT NULL,
    postal_code VARCHAR(50),
    is_primary BOOLEAN DEFAULT FALSE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_entity_addresses_entity 
ON "ob-poc".entity_addresses(entity_id);

-- 4. Create entity_identifiers table
CREATE TABLE IF NOT EXISTS "ob-poc".entity_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    identifier_type VARCHAR(30) NOT NULL,
    identifier_value VARCHAR(100) NOT NULL,
    issuing_authority VARCHAR(100),
    is_primary BOOLEAN DEFAULT FALSE,
    valid_from DATE,
    valid_until DATE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(entity_id, identifier_type, identifier_value)
);

CREATE INDEX IF NOT EXISTS idx_entity_identifiers_lookup 
ON "ob-poc".entity_identifiers(identifier_type, identifier_value);

-- 5. Create entity_parent_relationships table
CREATE TABLE IF NOT EXISTS "ob-poc".entity_parent_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    parent_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    parent_lei VARCHAR(20),
    relationship_type VARCHAR(50) NOT NULL,
    accounting_standard VARCHAR(20),
    relationship_start DATE,
    relationship_end DATE,
    validation_source VARCHAR(50),
    validation_reference TEXT,
    source VARCHAR(50) DEFAULT 'GLEIF',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_entity_parents_child 
ON "ob-poc".entity_parent_relationships(child_entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_parents_parent_lei 
ON "ob-poc".entity_parent_relationships(parent_lei);

-- 6. Create entity_lifecycle_events table
CREATE TABLE IF NOT EXISTS "ob-poc".entity_lifecycle_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL,
    event_status VARCHAR(30),
    effective_date DATE,
    recorded_date DATE,
    affected_fields JSONB,
    old_values JSONB,
    new_values JSONB,
    successor_lei VARCHAR(20),
    validation_documents VARCHAR(50),
    validation_reference TEXT,
    source VARCHAR(50) DEFAULT 'GLEIF',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_entity_events_entity 
ON "ob-poc".entity_lifecycle_events(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_events_type 
ON "ob-poc".entity_lifecycle_events(event_type);

COMMIT;
```

---

## DSL Verb Enhancement

### Current Entity Creation Verbs

Entity creation is handled via:
- `cbu.create` - Creates CBU with entities
- `entity.ensure-*` - Creates entities of specific types
- Role assignment verbs

Verb discovery uses YAML `invocation_phrases` and the learning system (`agent.invocation_phrases` table).

### Recommended New Verbs

```yaml
entity.enrich-from-gleif:
  description: "Fetch GLEIF data for an entity by LEI and populate extended fields"
  parameters:
    - lei: STRING (required)
    - entity_id: UUID (optional - link to existing)
  example_dsl: |
    entity.enrich-from-gleif lei:"529900K9B0N5BT694847"

entity.import-gleif-tree:
  description: "Import entire corporate tree from GLEIF starting from a name search"
  parameters:
    - search_name: STRING (required)
    - create_entities: BOOLEAN (default true)
    - import_relationships: BOOLEAN (default true)
  example_dsl: |
    entity.import-gleif-tree search_name:"Allianz" create_entities:true

entity.refresh-gleif:
  description: "Refresh GLEIF data for entities with LEIs, update changed fields"
  parameters:
    - entity_id: UUID (optional - specific entity)
    - stale_days: INTEGER (default 30 - refresh if older)
  example_dsl: |
    entity.refresh-gleif stale_days:7
```

---

## Implementation Priorities

### Phase 1: Schema Enhancement (1-2 days)
1. Run migration script
2. Update `entity_limited_companies` with new columns
3. Create supporting tables

### Phase 2: GLEIF Enrichment Service (2-3 days)
1. Extend existing `GleifScraper` to populate new fields
2. Create `gleif.enrich` DSL verb handler
3. Map GLEIF response to new schema fields

### Phase 3: Relationship Tracking (1-2 days)
1. Populate `entity_parent_relationships` from GLEIF
2. Create UBO chain query functions
3. Visualize in entity graph UI

### Phase 4: Continuous Refresh (1 day)
1. Scheduled job to refresh stale GLEIF data
2. Track `gleif_last_update` and `gleif_next_renewal`
3. Alert on expiring LEI registrations

---

## Summary: What You're Missing from Allianz

| Data Point | GLEIF Has | ob-poc Has | Gap |
|------------|-----------|------------|-----|
| LEI code | ✅ 529900K9B0N5BT694847 | ⚠️ In external_id (untyped) | Add `lei` column |
| Legal name | ✅ Allianz SE | ✅ company_name | None |
| Other names | ✅ Trading, transliterated | ❌ | Add `entity_names` table |
| Registration number | ✅ HRB 164232 | ✅ registration_number | None |
| Jurisdiction | ✅ DE | ✅ jurisdiction | None |
| Legal form code | ✅ MWBW (Europäische AG) | ❌ | Add `legal_form_code` |
| Legal address (structured) | ✅ Full address object | ⚠️ Single text field | Add `entity_addresses` |
| HQ address | ✅ Königinstr. 28, München | ❌ | Add columns/table |
| Entity status | ✅ ACTIVE | ❌ | Add `gleif_status` |
| Entity category | ✅ (null for Allianz SE) | ❌ | Add `gleif_category` |
| Direct parent | ✅ (none - it's the top) | ❌ | Add `direct_parent_lei` |
| Ultimate parent | ✅ (none - it's the top) | ❌ | Add `ultimate_parent_lei` |
| Child entities | ✅ ~500 subsidiaries | ❌ | Add relationship table |
| BIC codes | ✅ ALLIDEMM | ❌ | Add `entity_identifiers` |
| LEI renewal date | ✅ 2025-12-04 | ❌ | Add `gleif_next_renewal` |
| Validation level | ✅ FULLY_CORROBORATED | ❌ | Add `gleif_validation_level` |
| Corporate events | ✅ Name changes, etc | ❌ | Add `entity_lifecycle_events` |

---

## Appendix: Allianz Corporate Tree (from GLEIF)

The GLEIF API reveals Allianz SE (LEI: 529900K9B0N5BT694847) has ~500 related entities including:

- **Direct subsidiaries:** 
  - Allianz Technology SE
  - ALLIANZ GLOBAL CORPORATE & SPECIALTY SE
  - Allianz Europe B.V.
  - Allianz Finance II B.V.
  
- **Fund entities:**
  - Allianz Global Private Debt Opportunities Fund SCSp
  - Allianz Inflationsschutz
  - Multiple UCITS funds
  
- **Global offices:**
  - Allianz Life Insurance Japan Ltd.
  - Allianz Insurance Company - Egypt S.A.E.
  - ALLIANZ INVESTMENT MANAGEMENT SINGAPORE PTE. LTD.

Full tree can be scraped with existing `GleifScraper` and stored in enhanced schema.
