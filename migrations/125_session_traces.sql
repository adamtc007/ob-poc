-- Migration 125: Session trace infrastructure — append-only log of session mutations.
-- Supports replay, compliance auditing, and regression testing.

CREATE TABLE IF NOT EXISTS "ob-poc".session_traces (
    session_id      UUID NOT NULL,
    sequence        BIGINT NOT NULL,
    agent_mode      TEXT NOT NULL,
    op              JSONB NOT NULL,
    stack_snapshot   JSONB,
    hydrated_snap   JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (session_id, sequence)
);

COMMENT ON TABLE "ob-poc".session_traces IS 'Append-only session mutation trace for replay and audit';
