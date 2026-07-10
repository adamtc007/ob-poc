-- Phase 6c (state-graph remediation, RW-5): control_edges has zero writers
-- (confirmed: 0 rows live, no Rust code path writes to it -- the write
-- verbs were deleted in the W4 rip, per CLAUDE.md). Its associated
-- trigger + function are dead. Table itself is NOT dropped (fenced --
-- table drops ride the W1-proper cutover); COMMENT ON TABLE marks it
-- deprecated instead.
--
-- set_bods_interest_type() exists as two separate, identically-named
-- functions in different schemas (public.set_bods_interest_type and
-- "ob-poc".set_bods_interest_type) -- the trigger calls the "ob-poc" one;
-- the public one has zero triggers/callers at all. Both are dead; both
-- dropped.

BEGIN;

DROP TRIGGER IF EXISTS trg_control_edges_set_standards ON "ob-poc".control_edges;
DROP FUNCTION IF EXISTS "ob-poc".set_bods_interest_type();
DROP FUNCTION IF EXISTS public.set_bods_interest_type();

COMMENT ON TABLE "ob-poc".control_edges IS
  'DEPRECATED (state-graph remediation Phase 6c, 2026-07-02): zero writers -- '
  'the ubo/control write verbs that populated this table were deleted in the '
  'W4 rip. Superseded by "ob-poc".kyc_control_edge_projection (dsl.kyc '
  'stream fold, K-34). Table retained (not dropped) pending the W1-proper '
  'cutover.';

COMMIT;
