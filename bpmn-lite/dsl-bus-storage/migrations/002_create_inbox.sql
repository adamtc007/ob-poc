-- v0.6 §8.2 — federated DSL bus inbox.
--
-- Each domain runs this migration on its own Postgres. The receiver-side
-- gRPC handlers consult `idempotency_key` (PK) to detect duplicates so
-- the bus surface is exactly-once at the semantic layer even though
-- transport allows retries.
CREATE TABLE inbox (
    idempotency_key   UUID PRIMARY KEY,
    source_domain     TEXT NOT NULL,
    endpoint          TEXT NOT NULL
                      CHECK (endpoint IN ('invocation', 'result')),
    execution_id      UUID,
    received_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at      TIMESTAMPTZ,
    status            TEXT NOT NULL DEFAULT 'received'
                      CHECK (status IN ('received', 'processed')),
    payload           BYTEA
);

CREATE INDEX idx_inbox_source ON inbox(source_domain, received_at);
