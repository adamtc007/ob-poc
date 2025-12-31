-- ==============================================================================
-- PHASE 3: Settlement Chain and Tax Configuration Tables
-- ==============================================================================
-- These tables support:
--   - Multi-hop settlement chains (CSD/ICSD routing)
--   - Cross-border settlement configuration
--   - Tax jurisdiction reference data
--   - Tax treaty rates
--   - CBU tax status and documentation
--   - Tax reclaim configuration
--   - Tax reporting obligations (FATCA, CRS, etc.)
-- ==============================================================================

-- ==============================================================================
-- SETTLEMENT LOCATION REFERENCE DATA
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.settlement_locations (
    location_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    location_code VARCHAR(20) NOT NULL UNIQUE,
    location_name VARCHAR(200) NOT NULL,
    location_type VARCHAR(20) NOT NULL CHECK (location_type IN ('CSD', 'ICSD', 'CUSTODIAN')),
    country_code VARCHAR(2),
    bic VARCHAR(11),
    operating_hours JSONB,
    settlement_cycles JSONB,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_settlement_locations_code ON custody.settlement_locations(location_code);
CREATE INDEX IF NOT EXISTS idx_settlement_locations_type ON custody.settlement_locations(location_type);

COMMENT ON TABLE custody.settlement_locations IS 'Reference data for CSDs, ICSDs, and custodian locations';

-- ==============================================================================
-- CBU SETTLEMENT CHAINS
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_settlement_chains (
    chain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    chain_name VARCHAR(100) NOT NULL,
    market_id UUID REFERENCES custody.markets(market_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    currency VARCHAR(3),
    settlement_type VARCHAR(10),
    is_default BOOLEAN NOT NULL DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    effective_date DATE,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cbu_id, chain_name)
);

CREATE INDEX IF NOT EXISTS idx_cbu_settlement_chains_cbu ON custody.cbu_settlement_chains(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_settlement_chains_market ON custody.cbu_settlement_chains(market_id);

COMMENT ON TABLE custody.cbu_settlement_chains IS 'Settlement chain definitions per CBU';

-- ==============================================================================
-- SETTLEMENT CHAIN HOPS (INTERMEDIARIES)
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.settlement_chain_hops (
    hop_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id UUID NOT NULL REFERENCES custody.cbu_settlement_chains(chain_id) ON DELETE CASCADE,
    hop_sequence INTEGER NOT NULL,
    role VARCHAR(20) NOT NULL CHECK (role IN ('CUSTODIAN', 'SUBCUSTODIAN', 'AGENT', 'CSD', 'ICSD')),
    intermediary_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    intermediary_bic VARCHAR(11),
    intermediary_name VARCHAR(200),
    account_number VARCHAR(50),
    ssi_id UUID REFERENCES custody.cbu_ssi(ssi_id),
    instructions TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(chain_id, hop_sequence)
);

CREATE INDEX IF NOT EXISTS idx_settlement_chain_hops_chain ON custody.settlement_chain_hops(chain_id);

COMMENT ON TABLE custody.settlement_chain_hops IS 'Individual hops/intermediaries in a settlement chain';

-- ==============================================================================
-- SETTLEMENT LOCATION PREFERENCES
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_settlement_location_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    market_id UUID REFERENCES custody.markets(market_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    preferred_location_id UUID NOT NULL REFERENCES custody.settlement_locations(location_id),
    priority INTEGER NOT NULL DEFAULT 50,
    reason TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cbu_id, market_id, instrument_class_id, preferred_location_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_settlement_loc_prefs_cbu ON custody.cbu_settlement_location_preferences(cbu_id);

COMMENT ON TABLE custody.cbu_settlement_location_preferences IS 'Preferred settlement locations per CBU/market/instrument';

-- ==============================================================================
-- CROSS-BORDER CONFIGURATION
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_cross_border_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    source_market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    target_market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    settlement_method VARCHAR(20) NOT NULL CHECK (settlement_method IN ('BRIDGE', 'DIRECT', 'VIA_ICSD')),
    bridge_location_id UUID REFERENCES custody.settlement_locations(location_id),
    preferred_currency VARCHAR(3),
    fx_timing VARCHAR(20) CHECK (fx_timing IN ('PRE_SETTLEMENT', 'ON_SETTLEMENT', 'POST_SETTLEMENT')),
    additional_days INTEGER DEFAULT 0,
    special_instructions TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cbu_id, source_market_id, target_market_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_cross_border_cbu ON custody.cbu_cross_border_config(cbu_id);

COMMENT ON TABLE custody.cbu_cross_border_config IS 'Cross-border settlement routing configuration';

-- ==============================================================================
-- TAX JURISDICTIONS REFERENCE DATA
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.tax_jurisdictions (
    jurisdiction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    jurisdiction_code VARCHAR(10) NOT NULL UNIQUE,
    jurisdiction_name VARCHAR(200) NOT NULL,
    country_code VARCHAR(2) NOT NULL,
    default_withholding_rate DECIMAL(5,3),
    reclaim_available BOOLEAN NOT NULL DEFAULT true,
    reclaim_deadline_days INTEGER,
    tax_authority_name VARCHAR(200),
    tax_authority_code VARCHAR(50),
    documentation_requirements JSONB,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_code ON custody.tax_jurisdictions(jurisdiction_code);
CREATE INDEX IF NOT EXISTS idx_tax_jurisdictions_country ON custody.tax_jurisdictions(country_code);

COMMENT ON TABLE custody.tax_jurisdictions IS 'Tax jurisdiction reference data with withholding rates';

-- ==============================================================================
-- TAX TREATY RATES
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.tax_treaty_rates (
    treaty_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    investor_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    income_type VARCHAR(20) NOT NULL CHECK (income_type IN ('DIVIDEND', 'INTEREST', 'ROYALTY', 'CAPITAL_GAIN')),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    standard_rate DECIMAL(5,3) NOT NULL,
    treaty_rate DECIMAL(5,3) NOT NULL,
    beneficial_owner_required BOOLEAN NOT NULL DEFAULT true,
    documentation_codes TEXT[],
    effective_date DATE NOT NULL,
    expiry_date DATE,
    treaty_reference VARCHAR(100),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(source_jurisdiction_id, investor_jurisdiction_id, income_type, instrument_class_id)
);

CREATE INDEX IF NOT EXISTS idx_tax_treaty_source ON custody.tax_treaty_rates(source_jurisdiction_id);
CREATE INDEX IF NOT EXISTS idx_tax_treaty_investor ON custody.tax_treaty_rates(investor_jurisdiction_id);

COMMENT ON TABLE custody.tax_treaty_rates IS 'Bilateral tax treaty rates between jurisdictions';

-- ==============================================================================
-- CBU TAX STATUS
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_tax_status (
    status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    tax_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    investor_type VARCHAR(20) NOT NULL CHECK (investor_type IN ('PENSION', 'SOVEREIGN', 'CHARITY', 'CORPORATE', 'INDIVIDUAL', 'FUND')),
    tax_exempt BOOLEAN NOT NULL DEFAULT false,
    exempt_reason TEXT,
    documentation_status VARCHAR(20) DEFAULT 'PENDING' CHECK (documentation_status IN ('PENDING', 'SUBMITTED', 'VALIDATED', 'EXPIRED')),
    documentation_expiry DATE,
    applicable_treaty_rate DECIMAL(5,3),
    qualified_intermediary BOOLEAN NOT NULL DEFAULT false,
    qi_ein VARCHAR(20),
    fatca_status VARCHAR(20) CHECK (fatca_status IN ('EXEMPT', 'PARTICIPATING', 'NON_PARTICIPATING')),
    crs_status VARCHAR(20),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cbu_id, tax_jurisdiction_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_tax_status_cbu ON custody.cbu_tax_status(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_tax_status_jurisdiction ON custody.cbu_tax_status(tax_jurisdiction_id);

COMMENT ON TABLE custody.cbu_tax_status IS 'CBU tax status per jurisdiction';

-- ==============================================================================
-- TAX RECLAIM CONFIGURATION
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_tax_reclaim_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    source_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    reclaim_method VARCHAR(20) NOT NULL CHECK (reclaim_method IN ('AUTOMATIC', 'MANUAL', 'OUTSOURCED', 'NO_RECLAIM')),
    service_provider_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    minimum_reclaim_amount DECIMAL(15,2),
    minimum_reclaim_currency VARCHAR(3),
    batch_frequency VARCHAR(20) CHECK (batch_frequency IN ('IMMEDIATE', 'WEEKLY', 'MONTHLY', 'QUARTERLY')),
    expected_recovery_days INTEGER,
    fee_structure JSONB,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cbu_id, source_jurisdiction_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_tax_reclaim_cbu ON custody.cbu_tax_reclaim_config(cbu_id);

COMMENT ON TABLE custody.cbu_tax_reclaim_config IS 'Tax reclaim processing configuration per CBU/jurisdiction';

-- ==============================================================================
-- TAX REPORTING OBLIGATIONS
-- ==============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_tax_reporting (
    reporting_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    reporting_regime VARCHAR(20) NOT NULL CHECK (reporting_regime IN ('FATCA', 'CRS', 'DAC6', 'UK_CDOT', 'QI', '871M')),
    reporting_jurisdiction_id UUID NOT NULL REFERENCES custody.tax_jurisdictions(jurisdiction_id),
    reporting_status VARCHAR(20) DEFAULT 'REQUIRED' CHECK (reporting_status IN ('REQUIRED', 'EXEMPT', 'PARTICIPATING', 'PENDING')),
    giin VARCHAR(30),
    registration_date DATE,
    reporting_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    sponsor_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    notes TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cbu_id, reporting_regime, reporting_jurisdiction_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_tax_reporting_cbu ON custody.cbu_tax_reporting(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_tax_reporting_regime ON custody.cbu_tax_reporting(reporting_regime);

COMMENT ON TABLE custody.cbu_tax_reporting IS 'Tax reporting obligations (FATCA, CRS, etc.) per CBU';

-- ==============================================================================
-- UPDATE TRIGGERS FOR updated_at
-- ==============================================================================

CREATE OR REPLACE FUNCTION custody.update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    tbl TEXT;
BEGIN
    FOR tbl IN SELECT unnest(ARRAY[
        'settlement_locations',
        'cbu_settlement_chains',
        'settlement_chain_hops',
        'cbu_settlement_location_preferences',
        'cbu_cross_border_config',
        'tax_jurisdictions',
        'tax_treaty_rates',
        'cbu_tax_status',
        'cbu_tax_reclaim_config',
        'cbu_tax_reporting'
    ])
    LOOP
        EXECUTE format('
            DROP TRIGGER IF EXISTS update_%s_updated_at ON custody.%s;
            CREATE TRIGGER update_%s_updated_at
                BEFORE UPDATE ON custody.%s
                FOR EACH ROW
                EXECUTE FUNCTION custody.update_updated_at_column();
        ', tbl, tbl, tbl, tbl);
    END LOOP;
END;
$$;

-- ==============================================================================
-- GRANT PERMISSIONS (adjust as needed for your user)
-- ==============================================================================

-- GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA custody TO your_user;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA custody TO your_user;

SELECT 'Phase 3 Settlement Chain and Tax Configuration tables created successfully' AS status;
