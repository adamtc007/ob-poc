-- Migration: 023_sessions_persistence.sql
-- Purpose: Simplified session persistence for CBU session state
--
-- Design: Memory is truth, DB is backup
-- - All mutations happen in-memory, instant
-- - DB saves are fire-and-forget background tasks
-- - Load from DB only at startup, with timeout fallback
-- - If DB fails, session just won't survive refresh

-- =============================================================================
-- SESSIONS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Optional user association (NULL for anonymous sessions)
    user_id UUID,

    -- Optional friendly name
    name TEXT,

    -- Core state: the set of loaded CBU IDs
    cbu_ids UUID[] NOT NULL DEFAULT '{}',

    -- Undo stack: array of previous states (each state is array of UUIDs)
    -- Stored as JSONB for flexible serialization
    history JSONB NOT NULL DEFAULT '[]',

    -- Redo stack: array of future states (cleared on new action)
    future JSONB NOT NULL DEFAULT '[]',

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Auto-expiry: sessions expire after 7 days of inactivity
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '7 days'
);

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Find sessions by user
CREATE INDEX IF NOT EXISTS idx_sessions_user
    ON "ob-poc".sessions(user_id)
    WHERE user_id IS NOT NULL;

-- Cleanup expired sessions
CREATE INDEX IF NOT EXISTS idx_sessions_expires
    ON "ob-poc".sessions(expires_at);

-- Find recent sessions
CREATE INDEX IF NOT EXISTS idx_sessions_updated
    ON "ob-poc".sessions(updated_at DESC);

-- =============================================================================
-- AUTO-EXTEND EXPIRY ON ACTIVITY
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".extend_session_expiry()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.expires_at = NOW() + INTERVAL '7 days';
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS session_activity ON "ob-poc".sessions;

CREATE TRIGGER session_activity
    BEFORE UPDATE ON "ob-poc".sessions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".extend_session_expiry();

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE "ob-poc".sessions IS
'Simplified CBU session persistence. Memory is truth, DB is backup.';

COMMENT ON COLUMN "ob-poc".sessions.cbu_ids IS
'Set of CBU IDs currently loaded in this session';

COMMENT ON COLUMN "ob-poc".sessions.history IS
'Undo stack: JSON array of previous states, each state is array of UUID strings';

COMMENT ON COLUMN "ob-poc".sessions.future IS
'Redo stack: JSON array of future states, cleared on new action';

COMMENT ON COLUMN "ob-poc".sessions.expires_at IS
'Session expires 7 days after last activity. Auto-extended on update.';
