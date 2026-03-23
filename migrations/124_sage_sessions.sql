-- Migration 124: Sage session context for SemOS-scoped verb resolution
-- Tracks which client group, constellation, and entity are in focus for the Sage pipeline.

CREATE TABLE IF NOT EXISTS "ob-poc".sage_sessions (
    session_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_group_id  UUID,
    constellation_id TEXT,
    active_entity_id UUID,
    active_domain    TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_sage_sessions_client_group
    ON "ob-poc".sage_sessions(client_group_id) WHERE client_group_id IS NOT NULL;
