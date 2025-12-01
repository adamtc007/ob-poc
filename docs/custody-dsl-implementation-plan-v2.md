# Custody & Settlement DSL Implementation Plan v2

## Executive Summary

This document defines the implementation plan for extending the ob-poc DSL to support custody bank onboarding for settlement instruction management. The goal is to capture the **"what"** of custody onboarding (instruments, SSIs, instruction paths) that configures existing service resources (SWIFT gateway, routing) to handle the **"how"**.

**Perspective**: Bank/Custodian side, receiving instructions from investment managers ("the street").

**Key Model Insight**: CBU (Client Business Unit) is the umbrella. All entities (counterparties, investment managers, etc.) are linked to a CBU via ROLES. All SSIs are CBU-scoped instances that reference bank-wide settlement profiles.

**Scope**: 
- Standing Settlement Instructions (SSI) - CBU-scoped
- Instrument Matrix / Settlement Profiles - with Currency as core dimension
- Sub-custodian Network Matrix - bank's agent network
- Instruction routing configuration
- Foundation for FX Settlement Instructions extension

**Out of Scope**:
- SWIFT message generation (handled by service resources)
- Trade matching/affirmation logic (CTM-side)
- ISDA/CSA collateral management (phase 2)

---

## 1. Data Model Architecture

### 1.1 Conceptual Model: CBU as Umbrella

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CBU (Asset Owner)                               │
│                         "Acme Pension Fund LP"                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ENTITIES WITH ROLES (existing cbu_entity_roles pattern)                    │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │ Acme GP LLC     │  │ BlackRock IM    │  │ Morgan Stanley  │              │
│  │ Role: GP        │  │ Role: INV_MGR   │  │ Role: CPTY      │              │
│  │ Role: AUTH_SIG  │  │ Role: CPTY      │  │ Role: EXEC_BKR  │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
│                                                                              │
│  CBU SETTLEMENT INSTRUCTIONS (SSIs) - instances linked to profiles          │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ SSI: US Equities USD DVP                                            │    │
│  │   → Profile: EQUITY/XNYS/USD/DVP                                    │    │
│  │   → Safekeeping: 12345-SAFE @ BABOROCP                              │    │
│  │   → Cash: 12345-USD @ BABOROCP                                      │    │
│  ├─────────────────────────────────────────────────────────────────────┤    │
│  │ SSI: UK Equities GBP DVP                                            │    │
│  │   → Profile: EQUITY/XLON/GBP/DVP                                    │    │
│  │   → Safekeeping: UK-SAFE-001 @ MIDLBORC (sub-cust)                  │    │
│  │   → Cash: UK-CASH-001 @ MIDLBORC                                    │    │
│  ├─────────────────────────────────────────────────────────────────────┤    │
│  │ SSI: FX USD/EUR (future extension)                                  │    │
│  │   → Profile: FX/SPOT/USD-EUR                                        │    │
│  │   → Nostro: USD-NOSTRO @ CHASUS33                                   │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  TRADED/HELD UNIVERSE (which profiles this CBU needs)                       │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ Instrument Classes: EQUITY, FIXED_INCOME, ETF                       │    │
│  │ Markets: XNYS, XNAS, XLON, XETR                                     │    │
│  │ Currencies: USD, GBP, EUR                                           │    │
│  │ Settlement Types: DVP (primary), FOP (corporate actions)            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                    BANK REFERENCE DATA (not per-CBU)                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  MARKETS                         INSTRUMENT CLASSES                          │
│  ┌──────────────────────┐        ┌──────────────────────┐                   │
│  │ XNYS - NYSE          │        │ EQUITY - T+1         │                   │
│  │ XLON - LSE           │        │ FIXED_INCOME - T+1   │                   │
│  │ XETR - Xetra         │        │ FUND_ETF - T+1       │                   │
│  │ CSD, timezone, etc.  │        │ MONEY_MARKET - T+0   │                   │
│  └──────────────────────┘        └──────────────────────┘                   │
│                                                                              │
│  SETTLEMENT PROFILES (InstrumentClass × Market × Currency × SettlementType) │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ EQUITY/XNYS/USD/DVP → T+1, matching required, MT541/543              │   │
│  │ EQUITY/XLON/GBP/DVP → T+1, matching required, MT541/543              │   │
│  │ EQUITY/XLON/USD/DVP → T+1, cross-ccy, MT541/543                      │   │
│  │ FIXED_INCOME/XNYS/USD/DVP → T+1, MT541/543                           │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  SUB-CUSTODIAN MATRIX (Bank's global network - Omgeo Institution Network)   │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Market    │ Currency │ Sub-custodian     │ Local Agent    │ CSD      │   │
│  │───────────┼──────────┼───────────────────┼────────────────┼──────────│   │
│  │ XNYS      │ USD      │ BABOROCP (self)   │ BOFAUS3N       │ DTCYUS33 │   │
│  │ XLON      │ GBP      │ MIDLBORC          │ MIDLBORC       │ CABOROCP │   │
│  │ XLON      │ USD      │ MIDLBORC          │ CITIUS33       │ CABOROCP │   │
│  │ XETR      │ EUR      │ COBABOROCP        │ COBABOROCP     │ DAKVDEFF │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  INSTRUCTION PATHS (Profile → Service Resource routing)                      │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ EQUITY/XNYS/USD/DVP + RECEIVE → SWIFT_GATEWAY (MT541)                │   │
│  │ EQUITY/XNYS/USD/DVP + DELIVER → SWIFT_GATEWAY (MT543)                │   │
│  │ (enriched from sub-custodian matrix + client SSI)                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Omgeo/ALERT Model Mapping

| ALERT/Omgeo Concept | ob-poc Model | Notes |
|---------------------|--------------|-------|
| **Account** | `CBU` | The asset owner umbrella |
| **Account Party** | `Entity` with `Role` under CBU | Via existing `cbu_entity_roles` |
| **Account SSI** | `cbu_ssi` | Client's safekeeping/cash accounts per profile |
| **Institution** | Bank itself | We ARE the custodian |
| **Institution Network** | `subcustodian_network` | Bank's global sub-custodian matrix |
| **SSI Cross-Reference** | Entity with role=COUNTERPARTY | For matching incoming instructions |
| **Place of Settlement** | Derived from `subcustodian_network` | Per market/currency |

### 1.3 Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **CBU is the anchor** | All custody operations take `cbu-id`. SSIs, counterparties, traded universe - all scoped to CBU. |
| **Currency is core dimension** | Settlement Profile = Class × Market × Currency × Type. Enables FX SI extension and handles cross-currency settlement. |
| **Counterparty = Entity + Role** | Reuse existing `cbu_entity_roles`. A counterparty's settlement identity is their own SSI data (BIC, accounts) stored on the entity. |
| **Sub-custodian Matrix is bank-wide** | The bank's global network is reference data, not per-client. Client SSI references accounts AT sub-custodians. |
| **Traded Universe drives SSI requirements** | CBU declares what classes/markets/currencies they trade. System derives required SSIs. |

---

## 2. Database Schema Design

### 2.1 Reference Data Tables (schema: `custody`)

```sql
-- =============================================================================
-- MARKETS (enhanced with multi-currency support)
-- =============================================================================
CREATE TABLE custody.markets (
    market_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mic VARCHAR(4) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    country_code VARCHAR(2) NOT NULL,
    operating_mic VARCHAR(4),
    primary_currency VARCHAR(3) NOT NULL,
    supported_currencies VARCHAR(3)[] DEFAULT '{}',  -- Additional settlement ccys
    csd_bic VARCHAR(11),
    timezone VARCHAR(50) NOT NULL,
    cut_off_time TIME,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- CURRENCIES (for FX extension)
-- =============================================================================
CREATE TABLE custody.currencies (
    currency_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    iso_code VARCHAR(3) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    decimal_places INTEGER DEFAULT 2,
    is_cls_eligible BOOLEAN DEFAULT false,  -- CLS settlement eligible
    is_active BOOLEAN DEFAULT true
);

-- =============================================================================
-- INSTRUMENT CLASSES
-- =============================================================================
CREATE TABLE custody.instrument_classes (
    class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    class_code VARCHAR(20) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    asset_category VARCHAR(50),
    default_settlement_cycle VARCHAR(10) NOT NULL,
    swift_msg_family VARCHAR(10),
    requires_isin BOOLEAN DEFAULT true,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- SETTLEMENT PROFILES (Class × Market × Currency × Type)
-- =============================================================================
CREATE TABLE custody.settlement_profiles (
    profile_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_code VARCHAR(50) NOT NULL UNIQUE,
    class_id UUID NOT NULL REFERENCES custody.instrument_classes(class_id),
    market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    currency VARCHAR(3) NOT NULL,  -- Settlement currency
    settlement_type VARCHAR(10) NOT NULL,  -- DVP, FOP, RVP, DFP
    settlement_cycle VARCHAR(10) NOT NULL,
    is_cross_currency BOOLEAN DEFAULT false,  -- Trade ccy ≠ settlement ccy
    matching_required BOOLEAN DEFAULT true,
    partial_settlement_allowed BOOLEAN DEFAULT false,
    priority_default INTEGER DEFAULT 1,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(class_id, market_id, currency, settlement_type)
);

CREATE INDEX idx_settlement_profiles_lookup 
ON custody.settlement_profiles(class_id, market_id, currency);

-- =============================================================================
-- SUB-CUSTODIAN NETWORK (Bank's global agent network - Omgeo Institution Network)
-- =============================================================================
CREATE TABLE custody.subcustodian_network (
    network_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    currency VARCHAR(3) NOT NULL,
    subcustodian_bic VARCHAR(11) NOT NULL,
    subcustodian_name VARCHAR(255),
    local_agent_bic VARCHAR(11),
    local_agent_name VARCHAR(255),
    local_agent_account VARCHAR(35),
    csd_participant_id VARCHAR(35),  -- Bank's participant ID at the CSD
    place_of_settlement_bic VARCHAR(11) NOT NULL,  -- PSET for SWIFT
    is_primary BOOLEAN DEFAULT true,  -- Primary route for this market/ccy
    effective_date DATE NOT NULL,
    expiry_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(market_id, currency, subcustodian_bic, effective_date)
);

CREATE INDEX idx_subcustodian_network_lookup 
ON custody.subcustodian_network(market_id, currency, is_active);

-- =============================================================================
-- INSTRUCTION TYPES
-- =============================================================================
CREATE TABLE custody.instruction_types (
    type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type_code VARCHAR(30) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    direction VARCHAR(10) NOT NULL,  -- RECEIVE, DELIVER
    payment_type VARCHAR(10) NOT NULL,  -- DVP, FOP
    swift_mt_code VARCHAR(10),
    iso20022_msg_type VARCHAR(50),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- INSTRUCTION PATHS (Profile → Service Resource routing)
-- =============================================================================
CREATE TABLE custody.instruction_paths (
    path_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES custody.settlement_profiles(profile_id),
    instruction_type_id UUID NOT NULL REFERENCES custody.instruction_types(type_id),
    resource_id UUID NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id),
    routing_priority INTEGER DEFAULT 1,
    enrichment_sources JSONB DEFAULT '["SUBCUST_NETWORK", "CLIENT_SSI"]',
    validation_rules JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(profile_id, instruction_type_id, routing_priority)
);
```

### 2.2 CBU-Scoped Tables (schema: `custody`)

```sql
-- =============================================================================
-- CBU TRADED UNIVERSE (what this CBU trades/holds)
-- =============================================================================
CREATE TABLE custody.cbu_traded_universe (
    universe_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES custody.instrument_classes(class_id),
    market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    currency VARCHAR(3) NOT NULL,
    settlement_types VARCHAR(10)[] DEFAULT '{DVP}',  -- Which types they use
    is_active BOOLEAN DEFAULT true,
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, class_id, market_id, currency)
);

COMMENT ON TABLE custody.cbu_traded_universe IS 
'Defines which instrument classes, markets, and currencies a CBU trades. Used to derive required SSIs.';

-- =============================================================================
-- CBU SSI (Client's Standing Settlement Instructions)
-- =============================================================================
CREATE TABLE custody.cbu_ssi (
    ssi_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    profile_id UUID NOT NULL REFERENCES custody.settlement_profiles(profile_id),
    ssi_reference VARCHAR(50),  -- Client's internal reference
    
    -- Safekeeping (securities) account
    safekeeping_account VARCHAR(35) NOT NULL,
    safekeeping_bic VARCHAR(11) NOT NULL,  -- Usually sub-custodian from network
    safekeeping_account_name VARCHAR(100),
    
    -- Cash account (for DVP)
    cash_account VARCHAR(35),
    cash_account_bic VARCHAR(11),
    cash_account_currency VARCHAR(3),
    
    -- Lifecycle
    status VARCHAR(20) DEFAULT 'PENDING',  -- PENDING, ACTIVE, SUSPENDED, EXPIRED
    effective_date DATE NOT NULL,
    expiry_date DATE,
    
    -- Audit
    source VARCHAR(20) DEFAULT 'MANUAL',  -- MANUAL, ALERT_IMPORT, MIGRATION
    source_reference VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    created_by VARCHAR(100),
    
    UNIQUE(cbu_id, profile_id, safekeeping_account, effective_date)
);

CREATE INDEX idx_cbu_ssi_lookup ON custody.cbu_ssi(cbu_id, profile_id, status);
CREATE INDEX idx_cbu_ssi_active ON custody.cbu_ssi(cbu_id, status) WHERE status = 'ACTIVE';

-- =============================================================================
-- CBU SSI AGENT OVERRIDES (when client needs non-standard agent chain)
-- =============================================================================
CREATE TABLE custody.cbu_ssi_agent_override (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ssi_id UUID NOT NULL REFERENCES custody.cbu_ssi(ssi_id) ON DELETE CASCADE,
    agent_role VARCHAR(10) NOT NULL,  -- PSET, REAG, DEAG, etc.
    agent_bic VARCHAR(11) NOT NULL,
    agent_account VARCHAR(35),
    agent_name VARCHAR(100),
    sequence_order INTEGER NOT NULL,
    reason VARCHAR(255),  -- Why override standard network
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(ssi_id, agent_role, sequence_order)
);

COMMENT ON TABLE custody.cbu_ssi_agent_override IS 
'Client-specific agent chain overrides. Standard path uses subcustodian_network; this table captures exceptions.';

-- =============================================================================
-- CBU INSTRUCTION CONFIG (per-SSI routing configuration)
-- =============================================================================
CREATE TABLE custody.cbu_instruction_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    ssi_id UUID NOT NULL REFERENCES custody.cbu_ssi(ssi_id),
    path_id UUID NOT NULL REFERENCES custody.instruction_paths(path_id),
    service_instance_id UUID REFERENCES "ob-poc".cbu_service_resource_instances(instance_id),
    
    -- Behavioral config
    auto_release BOOLEAN DEFAULT false,
    default_priority INTEGER,
    hold_code VARCHAR(10),
    narrative_template TEXT,
    
    -- Matching config (for incoming instructions)
    counterparty_matching_strict BOOLEAN DEFAULT true,
    
    custom_config JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, ssi_id, path_id)
);

-- =============================================================================
-- ENTITY SETTLEMENT IDENTITY (Counterparty/IM settlement details)
-- Extends entities that have COUNTERPARTY or INV_MGR role
-- =============================================================================
CREATE TABLE custody.entity_settlement_identity (
    identity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Primary settlement identity (for matching)
    primary_bic VARCHAR(11) NOT NULL,
    lei VARCHAR(20),  -- Legal Entity Identifier
    
    -- ALERT/CTM integration
    alert_participant_id VARCHAR(50),
    ctm_participant_id VARCHAR(50),
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(entity_id, primary_bic)
);

-- =============================================================================
-- ENTITY SSI (Counterparty's settlement instructions per profile)
-- Used for matching and instruction enrichment
-- =============================================================================
CREATE TABLE custody.entity_ssi (
    entity_ssi_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    profile_id UUID NOT NULL REFERENCES custody.settlement_profiles(profile_id),
    
    -- Their settlement details
    counterparty_bic VARCHAR(11) NOT NULL,
    safekeeping_account VARCHAR(35),
    
    -- Source tracking
    source VARCHAR(20) DEFAULT 'ALERT',  -- ALERT, MANUAL, CTM
    source_reference VARCHAR(100),
    
    status VARCHAR(20) DEFAULT 'ACTIVE',
    effective_date DATE NOT NULL,
    expiry_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(entity_id, profile_id, counterparty_bic, effective_date)
);

COMMENT ON TABLE custody.entity_ssi IS 
'Settlement instructions for counterparties. Sourced from ALERT or manual input. Used for matching incoming instructions.';
```

### 2.3 Seed Data

```sql
-- =============================================================================
-- CURRENCIES
-- =============================================================================
INSERT INTO custody.currencies (iso_code, name, decimal_places, is_cls_eligible) VALUES
('USD', 'US Dollar', 2, true),
('EUR', 'Euro', 2, true),
('GBP', 'British Pound', 2, true),
('JPY', 'Japanese Yen', 0, true),
('CHF', 'Swiss Franc', 2, true),
('CAD', 'Canadian Dollar', 2, true),
('AUD', 'Australian Dollar', 2, true),
('HKD', 'Hong Kong Dollar', 2, true),
('SGD', 'Singapore Dollar', 2, true);

-- =============================================================================
-- MARKETS
-- =============================================================================
INSERT INTO custody.markets (mic, name, country_code, primary_currency, supported_currencies, csd_bic, timezone) VALUES
('XNYS', 'New York Stock Exchange', 'US', 'USD', '{}', 'DTCYUS33', 'America/New_York'),
('XNAS', 'NASDAQ', 'US', 'USD', '{}', 'DTCYUS33', 'America/New_York'),
('XLON', 'London Stock Exchange', 'GB', 'GBP', '{USD,EUR}', 'CABOROCP', 'Europe/London'),
('XPAR', 'Euronext Paris', 'FR', 'EUR', '{}', 'SICABOROCP', 'Europe/Paris'),
('XETR', 'Deutsche Börse Xetra', 'DE', 'EUR', '{}', 'DAKVDEFF', 'Europe/Berlin'),
('XTKS', 'Tokyo Stock Exchange', 'JP', 'JPY', '{}', 'JASDECTK', 'Asia/Tokyo'),
('XHKG', 'Hong Kong Stock Exchange', 'HK', 'HKD', '{USD}', 'CCABOROCP', 'Asia/Hong_Kong');

-- =============================================================================
-- INSTRUMENT CLASSES
-- =============================================================================
INSERT INTO custody.instrument_classes (class_code, name, default_settlement_cycle, swift_msg_family) VALUES
('EQUITY', 'Equities', 'T+1', 'MT54x'),
('FIXED_INCOME', 'Fixed Income', 'T+1', 'MT54x'),
('GOVT_BOND', 'Government Bonds', 'T+1', 'MT54x'),
('CORP_BOND', 'Corporate Bonds', 'T+2', 'MT54x'),
('FUND_ETF', 'Exchange Traded Funds', 'T+1', 'MT54x'),
('FUND_MUTUAL', 'Mutual Funds', 'T+2', 'MT50x'),
('MONEY_MARKET', 'Money Market', 'T+0', 'MT54x');

-- =============================================================================
-- INSTRUCTION TYPES
-- =============================================================================
INSERT INTO custody.instruction_types (type_code, name, direction, payment_type, swift_mt_code) VALUES
('RECEIVE_FOP', 'Receive Free of Payment', 'RECEIVE', 'FOP', 'MT540'),
('RECEIVE_DVP', 'Receive vs Payment', 'RECEIVE', 'DVP', 'MT541'),
('DELIVER_FOP', 'Deliver Free of Payment', 'DELIVER', 'FOP', 'MT542'),
('DELIVER_DVP', 'Deliver vs Payment', 'DELIVER', 'DVP', 'MT543');
```

---

## 3. DSL Domain Design

### 3.1 Domain Overview

| Domain | Scope | Primary Operations |
|--------|-------|-------------------|
| `market` | Bank reference | ensure, read, list |
| `instrument-class` | Bank reference | ensure, read, list |
| `settlement-profile` | Bank reference | ensure, add-instruction-path, list |
| `subcustodian` | Bank reference | ensure, list-by-market |
| `cbu-custody` | CBU-scoped | define-universe, add-ssi, configure-instruction, derive-required-ssis |
| `entity-settlement` | Entity extension | set-identity, add-ssi |

### 3.2 New Domains for verbs.yaml

```yaml
# =============================================================================
# DOMAIN: market (Reference Data)
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
        - name: currency
          type: string
          required: true
          maps_to: primary_currency
        - name: additional-currencies
          type: string_list
          required: false
          maps_to: supported_currencies
        - name: csd-bic
          type: string
          required: false
          maps_to: csd_bic
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
        - name: currency
          type: string
          required: false
          maps_to: primary_currency
        - name: is-active
          type: boolean
          required: false
          maps_to: is_active
      returns:
        type: record_set

# =============================================================================
# DOMAIN: instrument-class (Reference Data)
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

    list:
      description: "List instrument classes"
      behavior: crud
      crud:
        operation: select
        table: instrument_classes
        schema: custody
      args:
        - name: is-active
          type: boolean
          required: false
          maps_to: is_active
      returns:
        type: record_set

# =============================================================================
# DOMAIN: settlement-profile (Instrument Matrix)
# =============================================================================
settlement-profile:
  description: "Settlement profile (Instrument Matrix) operations"
  
  verbs:
    ensure:
      description: "Create or update settlement profile (Class × Market × Currency × Type)"
      behavior: crud
      crud:
        operation: upsert
        table: settlement_profiles
        schema: custody
        conflict_keys: [profile_code]
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
        - name: currency
          type: string
          required: true
          maps_to: currency
        - name: settlement-type
          type: string
          required: true
          maps_to: settlement_type
          valid_values: [DVP, FOP, RVP, DFP]
        - name: settlement-cycle
          type: string
          required: true
          maps_to: settlement_cycle
        - name: cross-currency
          type: boolean
          required: false
          maps_to: is_cross_currency
          default: false
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

    add-instruction-path:
      description: "Add instruction routing path to profile"
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
      returns:
        type: uuid
        name: path_id
        capture: true

    list:
      description: "List settlement profiles with filters"
      behavior: crud
      crud:
        operation: select
        table: settlement_profiles
        schema: custody
      args:
        - name: instrument-class
          type: lookup
          required: false
          lookup:
            table: instrument_classes
            schema: custody
            code_column: class_code
            id_column: class_id
        - name: market
          type: lookup
          required: false
          lookup:
            table: markets
            schema: custody
            code_column: mic
            id_column: market_id
        - name: currency
          type: string
          required: false
          maps_to: currency
        - name: settlement-type
          type: string
          required: false
          maps_to: settlement_type
      returns:
        type: record_set

# =============================================================================
# DOMAIN: subcustodian (Bank's Sub-custodian Network)
# =============================================================================
subcustodian:
  description: "Sub-custodian network (bank's global agent network)"
  
  verbs:
    ensure:
      description: "Create or update sub-custodian entry for market/currency"
      behavior: crud
      crud:
        operation: upsert
        table: subcustodian_network
        schema: custody
        conflict_keys: [market_id, currency, subcustodian_bic, effective_date]
        returning: network_id
      args:
        - name: market
          type: lookup
          required: true
          lookup:
            table: markets
            schema: custody
            code_column: mic
            id_column: market_id
        - name: currency
          type: string
          required: true
          maps_to: currency
        - name: subcustodian-bic
          type: string
          required: true
          maps_to: subcustodian_bic
        - name: subcustodian-name
          type: string
          required: false
          maps_to: subcustodian_name
        - name: local-agent-bic
          type: string
          required: false
          maps_to: local_agent_bic
        - name: local-agent-name
          type: string
          required: false
          maps_to: local_agent_name
        - name: local-agent-account
          type: string
          required: false
          maps_to: local_agent_account
        - name: csd-participant
          type: string
          required: false
          maps_to: csd_participant_id
        - name: pset
          type: string
          required: true
          maps_to: place_of_settlement_bic
        - name: is-primary
          type: boolean
          required: false
          maps_to: is_primary
          default: true
        - name: effective-date
          type: date
          required: true
          maps_to: effective_date
      returns:
        type: uuid
        name: network_id
        capture: true

    list-by-market:
      description: "List sub-custodian entries for a market"
      behavior: crud
      crud:
        operation: list_by_fk
        table: subcustodian_network
        schema: custody
        fk_col: market_id
      args:
        - name: market
          type: lookup
          required: true
          lookup:
            table: markets
            schema: custody
            code_column: mic
            id_column: market_id
        - name: currency
          type: string
          required: false
          maps_to: currency
        - name: is-active
          type: boolean
          required: false
          maps_to: is_active
      returns:
        type: record_set

    lookup:
      description: "Find sub-custodian for market/currency combination"
      behavior: plugin
      handler: subcustodian_lookup
      args:
        - name: market
          type: lookup
          required: true
          lookup:
            table: markets
            schema: custody
            code_column: mic
            id_column: market_id
        - name: currency
          type: string
          required: true
        - name: as-of-date
          type: date
          required: false
      returns:
        type: record

# =============================================================================
# DOMAIN: cbu-custody (CBU-scoped custody operations)
# This is the main domain for client onboarding
# =============================================================================
cbu-custody:
  description: "CBU-scoped custody and SSI operations"
  
  verbs:
    define-universe:
      description: "Define what instrument classes/markets/currencies a CBU trades"
      behavior: crud
      crud:
        operation: upsert
        table: cbu_traded_universe
        schema: custody
        conflict_keys: [cbu_id, class_id, market_id, currency]
        returning: universe_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
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
        - name: currency
          type: string
          required: true
          maps_to: currency
        - name: settlement-types
          type: string_list
          required: false
          maps_to: settlement_types
          default: ["DVP"]
        - name: effective-date
          type: date
          required: false
          maps_to: effective_date
      returns:
        type: uuid
        name: universe_id
        capture: false

    list-universe:
      description: "List CBU's traded universe"
      behavior: crud
      crud:
        operation: list_by_fk
        table: cbu_traded_universe
        schema: custody
        fk_col: cbu_id
      args:
        - name: cbu-id
          type: uuid
          required: true
        - name: is-active
          type: boolean
          required: false
          maps_to: is_active
      returns:
        type: record_set

    derive-required-ssis:
      description: "Derive which SSIs are needed based on traded universe"
      behavior: plugin
      handler: derive_required_ssis
      args:
        - name: cbu-id
          type: uuid
          required: true
        - name: include-existing
          type: boolean
          required: false
          default: true
      returns:
        type: record_set
        # Returns list of { profile_code, status: MISSING|ACTIVE|PENDING }

    add-ssi:
      description: "Add a Standing Settlement Instruction for this CBU"
      behavior: crud
      crud:
        operation: insert
        table: cbu_ssi
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
        - name: safekeeping-name
          type: string
          required: false
          maps_to: safekeeping_account_name
        - name: cash-account
          type: string
          required: false
          maps_to: cash_account
        - name: cash-bic
          type: string
          required: false
          maps_to: cash_account_bic
        - name: cash-currency
          type: string
          required: false
          maps_to: cash_account_currency
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

    add-ssi-agent-override:
      description: "Add client-specific agent override (when not using standard sub-custodian chain)"
      behavior: crud
      crud:
        operation: insert
        table: cbu_ssi_agent_override
        schema: custody
        returning: override_id
      args:
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
        - name: role
          type: string
          required: true
          maps_to: agent_role
          valid_values: [PSET, REAG, DEAG, BUYR, SELL, SAFE]
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
        - name: reason
          type: string
          required: false
          maps_to: reason
      returns:
        type: uuid
        name: override_id
        capture: false

    activate-ssi:
      description: "Activate an SSI"
      behavior: crud
      crud:
        operation: update
        table: cbu_ssi
        schema: custody
        key: ssi_id
      args:
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
      returns:
        type: affected

    suspend-ssi:
      description: "Suspend an SSI"
      behavior: crud
      crud:
        operation: update
        table: cbu_ssi
        schema: custody
        key: ssi_id
      args:
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
      returns:
        type: affected

    expire-ssi:
      description: "Expire an SSI"
      behavior: crud
      crud:
        operation: update
        table: cbu_ssi
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
      returns:
        type: affected

    list-ssis:
      description: "List SSIs for a CBU"
      behavior: crud
      crud:
        operation: list_by_fk
        table: cbu_ssi
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
        - name: profile
          type: lookup
          required: false
          lookup:
            table: settlement_profiles
            schema: custody
            code_column: profile_code
            id_column: profile_id
      returns:
        type: record_set

    configure-instruction:
      description: "Configure instruction routing for an SSI"
      behavior: crud
      crud:
        operation: upsert
        table: cbu_instruction_config
        schema: custody
        conflict_keys: [cbu_id, ssi_id, path_id]
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
        - name: service-instance
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
          maps_to: default_priority
        - name: hold-code
          type: string
          required: false
          maps_to: hold_code
        - name: strict-matching
          type: boolean
          required: false
          maps_to: counterparty_matching_strict
          default: true
      returns:
        type: uuid
        name: config_id
        capture: false

    validate-ssi:
      description: "Validate SSI completeness against profile requirements"
      behavior: plugin
      handler: validate_cbu_ssi
      args:
        - name: ssi-id
          type: uuid
          required: true
        - name: check-subcustodian
          type: boolean
          required: false
          default: true
        - name: check-cash-account
          type: boolean
          required: false
          default: true
      returns:
        type: record
        # Returns { valid: bool, errors: [], warnings: [] }

# =============================================================================
# DOMAIN: entity-settlement (Entity settlement identity extension)
# =============================================================================
entity-settlement:
  description: "Entity settlement identity operations (for counterparties, IMs)"
  
  verbs:
    set-identity:
      description: "Set primary settlement identity for an entity"
      behavior: crud
      crud:
        operation: upsert
        table: entity_settlement_identity
        schema: custody
        conflict_keys: [entity_id, primary_bic]
        returning: identity_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: bic
          type: string
          required: true
          maps_to: primary_bic
        - name: lei
          type: string
          required: false
          maps_to: lei
        - name: alert-id
          type: string
          required: false
          maps_to: alert_participant_id
        - name: ctm-id
          type: string
          required: false
          maps_to: ctm_participant_id
      returns:
        type: uuid
        name: identity_id
        capture: true

    add-ssi:
      description: "Add settlement instruction for a counterparty/IM entity"
      behavior: crud
      crud:
        operation: upsert
        table: entity_ssi
        schema: custody
        conflict_keys: [entity_id, profile_id, counterparty_bic, effective_date]
        returning: entity_ssi_id
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
        - name: source-reference
          type: string
          required: false
          maps_to: source_reference
        - name: effective-date
          type: date
          required: true
          maps_to: effective_date
      returns:
        type: uuid
        name: entity_ssi_id
        capture: true

    list-ssis:
      description: "List SSIs for an entity"
      behavior: crud
      crud:
        operation: list_by_fk
        table: entity_ssi
        schema: custody
        fk_col: entity_id
      args:
        - name: entity-id
          type: uuid
          required: true
        - name: status
          type: string
          required: false
          maps_to: status
      returns:
        type: record_set
```

---

## 4. Implementation Tasks

### Phase 1: Database Schema (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 4.1.1 | Create `custody` schema | S |
| 4.1.2 | Create `currencies` table | S |
| 4.1.3 | Create `markets` table with multi-currency support | S |
| 4.1.4 | Create `instrument_classes` table | S |
| 4.1.5 | Create `settlement_profiles` table (Class × Market × Currency × Type) | M |
| 4.1.6 | Create `subcustodian_network` table | M |
| 4.1.7 | Create `instruction_types` table | S |
| 4.1.8 | Create `instruction_paths` table with FK to ob-poc.service_resource_types | S |
| 4.1.9 | Create `cbu_traded_universe` table | S |
| 4.1.10 | Create `cbu_ssi` table | M |
| 4.1.11 | Create `cbu_ssi_agent_override` table | S |
| 4.1.12 | Create `cbu_instruction_config` table | S |
| 4.1.13 | Create `entity_settlement_identity` table | S |
| 4.1.14 | Create `entity_ssi` table | S |
| 4.1.15 | Create indexes for all lookup patterns | S |
| 4.1.16 | Insert seed data (currencies, markets, instrument classes, instruction types) | M |

### Phase 2: DSL Configuration (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 4.2.1 | Add `market` domain to verbs.yaml | S |
| 4.2.2 | Add `instrument-class` domain to verbs.yaml | S |
| 4.2.3 | Add `settlement-profile` domain to verbs.yaml | M |
| 4.2.4 | Add `subcustodian` domain to verbs.yaml | M |
| 4.2.5 | Add `cbu-custody` domain to verbs.yaml | L |
| 4.2.6 | Add `entity-settlement` domain to verbs.yaml | M |
| 4.2.7 | Configure cross-schema lookups (custody ↔ ob-poc) | M |
| 4.2.8 | Add plugin definitions | S |

### Phase 3: Plugin Handlers (Priority: MEDIUM)

| Task | Description | Effort |
|------|-------------|--------|
| 4.3.1 | Implement `subcustodian_lookup` handler | M |
| 4.3.2 | Implement `derive_required_ssis` handler | L |
| 4.3.3 | Implement `validate_cbu_ssi` handler | M |
| 4.3.4 | Add custody plugin module to dsl_v2/custom_ops | S |

### Phase 4: Testing (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 4.4.1 | Create reference data setup scripts | M |
| 4.4.2 | Create sub-custodian network setup scripts | M |
| 4.4.3 | Create CBU custody onboarding test scripts | L |
| 4.4.4 | Create counterparty SSI import test scripts | M |
| 4.4.5 | Validate instruction path derivation | M |

---

## 5. Example DSL Scripts

### 5.1 Bank Reference Data Setup

```clojure
;; =============================================================================
;; MARKETS (run once by ops)
;; =============================================================================
(market.ensure :mic "XNYS" :name "New York Stock Exchange" 
               :country "US" :currency "USD" :csd-bic "DTCYUS33"
               :timezone "America/New_York" :as @mkt-nyse)

(market.ensure :mic "XLON" :name "London Stock Exchange"
               :country "GB" :currency "GBP" :additional-currencies ["USD" "EUR"]
               :csd-bic "CABOROCP" :timezone "Europe/London" :as @mkt-lse)

;; =============================================================================
;; INSTRUMENT CLASSES
;; =============================================================================
(instrument-class.ensure :code "EQUITY" :name "Equities"
                         :settlement-cycle "T+1" :swift-family "MT54x")

(instrument-class.ensure :code "FIXED_INCOME" :name "Fixed Income"
                         :settlement-cycle "T+1" :swift-family "MT54x")

;; =============================================================================
;; SETTLEMENT PROFILES (Instrument Matrix entries)
;; =============================================================================
(settlement-profile.ensure
  :code "EQUITY/XNYS/USD/DVP"
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :settlement-cycle "T+1"
  :matching-required true
  :as @profile-us-eq-dvp)

(settlement-profile.ensure
  :code "EQUITY/XLON/GBP/DVP"
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :settlement-type "DVP"
  :settlement-cycle "T+1"
  :as @profile-uk-eq-dvp)

(settlement-profile.ensure
  :code "EQUITY/XLON/USD/DVP"
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "USD"
  :settlement-type "DVP"
  :settlement-cycle "T+1"
  :cross-currency true
  :as @profile-uk-eq-usd-dvp)

;; =============================================================================
;; INSTRUCTION PATHS (link profiles to SWIFT gateway resource)
;; =============================================================================
(settlement-profile.add-instruction-path
  :profile-id @profile-us-eq-dvp
  :instruction-type "RECEIVE_DVP"
  :resource "SWIFT_GATEWAY"
  :priority 1
  :enrichment-sources ["SUBCUST_NETWORK", "CLIENT_SSI"])

(settlement-profile.add-instruction-path
  :profile-id @profile-us-eq-dvp
  :instruction-type "DELIVER_DVP"
  :resource "SWIFT_GATEWAY"
  :priority 1)
```

### 5.2 Sub-custodian Network Setup

```clojure
;; =============================================================================
;; BANK'S SUB-CUSTODIAN NETWORK (Omgeo Institution Network equivalent)
;; =============================================================================

;; US Markets - we are our own sub-custodian
(subcustodian.ensure
  :market "XNYS"
  :currency "USD"
  :subcustodian-bic "BABOROCP"
  :subcustodian-name "Our Bank"
  :local-agent-bic "BOFAUS3N"
  :local-agent-name "Bank of America"
  :pset "DTCYUS33"
  :is-primary true
  :effective-date "2020-01-01")

;; UK Markets - using HSBC as sub-custodian for GBP
(subcustodian.ensure
  :market "XLON"
  :currency "GBP"
  :subcustodian-bic "MIDLGB22"
  :subcustodian-name "HSBC UK"
  :local-agent-bic "MIDLGB22"
  :pset "CABOROCP"
  :csd-participant "HSBC001"
  :is-primary true
  :effective-date "2020-01-01")

;; UK Markets - USD settlement uses Citi
(subcustodian.ensure
  :market "XLON"
  :currency "USD"
  :subcustodian-bic "CIABOROCP"
  :subcustodian-name "Citi UK"
  :local-agent-bic "CITIUS33"
  :local-agent-name "Citi NY"
  :pset "CABOROCP"
  :is-primary true
  :effective-date "2020-01-01")
```

### 5.3 CBU Custody Onboarding

```clojure
;; =============================================================================
;; CBU: Acme Pension Fund LP - Custody Onboarding
;; =============================================================================

;; Assume CBU already exists from KYC onboarding
;; (cbu.read :name "Acme Pension Fund LP" :as @cbu)

;; Step 1: Define what they trade
(cbu-custody.define-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-types ["DVP"])

(cbu-custody.define-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :settlement-types ["DVP"])

(cbu-custody.define-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "USD"
  :settlement-types ["DVP"])

;; Step 2: See what SSIs are needed
(cbu-custody.derive-required-ssis :cbu-id @cbu)
;; Returns:
;; [{ profile: "EQUITY/XNYS/USD/DVP", status: "MISSING" },
;;  { profile: "EQUITY/XLON/GBP/DVP", status: "MISSING" },
;;  { profile: "EQUITY/XLON/USD/DVP", status: "MISSING" }]

;; Step 3: Create SSIs
(cbu-custody.add-ssi
  :cbu-id @cbu
  :profile "EQUITY/XNYS/USD/DVP"
  :reference "ACME-US-EQ-001"
  :safekeeping-account "12345-SAFE"
  :safekeeping-bic "BABOROCP"
  :safekeeping-name "Acme Pension Safekeeping"
  :cash-account "12345-USD"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :effective-date "2024-12-01"
  :as @ssi-us)

(cbu-custody.add-ssi
  :cbu-id @cbu
  :profile "EQUITY/XLON/GBP/DVP"
  :reference "ACME-UK-EQ-001"
  :safekeeping-account "UK-SAFE-001"
  :safekeeping-bic "MIDLGB22"  ;; At our sub-custodian
  :cash-account "UK-CASH-GBP"
  :cash-bic "MIDLGB22"
  :cash-currency "GBP"
  :effective-date "2024-12-01"
  :as @ssi-uk)

;; Step 4: Validate SSIs
(cbu-custody.validate-ssi :ssi-id @ssi-us :check-subcustodian true)
(cbu-custody.validate-ssi :ssi-id @ssi-uk :check-subcustodian true)

;; Step 5: Activate SSIs
(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-uk)

;; Step 6: Configure instruction routing
(cbu-custody.configure-instruction
  :cbu-id @cbu
  :ssi-id @ssi-us
  :path-id @path-receive-dvp
  :service-instance @swift-gw-instance
  :auto-release false
  :strict-matching true)
```

### 5.4 Counterparty Setup (from ALERT)

```clojure
;; =============================================================================
;; Counterparty: Morgan Stanley - Settlement Identity
;; Assume entity exists and has COUNTERPARTY role under CBU
;; =============================================================================

;; (entity.read :name "Morgan Stanley" :as @ms)
;; (cbu.assign-role :cbu-id @cbu :entity-id @ms :role "COUNTERPARTY")

;; Set their primary settlement identity
(entity-settlement.set-identity
  :entity-id @ms
  :bic "MSNYUS33"
  :lei "IGJSJL3JD5P30I6NJZ34"
  :alert-id "ALERT-MS-001"
  :ctm-id "CTM-MS-001")

;; Add their SSIs per profile (sourced from ALERT)
(entity-settlement.add-ssi
  :entity-id @ms
  :profile "EQUITY/XNYS/USD/DVP"
  :counterparty-bic "MSNYUS33"
  :safekeeping-account "MS-CUSTODY-001"
  :source "ALERT"
  :source-reference "ALERT-MS-SSI-001"
  :effective-date "2024-01-01")

(entity-settlement.add-ssi
  :entity-id @ms
  :profile "EQUITY/XLON/GBP/DVP"
  :counterparty-bic "MABOROCP"
  :safekeeping-account "MS-UK-001"
  :source "ALERT"
  :source-reference "ALERT-MS-SSI-002"
  :effective-date "2024-01-01")
```

---

## 6. Instruction Flow (How It Works)

```
INCOMING INSTRUCTION FROM STREET
         │
         ▼
┌────────────────────────────────────────┐
│ 1. Parse Instruction                    │
│    - ISIN → Instrument Class           │
│    - MIC → Market                       │
│    - Currency                           │
│    - Direction (Buy/Sell)               │
│    - Counterparty BIC                   │
└────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────┐
│ 2. Derive Settlement Profile            │
│    Class × Market × Currency × Type     │
│    e.g., EQUITY/XNYS/USD/DVP            │
└────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────┐
│ 3. Lookup Client SSI                    │
│    cbu_ssi WHERE                        │
│      cbu_id = ? AND profile_id = ?      │
│      AND status = 'ACTIVE'              │
│    → Safekeeping account, Cash account  │
└────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────┐
│ 4. Lookup Sub-custodian                 │
│    subcustodian_network WHERE           │
│      market_id = ? AND currency = ?     │
│    → PSET, Agent BICs                   │
│    (unless client has agent override)   │
└────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────┐
│ 5. Lookup Instruction Path              │
│    instruction_paths WHERE              │
│      profile_id = ? AND                 │
│      instruction_type = RECEIVE_DVP     │
│    → service_resource_id                │
└────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────┐
│ 6. Route to Service Resource            │
│    SWIFT_GATEWAY instance               │
│    Enriched with:                       │
│    - Client safekeeping account         │
│    - PSET from sub-custodian            │
│    - Agent chain                        │
│    → Generates MT541                    │
└────────────────────────────────────────┘
```

---

## 7. Open Questions Resolved

| Question | Resolution |
|----------|------------|
| **Cross-schema FKs** | Yes, use FKs to `ob-poc` schema for cbu_id, entity_id, service_resource_types |
| **SSI versioning** | Use effective_date/expiry_date. New SSI with future effective_date; old SSI gets expiry_date set. |
| **Multi-currency** | Currency is core dimension of profile. One SSI per profile (per currency). |
| **Agent chain** | Standard chain from `subcustodian_network`. Client overrides via `cbu_ssi_agent_override`. |
| **Counterparty model** | Entity with ROLE under CBU. Settlement identity via `entity_settlement_identity` + `entity_ssi`. |

---

*Document Version: 2.0*
*Created: 2024-12-01*
*Updated: 2024-12-01*
*Author: Claude (Implementation Planning)*
*For: Claude Code Execution*
