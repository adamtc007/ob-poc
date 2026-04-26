-- Phase 5e — public.outbox table (per three-plane v0.3 spec §10.4).
--
-- Post-commit effect queue. Drafts are inserted INSIDE the stage-8
-- runtime transaction (so they commit atomically with the writes that
-- caused them) and consumed AFTER stage 9a commits by a separate
-- `outbox_drainer` task in ob-poc.
--
-- The table is at-least-once: consumers MUST be idempotent against
-- the (idempotency_key, effect_kind) unique constraint. The drainer
-- recycles `processing` rows whose `claimed_at` is older than the
-- worker timeout (handled in the drainer code, not the schema).
--
-- Status transitions (drainer-managed):
--   pending           -> processing -> done | failed_retryable | failed_terminal
--   failed_retryable  -> processing (next claim cycle)
--   failed_terminal   -> (alert; manual intervention)
--
-- See spec §10.4 for the full lifecycle model.

CREATE TABLE IF NOT EXISTS public.outbox (
    id                uuid PRIMARY KEY,
    trace_id          uuid NOT NULL,
    envelope_version  smallint NOT NULL,
    effect_kind       text NOT NULL,
    payload           jsonb NOT NULL,
    idempotency_key   text NOT NULL,
    status            text NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','processing','done','failed_retryable','failed_terminal')),
    attempts          integer NOT NULL DEFAULT 0,
    claimed_by        text,
    claimed_at        timestamptz,
    created_at        timestamptz NOT NULL DEFAULT now(),
    processed_at      timestamptz,
    last_error        text,
    UNIQUE (idempotency_key, effect_kind)
);

-- Drainer's primary scan path: pending or retry-eligible rows ordered by age.
CREATE INDEX IF NOT EXISTS outbox_pending_idx
    ON public.outbox (status, created_at)
    WHERE status IN ('pending','failed_retryable');

-- Worker-recovery scan path: stale claims that need recycling.
CREATE INDEX IF NOT EXISTS outbox_processing_claimed_at_idx
    ON public.outbox (claimed_at)
    WHERE status = 'processing';
