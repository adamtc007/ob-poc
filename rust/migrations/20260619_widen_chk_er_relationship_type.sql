-- Widen chk_er_relationship_type to admit the COMPLETE legitimate set of
-- relationship_type values that the application can produce — not only the four
-- fund types that happened to be hit by the parity test.
--
-- The constraint is a GUARD and is RETAINED (re-added), not dropped: it still
-- rejects any relationship_type outside the legitimate set. We only widen the
-- allowed set.
--
-- Legitimate set = the existing allowed values
--   ownership, control, trust_role, employment, management
-- UNION the full AssignFundRole output set
--   (sem_os_postgres/src/ops/cbu_role.rs:425-431)
--   FEEDER_FUND  -> master_feeder
--   SUB_FUND     -> umbrella_subfund
--   PARALLEL_FUND-> parallel
--   FUND_INVESTOR-> investment
--   MANAGEMENT_COMPANY|INVESTMENT_MANAGER -> management  (already allowed)
--   _ (any other fund role) -> fund_role
-- The ownership/control/trust_role writers (AssignOwnership/Control/TrustRole)
-- already produce allowed values; no direct entity_relationships writer
-- (ubo_graph, edge, import_run) produces a value outside this set.
--
-- No backfill: the constraint blocked every violator, so none exist
-- (SELECT DISTINCT relationship_type → {control, ownership, trust_role} only).

ALTER TABLE "ob-poc".entity_relationships
    DROP CONSTRAINT IF EXISTS chk_er_relationship_type;

ALTER TABLE "ob-poc".entity_relationships
    ADD CONSTRAINT chk_er_relationship_type CHECK (
        relationship_type = ANY (ARRAY[
            'ownership',
            'control',
            'trust_role',
            'employment',
            'management',
            'investment',
            'master_feeder',
            'umbrella_subfund',
            'parallel',
            'fund_role'
        ]::text[])
    );
