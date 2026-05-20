-- v0.6 §8.1 — federated DSL bus outbox.
--
-- Each domain runs this migration on its own Postgres. Rows are sent
-- by the outbox sender loop (§8.5) and removed from contention by the
-- `idx_outbox_pending` partial index, which only retains rows that are
-- still pending dispatch.
CREATE TABLE outbox (
    id                UUID PRIMARY KEY,
    target_domain     TEXT NOT NULL,
    target_endpoint   TEXT NOT NULL
                      CHECK (target_endpoint IN ('invocation', 'result')),
    payload           BYTEA NOT NULL,
    idempotency_key   UUID NOT NULL,
    execution_id      UUID,
    callout_id        UUID,
    status            TEXT NOT NULL DEFAULT 'pending'
                      CHECK (status IN ('pending', 'submitted', 'retrying', 'failed')),
    attempt_count     INT NOT NULL DEFAULT 0,
    next_attempt_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error        TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    submitted_at      TIMESTAMPTZ,
    UNIQUE (idempotency_key, target_endpoint)
);

CREATE INDEX idx_outbox_pending
    ON outbox(next_attempt_at) WHERE status = 'pending';
