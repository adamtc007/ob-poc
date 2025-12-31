-- =============================================================================
-- PHASE 3: CROSS-BORDER SETTLEMENT & TAX CONFIGURATION
-- =============================================================================
-- Created: 2025-01-01
-- Purpose: Settlement chain configuration and tax withholding management
-- =============================================================================

-- =============================================================================
-- SETTLEMENT CHAIN TABLES
-- =============================================================================

-- Settlement chain definitions (multi-hop settlement paths)
CREATE TABLE IF NOT EXISTS custody.cbu_settlement_chains (
    chain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    chain_name VARCHAR(100) NOT NULL,
    market_id UUID REFERENCES custody.markets(market_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    currency VARCHAR(3),
    settlement_type VARCHAR(20),
    is_default BOOLEAN DEFAULT false,
    is_active BOOLEAN DEFAULT true,
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    expiry_date DATE,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, chain_name)
);

-- Settlement chain hops (intermediaries in the chain)
CREATE TABLE IF NOT EXISTS custody.settlement_chain_hops (
    hop_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id UUID NOT NULL REFERENCES custody.cbu_settlement_chains(chain_id) ON DELETE CASCADE,
    hop_sequence INTEGER NOT NULL,
    intermediary_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    intermediary_bic VARCHAR(11),
    intermediary_name VARCHAR(255),
    role VARCHAR(50) NOT NULL,  -- CUSTODIAN, SUBCUSTODIAN, AGENT, CSD, ICSD
    account_number VARCHAR(50),
    ssi_id UUID REFERENCES custody.cbu_ssi(ssi_id),
    instructions TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(chain_id, hop_sequence)
);

-- CSD/ICSD settlement locations
CREATE TABLE IF NOT EXISTS custody.settlement_locations (
    location_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    location_code VARCHAR(20) NOT NULL UNIQUE,
    location_name VARCHAR(255) NOT NULL,
    location_type VARCHAR(20) NOT NULL,  -- CSD, ICSD, CUSTODIAN
    country_code VARCHAR(2),
    bic VARCHAR(11),
    operating_hours JSONB,  -- { "open": "08:00", "close": "18:00", "timezone": "CET" }
    settlement_cycles JSONB,  -- { "DVP": "T+2", "FOP": "T+0" }
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- CBU settlement location preferences
CREATE TABLE IF NOT EXISTS custody.cbu_settlement_location_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    market_id UUID REFERENCES custody.markets(market_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    preferred_location_id UUID NOT NULL REFERENCES custody.settlement_locations(location_id),
    priority INTEGER NOT NULL DEFAULT 50,
    reason TEXT,
    effective_date DATE DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, market_id, instrument_class_id, preferred_location_id)
);

-- Cross-border settlement configuration
CREATE TABLE IF NOT EXISTS custody.cbu_cross_border_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    source_market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    target_market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    settlement_method VARCHAR(50) NOT NULL,  -- BRIDGE, DIRECT, VIA_ICSD
    bridge_location_id UUID REFERENCES custody.settlement_locations(location_id),
    preferred_currency VARCHAR(3),
    fx_timing VARCHAR(20),  -- PRE_SETTLEMENT, ON_SETTLEMENT, POST_SETTLEMENT
    additional_days INTEGER DEFAULT 0,
    special_instructions TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, source_market_id, target_market_id)
);

-- =============================================================================
-- TAX CONFIGURATION TABLES
-- =============================================================================

-- Tax jurisdiction reference data
CREATE TABLE IF NOT EXISTS custody.tax_jurisdictions (
    jurisdiction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    jurisdiction_code VARCHAR(10) NOT NULL UNIQUE,
    jurisdiction_name VARCHAR(255) NOT NULL,
    country_code VARCHAR(2) NOT NULL,
    default_withholding_rate DECIMAL(5,2),
    treaty_network JSONB,  -- { "US": 15, "DE": 0, "FR": 10 }
    reclaim_available BOOLEAN DEFAULT true,
    reclaim_deadline_days INTEGER,
    tax_authority_name VARCHAR(255),
    tax_authority_code VARCHAR(50),
    documentation_requirements JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Tax treaty rates (bilateral agreements)
CREATE TABLE IF NOT EXISTS custody.tax_treaty_rates (
    treaty_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    investor_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    income_type VARCHAR(50) NOT NULL,  -- DIVIDEND, INTEREST, ROYALTY, CAPITAL_GAIN
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    standard_rate DECIMAL(5,2) NOT NULL,
    treaty_rate DECIMAL(5,2) NOT NULL,
    beneficial_owner_required BOOLEAN DEFAULT true,
    documentation_codes TEXT[],  -- Required form codes (e.g., W-8BEN, DAS-1)
    effective_date DATE NOT NULL,
    expiry_date DATE,
    treaty_reference VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(source_jurisdiction_id, investor_jurisdiction_id, income_type, instrument_class_id)
);

-- CBU tax status and documentation
CREATE TABLE IF NOT EXISTS custody.cbu_tax_status (
    status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    tax_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    investor_type VARCHAR(50) NOT NULL,  -- PENSION, SOVEREIGN, CHARITY, CORPORATE, INDIVIDUAL, FUND
    tax_exempt BOOLEAN DEFAULT false,
    exempt_reason TEXT,
    documentation_status VARCHAR(20) DEFAULT 'PENDING',  -- PENDING, SUBMITTED, VALIDATED, EXPIRED
    documentation_expiry DATE,
    applicable_treaty_rate DECIMAL(5,2),
    qualified_intermediary BOOLEAN DEFAULT false,
    qi_ein VARCHAR(20),
    fatca_status VARCHAR(20),  -- EXEMPT, PARTICIPATING, NON_PARTICIPATING
    crs_status VARCHAR(20),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, tax_jurisdiction_id)
);

-- Tax reclaim configuration
CREATE TABLE IF NOT EXISTS custody.cbu_tax_reclaim_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    source_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    reclaim_method VARCHAR(30) NOT NULL,  -- AUTOMATIC, MANUAL, OUTSOURCED, NO_RECLAIM
    service_provider_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    minimum_reclaim_amount DECIMAL(15,2),
    minimum_reclaim_currency VARCHAR(3),
    batch_frequency VARCHAR(20),  -- IMMEDIATE, WEEKLY, MONTHLY, QUARTERLY
    expected_recovery_days INTEGER,
    fee_structure JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, source_jurisdiction_id)
);

-- Tax reporting obligations
CREATE TABLE IF NOT EXISTS custody.cbu_tax_reporting (
    reporting_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    reporting_regime VARCHAR(30) NOT NULL,  -- FATCA, CRS, DAC6, UK_CDOT, QI, 871M
    reporting_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    reporting_status VARCHAR(20) DEFAULT 'REQUIRED',  -- REQUIRED, EXEMPT, PARTICIPATING, PENDING
    giin VARCHAR(20),  -- Global Intermediary Identification Number (FATCA)
    registration_date DATE,
    last_report_date DATE,
    next_report_due DATE,
    reporting_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    sponsor_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, reporting_regime, reporting_jurisdiction_id)
);

-- =============================================================================
-- INDEXES
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_cbu_settlement_chains_cbu ON custody.cbu_settlement_chains(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_settlement_chains_lookup ON custody.cbu_settlement_chains(cbu_id, market_id, instrument_class_id);
CREATE INDEX IF NOT EXISTS idx_settlement_chain_hops_chain ON custody.settlement_chain_hops(chain_id);
CREATE INDEX IF NOT EXISTS idx_cbu_settlement_location_prefs_cbu ON custody.cbu_settlement_location_preferences(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_cross_border_cbu ON custody.cbu_cross_border_config(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_tax_status_cbu ON custody.cbu_tax_status(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_tax_reclaim_cbu ON custody.cbu_tax_reclaim_config(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_tax_reporting_cbu ON custody.cbu_tax_reporting(cbu_id);
CREATE INDEX IF NOT EXISTS idx_tax_treaty_rates_lookup ON custody.tax_treaty_rates(source_jurisdiction_id, investor_jurisdiction_id);

-- =============================================================================
-- SEED DATA: Common Settlement Locations
-- =============================================================================

INSERT INTO custody.settlement_locations (location_code, location_name, location_type, country_code, bic, settlement_cycles) VALUES
    ('DTCC', 'Depository Trust & Clearing Corporation', 'CSD', 'US', 'DTCYUS33', '{"DVP": "T+1", "FOP": "T+0"}'::jsonb),
    ('EUROCLEAR', 'Euroclear Bank', 'ICSD', 'BE', 'MABOROP2', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('CLEARSTREAM', 'Clearstream Banking', 'ICSD', 'LU', 'CABOROPP', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('CREST', 'Euroclear UK & International', 'CSD', 'GB', 'CABOROCP', '{"DVP": "T+1", "FOP": "T+0"}'::jsonb),
    ('CLEARSTREAM_FFT', 'Clearstream Frankfurt', 'CSD', 'DE', 'DAKADEFF', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('EUROCLEAR_FR', 'Euroclear France', 'CSD', 'FR', 'SICAFR2P', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('MONTE_TITOLI', 'Monte Titoli', 'CSD', 'IT', 'MABOROIT', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('IBERCLEAR', 'Iberclear', 'CSD', 'ES', 'IBERCMADESS', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('SIX_SIS', 'SIX SIS', 'CSD', 'CH', 'INSECHZZ', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('JASDEC', 'Japan Securities Depository Center', 'CSD', 'JP', 'JASDECJP', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('HKSCC', 'Hong Kong Securities Clearing', 'CSD', 'HK', 'HKSCCCLCH', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb),
    ('ASX_CS', 'ASX Clear / CHESS', 'CSD', 'AU', 'XASXAU2S', '{"DVP": "T+2", "FOP": "T+0"}'::jsonb)
ON CONFLICT (location_code) DO NOTHING;

-- =============================================================================
-- SEED DATA: Major Tax Jurisdictions
-- =============================================================================

INSERT INTO custody.tax_jurisdictions (jurisdiction_code, jurisdiction_name, country_code, default_withholding_rate, reclaim_available, reclaim_deadline_days) VALUES
    ('US', 'United States', 'US', 30.00, true, 1095),
    ('GB', 'United Kingdom', 'GB', 0.00, false, NULL),
    ('DE', 'Germany', 'DE', 26.375, true, 1460),
    ('FR', 'France', 'FR', 30.00, true, 730),
    ('IT', 'Italy', 'IT', 26.00, true, 1460),
    ('ES', 'Spain', 'ES', 19.00, true, 1460),
    ('CH', 'Switzerland', 'CH', 35.00, true, 1095),
    ('JP', 'Japan', 'JP', 20.42, true, 1095),
    ('HK', 'Hong Kong', 'HK', 0.00, false, NULL),
    ('SG', 'Singapore', 'SG', 0.00, false, NULL),
    ('AU', 'Australia', 'AU', 30.00, true, 1460),
    ('CA', 'Canada', 'CA', 25.00, true, 730),
    ('NL', 'Netherlands', 'NL', 15.00, true, 1095),
    ('BE', 'Belgium', 'BE', 30.00, true, 1095),
    ('LU', 'Luxembourg', 'LU', 15.00, true, 1095),
    ('IE', 'Ireland', 'IE', 25.00, true, 1460)
ON CONFLICT (jurisdiction_code) DO NOTHING;

-- =============================================================================
-- SEED DATA: Common Treaty Rates (US source, major investor jurisdictions)
-- =============================================================================

-- Get US jurisdiction ID
DO $$
DECLARE
    us_id UUID;
    gb_id UUID;
    de_id UUID;
    fr_id UUID;
    jp_id UUID;
    ch_id UUID;
    lu_id UUID;
    ie_id UUID;
BEGIN
    SELECT jurisdiction_id INTO us_id FROM custody.tax_jurisdictions WHERE jurisdiction_code = 'US';
    SELECT jurisdiction_id INTO gb_id FROM custody.tax_jurisdictions WHERE jurisdiction_code = 'GB';
    SELECT jurisdiction_id INTO de_id FROM custody.tax_jurisdictions WHERE jurisdiction_code = 'DE';
    SELECT jurisdiction_id INTO fr_id FROM custody.tax_jurisdictions WHERE jurisdiction_code = 'FR';
    SELECT jurisdiction_id INTO jp_id FROM custody.tax_jurisdictions WHERE jurisdiction_code = 'JP';
    SELECT jurisdiction_id INTO ch_id FROM custody.tax_jurisdictions WHERE jurisdiction_code = 'CH';
    SELECT jurisdiction_id INTO lu_id FROM custody.tax_jurisdictions WHERE jurisdiction_code = 'LU';
    SELECT jurisdiction_id INTO ie_id FROM custody.tax_jurisdictions WHERE jurisdiction_code = 'IE';

    -- US source dividends - treaty rates
    INSERT INTO custody.tax_treaty_rates (source_jurisdiction_id, investor_jurisdiction_id, income_type, standard_rate, treaty_rate, effective_date, documentation_codes) VALUES
        (us_id, gb_id, 'DIVIDEND', 30.00, 15.00, '2003-03-31', ARRAY['W-8BEN', 'W-8BEN-E']),
        (us_id, de_id, 'DIVIDEND', 30.00, 15.00, '2006-12-28', ARRAY['W-8BEN', 'W-8BEN-E']),
        (us_id, fr_id, 'DIVIDEND', 30.00, 15.00, '1995-12-30', ARRAY['W-8BEN', 'W-8BEN-E']),
        (us_id, jp_id, 'DIVIDEND', 30.00, 10.00, '2019-08-30', ARRAY['W-8BEN', 'W-8BEN-E']),
        (us_id, ch_id, 'DIVIDEND', 30.00, 15.00, '1998-01-01', ARRAY['W-8BEN', 'W-8BEN-E']),
        (us_id, lu_id, 'DIVIDEND', 30.00, 15.00, '2001-01-01', ARRAY['W-8BEN', 'W-8BEN-E']),
        (us_id, ie_id, 'DIVIDEND', 30.00, 15.00, '1998-01-01', ARRAY['W-8BEN', 'W-8BEN-E'])
    ON CONFLICT (source_jurisdiction_id, investor_jurisdiction_id, income_type, instrument_class_id) DO NOTHING;

    -- US source interest - treaty rates (generally 0%)
    INSERT INTO custody.tax_treaty_rates (source_jurisdiction_id, investor_jurisdiction_id, income_type, standard_rate, treaty_rate, effective_date) VALUES
        (us_id, gb_id, 'INTEREST', 30.00, 0.00, '2003-03-31'),
        (us_id, de_id, 'INTEREST', 30.00, 0.00, '2006-12-28'),
        (us_id, fr_id, 'INTEREST', 30.00, 0.00, '1995-12-30'),
        (us_id, jp_id, 'INTEREST', 30.00, 0.00, '2019-08-30'),
        (us_id, ch_id, 'INTEREST', 30.00, 0.00, '1998-01-01'),
        (us_id, lu_id, 'INTEREST', 30.00, 0.00, '2001-01-01'),
        (us_id, ie_id, 'INTEREST', 30.00, 0.00, '1998-01-01')
    ON CONFLICT (source_jurisdiction_id, investor_jurisdiction_id, income_type, instrument_class_id) DO NOTHING;
END $$;
