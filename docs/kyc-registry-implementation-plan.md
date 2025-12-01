# KYC Domain Consolidation & Registry Model Implementation Plan

**Document**: `kyc-registry-implementation-plan.md`  
**Created**: 2025-12-01  
**Status**: READY FOR IMPLEMENTATION  
**Scope**: Unified KYC domain with Clearstream-style registry, supporting institutional KYC, UBO mapping, and investor registry (KYCaaS)

---

## Executive Summary

This plan consolidates:
- **KYC + UBO** into single domain (not separate)
- **Share class model** for accurate ownership/control representation
- **Clearstream-style registry** for fund investor management
- **Hedge fund vs Long fund** structural differences
- **Person management** (officers, UBOs, investors)

All attached to the **CBU as the central client model**.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                 CBU                                          │
│                        (Central Business Unit)                               │
│                                                                              │
│   The client. Everything attaches here.                                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        │                           │                           │
        ▼                           ▼                           ▼
┌───────────────────┐     ┌───────────────────┐     ┌───────────────────┐
│  PRODUCT DELIVERY │     │   KYC DOMAIN      │     │  ENTITY STRUCTURE │
│                   │     │                   │     │                   │
│  • Markets        │     │  • Documents      │     │  • Legal entities │
│  • SSIs           │     │  • Verification   │     │  • Share classes  │
│  • Booking rules  │     │  • Sanctions      │     │  • Holdings       │
│  • ISDA/CSA       │     │  • Status         │     │  • Persons/roles  │
│                   │     │                   │     │  • UBO chain      │
│  DSL: custody.*   │     │  DSL: kyc.*       │     │  DSL: kyc.*       │
└───────────────────┘     └───────────────────┘     └───────────────────┘
                                    │
                                    │ For funds issuing shares
                                    ▼
                          ┌───────────────────┐
                          │  INVESTOR REGISTRY│
                          │     (KYCaaS)      │
                          │                   │
                          │  • Subscriptions  │
                          │  • Redemptions    │
                          │  • Positions      │
                          │  • Investor KYC   │
                          │                   │
                          │  DSL: kyc.*       │
                          └───────────────────┘
```

---

## Phase 0: Audit Existing Infrastructure

### Task 0.1: Audit Database Schemas

```sql
-- Check existing tables in ob-poc schema
\dt "ob-poc".*

-- Check for existing kyc schema
\dn kyc

-- Look for entity-related tables
SELECT table_name FROM information_schema.tables 
WHERE table_schema = 'ob-poc' 
AND table_name LIKE '%entity%' OR table_name LIKE '%owner%' OR table_name LIKE '%document%';

-- Check entity_ownership_links structure
\d "ob-poc".entity_ownership_links

-- Check documents table
\d "ob-poc".documents
```

### Task 0.2: Audit Existing Verbs

```bash
# Check for existing KYC-related verbs
grep -E "^(kyc|ubo|entity|document):" rust/config/verbs.yaml

# Check for person-related verbs
grep -i "person" rust/config/verbs.yaml
```

### Task 0.3: Document Findings

Create `docs/kyc-audit-findings.md` with:
- Tables that exist vs need creation
- Verbs that exist vs need creation
- Any schema conflicts to resolve

**Effort**: 0.5 day

---

## Phase 1: Database Schema

### Create KYC Schema

File: `migrations/YYYYMMDD_001_kyc_schema.sql`

```sql
-- ═══════════════════════════════════════════════════════════════════════════
-- KYC SCHEMA CREATION
-- ═══════════════════════════════════════════════════════════════════════════

CREATE SCHEMA IF NOT EXISTS kyc;

-- ═══════════════════════════════════════════════════════════════════════════
-- PERSONS (All individuals in the system)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.persons (
    person_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Identity
    first_name VARCHAR(100) NOT NULL,
    middle_name VARCHAR(100),
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE,
    place_of_birth VARCHAR(100),
    nationality VARCHAR(3),                      -- ISO 3166-1 alpha-3
    dual_nationality VARCHAR(3),
    
    -- Tax
    tax_id VARCHAR(50),
    tax_country VARCHAR(3),
    
    -- Contact
    email VARCHAR(255),
    phone VARCHAR(50),
    
    -- Address
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state_province VARCHAR(100),
    postal_code VARCHAR(20),
    country VARCHAR(3),
    
    -- KYC Status (global for this person)
    kyc_status VARCHAR(20) DEFAULT 'NOT_STARTED',  -- NOT_STARTED, IN_PROGRESS, APPROVED, REJECTED, EXPIRED
    kyc_expiry_date DATE,
    risk_rating VARCHAR(20),                       -- LOW, MEDIUM, HIGH, PROHIBITED
    
    -- Screening
    pep_status VARCHAR(20) DEFAULT 'NOT_CHECKED',  -- NOT_CHECKED, NOT_PEP, PEP, PEP_ASSOCIATE
    sanctions_status VARCHAR(20) DEFAULT 'NOT_CHECKED', -- NOT_CHECKED, CLEAR, MATCH, POTENTIAL_MATCH
    last_screening_date DATE,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_person_name ON kyc.persons(last_name, first_name);
CREATE INDEX idx_person_tax ON kyc.persons(tax_id, tax_country) WHERE tax_id IS NOT NULL;
CREATE INDEX idx_person_kyc_status ON kyc.persons(kyc_status);

-- ═══════════════════════════════════════════════════════════════════════════
-- CBU-PERSON LINKS (Officers, Signatories, UBOs)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.cbu_persons (
    cbu_person_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    person_id UUID NOT NULL REFERENCES kyc.persons(person_id),
    
    -- Role
    role VARCHAR(30) NOT NULL,                    -- See valid_values in verbs
    title VARCHAR(100),                           -- "Chief Investment Officer"
    
    -- If UBO via direct ownership (legacy - prefer holdings model)
    ownership_percentage DECIMAL(5,2),
    control_type VARCHAR(30),
    
    -- Dates
    effective_date DATE DEFAULT CURRENT_DATE,
    end_date DATE,
    is_active BOOLEAN DEFAULT true,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(cbu_id, person_id, role)
);

CREATE INDEX idx_cbu_person_cbu ON kyc.cbu_persons(cbu_id);
CREATE INDEX idx_cbu_person_person ON kyc.cbu_persons(person_id);
CREATE INDEX idx_cbu_person_role ON kyc.cbu_persons(role);

-- ═══════════════════════════════════════════════════════════════════════════
-- SHARE CLASSES (Clearstream: Security Master)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.share_classes (
    share_class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Identification
    class_name VARCHAR(100) NOT NULL,             -- "Class A Ordinary", "Institutional USD"
    class_code VARCHAR(20),                       -- Short code: "CL-A", "INST-USD"
    isin VARCHAR(12),
    cusip VARCHAR(9),
    sedol VARCHAR(7),
    bloomberg_ticker VARCHAR(20),
    
    -- Classification
    class_type VARCHAR(30) NOT NULL,              -- See enum below
    security_type VARCHAR(30) DEFAULT 'FUND',     -- EQUITY, FUND, BOND, LP_INTEREST
    
    -- Currency & Denomination
    currency VARCHAR(3) NOT NULL,
    par_value DECIMAL(10,4),
    
    -- Capitalization
    shares_authorized BIGINT,
    shares_issued DECIMAL(20,6) DEFAULT 0,        -- Allow fractional for fund units
    shares_outstanding DECIMAL(20,6) DEFAULT 0,   -- Issued minus treasury
    
    -- Voting Rights
    votes_per_share DECIMAL(10,4) DEFAULT 1,      -- 0 = non-voting
    voting_rights_description TEXT,
    
    -- Economic Rights
    dividend_priority INTEGER DEFAULT 0,          -- Higher = paid first
    liquidation_priority INTEGER DEFAULT 0,       -- Higher = paid first
    participation_rights TEXT,                    -- Description of economic rights
    
    -- ═══════════════════════════════════════════════════════════════════════
    -- FUND-SPECIFIC FIELDS
    -- ═══════════════════════════════════════════════════════════════════════
    
    -- Fund Type Classification
    fund_type VARCHAR(30),                        -- HEDGE_FUND, UCITS, OEIC, 40_ACT, PE_FUND, VC_FUND
    fund_structure VARCHAR(30),                   -- OPEN_ENDED, CLOSED_ENDED, INTERVAL
    
    -- Investor Eligibility
    investor_eligibility VARCHAR(30),             -- RETAIL, PROFESSIONAL, QUALIFIED_PURCHASER, ACCREDITED
    min_initial_investment DECIMAL(15,2),
    min_additional_investment DECIMAL(15,2),
    
    -- Fees (as decimals: 0.02 = 2%)
    management_fee DECIMAL(6,5),
    performance_fee DECIMAL(6,5),                 -- Hedge funds
    entry_fee DECIMAL(6,5),                       -- Load
    exit_fee DECIMAL(6,5),                        -- Redemption fee
    
    -- Pricing
    nav_frequency VARCHAR(20),                    -- DAILY, WEEKLY, MONTHLY, QUARTERLY
    nav_per_share DECIMAL(15,6),
    nav_date DATE,
    
    -- Liquidity (Long Funds)
    dealing_frequency VARCHAR(20),                -- DAILY, WEEKLY, MONTHLY
    dealing_days VARCHAR(50),                     -- "MON,WED,FRI" or "1,15" (of month)
    settlement_period VARCHAR(10),                -- "T+1", "T+3"
    cut_off_time TIME,
    cut_off_timezone VARCHAR(50),
    
    -- Liquidity (Hedge Funds)
    lock_up_period_months INTEGER,                -- Initial lock-up
    redemption_notice_days INTEGER,               -- Notice period
    redemption_frequency VARCHAR(20),             -- MONTHLY, QUARTERLY, ANNUAL
    gate_percentage DECIMAL(5,2),                 -- Max % can redeem per period
    side_pocket_eligible BOOLEAN DEFAULT false,
    
    -- Performance Fee Mechanics (Hedge Funds)
    high_water_mark BOOLEAN DEFAULT false,
    hurdle_rate DECIMAL(6,5),                     -- e.g., 0.08 = 8%
    hurdle_type VARCHAR(20),                      -- HARD, SOFT
    crystallization_frequency VARCHAR(20),        -- ANNUAL, QUARTERLY
    equalisation_method VARCHAR(30),              -- SERIES, EQUALISATION_FACTOR, CONTINGENT_REDEMPTION
    
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE',          -- ACTIVE, SUSPENDED, SOFT_CLOSED, HARD_CLOSED, LIQUIDATING
    launch_date DATE,
    termination_date DATE,
    
    -- Metadata
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_share_class_entity ON kyc.share_classes(entity_id);
CREATE INDEX idx_share_class_isin ON kyc.share_classes(isin) WHERE isin IS NOT NULL;
CREATE INDEX idx_share_class_type ON kyc.share_classes(class_type);
CREATE INDEX idx_share_class_fund_type ON kyc.share_classes(fund_type) WHERE fund_type IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════════════
-- HOLDINGS / POSITIONS (Clearstream: Position)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.holdings (
    holding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_class_id UUID NOT NULL REFERENCES kyc.share_classes(share_class_id),
    
    -- Holder (exactly one must be set)
    holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    holder_person_id UUID REFERENCES kyc.persons(person_id),
    
    -- Account (for registry purposes)
    account_number VARCHAR(50),                   -- Investor account number
    account_type VARCHAR(30),                     -- INDIVIDUAL, JOINT, IRA, TRUST, CORPORATE
    
    -- Position
    units_held DECIMAL(20,6) NOT NULL,
    units_pending_subscription DECIMAL(20,6) DEFAULT 0,
    units_pending_redemption DECIMAL(20,6) DEFAULT 0,
    units_blocked DECIMAL(20,6) DEFAULT 0,        -- Pledged, locked, etc.
    
    -- Calculated Percentages (denormalized for performance)
    pct_of_class DECIMAL(8,5),
    pct_of_entity_total DECIMAL(8,5),
    pct_voting_power DECIMAL(8,5),
    pct_economic_interest DECIMAL(8,5),
    
    -- Cost Basis
    acquisition_date DATE,
    average_cost_per_unit DECIMAL(15,6),
    total_cost_basis DECIMAL(15,2),
    
    -- Hedge Fund Specific
    commitment_amount DECIMAL(15,2),              -- Total committed (PE/VC)
    unfunded_commitment DECIMAL(15,2),            -- Not yet called
    high_water_mark_nav DECIMAL(15,6),            -- Per-investor HWM
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    -- Constraints
    CONSTRAINT single_holder CHECK (
        (holder_entity_id IS NOT NULL)::int +
        (holder_person_id IS NOT NULL)::int = 1
    ),
    CONSTRAINT positive_units CHECK (units_held >= 0)
);

CREATE INDEX idx_holding_share_class ON kyc.holdings(share_class_id);
CREATE INDEX idx_holding_entity ON kyc.holdings(holder_entity_id) WHERE holder_entity_id IS NOT NULL;
CREATE INDEX idx_holding_person ON kyc.holdings(holder_person_id) WHERE holder_person_id IS NOT NULL;
CREATE INDEX idx_holding_account ON kyc.holdings(account_number) WHERE account_number IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════════════
-- MOVEMENTS / TRANSACTIONS (Clearstream: Movement)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.movements (
    movement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_class_id UUID NOT NULL REFERENCES kyc.share_classes(share_class_id),
    
    -- Movement Type
    movement_type VARCHAR(30) NOT NULL,           -- See enum below
    
    -- Parties
    from_holding_id UUID REFERENCES kyc.holdings(holding_id),
    to_holding_id UUID REFERENCES kyc.holdings(holding_id),
    
    -- Amounts
    units DECIMAL(20,6) NOT NULL,
    price_per_unit DECIMAL(15,6),                 -- NAV at execution
    gross_amount DECIMAL(15,2),
    fees DECIMAL(15,2) DEFAULT 0,
    net_amount DECIMAL(15,2),
    currency VARCHAR(3),
    
    -- Dates
    trade_date DATE NOT NULL,
    settlement_date DATE,
    value_date DATE,                              -- For NAV calculation
    
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING',         -- PENDING, SETTLED, FAILED, CANCELLED
    
    -- References
    external_reference VARCHAR(100),              -- Client reference
    internal_reference VARCHAR(100),              -- Our reference
    
    -- Hedge Fund Specific
    redemption_type VARCHAR(30),                  -- FULL, PARTIAL, GATE, SIDE_POCKET
    lock_up_waiver BOOLEAN DEFAULT false,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    settled_at TIMESTAMPTZ,
    notes TEXT
);

CREATE INDEX idx_movement_share_class ON kyc.movements(share_class_id);
CREATE INDEX idx_movement_type ON kyc.movements(movement_type);
CREATE INDEX idx_movement_status ON kyc.movements(status);
CREATE INDEX idx_movement_dates ON kyc.movements(trade_date, settlement_date);

-- ═══════════════════════════════════════════════════════════════════════════
-- OWNERSHIP LINKS (Entity-to-Entity via Holdings)
-- For UBO calculation, we walk holdings → share_classes → entities
-- This table is for non-share-class ownership (partnerships, trusts)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.ownership_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Owner (entity or person)
    owner_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    owner_person_id UUID REFERENCES kyc.persons(person_id),
    
    -- Owned entity
    owned_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Ownership details
    ownership_percentage DECIMAL(5,2),
    ownership_type VARCHAR(20) DEFAULT 'DIRECT',  -- DIRECT, INDIRECT, BENEFICIAL
    ownership_description TEXT,                   -- For complex structures
    
    -- Dates
    effective_date DATE DEFAULT CURRENT_DATE,
    end_date DATE,
    is_active BOOLEAN DEFAULT true,
    
    -- Verification
    verified BOOLEAN DEFAULT false,
    verified_date DATE,
    verified_by TEXT,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT single_owner CHECK (
        (owner_entity_id IS NOT NULL)::int +
        (owner_person_id IS NOT NULL)::int = 1
    ),
    CONSTRAINT valid_percentage CHECK (
        ownership_percentage IS NULL OR 
        (ownership_percentage >= 0 AND ownership_percentage <= 100)
    )
);

CREATE INDEX idx_ownership_owner_entity ON kyc.ownership_links(owner_entity_id) WHERE owner_entity_id IS NOT NULL;
CREATE INDEX idx_ownership_owner_person ON kyc.ownership_links(owner_person_id) WHERE owner_person_id IS NOT NULL;
CREATE INDEX idx_ownership_owned ON kyc.ownership_links(owned_entity_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- CONTROL LINKS (Non-ownership control)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.control_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Controller (entity or person)
    controller_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    controller_person_id UUID REFERENCES kyc.persons(person_id),
    
    -- Controlled entity
    controlled_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Control details
    control_type VARCHAR(30) NOT NULL,
    control_description TEXT,
    
    -- Dates
    effective_date DATE DEFAULT CURRENT_DATE,
    end_date DATE,
    is_active BOOLEAN DEFAULT true,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT single_controller CHECK (
        (controller_entity_id IS NOT NULL)::int +
        (controller_person_id IS NOT NULL)::int = 1
    )
);

CREATE INDEX idx_control_controller ON kyc.control_links(controller_entity_id) WHERE controller_entity_id IS NOT NULL;
CREATE INDEX idx_control_controlled ON kyc.control_links(controlled_entity_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- UBO DETERMINATIONS (Calculated/Confirmed UBOs)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.ubo_determinations (
    determination_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    person_id UUID NOT NULL REFERENCES kyc.persons(person_id),
    
    -- Determination
    is_ubo BOOLEAN NOT NULL,
    
    -- Calculated percentages
    effective_ownership_pct DECIMAL(5,2),
    effective_voting_pct DECIMAL(5,2),
    effective_economic_pct DECIMAL(5,2),
    
    -- Basis
    determination_basis TEXT,                     -- How we determined this
    threshold_applied DECIMAL(5,2) DEFAULT 25,    -- Regulatory threshold used
    
    -- Control (UBO via control, not ownership)
    is_control_ubo BOOLEAN DEFAULT false,
    control_description TEXT,
    
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING',         -- PENDING, CONFIRMED, DISPUTED, SUPERSEDED
    
    -- Dates
    determination_date DATE DEFAULT CURRENT_DATE,
    determined_by TEXT,
    next_review_date DATE,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(cbu_id, person_id)
);

CREATE INDEX idx_ubo_cbu ON kyc.ubo_determinations(cbu_id);
CREATE INDEX idx_ubo_person ON kyc.ubo_determinations(person_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- DOCUMENT REQUIREMENTS
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.document_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What this requirement is for
    person_id UUID REFERENCES kyc.persons(person_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    
    -- Document details
    document_type VARCHAR(50) NOT NULL,
    priority VARCHAR(20) DEFAULT 'STANDARD',
    
    -- Dates
    due_date DATE,
    
    -- Status
    status VARCHAR(20) DEFAULT 'REQUIRED',        -- REQUIRED, SUBMITTED, VERIFIED, REJECTED, WAIVED
    
    -- Notes
    notes TEXT,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_doc_req_person ON kyc.document_requirements(person_id) WHERE person_id IS NOT NULL;
CREATE INDEX idx_doc_req_entity ON kyc.document_requirements(entity_id) WHERE entity_id IS NOT NULL;
CREATE INDEX idx_doc_req_cbu ON kyc.document_requirements(cbu_id) WHERE cbu_id IS NOT NULL;
CREATE INDEX idx_doc_req_status ON kyc.document_requirements(status);

-- ═══════════════════════════════════════════════════════════════════════════
-- SUBMITTED DOCUMENTS
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.submitted_documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requirement_id UUID REFERENCES kyc.document_requirements(requirement_id),
    
    -- Owner
    person_id UUID REFERENCES kyc.persons(person_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    
    -- Document details
    document_type VARCHAR(50) NOT NULL,
    file_reference TEXT,                          -- S3 key, file path, etc.
    original_filename TEXT,
    mime_type VARCHAR(100),
    file_size_bytes BIGINT,
    
    -- Dates
    submitted_date DATE DEFAULT CURRENT_DATE,
    expiry_date DATE,
    
    -- Verification
    status VARCHAR(20) DEFAULT 'PENDING',
    verified_by TEXT,
    verified_date DATE,
    rejection_reason TEXT,
    
    -- Extracted data (JSON for flexibility)
    extracted_data JSONB,                         -- OCR/parsing results
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_doc_person ON kyc.submitted_documents(person_id) WHERE person_id IS NOT NULL;
CREATE INDEX idx_doc_entity ON kyc.submitted_documents(entity_id) WHERE entity_id IS NOT NULL;
CREATE INDEX idx_doc_status ON kyc.submitted_documents(status);

-- ═══════════════════════════════════════════════════════════════════════════
-- SANCTIONS SCREENINGS
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.sanctions_screenings (
    screening_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Subject
    person_id UUID REFERENCES kyc.persons(person_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    
    -- Screening details
    screening_provider VARCHAR(50),
    screening_type VARCHAR(30),                   -- SANCTIONS, PEP, ADVERSE_MEDIA, FULL
    
    -- Result
    result VARCHAR(20) NOT NULL,                  -- CLEAR, MATCH, POTENTIAL_MATCH, ERROR
    match_details JSONB,                          -- Detailed results
    
    -- Dates
    screening_date DATE DEFAULT CURRENT_DATE,
    
    -- Review (if matches found)
    reviewed_by TEXT,
    review_date DATE,
    review_notes TEXT,
    review_outcome VARCHAR(20),                   -- CLEARED, ESCALATED, BLOCKED
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_screening_person ON kyc.sanctions_screenings(person_id) WHERE person_id IS NOT NULL;
CREATE INDEX idx_screening_entity ON kyc.sanctions_screenings(entity_id) WHERE entity_id IS NOT NULL;
CREATE INDEX idx_screening_result ON kyc.sanctions_screenings(result);

-- ═══════════════════════════════════════════════════════════════════════════
-- ENTITY KYC STATUS (Rolled-up status per CBU)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.entity_kyc_status (
    status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    
    -- Status
    kyc_status VARCHAR(20) DEFAULT 'NOT_STARTED',
    risk_rating VARCHAR(20),
    
    -- Review
    last_review_date DATE,
    next_review_date DATE,
    reviewer TEXT,
    
    -- Notes
    notes TEXT,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(entity_id, cbu_id)
);

-- ═══════════════════════════════════════════════════════════════════════════
-- KYCAAS CLIENTS (For investor registry service)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc.kycaas_clients (
    client_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_name VARCHAR(200) NOT NULL,
    client_type VARCHAR(30),                      -- TRANSFER_AGENT, FUND_ADMIN, HEDGE_FUND, etc.
    
    -- Service level
    service_level VARCHAR(20) DEFAULT 'STANDARD', -- BASIC, STANDARD, ENHANCED
    sla_days INTEGER,                             -- KYC turnaround SLA
    
    -- Contact
    primary_contact_name VARCHAR(200),
    primary_contact_email VARCHAR(255),
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now()
);

-- ═══════════════════════════════════════════════════════════════════════════
-- ENUM COMMENTS (For reference)
-- ═══════════════════════════════════════════════════════════════════════════

COMMENT ON TABLE kyc.share_classes IS 'class_type values: ORDINARY, PREFERENCE, FOUNDER, SEED, SERIES_A, SERIES_B, SERIES_C, GP_INTEREST, LP_INTEREST, UNIT, INSTITUTIONAL, RETAIL';

COMMENT ON TABLE kyc.share_classes IS 'fund_type values: HEDGE_FUND, UCITS, OEIC, 40_ACT, PE_FUND, VC_FUND, REIT, MUTUAL_FUND';

COMMENT ON TABLE kyc.share_classes IS 'fund_structure values: OPEN_ENDED, CLOSED_ENDED, INTERVAL, TENDER_OFFER';

COMMENT ON TABLE kyc.movements IS 'movement_type values: SUBSCRIPTION, REDEMPTION, TRANSFER_IN, TRANSFER_OUT, SWITCH_IN, SWITCH_OUT, DIVIDEND_REINVEST, CAPITAL_CALL, DISTRIBUTION, CORPORATE_ACTION';

COMMENT ON TABLE kyc.cbu_persons IS 'role values: DIRECTOR, CEO, CFO, CIO, COO, CCO, PORTFOLIO_MANAGER, AUTH_SIGNATORY, UBO, TRUSTEE, SETTLOR, PROTECTOR, BENEFICIARY, GP, LP, MANAGING_MEMBER';

COMMENT ON TABLE kyc.control_links IS 'control_type values: BOARD_CONTROL, VOTING_RIGHTS, VETO_POWER, MANAGEMENT, TRUSTEE, INVESTMENT_DISCRETION, OTHER';

COMMENT ON TABLE kyc.document_requirements IS 'document_type values: PASSPORT, DRIVERS_LICENSE, NATIONAL_ID, PROOF_OF_ADDRESS, TAX_FORM_W9, TAX_FORM_W8BEN, TAX_FORM_W8IMY, CERT_OF_INCORPORATION, BOARD_RESOLUTION, ARTICLES_OF_ASSOCIATION, SHAREHOLDER_REGISTER, TRUST_DEED, PARTNERSHIP_AGREEMENT, FINANCIAL_STATEMENTS, BANK_REFERENCE, SOURCE_OF_FUNDS, SOURCE_OF_WEALTH, SUBSCRIPTION_AGREEMENT, INVESTOR_QUESTIONNAIRE';
```

**Effort**: 1 day

---

## Phase 2: KYC Verb Definitions

### File: `rust/config/verbs.yaml` - Add/Update KYC Domain

```yaml
kyc:
  description: "KYC - persons, documents, ownership, share classes, investor registry"
  schema: kyc
  
  verbs:
    # ═══════════════════════════════════════════════════════════════════════
    # PERSON MANAGEMENT
    # ═══════════════════════════════════════════════════════════════════════
    
    add-person:
      description: "Create a person record"
      behavior: crud
      crud:
        operation: insert
        table: persons
        schema: kyc
        returning: person_id
      args:
        - name: first-name
          type: string
          required: true
          maps_to: first_name
        - name: last-name
          type: string
          required: true
          maps_to: last_name
        - name: middle-name
          type: string
          required: false
          maps_to: middle_name
        - name: date-of-birth
          type: date
          required: false
          maps_to: date_of_birth
        - name: nationality
          type: string
          required: false
          maps_to: nationality
        - name: tax-id
          type: string
          required: false
          maps_to: tax_id
        - name: tax-country
          type: string
          required: false
          maps_to: tax_country
        - name: email
          type: string
          required: false
          maps_to: email
      returns:
        type: uuid
        name: person_id
        capture: true
    
    update-person:
      description: "Update person details"
      behavior: crud
      crud:
        operation: update
        table: persons
        schema: kyc
        key: person_id
      args:
        - name: person-id
          type: uuid
          required: true
          maps_to: person_id
        - name: first-name
          type: string
          required: false
          maps_to: first_name
        - name: last-name
          type: string
          required: false
          maps_to: last_name
        - name: date-of-birth
          type: date
          required: false
          maps_to: date_of_birth
        - name: nationality
          type: string
          required: false
          maps_to: nationality
        - name: tax-id
          type: string
          required: false
          maps_to: tax_id
        - name: tax-country
          type: string
          required: false
          maps_to: tax_country
        - name: email
          type: string
          required: false
          maps_to: email
      returns:
        type: affected
    
    read-person:
      description: "Get person by ID"
      behavior: crud
      crud:
        operation: select_by_pk
        table: persons
        schema: kyc
        pk: person_id
      args:
        - name: person-id
          type: uuid
          required: true
          maps_to: person_id
      returns:
        type: record
    
    search-persons:
      description: "Search persons by name"
      behavior: plugin
      handler: search_persons
      args:
        - name: query
          type: string
          required: true
          description: "Name search (first or last)"
      returns:
        type: record_set
    
    # ═══════════════════════════════════════════════════════════════════════
    # CBU-PERSON LINKS (Officers, UBOs)
    # ═══════════════════════════════════════════════════════════════════════
    
    link-person-to-cbu:
      description: "Link a person to a CBU with a role"
      behavior: crud
      crud:
        operation: upsert
        table: cbu_persons
        schema: kyc
        conflict_keys: [cbu_id, person_id, role]
        returning: cbu_person_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: person-id
          type: uuid
          required: true
          maps_to: person_id
        - name: role
          type: string
          required: true
          maps_to: role
          valid_values: [DIRECTOR, CEO, CFO, CIO, COO, CCO, PORTFOLIO_MANAGER,
                         AUTH_SIGNATORY, UBO, TRUSTEE, SETTLOR, PROTECTOR,
                         BENEFICIARY, GP, LP, MANAGING_MEMBER]
        - name: title
          type: string
          required: false
          maps_to: title
        - name: ownership-percentage
          type: decimal
          required: false
          maps_to: ownership_percentage
        - name: control-type
          type: string
          required: false
          maps_to: control_type
          valid_values: [BOARD_CONTROL, VOTING_RIGHTS, VETO_POWER, MANAGEMENT,
                         TRUSTEE, INVESTMENT_DISCRETION, OTHER]
        - name: effective-date
          type: date
          required: false
          maps_to: effective_date
      returns:
        type: uuid
        name: cbu_person_id
        capture: true
    
    remove-person-from-cbu:
      description: "Remove a person's role at a CBU"
      behavior: crud
      crud:
        operation: update
        table: cbu_persons
        schema: kyc
        key: cbu_person_id
        set_values:
          is_active: false
      args:
        - name: cbu-person-id
          type: uuid
          required: true
          maps_to: cbu_person_id
        - name: end-date
          type: date
          required: false
          maps_to: end_date
      returns:
        type: affected
    
    list-cbu-persons:
      description: "List persons linked to a CBU"
      behavior: crud
      crud:
        operation: list_by_fk
        table: cbu_persons
        schema: kyc
        fk_col: cbu_id
        filter:
          is_active: true
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
      returns:
        type: record_set
    
    # ═══════════════════════════════════════════════════════════════════════
    # SHARE CLASS MANAGEMENT
    # ═══════════════════════════════════════════════════════════════════════
    
    add-share-class:
      description: "Add a share class to an entity"
      behavior: crud
      crud:
        operation: insert
        table: share_classes
        schema: kyc
        returning: share_class_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: class-name
          type: string
          required: true
          maps_to: class_name
        - name: class-type
          type: string
          required: true
          maps_to: class_type
          valid_values: [ORDINARY, PREFERENCE, FOUNDER, SEED, SERIES_A, SERIES_B,
                         SERIES_C, GP_INTEREST, LP_INTEREST, UNIT, INSTITUTIONAL, RETAIL]
        - name: isin
          type: string
          required: false
          maps_to: isin
        - name: currency
          type: string
          required: true
          maps_to: currency
        - name: votes-per-share
          type: decimal
          required: false
          maps_to: votes_per_share
        - name: shares-authorized
          type: integer
          required: false
          maps_to: shares_authorized
        - name: shares-issued
          type: decimal
          required: false
          maps_to: shares_issued
        # Fund-specific
        - name: fund-type
          type: string
          required: false
          maps_to: fund_type
          valid_values: [HEDGE_FUND, UCITS, OEIC, 40_ACT, PE_FUND, VC_FUND, REIT, MUTUAL_FUND]
        - name: fund-structure
          type: string
          required: false
          maps_to: fund_structure
          valid_values: [OPEN_ENDED, CLOSED_ENDED, INTERVAL, TENDER_OFFER]
        - name: investor-eligibility
          type: string
          required: false
          maps_to: investor_eligibility
          valid_values: [RETAIL, PROFESSIONAL, QUALIFIED_PURCHASER, ACCREDITED]
        - name: min-investment
          type: decimal
          required: false
          maps_to: min_initial_investment
        - name: management-fee
          type: decimal
          required: false
          maps_to: management_fee
        - name: performance-fee
          type: decimal
          required: false
          maps_to: performance_fee
        - name: nav-frequency
          type: string
          required: false
          maps_to: nav_frequency
          valid_values: [DAILY, WEEKLY, MONTHLY, QUARTERLY]
        # Hedge fund liquidity
        - name: lock-up-months
          type: integer
          required: false
          maps_to: lock_up_period_months
        - name: redemption-notice-days
          type: integer
          required: false
          maps_to: redemption_notice_days
        - name: redemption-frequency
          type: string
          required: false
          maps_to: redemption_frequency
          valid_values: [DAILY, WEEKLY, MONTHLY, QUARTERLY, ANNUAL]
        - name: gate-percentage
          type: decimal
          required: false
          maps_to: gate_percentage
        # Performance fee mechanics
        - name: high-water-mark
          type: boolean
          required: false
          maps_to: high_water_mark
        - name: hurdle-rate
          type: decimal
          required: false
          maps_to: hurdle_rate
      returns:
        type: uuid
        name: share_class_id
        capture: true
    
    update-share-class:
      description: "Update share class details"
      behavior: crud
      crud:
        operation: update
        table: share_classes
        schema: kyc
        key: share_class_id
      args:
        - name: share-class-id
          type: uuid
          required: true
          maps_to: share_class_id
        - name: shares-issued
          type: decimal
          required: false
          maps_to: shares_issued
        - name: nav-per-share
          type: decimal
          required: false
          maps_to: nav_per_share
        - name: nav-date
          type: date
          required: false
          maps_to: nav_date
        - name: status
          type: string
          required: false
          maps_to: status
          valid_values: [ACTIVE, SUSPENDED, SOFT_CLOSED, HARD_CLOSED, LIQUIDATING]
      returns:
        type: affected
    
    list-share-classes:
      description: "List share classes for an entity"
      behavior: crud
      crud:
        operation: list_by_fk
        table: share_classes
        schema: kyc
        fk_col: entity_id
        filter:
          is_active: true
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
      returns:
        type: record_set
    
    # ═══════════════════════════════════════════════════════════════════════
    # HOLDINGS / POSITIONS
    # ═══════════════════════════════════════════════════════════════════════
    
    add-holding:
      description: "Add a holding (entity or person holds shares)"
      behavior: crud
      crud:
        operation: insert
        table: holdings
        schema: kyc
        returning: holding_id
      args:
        - name: share-class-id
          type: uuid
          required: true
          maps_to: share_class_id
        - name: holder-type
          type: string
          required: true
          valid_values: [ENTITY, PERSON]
        - name: holder-id
          type: uuid
          required: true
          description: "Entity ID or Person ID"
        - name: units
          type: decimal
          required: true
          maps_to: units_held
        - name: account-number
          type: string
          required: false
          maps_to: account_number
        - name: account-type
          type: string
          required: false
          maps_to: account_type
          valid_values: [INDIVIDUAL, JOINT, IRA, ROTH_IRA, TRUST, ESTATE, CORPORATE]
        - name: acquisition-date
          type: date
          required: false
          maps_to: acquisition_date
        - name: cost-per-unit
          type: decimal
          required: false
          maps_to: average_cost_per_unit
        - name: commitment-amount
          type: decimal
          required: false
          maps_to: commitment_amount
      returns:
        type: uuid
        name: holding_id
        capture: true
    
    update-holding:
      description: "Update holding units"
      behavior: crud
      crud:
        operation: update
        table: holdings
        schema: kyc
        key: holding_id
      args:
        - name: holding-id
          type: uuid
          required: true
          maps_to: holding_id
        - name: units
          type: decimal
          required: false
          maps_to: units_held
        - name: pending-subscription
          type: decimal
          required: false
          maps_to: units_pending_subscription
        - name: pending-redemption
          type: decimal
          required: false
          maps_to: units_pending_redemption
      returns:
        type: affected
    
    remove-holding:
      description: "Remove a holding (zero balance)"
      behavior: crud
      crud:
        operation: update
        table: holdings
        schema: kyc
        key: holding_id
        set_values:
          is_active: false
          units_held: 0
      args:
        - name: holding-id
          type: uuid
          required: true
          maps_to: holding_id
      returns:
        type: affected
    
    list-holdings-by-class:
      description: "List all holdings for a share class"
      behavior: crud
      crud:
        operation: list_by_fk
        table: holdings
        schema: kyc
        fk_col: share_class_id
        filter:
          is_active: true
      args:
        - name: share-class-id
          type: uuid
          required: true
          maps_to: share_class_id
      returns:
        type: record_set
    
    list-holdings-by-holder:
      description: "List all holdings for a holder"
      behavior: plugin
      handler: list_holdings_by_holder
      args:
        - name: holder-type
          type: string
          required: true
          valid_values: [ENTITY, PERSON]
        - name: holder-id
          type: uuid
          required: true
      returns:
        type: record_set
    
    # ═══════════════════════════════════════════════════════════════════════
    # MOVEMENTS / TRANSACTIONS
    # ═══════════════════════════════════════════════════════════════════════
    
    record-subscription:
      description: "Record a subscription (buy units)"
      behavior: plugin
      handler: record_subscription
      args:
        - name: share-class-id
          type: uuid
          required: true
        - name: holding-id
          type: uuid
          required: true
        - name: units
          type: decimal
          required: true
        - name: price-per-unit
          type: decimal
          required: false
        - name: gross-amount
          type: decimal
          required: false
        - name: trade-date
          type: date
          required: false
        - name: settlement-date
          type: date
          required: false
      returns:
        type: uuid
        name: movement_id
    
    record-redemption:
      description: "Record a redemption (sell units)"
      behavior: plugin
      handler: record_redemption
      args:
        - name: share-class-id
          type: uuid
          required: true
        - name: holding-id
          type: uuid
          required: true
        - name: units
          type: decimal
          required: true
        - name: price-per-unit
          type: decimal
          required: false
        - name: trade-date
          type: date
          required: false
        - name: redemption-type
          type: string
          required: false
          valid_values: [FULL, PARTIAL, GATE, SIDE_POCKET]
      returns:
        type: uuid
        name: movement_id
    
    record-transfer:
      description: "Transfer units between holders"
      behavior: plugin
      handler: record_transfer
      args:
        - name: from-holding-id
          type: uuid
          required: true
        - name: to-holding-id
          type: uuid
          required: true
        - name: units
          type: decimal
          required: true
        - name: trade-date
          type: date
          required: false
      returns:
        type: uuid
        name: movement_id
    
    settle-movement:
      description: "Mark a movement as settled"
      behavior: crud
      crud:
        operation: update
        table: movements
        schema: kyc
        key: movement_id
      args:
        - name: movement-id
          type: uuid
          required: true
          maps_to: movement_id
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [SETTLED, FAILED, CANCELLED]
        - name: settlement-date
          type: date
          required: false
          maps_to: settlement_date
      returns:
        type: affected
    
    list-movements:
      description: "List movements for a share class"
      behavior: crud
      crud:
        operation: list_by_fk
        table: movements
        schema: kyc
        fk_col: share_class_id
      args:
        - name: share-class-id
          type: uuid
          required: true
          maps_to: share_class_id
      returns:
        type: record_set
    
    # ═══════════════════════════════════════════════════════════════════════
    # OWNERSHIP (Entity-to-Entity, non-share-class)
    # ═══════════════════════════════════════════════════════════════════════
    
    add-ownership:
      description: "Add ownership link (for non-share-class ownership)"
      behavior: crud
      crud:
        operation: insert
        table: ownership_links
        schema: kyc
        returning: link_id
      args:
        - name: owner-type
          type: string
          required: true
          valid_values: [ENTITY, PERSON]
        - name: owner-id
          type: uuid
          required: true
        - name: owned-entity-id
          type: uuid
          required: true
          maps_to: owned_entity_id
        - name: percentage
          type: decimal
          required: false
          maps_to: ownership_percentage
        - name: ownership-type
          type: string
          required: false
          maps_to: ownership_type
          valid_values: [DIRECT, INDIRECT, BENEFICIAL]
        - name: effective-date
          type: date
          required: false
          maps_to: effective_date
      returns:
        type: uuid
        name: link_id
        capture: true
    
    remove-ownership:
      description: "End an ownership link"
      behavior: crud
      crud:
        operation: update
        table: ownership_links
        schema: kyc
        key: link_id
        set_values:
          is_active: false
      args:
        - name: link-id
          type: uuid
          required: true
          maps_to: link_id
        - name: end-date
          type: date
          required: false
          maps_to: end_date
      returns:
        type: affected
    
    # ═══════════════════════════════════════════════════════════════════════
    # CONTROL (Non-ownership control)
    # ═══════════════════════════════════════════════════════════════════════
    
    add-control:
      description: "Add control relationship"
      behavior: crud
      crud:
        operation: insert
        table: control_links
        schema: kyc
        returning: link_id
      args:
        - name: controller-type
          type: string
          required: true
          valid_values: [ENTITY, PERSON]
        - name: controller-id
          type: uuid
          required: true
        - name: controlled-entity-id
          type: uuid
          required: true
          maps_to: controlled_entity_id
        - name: control-type
          type: string
          required: true
          maps_to: control_type
          valid_values: [BOARD_CONTROL, VOTING_RIGHTS, VETO_POWER, MANAGEMENT,
                         TRUSTEE, INVESTMENT_DISCRETION, OTHER]
        - name: description
          type: string
          required: false
          maps_to: control_description
      returns:
        type: uuid
        name: link_id
        capture: true
    
    remove-control:
      description: "End a control relationship"
      behavior: crud
      crud:
        operation: update
        table: control_links
        schema: kyc
        key: link_id
        set_values:
          is_active: false
      args:
        - name: link-id
          type: uuid
          required: true
          maps_to: link_id
      returns:
        type: affected
    
    # ═══════════════════════════════════════════════════════════════════════
    # UBO CALCULATION
    # ═══════════════════════════════════════════════════════════════════════
    
    calculate-ubo-chain:
      description: "Calculate UBOs considering share class rights and ownership chain"
      behavior: plugin
      handler: calculate_ubo_chain
      args:
        - name: entity-id
          type: uuid
          required: true
        - name: voting-threshold
          type: decimal
          required: false
          description: "Voting control threshold (default 25)"
        - name: economic-threshold
          type: decimal
          required: false
          description: "Economic interest threshold (default 25)"
      returns:
        type: record_set
        description: "List of persons with effective %, basis"
    
    determine-ubo:
      description: "Record UBO determination for a person"
      behavior: crud
      crud:
        operation: upsert
        table: ubo_determinations
        schema: kyc
        conflict_keys: [cbu_id, person_id]
        returning: determination_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: person-id
          type: uuid
          required: true
          maps_to: person_id
        - name: is-ubo
          type: boolean
          required: true
          maps_to: is_ubo
        - name: ownership-pct
          type: decimal
          required: false
          maps_to: effective_ownership_pct
        - name: voting-pct
          type: decimal
          required: false
          maps_to: effective_voting_pct
        - name: economic-pct
          type: decimal
          required: false
          maps_to: effective_economic_pct
        - name: basis
          type: string
          required: false
          maps_to: determination_basis
        - name: is-control-ubo
          type: boolean
          required: false
          maps_to: is_control_ubo
        - name: control-description
          type: string
          required: false
          maps_to: control_description
      returns:
        type: uuid
        name: determination_id
        capture: false
    
    list-ubos:
      description: "List confirmed UBOs for a CBU"
      behavior: crud
      crud:
        operation: list_by_fk
        table: ubo_determinations
        schema: kyc
        fk_col: cbu_id
        filter:
          is_ubo: true
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
      returns:
        type: record_set
    
    # ═══════════════════════════════════════════════════════════════════════
    # DOCUMENTS
    # ═══════════════════════════════════════════════════════════════════════
    
    require-document:
      description: "Add document requirement"
      behavior: crud
      crud:
        operation: insert
        table: document_requirements
        schema: kyc
        returning: requirement_id
      args:
        - name: person-id
          type: uuid
          required: false
          maps_to: person_id
        - name: entity-id
          type: uuid
          required: false
          maps_to: entity_id
        - name: cbu-id
          type: uuid
          required: false
          maps_to: cbu_id
        - name: document-type
          type: string
          required: true
          maps_to: document_type
          valid_values: [PASSPORT, DRIVERS_LICENSE, NATIONAL_ID, PROOF_OF_ADDRESS,
                         TAX_FORM_W9, TAX_FORM_W8BEN, TAX_FORM_W8IMY,
                         CERT_OF_INCORPORATION, BOARD_RESOLUTION, ARTICLES_OF_ASSOCIATION,
                         SHAREHOLDER_REGISTER, TRUST_DEED, PARTNERSHIP_AGREEMENT,
                         FINANCIAL_STATEMENTS, BANK_REFERENCE,
                         SOURCE_OF_FUNDS, SOURCE_OF_WEALTH,
                         SUBSCRIPTION_AGREEMENT, INVESTOR_QUESTIONNAIRE]
        - name: priority
          type: string
          required: false
          maps_to: priority
          valid_values: [CRITICAL, HIGH, STANDARD, LOW]
        - name: due-date
          type: date
          required: false
          maps_to: due_date
      returns:
        type: uuid
        name: requirement_id
        capture: true
    
    submit-document:
      description: "Submit a document"
      behavior: crud
      crud:
        operation: insert
        table: submitted_documents
        schema: kyc
        returning: document_id
      args:
        - name: requirement-id
          type: uuid
          required: false
          maps_to: requirement_id
        - name: person-id
          type: uuid
          required: false
          maps_to: person_id
        - name: entity-id
          type: uuid
          required: false
          maps_to: entity_id
        - name: document-type
          type: string
          required: true
          maps_to: document_type
        - name: file-reference
          type: string
          required: true
          maps_to: file_reference
        - name: expiry-date
          type: date
          required: false
          maps_to: expiry_date
      returns:
        type: uuid
        name: document_id
        capture: true
    
    verify-document:
      description: "Verify a submitted document"
      behavior: crud
      crud:
        operation: update
        table: submitted_documents
        schema: kyc
        key: document_id
      args:
        - name: document-id
          type: uuid
          required: true
          maps_to: document_id
        - name: verified-by
          type: string
          required: true
          maps_to: verified_by
        - name: status
          type: string
          required: false
          maps_to: status
          valid_values: [VERIFIED, REJECTED]
        - name: rejection-reason
          type: string
          required: false
          maps_to: rejection_reason
      returns:
        type: affected
    
    list-documents:
      description: "List documents for a person or entity"
      behavior: plugin
      handler: list_documents
      args:
        - name: person-id
          type: uuid
          required: false
        - name: entity-id
          type: uuid
          required: false
      returns:
        type: record_set
    
    list-outstanding-docs:
      description: "List unfulfilled document requirements"
      behavior: plugin
      handler: list_outstanding_documents
      args:
        - name: cbu-id
          type: uuid
          required: false
        - name: person-id
          type: uuid
          required: false
      returns:
        type: record_set
    
    # ═══════════════════════════════════════════════════════════════════════
    # SCREENING
    # ═══════════════════════════════════════════════════════════════════════
    
    screen-sanctions:
      description: "Record sanctions screening result"
      behavior: crud
      crud:
        operation: insert
        table: sanctions_screenings
        schema: kyc
        returning: screening_id
      args:
        - name: person-id
          type: uuid
          required: false
          maps_to: person_id
        - name: entity-id
          type: uuid
          required: false
          maps_to: entity_id
        - name: provider
          type: string
          required: true
          maps_to: screening_provider
          valid_values: [REFINITIV, DOW_JONES, LEXISNEXIS, COMPLY_ADVANTAGE, INTERNAL]
        - name: screening-type
          type: string
          required: false
          maps_to: screening_type
          valid_values: [SANCTIONS, PEP, ADVERSE_MEDIA, FULL]
        - name: result
          type: string
          required: true
          maps_to: result
          valid_values: [CLEAR, MATCH, POTENTIAL_MATCH, ERROR]
        - name: match-details
          type: json
          required: false
          maps_to: match_details
      returns:
        type: uuid
        name: screening_id
        capture: true
    
    set-pep-status:
      description: "Set PEP status for a person"
      behavior: crud
      crud:
        operation: update
        table: persons
        schema: kyc
        key: person_id
      args:
        - name: person-id
          type: uuid
          required: true
          maps_to: person_id
        - name: pep-status
          type: string
          required: true
          maps_to: pep_status
          valid_values: [NOT_PEP, PEP, PEP_ASSOCIATE]
      returns:
        type: affected
    
    # ═══════════════════════════════════════════════════════════════════════
    # KYC STATUS
    # ═══════════════════════════════════════════════════════════════════════
    
    set-person-kyc-status:
      description: "Set KYC status for a person"
      behavior: crud
      crud:
        operation: update
        table: persons
        schema: kyc
        key: person_id
      args:
        - name: person-id
          type: uuid
          required: true
          maps_to: person_id
        - name: status
          type: string
          required: true
          maps_to: kyc_status
          valid_values: [NOT_STARTED, IN_PROGRESS, APPROVED, REJECTED, EXPIRED]
        - name: risk-rating
          type: string
          required: false
          maps_to: risk_rating
          valid_values: [LOW, MEDIUM, HIGH, PROHIBITED]
        - name: expiry-date
          type: date
          required: false
          maps_to: kyc_expiry_date
      returns:
        type: affected
    
    set-entity-kyc-status:
      description: "Set KYC status for an entity at a CBU"
      behavior: crud
      crud:
        operation: upsert
        table: entity_kyc_status
        schema: kyc
        conflict_keys: [entity_id, cbu_id]
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: status
          type: string
          required: true
          maps_to: kyc_status
          valid_values: [NOT_STARTED, IN_PROGRESS, PENDING_REVIEW, APPROVED, REJECTED]
        - name: risk-rating
          type: string
          required: false
          maps_to: risk_rating
        - name: reviewer
          type: string
          required: false
          maps_to: reviewer
      returns:
        type: affected
```

**Effort**: 1 day

---

## Phase 3: Plugin Handlers

### File: `rust/src/dsl_v2/custom_ops/kyc_ops.rs`

Implement these plugin handlers:

```rust
// Required plugins:
pub async fn search_persons(pool: &PgPool, args: &HashMap<String, Value>) -> Result<Value>
pub async fn list_holdings_by_holder(pool: &PgPool, args: &HashMap<String, Value>) -> Result<Value>
pub async fn record_subscription(pool: &PgPool, args: &HashMap<String, Value>) -> Result<Value>
pub async fn record_redemption(pool: &PgPool, args: &HashMap<String, Value>) -> Result<Value>
pub async fn record_transfer(pool: &PgPool, args: &HashMap<String, Value>) -> Result<Value>
pub async fn calculate_ubo_chain(pool: &PgPool, args: &HashMap<String, Value>) -> Result<Value>
pub async fn list_documents(pool: &PgPool, args: &HashMap<String, Value>) -> Result<Value>
pub async fn list_outstanding_documents(pool: &PgPool, args: &HashMap<String, Value>) -> Result<Value>
```

Key implementation: `calculate_ubo_chain` must:
1. Walk holdings → share_classes → entities (recursive CTE)
2. Walk ownership_links for non-share-class ownership
3. Calculate effective voting % (weighted by votes_per_share)
4. Calculate effective economic % (weighted by participation rights)
5. Aggregate to person level
6. Apply thresholds
7. Include control-only UBOs

**Effort**: 1.5 days

---

## Phase 4: Graph Builder Updates

### Update: `rust/src/graph/builder.rs`

Add methods:
- `load_persons_layer()` - Officers, UBOs linked to CBU
- `load_share_classes()` - Share classes issued by entities
- `load_holdings()` - Who owns what
- `load_documents_layer()` - Document status

Ensure UBO view shows:
- Entity hierarchy with share classes
- Holdings as edges (with % labels)
- Persons at top with ownership paths highlighted

**Effort**: 0.5 day

---

## Phase 5: Agentic Integration

### Add KYC intent extraction

File: `rust/src/agentic/prompts/kyc_intent_extraction_system.md`

Patterns to recognize:
- "Add John Smith as CIO" → add-person + link-person-to-cbu
- "Add Class A shares with voting rights" → add-share-class
- "John owns 60% of Class A" → add-holding
- "Map ownership structure" → series of add-holding + calculate-ubo-chain
- "Record subscription of 1000 units" → record-subscription

### CLI Extension

```bash
dsl_cli kyc -i "Add John Smith as CIO and authorized signatory for Apex Capital"
dsl_cli kyc -i "Create Class A voting shares and Class B non-voting shares for Apex"
dsl_cli kyc -i "John Smith holds 60% of Class A, Apex Holdings holds 40%"
dsl_cli kyc -i "Record $1M subscription to Institutional USD class for investor account 12345"
```

**Effort**: 1 day

---

## Phase 6: Testing & Validation

### Test Scenarios

1. **Institutional KYC**
   - Create CBU
   - Add officers (CIO, CFO, Directors)
   - Add share classes (voting/non-voting)
   - Add holdings
   - Calculate UBOs
   - Verify documents

2. **Fund Setup (Hedge Fund)**
   - Create fund entity
   - Add share class with HF attributes (lock-up, HWM, performance fee)
   - Add GP/LP structure
   - Record subscriptions
   - Process redemption with gate

3. **Fund Setup (UCITS)**
   - Create fund entity
   - Add retail share class (daily NAV, T+1 settlement)
   - Add institutional share class (lower fees)
   - Record subscriptions
   - Process redemptions

4. **Investor Registry (KYCaaS)**
   - Bulk add investors
   - Record subscriptions
   - Track KYC status per investor
   - List outstanding KYC

**Effort**: 1 day

---

## Summary

| Phase | Description | Effort |
|-------|-------------|--------|
| 0 | Audit existing infrastructure | 0.5 day |
| 1 | Database schema | 1 day |
| 2 | KYC verb definitions | 1 day |
| 3 | Plugin handlers | 1.5 days |
| 4 | Graph builder updates | 0.5 day |
| 5 | Agentic integration | 1 day |
| 6 | Testing | 1 day |
| **Total** | | **6.5 days** |

---

## Key Design Decisions

1. **Single KYC domain** - Not separate kyc/ubo domains
2. **Clearstream-style registry** - Security → Position → Movement
3. **Share class as ownership instrument** - Not just percentages
4. **Hedge fund vs Long fund** - Same schema, different fields populated
5. **Person reuse** - Same person can be officer, UBO, and investor
6. **Holdings-based UBO calculation** - Walks share classes, respects voting rights

---

## Demo Flow After Implementation

```bash
# 1. Create institutional client
dsl_cli custody -i "Onboard Apex Capital for US equities" --execute

# 2. Add corporate structure
dsl_cli kyc -i "Apex Capital has Class A voting shares (100 shares) and Class B non-voting (1000 shares). John Smith holds 60 Class A shares. Apex Holdings holds 40 Class A shares and 500 Class B shares." --execute

# 3. Add officers
dsl_cli kyc -i "Add John Smith as CEO and authorized signatory. Add Sarah Jones as CFO." --execute

# 4. Calculate UBOs
dsl_cli kyc -i "Calculate UBOs for Apex Capital with 25% threshold" --execute

# 5. Add document requirements
dsl_cli kyc -i "Require passport and proof of address for John Smith. Require cert of incorporation for Apex Capital." --execute

# 6. Visualize
# Browser shows CBU with:
# - Officers ring (John, Sarah)
# - Share classes (Class A, Class B)  
# - Holdings (John → 60% Class A, Apex Holdings → 40% Class A)
# - UBO determination (John = UBO via 60% voting control)
```

---

*End of Implementation Plan*
