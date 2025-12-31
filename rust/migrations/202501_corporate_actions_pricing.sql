-- =============================================================================
-- Phase 2: Corporate Actions & Pricing Extensions
-- Trading Matrix Implementation
-- =============================================================================

-- =============================================================================
-- CORPORATE ACTION TABLES
-- =============================================================================

-- CA event type definitions (reference data)
CREATE TABLE IF NOT EXISTS custody.ca_event_types (
    event_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_code VARCHAR(50) NOT NULL UNIQUE,
    event_name VARCHAR(255) NOT NULL,
    category VARCHAR(50) NOT NULL,
    is_elective BOOLEAN NOT NULL,
    default_election VARCHAR(50),
    iso_event_code VARCHAR(10),
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE custody.ca_event_types IS 'Reference data: corporate action event types';
COMMENT ON COLUMN custody.ca_event_types.category IS 'INCOME, REORGANIZATION, VOLUNTARY, MANDATORY, INFORMATION';

-- CBU CA preferences
CREATE TABLE IF NOT EXISTS custody.cbu_ca_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    processing_mode VARCHAR(20) NOT NULL,
    default_election VARCHAR(50),
    threshold_value DECIMAL(18,2),
    threshold_currency VARCHAR(3),
    notification_email VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, event_type_id, instrument_class_id)
);

COMMENT ON TABLE custody.cbu_ca_preferences IS 'CBU-specific corporate action processing preferences';
COMMENT ON COLUMN custody.cbu_ca_preferences.processing_mode IS 'AUTO_INSTRUCT, MANUAL, DEFAULT_ONLY, THRESHOLD';

-- CA instruction windows
CREATE TABLE IF NOT EXISTS custody.cbu_ca_instruction_windows (
    window_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    market_id UUID REFERENCES custody.markets(market_id),
    cutoff_days_before INTEGER NOT NULL,
    warning_days INTEGER DEFAULT 3,
    escalation_days INTEGER DEFAULT 1,
    escalation_contact VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, event_type_id, market_id)
);

COMMENT ON TABLE custody.cbu_ca_instruction_windows IS 'Instruction deadline configuration per CBU/event/market';

-- CA SSI mappings
CREATE TABLE IF NOT EXISTS custody.cbu_ca_ssi_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    currency VARCHAR(3) NOT NULL,
    ssi_id UUID NOT NULL REFERENCES custody.cbu_ssi(ssi_id),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, event_type_id, currency)
);

COMMENT ON TABLE custody.cbu_ca_ssi_mappings IS 'SSI mapping for CA payment/delivery by event type and currency';

-- Indexes for CA tables
CREATE INDEX IF NOT EXISTS idx_cbu_ca_preferences_cbu ON custody.cbu_ca_preferences(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_ca_instruction_windows_cbu ON custody.cbu_ca_instruction_windows(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_ca_ssi_mappings_cbu ON custody.cbu_ca_ssi_mappings(cbu_id);

-- =============================================================================
-- PRICING EXTENSION TABLES
-- =============================================================================

-- Valuation schedule
CREATE TABLE IF NOT EXISTS custody.cbu_valuation_schedule (
    schedule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    valuation_frequency VARCHAR(20) NOT NULL,
    valuation_time VARCHAR(10),
    timezone VARCHAR(50),
    business_days_only BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, market_id)
);

COMMENT ON TABLE custody.cbu_valuation_schedule IS 'Position valuation frequency and timing per CBU';
COMMENT ON COLUMN custody.cbu_valuation_schedule.valuation_frequency IS 'REAL_TIME, INTRADAY, EOD, T_PLUS_1, WEEKLY, MONTHLY';

-- Pricing fallback chains
CREATE TABLE IF NOT EXISTS custody.cbu_pricing_fallback_chains (
    chain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    fallback_sources TEXT[] NOT NULL,
    fallback_trigger VARCHAR(20) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, market_id)
);

COMMENT ON TABLE custody.cbu_pricing_fallback_chains IS 'Multi-source pricing fallback configuration';
COMMENT ON COLUMN custody.cbu_pricing_fallback_chains.fallback_trigger IS 'STALE, MISSING, THRESHOLD_BREACH, ANY_FAILURE';

-- Stale price policies
CREATE TABLE IF NOT EXISTS custody.cbu_stale_price_policies (
    policy_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    max_age_hours INTEGER NOT NULL,
    stale_action VARCHAR(20) NOT NULL,
    escalation_contact VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id)
);

COMMENT ON TABLE custody.cbu_stale_price_policies IS 'Stale price handling policy per CBU/instrument class';
COMMENT ON COLUMN custody.cbu_stale_price_policies.stale_action IS 'USE_LAST, USE_FALLBACK, ESCALATE, SUSPEND_NAV, MANUAL_OVERRIDE';

-- NAV impact thresholds
CREATE TABLE IF NOT EXISTS custody.cbu_nav_impact_thresholds (
    threshold_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    threshold_pct DECIMAL(5,2) NOT NULL,
    threshold_action VARCHAR(20) NOT NULL,
    notification_email VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id)
);

COMMENT ON TABLE custody.cbu_nav_impact_thresholds IS 'NAV impact alert thresholds per CBU';
COMMENT ON COLUMN custody.cbu_nav_impact_thresholds.threshold_action IS 'ALERT, MANUAL_REVIEW, SUSPEND_POSITION, ESCALATE';

-- =============================================================================
-- SEED REFERENCE DATA
-- =============================================================================

-- Common CA event types
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code)
VALUES
    -- Income events
    ('CASH_DIV', 'Cash Dividend', 'INCOME', false, 'CASH', 'DVCA'),
    ('STOCK_DIV', 'Stock Dividend', 'INCOME', false, 'STOCK', 'DVSE'),
    ('INTEREST', 'Interest Payment', 'INCOME', false, 'CASH', 'INTR'),
    ('DISTRIBUTION', 'Fund Distribution', 'INCOME', false, 'CASH', 'DVCA'),
    -- Elective events
    ('DIV_OPTION', 'Dividend Option', 'VOLUNTARY', true, 'CASH', 'DVOP'),
    ('RIGHTS_ISSUE', 'Rights Issue', 'VOLUNTARY', true, 'LAPSE', 'RHTS'),
    ('TENDER_OFFER', 'Tender Offer', 'VOLUNTARY', true, 'DECLINE', 'TEND'),
    ('BUYBACK', 'Share Buyback', 'VOLUNTARY', true, 'DECLINE', 'BIDS'),
    ('CONVERSION', 'Conversion', 'VOLUNTARY', true, 'NO_ACTION', 'CONV'),
    -- Mandatory events
    ('STOCK_SPLIT', 'Stock Split', 'MANDATORY', false, NULL, 'SPLF'),
    ('REVERSE_SPLIT', 'Reverse Stock Split', 'MANDATORY', false, NULL, 'SPLR'),
    ('NAME_CHANGE', 'Name Change', 'MANDATORY', false, NULL, 'CHAN'),
    ('MERGER', 'Merger', 'REORGANIZATION', false, NULL, 'MRGR'),
    ('SPINOFF', 'Spinoff', 'REORGANIZATION', false, NULL, 'SOFF'),
    ('EXCHANGE_OFFER', 'Exchange Offer', 'REORGANIZATION', true, 'CASH', 'EXOF'),
    -- Information events
    ('AGM', 'Annual General Meeting', 'INFORMATION', false, NULL, 'MEET'),
    ('PROXY', 'Proxy Vote', 'INFORMATION', true, NULL, 'MEET')
ON CONFLICT DO NOTHING;
