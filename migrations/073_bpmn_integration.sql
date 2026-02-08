-- Migration 073: BPMN-Lite Integration Stores
--
-- Three tables supporting workflow dispatch, job worker dedupe,
-- and parked token resolution for the bpmn-lite gRPC integration.
--
-- Architecture:
--   bpmn_correlations  — links BPMN process instances to ob-poc sessions
--   bpmn_job_frames    — dedupe for job worker processing (at-least-once → exactly-once)
--   bpmn_parked_tokens — ob-poc entries waiting for BPMN signals

-- ─── Correlation Store ───────────────────────────────────────────────────────
-- Links a BPMN process instance to an ob-poc REPL session/runbook entry.
-- One correlation per process instance. The runbook entry is the parked
-- DSL statement that triggered the orchestration.

CREATE TABLE IF NOT EXISTS "ob-poc".bpmn_correlations (
    correlation_id       UUID PRIMARY KEY,
    process_instance_id  UUID NOT NULL,
    session_id           UUID NOT NULL,
    runbook_id           UUID NOT NULL,
    entry_id             UUID NOT NULL,
    process_key          TEXT NOT NULL,
    domain_payload_hash  BYTEA NOT NULL,
    status               TEXT NOT NULL DEFAULT 'active'
                         CHECK (status IN ('active', 'completed', 'failed', 'cancelled')),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at         TIMESTAMPTZ
);

-- Process instance lookup (unique — one correlation per instance)
CREATE UNIQUE INDEX IF NOT EXISTS idx_bpmn_corr_process_instance
    ON "ob-poc".bpmn_correlations(process_instance_id);

-- Session+entry lookup for resolving from REPL side
CREATE INDEX IF NOT EXISTS idx_bpmn_corr_session_entry
    ON "ob-poc".bpmn_correlations(session_id, entry_id);

-- Active correlations for monitoring
CREATE INDEX IF NOT EXISTS idx_bpmn_corr_active
    ON "ob-poc".bpmn_correlations(status) WHERE status = 'active';


-- ─── Job Frame Store ─────────────────────────────────────────────────────────
-- Tracks job activation/completion for dedupe in the job worker.
-- The job_key is the idempotency key provided by bpmn-lite.
-- On redelivery, the worker checks: if completed, return cached result.

CREATE TABLE IF NOT EXISTS "ob-poc".bpmn_job_frames (
    job_key              TEXT PRIMARY KEY,
    process_instance_id  UUID NOT NULL,
    task_type            TEXT NOT NULL,
    worker_id            TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'active'
                         CHECK (status IN ('active', 'completed', 'failed')),
    activated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at         TIMESTAMPTZ,
    attempts             INTEGER NOT NULL DEFAULT 0
);

-- Active jobs for monitoring/cleanup
CREATE INDEX IF NOT EXISTS idx_bpmn_job_frames_active
    ON "ob-poc".bpmn_job_frames(status) WHERE status = 'active';

-- Process instance lookup for listing all jobs in a workflow
CREATE INDEX IF NOT EXISTS idx_bpmn_job_frames_instance
    ON "ob-poc".bpmn_job_frames(process_instance_id);


-- ─── Parked Token Store ──────────────────────────────────────────────────────
-- Represents an ob-poc REPL entry waiting for a BPMN signal.
-- Created when EventBridge receives wait events (WaitMsg, WaitTimer, UserTask).
-- Resolved when the corresponding signal arrives.

CREATE TABLE IF NOT EXISTS "ob-poc".bpmn_parked_tokens (
    token_id             UUID PRIMARY KEY,
    correlation_key      TEXT NOT NULL UNIQUE,
    session_id           UUID NOT NULL,
    entry_id             UUID NOT NULL,
    process_instance_id  UUID NOT NULL,
    expected_signal      TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'waiting'
                         CHECK (status IN ('waiting', 'resolved', 'timed_out', 'cancelled')),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at          TIMESTAMPTZ,
    result_payload       JSONB
);

-- Correlation key lookup for resolving tokens (only waiting ones matter)
CREATE INDEX IF NOT EXISTS idx_bpmn_parked_tokens_waiting
    ON "ob-poc".bpmn_parked_tokens(correlation_key) WHERE status = 'waiting';

-- Process instance lookup for resolving all tokens on completion
CREATE INDEX IF NOT EXISTS idx_bpmn_parked_tokens_process
    ON "ob-poc".bpmn_parked_tokens(process_instance_id);

-- Session lookup for listing parked tokens in a session
CREATE INDEX IF NOT EXISTS idx_bpmn_parked_tokens_session
    ON "ob-poc".bpmn_parked_tokens(session_id) WHERE status = 'waiting';
