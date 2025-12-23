# Data Architecture: A Master Data Management Perspective

## Executive Summary

This document describes the ob-poc data architecture for data governance and 
MDM professionals. It explains how the platform manages master data, reference 
data, and transactional data across the client onboarding domain.

**Key Takeaway:** The DSL-driven approach provides stronger data governance 
guarantees than traditional application-centric architectures because:
- Every data mutation flows through a single, auditable grammar
- Schema and validation are configuration, not buried code
- Lineage is automatic—every record traces to the verb that created it
- Entity resolution is first-class, not an afterthought

---

## Data Domain Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           MASTER DATA DOMAINS                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
│  │    PARTY    │    │   PRODUCT   │    │ INSTRUMENT  │    │   ACCOUNT   │  │
│  │             │    │             │    │             │    │             │  │
│  │ • CBU       │    │ • Products  │    │ • Symbols   │    │ • SSIs      │  │
│  │ • Entities  │    │ • Services  │    │ • Classes   │    │ • Custody   │  │
│  │ • Roles     │    │ • Resources │    │ • Markets   │    │ • Cash      │  │
│  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘  │
│         │                  │                  │                  │          │
│         └──────────────────┴──────────────────┴──────────────────┘          │
│                                    │                                         │
│                         ┌──────────▼──────────┐                             │
│                         │   REFERENCE DATA    │                             │
│                         │                     │                             │
│                         │ • Jurisdictions     │                             │
│                         │ • Currencies        │                             │
│                         │ • Entity Types      │                             │
│                         │ • Role Types        │                             │
│                         │ • Status Codes      │                             │
│                         └─────────────────────┘                             │
│                                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                         TRANSACTIONAL DATA                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  • KYC Cases           • Documents            • Audit Events                 │
│  • Workstreams         • Screening Results    • State Transitions           │
│  • Approvals           • Communications       • DSL Execution Logs          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 1. The CBU: Central Master Entity

### What Is a CBU?

The **Client Business Unit (CBU)** is the golden record for a client relationship. 
It is the apex entity from which all other data hangs.

**MDM Parallel:** Think of CBU as the "Customer Master" in a CDI (Customer Data 
Integration) hub. It is the single, authoritative representation of a client.

### CBU Attributes

| Attribute | Type | Governance | Description |
|-----------|------|------------|-------------|
| `cbu_id` | UUID | System-generated | Immutable primary key |
| `name` | String | User-provided | Display name, searchable |
| `jurisdiction` | FK → jurisdictions | Reference data | Legal domicile |
| `client_type` | Enum | Controlled vocabulary | INDIVIDUAL, CORPORATE, FUND |
| `status` | Enum | State machine | PROSPECT → ACTIVE → DORMANT → CLOSED |
| `created_at` | Timestamp | System-generated | Immutable creation time |
| `created_by` | UUID | System-captured | User/system that created |

### CBU as Aggregation Root

```
                              ┌─────────────┐
                              │     CBU     │
                              │ "Alpha Fund"│
                              └──────┬──────┘
                                     │
        ┌────────────┬───────────────┼───────────────┬────────────┐
        │            │               │               │            │
        ▼            ▼               ▼               ▼            ▼
   ┌─────────┐ ┌──────────┐   ┌───────────┐   ┌──────────┐ ┌──────────┐
   │Entities │ │ Products │   │ KYC Cases │   │ Trading  │ │   SSIs   │
   │(people, │ │(CUSTODY, │   │(lifecycle │   │ Profiles │ │(settlmt) │
   │companies│ │ FA, etc.)│   │ events)   │   │          │ │          │
   └─────────┘ └──────────┘   └───────────┘   └──────────┘ └──────────┘
```

**Data Governance Implication:** All queries for client data start from CBU. 
There is no orphaned data. Every record traces back to a CBU.

---

## 2. Entity Model: Party Master Data

### Entity Type Hierarchy

The platform supports a comprehensive party model:

```
                         Entity (Abstract)
                               │
          ┌────────────────────┼────────────────────┐
          │                    │                    │
     NaturalPerson        LegalEntity           Arrangement
          │                    │                    │
    ┌─────┴─────┐    ┌────────┼────────┐          │
    │           │    │        │        │          │
 ProperPerson  │  Limited  Partner-  Trust    PooledFund
              │  Company   ship
         Passport
         Holder
```

### Entity Attributes (Example: ProperPerson)

| Attribute | Type | Source | Quality Rules |
|-----------|------|--------|---------------|
| `entity_id` | UUID | System | Immutable PK |
| `first_name` | String | User/Document | Required, max 100 chars |
| `last_name` | String | User/Document | Required, max 100 chars |
| `date_of_birth` | Date | Document | Required for KYC, age validation |
| `nationality` | FK | Reference data | ISO 3166-1 alpha-2 |
| `tax_residency` | FK[] | User | Multi-value, jurisdiction codes |
| `pep_status` | Enum | Screening | NONE, DOMESTIC, FOREIGN |
| `risk_rating` | Enum | Calculated | LOW, MEDIUM, HIGH, PROHIBITED |

### Entity Relationships

```sql
-- Ownership relationships (UBO chains)
CREATE TABLE entity_ownership (
    owner_entity_id    UUID REFERENCES entities(entity_id),
    owned_entity_id    UUID REFERENCES entities(entity_id),
    ownership_pct      DECIMAL(5,2),
    ownership_type     VARCHAR(50),  -- DIRECT, INDIRECT, BENEFICIAL
    effective_date     DATE,
    source_document_id UUID,         -- Lineage to source
    CONSTRAINT valid_pct CHECK (ownership_pct BETWEEN 0 AND 100)
);

-- Role assignments (entity-to-CBU relationships)
CREATE TABLE cbu_entity_roles (
    cbu_id         UUID REFERENCES cbus(cbu_id),
    entity_id      UUID REFERENCES entities(entity_id),
    role_type      VARCHAR(50),  -- ACCOUNT_HOLDER, UBO, DIRECTOR, SIGNATORY
    effective_date DATE,
    expiry_date    DATE,
    CONSTRAINT valid_dates CHECK (expiry_date IS NULL OR expiry_date > effective_date)
);
```

**Key MDM Concept:** Entities are shared across CBUs. A person can be:
- Account holder on CBU-A
- UBO on CBU-B
- Director on CBU-C

Entity resolution ensures we don't create duplicates.

---

## 3. Attribute Management

### The Dynamic Attribute Pattern

Beyond fixed schema columns, entities support extensible attributes:

```sql
CREATE TABLE entity_attributes (
    attribute_id      UUID PRIMARY KEY,
    entity_id         UUID REFERENCES entities(entity_id),
    attribute_type_id UUID REFERENCES attribute_types(id),
    value_text        TEXT,
    value_date        DATE,
    value_numeric     DECIMAL,
    value_json        JSONB,
    effective_date    DATE NOT NULL,
    expiry_date       DATE,
    source_type       VARCHAR(50),  -- USER_INPUT, DOCUMENT_EXTRACT, SCREENING, CALC
    source_id         UUID,         -- FK to source record
    confidence        DECIMAL(3,2), -- 0.00-1.00 for ML-extracted values
    verified          BOOLEAN DEFAULT FALSE,
    verified_by       UUID,
    verified_at       TIMESTAMP
);
```

### Attribute Type Registry

```yaml
# config/ontology/attribute_types.yaml
attribute_types:
  - code: TAX_ID
    name: "Tax Identification Number"
    category: IDENTIFICATION
    data_type: STRING
    validation:
      patterns:
        US: "^\\d{9}$"           # SSN format
        GB: "^[A-Z]{2}\\d{6}[A-Z]$"  # NINO format
    sensitivity: PII
    retention_years: 7
    
  - code: SOURCE_OF_WEALTH
    name: "Source of Wealth"
    category: KYC
    data_type: ENUM
    allowed_values:
      - EMPLOYMENT
      - INHERITANCE
      - BUSINESS_SALE
      - INVESTMENTS
      - OTHER
    sensitivity: CONFIDENTIAL
    
  - code: AUM
    name: "Assets Under Management"
    category: FINANCIAL
    data_type: MONEY
    currency_field: aum_currency
    sensitivity: CONFIDENTIAL
```

### Attribute Lineage

Every attribute value traces to its source:

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│   Document      │      │   Attribute     │      │    Entity       │
│   (Passport)    │─────▶│   (DOB: 1985)   │─────▶│   (John Doe)    │
│                 │      │                 │      │                 │
│ source_type:    │      │ source_type:    │      │                 │
│ UPLOAD          │      │ DOCUMENT_EXTRACT│      │                 │
│ extracted_by:   │      │ source_id: →    │      │                 │
│ ML_MODEL_v2     │      │ confidence: 0.98│      │                 │
└─────────────────┘      └─────────────────┘      └─────────────────┘
```

---

## 4. Document Integration

### Document as First-Class Data

Documents aren't attachments—they're structured data sources:

```sql
CREATE TABLE documents (
    document_id       UUID PRIMARY KEY,
    entity_id         UUID REFERENCES entities(entity_id),
    document_type_id  UUID REFERENCES document_types(id),
    file_reference    VARCHAR(500),  -- S3/blob storage path
    original_filename VARCHAR(255),
    mime_type         VARCHAR(100),
    file_hash         VARCHAR(64),   -- SHA-256 for integrity
    upload_date       TIMESTAMP,
    expiry_date       DATE,          -- For ID documents
    extraction_status VARCHAR(50),   -- PENDING, COMPLETE, FAILED
    extraction_result JSONB,         -- Structured extracted data
    verified          BOOLEAN,
    verified_by       UUID,
    verified_at       TIMESTAMP
);

CREATE TABLE document_extracted_fields (
    extraction_id     UUID PRIMARY KEY,
    document_id       UUID REFERENCES documents(document_id),
    field_name        VARCHAR(100),
    field_value       TEXT,
    confidence        DECIMAL(3,2),
    bounding_box      JSONB,         -- Location in document
    promoted_to       UUID,          -- FK to entity_attributes if promoted
    promoted_at       TIMESTAMP
);
```

### Document → Attribute Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│  DOCUMENT PROCESSING PIPELINE                                            │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  1. UPLOAD              2. EXTRACT             3. VALIDATE              │
│  ┌─────────────┐        ┌─────────────┐        ┌─────────────┐          │
│  │  Passport   │───────▶│  ML Model   │───────▶│  Business   │          │
│  │  PDF/Image  │        │  extracts:  │        │  Rules:     │          │
│  │             │        │  - Name     │        │  - Format   │          │
│  │             │        │  - DOB      │        │  - Range    │          │
│  │             │        │  - Number   │        │  - Cross-ref│          │
│  └─────────────┘        └─────────────┘        └─────────────┘          │
│                                                       │                  │
│  4. PROMOTE             5. LINK                       ▼                  │
│  ┌─────────────┐        ┌─────────────┐        ┌─────────────┐          │
│  │  Create     │◀───────│  Human      │◀───────│  Review     │          │
│  │  Attribute  │        │  Approval   │        │  Queue      │          │
│  │  Records    │        │  (if conf   │        │             │          │
│  │             │        │   < 0.95)   │        │             │          │
│  └─────────────┘        └─────────────┘        └─────────────┘          │
│        │                                                                 │
│        ▼                                                                 │
│  ┌─────────────────────────────────────────────┐                        │
│  │  Entity Attribute with Full Lineage:        │                        │
│  │  - value: "1985-03-15"                      │                        │
│  │  - source_type: DOCUMENT_EXTRACT            │                        │
│  │  - source_id: doc_uuid                      │                        │
│  │  - confidence: 0.98                         │                        │
│  │  - verified: true                           │                        │
│  │  - verified_by: user_uuid                   │                        │
│  └─────────────────────────────────────────────┘                        │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Product → Service → Resource Taxonomy

### The Three-Level Hierarchy

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PRODUCT (What we sell)                                                     │
│  ═══════════════════════                                                    │
│  Examples: CUSTODY, FUND_ACCOUNTING, PRIME_BROKERAGE                        │
│  Granularity: Commercial offering, appears on contracts                     │
├─────────────────────────────────────────────────────────────────────────────┤
│       │                                                                     │
│       ▼                                                                     │
│  SERVICE (What we deliver)                                                  │
│  ═════════════════════════                                                  │
│  Examples: Safekeeping, Settlement, Corporate Actions, NAV Calculation      │
│  Granularity: Operational capability, has SLAs                              │
├─────────────────────────────────────────────────────────────────────────────┤
│       │                                                                     │
│       ▼                                                                     │
│  RESOURCE (What we provision)                                               │
│  ═══════════════════════════                                                │
│  Examples: Custody Account, Cash Account, Pricing Feed, Report Template     │
│  Granularity: Technical/operational instance, has configuration             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Data Model

```sql
-- Reference data: Product catalog
CREATE TABLE products (
    product_id   UUID PRIMARY KEY,
    code         VARCHAR(50) UNIQUE,
    name         VARCHAR(255),
    category     VARCHAR(50),
    is_active    BOOLEAN DEFAULT TRUE
);

-- Reference data: Service catalog
CREATE TABLE services (
    service_id   UUID PRIMARY KEY,
    code         VARCHAR(50) UNIQUE,
    name         VARCHAR(255),
    category     VARCHAR(50)
);

-- Reference data: Product-to-Service mapping
CREATE TABLE product_services (
    product_id   UUID REFERENCES products(product_id),
    service_id   UUID REFERENCES services(service_id),
    is_mandatory BOOLEAN DEFAULT TRUE,
    PRIMARY KEY (product_id, service_id)
);

-- Reference data: Resource types
CREATE TABLE resource_types (
    resource_type_id UUID PRIMARY KEY,
    code             VARCHAR(50) UNIQUE,
    name             VARCHAR(255),
    service_id       UUID REFERENCES services(service_id),
    config_schema    JSONB  -- JSON Schema for resource configuration
);

-- Transactional: CBU product subscriptions
CREATE TABLE cbu_product_subscriptions (
    subscription_id UUID PRIMARY KEY,
    cbu_id          UUID REFERENCES cbus(cbu_id),
    product_id      UUID REFERENCES products(product_id),
    status          VARCHAR(50),  -- PENDING, ACTIVE, SUSPENDED, TERMINATED
    effective_date  DATE,
    termination_date DATE
);

-- Transactional: Provisioned resources
CREATE TABLE cbu_resource_instances (
    instance_id      UUID PRIMARY KEY,
    cbu_id           UUID REFERENCES cbus(cbu_id),
    resource_type_id UUID REFERENCES resource_types(resource_type_id),
    configuration    JSONB,
    status           VARCHAR(50),
    provisioned_at   TIMESTAMP,
    provisioned_by   UUID
);
```

### Example Expansion

When a CBU subscribes to CUSTODY:

```
CUSTODY (Product)
    │
    ├── Safekeeping (Service)
    │       ├── Custody Account (Resource) ──► Provisioned for CBU
    │       └── Asset Servicing Config (Resource) ──► Provisioned
    │
    ├── Settlement (Service)
    │       ├── SSI Registry (Resource) ──► Provisioned
    │       └── PSET Connectivity (Resource) ──► Provisioned
    │
    ├── Corporate Actions (Service)
    │       ├── CA Notification Config (Resource) ──► Provisioned
    │       └── Election Template (Resource) ──► Provisioned
    │
    └── Income Collection (Service)
            └── Income Processing Config (Resource) ──► Provisioned
```

---

## 6. The Trading Matrix: Instrument-Centric Data

### Concept

The **CBU Trading Matrix** defines what a client CAN trade:
- Which instrument classes (Equity, Fixed Income, FX, Derivatives)
- In which markets (NYSE, LSE, TSE)
- Settled in which currencies (USD, EUR, GBP)
- Via which settlement types (DVP, FOP, RVP)

This is **instrument-centric master data**, distinct from product subscriptions.

### Data Model

```sql
-- Reference data: Instrument class hierarchy
CREATE TABLE instrument_classes (
    class_id     UUID PRIMARY KEY,
    code         VARCHAR(50) UNIQUE,
    name         VARCHAR(255),
    parent_id    UUID REFERENCES instrument_classes(class_id),
    level        INTEGER  -- 1=top, 2=sub, 3=leaf
);

-- Example hierarchy:
-- EQUITY (1)
--   ├── EQUITY_COMMON (2)
--   ├── EQUITY_PREFERRED (2)
--   └── EQUITY_ETF (2)
-- FIXED_INCOME (1)
--   ├── GOVT_BOND (2)
--   ├── CORP_BOND (2)
--   └── MUNICIPAL (2)

-- Reference data: Markets
CREATE TABLE markets (
    market_id    UUID PRIMARY KEY,
    mic_code     VARCHAR(10) UNIQUE,  -- ISO 10383
    name         VARCHAR(255),
    country      VARCHAR(2),
    timezone     VARCHAR(50)
);

-- Transactional: CBU instrument universe
CREATE TABLE cbu_instrument_universe (
    universe_id        UUID PRIMARY KEY,
    cbu_id             UUID REFERENCES cbus(cbu_id),
    instrument_class_id UUID REFERENCES instrument_classes(class_id),
    market_id          UUID REFERENCES markets(market_id),
    currencies         VARCHAR(3)[],     -- Array of ISO 4217 codes
    settlement_types   VARCHAR(20)[],    -- DVP, FOP, etc.
    effective_date     DATE,
    restrictions       JSONB,            -- Additional constraints
    approved_by        UUID,
    approved_at        TIMESTAMP
);
```

### Matrix View

For a given CBU, the trading matrix might look like:

| Instrument Class | Market | Currencies | Settlement | Status |
|------------------|--------|------------|------------|--------|
| EQUITY_COMMON | XNYS | USD | DVP | ✓ Active |
| EQUITY_COMMON | XLON | GBP, USD | DVP | ✓ Active |
| EQUITY_ETF | XNYS | USD | DVP | ✓ Active |
| GOVT_BOND | XLON | GBP | DVP, FOP | ✓ Active |
| FX_SPOT | OTC | USD, EUR, GBP | T+2 | ✓ Active |
| EQUITY_COMMON | XTKS | JPY | DVP | ○ Pending |

---

## 7. Settlement Instructions (SSIs)

### The SSI Data Model

SSIs are critical master data for trade settlement:

```sql
CREATE TABLE cbu_ssis (
    ssi_id                UUID PRIMARY KEY,
    cbu_id                UUID REFERENCES cbus(cbu_id),
    ssi_name              VARCHAR(255),
    ssi_type              VARCHAR(50),      -- SECURITIES, CASH, DERIVATIVES
    
    -- Securities leg
    safekeeping_account   VARCHAR(50),
    safekeeping_bic       VARCHAR(11),      -- SWIFT BIC
    safekeeping_name      VARCHAR(255),
    
    -- Cash leg  
    cash_account          VARCHAR(50),
    cash_bic              VARCHAR(11),
    cash_currency         VARCHAR(3),       -- ISO 4217
    
    -- Settlement details
    pset_bic              VARCHAR(11),      -- Place of settlement
    
    -- Lifecycle
    status                VARCHAR(50),
    effective_date        DATE,
    expiry_date           DATE,
    
    -- Audit
    created_at            TIMESTAMP,
    created_by            UUID,
    approved_by           UUID,
    approved_at           TIMESTAMP
);

-- Booking rules: Which SSI to use for which trades
CREATE TABLE ssi_booking_rules (
    rule_id              UUID PRIMARY KEY,
    ssi_id               UUID REFERENCES cbu_ssis(ssi_id),
    cbu_id               UUID REFERENCES cbus(cbu_id),
    rule_name            VARCHAR(255),
    priority             INTEGER,          -- Lower = higher priority
    
    -- Match criteria
    instrument_class_id  UUID,             -- NULL = any
    market_id            UUID,             -- NULL = any
    currency             VARCHAR(3),       -- NULL = any
    counterparty_id      UUID,             -- NULL = any
    
    -- Validity
    effective_date       DATE,
    expiry_date          DATE
);
```

### SSI Selection Logic

```
Trade comes in:
  - Instrument: AAPL (EQUITY_COMMON)
  - Market: XNYS
  - Currency: USD
  - Counterparty: Goldman Sachs

Query booking rules for CBU, ordered by priority:

  Rule 1: counterparty=Goldman, currency=USD → SSI-GS-USD
  Rule 2: market=XNYS, currency=USD → SSI-US-EQUITIES  
  Rule 3: currency=USD (catch-all) → SSI-DEFAULT-USD

Match Rule 1 → Use SSI-GS-USD
```

---

## 8. Reference Data Management

### Reference Data Domains

| Domain | Examples | Source | Update Frequency |
|--------|----------|--------|------------------|
| Jurisdictions | US, GB, DE, JP | ISO 3166-1 | Rare |
| Currencies | USD, EUR, GBP | ISO 4217 | Rare |
| Markets | XNYS, XLON, XTKS | ISO 10383 | Monthly |
| Entity Types | PROPER_PERSON, LIMITED_COMPANY | Internal | Quarterly |
| Role Types | ACCOUNT_HOLDER, UBO, DIRECTOR | Internal | Quarterly |
| Document Types | PASSPORT, CERT_INCORP, PROOF_ADDRESS | Internal | Monthly |
| Instrument Classes | EQUITY, FIXED_INCOME, FX | Internal | Quarterly |
| Products | CUSTODY, FUND_ACCOUNTING | Commercial | Yearly |

### Reference Data in the DSL

The DSL validates against reference data at parse time:

```yaml
# Verb definition with reference data constraint
args:
  - name: jurisdiction
    type: string
    required: true
    lookup:
      table: jurisdictions
      schema: ob_ref
      search_key: code
      validate: true  # Reject if not found
```

**Governance Benefit:** Invalid reference data values are rejected at the API 
layer, not discovered later in downstream processing.

---

## 9. Data Lineage Through the DSL

### Every Mutation Has a Verb

Traditional systems:
```java
// Who called this? When? Why? From where?
cbuRepository.save(cbu);
```

DSL approach:
```lisp
(cbu.create name:"Alpha Fund" jurisdiction:US) -> @alpha
```

The execution log captures:
```json
{
  "execution_id": "uuid",
  "timestamp": "2024-01-15T10:30:00Z",
  "session_id": "uuid",
  "user_id": "uuid",
  "verb": "cbu.create",
  "arguments": {
    "name": "Alpha Fund",
    "jurisdiction": "US"
  },
  "result": {
    "cbu_id": "uuid",
    "symbol": "@alpha"
  },
  "duration_ms": 45
}
```

### Lineage Query

"Where did this CBU come from?"

```sql
SELECT 
    e.verb,
    e.arguments,
    e.timestamp,
    u.username,
    s.session_type
FROM dsl_executions e
JOIN users u ON e.user_id = u.user_id
JOIN sessions s ON e.session_id = s.session_id
WHERE e.result->>'cbu_id' = 'uuid-of-alpha-fund'
ORDER BY e.timestamp;
```

Result:
```
verb         | arguments                              | timestamp           | user    | session
─────────────┼────────────────────────────────────────┼─────────────────────┼─────────┼─────────
cbu.create   | {name: "Alpha Fund", jurisdiction: US} | 2024-01-15 10:30:00 | jsmith  | UI
cbu.update   | {cbu-id: ..., status: ACTIVE}          | 2024-01-15 11:00:00 | jsmith  | UI
cbu.add-prod | {cbu-id: ..., product: CUSTODY}        | 2024-01-15 11:05:00 | agent   | API
```

---

## 10. Data Quality Rules

### Validation in Verb Definitions

```yaml
# config/verbs/entity.yaml
create-proper-person:
  description: "Create a natural person entity"
  args:
    - name: first-name
      type: string
      required: true
      validation:
        min_length: 1
        max_length: 100
        pattern: "^[A-Za-z\\s\\-']+$"
        
    - name: date-of-birth
      type: date
      required: true
      validation:
        max_value: "today"
        min_value: "today - 120 years"
        
    - name: nationality
      type: string
      required: true
      lookup:
        table: countries
        search_key: iso_alpha2
        validate: true
```

### Cross-Field Validation

```yaml
create-ownership:
  args:
    - name: ownership-pct
      type: decimal
      validation:
        min_value: 0.01
        max_value: 100.00
        
  cross_validation:
    - rule: total_ownership_check
      description: "Total ownership of an entity cannot exceed 100%"
      query: |
        SELECT COALESCE(SUM(ownership_pct), 0) 
        FROM entity_ownership 
        WHERE owned_entity_id = :owned-entity-id
      constraint: "result + :ownership-pct <= 100"
```

### Data Quality Dashboard Metrics

The DSL execution logs enable quality metrics:

| Metric | Query Approach |
|--------|----------------|
| Validation failure rate | `COUNT(*) WHERE status = 'VALIDATION_ERROR' / COUNT(*)` |
| Missing required attributes | Entities without expected attribute types |
| Stale documents | Documents past expiry_date |
| Unverified extractions | Attributes with `verified = false` |
| Orphan entities | Entities with no CBU role assignment |
| Duplicate candidates | Entity pairs with high name similarity |

---

## 11. Entity Resolution

### The Duplicate Problem

Same person, different CBUs:
- CBU-A: "John Smith", DOB 1980-05-15, US passport
- CBU-B: "J. Smith", DOB 1980-05-15, US passport
- CBU-C: "John David Smith", DOB 1980-05-15, US passport

### Resolution Approach

```sql
-- Candidate identification
CREATE TABLE entity_match_candidates (
    candidate_id    UUID PRIMARY KEY,
    entity_id_a     UUID REFERENCES entities(entity_id),
    entity_id_b     UUID REFERENCES entities(entity_id),
    match_score     DECIMAL(5,4),  -- 0.0000 to 1.0000
    match_reasons   JSONB,         -- Which fields matched
    status          VARCHAR(50),   -- PENDING, CONFIRMED_MATCH, CONFIRMED_DISTINCT
    reviewed_by     UUID,
    reviewed_at     TIMESTAMP
);

-- Confirmed links
CREATE TABLE entity_links (
    link_id         UUID PRIMARY KEY,
    primary_id      UUID REFERENCES entities(entity_id),  -- Golden record
    secondary_id    UUID REFERENCES entities(entity_id),  -- Merged away
    link_type       VARCHAR(50),   -- SAME_PERSON, SAME_COMPANY
    confidence      DECIMAL(3,2),
    linked_at       TIMESTAMP,
    linked_by       UUID
);
```

### Resolution in the DSL

```lisp
; Find potential duplicates
(entity.find-duplicates entity-id:@john-smith threshold:0.8)

; Link confirmed match
(entity.link primary:@john-smith-a secondary:@john-smith-b link-type:SAME_PERSON)

; Query uses primary automatically
(entity.get entity-id:@john-smith-b)  ; Returns @john-smith-a data
```

---

## 12. Schema Governance

### Schema as Configuration

The database schema is defined in versioned SQL migrations:

```
migrations/
├── V001__create_cbu_schema.sql
├── V002__create_entity_schema.sql
├── V003__create_kyc_schema.sql
├── V004__add_trading_matrix.sql
└── V005__add_document_extraction.sql
```

### Schema-to-Verb Alignment

The verb registry validates against the schema:

```yaml
# If this column doesn't exist, verb loading fails
crud:
  operation: insert
  table: cbus
  schema: ob_cbu
  columns:
    - name: cbu_name    # Must exist in cbus table
    - jurisdiction      # Must exist in cbus table
```

**Governance Benefit:** Schema changes require corresponding verb updates. 
You cannot accidentally break the API.

---

## 13. Audit and Compliance

### Immutable Audit Trail

Every DSL execution is logged:

```sql
CREATE TABLE dsl_execution_log (
    execution_id    UUID PRIMARY KEY,
    session_id      UUID NOT NULL,
    user_id         UUID,
    timestamp       TIMESTAMP NOT NULL DEFAULT NOW(),
    verb            VARCHAR(100) NOT NULL,
    arguments       JSONB NOT NULL,
    result          JSONB,
    status          VARCHAR(50),  -- SUCCESS, ERROR, VALIDATION_FAILED
    error_message   TEXT,
    duration_ms     INTEGER,
    
    -- Immutability
    CONSTRAINT no_updates CHECK (TRUE)  -- Trigger prevents UPDATE
);

-- Trigger to prevent modifications
CREATE TRIGGER prevent_audit_modification
    BEFORE UPDATE OR DELETE ON dsl_execution_log
    FOR EACH ROW EXECUTE FUNCTION raise_immutable_error();
```

### Compliance Queries

**"Show all changes to this entity in the last 90 days":**
```sql
SELECT * FROM dsl_execution_log
WHERE arguments->>'entity-id' = 'uuid'
  AND timestamp > NOW() - INTERVAL '90 days'
ORDER BY timestamp;
```

**"Who approved this KYC case?":**
```sql
SELECT user_id, timestamp 
FROM dsl_execution_log
WHERE verb = 'kyc-case.approve'
  AND arguments->>'case-id' = 'uuid';
```

**"What was the state of this CBU on December 31st?":**
```sql
-- Replay all verbs up to that date
SELECT * FROM dsl_execution_log
WHERE result->>'cbu_id' = 'uuid'
  AND timestamp <= '2023-12-31 23:59:59'
ORDER BY timestamp;
```

---

## 14. Data Governance Summary

### How ob-poc Supports Data Governance

| Governance Concern | ob-poc Approach |
|--------------------|-----------------|
| **Data Ownership** | CBU is the aggregation root; all data has an owner |
| **Data Quality** | Validation rules in verb definitions |
| **Data Lineage** | Every mutation logged with full context |
| **Reference Data** | Centralized, validated at parse time |
| **Master Data** | Single entity model, resolution built-in |
| **Schema Control** | Versioned migrations, verb-schema alignment |
| **Audit Trail** | Immutable execution log |
| **Access Control** | Verb-level permissions (future) |

### What Traditional Approaches Lack

| Concern | Traditional Problem | ob-poc Solution |
|---------|---------------------|-----------------|
| Lineage | Scattered across services | Single execution log |
| Validation | Buried in Java code | Declarative YAML |
| Schema drift | ORM hides changes | Explicit migrations |
| Audit gaps | Requires custom code | Automatic by design |
| Reference data | Hardcoded enums | Queryable tables |

---

## Appendix: Entity-Relationship Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│                              ┌─────────┐                                    │
│                              │   CBU   │                                    │
│                              └────┬────┘                                    │
│                                   │                                         │
│         ┌─────────────────────────┼─────────────────────────┐               │
│         │                         │                         │               │
│         ▼                         ▼                         ▼               │
│   ┌──────────┐             ┌──────────┐             ┌──────────┐           │
│   │ Entities │             │ Products │             │ KYC Cases│           │
│   └────┬─────┘             └────┬─────┘             └────┬─────┘           │
│        │                        │                        │                  │
│        │ has                    │ provisions             │ contains         │
│        ▼                        ▼                        ▼                  │
│   ┌──────────┐             ┌──────────┐             ┌──────────┐           │
│   │  Roles   │             │ Services │             │Workstream│           │
│   │Attributes│             │Resources │             │Screening │           │
│   │Documents │             │   SSIs   │             │Documents │           │
│   └──────────┘             └──────────┘             └──────────┘           │
│        │                        │                                           │
│        │ owns                   │ configures                                │
│        ▼                        ▼                                           │
│   ┌──────────┐             ┌──────────┐                                    │
│   │ Entities │             │ Trading  │                                    │
│   │ (UBO     │             │ Matrix   │                                    │
│   │  chain)  │             │          │                                    │
│   └──────────┘             └──────────┘                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```
