# TODO: Capital Structure & Ownership Model

> **Status:** Planning
> **Priority:** High - Completes GLEIF/BODS/Register triangulation
> **Created:** 2026-01-10
> **Estimated Effort:** 50-55 hours
> **Dependencies:** None (new schema extension)

---

## MANDATORY: Read Before Implementing

**Claude MUST read these documentation sections before writing ANY code:**

```bash
# REQUIRED READING - Execute these view commands first:

# 1. Core patterns and conventions
view /Users/adamtc007/Developer/ob-poc/CLAUDE.md

# 2. DSL verb definition patterns
view /Users/adamtc007/Developer/ob-poc/docs/verb-definition-spec.md

# 3. Custom operation handler patterns  
view /Users/adamtc007/Developer/ob-poc/docs/dsl-verb-flow.md

# 4. Taxonomy and graph model
view /Users/adamtc007/Developer/ob-poc/docs/entity-model-ascii.md

# 5. Existing share_classes schema (understand what exists)
view /Users/adamtc007/Developer/ob-poc/migrations/009_kyc_control_schema.sql

# 6. Agent architecture for lexicon integration
view /Users/adamtc007/Developer/ob-poc/docs/agent-architecture.md

# 7. egui viewport patterns
view /Users/adamtc007/Developer/ob-poc/docs/repl-viewport.md
```

**DO NOT proceed without reading these files. They contain mandatory patterns.**

---

## Problem Statement

The ownership model has three data sources that don't connect:

```
GLEIF (corporate hierarchy)     BODS (beneficial ownership)     REGISTER (holdings)
        │                               │                              │
        │ ownership_percentage          │ share_min/max               │ units held
        │ (declared)                    │ (declared)                  │ (factual)
        │                               │                              │
        └───────────────────────────────┴──────────────────────────────┘
                                        │
                                        ▼
                              ┌─────────────────────┐
                              │       THE GAP       │
                              │                     │
                              │ • No issuance ledger│
                              │ • No denominator    │
                              │ • No voting calc    │
                              │ • No control flags  │
                              │ • Can't reconcile   │
                              └─────────────────────┘
```

**Without this, we cannot:**
1. Compute who controls a company (voting majority)
2. Reconcile BODS declarations against actual holdings
3. Trace ownership chains through corporate structures
4. Render cap tables in the viewport

---

## Target State

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         COMPLETE OWNERSHIP MODEL                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  EXTERNAL ASSERTIONS                    INTERNAL REGISTER                   │
│                                                                              │
│  ┌─────────────┐                        ┌─────────────────┐                │
│  │   GLEIF     │                        │  share_classes  │                │
│  │ (corporate  │                        │  + identifiers  │                │
│  │  hierarchy) │                        │  + supply state │                │
│  └──────┬──────┘                        └────────┬────────┘                │
│         │                                        │                          │
│         │                               ┌────────┴────────┐                │
│  ┌──────┴──────┐                        │                 │                │
│  │    BODS     │                  ┌─────┴─────┐    ┌──────┴──────┐        │
│  │ (beneficial │                  │ issuance_ │    │  holdings   │        │
│  │  ownership) │                  │  events   │    │ (investor   │        │
│  └──────┬──────┘                  │ (supply   │    │  positions) │        │
│         │                         │  ledger)  │    └──────┬──────┘        │
│         │                         └─────┬─────┘           │                │
│         │                               │                 │                │
│         └───────────────┬───────────────┴─────────────────┘                │
│                         │                                                   │
│                         ▼                                                   │
│              ┌─────────────────────┐                                       │
│              │ ownership_snapshots │  ← COMPUTED from register             │
│              │                     │  ← IMPORTED from BODS/GLEIF           │
│              │ • basis: VOTES/ECON │  ← RECONCILED across sources          │
│              │ • derived_from      │                                       │
│              │ • as_of_date        │                                       │
│              └──────────┬──────────┘                                       │
│                         │                                                   │
│              ┌──────────┴──────────┐                                       │
│              │   TAXONOMY NODE     │  ← Renderable in egui viewport        │
│              │   ControlPosition   │                                       │
│              │   ShareClass        │                                       │
│              └─────────────────────┘                                       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase 1: Database Schema (Migration 016)
### Phase 2: SQL Functions (As-Of Computation)
### Phase 3: Verb YAML Definitions
### Phase 4: Plugin Handlers (Rust)
### Phase 5: Agent Lexicon Integration
### Phase 6: Taxonomy Extension
### Phase 7: Graph API Endpoints
### Phase 8: egui Rendering Rules
### Phase 9: Testing & Validation
### Phase 10: Documentation

---

## Phase 1: Database Schema

**File:** `migrations/016_capital_structure_ownership.sql`

**Claude: Before writing this migration, read:**
- `migrations/009_kyc_control_schema.sql` (existing share_classes extensions)
- `CLAUDE.md` section on migration patterns

### 1.1 Instrument Identifier Schemes

```sql
-- ============================================================================
-- 1.1: Instrument Identifier Schemes
-- ============================================================================
-- Handles both listed (ISIN, SEDOL) and private (INTERNAL, FUND_ADMIN) securities

CREATE TABLE IF NOT EXISTS kyc.instrument_identifier_schemes (
    scheme_code VARCHAR(20) PRIMARY KEY,
    scheme_name VARCHAR(100) NOT NULL,
    issuing_authority VARCHAR(100),
    format_regex VARCHAR(200),
    is_global BOOLEAN DEFAULT false,
    validation_url VARCHAR(500),
    display_order INTEGER DEFAULT 100,
    created_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE kyc.instrument_identifier_schemes IS 
    'Reference table for security identifier types (ISIN, SEDOL, CUSIP, INTERNAL, etc.)';

-- Seed data
INSERT INTO kyc.instrument_identifier_schemes (scheme_code, scheme_name, issuing_authority, format_regex, is_global, display_order) VALUES
    ('ISIN', 'International Securities Identification Number', 'ISO 6166', '^[A-Z]{2}[A-Z0-9]{9}[0-9]$', true, 1),
    ('SEDOL', 'Stock Exchange Daily Official List', 'LSE', '^[B-DF-HJ-NP-TV-Z0-9]{7}$', false, 2),
    ('CUSIP', 'Committee on Uniform Securities Identification', 'CUSIP Global Services', '^[0-9A-Z]{9}$', false, 3),
    ('FIGI', 'Financial Instrument Global Identifier', 'Bloomberg', '^BBG[A-Z0-9]{9}$', true, 4),
    ('LEI', 'Legal Entity Identifier', 'GLEIF', '^[A-Z0-9]{20}$', true, 5),
    ('INTERNAL', 'Internal Reference', NULL, NULL, false, 99),
    ('FUND_ADMIN', 'Fund Administrator ID', NULL, NULL, false, 10),
    ('REGISTRY', 'Share Registry Number', NULL, NULL, false, 11),
    ('TA_REF', 'Transfer Agent Reference', NULL, NULL, false, 12)
ON CONFLICT (scheme_code) DO NOTHING;
```

### 1.2 Share Class Identifiers

```sql
-- ============================================================================
-- 1.2: Share Class Identifiers (many-to-one)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.share_class_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_class_id UUID NOT NULL REFERENCES kyc.share_classes(id) ON DELETE CASCADE,
    scheme_code VARCHAR(20) NOT NULL REFERENCES kyc.instrument_identifier_schemes(scheme_code),
    identifier_value VARCHAR(100) NOT NULL,
    is_primary BOOLEAN DEFAULT false,
    valid_from DATE DEFAULT CURRENT_DATE,
    valid_to DATE,  -- NULL = current
    source VARCHAR(50),  -- GLEIF, BLOOMBERG, MANUAL, FUND_ADMIN
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT uq_share_class_scheme_value UNIQUE (share_class_id, scheme_code, identifier_value)
);

-- Only one primary identifier per share class at a time
CREATE UNIQUE INDEX idx_share_class_primary_identifier 
    ON kyc.share_class_identifiers(share_class_id) 
    WHERE is_primary = true AND valid_to IS NULL;

CREATE INDEX idx_share_class_identifiers_lookup 
    ON kyc.share_class_identifiers(scheme_code, identifier_value) 
    WHERE valid_to IS NULL;

COMMENT ON TABLE kyc.share_class_identifiers IS 
    'Security identifiers for share classes. Every class has at least INTERNAL. External IDs (ISIN, etc.) optional.';
```

### 1.3 Extend Share Classes

```sql
-- ============================================================================
-- 1.3: Extend share_classes for control computation
-- ============================================================================

ALTER TABLE kyc.share_classes
    -- Instrument classification
    ADD COLUMN IF NOT EXISTS instrument_kind VARCHAR(30) DEFAULT 'FUND_UNIT',
    
    -- Voting rights (critical for control)
    ADD COLUMN IF NOT EXISTS votes_per_unit NUMERIC(10,4) DEFAULT 1.0,
    ADD COLUMN IF NOT EXISTS voting_cap_pct NUMERIC(5,2),
    ADD COLUMN IF NOT EXISTS voting_threshold_pct NUMERIC(5,2),
    
    -- Economic rights
    ADD COLUMN IF NOT EXISTS economic_per_unit NUMERIC(10,4) DEFAULT 1.0,
    ADD COLUMN IF NOT EXISTS dividend_rate NUMERIC(10,4),
    ADD COLUMN IF NOT EXISTS liquidation_rank INTEGER DEFAULT 100,
    
    -- Conversion (for convertibles, warrants)
    ADD COLUMN IF NOT EXISTS converts_to_share_class_id UUID REFERENCES kyc.share_classes(id),
    ADD COLUMN IF NOT EXISTS conversion_ratio NUMERIC(10,4),
    ADD COLUMN IF NOT EXISTS conversion_price NUMERIC(20,6),
    
    -- LP/PE specific
    ADD COLUMN IF NOT EXISTS commitment_currency VARCHAR(3),
    ADD COLUMN IF NOT EXISTS vintage_year INTEGER,
    ADD COLUMN IF NOT EXISTS is_carried_interest BOOLEAN DEFAULT false;

-- Add constraint for instrument_kind
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'chk_instrument_kind'
        AND constraint_schema = 'kyc'
    ) THEN
        ALTER TABLE kyc.share_classes
            ADD CONSTRAINT chk_instrument_kind CHECK (
                instrument_kind IS NULL OR instrument_kind IN (
                    'ORDINARY_EQUITY', 'PREFERENCE_EQUITY', 'DEFERRED_EQUITY',
                    'FUND_UNIT', 'FUND_SHARE', 'LP_INTEREST', 'GP_INTEREST',
                    'DEBT', 'CONVERTIBLE', 'WARRANT', 'OTHER'
                )
            );
    END IF;
END $$;

COMMENT ON COLUMN kyc.share_classes.votes_per_unit IS 
    '0 = non-voting, 1 = standard, >1 = super-voting (founder shares)';
COMMENT ON COLUMN kyc.share_classes.instrument_kind IS
    'Determines calculation method for ownership/control derivation';
COMMENT ON COLUMN kyc.share_classes.liquidation_rank IS
    'Priority in liquidation. Lower = more senior. 100 = common equity.';
```

### 1.4 Share Class Supply (Current State)

```sql
-- ============================================================================
-- 1.4: Share Class Supply (current state - materialized for fast reads)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.share_class_supply (
    supply_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_class_id UUID NOT NULL REFERENCES kyc.share_classes(id) ON DELETE CASCADE,
    
    -- Supply figures
    authorized_units NUMERIC(20,6),
    issued_units NUMERIC(20,6) NOT NULL DEFAULT 0,
    outstanding_units NUMERIC(20,6) NOT NULL DEFAULT 0,
    treasury_units NUMERIC(20,6) DEFAULT 0,
    reserved_units NUMERIC(20,6) DEFAULT 0,
    
    -- Derived totals
    total_votes NUMERIC(20,6) GENERATED ALWAYS AS (issued_units * COALESCE(
        (SELECT votes_per_unit FROM kyc.share_classes WHERE id = share_class_id), 1
    )) STORED,
    total_economic NUMERIC(20,6) GENERATED ALWAYS AS (issued_units * COALESCE(
        (SELECT economic_per_unit FROM kyc.share_classes WHERE id = share_class_id), 1
    )) STORED,
    
    -- As-of tracking
    as_of_date DATE NOT NULL DEFAULT CURRENT_DATE,
    as_of_event_id UUID,
    
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT uq_supply_class_date UNIQUE (share_class_id, as_of_date)
);

-- Note: Generated columns may need to be regular columns updated by trigger
-- depending on PostgreSQL version. Alternative:

CREATE OR REPLACE FUNCTION kyc.fn_update_supply_totals()
RETURNS TRIGGER AS $$
BEGIN
    SELECT 
        NEW.issued_units * COALESCE(sc.votes_per_unit, 1),
        NEW.issued_units * COALESCE(sc.economic_per_unit, 1)
    INTO NEW.total_votes, NEW.total_economic
    FROM kyc.share_classes sc
    WHERE sc.id = NEW.share_class_id;
    
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_supply_totals
    BEFORE INSERT OR UPDATE ON kyc.share_class_supply
    FOR EACH ROW EXECUTE FUNCTION kyc.fn_update_supply_totals();

CREATE INDEX idx_supply_class ON kyc.share_class_supply(share_class_id);
CREATE INDEX idx_supply_date ON kyc.share_class_supply(as_of_date DESC);

COMMENT ON TABLE kyc.share_class_supply IS 
    'Current supply state per share class. Source of truth for denominators in control computation.';
```

### 1.5 Issuance Events (Supply Ledger)

```sql
-- ============================================================================
-- 1.5: Issuance Events (append-only supply ledger)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.issuance_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What
    share_class_id UUID NOT NULL REFERENCES kyc.share_classes(id),
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Event type
    event_type VARCHAR(30) NOT NULL,
    
    -- Quantities
    units_delta NUMERIC(20,6) NOT NULL,
    
    -- For splits/consolidations
    ratio_from INTEGER,
    ratio_to INTEGER,
    
    -- Pricing
    price_per_unit NUMERIC(20,6),
    price_currency VARCHAR(3),
    total_amount NUMERIC(20,2),
    
    -- Dates
    effective_date DATE NOT NULL,
    announcement_date DATE,
    record_date DATE,
    
    -- Status
    status VARCHAR(20) DEFAULT 'EFFECTIVE',
    
    -- Provenance
    board_resolution_ref VARCHAR(100),
    regulatory_filing_ref VARCHAR(100),
    source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    
    -- Audit
    created_by VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT now(),
    notes TEXT,
    
    CONSTRAINT chk_event_type CHECK (event_type IN (
        'INITIAL_ISSUE', 'NEW_ISSUE', 'STOCK_SPLIT', 'BONUS_ISSUE',
        'CANCELLATION', 'BUYBACK', 'CONSOLIDATION',
        'TREASURY_RELEASE', 'TREASURY_TRANSFER',
        'MERGER_IN', 'MERGER_OUT', 'SPINOFF', 'CONVERSION'
    )),
    CONSTRAINT chk_event_status CHECK (status IN (
        'DRAFT', 'PENDING_APPROVAL', 'EFFECTIVE', 'REVERSED', 'CANCELLED'
    )),
    CONSTRAINT chk_split_ratio CHECK (
        (event_type NOT IN ('STOCK_SPLIT', 'CONSOLIDATION')) OR
        (ratio_from IS NOT NULL AND ratio_to IS NOT NULL AND ratio_from > 0 AND ratio_to > 0)
    )
);

CREATE INDEX idx_issuance_class ON kyc.issuance_events(share_class_id, effective_date);
CREATE INDEX idx_issuance_issuer ON kyc.issuance_events(issuer_entity_id);
CREATE INDEX idx_issuance_status ON kyc.issuance_events(status) WHERE status = 'EFFECTIVE';

COMMENT ON TABLE kyc.issuance_events IS 
    'Append-only ledger of supply changes. Source for computing share_class_supply at any as-of date.';
```

### 1.6 Dilution Instruments (Options, Warrants, Convertibles)

```sql
-- ============================================================================
-- 1.6: Dilution Instruments
-- ============================================================================
-- Tracks potential future dilution from options, warrants, convertibles, SAFEs.
-- Required for FULLY_DILUTED basis computation.

CREATE TABLE IF NOT EXISTS kyc.dilution_instruments (
    instrument_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What company/fund this dilutes
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- What share class it converts INTO (the diluted class)
    converts_to_share_class_id UUID NOT NULL REFERENCES kyc.share_classes(id),
    
    -- Instrument type
    instrument_type VARCHAR(30) NOT NULL,
    
    -- Who holds this instrument
    holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    
    -- Quantities
    units_granted NUMERIC(20,6) NOT NULL,        -- Options/warrants granted
    units_exercised NUMERIC(20,6) DEFAULT 0,     -- Already exercised
    units_forfeited NUMERIC(20,6) DEFAULT 0,     -- Forfeited/cancelled
    units_outstanding NUMERIC(20,6) GENERATED ALWAYS AS (
        units_granted - units_exercised - units_forfeited
    ) STORED,
    
    -- Conversion terms
    conversion_ratio NUMERIC(10,4) DEFAULT 1.0,  -- How many shares per instrument
    exercise_price NUMERIC(20,6),                -- Strike price (NULL for SAFEs pre-price)
    exercise_currency VARCHAR(3),
    
    -- For SAFEs/Convertible Notes
    valuation_cap NUMERIC(20,2),                 -- Valuation cap
    discount_pct NUMERIC(5,2),                   -- Discount percentage
    principal_amount NUMERIC(20,2),              -- For convertible debt
    
    -- Exercisability
    vesting_start_date DATE,
    vesting_end_date DATE,                       -- Fully vested by this date
    vesting_cliff_months INTEGER,                -- Cliff period
    exercisable_from DATE,                       -- Can exercise after this
    expiration_date DATE,                        -- Must exercise by this
    
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE',
    
    -- For computing "exercisable now" vs "fully diluted"
    is_vested BOOLEAN GENERATED ALWAYS AS (
        vesting_end_date IS NULL OR vesting_end_date <= CURRENT_DATE
    ) STORED,
    is_exercisable BOOLEAN GENERATED ALWAYS AS (
        (exercisable_from IS NULL OR exercisable_from <= CURRENT_DATE) AND
        (expiration_date IS NULL OR expiration_date > CURRENT_DATE) AND
        status = 'ACTIVE'
    ) STORED,
    
    -- Provenance
    grant_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    board_approval_ref VARCHAR(100),
    plan_name VARCHAR(100),                      -- e.g., "2024 Stock Option Plan"
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    notes TEXT,
    
    CONSTRAINT chk_instrument_type CHECK (instrument_type IN (
        'STOCK_OPTION',           -- Employee/advisor options
        'WARRANT',                -- Investor warrants
        'CONVERTIBLE_NOTE',       -- Convertible debt
        'SAFE',                   -- Simple Agreement for Future Equity
        'CONVERTIBLE_PREFERRED',  -- Preferred that converts to common
        'RSU',                    -- Restricted Stock Units
        'PHANTOM_STOCK',          -- Phantom/synthetic equity
        'SAR',                    -- Stock Appreciation Rights
        'OTHER'
    )),
    CONSTRAINT chk_dilution_status CHECK (status IN (
        'ACTIVE', 'EXERCISED', 'EXPIRED', 'FORFEITED', 'CANCELLED'
    )),
    CONSTRAINT chk_units_positive CHECK (units_granted > 0),
    CONSTRAINT chk_exercised_lte_granted CHECK (units_exercised <= units_granted)
);

CREATE INDEX idx_dilution_issuer ON kyc.dilution_instruments(issuer_entity_id) 
    WHERE status = 'ACTIVE';
CREATE INDEX idx_dilution_converts_to ON kyc.dilution_instruments(converts_to_share_class_id);
CREATE INDEX idx_dilution_holder ON kyc.dilution_instruments(holder_entity_id) 
    WHERE holder_entity_id IS NOT NULL;
CREATE INDEX idx_dilution_exercisable ON kyc.dilution_instruments(issuer_entity_id) 
    WHERE is_exercisable = true;

COMMENT ON TABLE kyc.dilution_instruments IS 
    'Options, warrants, convertibles, SAFEs that may dilute existing shareholders. Required for FULLY_DILUTED computation.';
COMMENT ON COLUMN kyc.dilution_instruments.units_outstanding IS 
    'Computed: granted - exercised - forfeited. The potential dilution.';
COMMENT ON COLUMN kyc.dilution_instruments.is_exercisable IS 
    'Computed: within exercise window and active. Used for EXERCISABLE basis.';

-- ============================================================================
-- Dilution Exercise Events (audit trail)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.dilution_exercise_events (
    exercise_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instrument_id UUID NOT NULL REFERENCES kyc.dilution_instruments(instrument_id),
    
    -- Exercise details
    units_exercised NUMERIC(20,6) NOT NULL,
    exercise_date DATE NOT NULL,
    exercise_price_paid NUMERIC(20,6),
    
    -- Resulting shares
    shares_issued NUMERIC(20,6) NOT NULL,  -- units_exercised * conversion_ratio
    resulting_holding_id UUID REFERENCES kyc.holdings(id),
    
    -- For cashless exercise
    is_cashless BOOLEAN DEFAULT false,
    shares_withheld_for_tax NUMERIC(20,6),
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    notes TEXT
);

CREATE INDEX idx_exercise_instrument ON kyc.dilution_exercise_events(instrument_id);

COMMENT ON TABLE kyc.dilution_exercise_events IS 
    'Audit trail of option/warrant exercises. Links to resulting holdings.';
```

### 1.7 Issuer Control Configuration

```sql
-- ============================================================================
-- 1.7: Issuer Control Configuration
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.issuer_control_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Thresholds (jurisdiction/articles dependent)
    control_threshold_pct NUMERIC(5,2) DEFAULT 50.00,
    significant_threshold_pct NUMERIC(5,2) DEFAULT 25.00,
    material_threshold_pct NUMERIC(5,2) DEFAULT 10.00,
    disclosure_threshold_pct NUMERIC(5,2) DEFAULT 5.00,
    
    -- Basis for computation
    control_basis VARCHAR(20) DEFAULT 'VOTES',
    disclosure_basis VARCHAR(20) DEFAULT 'ECONOMIC',
    voting_basis VARCHAR(20) DEFAULT 'OUTSTANDING',
    
    -- Jurisdiction rules
    jurisdiction VARCHAR(10),
    applies_voting_caps BOOLEAN DEFAULT false,
    
    -- Temporal
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,
    
    created_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT chk_control_basis CHECK (control_basis IN ('VOTES', 'ECONOMIC', 'UNITS')),
    CONSTRAINT chk_disclosure_basis CHECK (disclosure_basis IN ('VOTES', 'ECONOMIC', 'UNITS')),
    CONSTRAINT chk_voting_basis CHECK (voting_basis IN (
        'ISSUED', 'OUTSTANDING', 'FULLY_DILUTED', 'EXERCISABLE'
    )),
    CONSTRAINT uq_issuer_control_config UNIQUE (issuer_entity_id, effective_from)
);

CREATE INDEX idx_control_config_issuer ON kyc.issuer_control_config(issuer_entity_id) 
    WHERE effective_to IS NULL;

COMMENT ON TABLE kyc.issuer_control_config IS 
    'Jurisdiction/articles-specific thresholds for control determination per issuer.';
```

### 1.8 Special Rights (Unified: Class + Holder)

```sql
-- ============================================================================
-- 1.8: Special Rights (class-level OR holder-level)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.special_rights (
    right_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Scope: exactly one of these must be set
    share_class_id UUID REFERENCES kyc.share_classes(id),
    holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    
    -- Always required
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Right type
    right_type VARCHAR(30) NOT NULL,
    
    -- Conditions
    threshold_pct NUMERIC(5,2),
    threshold_basis VARCHAR(20),
    requires_class_vote BOOLEAN DEFAULT false,
    
    -- For board rights
    board_seats INTEGER,
    board_seat_type VARCHAR(20),
    
    -- Provenance
    source_type VARCHAR(20),
    source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    source_clause_ref VARCHAR(50),
    
    -- Temporal
    effective_from DATE,
    effective_to DATE,
    
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT chk_right_scope CHECK (
        (share_class_id IS NOT NULL AND holder_entity_id IS NULL) OR
        (share_class_id IS NULL AND holder_entity_id IS NOT NULL)
    ),
    CONSTRAINT chk_right_type CHECK (right_type IN (
        'BOARD_APPOINTMENT', 'BOARD_OBSERVER', 'VETO_MA', 'VETO_FUNDRAISE',
        'VETO_DIVIDEND', 'VETO_LIQUIDATION', 'ANTI_DILUTION', 'DRAG_ALONG',
        'TAG_ALONG', 'FIRST_REFUSAL', 'REDEMPTION', 'CONVERSION_TRIGGER',
        'PROTECTIVE_PROVISION', 'INFORMATION_RIGHTS', 'OTHER'
    )),
    CONSTRAINT chk_source_type CHECK (source_type IS NULL OR source_type IN (
        'ARTICLES', 'SHA', 'SIDE_LETTER', 'BOARD_RESOLUTION', 'INVESTMENT_AGREEMENT'
    )),
    CONSTRAINT chk_board_seat_type CHECK (board_seat_type IS NULL OR board_seat_type IN (
        'DIRECTOR', 'OBSERVER', 'ALTERNATE', 'CHAIRMAN'
    ))
);

CREATE INDEX idx_special_rights_class ON kyc.special_rights(share_class_id) 
    WHERE share_class_id IS NOT NULL AND effective_to IS NULL;
CREATE INDEX idx_special_rights_holder ON kyc.special_rights(holder_entity_id) 
    WHERE holder_entity_id IS NOT NULL AND effective_to IS NULL;
CREATE INDEX idx_special_rights_issuer ON kyc.special_rights(issuer_entity_id);

COMMENT ON TABLE kyc.special_rights IS 
    'Control rights not reducible to voting percentage. Attached to either share class or specific holder.';
```

### 1.9 Ownership Snapshots

```sql
-- ============================================================================
-- 1.9: Ownership Snapshots (the bridge)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.ownership_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- The relationship
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    owner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    share_class_id UUID REFERENCES kyc.share_classes(id),
    
    -- Temporal
    as_of_date DATE NOT NULL,
    
    -- Ownership basis
    basis VARCHAR(20) NOT NULL,
    
    -- The numbers
    units NUMERIC(20,6),
    percentage NUMERIC(8,4),
    percentage_min NUMERIC(8,4),
    percentage_max NUMERIC(8,4),
    
    -- Denominator (for audit)
    numerator NUMERIC(20,6),
    denominator NUMERIC(20,6),
    
    -- Provenance
    derived_from VARCHAR(20) NOT NULL,
    
    -- Source references
    source_holding_ids UUID[],
    source_bods_statement_id VARCHAR(100),
    source_gleif_rel_id UUID,
    source_document_id UUID,
    
    -- Flags
    is_direct BOOLEAN DEFAULT true,
    is_aggregated BOOLEAN DEFAULT false,
    confidence VARCHAR(20) DEFAULT 'HIGH',
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    superseded_at TIMESTAMPTZ,
    superseded_by UUID REFERENCES kyc.ownership_snapshots(snapshot_id),
    
    CONSTRAINT chk_snapshot_basis CHECK (basis IN (
        'UNITS', 'VOTES', 'ECONOMIC', 'CAPITAL', 'DECLARED'
    )),
    CONSTRAINT chk_snapshot_source CHECK (derived_from IN (
        'REGISTER', 'BODS', 'GLEIF', 'PSC', 'MANUAL', 'INFERRED'
    )),
    CONSTRAINT chk_snapshot_confidence CHECK (confidence IN (
        'HIGH', 'MEDIUM', 'LOW', 'UNVERIFIED'
    ))
);

CREATE UNIQUE INDEX idx_snapshot_current ON kyc.ownership_snapshots(
    issuer_entity_id, owner_entity_id, share_class_id, as_of_date, basis, derived_from
) WHERE superseded_at IS NULL;

CREATE INDEX idx_snapshot_issuer ON kyc.ownership_snapshots(issuer_entity_id, as_of_date) 
    WHERE superseded_at IS NULL;
CREATE INDEX idx_snapshot_owner ON kyc.ownership_snapshots(owner_entity_id, as_of_date)
    WHERE superseded_at IS NULL;

COMMENT ON TABLE kyc.ownership_snapshots IS 
    'Computed ownership positions from register, or imported from BODS/GLEIF. Bridge for reconciliation.';
```

### 1.10 Reconciliation Tables

```sql
-- ============================================================================
-- 1.10: Reconciliation Framework
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.ownership_reconciliation_runs (
    run_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Scope
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    as_of_date DATE NOT NULL,
    basis VARCHAR(20) NOT NULL,
    
    -- What we're comparing
    source_a VARCHAR(20) NOT NULL,
    source_b VARCHAR(20) NOT NULL,
    
    -- Config
    tolerance_bps INTEGER DEFAULT 100,
    
    -- Results
    status VARCHAR(20) DEFAULT 'RUNNING',
    total_entities INTEGER,
    matched_count INTEGER,
    mismatched_count INTEGER,
    missing_in_a_count INTEGER,
    missing_in_b_count INTEGER,
    
    -- Audit
    started_at TIMESTAMPTZ DEFAULT now(),
    completed_at TIMESTAMPTZ,
    triggered_by VARCHAR(100),
    notes TEXT,
    
    CONSTRAINT chk_recon_status CHECK (status IN (
        'RUNNING', 'COMPLETED', 'FAILED', 'CANCELLED'
    ))
);

CREATE TABLE IF NOT EXISTS kyc.ownership_reconciliation_findings (
    finding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES kyc.ownership_reconciliation_runs(run_id) ON DELETE CASCADE,
    
    -- The entity being reconciled
    owner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Comparison
    source_a_pct NUMERIC(8,4),
    source_b_pct NUMERIC(8,4),
    delta_bps INTEGER,
    
    -- Finding type
    finding_type VARCHAR(30) NOT NULL,
    severity VARCHAR(10),
    
    -- Resolution
    resolution_status VARCHAR(20) DEFAULT 'OPEN',
    resolution_notes TEXT,
    resolved_by VARCHAR(100),
    resolved_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT chk_finding_type CHECK (finding_type IN (
        'MATCH', 'MISMATCH', 'MISSING_IN_REGISTER', 'MISSING_IN_EXTERNAL',
        'ENTITY_NOT_MAPPED', 'BASIS_MISMATCH'
    )),
    CONSTRAINT chk_finding_severity CHECK (severity IS NULL OR severity IN (
        'INFO', 'WARN', 'ERROR', 'CRITICAL'
    )),
    CONSTRAINT chk_resolution_status CHECK (resolution_status IN (
        'OPEN', 'ACKNOWLEDGED', 'INVESTIGATING', 'RESOLVED', 'FALSE_POSITIVE'
    ))
);

CREATE INDEX idx_recon_findings_run ON kyc.ownership_reconciliation_findings(run_id);
CREATE INDEX idx_recon_findings_open ON kyc.ownership_reconciliation_findings(resolution_status)
    WHERE resolution_status = 'OPEN';
```

### 1.11 BODS Interest Type Mapping

```sql
-- ============================================================================
-- 1.11: BODS Interest Type → Special Rights Mapping
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.bods_right_type_mapping (
    bods_interest_type VARCHAR(50) PRIMARY KEY,
    maps_to_right_type VARCHAR(30) REFERENCES kyc.special_rights(right_type),
    maps_to_control BOOLEAN DEFAULT false,
    maps_to_voting BOOLEAN DEFAULT false,
    maps_to_economic BOOLEAN DEFAULT false,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);

INSERT INTO kyc.bods_right_type_mapping (bods_interest_type, maps_to_right_type, maps_to_control, maps_to_voting, maps_to_economic, notes) VALUES
    ('shareholding', NULL, false, true, true, 'Maps to voting/economic pct computation'),
    ('voting-rights', NULL, true, true, false, 'Maps to voting pct computation'),
    ('right-to-share-in-surplus-assets', NULL, false, false, true, 'Economic only'),
    ('right-to-appoint-and-remove-directors', 'BOARD_APPOINTMENT', true, false, false, NULL),
    ('right-to-appoint-and-remove-members', 'BOARD_APPOINTMENT', true, false, false, NULL),
    ('right-to-exercise-significant-influence-or-control', 'PROTECTIVE_PROVISION', true, false, false, NULL),
    ('rights-under-a-shareholders-agreement', NULL, true, false, false, 'Needs manual review'),
    ('rights-under-articles-of-association', NULL, true, false, false, 'Needs manual review'),
    ('rights-under-a-contract', NULL, false, false, false, 'Needs manual review'),
    ('rights-under-the-law', NULL, false, false, false, 'Jurisdiction specific')
ON CONFLICT (bods_interest_type) DO NOTHING;

COMMENT ON TABLE kyc.bods_right_type_mapping IS 
    'Maps BODS 0.4 interest types to special_rights.right_type for reconciliation.';
```

---

## Phase 2: SQL Functions

**Claude: These functions enable as-of computation. Read the migration first.**

### 2.1 Compute Supply at Date

```sql
-- ============================================================================
-- 2.1: Compute supply at any as-of date from events ledger
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_share_class_supply_at(
    p_share_class_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    share_class_id UUID,
    authorized_units NUMERIC,
    issued_units NUMERIC,
    outstanding_units NUMERIC,
    treasury_units NUMERIC,
    total_votes NUMERIC,
    total_economic NUMERIC,
    as_of_date DATE
) AS $$
DECLARE
    v_votes_per_unit NUMERIC;
    v_economic_per_unit NUMERIC;
    v_authorized NUMERIC;
    v_issued NUMERIC := 0;
    v_treasury NUMERIC := 0;
BEGIN
    -- Get share class attributes
    SELECT 
        sc.votes_per_unit,
        sc.economic_per_unit,
        sc.authorized_shares
    INTO v_votes_per_unit, v_economic_per_unit, v_authorized
    FROM kyc.share_classes sc
    WHERE sc.id = p_share_class_id;
    
    -- Compute issued from events up to as_of
    SELECT COALESCE(SUM(
        CASE 
            WHEN ie.event_type IN ('INITIAL_ISSUE', 'NEW_ISSUE', 'BONUS_ISSUE', 'MERGER_IN', 'TREASURY_RELEASE', 'CONVERSION')
                THEN ie.units_delta
            WHEN ie.event_type IN ('CANCELLATION', 'BUYBACK', 'MERGER_OUT')
                THEN -ABS(ie.units_delta)
            WHEN ie.event_type = 'STOCK_SPLIT'
                THEN (SELECT COALESCE(SUM(units_delta), 0) FROM kyc.issuance_events 
                      WHERE share_class_id = p_share_class_id 
                      AND effective_date < ie.effective_date
                      AND status = 'EFFECTIVE') * (ie.ratio_to::NUMERIC / ie.ratio_from - 1)
            ELSE 0
        END
    ), 0)
    INTO v_issued
    FROM kyc.issuance_events ie
    WHERE ie.share_class_id = p_share_class_id
      AND ie.effective_date <= p_as_of
      AND ie.status = 'EFFECTIVE';
    
    -- Compute treasury
    SELECT COALESCE(SUM(
        CASE 
            WHEN ie.event_type = 'BUYBACK' THEN ie.units_delta
            WHEN ie.event_type IN ('TREASURY_RELEASE', 'TREASURY_TRANSFER') THEN -ie.units_delta
            ELSE 0
        END
    ), 0)
    INTO v_treasury
    FROM kyc.issuance_events ie
    WHERE ie.share_class_id = p_share_class_id
      AND ie.effective_date <= p_as_of
      AND ie.status = 'EFFECTIVE';
    
    RETURN QUERY SELECT
        p_share_class_id,
        v_authorized,
        v_issued,
        v_issued - v_treasury,
        v_treasury,
        v_issued * COALESCE(v_votes_per_unit, 1),
        v_issued * COALESCE(v_economic_per_unit, 1),
        p_as_of;
END;
$$ LANGUAGE plpgsql STABLE;
```

### 2.2 Compute Diluted Supply (FULLY_DILUTED / EXERCISABLE)

```sql
-- ============================================================================
-- 2.2: Compute diluted supply including options, warrants, convertibles
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_diluted_supply_at(
    p_share_class_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE,
    p_basis TEXT DEFAULT 'FULLY_DILUTED'  -- FULLY_DILUTED or EXERCISABLE
)
RETURNS TABLE (
    share_class_id UUID,
    issued_units NUMERIC,
    outstanding_units NUMERIC,
    dilution_units NUMERIC,
    fully_diluted_units NUMERIC,
    total_votes NUMERIC,
    total_economic NUMERIC,
    dilution_source_count INTEGER,
    as_of_date DATE
) AS $$
DECLARE
    v_base RECORD;
    v_dilution NUMERIC := 0;
    v_dilution_count INTEGER := 0;
    v_votes_per_unit NUMERIC;
    v_economic_per_unit NUMERIC;
BEGIN
    -- Get base supply from issuance events
    SELECT * INTO v_base 
    FROM kyc.fn_share_class_supply_at(p_share_class_id, p_as_of);
    
    -- Get voting/economic multipliers
    SELECT sc.votes_per_unit, sc.economic_per_unit
    INTO v_votes_per_unit, v_economic_per_unit
    FROM kyc.share_classes sc
    WHERE sc.id = p_share_class_id;
    
    -- Compute dilution from instruments that convert INTO this share class
    IF p_basis = 'FULLY_DILUTED' THEN
        -- All outstanding instruments (vested or not)
        SELECT 
            COALESCE(SUM(di.units_outstanding * di.conversion_ratio), 0),
            COUNT(*)
        INTO v_dilution, v_dilution_count
        FROM kyc.dilution_instruments di
        WHERE di.converts_to_share_class_id = p_share_class_id
          AND di.status = 'ACTIVE'
          AND (di.expiration_date IS NULL OR di.expiration_date > p_as_of);
          
    ELSIF p_basis = 'EXERCISABLE' THEN
        -- Only currently exercisable instruments
        SELECT 
            COALESCE(SUM(di.units_outstanding * di.conversion_ratio), 0),
            COUNT(*)
        INTO v_dilution, v_dilution_count
        FROM kyc.dilution_instruments di
        WHERE di.converts_to_share_class_id = p_share_class_id
          AND di.status = 'ACTIVE'
          AND di.is_exercisable = true
          AND (di.expiration_date IS NULL OR di.expiration_date > p_as_of);
    ELSE
        -- Default to no dilution for ISSUED/OUTSTANDING
        v_dilution := 0;
        v_dilution_count := 0;
    END IF;
    
    RETURN QUERY SELECT
        p_share_class_id,
        v_base.issued_units,
        v_base.outstanding_units,
        v_dilution,
        v_base.outstanding_units + v_dilution,
        (v_base.outstanding_units + v_dilution) * COALESCE(v_votes_per_unit, 1),
        (v_base.outstanding_units + v_dilution) * COALESCE(v_economic_per_unit, 1),
        v_dilution_count,
        p_as_of;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION kyc.fn_diluted_supply_at IS 
    'Compute supply including potential dilution from options/warrants/convertibles. 
     FULLY_DILUTED = all outstanding instruments. EXERCISABLE = only currently exercisable.';
```

### 2.3 Holder Control Position

```sql
-- ============================================================================
-- 2.3: Compute holder control position at any as-of date
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_holder_control_position(
    p_issuer_entity_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE,
    p_basis TEXT DEFAULT 'VOTES'
)
RETURNS TABLE (
    issuer_entity_id UUID,
    issuer_name TEXT,
    holder_entity_id UUID,
    holder_name TEXT,
    holder_type TEXT,
    holder_units NUMERIC,
    holder_votes NUMERIC,
    holder_economic NUMERIC,
    total_issuer_votes NUMERIC,
    total_issuer_economic NUMERIC,
    voting_pct NUMERIC,
    economic_pct NUMERIC,
    control_threshold_pct NUMERIC,
    significant_threshold_pct NUMERIC,
    has_control BOOLEAN,
    has_significant_influence BOOLEAN,
    has_board_rights BOOLEAN,
    board_seats INTEGER
) AS $$
BEGIN
    RETURN QUERY
    WITH issuer_supply AS (
        -- Aggregate supply across all share classes for issuer
        -- Uses diluted supply function for FULLY_DILUTED/EXERCISABLE basis
        SELECT 
            CASE 
                WHEN p_basis IN ('FULLY_DILUTED', 'EXERCISABLE') THEN
                    SUM(ds.total_votes)
                ELSE
                    SUM(s.total_votes)
            END AS total_votes,
            CASE 
                WHEN p_basis IN ('FULLY_DILUTED', 'EXERCISABLE') THEN
                    SUM(ds.total_economic)
                ELSE
                    SUM(s.total_economic)
            END AS total_economic
        FROM kyc.share_classes sc
        CROSS JOIN LATERAL kyc.fn_share_class_supply_at(sc.id, p_as_of) s
        CROSS JOIN LATERAL kyc.fn_diluted_supply_at(sc.id, p_as_of, p_basis) ds
        WHERE sc.issuer_entity_id = p_issuer_entity_id
    ),
    -- Include dilution instrument holders in positions for FULLY_DILUTED
    dilution_holder_positions AS (
        SELECT 
            di.holder_entity_id AS investor_entity_id,
            SUM(di.units_outstanding * di.conversion_ratio) AS units,
            SUM(di.units_outstanding * di.conversion_ratio * COALESCE(sc.votes_per_unit, 1)) AS votes,
            SUM(di.units_outstanding * di.conversion_ratio * COALESCE(sc.economic_per_unit, 1)) AS economic
        FROM kyc.dilution_instruments di
        JOIN kyc.share_classes sc ON sc.id = di.converts_to_share_class_id
        WHERE sc.issuer_entity_id = p_issuer_entity_id
          AND di.holder_entity_id IS NOT NULL
          AND di.status = 'ACTIVE'
          AND (di.expiration_date IS NULL OR di.expiration_date > p_as_of)
          AND (
              p_basis = 'FULLY_DILUTED' OR 
              (p_basis = 'EXERCISABLE' AND di.is_exercisable = true)
          )
        GROUP BY di.holder_entity_id
    ),
    holder_positions AS (
        -- Aggregate holdings per holder across all classes
        -- Plus dilution instruments for FULLY_DILUTED/EXERCISABLE
        SELECT 
            COALESCE(h.investor_entity_id, dh.investor_entity_id) AS investor_entity_id,
            COALESCE(h.units, 0) + COALESCE(dh.units, 0) AS units,
            COALESCE(h.votes, 0) + COALESCE(dh.votes, 0) AS votes,
            COALESCE(h.economic, 0) + COALESCE(dh.economic, 0) AS economic
        FROM (
            SELECT 
                hld.investor_entity_id,
                SUM(hld.units) AS units,
                SUM(hld.units * COALESCE(sc.votes_per_unit, 1)) AS votes,
                SUM(hld.units * COALESCE(sc.economic_per_unit, 1)) AS economic
            FROM kyc.holdings hld
            JOIN kyc.share_classes sc ON sc.id = hld.share_class_id
            WHERE sc.issuer_entity_id = p_issuer_entity_id
              AND hld.status = 'active'
            GROUP BY hld.investor_entity_id
        ) h
        FULL OUTER JOIN dilution_holder_positions dh 
            ON h.investor_entity_id = dh.investor_entity_id
    ),
    holder_rights AS (
        -- Check for board appointment rights
        SELECT 
            sr.holder_entity_id,
            COALESCE(SUM(sr.board_seats), 0) AS board_seats
        FROM kyc.special_rights sr
        WHERE sr.issuer_entity_id = p_issuer_entity_id
          AND sr.holder_entity_id IS NOT NULL
          AND sr.right_type = 'BOARD_APPOINTMENT'
          AND (sr.effective_to IS NULL OR sr.effective_to > p_as_of)
          AND (sr.effective_from IS NULL OR sr.effective_from <= p_as_of)
        GROUP BY sr.holder_entity_id
    ),
    config AS (
        SELECT 
            COALESCE(icc.control_threshold_pct, 50) AS control_threshold,
            COALESCE(icc.significant_threshold_pct, 25) AS significant_threshold
        FROM kyc.issuer_control_config icc
        WHERE icc.issuer_entity_id = p_issuer_entity_id
          AND (icc.effective_to IS NULL OR icc.effective_to > p_as_of)
          AND icc.effective_from <= p_as_of
        ORDER BY icc.effective_from DESC
        LIMIT 1
    )
    SELECT 
        p_issuer_entity_id,
        ie.name::TEXT,
        hp.investor_entity_id,
        he.name::TEXT,
        het.type_code::TEXT,
        hp.units,
        hp.votes,
        hp.economic,
        isu.total_votes,
        isu.total_economic,
        CASE WHEN isu.total_votes > 0 THEN ROUND((hp.votes / isu.total_votes) * 100, 4) ELSE 0 END,
        CASE WHEN isu.total_economic > 0 THEN ROUND((hp.economic / isu.total_economic) * 100, 4) ELSE 0 END,
        COALESCE(cfg.control_threshold, 50),
        COALESCE(cfg.significant_threshold, 25),
        CASE WHEN isu.total_votes > 0 AND (hp.votes / isu.total_votes) * 100 > COALESCE(cfg.control_threshold, 50) THEN true ELSE false END,
        CASE WHEN isu.total_votes > 0 AND (hp.votes / isu.total_votes) * 100 > COALESCE(cfg.significant_threshold, 25) THEN true ELSE false END,
        COALESCE(hr.board_seats, 0) > 0,
        COALESCE(hr.board_seats, 0)::INTEGER
    FROM holder_positions hp
    CROSS JOIN issuer_supply isu
    LEFT JOIN config cfg ON true
    LEFT JOIN holder_rights hr ON hr.holder_entity_id = hp.investor_entity_id
    JOIN "ob-poc".entities ie ON ie.entity_id = p_issuer_entity_id
    JOIN "ob-poc".entities he ON he.entity_id = hp.investor_entity_id
    LEFT JOIN "ob-poc".entity_types het ON he.entity_type_id = het.entity_type_id
    ORDER BY hp.votes DESC;
END;
$$ LANGUAGE plpgsql STABLE;
```

### 2.4 Derive Ownership Snapshots

```sql
-- ============================================================================
-- 2.4: Derive ownership snapshots from register
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_derive_ownership_snapshots(
    p_issuer_entity_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE
)
RETURNS INTEGER AS $$
DECLARE
    v_count INTEGER := 0;
BEGIN
    -- Supersede existing register-derived snapshots for this issuer/date
    UPDATE kyc.ownership_snapshots
    SET superseded_at = now()
    WHERE issuer_entity_id = p_issuer_entity_id
      AND as_of_date = p_as_of
      AND derived_from = 'REGISTER'
      AND superseded_at IS NULL;
    
    -- Insert VOTES basis snapshots
    INSERT INTO kyc.ownership_snapshots (
        issuer_entity_id, owner_entity_id, share_class_id, as_of_date,
        basis, units, percentage, numerator, denominator,
        derived_from, is_direct, is_aggregated, confidence
    )
    SELECT 
        p_issuer_entity_id,
        hcp.holder_entity_id,
        NULL,  -- Aggregated across classes
        p_as_of,
        'VOTES',
        hcp.holder_units,
        hcp.voting_pct,
        hcp.holder_votes,
        hcp.total_issuer_votes,
        'REGISTER',
        true,
        true,
        'HIGH'
    FROM kyc.fn_holder_control_position(p_issuer_entity_id, p_as_of, 'VOTES') hcp
    WHERE hcp.holder_votes > 0;
    
    GET DIAGNOSTICS v_count = ROW_COUNT;
    
    -- Insert ECONOMIC basis snapshots
    INSERT INTO kyc.ownership_snapshots (
        issuer_entity_id, owner_entity_id, share_class_id, as_of_date,
        basis, units, percentage, numerator, denominator,
        derived_from, is_direct, is_aggregated, confidence
    )
    SELECT 
        p_issuer_entity_id,
        hcp.holder_entity_id,
        NULL,
        p_as_of,
        'ECONOMIC',
        hcp.holder_units,
        hcp.economic_pct,
        hcp.holder_economic,
        hcp.total_issuer_economic,
        'REGISTER',
        true,
        true,
        'HIGH'
    FROM kyc.fn_holder_control_position(p_issuer_entity_id, p_as_of, 'ECONOMIC') hcp
    WHERE hcp.holder_economic > 0;
    
    GET DIAGNOSTICS v_count = v_count + ROW_COUNT;
    
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;
```

---

## Phase 3: Verb YAML Definitions

**Claude: Before writing verb YAML, read:**
- `docs/verb-definition-spec.md`
- Existing verb files in `rust/config/verbs/`

**File:** `rust/config/verbs/capital.yaml`

```yaml
# ============================================================================
# Capital Structure Verbs
# ============================================================================
# Issuer-side supply management: issuance, splits, buybacks

domain: capital

verbs:
  # --------------------------------------------------------------------------
  # Share Class Management
  # --------------------------------------------------------------------------
  
  share-class.create:
    description: Create a new share class for an issuer
    category: create
    handler: custom
    custom_handler: CapitalShareClassCreateOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
        description: The issuing entity
      - name: name
        type: string
        required: true
        description: Share class name (e.g., "Series A Preferred")
      - name: instrument-kind
        type: string
        required: true
        enum: [ORDINARY_EQUITY, PREFERENCE_EQUITY, FUND_UNIT, FUND_SHARE, LP_INTEREST, GP_INTEREST, DEBT, CONVERTIBLE, WARRANT, OTHER]
      - name: votes-per-unit
        type: decimal
        required: false
        default: 1.0
        description: "0 = non-voting, 1 = standard, >1 = super-voting"
      - name: economic-per-unit
        type: decimal
        required: false
        default: 1.0
      - name: currency
        type: string
        required: false
        default: "EUR"
      - name: authorized-units
        type: decimal
        required: false
        description: Maximum units that can be issued
    produces:
      type: share-class
      binding: share_class_id
    example: |
      (capital.share-class.create 
        :issuer-entity-id @acme 
        :name "Series A Preferred"
        :instrument-kind "PREFERENCE_EQUITY"
        :votes-per-unit 0
        :as @series_a)

  share-class.add-identifier:
    description: Add an identifier (ISIN, SEDOL, etc.) to a share class
    category: update
    handler: custom
    custom_handler: CapitalShareClassAddIdentifierOp
    args:
      - name: share-class-id
        type: uuid
        required: true
      - name: scheme
        type: string
        required: true
        enum: [ISIN, SEDOL, CUSIP, FIGI, INTERNAL, FUND_ADMIN, REGISTRY, TA_REF]
      - name: value
        type: string
        required: true
      - name: is-primary
        type: boolean
        required: false
        default: false
    example: |
      (capital.share-class.add-identifier
        :share-class-id @series_a
        :scheme "ISIN"
        :value "LU1234567890"
        :is-primary true)

  # --------------------------------------------------------------------------
  # Issuance Events
  # --------------------------------------------------------------------------
  
  issue.initial:
    description: Initial issuance of shares (incorporation/fund launch)
    category: create
    handler: custom
    custom_handler: CapitalIssueInitialOp
    args:
      - name: share-class-id
        type: uuid
        required: true
      - name: units
        type: decimal
        required: true
      - name: price-per-unit
        type: decimal
        required: false
      - name: effective-date
        type: date
        required: false
        default: today
      - name: board-resolution-ref
        type: string
        required: false
    produces:
      type: issuance-event
      binding: event_id
    example: |
      (capital.issue.initial
        :share-class-id @series_a
        :units 1000000
        :price-per-unit 1.00
        :effective-date "2024-01-15"
        :as @initial_issue)

  issue.new:
    description: Subsequent issuance (capital raise)
    category: create
    handler: custom
    custom_handler: CapitalIssueNewOp
    args:
      - name: share-class-id
        type: uuid
        required: true
      - name: units
        type: decimal
        required: true
      - name: price-per-unit
        type: decimal
        required: true
      - name: effective-date
        type: date
        required: false
        default: today
      - name: board-resolution-ref
        type: string
        required: false
    produces:
      type: issuance-event
      binding: event_id

  split:
    description: Stock split (e.g., 2:1 doubles shares)
    category: create
    handler: custom
    custom_handler: CapitalSplitOp
    args:
      - name: share-class-id
        type: uuid
        required: true
      - name: ratio-from
        type: integer
        required: true
        description: "Original shares (e.g., 1 in 2:1 split)"
      - name: ratio-to
        type: integer
        required: true
        description: "New shares (e.g., 2 in 2:1 split)"
      - name: effective-date
        type: date
        required: false
        default: today
      - name: record-date
        type: date
        required: false
    produces:
      type: issuance-event
      binding: event_id
    example: |
      (capital.split
        :share-class-id @ordinary
        :ratio-from 1
        :ratio-to 2
        :effective-date "2024-06-01")

  buyback:
    description: Share buyback into treasury
    category: create
    handler: custom
    custom_handler: CapitalBuybackOp
    args:
      - name: share-class-id
        type: uuid
        required: true
      - name: units
        type: decimal
        required: true
      - name: price-per-unit
        type: decimal
        required: true
      - name: effective-date
        type: date
        required: false
        default: today
    produces:
      type: issuance-event
      binding: event_id

  cancel:
    description: Permanent share cancellation
    category: create
    handler: custom
    custom_handler: CapitalCancelOp
    args:
      - name: share-class-id
        type: uuid
        required: true
      - name: units
        type: decimal
        required: true
      - name: effective-date
        type: date
        required: false
        default: today
      - name: reason
        type: string
        required: false
    produces:
      type: issuance-event
      binding: event_id

  # --------------------------------------------------------------------------
  # Dilution Instruments (Options, Warrants, Convertibles)
  # --------------------------------------------------------------------------
  
  dilution.grant-options:
    description: Grant stock options to a holder
    category: create
    handler: custom
    custom_handler: CapitalDilutionGrantOptionsOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: converts-to-share-class-id
        type: uuid
        required: true
        description: Share class these options convert into
      - name: holder-entity-id
        type: uuid
        required: true
      - name: units
        type: decimal
        required: true
        description: Number of options granted
      - name: exercise-price
        type: decimal
        required: true
      - name: exercise-currency
        type: string
        required: false
        default: "USD"
      - name: vesting-start-date
        type: date
        required: false
        default: today
      - name: vesting-end-date
        type: date
        required: false
        description: Fully vested by this date
      - name: vesting-cliff-months
        type: integer
        required: false
        default: 12
      - name: expiration-date
        type: date
        required: true
      - name: plan-name
        type: string
        required: false
    produces:
      type: dilution-instrument
      binding: instrument_id
    example: |
      (capital.dilution.grant-options
        :issuer-entity-id @acme
        :converts-to-share-class-id @common
        :holder-entity-id @employee
        :units 10000
        :exercise-price 1.50
        :vesting-cliff-months 12
        :vesting-end-date "2028-01-01"
        :expiration-date "2034-01-01"
        :plan-name "2024 Stock Option Plan"
        :as @employee_options)

  dilution.issue-warrant:
    description: Issue warrants to an investor
    category: create
    handler: custom
    custom_handler: CapitalDilutionIssueWarrantOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: converts-to-share-class-id
        type: uuid
        required: true
      - name: holder-entity-id
        type: uuid
        required: true
      - name: units
        type: decimal
        required: true
      - name: exercise-price
        type: decimal
        required: true
      - name: exercisable-from
        type: date
        required: false
        default: today
      - name: expiration-date
        type: date
        required: true
    produces:
      type: dilution-instrument
      binding: instrument_id
    example: |
      (capital.dilution.issue-warrant
        :issuer-entity-id @acme
        :converts-to-share-class-id @common
        :holder-entity-id @vc_fund
        :units 500000
        :exercise-price 2.00
        :expiration-date "2029-06-30"
        :as @vc_warrants)

  dilution.create-safe:
    description: Create a SAFE (Simple Agreement for Future Equity)
    category: create
    handler: custom
    custom_handler: CapitalDilutionCreateSafeOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: converts-to-share-class-id
        type: uuid
        required: false
        description: May be NULL until priced round
      - name: holder-entity-id
        type: uuid
        required: true
      - name: principal-amount
        type: decimal
        required: true
      - name: valuation-cap
        type: decimal
        required: false
      - name: discount-pct
        type: decimal
        required: false
        description: Discount to next round price (e.g., 20 for 20%)
    produces:
      type: dilution-instrument
      binding: instrument_id
    example: |
      (capital.dilution.create-safe
        :issuer-entity-id @acme
        :holder-entity-id @angel
        :principal-amount 100000
        :valuation-cap 5000000
        :discount-pct 20
        :as @angel_safe)

  dilution.exercise:
    description: Exercise options/warrants and convert to shares
    category: create
    handler: custom
    custom_handler: CapitalDilutionExerciseOp
    args:
      - name: instrument-id
        type: uuid
        required: true
      - name: units
        type: decimal
        required: true
        description: Number of instruments to exercise
      - name: exercise-date
        type: date
        required: false
        default: today
      - name: is-cashless
        type: boolean
        required: false
        default: false
        description: Cashless/net exercise (shares withheld for cost)
    produces:
      type: dilution-exercise-event
      binding: exercise_id
    example: |
      (capital.dilution.exercise
        :instrument-id @employee_options
        :units 5000
        :exercise-date "2026-03-15")

  dilution.forfeit:
    description: Forfeit/cancel unvested or unexercised instruments
    category: update
    handler: custom
    custom_handler: CapitalDilutionForfeitOp
    args:
      - name: instrument-id
        type: uuid
        required: true
      - name: units
        type: decimal
        required: true
      - name: reason
        type: string
        required: false
    example: |
      (capital.dilution.forfeit
        :instrument-id @employee_options
        :units 5000
        :reason "Employee termination")

  dilution.list:
    description: List dilution instruments for an issuer
    category: query
    handler: custom
    custom_handler: CapitalDilutionListOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: status
        type: string
        required: false
        default: "ACTIVE"
        enum: [ACTIVE, EXERCISED, EXPIRED, FORFEITED, ALL]
      - name: instrument-type
        type: string
        required: false
        enum: [STOCK_OPTION, WARRANT, CONVERTIBLE_NOTE, SAFE, RSU, ALL]
    example: |
      (capital.dilution.list
        :issuer-entity-id @acme
        :status "ACTIVE")

  # --------------------------------------------------------------------------
  # Control Configuration
  # --------------------------------------------------------------------------
  
  control-config.set:
    description: Set control thresholds for an issuer
    category: create
    handler: custom
    custom_handler: CapitalControlConfigSetOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: control-threshold-pct
        type: decimal
        required: false
        default: 50
      - name: significant-threshold-pct
        type: decimal
        required: false
        default: 25
      - name: material-threshold-pct
        type: decimal
        required: false
        default: 10
      - name: disclosure-threshold-pct
        type: decimal
        required: false
        default: 5
      - name: control-basis
        type: string
        required: false
        default: "VOTES"
        enum: [VOTES, ECONOMIC, UNITS]
      - name: jurisdiction
        type: string
        required: false
    produces:
      type: issuer-control-config
      binding: config_id
    example: |
      (capital.control-config.set
        :issuer-entity-id @acme
        :control-threshold-pct 50
        :significant-threshold-pct 25
        :jurisdiction "GB")
```

**File:** `rust/config/verbs/ownership.yaml`

```yaml
# ============================================================================
# Ownership Computation & Reconciliation Verbs
# ============================================================================

domain: ownership

verbs:
  # --------------------------------------------------------------------------
  # Ownership Computation
  # --------------------------------------------------------------------------
  
  compute:
    description: Derive ownership snapshots from register holdings
    category: compute
    handler: custom
    custom_handler: OwnershipComputeOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: as-of
        type: date
        required: false
        default: today
      - name: basis
        type: string
        required: false
        default: "VOTES"
        enum: [VOTES, ECONOMIC, BOTH]
    produces:
      type: ownership-computation
      binding: computation_id
    example: |
      (ownership.compute
        :issuer-entity-id @acme
        :as-of "2024-01-15"
        :basis "BOTH")

  snapshot.list:
    description: List ownership snapshots for an issuer
    category: query
    handler: custom
    custom_handler: OwnershipSnapshotListOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: as-of
        type: date
        required: false
        default: today
      - name: derived-from
        type: string
        required: false
        enum: [REGISTER, BODS, GLEIF, PSC, MANUAL, ALL]
        default: "ALL"
      - name: min-pct
        type: decimal
        required: false
        description: Filter to holdings above this percentage
    example: |
      (ownership.snapshot.list
        :issuer-entity-id @acme
        :derived-from "REGISTER"
        :min-pct 5)

  # --------------------------------------------------------------------------
  # Special Rights
  # --------------------------------------------------------------------------
  
  right.add-to-class:
    description: Add a special right attached to a share class
    category: create
    handler: custom
    custom_handler: OwnershipRightAddToClassOp
    args:
      - name: share-class-id
        type: uuid
        required: true
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: right-type
        type: string
        required: true
        enum: [BOARD_APPOINTMENT, BOARD_OBSERVER, VETO_MA, VETO_FUNDRAISE, VETO_DIVIDEND, VETO_LIQUIDATION, ANTI_DILUTION, DRAG_ALONG, TAG_ALONG, FIRST_REFUSAL, REDEMPTION, PROTECTIVE_PROVISION, OTHER]
      - name: board-seats
        type: integer
        required: false
      - name: threshold-pct
        type: decimal
        required: false
      - name: source-type
        type: string
        required: false
        enum: [ARTICLES, SHA, SIDE_LETTER, BOARD_RESOLUTION, INVESTMENT_AGREEMENT]
      - name: source-clause-ref
        type: string
        required: false
    produces:
      type: special-right
      binding: right_id
    example: |
      (ownership.right.add-to-class
        :share-class-id @series_a
        :issuer-entity-id @acme
        :right-type "BOARD_APPOINTMENT"
        :board-seats 2
        :source-type "SHA"
        :source-clause-ref "Section 4.2(a)")

  right.add-to-holder:
    description: Add a special right attached to a specific holder (side letter)
    category: create
    handler: custom
    custom_handler: OwnershipRightAddToHolderOp
    args:
      - name: holder-entity-id
        type: uuid
        required: true
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: right-type
        type: string
        required: true
        enum: [BOARD_APPOINTMENT, BOARD_OBSERVER, VETO_MA, VETO_FUNDRAISE, VETO_DIVIDEND, VETO_LIQUIDATION, ANTI_DILUTION, DRAG_ALONG, TAG_ALONG, FIRST_REFUSAL, INFORMATION_RIGHTS, PROTECTIVE_PROVISION, OTHER]
      - name: board-seats
        type: integer
        required: false
      - name: threshold-pct
        type: decimal
        required: false
      - name: threshold-basis
        type: string
        required: false
        enum: [VOTES, ECONOMIC, UNITS]
      - name: source-type
        type: string
        required: false
        enum: [SHA, SIDE_LETTER, INVESTMENT_AGREEMENT]
      - name: source-clause-ref
        type: string
        required: false
    produces:
      type: special-right
      binding: right_id
    example: |
      (ownership.right.add-to-holder
        :holder-entity-id @sequoia
        :issuer-entity-id @acme
        :right-type "BOARD_APPOINTMENT"
        :board-seats 1
        :threshold-pct 5
        :threshold-basis "ECONOMIC"
        :source-type "SIDE_LETTER")

  # --------------------------------------------------------------------------
  # Reconciliation
  # --------------------------------------------------------------------------
  
  reconcile:
    description: Compare ownership from different sources
    category: compute
    handler: custom
    custom_handler: OwnershipReconcileOp
    args:
      - name: issuer-entity-id
        type: uuid
        required: true
      - name: as-of
        type: date
        required: false
        default: today
      - name: source-a
        type: string
        required: false
        default: "REGISTER"
        enum: [REGISTER, BODS, GLEIF]
      - name: source-b
        type: string
        required: false
        default: "BODS"
        enum: [REGISTER, BODS, GLEIF]
      - name: basis
        type: string
        required: false
        default: "VOTES"
        enum: [VOTES, ECONOMIC]
      - name: tolerance-bps
        type: integer
        required: false
        default: 100
        description: Tolerance in basis points (100 = 1%)
    produces:
      type: reconciliation-run
      binding: run_id
    example: |
      (ownership.reconcile
        :issuer-entity-id @acme
        :source-a "REGISTER"
        :source-b "BODS"
        :basis "VOTES"
        :tolerance-bps 100)

  reconcile.findings:
    description: List findings from a reconciliation run
    category: query
    handler: custom
    custom_handler: OwnershipReconcileFindingsOp
    args:
      - name: run-id
        type: uuid
        required: true
      - name: severity
        type: string
        required: false
        enum: [INFO, WARN, ERROR, CRITICAL, ALL]
        default: "ALL"
      - name: status
        type: string
        required: false
        enum: [OPEN, ACKNOWLEDGED, INVESTIGATING, RESOLVED, FALSE_POSITIVE, ALL]
        default: "OPEN"
```

---

## Phase 4: Plugin Handlers (Rust)

**Claude: Before writing handlers, read:**
- `docs/dsl-verb-flow.md`
- Existing handlers in `rust/src/dsl_v2/custom_ops/`
- Patterns in `CLAUDE.md`

**File:** `rust/src/dsl_v2/custom_ops/capital_ops.rs`

```rust
//! Capital Structure Operations
//!
//! Handles issuer-side supply management: share class creation, issuance events,
//! splits, buybacks, and control configuration.
//!
//! ## Key Tables
//! - kyc.share_classes (extended)
//! - kyc.share_class_identifiers
//! - kyc.share_class_supply
//! - kyc.issuance_events
//! - kyc.issuer_control_config
//!
//! ## Pattern
//! All handlers follow the CustomOperation trait pattern.
//! See docs/dsl-verb-flow.md for the complete flow.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::dsl_v2::custom_ops::CustomOperation;

// ============================================================================
// Share Class Create
// ============================================================================

pub struct CapitalShareClassCreateOp;

#[async_trait]
impl CustomOperation for CapitalShareClassCreateOp {
    fn name(&self) -> &'static str {
        "capital.share-class.create"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Extract required args
        let issuer_entity_id = ctx.resolve_uuid_arg(verb_call, "issuer-entity-id")?;
        let name = ctx.resolve_string_arg(verb_call, "name")?;
        let instrument_kind = ctx.resolve_string_arg(verb_call, "instrument-kind")?;
        
        // Extract optional args with defaults
        let votes_per_unit = ctx.resolve_decimal_arg_or(verb_call, "votes-per-unit", Decimal::ONE)?;
        let economic_per_unit = ctx.resolve_decimal_arg_or(verb_call, "economic-per-unit", Decimal::ONE)?;
        let currency = ctx.resolve_string_arg_or(verb_call, "currency", "EUR".to_string())?;
        let authorized_units = ctx.resolve_decimal_arg_opt(verb_call, "authorized-units")?;
        
        // Validate issuer exists
        let issuer_exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entities WHERE entity_id = $1)"#
        )
        .bind(issuer_entity_id)
        .fetch_one(pool)
        .await?;
        
        if !issuer_exists {
            return Err(anyhow!("Issuer entity {} not found", issuer_entity_id));
        }
        
        // Insert share class
        let share_class_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.share_classes (
                issuer_entity_id, name, instrument_kind, 
                votes_per_unit, economic_per_unit, currency,
                authorized_shares, class_category
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'CORPORATE')
            RETURNING id
            "#
        )
        .bind(issuer_entity_id)
        .bind(&name)
        .bind(&instrument_kind)
        .bind(votes_per_unit)
        .bind(economic_per_unit)
        .bind(&currency)
        .bind(authorized_units)
        .fetch_one(pool)
        .await?;
        
        // Auto-generate INTERNAL identifier
        let internal_ref = format!("SC-{}", &share_class_id.to_string()[..8]);
        sqlx::query(
            r#"
            INSERT INTO kyc.share_class_identifiers (
                share_class_id, scheme_code, identifier_value, is_primary
            ) VALUES ($1, 'INTERNAL', $2, true)
            "#
        )
        .bind(share_class_id)
        .bind(&internal_ref)
        .execute(pool)
        .await?;
        
        // Initialize supply at zero
        sqlx::query(
            r#"
            INSERT INTO kyc.share_class_supply (
                share_class_id, authorized_units, issued_units, outstanding_units, as_of_date
            ) VALUES ($1, $2, 0, 0, CURRENT_DATE)
            "#
        )
        .bind(share_class_id)
        .bind(authorized_units)
        .execute(pool)
        .await?;
        
        // Bind if :as specified
        if let Some(binding) = verb_call.binding() {
            ctx.bind(binding, share_class_id);
        }
        
        tracing::info!(
            "capital.share-class.create: {} ({}) for issuer {}, votes_per_unit={}",
            name, share_class_id, issuer_entity_id, votes_per_unit
        );
        
        Ok(ExecutionResult::Uuid(share_class_id))
    }
}

// ============================================================================
// Initial Issue
// ============================================================================

pub struct CapitalIssueInitialOp;

#[async_trait]
impl CustomOperation for CapitalIssueInitialOp {
    fn name(&self) -> &'static str {
        "capital.issue.initial"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = ctx.resolve_uuid_arg(verb_call, "share-class-id")?;
        let units = ctx.resolve_decimal_arg(verb_call, "units")?;
        let price_per_unit = ctx.resolve_decimal_arg_opt(verb_call, "price-per-unit")?;
        let effective_date = ctx.resolve_date_arg_or(verb_call, "effective-date", 
            NaiveDate::from(chrono::Utc::now().date_naive()))?;
        let board_resolution_ref = ctx.resolve_string_arg_opt(verb_call, "board-resolution-ref")?;
        
        // Get issuer from share class
        let issuer_entity_id: Uuid = sqlx::query_scalar(
            r#"SELECT issuer_entity_id FROM kyc.share_classes WHERE id = $1"#
        )
        .bind(share_class_id)
        .fetch_one(pool)
        .await?;
        
        // Check no prior issuance exists
        let prior_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM kyc.issuance_events 
                WHERE share_class_id = $1 AND status = 'EFFECTIVE'
            )
            "#
        )
        .bind(share_class_id)
        .fetch_one(pool)
        .await?;
        
        if prior_exists {
            return Err(anyhow!(
                "Share class {} already has issuance events. Use capital.issue.new for subsequent issues.",
                share_class_id
            ));
        }
        
        // Insert event
        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.issuance_events (
                share_class_id, issuer_entity_id, event_type, units_delta,
                price_per_unit, effective_date, board_resolution_ref, status
            ) VALUES ($1, $2, 'INITIAL_ISSUE', $3, $4, $5, $6, 'EFFECTIVE')
            RETURNING event_id
            "#
        )
        .bind(share_class_id)
        .bind(issuer_entity_id)
        .bind(units)
        .bind(price_per_unit)
        .bind(effective_date)
        .bind(board_resolution_ref)
        .fetch_one(pool)
        .await?;
        
        // Update supply
        sqlx::query(
            r#"
            UPDATE kyc.share_class_supply
            SET issued_units = $2, 
                outstanding_units = $2,
                as_of_date = $3,
                as_of_event_id = $4,
                updated_at = now()
            WHERE share_class_id = $1
            "#
        )
        .bind(share_class_id)
        .bind(units)
        .bind(effective_date)
        .bind(event_id)
        .execute(pool)
        .await?;
        
        if let Some(binding) = verb_call.binding() {
            ctx.bind(binding, event_id);
        }
        
        tracing::info!(
            "capital.issue.initial: {} units of {} at {:?}/unit",
            units, share_class_id, price_per_unit
        );
        
        Ok(ExecutionResult::Uuid(event_id))
    }
}

// ============================================================================
// TODO: Implement remaining handlers following same pattern
// ============================================================================
// - CapitalIssueNewOp
// - CapitalSplitOp
// - CapitalBuybackOp
// - CapitalCancelOp
// - CapitalControlConfigSetOp
// - CapitalShareClassAddIdentifierOp

// Register all handlers
pub fn register_capital_ops(registry: &mut crate::dsl_v2::CustomOpRegistry) {
    registry.register(Box::new(CapitalShareClassCreateOp));
    registry.register(Box::new(CapitalIssueInitialOp));
    // TODO: Register remaining ops
}
```

**File:** `rust/src/dsl_v2/custom_ops/ownership_ops.rs`

```rust
//! Ownership Computation & Reconciliation Operations
//!
//! Computes ownership snapshots from register, and reconciles against BODS/GLEIF.
//!
//! ## Key Tables
//! - kyc.ownership_snapshots
//! - kyc.special_rights
//! - kyc.ownership_reconciliation_runs
//! - kyc.ownership_reconciliation_findings
//!
//! ## Key Functions
//! - kyc.fn_holder_control_position()
//! - kyc.fn_derive_ownership_snapshots()

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::dsl_v2::custom_ops::CustomOperation;

// ============================================================================
// Ownership Compute
// ============================================================================

pub struct OwnershipComputeOp;

#[async_trait]
impl CustomOperation for OwnershipComputeOp {
    fn name(&self) -> &'static str {
        "ownership.compute"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = ctx.resolve_uuid_arg(verb_call, "issuer-entity-id")?;
        let as_of = ctx.resolve_date_arg_or(verb_call, "as-of",
            NaiveDate::from(chrono::Utc::now().date_naive()))?;
        let _basis = ctx.resolve_string_arg_or(verb_call, "basis", "VOTES".to_string())?;
        
        // Call the derivation function
        let count: i32 = sqlx::query_scalar(
            r#"SELECT kyc.fn_derive_ownership_snapshots($1, $2)"#
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .fetch_one(pool)
        .await?;
        
        tracing::info!(
            "ownership.compute: derived {} snapshots for issuer {} as-of {}",
            count, issuer_entity_id, as_of
        );
        
        // Return count as JSON result
        Ok(ExecutionResult::Record(serde_json::json!({
            "issuer_entity_id": issuer_entity_id,
            "as_of_date": as_of.to_string(),
            "snapshots_created": count
        })))
    }
}

// ============================================================================
// Ownership Reconcile
// ============================================================================

pub struct OwnershipReconcileOp;

#[async_trait]
impl CustomOperation for OwnershipReconcileOp {
    fn name(&self) -> &'static str {
        "ownership.reconcile"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = ctx.resolve_uuid_arg(verb_call, "issuer-entity-id")?;
        let as_of = ctx.resolve_date_arg_or(verb_call, "as-of",
            NaiveDate::from(chrono::Utc::now().date_naive()))?;
        let source_a = ctx.resolve_string_arg_or(verb_call, "source-a", "REGISTER".to_string())?;
        let source_b = ctx.resolve_string_arg_or(verb_call, "source-b", "BODS".to_string())?;
        let basis = ctx.resolve_string_arg_or(verb_call, "basis", "VOTES".to_string())?;
        let tolerance_bps = ctx.resolve_int_arg_or(verb_call, "tolerance-bps", 100)?;
        
        // Create reconciliation run
        let run_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.ownership_reconciliation_runs (
                issuer_entity_id, as_of_date, basis, source_a, source_b, tolerance_bps, status
            ) VALUES ($1, $2, $3, $4, $5, $6, 'RUNNING')
            RETURNING run_id
            "#
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .bind(&basis)
        .bind(&source_a)
        .bind(&source_b)
        .bind(tolerance_bps)
        .fetch_one(pool)
        .await?;
        
        // Get snapshots from both sources
        let snapshots_a: Vec<(Uuid, Decimal)> = sqlx::query_as(
            r#"
            SELECT owner_entity_id, percentage 
            FROM kyc.ownership_snapshots
            WHERE issuer_entity_id = $1 
              AND as_of_date = $2 
              AND derived_from = $3
              AND basis = $4
              AND superseded_at IS NULL
            "#
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .bind(&source_a)
        .bind(&basis)
        .fetch_all(pool)
        .await?;
        
        let snapshots_b: Vec<(Uuid, Decimal)> = sqlx::query_as(
            r#"
            SELECT owner_entity_id, COALESCE(percentage, (percentage_min + percentage_max) / 2)
            FROM kyc.ownership_snapshots
            WHERE issuer_entity_id = $1 
              AND as_of_date = $2 
              AND derived_from = $3
              AND basis = $4
              AND superseded_at IS NULL
            "#
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .bind(&source_b)
        .bind(&basis)
        .fetch_all(pool)
        .await?;
        
        // Build lookup maps
        use std::collections::HashMap;
        let map_a: HashMap<Uuid, Decimal> = snapshots_a.into_iter().collect();
        let map_b: HashMap<Uuid, Decimal> = snapshots_b.into_iter().collect();
        
        let mut matched = 0;
        let mut mismatched = 0;
        let mut missing_in_a = 0;
        let mut missing_in_b = 0;
        
        // Compare A against B
        for (entity_id, pct_a) in &map_a {
            if let Some(pct_b) = map_b.get(entity_id) {
                let delta_bps = ((pct_a - pct_b).abs() * Decimal::from(10000)).to_i32().unwrap_or(0);
                let (finding_type, severity) = if delta_bps <= tolerance_bps {
                    matched += 1;
                    ("MATCH", "INFO")
                } else {
                    mismatched += 1;
                    let sev = if delta_bps > 500 { "ERROR" } else { "WARN" };
                    ("MISMATCH", sev)
                };
                
                sqlx::query(
                    r#"
                    INSERT INTO kyc.ownership_reconciliation_findings (
                        run_id, owner_entity_id, source_a_pct, source_b_pct, delta_bps,
                        finding_type, severity
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#
                )
                .bind(run_id)
                .bind(entity_id)
                .bind(pct_a)
                .bind(pct_b)
                .bind(delta_bps)
                .bind(finding_type)
                .bind(severity)
                .execute(pool)
                .await?;
            } else {
                missing_in_b += 1;
                sqlx::query(
                    r#"
                    INSERT INTO kyc.ownership_reconciliation_findings (
                        run_id, owner_entity_id, source_a_pct, finding_type, severity
                    ) VALUES ($1, $2, $3, 'MISSING_IN_EXTERNAL', 'WARN')
                    "#
                )
                .bind(run_id)
                .bind(entity_id)
                .bind(pct_a)
                .execute(pool)
                .await?;
            }
        }
        
        // Check for entities in B but not A
        for (entity_id, pct_b) in &map_b {
            if !map_a.contains_key(entity_id) {
                missing_in_a += 1;
                sqlx::query(
                    r#"
                    INSERT INTO kyc.ownership_reconciliation_findings (
                        run_id, owner_entity_id, source_b_pct, finding_type, severity
                    ) VALUES ($1, $2, $3, 'MISSING_IN_REGISTER', 'ERROR')
                    "#
                )
                .bind(run_id)
                .bind(entity_id)
                .bind(pct_b)
                .execute(pool)
                .await?;
            }
        }
        
        // Update run status
        sqlx::query(
            r#"
            UPDATE kyc.ownership_reconciliation_runs
            SET status = 'COMPLETED',
                completed_at = now(),
                total_entities = $2,
                matched_count = $3,
                mismatched_count = $4,
                missing_in_a_count = $5,
                missing_in_b_count = $6
            WHERE run_id = $1
            "#
        )
        .bind(run_id)
        .bind((map_a.len() + map_b.len()) as i32)
        .bind(matched)
        .bind(mismatched)
        .bind(missing_in_a)
        .bind(missing_in_b)
        .execute(pool)
        .await?;
        
        if let Some(binding) = verb_call.binding() {
            ctx.bind(binding, run_id);
        }
        
        tracing::info!(
            "ownership.reconcile: {} vs {} for {}: matched={}, mismatched={}, missing_a={}, missing_b={}",
            source_a, source_b, issuer_entity_id, matched, mismatched, missing_in_a, missing_in_b
        );
        
        Ok(ExecutionResult::Uuid(run_id))
    }
}

// ============================================================================
// TODO: Implement remaining handlers
// ============================================================================
// - OwnershipSnapshotListOp
// - OwnershipRightAddToClassOp
// - OwnershipRightAddToHolderOp
// - OwnershipReconcileFindingsOp

pub fn register_ownership_ops(registry: &mut crate::dsl_v2::CustomOpRegistry) {
    registry.register(Box::new(OwnershipComputeOp));
    registry.register(Box::new(OwnershipReconcileOp));
    // TODO: Register remaining ops
}
```

---

## Phase 5: Agent Lexicon Integration

**Claude: Read `docs/agent-architecture.md` before implementing.**

**File:** `rust/config/lexicon/capital_ownership.yaml`

```yaml
# ============================================================================
# Capital Structure & Ownership Lexicon
# ============================================================================
# Maps natural language to DSL verbs

intents:
  # --------------------------------------------------------------------------
  # Cap Table / Capital Structure
  # --------------------------------------------------------------------------
  
  - patterns:
      - "show (me )?(the )?cap table for {entity}"
      - "what('s| is) the capital structure of {entity}"
      - "who are the shareholders of {entity}"
      - "list share classes for {entity}"
    intent: show_cap_table
    verb: ownership.snapshot.list
    params:
      issuer-entity-id: "{entity}"
      derived-from: "REGISTER"
    followup: "Would you like to see control analysis or reconciliation?"

  - patterns:
      - "who controls {entity}"
      - "who has control of {entity}"
      - "who has voting control (of|over) {entity}"
      - "who can control the board (of|at) {entity}"
    intent: show_control
    verb: ownership.snapshot.list
    params:
      issuer-entity-id: "{entity}"
      derived-from: "REGISTER"
      min-pct: 25
    followup: "I'll show holders with >25% voting or board appointment rights."

  - patterns:
      - "compute ownership for {entity}"
      - "derive ownership (snapshots )?for {entity}"
      - "calculate control positions for {entity}"
    intent: compute_ownership
    verb: ownership.compute
    params:
      issuer-entity-id: "{entity}"
      basis: "BOTH"

  # --------------------------------------------------------------------------
  # Share Class Management
  # --------------------------------------------------------------------------
  
  - patterns:
      - "create (a )?share class (called |named ){name} for {entity}"
      - "add (a )?{kind} share class to {entity}"
    intent: create_share_class
    verb: capital.share-class.create
    params:
      issuer-entity-id: "{entity}"
      name: "{name}"
      instrument-kind: "{kind}"
    defaults:
      kind: "ORDINARY_EQUITY"
      name: "Ordinary Shares"

  - patterns:
      - "issue {units} (shares|units) of {share_class}"
      - "initial issue(ance)? of {units} {share_class}"
    intent: initial_issue
    verb: capital.issue.initial
    params:
      share-class-id: "{share_class}"
      units: "{units}"

  # --------------------------------------------------------------------------
  # Reconciliation
  # --------------------------------------------------------------------------
  
  - patterns:
      - "reconcile (ownership of )?{entity}( against BODS)?"
      - "compare register (to|with|against) BODS for {entity}"
      - "check BODS declarations for {entity}"
    intent: reconcile_bods
    verb: ownership.reconcile
    params:
      issuer-entity-id: "{entity}"
      source-a: "REGISTER"
      source-b: "BODS"

  - patterns:
      - "reconcile {entity} against GLEIF"
      - "compare register (to|with) GLEIF for {entity}"
    intent: reconcile_gleif
    verb: ownership.reconcile
    params:
      issuer-entity-id: "{entity}"
      source-a: "REGISTER"
      source-b: "GLEIF"

  # --------------------------------------------------------------------------
  # Special Rights
  # --------------------------------------------------------------------------
  
  - patterns:
      - "{holder} has board (appointment )?right(s)? (at|for|in) {entity}"
      - "give {holder} {seats} board seat(s)? (at|for|in) {entity}"
    intent: add_board_right
    verb: ownership.right.add-to-holder
    params:
      holder-entity-id: "{holder}"
      issuer-entity-id: "{entity}"
      right-type: "BOARD_APPOINTMENT"
      board-seats: "{seats}"
    defaults:
      seats: 1

  - patterns:
      - "{holder} has veto (right )?over (M&A|mergers) (at|for) {entity}"
      - "give {holder} MA veto (at|for) {entity}"
    intent: add_veto_ma
    verb: ownership.right.add-to-holder
    params:
      holder-entity-id: "{holder}"
      issuer-entity-id: "{entity}"
      right-type: "VETO_MA"

  # --------------------------------------------------------------------------
  # Dilution / Options / Warrants
  # --------------------------------------------------------------------------
  
  - patterns:
      - "grant {units} options to {holder} (at|for|in) {entity}"
      - "issue {units} stock options to {holder}"
      - "{holder} gets {units} options"
    intent: grant_options
    verb: capital.dilution.grant-options
    params:
      issuer-entity-id: "{entity}"
      holder-entity-id: "{holder}"
      units: "{units}"
    followup: "I'll need the exercise price and vesting details."

  - patterns:
      - "show (me )?(the )?dilution for {entity}"
      - "what('s| is) the fully diluted cap table for {entity}"
      - "list options and warrants for {entity}"
    intent: show_dilution
    verb: capital.dilution.list
    params:
      issuer-entity-id: "{entity}"
      status: "ACTIVE"

  - patterns:
      - "exercise {units} options from {instrument}"
      - "{holder} exercises {units} options"
    intent: exercise_options
    verb: capital.dilution.exercise
    params:
      instrument-id: "{instrument}"
      units: "{units}"

  - patterns:
      - "show (me )?control (of|for) {entity} on (a )?fully diluted basis"
      - "who controls {entity} fully diluted"
    intent: show_control_diluted
    verb: ownership.snapshot.list
    params:
      issuer-entity-id: "{entity}"
      derived-from: "REGISTER"
      basis: "FULLY_DILUTED"
```

---

## Phase 6: Taxonomy Extension

**Claude: Read `docs/entity-model-ascii.md` before implementing.**

**File:** `rust/crates/ob-poc-types/src/taxonomy.rs` (extend existing)

```rust
// ============================================================================
// New Node Types for Capital Structure
// ============================================================================

/// Extended NodeType enum
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    // Existing types...
    Cbu,
    Entity,
    Role,
    Document,
    KycCase,
    
    // NEW: Capital structure nodes
    ShareClass,
    IssuanceEvent,
    Holding,
    ControlPosition,
    SpecialRight,
    DilutionInstrument,  // Options, warrants, convertibles
    ReconciliationRun,
}

/// Extended EdgeType enum
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    // Existing types...
    HasRole,
    HasDocument,
    BelongsToCbu,
    
    // NEW: Ownership edges
    IssuedBy,           // ShareClass → Entity (issuer)
    HoldsUnits,         // Entity → ShareClass (investor)
    HasVotingControl,   // Entity → Entity (computed, >threshold)
    HasEconomicInterest,// Entity → Entity (computed)
    HasBoardRight,      // Entity → Entity (via special right)
    DerivedFrom,        // ControlPosition → Holding (provenance)
    ModifiedSupply,     // IssuanceEvent → ShareClass
    PotentiallyDilutes, // DilutionInstrument → ShareClass
    GrantedTo,          // DilutionInstrument → Entity (holder)
}

// ============================================================================
// Node Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareClassNode {
    pub share_class_id: Uuid,
    pub name: String,
    pub instrument_kind: String,
    pub votes_per_unit: f64,
    pub economic_per_unit: f64,
    pub issued_units: f64,
    pub outstanding_units: f64,
    pub is_voting: bool,
    pub currency: String,
    pub identifiers: Vec<IdentifierInfo>,
    pub issuer_entity_id: Uuid,
    pub issuer_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifierInfo {
    pub scheme: String,
    pub value: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPositionNode {
    pub snapshot_id: Uuid,
    pub owner_entity_id: Uuid,
    pub owner_name: String,
    pub issuer_entity_id: Uuid,
    pub issuer_name: String,
    pub basis: String,
    pub voting_pct: f64,
    pub economic_pct: f64,
    pub has_control: bool,
    pub has_significant_influence: bool,
    pub has_board_rights: bool,
    pub board_seats: i32,
    pub derived_from: String,
    pub as_of_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuanceEventNode {
    pub event_id: Uuid,
    pub share_class_id: Uuid,
    pub event_type: String,
    pub units_delta: f64,
    pub effective_date: NaiveDate,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialRightNode {
    pub right_id: Uuid,
    pub right_type: String,
    pub scope_type: String,  // "class" or "holder"
    pub scope_id: Uuid,      // share_class_id or holder_entity_id
    pub board_seats: Option<i32>,
    pub threshold_pct: Option<f64>,
    pub source_type: Option<String>,
    pub source_clause_ref: Option<String>,
}

// ============================================================================
// Edge Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlEdge {
    pub voting_pct: f64,
    pub economic_pct: f64,
    pub has_control: bool,
    pub has_significant: bool,
    pub derived_from: String,
    pub as_of_date: NaiveDate,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldingEdge {
    pub holding_id: Uuid,
    pub units: f64,
    pub cost_basis: Option<f64>,
    pub acquisition_date: Option<NaiveDate>,
    pub status: String,
}
```

---

## Phase 7: Graph API Endpoints

**Claude: Read existing API patterns in `rust/src/api/`**

**File:** `rust/src/api/capital_routes.rs`

```rust
//! Capital Structure API Endpoints
//!
//! Provides REST endpoints for cap table queries, control analysis,
//! and reconciliation.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::AppState;

pub fn capital_routes() -> Router<AppState> {
    Router::new()
        // Cap table
        .route("/api/capital/:issuer_id/cap-table", get(get_cap_table))
        .route("/api/capital/:issuer_id/share-classes", get(get_share_classes))
        .route("/api/capital/:issuer_id/supply", get(get_supply))
        
        // Control analysis
        .route("/api/capital/:issuer_id/control", get(get_control_positions))
        .route("/api/capital/:issuer_id/special-rights", get(get_special_rights))
        
        // Reconciliation
        .route("/api/capital/:issuer_id/reconcile", get(get_reconciliation_runs))
        .route("/api/capital/reconciliation/:run_id/findings", get(get_reconciliation_findings))
        
        // Graph data (for viewport)
        .route("/api/capital/:issuer_id/graph", get(get_ownership_graph))
}

#[derive(Debug, Deserialize)]
pub struct CapTableQuery {
    pub as_of: Option<String>,  // ISO date
    pub include_special_rights: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CapTableResponse {
    pub issuer_entity_id: Uuid,
    pub issuer_name: String,
    pub as_of_date: String,
    pub share_classes: Vec<ShareClassSummary>,
    pub holders: Vec<HolderPosition>,
    pub total_votes: f64,
    pub total_economic: f64,
}

#[derive(Debug, Serialize)]
pub struct ShareClassSummary {
    pub share_class_id: Uuid,
    pub name: String,
    pub instrument_kind: String,
    pub votes_per_unit: f64,
    pub issued_units: f64,
    pub total_votes: f64,
    pub voting_weight_pct: f64,
    pub identifiers: Vec<(String, String)>,
}

#[derive(Debug, Serialize)]
pub struct HolderPosition {
    pub holder_entity_id: Uuid,
    pub holder_name: String,
    pub holder_type: String,
    pub units: f64,
    pub votes: f64,
    pub economic: f64,
    pub voting_pct: f64,
    pub economic_pct: f64,
    pub has_control: bool,
    pub has_significant_influence: bool,
    pub board_seats: i32,
    pub special_rights: Vec<String>,
}

async fn get_cap_table(
    State(state): State<AppState>,
    Path(issuer_id): Path<Uuid>,
    Query(query): Query<CapTableQuery>,
) -> Json<CapTableResponse> {
    // TODO: Implement using kyc.fn_holder_control_position
    todo!()
}

// ... implement other handlers

#[derive(Debug, Serialize)]
pub struct OwnershipGraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub label: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub data: serde_json::Value,
}

async fn get_ownership_graph(
    State(state): State<AppState>,
    Path(issuer_id): Path<Uuid>,
    Query(query): Query<CapTableQuery>,
) -> Json<OwnershipGraphResponse> {
    // TODO: Build graph for egui viewport
    // Include: ShareClass nodes, Entity nodes (holders), ControlPosition edges
    todo!()
}
```

---

## Phase 8: egui Rendering Rules

**Claude: Read `docs/repl-viewport.md` and existing egui code in `rust/crates/ob-poc-ui/`**

**File:** `rust/crates/ob-poc-ui/src/rendering/capital.rs`

```rust
//! Capital Structure Rendering
//!
//! Renders share classes, control positions, and cap table visualizations
//! in the egui viewport.

use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use uuid::Uuid;

use crate::taxonomy::{ControlEdge, ShareClassNode, NodeType, EdgeType};

// ============================================================================
// Color Scheme
// ============================================================================

pub struct CapitalColors;

impl CapitalColors {
    // Share class colors by instrument kind
    pub fn share_class(instrument_kind: &str, is_voting: bool) -> Color32 {
        if !is_voting {
            return Color32::GRAY;
        }
        match instrument_kind {
            "ORDINARY_EQUITY" => Color32::from_rgb(100, 149, 237),  // Cornflower blue
            "PREFERENCE_EQUITY" => Color32::from_rgb(255, 215, 0), // Gold
            "FUND_UNIT" => Color32::from_rgb(144, 238, 144),       // Light green
            "LP_INTEREST" => Color32::from_rgb(221, 160, 221),     // Plum
            "GP_INTEREST" => Color32::from_rgb(255, 182, 193),     // Light pink
            "CONVERTIBLE" => Color32::from_rgb(255, 165, 0),       // Orange
            _ => Color32::from_rgb(192, 192, 192),                 // Silver
        }
    }
    
    // Control edge colors by source and status
    pub fn control_edge(has_control: bool, derived_from: &str) -> Color32 {
        match (has_control, derived_from) {
            (true, "REGISTER") => Color32::RED,           // Proven control
            (true, "BODS") => Color32::from_rgb(255, 140, 0), // Dark orange
            (true, "GLEIF") => Color32::YELLOW,           // Consolidation
            (false, _) if has_control => Color32::from_rgb(255, 200, 200), // Significant
            _ => Color32::LIGHT_GRAY,
        }
    }
    
    // Special right indicator
    pub fn special_right(right_type: &str) -> Color32 {
        match right_type {
            "BOARD_APPOINTMENT" => Color32::from_rgb(138, 43, 226), // Blue violet
            "VETO_MA" | "VETO_FUNDRAISE" => Color32::from_rgb(220, 20, 60), // Crimson
            _ => Color32::from_rgb(75, 0, 130), // Indigo
        }
    }
}

// ============================================================================
// Node Icons
// ============================================================================

pub fn share_class_icon(instrument_kind: &str) -> &'static str {
    match instrument_kind {
        "ORDINARY_EQUITY" => "🏛️",
        "PREFERENCE_EQUITY" => "⭐",
        "FUND_UNIT" => "📊",
        "FUND_SHARE" => "📈",
        "LP_INTEREST" => "🤝",
        "GP_INTEREST" => "👔",
        "DEBT" => "📜",
        "CONVERTIBLE" => "🔄",
        "WARRANT" => "📋",
        _ => "📄",
    }
}

pub fn control_indicator(has_control: bool, has_board_rights: bool) -> &'static str {
    match (has_control, has_board_rights) {
        (true, true) => "⚡🪑",   // Control + board
        (true, false) => "⚡",    // Control only
        (false, true) => "🪑",    // Board only
        _ => "",
    }
}

// ============================================================================
// Share Class Node Rendering
// ============================================================================

pub fn render_share_class_node(
    ui: &mut Ui,
    node: &ShareClassNode,
    pos: Pos2,
    selected: bool,
) {
    let size = Vec2::new(160.0, 80.0);
    let rect = Rect::from_min_size(pos, size);
    
    let bg_color = CapitalColors::share_class(&node.instrument_kind, node.is_voting);
    let stroke = if selected {
        Stroke::new(3.0, Color32::WHITE)
    } else {
        Stroke::new(1.0, Color32::DARK_GRAY)
    };
    
    ui.painter().rect(rect, 8.0, bg_color, stroke);
    
    // Icon and name
    let icon = share_class_icon(&node.instrument_kind);
    let voting_indicator = if node.is_voting {
        format!(" ({}v)", node.votes_per_unit)
    } else {
        " (non-voting)".to_string()
    };
    
    ui.painter().text(
        pos + Vec2::new(8.0, 8.0),
        egui::Align2::LEFT_TOP,
        format!("{} {}", icon, node.name),
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );
    
    // Supply info
    ui.painter().text(
        pos + Vec2::new(8.0, 30.0),
        egui::Align2::LEFT_TOP,
        format!("{:.0} issued{}", node.issued_units, voting_indicator),
        egui::FontId::proportional(11.0),
        Color32::from_rgba_unmultiplied(255, 255, 255, 200),
    );
    
    // Primary identifier
    if let Some(primary) = node.identifiers.iter().find(|i| i.is_primary) {
        ui.painter().text(
            pos + Vec2::new(8.0, 50.0),
            egui::Align2::LEFT_TOP,
            format!("{}: {}", primary.scheme, primary.value),
            egui::FontId::proportional(10.0),
            Color32::from_rgba_unmultiplied(255, 255, 255, 150),
        );
    }
}

// ============================================================================
// Control Edge Rendering
// ============================================================================

pub fn render_control_edge(
    ui: &mut Ui,
    edge: &ControlEdge,
    from: Pos2,
    to: Pos2,
) {
    let color = CapitalColors::control_edge(edge.has_control, &edge.derived_from);
    let thickness = if edge.has_control { 3.0 } else { 1.5 };
    
    // Draw edge line
    ui.painter().line_segment([from, to], Stroke::new(thickness, color));
    
    // Draw arrowhead
    let dir = (to - from).normalized();
    let arrow_size = 10.0;
    let arrow_pos = to - dir * 15.0;
    let perp = Vec2::new(-dir.y, dir.x);
    
    ui.painter().line_segment(
        [arrow_pos + perp * arrow_size * 0.5, to],
        Stroke::new(thickness, color),
    );
    ui.painter().line_segment(
        [arrow_pos - perp * arrow_size * 0.5, to],
        Stroke::new(thickness, color),
    );
    
    // Label at midpoint
    let mid = from + (to - from) * 0.5;
    let label = format!(
        "{:.1}%v / {:.1}%e",
        edge.voting_pct,
        edge.economic_pct
    );
    
    ui.painter().text(
        mid,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(10.0),
        Color32::WHITE,
    );
}

// ============================================================================
// Cap Table Layout
// ============================================================================

pub struct CapTableLayout {
    pub issuer_pos: Pos2,
    pub share_class_positions: Vec<(Uuid, Pos2)>,
    pub holder_positions: Vec<(Uuid, Pos2)>,
}

impl CapTableLayout {
    /// Create a hierarchical layout for cap table visualization
    pub fn compute(
        issuer_id: Uuid,
        share_classes: &[ShareClassNode],
        holders: &[Uuid],
        canvas_size: Vec2,
    ) -> Self {
        let center_x = canvas_size.x / 2.0;
        
        // Issuer at top
        let issuer_pos = Pos2::new(center_x, 50.0);
        
        // Share classes in row below issuer
        let class_y = 180.0;
        let class_spacing = 180.0;
        let class_start_x = center_x - (share_classes.len() as f32 - 1.0) * class_spacing / 2.0;
        
        let share_class_positions: Vec<_> = share_classes
            .iter()
            .enumerate()
            .map(|(i, sc)| {
                (sc.share_class_id, Pos2::new(class_start_x + i as f32 * class_spacing, class_y))
            })
            .collect();
        
        // Holders in row below share classes
        let holder_y = 350.0;
        let holder_spacing = 150.0;
        let holder_start_x = center_x - (holders.len() as f32 - 1.0) * holder_spacing / 2.0;
        
        let holder_positions: Vec<_> = holders
            .iter()
            .enumerate()
            .map(|(i, &h)| {
                (h, Pos2::new(holder_start_x + i as f32 * holder_spacing, holder_y))
            })
            .collect();
        
        Self {
            issuer_pos,
            share_class_positions,
            holder_positions,
        }
    }
}
```

---

## Phase 9: Testing & Validation

**File:** `rust/tests/integration/capital_ownership_test.rs`

```rust
//! Integration tests for capital structure and ownership model

#[tokio::test]
async fn test_share_class_creation_and_issuance() {
    // 1. Create issuer entity
    // 2. Create share class with super-voting
    // 3. Initial issue
    // 4. Verify supply state
}

#[tokio::test]
async fn test_control_computation() {
    // 1. Create issuer with multiple share classes
    // 2. Create holdings for multiple investors
    // 3. Compute ownership
    // 4. Verify control flags based on thresholds
}

#[tokio::test]
async fn test_special_rights_override() {
    // 1. Create holder with <50% voting
    // 2. Add board appointment right
    // 3. Verify has_board_rights = true
}

#[tokio::test]
async fn test_reconciliation_against_bods() {
    // 1. Create register holdings
    // 2. Import BODS statements
    // 3. Run reconciliation
    // 4. Verify findings
}

#[tokio::test]
async fn test_stock_split_adjusts_supply() {
    // 1. Initial issue 1M shares
    // 2. Execute 2:1 split
    // 3. Verify supply shows 2M
}

#[tokio::test]
async fn test_as_of_computation() {
    // 1. Create issuance events at different dates
    // 2. Compute supply at various as-of dates
    // 3. Verify correct historical values
}
```

---

## Phase 10: Documentation Updates

### 10.1 Update CLAUDE.md

Add section:

```markdown
## Capital Structure & Ownership

### Key Tables
- `kyc.share_classes` - Extended with voting/economic rights
- `kyc.share_class_identifiers` - ISIN, SEDOL, INTERNAL, etc.
- `kyc.share_class_supply` - Current supply state
- `kyc.issuance_events` - Supply ledger (append-only)
- `kyc.issuer_control_config` - Jurisdiction-specific thresholds
- `kyc.special_rights` - Board seats, vetos, etc.
- `kyc.ownership_snapshots` - Computed/imported ownership
- `kyc.ownership_reconciliation_*` - Reconciliation framework

### Key Functions
- `kyc.fn_share_class_supply_at(class_id, as_of)` - Supply at date
- `kyc.fn_holder_control_position(issuer_id, as_of, basis)` - Control computation
- `kyc.fn_derive_ownership_snapshots(issuer_id, as_of)` - Derive from register

### Verb Domains
- `capital.*` - Share class creation, issuance, splits, buybacks
- `ownership.*` - Computation, reconciliation, special rights

### Taxonomy Nodes
- `ShareClass` - Instrument with voting/economic attributes
- `ControlPosition` - Computed ownership with control flags
- `IssuanceEvent` - Supply change event
- `SpecialRight` - Non-percentage control mechanism

### Rendering
- Share classes colored by instrument_kind
- Control edges colored by source (REGISTER=red, BODS=orange, GLEIF=yellow)
- Control indicators: ⚡ = voting control, 🪑 = board rights
```

### 10.2 Update docs/entity-model-ascii.md

Add capital structure diagram.

---

## Implementation Checklist

### Phase 1: Database Schema
- [ ] 1.1 Create migration file `016_capital_structure_ownership.sql`
- [ ] 1.2 Add identifier schemes table + seed data
- [ ] 1.3 Add share_class_identifiers table
- [ ] 1.4 Extend share_classes with voting/economic columns
- [ ] 1.5 Add share_class_supply table
- [ ] 1.6 Add issuance_events table
- [ ] 1.7 Add dilution_instruments table
- [ ] 1.8 Add dilution_exercise_events table
- [ ] 1.9 Add issuer_control_config table
- [ ] 1.10 Add special_rights table (unified)
- [ ] 1.11 Add ownership_snapshots table
- [ ] 1.12 Add reconciliation tables
- [ ] 1.13 Add BODS mapping table
- [ ] 1.14 Run migration, verify schema

### Phase 2: SQL Functions
- [ ] 2.1 Implement fn_share_class_supply_at()
- [ ] 2.2 Implement fn_diluted_supply_at() for FULLY_DILUTED/EXERCISABLE
- [ ] 2.3 Implement fn_holder_control_position() with dilution support
- [ ] 2.4 Implement fn_derive_ownership_snapshots()
- [ ] 2.5 Test functions with sample data including dilution instruments

### Phase 3: Verb YAML
- [ ] 3.1 Create capital.yaml (including dilution.* verbs)
- [ ] 3.2 Create ownership.yaml
- [ ] 3.3 Validate YAML syntax
- [ ] 3.4 Verify verb registry loads

### Phase 4: Plugin Handlers
- [ ] 4.1 Create capital_ops.rs
- [ ] 4.2 Implement CapitalShareClassCreateOp
- [ ] 4.3 Implement CapitalIssueInitialOp
- [ ] 4.4 Implement remaining capital issuance ops (split, buyback, cancel)
- [ ] 4.5 Implement CapitalDilutionGrantOptionsOp
- [ ] 4.6 Implement CapitalDilutionIssueWarrantOp
- [ ] 4.7 Implement CapitalDilutionCreateSafeOp
- [ ] 4.8 Implement CapitalDilutionExerciseOp
- [ ] 4.9 Implement CapitalDilutionForfeitOp
- [ ] 4.10 Implement CapitalDilutionListOp
- [ ] 4.11 Create ownership_ops.rs
- [ ] 4.12 Implement OwnershipComputeOp (with basis parameter)
- [ ] 4.13 Implement OwnershipReconcileOp
- [ ] 4.14 Implement remaining ownership ops
- [ ] 4.15 Register all ops in custom_ops/mod.rs

### Phase 5: Agent Lexicon
- [ ] 5.1 Create capital_ownership.yaml lexicon
- [ ] 5.2 Add dilution-specific intents ("grant options to...", "show dilution for...")
- [ ] 5.3 Test intent recognition
- [ ] 5.4 Verify DSL generation from natural language

### Phase 6: Taxonomy Extension
- [ ] 6.1 Add new NodeType variants (ShareClass, ControlPosition, DilutionInstrument)
- [ ] 6.2 Add new EdgeType variants (HoldsUnits, HasVotingControl, PotentiallyDilutes)
- [ ] 6.3 Create node data structures (DilutionInstrumentNode)
- [ ] 6.4 Create edge data structures
- [ ] 6.5 Update graph builder to include capital + dilution nodes

### Phase 7: Graph API
- [ ] 7.1 Create capital_routes.rs
- [ ] 7.2 Implement get_cap_table endpoint (with dilution summary)
- [ ] 7.3 Implement get_control_positions endpoint (supporting all bases)
- [ ] 7.4 Implement get_dilution_instruments endpoint
- [ ] 7.5 Implement get_ownership_graph endpoint
- [ ] 7.6 Register routes in main router

### Phase 8: egui Rendering
- [ ] 8.1 Create rendering/capital.rs
- [ ] 8.2 Implement share class node rendering
- [ ] 8.3 Implement dilution instrument node rendering (with vesting indicator)
- [ ] 8.4 Implement control edge rendering
- [ ] 8.5 Implement cap table layout (with dilution waterfall)
- [ ] 8.6 Add to viewport rendering pipeline
- [ ] 8.7 Test visual output

### Phase 9: Testing
- [ ] 9.1 Create integration test file
- [ ] 9.2 Test share class creation
- [ ] 9.3 Test issuance events
- [ ] 9.4 Test dilution instrument creation (options, warrants, SAFE)
- [ ] 9.5 Test option exercise → holding creation
- [ ] 9.6 Test FULLY_DILUTED vs EXERCISABLE vs OUTSTANDING control computation
- [ ] 9.7 Test reconciliation
- [ ] 9.8 Test as-of queries with dilution

### Phase 10: Documentation
- [ ] 10.1 Update CLAUDE.md (add dilution tables and verbs)
- [ ] 10.2 Update entity-model-ascii.md (add dilution to ownership diagram)
- [ ] 10.3 Add examples to verb-definition-spec.md

---

## Estimated Effort by Phase

| Phase | Effort | Dependencies |
|-------|--------|--------------|
| 1. Database Schema | 5h | None |
| 2. SQL Functions | 4h | Phase 1 |
| 3. Verb YAML | 3h | Phase 1 |
| 4. Plugin Handlers | 12h | Phases 1, 2, 3 |
| 5. Agent Lexicon | 3h | Phases 3, 4 |
| 6. Taxonomy Extension | 4h | Phase 1 |
| 7. Graph API | 5h | Phases 1, 2, 6 |
| 8. egui Rendering | 8h | Phases 6, 7 |
| 9. Testing | 6h | All above |
| 10. Documentation | 2h | All above |
| **Total** | **~52h** | |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Dilution vesting complexity | Start with immediate vest; add vesting schedules incrementally |
| SAFE conversion ambiguity | Require explicit share class assignment at priced round |
| Holdings temporal tracking | Use movements ledger for as-of if holdings lack history |
| BODS data quality | Reconciliation findings surface issues |
| egui performance with large graphs | Implement viewport culling |

---

## Success Criteria

1. **Can create multi-class cap table** with voting/non-voting classes
2. **Can compute control** using register holdings
3. **Can reconcile** against BODS declarations
4. **Agent understands** "who controls X?" queries
5. **Viewport renders** cap table with control indicators
6. **As-of queries work** for historical analysis

