-- Migration 083: Semantic Registry Phases 4-5 — Derivation spec views + indexes
--
-- Adds:
--   - v_active_derivation_specs convenience view
--   - FQN lookup index for derivation specs
--   - Dependency lookup index (derivation inputs)

-- ────────────────────────────────────────────────────────────────
-- 1. Derivation Spec convenience view
-- ────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW sem_reg.v_active_derivation_specs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'                      AS fqn,
    s.definition->>'name'                     AS name,
    s.definition->>'description'              AS description,
    s.definition->>'output_attribute_fqn'     AS output_attribute_fqn,
    s.definition->'inputs'                    AS inputs,
    s.definition->'expression'                AS expression,
    s.definition->>'null_semantics'           AS null_semantics,
    s.definition->>'security_inheritance'     AS security_inheritance,
    s.definition->>'evidence_grade'           AS evidence_grade,
    s.definition->'freshness_rule'            AS freshness_rule,
    s.definition->'tests'                     AS tests,
    s.security_label                          AS security_label
FROM sem_reg.snapshots s
WHERE s.object_type = 'derivation_spec'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 2. FQN lookup index for derivation specs
-- ────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_snapshots_derivation_spec_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'derivation_spec'
      AND status = 'active'
      AND effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 3. Dependency lookup index (derivation inputs)
--    GIN index on the inputs array for containment queries like:
--    WHERE definition->'inputs' @> '[{"attribute_fqn": "kyc.raw_score"}]'
-- ────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_snapshots_derivation_inputs
    ON sem_reg.snapshots USING gin ((definition->'inputs') jsonb_path_ops)
    WHERE object_type = 'derivation_spec'
      AND status = 'active'
      AND effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 4. Security label index for label-based queries
--    Supports queries like: WHERE security_label->>'classification' = 'restricted'
-- ────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_snapshots_security_classification
    ON sem_reg.snapshots ((security_label->>'classification'))
    WHERE status = 'active'
      AND effective_until IS NULL;
