# CBU Database Architecture

**Document Version:** 1.0  
**Last Updated:** 2026-02-05  
**Audience:** Engineering Team  

---

## Executive Summary

This document explains the database design for the Client Business Unit (CBU) system, which is the **atomic unit** of our custody and fund administration platform. Everything in the system resolves to sets of CBUs.

**Key Concept:** A CBU represents a single trading/investment unit (fund, mandate, segregated account) that we service. It's the lens through which we view entities, products, trading permissions, and investor relationships.

---

## Database Overview

| Schema | Purpose | Key Tables |
|--------|---------|------------|
| `ob-poc` | Core domain model | cbus, entities, roles, products, trading_profiles |
| `kyc` | Investor register & KYC | investors, holdings, share_classes, cases |
| `custody` | Settlement & safekeeping | markets, instrument_classes, ssi |
| `agent` | Learning & search | verb_pattern_embeddings, learning_candidates |

**Statistics:**
- **11 schemas**, **335+ tables**, **106 views**, **189 functions**
- PostgreSQL 18.1 with pgvector extension for semantic search

---

## 1. CBU Core Structure

### 1.1 The `cbus` Table

The central table representing Client Business Units.

```
┌─────────────────────────────────────────────────────────────────────┐
│                              cbus                                    │
├─────────────────────────────────────────────────────────────────────┤
│ cbu_id (PK)              UUID        Auto-generated UUIDv7          │
│ name                     VARCHAR     Display name                    │
│ description              TEXT        Optional description            │
│ cbu_category             VARCHAR     FUND | MANDATE | SEGREGATED    │
│ jurisdiction             VARCHAR     LU | IE | DE | UK | US         │
│ status                   VARCHAR     DISCOVERED | ACTIVE | CLOSED   │
│ client_type              VARCHAR     Client classification          │
│ commercial_client_entity_id  UUID → entities  The parent client     │
│ product_id               UUID → products      Primary product        │
│ nature_purpose           TEXT        Business purpose               │
│ source_of_funds          TEXT        AML source of funds            │
│ kyc_scope_template       VARCHAR     KYC template to apply          │
│ risk_context             JSONB       Risk metadata                   │
│ onboarding_context       JSONB       Onboarding state               │
│ semantic_context         JSONB       Search/AI context              │
│ embedding                VECTOR(384) BGE semantic embedding         │
│ created_at / updated_at  TIMESTAMPTZ Audit timestamps               │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 CBU Categories

| Category | Description | Example |
|----------|-------------|---------|
| `FUND` | Collective investment vehicle (UCITS, AIF, PE) | Allianz Euro High Yield Fund |
| `MANDATE` | Discretionary investment mandate | Pension Scheme Mandate |
| `SEGREGATED` | Segregated account for single investor | Family Office Account |

### 1.3 CBU Status Lifecycle

```
DISCOVERED → ONBOARDING → ACTIVE → SUSPENDED → CLOSED
     │            │           │         │
     │            │           │         └─► Can be reactivated
     │            │           └─► Normal operating state
     │            └─► KYC/setup in progress
     └─► Initial discovery (GLEIF import, manual entry)
```

---

## 2. Entity Model

### 2.1 The `entities` Table

All natural persons and legal entities in the system.

```
┌─────────────────────────────────────────────────────────────────────┐
│                            entities                                  │
├─────────────────────────────────────────────────────────────────────┤
│ entity_id (PK)           UUID        Auto-generated                 │
│ entity_type_id           UUID → entity_types                        │
│ name                     VARCHAR     Canonical name                  │
│ name_norm                TEXT        Normalized for search          │
│ external_id              VARCHAR     LEI, company number, etc.      │
│ bods_entity_type         VARCHAR     BODS classification            │
│ bods_entity_subtype      VARCHAR     BODS sub-classification        │
│ founding_date            DATE        Incorporation date             │
│ dissolution_date         DATE        If dissolved                   │
│ is_publicly_listed       BOOLEAN     Listed company flag            │
│ created_at / updated_at  TIMESTAMPTZ Audit timestamps               │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 Entity Types (`entity_types`)

Hierarchical classification of entity structures:

| Category | Type Codes | Description |
|----------|------------|-------------|
| **PERSON** | `PROPER_PERSON_NATURAL`, `PROPER_PERSON_BENEFICIAL_OWNER` | Natural persons |
| **SHELL** | `limited_company`, `fund_umbrella`, `fund_subfund`, `SPV`, `TRUST_*`, `PARTNERSHIP_*` | Legal structures |

```
┌─────────────────────────────────────────────────────────────────────┐
│                         entity_types                                 │
├─────────────────────────────────────────────────────────────────────┤
│ entity_type_id (PK)      UUID                                       │
│ name                     VARCHAR     Human-readable name            │
│ type_code                VARCHAR     Machine identifier             │
│ entity_category          VARCHAR     PERSON | SHELL                 │
│ table_name               VARCHAR     Sub-table for attributes       │
│ parent_type_id           UUID → entity_types (hierarchy)            │
│ type_hierarchy_path      ARRAY       Ancestry path                  │
│ semantic_context         JSONB       Search context                 │
│ embedding                VECTOR(384) Semantic embedding             │
│ deprecated               BOOLEAN     Soft-delete flag               │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. CBU-Entity Role Linkage

### 3.1 The Junction Table (`cbu_entity_roles`)

**This is the core relationship** - it links entities to CBUs with specific roles.

```
┌─────────────────────────────────────────────────────────────────────┐
│                       cbu_entity_roles                               │
├─────────────────────────────────────────────────────────────────────┤
│ cbu_entity_role_id (PK)  UUID        Unique assignment ID           │
│ cbu_id                   UUID → cbus                                │
│ entity_id                UUID → entities                            │
│ role_id                  UUID → roles                               │
│ ownership_percentage     NUMERIC     % ownership (if applicable)    │
│ target_entity_id         UUID → entities  (for directed roles)      │
│ authority_limit          NUMERIC     Signing authority limit        │
│ authority_currency       VARCHAR     Currency for authority         │
│ requires_co_signatory    BOOLEAN     Dual signature required        │
│ effective_from           DATE        Role start date                │
│ effective_to             DATE        Role end date (null = active)  │
│ created_at / updated_at  TIMESTAMPTZ Audit timestamps               │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Entity Relationship Diagram

```
                              ┌─────────────┐
                              │   entities  │
                              │─────────────│
                              │ entity_id   │◄─────────────────────────┐
                              │ name        │                          │
                              │ entity_type │                          │
                              └──────┬──────┘                          │
                                     │                                 │
                                     │ 1:N                             │
                                     ▼                                 │
┌─────────────┐           ┌─────────────────────┐           ┌─────────┴───────┐
│    cbus     │           │  cbu_entity_roles   │           │     roles       │
│─────────────│◄──────────│─────────────────────│──────────►│─────────────────│
│ cbu_id (PK) │    N:1    │ cbu_id (FK)         │    N:1    │ role_id (PK)    │
│ name        │           │ entity_id (FK)      │           │ name            │
│ category    │           │ role_id (FK)        │           │ role_category   │
│ jurisdiction│           │ ownership_pct       │           │ ubo_treatment   │
│ status      │           │ effective_from/to   │           │ kyc_obligation  │
└─────────────┘           └─────────────────────┘           └─────────────────┘
```

### 3.3 The `roles` Table

107 pre-defined roles organized by category:

| Role Category | Example Roles | KYC Obligation |
|---------------|---------------|----------------|
| **OWNERSHIP_CHAIN** | `SHAREHOLDER`, `LIMITED_PARTNER`, `GENERAL_PARTNER`, `UBO` | FULL_KYC |
| **CONTROL_CHAIN** | `DIRECTOR`, `CHAIRMAN`, `CHIEF_EXECUTIVE`, `CONTROLLING_PERSON` | FULL_KYC / SCREEN_AND_ID |
| **FUND_STRUCTURE** | `ASSET_OWNER`, `MASTER_FUND`, `FEEDER_FUND`, `SUB_FUND` | SIMPLIFIED |
| **FUND_MANAGEMENT** | `MANAGEMENT_COMPANY`, `INVESTMENT_MANAGER`, `SPONSOR` | SIMPLIFIED / FULL_KYC |
| **SERVICE_PROVIDER** | `DEPOSITARY`, `CUSTODIAN`, `PRIME_BROKER`, `ADMINISTRATOR` | SIMPLIFIED |
| **TRUST_ROLES** | `SETTLOR`, `TRUSTEE`, `BENEFICIARY_FIXED`, `PROTECTOR` | FULL_KYC |
| **TRADING_EXECUTION** | `AUTHORIZED_SIGNATORY`, `AUTHORIZED_TRADER`, `SETTLEMENT_CONTACT` | SCREEN_AND_ID |
| **INVESTOR_CHAIN** | `NOMINEE`, `OMNIBUS_ACCOUNT`, `PLATFORM_INVESTOR` | FULL_KYC / SIMPLIFIED |

### 3.4 UBO Treatment Rules

The `ubo_treatment` field determines how to handle beneficial ownership:

| Treatment | Meaning | Action |
|-----------|---------|--------|
| `TERMINUS` | This IS the UBO | Stop here, full KYC |
| `LOOK_THROUGH` | Must identify who's behind | Continue up the chain |
| `LOOK_THROUGH_CONDITIONAL` | Look through if threshold met | Apply 25% rule |
| `BY_PERCENTAGE` | UBO if owns ≥25% | Check ownership_percentage |
| `CONTROL_PRONG` | Control = UBO regardless of % | Flag as controlling person |
| `EXEMPT` | Exempt entity (sovereign, listed) | No look-through required |
| `NOT_APPLICABLE` | Role doesn't affect UBO | Skip in UBO calculation |

---

## 4. Trading Matrix & Products

### 4.1 Product Subscriptions

CBUs subscribe to products, which determine what services they can access.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    cbu_product_subscriptions                         │
├─────────────────────────────────────────────────────────────────────┤
│ subscription_id (PK)     UUID                                       │
│ cbu_id                   UUID → cbus                                │
│ product_id               UUID → products                            │
│ status                   VARCHAR     ACTIVE | SUSPENDED | CANCELLED │
│ effective_from           DATE        Start date                     │
│ effective_to             DATE        End date (null = ongoing)      │
│ config                   JSONB       Product-specific config        │
│ created_at / updated_at  TIMESTAMPTZ                                │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 Products Table

```
┌─────────────────────────────────────────────────────────────────────┐
│                           products                                   │
├─────────────────────────────────────────────────────────────────────┤
│ product_id (PK)          UUID                                       │
│ name                     VARCHAR     e.g., "CUSTODY", "TA", "FA"    │
│ product_code             VARCHAR     Short code                     │
│ product_category         VARCHAR     CUSTODY | ADMINISTRATION       │
│ regulatory_framework     VARCHAR     UCITS | AIFMD | MiFID          │
│ requires_kyc             BOOLEAN     KYC gate                       │
│ kyc_risk_rating          VARCHAR     LOW | MEDIUM | HIGH            │
│ min_asset_requirement    NUMERIC     Minimum AUM                    │
│ is_active                BOOLEAN     Active flag                    │
│ metadata                 JSONB       Additional config              │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.3 Trading Profiles

The trading profile defines WHAT a CBU can trade (instruments × markets × currencies).

```
┌─────────────────────────────────────────────────────────────────────┐
│                     cbu_trading_profiles                             │
├─────────────────────────────────────────────────────────────────────┤
│ profile_id (PK)          UUID                                       │
│ cbu_id                   UUID → cbus                                │
│ version                  INTEGER     Version number                 │
│ status                   VARCHAR     DRAFT | ACTIVE | SUPERSEDED    │
│ document                 JSONB       The trading matrix definition  │
│ document_hash            TEXT        Hash for change detection      │
│ created_by / created_at  VARCHAR/TIMESTAMPTZ                        │
│ activated_at/by          TIMESTAMPTZ/VARCHAR                        │
│ validated_at/by          TIMESTAMPTZ/VARCHAR                        │
│ submitted_at/by          TIMESTAMPTZ/VARCHAR                        │
│ materialization_status   VARCHAR     PENDING | COMPLETE | FAILED    │
│ materialized_at          TIMESTAMPTZ When matrix was expanded       │
│ sla_profile_id           UUID        SLA commitments                │
│ source_document_id       UUID → document_catalog                    │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.4 Trading Matrix Structure (JSONB)

The `document` field contains the trading matrix definition:

```json
{
  "instruments": [
    {
      "class": "EQUITY",
      "markets": ["XLON", "XNYS", "XPAR"],
      "currencies": ["GBP", "USD", "EUR"],
      "settlement_types": ["DVP", "FOP"]
    },
    {
      "class": "FIXED_INCOME",
      "markets": ["XLON", "XFRA"],
      "currencies": ["GBP", "EUR"],
      "settlement_types": ["DVP"]
    },
    {
      "class": "OTC_IRS",
      "counterparties": ["Goldman Sachs", "JP Morgan"],
      "currencies": ["USD", "EUR"],
      "governing_law": "ISDA_NY"
    }
  ],
  "default_settlement": {
    "EQUITY": { "cycle": "T+2", "method": "DVP" },
    "FIXED_INCOME": { "cycle": "T+2", "method": "DVP" }
  }
}
```

### 4.5 Matrix Product Overlay

Links trading matrix entries to specific products and counterparties:

```
┌─────────────────────────────────────────────────────────────────────┐
│                   cbu_matrix_product_overlay                         │
├─────────────────────────────────────────────────────────────────────┤
│ overlay_id (PK)          UUID                                       │
│ cbu_id                   UUID → cbus                                │
│ subscription_id          UUID → cbu_product_subscriptions           │
│ instrument_class_id      UUID → custody.instrument_classes          │
│ market_id                UUID → custody.markets                     │
│ counterparty_entity_id   UUID → entities (for OTC)                  │
│ status                   VARCHAR     ENABLED | DISABLED             │
│ config                   JSONB       Override configuration         │
│ created_at / updated_at  TIMESTAMPTZ                                │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.6 Relationship Diagram: CBU → Products → Trading

```
┌─────────────┐         ┌─────────────────────────┐         ┌─────────────┐
│    cbus     │         │ cbu_product_subscriptions│         │  products   │
│─────────────│◄────────│─────────────────────────│────────►│─────────────│
│ cbu_id      │   1:N   │ cbu_id                  │   N:1   │ product_id  │
│ name        │         │ product_id              │         │ name        │
└──────┬──────┘         │ status                  │         │ category    │
       │                │ effective_from          │         └─────────────┘
       │                └───────────┬─────────────┘
       │                            │
       │ 1:N                        │ 1:N (via subscription_id)
       ▼                            ▼
┌─────────────────────┐    ┌─────────────────────────┐
│ cbu_trading_profiles│    │ cbu_matrix_product_overlay│
│─────────────────────│    │─────────────────────────│
│ profile_id          │    │ subscription_id         │
│ cbu_id              │    │ instrument_class_id     │──────►custody.instrument_classes
│ document (JSONB)    │    │ market_id               │──────►custody.markets
│ status              │    │ counterparty_entity_id  │──────►entities
└─────────────────────┘    └─────────────────────────┘
```

---

## 5. Investor Register

### 5.1 Overview

The investor register tracks who owns what in each CBU's share classes.

```
CBU (Fund)
  └── Share Classes (what can be owned)
        └── Holdings (who owns how much)
              └── Investors (the entity that owns)
                    └── Entity (the actual person/company)
```

### 5.2 Share Classes (`kyc.share_classes`)

```
┌─────────────────────────────────────────────────────────────────────┐
│                       kyc.share_classes                              │
├─────────────────────────────────────────────────────────────────────┤
│ id (PK)                  UUID                                       │
│ cbu_id                   UUID → ob-poc.cbus                         │
│ name                     VARCHAR     "Class A EUR Acc"              │
│ isin                     VARCHAR     International identifier       │
│ currency                 CHAR(3)     Base currency                  │
│ nav_per_share            NUMERIC     Current NAV                    │
│ nav_date                 DATE        NAV date                       │
│ management_fee_bps       INTEGER     Fee in basis points            │
│ performance_fee_bps      INTEGER     Performance fee                │
│ minimum_investment       NUMERIC     Minimum subscription           │
│ subscription_frequency   VARCHAR     DAILY | WEEKLY | MONTHLY       │
│ redemption_frequency     VARCHAR     DAILY | WEEKLY | MONTHLY       │
│ redemption_notice_days   INTEGER     Notice period                  │
│ lock_up_period_months    INTEGER     Lock-up period                 │
│ investor_eligibility     VARCHAR     RETAIL | PROFESSIONAL | INSTITUTIONAL │
│ status                   VARCHAR     ACTIVE | CLOSED | SOFT_CLOSED  │
│ fund_type                VARCHAR     UCITS | AIF | ELTIF            │
│ fund_structure           VARCHAR     SICAV | FCP | OEIC             │
│ instrument_kind          VARCHAR     EQUITY | DEBT | HYBRID         │
│ votes_per_unit           NUMERIC     Voting rights                  │
│ economic_per_unit        NUMERIC     Economic rights per unit       │
│ entity_id                UUID → entities (the share class as entity)│
│ issuer_entity_id         UUID → entities (issuing entity)           │
│ compartment_id           UUID        For umbrella funds             │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.3 Investors (`kyc.investors`)

```
┌─────────────────────────────────────────────────────────────────────┐
│                        kyc.investors                                 │
├─────────────────────────────────────────────────────────────────────┤
│ investor_id (PK)         UUID                                       │
│ entity_id                UUID → ob-poc.entities                     │
│ owning_cbu_id            UUID → ob-poc.cbus (which CBU manages)     │
│ investor_type            VARCHAR     INDIVIDUAL | INSTITUTIONAL     │
│ investor_category        VARCHAR     RETAIL | PROFESSIONAL | ELIGIBLE│
│ lifecycle_state          VARCHAR     See lifecycle states below     │
│ kyc_status               VARCHAR     PENDING | APPROVED | EXPIRED   │
│ kyc_case_id              UUID → kyc.cases                           │
│ kyc_risk_rating          VARCHAR     LOW | MEDIUM | HIGH            │
│ kyc_approved_at          TIMESTAMPTZ                                │
│ kyc_expires_at           TIMESTAMPTZ                                │
│ tax_status               VARCHAR     Tax classification             │
│ tax_jurisdiction         VARCHAR     Tax residence                  │
│ fatca_status             VARCHAR     US tax status                  │
│ crs_status               VARCHAR     CRS classification             │
│ eligible_fund_types      ARRAY       What fund types allowed        │
│ restricted_jurisdictions ARRAY       Blocked jurisdictions          │
│ provider                 VARCHAR     TA system source               │
│ provider_reference       VARCHAR     External reference             │
│ first_subscription_at    TIMESTAMPTZ First investment date          │
│ created_at / updated_at  TIMESTAMPTZ                                │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.4 Investor Lifecycle States

```
PROSPECT → ONBOARDING → ACTIVE → SUSPENDED → OFFBOARDED
    │          │           │         │            │
    │          │           │         │            └─► Fully exited
    │          │           │         └─► Temporarily blocked
    │          │           └─► Can trade
    │          └─► KYC in progress
    └─► Initial contact, not yet onboarding
```

### 5.5 Holdings (`kyc.holdings`)

```
┌─────────────────────────────────────────────────────────────────────┐
│                         kyc.holdings                                 │
├─────────────────────────────────────────────────────────────────────┤
│ id (PK)                  UUID                                       │
│ share_class_id           UUID → kyc.share_classes                   │
│ investor_id              UUID → kyc.investors                       │
│ investor_entity_id       UUID → ob-poc.entities                     │
│ units                    NUMERIC     Number of units held           │
│ cost_basis               NUMERIC     Original investment amount     │
│ acquisition_date         DATE        When acquired                  │
│ status                   VARCHAR     ACTIVE | REDEEMED | TRANSFERRED│
│ holding_status           VARCHAR     CONFIRMED | PENDING            │
│ usage_type               VARCHAR     BENEFICIAL | NOMINEE           │
│ provider                 VARCHAR     TA system source               │
│ provider_reference       VARCHAR     External reference             │
│ created_at / updated_at  TIMESTAMPTZ                                │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.6 Investor Register ERD

```
┌─────────────┐         ┌─────────────────┐         ┌─────────────┐
│    cbus     │         │  share_classes  │         │  holdings   │
│─────────────│◄────────│─────────────────│◄────────│─────────────│
│ cbu_id      │   1:N   │ id              │   1:N   │ id          │
│ name        │         │ cbu_id          │         │ share_class │
│ category    │         │ name            │         │ investor_id │
└─────────────┘         │ isin            │         │ units       │
                        │ currency        │         │ cost_basis  │
                        └─────────────────┘         └──────┬──────┘
                                                          │
                                                          │ N:1
                                                          ▼
                        ┌─────────────────┐         ┌─────────────┐
                        │    investors    │         │  entities   │
                        │─────────────────│────────►│─────────────│
                        │ investor_id     │   N:1   │ entity_id   │
                        │ entity_id       │         │ name        │
                        │ kyc_status      │         │ entity_type │
                        │ lifecycle_state │         └─────────────┘
                        │ owning_cbu_id   │
                        └─────────────────┘
```

---

## 6. Key Views

### 6.1 CBU Entity Graph View

```sql
-- v_cbu_entity_graph: All entities linked to a CBU with their roles
SELECT 
    c.cbu_id,
    c.name AS cbu_name,
    e.entity_id,
    e.name AS entity_name,
    r.name AS role_name,
    r.role_category,
    cer.ownership_percentage,
    cer.effective_from,
    cer.effective_to
FROM cbus c
JOIN cbu_entity_roles cer ON c.cbu_id = cer.cbu_id
JOIN entities e ON cer.entity_id = e.entity_id
JOIN roles r ON cer.role_id = r.role_id
WHERE cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE;
```

### 6.2 CBU Trading Matrix Effective View

```sql
-- v_cbu_matrix_effective: Materialized trading permissions
SELECT 
    cbu_id,
    instrument_class,
    market_mic,
    currency,
    settlement_type,
    is_enabled,
    counterparty_name  -- For OTC
FROM cbu_matrix_effective
WHERE is_enabled = true;
```

### 6.3 CBU Investor Summary View

```sql
-- v_cbu_investor_details: Holdings per CBU
SELECT 
    sc.cbu_id,
    c.name AS cbu_name,
    i.investor_id,
    e.name AS investor_name,
    i.investor_type,
    i.kyc_status,
    sc.name AS share_class,
    h.units,
    h.units * sc.nav_per_share AS market_value
FROM kyc.share_classes sc
JOIN ob-poc.cbus c ON sc.cbu_id = c.cbu_id
JOIN kyc.holdings h ON sc.id = h.share_class_id
JOIN kyc.investors i ON h.investor_id = i.investor_id
JOIN ob-poc.entities e ON i.entity_id = e.entity_id;
```

---

## 7. Complete Data Flow

### 7.1 CBU Onboarding Flow

```
1. ENTITY CREATION
   └── Create entities (ManCo, Fund Manager, Directors, etc.)
   
2. CBU CREATION
   └── Create CBU with category, jurisdiction, status
   
3. ROLE ASSIGNMENT
   └── Link entities to CBU via cbu_entity_roles
   └── Assign: ASSET_OWNER, MANAGEMENT_COMPANY, DIRECTORS, etc.
   
4. PRODUCT SUBSCRIPTION
   └── Subscribe CBU to products (CUSTODY, TA, FA)
   
5. TRADING PROFILE
   └── Define what the CBU can trade
   └── Instruments × Markets × Currencies × Counterparties
   
6. SHARE CLASSES (if Fund)
   └── Create share classes with terms
   
7. INVESTOR ONBOARDING
   └── Create investors (linked to entities)
   └── KYC approval
   
8. HOLDINGS
   └── Record investor holdings per share class
```

### 7.2 Full Entity Relationship Diagram

```
                                    ┌──────────────────┐
                                    │   entity_types   │
                                    └────────┬─────────┘
                                             │
                                             │ 1:N
                                             ▼
┌──────────────────┐               ┌──────────────────┐
│     products     │               │     entities     │◄──────────────────────┐
└────────┬─────────┘               └────────┬─────────┘                       │
         │                                  │                                 │
         │                                  │                                 │
         │ N:1                              │ 1:N                             │
         ▼                                  ▼                                 │
┌──────────────────┐               ┌──────────────────┐         ┌────────────┴───────┐
│      cbus        │◄──────────────│ cbu_entity_roles │────────►│      roles         │
└────────┬─────────┘      1:N      └──────────────────┘   N:1   └────────────────────┘
         │
         │ 1:N
         ├─────────────────────────────────┬──────────────────────────────┐
         ▼                                 ▼                              ▼
┌──────────────────────┐    ┌──────────────────────────┐    ┌──────────────────┐
│cbu_product_subscriptions│    │  cbu_trading_profiles    │    │  share_classes   │
└──────────────────────┘    └──────────────────────────┘    └────────┬─────────┘
                                                                     │
                                                                     │ 1:N
                                                                     ▼
                            ┌──────────────────┐            ┌──────────────────┐
                            │    investors     │◄───────────│    holdings      │
                            └────────┬─────────┘      N:1   └──────────────────┘
                                     │
                                     │ N:1
                                     ▼
                            ┌──────────────────┐
                            │     entities     │
                            └──────────────────┘
```

---

## 8. Indexing Strategy

### 8.1 Primary Indexes

| Table | Index | Purpose |
|-------|-------|---------|
| `cbus` | `cbu_id` (PK) | Primary lookup |
| `cbus` | `commercial_client_entity_id` | Client grouping |
| `cbus` | `jurisdiction` | Jurisdiction filtering |
| `cbus` | `status` | Status filtering |
| `entities` | `entity_id` (PK) | Primary lookup |
| `entities` | `name_norm` | Name search (trigram) |
| `entities` | `external_id` | LEI lookup |
| `cbu_entity_roles` | `(cbu_id, entity_id, role_id)` | Unique assignment |
| `cbu_entity_roles` | `entity_id` | Entity's roles |

### 8.2 Semantic Search Indexes (pgvector)

```sql
-- CBU semantic search
CREATE INDEX idx_cbus_embedding ON cbus 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Entity semantic search
CREATE INDEX idx_entities_embedding ON entities 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
```

---

## 9. Key Queries

### 9.1 Get All Entities for a CBU

```sql
SELECT e.entity_id, e.name, r.name as role, cer.ownership_percentage
FROM cbu_entity_roles cer
JOIN entities e ON cer.entity_id = e.entity_id
JOIN roles r ON cer.role_id = r.role_id
WHERE cer.cbu_id = $1
AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE);
```

### 9.2 Get CBUs for an Entity

```sql
SELECT c.cbu_id, c.name, r.name as role
FROM cbu_entity_roles cer
JOIN cbus c ON cer.cbu_id = c.cbu_id
JOIN roles r ON cer.role_id = r.role_id
WHERE cer.entity_id = $1
AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE);
```

### 9.3 Get Trading Matrix for a CBU

```sql
SELECT * FROM "ob-poc".v_cbu_matrix_effective
WHERE cbu_id = $1 AND is_enabled = true;
```

### 9.4 Get Investor Holdings for a CBU

```sql
SELECT 
    i.investor_id,
    e.name as investor_name,
    sc.name as share_class,
    h.units,
    h.units * sc.nav_per_share as value
FROM kyc.share_classes sc
JOIN kyc.holdings h ON sc.id = h.share_class_id
JOIN kyc.investors i ON h.investor_id = i.investor_id
JOIN "ob-poc".entities e ON i.entity_id = e.entity_id
WHERE sc.cbu_id = $1 AND h.status = 'ACTIVE';
```

---

## 10. Summary

### Key Design Principles

1. **CBU-Centric**: Everything resolves to CBU sets
2. **Role-Based Access**: Entities linked via explicit roles with KYC obligations
3. **Temporal Tracking**: Effective dates on relationships, audit timestamps everywhere
4. **Hierarchical Types**: Entity types and roles have categories and hierarchies
5. **JSONB Flexibility**: Trading profiles and configs use JSONB for flexibility
6. **Semantic Search**: pgvector embeddings for natural language queries

### Table Counts by Domain

| Domain | Tables | Views |
|--------|--------|-------|
| CBU Core | 25 | 20 |
| Entity | 15 | 10 |
| Products | 10 | 5 |
| Investor | 10 | 8 |
| KYC | 20 | 10 |
| Custody | 34 | 6 |

---

## Appendix A: Migration Files

Key migrations for this architecture:

| Migration | Description |
|-----------|-------------|
| `001` | CBU category constraints |
| `004` | Entity type coverage |
| `007` | BODS UBO layer |
| `008-009` | KYC control roles |
| `011` | Investor register |
| `013` | Capital structure/ownership |
| `027` | Trading matrix canonical pivot |
| `045` | Legal contracts |
| `048` | Client groups |

---

## Appendix B: Foreign Key Reference

All foreign keys from `cbu_*` tables:

| Source Table | Column | Target Table |
|--------------|--------|--------------|
| `cbus` | `commercial_client_entity_id` | `entities` |
| `cbus` | `product_id` | `products` |
| `cbu_entity_roles` | `cbu_id` | `cbus` |
| `cbu_entity_roles` | `entity_id` | `entities` |
| `cbu_entity_roles` | `role_id` | `roles` |
| `cbu_product_subscriptions` | `cbu_id` | `cbus` |
| `cbu_product_subscriptions` | `product_id` | `products` |
| `cbu_trading_profiles` | `cbu_id` | `cbus` |
| `cbu_matrix_product_overlay` | `cbu_id` | `cbus` |
| `cbu_matrix_product_overlay` | `instrument_class_id` | `custody.instrument_classes` |
| `cbu_matrix_product_overlay` | `market_id` | `custody.markets` |
| `share_classes` | `cbu_id` | `cbus` |
| `holdings` | `share_class_id` | `share_classes` |
| `holdings` | `investor_id` | `investors` |
| `investors` | `entity_id` | `entities` |
| `investors` | `owning_cbu_id` | `cbus` |
