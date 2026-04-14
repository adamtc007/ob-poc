CREATE SCHEMA IF NOT EXISTS "ob-poc";

CREATE TABLE IF NOT EXISTS "ob-poc".repl_sessions_v2 (
    session_id UUID PRIMARY KEY,
    state JSONB NOT NULL,
    client_context JSONB,
    journey_context JSONB,
    runbook JSONB NOT NULL,
    messages JSONB NOT NULL,
    extended_state JSONB,
    created_at TIMESTAMPTZ NOT NULL,
    last_active_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS "ob-poc".runbook_plans (
    plan_id TEXT PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES "ob-poc".repl_sessions_v2(session_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    steps JSONB NOT NULL,
    bindings JSONB NOT NULL,
    approval JSONB,
    compiled_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS "ob-poc".session_traces (
    session_id UUID NOT NULL REFERENCES "ob-poc".repl_sessions_v2(session_id) ON DELETE CASCADE,
    sequence BIGINT NOT NULL,
    agent_mode TEXT NOT NULL,
    op JSONB NOT NULL,
    stack_snapshot JSONB,
    hydrated_snap JSONB,
    created_at TIMESTAMPTZ NOT NULL,
    verb_resolved TEXT,
    execution_result JSONB,
    PRIMARY KEY (session_id, sequence)
);
