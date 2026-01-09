# BODS + GLEIF Integration Crosswalk

> Based on OpenGPT's recommendations + actual OB-POC current state
> Goal: Smallest additive change set that keeps existing pipeline stable

---

## Current State Summary

### Your Core Structs (Rust)

```rust
// CbuRow - Client Business Unit
pub struct CbuRow {
    pub cbu_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
    pub source_of_funds: Option<String>,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
}

// EntityRow - Generic entity (hub for all entity types)
pub struct EntityRow {
    pub entity_id: Uuid,
    pub entity_type_id: Uuid,  // FK to entity_types
    pub external_id: Option<String>,
    pub name: String,
}

// ProperPersonRow - Natural person details
pub struct ProperPersonRow {
    pub proper_person_id: Uuid,  // Same as entity_id
    pub first_name: String,
    pub last_name: String,
    pub middle_names: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub residence_address: Option<String>,
    pub id_document_type: Option<String>,
    pub id_document_number: Option<String>,
}
```

### Your Edge Model (from ubo.yaml verbs)

```yaml
# Entity relationships table schema (inferred from verbs):
entity_relationships:
  relationship_id: uuid PK
  from_entity_id: uuid FK  # owner/controller
  to_entity_id: uuid FK    # owned/controlled
  relationship_type: enum  # ownership, control, trust_role, UBO_TERMINUS
  percentage: decimal      # for ownership
  ownership_type: string   # DIRECT, INDIRECT, BENEFICIAL, etc.
  control_type: string     # board_member, executive, voting_rights, etc.
  trust_role: string       # settlor, trustee, beneficiary, protector
  trust_interest_type: string  # fixed, discretionary, contingent
  effective_from: date
  effective_to: date
  source_document_id: uuid FK
  notes: text
```

---

## OpenGPT's Key Insight: Two Separate Edge Classes

```
GLEIF Relationships (Corporate Hierarchy)    UBO Relationships (Beneficial Ownership)
────────────────────────────────────────    ────────────────────────────────────────
Purpose: Accounting consolidation            Purpose: KYC/AML compliance
Source: GLEIF API                            Source: Documents, declarations, research
Semantics: "legally owns for consolidation"  Semantics: "ultimately benefits/controls"

Edge types:                                  Edge types:
├── DIRECT_PARENT                           ├── shareholding (BODS)
├── ULTIMATE_PARENT                         ├── votingRights (BODS)
├── DIRECTLY_CONSOLIDATED                   ├── boardMember (BODS)
└── ULTIMATELY_CONSOLIDATED                 ├── trustee (BODS)
                                            ├── settlor (BODS)
                                            ├── beneficiary (BODS)
                                            └── ... (22 BODS types)

DON'T MIX THESE! Keep them as separate edge classes.
```

---

## Crosswalk: BODS → OB-POC

### Entities

| BODS Field | OB-POC Location | Notes |
|------------|-----------------|-------|
| `entityRecord.name` | `entities.name` | Direct |
| `entityRecord.entityType.type` | `entity_types.code` | Need mapping table |
| `entityRecord.jurisdiction.code` | `entity_limited_companies.jurisdiction` | Per-type table |
| `entityRecord.identifiers[].id` (LEI) | **NEW: `entity_identifiers`** | Need new table |
| `entityRecord.identifiers[].scheme` | **NEW: `entity_identifiers.scheme`** | LEI, company_register, etc. |
| `entityRecord.foundingDate` | `entity_limited_companies.incorporation_date` | Per-type table |
| `entityRecord.addresses[]` | Scattered | Need consolidation |

### Persons

| BODS Field | OB-POC Location | Notes |
|------------|-----------------|-------|
| `personRecord.names[].fullName` | `entity_proper_persons.first_name + last_name` | Concatenated |
| `personRecord.names[].familyName` | `entity_proper_persons.last_name` | Direct |
| `personRecord.names[].givenName` | `entity_proper_persons.first_name` | Direct |
| `personRecord.personType` | **NEW: `entity_proper_persons.person_type`** | knownPerson, anonymousPerson, unknownPerson |
| `personRecord.nationalities[]` | `entity_proper_persons.nationality` | Single only, need array |
| `personRecord.birthDate` | `entity_proper_persons.date_of_birth` | Direct |
| `personRecord.politicalExposure` | **NEW: `person_pep_status`** | Need new table |

### Relationships (The Key Part)

| BODS Field | OB-POC Location | Notes |
|------------|-----------------|-------|
| `relationshipRecord.subject` | `entity_relationships.to_entity_id` | Entity being owned |
| `relationshipRecord.interestedParty` | `entity_relationships.from_entity_id` | Owner/controller |
| `relationshipRecord.isComponent` | **NEW** | For indirect chain tracking |
| `relationshipRecord.componentRecords[]` | **NEW** | Links intermediate entities |
| `interests[].type` | `entity_relationships.relationship_type` | Need BODS codelist |
| `interests[].directOrIndirect` | **NEW: `entity_relationships.direct_or_indirect`** | Add column |
| `interests[].beneficialOwnershipOrControl` | **DERIVE** | From v_ubo_candidates view |
| `interests[].share.exact` | `entity_relationships.percentage` | Direct |
| `interests[].share.minimum` | **NEW** | For ranges |
| `interests[].share.maximum` | **NEW** | For ranges |
| `interests[].startDate` | `entity_relationships.effective_from` | Direct |
| `interests[].endDate` | `entity_relationships.effective_to` | Direct |

---

## Crosswalk: GLEIF → OB-POC

**Critical: Keep GLEIF relationships SEPARATE from UBO relationships**

| GLEIF Field | OB-POC Location | Notes |
|-------------|-----------------|-------|
| `lei` | **NEW: `entity_identifiers`** | scheme='LEI' |
| `entity.legalName` | `entities.name` | Direct |
| `entity.jurisdiction` | `entity_limited_companies.jurisdiction` | Direct |
| `entity.registrationStatus` | **NEW: `entity_identifiers.lei_status`** | ISSUED, LAPSED, etc. |
| `relationship.type` | **NEW: `gleif_relationships.relationship_type`** | Separate table! |
| `relationship.startDate` | **NEW: `gleif_relationships.start_date`** | |
| `relationship.endDate` | **NEW: `gleif_relationships.end_date`** | |

---

## Minimal Additive Changes

### 1. New Table: `entity_identifiers` (LEI spine)

```sql
CREATE TABLE "ob-poc".entity_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- BODS identifier structure
    scheme VARCHAR(50) NOT NULL,  -- 'LEI', 'company_register', 'tax_id'
    id VARCHAR(100) NOT NULL,
    uri VARCHAR(500),
    
    -- LEI-specific (when scheme = 'LEI')
    lei_status VARCHAR(30),        -- 'ISSUED', 'LAPSED', 'RETIRED'
    lei_next_renewal DATE,
    lei_managing_lou VARCHAR(100),
    
    -- Validation
    is_validated BOOLEAN DEFAULT false,
    validated_at TIMESTAMPTZ,
    validation_source VARCHAR(100),
    
    -- Temporal
    effective_from DATE,
    effective_to DATE,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(entity_id, scheme, id)
);

CREATE INDEX idx_entity_identifiers_lei ON "ob-poc".entity_identifiers(id) 
    WHERE scheme = 'LEI';
```

### 2. New Table: `gleif_relationships` (Corporate Hierarchy - SEPARATE)

```sql
-- SEPARATE from entity_relationships (UBO edges)
-- This is GLEIF's view of corporate structure, not KYC beneficial ownership
CREATE TABLE "ob-poc".gleif_relationships (
    gleif_rel_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Parent (owner in GLEIF terms)
    parent_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    parent_lei VARCHAR(20) NOT NULL,
    
    -- Child (owned in GLEIF terms)
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    child_lei VARCHAR(20) NOT NULL,
    
    -- GLEIF relationship semantics
    relationship_type VARCHAR(50) NOT NULL,  -- DirectParent, UltimateParent, etc.
    relationship_status VARCHAR(30),         -- Active, Inactive
    
    -- Accounting consolidation info
    accounting_standard VARCHAR(50),
    
    -- Temporal
    start_date DATE,
    end_date DATE,
    
    -- Source tracking
    gleif_record_id VARCHAR(100),
    fetched_at TIMESTAMPTZ DEFAULT NOW(),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(parent_lei, child_lei, relationship_type)
);
```

### 3. Extend `entity_relationships` for BODS Alignment

```sql
-- Add columns to existing table (non-breaking)
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS interest_type VARCHAR(50),  -- BODS codelist
ADD COLUMN IF NOT EXISTS direct_or_indirect VARCHAR(10) DEFAULT 'unknown',
ADD COLUMN IF NOT EXISTS share_minimum NUMERIC(5,2),
ADD COLUMN IF NOT EXISTS share_maximum NUMERIC(5,2),
ADD COLUMN IF NOT EXISTS is_component BOOLEAN DEFAULT false,
ADD COLUMN IF NOT EXISTS component_of_relationship_id UUID REFERENCES "ob-poc".entity_relationships(relationship_id);

-- Create BODS interest type lookup
CREATE TABLE "ob-poc".bods_interest_types (
    type_code VARCHAR(50) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    category VARCHAR(30) NOT NULL,
    bods_standard BOOLEAN DEFAULT true,
    display_order INTEGER DEFAULT 0
);

-- Populate with BODS 0.4 standard types
INSERT INTO "ob-poc".bods_interest_types (type_code, display_name, category) VALUES
('shareholding', 'Shareholding', 'ownership'),
('votingRights', 'Voting Rights', 'ownership'),
('appointmentOfBoard', 'Appointment of Board', 'control'),
('otherInfluenceOrControl', 'Other Influence or Control', 'control'),
('seniorManagingOfficial', 'Senior Managing Official', 'control'),
('settlor', 'Settlor', 'trust'),
('trustee', 'Trustee', 'trust'),
('protector', 'Protector', 'trust'),
('beneficiaryOfLegalArrangement', 'Beneficiary', 'trust'),
('nominee', 'Nominee', 'nominee'),
('nominator', 'Nominator', 'nominee'),
('boardMember', 'Board Member', 'control'),
('boardChair', 'Board Chair', 'control');
```

### 4. New Table: `person_pep_status` (BODS PEP Details)

```sql
CREATE TABLE "ob-poc".person_pep_status (
    pep_status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- BODS PEP fields
    status VARCHAR(20) NOT NULL CHECK (status IN ('isPep', 'isNotPep', 'unknown')),
    
    -- Per-role details
    reason TEXT,
    jurisdiction VARCHAR(10),
    position_held TEXT,
    start_date DATE,
    end_date DATE,
    
    -- Source
    screening_id UUID REFERENCES kyc.screenings(screening_id),
    source_type VARCHAR(50),
    source_reference TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Verb Updates (Minimal)

### Enhanced `gleif.attach-lei`

```yaml
gleif:
  attach-lei:
    description: Attach LEI to entity, validate against GLEIF, store in entity_identifiers
    behavior: plugin
    args:
      - name: entity-id
        type: uuid
        required: true
        lookup:
          table: entities
          schema: ob-poc
          search_key: name
          primary_key: entity_id
      - name: lei
        type: string
        required: true
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
```

### New `gleif.import-hierarchy`

```yaml
gleif:
  import-hierarchy:
    description: Import GLEIF corporate hierarchy (SEPARATE from UBO)
    behavior: plugin
    args:
      - name: lei
        type: string
        required: true
      - name: depth
        type: integer
        required: false
        default: 5
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
      description: Summary of imported entities and gleif_relationships
```

### Enhanced `ubo.add-ownership`

```yaml
ubo:
  add-ownership:
    # ... existing args ...
    args:
      # ... existing args ...
      - name: interest-type
        type: string
        required: false
        maps_to: interest_type
        lookup:
          table: bods_interest_types
          schema: ob-poc
          search_key: type_code
          primary_key: type_code
        default: shareholding
      - name: direct
        type: boolean
        required: false
        description: Is this direct ownership?
        # Maps to direct_or_indirect column
```

---

## Query: Get Entity with All Identifiers

```sql
SELECT 
    e.entity_id,
    e.name,
    et.code AS entity_type,
    jsonb_agg(
        jsonb_build_object(
            'scheme', ei.scheme,
            'id', ei.id,
            'status', CASE WHEN ei.scheme = 'LEI' THEN ei.lei_status END
        )
    ) FILTER (WHERE ei.identifier_id IS NOT NULL) AS identifiers
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
LEFT JOIN "ob-poc".entity_identifiers ei ON ei.entity_id = e.entity_id
WHERE e.entity_id = $1
GROUP BY e.entity_id, e.name, et.code;
```

## Query: Get UBO Chains (Ignoring GLEIF Hierarchy)

```sql
-- Uses entity_relationships (UBO edges), NOT gleif_relationships
WITH RECURSIVE ubo_chain AS (
    SELECT 
        er.from_entity_id AS owner_id,
        er.to_entity_id AS owned_id,
        er.percentage,
        er.interest_type,
        er.direct_or_indirect,
        1 AS depth,
        ARRAY[er.from_entity_id] AS path
    FROM "ob-poc".entity_relationships er
    WHERE er.to_entity_id = $1  -- Start from target entity
      AND er.relationship_type IN ('ownership', 'control')
      AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
    
    UNION ALL
    
    SELECT 
        er.from_entity_id,
        er.to_entity_id,
        er.percentage,
        er.interest_type,
        er.direct_or_indirect,
        c.depth + 1,
        c.path || er.from_entity_id
    FROM "ob-poc".entity_relationships er
    JOIN ubo_chain c ON c.owner_id = er.to_entity_id
    WHERE c.depth < 10
      AND NOT er.from_entity_id = ANY(c.path)
      AND er.relationship_type IN ('ownership', 'control')
)
SELECT * FROM ubo_chain;
```

---

## Summary: What to Build

| Priority | Change | Purpose |
|----------|--------|---------|
| **P0** | `entity_identifiers` table | LEI as global spine |
| **P0** | `gleif_relationships` table | Separate GLEIF from UBO |
| **P1** | Extend `entity_relationships` | BODS interest_type, direct_or_indirect |
| **P1** | `bods_interest_types` codelist | Standardized 22 types |
| **P2** | `person_pep_status` table | BODS PEP details |
| **P2** | `gleif.attach-lei` verb | LEI management |
| **P3** | `gleif.import-hierarchy` verb | GLEIF tree import |
| **P3** | BODS export capability | Export as standard format |

**Key Principle:** LEI/GLEIF is the entity spine, UBO edges stay separate, BODS provides the vocabulary.
