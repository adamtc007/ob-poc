-- Migration 030: Fund Vehicle Taxonomy
--
-- Purpose: Add explicit metadata for fund structures to support:
-- - FoF/umbrella/master pool representation (Allianz-style structures)
-- - Compartment/sleeve modeling without inventing fake legal entities
-- - Instrument type classification for holdings
--
-- This enables proper representation of multi-tier fund structures without
-- misclassifying pooled vehicles as UBOs.

-- =============================================================================
-- FUND VEHICLES TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.fund_vehicles (
    fund_entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- Vehicle classification
    vehicle_type VARCHAR(30) NOT NULL,

    -- Umbrella relationship (NULL if standalone fund)
    umbrella_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,
    is_umbrella BOOLEAN NOT NULL DEFAULT false,

    -- Domicile and management
    domicile_country CHAR(2) NULL,
    manager_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,

    -- Flexible metadata
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by VARCHAR(100) NULL,

    -- Vehicle type enum (expanded for broader use)
    CONSTRAINT chk_vehicle_type CHECK (vehicle_type IN (
        'SCSP',           -- Luxembourg SCSp (Société en Commandite Spéciale)
        'SICAV_RAIF',     -- Luxembourg SICAV-RAIF (Reserved Alternative Investment Fund)
        'SICAV_SIF',      -- Luxembourg SICAV-SIF (Specialized Investment Fund)
        'SIF',            -- Luxembourg SIF (standalone)
        'SICAV_UCITS',    -- UCITS umbrella fund
        'FCP',            -- Fonds Commun de Placement
        'LLC',            -- US LLC
        'LP',             -- Limited Partnership (generic)
        'TRUST',          -- Unit trust structure
        'OEIC',           -- UK Open-Ended Investment Company
        'ETF',            -- Exchange-traded fund
        'REIT',           -- Real Estate Investment Trust
        'BDC',            -- Business Development Company
        'OTHER'
    ))
);

COMMENT ON TABLE kyc.fund_vehicles IS
'Fund vehicle metadata for fund structures (FoF/umbrella/master pool). Links to entities table.';

COMMENT ON COLUMN kyc.fund_vehicles.vehicle_type IS
'Luxembourg: SCSP, SICAV_RAIF, SICAV_SIF, SIF, FCP. Generic: LP, LLC, TRUST, OEIC, ETF, REIT, BDC, OTHER';

COMMENT ON COLUMN kyc.fund_vehicles.umbrella_entity_id IS
'Parent umbrella fund (if this is a sub-fund/compartment). NULL for standalone funds.';

COMMENT ON COLUMN kyc.fund_vehicles.is_umbrella IS
'True if this fund is an umbrella containing compartments/sub-funds';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_fund_vehicles_umbrella
    ON kyc.fund_vehicles(umbrella_entity_id) WHERE umbrella_entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_fund_vehicles_manager
    ON kyc.fund_vehicles(manager_entity_id) WHERE manager_entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_fund_vehicles_type
    ON kyc.fund_vehicles(vehicle_type);

-- =============================================================================
-- FUND COMPARTMENTS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.fund_compartments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Parent umbrella fund
    umbrella_fund_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- Compartment identification
    compartment_code TEXT NOT NULL,
    compartment_name TEXT NULL,

    -- Optional: link to entity if compartment has separate legal identity
    compartment_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,

    -- Flexible metadata
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Unique compartment per umbrella
    CONSTRAINT uq_compartment UNIQUE (umbrella_fund_entity_id, compartment_code)
);

COMMENT ON TABLE kyc.fund_compartments IS
'Compartments/sleeves under umbrella funds. May or may not have separate legal identity.';

COMMENT ON COLUMN kyc.fund_compartments.compartment_entity_id IS
'Optional link to entity if compartment has separate legal identity (e.g., separate LEI)';

-- Index
CREATE INDEX IF NOT EXISTS idx_fund_compartments_umbrella
    ON kyc.fund_compartments(umbrella_fund_entity_id);

-- =============================================================================
-- EXTEND SHARE_CLASSES WITH INSTRUMENT_TYPE
-- =============================================================================

ALTER TABLE kyc.share_classes
ADD COLUMN IF NOT EXISTS instrument_type VARCHAR(30) DEFAULT 'SHARES';

COMMENT ON COLUMN kyc.share_classes.instrument_type IS
'UNITS, SHARES, LP_INTEREST, PARTNERSHIP_INTEREST, NOMINEE_POSITION, TRACKING_SHARES, CARRIED_INTEREST';

-- Add compartment link to share classes
ALTER TABLE kyc.share_classes
ADD COLUMN IF NOT EXISTS compartment_id UUID NULL REFERENCES kyc.fund_compartments(id) ON DELETE SET NULL;

COMMENT ON COLUMN kyc.share_classes.compartment_id IS
'Optional link to fund compartment (for umbrella funds with compartment-specific share classes)';

-- Index
CREATE INDEX IF NOT EXISTS idx_share_classes_compartment
    ON kyc.share_classes(compartment_id) WHERE compartment_id IS NOT NULL;

-- =============================================================================
-- VIEW: Fund vehicle summary
-- =============================================================================

CREATE OR REPLACE VIEW kyc.v_fund_vehicle_summary AS
SELECT
    fv.fund_entity_id,
    e.name AS fund_name,
    fv.vehicle_type,
    fv.is_umbrella,
    fv.domicile_country,
    umbrella.name AS umbrella_name,
    manager.name AS manager_name,
    (SELECT COUNT(*) FROM kyc.fund_compartments fc WHERE fc.umbrella_fund_entity_id = fv.fund_entity_id) AS compartment_count,
    (SELECT COUNT(*) FROM kyc.share_classes sc WHERE sc.entity_id = fv.fund_entity_id) AS share_class_count,
    fv.meta,
    fv.created_at
FROM kyc.fund_vehicles fv
JOIN "ob-poc".entities e ON fv.fund_entity_id = e.entity_id
LEFT JOIN "ob-poc".entities umbrella ON fv.umbrella_entity_id = umbrella.entity_id
LEFT JOIN "ob-poc".entities manager ON fv.manager_entity_id = manager.entity_id;

COMMENT ON VIEW kyc.v_fund_vehicle_summary IS
'Fund vehicles with resolved entity names and aggregate counts';

-- =============================================================================
-- UPDATE TRIGGER for updated_at
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.update_fund_vehicle_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_fund_vehicle_updated ON kyc.fund_vehicles;
CREATE TRIGGER trg_fund_vehicle_updated
    BEFORE UPDATE ON kyc.fund_vehicles
    FOR EACH ROW
    EXECUTE FUNCTION kyc.update_fund_vehicle_timestamp();

DROP TRIGGER IF EXISTS trg_fund_compartment_updated ON kyc.fund_compartments;
CREATE TRIGGER trg_fund_compartment_updated
    BEFORE UPDATE ON kyc.fund_compartments
    FOR EACH ROW
    EXECUTE FUNCTION kyc.update_fund_vehicle_timestamp();
