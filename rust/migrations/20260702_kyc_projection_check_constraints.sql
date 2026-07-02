-- Phase 1b (state-graph remediation, RW-6): mirror the Rust fold enums as
-- CHECK constraints on the disposable projection tables so the DB-legal
-- value set (source A) is no longer silently unconstrained text.
--
-- Values transcribed exactly from the Display/state_name() impls that write
-- these columns (verified against source, not guessed):
--   kyc_control_edge_projection.status      <- EdgeStatus::Display
--     (rust/crates/ob-poc-kyc-substrate/src/fold/control.rs)
--   kyc_obligation_projection.{identity,screening,risk}_state <- TrackState::state_name()
--     (rust/crates/ob-poc-kyc-substrate/src/fold/obligation.rs) -- 7 variants,
--     not 6: Pending, InProgress, Satisfied, Waived, Deferred, Expired, Rejected.
--   kyc_subject_rollup_projection.overall_state <- SubjectOverallState match arm
--     (rust/crates/ob-poc-kyc-store/src/projection.rs)

BEGIN;

ALTER TABLE "ob-poc".kyc_control_edge_projection
  ADD CONSTRAINT kyc_control_edge_projection_status_check
  CHECK (status IN ('Asserted', 'Evidenced', 'Verified', 'Superseded'));

ALTER TABLE "ob-poc".kyc_obligation_projection
  ADD CONSTRAINT kyc_obligation_projection_identity_state_check
  CHECK (identity_state IN ('Pending', 'InProgress', 'Satisfied', 'Waived', 'Deferred', 'Expired', 'Rejected'));

ALTER TABLE "ob-poc".kyc_obligation_projection
  ADD CONSTRAINT kyc_obligation_projection_screening_state_check
  CHECK (screening_state IN ('Pending', 'InProgress', 'Satisfied', 'Waived', 'Deferred', 'Expired', 'Rejected'));

ALTER TABLE "ob-poc".kyc_obligation_projection
  ADD CONSTRAINT kyc_obligation_projection_risk_state_check
  CHECK (risk_state IN ('Pending', 'InProgress', 'Satisfied', 'Waived', 'Deferred', 'Expired', 'Rejected'));

ALTER TABLE "ob-poc".kyc_subject_rollup_projection
  ADD CONSTRAINT kyc_subject_rollup_projection_overall_state_check
  CHECK (overall_state IN ('InProgress', 'AllTerminal', 'Approved', 'Rejected'));

COMMIT;
