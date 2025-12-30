# Attribute/Document/Resource Dictionary Audit

## Executive Summary

**Architecture Assessment: SOLID FOUNDATION, UNDERUTILIZED**

You have a well-designed multi-table data dictionary architecture, but:
1. **Duplication**: Three overlapping tables (`attribute_registry`, `attribute_dictionary`, `dictionary`)
2. **Sparse mappings**: Only 22 document→attribute mappings for 194 document types
3. **Empty observations**: `attribute_observations` has 0 rows - the lineage isn't flowing
4. **Missing verbs**: No DSL verbs for dictionary management

| Component | Schema | Data | Usage |
|-----------|--------|------|-------|
| `attribute_registry` | ✅ Rich | ✅ 260 attrs | ✅ FK target |
| `attribute_dictionary` | ⚠️ Simpler | ⚠️ 22 attrs | ❓ Unclear |
| `dictionary` | ⚠️ Legacy? | ⚠️ 52 attrs | ⚠️ Source/sink JSONB |
| `document_types` | ✅ Excellent | ✅ 194 types | ✅ Comprehensive |
| `document_attribute_links` | ✅ Rich | ⚠️ 61 links | ⚠️ Sparse (16 doc types) |
| `document_attribute_mappings` | ⚠️ Simpler | ⚠️ 22 mappings | ⚠️ OCR/MRZ only |
| `attribute_observations` | ✅ Bi-temporal | ❌ 0 rows | ❌ Not flowing |
| `resource_attribute_requirements` | ✅ Good | ✅ 34 reqs | ✅ 8 resources |

---

## 1. Table Analysis

### 1.1 The Three Dictionaries 🔴 PROBLEM

You have THREE attribute definition tables:

```
┌────────────────────────────────────────────────────────────────────────────┐
│ attribute_registry (260 rows) - THE AUTHORITATIVE ONE                       │
│ ────────────────────────────────────────────────────────────────────────── │
│ id (text, e.g. "attr.address.city")                                        │
│ uuid (UUID) ← FK target for all relationships                              │
│ display_name, category, value_type, domain                                 │
│ validation_rules (jsonb), applicability (jsonb)                            │
│ embedding, embedding_model, embedding_updated_at                           │
│ reconciliation_rules, acceptable_variation_threshold                        │
│ requires_authoritative_source                                              │
└────────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────┐
│ dictionary (52 rows) - LEGACY? HAS SOURCE/SINK                             │
│ ────────────────────────────────────────────────────────────────────────── │
│ attribute_id (UUID), name, long_description                                │
│ group_id, domain, mask                                                     │
│ source (jsonb) ← {"type": "manual", "required": true, "pii": true}        │
│ sink (jsonb)   ← {"type": "database", "table": "entity_proper_persons"}   │
│ vector (text)                                                              │
└────────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────┐
│ attribute_dictionary (22 rows) - MINIMAL                                   │
│ ────────────────────────────────────────────────────────────────────────── │
│ attribute_id (UUID), attr_id (varchar), attr_name                          │
│ domain, data_type, description                                             │
│ validation_pattern, is_required, is_active                                 │
└────────────────────────────────────────────────────────────────────────────┘
```

**Issue**: `attribute_registry` is the FK target but `dictionary` has the lineage (source/sink). No clear merge strategy.

### 1.2 Document→Attribute Mapping Gap 🟡 SPARSE

Two mapping tables, both underpopulated:

| Table | Rows | Doc Types | Purpose |
|-------|------|-----------|---------|
| `document_attribute_links` | 61 | 16 | SOURCE/SINK direction, proof strength |
| `document_attribute_mappings` | 22 | 5 | Extraction method, field location |

**Coverage Problem**: 194 document types but only 16 have attribute links.

**Direction Analysis** (from `document_attribute_links`):
- SOURCE (45): Extract FROM document → attribute
- SINK (16): Attribute value proves document requirement

**Mapped Document Types** (only these 16 of 194):
```
ARTICLES_OF_ASSOCIATION, AUDITED_ACCOUNTS, CERT_OF_GOOD_STANDING,
CERT_OF_INCORPORATION, DRIVERS_LICENSE, LEI_CERTIFICATE,
NATIONAL_ID, PASSPORT, SOURCE_OF_FUNDS, SOURCE_OF_WEALTH,
TRUST_DEED, UBO_DECLARATION, UTILITY_BILL, W8_BEN
```

### 1.3 Observations Not Flowing 🔴 CRITICAL

```sql
SELECT COUNT(*) FROM "ob-poc".attribute_observations;  -- 0 rows!
```

The entire observation system is dormant:
- Verbs exist: `observation.record`, `observation.record-from-document`, `observation.reconcile`
- Table has rich schema (27 columns, bi-temporal, supersession)
- **Nothing is calling it**

### 1.4 Resource Requirements ✅ GOOD

8 resources with attribute requirements defined:

| Resource | Attr Count |
|----------|------------|
| CUSTODY_ACCT | 6 |
| NAV_ENGINE | 5 |
| IBOR_SYSTEM | 5 |
| SETTLE_ACCT | 5 |
| SWIFT_CONN | 4 |
| APAC_CLEAR | 3 |
| EUROCLEAR | 3 |
| DTCC_SETTLE | 3 |

---

## 2. Data Lineage Architecture

### Current Model (Partial)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         DATA LINEAGE FLOW                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SOURCES                        ATTRIBUTES                   SINKS          │
│  ───────                        ──────────                   ─────          │
│                                                                             │
│  ┌─────────────────┐           ┌───────────────┐                           │
│  │ document_catalog│ ─extract─>│ attribute_    │ ─persist─> attribute_     │
│  │ (uploaded docs) │           │ observations  │            values_typed   │
│  └─────────────────┘           │ (❌ 0 rows)   │                           │
│         │                      └───────────────┘                           │
│         │ type_id                    ▲                                     │
│         ▼                            │                                     │
│  ┌─────────────────┐                 │                                     │
│  │ document_types  │                 │                                     │
│  │ (194 types)     │                 │                                     │
│  └─────────────────┘                 │                                     │
│         │                            │                                     │
│         │ links                      │                                     │
│         ▼                            │                                     │
│  ┌─────────────────┐           ┌───────────────┐                           │
│  │ document_       │─attribute─│ attribute_    │                           │
│  │ attribute_links │    FK    >│ registry      │                           │
│  │ (61 links)      │           │ (260 attrs)   │                           │
│  └─────────────────┘           └───────────────┘                           │
│                                      │                                     │
│                                      │ FK                                  │
│                                      ▼                                     │
│                               ┌───────────────┐                           │
│                               │ resource_attr_│ ─required by─> resources  │
│                               │ requirements  │                           │
│                               └───────────────┘                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### What's Missing

1. **Extraction Pipeline**: Document → Observations flow not wired
2. **Lineage Metadata**: Source/sink config in `dictionary` not in `attribute_registry`
3. **Coverage**: Most documents don't know what attributes they provide

---

## 3. Rust Implementation Status

### Services Available

| Service | File | Status |
|---------|------|--------|
| `DictionaryServiceImpl` | `services/dictionary_service_impl.rs` | ✅ Implemented |
| `DocumentExtractionService` | `services/document_extraction_service.rs` | ✅ Implemented |
| `SinkExecutor` | `services/sink_executor.rs` | ✅ Implemented |

### DSL Verbs

| Domain | Verb | Status |
|--------|------|--------|
| `document` | `catalog` | ✅ Plugin |
| `document` | `extract` | ✅ Plugin |
| `document` | `extract-to-observations` | ✅ Plugin (handler: document_extract_observations) |
| `document` | `request/upload/waive` | ✅ Fire-and-forget pattern |
| `observation` | `record` | ✅ CRUD |
| `observation` | `record-from-document` | ✅ Plugin |
| `observation` | `supersede` | ✅ CRUD |
| `observation` | `reconcile` | ✅ Plugin |
| `trading-profile` | `import/materialize` | ✅ Plugin |
| `isda` | `create/add-coverage/add-csa` | ✅ CRUD |

### Missing Verbs 🔴

No verbs for:
- Dictionary management (`attribute.define`, `attribute.update-mapping`)
- Bulk document→attribute mapping
- Lineage discovery (`lineage.trace-attribute`, `lineage.show-sources`)

---

## 4. Document Types as Data Sources

### ISDA / Trading Matrix Treatment ✅ GOOD

You correctly treat ISDA and trading profiles as documents:

```yaml
# Document types for trading/ISDA
ISDA_MASTER, ISDA_SCHEDULE, ISDA_DEFINITIONS
CSA (VM_CSA, IM_CSA), GMRA, GMSLA, MNA
TRADING_PROFILE, TRADING_AUTHORITY, SSI_TEMPLATE
```

**Verbs support this**:
- `trading-profile.import` → `cbu_trading_profiles.document` (JSONB)
- `trading-profile.materialize` → Expands to operational tables
- `isda.create`, `isda.add-coverage`, `isda.add-csa`

### Attributes in Trading Profile Category

```sql
-- 11 ISDA attributes defined
SELECT id FROM attribute_registry WHERE category = 'isda';
-- attr.isda.governing_law, attr.isda.threshold, attr.isda.csa_type, etc.
```

---

## 5. Recommendations

### P0: Consolidate Dictionary Tables 🔴

**Decision needed**: One canonical source of truth

**Option A**: Merge into `attribute_registry`
- Add `source` and `sink` JSONB columns
- Migrate data from `dictionary`
- Deprecate `attribute_dictionary` and `dictionary`

**Option B**: Keep `dictionary` for lineage metadata
- Use `attribute_registry` for schema/validation
- Use `dictionary` for lineage config
- Link via UUID

**Suggested SQL for Option A**:
```sql
-- Add source/sink to attribute_registry
ALTER TABLE "ob-poc".attribute_registry 
ADD COLUMN source_config jsonb,
ADD COLUMN sink_config jsonb;

-- Migrate from dictionary
UPDATE "ob-poc".attribute_registry ar
SET source_config = d.source,
    sink_config = d.sink
FROM "ob-poc".dictionary d
WHERE ar.id = d.name;
```

### P0: Wire Observation Flow 🔴

Document extraction should create observations:

```rust
// In document.extract-to-observations handler:
// 1. Get document type
// 2. Look up attribute_links for that type
// 3. Extract values (OCR/AI/MRZ based on method)
// 4. For each extracted value:
//    - Call observation.record
//    - Set source_type = 'DOCUMENT_EXTRACTION'
//    - Set source_document_id = doc_id
```

### P1: Expand Document→Attribute Mappings 🟡

Only 16 of 194 document types have mappings. Priority coverage:

| Priority | Document Type | Expected Attributes |
|----------|---------------|---------------------|
| HIGH | CERTIFICATE_OF_INCORPORATION | legal_name, registration_number, incorporation_date, jurisdiction |
| HIGH | REGISTER_OF_SHAREHOLDERS | ownership_percentage, shareholder_name |
| HIGH | FINANCIAL_STATEMENT | revenue, assets, liabilities |
| HIGH | FUND_PROSPECTUS | fund_name, strategy, benchmark |
| HIGH | W8_BEN_E | tax_residence, entity_type, treaty_claim |
| MEDIUM | All TAX forms | tax_id, tax_residence, withholding_rate |
| MEDIUM | ISDA documents | governing_law, threshold, netting_provisions |

### P1: Add Dictionary Management Verbs 🟡

```yaml
# New verbs/attribute.yaml
domains:
  attribute:
    verbs:
      define:
        description: Define a new attribute in the registry
        behavior: crud
        crud:
          operation: insert
          table: attribute_registry
          schema: ob-poc
        args:
          - name: id
            type: string
            required: true
          - name: display-name
            type: string
            required: true
          - name: category
            type: string
            required: true
          - name: value-type
            type: string
            required: true
          - name: domain
            type: string
            required: false

      map-to-document:
        description: Map attribute to document type (source or sink)
        behavior: crud
        crud:
          operation: insert
          table: document_attribute_links
          schema: ob-poc
        args:
          - name: document-type
            type: lookup
            required: true
            lookup:
              table: document_types
              code_column: type_code
              id_column: type_id
          - name: attribute
            type: lookup
            required: true
            lookup:
              table: attribute_registry
              code_column: id
              id_column: uuid
          - name: direction
            type: string
            required: true
            valid_values: [SOURCE, SINK]
          - name: extraction-method
            type: string
            required: false
            valid_values: [OCR, AI, MRZ, IMAGE, MANUAL]
          - name: is-authoritative
            type: boolean
            required: false
            default: false

      trace-lineage:
        description: Show all sources and sinks for an attribute
        behavior: plugin
        handler: attribute_trace_lineage
```

### P2: Add Lineage Discovery View 🟢

```sql
CREATE VIEW "ob-poc".v_attribute_lineage AS
SELECT 
    ar.id as attribute_id,
    ar.display_name as attribute_name,
    ar.category,
    -- Sources (documents that provide this attribute)
    (SELECT jsonb_agg(jsonb_build_object(
        'document_type', dt.type_code,
        'extraction_method', dal.extraction_method,
        'is_authoritative', dal.is_authoritative
    ))
    FROM "ob-poc".document_attribute_links dal
    JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
    WHERE dal.attribute_id = ar.uuid AND dal.direction = 'SOURCE'
    ) as sources,
    -- Sinks (documents that require this attribute)
    (SELECT jsonb_agg(jsonb_build_object(
        'document_type', dt.type_code,
        'proof_strength', dal.proof_strength
    ))
    FROM "ob-poc".document_attribute_links dal
    JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
    WHERE dal.attribute_id = ar.uuid AND dal.direction = 'SINK'
    ) as sinks,
    -- Resources that need this attribute
    (SELECT jsonb_agg(jsonb_build_object(
        'resource', srt.resource_code,
        'is_mandatory', rar.is_mandatory
    ))
    FROM "ob-poc".resource_attribute_requirements rar
    JOIN "ob-poc".service_resource_types srt ON srt.resource_id = rar.resource_id
    WHERE rar.attribute_id = ar.uuid
    ) as required_by_resources
FROM "ob-poc".attribute_registry ar;
```

---

## 6. Summary

| Issue | Priority | Fix |
|-------|----------|-----|
| Three dictionary tables | **P0** | Consolidate to `attribute_registry` + source/sink |
| 0 observations flowing | **P0** | Wire `document.extract-to-observations` → `observation.record` |
| 178/194 doc types unmapped | **P1** | Bulk populate `document_attribute_links` |
| No dictionary verbs | **P1** | Add `attribute.define`, `attribute.map-to-document` |
| No lineage view | **P2** | Create `v_attribute_lineage` |

**The architecture is sound** - you have the tables, the Rust services, and the DSL verbs. The gap is **population and wiring**:
1. Most document types don't declare their attributes
2. Observations aren't being created from extractions
3. Dictionary management is manual (SQL) not DSL-driven
