-- EOP-DD-KYCUBO-D2.1 §7 Q3 (RULED): "a run is an act in a session, not a
-- background job ... Sage triggers the permission and the REPL runs it
-- against the database. No side doors, except in test mode."
--
-- Every run must carry the session identity that acted, not just which
-- verb ran it. `trigger` (added by 20260822c_kyc_evaluation_runs.sql)
-- already carries the verb FQN ("who or what triggered it", D2.0 §4);
-- `triggering_session` adds the WHO a bare verb name can't express.
--
-- No pre-existing production data depends on a specific value here (every
-- row so far was written by test/reconciliation scratch work and cleaned
-- up); the nil-UUID default exists only so ADD COLUMN ... NOT NULL is
-- valid syntax against a table that might not be empty at apply time —
-- every INSERT going forward supplies a real session id
-- (`ob-poc-kyc-decide::compute_and_persist_run` refuses to construct a run
-- without one, EvaluationRun::new's IncompleteRun check).

ALTER TABLE "ob-poc".kyc_evaluation_runs
    ADD COLUMN IF NOT EXISTS triggering_session uuid NOT NULL
        DEFAULT '00000000-0000-0000-0000-000000000000';

COMMENT ON COLUMN "ob-poc".kyc_evaluation_runs.triggering_session IS
    'D2.1 §7 Q3: the session that triggered this run. A run is an act in a '
    'session, not a background job — EvaluationRun::new refuses to '
    'construct a run whose trigger carries the nil session id.';
