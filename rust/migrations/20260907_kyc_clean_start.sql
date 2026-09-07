-- EOP-DD-UBO-CLEANOUT-001 (Clean Start) — T6 P1.
--
-- This is development, not production (§0): nothing cleared here is a
-- customer record, a regulatory artifact, or evidence of anything. The
-- events were written by test sessions across three retired vocabulary
-- generations (T2/T3/T4/T5 renames) against a dev database.
--
-- Counts at the moment of clearing (2026-09-07, taken immediately before
-- this migration ran — the audit's 2026-08-28 figure was 854; the stream
-- kept growing from test runs in the interim, confirming §1's point that
-- the argument for a clean start only strengthens the longer this waits):
--   kyc_intent_events            872 rows / 211 distinct subject_root / 23 distinct lexicon_hash
--   kyc_subject_streams          211 rows
--   kyc_lexicon_manifest          19 rows (only the newest — 14 entries — matches the live lexicon)
--   kyc_control_edge_projection   64 rows (disposable fold projection, K-34 — re-derivable, not lost)
--   kyc_evaluation_runs             6 rows
--   kyc_decision_records          11 rows
--
-- §4 (settled 2026-09-07, Adam): kyc_decision_records and kyc_decisions
-- both go — a decision whose basis cannot be reconstructed is not
-- evidence of anything, and keeping one teaches a reader that dangling
-- citations are tolerable.
--
-- C1: clear, do not archive. No parallel table, no `_archive` suffix.
--
-- Dead tables dropped (§2, corrected twice before this migration
-- finished — the original draft named "kyc_ubo_evidence"; that was
-- wrong, and so, discovered mid-migration, was "kyc_clearance_mandates"):
--   kyc_decisions            — cbu_id/case_id-keyed, its only writer
--                               (xtask/ubo_test.rs's record_decision) is
--                               a leftover from the already-retired
--                               cbu.decide-era demo path (CBU⊥KYC
--                               decoupling, 2026-06-22); removed from
--                               ubo_test.rs in the same tranche as this
--                               migration so `cargo x ubo-test` keeps
--                               compiling and running. DROPPED.
--
-- NOT dropped, despite appearing on the original census gloss —
-- verified live, not dead:
--   kyc_ubo_evidence         — live FK to kyc_ubo_registry (not the
--                               deleted entity_ubos path) and 5 live
--                               readers/writers in the separate,
--                               still-active KYC/UBO Skeleton subsystem
--                               (S1-S2) this cleanout does not touch.
--   kyc_clearance_mandates   — 0 rows and 0 Rust code references, but
--                               `DROP TABLE` failed live: two Deal-domain
--                               views (`deal_onboarding_request_compliance`,
--                               `deal_contracting_compliance`) depend on
--                               it, and the first of those currently
--                               returns a row — another domain's live
--                               state, out of this tranche's SCOPE FENCE
--                               ("the other domains' dirty state").
--                               A grep-only dead-table census misses
--                               view dependents; only the live DROP
--                               attempt caught this one.

DELETE FROM "ob-poc".kyc_intent_events;
DELETE FROM "ob-poc".kyc_subject_streams;
DELETE FROM "ob-poc".kyc_lexicon_manifest;
DELETE FROM "ob-poc".kyc_control_edge_projection;
DELETE FROM "ob-poc".kyc_evaluation_runs;
DELETE FROM "ob-poc".kyc_decision_records;

DROP TABLE IF EXISTS "ob-poc".kyc_decisions;
