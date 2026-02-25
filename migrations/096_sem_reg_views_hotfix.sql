-- Migration 096: Semantic Registry views hotfix
--
-- Fixes v_active_taxonomy_defs.classification_axis extraction:
--   definition->'classification_axis'  (JSONB)
--   →  definition->>'classification_axis' (TEXT)
--
-- Safe to run on any DB state — CREATE OR REPLACE VIEW is idempotent.

CREATE OR REPLACE VIEW sem_reg.v_active_taxonomy_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'         AS fqn,
    s.definition->>'name'        AS name,
    s.definition->>'description' AS description,
    s.definition->>'domain'      AS domain,
    s.definition->>'root_node_fqn' AS root_node_fqn,
    (s.definition->>'max_depth')::int AS max_depth,
    s.definition->>'classification_axis' AS classification_axis
FROM sem_reg.snapshots s
WHERE s.object_type = 'taxonomy_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;
