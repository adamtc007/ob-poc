-- Migration 089: Compiled Runbooks — Append-Only Storage (INV-9)
--
-- Implements the storage layer for compiled runbooks as specified in
-- docs/MACRO_EXPANSION_COMPILER_PHASE_PAPER_v0_6.md §9 (Storage).
--
-- Design principles:
--   - compiled_runbooks is INSERT-only (no UPDATE, no DELETE) — enforced by trigger
--   - Status changes tracked via compiled_runbook_events (also INSERT-only)
--   - Content-addressed IDs: compiled_runbook_id = SHA-256(bincode(steps) ++ bincode(envelope))[..16]
--   - Dedup on insert via ON CONFLICT DO NOTHING (idempotent)
--   - Session version mapping via existing repl_sessions_v2.version column

-- ---------------------------------------------------------------------------
-- Immutable compiled runbook artefacts (INV-9: no UPDATE, no DELETE)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS "ob-poc".compiled_runbooks (
    compiled_runbook_id UUID PRIMARY KEY,
    session_id          UUID NOT NULL,
    version             BIGINT NOT NULL,
    steps               JSONB NOT NULL,
    envelope            JSONB NOT NULL,
    canonical_hash      BYTEA NOT NULL,  -- Full SHA-256 (32 bytes) for integrity verification
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (session_id, version)
);

COMMENT ON TABLE "ob-poc".compiled_runbooks IS
    'Immutable compiled runbook artefacts. INSERT-only — UPDATE and DELETE prohibited by trigger (INV-9).';

COMMENT ON COLUMN "ob-poc".compiled_runbooks.compiled_runbook_id IS
    'Content-addressed UUID: SHA-256(bincode(steps) ++ bincode(envelope)) truncated to 128 bits (INV-2).';

COMMENT ON COLUMN "ob-poc".compiled_runbooks.canonical_hash IS
    'Full SHA-256 (32 bytes) of canonical bincode representation. Used for integrity verification on read.';

-- ---------------------------------------------------------------------------
-- Status changes and lifecycle events (INV-9: INSERT-only event log)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS "ob-poc".compiled_runbook_events (
    event_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    compiled_runbook_id UUID NOT NULL REFERENCES "ob-poc".compiled_runbooks(compiled_runbook_id),
    event_type          VARCHAR(30) NOT NULL,
    old_status          VARCHAR(20),
    new_status          VARCHAR(20),
    detail              JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT valid_event_type CHECK (
        event_type IN (
            'status_change',
            'lock_acquired',
            'lock_released',
            'lock_contention',
            'step_completed',
            'step_failed'
        )
    )
);

COMMENT ON TABLE "ob-poc".compiled_runbook_events IS
    'Append-only event log for compiled runbook lifecycle. Status is derived from latest status_change event (INV-9).';

-- ---------------------------------------------------------------------------
-- Immutability trigger — prevent UPDATE and DELETE on compiled_runbooks (INV-9)
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION "ob-poc".compiled_runbooks_immutable()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'compiled_runbooks is append-only: UPDATE and DELETE are prohibited (INV-9)';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_compiled_runbooks_immutable ON "ob-poc".compiled_runbooks;

CREATE TRIGGER trg_compiled_runbooks_immutable
    BEFORE UPDATE OR DELETE ON "ob-poc".compiled_runbooks
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".compiled_runbooks_immutable();

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------

CREATE INDEX IF NOT EXISTS idx_compiled_runbooks_session
    ON "ob-poc".compiled_runbooks(session_id, version DESC);

CREATE INDEX IF NOT EXISTS idx_compiled_runbook_events_runbook
    ON "ob-poc".compiled_runbook_events(compiled_runbook_id, created_at);

-- Latest status lookup: most recent status_change event per runbook
CREATE INDEX IF NOT EXISTS idx_compiled_runbook_events_status
    ON "ob-poc".compiled_runbook_events(compiled_runbook_id, created_at DESC)
    WHERE event_type = 'status_change';
