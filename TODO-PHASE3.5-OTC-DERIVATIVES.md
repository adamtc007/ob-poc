# Phase 3.5: OTC Derivatives & Collateral Management

**Purpose:** Extend the agent intelligence layer to support OTC derivatives trading matrix capture, demonstrating deep domain understanding of derivatives custody - a primary revenue driver for custody banks.

**Business Context:** Derivatives custody generates significant fee income through:
- Collateral management fees (movement, optimization, transformation)
- Valuation and mark-to-market services
- Margin call processing
- Tri-party collateral services
- Regulatory reporting (EMIR, Dodd-Frank, MiFID II)

**Success Criteria:**
1. Agent can capture complete OTC trading setup through conversation
2. ISDA/CSA relationship modeling is correct
3. Collateral workflows are properly represented
4. Demonstrates understanding that impresses custody operations experts

---

## Domain Understanding: Why This Matters

### The Derivatives Custody Value Chain

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        OTC DERIVATIVES LIFECYCLE                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐              │
│  │  TRADE   │───▶│ CONFIRM  │───▶│ COLLAT   │───▶│ SETTLE/  │              │
│  │EXECUTION │    │  MATCH   │    │  MGMT    │    │ LIFECYCLE│              │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘              │
│       │               │               │               │                     │
│       ▼               ▼               ▼               ▼                     │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐              │
│  │ Trading  │    │  DTCC    │    │  Daily   │    │ Cashflow │              │
│  │ Platform │    │MarkitWire│    │  Margin  │    │ Settlement│             │
│  │Bloomberg │    │  SWIFT   │    │  Calls   │    │ Novation │              │
│  │Tradeweb  │    │  Paper   │    │          │    │ Unwind   │              │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### ISDA Framework Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ISDA MASTER AGREEMENT                              │
│                    (Governs all OTC trades between parties)                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     ISDA MASTER (2002 or 1992)                       │    │
│  │  • Netting provisions (close-out netting)                           │    │
│  │  • Events of default / termination events                           │    │
│  │  • Representations and warranties                                   │    │
│  │  • Governing law (typically NY or English)                          │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                          SCHEDULE                                    │    │
│  │  • Party-specific elections                                         │    │
│  │  • Threshold amounts                                                │    │
│  │  • Credit support provisions                                        │    │
│  │  • Additional termination events                                    │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    CREDIT SUPPORT ANNEX (CSA)                        │    │
│  │  • Eligible collateral (cash, govvies, corps)                       │    │
│  │  • Haircuts by asset class                                          │    │
│  │  • Threshold / Minimum Transfer Amount (MTA)                        │    │
│  │  • Independent Amount (IA) / Initial Margin                         │    │
│  │  • Valuation timing and dispute resolution                          │    │
│  │  • Interest on cash collateral                                      │    │
│  │  • Rehypothecation rights                                           │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                      TRADE CONFIRMATIONS                             │    │
│  │  • Individual transaction details                                   │    │
│  │  • Economic terms                                                   │    │
│  │  • Incorporate Master by reference                                  │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Collateral Operations Daily Cycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     DAILY COLLATERAL CYCLE (T+0)                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  06:00 ─────────────────────────────────────────────────────────────────    │
│         │ Mark-to-Market: Value all OTC positions                           │
│         │ • Receive valuations from pricing sources                         │
│         │ • Apply agreed valuation methodologies                            │
│         │                                                                    │
│  08:00 ─────────────────────────────────────────────────────────────────    │
│         │ Exposure Calculation:                                             │
│         │ • Net exposures by counterparty/CSA                               │
│         │ • Apply thresholds and MTAs                                       │
│         │ • Calculate required collateral                                   │
│         │                                                                    │
│  09:00 ─────────────────────────────────────────────────────────────────    │
│         │ Margin Call Generation:                                           │
│         │ • Issue calls for increased exposure                              │
│         │ • Process returns for decreased exposure                          │
│         │ • Handle disputes                                                 │
│         │                                                                    │
│  14:00 ─────────────────────────────────────────────────────────────────    │
│         │ Collateral Movement:                                              │
│         │ • Agree collateral with counterparty                              │
│         │ • Instruct settlements                                            │
│         │ • Update collateral positions                                     │
│         │                                                                    │
│  16:00 ─────────────────────────────────────────────────────────────────    │
│         │ Reconciliation:                                                   │
│         │ • Confirm movements settled                                       │
│         │ • Reconcile positions                                             │
│         │ • Report to client                                                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Uncleared Margin Rules (UMR) Context

```
Regulatory driver for derivatives collateral complexity:

Phase 1-6 (2016-2022): Initial Margin requirements phased in by AANA threshold
  • Covered entities must post/collect IM for uncleared OTC derivatives
  • IM must be held at independent third-party custodian
  • Segregation requirements (no rehypothecation of IM)

Impact on Custody:
  • Tri-party IM segregation accounts
  • IM calculation agent services  
  • SIMM (Standard Initial Margin Model) calculations
  • Regulatory reporting

This is why derivatives custody setup is complex - regulatory requirements
drive multi-party relationships and specific account structures.
```

---

## Phase 3.5.1: Data Model Extensions

### Task 3.5.1.1: Counterparty Entity
**File:** `rust/migrations/YYYYMMDD_counterparty.sql`

```sql
-- Counterparty: External entity we trade derivatives with
CREATE TABLE counterparty (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- Identification
    name VARCHAR(255) NOT NULL,
    short_name VARCHAR(50),
    lei VARCHAR(20) UNIQUE,  -- Legal Entity Identifier (validated)
    
    -- Classification
    counterparty_type VARCHAR(50) NOT NULL,  -- BANK, BROKER_DEALER, ASSET_MANAGER, CORPORATE, SOVEREIGN
    jurisdiction VARCHAR(10) NOT NULL,
    
    -- Regulatory status
    is_financial_counterparty BOOLEAN DEFAULT true,  -- EMIR classification
    is_covered_entity BOOLEAN DEFAULT false,  -- UMR Phase 1-6
    
    -- Operational
    primary_contact_email VARCHAR(255),
    operations_contact_email VARCHAR(255),
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Counterparty BIC codes (may have multiple)
CREATE TABLE counterparty_bic (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    counterparty_id UUID NOT NULL REFERENCES counterparty(id),
    bic VARCHAR(11) NOT NULL,
    bic_type VARCHAR(20) NOT NULL,  -- SWIFT, DTCC, MARKITWIRE
    is_primary BOOLEAN DEFAULT false,
    UNIQUE(counterparty_id, bic, bic_type)
);
```

### Task 3.5.1.2: ISDA Master Agreement
**File:** `rust/migrations/YYYYMMDD_isda.sql`

```sql
-- ISDA Master Agreement between CBU and Counterparty
CREATE TABLE isda_master_agreement (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- Parties
    cbu_id UUID NOT NULL REFERENCES cbu(id),
    counterparty_id UUID NOT NULL REFERENCES counterparty(id),
    
    -- Agreement details
    agreement_version VARCHAR(10) NOT NULL,  -- 2002, 1992
    agreement_date DATE NOT NULL,
    governing_law VARCHAR(20) NOT NULL,  -- NY, ENGLISH, OTHER
    
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE',  -- DRAFT, ACTIVE, SUSPENDED, TERMINATED
    effective_date DATE,
    termination_date DATE,
    
    -- Key elections (from Schedule)
    netting_applicable BOOLEAN DEFAULT true,
    cross_default_applicable BOOLEAN DEFAULT true,
    cross_default_threshold DECIMAL(18,2),
    cross_default_threshold_ccy VARCHAR(3),
    
    -- Document reference
    document_reference VARCHAR(255),
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(cbu_id, counterparty_id)  -- One ISDA per counterparty relationship
);

-- Product scope under the ISDA
CREATE TABLE isda_product_scope (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    isda_id UUID NOT NULL REFERENCES isda_master_agreement(id),
    product_type VARCHAR(50) NOT NULL,  -- IRS, CDS, XCCY, FX_OPTION, SWAPTION, etc.
    is_included BOOLEAN DEFAULT true,
    notes TEXT,
    UNIQUE(isda_id, product_type)
);
```

### Task 3.5.1.3: Credit Support Annex (CSA)
**File:** `rust/migrations/YYYYMMDD_csa.sql`

```sql
-- Credit Support Annex - collateral terms
CREATE TABLE credit_support_annex (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- Link to ISDA
    isda_id UUID NOT NULL REFERENCES isda_master_agreement(id),
    
    -- CSA type
    csa_type VARCHAR(20) NOT NULL,  -- VM (Variation Margin), IM (Initial Margin), LEGACY
    csa_version VARCHAR(20),  -- 2016 VM, 2018 IM, 1995 (legacy)
    
    -- Threshold and MTA (party-specific)
    our_threshold DECIMAL(18,2) DEFAULT 0,
    our_threshold_ccy VARCHAR(3) DEFAULT 'USD',
    their_threshold DECIMAL(18,2) DEFAULT 0,
    their_threshold_ccy VARCHAR(3) DEFAULT 'USD',
    
    minimum_transfer_amount DECIMAL(18,2) DEFAULT 500000,
    mta_ccy VARCHAR(3) DEFAULT 'USD',
    
    rounding DECIMAL(18,2) DEFAULT 10000,
    
    -- Independent Amount (Initial Margin proxy for legacy CSAs)
    our_independent_amount DECIMAL(18,2) DEFAULT 0,
    their_independent_amount DECIMAL(18,2) DEFAULT 0,
    ia_ccy VARCHAR(3) DEFAULT 'USD',
    
    -- Valuation
    valuation_time VARCHAR(50),  -- "5pm NY", "Close London"
    valuation_agent VARCHAR(20),  -- US, THEM, CALCULATION_AGENT
    dispute_resolution_days INTEGER DEFAULT 2,
    
    -- Interest on cash collateral
    interest_rate_benchmark VARCHAR(50),  -- SOFR, ESTR, SONIA
    interest_rate_spread DECIMAL(8,4) DEFAULT 0,
    interest_payment_frequency VARCHAR(20) DEFAULT 'MONTHLY',
    
    -- Rehypothecation
    rehypothecation_permitted BOOLEAN DEFAULT false,
    
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE',
    effective_date DATE,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Eligible collateral under CSA
CREATE TABLE csa_eligible_collateral (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    csa_id UUID NOT NULL REFERENCES credit_support_annex(id),
    
    -- Collateral type
    asset_class VARCHAR(50) NOT NULL,  -- CASH, GOVT_BOND, AGENCY, CORP_BOND, EQUITY, GOLD
    currency VARCHAR(3),  -- For cash, or issuer currency for bonds
    issuer_jurisdiction VARCHAR(10),  -- For bonds
    min_rating VARCHAR(10),  -- AA, A, BBB
    max_maturity_years INTEGER,  -- For bonds
    
    -- Haircut
    haircut_pct DECIMAL(8,4) NOT NULL DEFAULT 0,
    
    -- FX haircut (additional for non-base currency)
    fx_haircut_pct DECIMAL(8,4) DEFAULT 8.0,
    
    -- Concentration limits
    max_concentration_pct DECIMAL(8,4),
    
    -- Priority (for optimization)
    priority INTEGER DEFAULT 100,
    
    UNIQUE(csa_id, asset_class, currency, issuer_jurisdiction)
);
```

### Task 3.5.1.4: Collateral Account Structure
**File:** `rust/migrations/YYYYMMDD_collateral_accounts.sql`

```sql
-- Collateral accounts (segregated per CSA/counterparty)
CREATE TABLE collateral_account (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    cbu_id UUID NOT NULL REFERENCES cbu(id),
    csa_id UUID REFERENCES credit_support_annex(id),
    
    -- Account identification
    account_name VARCHAR(255) NOT NULL,
    account_number VARCHAR(50),
    
    -- Account type
    account_type VARCHAR(30) NOT NULL,  
    -- POSTED_VM (we posted to them)
    -- RECEIVED_VM (they posted to us)  
    -- POSTED_IM (we posted - segregated)
    -- RECEIVED_IM (they posted - segregated)
    -- TRI_PARTY (tri-party IM)
    
    -- Custodian (for IM must be third-party)
    custodian_bic VARCHAR(11),
    custodian_name VARCHAR(255),
    is_third_party_custodian BOOLEAN DEFAULT false,
    
    -- Segregation
    is_segregated BOOLEAN DEFAULT false,
    segregation_type VARCHAR(30),  -- INDIVIDUAL, OMNIBUS
    
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE',
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Collateral positions
CREATE TABLE collateral_position (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    account_id UUID NOT NULL REFERENCES collateral_account(id),
    
    -- Asset identification
    asset_type VARCHAR(30) NOT NULL,  -- CASH, SECURITY
    currency VARCHAR(3),  -- For cash
    isin VARCHAR(12),  -- For securities
    
    -- Position
    quantity DECIMAL(18,4) NOT NULL,
    market_value DECIMAL(18,2),
    market_value_ccy VARCHAR(3),
    haircut_value DECIMAL(18,2),  -- After haircut
    
    -- Valuation date
    valuation_date DATE NOT NULL,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now()
);
```

### Task 3.5.1.5: Confirmation Workflow Configuration
**File:** `rust/migrations/YYYYMMDD_confirmation.sql`

```sql
-- Confirmation method by counterparty/product
CREATE TABLE confirmation_config (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    cbu_id UUID NOT NULL REFERENCES cbu(id),
    counterparty_id UUID REFERENCES counterparty(id),  -- NULL = default
    
    -- Scope
    product_type VARCHAR(50),  -- NULL = all products
    
    -- Confirmation method
    confirmation_method VARCHAR(30) NOT NULL,
    -- DTCC_GTR (DTCC Global Trade Repository)
    -- MARKITWIRE
    -- SWIFT_MT300 (FX)
    -- SWIFT_MT360 (IRS)
    -- PAPER
    -- EMAIL
    
    -- Platform details
    platform_id VARCHAR(50),
    our_platform_id VARCHAR(50),  -- Our ID on platform
    
    -- STP settings
    auto_match_enabled BOOLEAN DEFAULT true,
    match_tolerance_bps DECIMAL(8,4) DEFAULT 1.0,  -- Valuation tolerance
    
    -- Timing
    target_confirmation_days INTEGER DEFAULT 1,  -- T+1 confirmation target
    
    -- Priority (lower = higher priority)
    priority INTEGER DEFAULT 100,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);
```

---

## Phase 3.5.2: DSL Verbs for Derivatives

### Task 3.5.2.1: Counterparty Verbs
**File:** `rust/config/verbs/counterparty.yaml`

```yaml
domain: counterparty
description: "Counterparty management for derivatives trading"

verbs:
  - verb: ensure
    description: "Create or update counterparty"
    entity_type: counterparty
    operation: upsert
    table: counterparty
    
    parameters:
      - name: name
        type: string
        required: true
        description: "Legal name"
      - name: lei
        type: string
        pattern: "^[A-Z0-9]{20}$"
        description: "Legal Entity Identifier"
      - name: short-name
        type: string
        column: short_name
      - name: counterparty-type
        type: enum
        values: [BANK, BROKER_DEALER, ASSET_MANAGER, CORPORATE, SOVEREIGN]
        required: true
        column: counterparty_type
      - name: jurisdiction
        type: string
        required: true
      - name: is-financial-counterparty
        type: boolean
        default: true
        column: is_financial_counterparty
      - name: is-covered-entity
        type: boolean
        default: false
        column: is_covered_entity
        description: "Subject to UMR IM requirements"
    
    binding: counterparty_id

  - verb: add-bic
    description: "Add BIC code to counterparty"
    entity_type: counterparty_bic
    operation: insert
    table: counterparty_bic
    
    parameters:
      - name: counterparty-id
        type: reference
        entity_type: counterparty
        required: true
        column: counterparty_id
      - name: bic
        type: string
        required: true
        pattern: "^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$"
      - name: bic-type
        type: enum
        values: [SWIFT, DTCC, MARKITWIRE]
        required: true
        column: bic_type
      - name: is-primary
        type: boolean
        default: false
        column: is_primary

  - verb: list
    description: "List counterparties"
    operation: query
    table: counterparty
    
    parameters:
      - name: counterparty-type
        type: enum
        values: [BANK, BROKER_DEALER, ASSET_MANAGER, CORPORATE, SOVEREIGN]
        column: counterparty_type
      - name: jurisdiction
        type: string
```

### Task 3.5.2.2: ISDA Verbs
**File:** `rust/config/verbs/isda.yaml`

```yaml
domain: isda
description: "ISDA Master Agreement management"

verbs:
  - verb: establish
    description: "Establish ISDA Master Agreement with counterparty"
    entity_type: isda_master
    operation: upsert
    table: isda_master_agreement
    unique_on: [cbu_id, counterparty_id]
    
    parameters:
      - name: cbu-id
        type: reference
        entity_type: cbu
        required: true
        column: cbu_id
      - name: counterparty-id
        type: reference
        entity_type: counterparty
        required: true
        column: counterparty_id
      - name: version
        type: enum
        values: ["2002", "1992"]
        required: true
        column: agreement_version
      - name: agreement-date
        type: date
        required: true
        column: agreement_date
      - name: governing-law
        type: enum
        values: [NY, ENGLISH, OTHER]
        required: true
        column: governing_law
      - name: netting-applicable
        type: boolean
        default: true
        column: netting_applicable
      - name: cross-default-applicable
        type: boolean
        default: true
        column: cross_default_applicable
      - name: cross-default-threshold
        type: decimal
        column: cross_default_threshold
      - name: cross-default-threshold-ccy
        type: string
        default: USD
        column: cross_default_threshold_ccy
    
    binding: isda_id

  - verb: add-product-scope
    description: "Add product type to ISDA scope"
    entity_type: isda_product_scope
    operation: upsert
    table: isda_product_scope
    unique_on: [isda_id, product_type]
    
    parameters:
      - name: isda-id
        type: reference
        entity_type: isda_master
        required: true
        column: isda_id
      - name: product-type
        type: enum
        values: [IRS, XCCY, CDS, FX_FORWARD, FX_OPTION, SWAPTION, EQUITY_SWAP, COMMODITY_SWAP, REPO]
        required: true
        column: product_type
      - name: included
        type: boolean
        default: true
        column: is_included

  - verb: query-by-counterparty
    description: "Find ISDA for counterparty"
    operation: query
    table: isda_master_agreement
    
    parameters:
      - name: cbu-id
        type: reference
        entity_type: cbu
        required: true
      - name: counterparty-id
        type: reference
        entity_type: counterparty
```

### Task 3.5.2.3: CSA Verbs
**File:** `rust/config/verbs/csa.yaml`

```yaml
domain: csa
description: "Credit Support Annex management"

verbs:
  - verb: establish
    description: "Establish CSA under ISDA"
    entity_type: csa
    operation: insert
    table: credit_support_annex
    
    parameters:
      - name: isda-id
        type: reference
        entity_type: isda_master
        required: true
        column: isda_id
      - name: csa-type
        type: enum
        values: [VM, IM, LEGACY]
        required: true
        column: csa_type
      - name: csa-version
        type: string
        column: csa_version
      - name: our-threshold
        type: decimal
        default: 0
        column: our_threshold
      - name: their-threshold
        type: decimal
        default: 0
        column: their_threshold
      - name: threshold-ccy
        type: string
        default: USD
        column: our_threshold_ccy
      - name: mta
        type: decimal
        default: 500000
        column: minimum_transfer_amount
      - name: mta-ccy
        type: string
        default: USD
        column: mta_ccy
      - name: valuation-agent
        type: enum
        values: [US, THEM, CALCULATION_AGENT]
        default: US
        column: valuation_agent
      - name: interest-benchmark
        type: enum
        values: [SOFR, ESTR, SONIA, TONAR, FED_FUNDS]
        column: interest_rate_benchmark
      - name: rehypothecation
        type: boolean
        default: false
        column: rehypothecation_permitted
    
    binding: csa_id

  - verb: add-eligible-collateral
    description: "Add eligible collateral type to CSA"
    entity_type: csa_eligible_collateral
    operation: upsert
    table: csa_eligible_collateral
    unique_on: [csa_id, asset_class, currency, issuer_jurisdiction]
    
    parameters:
      - name: csa-id
        type: reference
        entity_type: csa
        required: true
        column: csa_id
      - name: asset-class
        type: enum
        values: [CASH, GOVT_BOND, AGENCY, CORP_BOND, EQUITY, GOLD]
        required: true
        column: asset_class
      - name: currency
        type: string
      - name: issuer-jurisdiction
        type: string
        column: issuer_jurisdiction
      - name: min-rating
        type: enum
        values: [AAA, AA, A, BBB]
        column: min_rating
      - name: max-maturity-years
        type: integer
        column: max_maturity_years
      - name: haircut-pct
        type: decimal
        required: true
        column: haircut_pct
      - name: fx-haircut-pct
        type: decimal
        default: 8.0
        column: fx_haircut_pct
      - name: priority
        type: integer
        default: 100

  - verb: set-im-terms
    description: "Set Initial Margin specific terms"
    entity_type: csa
    operation: update
    table: credit_support_annex
    
    parameters:
      - name: csa-id
        type: reference
        entity_type: csa
        required: true
      - name: our-independent-amount
        type: decimal
        column: our_independent_amount
      - name: their-independent-amount
        type: decimal
        column: their_independent_amount
      - name: ia-ccy
        type: string
        column: ia_ccy
```

### Task 3.5.2.4: Collateral Account Verbs
**File:** `rust/config/verbs/collateral.yaml`

```yaml
domain: collateral
description: "Collateral account and position management"

verbs:
  - verb: ensure-account
    description: "Ensure collateral account exists"
    entity_type: collateral_account
    operation: upsert
    table: collateral_account
    unique_on: [cbu_id, csa_id, account_type]
    
    parameters:
      - name: cbu-id
        type: reference
        entity_type: cbu
        required: true
        column: cbu_id
      - name: csa-id
        type: reference
        entity_type: csa
        column: csa_id
      - name: account-name
        type: string
        required: true
        column: account_name
      - name: account-type
        type: enum
        values: [POSTED_VM, RECEIVED_VM, POSTED_IM, RECEIVED_IM, TRI_PARTY]
        required: true
        column: account_type
      - name: custodian-bic
        type: string
        column: custodian_bic
      - name: custodian-name
        type: string
        column: custodian_name
      - name: is-third-party
        type: boolean
        default: false
        column: is_third_party_custodian
      - name: is-segregated
        type: boolean
        default: false
        column: is_segregated
      - name: segregation-type
        type: enum
        values: [INDIVIDUAL, OMNIBUS]
        column: segregation_type
    
    binding: collateral_account_id

  - verb: list-accounts
    description: "List collateral accounts"
    operation: query
    table: collateral_account
    
    parameters:
      - name: cbu-id
        type: reference
        entity_type: cbu
        required: true
      - name: csa-id
        type: reference
        entity_type: csa
      - name: account-type
        type: enum
        values: [POSTED_VM, RECEIVED_VM, POSTED_IM, RECEIVED_IM, TRI_PARTY]
```

### Task 3.5.2.5: Confirmation Configuration Verbs
**File:** `rust/config/verbs/confirmation.yaml`

```yaml
domain: confirmation
description: "Trade confirmation workflow configuration"

verbs:
  - verb: configure
    description: "Configure confirmation method"
    entity_type: confirmation_config
    operation: upsert
    table: confirmation_config
    unique_on: [cbu_id, counterparty_id, product_type]
    
    parameters:
      - name: cbu-id
        type: reference
        entity_type: cbu
        required: true
        column: cbu_id
      - name: counterparty-id
        type: reference
        entity_type: counterparty
        column: counterparty_id
        description: "NULL for default"
      - name: product-type
        type: enum
        values: [IRS, XCCY, CDS, FX_FORWARD, FX_OPTION, SWAPTION]
        column: product_type
        description: "NULL for all products"
      - name: method
        type: enum
        values: [DTCC_GTR, MARKITWIRE, SWIFT_MT300, SWIFT_MT360, PAPER, EMAIL]
        required: true
        column: confirmation_method
      - name: platform-id
        type: string
        column: platform_id
      - name: our-platform-id
        type: string
        column: our_platform_id
      - name: auto-match
        type: boolean
        default: true
        column: auto_match_enabled
      - name: match-tolerance-bps
        type: decimal
        default: 1.0
        column: match_tolerance_bps
      - name: target-days
        type: integer
        default: 1
        column: target_confirmation_days

  - verb: list
    description: "List confirmation configurations"
    operation: query
    table: confirmation_config
    
    parameters:
      - name: cbu-id
        type: reference
        entity_type: cbu
        required: true
      - name: counterparty-id
        type: reference
        entity_type: counterparty
```

---

## Phase 3.5.3: Agent Intent Taxonomy Extension

### Task 3.5.3.1: OTC Derivatives Intent Taxonomy
**File:** `rust/config/agent/intent_taxonomy.yaml` (extend)

```yaml
# Add to intent_taxonomy.yaml under intent_taxonomy:

  # ==========================================================================
  # OTC DERIVATIVES DOMAIN
  # ==========================================================================
  otc_derivatives:
    description: "OTC derivatives trading setup and collateral management"

    # ------------------------------------------------------------------------
    # Counterparty Management Sub-domain
    # ------------------------------------------------------------------------
    counterparty:
      description: "Derivatives counterparty management"

      intents:
        - intent: counterparty_create
          description: "Create or onboard a derivatives counterparty"
          canonical_verb: counterparty.ensure
          trigger_phrases:
            - "add {counterparty} as counterparty"
            - "onboard {counterparty} for derivatives"
            - "set up {counterparty} as trading counterparty"
            - "we trade derivatives with {counterparty}"
            - "{counterparty} is our swap counterparty"
            - "create counterparty {counterparty}"
          required_entities:
            - counterparty_name_or_lei
          optional_entities:
            - counterparty_type
            - jurisdiction
          default_inferences:
            counterparty_type: BANK
            is_financial_counterparty: true
          examples:
            - input: "Add Goldman Sachs as counterparty for derivatives"
              entities:
                counterparty_name: "Goldman Sachs"
                counterparty_type: "BANK"

        - intent: counterparty_query
          description: "Query counterparties"
          canonical_verb: counterparty.list
          trigger_phrases:
            - "who are our swap counterparties"
            - "list counterparties"
            - "show derivatives counterparties"
            - "who do we trade with"
          is_query: true

    # ------------------------------------------------------------------------
    # ISDA Management Sub-domain
    # ------------------------------------------------------------------------
    isda:
      description: "ISDA Master Agreement management"

      intents:
        - intent: isda_establish
          description: "Establish ISDA Master Agreement"
          canonical_verb: isda.establish
          trigger_phrases:
            - "establish ISDA with {counterparty}"
            - "set up ISDA master with {counterparty}"
            - "we have an ISDA with {counterparty}"
            - "create ISDA for {counterparty}"
            - "2002 ISDA with {counterparty}"
            - "link ISDA to {counterparty}"
          required_entities:
            - counterparty_reference
          optional_entities:
            - isda_version
            - governing_law
            - agreement_date
          default_inferences:
            isda_version: "2002"
            governing_law: NY
            netting_applicable: true
          examples:
            - input: "Establish 2002 ISDA with Goldman under NY law"
              entities:
                counterparty_reference: "Goldman Sachs"
                isda_version: "2002"
                governing_law: "NY"

        - intent: isda_add_products
          description: "Add products to ISDA scope"
          canonical_verb: isda.add-product-scope
          trigger_phrases:
            - "add {products} to the ISDA"
            - "ISDA covers {products}"
            - "include {products} under ISDA"
            - "trade {products} under {counterparty} ISDA"
          required_entities:
            - isda_reference
            - derivative_product_types
          examples:
            - input: "Add IRS and CDS to the Goldman ISDA"
              entities:
                isda_reference: "@isda-goldman"
                derivative_product_types: ["IRS", "CDS"]

        - intent: isda_query
          description: "Query ISDA agreements"
          canonical_verb: isda.query-by-counterparty
          trigger_phrases:
            - "do we have an ISDA with {counterparty}"
            - "show ISDA for {counterparty}"
            - "what's our ISDA status with {counterparty}"
            - "list our ISDAs"
          is_query: true

    # ------------------------------------------------------------------------
    # CSA Management Sub-domain
    # ------------------------------------------------------------------------
    csa:
      description: "Credit Support Annex and collateral terms"

      intents:
        - intent: csa_establish
          description: "Establish CSA under ISDA"
          canonical_verb: csa.establish
          trigger_phrases:
            - "set up CSA with {counterparty}"
            - "establish VM CSA under {isda}"
            - "add CSA to ISDA"
            - "create collateral agreement with {counterparty}"
            - "we have a CSA with {counterparty}"
            - "collateral terms with {counterparty}"
          required_entities:
            - isda_reference
          optional_entities:
            - csa_type
            - threshold_amount
            - mta
          default_inferences:
            csa_type: VM
            our_threshold: 0
            their_threshold: 0
            mta: 500000
          examples:
            - input: "Set up VM CSA with Goldman, zero threshold both ways"
              entities:
                counterparty_reference: "Goldman Sachs"
                csa_type: "VM"
                our_threshold: 0
                their_threshold: 0

        - intent: csa_add_eligible_collateral
          description: "Add eligible collateral to CSA"
          canonical_verb: csa.add-eligible-collateral
          trigger_phrases:
            - "accept {collateral_type} as collateral"
            - "add {collateral_type} to eligible collateral"
            - "{collateral_type} with {haircut}% haircut"
            - "we can post {collateral_type}"
            - "eligible collateral includes {collateral_type}"
          required_entities:
            - csa_reference
            - collateral_asset_class
            - haircut_percentage
          examples:
            - input: "Accept USD cash and US treasuries with 2% haircut"
              entities:
                collateral_asset_class: ["CASH", "GOVT_BOND"]
                currency: "USD"
                issuer_jurisdiction: "US"
                haircut_percentage: 2.0

        - intent: csa_set_im_terms
          description: "Configure Initial Margin terms"
          canonical_verb: csa.set-im-terms
          trigger_phrases:
            - "set up IM CSA"
            - "initial margin terms"
            - "configure IM with {counterparty}"
            - "post IM to {custodian}"
            - "IM segregation at {custodian}"
          required_entities:
            - csa_reference
          optional_entities:
            - im_custodian
            - our_independent_amount
          examples:
            - input: "Set up IM CSA with BNY as custodian"
              entities:
                im_custodian: "BNY Mellon"

        - intent: csa_query
          description: "Query CSA terms"
          canonical_verb: csa.list
          trigger_phrases:
            - "what are our collateral terms with {counterparty}"
            - "show CSA for {counterparty}"
            - "what collateral can we post"
            - "what's the threshold with {counterparty}"
          is_query: true

    # ------------------------------------------------------------------------
    # Collateral Operations Sub-domain
    # ------------------------------------------------------------------------
    collateral:
      description: "Collateral account and position management"

      intents:
        - intent: collateral_setup_accounts
          description: "Set up collateral accounts"
          canonical_verb: collateral.ensure-account
          trigger_phrases:
            - "set up collateral accounts for {csa}"
            - "create VM account with {counterparty}"
            - "establish IM segregated account"
            - "collateral account at {custodian}"
            - "tri-party account with {agent}"
          required_entities:
            - csa_reference
            - account_type
          optional_entities:
            - custodian_reference
          examples:
            - input: "Set up tri-party IM account at BNY"
              entities:
                account_type: "TRI_PARTY"
                custodian_reference: "BKNYUS33"

        - intent: collateral_query_accounts
          description: "Query collateral accounts"
          canonical_verb: collateral.list-accounts
          trigger_phrases:
            - "show collateral accounts"
            - "list our collateral positions"
            - "where is our collateral"
            - "collateral account status"
          is_query: true

    # ------------------------------------------------------------------------
    # Confirmation Workflow Sub-domain  
    # ------------------------------------------------------------------------
    confirmation:
      description: "Trade confirmation configuration"

      intents:
        - intent: confirmation_configure
          description: "Configure confirmation method"
          canonical_verb: confirmation.configure
          trigger_phrases:
            - "use {platform} for confirmations with {counterparty}"
            - "confirm {products} via {platform}"
            - "set up {platform} for {counterparty}"
            - "confirmation method is {platform}"
            - "{products} confirms through {platform}"
          required_entities:
            - confirmation_method
          optional_entities:
            - counterparty_reference
            - derivative_product_types
          default_inferences:
            auto_match: true
            target_days: 1
          examples:
            - input: "Use MarkitWire for IRS confirmations with Goldman"
              entities:
                confirmation_method: "MARKITWIRE"
                derivative_product_types: ["IRS"]
                counterparty_reference: "Goldman Sachs"

        - intent: confirmation_query
          description: "Query confirmation setup"
          canonical_verb: confirmation.list
          trigger_phrases:
            - "how do we confirm with {counterparty}"
            - "show confirmation methods"
            - "confirmation status"
          is_query: true

    # ------------------------------------------------------------------------
    # Compound OTC Intents
    # ------------------------------------------------------------------------
    compound_otc:
      description: "Multi-step OTC setup"

      intents:
        - intent: full_counterparty_setup
          description: "Complete counterparty onboarding"
          expands_to:
            - counterparty_create
            - isda_establish
            - isda_add_products
            - csa_establish
            - csa_add_eligible_collateral
            - confirmation_configure
          trigger_phrases:
            - "set up complete derivatives relationship with {counterparty}"
            - "full counterparty onboarding for {counterparty}"
            - "establish trading with {counterparty}"
          examples:
            - input: "Set up complete derivatives relationship with Goldman"

        - intent: im_setup
          description: "Complete IM setup"
          expands_to:
            - csa_establish  # IM type
            - csa_add_eligible_collateral
            - collateral_setup_accounts  # segregated
          trigger_phrases:
            - "set up initial margin with {counterparty}"
            - "IM setup for {counterparty}"
            - "configure UMR compliance with {counterparty}"
```

### Task 3.5.3.2: OTC Entity Types
**File:** `rust/config/agent/entity_types.yaml` (extend)

```yaml
# Add to entity_types.yaml

  # ------------------------------------------------------------------------
  # Counterparty Reference
  # ------------------------------------------------------------------------
  counterparty_reference:
    description: "Reference to derivatives counterparty"
    category: "entity"
    patterns:
      - type: NAME
        description: "Counterparty name"
        examples:
          - "Goldman Sachs"
          - "JP Morgan"
          - "Citi"
          - "Morgan Stanley"
          - "Bank of America"
          - "Deutsche Bank"
          - "Barclays"
          - "Credit Suisse"
          - "UBS"
          - "BNP Paribas"
          - "Societe Generale"
          - "HSBC"
        fuzzy_match: true

      - type: LEI
        description: "Legal Entity Identifier"
        regex: "[A-Z0-9]{20}"
        validation: lei_checksum

      - type: SHORT_NAME
        mappings:
          "GS": "Goldman Sachs"
          "JPM": "JP Morgan"
          "MS": "Morgan Stanley"
          "DB": "Deutsche Bank"
          "Barclays": "Barclays"
          "BNPP": "BNP Paribas"
          "SocGen": "Societe Generale"
          "CS": "Credit Suisse"

      - type: SYMBOL
        regex: "@cp-[a-z][a-z0-9-]*"
        examples:
          - "@cp-goldman"
          - "@cp-jpm"

    normalization:
      lookup_table: counterparties
      fuzzy_match: true
      fuzzy_threshold: 0.8

  # ------------------------------------------------------------------------
  # ISDA Reference
  # ------------------------------------------------------------------------
  isda_reference:
    description: "Reference to ISDA Master Agreement"
    category: "entity"
    patterns:
      - type: SYMBOL
        regex: "@isda-[a-z][a-z0-9-]*"
        examples:
          - "@isda-goldman"
          - "@isda-jpm"

      - type: COUNTERPARTY_CONTEXT
        description: "Resolve from counterparty in context"
        examples:
          - "the ISDA"
          - "our ISDA"
          - "Goldman's ISDA"
        requires_context: true

  # ------------------------------------------------------------------------
  # CSA Reference
  # ------------------------------------------------------------------------
  csa_reference:
    description: "Reference to Credit Support Annex"
    category: "entity"
    patterns:
      - type: SYMBOL
        regex: "@csa-[a-z][a-z0-9-]*"

      - type: CONTEXT
        examples:
          - "the CSA"
          - "our CSA"
          - "the VM CSA"
          - "the IM CSA"
        requires_context: true

  # ------------------------------------------------------------------------
  # Derivative Product Types
  # ------------------------------------------------------------------------
  derivative_product_type:
    description: "OTC derivative product type"
    category: "reference_data"
    patterns:
      - type: CODE
        valid_values:
          - IRS
          - XCCY
          - CDS
          - FX_FORWARD
          - FX_OPTION
          - SWAPTION
          - EQUITY_SWAP
          - COMMODITY_SWAP
          - REPO
          - TRS

      - type: NAME
        mappings:
          "interest rate swap": "IRS"
          "interest rate swaps": "IRS"
          "rate swap": "IRS"
          "rate swaps": "IRS"
          "swaps": "IRS"
          "cross currency": "XCCY"
          "cross-currency swap": "XCCY"
          "xccy swap": "XCCY"
          "credit default swap": "CDS"
          "credit default swaps": "CDS"
          "credit swaps": "CDS"
          "CDS": "CDS"
          "FX forward": "FX_FORWARD"
          "FX forwards": "FX_FORWARD"
          "currency forward": "FX_FORWARD"
          "FX option": "FX_OPTION"
          "FX options": "FX_OPTION"
          "currency option": "FX_OPTION"
          "swaption": "SWAPTION"
          "swaptions": "SWAPTION"
          "equity swap": "EQUITY_SWAP"
          "total return swap": "TRS"
          "TRS": "TRS"
          "repo": "REPO"
          "repurchase agreement": "REPO"

      - type: CATEGORY
        description: "Product category (expands to types)"
        mappings:
          "rates": ["IRS", "XCCY", "SWAPTION"]
          "rate derivatives": ["IRS", "XCCY", "SWAPTION"]
          "credit": ["CDS"]
          "credit derivatives": ["CDS"]
          "FX": ["FX_FORWARD", "FX_OPTION"]
          "FX derivatives": ["FX_FORWARD", "FX_OPTION"]
          "all OTC": ["IRS", "XCCY", "CDS", "FX_FORWARD", "FX_OPTION", "SWAPTION"]

    normalization:
      uppercase: true

  # ------------------------------------------------------------------------
  # ISDA Version
  # ------------------------------------------------------------------------
  isda_version:
    description: "ISDA Master Agreement version"
    category: "reference_data"
    patterns:
      - type: YEAR
        valid_values: ["2002", "1992"]
        mappings:
          "2002 ISDA": "2002"
          "1992 ISDA": "1992"
          "old ISDA": "1992"
          "current": "2002"
          "standard": "2002"

    default: "2002"

  # ------------------------------------------------------------------------
  # CSA Type
  # ------------------------------------------------------------------------
  csa_type:
    description: "Type of Credit Support Annex"
    category: "reference_data"
    patterns:
      - type: CODE
        valid_values: [VM, IM, LEGACY]
        mappings:
          "variation margin": "VM"
          "VM CSA": "VM"
          "initial margin": "IM"
          "IM CSA": "IM"
          "SIMM": "IM"
          "legacy": "LEGACY"
          "old style": "LEGACY"
          "1995 CSA": "LEGACY"

  # ------------------------------------------------------------------------
  # Collateral Asset Class
  # ------------------------------------------------------------------------
  collateral_asset_class:
    description: "Asset class for collateral"
    category: "reference_data"
    patterns:
      - type: CODE
        valid_values: [CASH, GOVT_BOND, AGENCY, CORP_BOND, EQUITY, GOLD]

      - type: NAME
        mappings:
          "cash": "CASH"
          "government bonds": "GOVT_BOND"
          "govvies": "GOVT_BOND"
          "treasuries": "GOVT_BOND"
          "gilts": "GOVT_BOND"
          "bunds": "GOVT_BOND"
          "JGBs": "GOVT_BOND"
          "agency": "AGENCY"
          "agency bonds": "AGENCY"
          "agencies": "AGENCY"
          "corporate bonds": "CORP_BOND"
          "corporates": "CORP_BOND"
          "equity": "EQUITY"
          "stocks": "EQUITY"
          "gold": "GOLD"

  # ------------------------------------------------------------------------
  # Confirmation Method
  # ------------------------------------------------------------------------
  confirmation_method:
    description: "Trade confirmation method/platform"
    category: "reference_data"
    patterns:
      - type: CODE
        valid_values: [DTCC_GTR, MARKITWIRE, SWIFT_MT300, SWIFT_MT360, PAPER, EMAIL]

      - type: NAME
        mappings:
          "DTCC": "DTCC_GTR"
          "GTR": "DTCC_GTR"
          "trade repository": "DTCC_GTR"
          "MarkitWire": "MARKITWIRE"
          "Markit": "MARKITWIRE"
          "SWIFT": "SWIFT_MT360"
          "MT300": "SWIFT_MT300"
          "MT360": "SWIFT_MT360"
          "paper": "PAPER"
          "long form": "PAPER"
          "email": "EMAIL"
          "electronic": "MARKITWIRE"

  # ------------------------------------------------------------------------
  # Haircut Percentage
  # ------------------------------------------------------------------------
  haircut_percentage:
    description: "Collateral haircut percentage"
    category: "numeric"
    patterns:
      - type: PERCENTAGE
        regex: "(\\d+(?:\\.\\d+)?)\\s*%?"
        examples:
          - "2%"
          - "5"
          - "0.5%"
          - "15%"

    normalization:
      type: decimal
      max: 100
      min: 0

  # ------------------------------------------------------------------------
  # Threshold Amount
  # ------------------------------------------------------------------------
  threshold_amount:
    description: "Collateral threshold amount"
    category: "numeric"
    patterns:
      - type: AMOUNT
        examples:
          - "10 million"
          - "10mm"
          - "0"
          - "zero"
          - "nil"
        mappings:
          "zero": 0
          "nil": 0
          "none": 0

    normalization:
      type: decimal
```

### Task 3.5.3.3: OTC Parameter Mappings
**File:** `rust/config/agent/parameter_mappings.yaml` (extend)

```yaml
# Add to parameter_mappings.yaml

  # ==========================================================================
  # Counterparty Verbs
  # ==========================================================================
  counterparty.ensure:
    description: "Create or update counterparty"
    mappings:
      - entity_type: counterparty_reference
        param: name
        required: true
      - entity_type: lei
        param: lei
      - entity_type: counterparty_type
        param: counterparty-type
        default_if_missing: BANK
      - entity_type: jurisdiction
        param: jurisdiction
        default_if_missing: US
      - entity_type: is_financial_counterparty
        param: is-financial-counterparty
        default_if_missing: true
    symbol_template: "@cp-{name}"
    symbol_transform: lowercase_hyphenate

  counterparty.list:
    description: "List counterparties"
    mappings:
      - entity_type: cbu_reference
        param: cbu-id
        source: context
        fallback: session.current_cbu
    is_query: true

  # ==========================================================================
  # ISDA Verbs
  # ==========================================================================
  isda.establish:
    description: "Establish ISDA Master"
    mappings:
      - entity_type: cbu_reference
        param: cbu-id
        source: context
        fallback: session.current_cbu
      - entity_type: counterparty_reference
        param: counterparty-id
        required: true
      - entity_type: isda_version
        param: version
        default_if_missing: "2002"
      - entity_type: governing_law
        param: governing-law
        default_if_missing: NY
      - entity_type: date
        param: agreement-date
        default_if_missing: TODAY
    defaults:
      netting-applicable: true
      cross-default-applicable: true
    symbol_template: "@isda-{counterparty-id}"
    symbol_transform: lowercase_hyphenate

  isda.add-product-scope:
    description: "Add products to ISDA"
    mappings:
      - entity_type: isda_reference
        param: isda-id
        required: true
        source: context_or_extract
      - entity_type: derivative_product_type
        param: product-type
        required: true
        is_list: true
        iterate_if_list: true
    defaults:
      included: true

  # ==========================================================================
  # CSA Verbs
  # ==========================================================================
  csa.establish:
    description: "Establish CSA"
    mappings:
      - entity_type: isda_reference
        param: isda-id
        required: true
        source: context_or_extract
      - entity_type: csa_type
        param: csa-type
        required: true
        default_if_missing: VM
      - entity_type: threshold_amount
        param: our-threshold
        context_key: our
        default_if_missing: 0
      - entity_type: threshold_amount
        param: their-threshold
        context_key: their
        default_if_missing: 0
      - entity_type: currency
        param: threshold-ccy
        default_if_missing: USD
      - entity_type: amount
        param: mta
        default_if_missing: 500000
      - entity_type: interest_benchmark
        param: interest-benchmark
        infer_from: threshold_ccy
        inference_rules:
          USD: SOFR
          EUR: ESTR
          GBP: SONIA
          JPY: TONAR
          default: SOFR
    symbol_template: "@csa-{isda-id}"
    symbol_transform: lowercase_hyphenate

  csa.add-eligible-collateral:
    description: "Add eligible collateral"
    mappings:
      - entity_type: csa_reference
        param: csa-id
        required: true
        source: context_or_extract
      - entity_type: collateral_asset_class
        param: asset-class
        required: true
        is_list: true
        iterate_if_list: true
      - entity_type: currency
        param: currency
      - entity_type: jurisdiction
        param: issuer-jurisdiction
      - entity_type: credit_rating
        param: min-rating
      - entity_type: haircut_percentage
        param: haircut-pct
        required: true
        default_if_missing: 0
      - entity_type: haircut_percentage
        param: fx-haircut-pct
        context_key: fx
        default_if_missing: 8.0

  # ==========================================================================
  # Collateral Verbs
  # ==========================================================================
  collateral.ensure-account:
    description: "Ensure collateral account"
    mappings:
      - entity_type: cbu_reference
        param: cbu-id
        source: context
        fallback: session.current_cbu
      - entity_type: csa_reference
        param: csa-id
        required: true
        source: context_or_extract
      - entity_type: account_name
        param: account-name
        required: true
      - entity_type: collateral_account_type
        param: account-type
        required: true
      - entity_type: bic
        param: custodian-bic
      - entity_type: custodian_name
        param: custodian-name
      - entity_type: is_third_party
        param: is-third-party
        infer_from: account_type
        inference_rules:
          POSTED_IM: true
          RECEIVED_IM: true
          TRI_PARTY: true
          default: false
      - entity_type: is_segregated
        param: is-segregated
        infer_from: account_type
        inference_rules:
          POSTED_IM: true
          RECEIVED_IM: true
          default: false
    symbol_template: "@collat-{account-type}"
    symbol_transform: lowercase_hyphenate

  # ==========================================================================
  # Confirmation Verbs
  # ==========================================================================
  confirmation.configure:
    description: "Configure confirmation method"
    mappings:
      - entity_type: cbu_reference
        param: cbu-id
        source: context
        fallback: session.current_cbu
      - entity_type: counterparty_reference
        param: counterparty-id
      - entity_type: derivative_product_type
        param: product-type
        is_list: true
        iterate_if_list: true
      - entity_type: confirmation_method
        param: method
        required: true
      - entity_type: platform_id
        param: platform-id
      - entity_type: auto_match
        param: auto-match
        default_if_missing: true
      - entity_type: target_days
        param: target-days
        default_if_missing: 1
```

---

## Phase 3.5.4: Reference Data

### Task 3.5.4.1: Counterparty Seed Data
**File:** `rust/config/seed/reference_data/counterparties.yaml`

```yaml
version: "1.0"
description: "Major derivatives counterparties"

counterparties:
  # US Banks
  - name: Goldman Sachs
    short_name: GS
    lei: 784F5XWPLTWKTBV3E584
    counterparty_type: BANK
    jurisdiction: US
    bics:
      - bic: GABORUSMXXX
        type: SWIFT
      - bic: GSCO
        type: DTCC
    is_financial_counterparty: true
    is_covered_entity: true

  - name: JP Morgan
    short_name: JPM
    lei: 8I5DZWZKVSZI1NUHU748
    counterparty_type: BANK
    jurisdiction: US
    bics:
      - bic: CHASUS33XXX
        type: SWIFT
      - bic: JPMC
        type: DTCC
    is_financial_counterparty: true
    is_covered_entity: true

  - name: Morgan Stanley
    short_name: MS
    lei: IGJSJL3JD5P30I6NJZ34
    counterparty_type: BANK
    jurisdiction: US
    bics:
      - bic: MSTCUS44XXX
        type: SWIFT
    is_financial_counterparty: true
    is_covered_entity: true

  - name: Citibank
    short_name: Citi
    lei: E57ODZWZ7FF32TWEFA76
    counterparty_type: BANK
    jurisdiction: US
    bics:
      - bic: CITIUS33XXX
        type: SWIFT
    is_financial_counterparty: true
    is_covered_entity: true

  - name: Bank of America
    short_name: BofA
    lei: 9DJT3UXIJIZJI4WXO774
    counterparty_type: BANK
    jurisdiction: US
    bics:
      - bic: BABORUMCXXX
        type: SWIFT
    is_financial_counterparty: true
    is_covered_entity: true

  # European Banks
  - name: Deutsche Bank
    short_name: DB
    lei: 7LTWFZYICNSX8D621K86
    counterparty_type: BANK
    jurisdiction: DE
    bics:
      - bic: DEUTDEFFXXX
        type: SWIFT
    is_financial_counterparty: true
    is_covered_entity: true

  - name: Barclays
    short_name: Barclays
    lei: G5GSEF7VJP5I7OUK5573
    counterparty_type: BANK
    jurisdiction: GB
    bics:
      - bic: BABORPLPXXX
        type: SWIFT
    is_financial_counterparty: true
    is_covered_entity: true

  - name: BNP Paribas
    short_name: BNPP
    lei: R0MUWSFPU8MPRO8K5P83
    counterparty_type: BANK
    jurisdiction: FR
    bics:
      - bic: BNPABORPXXX
        type: SWIFT
    is_financial_counterparty: true
    is_covered_entity: true

  - name: UBS
    short_name: UBS
    lei: BFM8T61CT2L1QCEMIK50
    counterparty_type: BANK
    jurisdiction: CH
    bics:
      - bic: UBSWCHZH80A
        type: SWIFT
    is_financial_counterparty: true
    is_covered_entity: true

  # Asian Banks
  - name: Nomura
    short_name: Nomura
    lei: 353800GXC94LB13DCQ82
    counterparty_type: BANK
    jurisdiction: JP
    bics:
      - bic: NOMUUS33XXX
        type: SWIFT
    is_financial_counterparty: true
    is_covered_entity: true
```

### Task 3.5.4.2: Standard CSA Terms
**File:** `rust/config/seed/reference_data/csa_templates.yaml`

```yaml
version: "1.0"
description: "Standard CSA configurations"

csa_templates:
  # Zero threshold bilateral VM
  - name: zero_threshold_vm
    description: "Standard bilateral VM CSA with zero thresholds"
    csa_type: VM
    csa_version: "2016 VM"
    our_threshold: 0
    their_threshold: 0
    mta: 500000
    mta_ccy: USD
    valuation_agent: CALCULATION_AGENT
    eligible_collateral:
      - asset_class: CASH
        currencies: [USD, EUR, GBP, JPY, CHF]
        haircut_pct: 0
      - asset_class: GOVT_BOND
        issuers: [US, DE, GB, FR, JP]
        max_maturity_years: 10
        min_rating: AA
        haircut_pct: 2.0
      - asset_class: GOVT_BOND
        issuers: [US, DE, GB, FR, JP]
        max_maturity_years: 30
        min_rating: AA
        haircut_pct: 4.0
    interest_benchmarks:
      USD: SOFR
      EUR: ESTR
      GBP: SONIA
      JPY: TONAR
      CHF: SARON

  # Standard IM CSA for UMR
  - name: standard_im
    description: "Standard IM CSA for UMR compliance"
    csa_type: IM
    csa_version: "2018 IM"
    segregation_required: true
    third_party_custodian_required: true
    eligible_collateral:
      - asset_class: CASH
        currencies: [USD, EUR, GBP, JPY]
        haircut_pct: 0
      - asset_class: GOVT_BOND
        issuers: [US, DE, GB, FR, JP, CA, AU]
        max_maturity_years: 1
        min_rating: AA
        haircut_pct: 0.5
      - asset_class: GOVT_BOND
        issuers: [US, DE, GB, FR, JP, CA, AU]
        max_maturity_years: 5
        min_rating: AA
        haircut_pct: 2.0
      - asset_class: GOVT_BOND
        issuers: [US, DE, GB, FR, JP, CA, AU]
        max_maturity_years: 10
        min_rating: AA
        haircut_pct: 4.0
    fx_haircut_pct: 8.0
```

### Task 3.5.4.3: Confirmation Platform Reference
**File:** `rust/config/seed/reference_data/confirmation_platforms.yaml`

```yaml
version: "1.0"
description: "Trade confirmation platforms"

platforms:
  - code: DTCC_GTR
    name: DTCC Global Trade Repository
    platform_type: REPOSITORY
    products: [IRS, CDS, FX_FORWARD, EQUITY_SWAP]
    jurisdictions: [US, EU, APAC]
    stp_capable: true
    regulatory_reporting: true

  - code: MARKITWIRE
    name: MarkitWire
    platform_type: AFFIRMATION
    products: [IRS, XCCY, CDS, SWAPTION]
    jurisdictions: [GLOBAL]
    stp_capable: true
    owned_by: IHS Markit

  - code: BLOOMBERG_VCON
    name: Bloomberg VCON
    platform_type: AFFIRMATION
    products: [IRS, XCCY, FX_FORWARD, FX_OPTION]
    jurisdictions: [GLOBAL]
    stp_capable: true

  - code: SWIFT_FIN
    name: SWIFT FIN Messages
    platform_type: MESSAGING
    products: [FX_FORWARD, FX_OPTION]
    message_types:
      FX_FORWARD: MT300
      FX_OPTION: MT305
      IRS: MT360
    jurisdictions: [GLOBAL]
```

---

## Phase 3.5.5: Evaluation Dataset Extension

### Task 3.5.5.1: OTC Evaluation Cases
**File:** `rust/config/agent/evaluation_dataset.yaml` (extend)

```yaml
# Add to evaluation_cases:

  # ==================== Counterparty Tests ====================
  
  - id: cp_simple_1
    category: counterparty
    difficulty: easy
    input: "Add Goldman Sachs as counterparty"
    expected_intents:
      - counterparty_create
    expected_entities:
      counterparty_reference: "Goldman Sachs"
    expected_dsl_contains:
      - "counterparty.ensure"
      - "Goldman Sachs"

  - id: cp_with_lei
    category: counterparty
    difficulty: medium
    input: "Onboard JP Morgan LEI 8I5DZWZKVSZI1NUHU748 for derivatives"
    expected_intents:
      - counterparty_create
    expected_entities:
      counterparty_reference: "JP Morgan"
      lei: "8I5DZWZKVSZI1NUHU748"
    expected_dsl_contains:
      - "counterparty.ensure"
      - "8I5DZWZKVSZI1NUHU748"

  # ==================== ISDA Tests ====================

  - id: isda_simple_1
    category: isda
    difficulty: easy
    input: "Establish ISDA with Goldman"
    expected_intents:
      - isda_establish
    expected_entities:
      counterparty_reference: "Goldman Sachs"
    expected_dsl_contains:
      - "isda.establish"

  - id: isda_with_details
    category: isda
    difficulty: medium
    input: "Set up 2002 ISDA with Deutsche Bank under English law"
    expected_intents:
      - isda_establish
    expected_entities:
      counterparty_reference: "Deutsche Bank"
      isda_version: "2002"
      governing_law: "ENGLISH"
    expected_dsl_contains:
      - "isda.establish"
      - "2002"
      - "ENGLISH"

  - id: isda_add_products
    category: isda
    difficulty: medium
    input: "Add IRS, CDS and FX to our Goldman ISDA"
    expected_intents:
      - isda_add_products
    expected_entities:
      counterparty_reference: "Goldman Sachs"
      derivative_product_type: ["IRS", "CDS", "FX_FORWARD"]
    expected_dsl_contains:
      - "isda.add-product-scope"
      - "IRS"
      - "CDS"

  # ==================== CSA Tests ====================

  - id: csa_simple_1
    category: csa
    difficulty: easy
    input: "Set up CSA with Goldman"
    expected_intents:
      - csa_establish
    expected_dsl_contains:
      - "csa.establish"

  - id: csa_zero_threshold
    category: csa
    difficulty: medium
    input: "Establish VM CSA with JP Morgan, zero threshold both ways"
    expected_intents:
      - csa_establish
    expected_entities:
      counterparty_reference: "JP Morgan"
      csa_type: "VM"
      threshold_amount: 0
    expected_dsl_contains:
      - "csa.establish"
      - "VM"

  - id: csa_add_collateral
    category: csa
    difficulty: medium
    input: "Accept USD cash and US treasuries with 2% haircut"
    expected_intents:
      - csa_add_eligible_collateral
    expected_entities:
      collateral_asset_class: ["CASH", "GOVT_BOND"]
      currency: "USD"
      haircut_percentage: 2.0
    expected_dsl_contains:
      - "csa.add-eligible-collateral"
      - "CASH"
      - "GOVT_BOND"

  - id: csa_im_setup
    category: csa
    difficulty: hard
    input: "Set up IM CSA with Morgan Stanley, BNY as tri-party custodian"
    expected_intents:
      - csa_establish
      - collateral_setup_accounts
    expected_entities:
      counterparty_reference: "Morgan Stanley"
      csa_type: "IM"
      custodian_reference: "BNY"
    expected_dsl_contains:
      - "csa.establish"
      - "IM"

  # ==================== Confirmation Tests ====================

  - id: confirm_simple_1
    category: confirmation
    difficulty: easy
    input: "Use MarkitWire for confirmations"
    expected_intents:
      - confirmation_configure
    expected_entities:
      confirmation_method: "MARKITWIRE"
    expected_dsl_contains:
      - "confirmation.configure"
      - "MARKITWIRE"

  - id: confirm_by_product
    category: confirmation
    difficulty: medium
    input: "Confirm IRS via MarkitWire, FX via SWIFT"
    expected_intents:
      - confirmation_configure
      - confirmation_configure
    expected_dsl_statements: 2

  # ==================== Compound OTC Tests ====================

  - id: otc_full_setup
    category: compound_otc
    difficulty: hard
    input: |
      Set up derivatives trading with Goldman Sachs:
      2002 ISDA under NY law,
      VM CSA with zero thresholds,
      accepting USD cash and US treasuries,
      confirm via MarkitWire
    expected_intents:
      - counterparty_create
      - isda_establish
      - csa_establish
      - csa_add_eligible_collateral
      - confirmation_configure
    expected_dsl_statements: 5

  - id: otc_rates_setup
    category: compound_otc
    difficulty: hard
    input: "Set up rate derivatives trading with Barclays - IRS and swaptions via MarkitWire"
    expected_intents:
      - isda_add_products
      - confirmation_configure
    expected_entities:
      counterparty_reference: "Barclays"
      derivative_product_type: ["IRS", "SWAPTION"]
      confirmation_method: "MARKITWIRE"

# Add to categories:
categories:
  otc_quick_smoke:
    - cp_simple_1
    - isda_simple_1
    - csa_simple_1
    - confirm_simple_1

  otc_full:
    - cp_simple_1
    - cp_with_lei
    - isda_simple_1
    - isda_with_details
    - isda_add_products
    - csa_simple_1
    - csa_zero_threshold
    - csa_add_collateral
    - csa_im_setup
    - confirm_simple_1
    - confirm_by_product
    - otc_full_setup
    - otc_rates_setup
```

---

## Phase 3.5.6: Demo Scenarios

### Task 3.5.6.1: OTC Trading Setup Scenario
**File:** `rust/examples/scenarios/06_otc_derivatives_setup.dsl`

```clojure
;; =============================================================================
;; Scenario 6: Complete OTC Derivatives Setup
;; =============================================================================
;; 
;; This scenario demonstrates setting up a complete OTC derivatives trading
;; relationship with a major dealer, including:
;; - Counterparty onboarding
;; - ISDA Master Agreement
;; - VM CSA with eligible collateral
;; - IM CSA for UMR compliance  
;; - Confirmation workflows
;; - Collateral accounts
;;
;; Domain context:
;; - Institutional fund trading interest rate and credit derivatives
;; - Subject to Uncleared Margin Rules (UMR Phase 6)
;; - Multiple dealer relationships
;; =============================================================================

;; -----------------------------------------------------------------------------
;; Step 1: Ensure the fund/CBU exists
;; -----------------------------------------------------------------------------
(cbu.ensure
  :name "Global Macro Fund"
  :jurisdiction "LU"
  :legal-structure "SICAV"
  :as @fund)

;; -----------------------------------------------------------------------------
;; Step 2: Onboard counterparties
;; -----------------------------------------------------------------------------
(counterparty.ensure
  :name "Goldman Sachs"
  :lei "784F5XWPLTWKTBV3E584"
  :counterparty-type BANK
  :jurisdiction "US"
  :is-financial-counterparty true
  :is-covered-entity true
  :as @cp-goldman)

(counterparty.add-bic
  :counterparty-id @cp-goldman
  :bic "GABORUSMXXX"
  :bic-type SWIFT
  :is-primary true)

(counterparty.ensure
  :name "Deutsche Bank"
  :lei "7LTWFZYICNSX8D621K86"
  :counterparty-type BANK
  :jurisdiction "DE"
  :is-financial-counterparty true
  :is-covered-entity true
  :as @cp-db)

;; -----------------------------------------------------------------------------
;; Step 3: Establish ISDA Master Agreements
;; -----------------------------------------------------------------------------
(isda.establish
  :cbu-id @fund
  :counterparty-id @cp-goldman
  :version "2002"
  :governing-law NY
  :agreement-date "2024-01-15"
  :netting-applicable true
  :cross-default-applicable true
  :as @isda-goldman)

(isda.establish
  :cbu-id @fund
  :counterparty-id @cp-db
  :version "2002"
  :governing-law ENGLISH
  :agreement-date "2024-02-01"
  :as @isda-db)

;; -----------------------------------------------------------------------------
;; Step 4: Add product scope to ISDAs
;; -----------------------------------------------------------------------------
;; Goldman: Full rates and credit
(isda.add-product-scope :isda-id @isda-goldman :product-type IRS)
(isda.add-product-scope :isda-id @isda-goldman :product-type XCCY)
(isda.add-product-scope :isda-id @isda-goldman :product-type CDS)
(isda.add-product-scope :isda-id @isda-goldman :product-type SWAPTION)

;; DB: Rates only
(isda.add-product-scope :isda-id @isda-db :product-type IRS)
(isda.add-product-scope :isda-id @isda-db :product-type XCCY)
(isda.add-product-scope :isda-id @isda-db :product-type SWAPTION)

;; -----------------------------------------------------------------------------
;; Step 5: Establish VM CSAs
;; -----------------------------------------------------------------------------
(csa.establish
  :isda-id @isda-goldman
  :csa-type VM
  :csa-version "2016 VM"
  :our-threshold 0
  :their-threshold 0
  :threshold-ccy USD
  :mta 500000
  :valuation-agent CALCULATION_AGENT
  :interest-benchmark SOFR
  :rehypothecation false
  :as @csa-goldman-vm)

(csa.establish
  :isda-id @isda-db
  :csa-type VM
  :csa-version "2016 VM"
  :our-threshold 0
  :their-threshold 0
  :threshold-ccy EUR
  :mta 500000
  :interest-benchmark ESTR
  :as @csa-db-vm)

;; -----------------------------------------------------------------------------
;; Step 6: Define eligible collateral for VM CSAs
;; -----------------------------------------------------------------------------
;; Goldman VM - USD focused
(csa.add-eligible-collateral
  :csa-id @csa-goldman-vm
  :asset-class CASH
  :currency USD
  :haircut-pct 0)

(csa.add-eligible-collateral
  :csa-id @csa-goldman-vm
  :asset-class CASH
  :currency EUR
  :haircut-pct 0
  :fx-haircut-pct 8.0)

(csa.add-eligible-collateral
  :csa-id @csa-goldman-vm
  :asset-class GOVT_BOND
  :issuer-jurisdiction US
  :max-maturity-years 10
  :min-rating AA
  :haircut-pct 2.0)

;; DB VM - EUR focused
(csa.add-eligible-collateral
  :csa-id @csa-db-vm
  :asset-class CASH
  :currency EUR
  :haircut-pct 0)

(csa.add-eligible-collateral
  :csa-id @csa-db-vm
  :asset-class GOVT_BOND
  :issuer-jurisdiction DE
  :max-maturity-years 10
  :min-rating AA
  :haircut-pct 1.5)

;; -----------------------------------------------------------------------------
;; Step 7: Establish IM CSAs (UMR compliance)
;; -----------------------------------------------------------------------------
(csa.establish
  :isda-id @isda-goldman
  :csa-type IM
  :csa-version "2018 IM"
  :our-threshold 0
  :their-threshold 0
  :as @csa-goldman-im)

;; IM eligible collateral (more restrictive)
(csa.add-eligible-collateral
  :csa-id @csa-goldman-im
  :asset-class CASH
  :currency USD
  :haircut-pct 0)

(csa.add-eligible-collateral
  :csa-id @csa-goldman-im
  :asset-class GOVT_BOND
  :issuer-jurisdiction US
  :max-maturity-years 5
  :min-rating AAA
  :haircut-pct 1.0)

;; -----------------------------------------------------------------------------
;; Step 8: Set up collateral accounts
;; -----------------------------------------------------------------------------
;; VM accounts (bilateral)
(collateral.ensure-account
  :cbu-id @fund
  :csa-id @csa-goldman-vm
  :account-name "Goldman VM Posted"
  :account-type POSTED_VM
  :as @collat-gs-vm-posted)

(collateral.ensure-account
  :cbu-id @fund
  :csa-id @csa-goldman-vm
  :account-name "Goldman VM Received"
  :account-type RECEIVED_VM
  :as @collat-gs-vm-received)

;; IM accounts (segregated at third-party custodian)
(collateral.ensure-account
  :cbu-id @fund
  :csa-id @csa-goldman-im
  :account-name "Goldman IM - BNY Segregated"
  :account-type POSTED_IM
  :custodian-bic "IRVTUS3NXXX"
  :custodian-name "BNY Mellon"
  :is-third-party true
  :is-segregated true
  :segregation-type INDIVIDUAL
  :as @collat-gs-im-posted)

;; -----------------------------------------------------------------------------
;; Step 9: Configure confirmation methods
;; -----------------------------------------------------------------------------
;; Goldman: MarkitWire for rates, DTCC for credit
(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-goldman
  :product-type IRS
  :method MARKITWIRE
  :auto-match true
  :target-days 1)

(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-goldman
  :product-type XCCY
  :method MARKITWIRE)

(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-goldman
  :product-type CDS
  :method DTCC_GTR)

(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-goldman
  :product-type SWAPTION
  :method MARKITWIRE)

;; DB: MarkitWire for all rates
(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-db
  :method MARKITWIRE
  :auto-match true)

;; -----------------------------------------------------------------------------
;; Summary
;; -----------------------------------------------------------------------------
;; Created:
;; - 1 CBU (Global Macro Fund)
;; - 2 Counterparties (Goldman, Deutsche Bank)
;; - 2 ISDA Master Agreements
;; - 7 Product scopes (IRS, XCCY, CDS, SWAPTION across dealers)
;; - 3 CSAs (Goldman VM, Goldman IM, DB VM)
;; - 8 Eligible collateral definitions
;; - 3 Collateral accounts
;; - 5 Confirmation configurations
;;
;; This represents a realistic institutional derivatives setup
;; with proper separation of VM/IM, segregation for UMR compliance,
;; and platform-specific confirmation routing.
```

---

## Verification Checklist

### Domain Understanding
- [ ] ISDA/CSA structure correctly modeled
- [ ] Collateral haircuts match market standards
- [ ] IM segregation requirements captured
- [ ] Confirmation platforms correctly mapped
- [ ] Interest rate benchmarks up-to-date (SOFR, ESTR, SONIA)

### Implementation
- [ ] All migrations run successfully
- [ ] All verb YAML files parse correctly
- [ ] Intent taxonomy loads without errors
- [ ] Entity types extract correctly
- [ ] Parameter mappings generate valid DSL
- [ ] Demo scenario executes end-to-end

### Demo Readiness
- [ ] Can explain ISDA/CSA hierarchy
- [ ] Can explain VM vs IM distinction
- [ ] Can explain UMR requirements
- [ ] Can demonstrate full counterparty onboarding
- [ ] Can show collateral account segregation

---

## Estimated Effort

| Phase | Days |
|-------|------|
| 3.5.1 Data Model | 2-3 |
| 3.5.2 DSL Verbs | 2 |
| 3.5.3 Intent Taxonomy | 2-3 |
| 3.5.4 Reference Data | 1 |
| 3.5.5 Evaluation | 1 |
| 3.5.6 Demo Scenarios | 1 |
| **Total** | **9-11 days** |

---

## Notes for Claude Code

1. **Run migrations first** - Data model must exist before verbs work
2. **Test verb YAML** - Each verb config should be loadable
3. **Validate LEIs** - The LEI examples are real; checksum validation should pass
4. **Interest benchmarks** - LIBOR is dead; use SOFR, ESTR, SONIA, TONAR
5. **UMR matters** - Initial Margin rules drive segregation requirements
6. **Domain experts** - If unsure about derivatives terms, ask before guessing

This demonstrates that we **understand the custody derivatives business**, not just the technology.
