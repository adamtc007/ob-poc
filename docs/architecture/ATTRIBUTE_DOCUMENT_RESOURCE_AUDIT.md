# Attribute / Document / Resource Dictionary Deep Review

## Executive Summary

**Assessment: SOLID FOUNDATION, PARTIAL IMPLEMENTATION**

You have a sophisticated data lineage architecture that could be first-class infrastructure. The schema design is excellent. The execution gaps are in:

| Component | Schema | Data | Verbs | Handlers |
|-----------|--------|------|-------|----------|
| Attribute Registry | ✅ 260 attrs | ✅ | ❌ No CRUD | N/A |
| Attribute Dictionary | ⚠️ Simpler | 52 rows | ❌ No verbs | N/A |
| Document Types | ✅ Rich | ✅ 194 types | ✅ | ✅ |
| Document→Attribute Links | ✅ | 61 links | ⚠️ Partial | ❌ Missing |
| Observations | ✅ Excellent | **0 rows** | ✅ | ⚠️ Partial |
| Trading Profiles | ✅ | 0 rows | ✅ | ❌ Missing |
| Resources→Attributes | ✅ | 34 mappings | ⚠️ Read-only | ⚠️ Partial |

---

## 1. The Attribute Ecosystem (Two Tables)

### 1.1 `attribute_registry` (260 rows) - THE CANONICAL SOURCE

**Schema Strengths:**
```
id                              -- "attr.identity.legal_name" format
display_name                    -- Human readable
category                        -- identity, entity, financial, compliance
value_type                      -- string, number, date, boolean, json
validation_rules                -- JSONB validation
applicability                   -- JSONB entity type filters
embedding                       -- pgvector for semantic search
reconciliation_rules            -- Conflict resolution rules
acceptable_variation_threshold  -- Tolerance for discrepancies
requires_authoritative_source   -- Must come from authoritative doc
```

**Sample Categories:**
- `identity`: first_name, last_name, passport_number, etc.
- `entity`: legal_name, domicile, registration_number
- `financial`: net_worth, annual_income, revenue
- `compliance`: source_of_wealth, source_of_funds
- `resource.*`: Namespaced resource attributes

**Gap: No CRUD verbs** for managing the registry itself.

### 1.2 `attribute_dictionary` (52 rows) - LINEAGE METADATA

**Schema:**
```
source    -- JSONB: {"type": "manual", "required": true, "pii": true}
sink      -- JSONB: {"type": "database", "table": "entity_proper_persons"}
vector    -- Lineage path?
```

**Gap: Only 16 of 52 have source/sink populated.**

### Recommendation: Consolidate or Clarify

You have TWO attribute catalogs. Options:
1. **Merge**: Add source/sink columns to `attribute_registry`
2. **Foreign Key**: `attribute_dictionary.attribute_id` → `attribute_registry.uuid`
3. **Deprecate**: Phase out `attribute_dictionary`, move lineage to registry

---

## 2. Document → Attribute Flow

### 2.1 Document Types (194 defined)

Rich categorization including ISDA:
```
ISDA_MASTER, ISDA_SCHEDULE, ISDA_DEFINITIONS
ISDA_LEGAL_OPINION, ISDA_DF_PROTOCOL, ISDA_EMIR_PROTOCOL
CSA_AGREEMENT, EMIR_CLASSIFICATION
```

Plus custody, fund, banking, KYC document types.

### 2.2 Document → Attribute Links (61 mappings)

**Two tables serve similar purposes:**

| Table | Rows | Purpose |
|-------|------|---------|
| `document_attribute_mappings` | 22 | Extraction field mappings |
| `document_attribute_links` | 61 | Semantic relationships + direction |

**`document_attribute_links` is richer:**
```sql
direction                     -- SOURCE (extract) or SINK (verify)
is_authoritative              -- Is this THE golden source?
proof_strength                -- PRIMARY, SECONDARY, SUPPORTING
alternative_doc_types         -- Other docs that could provide this
entity_types                  -- Which entities this applies to
```

**Current Coverage:**
- 45 SOURCE links (extract from doc)
- 16 SINK links (verify against doc)

**Gap: No ISDA document→attribute mappings defined!**

Example of what's missing:
```yaml
# ISDA_MASTER → attributes
- ISDA_MASTER:
    SOURCE:
      - attr.isda.governing_law
      - attr.isda.agreement_date
      - attr.counterparty.legal_name
      - attr.counterparty.lei
    SINK:
      - attr.entity.legal_name  # Verify our entity name matches
```

---

## 3. The Observation System (UNUSED)

### 3.1 Schema (Excellent Design)

```sql
-- attribute_observations
entity_id           -- Which entity
attribute_id        -- Which attribute  
value_text/number/boolean/date/json  -- Typed values!
source_type         -- Where it came from
source_document_id  -- Link to document
confidence          -- Extraction confidence
is_authoritative    -- Golden source flag
extraction_method   -- OCR, MRZ, API, MANUAL
superseded_by/at    -- Version tracking
effective_from/to   -- Temporal validity
```

### 3.2 Verbs Exist But Handlers Missing

| Verb | Behavior | Handler Status |
|------|----------|----------------|
| `observation.record` | CRUD | ✅ Works |
| `observation.record-from-document` | Plugin | ❌ `observation_from_document` NOT IMPLEMENTED |
| `observation.supersede` | CRUD | ✅ Works |
| `observation.get-current` | Plugin | ❌ `observation_get_current` NOT IMPLEMENTED |
| `observation.reconcile` | Plugin | ❌ `observation_reconcile` NOT IMPLEMENTED |
| `observation.verify-allegations` | Plugin | ❌ NOT IMPLEMENTED |

### 3.3 Current State: 0 Rows

```sql
SELECT COUNT(*) FROM "ob-poc".attribute_observations;
-- 0
```

**This means:**
- Document extraction isn't creating observations
- No audit trail of attribute values
- No multi-source reconciliation
- No lineage from document → attribute value → entity

---

## 4. Trading Profiles & ISDA (Correct Architecture)

### 4.1 Trading Profile as Document

Your architecture correctly treats trading profiles as versioned JSONB documents:

```sql
-- cbu_trading_profiles
document              -- JSONB: Full trading config
document_hash         -- For change detection
version               -- Version number
status                -- DRAFT, PENDING_REVIEW, ACTIVE
materialization_status -- How it's deployed
```

**Verbs exist:**
- `trading-profile.import` - Import YAML/JSON
- `trading-profile.activate` - Set as active
- `trading-profile.materialize` - Deploy to operational tables
- `trading-profile.diff` - Compare versions
- `trading-profile.validate` - Validation

**Gap: Plugin handlers not implemented.**

### 4.2 ISDA as First-Class Entity

Schema and verbs exist in `custody` schema:
```
isda.create           -- Create master agreement
isda.add-coverage     -- Add instrument class
isda.add-csa          -- Add Credit Support Annex
isda.list             -- List for CBU
```

**Current State:** Tables exist, verbs exist, no data loaded.

---

## 5. Resource → Attribute Requirements

### 5.1 Current Mapping (34 requirements)

Resources require specific attributes:
```
Custody Account    → account_number, account_name, base_currency
NAV Calculation    → fund_code, valuation_frequency, pricing_source
IBOR System        → portfolio_code, accounting_basis, position_source
```

### 5.2 Instance Attributes (0 rows)

```sql
SELECT COUNT(*) FROM "ob-poc".resource_instance_attributes;
-- 0
```

**Despite 634 resource instances, none have attribute values!**

The verb `service-resource.set-attr` exists but isn't being used during provisioning.

---

## 6. Critical Gaps Summary

### P0: Core Pipeline Broken

| Gap | Impact | Fix Effort |
|-----|--------|------------|
| `observation_from_document` handler missing | Cannot extract attributes from documents | 2-3 hours |
| `observation_get_current` handler missing | Cannot query current attribute values | 1 hour |
| Zero observations | No data lineage | Schema ready, need flow |
| Resource instance attrs empty | Cannot validate resource requirements | 1 hour |

### P1: Missing Mappings

| Gap | Impact | Fix Effort |
|-----|--------|------------|
| No ISDA→attribute mappings | Can't extract from ISDA docs | 2 hours (data) |
| No trading profile→attribute mappings | Can't link profile sections to attrs | 2 hours (data) |
| Dictionary source/sink incomplete | Lineage tracking broken | 2 hours (data) |

### P2: Missing Verbs

| Gap | Impact | Fix Effort |
|-----|--------|------------|
| No `attribute.` domain verbs | Cannot manage attribute registry via DSL | 1 hour |
| No `dictionary.` domain verbs | Cannot manage lineage via DSL | 1 hour |

---

## 7. Recommended Fix Sequence

### Phase 1: Enable Observation Pipeline (Day 1)

```rust
// 1. Implement observation_from_document handler
// rust/src/plugins/observation.rs

pub async fn observation_from_document(
    ctx: &ExecutionContext,
    args: &VerbArgs,
) -> Result<StackValue> {
    let entity_id: Uuid = args.get_uuid("entity-id")?;
    let document_id: Uuid = args.get_uuid("document-id")?;
    let attr_code: &str = args.get_str("attribute")?;
    let value: &str = args.get_str("value")?;
    
    // Lookup attribute
    let attr = sqlx::query_as::<_, AttributeRow>(
        r#"SELECT uuid, value_type FROM "ob-poc".attribute_registry WHERE id = $1"#
    )
    .bind(attr_code)
    .fetch_one(&ctx.pool)
    .await?;
    
    // Create observation with typed value
    let obs_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO "ob-poc".attribute_observations (
            entity_id, attribute_id, value_text, source_type, 
            source_document_id, extraction_method, is_authoritative
        ) VALUES ($1, $2, $3, 'DOCUMENT', $4, $5, 
            (SELECT is_authoritative FROM "ob-poc".document_attribute_links 
             WHERE document_type_id = (SELECT type_id FROM "ob-poc".document_catalog WHERE doc_id = $4)
             AND attribute_id = $2))
        RETURNING observation_id"#
    )
    .bind(entity_id)
    .bind(attr.uuid)
    .bind(value)
    .bind(document_id)
    .bind(args.get_str("extraction-method").unwrap_or("MANUAL"))
    .fetch_one(&ctx.pool)
    .await?;
    
    Ok(StackValue::Uuid(obs_id))
}
```

### Phase 2: Populate Document→Attribute Mappings (Day 1)

```sql
-- Add ISDA document→attribute links
INSERT INTO "ob-poc".document_attribute_links (
    document_type_id, attribute_id, direction, 
    is_authoritative, proof_strength
)
SELECT 
    dt.type_id,
    ar.uuid,
    'SOURCE',
    true,
    'PRIMARY'
FROM "ob-poc".document_types dt
CROSS JOIN "ob-poc".attribute_registry ar
WHERE dt.type_code = 'ISDA_MASTER'
  AND ar.id IN (
    'attr.isda.governing_law',
    'attr.isda.agreement_date',
    'attr.counterparty.legal_name'
  );

-- Add Trading Profile→attribute links
-- ... similar pattern
```

### Phase 3: Resource Instance Attributes (Day 2)

```rust
// In resource_instance_create handler, after creating instance:

// Auto-populate required attributes from config
for attr_req in resource_type.attribute_requirements {
    if let Some(default_value) = &attr_req.default_value {
        sqlx::query(
            r#"INSERT INTO "ob-poc".resource_instance_attributes 
               (instance_id, attribute_id, value, state, observed_at)
               VALUES ($1, $2, $3, 'system', NOW())"#
        )
        .bind(instance_id)
        .bind(attr_req.attribute_id)
        .bind(default_value)
        .execute(&ctx.pool)
        .await?;
    }
}
```

### Phase 4: Attribute Registry Verbs (Day 2)

```yaml
# verbs/attribute.yaml
domains:
  attribute:
    description: Attribute registry management
    verbs:
      list:
        description: List attributes by category or domain
        behavior: crud
        crud:
          operation: select
          table: attribute_registry
          schema: ob-poc
        args:
          - name: category
            type: string
            required: false
          - name: domain
            type: string
            required: false

      get:
        description: Get attribute by ID
        behavior: crud
        crud:
          operation: select
          table: attribute_registry
          schema: ob-poc
        args:
          - name: attr-id
            type: string
            required: true
            maps_to: id

      search:
        description: Semantic search for attributes
        behavior: plugin
        handler: attribute_semantic_search
        args:
          - name: query
            type: string
            required: true
          - name: limit
            type: integer
            default: 10
```

---

## 8. Data Flow Vision

Once implemented, the flow should be:

```
┌─────────────────┐
│   Document      │  (passport, ISDA, trading profile)
│   Uploaded      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ document.extract│  Uses document_attribute_mappings
│  -to-observations│  to know WHAT to extract
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ attribute_      │  Multi-source observations
│ observations    │  with confidence, lineage
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ observation.    │  Finds discrepancies between
│ reconcile       │  document vs allegation vs screening
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ observation_    │  Flags for manual review
│ discrepancies   │
└─────────────────┘
```

For ISDA/Trading Matrix:
```
┌─────────────────┐
│ trading-profile │  YAML/JSON document
│ .import         │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ cbu_trading_    │  Versioned document store
│ profiles        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ trading-profile │  Creates operational records
│ .materialize    │  (SSIs, universe, booking rules)
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────────────┐
│ isda_agreements │ ssi_instructions │ etc.   │
│ (custody schema)│                  │        │
└─────────────────────────────────────────────┘
```

---

## 9. Immediate Actions

**Today (4 hours):**
1. ✅ Review complete (this document)
2. 🔧 Implement `observation_from_document` handler
3. 🔧 Implement `observation_get_current` handler
4. 📊 Add 20 ISDA→attribute mappings to document_attribute_links

**This Week:**
5. 📊 Add trading profile→attribute mappings
6. 🔧 Wire resource provisioning to set instance attributes
7. 🔧 Implement trading-profile plugin handlers
8. 📊 Complete dictionary source/sink for remaining 36 attributes

**Validation Query (after fixes):**
```sql
-- Should have observations
SELECT COUNT(*) FROM "ob-poc".attribute_observations;

-- Should have resource instance attributes  
SELECT COUNT(*) FROM "ob-poc".resource_instance_attributes;

-- Should have ISDA attribute links
SELECT COUNT(*) 
FROM "ob-poc".document_attribute_links dal
JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
WHERE dt.type_code LIKE 'ISDA%';
```
