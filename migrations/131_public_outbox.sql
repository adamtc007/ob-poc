-- Migration 131: public.outbox — post-commit effect queue for the
-- Agentic Sequencer (three-plane architecture refactor, v0.3 §10.7).
--
-- Decision D10: this is a SEPARATE table from `sem_reg.outbox_events`.
-- Rationale:
--   - sem_reg.outbox_events is scoped to snapshot-publish notifications
--     within the Semantic Registry plane.
--   - public.outbox is scoped to Sequencer-emitted post-commit effects
--     (narration, UI push, constellation broadcast, external notify,
--     maintenance spawn) from verb executions in the data plane.
--   - Distinct drainer workers consume each; mixing them would blur the
--     two planes' effect catalogues.
--
-- Status: scaffolded, NOT YET WIRED to production write paths. Sequencer
-- stage-8 transaction integration happens in Phase 5e per the
-- three-plane-architecture-implementation-plan-v0.1.md. Phase 0d (this
-- migration) provides the schema so the drainer trait, consumers, and
-- tests can be developed against a concrete shape.
--
-- Idempotency: `UNIQUE (idempotency_key, effect_kind)` — consumers MUST
-- dedupe against prior processed_at/failed_at records.

CREATE TABLE IF NOT EXISTS public.outbox (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- trace_id threaded from the originating envelope (ob-poc-types::TraceId).
    trace_id          UUID NOT NULL,
    -- envelope version that produced this draft — enables drainers to detect
    -- version drift (v0.3 §10.3 envelope_version, R11).
    envelope_version  SMALLINT NOT NULL,
    -- Matches ob-poc-types::OutboxEffectKind snake_case variants:
    --   narrate | ui_push | constellation_broadcast | external_notify | maintenance_spawn
    effect_kind       TEXT NOT NULL,
    payload           JSONB NOT NULL,
    -- Format convention (documented in ob-poc-types::IdempotencyKey):
    --   <effect_kind>:<trace_id>:<sub_key>
    -- Drainers MUST dedupe on this. UNIQUE constraint below enforces.
    idempotency_key   TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'pending'
                      CHECK (status IN ('pending','processing','done','failed_retryable','failed_terminal')),
    attempts          INTEGER NOT NULL DEFAULT 0,
    claimed_by        TEXT,
    claimed_at        TIMESTAMPTZ,
    -- Reclaim window: if claimed_at is older than claim_timeout_at, a
    -- different worker can steal the row (worker-crash recovery).
    claim_timeout_at  TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at      TIMESTAMPTZ,
    failed_at         TIMESTAMPTZ,
    last_error        TEXT,

    -- Idempotency guard: each (key, kind) pair lands at most once.
    -- The drainer uses this to short-circuit re-delivery attempts.
    UNIQUE (idempotency_key, effect_kind)
);

-- Partial index for the drainer's claim query: fetch the next unclaimed
-- or retryable row. `now()` is not IMMUTABLE so it cannot appear in a
-- partial index predicate — the drainer adds `claimed_at IS NULL
-- OR claim_timeout_at < now()` to the WHERE clause instead.
CREATE INDEX IF NOT EXISTS idx_outbox_pending
    ON public.outbox (created_at)
    WHERE status IN ('pending','failed_retryable');

CREATE INDEX IF NOT EXISTS idx_outbox_trace
    ON public.outbox (trace_id);

CREATE INDEX IF NOT EXISTS idx_outbox_claim
    ON public.outbox (claimed_at)
    WHERE status = 'processing';

COMMENT ON TABLE public.outbox IS
    'Post-commit effect queue — writes are enqueued inside the Sequencer''s stage-8 transaction and drained after stage 9a commit. See docs/todo/three-plane-architecture-v0.3.md §10.7.';

COMMENT ON COLUMN public.outbox.effect_kind IS
    'One of: narrate, ui_push, constellation_broadcast, external_notify, maintenance_spawn. Matches ob-poc-types::OutboxEffectKind.';

COMMENT ON COLUMN public.outbox.idempotency_key IS
    'Format: <effect_kind>:<trace_id>:<sub_key>. Drainers dedupe against (idempotency_key, effect_kind).';
