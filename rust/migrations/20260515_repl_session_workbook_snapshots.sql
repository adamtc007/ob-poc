-- Append-only REPL workbook snapshots.
--
-- Normal REPL persistence must not take an advisory session lock. The mutable
-- repl_sessions_v2 row remains the compatibility/current-session header, while
-- each checkpoint appends an immutable workbook snapshot keyed by UUIDv7.

ALTER TABLE "ob-poc".repl_sessions_v2
    ADD COLUMN IF NOT EXISTS current_snapshot_id uuid;

CREATE TABLE IF NOT EXISTS "ob-poc".repl_session_workbook_snapshots (
    session_id uuid NOT NULL REFERENCES "ob-poc".repl_sessions_v2(session_id) ON DELETE CASCADE,
    snapshot_id uuid NOT NULL,
    session_version bigint NOT NULL,
    state jsonb NOT NULL,
    client_context jsonb,
    journey_context jsonb,
    runbook jsonb NOT NULL,
    messages jsonb NOT NULL DEFAULT '[]'::jsonb,
    extended_state jsonb NOT NULL DEFAULT '{}'::jsonb,
    workbook jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    session_created_at timestamp with time zone NOT NULL,
    session_last_active_at timestamp with time zone NOT NULL,
    PRIMARY KEY (session_id, snapshot_id)
);

CREATE INDEX IF NOT EXISTS idx_repl_session_workbook_snapshots_latest
    ON "ob-poc".repl_session_workbook_snapshots (session_id, snapshot_id DESC);

CREATE INDEX IF NOT EXISTS idx_repl_session_workbook_snapshots_created
    ON "ob-poc".repl_session_workbook_snapshots (session_id, created_at DESC);
