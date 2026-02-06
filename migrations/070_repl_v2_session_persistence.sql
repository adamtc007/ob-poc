-- Migration 070: REPL V2 Session Persistence
-- Supports durable execution and human gate parking.

CREATE TABLE IF NOT EXISTS "ob-poc".repl_sessions_v2 (
    session_id UUID PRIMARY KEY,
    state JSONB NOT NULL,
    client_context JSONB,
    journey_context JSONB,
    runbook JSONB NOT NULL,
    messages JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_active_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    park_expires_at TIMESTAMPTZ,
    version BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_repl_v2_sessions_parked
    ON "ob-poc".repl_sessions_v2 (last_active_at)
    WHERE state->>'state' = 'executing';

CREATE TABLE IF NOT EXISTS "ob-poc".repl_invocation_records (
    invocation_id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES "ob-poc".repl_sessions_v2(session_id) ON DELETE CASCADE,
    entry_id UUID NOT NULL,
    runbook_id UUID NOT NULL,
    correlation_key TEXT NOT NULL UNIQUE,
    gate_type TEXT NOT NULL CHECK (gate_type IN ('durable_task', 'human_approval')),
    task_id UUID,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'completed', 'timed_out', 'cancelled')),
    parked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    timeout_at TIMESTAMPTZ,
    resumed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_repl_invocations_active
    ON "ob-poc".repl_invocation_records(correlation_key)
    WHERE status = 'active';
