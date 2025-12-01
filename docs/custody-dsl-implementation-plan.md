# Custody & Settlement DSL Implementation Plan

## Executive Summary

This document defines the implementation plan for extending the ob-poc DSL to support custody bank onboarding for settlement instruction management. The goal is to capture the **"what"** of custody onboarding (instruments, SSIs, instruction paths) that configures existing service resources (SWIFT gateway, routing) to handle the **"how"**.

**Perspective**: Bank/Custodian side, receiving instructions from investment managers ("the street").

**Scope**: 
- Standing Settlement Instructions (SSI)
- Instrument Matrix / Instruction Map
- Trade settlement instruction routing
- Service resource configuration linkage

**Out of Scope**:
- SWIFT message generation (handled by service resources)
- Trade matching/affirmation logic (CTM-side, not custodian-side)
- ISDA/CSA collateral management (phase 2)

---

## 1. Data Model Architecture

### 1.1 Conceptual Layers

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     REFERENCE DATA LAYER (Bank-wide)                     │
├─────────────────────────────────────────────────────────────────────────┤
│  Markets              │  Instrument Classes    │  Settlement Profiles   │
│  - MIC code           │  - Class code          │  - Profile per         │
│  - CSD BIC            │  - SWIFT msg family    │    Class × Market      │
│  - Operating hours    │  - Default cycle       │  - Settlement type     │
│  - Settlement ccy     │  - Asset category      │  - Matching rules      │
│                       │                        │  - Instruction path    │
├─────────────────────────────────────────────────────────────────────────┤
│                      INSTRUCTION PATH LAYER                              │
├─────────────────────────────────────────────────────────────────────────┤
│  Instruction Types         │  Path Definitions                          │
│  - RECEIVE_DVP (MT541)     │  - InstructionType → ServiceResource       │
│  - DELIVER_DVP (MT543)     │  - Routing rules                           │
│  - RECEIVE_FOP (MT540)     │  - Enrichment sources                      │
│  - DELIVER_FOP (MT542)     │  - Validation requirements                 │
│  - FUND_SUBSCRIPTION       │                                            │
│  - FUND_REDEMPTION         │                                            │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                      CLIENT DATA LAYER (Per CBU)                         │
├─────────────────────────────────────────────────────────────────────────┤
│  Client SSIs                    │  Counterparty SSIs                    │
│  - CBU → Safekeeping accounts   │  - Street-side settlement identity    │
│  - Agent chain (PSET, REAG...)  │  - Sourced from ALERT/CTM             │
│  - Linked to Settlement Profile │  - Used for instruction matching      │
│  - Effective/expiry dates       │                                       │
├─────────────────────────────────────────────────────────────────────────┤
│  Client Instruction Config                                               │
│  - CBU × Profile → SSI mapping                                          │
│  - Override rules per client                                            │
│  - Service resource instance binding                                    │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Instrument Matrix is reference data, not per-client** | Settlement rules for "US Equities in NYSE" are the same for all clients. Only the accounts differ. |
| **SSI links to Profile, not directly to Instrument** | A client's SSI for "US Equities" applies to the entire asset class, not individual ISINs. |
| **Counterparty SSIs are entities with settlement attributes** | Reuse existing entity model; add settlement-specific extension table. |
| **Instruction paths route to Service Resources** | The DSL configures which `service_resource_instance` handles which instruction type. The resource does the actual SWIFT work. |
| **Markets and Instrument Classes are seed data** | Loaded once, maintained by ops. DSL can reference but rarely creates. |

### 1.3 Relationship to Existing Model

```
Existing ob-poc Model              Custody Extension
─────────────────────              ─────────────────
CBU ─────────────────────────────→ Client SSI
  │                                  │
  └─ Entity (counterparty) ───────→ Counterparty SSI (extension)
                                     │
Product ──→ Service ──→ Resource ──→ Instruction Path
                           │           │
                           └───────────┴─→ Service Resource Instance
                                            (SWIFT Gateway config)
```

---

## 2. Database Schema Design

### 2.1 Reference Data Tables (schema: `custody`)

```sql
-- =============================================================================
-- MARKETS
-- =============================================================================
CREATE TABLE custody.markets (
    market_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mic VARCHAR(4) NOT NULL UNIQUE,  -- ISO 10383 Market Identifier Code
    name VARCHAR(255) NOT NULL,
    country_code VARCHAR(2) NOT NULL,  -- ISO 3166-1 alpha-2
    csd_bic VARCHAR(11),  -- Central Securities Depository BIC
    operating_mic VARCHAR(4),  -- Parent operating MIC if segment
    settlement_currency VARCHAR(3) NOT NULL,  -- Primary settlement currency
    timezone VARCHAR(50) NOT NULL,
    cut_off_time TIME,  -- Settlement instruction cut-off
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- INSTRUMENT CLASSES
-- =============================================================================
CREATE TABLE custody.instrument_classes (
    class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    class_code VARCHAR(20) NOT NULL UNIQUE,  -- EQUITY, FIXED_INCOME, FUND, DERIVATIVE
    name VARCHAR(100) NOT NULL,
    asset_category VARCHAR(50),  -- ISO 10962 CFI category
    default_settlement_cycle VARCHAR(10) NOT NULL,  -- T+1, T+2, T+0
    swift_msg_family VARCHAR(10),  -- MT54x, MT50x
    requires_isin BOOLEAN DEFAULT true,
    requires_quantity BOOLEAN DEFAULT true,
    requires_price BOOLEAN DEFAULT true,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- SETTLEMENT PROFILES (Instrument Matrix core)
-- =============================================================================
CREATE TABLE custody.settlement_profiles (
    profile_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_code VARCHAR(50) NOT NULL UNIQUE,  -- e.g., "EQUITY_XNYS_DVP"
    class_id UUID NOT NULL REFERENCES custody.instrument_classes(class_id),
    market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    settlement_type VARCHAR(10) NOT NULL,  -- DVP, FOP, RVP, DFP
    settlement_cycle VARCHAR(10) NOT NULL,  -- Override of class default
    matching_required BOOLEAN DEFAULT true,
    partial_settlement_allowed BOOLEAN DEFAULT false,
    hold_release_supported BOOLEAN DEFAULT false,
    priority_default INTEGER DEFAULT 1,  -- 1-4 SWIFT priority
    narrative_required BOOLEAN DEFAULT false,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(class_id, market_id, settlement_type)
);

-- =============================================================================
-- INSTRUCTION TYPES
-- =============================================================================
CREATE TABLE custody.instruction_types (
    type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type_code VARCHAR(30) NOT NULL UNIQUE,  -- RECEIVE_DVP, DELIVER_FOP, etc.
    name VARCHAR(100) NOT NULL,
    direction VARCHAR(10) NOT NULL,  -- RECEIVE, DELIVER
    payment_type VARCHAR(10) NOT NULL,  -- DVP, FOP
    swift_mt_code VARCHAR(10),  -- MT540, MT541, MT542, MT543
    iso20022_msg_type VARCHAR(50),  -- sese.023, etc.
    applies_to_class_ids UUID[],  -- NULL = all classes
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- INSTRUCTION PATHS (Profile → Service Resource routing)
-- =============================================================================
CREATE TABLE custody.instruction_paths (
    path_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES custody.settlement_profiles(profile_id),
    instruction_type_id UUID NOT NULL REFERENCES custody.instruction_types(type_id),
    resource_id UUID NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id),
    routing_priority INTEGER DEFAULT 1,  -- For failover scenarios
    enrichment_sources JSONB,  -- ["ALERT", "STATIC", "CLIENT_OVERRIDE"]
    validation_rules JSONB,  -- Custom validation per path
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(profile_id, instruction_type_id, routing_priority)
);

-- =============================================================================
-- AGENT ROLES (for SSI agent chains)
-- =============================================================================
CREATE TABLE custody.agent_roles (
    role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role_code VARCHAR(10) NOT NULL UNIQUE,  -- PSET, REAG, DEAG, BUYR, SELL, SAFE
    name VARCHAR(50) NOT NULL,
    swift_qualifier VARCHAR(4),  -- :95a qualifier
    is_mandatory BOOLEAN DEFAULT false,
    sequence_hint INTEGER,  -- Typical position in chain
    description TEXT
);
```

### 2.2 Client Data Tables (schema: `custody`)

```sql
-- =============================================================================
-- CLIENT SSI RECORDS
-- =============================================================================
CREATE TABLE custody.client_ssi (
    ssi_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    profile_id UUID NOT NULL REFERENCES custody.settlement_profiles(profile_id),
    ssi_reference VARCHAR(50),  -- Client's internal SSI reference
    safekeeping_account VARCHAR(35) NOT NULL,  -- :97a SAFE
    safekeeping_bic VARCHAR(11) NOT NULL,  -- Custodian BIC
    cash_account VARCHAR(35),  -- :97a CASH (if DVP)
    cash_account_bic VARCHAR(11),
    status VARCHAR(20) DEFAULT 'PENDING',  -- PENDING, ACTIVE, SUSPENDED, EXPIRED
    effective_date DATE NOT NULL,
    expiry_date DATE,
    source VARCHAR(20) DEFAULT 'MANUAL',  -- MANUAL, ALERT, IMPORT
    source_reference VARCHAR(100),  -- ALERT account ID, etc.
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    created_by VARCHAR(100),
    UNIQUE(cbu_id, profile_id, safekeeping_account, effective_date)
);

-- =============================================================================
-- SSI AGENT CHAIN
-- =============================================================================
CREATE TABLE custody.ssi_agent_chain (
    agent_link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ssi_id UUID NOT NULL REFERENCES custody.client_ssi(ssi_id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES custody.agent_roles(role_id),
    agent_bic VARCHAR(11) NOT NULL,
    agent_account VARCHAR(35),
    agent_name VARCHAR(100),
    sequence_order INTEGER NOT NULL,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(ssi_id, role_id, sequence_order)
);

-- =============================================================================
-- COUNTERPARTY SETTLEMENT IDENTITY (extends existing entities)
-- =============================================================================
CREATE TABLE custody.counterparty_ssi (
    counterparty_ssi_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    profile_id UUID NOT NULL REFERENCES custody.settlement_profiles(profile_id),
    counterparty_bic VARCHAR(11) NOT NULL,
    safekeeping_account VARCHAR(35),
    source VARCHAR(20) DEFAULT 'ALERT',  -- ALERT, MANUAL, CTM
    alert_account_id VARCHAR(50),  -- DTCC ALERT reference
    status VARCHAR(20) DEFAULT 'ACTIVE',
    effective_date DATE NOT NULL,
    expiry_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(entity_id, profile_id, counterparty_bic, effective_date)
);

-- =============================================================================
-- CLIENT INSTRUCTION CONFIG (CBU-specific overrides)
-- =============================================================================
CREATE TABLE custody.client_instruction_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    ssi_id UUID NOT NULL REFERENCES custody.client_ssi(ssi_id),
    path_id UUID NOT NULL REFERENCES custody.instruction_paths(path_id),
    service_instance_id UUID REFERENCES "ob-poc".cbu_service_resource_instances(instance_id),
    priority_override INTEGER,
    auto_release BOOLEAN DEFAULT false,
    hold_code VARCHAR(10),
    narrative_template TEXT,
    custom_config JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, ssi_id, path_id)
);
```

### 2.3 Seed Data Requirements

The following reference data should be seeded during implementation:

**Markets** (subset - expand as needed):
| MIC | Name | CSD BIC | Currency |
|-----|------|---------|----------|
| XNYS | New York Stock Exchange | DTCYUS33 | USD |
| XNAS | NASDAQ | DTCYUS33 | USD |
| XLON | London Stock Exchange | CABOROCP | GBP |
| XPAR | Euronext Paris | SICABOROCP | EUR |
| XETR | Deutsche Börse Xetra | DAABOROCP | EUR |
| XTKS | Tokyo Stock Exchange | JABOROCP | JPY |

**Instrument Classes**:
| Code | Name | Cycle | SWIFT Family |
|------|------|-------|--------------|
| EQUITY | Equities | T+1 | MT54x |
| FIXED_INCOME | Fixed Income | T+1 | MT54x |
| GOVT_BOND | Government Bonds | T+1 | MT54x |
| CORP_BOND | Corporate Bonds | T+2 | MT54x |
| FUND_UCITS | UCITS Funds | T+2 | MT50x |
| FUND_ETF | ETFs | T+1 | MT54x |
| MONEY_MARKET | Money Market | T+0 | MT54x |

**Agent Roles**:
| Code | Name | SWIFT Qualifier |
|------|------|-----------------|
| PSET | Place of Settlement | PSET |
| REAG | Receiving Agent | REAG |
| DEAG | Delivering Agent | DEAG |
| BUYR | Buyer | BUYR |
| SELL | Seller | SELL |
| SAFE | Safekeeping Account | SAFE |

**Instruction Types**:
| Code | Direction | Payment | SWIFT |
|------|-----------|---------|-------|
| RECEIVE_DVP | RECEIVE | DVP | MT541 |
| RECEIVE_FOP | RECEIVE | FOP | MT540 |
| DELIVER_DVP | DELIVER | DVP | MT543 |
| DELIVER_FOP | DELIVER | FOP | MT542 |

---

## 3. DSL Domain Design

### 3.1 New Domains

Add to `config/verbs.yaml`:

```yaml
# =============================================================================
# DOMAIN: market (Reference Data - Markets)
# =============================================================================
market:
  description: "Market reference data operations"
  
  verbs:
    ensure:
      description: "Create or update a market"
      behavior: crud
      crud:
        operation: upsert
        table: markets
        schema: custody
        conflict_keys: [mic]
        returning: market_id
      args:
        - name: mic
          type: string
          required: true
          maps_to: mic
        - name: name
          type: string
          required: true
          maps_to: name
        - name: country
          type: string
          required: true
          maps_to: country_code
        - name: csd-bic
          type: string
          required: false
          maps_to: csd_bic
        - name: currency
          type: string
          required: true
          maps_to: settlement_currency
        - name: timezone
          type: string
          required: true
          maps_to: timezone
        - name: cut-off
          type: string
          required: false
          maps_to: cut_off_time
      returns:
        type: uuid
        name: market_id
        capture: true

    read:
      description: "Read market by MIC"
      behavior: crud
      crud:
        operation: select
        table: markets
        schema: custody
      args:
        - name: mic
          type: string
          required: true
          maps_to: mic
      returns:
        type: record

    list:
      description: "List markets"
      behavior: crud
      crud:
        operation: select
        table: markets
        schema: custody
      args:
        - name: country
          type: string
          required: false
          maps_to: country_code
        - name: is-active
          type: boolean
          required: false
          maps_to: is_active
      returns:
        type: record_set

# =============================================================================
# DOMAIN: instrument-class (Reference Data - Instrument Classes)
# =============================================================================
instrument-class:
  description: "Instrument class reference data"
  
  verbs:
    ensure:
      description: "Create or update an instrument class"
      behavior: crud
      crud:
        operation: upsert
        table: instrument_classes
        schema: custody
        conflict_keys: [class_code]
        returning: class_id
      args:
        - name: code
          type: string
          required: true
          maps_to: class_code
        - name: name
          type: string
          required: true
          maps_to: name
        - name: category
          type: string
          required: false
          maps_to: asset_category
        - name: settlement-cycle
          type: string
          required: true
          maps_to: default_settlement_cycle
        - name: swift-family
          type: string
          required: false
          maps_to: swift_msg_family
      returns:
        type: uuid
        name: class_id
        capture: true

    read:
      description: "Read instrument class by code"
      behavior: crud
      crud:
        operation: select
        table: instrument_classes
        schema: custody
      args:
        - name: code
          type: string
          required: true
          maps_to: class_code
      returns:
        type: record

# =============================================================================
# DOMAIN: settlement-profile (Instrument Matrix)
# =============================================================================
settlement-profile:
  description: "Settlement profile (Instrument Matrix) operations"
  
  verbs:
    create:
      description: "Create a settlement profile for class/market combination"
      behavior: crud
      crud:
        operation: insert
        table: settlement_profiles
        schema: custody
        returning: profile_id
      args:
        - name: code
          type: string
          required: true
          maps_to: profile_code
        - name: instrument-class
          type: lookup
          required: true
          lookup:
            table: instrument_classes
            schema: custody
            code_column: class_code
            id_column: class_id
        - name: market
          type: lookup
          required: true
          lookup:
            table: markets
            schema: custody
            code_column: mic
            id_column: market_id
        - name: settlement-type
          type: string
          required: true
          maps_to: settlement_type
          valid_values: [DVP, FOP, RVP, DFP]
        - name: settlement-cycle
          type: string
          required: true
          maps_to: settlement_cycle
        - name: matching-required
          type: boolean
          required: false
          maps_to: matching_required
          default: true
        - name: partial-allowed
          type: boolean
          required: false
          maps_to: partial_settlement_allowed
          default: false
      returns:
        type: uuid
        name: profile_id
        capture: true

    ensure:
      description: "Create or update settlement profile"
      behavior: crud
      crud:
        operation: upsert
        table: settlement_profiles
        schema: custody
        conflict_keys: [profile_code]
        returning: profile_id
      args:
        # Same as create
      returns:
        type: uuid
        name: profile_id
        capture: true

    add-instruction-path:
      description: "Add instruction path to settlement profile"
      behavior: crud
      crud:
        operation: insert
        table: instruction_paths
        schema: custody
        returning: path_id
      args:
        - name: profile-id
          type: uuid
          required: true
          maps_to: profile_id
        - name: instruction-type
          type: lookup
          required: true
          lookup:
            table: instruction_types
            schema: custody
            code_column: type_code
            id_column: type_id
        - name: resource
          type: lookup
          required: true
          lookup:
            table: service_resource_types
            schema: ob-poc
            code_column: resource_code
            id_column: resource_id
        - name: priority
          type: integer
          required: false
          maps_to: routing_priority
          default: 1
        - name: enrichment-sources
          type: json
          required: false
          maps_to: enrichment_sources
        - name: validation-rules
          type: json
          required: false
          maps_to: validation_rules
      returns:
        type: uuid
        name: path_id
        capture: true

    list:
      description: "List settlement profiles"
      behavior: crud
      crud:
        operation: select
        table: settlement_profiles
        schema: custody
      args:
        - name: market
          type: lookup
          required: false
          lookup:
            table: markets
            schema: custody
            code_column: mic
            id_column: market_id
        - name: instrument-class
          type: lookup
          required: false
          lookup:
            table: instrument_classes
            schema: custody
            code_column: class_code
            id_column: class_id
        - name: settlement-type
          type: string
          required: false
          maps_to: settlement_type
      returns:
        type: record_set

# =============================================================================
# DOMAIN: ssi (Standing Settlement Instructions)
# =============================================================================
ssi:
  description: "Client Standing Settlement Instruction operations"
  
  verbs:
    create:
      description: "Create a client SSI"
      behavior: crud
      crud:
        operation: insert
        table: client_ssi
        schema: custody
        returning: ssi_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: profile
          type: lookup
          required: true
          lookup:
            table: settlement_profiles
            schema: custody
            code_column: profile_code
            id_column: profile_id
        - name: reference
          type: string
          required: false
          maps_to: ssi_reference
        - name: safekeeping-account
          type: string
          required: true
          maps_to: safekeeping_account
        - name: safekeeping-bic
          type: string
          required: true
          maps_to: safekeeping_bic
        - name: cash-account
          type: string
          required: false
          maps_to: cash_account
        - name: cash-bic
          type: string
          required: false
          maps_to: cash_account_bic
        - name: effective-date
          type: date
          required: true
          maps_to: effective_date
        - name: expiry-date
          type: date
          required: false
          maps_to: expiry_date
      returns:
        type: uuid
        name: ssi_id
        capture: true

    add-agent:
      description: "Add agent to SSI chain"
      behavior: crud
      crud:
        operation: insert
        table: ssi_agent_chain
        schema: custody
        returning: agent_link_id
      args:
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
        - name: role
          type: lookup
          required: true
          lookup:
            table: agent_roles
            schema: custody
            code_column: role_code
            id_column: role_id
        - name: agent-bic
          type: string
          required: true
          maps_to: agent_bic
        - name: agent-account
          type: string
          required: false
          maps_to: agent_account
        - name: agent-name
          type: string
          required: false
          maps_to: agent_name
        - name: sequence
          type: integer
          required: true
          maps_to: sequence_order
      returns:
        type: uuid
        name: agent_link_id
        capture: false

    activate:
      description: "Activate an SSI"
      behavior: crud
      crud:
        operation: update
        table: client_ssi
        schema: custody
        key: ssi_id
      args:
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [ACTIVE]
      returns:
        type: affected

    suspend:
      description: "Suspend an SSI"
      behavior: crud
      crud:
        operation: update
        table: client_ssi
        schema: custody
        key: ssi_id
      args:
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [SUSPENDED]
      returns:
        type: affected

    expire:
      description: "Expire an SSI"
      behavior: crud
      crud:
        operation: update
        table: client_ssi
        schema: custody
        key: ssi_id
      args:
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
        - name: expiry-date
          type: date
          required: true
          maps_to: expiry_date
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [EXPIRED]
      returns:
        type: affected

    list-by-cbu:
      description: "List SSIs for a CBU"
      behavior: crud
      crud:
        operation: list_by_fk
        table: client_ssi
        schema: custody
        fk_col: cbu_id
      args:
        - name: cbu-id
          type: uuid
          required: true
        - name: status
          type: string
          required: false
          maps_to: status
      returns:
        type: record_set

    list-by-profile:
      description: "List SSIs for a settlement profile"
      behavior: crud
      crud:
        operation: list_by_fk
        table: client_ssi
        schema: custody
        fk_col: profile_id
      args:
        - name: profile
          type: lookup
          required: true
          lookup:
            table: settlement_profiles
            schema: custody
            code_column: profile_code
            id_column: profile_id
      returns:
        type: record_set

    configure-instruction:
      description: "Configure instruction routing for this SSI"
      behavior: crud
      crud:
        operation: insert
        table: client_instruction_config
        schema: custody
        returning: config_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
        - name: path-id
          type: uuid
          required: true
          maps_to: path_id
        - name: service-instance-id
          type: uuid
          required: false
          maps_to: service_instance_id
        - name: auto-release
          type: boolean
          required: false
          maps_to: auto_release
          default: false
        - name: priority
          type: integer
          required: false
          maps_to: priority_override
        - name: hold-code
          type: string
          required: false
          maps_to: hold_code
      returns:
        type: uuid
        name: config_id
        capture: false

# =============================================================================
# DOMAIN: counterparty-ssi (Street-side SSIs)
# =============================================================================
counterparty-ssi:
  description: "Counterparty (street-side) settlement identity"
  
  verbs:
    create:
      description: "Create counterparty SSI from ALERT/CTM data"
      behavior: crud
      crud:
        operation: insert
        table: counterparty_ssi
        schema: custody
        returning: counterparty_ssi_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: profile
          type: lookup
          required: true
          lookup:
            table: settlement_profiles
            schema: custody
            code_column: profile_code
            id_column: profile_id
        - name: counterparty-bic
          type: string
          required: true
          maps_to: counterparty_bic
        - name: safekeeping-account
          type: string
          required: false
          maps_to: safekeeping_account
        - name: source
          type: string
          required: false
          maps_to: source
          valid_values: [ALERT, MANUAL, CTM]
          default: ALERT
        - name: alert-account-id
          type: string
          required: false
          maps_to: alert_account_id
        - name: effective-date
          type: date
          required: true
          maps_to: effective_date
      returns:
        type: uuid
        name: counterparty_ssi_id
        capture: true

    ensure:
      description: "Upsert counterparty SSI"
      behavior: crud
      crud:
        operation: upsert
        table: counterparty_ssi
        schema: custody
        conflict_keys: [entity_id, profile_id, counterparty_bic, effective_date]
        returning: counterparty_ssi_id
      args:
        # Same as create
      returns:
        type: uuid
        name: counterparty_ssi_id
        capture: true

    list-by-entity:
      description: "List SSIs for a counterparty entity"
      behavior: crud
      crud:
        operation: list_by_fk
        table: counterparty_ssi
        schema: custody
        fk_col: entity_id
      args:
        - name: entity-id
          type: uuid
          required: true
      returns:
        type: record_set
```

### 3.2 Plugin Operations (for complex logic)

```yaml
# Add to plugins section of verbs.yaml

plugins:
  ssi.validate:
    description: "Validate SSI completeness for settlement profile"
    handler: ssi_validate
    args:
      - name: ssi-id
        type: uuid
        required: true
      - name: check-agents
        type: boolean
        required: false
        default: true
      - name: check-dates
        type: boolean
        required: false
        default: true
    returns:
      type: record
      # Returns: { valid: bool, errors: [], warnings: [] }

  ssi.clone:
    description: "Clone SSI with new effective date (for amendments)"
    handler: ssi_clone
    args:
      - name: source-ssi-id
        type: uuid
        required: true
      - name: effective-date
        type: date
        required: true
      - name: expire-source
        type: boolean
        required: false
        default: true
    returns:
      type: uuid
      name: new_ssi_id

  settlement-profile.derive-path:
    description: "Derive instruction path for a given scenario"
    handler: profile_derive_path
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: instrument-class
        type: lookup
        required: true
        lookup:
          table: instrument_classes
          schema: custody
          code_column: class_code
          id_column: class_id
      - name: market
        type: lookup
        required: true
        lookup:
          table: markets
          schema: custody
          code_column: mic
          id_column: market_id
      - name: direction
        type: string
        required: true
        valid_values: [RECEIVE, DELIVER]
      - name: payment-type
        type: string
        required: true
        valid_values: [DVP, FOP]
    returns:
      type: record
      # Returns: { ssi_id, path_id, resource_code, instruction_type }
```

---

## 4. Implementation Tasks

### Phase 1: Database Schema (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 4.1.1 | Create `custody` schema | S |
| 4.1.2 | Create reference data tables (markets, instrument_classes, instruction_types, agent_roles) | M |
| 4.1.3 | Create settlement_profiles and instruction_paths tables | M |
| 4.1.4 | Create client_ssi and ssi_agent_chain tables | M |
| 4.1.5 | Create counterparty_ssi table | S |
| 4.1.6 | Create client_instruction_config table | S |
| 4.1.7 | Create FK constraints to ob-poc schema (cbus, entities, service_resource_types) | S |
| 4.1.8 | Create indexes for common query patterns | S |
| 4.1.9 | Insert seed data (markets, instrument classes, instruction types, agent roles) | M |

### Phase 2: DSL Configuration (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 4.2.1 | Add `market` domain to verbs.yaml | S |
| 4.2.2 | Add `instrument-class` domain to verbs.yaml | S |
| 4.2.3 | Add `settlement-profile` domain to verbs.yaml | M |
| 4.2.4 | Add `ssi` domain to verbs.yaml | L |
| 4.2.5 | Add `counterparty-ssi` domain to verbs.yaml | M |
| 4.2.6 | Add plugin definitions for ssi.validate, ssi.clone, settlement-profile.derive-path | S |
| 4.2.7 | Update lookup table references for cross-schema lookups (custody ↔ ob-poc) | M |

### Phase 3: Plugin Handlers (Priority: MEDIUM)

| Task | Description | Effort |
|------|-------------|--------|
| 4.3.1 | Implement `ssi_validate` handler | M |
| 4.3.2 | Implement `ssi_clone` handler | M |
| 4.3.3 | Implement `profile_derive_path` handler | L |
| 4.3.4 | Add custody plugin module to dsl_v2/custom_ops | S |

### Phase 4: Testing & Validation (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 4.4.1 | Create test DSL scripts for reference data setup | M |
| 4.4.2 | Create test DSL scripts for SSI onboarding scenarios | L |
| 4.4.3 | Create integration tests for cross-schema operations | M |
| 4.4.4 | Validate instruction path derivation logic | M |

---

## 5. Example DSL Scripts

### 5.1 Reference Data Setup (run once)

```clojure
;; =============================================================================
;; MARKET REFERENCE DATA
;; =============================================================================
(market.ensure :mic "XNYS" :name "New York Stock Exchange" 
               :country "US" :csd-bic "DTCYUS33" :currency "USD" 
               :timezone "America/New_York" :cut-off "16:00" :as @mkt-nyse)

(market.ensure :mic "XNAS" :name "NASDAQ" 
               :country "US" :csd-bic "DTCYUS33" :currency "USD" 
               :timezone "America/New_York" :as @mkt-nasdaq)

(market.ensure :mic "XLON" :name "London Stock Exchange" 
               :country "GB" :csd-bic "CABOROCP" :currency "GBP" 
               :timezone "Europe/London" :as @mkt-lse)

;; =============================================================================
;; INSTRUMENT CLASSES
;; =============================================================================
(instrument-class.ensure :code "EQUITY" :name "Equities" 
                         :settlement-cycle "T+1" :swift-family "MT54x" :as @cls-equity)

(instrument-class.ensure :code "FIXED_INCOME" :name "Fixed Income" 
                         :settlement-cycle "T+1" :swift-family "MT54x" :as @cls-fi)

(instrument-class.ensure :code "FUND_ETF" :name "Exchange Traded Funds" 
                         :settlement-cycle "T+1" :swift-family "MT54x" :as @cls-etf)

;; =============================================================================
;; SETTLEMENT PROFILES (Instrument Matrix)
;; =============================================================================
(settlement-profile.ensure 
  :code "EQUITY_XNYS_DVP" 
  :instrument-class "EQUITY" 
  :market "XNYS"
  :settlement-type "DVP" 
  :settlement-cycle "T+1"
  :matching-required true
  :as @profile-us-eq-dvp)

(settlement-profile.ensure 
  :code "EQUITY_XLON_DVP" 
  :instrument-class "EQUITY" 
  :market "XLON"
  :settlement-type "DVP" 
  :settlement-cycle "T+1"
  :matching-required true
  :as @profile-uk-eq-dvp)

;; =============================================================================
;; INSTRUCTION PATHS (Link profiles to service resources)
;; =============================================================================
;; Assumes service resource "SWIFT_GATEWAY" exists in ob-poc.service_resource_types

(settlement-profile.add-instruction-path
  :profile-id @profile-us-eq-dvp
  :instruction-type "RECEIVE_DVP"
  :resource "SWIFT_GATEWAY"
  :priority 1
  :enrichment-sources ["ALERT", "CLIENT_SSI"])

(settlement-profile.add-instruction-path
  :profile-id @profile-us-eq-dvp
  :instruction-type "DELIVER_DVP"
  :resource "SWIFT_GATEWAY"
  :priority 1
  :enrichment-sources ["ALERT", "CLIENT_SSI"])
```

### 5.2 Client Onboarding (per CBU)

```clojure
;; =============================================================================
;; CLIENT: Acme Fund LP - US Equity Custody Onboarding
;; =============================================================================

;; Assume CBU already exists from KYC onboarding
;; (cbu.read :name "Acme Fund LP" :as @cbu)

;; Create SSI for US Equities DVP settlement
(ssi.create
  :cbu-id @cbu
  :profile "EQUITY_XNYS_DVP"
  :reference "ACME-US-EQ-001"
  :safekeeping-account "12345-CUSTODY"
  :safekeeping-bic "BABOROCP"
  :cash-account "12345-CASH-USD"
  :cash-bic "BABOROCP"
  :effective-date "2024-12-01"
  :as @ssi-us-eq)

;; Add agent chain
(ssi.add-agent :ssi-id @ssi-us-eq :role "PSET" :agent-bic "DTCYUS33" :sequence 1)
(ssi.add-agent :ssi-id @ssi-us-eq :role "REAG" :agent-bic "BOFAUS3N" 
               :agent-account "88776655" :agent-name "Bank of America" :sequence 2)

;; Validate SSI completeness
(ssi.validate :ssi-id @ssi-us-eq :check-agents true :check-dates true)

;; Activate SSI
(ssi.activate :ssi-id @ssi-us-eq :status "ACTIVE")

;; Configure instruction routing to specific gateway instance
(ssi.configure-instruction
  :cbu-id @cbu
  :ssi-id @ssi-us-eq
  :path-id @path-receive-dvp  ;; From instruction path setup
  :service-instance-id @swift-gw-instance  ;; Provisioned service resource
  :auto-release false
  :priority 1)
```

### 5.3 Counterparty SSI Setup (from ALERT feed)

```clojure
;; =============================================================================
;; COUNTERPARTY: Morgan Stanley - Settlement Identity
;; =============================================================================

;; Assume entity exists from entity creation
;; (entity.read :name "Morgan Stanley" :as @ms-entity)

(counterparty-ssi.ensure
  :entity-id @ms-entity
  :profile "EQUITY_XNYS_DVP"
  :counterparty-bic "MSNYUS33"
  :safekeeping-account "MS-CUSTODY-001"
  :source "ALERT"
  :alert-account-id "ALERT-MS-12345"
  :effective-date "2024-01-01")

(counterparty-ssi.ensure
  :entity-id @ms-entity
  :profile "EQUITY_XLON_DVP"
  :counterparty-bic "MABOROCP"
  :safekeeping-account "MS-CUSTODY-UK-001"
  :source "ALERT"
  :alert-account-id "ALERT-MS-UK-001"
  :effective-date "2024-01-01")
```

---

## 6. Service Resource Integration

### 6.1 Existing Service Resource Pattern

The custody DSL **configures** service resources, it does not replace them. The flow is:

```
DSL Onboarding                    Service Resource (SWIFT Gateway)
──────────────                    ──────────────────────────────
1. Define SSI                 →   Knows client's accounts
2. Define Instruction Path    →   Knows message type to use
3. Bind to Resource Instance  →   Knows which gateway to route
                                  
When instruction arrives:
4. Derive path from profile   →   Look up client config
5. Resource generates SWIFT   →   Using SSI data + incoming instruction
```

### 6.2 Required Service Resource Types

Ensure these exist in `ob-poc.service_resource_types`:

| resource_code | name | owner | description |
|---------------|------|-------|-------------|
| SWIFT_GATEWAY | SWIFT Messaging Gateway | CUSTODY_OPS | Handles MT54x generation and transmission |
| SETTLEMENT_ENGINE | Settlement Processing Engine | CUSTODY_OPS | Matches and settles instructions |
| ALERT_CONNECTOR | DTCC ALERT Connector | CUSTODY_OPS | Syncs SSI data from ALERT |
| CTM_CONNECTOR | DTCC CTM Connector | CUSTODY_OPS | Trade matching integration |

### 6.3 Resource Attribute Requirements

Link custody attributes to service resources:

```clojure
;; SWIFT Gateway needs these attributes from SSI
(service-resource.add-attribute
  :resource "SWIFT_GATEWAY"
  :attribute "safekeeping_account"
  :is-mandatory true
  :resource-field-name "SAFE_ACCT")

(service-resource.add-attribute
  :resource "SWIFT_GATEWAY"
  :attribute "safekeeping_bic"
  :is-mandatory true
  :resource-field-name "SAFE_BIC")

(service-resource.add-attribute
  :resource "SWIFT_GATEWAY"
  :attribute "pset_bic"
  :is-mandatory true
  :resource-field-name "PSET")
```

---

## 7. Validation Rules

### 7.1 SSI Validation Rules

| Rule | Condition | Severity |
|------|-----------|----------|
| SSI_001 | SSI must have at least PSET agent | ERROR |
| SSI_002 | DVP settlement requires cash account | ERROR |
| SSI_003 | Agent BICs must be valid SWIFT format | ERROR |
| SSI_004 | Effective date must not be in past for new SSI | WARNING |
| SSI_005 | SSI should have REAG/DEAG for cross-border | WARNING |

### 7.2 Profile Validation Rules

| Rule | Condition | Severity |
|------|-----------|----------|
| PROF_001 | Profile must have at least one instruction path | ERROR |
| PROF_002 | Instruction path must reference active resource | ERROR |
| PROF_003 | DVP profile should have both RECEIVE and DELIVER paths | WARNING |

---

## 8. Migration Considerations

### 8.1 Existing Data

If there's existing settlement data in another format:
1. Create migration script to map to new schema
2. Run in parallel during transition
3. Validate SSI completeness before cutover

### 8.2 Schema Versioning

- Add `schema_version` to custody schema
- Support forward/backward compatible changes
- DSL version should track schema version

---

## 9. Open Questions for Review

1. **Cross-schema references**: Should `custody` schema tables use FK to `ob-poc` schema, or soft references via code lookups?

2. **Historical SSIs**: When SSI is amended, do we version (clone with new effective date) or update in place?

3. **Multi-currency SSIs**: Can one SSI handle multiple currencies, or is it one SSI per currency?

4. **Agent chain complexity**: Is the linear sequence sufficient, or do we need tree structures for complex sub-custodian chains?

5. **Profile inheritance**: Should profiles inherit defaults from instrument class, or be fully explicit?

---

## 10. Appendix: SWIFT Message Mapping Reference

### MT540 - Receive Free (FOP)

| SWIFT Field | Source |
|-------------|--------|
| :16R:GENL | Static |
| :20C::SEME | Instruction reference |
| :23G: | NEWM |
| :98C::PREP | Instruction timestamp |
| :16R:SETDET | |
| :22F::SETR | TRAD |
| :16R:SETPRTY | |
| :95P::PSET | SSI → pset_bic |
| :95P::REAG | SSI Agent Chain |
| :97A::SAFE | SSI → safekeeping_account |

### MT541 - Receive Against Payment (DVP)

Same as MT540, plus:

| SWIFT Field | Source |
|-------------|--------|
| :16R:AMT | |
| :19A::SETT | Settlement amount |
| :97A::CASH | SSI → cash_account |

---

*Document Version: 1.0*
*Created: 2024-12-01*
*Author: Claude (Implementation Planning)*
*For: Claude Code Execution*
