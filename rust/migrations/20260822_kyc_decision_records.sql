-- TS.6 P1/P2 — Evaluation pack decision records (EOP-DD-KYCUBO-TS.6 §1/§4).
--
-- `decide.approve`/`decide.reject` (renamed from `kyc.person.approve`/
-- `.reject`) write here instead of the dsl.kyc fact stream
-- (`"ob-poc".kyc_intent_events`) — the structural proof that the Evaluation
-- pack has no write path to the fact stream: `ob-poc-kyc-decide`, the crate
-- that owns these ops, has no dependency on `ob-poc-kyc-seam` (the sole
-- fact-stream append chokepoint), verified by
-- `scripts/check_kyc_decide_deps.sh`.
--
-- The K-23 "decision is final" finality guard moved here too
-- (`Precondition::SubjectNotDecided` and `SubjectOverallState::Approved`/
-- `Rejected` retired from the substrate fold, TS.6 P2) — `decide.approve`/
-- `decide.reject` query this table directly for a prior terminal decision
-- before writing a new one, rather than relying on the fact-stream fold to
-- have seen their own prior event.

CREATE TABLE IF NOT EXISTS "ob-poc".kyc_decision_records (
    id               uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_root     uuid        NOT NULL,
    verb_fqn         text        NOT NULL,   -- decide.approve | decide.reject
    decided_by       text        NOT NULL,   -- object-capability authority string
    decided_at       timestamptz NOT NULL DEFAULT clock_timestamp(),
    -- What the decision relied on (§8 decide_verbs_cite_their_basis) — the
    -- obligation-fold snapshot (overall_state + obligation ids) folded at
    -- decision time. Never empty by construction (every DecideApprove/
    -- DecideReject::execute() builds this before inserting).
    basis            jsonb       NOT NULL,
    reason           text        NULL,
    raw_args         jsonb       NOT NULL
);

CREATE INDEX IF NOT EXISTS kyc_decision_records_subject_root_idx
    ON "ob-poc".kyc_decision_records (subject_root);

COMMENT ON TABLE "ob-poc".kyc_decision_records IS
    'Evaluation-pack decision records (TS.6 §1/§4). Written only by '
    'ob-poc-kyc-decide (decide.approve/decide.reject) — never by anything '
    'with a dependency on ob-poc-kyc-seam. Not part of the dsl.kyc fact '
    'stream; the finality guard (K-23, "decision is final") queries this '
    'table directly rather than the fact-stream fold.';
