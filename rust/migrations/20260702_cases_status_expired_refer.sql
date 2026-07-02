-- Phase 2d (state-graph remediation, RW-1, ratified 1.4): extend
-- cases_chk_case_status to 11 values, adding EXPIRED and REFER_TO_REGULATOR.
--
-- These were already phantom-written by kyc-case.close/.reject (Rust-side
-- CLOSE_STATUSES includes both) and already declared in kyc_dag.yaml's
-- `decided` phase terminal_states (:284) -- the live CHECK constraint was the
-- only source that disagreed, causing a confirmed live G3 phantom-write bug.

BEGIN;

ALTER TABLE "ob-poc".cases DROP CONSTRAINT cases_chk_case_status;

ALTER TABLE "ob-poc".cases
  ADD CONSTRAINT cases_chk_case_status
  CHECK (status IN (
    'INTAKE', 'DISCOVERY', 'ASSESSMENT', 'REVIEW', 'APPROVED', 'REJECTED',
    'BLOCKED', 'WITHDRAWN', 'DO_NOT_ONBOARD', 'EXPIRED', 'REFER_TO_REGULATOR'
  ));

COMMIT;
