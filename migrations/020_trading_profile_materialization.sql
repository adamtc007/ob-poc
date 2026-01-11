-- Migration 020: Trading Profile Materialization Audit Trail
-- ============================================================================
-- Tracks materialization events: when trading profile documents are projected
-- to operational tables (universe, SSIs, booking rules, ISDA/CSA).
-- ============================================================================

-- Materialization audit log
-- Records each time trading-profile:materialize is executed
CREATE TABLE IF NOT EXISTS "ob-poc".trading_profile_materializations (
    materialization_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id),

    -- What was materialized
    sections_materialized TEXT[] NOT NULL DEFAULT '{}',

    -- Record counts by table
    records_created JSONB NOT NULL DEFAULT '{}',
    records_updated JSONB NOT NULL DEFAULT '{}',
    records_deleted JSONB NOT NULL DEFAULT '{}',

    -- Performance tracking
    duration_ms INTEGER NOT NULL DEFAULT 0,

    -- Audit fields
    materialized_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    materialized_by TEXT,

    -- Error tracking (null = success)
    error_message TEXT,

    -- Session context (if materialized within a session)
    session_id UUID
);

-- Index for querying materializations by profile
CREATE INDEX IF NOT EXISTS idx_materializations_profile_id
    ON "ob-poc".trading_profile_materializations(profile_id);

-- Index for time-based queries
CREATE INDEX IF NOT EXISTS idx_materializations_at
    ON "ob-poc".trading_profile_materializations(materialized_at DESC);

-- Comment
COMMENT ON TABLE "ob-poc".trading_profile_materializations IS
    'Audit trail for trading-profile:materialize operations - tracks when documents are projected to operational tables';
