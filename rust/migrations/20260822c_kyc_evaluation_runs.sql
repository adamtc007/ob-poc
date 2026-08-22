-- EOP-DD-KYCUBO-D2.0 §4 — the run book. Append-only evaluation runs.
--
-- Written only by `ob-poc-kyc-decide` (the Evaluation pack) — the crate has
-- no dependency on `ob-poc-kyc-seam` (the sole fact-stream append
-- chokepoint), so this table, like `kyc_decision_records`, is structurally
-- outside the dsl.kyc fact stream (`evaluation_never_writes_facts`, D2.0
-- §6). Rows are never updated or deleted: re-running creates a new row; the
-- old one stands as what was concluded then, from what was known then
-- (§4's bitemporal-conclusions law).

CREATE TABLE IF NOT EXISTS "ob-poc".kyc_evaluation_runs (
    run_id                       uuid        PRIMARY KEY,
    subject_root                 uuid        NOT NULL,
    board_state_hash             text        NOT NULL,
    evaluation_pack_version_hash text        NOT NULL,
    valid_time                   timestamptz NOT NULL,
    knowledge_time                timestamptz NOT NULL,
    trigger                      text        NOT NULL,
    -- The in-scope check set AS COMPUTED at run time (D2.0 §3's one
    -- invariant regardless: never re-derived from this row, only compared
    -- against a fresh computation for staleness).
    in_scope_check_ids           jsonb       NOT NULL,
    -- Per-check verdict + findings, each citing the facts it relied on.
    findings                     jsonb       NOT NULL,
    created_at                   timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS kyc_evaluation_runs_subject_root_idx
    ON "ob-poc".kyc_evaluation_runs (subject_root, created_at DESC);

COMMENT ON TABLE "ob-poc".kyc_evaluation_runs IS
    'D2.0 run book. Append-only — no UPDATE, no DELETE. Written only by '
    'ob-poc-kyc-decide, never by anything with a dependency on '
    'ob-poc-kyc-seam. The current work list and staleness are DERIVED by '
    'querying this table, never stored as separate columns/flags.';
