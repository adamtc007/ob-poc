-- Migration: Create CBU Trading Profiles table
-- Purpose: Store versioned trading profile documents as single source of truth
-- The document JSONB contains: universe, investment_managers, isda_agreements,
-- settlement_config, booking_rules, standing_instructions, pricing_matrix, etc.

-- Trading profile document storage
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_trading_profiles (
    profile_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    version INTEGER NOT NULL DEFAULT 1,
    status VARCHAR(20) NOT NULL DEFAULT 'DRAFT'
        CHECK (status IN ('DRAFT', 'PENDING_REVIEW', 'ACTIVE', 'SUPERSEDED', 'ARCHIVED')),
    document JSONB NOT NULL,
    document_hash TEXT NOT NULL,  -- SHA-256 for change detection
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    activated_at TIMESTAMPTZ,
    activated_by VARCHAR(255),
    notes TEXT,
    UNIQUE(cbu_id, version)
);

-- Index for active profile lookup
CREATE INDEX IF NOT EXISTS idx_trading_profiles_cbu_active
    ON "ob-poc".cbu_trading_profiles(cbu_id, status)
    WHERE status = 'ACTIVE';

-- Index for version history queries
CREATE INDEX IF NOT EXISTS idx_trading_profiles_cbu_version
    ON "ob-poc".cbu_trading_profiles(cbu_id, version DESC);

-- Materialization audit log - tracks what was synced from document to operational tables
CREATE TABLE IF NOT EXISTS "ob-poc".trading_profile_materializations (
    materialization_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id) ON DELETE CASCADE,
    materialized_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    materialized_by VARCHAR(255),
    sections_materialized TEXT[] NOT NULL,  -- e.g., ['universe', 'ssis', 'booking_rules']
    records_created JSONB NOT NULL DEFAULT '{}',  -- { "cbu_ssi": 5, "ssi_booking_rules": 12 }
    records_updated JSONB NOT NULL DEFAULT '{}',
    records_deleted JSONB NOT NULL DEFAULT '{}',
    errors JSONB,  -- Any errors encountered during materialization
    duration_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_materializations_profile
    ON "ob-poc".trading_profile_materializations(profile_id);

-- View for current active trading profile per CBU
CREATE OR REPLACE VIEW "ob-poc".v_active_trading_profiles AS
SELECT
    tp.profile_id,
    tp.cbu_id,
    c.name AS cbu_name,
    tp.version,
    tp.document,
    tp.document_hash,
    tp.created_at,
    tp.activated_at,
    tp.activated_by
FROM "ob-poc".cbu_trading_profiles tp
JOIN "ob-poc".cbus c ON c.cbu_id = tp.cbu_id
WHERE tp.status = 'ACTIVE';

COMMENT ON TABLE "ob-poc".cbu_trading_profiles IS
'Versioned trading profile documents - single source of truth for CBU trading configuration.
Documents are materialized to operational tables (cbu_ssi, ssi_booking_rules, etc.) via the
trading-profile.materialize verb.';

COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.document IS
'JSONB document containing: universe, investment_managers, isda_agreements, settlement_config,
booking_rules, standing_instructions, pricing_matrix, valuation_config, constraints';

COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.document_hash IS
'SHA-256 hash of document for change detection and idempotency';
