# KYC Observation Model Refactoring Specification

## Overview

This document specifies the refactoring of the OB-POC attribute system from a "single source of truth" model to an **observation-based model** that accurately reflects KYC reality.

**Key Insight**: KYC is not about storing THE value of an attribute. It's about collecting OBSERVATIONS from SOURCES and making decisions based on their reconciliation.

## Business Context

### The KYC Reality

```
Client alleges: "Our UBO is Adam Timothy Cearns, DOB 1980-01-15"

Evidence collected:
  - UK Passport:    "Adam Cearns",     DOB 1980-01-15
  - Swiss Passport: "Adam T Cearns",   DOB 1980-01-15  
  - Utility Bill:   "A. Cearns"
  
KYC Decision: Allegations VERIFIED (name variations are acceptable, DOB confirmed)
```

There is no single "correct" name - there are observations from authoritative sources that corroborate the client's allegations.

### Current Problem

The existing `attribute_values_typed` table assumes one value per attribute per entity:
```sql
-- Current: Forces artificial "pick one" decisions
entity_id + attribute_id → single value
```

### Target State

Multiple observations per attribute, each with source provenance:
```sql
-- Target: Captures reality
entity_id + attribute_id → [observation₁, observation₂, ... observationₙ]
```

---

## Architecture

### The Observation Triangle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CLIENT ALLEGATIONS                                   │
│  "The client claims..." (unverified starting point)                         │
│  Source: Onboarding form, KYC questionnaire, email                          │
└────────────────────────────────────────┬────────────────────────────────────┘
                                         │
                                         │ verification
                                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       ATTRIBUTE OBSERVATIONS                                 │
│  Multiple observations per attribute from different sources                  │
│  Each with: source_type, source_document, confidence, is_authoritative      │
└────────────────────────────────────────┬────────────────────────────────────┘
                                         │
                            ┌────────────┴────────────┐
                            │                         │
                            ▼                         ▼
              ┌─────────────────────┐   ┌─────────────────────┐
              │   SOURCE DOCUMENTS  │   │   SINK DOCUMENTS    │
              │   (extraction)      │   │   (fulfillment)     │
              │                     │   │                     │
              │   Passport PROVIDES │   │   Identity REQUIRES │
              │   name, DOB, etc.   │   │   passport as proof │
              └─────────────────────┘   └─────────────────────┘
```

### Document-Attribute Bidirectional Links

```
DOCUMENT TYPE                    ATTRIBUTE                      DIRECTION
─────────────────────────────────────────────────────────────────────────────
PASSPORT          ───extracts──▶  attr.identity.full_name       SOURCE
PASSPORT          ───extracts──▶  attr.identity.dob             SOURCE
PASSPORT          ───extracts──▶  attr.identity.nationality     SOURCE
PASSPORT          ◀──proves─────  attr.identity.verified        SINK

UTILITY_BILL      ───extracts──▶  attr.address.residential      SOURCE
UTILITY_BILL      ◀──proves─────  attr.address.proof            SINK

CERT_OF_INCORP    ───extracts──▶  attr.entity.legal_name        SOURCE
CERT_OF_INCORP    ───extracts──▶  attr.entity.registration_num  SOURCE
CERT_OF_INCORP    ◀──proves─────  attr.entity.existence         SINK
```

---

## Database Schema Changes

### Phase 1: New Tables

#### 1.1 Attribute Observations

Replaces `attribute_values_typed` as the primary attribute storage.

```sql
-- ============================================================================
-- ATTRIBUTE OBSERVATIONS
-- Multiple observations per attribute, each with source provenance
-- ============================================================================

CREATE TABLE "ob-poc".attribute_observations (
    observation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What entity and attribute this observation is about
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    
    -- The observed value (polymorphic - exactly one must be set)
    value_text TEXT,
    value_number NUMERIC,
    value_boolean BOOLEAN,
    value_date DATE,
    value_datetime TIMESTAMPTZ,
    value_json JSONB,
    
    -- SOURCE PROVENANCE
    source_type VARCHAR(30) NOT NULL,
    source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    source_workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id),
    source_screening_id UUID REFERENCES kyc.screenings(screening_id),
    source_reference TEXT,  -- External reference (provider ID, etc.)
    source_metadata JSONB DEFAULT '{}',  -- Additional source context
    
    -- Observation quality
    confidence NUMERIC(3,2) DEFAULT 0.50 CHECK (confidence >= 0 AND confidence <= 1),
    is_authoritative BOOLEAN DEFAULT FALSE,
    extraction_method VARCHAR(50),  -- OCR, MRZ, NLP, AI, MANUAL
    
    -- Lifecycle
    observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observed_by TEXT,  -- system, user email, AI model name
    status VARCHAR(30) DEFAULT 'ACTIVE',
    superseded_by UUID REFERENCES "ob-poc".attribute_observations(observation_id),
    superseded_at TIMESTAMPTZ,
    
    -- Temporal validity (for time-bounded attributes like addresses)
    effective_from DATE,
    effective_to DATE,
    
    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT check_source_type CHECK (source_type IN (
        'ALLEGATION',     -- Client stated this
        'DOCUMENT',       -- Extracted from document
        'SCREENING',      -- From screening provider (PEP, sanctions)
        'THIRD_PARTY',    -- Credit bureau, registry lookup
        'SYSTEM',         -- Calculated/derived by system
        'DERIVED',        -- Computed from other observations
        'MANUAL'          -- Analyst manually entered
    )),
    
    CONSTRAINT check_status CHECK (status IN (
        'ACTIVE',         -- Current valid observation
        'SUPERSEDED',     -- Replaced by newer observation
        'DISPUTED',       -- Under review/contested
        'WITHDRAWN',      -- Source retracted
        'REJECTED'        -- Determined to be incorrect
    )),
    
    CONSTRAINT check_single_value CHECK (
        (CASE WHEN value_text IS NOT NULL THEN 1 ELSE 0 END +
         CASE WHEN value_number IS NOT NULL THEN 1 ELSE 0 END +
         CASE WHEN value_boolean IS NOT NULL THEN 1 ELSE 0 END +
         CASE WHEN value_date IS NOT NULL THEN 1 ELSE 0 END +
         CASE WHEN value_datetime IS NOT NULL THEN 1 ELSE 0 END +
         CASE WHEN value_json IS NOT NULL THEN 1 ELSE 0 END) = 1
    ),
    
    CONSTRAINT check_document_source CHECK (
        source_type != 'DOCUMENT' OR source_document_id IS NOT NULL
    )
);

-- Indexes
CREATE INDEX idx_obs_entity_attr ON "ob-poc".attribute_observations(entity_id, attribute_id);
CREATE INDEX idx_obs_entity_active ON "ob-poc".attribute_observations(entity_id) WHERE status = 'ACTIVE';
CREATE INDEX idx_obs_source_doc ON "ob-poc".attribute_observations(source_document_id) 
    WHERE source_document_id IS NOT NULL;
CREATE INDEX idx_obs_source_type ON "ob-poc".attribute_observations(source_type);
CREATE INDEX idx_obs_attribute ON "ob-poc".attribute_observations(attribute_id);

COMMENT ON TABLE "ob-poc".attribute_observations IS 
    'Observation-based attribute storage. Multiple observations per attribute per entity, each with source provenance.';
```

#### 1.2 Client Allegations

The starting point for KYC - what the client claims.

```sql
-- ============================================================================
-- CLIENT ALLEGATIONS
-- What the client claims about their structure (unverified starting point)
-- ============================================================================

CREATE TABLE "ob-poc".client_allegations (
    allegation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Context
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    case_id UUID REFERENCES kyc.cases(case_id),
    workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id),
    
    -- What is being alleged
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    
    -- The alleged value (stored as JSONB for flexibility)
    alleged_value JSONB NOT NULL,
    alleged_value_display TEXT,  -- Human-readable form
    
    -- Allegation source
    alleged_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    alleged_by TEXT,  -- Contact person at client
    allegation_source VARCHAR(50) NOT NULL,
    allegation_reference TEXT,  -- Form ID, email reference, etc.
    
    -- Verification outcome
    verification_status VARCHAR(30) DEFAULT 'PENDING',
    verified_by_observation_id UUID REFERENCES "ob-poc".attribute_observations(observation_id),
    verification_result VARCHAR(30),
    verification_notes TEXT,
    verified_at TIMESTAMPTZ,
    verified_by TEXT,  -- Analyst who verified
    
    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT check_allegation_source CHECK (allegation_source IN (
        'ONBOARDING_FORM',
        'KYC_QUESTIONNAIRE', 
        'EMAIL',
        'VERBAL',
        'API',
        'DOCUMENT',  -- Client-provided document (unverified)
        'PRIOR_CASE'  -- Carried forward from previous KYC
    )),
    
    CONSTRAINT check_verification_status CHECK (verification_status IN (
        'PENDING',        -- Not yet checked
        'IN_PROGRESS',    -- Being verified
        'VERIFIED',       -- Document confirms allegation
        'CONTRADICTED',   -- Document contradicts allegation
        'PARTIAL',        -- Partially confirmed (e.g., name spelling differs)
        'UNVERIFIABLE',   -- Cannot be verified (no document available)
        'WAIVED'          -- Verification requirement waived
    )),
    
    CONSTRAINT check_verification_result CHECK (verification_result IS NULL OR verification_result IN (
        'EXACT_MATCH',
        'ACCEPTABLE_VARIATION',
        'MATERIAL_DISCREPANCY',
        'CONTRADICTION',
        'INCONCLUSIVE'
    ))
);

-- Indexes
CREATE INDEX idx_alleg_cbu ON "ob-poc".client_allegations(cbu_id);
CREATE INDEX idx_alleg_entity ON "ob-poc".client_allegations(entity_id);
CREATE INDEX idx_alleg_case ON "ob-poc".client_allegations(case_id) WHERE case_id IS NOT NULL;
CREATE INDEX idx_alleg_pending ON "ob-poc".client_allegations(cbu_id) WHERE verification_status = 'PENDING';
CREATE INDEX idx_alleg_workstream ON "ob-poc".client_allegations(workstream_id) WHERE workstream_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".client_allegations IS 
    'Client allegations - the unverified claims that form the starting point of KYC verification.';
```

#### 1.3 Document-Attribute Links (Bidirectional)

Replaces `document_attribute_mappings` with explicit SOURCE/SINK direction.

```sql
-- ============================================================================
-- DOCUMENT-ATTRIBUTE LINKS (Bidirectional)
-- Defines which attributes a document can SOURCE (extract) or SINK (prove)
-- ============================================================================

CREATE TABLE "ob-poc".document_attribute_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- The relationship
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types(type_id),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    direction VARCHAR(10) NOT NULL,
    
    -- SOURCE direction config (extraction)
    extraction_method VARCHAR(50),
    extraction_field_path JSONB,  -- JSONPath or field locator
    extraction_confidence_default NUMERIC(3,2) DEFAULT 0.80,
    extraction_hints JSONB DEFAULT '{}',  -- AI hints, regex patterns, etc.
    
    -- SINK direction config (fulfillment/proof)
    is_authoritative BOOLEAN DEFAULT FALSE,  -- This doc type is THE definitive proof
    proof_strength VARCHAR(20),  -- PRIMARY, SECONDARY, SUPPORTING
    alternative_doc_types UUID[],  -- Other doc types that could also prove this
    
    -- Applicability constraints
    entity_types TEXT[],  -- Only for these entity types (NULL = all)
    jurisdictions TEXT[],  -- Only for these jurisdictions (NULL = all)
    client_types TEXT[],  -- Only for these client types (NULL = all)
    
    -- Metadata
    is_active BOOLEAN DEFAULT TRUE,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT check_direction CHECK (direction IN ('SOURCE', 'SINK', 'BOTH')),
    CONSTRAINT check_proof_strength CHECK (proof_strength IS NULL OR proof_strength IN (
        'PRIMARY',      -- Definitive proof (passport for identity)
        'SECONDARY',    -- Strong supporting evidence
        'SUPPORTING'    -- Additional corroboration
    )),
    CONSTRAINT check_extraction_config CHECK (
        direction = 'SINK' OR extraction_method IS NOT NULL
    ),
    CONSTRAINT unique_doc_attr_direction UNIQUE (document_type_id, attribute_id, direction)
);

-- Indexes
CREATE INDEX idx_dal_document ON "ob-poc".document_attribute_links(document_type_id);
CREATE INDEX idx_dal_attribute ON "ob-poc".document_attribute_links(attribute_id);
CREATE INDEX idx_dal_source ON "ob-poc".document_attribute_links(document_type_id) 
    WHERE direction IN ('SOURCE', 'BOTH');
CREATE INDEX idx_dal_sink ON "ob-poc".document_attribute_links(attribute_id) 
    WHERE direction IN ('SINK', 'BOTH');

COMMENT ON TABLE "ob-poc".document_attribute_links IS 
    'Bidirectional links between document types and attributes. SOURCE = document provides attribute value. SINK = attribute requires document as proof.';
```

#### 1.4 Observation Discrepancies

Track discrepancies found during reconciliation.

```sql
-- ============================================================================
-- OBSERVATION DISCREPANCIES
-- Tracks discrepancies found when comparing observations
-- ============================================================================

CREATE TABLE "ob-poc".observation_discrepancies (
    discrepancy_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Context
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    case_id UUID REFERENCES kyc.cases(case_id),
    workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id),
    
    -- The conflicting observations
    observation_1_id UUID NOT NULL REFERENCES "ob-poc".attribute_observations(observation_id),
    observation_2_id UUID NOT NULL REFERENCES "ob-poc".attribute_observations(observation_id),
    
    -- Discrepancy details
    discrepancy_type VARCHAR(30) NOT NULL,
    severity VARCHAR(20) NOT NULL,
    description TEXT NOT NULL,
    value_1_display TEXT,
    value_2_display TEXT,
    
    -- Resolution
    resolution_status VARCHAR(30) DEFAULT 'OPEN',
    resolution_type VARCHAR(30),
    resolution_notes TEXT,
    resolved_at TIMESTAMPTZ,
    resolved_by TEXT,
    
    -- If resolved, which observation was accepted?
    accepted_observation_id UUID REFERENCES "ob-poc".attribute_observations(observation_id),
    
    -- Did this create a red flag?
    red_flag_id UUID REFERENCES kyc.red_flags(red_flag_id),
    
    -- Audit
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    detected_by TEXT DEFAULT 'SYSTEM',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT check_discrepancy_type CHECK (discrepancy_type IN (
        'VALUE_MISMATCH',       -- Different values
        'DATE_MISMATCH',        -- Different dates
        'SPELLING_VARIATION',   -- Minor spelling difference
        'FORMAT_DIFFERENCE',    -- Same value, different format
        'MISSING_VS_PRESENT',   -- One source has value, other doesn't
        'CONTRADICTORY'         -- Mutually exclusive values
    )),
    
    CONSTRAINT check_severity CHECK (severity IN (
        'INFO',       -- Noted but not concerning (spelling variation)
        'LOW',        -- Minor discrepancy, monitor
        'MEDIUM',     -- Requires investigation
        'HIGH',       -- Material discrepancy, escalate
        'CRITICAL'    -- Potential fraud indicator
    )),
    
    CONSTRAINT check_resolution_status CHECK (resolution_status IN (
        'OPEN',
        'INVESTIGATING',
        'RESOLVED',
        'ESCALATED',
        'ACCEPTED'  -- Discrepancy acknowledged but accepted
    )),
    
    CONSTRAINT check_resolution_type CHECK (resolution_type IS NULL OR resolution_type IN (
        'ACCEPTABLE_VARIATION',
        'SOURCE_ERROR',
        'DATA_ENTRY_ERROR',
        'LEGITIMATE_CHANGE',
        'FRAUD_CONFIRMED',
        'FALSE_POSITIVE',
        'WAIVED'
    ))
);

-- Indexes
CREATE INDEX idx_disc_entity ON "ob-poc".observation_discrepancies(entity_id);
CREATE INDEX idx_disc_case ON "ob-poc".observation_discrepancies(case_id) WHERE case_id IS NOT NULL;
CREATE INDEX idx_disc_open ON "ob-poc".observation_discrepancies(entity_id) WHERE resolution_status = 'OPEN';
CREATE INDEX idx_disc_severity ON "ob-poc".observation_discrepancies(severity) WHERE resolution_status = 'OPEN';

COMMENT ON TABLE "ob-poc".observation_discrepancies IS 
    'Tracks discrepancies detected between attribute observations during KYC reconciliation.';
```

### Phase 2: Modify Existing Tables

#### 2.1 Add entity_id to document_catalog

```sql
-- Add entity linkage to documents
ALTER TABLE "ob-poc".document_catalog 
ADD COLUMN entity_id UUID REFERENCES "ob-poc".entities(entity_id);

CREATE INDEX idx_doc_entity ON "ob-poc".document_catalog(entity_id) WHERE entity_id IS NOT NULL;

COMMENT ON COLUMN "ob-poc".document_catalog.entity_id IS 
    'The entity this document relates to (e.g., passport belongs to person, cert of incorp belongs to company)';
```

#### 2.2 Enhance attribute_registry

```sql
-- Add observation-related metadata to attribute registry
ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN reconciliation_rules JSONB DEFAULT '{}',
ADD COLUMN acceptable_variation_threshold NUMERIC(3,2),
ADD COLUMN requires_authoritative_source BOOLEAN DEFAULT FALSE;

COMMENT ON COLUMN "ob-poc".attribute_registry.reconciliation_rules IS 
    'Rules for comparing observations: {"allow_spelling_variation": true, "date_tolerance_days": 0}';
COMMENT ON COLUMN "ob-poc".attribute_registry.acceptable_variation_threshold IS 
    'Similarity threshold (0-1) for acceptable string variations';
COMMENT ON COLUMN "ob-poc".attribute_registry.requires_authoritative_source IS 
    'If true, at least one observation must be from an authoritative source';
```

### Phase 3: Views

```sql
-- ============================================================================
-- VIEWS
-- ============================================================================

-- Current value view (most recent authoritative observation, or highest confidence)
CREATE VIEW "ob-poc".v_attribute_current AS
SELECT DISTINCT ON (entity_id, attribute_id)
    entity_id,
    attribute_id,
    observation_id,
    value_text,
    value_number,
    value_boolean,
    value_date,
    value_datetime,
    value_json,
    source_type,
    source_document_id,
    confidence,
    is_authoritative,
    observed_at
FROM "ob-poc".attribute_observations
WHERE status = 'ACTIVE'
ORDER BY 
    entity_id, 
    attribute_id, 
    is_authoritative DESC,
    confidence DESC,
    observed_at DESC;

COMMENT ON VIEW "ob-poc".v_attribute_current IS 
    'Current "best" value for each attribute - prioritizes authoritative sources, then confidence, then recency';


-- Allegation verification status summary
CREATE VIEW "ob-poc".v_allegation_summary AS
SELECT 
    ca.cbu_id,
    ca.entity_id,
    e.name AS entity_name,
    COUNT(*) AS total_allegations,
    COUNT(*) FILTER (WHERE ca.verification_status = 'VERIFIED') AS verified,
    COUNT(*) FILTER (WHERE ca.verification_status = 'CONTRADICTED') AS contradicted,
    COUNT(*) FILTER (WHERE ca.verification_status = 'PARTIAL') AS partial,
    COUNT(*) FILTER (WHERE ca.verification_status = 'PENDING') AS pending,
    COUNT(*) FILTER (WHERE ca.verification_status = 'UNVERIFIABLE') AS unverifiable
FROM "ob-poc".client_allegations ca
JOIN "ob-poc".entities e ON ca.entity_id = e.entity_id
GROUP BY ca.cbu_id, ca.entity_id, e.name;


-- Open discrepancies requiring attention
CREATE VIEW "ob-poc".v_open_discrepancies AS
SELECT 
    od.*,
    e.name AS entity_name,
    ar.display_name AS attribute_name,
    ar.category AS attribute_category
FROM "ob-poc".observation_discrepancies od
JOIN "ob-poc".entities e ON od.entity_id = e.entity_id
JOIN "ob-poc".attribute_registry ar ON od.attribute_id = ar.uuid
WHERE od.resolution_status IN ('OPEN', 'INVESTIGATING')
ORDER BY 
    CASE od.severity 
        WHEN 'CRITICAL' THEN 1 
        WHEN 'HIGH' THEN 2 
        WHEN 'MEDIUM' THEN 3 
        WHEN 'LOW' THEN 4 
        ELSE 5 
    END,
    od.detected_at;


-- Document extraction coverage
CREATE VIEW "ob-poc".v_document_extraction_map AS
SELECT 
    dt.type_code AS document_type,
    dt.display_name AS document_name,
    ar.id AS attribute_id,
    ar.display_name AS attribute_name,
    dal.direction,
    dal.extraction_method,
    dal.is_authoritative,
    dal.proof_strength
FROM "ob-poc".document_attribute_links dal
JOIN "ob-poc".document_types dt ON dal.document_type_id = dt.type_id
JOIN "ob-poc".attribute_registry ar ON dal.attribute_id = ar.uuid
WHERE dal.is_active = TRUE
ORDER BY dt.type_code, dal.direction, ar.id;
```

---

## DSL Changes

### New Domain: allegation

```yaml
# Add to verbs.yaml
allegation:
  description: "Client allegations - unverified claims that start the KYC process"

  verbs:
    record:
      description: "Record a client allegation about an entity attribute"
      behavior: crud
      crud:
        operation: insert
        table: client_allegations
        schema: ob-poc
        returning: allegation_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: attribute
          type: lookup
          required: true
          maps_to: attribute_id
          lookup:
            table: attribute_registry
            schema: ob-poc
            code_column: id
            id_column: uuid
        - name: value
          type: json
          required: true
          maps_to: alleged_value
        - name: display-value
          type: string
          required: false
          maps_to: alleged_value_display
        - name: source
          type: string
          required: true
          maps_to: allegation_source
          valid_values: [ONBOARDING_FORM, KYC_QUESTIONNAIRE, EMAIL, VERBAL, API, DOCUMENT, PRIOR_CASE]
        - name: source-reference
          type: string
          required: false
          maps_to: allegation_reference
        - name: case-id
          type: uuid
          required: false
          maps_to: case_id
        - name: workstream-id
          type: uuid
          required: false
          maps_to: workstream_id
      returns:
        type: uuid
        name: allegation_id
        capture: true

    verify:
      description: "Mark allegation as verified by an observation"
      behavior: crud
      crud:
        operation: update
        table: client_allegations
        schema: ob-poc
        key: allegation_id
        set_values:
          verification_status: VERIFIED
          verified_at: now()
      args:
        - name: allegation-id
          type: uuid
          required: true
          maps_to: allegation_id
        - name: observation-id
          type: uuid
          required: true
          maps_to: verified_by_observation_id
        - name: result
          type: string
          required: false
          maps_to: verification_result
          valid_values: [EXACT_MATCH, ACCEPTABLE_VARIATION]
        - name: notes
          type: string
          required: false
          maps_to: verification_notes
      returns:
        type: affected

    contradict:
      description: "Mark allegation as contradicted by evidence"
      behavior: crud
      crud:
        operation: update
        table: client_allegations
        schema: ob-poc
        key: allegation_id
        set_values:
          verification_status: CONTRADICTED
          verified_at: now()
      args:
        - name: allegation-id
          type: uuid
          required: true
          maps_to: allegation_id
        - name: observation-id
          type: uuid
          required: true
          maps_to: verified_by_observation_id
        - name: result
          type: string
          required: false
          maps_to: verification_result
          valid_values: [MATERIAL_DISCREPANCY, CONTRADICTION]
        - name: notes
          type: string
          required: true
          maps_to: verification_notes
      returns:
        type: affected

    mark-partial:
      description: "Mark allegation as partially verified"
      behavior: crud
      crud:
        operation: update
        table: client_allegations
        schema: ob-poc
        key: allegation_id
        set_values:
          verification_status: PARTIAL
          verified_at: now()
      args:
        - name: allegation-id
          type: uuid
          required: true
          maps_to: allegation_id
        - name: observation-id
          type: uuid
          required: true
          maps_to: verified_by_observation_id
        - name: notes
          type: string
          required: true
          maps_to: verification_notes
      returns:
        type: affected

    list-by-entity:
      description: "List allegations for an entity"
      behavior: crud
      crud:
        operation: list_by_fk
        table: client_allegations
        schema: ob-poc
        fk_col: entity_id
      args:
        - name: entity-id
          type: uuid
          required: true
        - name: status
          type: string
          required: false
          maps_to: verification_status
      returns:
        type: record_set

    list-pending:
      description: "List pending allegations for a CBU"
      behavior: crud
      crud:
        operation: select
        table: client_allegations
        schema: ob-poc
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: verification_status
          type: string
          required: false
          default: "PENDING"
          maps_to: verification_status
      returns:
        type: record_set
```

### New Domain: observation

```yaml
observation:
  description: "Attribute observations from various sources"

  verbs:
    record:
      description: "Record an attribute observation"
      behavior: crud
      crud:
        operation: insert
        table: attribute_observations
        schema: ob-poc
        returning: observation_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: attribute
          type: lookup
          required: true
          maps_to: attribute_id
          lookup:
            table: attribute_registry
            schema: ob-poc
            code_column: id
            id_column: uuid
        - name: value
          type: string
          required: true
          maps_to: value_text  # Plugin will handle type dispatch
        - name: source-type
          type: string
          required: true
          maps_to: source_type
          valid_values: [ALLEGATION, DOCUMENT, SCREENING, THIRD_PARTY, SYSTEM, DERIVED, MANUAL]
        - name: source-document-id
          type: uuid
          required: false
          maps_to: source_document_id
        - name: confidence
          type: decimal
          required: false
          maps_to: confidence
          default: 0.50
        - name: is-authoritative
          type: boolean
          required: false
          maps_to: is_authoritative
          default: false
        - name: extraction-method
          type: string
          required: false
          maps_to: extraction_method
          valid_values: [OCR, MRZ, BARCODE, QR_CODE, NLP, AI, MANUAL, API]
      returns:
        type: uuid
        name: observation_id
        capture: true

    record-from-document:
      description: "Record observation extracted from a document"
      behavior: plugin
      handler: observation_from_document
      args:
        - name: entity-id
          type: uuid
          required: true
        - name: document-id
          type: uuid
          required: true
        - name: attribute
          type: string
          required: true
        - name: value
          type: string
          required: true
        - name: extraction-method
          type: string
          required: false
        - name: confidence
          type: decimal
          required: false
      returns:
        type: uuid
        name: observation_id
        capture: true

    supersede:
      description: "Supersede an observation with a newer one"
      behavior: crud
      crud:
        operation: update
        table: attribute_observations
        schema: ob-poc
        key: observation_id
        set_values:
          status: SUPERSEDED
          superseded_at: now()
      args:
        - name: observation-id
          type: uuid
          required: true
          maps_to: observation_id
        - name: superseded-by
          type: uuid
          required: true
          maps_to: superseded_by
      returns:
        type: affected

    list-for-entity:
      description: "List all observations for an entity"
      behavior: crud
      crud:
        operation: list_by_fk
        table: attribute_observations
        schema: ob-poc
        fk_col: entity_id
      args:
        - name: entity-id
          type: uuid
          required: true
        - name: status
          type: string
          required: false
          maps_to: status
          default: "ACTIVE"
      returns:
        type: record_set

    list-for-attribute:
      description: "List observations of a specific attribute for an entity"
      behavior: crud
      crud:
        operation: select
        table: attribute_observations
        schema: ob-poc
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: attribute
          type: lookup
          required: true
          maps_to: attribute_id
          lookup:
            table: attribute_registry
            schema: ob-poc
            code_column: id
            id_column: uuid
        - name: status
          type: string
          required: false
          maps_to: status
          default: "ACTIVE"
      returns:
        type: record_set

    get-current:
      description: "Get the current best value for an attribute"
      behavior: plugin
      handler: observation_get_current
      args:
        - name: entity-id
          type: uuid
          required: true
        - name: attribute
          type: string
          required: true
      returns:
        type: record
```

### New Domain: discrepancy

```yaml
discrepancy:
  description: "Observation discrepancy tracking and resolution"

  verbs:
    record:
      description: "Record a discrepancy between observations"
      behavior: crud
      crud:
        operation: insert
        table: observation_discrepancies
        schema: ob-poc
        returning: discrepancy_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: attribute
          type: lookup
          required: true
          maps_to: attribute_id
          lookup:
            table: attribute_registry
            schema: ob-poc
            code_column: id
            id_column: uuid
        - name: observation-1-id
          type: uuid
          required: true
          maps_to: observation_1_id
        - name: observation-2-id
          type: uuid
          required: true
          maps_to: observation_2_id
        - name: type
          type: string
          required: true
          maps_to: discrepancy_type
          valid_values: [VALUE_MISMATCH, DATE_MISMATCH, SPELLING_VARIATION, FORMAT_DIFFERENCE, MISSING_VS_PRESENT, CONTRADICTORY]
        - name: severity
          type: string
          required: true
          maps_to: severity
          valid_values: [INFO, LOW, MEDIUM, HIGH, CRITICAL]
        - name: description
          type: string
          required: true
          maps_to: description
        - name: case-id
          type: uuid
          required: false
          maps_to: case_id
        - name: workstream-id
          type: uuid
          required: false
          maps_to: workstream_id
      returns:
        type: uuid
        name: discrepancy_id
        capture: true

    resolve:
      description: "Resolve a discrepancy"
      behavior: crud
      crud:
        operation: update
        table: observation_discrepancies
        schema: ob-poc
        key: discrepancy_id
        set_values:
          resolution_status: RESOLVED
          resolved_at: now()
      args:
        - name: discrepancy-id
          type: uuid
          required: true
          maps_to: discrepancy_id
        - name: resolution-type
          type: string
          required: true
          maps_to: resolution_type
          valid_values: [ACCEPTABLE_VARIATION, SOURCE_ERROR, DATA_ENTRY_ERROR, LEGITIMATE_CHANGE, FRAUD_CONFIRMED, FALSE_POSITIVE, WAIVED]
        - name: accepted-observation-id
          type: uuid
          required: false
          maps_to: accepted_observation_id
        - name: notes
          type: string
          required: true
          maps_to: resolution_notes
      returns:
        type: affected

    escalate:
      description: "Escalate a discrepancy"
      behavior: crud
      crud:
        operation: update
        table: observation_discrepancies
        schema: ob-poc
        key: discrepancy_id
        set_values:
          resolution_status: ESCALATED
      args:
        - name: discrepancy-id
          type: uuid
          required: true
          maps_to: discrepancy_id
        - name: notes
          type: string
          required: true
          maps_to: resolution_notes
      returns:
        type: affected

    list-open:
      description: "List open discrepancies for an entity or case"
      behavior: crud
      crud:
        operation: select
        table: observation_discrepancies
        schema: ob-poc
      args:
        - name: entity-id
          type: uuid
          required: false
          maps_to: entity_id
        - name: case-id
          type: uuid
          required: false
          maps_to: case_id
        - name: severity
          type: string
          required: false
          maps_to: severity
      returns:
        type: record_set
```

### Plugin: reconcile

```yaml
# Add to plugins section
plugins:
  observation.reconcile:
    description: "Compare observations for an entity and detect discrepancies"
    handler: observation_reconcile
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: attribute
        type: string
        required: false  # If not provided, reconciles all attributes
      - name: case-id
        type: uuid
        required: false
      - name: auto-create-discrepancies
        type: boolean
        required: false
        default: true
    returns:
      type: record_set  # List of discrepancies found

  observation.verify-allegations:
    description: "Verify all pending allegations against observations"
    handler: verify_allegations
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: false  # If not provided, verifies all entities
      - name: case-id
        type: uuid
        required: false
    returns:
      type: record_set  # Verification results

  document.extract-to-observations:
    description: "Extract attributes from document and create observations"
    handler: document_extract_observations
    args:
      - name: document-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
      - name: auto-verify-allegations
        type: boolean
        required: false
        default: true
    returns:
      type: record_set  # Created observations
```

---

## Migration Strategy

### Phase 1: Schema Creation (Non-Breaking)
1. Create new tables: `attribute_observations`, `client_allegations`, `document_attribute_links`, `observation_discrepancies`
2. Add `entity_id` to `document_catalog`
3. Add new columns to `attribute_registry`
4. Create views

### Phase 2: DSL Integration
1. Add new verbs.yaml domains: `allegation`, `observation`, `discrepancy`
2. Implement plugins: `observation_reconcile`, `verify_allegations`, `document_extract_observations`
3. Update `ResourceSetAttrOp` to create observations instead of direct values

### Phase 3: Data Migration
1. Migrate existing `attribute_values_typed` to `attribute_observations` with `source_type: 'SYSTEM'`
2. Migrate `document_attribute_mappings` to `document_attribute_links` with `direction: 'SOURCE'`
3. Populate SINK links from document_types.applicability

### Phase 4: Deprecation
1. Mark `attribute_values_typed` as deprecated
2. Mark `document_attribute_mappings` as deprecated
3. Update queries to use new tables
4. Remove deprecated tables after verification period

---

## Example DSL Flow

```clojure
;; ============================================================================
;; PHASE 1: Client submits onboarding information (ALLEGATIONS)
;; ============================================================================

(cbu.ensure :name "Pacific Growth Fund" :jurisdiction "LU" :client-type "FUND" :as @fund)

(entity.create-proper-person :first-name "Adam" :last-name "Cearns" :as @ubo)
(cbu.assign-role :cbu-id @fund :entity-id @ubo :role "BENEFICIAL_OWNER")

;; Record client's allegations from onboarding form
(allegation.record :cbu-id @fund :entity-id @ubo
  :attribute "attr.identity.full_name" :value "\"Adam Timothy Cearns\""
  :source "ONBOARDING_FORM" :source-reference "FORM-2024-001" :as @alleg-name)

(allegation.record :cbu-id @fund :entity-id @ubo
  :attribute "attr.identity.dob" :value "\"1980-01-15\""
  :source "ONBOARDING_FORM" :as @alleg-dob)

(allegation.record :cbu-id @fund :entity-id @ubo
  :attribute "attr.identity.nationality" :value "\"GB\""
  :source "ONBOARDING_FORM" :as @alleg-nationality)


;; ============================================================================
;; PHASE 2: KYC case opened, documents requested
;; ============================================================================

(kyc-case.create :cbu-id @fund :case-type "NEW_CLIENT" :as @case)
(entity-workstream.create :case-id @case :entity-id @ubo :as @ws-ubo)

;; System determines required documents based on SINK links
(doc-request.create :workstream-id @ws-ubo :doc-type "PASSPORT" :is-mandatory true)


;; ============================================================================
;; PHASE 3: Documents received and extracted
;; ============================================================================

;; UK Passport received
(document.catalog :cbu-id @fund :entity-id @ubo 
  :doc-type "PASSPORT" :title "UK Passport" :as @uk-passport)

;; Extraction creates observations
(observation.record-from-document :entity-id @ubo :document-id @uk-passport
  :attribute "attr.identity.full_name" :value "Adam Cearns"
  :extraction-method "MRZ" :confidence 0.98 :as @obs-name-uk)

(observation.record-from-document :entity-id @ubo :document-id @uk-passport
  :attribute "attr.identity.dob" :value "1980-01-15"
  :extraction-method "MRZ" :confidence 0.99 :as @obs-dob-uk)

(observation.record-from-document :entity-id @ubo :document-id @uk-passport
  :attribute "attr.identity.nationality" :value "GBR"
  :extraction-method "MRZ" :confidence 0.99 :as @obs-nat-uk)

;; Swiss passport also received
(document.catalog :cbu-id @fund :entity-id @ubo
  :doc-type "PASSPORT" :title "Swiss Passport" :as @ch-passport)

(observation.record-from-document :entity-id @ubo :document-id @ch-passport
  :attribute "attr.identity.full_name" :value "Adam T Cearns"
  :extraction-method "MRZ" :confidence 0.98 :as @obs-name-ch)


;; ============================================================================
;; PHASE 4: Reconciliation
;; ============================================================================

;; System reconciles observations and finds discrepancy
(observation.reconcile :entity-id @ubo :case-id @case)

;; Creates discrepancy record:
;; - @obs-name-uk: "Adam Cearns"
;; - @obs-name-ch: "Adam T Cearns"  
;; - Type: SPELLING_VARIATION, Severity: INFO

;; Analyst resolves as acceptable variation
(discrepancy.resolve :discrepancy-id @disc-name
  :resolution-type "ACCEPTABLE_VARIATION"
  :accepted-observation-id @obs-name-uk
  :notes "Middle initial variation between passports is acceptable")


;; ============================================================================
;; PHASE 5: Verify allegations
;; ============================================================================

(observation.verify-allegations :cbu-id @fund :entity-id @ubo :case-id @case)

;; Results:
;; - @alleg-dob: VERIFIED (exact match)
;; - @alleg-nationality: VERIFIED (GB = GBR)
;; - @alleg-name: PARTIAL (alleged "Adam Timothy Cearns", passport shows "Adam Cearns")

;; Analyst confirms partial match is acceptable
(allegation.verify :allegation-id @alleg-name :observation-id @obs-name-uk
  :result "ACCEPTABLE_VARIATION"
  :notes "Client used full middle name, passport shows initial only")
```

---

## Testing Checklist

- [ ] Multiple observations per attribute per entity
- [ ] Observation source provenance tracking
- [ ] Document extraction creates observations
- [ ] Allegation → Observation verification flow
- [ ] Discrepancy detection on reconciliation
- [ ] Spelling variation detection (fuzzy matching)
- [ ] Authoritative source prioritization
- [ ] Current value view returns correct value
- [ ] Supersession chain works
- [ ] Red flag creation from critical discrepancies
- [ ] Entity-document linkage

---

## Notes for Claude Code

1. **Start with schema changes** - create tables in order (observations depends on attribute_registry)
2. **Seed document_attribute_links** - populate from existing document_types.applicability + document_attribute_mappings
3. **Plugin implementation priority**: `observation_from_document` first, then `reconcile`, then `verify_allegations`
4. **Use existing patterns** from `generic_executor.rs` for CRUD verbs
5. **Fuzzy matching for reconciliation** - consider using pg_trgm or Levenshtein distance for name comparison
6. **Consolidate dictionaries** - migrate from `dictionary` table to `attribute_registry` as single source

The key insight to preserve: **KYC is about collecting observations and making decisions, not storing single truths.**
