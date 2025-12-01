# Custody & Settlement DSL Implementation Plan v3

## Executive Summary

This document defines the implementation plan for extending the ob-poc DSL to support custody bank onboarding for settlement instruction management. The design is **fully aligned with industry standards** (DTCC ALERT/Omgeo, ISO 10962 CFI, SMPG, ISDA taxonomy) to ensure no proprietary dialect issues.

**Key Architectural Changes (v2 → v3):**
1. **Three-Layer Model**: Universe → SSI Data → Booking Rules (ALERT-style)
2. **Industry Taxonomy Alignment**: CFI codes, SMPG/ALERT security types, ISDA OTC taxonomy
3. **Rule-Based SSI Routing**: Priority-based matching with wildcards (mirrors ALERT booking rules)

**Perspective**: Bank/Custodian side, receiving instructions from investment managers.

**Scope**: 
- Standing Settlement Instructions (SSI) - CBU-scoped
- Instrument Matrix with industry-standard classification
- ALERT-style Booking Rules for SSI routing
- Sub-custodian Network Matrix
- OTC derivatives support (ISDA taxonomy)
- Foundation for FX Settlement Instructions

**Out of Scope**:
- SWIFT message generation (handled by service resources)
- Trade matching/affirmation logic (CTM-side)

---

## 1. Industry Standards Alignment

### 1.1 Taxonomy Landscape

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    INSTRUMENT CLASSIFICATION STANDARDS                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ISO 10962 (CFI)                    SMPG/ALERT Codes         ISDA Taxonomy  │
│  ───────────────                    ──────────────           ─────────────  │
│  Global standard                    Operational codes        OTC Derivatives│
│  6-char code                        Settlement routing       Regulatory/UPI │
│  Issued with ISIN                   DTCC industry use        FpML aligned   │
│                                                                              │
│  Used for:                          Used for:                Used for:      │
│  - Incoming security ID             - Booking rule match     - OTC classify │
│  - Regulatory reporting             - SSI selection          - ISDA/CSA link│
│  - Instrument properties            - ALERT enrichment       - UPI reporting│
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 CFI Code Structure (ISO 10962:2021)

| Position | Meaning | Example Values |
|----------|---------|----------------|
| 1 | Category | E=Equity, D=Debt, C=CIV, S=Swaps, J=Spot, K=Forward |
| 2 | Group | ES=Common shares, DB=Bonds, SR=Rate swaps |
| 3-6 | Attributes | Varies by group (voting rights, guarantee, etc.) |

**Key CFI Categories:**
- **E** - Equities (ES=Common, EP=Preferred, ED=Depositary Receipts)
- **D** - Debt Instruments (DB=Bonds, DY=Money Market)
- **C** - Collective Investment Vehicles (CI=Standard Funds)
- **S** - Swaps (SR=Rate, SC=Credit, SE=Equity, SF=FX)
- **H** - Non-listed/Complex Options
- **J** - Spot (FX, Commodities)
- **K** - Forwards

### 1.3 SMPG/ALERT Security Types

These are the operational codes used by DTCC ALERT for SSI matching:

| Group | Codes | Description |
|-------|-------|-------------|
| **EQU** | EQU, ADR, ETF, GDR, PRS, RTS, UIT | Equities |
| **Corp FI** | COB, ABS, BKL, CMO, CON, CPN, MBS, NTE | Corporate Fixed Income |
| **Govt FI** | TRY, AGS, FNM, FRM, GNM, MNB, NSD | Government Fixed Income |
| **MM** | MMT, BAS, CER, COD, COM, REP | Money Market |
| **FX/CSH** | CSH, F/X, MRG, TIM | Foreign Exchange / Cash |
| **DERIV** | CDS, CFD, CMF, EFU, IRS, OTC, TRS | Derivatives |
| **Collateral** | FXC, EQC, CBC, TRC | Collateral-specific SSIs |

### 1.4 ISDA OTC Taxonomy

For OTC derivatives (regulatory reporting, agreement linking):

| Asset Class | Base Products | Sub Products |
|-------------|---------------|--------------|
| **InterestRate** | IRSwap, Swaption, Cap-Floor, FRA | FixedFloat, Basis, OIS, CrossCurrency |
| **Credit** | CreditDefaultSwap, TotalReturnSwap | SingleName, Index, Basket |
| **Equity** | EquitySwap, EquityOption | PriceReturn, TotalReturn, Variance |
| **ForeignExchange** | FXSpot, FXForward, FXSwap, FXOption | Deliverable, NDF |
| **Commodity** | CommoditySwap, CommodityOption | Fixed-Float, Basis |

---

## 2. Three-Layer Architecture

### 2.1 Conceptual Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 1: CBU INSTRUMENT UNIVERSE                                            │
│  "What does this CBU trade/hold?"                                            │
│  Declarative. Drives completeness checks.                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│  EQUITY → XNYS, XLON, XETR                                                  │
│  FIXED_INCOME → XNYS                                                        │
│  OTC_IRS → (with counterparties: MS, GS, JPM + ISDA agreements)            │
│  FX_SPOT → USD/EUR, USD/JPY                                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    │ "Do we have SSIs covering this universe?"
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 2: SSI DATA                                                           │
│  "What are our actual accounts?"                                             │
│  Pure account data. No routing logic embedded.                               │
├─────────────────────────────────────────────────────────────────────────────┤
│  SSI-001: "US Primary" - Safekeeping 12345 @ BABOROCP, Cash USD             │
│  SSI-002: "UK GBP" - Safekeeping UK-001 @ MIDLGB22, Cash GBP                │
│  SSI-003: "Collateral IM" - Segregated SEG-001 @ BABOROCP                   │
│  SSI-004: "MS Special" - Special account for Morgan Stanley trades          │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    │ "Which SSI for this trade?" (ALERT-style matching)
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 3: BOOKING RULES (SSI Routing)                                        │
│  "Given trade characteristics, which SSI?"                                   │
│  Rule-based with priority and wildcards. Mirrors DTCC ALERT.                │
├─────────────────────────────────────────────────────────────────────────────┤
│  Rule 1 (pri 10): EQUITY + XNYS + USD + DVP          → SSI-001              │
│  Rule 2 (pri 10): EQUITY + XLON + GBP + DVP          → SSI-002              │
│  Rule 3 (pri 10): EQUITY + XLON + USD + DVP          → SSI-001 (cross-ccy)  │
│  Rule 4 (pri 20): EQUITY + ANY  + USD + DVP          → SSI-001 (fallback)   │
│  Rule 5 (pri 5):  ANY    + ANY  + USD + CPTY=MS      → SSI-004 (override)   │
│  Rule 6 (pri 100): ANYY  + ANY  + USD + ANY          → SSI-001 (ultimate)   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 ALERT Booking Rules Alignment

| ALERT Concept | Our Implementation |
|---------------|-------------------|
| Security Type (EQTY, BOND, ANYY) | `instrument_class_id` or `security_type_id` (NULL = wildcard) |
| Country/PSET | `market_id` (NULL = any market) |
| Currency | `currency` (NULL = any currency) |
| Method (DVP, FOP) | `settlement_type` (NULL = any) |
| Counterparty override | `counterparty_entity_id` (NULL = any counterparty) |
| Priority/Specificity | Explicit `priority` column (lower = higher priority) |
| Model linking | SSI linked via `ssi_id` |

### 2.3 Rule Matching Algorithm

```
1. Collect all ACTIVE rules for CBU
2. Filter rules where trade matches all non-NULL criteria
3. Sort by priority ASC (lowest priority number = first match)
4. Return first matching rule's SSI
5. If no match, return error (incomplete SSI setup)
```

**Specificity Score** (computed for audit/debugging):
```sql
specificity_score = 
  (CASE WHEN instrument_class_id IS NOT NULL THEN 16 ELSE 0 END) +
  (CASE WHEN security_type_id IS NOT NULL THEN 8 ELSE 0 END) +
  (CASE WHEN market_id IS NOT NULL THEN 4 ELSE 0 END) +
  (CASE WHEN currency IS NOT NULL THEN 2 ELSE 0 END) +
  (CASE WHEN settlement_type IS NOT NULL THEN 1 ELSE 0 END) +
  (CASE WHEN counterparty_entity_id IS NOT NULL THEN 32 ELSE 0 END)
```

---

## 3. Database Schema Design

### 3.1 Taxonomy Reference Tables (schema: `custody`)

```sql
-- =============================================================================
-- INSTRUMENT CLASSES (Our canonical abstraction layer)
-- Maps to both CFI categories and SMPG groups
-- =============================================================================
CREATE TABLE custody.instrument_classes (
    class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Our canonical code
    code VARCHAR(20) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    
    -- Settlement characteristics
    default_settlement_cycle VARCHAR(10) NOT NULL,
    swift_message_family VARCHAR(10),
    requires_isda BOOLEAN DEFAULT false,
    requires_collateral BOOLEAN DEFAULT false,
    
    -- ISO 10962 CFI mapping
    cfi_category CHAR(1),          -- E, D, C, O, F, S, H, J, K, L, T, M
    cfi_group CHAR(2),             -- ES, DB, CI, SR, etc.
    
    -- SMPG/ALERT group mapping
    smpg_group VARCHAR(20),        -- 'EQU', 'Corp FI', 'Govt FI', 'MM', 'FX/CSH', 'DERIV'
    
    -- ISDA mapping (for OTC)
    isda_asset_class VARCHAR(30),  -- 'InterestRate', 'Credit', 'Equity', 'Commodity', 'ForeignExchange'
    
    -- Hierarchy (e.g., CORP_BOND under FIXED_INCOME)
    parent_class_id UUID REFERENCES custody.instrument_classes(class_id),
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE custody.instrument_classes IS 
'Canonical instrument classification. Maps to CFI, SMPG/ALERT, and ISDA taxonomies.';

-- =============================================================================
-- SECURITY TYPES (ALERT-compatible codes for granular routing)
-- =============================================================================
CREATE TABLE custody.security_types (
    security_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    class_id UUID NOT NULL REFERENCES custody.instrument_classes(class_id),
    
    -- ALERT/SMPG code (3-4 chars)
    code VARCHAR(4) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    
    -- CFI pattern for matching (with wildcards)
    cfi_pattern VARCHAR(6),        -- 'ES****', 'DBFN**', etc.
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE custody.security_types IS 
'SMPG/ALERT security type codes. Used for granular booking rule matching.';

-- =============================================================================
-- CFI CODE REGISTRY (Reference for incoming securities)
-- =============================================================================
CREATE TABLE custody.cfi_codes (
    cfi_code CHAR(6) PRIMARY KEY,
    
    -- Decoded components
    category CHAR(1) NOT NULL,
    category_name VARCHAR(50),
    group_code CHAR(2) NOT NULL,
    group_name VARCHAR(50),
    attribute_1 CHAR(1),
    attribute_2 CHAR(1),
    attribute_3 CHAR(1),
    attribute_4 CHAR(1),
    
    -- Map to our classification
    class_id UUID REFERENCES custody.instrument_classes(class_id),
    security_type_id UUID REFERENCES custody.security_types(security_type_id),
    
    created_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE custody.cfi_codes IS 
'ISO 10962 CFI code registry. Maps incoming security CFI to our classification.';

-- =============================================================================
-- ISDA PRODUCT TAXONOMY (For OTC derivatives)
-- =============================================================================
CREATE TABLE custody.isda_product_taxonomy (
    taxonomy_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- ISDA taxonomy hierarchy
    asset_class VARCHAR(30) NOT NULL,    -- 'InterestRate', 'Credit', etc.
    base_product VARCHAR(50),            -- 'IRSwap', 'Swaption', 'CDS'
    sub_product VARCHAR(50),             -- 'FixedFloat', 'Basis', 'OIS'
    
    -- Full taxonomy path as code
    taxonomy_code VARCHAR(100) NOT NULL UNIQUE,
    
    -- UPI template (for regulatory reporting)
    upi_template VARCHAR(50),
    
    -- Map to our classification
    class_id UUID REFERENCES custody.instrument_classes(class_id),
    
    -- Equivalent CFI pattern
    cfi_pattern VARCHAR(6),
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE custody.isda_product_taxonomy IS 
'ISDA OTC derivatives taxonomy. Used for regulatory reporting and ISDA/CSA linking.';
```


### 3.2 Core Reference Tables (schema: `custody`)

```sql
-- =============================================================================
-- CURRENCIES
-- =============================================================================
CREATE TABLE custody.currencies (
    currency_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    iso_code VARCHAR(3) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    decimal_places INTEGER DEFAULT 2,
    is_cls_eligible BOOLEAN DEFAULT false,
    is_active BOOLEAN DEFAULT true
);

-- =============================================================================
-- MARKETS
-- =============================================================================
CREATE TABLE custody.markets (
    market_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mic VARCHAR(4) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    country_code VARCHAR(2) NOT NULL,
    operating_mic VARCHAR(4),
    primary_currency VARCHAR(3) NOT NULL,
    supported_currencies VARCHAR(3)[] DEFAULT '{}',
    csd_bic VARCHAR(11),
    timezone VARCHAR(50) NOT NULL,
    cut_off_time TIME,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- INSTRUCTION TYPES
-- =============================================================================
CREATE TABLE custody.instruction_types (
    type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type_code VARCHAR(30) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    direction VARCHAR(10) NOT NULL,    -- RECEIVE, DELIVER
    payment_type VARCHAR(10) NOT NULL, -- DVP, FOP
    swift_mt_code VARCHAR(10),
    iso20022_msg_type VARCHAR(50),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- SUB-CUSTODIAN NETWORK (Bank's global agent network)
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
    csd_participant_id VARCHAR(35),
    place_of_settlement_bic VARCHAR(11) NOT NULL,  -- PSET
    is_primary BOOLEAN DEFAULT true,
    effective_date DATE NOT NULL,
    expiry_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(market_id, currency, subcustodian_bic, effective_date)
);
```

### 3.3 Three-Layer Tables (schema: `custody`)

```sql
-- =============================================================================
-- LAYER 1: CBU INSTRUMENT UNIVERSE
-- "What does this CBU trade/hold?"
-- =============================================================================
CREATE TABLE custody.cbu_instrument_universe (
    universe_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    
    -- What they trade
    instrument_class_id UUID NOT NULL REFERENCES custody.instrument_classes(class_id),
    
    -- For cash securities: market matters
    market_id UUID REFERENCES custody.markets(market_id),
    currencies VARCHAR(3)[] NOT NULL DEFAULT '{}',
    settlement_types VARCHAR(10)[] DEFAULT '{DVP}',
    
    -- For OTC derivatives: counterparty/agreement matters
    counterparty_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    
    is_held BOOLEAN DEFAULT true,
    is_traded BOOLEAN DEFAULT true,
    is_active BOOLEAN DEFAULT true,
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(cbu_id, instrument_class_id, market_id, counterparty_entity_id)
);

COMMENT ON TABLE custody.cbu_instrument_universe IS 
'Layer 1: Declares what instrument classes, markets, currencies a CBU trades. Drives SSI completeness checks.';

-- =============================================================================
-- LAYER 2: CBU SSI DATA (Pure account data - no routing logic)
-- =============================================================================
CREATE TABLE custody.cbu_ssi (
    ssi_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    
    -- Human-readable identifier
    ssi_name VARCHAR(100) NOT NULL,
    ssi_type VARCHAR(20) NOT NULL,  -- SECURITIES, CASH, COLLATERAL, FX_NOSTRO
    
    -- Securities/Safekeeping account
    safekeeping_account VARCHAR(35),
    safekeeping_bic VARCHAR(11),
    safekeeping_account_name VARCHAR(100),
    
    -- Cash account (for DVP, payments)
    cash_account VARCHAR(35),
    cash_account_bic VARCHAR(11),
    cash_currency VARCHAR(3),
    
    -- Collateral account (for IM segregation)
    collateral_account VARCHAR(35),
    collateral_account_bic VARCHAR(11),
    
    -- Agent chain (standard from subcustodian_network unless overridden)
    pset_bic VARCHAR(11),
    receiving_agent_bic VARCHAR(11),
    delivering_agent_bic VARCHAR(11),
    
    -- Lifecycle
    status VARCHAR(20) DEFAULT 'PENDING',  -- PENDING, ACTIVE, SUSPENDED, EXPIRED
    effective_date DATE NOT NULL,
    expiry_date DATE,
    
    -- Audit
    source VARCHAR(20) DEFAULT 'MANUAL',
    source_reference VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    created_by VARCHAR(100)
);

CREATE INDEX idx_cbu_ssi_lookup ON custody.cbu_ssi(cbu_id, status);
CREATE INDEX idx_cbu_ssi_active ON custody.cbu_ssi(cbu_id, status) WHERE status = 'ACTIVE';

COMMENT ON TABLE custody.cbu_ssi IS 
'Layer 2: Pure SSI account data. No routing logic - just the accounts themselves.';

-- =============================================================================
-- LAYER 3: SSI BOOKING RULES (ALERT-style routing)
-- =============================================================================
CREATE TABLE custody.ssi_booking_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    ssi_id UUID NOT NULL REFERENCES custody.cbu_ssi(ssi_id) ON DELETE CASCADE,
    
    -- Human-readable rule name
    rule_name VARCHAR(100) NOT NULL,
    
    -- Priority (lower = higher priority, matched first)
    priority INTEGER NOT NULL DEFAULT 50,
    
    -- Match criteria (NULL = wildcard / "ANYY")
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    security_type_id UUID REFERENCES custody.security_types(security_type_id),
    market_id UUID REFERENCES custody.markets(market_id),
    currency VARCHAR(3),
    settlement_type VARCHAR(10),  -- DVP, FOP, NULL = any
    counterparty_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    
    -- For OTC - match on ISDA taxonomy
    isda_asset_class VARCHAR(30),
    isda_base_product VARCHAR(50),
    
    -- Computed specificity (for debugging/audit)
    specificity_score INTEGER GENERATED ALWAYS AS (
        (CASE WHEN counterparty_entity_id IS NOT NULL THEN 32 ELSE 0 END) +
        (CASE WHEN instrument_class_id IS NOT NULL THEN 16 ELSE 0 END) +
        (CASE WHEN security_type_id IS NOT NULL THEN 8 ELSE 0 END) +
        (CASE WHEN market_id IS NOT NULL THEN 4 ELSE 0 END) +
        (CASE WHEN currency IS NOT NULL THEN 2 ELSE 0 END) +
        (CASE WHEN settlement_type IS NOT NULL THEN 1 ELSE 0 END)
    ) STORED,
    
    is_active BOOLEAN DEFAULT true,
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    expiry_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    -- Unique constraint on priority per CBU
    UNIQUE(cbu_id, priority, rule_name)
);

CREATE INDEX idx_booking_rules_lookup ON custody.ssi_booking_rules (
    cbu_id, is_active, priority,
    instrument_class_id, security_type_id, market_id, currency
);

COMMENT ON TABLE custody.ssi_booking_rules IS 
'Layer 3: ALERT-style booking rules. Priority-based matching with wildcards (NULL = any).';
```

### 3.4 Supporting Tables

```sql
-- =============================================================================
-- CBU SSI AGENT OVERRIDES (non-standard agent chain)
-- =============================================================================
CREATE TABLE custody.cbu_ssi_agent_override (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ssi_id UUID NOT NULL REFERENCES custody.cbu_ssi(ssi_id) ON DELETE CASCADE,
    agent_role VARCHAR(10) NOT NULL,  -- PSET, REAG, DEAG, BUYR, SELL
    agent_bic VARCHAR(11) NOT NULL,
    agent_account VARCHAR(35),
    agent_name VARCHAR(100),
    sequence_order INTEGER NOT NULL,
    reason VARCHAR(255),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(ssi_id, agent_role, sequence_order)
);

-- =============================================================================
-- INSTRUCTION PATHS (Profile → Service Resource routing)
-- =============================================================================
CREATE TABLE custody.instruction_paths (
    path_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Match criteria (similar to booking rules)
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    currency VARCHAR(3),
    instruction_type_id UUID NOT NULL REFERENCES custody.instruction_types(type_id),
    
    -- Route to service resource
    resource_id UUID NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id),
    routing_priority INTEGER DEFAULT 1,
    enrichment_sources JSONB DEFAULT '["SUBCUST_NETWORK", "CLIENT_SSI"]',
    validation_rules JSONB,
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- ENTITY SETTLEMENT IDENTITY (Counterparty settlement details)
-- =============================================================================
CREATE TABLE custody.entity_settlement_identity (
    identity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    primary_bic VARCHAR(11) NOT NULL,
    lei VARCHAR(20),
    alert_participant_id VARCHAR(50),
    ctm_participant_id VARCHAR(50),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(entity_id, primary_bic)
);

-- =============================================================================
-- ENTITY SSI (Counterparty's SSIs - sourced from ALERT)
-- =============================================================================
CREATE TABLE custody.entity_ssi (
    entity_ssi_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- What this SSI covers
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    security_type_id UUID REFERENCES custody.security_types(security_type_id),
    market_id UUID REFERENCES custody.markets(market_id),
    currency VARCHAR(3),
    
    -- Their settlement details
    counterparty_bic VARCHAR(11) NOT NULL,
    safekeeping_account VARCHAR(35),
    
    source VARCHAR(20) DEFAULT 'ALERT',  -- ALERT, MANUAL, CTM
    source_reference VARCHAR(100),
    status VARCHAR(20) DEFAULT 'ACTIVE',
    effective_date DATE NOT NULL,
    expiry_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- =============================================================================
-- ISDA AGREEMENTS (Link OTC trades to agreements)
-- =============================================================================
CREATE TABLE custody.isda_agreements (
    isda_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    counterparty_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    agreement_date DATE NOT NULL,
    governing_law VARCHAR(20),  -- NY, ENGLISH
    
    is_active BOOLEAN DEFAULT true,
    effective_date DATE NOT NULL,
    termination_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(cbu_id, counterparty_entity_id, agreement_date)
);

-- =============================================================================
-- ISDA PRODUCT COVERAGE (Which instrument classes an ISDA covers)
-- =============================================================================
CREATE TABLE custody.isda_product_coverage (
    coverage_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    isda_id UUID NOT NULL REFERENCES custody.isda_agreements(isda_id) ON DELETE CASCADE,
    instrument_class_id UUID NOT NULL REFERENCES custody.instrument_classes(class_id),
    isda_taxonomy_id UUID REFERENCES custody.isda_product_taxonomy(taxonomy_id),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(isda_id, instrument_class_id)
);

-- =============================================================================
-- CSA AGREEMENTS (Credit Support Annex under ISDA)
-- =============================================================================
CREATE TABLE custody.csa_agreements (
    csa_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    isda_id UUID NOT NULL REFERENCES custody.isda_agreements(isda_id) ON DELETE CASCADE,
    
    csa_type VARCHAR(20) NOT NULL,  -- VM (Variation Margin), IM (Initial Margin)
    threshold_amount DECIMAL(18,2),
    threshold_currency VARCHAR(3),
    minimum_transfer_amount DECIMAL(18,2),
    rounding_amount DECIMAL(18,2),
    
    -- Collateral SSI link
    collateral_ssi_id UUID REFERENCES custody.cbu_ssi(ssi_id),
    
    is_active BOOLEAN DEFAULT true,
    effective_date DATE NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);
```



### 3.5 Seed Data (Industry-Standard Taxonomies)

```sql
-- =============================================================================
-- INSTRUMENT CLASSES (with CFI, SMPG, ISDA mappings)
-- =============================================================================
INSERT INTO custody.instrument_classes 
(code, name, default_settlement_cycle, swift_message_family, cfi_category, cfi_group, smpg_group, isda_asset_class, requires_isda) 
VALUES
-- Cash Securities
('EQUITY', 'Equities', 'T+1', 'MT54x', 'E', 'ES', 'EQU', NULL, false),
('EQUITY_ADR', 'American Depositary Receipts', 'T+1', 'MT54x', 'E', 'ED', 'EQU', NULL, false),
('EQUITY_ETF', 'Exchange Traded Funds', 'T+1', 'MT54x', 'C', 'CI', 'EQU', NULL, false),
('FIXED_INCOME', 'Fixed Income', 'T+1', 'MT54x', 'D', 'DB', 'Corp FI', NULL, false),
('GOVT_BOND', 'Government Bonds', 'T+1', 'MT54x', 'D', 'DB', 'Govt FI', NULL, false),
('CORP_BOND', 'Corporate Bonds', 'T+2', 'MT54x', 'D', 'DB', 'Corp FI', NULL, false),
('MONEY_MARKET', 'Money Market', 'T+0', 'MT54x', 'D', 'DY', 'MM', NULL, false),
-- OTC Derivatives
('OTC_IRS', 'Interest Rate Swaps', 'T+0', NULL, 'S', 'SR', 'DERIV', 'InterestRate', true),
('OTC_CDS', 'Credit Default Swaps', 'T+0', NULL, 'S', 'SC', 'DERIV', 'Credit', true),
('OTC_EQD', 'Equity Derivatives', 'T+0', NULL, 'S', 'SE', 'DERIV', 'Equity', true),
('OTC_FX', 'FX Derivatives', 'T+0', NULL, 'S', 'SF', 'DERIV', 'ForeignExchange', true),
-- FX
('FX_SPOT', 'FX Spot', 'T+2', 'MT3xx', 'J', 'JF', 'FX/CSH', 'ForeignExchange', false),
('FX_FORWARD', 'FX Forward', 'T+2', 'MT3xx', 'K', 'KF', 'FX/CSH', 'ForeignExchange', false);

-- Set parent relationships
UPDATE custody.instrument_classes SET parent_class_id = 
    (SELECT class_id FROM custody.instrument_classes WHERE code = 'EQUITY')
WHERE code IN ('EQUITY_ADR', 'EQUITY_ETF');

UPDATE custody.instrument_classes SET parent_class_id = 
    (SELECT class_id FROM custody.instrument_classes WHERE code = 'FIXED_INCOME')
WHERE code IN ('GOVT_BOND', 'CORP_BOND', 'MONEY_MARKET');

-- =============================================================================
-- SECURITY TYPES (ALERT/SMPG codes)
-- =============================================================================
INSERT INTO custody.security_types (class_id, code, name, cfi_pattern)
SELECT class_id, t.code, t.name, t.cfi_pattern
FROM custody.instrument_classes ic
CROSS JOIN (VALUES
    ('EQUITY', 'EQU', 'Equities', 'ES****'),
    ('EQUITY', 'ADR', 'American Depositary Receipt', 'ED****'),
    ('EQUITY', 'GDR', 'Global Depositary Receipt', 'ED****'),
    ('EQUITY', 'ETF', 'Exchange Traded Fund', 'CI****'),
    ('EQUITY', 'PRS', 'Preference Shares', 'EP****'),
    ('EQUITY', 'RTS', 'Rights', 'RA****'),
    ('EQUITY', 'UIT', 'Unit Investment Trust', 'CI****'),
    ('FIXED_INCOME', 'COB', 'Corporate Bond', 'DB****'),
    ('FIXED_INCOME', 'ABS', 'Asset Backed Security', 'DA****'),
    ('FIXED_INCOME', 'MBS', 'Mortgage Backed Security', 'DM****'),
    ('FIXED_INCOME', 'CMO', 'Collateralized Mortgage Obligation', 'DM****'),
    ('FIXED_INCOME', 'CON', 'Convertible Bond', 'DC****'),
    ('GOVT_BOND', 'TRY', 'Treasuries', 'DB****'),
    ('GOVT_BOND', 'AGS', 'Agencies', 'DB****'),
    ('GOVT_BOND', 'MNB', 'Municipal Bond', 'DB****'),
    ('MONEY_MARKET', 'MMT', 'Money Market', 'DY****'),
    ('MONEY_MARKET', 'COD', 'Certificate of Deposit', 'DY****'),
    ('MONEY_MARKET', 'COM', 'Commercial Paper', 'DY****'),
    ('MONEY_MARKET', 'REP', 'Repurchase Agreement', 'LR****'),
    ('OTC_IRS', 'IRS', 'Interest Rate Swap', 'SR****'),
    ('OTC_CDS', 'CDS', 'Credit Default Swap', 'SC****'),
    ('OTC_EQD', 'TRS', 'Total Return Swap', 'SE****'),
    ('FX_SPOT', 'CSH', 'Cash', 'JF****'),
    ('FX_SPOT', 'F/X', 'Foreign Exchange', 'JF****')
) AS t(class_code, code, name, cfi_pattern)
WHERE ic.code = t.class_code;

-- =============================================================================
-- ISDA PRODUCT TAXONOMY
-- =============================================================================
INSERT INTO custody.isda_product_taxonomy 
(asset_class, base_product, sub_product, taxonomy_code, class_id)
SELECT t.asset_class, t.base_product, t.sub_product, t.taxonomy_code, ic.class_id
FROM (VALUES
    ('InterestRate', 'IRSwap', 'FixedFloat', 'InterestRate:IRSwap:FixedFloat', 'OTC_IRS'),
    ('InterestRate', 'IRSwap', 'Basis', 'InterestRate:IRSwap:Basis', 'OTC_IRS'),
    ('InterestRate', 'IRSwap', 'OIS', 'InterestRate:IRSwap:OIS', 'OTC_IRS'),
    ('InterestRate', 'IRSwap', 'CrossCurrency', 'InterestRate:IRSwap:CrossCurrency', 'OTC_IRS'),
    ('InterestRate', 'Swaption', NULL, 'InterestRate:Swaption', 'OTC_IRS'),
    ('InterestRate', 'Cap-Floor', NULL, 'InterestRate:Cap-Floor', 'OTC_IRS'),
    ('InterestRate', 'FRA', NULL, 'InterestRate:FRA', 'OTC_IRS'),
    ('Credit', 'CreditDefaultSwap', 'SingleName', 'Credit:CDS:SingleName', 'OTC_CDS'),
    ('Credit', 'CreditDefaultSwap', 'Index', 'Credit:CDS:Index', 'OTC_CDS'),
    ('Credit', 'TotalReturnSwap', NULL, 'Credit:TRS', 'OTC_CDS'),
    ('Equity', 'EquitySwap', 'PriceReturn', 'Equity:Swap:PriceReturn', 'OTC_EQD'),
    ('Equity', 'EquitySwap', 'TotalReturn', 'Equity:Swap:TotalReturn', 'OTC_EQD'),
    ('Equity', 'EquityOption', 'Vanilla', 'Equity:Option:Vanilla', 'OTC_EQD'),
    ('ForeignExchange', 'FXSpot', NULL, 'FX:Spot', 'FX_SPOT'),
    ('ForeignExchange', 'FXForward', 'Deliverable', 'FX:Forward:Deliverable', 'FX_FORWARD'),
    ('ForeignExchange', 'FXForward', 'NDF', 'FX:Forward:NDF', 'FX_FORWARD'),
    ('ForeignExchange', 'FXSwap', NULL, 'FX:Swap', 'OTC_FX'),
    ('ForeignExchange', 'FXOption', 'Vanilla', 'FX:Option:Vanilla', 'OTC_FX')
) AS t(asset_class, base_product, sub_product, taxonomy_code, class_code)
JOIN custody.instrument_classes ic ON ic.code = t.class_code;

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
('SGD', 'Singapore Dollar', 2, true),
('MXN', 'Mexican Peso', 2, true),
('NZD', 'New Zealand Dollar', 2, true),
('SEK', 'Swedish Krona', 2, true),
('NOK', 'Norwegian Krone', 2, true),
('DKK', 'Danish Krone', 2, true),
('ZAR', 'South African Rand', 2, true),
('ILS', 'Israeli Shekel', 2, true),
('KRW', 'South Korean Won', 0, true);

-- =============================================================================
-- MARKETS
-- =============================================================================
INSERT INTO custody.markets (mic, name, country_code, primary_currency, supported_currencies, csd_bic, timezone) VALUES
('XNYS', 'New York Stock Exchange', 'US', 'USD', '{}', 'DTCYUS33', 'America/New_York'),
('XNAS', 'NASDAQ', 'US', 'USD', '{}', 'DTCYUS33', 'America/New_York'),
('XLON', 'London Stock Exchange', 'GB', 'GBP', '{USD,EUR}', 'CABOROCP', 'Europe/London'),
('XPAR', 'Euronext Paris', 'FR', 'EUR', '{}', 'SICABOROCP', 'Europe/Paris'),
('XETR', 'Deutsche Börse Xetra', 'DE', 'EUR', '{}', 'DAKVDEFF', 'Europe/Berlin'),
('XAMS', 'Euronext Amsterdam', 'NL', 'EUR', '{}', 'ECABOROCP', 'Europe/Amsterdam'),
('XSWX', 'SIX Swiss Exchange', 'CH', 'CHF', '{EUR}', 'SABOROCP', 'Europe/Zurich'),
('XTKS', 'Tokyo Stock Exchange', 'JP', 'JPY', '{}', 'JASDECTK', 'Asia/Tokyo'),
('XHKG', 'Hong Kong Stock Exchange', 'HK', 'HKD', '{USD}', 'CCABOROCP', 'Asia/Hong_Kong'),
('XSES', 'Singapore Exchange', 'SG', 'SGD', '{USD}', 'CDABOROCP', 'Asia/Singapore'),
('XASX', 'Australian Securities Exchange', 'AU', 'AUD', '{}', 'CHESAU2S', 'Australia/Sydney'),
('XTSE', 'Toronto Stock Exchange', 'CA', 'CAD', '{USD}', 'CDSLCA2O', 'America/Toronto');

-- =============================================================================
-- INSTRUCTION TYPES
-- =============================================================================
INSERT INTO custody.instruction_types (type_code, name, direction, payment_type, swift_mt_code, iso20022_msg_type) VALUES
('RECEIVE_FOP', 'Receive Free of Payment', 'RECEIVE', 'FOP', 'MT540', 'sese.023'),
('RECEIVE_DVP', 'Receive vs Payment', 'RECEIVE', 'DVP', 'MT541', 'sese.023'),
('DELIVER_FOP', 'Deliver Free of Payment', 'DELIVER', 'FOP', 'MT542', 'sese.023'),
('DELIVER_DVP', 'Deliver vs Payment', 'DELIVER', 'DVP', 'MT543', 'sese.023'),
('RECEIVE_RVP', 'Receive vs Payment (Repo)', 'RECEIVE', 'RVP', 'MT541', 'sese.023'),
('DELIVER_DFP', 'Deliver Free of Payment (Repo)', 'DELIVER', 'DFP', 'MT542', 'sese.023');
```

---

## 4. DSL Domain Design

### 4.1 Domain Overview

| Domain | Scope | Purpose |
|--------|-------|---------|
| `instrument-class` | Bank reference | Instrument taxonomy with CFI/SMPG/ISDA mappings |
| `security-type` | Bank reference | ALERT security type codes |
| `market` | Bank reference | Market/exchange reference data |
| `subcustodian` | Bank reference | Bank's sub-custodian network |
| `cbu-custody` | CBU-scoped | Universe, SSIs, and Booking Rules (3-layer) |
| `entity-settlement` | Entity extension | Counterparty settlement identity |
| `isda` | CBU-scoped | ISDA/CSA agreement management |

### 4.2 New Domains for verbs.yaml

```yaml
# =============================================================================
# DOMAIN: instrument-class (Taxonomy Reference)
# =============================================================================
instrument-class:
  description: "Instrument class with industry taxonomy mappings"
  
  verbs:
    ensure:
      description: "Create or update instrument class with CFI/SMPG/ISDA mappings"
      behavior: crud
      crud:
        operation: upsert
        table: instrument_classes
        schema: custody
        conflict_keys: [code]
        returning: class_id
      args:
        - name: code
          type: string
          required: true
          maps_to: code
        - name: name
          type: string
          required: true
          maps_to: name
        - name: settlement-cycle
          type: string
          required: true
          maps_to: default_settlement_cycle
        - name: swift-family
          type: string
          required: false
          maps_to: swift_message_family
        - name: cfi-category
          type: string
          required: false
          maps_to: cfi_category
        - name: cfi-group
          type: string
          required: false
          maps_to: cfi_group
        - name: smpg-group
          type: string
          required: false
          maps_to: smpg_group
        - name: isda-asset-class
          type: string
          required: false
          maps_to: isda_asset_class
        - name: requires-isda
          type: boolean
          required: false
          maps_to: requires_isda
          default: false
        - name: parent
          type: lookup
          required: false
          lookup:
            table: instrument_classes
            schema: custody
            code_column: code
            id_column: class_id
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
          maps_to: code
      returns:
        type: record

    list:
      description: "List instrument classes with filters"
      behavior: crud
      crud:
        operation: select
        table: instrument_classes
        schema: custody
      args:
        - name: smpg-group
          type: string
          required: false
          maps_to: smpg_group
        - name: isda-asset-class
          type: string
          required: false
          maps_to: isda_asset_class
        - name: requires-isda
          type: boolean
          required: false
          maps_to: requires_isda
      returns:
        type: record_set

# =============================================================================
# DOMAIN: security-type (ALERT codes)
# =============================================================================
security-type:
  description: "SMPG/ALERT security type codes"
  
  verbs:
    ensure:
      description: "Create or update ALERT security type"
      behavior: crud
      crud:
        operation: upsert
        table: security_types
        schema: custody
        conflict_keys: [code]
        returning: security_type_id
      args:
        - name: class
          type: lookup
          required: true
          lookup:
            table: instrument_classes
            schema: custody
            code_column: code
            id_column: class_id
        - name: code
          type: string
          required: true
          maps_to: code
        - name: name
          type: string
          required: true
          maps_to: name
        - name: cfi-pattern
          type: string
          required: false
          maps_to: cfi_pattern
      returns:
        type: uuid
        name: security_type_id
        capture: true

    list:
      description: "List security types for an instrument class"
      behavior: crud
      crud:
        operation: list_by_fk
        table: security_types
        schema: custody
        fk_col: class_id
      args:
        - name: class
          type: lookup
          required: true
          lookup:
            table: instrument_classes
            schema: custody
            code_column: code
            id_column: class_id
      returns:
        type: record_set

# =============================================================================
# DOMAIN: cbu-custody (Three-Layer Model)
# =============================================================================
cbu-custody:
  description: "CBU custody operations: Universe, SSIs, and Booking Rules"
  
  verbs:
    # -------------------------------------------------------------------------
    # LAYER 1: Universe
    # -------------------------------------------------------------------------
    add-universe:
      description: "Declare what a CBU trades (instrument class + market + currencies)"
      behavior: crud
      crud:
        operation: upsert
        table: cbu_instrument_universe
        schema: custody
        conflict_keys: [cbu_id, instrument_class_id, market_id, counterparty_entity_id]
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
            code_column: code
            id_column: class_id
        - name: market
          type: lookup
          required: false
          lookup:
            table: markets
            schema: custody
            code_column: mic
            id_column: market_id
        - name: currencies
          type: string_list
          required: true
          maps_to: currencies
        - name: settlement-types
          type: string_list
          required: false
          maps_to: settlement_types
          default: ["DVP"]
        - name: counterparty
          type: uuid
          required: false
          maps_to: counterparty_entity_id
        - name: is-held
          type: boolean
          required: false
          maps_to: is_held
          default: true
        - name: is-traded
          type: boolean
          required: false
          maps_to: is_traded
          default: true
      returns:
        type: uuid
        name: universe_id
        capture: false

    list-universe:
      description: "List CBU's traded universe"
      behavior: crud
      crud:
        operation: list_by_fk
        table: cbu_instrument_universe
        schema: custody
        fk_col: cbu_id
      args:
        - name: cbu-id
          type: uuid
          required: true
      returns:
        type: record_set

    # -------------------------------------------------------------------------
    # LAYER 2: SSI Data
    # -------------------------------------------------------------------------
    create-ssi:
      description: "Create a Standing Settlement Instruction (pure account data)"
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
        - name: name
          type: string
          required: true
          maps_to: ssi_name
        - name: type
          type: string
          required: true
          maps_to: ssi_type
          valid_values: [SECURITIES, CASH, COLLATERAL, FX_NOSTRO]
        - name: safekeeping-account
          type: string
          required: false
          maps_to: safekeeping_account
        - name: safekeeping-bic
          type: string
          required: false
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
          maps_to: cash_currency
        - name: collateral-account
          type: string
          required: false
          maps_to: collateral_account
        - name: collateral-bic
          type: string
          required: false
          maps_to: collateral_account_bic
        - name: pset-bic
          type: string
          required: false
          maps_to: pset_bic
        - name: effective-date
          type: date
          required: true
          maps_to: effective_date
      returns:
        type: uuid
        name: ssi_id
        capture: true

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
        - name: type
          type: string
          required: false
          maps_to: ssi_type
      returns:
        type: record_set

    activate-ssi:
      description: "Activate an SSI"
      behavior: crud
      crud:
        operation: update
        table: cbu_ssi
        schema: custody
        key: ssi_id
        set_values:
          status: ACTIVE
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
        set_values:
          status: SUSPENDED
      args:
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
      returns:
        type: affected

    # -------------------------------------------------------------------------
    # LAYER 3: Booking Rules (ALERT-style)
    # -------------------------------------------------------------------------
    add-booking-rule:
      description: "Add ALERT-style booking rule for SSI routing"
      behavior: crud
      crud:
        operation: insert
        table: ssi_booking_rules
        schema: custody
        returning: rule_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: ssi-id
          type: uuid
          required: true
          maps_to: ssi_id
        - name: name
          type: string
          required: true
          maps_to: rule_name
        - name: priority
          type: integer
          required: true
          maps_to: priority
        # Match criteria (all optional = wildcards)
        - name: instrument-class
          type: lookup
          required: false
          lookup:
            table: instrument_classes
            schema: custody
            code_column: code
            id_column: class_id
        - name: security-type
          type: lookup
          required: false
          lookup:
            table: security_types
            schema: custody
            code_column: code
            id_column: security_type_id
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
        - name: counterparty
          type: uuid
          required: false
          maps_to: counterparty_entity_id
        # OTC criteria
        - name: isda-asset-class
          type: string
          required: false
          maps_to: isda_asset_class
        - name: isda-base-product
          type: string
          required: false
          maps_to: isda_base_product
        - name: effective-date
          type: date
          required: false
          maps_to: effective_date
      returns:
        type: uuid
        name: rule_id
        capture: true

    list-booking-rules:
      description: "List booking rules for a CBU"
      behavior: crud
      crud:
        operation: list_by_fk
        table: ssi_booking_rules
        schema: custody
        fk_col: cbu_id
        order_by: priority
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

    update-rule-priority:
      description: "Update booking rule priority"
      behavior: crud
      crud:
        operation: update
        table: ssi_booking_rules
        schema: custody
        key: rule_id
      args:
        - name: rule-id
          type: uuid
          required: true
          maps_to: rule_id
        - name: priority
          type: integer
          required: true
          maps_to: priority
      returns:
        type: affected

    deactivate-rule:
      description: "Deactivate a booking rule"
      behavior: crud
      crud:
        operation: update
        table: ssi_booking_rules
        schema: custody
        key: rule_id
        set_values:
          is_active: false
      args:
        - name: rule-id
          type: uuid
          required: true
          maps_to: rule_id
      returns:
        type: affected

    # -------------------------------------------------------------------------
    # Plugins
    # -------------------------------------------------------------------------
    derive-required-coverage:
      description: "Compare universe to booking rules, find gaps"
      behavior: plugin
      handler: derive_required_coverage
      args:
        - name: cbu-id
          type: uuid
          required: true
      returns:
        type: record_set
        # Returns list of { universe_entry, coverage_status: COVERED|MISSING|PARTIAL }

    validate-booking-coverage:
      description: "Validate that all universe entries have matching booking rules"
      behavior: plugin
      handler: validate_booking_coverage
      args:
        - name: cbu-id
          type: uuid
          required: true
      returns:
        type: record
        # Returns { complete: bool, gaps: [], orphan_rules: [] }

    lookup-ssi:
      description: "Find SSI for given trade characteristics (simulate ALERT lookup)"
      behavior: plugin
      handler: lookup_ssi_for_trade
      args:
        - name: cbu-id
          type: uuid
          required: true
        - name: instrument-class
          type: string
          required: true
        - name: security-type
          type: string
          required: false
        - name: market
          type: string
          required: false
        - name: currency
          type: string
          required: true
        - name: settlement-type
          type: string
          required: false
        - name: counterparty-bic
          type: string
          required: false
      returns:
        type: record
        # Returns { ssi_id, ssi_name, matched_rule, rule_priority }
```



```yaml
# =============================================================================
# DOMAIN: isda (ISDA/CSA Agreement Management)
# =============================================================================
isda:
  description: "ISDA and CSA agreement management for OTC derivatives"
  
  verbs:
    create:
      description: "Create ISDA agreement with counterparty"
      behavior: crud
      crud:
        operation: insert
        table: isda_agreements
        schema: custody
        returning: isda_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: counterparty
          type: uuid
          required: true
          maps_to: counterparty_entity_id
        - name: agreement-date
          type: date
          required: true
          maps_to: agreement_date
        - name: governing-law
          type: string
          required: false
          maps_to: governing_law
          valid_values: [NY, ENGLISH]
        - name: effective-date
          type: date
          required: true
          maps_to: effective_date
      returns:
        type: uuid
        name: isda_id
        capture: true

    add-coverage:
      description: "Add instrument class coverage to ISDA"
      behavior: crud
      crud:
        operation: insert
        table: isda_product_coverage
        schema: custody
        returning: coverage_id
      args:
        - name: isda-id
          type: uuid
          required: true
          maps_to: isda_id
        - name: instrument-class
          type: lookup
          required: true
          lookup:
            table: instrument_classes
            schema: custody
            code_column: code
            id_column: class_id
        - name: isda-taxonomy
          type: lookup
          required: false
          lookup:
            table: isda_product_taxonomy
            schema: custody
            code_column: taxonomy_code
            id_column: taxonomy_id
      returns:
        type: uuid
        name: coverage_id

    add-csa:
      description: "Add CSA (Credit Support Annex) to ISDA"
      behavior: crud
      crud:
        operation: insert
        table: csa_agreements
        schema: custody
        returning: csa_id
      args:
        - name: isda-id
          type: uuid
          required: true
          maps_to: isda_id
        - name: csa-type
          type: string
          required: true
          maps_to: csa_type
          valid_values: [VM, IM]
        - name: threshold
          type: decimal
          required: false
          maps_to: threshold_amount
        - name: threshold-currency
          type: string
          required: false
          maps_to: threshold_currency
        - name: mta
          type: decimal
          required: false
          maps_to: minimum_transfer_amount
        - name: collateral-ssi
          type: uuid
          required: false
          maps_to: collateral_ssi_id
        - name: effective-date
          type: date
          required: true
          maps_to: effective_date
      returns:
        type: uuid
        name: csa_id
        capture: true

    list:
      description: "List ISDA agreements for CBU"
      behavior: crud
      crud:
        operation: list_by_fk
        table: isda_agreements
        schema: custody
        fk_col: cbu_id
      args:
        - name: cbu-id
          type: uuid
          required: true
        - name: counterparty
          type: uuid
          required: false
          maps_to: counterparty_entity_id
      returns:
        type: record_set

# =============================================================================
# DOMAIN: entity-settlement (Counterparty SSI from ALERT)
# =============================================================================
entity-settlement:
  description: "Entity settlement identity and SSIs (counterparty data from ALERT)"
  
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
      description: "Add counterparty SSI (from ALERT or manual)"
      behavior: crud
      crud:
        operation: insert
        table: entity_ssi
        schema: custody
        returning: entity_ssi_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: instrument-class
          type: lookup
          required: false
          lookup:
            table: instrument_classes
            schema: custody
            code_column: code
            id_column: class_id
        - name: security-type
          type: lookup
          required: false
          lookup:
            table: security_types
            schema: custody
            code_column: code
            id_column: security_type_id
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

# =============================================================================
# DOMAIN: subcustodian (Bank's Sub-custodian Network)
# =============================================================================
subcustodian:
  description: "Bank's sub-custodian network (Omgeo Institution Network)"
  
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
        - name: local-agent-account
          type: string
          required: false
          maps_to: local_agent_account
        - name: pset
          type: string
          required: true
          maps_to: place_of_settlement_bic
        - name: csd-participant
          type: string
          required: false
          maps_to: csd_participant_id
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
      returns:
        type: record_set

    lookup:
      description: "Find sub-custodian for market/currency"
      behavior: plugin
      handler: subcustodian_lookup
      args:
        - name: market
          type: string
          required: true
        - name: currency
          type: string
          required: true
        - name: as-of-date
          type: date
          required: false
      returns:
        type: record
```

---

## 5. Example DSL Scripts

### 5.1 Reference Data Setup (Run Once by Ops)

```clojure
;; =============================================================================
;; INSTRUMENT CLASSES with Industry Mappings
;; =============================================================================
(instrument-class.ensure
  :code "EQUITY"
  :name "Equities"
  :settlement-cycle "T+1"
  :swift-family "MT54x"
  :cfi-category "E"
  :cfi-group "ES"
  :smpg-group "EQU"
  :as @class-equity)

(instrument-class.ensure
  :code "OTC_IRS"
  :name "Interest Rate Swaps"
  :settlement-cycle "T+0"
  :requires-isda true
  :cfi-category "S"
  :cfi-group "SR"
  :smpg-group "DERIV"
  :isda-asset-class "InterestRate"
  :as @class-irs)

;; =============================================================================
;; SECURITY TYPES (ALERT codes)
;; =============================================================================
(security-type.ensure
  :class "EQUITY"
  :code "EQU"
  :name "Equities"
  :cfi-pattern "ES****")

(security-type.ensure
  :class "EQUITY"
  :code "ADR"
  :name "American Depositary Receipt"
  :cfi-pattern "ED****")

(security-type.ensure
  :class "OTC_IRS"
  :code "IRS"
  :name "Interest Rate Swap"
  :cfi-pattern "SR****")

;; =============================================================================
;; SUB-CUSTODIAN NETWORK (Bank's global agent network)
;; =============================================================================
(subcustodian.ensure
  :market "XNYS"
  :currency "USD"
  :subcustodian-bic "BABOROCP"
  :subcustodian-name "Our Bank"
  :local-agent-bic "BOFAUS3N"
  :pset "DTCYUS33"
  :is-primary true
  :effective-date "2020-01-01")

(subcustodian.ensure
  :market "XLON"
  :currency "GBP"
  :subcustodian-bic "MIDLGB22"
  :subcustodian-name "HSBC UK"
  :pset "CABOROCP"
  :csd-participant "HSBC001"
  :effective-date "2020-01-01")

(subcustodian.ensure
  :market "XLON"
  :currency "USD"
  :subcustodian-bic "CITIUS33"
  :subcustodian-name "Citi UK"
  :pset "CABOROCP"
  :effective-date "2020-01-01")
```

### 5.2 CBU Custody Onboarding (Three-Layer Model)

```clojure
;; =============================================================================
;; CBU: Acme Pension Fund LP - Custody Onboarding
;; =============================================================================

;; Assume CBU exists from KYC onboarding
;; (cbu.read :name "Acme Pension Fund LP" :as @cbu)

;; =============================================================================
;; LAYER 1: Define what they trade (Universe)
;; =============================================================================
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"])

(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XLON"
  :currencies ["GBP" "USD"]  ;; They settle in both
  :settlement-types ["DVP"])

(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "OTC_IRS"
  :currencies ["USD" "EUR"]
  :counterparty @ms-entity)  ;; OTC with Morgan Stanley

;; =============================================================================
;; LAYER 2: Create SSI Data (Pure account info)
;; =============================================================================
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "US Primary Safekeeping"
  :type "SECURITIES"
  :safekeeping-account "12345-SAFE"
  :safekeeping-bic "BABOROCP"
  :safekeeping-name "Acme Pension Safekeeping"
  :cash-account "12345-USD"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-12-01"
  :as @ssi-us)

(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "UK GBP Safekeeping"
  :type "SECURITIES"
  :safekeeping-account "UK-001"
  :safekeeping-bic "MIDLGB22"
  :cash-account "UK-GBP"
  :cash-bic "MIDLGB22"
  :cash-currency "GBP"
  :pset-bic "CABOROCP"
  :effective-date "2024-12-01"
  :as @ssi-uk)

(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "Morgan Stanley Special Account"
  :type "SECURITIES"
  :safekeeping-account "MS-SPECIAL-001"
  :safekeeping-bic "BABOROCP"
  :cash-account "MS-USD"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :effective-date "2024-12-01"
  :as @ssi-ms)

(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "OTC Collateral IM"
  :type "COLLATERAL"
  :collateral-account "SEG-IM-001"
  :collateral-bic "BABOROCP"
  :effective-date "2024-12-01"
  :as @ssi-collateral)

;; Activate SSIs
(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-uk)
(cbu-custody.activate-ssi :ssi-id @ssi-ms)
(cbu-custody.activate-ssi :ssi-id @ssi-collateral)

;; =============================================================================
;; LAYER 3: Define Booking Rules (ALERT-style routing)
;; =============================================================================

;; Specific rules (priority 10)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "US Equity DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP")

(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-uk
  :name "UK Equity GBP DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :settlement-type "DVP")

;; Cross-currency: UK equities settling in USD
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "UK Equity USD Settlement"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "USD"
  :settlement-type "DVP")

;; Counterparty override (priority 5 = higher than standard rules)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-ms
  :name "Morgan Stanley Override"
  :priority 5
  :counterparty @ms-entity
  :currency "USD")  ;; All USD trades with MS use special account

;; OTC IRS rule
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-collateral
  :name "IRS Collateral"
  :priority 10
  :isda-asset-class "InterestRate"
  :isda-base-product "IRSwap")

;; Fallback rules (priority 50)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "USD Fallback"
  :priority 50
  :currency "USD")  ;; Any USD not matched above

;; Ultimate fallback (priority 100)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "Ultimate Fallback"
  :priority 100)  ;; No criteria = matches anything

;; =============================================================================
;; Validate coverage
;; =============================================================================
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
;; Returns: { complete: true, gaps: [], orphan_rules: [] }

;; Simulate SSI lookup for a trade
(cbu-custody.lookup-ssi
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :security-type "EQU"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP")
;; Returns: { ssi_id: @ssi-us, ssi_name: "US Primary Safekeeping", 
;;            matched_rule: "US Equity DVP", rule_priority: 10 }
```

### 5.3 OTC Counterparty & ISDA Setup

```clojure
;; =============================================================================
;; Counterparty: Morgan Stanley - Settlement Identity (from ALERT)
;; =============================================================================

;; Set their primary settlement identity
(entity-settlement.set-identity
  :entity-id @ms
  :bic "MSNYUS33"
  :lei "IGJSJL3JD5P30I6NJZ34"
  :alert-id "ALERT-MS-001"
  :ctm-id "CTM-MS-001")

;; Add their SSIs per security type (sourced from ALERT)
(entity-settlement.add-ssi
  :entity-id @ms
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :counterparty-bic "MSNYUS33"
  :safekeeping-account "MS-CUSTODY-001"
  :source "ALERT"
  :source-reference "ALERT-MS-SSI-001"
  :effective-date "2024-01-01")

;; =============================================================================
;; ISDA Agreement with Morgan Stanley
;; =============================================================================
(isda.create
  :cbu-id @cbu
  :counterparty @ms
  :agreement-date "2024-01-01"
  :governing-law "NY"
  :effective-date "2024-01-01"
  :as @isda-ms)

;; Add product coverage
(isda.add-coverage
  :isda-id @isda-ms
  :instrument-class "OTC_IRS"
  :isda-taxonomy "InterestRate:IRSwap:FixedFloat")

(isda.add-coverage
  :isda-id @isda-ms
  :instrument-class "OTC_IRS"
  :isda-taxonomy "InterestRate:IRSwap:OIS")

;; Add CSA for Variation Margin
(isda.add-csa
  :isda-id @isda-ms
  :csa-type "VM"
  :threshold 250000
  :threshold-currency "USD"
  :mta 500000
  :collateral-ssi @ssi-collateral
  :effective-date "2024-01-01"
  :as @csa-vm)

;; Add CSA for Initial Margin (segregated)
(isda.add-csa
  :isda-id @isda-ms
  :csa-type "IM"
  :collateral-ssi @ssi-collateral
  :effective-date "2024-01-01"
  :as @csa-im)
```



---

## 6. Trade Routing Flow

### 6.1 ALERT-Style SSI Lookup Algorithm

```
INCOMING TRADE
     │
     ├── Parse: ISIN, MIC, Currency, Direction, Counterparty BIC
     │
     ▼
┌────────────────────────────────────────┐
│ 1. Map ISIN to Classification          │
│    - Lookup CFI code from ISIN         │
│    - Map CFI → instrument_class_id     │
│    - Map CFI → security_type_id        │
└────────────────────────────────────────┘
     │
     ▼
┌────────────────────────────────────────┐
│ 2. Collect Booking Rules               │
│    SELECT * FROM ssi_booking_rules     │
│    WHERE cbu_id = ?                    │
│      AND is_active = true              │
│      AND (expiry_date IS NULL          │
│           OR expiry_date > now())      │
│    ORDER BY priority ASC               │
└────────────────────────────────────────┘
     │
     ▼
┌────────────────────────────────────────┐
│ 3. Match Rules (first match wins)      │
│    FOR each rule in priority order:    │
│      IF rule.instrument_class_id IS NULL OR matches trade │
│      AND rule.security_type_id IS NULL OR matches trade   │
│      AND rule.market_id IS NULL OR matches trade          │
│      AND rule.currency IS NULL OR matches trade           │
│      AND rule.settlement_type IS NULL OR matches trade    │
│      AND rule.counterparty_entity_id IS NULL OR matches   │
│      THEN return rule.ssi_id           │
└────────────────────────────────────────┘
     │
     ▼
┌────────────────────────────────────────┐
│ 4. Retrieve SSI Data                   │
│    SELECT * FROM cbu_ssi               │
│    WHERE ssi_id = matched_ssi_id       │
│    → Safekeeping account               │
│    → Cash account                      │
│    → PSET                              │
└────────────────────────────────────────┘
     │
     ▼
┌────────────────────────────────────────┐
│ 5. Enrich from Sub-custodian Network   │
│    SELECT * FROM subcustodian_network  │
│    WHERE market_id = ? AND currency = ?│
│    → Agent chain                       │
│    → CSD participant ID                │
└────────────────────────────────────────┘
     │
     ▼
┌────────────────────────────────────────┐
│ 6. Route to Service Resource           │
│    SWIFT_GATEWAY                       │
│    → Generate MT541/543                │
└────────────────────────────────────────┘
```

### 6.2 SQL Implementation of Rule Matching

```sql
-- Function to find matching SSI for a trade
CREATE OR REPLACE FUNCTION custody.find_ssi_for_trade(
    p_cbu_id UUID,
    p_instrument_class_id UUID,
    p_security_type_id UUID,
    p_market_id UUID,
    p_currency VARCHAR(3),
    p_settlement_type VARCHAR(10),
    p_counterparty_entity_id UUID DEFAULT NULL
)
RETURNS TABLE (
    ssi_id UUID,
    ssi_name VARCHAR(100),
    rule_id UUID,
    rule_name VARCHAR(100),
    rule_priority INTEGER,
    specificity_score INTEGER
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        r.ssi_id,
        s.ssi_name,
        r.rule_id,
        r.rule_name,
        r.priority,
        r.specificity_score
    FROM custody.ssi_booking_rules r
    JOIN custody.cbu_ssi s ON r.ssi_id = s.ssi_id
    WHERE r.cbu_id = p_cbu_id
      AND r.is_active = true
      AND s.status = 'ACTIVE'
      AND (r.expiry_date IS NULL OR r.expiry_date > CURRENT_DATE)
      -- Match criteria (NULL = wildcard)
      AND (r.instrument_class_id IS NULL OR r.instrument_class_id = p_instrument_class_id)
      AND (r.security_type_id IS NULL OR r.security_type_id = p_security_type_id)
      AND (r.market_id IS NULL OR r.market_id = p_market_id)
      AND (r.currency IS NULL OR r.currency = p_currency)
      AND (r.settlement_type IS NULL OR r.settlement_type = p_settlement_type)
      AND (r.counterparty_entity_id IS NULL OR r.counterparty_entity_id = p_counterparty_entity_id)
    ORDER BY r.priority ASC
    LIMIT 1;
END;
$$ LANGUAGE plpgsql;
```

---

## 7. Implementation Tasks

### Phase 1: Database Schema (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 7.1.1 | Create `custody` schema | S |
| 7.1.2 | Create taxonomy tables (instrument_classes, security_types, cfi_codes, isda_product_taxonomy) | M |
| 7.1.3 | Create core reference tables (currencies, markets, instruction_types) | S |
| 7.1.4 | Create subcustodian_network table | M |
| 7.1.5 | Create Layer 1: cbu_instrument_universe | S |
| 7.1.6 | Create Layer 2: cbu_ssi with agent_override | M |
| 7.1.7 | Create Layer 3: ssi_booking_rules with computed specificity | M |
| 7.1.8 | Create instruction_paths table | S |
| 7.1.9 | Create entity_settlement_identity and entity_ssi | S |
| 7.1.10 | Create ISDA tables (isda_agreements, isda_product_coverage, csa_agreements) | M |
| 7.1.11 | Create all indexes | S |
| 7.1.12 | Insert taxonomy seed data (CFI, SMPG, ISDA mappings) | M |
| 7.1.13 | Insert reference seed data (currencies, markets, instruction types) | S |
| 7.1.14 | Create find_ssi_for_trade function | S |

### Phase 2: DSL Configuration (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 7.2.1 | Add `instrument-class` domain to verbs.yaml | S |
| 7.2.2 | Add `security-type` domain to verbs.yaml | S |
| 7.2.3 | Add `market` domain to verbs.yaml | S |
| 7.2.4 | Add `subcustodian` domain to verbs.yaml | M |
| 7.2.5 | Add `cbu-custody` domain (3-layer model) to verbs.yaml | L |
| 7.2.6 | Add `entity-settlement` domain to verbs.yaml | M |
| 7.2.7 | Add `isda` domain to verbs.yaml | M |
| 7.2.8 | Configure cross-schema lookups (custody ↔ ob-poc) | M |

### Phase 3: Plugin Handlers (Priority: MEDIUM)

| Task | Description | Effort |
|------|-------------|--------|
| 7.3.1 | Implement `subcustodian_lookup` handler | M |
| 7.3.2 | Implement `derive_required_coverage` handler | L |
| 7.3.3 | Implement `validate_booking_coverage` handler | M |
| 7.3.4 | Implement `lookup_ssi_for_trade` handler | M |
| 7.3.5 | Add custody plugin module to dsl_v2/custom_ops | S |

### Phase 4: Testing (Priority: HIGH)

| Task | Description | Effort |
|------|-------------|--------|
| 7.4.1 | Create taxonomy seed data validation tests | M |
| 7.4.2 | Create sub-custodian network setup tests | M |
| 7.4.3 | Create CBU 3-layer onboarding test scripts | L |
| 7.4.4 | Create booking rule matching tests | L |
| 7.4.5 | Create OTC/ISDA setup tests | M |
| 7.4.6 | Create ALERT SSI import simulation tests | M |

---

## 8. Summary: v2 → v3 Changes

| Aspect | v2 | v3 |
|--------|----|----|
| **Model** | Settlement Profile → SSI | 3-Layer: Universe → SSI → Booking Rules |
| **SSI Routing** | Profile code lookup | ALERT-style priority rules with wildcards |
| **Instrument Classification** | Custom codes | CFI + SMPG/ALERT + ISDA taxonomy |
| **Rule Matching** | Implicit (by profile) | Explicit rules with priority ordering |
| **Wildcards** | None | NULL = any (ALERT "ANYY" equivalent) |
| **Counterparty Override** | Separate table | Booking rule with counterparty criterion |
| **OTC Support** | Out of scope | ISDA taxonomy + agreement tables |
| **Industry Alignment** | Partial | Full (DTCC ALERT, ISO 10962, SMPG, ISDA) |

---

## 9. Omgeo/ALERT Equivalence

| ALERT Feature | ob-poc Implementation |
|---------------|----------------------|
| Account | CBU |
| Account SSI | cbu_ssi |
| Security Type (EQTY, BOND, ANYY) | instrument_class_id / security_type_id |
| Country | market_id |
| Currency | currency |
| Method (DVP, FOP) | settlement_type |
| Priority matching | priority column in booking rules |
| "ANYY" wildcard | NULL in rule criteria |
| Model SSI | SSI with booking rules pointing to it |
| GC Direct | We ARE the custodian - we define SSIs |
| Institution Network | subcustodian_network |
| Cross-reference | entity_settlement_identity + entity_ssi |
| Compliance Scan | validate_booking_coverage plugin |

---

## 10. Future Extensions

| Extension | Foundation in v3 |
|-----------|-----------------|
| **FX Settlement Instructions** | Currency in universe, FX_NOSTRO SSI type |
| **Repo/Securities Lending** | GMRA/GMSLA as agreement type alongside ISDA |
| **Collateral Optimization** | CSA tables link to collateral SSIs |
| **ALERT Import** | entity_ssi with source='ALERT' |
| **CTM Integration** | ctm_participant_id in entity_settlement_identity |
| **Regulatory Reporting** | ISDA taxonomy + UPI templates |

---

*Document Version: 3.0*
*Created: 2024-12-01*
*Key Changes: Three-layer model, ALERT-style booking rules, industry taxonomy alignment*
*Author: Claude (Implementation Planning)*
*For: Claude Code Execution*

