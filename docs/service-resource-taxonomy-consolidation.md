# Service Resource Taxonomy Consolidation Plan

## Current State Analysis

### The Triangle: Product → Service → Resource

```
┌─────────────────────────────────────────────────────────────────┐
│  PRODUCT (What we sell)                                         │
│  e.g., "Global Custody", "Fund Administration"                  │
│  Table: ob-poc.products                                         │
└─────────────────────────────────────────────────────────────────┘
              │
              │ product_services (M:N junction)
              ▼
┌─────────────────────────────────────────────────────────────────┐
│  SERVICE (What we deliver)                                      │
│  e.g., "Trade Settlement", "NAV Calculation"                    │
│  Table: ob-poc.services                                         │
└─────────────────────────────────────────────────────────────────┘
              │
              │ service_resource_capabilities (M:N junction)
              ▼
┌─────────────────────────────────────────────────────────────────┐
│  SERVICE_RESOURCE_TYPE (How we deliver it)                      │
│  e.g., "DTCC Settlement System", "Custody Account"              │
│  Table: ob-poc.service_resource_types                           │
│                                                                  │
│  Defines: API endpoint, auth, capabilities, capacity limits     │
│  Has: resource_attribute_requirements (what attrs are needed)   │
└─────────────────────────────────────────────────────────────────┘
              │
              │ FK: resource_type_id
              ▼
┌─────────────────────────────────────────────────────────────────┐
│  CBU_RESOURCE_INSTANCE (Provisioned for a client)               │
│  e.g., "Apex Capital's DTCC connection"                         │
│  Table: ob-poc.cbu_resource_instances                           │
│                                                                  │
│  Has: resource_instance_attributes (actual values)              │
│  Lifecycle: PENDING → ACTIVE → SUSPENDED → DECOMMISSIONED       │
└─────────────────────────────────────────────────────────────────┘
```

### Current Problems

#### 1. Attribute Dictionary Fragmentation (3 overlapping tables)

| Table | Records | Purpose | Used By |
|-------|---------|---------|---------|
| dictionary | 52 | Resource attributes | resource_attribute_requirements, resource_instance_attributes |
| attribute_registry | 59 | Document/entity attributes | attribute_values_typed, document_attribute_mappings |
| attribute_dictionary | 22 | DSL validation | Nothing (orphaned?) |

**Problem**: Same concept (attribute definitions) split across 3 tables with different schemas.

#### 2. Duplicate Resource Type Tables

| Table | Schema | Records | Purpose |
|-------|--------|---------|---------|
| service_resource_types | ob-poc | 13 | Service delivery resources |
| resource_types | public | 3 | API/endpoint definitions |

**Problem**: public.resource_types appears to be an older/experimental table.

#### 3. Missing Relationships

- document_catalog has cbu_id but no entity_id - cannot link docs to specific entities
- cbu_resource_instances.service_id is always NULL - no service context
- No FK from resource_attribute_requirements to attribute_registry

#### 4. No Attribute Validation Verb

- service-resource.provision creates instance
- service-resource.set-attr sets values
- service-resource.activate changes status
- **Missing**: service-resource.validate-attrs to check readiness before activation

---

## Consolidation Plan

### Phase 1: Unify Attribute Dictionary

**Goal**: Single source of truth for all attribute definitions.

#### Step 1.1: Migrate to attribute_registry as canonical table

attribute_registry has the richest schema:
- UUID + string ID
- Category, value_type with constraints
- Validation rules (jsonb)
- Applicability (jsonb) - for entity/document/resource scoping
- Embedding support for semantic search

#### Step 1.2: Add missing columns to attribute_registry

```sql
ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS domain VARCHAR(100),
ADD COLUMN IF NOT EXISTS is_required BOOLEAN DEFAULT false,
ADD COLUMN IF NOT EXISTS default_value TEXT,
ADD COLUMN IF NOT EXISTS group_id VARCHAR(100);
```

#### Step 1.3: Migrate dictionary data

```sql
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, group_id)
SELECT 
  name,
  attribute_id,
  name,
  COALESCE(domain, 'resource'),
  LOWER(COALESCE(mask, 'string')),
  domain,
  group_id
FROM "ob-poc".dictionary
ON CONFLICT (id) DO NOTHING;
```

#### Step 1.4: Update FKs

```sql
ALTER TABLE "ob-poc".resource_attribute_requirements
DROP CONSTRAINT resource_attribute_requirements_attribute_id_fkey,
ADD CONSTRAINT resource_attribute_requirements_attribute_uuid_fkey 
  FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(uuid);

ALTER TABLE "ob-poc".resource_instance_attributes
DROP CONSTRAINT resource_instance_attributes_attribute_id_fkey,
ADD CONSTRAINT resource_instance_attributes_attribute_uuid_fkey 
  FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(uuid);
```

#### Step 1.5: Drop redundant tables

```sql
DROP TABLE "ob-poc".dictionary;
DROP TABLE "ob-poc".attribute_dictionary;
DROP TABLE "ob-poc".attribute_uuid_map;
```

---

### Phase 2: Clean Up Resource Types

#### Step 2.1: Audit public.resource_types usage

Check if anything uses it beyond the 3 seed records.

#### Step 2.2: Drop or migrate public schema tables

```sql
DROP TABLE IF EXISTS public.resource_type_endpoints;
DROP TABLE IF EXISTS public.resource_type_attributes;
DROP TABLE IF EXISTS public.actions_registry;
DROP TABLE IF EXISTS public.resource_types;
```

---

### Phase 3: Add entity_id to document_catalog

```sql
ALTER TABLE "ob-poc".document_catalog
ADD COLUMN entity_id UUID REFERENCES "ob-poc".entities(entity_id);

CREATE INDEX idx_document_catalog_entity ON "ob-poc".document_catalog(entity_id);
```

Update DSL verb document.catalog to include entity-id argument.

---

### Phase 4: Fix cbu_resource_instances.service_id

#### Step 4.1: Populate service_id from resource type

```sql
UPDATE "ob-poc".cbu_resource_instances cri
SET service_id = src.service_id
FROM "ob-poc".service_resource_capabilities src
WHERE cri.resource_type_id = src.resource_id
AND cri.service_id IS NULL;
```

#### Step 4.2: Update provision verb to require service context

Make service-id required in service-resource.provision.

---

### Phase 5: Add validate-attrs Verb

Add service-resource.validate-attrs verb:
- Takes instance-id
- Returns { valid: bool, missing: [...], errors: [...] }
- Checks all mandatory attributes from resource_attribute_requirements are set

Update activate to auto-validate and fail if not ready.

---

### Phase 6: Documentation

Add Service Resource Taxonomy section to CLAUDE.md with the triangle diagram.

---

## Migration Order

1. Phase 1 - Attribute consolidation (data migration, FK updates)
2. Phase 3 - Add entity_id to document_catalog (schema only)
3. Phase 4 - Fix service_id on instances (data + verb update)
4. Phase 5 - Add validate-attrs verb (new code)
5. Phase 2 - Drop public.resource_types (cleanup after verification)
6. Phase 6 - Documentation

## Rollback Plan

Each phase is independent with its own rollback:
- Phase 1: Keep dictionary tables until verified
- Phase 3: entity_id is nullable, no data loss
- Phase 4: service_id was already nullable
- Phase 5: New verb, just remove if issues
