# KYC DSL Lifecycle Implementation Plan

## Overview

Implement the complete KYC/UBO lifecycle as a DSL-driven state machine with:
- Threshold decision matrix (computes requirements from CBU attributes + product risk)
- RFI generation and tracking (references document dictionary)
- UBO chain analysis and completeness checking
- Event-driven evaluation with HITL override

## Reference Documents

- `/docs/CBU_ENTITY_GRAPH_SPEC.md` - CBU visualization
- `/docs/OBSERVATION_MODEL_REFACTOR.md` - Observation system
- `/sql/seeds/document_types.sql` - 181 document types
- `/sql/seeds/document_validity_rules.sql` - Expiry/renewal rules

---

## Phase 1: DSL Argument Type Extensions

### 1.1 Add DocumentTypeCode Argument Type

**Location**: DSL parser/validator (wherever argument types are defined)

The RFI verbs reference document types from the `document_types` table. Need a validated argument type.

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ArgumentType {
    // ... existing types ...
    
    /// Single document type code - validates against document_types.type_code
    DocumentTypeCode,
    
    /// List of document type codes
    DocumentTypeList,
    
    /// Attribute code for threshold checking (identity, address, source_of_wealth, etc.)
    AttributeCode,
    
    /// Risk band enum
    RiskBand,
    
    /// RFI type enum
    RfiType,
}
```

### 1.2 Document Type Validation

At DSL parse/compile time, validate document type codes exist:

```rust
impl DslValidator {
    fn validate_document_type(&self, code: &str) -> Result<(), DslError> {
        // Query document_types table or use cached set
        if !self.document_types.contains(code) {
            return Err(DslError::InvalidDocumentType {
                code: code.to_string(),
                hint: self.suggest_similar_document_type(code),
            });
        }
        Ok(())
    }
}
```

### 1.3 Load Document Types at Startup

```rust
pub struct DslContext {
    // ... existing fields ...
    
    /// Valid document type codes (from document_types table)
    pub document_types: HashSet<String>,
    
    /// Document type metadata (category, typical attributes proved)
    pub document_type_info: HashMap<String, DocumentTypeInfo>,
}

pub struct DocumentTypeInfo {
    pub type_code: String,
    pub category: DocumentCategory,
    pub proves_attributes: Vec<String>,  // What this doc type typically proves
    pub typical_validity_days: Option<i32>,
}

impl DslContext {
    pub async fn load_document_types(pool: &PgPool) -> Result<Self, Error> {
        let types = sqlx::query_as!(
            DocumentTypeRow,
            r#"SELECT type_code, category, description 
               FROM "ob-poc".document_types 
               WHERE is_active = true"#
        ).fetch_all(pool).await?;
        
        // Build lookup sets
        // ...
    }
}
```

---

## Phase 2: Threshold Decision Matrix

### 2.1 Database Schema

```sql
-- Threshold matrix configuration
CREATE TABLE "ob-poc".threshold_factors (
    factor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    factor_type VARCHAR(50) NOT NULL,  -- 'CBU_TYPE', 'SOURCE_OF_FUNDS', 'NATURE_PURPOSE', 'JURISDICTION'
    factor_code VARCHAR(50) NOT NULL,
    risk_weight INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    UNIQUE(factor_type, factor_code)
);

-- Risk bands derived from composite score
CREATE TABLE "ob-poc".risk_bands (
    band_code VARCHAR(20) PRIMARY KEY,  -- 'LOW', 'MEDIUM', 'HIGH', 'ENHANCED'
    min_score INTEGER NOT NULL,
    max_score INTEGER NOT NULL,
    description TEXT
);

-- Requirements per entity role + risk band
CREATE TABLE "ob-poc".threshold_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_role VARCHAR(50) NOT NULL,       -- 'UBO', 'DIRECTOR', 'SHAREHOLDER', etc.
    risk_band VARCHAR(20) NOT NULL REFERENCES "ob-poc".risk_bands(band_code),
    attribute_code VARCHAR(50) NOT NULL,    -- 'identity', 'address', 'source_of_wealth'
    is_required BOOLEAN NOT NULL DEFAULT true,
    confidence_min NUMERIC(3,2) DEFAULT 0.85,
    max_age_days INTEGER,                   -- For document freshness
    must_be_authoritative BOOLEAN DEFAULT false,
    notes TEXT,
    UNIQUE(entity_role, risk_band, attribute_code)
);

-- Acceptable document types per requirement
CREATE TABLE "ob-poc".requirement_acceptable_docs (
    requirement_id UUID REFERENCES "ob-poc".threshold_requirements(requirement_id),
    document_type_code VARCHAR(50) REFERENCES "ob-poc".document_types(type_code),
    priority INTEGER DEFAULT 1,  -- Preference order
    PRIMARY KEY (requirement_id, document_type_code)
);

-- Screening requirements per risk band
CREATE TABLE "ob-poc".screening_requirements (
    risk_band VARCHAR(20) REFERENCES "ob-poc".risk_bands(band_code),
    screening_type VARCHAR(50) NOT NULL,  -- 'SANCTIONS', 'PEP', 'ADVERSE_MEDIA'
    is_required BOOLEAN NOT NULL DEFAULT true,
    PRIMARY KEY (risk_band, screening_type)
);
```

### 2.2 Seed Data

```sql
-- Risk bands
INSERT INTO "ob-poc".risk_bands (band_code, min_score, max_score, description) VALUES
('LOW', 0, 3, 'Low risk - standard due diligence'),
('MEDIUM', 4, 6, 'Medium risk - enhanced monitoring'),
('HIGH', 7, 9, 'High risk - enhanced due diligence'),
('ENHANCED', 10, 99, 'Enhanced risk - senior management approval required');

-- CBU type factors
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('CBU_TYPE', 'LUXSICAV_UCITS', 1, 'Luxembourg SICAV - UCITS regulated'),
('CBU_TYPE', 'LUXSICAV_PART2', 2, 'Luxembourg SICAV - Part II (less regulated)'),
('CBU_TYPE', 'HEDGE_FUND', 3, 'Hedge fund - higher risk strategies'),
('CBU_TYPE', '40_ACT_FUND', 1, 'US 40 Act fund - SEC regulated'),
('CBU_TYPE', 'FAMILY_TRUST', 2, 'Family trust - private wealth'),
('CBU_TYPE', 'TRADING_COMPANY', 3, 'Trading company - higher velocity'),
('CBU_TYPE', 'SPV', 2, 'Special purpose vehicle'),
('CBU_TYPE', 'PENSION_FUND', 1, 'Pension fund - regulated, institutional');

-- Source of funds factors
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('SOURCE_OF_FUNDS', 'REGULATED_INSTITUTION', 0, 'Regulated bank/insurer - lowest risk'),
('SOURCE_OF_FUNDS', 'INSTITUTIONAL_INVESTOR', 1, 'Pension, sovereign wealth, endowment'),
('SOURCE_OF_FUNDS', 'PRIVATE_WEALTH', 2, 'HNWI, family office'),
('SOURCE_OF_FUNDS', 'CORPORATE', 2, 'Corporate treasury'),
('SOURCE_OF_FUNDS', 'MIXED', 2, 'Multiple source types'),
('SOURCE_OF_FUNDS', 'UNKNOWN', 4, 'Source not yet determined');

-- Nature/purpose factors
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('NATURE_PURPOSE', 'LONG_ONLY', 1, 'Long-only investment strategy'),
('NATURE_PURPOSE', 'LEVERAGED_TRADING', 3, 'Leveraged or short strategies'),
('NATURE_PURPOSE', 'REAL_ESTATE', 2, 'Real estate investment'),
('NATURE_PURPOSE', 'PRIVATE_EQUITY', 2, 'Private equity / venture'),
('NATURE_PURPOSE', 'HOLDING', 2, 'Holding structure'),
('NATURE_PURPOSE', 'OPERATING', 1, 'Operating business');

-- Jurisdiction factors
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('JURISDICTION', 'LU', 0, 'Luxembourg - EU regulated'),
('JURISDICTION', 'IE', 0, 'Ireland - EU regulated'),
('JURISDICTION', 'GB', 0, 'UK - well regulated'),
('JURISDICTION', 'US', 0, 'United States - SEC/CFTC'),
('JURISDICTION', 'KY', 2, 'Cayman Islands - offshore'),
('JURISDICTION', 'VG', 2, 'British Virgin Islands - offshore'),
('JURISDICTION', 'JE', 1, 'Jersey - Crown dependency'),
('JURISDICTION', 'GG', 1, 'Guernsey - Crown dependency'),
('JURISDICTION', 'HIGH_RISK', 4, 'FATF high-risk jurisdiction');

-- UBO requirements by risk band
-- LOW risk
INSERT INTO "ob-poc".threshold_requirements 
    (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) 
VALUES
('UBO', 'LOW', 'identity', true, 0.90, NULL, false),
('UBO', 'LOW', 'address', true, 0.85, 180, false),
('UBO', 'LOW', 'date_of_birth', true, 0.90, NULL, false),
('UBO', 'LOW', 'nationality', true, 0.85, NULL, false);

-- HIGH risk
INSERT INTO "ob-poc".threshold_requirements 
    (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) 
VALUES
('UBO', 'HIGH', 'identity', true, 0.95, NULL, true),
('UBO', 'HIGH', 'address', true, 0.90, 90, false),
('UBO', 'HIGH', 'date_of_birth', true, 0.95, NULL, true),
('UBO', 'HIGH', 'nationality', true, 0.90, NULL, false),
('UBO', 'HIGH', 'source_of_wealth', true, 0.85, NULL, false),
('UBO', 'HIGH', 'tax_residence', true, 0.85, 365, false);

-- ENHANCED risk
INSERT INTO "ob-poc".threshold_requirements 
    (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) 
VALUES
('UBO', 'ENHANCED', 'identity', true, 0.98, NULL, true),
('UBO', 'ENHANCED', 'address', true, 0.95, 60, true),
('UBO', 'ENHANCED', 'date_of_birth', true, 0.98, NULL, true),
('UBO', 'ENHANCED', 'nationality', true, 0.95, NULL, true),
('UBO', 'ENHANCED', 'source_of_wealth', true, 0.90, NULL, false),
('UBO', 'ENHANCED', 'source_of_funds', true, 0.90, NULL, false),
('UBO', 'ENHANCED', 'tax_residence', true, 0.90, 180, false),
('UBO', 'ENHANCED', 'pep_status', true, 0.95, NULL, false);

-- Director requirements (simpler than UBO)
INSERT INTO "ob-poc".threshold_requirements 
    (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days) 
VALUES
('DIRECTOR', 'LOW', 'identity', true, 0.85, NULL),
('DIRECTOR', 'LOW', 'address', true, 0.80, 365),
('DIRECTOR', 'HIGH', 'identity', true, 0.90, NULL),
('DIRECTOR', 'HIGH', 'address', true, 0.85, 180),
('DIRECTOR', 'ENHANCED', 'identity', true, 0.95, NULL),
('DIRECTOR', 'ENHANCED', 'address', true, 0.90, 90);

-- Acceptable documents per requirement
-- (Link requirements to document_types)
INSERT INTO "ob-poc".requirement_acceptable_docs (requirement_id, document_type_code, priority)
SELECT r.requirement_id, dt.type_code, 
    CASE dt.type_code 
        WHEN 'PASSPORT' THEN 1 
        WHEN 'NATIONAL_ID' THEN 2 
        WHEN 'DRIVERS_LICENSE' THEN 3 
        ELSE 5 
    END
FROM "ob-poc".threshold_requirements r
CROSS JOIN "ob-poc".document_types dt
WHERE r.attribute_code = 'identity'
  AND dt.type_code IN ('PASSPORT', 'NATIONAL_ID', 'DRIVERS_LICENSE');

-- Address proof documents
INSERT INTO "ob-poc".requirement_acceptable_docs (requirement_id, document_type_code, priority)
SELECT r.requirement_id, dt.type_code, 
    CASE dt.type_code 
        WHEN 'UTILITY_BILL' THEN 1 
        WHEN 'BANK_STATEMENT' THEN 2 
        WHEN 'COUNCIL_TAX_BILL' THEN 3 
        ELSE 5 
    END
FROM "ob-poc".threshold_requirements r
CROSS JOIN "ob-poc".document_types dt
WHERE r.attribute_code = 'address'
  AND dt.type_code IN ('UTILITY_BILL', 'BANK_STATEMENT', 'COUNCIL_TAX_BILL', 'TENANCY_AGREEMENT');

-- Screening requirements
INSERT INTO "ob-poc".screening_requirements (risk_band, screening_type, is_required) VALUES
('LOW', 'SANCTIONS', true),
('LOW', 'PEP', true),
('LOW', 'ADVERSE_MEDIA', false),
('MEDIUM', 'SANCTIONS', true),
('MEDIUM', 'PEP', true),
('MEDIUM', 'ADVERSE_MEDIA', true),
('HIGH', 'SANCTIONS', true),
('HIGH', 'PEP', true),
('HIGH', 'ADVERSE_MEDIA', true),
('ENHANCED', 'SANCTIONS', true),
('ENHANCED', 'PEP', true),
('ENHANCED', 'ADVERSE_MEDIA', true);
```

### 2.3 Threshold DSL Verbs

**Verb: `threshold.derive`**

Computes requirements for a CBU based on its attributes.

```yaml
verb: threshold.derive
category: kyc
description: Compute KYC requirements based on CBU risk factors
arguments:
  - name: cbu-id
    type: Reference
    required: true
  - name: as
    type: Binding
    required: true
    description: Binds computed requirements

returns:
  type: ThresholdRequirements
  structure:
    risk_score: integer
    risk_band: RiskBand
    entity_requirements:
      - entity_id: uuid
        entity_role: string
        requirements:
          - attribute: string
            required: boolean
            acceptable_docs: [DocumentTypeCode]
            confidence_min: float
            max_age_days: integer?
            must_be_authoritative: boolean
    screening_requirements:
      sanctions: boolean
      pep: boolean
      adverse_media: boolean

implementation: |
  1. Load CBU attributes: cbu_type, source_of_funds, nature_purpose, jurisdiction
  2. Load product risk ratings from service_delivery_map (take MAX)
  3. Sum risk weights from threshold_factors table
  4. Map to risk_band via risk_bands table
  5. For each entity in CBU with KYC-relevant role:
     a. Look up threshold_requirements for (role, risk_band)
     b. Look up acceptable docs from requirement_acceptable_docs
     c. Add to result
  6. Add screening requirements from screening_requirements table
  7. Return structured requirements
```

**Verb: `threshold.evaluate`**

Evaluates current state against requirements.

```yaml
verb: threshold.evaluate
category: kyc
description: Check if CBU meets threshold requirements
arguments:
  - name: cbu-id
    type: Reference
    required: true
  - name: requirements
    type: Reference
    required: false
    description: Pre-computed requirements (if not provided, derives them)
  - name: as
    type: Binding
    required: true

returns:
  type: ThresholdEvaluation
  structure:
    overall_status: COMPLETE | INCOMPLETE | BLOCKED
    entities:
      - entity_id: uuid
        role: string
        status: COMPLETE | INCOMPLETE | BLOCKED
        checks:
          - attribute: string
            status: MET | MISSING | EXPIRED | INSUFFICIENT_CONFIDENCE | CONFLICT
            observations: [Observation]
            proof_document: uuid?
            notes: string?
    gaps:
      - type: MISSING_ATTRIBUTE | EXPIRED_DOCUMENT | INSUFFICIENT_CONFIDENCE | UNTERMINATED_CHAIN
        entity_id: uuid
        attribute: string?
        details: string
    blocking:
      - type: UNRESOLVED_CONFLICT | SCREENING_HIT | SANCTIONED_ENTITY
        entity_id: uuid
        details: string

implementation: |
  1. Get requirements (derive if not passed)
  2. For each entity requirement:
     a. Query observations for entity + attribute
     b. Check if authoritative observation exists
     c. Check confidence meets threshold
     d. Check document age if max_age_days set
     e. Check for unresolved conflicts
     f. Record status
  3. Check screening results
  4. Aggregate gaps and blockers
  5. Compute overall status
```

---

## Phase 3: RFI System

### 3.1 Database Schema

```sql
-- RFI (Request for Information)
CREATE TABLE "ob-poc".rfis (
    rfi_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES "ob-poc".kyc_cases(case_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    rfi_type VARCHAR(30) NOT NULL DEFAULT 'INITIAL',  -- INITIAL, SUPPLEMENTARY, REFRESH
    status VARCHAR(30) NOT NULL DEFAULT 'DRAFT',      -- DRAFT, SENT, PARTIAL, COMPLETE, CLOSED, CANCELLED
    created_at TIMESTAMPTZ DEFAULT now(),
    sent_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    due_date DATE,
    notes TEXT,
    created_by VARCHAR(100),
    CONSTRAINT valid_rfi_type CHECK (rfi_type IN ('INITIAL', 'SUPPLEMENTARY', 'REFRESH')),
    CONSTRAINT valid_rfi_status CHECK (status IN ('DRAFT', 'SENT', 'PARTIAL', 'COMPLETE', 'CLOSED', 'CANCELLED'))
);

-- RFI Items (individual document requests)
CREATE TABLE "ob-poc".rfi_items (
    item_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rfi_id UUID NOT NULL REFERENCES "ob-poc".rfis(rfi_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    proves_attribute VARCHAR(50) NOT NULL,
    request_text TEXT,
    is_required BOOLEAN DEFAULT true,
    max_age_days INTEGER,
    status VARCHAR(30) NOT NULL DEFAULT 'PENDING',  -- PENDING, RECEIVED, ACCEPTED, REJECTED
    received_at TIMESTAMPTZ,
    reviewed_at TIMESTAMPTZ,
    reviewed_by VARCHAR(100),
    rejection_reason TEXT,
    notes TEXT,
    CONSTRAINT valid_item_status CHECK (status IN ('PENDING', 'RECEIVED', 'ACCEPTED', 'REJECTED'))
);

-- Acceptable document types per RFI item
CREATE TABLE "ob-poc".rfi_item_acceptable_docs (
    item_id UUID REFERENCES "ob-poc".rfi_items(item_id),
    document_type_code VARCHAR(50) REFERENCES "ob-poc".document_types(type_code),
    priority INTEGER DEFAULT 1,
    PRIMARY KEY (item_id, document_type_code)
);

-- Documents received against RFI items
CREATE TABLE "ob-poc".rfi_item_documents (
    item_id UUID REFERENCES "ob-poc".rfi_items(item_id),
    document_id UUID REFERENCES "ob-poc".document_catalog(document_id),
    received_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (item_id, document_id)
);

-- RFI delivery log (audit trail)
CREATE TABLE "ob-poc".rfi_delivery_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rfi_id UUID REFERENCES "ob-poc".rfis(rfi_id),
    channel VARCHAR(30) NOT NULL,  -- EMAIL, PORTAL, API
    recipient VARCHAR(255),
    sent_at TIMESTAMPTZ DEFAULT now(),
    status VARCHAR(30),  -- SENT, DELIVERED, BOUNCED, OPENED
    notes TEXT
);

-- Indexes
CREATE INDEX idx_rfis_case ON "ob-poc".rfis(case_id);
CREATE INDEX idx_rfis_cbu ON "ob-poc".rfis(cbu_id);
CREATE INDEX idx_rfis_status ON "ob-poc".rfis(status);
CREATE INDEX idx_rfi_items_rfi ON "ob-poc".rfi_items(rfi_id);
CREATE INDEX idx_rfi_items_entity ON "ob-poc".rfi_items(entity_id);
CREATE INDEX idx_rfi_items_status ON "ob-poc".rfi_items(status);
```

### 3.2 RFI DSL Verbs

**Verb: `rfi.generate`**

Auto-generate RFI from gaps.

```yaml
verb: rfi.generate
category: kyc
description: Generate RFI from threshold evaluation gaps
arguments:
  - name: case-id
    type: Reference
    required: true
  - name: gaps
    type: Reference
    required: true
    description: Gaps from threshold.evaluate
  - name: type
    type: RfiType
    required: false
    default: INITIAL
  - name: due-days
    type: Integer
    required: false
    default: 14
  - name: as
    type: Binding
    required: true

implementation: |
  1. Create RFI record with status DRAFT
  2. For each gap:
     a. Create rfi_item for entity + attribute
     b. Look up acceptable docs from requirement_acceptable_docs
     c. Copy to rfi_item_acceptable_docs
     d. Generate request_text from template
  3. De-duplicate (if same entity + attribute from multiple gaps)
  4. Group items by entity for logical ordering
  5. Return RFI reference
```

**Verb: `rfi.create`**

Manually create RFI.

```yaml
verb: rfi.create
category: kyc
description: Create empty RFI for manual population
arguments:
  - name: case-id
    type: Reference
    required: true
  - name: type
    type: RfiType
    required: false
    default: INITIAL
  - name: due-days
    type: Integer
    required: false
  - name: notes
    type: String
    required: false
  - name: as
    type: Binding
    required: true
```

**Verb: `rfi.request-document`**

Add item to RFI.

```yaml
verb: rfi.request-document
category: kyc
description: Add document request to RFI
arguments:
  - name: rfi-id
    type: Reference
    required: true
  - name: entity-id
    type: Reference
    required: true
  - name: proves
    type: AttributeCode
    required: true
    description: Attribute this document proves (identity, address, etc.)
  - name: acceptable-docs
    type: DocumentTypeList
    required: true
    description: List of acceptable document type codes from document_types table
  - name: required
    type: Boolean
    required: false
    default: true
  - name: max-age-days
    type: Integer
    required: false
    description: Maximum document age in days
  - name: notes
    type: String
    required: false

validation: |
  - All document type codes in acceptable-docs must exist in document_types table
  - proves must be a valid attribute code
  - RFI must be in DRAFT status
```

**Verb: `rfi.finalize`**

Mark RFI ready to send.

```yaml
verb: rfi.finalize
category: kyc
description: Finalize RFI (lock for editing)
arguments:
  - name: rfi-id
    type: Reference
    required: true

validation: |
  - RFI must be in DRAFT status
  - RFI must have at least one item
  
side_effects: |
  - Sets status to PENDING_SEND
  - Locks RFI from edits
```

**Verb: `rfi.send`**

Send RFI to client.

```yaml
verb: rfi.send
category: kyc
description: Send RFI to recipient
arguments:
  - name: rfi-id
    type: Reference
    required: true
  - name: channel
    type: Enum[EMAIL, PORTAL, API]
    required: true
  - name: recipient
    type: String
    required: true
    description: Email address or portal user ID

side_effects: |
  - Updates status to SENT
  - Sets sent_at timestamp
  - Creates rfi_delivery_log entry
  - (Integration: triggers email/portal notification)
```

**Verb: `rfi.receive`**

Record document received against RFI item.

```yaml
verb: rfi.receive
category: kyc
description: Record document received for RFI item
arguments:
  - name: rfi-id
    type: Reference
    required: true
  - name: item-id
    type: Reference
    required: true
  - name: document-id
    type: Reference
    required: true
    description: Reference to uploaded document in document_catalog

side_effects: |
  - Creates rfi_item_documents link
  - Updates item status to RECEIVED
  - If all required items received, updates RFI status to COMPLETE
  - Triggers document.extract-observations if auto-processing enabled
```

**Verb: `rfi.close`**

Close RFI (complete or cancelled).

```yaml
verb: rfi.close
category: kyc
description: Close RFI
arguments:
  - name: rfi-id
    type: Reference
    required: true
  - name: status
    type: Enum[COMPLETE, CANCELLED]
    required: true
  - name: notes
    type: String
    required: false
```

---

## Phase 4: UBO Analysis

### 4.1 Database Views

```sql
-- Ownership chain computation (recursive CTE)
CREATE OR REPLACE FUNCTION "ob-poc".compute_ownership_chains(target_cbu_id UUID)
RETURNS TABLE (
    chain_id INTEGER,
    depth INTEGER,
    entity_id UUID,
    entity_name TEXT,
    entity_type VARCHAR(50),
    parent_entity_id UUID,
    ownership_pct NUMERIC(5,2),
    aggregate_pct NUMERIC(5,2),
    terminates_at VARCHAR(50),  -- NATURAL_PERSON, LISTED_COMPANY, REGULATED_FUND, etc.
    is_ubo BOOLEAN,
    chain_path UUID[]
) AS $$
WITH RECURSIVE ownership_tree AS (
    -- Base: start from CBU anchor (commercial client entity)
    SELECT 
        1 as chain_id,
        0 as depth,
        c.commercial_client_entity_id as entity_id,
        e.name as entity_name,
        et.type_code as entity_type,
        NULL::UUID as parent_entity_id,
        100.00::NUMERIC as ownership_pct,
        100.00::NUMERIC as aggregate_pct,
        ARRAY[c.commercial_client_entity_id] as chain_path
    FROM "ob-poc".cbus c
    JOIN "ob-poc".entities e ON c.commercial_client_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE c.cbu_id = target_cbu_id
    
    UNION ALL
    
    -- Recursive: follow ownership links upward
    SELECT 
        ot.chain_id + CASE WHEN ol.parent_entity_id = ANY(ot.chain_path) THEN 1000 ELSE 0 END,
        ot.depth + 1,
        ol.parent_entity_id,
        e.name,
        et.type_code,
        ot.entity_id,
        ol.ownership_percentage,
        (ot.aggregate_pct * ol.ownership_percentage / 100)::NUMERIC(5,2),
        ot.chain_path || ol.parent_entity_id
    FROM ownership_tree ot
    JOIN "ob-poc".ownership_links ol ON ol.child_entity_id = ot.entity_id
    JOIN "ob-poc".entities e ON ol.parent_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE ot.depth < 10  -- Max depth to prevent infinite loops
      AND NOT (ol.parent_entity_id = ANY(ot.chain_path))  -- Cycle detection
)
SELECT 
    chain_id,
    depth,
    entity_id,
    entity_name,
    entity_type,
    parent_entity_id,
    ownership_pct,
    aggregate_pct,
    CASE 
        WHEN entity_type = 'NATURAL_PERSON' THEN 'NATURAL_PERSON'
        WHEN entity_type = 'LISTED_COMPANY' THEN 'LISTED_COMPANY'
        WHEN entity_type = 'GOVERNMENT_BODY' THEN 'GOVERNMENT'
        WHEN entity_type = 'REGULATED_FUND' THEN 'REGULATED_FUND'
        ELSE NULL  -- Chain continues
    END as terminates_at,
    entity_type = 'NATURAL_PERSON' AND aggregate_pct >= 25.00 as is_ubo,
    chain_path
FROM ownership_tree
ORDER BY chain_id, depth;
$$ LANGUAGE SQL;

-- UBO summary view
CREATE OR REPLACE VIEW "ob-poc".v_cbu_ubo_summary AS
SELECT 
    c.cbu_id,
    oc.entity_id,
    oc.entity_name,
    SUM(oc.aggregate_pct) as total_aggregate_pct,
    bool_or(oc.is_ubo) as is_ubo,
    MAX(oc.depth) as max_chain_depth,
    COUNT(DISTINCT oc.chain_id) as num_ownership_paths
FROM "ob-poc".cbus c
CROSS JOIN LATERAL "ob-poc".compute_ownership_chains(c.cbu_id) oc
WHERE oc.terminates_at = 'NATURAL_PERSON'
GROUP BY c.cbu_id, oc.entity_id, oc.entity_name;

-- Unterminated chains (gaps in structure)
CREATE OR REPLACE VIEW "ob-poc".v_cbu_unterminated_chains AS
SELECT 
    c.cbu_id,
    oc.chain_id,
    oc.entity_id as last_entity_id,
    oc.entity_name as last_entity_name,
    oc.entity_type,
    oc.aggregate_pct,
    oc.chain_path
FROM "ob-poc".cbus c
CROSS JOIN LATERAL "ob-poc".compute_ownership_chains(c.cbu_id) oc
WHERE oc.terminates_at IS NULL
  AND oc.depth = (
      SELECT MAX(oc2.depth) 
      FROM "ob-poc".compute_ownership_chains(c.cbu_id) oc2 
      WHERE oc2.chain_id = oc.chain_id
  );
```

### 4.2 UBO DSL Verbs

**Verb: `ubo.trace-chains`**

```yaml
verb: ubo.trace-chains
category: kyc
description: Compute all ownership chains for CBU
arguments:
  - name: cbu-id
    type: Reference
    required: true
  - name: as
    type: Binding
    required: true

returns:
  type: OwnershipChains
  structure:
    chains:
      - chain_id: integer
        path: [EntityInChain]
        terminates_at: TerminationType?
        aggregate_pct: float
    ubos:
      - entity_id: uuid
        name: string
        aggregate_pct: float
        chain_ids: [integer]
```

**Verb: `ubo.check-completeness`**

```yaml
verb: ubo.check-completeness
category: kyc
description: Check if UBO structure is complete
arguments:
  - name: cbu-id
    type: Reference
    required: true
  - name: as
    type: Binding
    required: true

returns:
  type: UboCompletenessResult
  structure:
    complete: boolean
    identified_ownership_pct: float
    ubos:
      - entity_id: uuid
        aggregate_pct: float
        kyc_complete: boolean
    unterminated_chains:
      - chain_id: integer
        last_entity_id: uuid
        last_entity_name: string
        aggregate_pct: float
    placeholders_needed:
      - role: string
        parent_entity_id: uuid
        ownership_pct: float?
```

**Verb: `ubo.insert-placeholder`**

```yaml
verb: ubo.insert-placeholder
category: kyc
description: Insert placeholder entity for unknown UBO
arguments:
  - name: cbu-id
    type: Reference
    required: true
  - name: role
    type: RoleCode
    required: true
  - name: parent-entity-id
    type: Reference
    required: true
  - name: ownership-pct
    type: Float
    required: false
  - name: notes
    type: String
    required: false
  - name: as
    type: Binding
    required: true

side_effects: |
  - Creates entity with is_placeholder = true
  - Links to parent via ownership_links
  - Creates cbu_entity_role entry
```

---

## Phase 5: Event System

### 5.1 Database Schema

```sql
-- Event definitions
CREATE TABLE "ob-poc".event_types (
    event_type VARCHAR(50) PRIMARY KEY,
    description TEXT,
    payload_schema JSONB,  -- JSON schema for validation
    is_active BOOLEAN DEFAULT true
);

-- Event handlers (DSL-defined)
CREATE TABLE "ob-poc".event_handlers (
    handler_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type VARCHAR(50) REFERENCES "ob-poc".event_types(event_type),
    handler_name VARCHAR(100) NOT NULL,
    conditions JSONB,  -- Conditions for handler to fire
    dsl_source TEXT NOT NULL,  -- DSL code to execute
    priority INTEGER DEFAULT 100,
    is_active BOOLEAN DEFAULT true,
    automation_mode VARCHAR(20) DEFAULT 'SEMI_AUTO',  -- FULL_AUTO, SEMI_AUTO, MANUAL
    created_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT valid_automation_mode CHECK (automation_mode IN ('FULL_AUTO', 'SEMI_AUTO', 'MANUAL'))
);

-- Event log
CREATE TABLE "ob-poc".event_log (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type VARCHAR(50) NOT NULL,
    payload JSONB NOT NULL,
    cbu_id UUID,
    case_id UUID,
    entity_id UUID,
    occurred_at TIMESTAMPTZ DEFAULT now(),
    processed_at TIMESTAMPTZ,
    status VARCHAR(20) DEFAULT 'PENDING',  -- PENDING, PROCESSING, COMPLETED, FAILED
    error_message TEXT
);

-- Action queue (for SEMI_AUTO mode)
CREATE TABLE "ob-poc".action_queue (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES "ob-poc".event_log(event_id),
    action_type VARCHAR(50) NOT NULL,
    payload JSONB NOT NULL,
    status VARCHAR(20) DEFAULT 'PENDING',  -- PENDING, APPROVED, REJECTED, EXECUTED
    assigned_to VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT now(),
    reviewed_at TIMESTAMPTZ,
    reviewed_by VARCHAR(100),
    notes TEXT
);

-- Seed event types
INSERT INTO "ob-poc".event_types (event_type, description) VALUES
('DOCUMENT_UPLOADED', 'Document uploaded to system'),
('RFI_SENT', 'RFI sent to client'),
('RFI_ITEM_RECEIVED', 'Document received for RFI item'),
('RFI_COMPLETE', 'All required RFI items received'),
('OBSERVATION_CREATED', 'New observation extracted from document'),
('OBSERVATION_CONFLICT', 'Conflicting observations detected'),
('OWNERSHIP_STRUCTURE_CHANGED', 'CBU ownership structure modified'),
('ENTITY_ADDED', 'New entity added to CBU'),
('THRESHOLD_EVALUATION_COMPLETE', 'Threshold evaluation finished'),
('SCREENING_COMPLETE', 'Screening check completed'),
('CASE_STATE_CHANGED', 'KYC case state transition');
```

### 5.2 Event DSL Verbs

**Verb: `on-event`**

Define event handler (declarative).

```yaml
verb: on-event
category: automation
description: Define handler for event type
arguments:
  - name: event-type
    type: EventType
    required: true
  - name: conditions
    type: ConditionList
    required: false
    description: Conditions that must be true for handler to fire
  - name: mode
    type: AutomationMode
    required: false
    default: SEMI_AUTO
  - name: actions
    type: ActionList
    required: true
    description: DSL statements to execute

example: |
  (on-event :DOCUMENT_UPLOADED
    :conditions [:rfi-item-linked :document-type-accepted]
    :mode :SEMI_AUTO
    :actions [
      (document.extract-observations :document-id @event.document-id :entity-id @event.entity-id)
      (threshold.evaluate :cbu-id @event.cbu-id :as @eval)
      (when @eval.has-new-gaps
        (action.queue-for-review 
          :action-type :GENERATE_SUPPLEMENTARY_RFI 
          :payload {:gaps @eval.gaps}))
    ])
```

**Verb: `kyc-case.reevaluate`**

Force re-evaluation (HITL trigger).

```yaml
verb: kyc-case.reevaluate
category: kyc
description: Manually trigger full evaluation cycle
arguments:
  - name: case-id
    type: Reference
    required: true
  - name: reason
    type: String
    required: false
  - name: as
    type: Binding
    required: true

implementation: |
  1. Run threshold.derive for CBU
  2. Run threshold.evaluate
  3. Run ubo.check-completeness
  4. Generate evaluation report
  5. Log in case history
  6. Return result for analyst review
```

**Verb: `action.queue-for-review`**

Queue action for human approval (SEMI_AUTO mode).

```yaml
verb: action.queue-for-review
category: automation
description: Queue action for human review before execution
arguments:
  - name: action-type
    type: ActionType
    required: true
  - name: payload
    type: Any
    required: true
  - name: assigned-to
    type: String
    required: false
  - name: notes
    type: String
    required: false
```

---

## Phase 6: KYC Case State Machine

### 6.1 States and Transitions

```sql
-- Case states
CREATE TABLE "ob-poc".kyc_case_states (
    state_code VARCHAR(30) PRIMARY KEY,
    description TEXT,
    is_terminal BOOLEAN DEFAULT false,
    allowed_transitions VARCHAR(30)[]
);

INSERT INTO "ob-poc".kyc_case_states VALUES
('INTAKE', 'Initial intake', false, ARRAY['DISCOVERY', 'CANCELLED']),
('DISCOVERY', 'RFI loop - gathering information', false, ARRAY['ASSESSMENT', 'INTAKE', 'CANCELLED']),
('ASSESSMENT', 'Evaluating completeness', false, ARRAY['DISCOVERY', 'REVIEW', 'CANCELLED']),
('REVIEW', 'Analyst review', false, ARRAY['ASSESSMENT', 'APPROVED', 'ESCALATED', 'REJECTED']),
('ESCALATED', 'Compliance/senior review', false, ARRAY['APPROVED', 'REJECTED', 'REVIEW']),
('APPROVED', 'KYC approved', true, ARRAY['REVIEW']),  -- Can reopen
('REJECTED', 'KYC rejected', true, ARRAY[]),
('CANCELLED', 'Case cancelled', true, ARRAY[]);

-- Case state history
CREATE TABLE "ob-poc".kyc_case_state_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID REFERENCES "ob-poc".kyc_cases(case_id),
    from_state VARCHAR(30),
    to_state VARCHAR(30) REFERENCES "ob-poc".kyc_case_states(state_code),
    transitioned_at TIMESTAMPTZ DEFAULT now(),
    transitioned_by VARCHAR(100),
    reason TEXT,
    automation_mode VARCHAR(20)  -- Was this auto or manual?
);
```

### 6.2 Case State DSL Verbs

```yaml
verb: kyc-case.advance
category: kyc
description: Advance case to next state
arguments:
  - name: case-id
    type: Reference
    required: true
  - name: to
    type: CaseState
    required: true
  - name: reason
    type: String
    required: false

validation: |
  - Transition must be allowed from current state
  - Required conditions for target state must be met

verb: kyc-case.approve
category: kyc
arguments:
  - name: case-id
    type: Reference
    required: true
  - name: risk-rating
    type: RiskRating
    required: true
  - name: next-review
    type: Date
    required: true
  - name: notes
    type: String
    required: false

verb: kyc-case.escalate
category: kyc
arguments:
  - name: case-id
    type: Reference
    required: true
  - name: reason
    type: String
    required: true
  - name: escalate-to
    type: String
    required: false

verb: kyc-case.reject
category: kyc
arguments:
  - name: case-id
    type: Reference
    required: true
  - name: reason
    type: String
    required: true
```

---

## Phase 7: Go Test Harness

### 7.1 Purpose

A Go service that:
- Provides web UI for running DSL test scenarios
- Displays DSL execution results
- Shows database state (CBU, entities, observations, RFIs)
- Allows manual event triggering
- Supports step-through of DSL execution

### 7.2 Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Go Test Harness                    │
│  ┌─────────────────────────────────────────────┐   │
│  │            Web UI (HTML/HTMX)               │   │
│  │  - Test scenario picker                      │   │
│  │  - DSL source viewer                         │   │
│  │  - Execution log                             │   │
│  │  - DB state inspector                        │   │
│  └─────────────────────────────────────────────┘   │
│                       │                             │
│  ┌─────────────────────────────────────────────┐   │
│  │           Test Runner Service               │   │
│  │  - Load test scenarios                       │   │
│  │  - Call Rust DSL executor                    │   │
│  │  - Capture results                           │   │
│  │  - Manage test state                         │   │
│  └─────────────────────────────────────────────┘   │
│                       │                             │
│  ┌─────────────────────────────────────────────┐   │
│  │           Database Viewer                   │   │
│  │  - Query v_cbu_* views                       │   │
│  │  - Show entity graph                         │   │
│  │  - Display observations                      │   │
│  │  - Track RFI status                          │   │
│  └─────────────────────────────────────────────┘   │
└──────────────────────┬──────────────────────────────┘
                       │ gRPC / CLI
┌──────────────────────▼──────────────────────────────┐
│                 Rust DSL Engine                     │
│  - Parse DSL                                        │
│  - Execute verbs                                    │
│  - Return results                                   │
└─────────────────────────────────────────────────────┘
```

### 7.3 Test Scenario Format

```yaml
# test_scenarios/luxsicav_initial_kyc.yaml
name: "LuxSICAV Initial KYC"
description: "Complete initial KYC for a Luxembourg SICAV fund"

setup:
  - create_cbu:
      name: "Acme SICAV"
      type: LUXSICAV_UCITS
      jurisdiction: LU
  - add_product:
      cbu: @cbu
      product: CUSTODY
      risk: HIGH
  - add_entities:
      - { name: "Acme ManCo", type: LIMITED_COMPANY, role: MANAGEMENT_COMPANY }
      - { name: "State Street", type: LIMITED_COMPANY, role: DEPOSITARY }
      - { name: "PwC Luxembourg", type: LIMITED_COMPANY, role: AUDITOR }
      - { name: "John Smith", type: NATURAL_PERSON, role: DIRECTOR }

steps:
  - name: "Create KYC Case"
    dsl: |
      (kyc-case.create :cbu-id @cbu :as @case)
    expect:
      case.status: INTAKE
      
  - name: "Derive Thresholds"
    dsl: |
      (threshold.derive :cbu-id @cbu :as @requirements)
    expect:
      requirements.risk_band: HIGH
      requirements.entity_requirements.length: 4
      
  - name: "Evaluate (should have gaps)"
    dsl: |
      (threshold.evaluate :cbu-id @cbu :as @eval)
    expect:
      eval.overall_status: INCOMPLETE
      eval.gaps.length: "> 0"
      
  - name: "Generate RFI"
    dsl: |
      (rfi.generate :case-id @case :gaps @eval.gaps :as @rfi)
    expect:
      rfi.status: DRAFT
      rfi.items.length: "> 0"
      
  - name: "Send RFI"
    dsl: |
      (rfi.finalize :rfi-id @rfi)
      (rfi.send :rfi-id @rfi :channel :EMAIL :recipient "client@acme.lu")
    expect:
      rfi.status: SENT
      
  - name: "Simulate Document Upload"
    action: upload_document
    params:
      entity: "John Smith"
      type: PASSPORT
      file: "test_data/passport_john_smith.pdf"
    trigger_event: DOCUMENT_UPLOADED
    
  - name: "Process Document"
    dsl: |
      (document.extract-observations :document-id @uploaded :entity-id @john)
    expect:
      observations.count: "> 0"
      
  - name: "Re-evaluate"
    dsl: |
      (kyc-case.reevaluate :case-id @case :as @result)
    expect:
      result.gaps.length: "< previous"

cleanup:
  - delete_test_data: true
```

### 7.4 Go Project Structure

```
cmd/
  kyc-test-harness/
    main.go
    
internal/
  server/
    server.go           # HTTP server setup
    handlers.go         # Route handlers
    
  testrunner/
    runner.go           # Test execution engine
    scenario.go         # Scenario parser
    results.go          # Result tracking
    
  dslclient/
    client.go           # gRPC/CLI client to Rust DSL
    types.go            # Shared types
    
  dbviewer/
    queries.go          # DB query functions
    views.go            # Formatted views
    
  templates/
    layout.html
    scenarios.html
    execution.html
    dbstate.html
    
config/
  config.yaml
  
test_scenarios/
  luxsicav_initial_kyc.yaml
  hedge_fund_complex_ubo.yaml
  family_trust_with_placeholders.yaml
  
go.mod
go.sum
```

---

## Implementation Order

### Week 1: Foundation
1. Phase 1 - Argument types (DocumentTypeCode validation)
2. Phase 2 - Threshold schema + seed data
3. Phase 2 - `threshold.derive` verb

### Week 2: Threshold Evaluation
4. Phase 2 - `threshold.evaluate` verb
5. Phase 4 - UBO chain computation (SQL function)
6. Phase 4 - `ubo.trace-chains`, `ubo.check-completeness`

### Week 3: RFI System
7. Phase 3 - RFI schema
8. Phase 3 - RFI verbs (`create`, `request-document`, `finalize`, `send`, `receive`)
9. Phase 3 - `rfi.generate` (auto from gaps)

### Week 4: State Machine + Events
10. Phase 6 - Case state machine
11. Phase 5 - Event schema
12. Phase 5 - Event verbs (`on-event`, `action.queue-for-review`)

### Week 5: Go Harness
13. Phase 7 - Go project setup
14. Phase 7 - DSL client (gRPC to Rust)
15. Phase 7 - Web UI + test scenarios

### Week 6: Integration Testing
16. End-to-end scenario: Initial KYC
17. End-to-end scenario: Complex UBO discovery
18. End-to-end scenario: RFI loop with document upload

---

## Testing Checklist

- [ ] Document type codes validate at parse time
- [ ] Threshold derive correctly computes risk band
- [ ] Threshold evaluate identifies all gap types
- [ ] UBO chains terminate correctly
- [ ] Circular ownership detected and handled
- [ ] RFI generates correct document requests
- [ ] RFI items link to correct document types
- [ ] Event handlers fire on correct events
- [ ] SEMI_AUTO mode queues actions for review
- [ ] Case state transitions enforce rules
- [ ] Go harness can execute full scenario
- [ ] Go harness displays DB state correctly
