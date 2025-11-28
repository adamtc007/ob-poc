# Sparse Matrix Attribute Model - Implementation Plan

## Current State

We now have the data model in place:

```
DOCUMENT TYPE HIERARCHY (with parent_type_code)
├── PASSPORT (abstract)
│   ├── PASSPORT_GBR, PASSPORT_USA, PASSPORT_DEU...
├── DRIVERS_LICENSE (abstract)
│   ├── DRIVERS_LICENSE_GBR, DRIVERS_LICENSE_DEU...
│   └── DRIVERS_LICENSE_USA (abstract - country level)
│       ├── DRIVERS_LICENSE_USA_CA, DRIVERS_LICENSE_USA_NY...

ATTRIBUTE_SOURCES (sparse matrix)
┌────────────────────────┬──────────┬─────────────────┬─────────────┬───────────────┐
│ Attribute              │ PASSPORT │ DRIVERS_LICENSE │ UTILITY_BILL│ BANK_STATEMENT│
├────────────────────────┼──────────┼─────────────────┼─────────────┼───────────────┤
│ person.full_name       │    ✓     │       ✓         │      ✓      │       ✓       │
│ person.date_of_birth   │    ✓     │       ✓         │             │               │
│ person.nationality     │    ✓     │                 │             │               │
│ person.residential_addr│          │       ✓         │      ✓      │       ✓       │
│ document.passport_no   │    ✓     │                 │             │               │
│ document.license_no    │          │       ✓         │             │               │
│ document.expiry_date   │    ✓     │       ✓         │             │               │
└────────────────────────┴──────────┴─────────────────┴─────────────┴───────────────┘

ATTRIBUTE_SINKS (where values flow)
- kyc_report
- sanctions_screening
- pep_screening
- jurisdiction_check
- regulatory_filing
- account_opening
```

## Required Changes

### 1. Document Extraction Service - Hierarchy Resolution

**File:** `rust/src/services/document_extraction_service.rs`

**Current Problem:** Queries `document_attribute_mappings` by exact `document_type_code`.
When we receive a `PASSPORT_GBR`, we need to find mappings for `PASSPORT` (the abstract parent).

**Change:**
```rust
/// Resolve document type to its root abstract category
/// PASSPORT_GBR → PASSPORT
/// DRIVERS_LICENSE_USA_CA → DRIVERS_LICENSE_USA → DRIVERS_LICENSE
async fn resolve_document_category(&self, document_type_code: &str) -> Result<String, String> {
    let result = sqlx::query!(
        r#"
        WITH RECURSIVE type_hierarchy AS (
            SELECT type_code, parent_type_code, 0 as depth
            FROM "ob-poc".document_types
            WHERE type_code = $1
            
            UNION ALL
            
            SELECT dt.type_code, dt.parent_type_code, th.depth + 1
            FROM "ob-poc".document_types dt
            JOIN type_hierarchy th ON dt.type_code = th.parent_type_code
        )
        SELECT type_code FROM type_hierarchy
        WHERE parent_type_code IS NULL
        "#,
        document_type_code
    )
    .fetch_one(&self.pool)
    .await?;
    
    Ok(result.type_code)
}
```

Then modify `get_attribute_mappings_for_doc_type()` to use resolved category:
```rust
// Before: Query exact type_code
// After: Query abstract category AND exact type_code (for type-specific overrides)
let category = self.resolve_document_category(document_type_code).await?;
// Query mappings where document_type_code IN (category, original_type_code)
// Priority: specific > abstract
```

---

### 2. New Service: Attribute Sources/Sinks Resolver

**New File:** `rust/src/services/attribute_routing_service.rs`

**Purpose:** Given an attribute, find all document types that can provide it (sources)
and all destinations that need it (sinks).

```rust
pub struct AttributeRoutingService {
    pool: PgPool,
}

impl AttributeRoutingService {
    /// Find all document categories that can provide this attribute
    /// Returns: ["PASSPORT", "DRIVERS_LICENSE", "UTILITY_BILL"]
    pub async fn get_document_sources(&self, attribute_name: &str) -> Result<Vec<DocumentSource>, String>;
    
    /// Find all sinks for this attribute
    /// Returns: [kyc_report, sanctions_screening, ...]
    pub async fn get_sinks(&self, attribute_name: &str) -> Result<Vec<AttributeSink>, String>;
    
    /// Given a document type, what attributes can we extract?
    /// Resolves hierarchy: PASSPORT_GBR → PASSPORT → attributes
    pub async fn get_extractable_attributes(&self, document_type_code: &str) -> Result<Vec<AttributeInfo>, String>;
    
    /// Given a required attribute, which documents should we request?
    /// Useful for "what documents do we need to complete KYC?"
    pub async fn get_required_documents_for_attribute(&self, attribute_name: &str) -> Result<Vec<String>, String>;
}

#[derive(Debug, Clone)]
pub struct DocumentSource {
    pub document_category: String,  // Abstract: PASSPORT, DRIVERS_LICENSE
    pub priority: i32,
    pub field_hints: Vec<String>,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AttributeSink {
    pub sink_type: String,  // kyc_report, sanctions_screening, etc.
    pub config: serde_json::Value,
}
```

---

### 3. Cross-Document Validation Service

**New File:** `rust/src/services/attribute_validation_service.rs`

**Purpose:** When the same attribute is extracted from multiple documents,
validate consistency. Flag discrepancies for review.

```rust
pub struct AttributeValidationService {
    pool: PgPool,
}

impl AttributeValidationService {
    /// Check if a new value is consistent with existing values
    /// Returns: Valid, Conflict(existing_values), or NeedsReview
    pub async fn validate_consistency(
        &self,
        cbu_id: Uuid,
        attribute_name: &str,
        new_value: &Value,
        source_document_id: Uuid,
    ) -> Result<ValidationResult, String>;
    
    /// Get all values for an attribute across all source documents
    pub async fn get_all_values_with_provenance(
        &self,
        cbu_id: Uuid,
        attribute_name: &str,
    ) -> Result<Vec<AttributeValueWithProvenance>, String>;
}

#[derive(Debug)]
pub enum ValidationResult {
    Valid,                           // Matches existing or first value
    Conflict(Vec<ConflictDetail>),   // Values differ - flag for review
    MultipleAllowed(Vec<Value>),     // e.g., nationality for dual nationals
}

#[derive(Debug)]
pub struct AttributeValueWithProvenance {
    pub value: Value,
    pub source_document_id: Uuid,
    pub source_document_type: String,
    pub extracted_at: DateTime<Utc>,
    pub confidence: f64,
}
```

**Validation Rules** (from `consolidated_attributes.cross_document_validation`):
- `FULL_NAME`: MUST match across all documents
- `DATE_OF_BIRTH`: MUST match (immutable)
- `NATIONALITY`: Multiple values allowed (dual nationals)

---

### 4. Schema Changes

**New table:** `attribute_value_conflicts`
```sql
CREATE TABLE "ob-poc".attribute_value_conflicts (
    conflict_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    
    -- The conflicting values
    value_1 JSONB NOT NULL,
    document_1_id UUID REFERENCES "ob-poc".document_catalog(document_id),
    
    value_2 JSONB NOT NULL,
    document_2_id UUID REFERENCES "ob-poc".document_catalog(document_id),
    
    -- Resolution
    status VARCHAR(20) DEFAULT 'pending',  -- pending, resolved, overridden
    resolved_value JSONB,
    resolved_by VARCHAR(255),
    resolution_notes TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);
```

---

### 5. Integration with DSL v2

**Custom Operations to Add:**

```clojure
;; Request documents needed for missing attributes
(kyc.request-documents :cbu-id @cbu :missing-attributes [:nationality :residential-address])

;; Validate attribute consistency across all documents
(kyc.validate-attributes :cbu-id @cbu)

;; Get attribute value with provenance chain
(attribute.get-with-provenance :cbu-id @cbu :attribute "person.full_name")
```

---

### 6. Updated Extraction Flow

```
Document Upload (e.g., UK Passport)
         │
         ▼
┌─────────────────────────────────────────┐
│ 1. Identify document type: PASSPORT_GBR │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│ 2. Resolve hierarchy: PASSPORT_GBR      │
│    → parent: PASSPORT                    │
│    → root category: PASSPORT             │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│ 3. Query attribute_sources WHERE        │
│    document_category = 'PASSPORT'        │
│    → person.full_name                    │
│    → person.date_of_birth                │
│    → person.nationality                  │
│    → document.passport_number            │
│    → document.expiry_date                │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│ 4. Extract values using field_hints     │
│    (country-specific from mappings)      │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│ 5. Cross-document validation            │
│    - Does name match other docs?         │
│    - Flag conflicts for review           │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│ 6. Store with provenance                │
│    - document_metadata (per-doc)         │
│    - attribute_values_typed (canonical)  │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│ 7. Route to sinks                       │
│    - kyc_report                          │
│    - sanctions_screening                 │
│    - regulatory_filing                   │
└─────────────────────────────────────────┘
```

---

## Implementation Order

1. **Phase 1: Hierarchy Resolution** (document_extraction_service.rs)
   - Add `resolve_document_category()` 
   - Update extraction to use resolved category
   - Test with PASSPORT_GBR → extracts PASSPORT attributes

2. **Phase 2: Attribute Routing Service** (new file)
   - Create `AttributeRoutingService`
   - Query attribute_sources/sinks
   - "What documents provide person.full_name?"

3. **Phase 3: Cross-Document Validation** (new file)
   - Create `AttributeValidationService`
   - Add `attribute_value_conflicts` table
   - Validation on extraction

4. **Phase 4: DSL Integration**
   - Add custom ops for validation/routing
   - Integrate into onboarding workflows

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `services/document_extraction_service.rs` | Modify | Add hierarchy resolution |
| `services/attribute_routing_service.rs` | **NEW** | Sources/sinks resolution |
| `services/attribute_validation_service.rs` | **NEW** | Cross-document validation |
| `database/attribute_conflict_repository.rs` | **NEW** | Conflict storage |
| `dsl_v2/custom_ops/kyc_ops.rs` | **NEW** | KYC DSL operations |
| `sql/migrations/add_attribute_conflicts.sql` | **NEW** | Conflicts table |

---

## Test Scenarios

1. **Hierarchy Resolution**
   - Upload PASSPORT_GBR → extracts person.full_name, person.date_of_birth, etc.
   - Upload DRIVERS_LICENSE_USA_CA → extracts same person.* attributes

2. **Cross-Document Validation**
   - Upload UK passport with name "John Smith"
   - Upload US passport with name "John Smith" → Valid (matches)
   - Upload utility bill with name "J. Smith" → Conflict flagged

3. **Dual Nationality**
   - Upload UK passport → nationality: GBR
   - Upload US passport → nationality: USA
   - Both values stored (MultipleAllowed)

4. **Missing Document Routing**
   - Required: person.residential_address
   - Query: "What documents can provide this?"
   - Result: UTILITY_BILL, BANK_STATEMENT, DRIVERS_LICENSE
