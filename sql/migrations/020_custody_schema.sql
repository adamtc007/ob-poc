-- =============================================================================
-- Migration 020: Custody & Settlement Schema
--
-- Implements the three-layer custody model aligned with industry standards:
-- - Layer 1: CBU Instrument Universe (what they trade)
-- - Layer 2: SSI Data (pure account data)
-- - Layer 3: Booking Rules (ALERT-style routing)
--
-- Industry Standards:
-- - ISO 10962 CFI codes
-- - SMPG/ALERT security types
-- - ISDA OTC taxonomy
-- =============================================================================

-- Create custody schema
CREATE SCHEMA IF NOT EXISTS custody;

-- =============================================================================
-- TAXONOMY REFERENCE TABLES
-- =============================================================================

-- Instrument Classes (canonical abstraction layer)
-- Maps to both CFI categories and SMPG groups
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

-- Security Types (ALERT-compatible codes for granular routing)
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

-- CFI Code Registry (Reference for incoming securities)
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

-- ISDA Product Taxonomy (For OTC derivatives)
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

-- =============================================================================
-- CORE REFERENCE TABLES
-- =============================================================================

-- Currencies
CREATE TABLE custody.currencies (
    currency_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    iso_code VARCHAR(3) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    decimal_places INTEGER DEFAULT 2,
    is_cls_eligible BOOLEAN DEFAULT false,
    is_active BOOLEAN DEFAULT true
);

-- Markets
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

-- Instruction Types
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

-- Sub-custodian Network (Bank's global agent network)
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

-- =============================================================================
-- THREE-LAYER TABLES
-- =============================================================================

-- LAYER 1: CBU Instrument Universe
-- "What does this CBU trade/hold?"
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

-- LAYER 2: CBU SSI Data (Pure account data - no routing logic)
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

-- LAYER 3: SSI Booking Rules (ALERT-style routing)
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

-- =============================================================================
-- SUPPORTING TABLES
-- =============================================================================

-- CBU SSI Agent Overrides (non-standard agent chain)
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

-- Instruction Paths (Profile → Service Resource routing)
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

-- Entity Settlement Identity (Counterparty settlement details)
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

-- Entity SSI (Counterparty's SSIs - sourced from ALERT)
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

-- ISDA Agreements (Link OTC trades to agreements)
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

-- ISDA Product Coverage (Which instrument classes an ISDA covers)
CREATE TABLE custody.isda_product_coverage (
    coverage_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    isda_id UUID NOT NULL REFERENCES custody.isda_agreements(isda_id) ON DELETE CASCADE,
    instrument_class_id UUID NOT NULL REFERENCES custody.instrument_classes(class_id),
    isda_taxonomy_id UUID REFERENCES custody.isda_product_taxonomy(taxonomy_id),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),

    UNIQUE(isda_id, instrument_class_id)
);

-- CSA Agreements (Credit Support Annex under ISDA)
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

-- =============================================================================
-- FUNCTIONS
-- =============================================================================

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

COMMENT ON FUNCTION custody.find_ssi_for_trade IS
'ALERT-style SSI lookup. Returns the first matching SSI based on booking rule priority.';
