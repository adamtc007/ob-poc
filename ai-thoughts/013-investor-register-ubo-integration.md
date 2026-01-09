# Investor Register + KYC-as-a-Service + UBO Integration

> **Purpose:** Complete implementation guide for the dual-use Investor Register
> **Status:** TODO - Ready for Claude Code implementation
> **Created:** 2026-01-09
> **Replaces:** 013-investor-register-envestors-ubo-integration.md (misunderstood scope)

---

## Executive Summary

The **Investor Register** is a dual-purpose system:

### Use Case A: Transfer Agency KYC-as-a-Service
BNY provides KYC services to clients (fund managers). The client's end investors (retail, institutional, PE) are onboarded, KYC'd, and then allowed to subscribe to the client's fund shares. Full lifecycle from enquiry to offboard.

### Use Case B: UBO Intra-Group Holdings
Institutional shareholdings within corporate structures, used for UBO discovery. Holdings ≥25% identify beneficial owners for regulatory compliance.

**Data Sources:** Clearstream (industry standard), other custodians, CSV import, manual entry, API feeds.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           DATA SOURCES (Provider-Agnostic)                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  Clearstream  │  Euroclear  │  CSV Import  │  API Feed  │  Manual Entry    │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     INVESTOR REGISTER (kyc schema)                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │  investors      │  │  share_classes  │  │  holdings                   │  │
│  │  (NEW TABLE)    │  │  (fund shares)  │  │  (positions)                │  │
│  ├─────────────────┤  ├─────────────────┤  ├─────────────────────────────┤  │
│  │ entity_id (FK)  │  │ ISIN            │  │ investor_id + share_class   │  │
│  │ investor_type   │  │ NAV, fees       │  │ units, cost_basis           │  │
│  │ kyc_status      │  │ fund CBU        │  │ status, lifecycle_state     │  │
│  │ lifecycle_state │  │                 │  │ provider, provider_ref      │  │
│  │ eligible_funds  │  │                 │  │                             │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  movements (subscription, redemption, transfer, dividend, adjustment)   ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
┌───────────────────────────────────┐ ┌─────────────────────────────────────┐
│  USE CASE A: TA KYC-as-a-Service  │ │  USE CASE B: UBO Discovery          │
├───────────────────────────────────┤ ├─────────────────────────────────────┤
│  End investor onboarding          │ │  Holdings → entity_relationships    │
│  KYC case per investor            │ │  ≥25% = UBO candidate               │
│  Eligibility → subscription       │ │  Feeds into KYC case for structure  │
│  Full lifecycle management        │ │  GLEIF tracing for corporates       │
└───────────────────────────────────┘ └─────────────────────────────────────┘
                    │                               │
                    └───────────────┬───────────────┘
                                    ▼
                    ┌───────────────────────────────┐
                    │  BODS 0.4 Regulatory Export   │
                    │  Unified ownership statements │
                    └───────────────────────────────┘
```

---

## PHASE 1: Investor Entity & Lifecycle Model

### Task 1.1: Create Investors Table

The `investors` table links an entity to investor-specific attributes and lifecycle state.

**File:** `migrations/011_investor_register.sql`

```sql
-- =============================================================================
-- Migration 011: Investor Register with Full Lifecycle
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. Investors Table (links entity to investor-specific data)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS kyc.investors (
    investor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Link to entity (person or company)
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Investor classification
    investor_type VARCHAR(50) NOT NULL,
    investor_category VARCHAR(50),
    
    -- Lifecycle state (the investor's journey)
    lifecycle_state VARCHAR(50) NOT NULL DEFAULT 'ENQUIRY',
    lifecycle_state_at TIMESTAMPTZ DEFAULT NOW(),
    lifecycle_notes TEXT,
    
    -- KYC status (separate from lifecycle - an investor can be SUBSCRIBED but KYC_EXPIRED)
    kyc_status VARCHAR(50) NOT NULL DEFAULT 'NOT_STARTED',
    kyc_case_id UUID,  -- Current/latest KYC case
    kyc_approved_at TIMESTAMPTZ,
    kyc_expires_at TIMESTAMPTZ,
    kyc_risk_rating VARCHAR(20),
    
    -- Tax & regulatory
    tax_status VARCHAR(50),
    tax_jurisdiction VARCHAR(10),
    fatca_status VARCHAR(50),
    crs_status VARCHAR(50),
    
    -- Eligibility & restrictions
    eligible_fund_types TEXT[],  -- Array of fund types investor can access
    restricted_jurisdictions TEXT[],
    
    -- Data source tracking
    provider VARCHAR(50) DEFAULT 'MANUAL',
    provider_reference VARCHAR(100),
    provider_sync_at TIMESTAMPTZ,
    
    -- Context
    owning_cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),  -- Which client "owns" this investor
    
    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Unique: one investor record per entity per owning client
    UNIQUE(entity_id, owning_cbu_id)
);

-- Investor type enum
COMMENT ON COLUMN kyc.investors.investor_type IS 
'RETAIL, PROFESSIONAL, INSTITUTIONAL, NOMINEE, INTRA_GROUP';

-- Investor category for segmentation
COMMENT ON COLUMN kyc.investors.investor_category IS 
'HIGH_NET_WORTH, PENSION_FUND, INSURANCE, SOVEREIGN_WEALTH, FAMILY_OFFICE, CORPORATE, INDIVIDUAL';

-- Lifecycle states
COMMENT ON COLUMN kyc.investors.lifecycle_state IS 
'ENQUIRY, PENDING_DOCUMENTS, KYC_IN_PROGRESS, KYC_APPROVED, KYC_REJECTED, ELIGIBLE_TO_SUBSCRIBE, SUBSCRIBED, ACTIVE_HOLDER, REDEEMING, OFFBOARDED, SUSPENDED, BLOCKED';

-- KYC status (separate concern from lifecycle)
COMMENT ON COLUMN kyc.investors.kyc_status IS 
'NOT_STARTED, IN_PROGRESS, APPROVED, REJECTED, EXPIRED, REFRESH_REQUIRED';

-- Provider tracking
COMMENT ON COLUMN kyc.investors.provider IS 
'CLEARSTREAM, EUROCLEAR, CSV_IMPORT, API_FEED, MANUAL';

-- Owning CBU
COMMENT ON COLUMN kyc.investors.owning_cbu_id IS 
'The BNY client (fund manager) who owns this investor relationship';

-- Indexes
CREATE INDEX idx_investors_entity ON kyc.investors(entity_id);
CREATE INDEX idx_investors_lifecycle ON kyc.investors(lifecycle_state);
CREATE INDEX idx_investors_kyc_status ON kyc.investors(kyc_status);
CREATE INDEX idx_investors_owning_cbu ON kyc.investors(owning_cbu_id);
CREATE INDEX idx_investors_provider ON kyc.investors(provider, provider_reference);
```

### Task 1.2: Lifecycle State Machine

```sql
-- -----------------------------------------------------------------------------
-- 2. Investor Lifecycle State Transitions (validation)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS kyc.investor_lifecycle_transitions (
    from_state VARCHAR(50) NOT NULL,
    to_state VARCHAR(50) NOT NULL,
    requires_kyc_approved BOOLEAN DEFAULT false,
    requires_document TEXT,  -- Document type required for transition
    auto_trigger VARCHAR(100),  -- Event that auto-triggers this transition
    PRIMARY KEY (from_state, to_state)
);

INSERT INTO kyc.investor_lifecycle_transitions (from_state, to_state, requires_kyc_approved, auto_trigger) VALUES
-- Initial journey
('ENQUIRY', 'PENDING_DOCUMENTS', false, NULL),
('PENDING_DOCUMENTS', 'KYC_IN_PROGRESS', false, 'ALL_DOCS_RECEIVED'),
('KYC_IN_PROGRESS', 'KYC_APPROVED', false, 'KYC_CASE_APPROVED'),
('KYC_IN_PROGRESS', 'KYC_REJECTED', false, 'KYC_CASE_REJECTED'),
('KYC_APPROVED', 'ELIGIBLE_TO_SUBSCRIBE', true, NULL),

-- Subscription journey
('ELIGIBLE_TO_SUBSCRIBE', 'SUBSCRIBED', true, 'FIRST_SUBSCRIPTION'),
('SUBSCRIBED', 'ACTIVE_HOLDER', true, 'SUBSCRIPTION_SETTLED'),

-- Exit journey
('ACTIVE_HOLDER', 'REDEEMING', true, 'FULL_REDEMPTION_REQUESTED'),
('REDEEMING', 'OFFBOARDED', false, 'REDEMPTION_SETTLED'),

-- Exceptional states
('ACTIVE_HOLDER', 'SUSPENDED', false, NULL),
('SUSPENDED', 'ACTIVE_HOLDER', true, NULL),
('ACTIVE_HOLDER', 'BLOCKED', false, NULL),
('SUBSCRIBED', 'BLOCKED', false, NULL),

-- Re-engagement
('KYC_REJECTED', 'PENDING_DOCUMENTS', false, NULL),
('OFFBOARDED', 'ENQUIRY', false, NULL)

ON CONFLICT DO NOTHING;

-- Function to validate lifecycle transition
CREATE OR REPLACE FUNCTION kyc.validate_investor_lifecycle_transition()
RETURNS TRIGGER AS $$
BEGIN
    -- Allow if transition is valid
    IF EXISTS (
        SELECT 1 FROM kyc.investor_lifecycle_transitions
        WHERE from_state = OLD.lifecycle_state 
          AND to_state = NEW.lifecycle_state
    ) THEN
        NEW.lifecycle_state_at := NOW();
        RETURN NEW;
    END IF;
    
    -- Reject invalid transition
    RAISE EXCEPTION 'Invalid lifecycle transition from % to %', 
        OLD.lifecycle_state, NEW.lifecycle_state;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_validate_investor_lifecycle
    BEFORE UPDATE OF lifecycle_state ON kyc.investors
    FOR EACH ROW
    WHEN (OLD.lifecycle_state IS DISTINCT FROM NEW.lifecycle_state)
    EXECUTE FUNCTION kyc.validate_investor_lifecycle_transition();
```

### Task 1.3: Investor Lifecycle History

```sql
-- -----------------------------------------------------------------------------
-- 3. Investor Lifecycle Audit Trail
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS kyc.investor_lifecycle_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investor_id UUID NOT NULL REFERENCES kyc.investors(investor_id),
    from_state VARCHAR(50),
    to_state VARCHAR(50) NOT NULL,
    transitioned_at TIMESTAMPTZ DEFAULT NOW(),
    triggered_by VARCHAR(100),  -- User, system event, or auto-trigger
    notes TEXT,
    metadata JSONB
);

CREATE INDEX idx_investor_lifecycle_history ON kyc.investor_lifecycle_history(investor_id, transitioned_at);

-- Trigger to log lifecycle changes
CREATE OR REPLACE FUNCTION kyc.log_investor_lifecycle_change()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO kyc.investor_lifecycle_history (
        investor_id, from_state, to_state, triggered_by, notes
    ) VALUES (
        NEW.investor_id, OLD.lifecycle_state, NEW.lifecycle_state,
        current_setting('app.current_user', true),
        NEW.lifecycle_notes
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_log_investor_lifecycle
    AFTER UPDATE OF lifecycle_state ON kyc.investors
    FOR EACH ROW
    EXECUTE FUNCTION kyc.log_investor_lifecycle_change();
```

---

## PHASE 2: Enhanced Holdings with Lifecycle

### Task 2.1: Update Holdings Table

```sql
-- -----------------------------------------------------------------------------
-- 4. Enhanced Holdings Table
-- -----------------------------------------------------------------------------
-- Add columns to existing holdings table

ALTER TABLE kyc.holdings 
ADD COLUMN IF NOT EXISTS investor_id UUID REFERENCES kyc.investors(investor_id),
ADD COLUMN IF NOT EXISTS holding_status VARCHAR(50) DEFAULT 'ACTIVE',
ADD COLUMN IF NOT EXISTS provider VARCHAR(50) DEFAULT 'MANUAL',
ADD COLUMN IF NOT EXISTS provider_reference VARCHAR(100),
ADD COLUMN IF NOT EXISTS provider_sync_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS usage_type VARCHAR(20) DEFAULT 'TA';

COMMENT ON COLUMN kyc.holdings.investor_id IS 
'Link to investor record (for TA use case)';

COMMENT ON COLUMN kyc.holdings.holding_status IS 
'PENDING, ACTIVE, SUSPENDED, CLOSED';

COMMENT ON COLUMN kyc.holdings.usage_type IS 
'TA (Transfer Agency - client investors) or UBO (intra-group ownership)';

-- Update existing status column to holding_status if needed
-- (Check if 'status' column exists and rename/migrate)

-- Index for investor lookups
CREATE INDEX IF NOT EXISTS idx_holdings_investor ON kyc.holdings(investor_id);
CREATE INDEX IF NOT EXISTS idx_holdings_usage_type ON kyc.holdings(usage_type);
```

### Task 2.2: Enhanced Movements with Lifecycle Events

```sql
-- -----------------------------------------------------------------------------
-- 5. Enhanced Movements Table
-- -----------------------------------------------------------------------------
-- Add lifecycle-relevant movement types

ALTER TABLE kyc.movements 
DROP CONSTRAINT IF EXISTS movements_movement_type_check;

ALTER TABLE kyc.movements 
ADD CONSTRAINT movements_movement_type_check CHECK (
    movement_type IN (
        -- Standard movements
        'subscription', 'redemption', 'transfer_in', 'transfer_out', 
        'dividend', 'adjustment',
        -- PE/VC specific
        'commitment', 'capital_call', 'distribution', 'recallable',
        -- Lifecycle events
        'initial_subscription', 'additional_subscription', 
        'partial_redemption', 'full_redemption',
        -- Corporate actions
        'stock_split', 'merger', 'spinoff'
    )
);

-- Add PE-specific columns
ALTER TABLE kyc.movements
ADD COLUMN IF NOT EXISTS commitment_id UUID,  -- Links calls/distributions to original commitment
ADD COLUMN IF NOT EXISTS call_number INTEGER,  -- For capital calls: 1st, 2nd, etc.
ADD COLUMN IF NOT EXISTS distribution_type VARCHAR(50);  -- INCOME, CAPITAL, RETURN_OF_CAPITAL

COMMENT ON COLUMN kyc.movements.distribution_type IS 
'For distributions: INCOME, CAPITAL_GAIN, RETURN_OF_CAPITAL, RECALLABLE';
```

---

## PHASE 3: Views for Dual Use Cases

### Task 3.1: TA Investor View (Use Case A)

```sql
-- -----------------------------------------------------------------------------
-- 6. Transfer Agency Investor View
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_ta_investors AS
SELECT
    -- Investor details
    i.investor_id,
    i.entity_id,
    e.name AS investor_name,
    e.entity_type,
    e.country_code,
    i.investor_type,
    i.investor_category,
    
    -- Lifecycle
    i.lifecycle_state,
    i.lifecycle_state_at,
    
    -- KYC
    i.kyc_status,
    i.kyc_case_id,
    i.kyc_approved_at,
    i.kyc_expires_at,
    i.kyc_risk_rating,
    
    -- Eligibility
    i.eligible_fund_types,
    i.tax_status,
    i.fatca_status,
    i.crs_status,
    
    -- Owning client
    i.owning_cbu_id,
    c.name AS owning_client_name,
    
    -- Holdings summary
    COALESCE(hs.holding_count, 0) AS holding_count,
    COALESCE(hs.total_value, 0) AS total_value,
    
    -- Identifiers
    lei.id AS lei,
    tax_id.id AS tax_id,
    
    -- Provider
    i.provider,
    i.provider_reference,
    
    -- Timestamps
    i.created_at,
    i.updated_at

FROM kyc.investors i
JOIN "ob-poc".entities e ON i.entity_id = e.entity_id
LEFT JOIN "ob-poc".cbus c ON i.owning_cbu_id = c.cbu_id
LEFT JOIN "ob-poc".entity_identifiers lei 
    ON e.entity_id = lei.entity_id AND lei.scheme = 'LEI'
LEFT JOIN "ob-poc".entity_identifiers tax_id 
    ON e.entity_id = tax_id.entity_id AND tax_id.scheme = 'tax_id'
LEFT JOIN LATERAL (
    SELECT 
        COUNT(*) AS holding_count,
        SUM(h.units * COALESCE(sc.nav_per_share, 0)) AS total_value
    FROM kyc.holdings h
    JOIN kyc.share_classes sc ON h.share_class_id = sc.id
    WHERE h.investor_id = i.investor_id
      AND h.holding_status = 'ACTIVE'
) hs ON true;

COMMENT ON VIEW kyc.v_ta_investors IS 
'Transfer Agency view: Client investors with lifecycle state, KYC status, and holdings summary';
```

### Task 3.2: UBO Holdings View (Use Case B)

```sql
-- -----------------------------------------------------------------------------
-- 7. UBO-Qualified Holdings View
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_ubo_holdings AS
SELECT
    -- Holding details
    h.id AS holding_id,
    h.share_class_id,
    h.investor_entity_id,
    h.units,
    h.acquisition_date,
    h.usage_type,
    h.provider,
    
    -- Share class context
    sc.isin,
    sc.name AS share_class_name,
    sc.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    
    -- Entity being owned (the fund entity)
    sc.entity_id AS owned_entity_id,
    
    -- Investor/owner details
    e.name AS owner_name,
    e.entity_type AS owner_entity_type,
    e.country_code AS owner_country,
    
    -- Ownership percentage
    ROUND((h.units / NULLIF(total.total_units, 0)) * 100, 4) AS ownership_percentage,
    
    -- UBO qualification
    CASE 
        WHEN total.total_units > 0 AND (h.units / total.total_units) >= 0.25
        THEN true ELSE false
    END AS is_ubo_qualified,
    
    -- UBO type determination
    CASE 
        WHEN e.entity_type IN ('proper_person', 'natural_person') THEN 'DIRECT_UBO'
        ELSE 'REQUIRES_GLEIF_TRACE'
    END AS ubo_determination,
    
    -- LEI for corporate tracing
    lei.id AS owner_lei

FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_identifiers lei 
    ON e.entity_id = lei.entity_id AND lei.scheme = 'LEI'
CROSS JOIN LATERAL (
    SELECT COALESCE(SUM(h2.units), 0) AS total_units
    FROM kyc.holdings h2
    WHERE h2.share_class_id = sc.id 
      AND h2.holding_status = 'ACTIVE'
) total
WHERE h.holding_status = 'ACTIVE'
  AND h.usage_type IN ('UBO', 'TA');  -- Both can contribute to UBO

COMMENT ON VIEW kyc.v_ubo_holdings IS 
'Holdings view for UBO discovery. Shows ownership percentage and UBO qualification.';
```

### Task 3.3: Investor Register Summary View

```sql
-- -----------------------------------------------------------------------------
-- 8. Investor Register (Clearstream-style format)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_investor_register AS
SELECT
    -- Share class (the "register" is per share class)
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    sc.currency,
    sc.nav_per_share,
    sc.nav_date,
    
    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    c.jurisdiction AS fund_jurisdiction,
    
    -- Investor
    h.id AS holding_id,
    i.investor_id,
    e.entity_id AS investor_entity_id,
    e.name AS investor_name,
    e.entity_type AS investor_entity_type,
    e.country_code AS investor_country,
    i.investor_type,
    i.investor_category,
    
    -- Lifecycle & KYC
    i.lifecycle_state,
    i.kyc_status,
    i.kyc_risk_rating,
    
    -- Position data
    h.units AS holding_quantity,
    h.cost_basis,
    h.acquisition_date AS registration_date,
    h.holding_status,
    
    -- Computed values
    h.units * COALESCE(sc.nav_per_share, 0) AS market_value,
    ROUND((h.units / NULLIF(total.total_units, 0)) * 100, 4) AS ownership_percentage,
    
    -- Identifiers
    lei.id AS investor_lei,
    clr.id AS clearstream_ref,
    
    -- Provider tracking
    h.provider,
    h.provider_reference,
    h.provider_sync_at,
    
    -- Timestamps
    h.created_at AS holding_created_at,
    h.updated_at AS holding_updated_at

FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
LEFT JOIN kyc.investors i ON h.investor_id = i.investor_id
LEFT JOIN "ob-poc".entity_identifiers lei 
    ON e.entity_id = lei.entity_id AND lei.scheme = 'LEI'
LEFT JOIN "ob-poc".entity_identifiers clr 
    ON e.entity_id = clr.entity_id AND clr.scheme = 'CLEARSTREAM_KV'
CROSS JOIN LATERAL (
    SELECT COALESCE(SUM(h2.units), 0) AS total_units
    FROM kyc.holdings h2
    WHERE h2.share_class_id = sc.id AND h2.holding_status = 'ACTIVE'
) total
WHERE h.holding_status = 'ACTIVE';

COMMENT ON VIEW kyc.v_investor_register IS 
'Clearstream-style investor register with holdings, identifiers, and ownership percentages';
```

---

## PHASE 4: Holdings → UBO Sync

### Task 4.1: Sync Trigger

```sql
-- -----------------------------------------------------------------------------
-- 9. Sync UBO-Qualified Holdings to entity_relationships
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION kyc.sync_holding_to_ubo_relationship()
RETURNS TRIGGER AS $$
DECLARE
    v_total_units NUMERIC;
    v_ownership_pct NUMERIC;
    v_fund_entity_id UUID;
BEGIN
    -- Only process UBO-relevant holdings
    IF NEW.usage_type NOT IN ('UBO', 'TA') THEN
        RETURN NEW;
    END IF;
    
    -- Get total units for percentage
    SELECT COALESCE(SUM(units), 0) INTO v_total_units
    FROM kyc.holdings
    WHERE share_class_id = NEW.share_class_id AND holding_status = 'ACTIVE';
    
    -- Calculate ownership
    IF v_total_units > 0 THEN
        v_ownership_pct := (NEW.units / v_total_units) * 100;
    ELSE
        v_ownership_pct := 0;
    END IF;
    
    -- Get fund entity ID
    SELECT entity_id INTO v_fund_entity_id
    FROM kyc.share_classes WHERE id = NEW.share_class_id;
    
    -- Create/update ownership relationship if ≥25% and fund entity exists
    IF v_ownership_pct >= 25 AND v_fund_entity_id IS NOT NULL THEN
        INSERT INTO "ob-poc".entity_relationships (
            from_entity_id, to_entity_id, relationship_type,
            percentage, ownership_type, interest_type, direct_or_indirect,
            effective_from, source, notes
        ) VALUES (
            NEW.investor_entity_id, v_fund_entity_id, 'ownership',
            v_ownership_pct, 'DIRECT', 'shareholding', 'direct',
            COALESCE(NEW.acquisition_date, CURRENT_DATE),
            'INVESTOR_REGISTER',
            'Synced from holding ' || NEW.id::text
        )
        ON CONFLICT (from_entity_id, to_entity_id, relationship_type)
        DO UPDATE SET
            percentage = EXCLUDED.percentage,
            updated_at = NOW();
    ELSE
        -- Remove relationship if dropped below 25%
        DELETE FROM "ob-poc".entity_relationships
        WHERE from_entity_id = NEW.investor_entity_id
          AND to_entity_id = v_fund_entity_id
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER';
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_sync_holding_to_ubo ON kyc.holdings;
CREATE TRIGGER trg_sync_holding_to_ubo
    AFTER INSERT OR UPDATE OF units, holding_status ON kyc.holdings
    FOR EACH ROW
    EXECUTE FUNCTION kyc.sync_holding_to_ubo_relationship();
```

---

## PHASE 5: DSL Verbs for Investor Management

### Task 5.1: Investor Domain Verbs

**File:** `rust/config/verbs/investor.yaml`

```yaml
domains:
  investor:
    description: Investor lifecycle management for Transfer Agency KYC-as-a-Service
    verbs:
      # =======================================================================
      # ONBOARDING
      # =======================================================================
      register:
        description: Register a new investor (creates investor record linked to entity)
        behavior: crud
        crud:
          operation: upsert
          table: investors
          schema: kyc
          returning: investor_id
          conflict_keys:
            - entity_id
            - owning_cbu_id
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: owning-cbu-id
            type: uuid
            required: true
            maps_to: owning_cbu_id
            description: The BNY client who owns this investor relationship
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: investor-type
            type: string
            required: true
            maps_to: investor_type
            valid_values:
              - RETAIL
              - PROFESSIONAL
              - INSTITUTIONAL
              - NOMINEE
              - INTRA_GROUP
          - name: investor-category
            type: string
            required: false
            maps_to: investor_category
            valid_values:
              - HIGH_NET_WORTH
              - PENSION_FUND
              - INSURANCE
              - SOVEREIGN_WEALTH
              - FAMILY_OFFICE
              - CORPORATE
              - INDIVIDUAL
          - name: tax-jurisdiction
            type: string
            required: false
            maps_to: tax_jurisdiction
          - name: provider
            type: string
            required: false
            maps_to: provider
            default: MANUAL
          - name: provider-reference
            type: string
            required: false
            maps_to: provider_reference
        returns:
          type: uuid
          name: investor_id
          capture: true

      # =======================================================================
      # LIFECYCLE TRANSITIONS
      # =======================================================================
      submit-documents:
        description: Transition investor from ENQUIRY to PENDING_DOCUMENTS
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: notes
            type: string
            required: false
        returns:
          type: affected

      start-kyc:
        description: Transition to KYC_IN_PROGRESS and create KYC case
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: case-type
            type: string
            required: false
            default: INVESTOR_ONBOARDING
        returns:
          type: record

      approve-kyc:
        description: Transition to KYC_APPROVED after case approval
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: risk-rating
            type: string
            required: true
            valid_values:
              - LOW
              - MEDIUM
              - HIGH
              - PROHIBITED
          - name: kyc-expires-at
            type: date
            required: false
        returns:
          type: affected

      reject-kyc:
        description: Transition to KYC_REJECTED
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: rejection-reason
            type: string
            required: true
        returns:
          type: affected

      make-eligible:
        description: Transition to ELIGIBLE_TO_SUBSCRIBE (after KYC approval)
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: eligible-fund-types
            type: string[]
            required: false
            description: Fund types investor can subscribe to
        returns:
          type: affected

      suspend:
        description: Suspend an active investor
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: reason
            type: string
            required: true
        returns:
          type: affected

      reinstate:
        description: Reinstate a suspended investor
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: notes
            type: string
            required: false
        returns:
          type: affected

      block:
        description: Block an investor (sanctions, fraud, etc.)
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: reason
            type: string
            required: true
          - name: block-type
            type: string
            required: true
            valid_values:
              - SANCTIONS
              - FRAUD
              - AML
              - REGULATORY
              - OTHER
        returns:
          type: affected

      offboard:
        description: Complete investor offboarding after full redemption
        behavior: plugin
        args:
          - name: investor-id
            type: uuid
            required: true
          - name: reason
            type: string
            required: false
        returns:
          type: affected

      # =======================================================================
      # QUERIES
      # =======================================================================
      get:
        description: Get investor by ID
        behavior: crud
        crud:
          operation: select
          table: investors
          schema: kyc
        args:
          - name: investor-id
            type: uuid
            required: true
            maps_to: investor_id
        returns:
          type: record

      list-by-client:
        description: List investors for a BNY client (owning CBU)
        behavior: crud
        crud:
          operation: list_by_fk
          table: investors
          schema: kyc
          fk_col: owning_cbu_id
        args:
          - name: owning-cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: lifecycle-state
            type: string
            required: false
            maps_to: lifecycle_state
          - name: kyc-status
            type: string
            required: false
            maps_to: kyc_status
        returns:
          type: record_set

      list-pending-kyc:
        description: List investors pending KYC review
        behavior: crud
        crud:
          operation: list_by_fk
          table: investors
          schema: kyc
          fk_col: lifecycle_state
        args:
          - name: owning-cbu-id
            type: uuid
            required: false
            maps_to: owning_cbu_id
        set_values:
          lifecycle_state: KYC_IN_PROGRESS
        returns:
          type: record_set

      list-expiring-kyc:
        description: List investors with KYC expiring within N days
        behavior: plugin
        args:
          - name: days
            type: integer
            required: false
            default: 30
          - name: owning-cbu-id
            type: uuid
            required: false
        returns:
          type: record_set

      get-lifecycle-history:
        description: Get lifecycle history for an investor
        behavior: crud
        crud:
          operation: list_by_fk
          table: investor_lifecycle_history
          schema: kyc
          fk_col: investor_id
        args:
          - name: investor-id
            type: uuid
            required: true
        returns:
          type: record_set
```

### Task 5.2: Enhanced Holding Verbs

**File:** `rust/config/verbs/registry/holding.yaml` (update existing)

Add to existing verbs:

```yaml
      # Add to existing holding domain
      
      create-for-investor:
        description: Create holding linked to investor record
        behavior: crud
        crud:
          operation: upsert
          table: holdings
          schema: kyc
          returning: id
          conflict_keys:
            - share_class_id
            - investor_id
        args:
          - name: investor-id
            type: uuid
            required: true
            maps_to: investor_id
            lookup:
              table: investors
              entity_type: investor
              schema: kyc
              search_key: investor_id
              primary_key: investor_id
          - name: share-class-id
            type: uuid
            required: true
            maps_to: share_class_id
            lookup:
              table: share_classes
              entity_type: share_class
              schema: kyc
              search_key: name
              primary_key: id
          - name: units
            type: decimal
            required: false
            maps_to: units
            default: 0
          - name: usage-type
            type: string
            required: false
            maps_to: usage_type
            default: TA
            valid_values:
              - TA
              - UBO
          - name: provider
            type: string
            required: false
            maps_to: provider
          - name: provider-reference
            type: string
            required: false
            maps_to: provider_reference
        returns:
          type: uuid
          name: holding_id
          capture: true
```

### Task 5.3: Enhanced Movement Verbs for PE

**File:** `rust/config/verbs/registry/movement.yaml` (add to existing)

```yaml
      # PE/VC specific movements
      
      commit:
        description: Record a capital commitment (PE/VC)
        behavior: crud
        crud:
          operation: insert
          table: movements
          schema: kyc
          returning: id
        args:
          - name: holding-id
            type: uuid
            required: true
            maps_to: holding_id
          - name: amount
            type: decimal
            required: true
            maps_to: amount
          - name: currency
            type: string
            required: false
            maps_to: currency
            default: EUR
          - name: commitment-date
            type: date
            required: true
            maps_to: trade_date
          - name: reference
            type: string
            required: true
            maps_to: reference
        set_values:
          movement_type: commitment
          status: confirmed
        returns:
          type: uuid
          name: movement_id
          capture: true

      capital-call:
        description: Record a capital call against a commitment
        behavior: plugin
        args:
          - name: commitment-id
            type: uuid
            required: true
            description: The original commitment movement
          - name: amount
            type: decimal
            required: true
          - name: call-number
            type: integer
            required: true
          - name: call-date
            type: date
            required: true
          - name: due-date
            type: date
            required: true
          - name: reference
            type: string
            required: true
        returns:
          type: uuid
          name: movement_id
          capture: true

      distribute:
        description: Record a distribution to investor
        behavior: plugin
        args:
          - name: holding-id
            type: uuid
            required: true
          - name: amount
            type: decimal
            required: true
          - name: distribution-type
            type: string
            required: true
            valid_values:
              - INCOME
              - CAPITAL_GAIN
              - RETURN_OF_CAPITAL
              - RECALLABLE
          - name: distribution-date
            type: date
            required: true
          - name: reference
            type: string
            required: true
        returns:
          type: uuid
          name: movement_id
          capture: true
```

---

## PHASE 6: BODS Export

### Task 6.1: Unified BODS View

```sql
-- -----------------------------------------------------------------------------
-- 10. BODS Ownership Statements (unified from all sources)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_bods_ownership_statements AS

-- Source 1: Investor Register holdings
SELECT
    'ooc-holding-' || h.holding_id::text AS statement_id,
    'ownershipOrControlStatement' AS statement_type,
    h.isin AS subject_identifier,
    h.fund_name AS subject_name,
    h.investor_name AS interested_party_name,
    h.owner_lei AS interested_party_lei,
    'shareholding' AS interest_type,
    'direct' AS interest_directness,
    h.units AS share_exact,
    h.ownership_percentage,
    h.is_ubo_qualified AS beneficial_ownership_or_control,
    h.acquisition_date AS interest_start_date,
    h.provider AS source_type,
    h.provider_reference AS source_reference,
    CURRENT_DATE AS statement_date
FROM kyc.v_ubo_holdings h

UNION ALL

-- Source 2: Direct entity_relationships
SELECT
    'ooc-rel-' || er.relationship_id::text AS statement_id,
    'ownershipOrControlStatement' AS statement_type,
    NULL AS subject_identifier,
    subject_e.name AS subject_name,
    owner_e.name AS interested_party_name,
    owner_lei.id AS interested_party_lei,
    COALESCE(er.interest_type, 'shareholding') AS interest_type,
    COALESCE(er.direct_or_indirect, 'direct') AS interest_directness,
    NULL AS share_exact,
    er.percentage AS ownership_percentage,
    er.percentage >= 25 AS beneficial_ownership_or_control,
    er.effective_from AS interest_start_date,
    COALESCE(er.source, 'MANUAL') AS source_type,
    er.relationship_id::text AS source_reference,
    CURRENT_DATE AS statement_date
FROM "ob-poc".entity_relationships er
JOIN "ob-poc".entities owner_e ON er.from_entity_id = owner_e.entity_id
JOIN "ob-poc".entities subject_e ON er.to_entity_id = subject_e.entity_id
LEFT JOIN "ob-poc".entity_identifiers owner_lei 
    ON owner_e.entity_id = owner_lei.entity_id AND owner_lei.scheme = 'LEI'
WHERE er.relationship_type = 'ownership'
  AND er.source != 'INVESTOR_REGISTER'  -- Avoid double-count
  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE);
```

---

## PHASE 7: Plugin Handlers

### Task 7.1: Investor Lifecycle Plugin

**File:** `rust/src/dsl_v2/custom_ops/investor_ops.rs`

```rust
//! Investor Lifecycle Plugin Handlers
//!
//! Manages investor lifecycle transitions with validation and side effects.

use crate::dsl_v2::custom_ops::helpers::*;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

pub struct InvestorSubmitDocumentsOp;
pub struct InvestorStartKycOp;
pub struct InvestorApproveKycOp;
pub struct InvestorRejectKycOp;
pub struct InvestorMakeEligibleOp;
pub struct InvestorSuspendOp;
pub struct InvestorReinstateOp;
pub struct InvestorBlockOp;
pub struct InvestorOffboardOp;
pub struct InvestorListExpiringKycOp;

#[async_trait]
impl PluginOp for InvestorStartKycOp {
    async fn execute(
        &self,
        args: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        let investor_id = get_uuid_arg(args, "investor-id")?;
        let case_type = get_string_arg_or(args, "case-type", "INVESTOR_ONBOARDING");
        
        // 1. Get investor and validate current state
        let investor = sqlx::query!(
            r#"SELECT i.*, e.name as entity_name, c.cbu_id
               FROM kyc.investors i
               JOIN "ob-poc".entities e ON i.entity_id = e.entity_id
               LEFT JOIN "ob-poc".cbus c ON i.owning_cbu_id = c.cbu_id
               WHERE i.investor_id = $1"#,
            investor_id
        )
        .fetch_one(&ctx.pool)
        .await?;
        
        if investor.lifecycle_state != "PENDING_DOCUMENTS" {
            return Err(ExecutionError::ValidationError(
                format!("Cannot start KYC from state: {}", investor.lifecycle_state)
            ));
        }
        
        // 2. Create KYC case for the investor's entity
        let case_id = sqlx::query_scalar!(
            r#"INSERT INTO kyc.kyc_cases (cbu_id, case_type, status)
               VALUES ($1, $2, 'OPEN')
               RETURNING case_id"#,
            investor.owning_cbu_id,
            case_type
        )
        .fetch_one(&ctx.pool)
        .await?;
        
        // 3. Create entity workstream for the investor
        sqlx::query!(
            r#"INSERT INTO kyc.entity_workstreams (case_id, entity_id, discovery_reason, status)
               VALUES ($1, $2, 'INVESTOR_ONBOARDING', 'PENDING')"#,
            case_id,
            investor.entity_id
        )
        .execute(&ctx.pool)
        .await?;
        
        // 4. Transition lifecycle state
        sqlx::query!(
            r#"UPDATE kyc.investors 
               SET lifecycle_state = 'KYC_IN_PROGRESS',
                   kyc_status = 'IN_PROGRESS',
                   kyc_case_id = $2
               WHERE investor_id = $1"#,
            investor_id,
            case_id
        )
        .execute(&ctx.pool)
        .await?;
        
        Ok(ExecutionResult::Record(json!({
            "investor_id": investor_id,
            "case_id": case_id,
            "lifecycle_state": "KYC_IN_PROGRESS"
        })))
    }
}

#[async_trait]
impl PluginOp for InvestorApproveKycOp {
    async fn execute(
        &self,
        args: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        let investor_id = get_uuid_arg(args, "investor-id")?;
        let risk_rating = get_string_arg(args, "risk-rating")?;
        let expires_at = get_optional_date_arg(args, "kyc-expires-at");
        
        // Default expiry: 1 year for LOW, 6 months for MEDIUM/HIGH
        let default_expiry = match risk_rating.as_str() {
            "LOW" => chrono::Utc::now() + chrono::Duration::days(365),
            _ => chrono::Utc::now() + chrono::Duration::days(180),
        };
        
        let result = sqlx::query!(
            r#"UPDATE kyc.investors 
               SET lifecycle_state = 'KYC_APPROVED',
                   kyc_status = 'APPROVED',
                   kyc_risk_rating = $2,
                   kyc_approved_at = NOW(),
                   kyc_expires_at = COALESCE($3, $4)
               WHERE investor_id = $1
                 AND lifecycle_state = 'KYC_IN_PROGRESS'
               RETURNING investor_id"#,
            investor_id,
            risk_rating,
            expires_at,
            default_expiry.naive_utc()
        )
        .fetch_optional(&ctx.pool)
        .await?;
        
        match result {
            Some(_) => Ok(ExecutionResult::Affected(1)),
            None => Err(ExecutionError::ValidationError(
                "Investor not in KYC_IN_PROGRESS state".into()
            ))
        }
    }
}

// ... implement other handlers
```

---

## File Checklist

### New Files
- [ ] `migrations/011_investor_register.sql`
- [ ] `rust/config/verbs/investor.yaml`
- [ ] `rust/src/dsl_v2/custom_ops/investor_ops.rs`
- [ ] `scripts/verify_investor_register.sql`
- [ ] `rust/tests/scenarios/investor_lifecycle.dsl`

### Modified Files
- [ ] `rust/config/verbs/identifier.yaml` - Add provider schemes
- [ ] `rust/config/verbs/registry/holding.yaml` - Add investor_id, usage_type, create-for-investor
- [ ] `rust/config/verbs/registry/movement.yaml` - Add PE verbs (commit, capital-call, distribute)
- [ ] `rust/src/dsl_v2/custom_ops/mod.rs` - Register investor_ops
- [ ] `CLAUDE.md` - Update documentation

### Files to Delete/Replace
- [ ] `migrations/011_clearstream_investor_views.sql` → replaced by `011_investor_register.sql`

---

## Execution Order

1. **Phase 1**: Create investors table + lifecycle state machine
2. **Phase 2**: Enhance holdings table with investor_id, usage_type, provider
3. **Phase 3**: Create views (v_ta_investors, v_ubo_holdings, v_investor_register)
4. **Phase 4**: Holdings → UBO sync trigger
5. **Phase 5**: DSL verbs (investor.yaml + enhanced holding/movement)
6. **Phase 6**: BODS export view
7. **Phase 7**: Plugin handlers for lifecycle transitions
8. **Phase 8**: Testing + CLAUDE.md update

---

## Test Scenario

```clojure
;; =============================================================================
;; Investor Lifecycle: Enquiry → Active Holder → Offboard
;; =============================================================================

;; Setup: BNY client and their fund
(cbu.ensure :name "Acme Fund Manager" :jurisdiction "LU" :client-type "corporate" :as @client)
(share-class.create :cbu-id @client :name "Class A EUR" :isin "LU0001234567" :as @class-a)

;; 1. Investor enquires
(entity.create-proper-person :first-name "Alice" :last-name "Smith" :nationality "GB" :as @alice)
(investor.register 
  :entity-id @alice 
  :owning-cbu-id @client 
  :investor-type "RETAIL"
  :investor-category "INDIVIDUAL"
  :as @inv-alice)
;; State: ENQUIRY

;; 2. Submit documents
(investor.submit-documents :investor-id @inv-alice)
;; State: PENDING_DOCUMENTS

;; 3. Start KYC (creates case + workstream)
(investor.start-kyc :investor-id @inv-alice :case-type "INVESTOR_ONBOARDING")
;; State: KYC_IN_PROGRESS

;; 4. KYC approved
(investor.approve-kyc :investor-id @inv-alice :risk-rating "LOW")
;; State: KYC_APPROVED

;; 5. Make eligible
(investor.make-eligible :investor-id @inv-alice :eligible-fund-types ["UCITS"])
;; State: ELIGIBLE_TO_SUBSCRIBE

;; 6. Subscribe
(holding.create-for-investor :investor-id @inv-alice :share-class-id @class-a :as @holding)
(movement.subscribe 
  :holding-id @holding 
  :units 1000 
  :price-per-unit 100.00 
  :trade-date "2025-01-20"
  :reference "SUB-001")
;; State: SUBSCRIBED → ACTIVE_HOLDER (after settlement)

;; 7. Redeem and offboard
(movement.redeem :holding-id @holding :units 1000 :price-per-unit 105.00 :trade-date "2025-12-01" :reference "RED-001")
(investor.offboard :investor-id @inv-alice)
;; State: OFFBOARDED
```
