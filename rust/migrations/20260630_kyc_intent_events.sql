-- EOP-DD-KYCUBO-002 §2 — the first KYC/UBO durable-substrate migration.
--
-- Two tables: the authoritative append-only verb stream, and the per-subject
-- sequence allocator + fold checkpoint. Additive only — no existing table is
-- touched. Idempotent (IF NOT EXISTS) so re-application is a no-op.
--
-- AUTHORITY: the verb stream IS the system of record (K-16). Projections
-- (ubo_edges, cases, cbu_board_controller, ...) become folds over this stream
-- in later W1-proper steps; nothing in this migration makes them so yet.

-- ── The authoritative system of record (K-16). Append-only. Per-subject order (Q6). ──
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_intent_events (
    subject_root     uuid        NOT NULL,
    -- Dense, per subject_root (Q6). ALLOCATED FROM kyc_subject_streams.next_seq
    -- inside the append transaction — NEVER from a Postgres SEQUENCE/nextval().
    -- A SEQUENCE is non-transactional and would leave gaps under rollback; the
    -- gap-free guarantee (DD-002 exit criterion 2) depends on row-allocation.
    seq              bigint      NOT NULL,
    event_id         uuid        NOT NULL,
    verb_fqn         text        NOT NULL,
    -- Exact verb definition this event was written against (Q7). The fold
    -- dispatches on this via the substrate FoldRegistry (D2); an unregistered
    -- hash is a hard replay-integrity error, never a silent latest-version fold.
    lexicon_hash     text        NOT NULL,
    actor            jsonb       NOT NULL,            -- Principal
    authority        text        NOT NULL,            -- object-capability (K-17, K-35)
    target           jsonb       NOT NULL,            -- TargetBinding
    payload          jsonb       NOT NULL,
    payload_hash     text        NOT NULL,            -- SHA-256 hex of payload
    idempotency_key  text        NOT NULL,
    causation_id     uuid        NULL,                -- load-bearing for cross-stream (§3.4)
    correlation_id   uuid        NOT NULL,
    -- VALID-time (B1): the frozen clock, an input; can be backdated. NEVER the
    -- recovery filter, NEVER read by a fold's logic.
    as_of            timestamptz NOT NULL,
    -- TRANSACTION-time (B1): when we recorded the belief. Monotonic with seq
    -- within a subject by construction. THIS is the recovery axis
    -- (recover_determination_at folds a committed_at <= T prefix).
    committed_at     timestamptz NOT NULL DEFAULT now(),
    -- External-lookup results captured at first apply (Q6/H5); replay reads
    -- these, never re-calls the external service.
    captured_effects jsonb       NOT NULL DEFAULT '[]'::jsonb,
    PRIMARY KEY (subject_root, seq),
    -- Idempotent re-apply (F): a retried verb with the same key is a no-op.
    UNIQUE (subject_root, idempotency_key)
);

CREATE INDEX IF NOT EXISTS kyc_intent_events_event_id_idx
    ON "ob-poc".kyc_intent_events (event_id);
CREATE INDEX IF NOT EXISTS kyc_intent_events_corr_idx
    ON "ob-poc".kyc_intent_events (correlation_id);
-- B1: recovery is by transaction-time prefix; index it per subject.
CREATE INDEX IF NOT EXISTS kyc_intent_events_committed_idx
    ON "ob-poc".kyc_intent_events (subject_root, committed_at);

-- ── Per-subject sequence allocator + optional fold checkpoint. ──
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_subject_streams (
    subject_root      uuid PRIMARY KEY,
    -- The next seq to allocate for this subject. Bumped transactionally with
    -- each event insert; the FOR UPDATE lock on this row is the ordering domain.
    next_seq          bigint      NOT NULL DEFAULT 0,
    -- Last folded seq (perf cache; pure-derivable by replaying 0..=checkpoint_seq).
    checkpoint_seq    bigint      NULL,
    checkpoint_state  jsonb       NULL,
    -- The lexicon manifest the checkpoint was folded under (D2). A manifest
    -- change invalidates the checkpoint (drop and re-fold under the new registry).
    checkpoint_lexicon_manifest text NULL,
    updated_at        timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE "ob-poc".kyc_intent_events IS
    'KYC/UBO authoritative verb stream (K-16). Append-only; per-subject ordered (Q6). Projections fold over this. EOP-DD-KYCUBO-002 §2.';
COMMENT ON TABLE "ob-poc".kyc_subject_streams IS
    'Per-subject seq allocator + fold checkpoint. The FOR UPDATE lock on a row is the per-subject ordering domain. EOP-DD-KYCUBO-002 §3.';
