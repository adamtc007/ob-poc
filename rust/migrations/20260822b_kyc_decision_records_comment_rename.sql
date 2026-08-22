-- Refresh `kyc_decision_records`'s schema comments for the four-segment verb
-- rename (RATIFIED 2026-08-22): `decide.approve`/`decide.reject` became
-- `kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject`.
--
-- A separate migration rather than an edit to 20260822_kyc_decision_records.sql:
-- that one is applied, and migrations are forward-only. This is also the
-- "a rename does NOT auto-update COMMENT" lesson from the 2026-06-22
-- cbu_relationship_verification -> ubo_relationship_verification rename, where
-- the table rename left the view comment describing the old name.
--
-- Comments only. No DDL, no data change. `verb_fqn` has no CHECK constraint, so
-- existing rows keep whatever FQN they were written with — correct for an
-- append-only decision record: it says what the verb was called at decision time.

COMMENT ON TABLE "ob-poc".kyc_decision_records IS
  'Evaluation-pack decision records (EOP-DD-KYCUBO-TS.6 §1/§4). Written ONLY by '
  'ob-poc-kyc-decide (kyc_ubo.decide.subject.approve / kyc_ubo.decide.subject.reject) '
  '— never by anything holding a fact-stream append. The K-23 "decision is final" '
  'check reads this table directly rather than the dsl.kyc fold, which cannot see '
  'a decision (Approved/Rejected were retired from the substrate fold in TS.6 P2).';

COMMENT ON COLUMN "ob-poc".kyc_decision_records.verb_fqn IS
  'kyc_ubo.decide.subject.approve | kyc_ubo.decide.subject.reject. Historical rows '
  'may carry the pre-2026-08-22 names (decide.approve / kyc.person.approve) — the '
  'record states what the verb was called when the decision was taken.';
