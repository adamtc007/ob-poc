-- Migration 076: Pending dispatch queue for BPMN resilience
--
-- When the bpmn-lite gRPC service is unavailable at dispatch time,
-- the WorkflowDispatcher persists the dispatch request here. A
-- background PendingDispatchWorker retries periodically until the
-- service recovers. This is the "local durable queue" layer in the
-- stack: ob-poc verb -> [pending queue] -> BPMN gRPC -> correlation -> parked token -> signal -> resume.

CREATE TABLE IF NOT EXISTS "ob-poc".bpmn_pending_dispatches (
    dispatch_id            UUID PRIMARY KEY,
    -- SHA-256 of canonical domain payload. Idempotency key — prevents
    -- duplicate pending dispatches for the same payload.
    payload_hash           BYTEA NOT NULL,
    verb_fqn               TEXT NOT NULL,
    process_key            TEXT NOT NULL,
    bytecode_version       BYTEA NOT NULL DEFAULT '',
    -- Canonical JSON domain payload (passed to StartProcess).
    domain_payload         TEXT NOT NULL,
    -- Original DSL string (audit trail).
    dsl_source             TEXT NOT NULL,
    entry_id               UUID NOT NULL,
    runbook_id             UUID NOT NULL,
    -- Pre-generated at dispatch time, stable across retries.
    correlation_id         UUID NOT NULL,
    correlation_key        TEXT NOT NULL,
    -- Business-level correlation key (e.g., case_id).
    domain_correlation_key TEXT,
    status                 TEXT NOT NULL DEFAULT 'pending'
                           CHECK (status IN ('pending', 'dispatched', 'failed_permanent')),
    attempts               INTEGER NOT NULL DEFAULT 0,
    last_error             TEXT,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_attempted_at      TIMESTAMPTZ,
    dispatched_at          TIMESTAMPTZ
);

-- Worker scans pending rows ordered by age, with backoff via last_attempted_at.
CREATE INDEX IF NOT EXISTS idx_bpmn_pending_dispatches_pending
    ON "ob-poc".bpmn_pending_dispatches(status, last_attempted_at)
    WHERE status = 'pending';

-- Prevent duplicate pending dispatches for the same payload.
CREATE UNIQUE INDEX IF NOT EXISTS idx_bpmn_pending_dispatches_hash
    ON "ob-poc".bpmn_pending_dispatches(payload_hash)
    WHERE status = 'pending';
