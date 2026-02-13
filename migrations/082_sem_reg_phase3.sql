-- Migration 082: Semantic Registry Phase 3 — Policy, Evidence, Document Type, Observation views
--
-- Adds convenience views for Phase 3 registry object types:
--   - v_active_policy_rules
--   - v_active_evidence_requirements
--   - v_active_document_type_defs
--   - v_active_observation_defs
--
-- All views extract typed fields from the JSONB definition column
-- in sem_reg.snapshots, filtered to active snapshots only.

-- ────────────────────────────────────────────────────────────────
-- 1. Policy Rule view
-- ────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW sem_reg.v_active_policy_rules AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'         AS fqn,
    s.definition->>'name'        AS name,
    s.definition->>'description' AS description,
    s.definition->>'domain'      AS domain,
    s.definition->>'scope'       AS scope,
    (s.definition->>'priority')::int AS priority,
    s.definition->'predicates'   AS predicates,
    s.definition->'actions'      AS actions,
    (s.definition->>'enabled')::boolean AS enabled
FROM sem_reg.snapshots s
WHERE s.object_type = 'policy_rule'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 2. Evidence Requirement view
-- ────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW sem_reg.v_active_evidence_requirements AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'              AS fqn,
    s.definition->>'name'             AS name,
    s.definition->>'description'      AS description,
    s.definition->>'target_entity_type' AS target_entity_type,
    s.definition->>'trigger_context'  AS trigger_context,
    s.definition->'required_documents'   AS required_documents,
    s.definition->'required_observations' AS required_observations,
    (s.definition->>'all_required')::boolean AS all_required
FROM sem_reg.snapshots s
WHERE s.object_type = 'evidence_requirement'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 3. Document Type Definition view
-- ────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW sem_reg.v_active_document_type_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'             AS fqn,
    s.definition->>'name'            AS name,
    s.definition->>'description'     AS description,
    s.definition->>'category'        AS category,
    (s.definition->>'max_age_days')::int AS max_age_days,
    s.definition->'accepted_formats' AS accepted_formats,
    s.definition->'extraction_rules' AS extraction_rules
FROM sem_reg.snapshots s
WHERE s.object_type = 'document_type_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 4. Observation Definition view
-- ────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW sem_reg.v_active_observation_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'              AS fqn,
    s.definition->>'name'             AS name,
    s.definition->>'description'      AS description,
    s.definition->>'observation_type' AS observation_type,
    s.definition->>'source_verb_fqn'  AS source_verb_fqn,
    s.definition->'extraction_rules'  AS extraction_rules,
    (s.definition->>'requires_human_review')::boolean AS requires_human_review
FROM sem_reg.snapshots s
WHERE s.object_type = 'observation_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 5. FQN lookup indexes for Phase 3 types
-- ────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_snapshots_policy_rule_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'policy_rule'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_evidence_req_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'evidence_requirement'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_document_type_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'document_type_def'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_observation_def_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'observation_def'
      AND status = 'active'
      AND effective_until IS NULL;

-- Index for policy rules by domain+scope (common query pattern)
CREATE INDEX IF NOT EXISTS idx_snapshots_policy_rule_domain
    ON sem_reg.snapshots ((definition->>'domain'), (definition->>'scope'))
    WHERE object_type = 'policy_rule'
      AND status = 'active'
      AND effective_until IS NULL;

-- Index for evidence requirements by target entity type
CREATE INDEX IF NOT EXISTS idx_snapshots_evidence_req_target
    ON sem_reg.snapshots ((definition->>'target_entity_type'))
    WHERE object_type = 'evidence_requirement'
      AND status = 'active'
      AND effective_until IS NULL;
