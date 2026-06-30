-- EOP-DD-KYCUBO-001 §4.2 / W6 — obligation-graph stream projection.
--
-- A DISPOSABLE, REBUILDABLE fold of the obligation graph from the verb stream
-- (K-34). Written ONLY by the obligation projector, never by a direct verb write.
-- Two tables: one per obligation track (with basis), one per subject rollup.
-- The verb stream (kyc_intent_events) is the system of record.

-- ── Per-obligation track projection ──────────────────────────────────────────
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_obligation_projection (
    subject_root         uuid      NOT NULL,  -- same as the stream subject_root
    obligation_id        uuid      NOT NULL,
    -- ObligationBasis (K-21: recorded, never inferred)
    basis_role           text      NOT NULL,
    basis_jurisdiction   text      NULL,
    basis_cbu_role       text      NULL,
    basis_source_event_id uuid     NOT NULL,
    -- Parallel tracks (Q4: independent, all-terminal gates approval K-23)
    identity_state       text      NOT NULL DEFAULT 'Pending',
    screening_state      text      NOT NULL DEFAULT 'Pending',
    risk_state           text      NOT NULL DEFAULT 'Pending',
    -- K-35 traceability
    originating_event_id uuid      NOT NULL,
    projected_at         timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (subject_root, obligation_id)
);

-- ── Per-subject rollup projection ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_subject_rollup_projection (
    subject_root         uuid      NOT NULL PRIMARY KEY,
    overall_state        text      NOT NULL DEFAULT 'InProgress',
    obligation_count     integer   NOT NULL DEFAULT 0,
    all_terminal         boolean   NOT NULL DEFAULT false,
    decision_event_id    uuid      NULL,
    projected_at         timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS kyc_obligation_projection_state_idx
    ON "ob-poc".kyc_obligation_projection (subject_root, identity_state, screening_state, risk_state);

COMMENT ON TABLE "ob-poc".kyc_obligation_projection IS
    'Disposable fold of obligation tracks from kyc_intent_events (K-34). Written only '
    'by the obligation projector; rebuildable. EOP-DD-KYCUBO-001 §4.2 / W6.';
COMMENT ON TABLE "ob-poc".kyc_subject_rollup_projection IS
    'Per-subject overall KYC state (fold over obligation tracks, Q4). Disposable. EOP-DD-KYCUBO-001 §4.2 / W6.';
