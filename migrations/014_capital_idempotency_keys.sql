-- Migration: 014_capital_idempotency_keys.sql
-- Purpose: Add idempotency keys for transactional safety on complex capital operations
-- Date: 2026-01-10

-- =============================================================================
-- IDEMPOTENCY KEYS
-- Prevent duplicate operations from retries (network failures, client retries)
-- =============================================================================

-- Add idempotency_key to issuance_events (for splits, consolidations)
ALTER TABLE kyc.issuance_events
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(100) UNIQUE;

-- Add idempotency_key to dilution_exercise_events (for option exercises)
ALTER TABLE kyc.dilution_exercise_events
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(100) UNIQUE;

-- Create index for fast idempotency lookups
CREATE INDEX IF NOT EXISTS idx_issuance_idempotency
    ON kyc.issuance_events(idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_exercise_idempotency
    ON kyc.dilution_exercise_events(idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- =============================================================================
-- HELPER FUNCTION: Convert UUID to advisory lock ID
-- PostgreSQL advisory locks use bigint, so we hash the UUID
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.uuid_to_lock_id(p_uuid UUID)
RETURNS BIGINT AS $$
BEGIN
    -- Use hashtext on the UUID string to get a stable bigint
    RETURN ('x' || substr(md5(p_uuid::text), 1, 16))::bit(64)::bigint;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION kyc.uuid_to_lock_id IS
'Convert UUID to bigint for use with pg_advisory_xact_lock. Uses MD5 hash for stability.';
