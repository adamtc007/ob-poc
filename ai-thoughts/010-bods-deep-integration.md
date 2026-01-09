# BODS 0.4 Deep Integration Architecture

> **Goal:** Make BODS concepts native to OB-POC, not just an export format
> **Result:** GLEIF + BODS + DSL CBU.UBO verbs work as unified system

---

## What BODS 0.4 Gives Us (That We're Missing)

### Current Schema Gaps

| BODS Concept | Current OB-POC | Gap |
|--------------|----------------|-----|
| **Statement versioning** | `ownership_relationships` has no audit trail | Every claim needs `statement_id`, `statement_date` |
| **22 Interest Types** | `ownership_type VARCHAR(30)` free text | Need standardized codelist |
| **Direct/Indirect** | Implicit via ownership chain | Need explicit `direct_or_indirect` field |
| **Component Records** | Not tracked | Need to link intermediate entities in indirect chains |
| **Share Ranges** | `ownership_percent NUMERIC` only | Need `min`, `max`, `exclusive_min`, `exclusive_max` |
| **PEP Details** | `screening_result` simple status | Need jurisdiction, start_date, end_date per PEP role |
| **Entity Type Codelist** | `entity_types.name` free text | Need BODS enum alignment |
| **Identifiers Array** | LEI in GLEIF, others scattered | Need unified identifiers with scheme + id |
| **Unspecified Records** | No explicit "unknown owner" tracking | Need `anonymousEntity`, `unknownPerson` types |

---

## Integration Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         UNIFIED OWNERSHIP MODEL                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   GLEIF API                    BODS Schema                 OB-POC Core      │
│   ─────────                    ───────────                 ───────────      │
│   LEI lookup ─────────────────► Entity identifiers[] ◄──── entities        │
│   Ownership tree ─────────────► Relationships ◄──────────── ownership_     │
│   Corporate hierarchy ────────► Component records            relationships │
│   Registration status ────────► Entity status                              │
│                                                                             │
│                         ┌──────────────────────┐                           │
│                         │   bods_statements    │                           │
│                         │   (audit wrapper)    │                           │
│                         └──────────────────────┘                           │
│                                    │                                        │
│            ┌───────────────────────┼───────────────────────┐               │
│            ▼                       ▼                       ▼               │
│   ┌────────────────┐    ┌──────────────────┐    ┌──────────────────┐      │
│   │ bods_entities  │    │  bods_persons    │    │ bods_interests   │      │
│   │ (entity layer) │    │ (person layer)   │    │ (relationship)   │      │
│   └────────────────┘    └──────────────────┘    └──────────────────┘      │
│                                                                             │
│   DSL Verbs                                                                │
│   ─────────                                                                │
│   cbu.add-owner ──────► Creates bods_interest with proper type            │
│   cbu.trace-ubo ──────► Traverses bods_interests, tracks components       │
│   gleif.enrich ───────► Populates bods_entities.identifiers               │
│   gleif.import-tree ──► Creates bods_interests with component_records     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## New Tables (BODS-Native)

### 1. `bods_statements` - Audit Wrapper for All Claims

```sql
-- Every ownership claim, entity record, or person record is a versioned statement
CREATE TABLE "ob-poc".bods_statements (
    statement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- BODS required fields
    record_type VARCHAR(20) NOT NULL CHECK (record_type IN ('entity', 'person', 'relationship')),
    record_id UUID NOT NULL,  -- FK to the actual record
    
    -- Statement metadata
    statement_date DATE NOT NULL DEFAULT CURRENT_DATE,
    publication_date TIMESTAMPTZ DEFAULT NOW(),
    bods_version VARCHAR(10) NOT NULL DEFAULT '0.4',
    
    -- Publisher info
    publisher_name VARCHAR(255) DEFAULT 'BNY Mellon OB-POC',
    publisher_url VARCHAR(500),
    license_url VARCHAR(500),
    
    -- Lifecycle
    replaces_statement_id UUID REFERENCES "ob-poc".bods_statements(statement_id),
    replaced_by_statement_id UUID REFERENCES "ob-poc".bods_statements(statement_id),
    
    -- Annotations (for notes, flags, audit comments)
    annotations JSONB DEFAULT '[]',
    
    -- Source tracking
    source_type VARCHAR(50),  -- 'GLEIF', 'MANUAL', 'DOCUMENT', 'SCREENING'
    source_reference TEXT,
    source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by VARCHAR(255)
);

CREATE INDEX idx_bods_statements_record ON "ob-poc".bods_statements(record_type, record_id);
CREATE INDEX idx_bods_statements_date ON "ob-poc".bods_statements(statement_date);
```

### 2. `bods_interest_types` - Codelist (22 Standard Types)

```sql
CREATE TABLE "ob-poc".bods_interest_types (
    type_code VARCHAR(50) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    category VARCHAR(30) NOT NULL,  -- 'ownership', 'control', 'trust', 'other'
    description TEXT,
    bods_standard BOOLEAN DEFAULT true,  -- false for custom extensions
    display_order INTEGER DEFAULT 0
);

-- Insert BODS 0.4 standard types
INSERT INTO "ob-poc".bods_interest_types (type_code, display_name, category) VALUES
-- Ownership
('shareholding', 'Shareholding', 'ownership'),
('votingRights', 'Voting Rights', 'ownership'),
('rightsToSurplusAssetsOnDissolution', 'Rights to Surplus Assets on Dissolution', 'ownership'),
('rightsToProfitOrIncome', 'Rights to Profit or Income', 'ownership'),

-- Control
('appointmentOfBoard', 'Appointment of Board', 'control'),
('otherInfluenceOrControl', 'Other Influence or Control', 'control'),
('seniorManagingOfficial', 'Senior Managing Official', 'control'),
('controlViaCompanyRulesOrArticles', 'Control via Company Rules/Articles', 'control'),
('controlByLegalFramework', 'Control by Legal Framework', 'control'),
('boardMember', 'Board Member', 'control'),
('boardChair', 'Board Chair', 'control'),

-- Trust/Arrangement
('settlor', 'Settlor', 'trust'),
('trustee', 'Trustee', 'trust'),
('protector', 'Protector', 'trust'),
('beneficiaryOfLegalArrangement', 'Beneficiary of Legal Arrangement', 'trust'),
('enjoymentAndUseOfAssets', 'Enjoyment and Use of Assets', 'trust'),
('rightToProfitOrIncomeFromAssets', 'Right to Profit/Income from Assets', 'trust'),

-- Contractual
('rightsGrantedByContract', 'Rights Granted by Contract', 'contractual'),
('conditionalRightsGrantedByContract', 'Conditional Rights Granted by Contract', 'contractual'),

-- Nominee
('nominee', 'Nominee', 'nominee'),
('nominator', 'Nominator', 'nominee'),

-- Unknown
('unknownInterest', 'Unknown Interest', 'unknown'),
('unpublishedInterest', 'Unpublished Interest', 'unknown');
```

### 3. `bods_interests` - Replaces Simple `ownership_relationships`

```sql
CREATE TABLE "ob-poc".bods_interests (
    interest_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Link to CBU context
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    
    -- Subject (the entity being owned/controlled) - MUST be entity
    subject_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Interested Party (the owner/controller) - can be entity OR person
    interested_party_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    interested_party_person_id UUID REFERENCES "ob-poc".entity_proper_persons(proper_person_id),
    interested_party_unknown BOOLEAN DEFAULT false,  -- True if unknown/anonymous
    unspecified_reason VARCHAR(100),  -- Why unknown: 'no-beneficial-owners', 'subject-exempt', etc.
    
    -- Interest Type (from BODS codelist)
    interest_type VARCHAR(50) NOT NULL REFERENCES "ob-poc".bods_interest_types(type_code),
    
    -- Direct or Indirect
    direct_or_indirect VARCHAR(10) NOT NULL DEFAULT 'unknown' 
        CHECK (direct_or_indirect IN ('direct', 'indirect', 'unknown')),
    
    -- Is this a UBO determination?
    beneficial_ownership_or_control BOOLEAN DEFAULT false,
    
    -- Share/Percentage (BODS style - can be range)
    share_exact NUMERIC(5,2),
    share_minimum NUMERIC(5,2),
    share_maximum NUMERIC(5,2),
    share_exclusive_minimum NUMERIC(5,2),
    share_exclusive_maximum NUMERIC(5,2),
    
    -- Temporal
    start_date DATE,
    end_date DATE,
    
    -- Additional details
    details TEXT,
    
    -- Component tracking (for indirect chains)
    is_component BOOLEAN DEFAULT false,  -- Is this part of an indirect chain?
    component_of_interest_id UUID REFERENCES "ob-poc".bods_interests(interest_id),
    
    -- Statement link (for audit trail)
    statement_id UUID REFERENCES "ob-poc".bods_statements(statement_id),
    
    -- Evidence
    evidence_doc_ids UUID[] DEFAULT '{}',
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT chk_interested_party CHECK (
        (interested_party_entity_id IS NOT NULL) OR 
        (interested_party_person_id IS NOT NULL) OR 
        (interested_party_unknown = true)
    ),
    CONSTRAINT chk_share_range CHECK (
        (share_exact IS NULL) OR 
        (share_minimum IS NULL AND share_maximum IS NULL)
    )
);

CREATE INDEX idx_bods_interests_cbu ON "ob-poc".bods_interests(cbu_id);
CREATE INDEX idx_bods_interests_subject ON "ob-poc".bods_interests(subject_entity_id);
CREATE INDEX idx_bods_interests_party_entity ON "ob-poc".bods_interests(interested_party_entity_id);
CREATE INDEX idx_bods_interests_party_person ON "ob-poc".bods_interests(interested_party_person_id);
CREATE INDEX idx_bods_interests_type ON "ob-poc".bods_interests(interest_type);
CREATE INDEX idx_bods_interests_ubo ON "ob-poc".bods_interests(beneficial_ownership_or_control) WHERE beneficial_ownership_or_control = true;
```

### 4. `bods_entity_identifiers` - Unified Identifier Storage

```sql
-- Replaces scattered LEI, registration numbers, etc.
CREATE TABLE "ob-poc".bods_entity_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- BODS identifier structure
    scheme VARCHAR(50) NOT NULL,  -- 'LEI', 'company_register', 'tax_id', etc.
    scheme_name VARCHAR(100),      -- Human readable: 'Global LEI System'
    id VARCHAR(100) NOT NULL,      -- The actual identifier value
    uri VARCHAR(500),              -- Full URI if available
    
    -- Validation
    is_validated BOOLEAN DEFAULT false,
    validated_at TIMESTAMPTZ,
    validation_source VARCHAR(100),  -- 'GLEIF_API', 'MANUAL', etc.
    
    -- GLEIF-specific fields (when scheme = 'LEI')
    lei_status VARCHAR(30),        -- 'ISSUED', 'LAPSED', 'RETIRED', etc.
    lei_next_renewal DATE,
    lei_managing_lou VARCHAR(100),
    
    -- Temporal
    effective_from DATE,
    effective_to DATE,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(entity_id, scheme, id)
);

CREATE INDEX idx_bods_identifiers_entity ON "ob-poc".bods_entity_identifiers(entity_id);
CREATE INDEX idx_bods_identifiers_scheme ON "ob-poc".bods_entity_identifiers(scheme);
CREATE INDEX idx_bods_identifiers_lei ON "ob-poc".bods_entity_identifiers(id) WHERE scheme = 'LEI';
```

### 5. `bods_entity_types` - Codelist Alignment

```sql
CREATE TABLE "ob-poc".bods_entity_types (
    type_code VARCHAR(30) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    description TEXT,
    subtypes JSONB DEFAULT '[]',  -- Valid subtypes for this type
    display_order INTEGER DEFAULT 0
);

INSERT INTO "ob-poc".bods_entity_types (type_code, display_name, subtypes) VALUES
('registeredEntity', 'Registered Entity', '["other"]'),
('legalEntity', 'Legal Entity', '["trust", "other"]'),
('arrangement', 'Arrangement', '["trust", "nomination", "other"]'),
('anonymousEntity', 'Anonymous Entity', '["other"]'),
('unknownEntity', 'Unknown Entity', '["other"]'),
('state', 'State', '["other"]'),
('stateBody', 'State Body', '["governmentDepartment", "stateAgency", "other"]');

-- Add column to entities table to link to BODS type
ALTER TABLE "ob-poc".entities 
ADD COLUMN bods_entity_type VARCHAR(30) REFERENCES "ob-poc".bods_entity_types(type_code),
ADD COLUMN bods_entity_subtype VARCHAR(30);
```

### 6. `bods_pep_status` - Detailed PEP Tracking

```sql
-- More detailed than current screening_result
CREATE TABLE "ob-poc".bods_pep_status (
    pep_status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_id UUID NOT NULL REFERENCES "ob-poc".entity_proper_persons(proper_person_id),
    
    -- BODS PEP fields
    status VARCHAR(20) NOT NULL CHECK (status IN ('isPep', 'isNotPep', 'unknown')),
    
    -- Details per PEP role (person can have multiple)
    reason TEXT,                   -- Why they're a PEP
    jurisdiction VARCHAR(10),      -- ISO country code
    position_held TEXT,            -- "Minister of Finance"
    start_date DATE,
    end_date DATE,
    
    -- Source
    source_type VARCHAR(50),       -- 'screening_provider', 'manual', 'document'
    source_reference TEXT,
    screening_id UUID REFERENCES kyc.screenings(screening_id),
    
    -- Statement link
    statement_id UUID REFERENCES "ob-poc".bods_statements(statement_id),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_bods_pep_person ON "ob-poc".bods_pep_status(person_id);
CREATE INDEX idx_bods_pep_status ON "ob-poc".bods_pep_status(status);
```

### 7. `bods_interest_components` - Indirect Chain Tracking

```sql
-- Tracks the chain of intermediate entities in indirect ownership
CREATE TABLE "ob-poc".bods_interest_components (
    component_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- The primary (end-to-end) interest this is part of
    primary_interest_id UUID NOT NULL REFERENCES "ob-poc".bods_interests(interest_id),
    
    -- The component interest (one hop in the chain)
    component_interest_id UUID NOT NULL REFERENCES "ob-poc".bods_interests(interest_id),
    
    -- Position in chain (1 = first hop, 2 = second hop, etc.)
    chain_position INTEGER NOT NULL,
    
    -- Calculated share at this point in chain
    cumulative_share NUMERIC(5,2),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(primary_interest_id, component_interest_id)
);

CREATE INDEX idx_bods_components_primary ON "ob-poc".bods_interest_components(primary_interest_id);
```

---

## DSL Verb Updates

### Enhanced `cbu.add-owner`

```yaml
cbu.add-owner:
  description: "Add ownership relationship with BODS-compliant interest type"
  params:
    - name: subject
      type: entity_ref
      required: true
      description: "Entity being owned (the subject)"
    - name: owner
      type: ref  # Can be entity or person
      required: true
      description: "The interested party (owner/controller)"
    - name: interest-type
      type: bods_interest_type
      required: true
      description: "BODS interest type code"
      enum: [shareholding, votingRights, appointmentOfBoard, ...]
    - name: share
      type: percentage_or_range
      required: false
      description: "Exact percentage or {min, max} range"
    - name: direct
      type: boolean
      required: false
      default: null  # unknown
      description: "Is this direct ownership?"
    - name: is-ubo
      type: boolean
      required: false
      default: false
      description: "Does this make owner a beneficial owner?"
    - name: start-date
      type: date
      required: false
    - name: evidence
      type: document_ref[]
      required: false
  returns: interest_id

  # Example usage
  example: |
    (cbu.add-owner 
      :subject @allianz-se 
      :owner @oliver-baete 
      :interest-type "boardChair"
      :is-ubo true
      :start-date "2015-05-07")
```

### New `cbu.trace-ubo-chain`

```yaml
cbu.trace-ubo-chain:
  description: "Trace UBO chain and create component records"
  params:
    - name: from
      type: entity_ref
      required: true
      description: "Starting entity"
    - name: to
      type: person_ref
      required: true
      description: "Ultimate beneficial owner"
    - name: threshold
      type: percentage
      required: false
      default: 25
      description: "Minimum ownership threshold for UBO"
  returns: 
    primary_interest_id: uuid
    chain_length: integer
    cumulative_share: percentage
    component_ids: uuid[]
  
  # Example usage
  example: |
    (cbu.trace-ubo-chain 
      :from @allianz-se 
      :to @oliver-baete
      :threshold 25)
    
    ; Returns:
    ; { primary_interest_id: "...",
    ;   chain_length: 3,
    ;   cumulative_share: 32.5,
    ;   component_ids: ["...", "...", "..."] }
```

### Enhanced `gleif.import-tree`

```yaml
gleif.import-tree:
  description: "Import GLEIF ownership tree as BODS-compliant interests"
  params:
    - name: lei
      type: string
      required: true
    - name: depth
      type: integer
      required: false
      default: 5
    - name: include-components
      type: boolean
      required: false
      default: true
      description: "Create component records for indirect chains"
  
  behavior: |
    1. Fetch GLEIF relationships (direct/ultimate parents)
    2. Create bods_entities for each entity in tree
    3. Create bods_entity_identifiers with LEI for each
    4. Create bods_interests with:
       - interest_type = 'shareholding' or 'otherInfluenceOrControl'
       - direct_or_indirect = 'direct' or 'indirect'
       - share from GLEIF (exact or range)
    5. Create bods_interest_components for indirect chains
    6. Create bods_statements for audit trail
```

### New `bods.validate-cbu`

```yaml
bods.validate-cbu:
  description: "Validate CBU ownership structure against BODS schema"
  params:
    - name: cbu
      type: cbu_ref
      required: true
  returns:
    valid: boolean
    errors: validation_error[]
    warnings: validation_warning[]
  
  validations:
    - All interests have valid interest_type
    - All entities have bods_entity_type
    - Indirect interests have component records
    - Share ranges are valid (min <= max)
    - UBO flags only on person owners
    - No circular ownership
```

### New `bods.export-package`

```yaml
bods.export-package:
  description: "Export CBU as BODS 0.4 compliant JSON package"
  params:
    - name: cbu
      type: cbu_ref
      required: true
    - name: format
      type: string
      required: false
      default: "json"
      enum: ["json", "jsonl"]
    - name: as-of-date
      type: date
      required: false
  returns: bods_package_json
```

---

## Migration Path

### Phase 1: Add BODS Tables (Non-Breaking)

```sql
-- Run these migrations, existing tables unchanged
CREATE TABLE bods_statements ...
CREATE TABLE bods_interest_types ...
CREATE TABLE bods_interests ...
CREATE TABLE bods_entity_identifiers ...
CREATE TABLE bods_entity_types ...
CREATE TABLE bods_pep_status ...
CREATE TABLE bods_interest_components ...
```

### Phase 2: Populate from Existing Data

```sql
-- Migrate existing ownership_relationships to bods_interests
INSERT INTO "ob-poc".bods_interests (
    cbu_id,
    subject_entity_id,
    interested_party_entity_id,
    interest_type,
    direct_or_indirect,
    share_exact,
    start_date,
    end_date
)
SELECT 
    c.cbu_id,
    o.owned_entity_id,
    o.owner_entity_id,
    CASE o.ownership_type
        WHEN 'EQUITY' THEN 'shareholding'
        WHEN 'VOTING' THEN 'votingRights'
        WHEN 'CONTROL' THEN 'otherInfluenceOrControl'
        ELSE 'shareholding'
    END,
    'unknown',  -- Direct/indirect unknown from old data
    o.ownership_percent,
    o.effective_from,
    o.effective_to
FROM "ob-poc".ownership_relationships o
JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = o.owned_entity_id
JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id;

-- Migrate LEIs to bods_entity_identifiers
-- (Requires GLEIF records table - adjust as needed)
```

### Phase 3: Update DSL Verbs

- `cbu.add-owner` writes to `bods_interests`
- `cbu.trace-ubo` reads from `bods_interests` + creates components
- `gleif.enrich` populates `bods_entity_identifiers`
- New verbs: `bods.validate-cbu`, `bods.export-package`

### Phase 4: Deprecate Old Tables

- Mark `ownership_relationships` as deprecated
- Read from `bods_interests` everywhere
- Keep `ownership_relationships` as fallback/legacy view

---

## Query Examples

### Get All UBOs for a CBU (BODS-Native)

```sql
SELECT 
    p.first_name || ' ' || p.last_name AS ubo_name,
    i.interest_type,
    COALESCE(i.share_exact, 
        i.share_minimum || '-' || i.share_maximum) AS share,
    i.direct_or_indirect,
    pep.status AS pep_status
FROM "ob-poc".bods_interests i
JOIN "ob-poc".entity_proper_persons p 
    ON p.proper_person_id = i.interested_party_person_id
LEFT JOIN "ob-poc".bods_pep_status pep 
    ON pep.person_id = p.proper_person_id
WHERE i.cbu_id = @cbu_id
  AND i.beneficial_ownership_or_control = true
  AND i.end_date IS NULL;
```

### Trace Ownership Chain with Components

```sql
WITH RECURSIVE chain AS (
    -- Start from the anchor entity
    SELECT 
        i.interest_id,
        i.subject_entity_id,
        i.interested_party_entity_id,
        i.interested_party_person_id,
        i.share_exact,
        i.interest_type,
        1 AS depth,
        ARRAY[i.interest_id] AS path
    FROM "ob-poc".bods_interests i
    WHERE i.subject_entity_id = @anchor_entity_id
    
    UNION ALL
    
    -- Traverse up the chain
    SELECT 
        i.interest_id,
        i.subject_entity_id,
        i.interested_party_entity_id,
        i.interested_party_person_id,
        i.share_exact,
        i.interest_type,
        c.depth + 1,
        c.path || i.interest_id
    FROM "ob-poc".bods_interests i
    JOIN chain c ON c.interested_party_entity_id = i.subject_entity_id
    WHERE c.depth < 10
      AND NOT i.interest_id = ANY(c.path)  -- Prevent cycles
)
SELECT * FROM chain;
```

### Export as BODS JSON

```sql
SELECT jsonb_agg(
    jsonb_build_object(
        'statementId', s.statement_id,
        'statementDate', s.statement_date,
        'recordType', s.record_type,
        'recordDetails', CASE s.record_type
            WHEN 'relationship' THEN (
                SELECT jsonb_build_object(
                    'isComponent', i.is_component,
                    'subject', i.subject_entity_id,
                    'interestedParty', COALESCE(
                        i.interested_party_entity_id::text,
                        i.interested_party_person_id::text
                    ),
                    'interests', jsonb_build_array(
                        jsonb_build_object(
                            'type', i.interest_type,
                            'directOrIndirect', i.direct_or_indirect,
                            'beneficialOwnershipOrControl', i.beneficial_ownership_or_control,
                            'share', jsonb_build_object(
                                'exact', i.share_exact,
                                'minimum', i.share_minimum,
                                'maximum', i.share_maximum
                            )
                        )
                    )
                )
                FROM "ob-poc".bods_interests i
                WHERE i.statement_id = s.statement_id
            )
            -- ... entity and person cases
        END
    )
) AS bods_package
FROM "ob-poc".bods_statements s
WHERE s.record_id IN (
    SELECT interest_id FROM "ob-poc".bods_interests WHERE cbu_id = @cbu_id
);
```

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `migrations/XXX_bods_integration.sql` | CREATE | New BODS tables |
| `rust/config/verbs/cbu.yaml` | MODIFY | Update cbu.add-owner, add cbu.trace-ubo-chain |
| `rust/config/verbs/bods.yaml` | CREATE | New bods.* verbs |
| `rust/config/verbs/gleif.yaml` | MODIFY | Update gleif.import-tree for components |
| `rust/src/services/bods_service.rs` | CREATE | BODS export/validate logic |
| `rust/src/dsl_v2/custom_ops/cbu_ops.rs` | MODIFY | Write to bods_interests |
| `rust/src/dsl_v2/custom_ops/gleif_ops.rs` | MODIFY | Populate identifiers + components |
| `data/config/bods_interest_types.yaml` | CREATE | Interest type codelist |

---

## Summary: What This Integration Gives You

| Capability | Before | After |
|------------|--------|-------|
| **Audit Trail** | None | Every claim has statement_id + date |
| **Interest Types** | Free text | 22 standardized BODS types |
| **Direct/Indirect** | Implicit | Explicit field + component tracking |
| **Share Ranges** | Exact only | Exact OR min/max range |
| **PEP Details** | Simple status | Full PEP with jurisdiction/dates |
| **Identifiers** | LEI scattered | Unified multi-scheme array |
| **Unknown Owners** | Not tracked | Explicit anonymousEntity/unknownPerson |
| **Chain Tracing** | Manual | Automatic component records |
| **Export** | Custom | BODS 0.4 compliant |
| **Validation** | Ad-hoc | Schema-enforced |
