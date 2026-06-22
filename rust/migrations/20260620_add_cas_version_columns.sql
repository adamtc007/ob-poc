-- Phase 1 Task 2 (CAS): per-row optimistic-concurrency version columns on the
-- two directly-mutated compliance planes. TWO INDEPENDENT guards (not shared):
--   A1 = "ob-poc".cbu_entity_roles      (onboarding plane)
--   A7 = "ob-poc".entity_relationships  (UBO/ownership-graph plane)
--
-- DEFAULT 1 backfills every existing row (2854 A1 / 120 A7) with version 1 — no
-- separate backfill, no violation possible (the column is new).
--
-- The CAS guard (expected-version compare-and-set on UPDATE) is enforced in the
-- directly-mutated write paths (ops/cbu_role.rs, ops/entity_relationship.rs) and
-- CARVES OUT, by confirmed discriminators:
--   Class 2 — the ManCo designation (maker-checker, KYC-canonical):
--             A1 role_id ∈ {MANAGEMENT_COMPANY, INVESTMENT_MANAGER}
--             A7 relationship_type = 'management'
--   Class 3 — ubo_graph recompute (derived plane, ratification-exempt):
--             A7 source LIKE 'ubo.%'  (ubo_graph writes 'ubo.supersede' / 'ubo.transfer-control')
-- Only Class 1 (operator-authored, non-ManCo) is version-CAS'd.

ALTER TABLE "ob-poc".cbu_entity_roles
    ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 1;

ALTER TABLE "ob-poc".entity_relationships
    ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 1;
