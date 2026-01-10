-- Migration 013: Capital Structure & Ownership Model
-- Completes GLEIF/BODS/Register triangulation for ownership reconciliation
--
-- This migration extends the capital structure from migration 009 with:
-- - Instrument identifier schemes (ISIN, SEDOL, CUSIP, etc.)
-- - Share class identifiers (many-to-one)
-- - Share class supply tracking (current state)
-- - Issuance events ledger (append-only supply changes)
-- - Dilution instruments (options, warrants, SAFEs, convertibles)
-- - Issuer control configuration (jurisdiction thresholds)
-- - Special rights (board seats, vetos - unified class/holder)
-- - Ownership snapshots (computed from register, imported from BODS/GLEIF)
-- - Reconciliation framework (compare sources, track findings)

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
CREATE UNIQUE INDEX IF NOT EXISTS idx_share_class_primary_identifier
    ON kyc.share_class_identifiers(share_class_id)
    WHERE is_primary = true AND valid_to IS NULL;

CREATE INDEX IF NOT EXISTS idx_share_class_identifiers_lookup
    ON kyc.share_class_identifiers(scheme_code, identifier_value)
    WHERE valid_to IS NULL;

COMMENT ON TABLE kyc.share_class_identifiers IS
    'Security identifiers for share classes. Every class has at least INTERNAL. External IDs (ISIN, etc.) optional.';

-- ============================================================================
-- 1.3: Extend share_classes for control computation
-- ============================================================================
-- Migration 009 added basic columns. We extend with additional control-related fields.

ALTER TABLE kyc.share_classes
    -- Instrument classification (more specific than share_type)
    ADD COLUMN IF NOT EXISTS instrument_kind VARCHAR(30) DEFAULT 'FUND_UNIT',

    -- Voting rights (use existing voting_rights_per_share, add cap/threshold)
    ADD COLUMN IF NOT EXISTS votes_per_unit NUMERIC(10,4) DEFAULT 1.0,
    ADD COLUMN IF NOT EXISTS voting_cap_pct NUMERIC(5,2),
    ADD COLUMN IF NOT EXISTS voting_threshold_pct NUMERIC(5,2),

    -- Economic rights
    ADD COLUMN IF NOT EXISTS economic_per_unit NUMERIC(10,4) DEFAULT 1.0,
    ADD COLUMN IF NOT EXISTS dividend_rate NUMERIC(10,4),
    ADD COLUMN IF NOT EXISTS liquidation_rank INTEGER DEFAULT 100,

    -- Conversion (for convertibles, warrants)
    ADD COLUMN IF NOT EXISTS converts_to_share_class_id UUID REFERENCES kyc.share_classes(id),
    ADD COLUMN IF NOT EXISTS conversion_ratio_num NUMERIC(10,4),
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

    -- As-of tracking
    as_of_date DATE NOT NULL DEFAULT CURRENT_DATE,
    as_of_event_id UUID,

    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT uq_supply_class_date UNIQUE (share_class_id, as_of_date)
);

CREATE INDEX IF NOT EXISTS idx_supply_class ON kyc.share_class_supply(share_class_id);
CREATE INDEX IF NOT EXISTS idx_supply_date ON kyc.share_class_supply(as_of_date DESC);

COMMENT ON TABLE kyc.share_class_supply IS
    'Current supply state per share class. Source of truth for denominators in control computation.';

-- Trigger to update totals when supply changes
CREATE OR REPLACE FUNCTION kyc.fn_update_supply_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_supply_timestamp ON kyc.share_class_supply;
CREATE TRIGGER trg_supply_timestamp
    BEFORE UPDATE ON kyc.share_class_supply
    FOR EACH ROW EXECUTE FUNCTION kyc.fn_update_supply_timestamp();

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

CREATE INDEX IF NOT EXISTS idx_issuance_class ON kyc.issuance_events(share_class_id, effective_date);
CREATE INDEX IF NOT EXISTS idx_issuance_issuer ON kyc.issuance_events(issuer_entity_id);
CREATE INDEX IF NOT EXISTS idx_issuance_status ON kyc.issuance_events(status) WHERE status = 'EFFECTIVE';

COMMENT ON TABLE kyc.issuance_events IS
    'Append-only ledger of supply changes. Source for computing share_class_supply at any as-of date.';

-- ============================================================================
-- 1.6: Dilution Instruments (Options, Warrants, Convertibles, SAFEs)
-- ============================================================================
-- Tracks potential future dilution from options, warrants, convertibles, SAFEs.
-- Required for FULLY_DILUTED basis computation.

CREATE TABLE IF NOT EXISTS kyc.dilution_instruments (
    instrument_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What company/fund this dilutes
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- What share class it converts INTO (the diluted class)
    converts_to_share_class_id UUID REFERENCES kyc.share_classes(id),

    -- Instrument type
    instrument_type VARCHAR(30) NOT NULL,

    -- Who holds this instrument
    holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Quantities
    units_granted NUMERIC(20,6) NOT NULL,
    units_exercised NUMERIC(20,6) DEFAULT 0,
    units_forfeited NUMERIC(20,6) DEFAULT 0,

    -- Conversion terms
    conversion_ratio NUMERIC(10,4) DEFAULT 1.0,
    exercise_price NUMERIC(20,6),
    exercise_currency VARCHAR(3),

    -- For SAFEs/Convertible Notes
    valuation_cap NUMERIC(20,2),
    discount_pct NUMERIC(5,2),
    principal_amount NUMERIC(20,2),

    -- Exercisability
    vesting_start_date DATE,
    vesting_end_date DATE,
    vesting_cliff_months INTEGER,
    exercisable_from DATE,
    expiration_date DATE,

    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE',

    -- Provenance
    grant_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    board_approval_ref VARCHAR(100),
    plan_name VARCHAR(100),

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    notes TEXT,

    CONSTRAINT chk_instrument_type CHECK (instrument_type IN (
        'STOCK_OPTION',
        'WARRANT',
        'CONVERTIBLE_NOTE',
        'SAFE',
        'CONVERTIBLE_PREFERRED',
        'RSU',
        'PHANTOM_STOCK',
        'SAR',
        'OTHER'
    )),
    CONSTRAINT chk_dilution_status CHECK (status IN (
        'ACTIVE', 'EXERCISED', 'EXPIRED', 'FORFEITED', 'CANCELLED'
    )),
    CONSTRAINT chk_units_positive CHECK (units_granted > 0),
    CONSTRAINT chk_exercised_lte_granted CHECK (units_exercised <= units_granted)
);

CREATE INDEX IF NOT EXISTS idx_dilution_issuer ON kyc.dilution_instruments(issuer_entity_id)
    WHERE status = 'ACTIVE';
CREATE INDEX IF NOT EXISTS idx_dilution_converts_to ON kyc.dilution_instruments(converts_to_share_class_id);
CREATE INDEX IF NOT EXISTS idx_dilution_holder ON kyc.dilution_instruments(holder_entity_id)
    WHERE holder_entity_id IS NOT NULL;

COMMENT ON TABLE kyc.dilution_instruments IS
    'Options, warrants, convertibles, SAFEs that may dilute existing shareholders. Required for FULLY_DILUTED computation.';

-- ============================================================================
-- 1.7: Dilution Exercise Events (audit trail)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.dilution_exercise_events (
    exercise_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instrument_id UUID NOT NULL REFERENCES kyc.dilution_instruments(instrument_id),

    -- Exercise details
    units_exercised NUMERIC(20,6) NOT NULL,
    exercise_date DATE NOT NULL,
    exercise_price_paid NUMERIC(20,6),

    -- Resulting shares
    shares_issued NUMERIC(20,6) NOT NULL,
    resulting_holding_id UUID REFERENCES kyc.holdings(id),

    -- For cashless exercise
    is_cashless BOOLEAN DEFAULT false,
    shares_withheld_for_tax NUMERIC(20,6),

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_exercise_instrument ON kyc.dilution_exercise_events(instrument_id);

COMMENT ON TABLE kyc.dilution_exercise_events IS
    'Audit trail of option/warrant exercises. Links to resulting holdings.';

-- ============================================================================
-- 1.8: Issuer Control Configuration
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

CREATE INDEX IF NOT EXISTS idx_control_config_issuer ON kyc.issuer_control_config(issuer_entity_id)
    WHERE effective_to IS NULL;

COMMENT ON TABLE kyc.issuer_control_config IS
    'Jurisdiction/articles-specific thresholds for control determination per issuer.';

-- ============================================================================
-- 1.9: Special Rights (class-level OR holder-level)
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

CREATE INDEX IF NOT EXISTS idx_special_rights_class ON kyc.special_rights(share_class_id)
    WHERE share_class_id IS NOT NULL AND effective_to IS NULL;
CREATE INDEX IF NOT EXISTS idx_special_rights_holder ON kyc.special_rights(holder_entity_id)
    WHERE holder_entity_id IS NOT NULL AND effective_to IS NULL;
CREATE INDEX IF NOT EXISTS idx_special_rights_issuer ON kyc.special_rights(issuer_entity_id);

COMMENT ON TABLE kyc.special_rights IS
    'Control rights not reducible to voting percentage. Attached to either share class or specific holder.';

-- ============================================================================
-- 1.10: Ownership Snapshots (the bridge)
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

CREATE UNIQUE INDEX IF NOT EXISTS idx_snapshot_current ON kyc.ownership_snapshots(
    issuer_entity_id, owner_entity_id, COALESCE(share_class_id, '00000000-0000-0000-0000-000000000000'::uuid), as_of_date, basis, derived_from
) WHERE superseded_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshot_issuer ON kyc.ownership_snapshots(issuer_entity_id, as_of_date)
    WHERE superseded_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_snapshot_owner ON kyc.ownership_snapshots(owner_entity_id, as_of_date)
    WHERE superseded_at IS NULL;

COMMENT ON TABLE kyc.ownership_snapshots IS
    'Computed ownership positions from register, or imported from BODS/GLEIF. Bridge for reconciliation.';

-- ============================================================================
-- 1.11: Reconciliation Framework
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

CREATE INDEX IF NOT EXISTS idx_recon_findings_run ON kyc.ownership_reconciliation_findings(run_id);
CREATE INDEX IF NOT EXISTS idx_recon_findings_open ON kyc.ownership_reconciliation_findings(resolution_status)
    WHERE resolution_status = 'OPEN';

-- ============================================================================
-- 1.12: BODS Interest Type Mapping
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.bods_right_type_mapping (
    bods_interest_type VARCHAR(50) PRIMARY KEY,
    maps_to_right_type VARCHAR(30),
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

-- ============================================================================
-- 2.1: Function - Compute supply at any as-of date from events ledger
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
        COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1),
        COALESCE(sc.economic_per_unit, 1),
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
                THEN (SELECT COALESCE(SUM(ie2.units_delta), 0) FROM kyc.issuance_events ie2
                      WHERE ie2.share_class_id = p_share_class_id
                      AND ie2.effective_date < ie.effective_date
                      AND ie2.status = 'EFFECTIVE') * (ie.ratio_to::NUMERIC / ie.ratio_from - 1)
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

COMMENT ON FUNCTION kyc.fn_share_class_supply_at IS
    'Compute supply at any as-of date from the issuance events ledger.';

-- ============================================================================
-- 2.2: Function - Compute diluted supply (FULLY_DILUTED / EXERCISABLE)
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_diluted_supply_at(
    p_share_class_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE,
    p_basis TEXT DEFAULT 'FULLY_DILUTED'
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
    SELECT COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1), COALESCE(sc.economic_per_unit, 1)
    INTO v_votes_per_unit, v_economic_per_unit
    FROM kyc.share_classes sc
    WHERE sc.id = p_share_class_id;

    -- Compute dilution from instruments that convert INTO this share class
    IF p_basis = 'FULLY_DILUTED' THEN
        -- All outstanding instruments (vested or not)
        SELECT
            COALESCE(SUM((di.units_granted - di.units_exercised - di.units_forfeited) * di.conversion_ratio), 0),
            COUNT(*)
        INTO v_dilution, v_dilution_count
        FROM kyc.dilution_instruments di
        WHERE di.converts_to_share_class_id = p_share_class_id
          AND di.status = 'ACTIVE'
          AND (di.expiration_date IS NULL OR di.expiration_date > p_as_of);

    ELSIF p_basis = 'EXERCISABLE' THEN
        -- Only currently exercisable instruments
        SELECT
            COALESCE(SUM((di.units_granted - di.units_exercised - di.units_forfeited) * di.conversion_ratio), 0),
            COUNT(*)
        INTO v_dilution, v_dilution_count
        FROM kyc.dilution_instruments di
        WHERE di.converts_to_share_class_id = p_share_class_id
          AND di.status = 'ACTIVE'
          AND (di.exercisable_from IS NULL OR di.exercisable_from <= p_as_of)
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

-- ============================================================================
-- 2.3: Function - Holder control position
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
        SELECT
            SUM(COALESCE(scs.issued_units, sc.issued_shares, 0) * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1)) AS total_votes,
            SUM(COALESCE(scs.issued_units, sc.issued_shares, 0) * COALESCE(sc.economic_per_unit, 1)) AS total_economic
        FROM kyc.share_classes sc
        LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
            AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id AND as_of_date <= p_as_of)
        WHERE sc.issuer_entity_id = p_issuer_entity_id
    ),
    holder_positions AS (
        -- Aggregate holdings per holder across all classes
        SELECT
            h.investor_entity_id,
            SUM(h.units) AS units,
            SUM(h.units * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1)) AS votes,
            SUM(h.units * COALESCE(sc.economic_per_unit, 1)) AS economic
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        WHERE sc.issuer_entity_id = p_issuer_entity_id
          AND h.status = 'active'
        GROUP BY h.investor_entity_id
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

COMMENT ON FUNCTION kyc.fn_holder_control_position IS
    'Compute holder control positions for an issuer including voting %, economic %, control flags, and board rights.';

-- ============================================================================
-- 2.4: Function - Derive ownership snapshots from register
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

COMMENT ON FUNCTION kyc.fn_derive_ownership_snapshots IS
    'Derive ownership snapshots from register holdings for an issuer at a given date. Returns count of snapshots created.';

-- ============================================================================
-- 2.5: Enhanced Capital Structure View (extends B.2 from migration 009)
-- ============================================================================

CREATE OR REPLACE VIEW kyc.v_capital_structure_extended AS
SELECT
    sc.id AS share_class_id,
    sc.cbu_id,
    sc.issuer_entity_id,
    sc.name AS share_class_name,
    sc.share_type,
    COALESCE(sc.instrument_kind, 'FUND_UNIT') AS instrument_kind,
    sc.class_category,
    sc.authorized_shares,
    COALESCE(scs.issued_units, sc.issued_shares) AS issued_shares,
    COALESCE(scs.outstanding_units, sc.issued_shares) AS outstanding_shares,
    COALESCE(scs.treasury_units, 0) AS treasury_shares,
    COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1) AS votes_per_unit,
    COALESCE(sc.economic_per_unit, 1) AS economic_per_unit,
    sc.par_value,
    sc.par_value_currency,
    sc.dividend_rights,
    sc.liquidation_preference,
    sc.liquidation_rank,
    -- Primary identifier
    sci.scheme_code AS primary_id_scheme,
    sci.identifier_value AS primary_id_value,
    -- Ownership data
    h.id AS holding_id,
    h.investor_entity_id,
    h.units,
    h.cost_basis,
    h.status AS holding_status,
    -- Ownership calculation
    CASE
        WHEN COALESCE(scs.issued_units, sc.issued_shares, 0) > 0
        THEN ROUND((h.units / COALESCE(scs.issued_units, sc.issued_shares)) * 100, 4)
        ELSE 0
    END AS ownership_pct,
    -- Voting rights calculation
    h.units * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1) AS holder_voting_rights,
    COALESCE(scs.issued_units, sc.issued_shares, 0) * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1) AS total_class_voting_rights,
    -- Entity details
    e.name AS investor_name,
    et.type_code AS investor_entity_type,
    ie.name AS issuer_name,
    iet.type_code AS issuer_entity_type
FROM kyc.share_classes sc
LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
    AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id)
LEFT JOIN kyc.share_class_identifiers sci ON sci.share_class_id = sc.id AND sci.is_primary = true AND sci.valid_to IS NULL
LEFT JOIN kyc.holdings h ON h.share_class_id = sc.id AND h.status = 'active'
LEFT JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
LEFT JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entities ie ON ie.entity_id = sc.issuer_entity_id
LEFT JOIN "ob-poc".entity_types iet ON ie.entity_type_id = iet.entity_type_id;

COMMENT ON VIEW kyc.v_capital_structure_extended IS
    'Extended capital structure view with supply tracking, identifiers, and computed ownership/voting percentages.';

-- ============================================================================
-- 2.6: Dilution Summary View
-- ============================================================================

CREATE OR REPLACE VIEW kyc.v_dilution_summary AS
SELECT
    di.issuer_entity_id,
    ie.name AS issuer_name,
    di.converts_to_share_class_id,
    sc.name AS target_share_class_name,
    di.instrument_type,
    di.status,
    COUNT(*) AS instrument_count,
    SUM(di.units_granted) AS total_granted,
    SUM(di.units_exercised) AS total_exercised,
    SUM(di.units_forfeited) AS total_forfeited,
    SUM(di.units_granted - di.units_exercised - di.units_forfeited) AS total_outstanding,
    SUM((di.units_granted - di.units_exercised - di.units_forfeited) * di.conversion_ratio) AS potential_dilution_shares,
    -- As percentage of current outstanding
    CASE
        WHEN COALESCE(scs.outstanding_units, sc.issued_shares, 0) > 0 THEN
            ROUND(
                SUM((di.units_granted - di.units_exercised - di.units_forfeited) * di.conversion_ratio)
                / COALESCE(scs.outstanding_units, sc.issued_shares) * 100,
                2
            )
        ELSE 0
    END AS dilution_pct
FROM kyc.dilution_instruments di
JOIN "ob-poc".entities ie ON ie.entity_id = di.issuer_entity_id
LEFT JOIN kyc.share_classes sc ON sc.id = di.converts_to_share_class_id
LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
    AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id)
WHERE di.status = 'ACTIVE'
GROUP BY
    di.issuer_entity_id, ie.name,
    di.converts_to_share_class_id, sc.name,
    di.instrument_type, di.status,
    scs.outstanding_units, sc.issued_shares;

COMMENT ON VIEW kyc.v_dilution_summary IS
    'Summary of dilution instruments by issuer and type, showing potential dilution impact.';

-- ============================================================================
-- Done
-- ============================================================================
