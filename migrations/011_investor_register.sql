-- =============================================================================
-- Migration 011: Investor Register with Full Lifecycle
-- =============================================================================
-- Purpose: Dual-purpose investor register supporting:
--   Use Case A: Transfer Agency KYC-as-a-Service (client's end investors)
--   Use Case B: UBO Intra-Group Holdings (≥25% = UBO candidate)
--
-- Replaces: 011_clearstream_investor_views.sql (provider-agnostic now)
-- =============================================================================

-- Drop old Clearstream-specific views (will be replaced with generic versions)
DROP VIEW IF EXISTS kyc.v_clearstream_register CASCADE;
DROP VIEW IF EXISTS kyc.v_clearstream_movements CASCADE;
DROP VIEW IF EXISTS kyc.v_bods_ownership_statements CASCADE;
DROP VIEW IF EXISTS kyc.v_share_class_summary CASCADE;
DROP VIEW IF EXISTS kyc.v_investor_portfolio CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_entity_identifier_xref CASCADE;

-- =============================================================================
-- PHASE 1: INVESTORS TABLE & LIFECYCLE STATE MACHINE
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1.1 Investors Table (links entity to investor-specific data)
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

    -- Rejection/suspension tracking
    rejection_reason TEXT,
    suspended_reason TEXT,
    pre_suspension_state VARCHAR(50),  -- State before suspension (for reinstatement)
    suspended_at TIMESTAMPTZ,
    offboard_reason TEXT,
    offboarded_at TIMESTAMPTZ,

    -- Subscription tracking
    first_subscription_at TIMESTAMPTZ,
    redemption_type VARCHAR(50),  -- FULL, PARTIAL (when in REDEEMING state)

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Unique: one investor record per entity per owning client
    UNIQUE(entity_id, owning_cbu_id)
);

COMMENT ON TABLE kyc.investors IS
'Investor register linking entities to investor-specific lifecycle and KYC status';

COMMENT ON COLUMN kyc.investors.investor_type IS
'RETAIL, PROFESSIONAL, INSTITUTIONAL, NOMINEE, INTRA_GROUP';

COMMENT ON COLUMN kyc.investors.investor_category IS
'HIGH_NET_WORTH, PENSION_FUND, INSURANCE, SOVEREIGN_WEALTH, FAMILY_OFFICE, CORPORATE, INDIVIDUAL';

COMMENT ON COLUMN kyc.investors.lifecycle_state IS
'ENQUIRY, PENDING_DOCUMENTS, KYC_IN_PROGRESS, KYC_APPROVED, KYC_REJECTED, ELIGIBLE_TO_SUBSCRIBE, SUBSCRIBED, ACTIVE_HOLDER, REDEEMING, OFFBOARDED, SUSPENDED, BLOCKED';

COMMENT ON COLUMN kyc.investors.kyc_status IS
'NOT_STARTED, IN_PROGRESS, APPROVED, REJECTED, EXPIRED, REFRESH_REQUIRED';

COMMENT ON COLUMN kyc.investors.provider IS
'CLEARSTREAM, EUROCLEAR, CSV_IMPORT, API_FEED, MANUAL';

COMMENT ON COLUMN kyc.investors.owning_cbu_id IS
'The BNY client (fund manager) who owns this investor relationship';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_investors_entity ON kyc.investors(entity_id);
CREATE INDEX IF NOT EXISTS idx_investors_lifecycle ON kyc.investors(lifecycle_state);
CREATE INDEX IF NOT EXISTS idx_investors_kyc_status ON kyc.investors(kyc_status);
CREATE INDEX IF NOT EXISTS idx_investors_owning_cbu ON kyc.investors(owning_cbu_id);
CREATE INDEX IF NOT EXISTS idx_investors_provider ON kyc.investors(provider, provider_reference);
CREATE INDEX IF NOT EXISTS idx_investors_kyc_expires ON kyc.investors(kyc_expires_at) WHERE kyc_expires_at IS NOT NULL;

-- -----------------------------------------------------------------------------
-- 1.2 Lifecycle State Transitions (validation table)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS kyc.investor_lifecycle_transitions (
    from_state VARCHAR(50) NOT NULL,
    to_state VARCHAR(50) NOT NULL,
    requires_kyc_approved BOOLEAN DEFAULT false,
    requires_document TEXT,  -- Document type required for transition
    auto_trigger VARCHAR(100),  -- Event that auto-triggers this transition
    PRIMARY KEY (from_state, to_state)
);

COMMENT ON TABLE kyc.investor_lifecycle_transitions IS
'Valid state transitions for investor lifecycle with requirements';

-- Insert valid transitions
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
('ELIGIBLE_TO_SUBSCRIBE', 'SUSPENDED', false, NULL),
('SUSPENDED', 'ELIGIBLE_TO_SUBSCRIBE', true, NULL),

-- Re-engagement
('KYC_REJECTED', 'PENDING_DOCUMENTS', false, NULL),
('OFFBOARDED', 'ENQUIRY', false, NULL)

ON CONFLICT DO NOTHING;

-- -----------------------------------------------------------------------------
-- 1.3 Lifecycle Transition Validation Trigger
-- -----------------------------------------------------------------------------
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
        NEW.updated_at := NOW();
        RETURN NEW;
    END IF;

    -- Reject invalid transition
    RAISE EXCEPTION 'Invalid lifecycle transition from % to % for investor %',
        OLD.lifecycle_state, NEW.lifecycle_state, OLD.investor_id;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_validate_investor_lifecycle ON kyc.investors;
CREATE TRIGGER trg_validate_investor_lifecycle
    BEFORE UPDATE OF lifecycle_state ON kyc.investors
    FOR EACH ROW
    WHEN (OLD.lifecycle_state IS DISTINCT FROM NEW.lifecycle_state)
    EXECUTE FUNCTION kyc.validate_investor_lifecycle_transition();

-- -----------------------------------------------------------------------------
-- 1.4 Investor Lifecycle Audit Trail
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

COMMENT ON TABLE kyc.investor_lifecycle_history IS
'Audit trail of all investor lifecycle state changes';

CREATE INDEX IF NOT EXISTS idx_investor_lifecycle_history ON kyc.investor_lifecycle_history(investor_id, transitioned_at DESC);

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

DROP TRIGGER IF EXISTS trg_log_investor_lifecycle ON kyc.investors;
CREATE TRIGGER trg_log_investor_lifecycle
    AFTER UPDATE OF lifecycle_state ON kyc.investors
    FOR EACH ROW
    EXECUTE FUNCTION kyc.log_investor_lifecycle_change();

-- =============================================================================
-- PHASE 2: ENHANCED HOLDINGS TABLE
-- =============================================================================

-- Add investor_id column to link holdings to investor records
ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS investor_id UUID REFERENCES kyc.investors(investor_id);

-- Add holding_status for lifecycle tracking (separate from old 'status' column)
ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS holding_status VARCHAR(50) DEFAULT 'ACTIVE';

-- Add provider tracking columns
ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS provider VARCHAR(50) DEFAULT 'MANUAL';

ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS provider_reference VARCHAR(100);

ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS provider_sync_at TIMESTAMPTZ;

-- Add usage_type to distinguish TA vs UBO holdings
ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS usage_type VARCHAR(20) DEFAULT 'TA';

COMMENT ON COLUMN kyc.holdings.investor_id IS
'Link to investor record (for TA use case - may be NULL for legacy data)';

COMMENT ON COLUMN kyc.holdings.holding_status IS
'PENDING, ACTIVE, SUSPENDED, CLOSED';

COMMENT ON COLUMN kyc.holdings.usage_type IS
'TA (Transfer Agency - client investors) or UBO (intra-group ownership)';

COMMENT ON COLUMN kyc.holdings.provider IS
'Data source: CLEARSTREAM, EUROCLEAR, CSV_IMPORT, API_FEED, MANUAL';

-- Indexes for new columns
CREATE INDEX IF NOT EXISTS idx_holdings_investor ON kyc.holdings(investor_id);
CREATE INDEX IF NOT EXISTS idx_holdings_usage_type ON kyc.holdings(usage_type);
CREATE INDEX IF NOT EXISTS idx_holdings_provider ON kyc.holdings(provider, provider_reference);
CREATE INDEX IF NOT EXISTS idx_holdings_status ON kyc.holdings(holding_status);

-- =============================================================================
-- PHASE 3: ENHANCED MOVEMENTS TABLE
-- =============================================================================

-- Update movement_type constraint to include PE/VC and lifecycle types
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
ADD COLUMN IF NOT EXISTS commitment_id UUID;  -- Links calls/distributions to original commitment

ALTER TABLE kyc.movements
ADD COLUMN IF NOT EXISTS call_number INTEGER;  -- For capital calls: 1st, 2nd, etc.

ALTER TABLE kyc.movements
ADD COLUMN IF NOT EXISTS distribution_type VARCHAR(50);  -- INCOME, CAPITAL, RETURN_OF_CAPITAL

COMMENT ON COLUMN kyc.movements.commitment_id IS
'For capital_call and distribution: links to the original commitment movement';

COMMENT ON COLUMN kyc.movements.call_number IS
'For capital_call: sequence number (1st call, 2nd call, etc.)';

COMMENT ON COLUMN kyc.movements.distribution_type IS
'For distributions: INCOME, CAPITAL_GAIN, RETURN_OF_CAPITAL, RECALLABLE';

-- Index for commitment tracking
CREATE INDEX IF NOT EXISTS idx_movements_commitment ON kyc.movements(commitment_id) WHERE commitment_id IS NOT NULL;

-- =============================================================================
-- PHASE 4: VIEWS FOR DUAL USE CASES
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 4.1 Transfer Agency Investor View (Use Case A)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_ta_investors AS
SELECT
    -- Investor details
    i.investor_id,
    i.entity_id,
    e.name AS investor_name,
    et.type_code AS entity_type,
    pp.nationality AS investor_country,
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

    -- Tax & regulatory
    i.tax_status,
    i.tax_jurisdiction,
    i.fatca_status,
    i.crs_status,

    -- Eligibility
    i.eligible_fund_types,
    i.restricted_jurisdictions,

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
    i.provider_sync_at,

    -- Timestamps
    i.created_at,
    i.updated_at

FROM kyc.investors i
JOIN "ob-poc".entities e ON i.entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
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

-- -----------------------------------------------------------------------------
-- 4.2 UBO-Qualified Holdings View (Use Case B)
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
    h.provider_reference,

    -- Share class context
    sc.isin,
    sc.name AS share_class_name,
    sc.cbu_id AS fund_cbu_id,
    c.name AS fund_name,

    -- Entity being owned (the fund entity)
    sc.entity_id AS owned_entity_id,

    -- Investor/owner details
    e.name AS owner_name,
    et.type_code AS owner_entity_type,
    COALESCE(pp.nationality, lc.jurisdiction) AS owner_country,

    -- Ownership percentage
    ROUND((h.units / NULLIF(total.total_units, 0)) * 100, 4) AS ownership_percentage,

    -- UBO qualification
    CASE
        WHEN total.total_units > 0 AND (h.units / total.total_units) >= 0.25
        THEN true ELSE false
    END AS is_ubo_qualified,

    -- UBO type determination
    CASE
        WHEN et.type_code IN ('proper_person', 'natural_person') THEN 'DIRECT_UBO'
        ELSE 'REQUIRES_GLEIF_TRACE'
    END AS ubo_determination,

    -- LEI for corporate tracing
    lei.id AS owner_lei

FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id AND lei.scheme = 'LEI'
CROSS JOIN LATERAL (
    SELECT COALESCE(SUM(h2.units), 0) AS total_units
    FROM kyc.holdings h2
    WHERE h2.share_class_id = sc.id
      AND COALESCE(h2.holding_status, h2.status) = 'active'
) total
WHERE COALESCE(h.holding_status, h.status) = 'active';

COMMENT ON VIEW kyc.v_ubo_holdings IS
'Holdings view for UBO discovery. Shows ownership percentage and UBO qualification.';

-- -----------------------------------------------------------------------------
-- 4.3 Investor Register Summary View (Provider-Agnostic)
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
    et.type_code AS investor_entity_type,
    COALESCE(pp.nationality, lc.jurisdiction) AS investor_country,
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
    COALESCE(h.holding_status, h.status) AS holding_status,

    -- Computed values
    h.units * COALESCE(sc.nav_per_share, 0) AS market_value,
    ROUND((h.units / NULLIF(total.total_units, 0)) * 100, 4) AS ownership_percentage,

    -- Identifiers
    lei.id AS investor_lei,
    clr.id AS clearstream_ref,

    -- Provider tracking
    COALESCE(h.provider, 'MANUAL') AS provider,
    h.provider_reference,
    h.provider_sync_at,

    -- Timestamps
    h.created_at AS holding_created_at,
    h.updated_at AS holding_updated_at

FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN kyc.investors i ON h.investor_id = i.investor_id
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id AND lei.scheme = 'LEI'
LEFT JOIN "ob-poc".entity_identifiers clr
    ON e.entity_id = clr.entity_id AND clr.scheme = 'CLEARSTREAM_KV'
CROSS JOIN LATERAL (
    SELECT COALESCE(SUM(h2.units), 0) AS total_units
    FROM kyc.holdings h2
    WHERE h2.share_class_id = sc.id
      AND COALESCE(h2.holding_status, h2.status) = 'active'
) total
WHERE COALESCE(h.holding_status, h.status) = 'active';

COMMENT ON VIEW kyc.v_investor_register IS
'Provider-agnostic investor register with holdings, identifiers, and ownership percentages';

-- -----------------------------------------------------------------------------
-- 4.4 Movement Report View (Provider-Agnostic)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_movements AS
SELECT
    -- Movement Details
    m.id AS movement_id,
    m.reference AS trans_ref,
    m.movement_type,
    m.units,
    m.price_per_unit,
    m.amount,
    m.currency,
    m.trade_date,
    m.settlement_date,
    m.status AS movement_status,
    m.notes,

    -- PE/VC specific
    m.commitment_id,
    m.call_number,
    m.distribution_type,

    -- Holding Context
    h.id AS holding_id,
    h.units AS current_holding_units,
    COALESCE(h.provider, 'MANUAL') AS provider,

    -- Share Class
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,

    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,

    -- Investor
    e.entity_id AS investor_entity_id,
    e.name AS investor_name,
    i.investor_id,
    i.lifecycle_state,
    clr_id.id AS clearstream_reference,
    lei.id AS investor_lei,

    -- Timestamps
    m.created_at,
    m.updated_at

FROM kyc.movements m
JOIN kyc.holdings h ON m.holding_id = h.id
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
LEFT JOIN kyc.investors i ON h.investor_id = i.investor_id

-- Clearstream KV reference
LEFT JOIN "ob-poc".entity_identifiers clr_id
    ON e.entity_id = clr_id.entity_id
    AND clr_id.scheme = 'CLEARSTREAM_KV'

-- LEI
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id
    AND lei.scheme = 'LEI';

COMMENT ON VIEW kyc.v_movements IS
'Movement/transaction log with investor and fund context (provider-agnostic)';

-- -----------------------------------------------------------------------------
-- 4.5 Share Class Summary View
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_share_class_summary AS
SELECT
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    sc.currency,
    sc.nav_per_share,
    sc.nav_date,
    sc.fund_type,
    sc.fund_structure,
    sc.investor_eligibility,
    sc.status,

    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    c.jurisdiction AS fund_jurisdiction,

    -- Aggregates
    COALESCE(stats.investor_count, 0) AS investor_count,
    COALESCE(stats.total_units, 0) AS total_units,
    CASE
        WHEN sc.nav_per_share IS NOT NULL
        THEN COALESCE(stats.total_units, 0) * sc.nav_per_share
        ELSE NULL
    END AS assets_under_management,

    -- Movement activity (last 30 days)
    COALESCE(activity.subscription_count, 0) AS subscriptions_30d,
    COALESCE(activity.redemption_count, 0) AS redemptions_30d,
    COALESCE(activity.net_flow_units, 0) AS net_flow_units_30d

FROM kyc.share_classes sc
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id

-- Holding statistics
LEFT JOIN LATERAL (
    SELECT
        COUNT(DISTINCT h.investor_entity_id) AS investor_count,
        SUM(h.units) AS total_units
    FROM kyc.holdings h
    WHERE h.share_class_id = sc.id
    AND COALESCE(h.holding_status, h.status) = 'active'
) stats ON true

-- Recent activity
LEFT JOIN LATERAL (
    SELECT
        COUNT(*) FILTER (WHERE m.movement_type IN ('subscription', 'initial_subscription', 'additional_subscription')) AS subscription_count,
        COUNT(*) FILTER (WHERE m.movement_type IN ('redemption', 'partial_redemption', 'full_redemption')) AS redemption_count,
        COALESCE(SUM(CASE
            WHEN m.movement_type IN ('subscription', 'initial_subscription', 'additional_subscription', 'transfer_in') THEN m.units
            WHEN m.movement_type IN ('redemption', 'partial_redemption', 'full_redemption', 'transfer_out') THEN -m.units
            ELSE 0
        END), 0) AS net_flow_units
    FROM kyc.movements m
    JOIN kyc.holdings h ON m.holding_id = h.id
    WHERE h.share_class_id = sc.id
    AND m.trade_date >= CURRENT_DATE - INTERVAL '30 days'
    AND m.status IN ('confirmed', 'settled')
) activity ON true;

COMMENT ON VIEW kyc.v_share_class_summary IS
'Share class summary with investor counts, AUM, and recent activity metrics';

-- -----------------------------------------------------------------------------
-- 4.6 Investor Portfolio View
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_investor_portfolio AS
SELECT
    -- Investor
    e.entity_id AS investor_entity_id,
    e.name AS investor_name,
    et.type_code AS investor_type,
    COALESCE(pp.nationality, lc.jurisdiction) AS investor_country,

    -- Investor lifecycle (if available)
    i.investor_id,
    i.lifecycle_state,
    i.kyc_status,
    i.kyc_risk_rating,

    -- Identifiers
    lei.id AS investor_lei,
    clr_id.id AS clearstream_reference,

    -- Holding
    h.id AS holding_id,
    h.units,
    h.cost_basis,
    h.acquisition_date,
    COALESCE(h.holding_status, h.status) AS holding_status,
    COALESCE(h.provider, 'MANUAL') AS provider,

    -- Share Class
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

    -- Computed values
    CASE
        WHEN sc.nav_per_share IS NOT NULL
        THEN h.units * sc.nav_per_share
        ELSE NULL
    END AS market_value,

    CASE
        WHEN h.cost_basis IS NOT NULL AND sc.nav_per_share IS NOT NULL
        THEN (h.units * sc.nav_per_share) - h.cost_basis
        ELSE NULL
    END AS unrealized_pnl

FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
JOIN kyc.holdings h ON e.entity_id = h.investor_entity_id
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN kyc.investors i ON h.investor_id = i.investor_id

-- LEI
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id
    AND lei.scheme = 'LEI'

-- Clearstream reference
LEFT JOIN "ob-poc".entity_identifiers clr_id
    ON e.entity_id = clr_id.entity_id
    AND clr_id.scheme = 'CLEARSTREAM_KV'

WHERE COALESCE(h.holding_status, h.status) = 'active';

COMMENT ON VIEW kyc.v_investor_portfolio IS
'Investor portfolio view showing all holdings across funds with market values';

-- -----------------------------------------------------------------------------
-- 4.7 Identifier Cross-Reference View
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW "ob-poc".v_entity_identifier_xref AS
SELECT
    e.entity_id,
    e.name AS entity_name,
    et.type_code AS entity_type,
    COALESCE(pp.nationality, lc.jurisdiction) AS country_code,

    -- Pivot common identifier schemes
    MAX(CASE WHEN ei.scheme = 'LEI' THEN ei.id END) AS lei,
    MAX(CASE WHEN ei.scheme = 'LEI' THEN ei.lei_status END) AS lei_status,
    MAX(CASE WHEN ei.scheme = 'CLEARSTREAM_KV' THEN ei.id END) AS clearstream_kv,
    MAX(CASE WHEN ei.scheme = 'CLEARSTREAM_ACCT' THEN ei.id END) AS clearstream_account,
    MAX(CASE WHEN ei.scheme = 'EUROCLEAR' THEN ei.id END) AS euroclear_id,
    MAX(CASE WHEN ei.scheme = 'company_register' THEN ei.id END) AS company_register_id,
    MAX(CASE WHEN ei.scheme = 'tax_id' THEN ei.id END) AS tax_id,
    MAX(CASE WHEN ei.scheme = 'ISIN' THEN ei.id END) AS isin,

    -- Count of all identifiers
    COUNT(ei.identifier_id) AS identifier_count,

    -- Validation status
    BOOL_OR(ei.is_validated) AS has_validated_identifier

FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN "ob-poc".entity_identifiers ei ON e.entity_id = ei.entity_id
GROUP BY e.entity_id, e.name, et.type_code, pp.nationality, lc.jurisdiction;

COMMENT ON VIEW "ob-poc".v_entity_identifier_xref IS
'Cross-reference view of all entity identifiers (LEI, Clearstream, Euroclear, Tax ID, etc.)';

-- =============================================================================
-- PHASE 5: BODS EXPORT VIEW
-- =============================================================================

-- Unified BODS Ownership Statements from all sources
CREATE OR REPLACE VIEW kyc.v_bods_ownership_statements AS

-- Source 1: Investor Register holdings (qualified UBOs)
SELECT
    'ooc-holding-' || h.holding_id::text AS statement_id,
    'ownershipOrControlStatement' AS statement_type,
    h.isin AS subject_identifier,
    h.fund_name AS subject_name,
    h.owner_name AS interested_party_name,
    h.owner_lei AS interested_party_lei,
    'shareholding' AS interest_type,
    'direct' AS interest_directness,
    h.units AS share_exact,
    h.ownership_percentage,
    h.is_ubo_qualified AS beneficial_ownership_or_control,
    h.acquisition_date AS interest_start_date,
    NULL::DATE AS interest_end_date,
    COALESCE(h.provider, 'MANUAL') AS source_type,
    h.provider_reference AS source_reference,
    CURRENT_DATE AS statement_date
FROM kyc.v_ubo_holdings h
WHERE h.is_ubo_qualified = true

UNION ALL

-- Source 2: Direct entity_relationships (not from holdings)
SELECT
    'ooc-rel-' || er.relationship_id::text AS statement_id,
    'ownershipOrControlStatement' AS statement_type,
    NULL AS subject_identifier,
    subject_e.name AS subject_name,
    owner_e.name AS interested_party_name,
    owner_lei.id AS interested_party_lei,
    COALESCE(er.interest_type, 'shareholding') AS interest_type,
    COALESCE(er.direct_or_indirect, 'direct') AS interest_directness,
    NULL::NUMERIC AS share_exact,
    er.percentage AS ownership_percentage,
    er.percentage >= 25 AS beneficial_ownership_or_control,
    er.effective_from AS interest_start_date,
    er.effective_to AS interest_end_date,
    COALESCE(er.source, 'MANUAL') AS source_type,
    er.relationship_id::text AS source_reference,
    CURRENT_DATE AS statement_date
FROM "ob-poc".entity_relationships er
JOIN "ob-poc".entities owner_e ON er.from_entity_id = owner_e.entity_id
JOIN "ob-poc".entities subject_e ON er.to_entity_id = subject_e.entity_id
LEFT JOIN "ob-poc".entity_identifiers owner_lei
    ON owner_e.entity_id = owner_lei.entity_id AND owner_lei.scheme = 'LEI'
WHERE er.relationship_type = 'ownership'
  AND er.source != 'INVESTOR_REGISTER'  -- Avoid double-count with holdings
  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE);

COMMENT ON VIEW kyc.v_bods_ownership_statements IS
'BODS 0.4 Ownership-or-Control Statement format for regulatory reporting (unified from all sources)';

-- =============================================================================
-- PHASE 6: UBO SYNC TRIGGER (Holdings ≥25% → entity_relationships)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.sync_holding_to_ubo_relationship()
RETURNS TRIGGER AS $$
DECLARE
    v_total_units NUMERIC;
    v_ownership_pct NUMERIC;
    v_fund_entity_id UUID;
BEGIN
    -- Get total units for percentage calculation
    SELECT COALESCE(SUM(units), 0) INTO v_total_units
    FROM kyc.holdings
    WHERE share_class_id = NEW.share_class_id
      AND COALESCE(holding_status, status) = 'active';

    -- Calculate ownership percentage
    IF v_total_units > 0 THEN
        v_ownership_pct := (NEW.units / v_total_units) * 100;
    ELSE
        v_ownership_pct := 0;
    END IF;

    -- Get fund entity ID from share class
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
        WHERE effective_to IS NULL
        DO UPDATE SET
            percentage = EXCLUDED.percentage,
            updated_at = NOW(),
            notes = EXCLUDED.notes;
    ELSE
        -- Remove relationship if dropped below 25%
        UPDATE "ob-poc".entity_relationships
        SET effective_to = CURRENT_DATE,
            updated_at = NOW()
        WHERE from_entity_id = NEW.investor_entity_id
          AND to_entity_id = v_fund_entity_id
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
          AND effective_to IS NULL;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_sync_holding_to_ubo ON kyc.holdings;
CREATE TRIGGER trg_sync_holding_to_ubo
    AFTER INSERT OR UPDATE OF units, holding_status, status ON kyc.holdings
    FOR EACH ROW
    EXECUTE FUNCTION kyc.sync_holding_to_ubo_relationship();

COMMENT ON FUNCTION kyc.sync_holding_to_ubo_relationship() IS
'Syncs holdings ≥25% to entity_relationships for UBO discovery';

-- =============================================================================
-- PHASE 7: ADDITIONAL INDEXES FOR PERFORMANCE
-- =============================================================================

-- Index for identifier lookups (including Euroclear)
CREATE INDEX IF NOT EXISTS idx_entity_identifiers_provider
ON "ob-poc".entity_identifiers(scheme, id)
WHERE scheme IN ('CLEARSTREAM_KV', 'CLEARSTREAM_ACCT', 'EUROCLEAR');

-- Index for share class by ISIN
CREATE INDEX IF NOT EXISTS idx_share_classes_isin
ON kyc.share_classes(isin)
WHERE isin IS NOT NULL;

-- Index for active holdings with usage type
CREATE INDEX IF NOT EXISTS idx_holdings_active_usage
ON kyc.holdings(share_class_id, investor_entity_id, usage_type)
WHERE COALESCE(holding_status, status) = 'active';

-- Index for movement lookups by date and type
CREATE INDEX IF NOT EXISTS idx_movements_trade_date
ON kyc.movements(trade_date, movement_type, status);

-- Index for KYC expiring soon
CREATE INDEX IF NOT EXISTS idx_investors_kyc_expiring
ON kyc.investors(kyc_expires_at)
WHERE kyc_status = 'APPROVED' AND kyc_expires_at IS NOT NULL;

-- =============================================================================
-- DONE
-- =============================================================================
