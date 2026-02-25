-- Migration 092: Semantic Registry outbox table for reliable event dispatch.
--
-- The outbox pattern ensures that snapshot publishes and their corresponding
-- event notifications are committed atomically. The outbox poller claims
-- events and dispatches them to projections/subscribers.

CREATE TABLE sem_reg.outbox_events (
    outbox_seq        BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id          UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    event_type        TEXT NOT NULL,                     -- 'snapshot_set_published'
    aggregate_version BIGINT,
    snapshot_set_id   UUID NOT NULL,
    correlation_id    UUID NOT NULL,
    payload           JSONB NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    claimed_at        TIMESTAMPTZ,
    claimer_id        TEXT,
    claim_timeout_at  TIMESTAMPTZ,
    processed_at      TIMESTAMPTZ,
    failed_at         TIMESTAMPTZ,
    attempt_count     INT NOT NULL DEFAULT 0,
    last_error        TEXT
);

-- Index for efficient claim queries: find the next unclaimed event.
-- The claim_timeout_at expiry check uses now() which is not IMMUTABLE,
-- so it cannot appear in a partial index predicate — handled in query WHERE instead.
CREATE INDEX idx_outbox_claimable ON sem_reg.outbox_events (outbox_seq)
    WHERE processed_at IS NULL AND failed_at IS NULL;
