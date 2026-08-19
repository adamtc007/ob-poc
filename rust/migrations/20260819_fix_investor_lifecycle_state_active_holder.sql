-- EOP-PLAN-GAMEBOARD-001 R2 Stage 2 Phase 0 (2026-08-19): fix a
-- declaration-vs-schema-reality bug found while scoping Stage 2.
--
-- The real "active" value for "ob-poc".investors.lifecycle_state, as
-- written by investor.activate's plugin op
-- (crates/sem_os_postgres/src/ops/investor.rs, the `Activate` struct), is
-- 'ACTIVE_HOLDER' -- not 'ACTIVE'. But this session's earlier
-- 20260819_backfill_investors_register_allianz.sql migration set the one
-- real investor row's lifecycle_state to 'ACTIVE' by hand, matching a
-- parallel bug in cross_slot_census.rs's investor_kyc_approved() /
-- holding_investor_active() checks (both queried 'ACTIVE' too). All three
-- "lined up" by coincidence, which is exactly why both R2 cross_slot_
-- constraints rules reported [clean] -- a vacuous clean (querying for a
-- value that never legitimately occurs), not a real one.
--
-- This migration corrects the one live row. The matching check-function
-- and DAG-rule-text fixes land in the same commit (not a separate SQL
-- change).

UPDATE "ob-poc".investors
SET lifecycle_state = 'ACTIVE_HOLDER'
WHERE lifecycle_state = 'ACTIVE';
