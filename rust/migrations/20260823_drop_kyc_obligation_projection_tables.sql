-- EOP-DD-KYCUBO-D2.1 Tranche A (§5.1 of the sequencing, closing a §4
-- "failing the rule" item from the 2026-08-23 reconciliation).
--
-- D2.0 §5 said the obligation projection "goes with obligations." The
-- Rust projector (`PgKycObligationProjector`/`ObligationProjectionStats`)
-- was removed 2026-08-22 alongside the dissolution of
-- `kyc_ubo.assert.obligation.creation`/`.satisfaction` (the only writer
-- of a new `ObligationTracks` entry, whose dissolution left the projector
-- with nothing to project) -- but the tables themselves were left behind.
--
-- Confirmed by query before this migration was written: no view selects
-- from either table, no `src/` module reads from either table (the two
-- source hits are retirement comments, not code), no writer exists (the
-- only remaining references are test-cleanup `DELETE FROM ... WHERE
-- subject_root = $1` table lists), and no foreign key targets either
-- table. `kyc_obligation_projection` held 6 rows and
-- `kyc_subject_rollup_projection` held 93 rows -- both permanently frozen
-- (no writer since 2026-08-22) and readable as if current by anything
-- that queried them directly. Dropping both closes that gap.
--
-- Reintroduction path: none named -- the run book (D2.0 §4) replaces this
-- capability, not a future projection.

DROP TABLE IF EXISTS "ob-poc".kyc_obligation_projection;
DROP TABLE IF EXISTS "ob-poc".kyc_subject_rollup_projection;
