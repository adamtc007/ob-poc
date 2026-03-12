-- Migration 122: SemReg document governance bootstrap
--
-- Adds first-wave governed document requirement object types:
--   - requirement_profile_def
--   - proof_obligation_def
--   - evidence_strategy_def
--
-- And exposes active views / indexes for snapshot consumption.

DO $$
BEGIN
    ALTER TYPE sem_reg.object_type ADD VALUE IF NOT EXISTS 'requirement_profile_def';
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$
BEGIN
    ALTER TYPE sem_reg.object_type ADD VALUE IF NOT EXISTS 'proof_obligation_def';
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$
BEGIN
    ALTER TYPE sem_reg.object_type ADD VALUE IF NOT EXISTS 'evidence_strategy_def';
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

CREATE OR REPLACE VIEW sem_reg.v_active_requirement_profile_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn' AS fqn,
    s.definition->>'name' AS name,
    s.definition->>'description' AS description,
    s.definition->'entity_types' AS entity_types,
    s.definition->'jurisdictions' AS jurisdictions,
    s.definition->'client_types' AS client_types,
    s.definition->'contexts' AS contexts,
    s.definition->>'effective_from' AS effective_from_date,
    s.definition->>'effective_to' AS effective_to_date,
    s.definition->'obligation_fqns' AS obligation_fqns
FROM sem_reg.snapshots s
WHERE s.object_type = 'requirement_profile_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

CREATE OR REPLACE VIEW sem_reg.v_active_proof_obligation_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn' AS fqn,
    s.definition->>'name' AS name,
    s.definition->>'description' AS description,
    s.definition->>'category' AS category,
    s.definition->>'strength_required' AS strength_required,
    (s.definition->>'is_mandatory')::boolean AS is_mandatory,
    (s.definition->>'freshness_days')::int AS freshness_days,
    (s.definition->>'legalisation_required')::boolean AS legalisation_required,
    (s.definition->>'notarisation_required')::boolean AS notarisation_required,
    s.definition->'evidence_strategy_fqns' AS evidence_strategy_fqns,
    s.definition->'conditions' AS conditions
FROM sem_reg.snapshots s
WHERE s.object_type = 'proof_obligation_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

CREATE OR REPLACE VIEW sem_reg.v_active_evidence_strategy_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn' AS fqn,
    s.definition->>'name' AS name,
    s.definition->>'description' AS description,
    s.definition->>'obligation_fqn' AS obligation_fqn,
    (s.definition->>'priority')::int AS priority,
    s.definition->>'proof_strength' AS proof_strength,
    s.definition->'components' AS components,
    s.definition->'extra_conditions' AS extra_conditions,
    s.definition->>'strength_downgrade_note' AS strength_downgrade_note,
    COALESCE((s.definition->>'enabled')::boolean, true) AS enabled
FROM sem_reg.snapshots s
WHERE s.object_type = 'evidence_strategy_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_requirement_profile_def_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'requirement_profile_def'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_proof_obligation_def_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'proof_obligation_def'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_evidence_strategy_def_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'evidence_strategy_def'
      AND status = 'active'
      AND effective_until IS NULL;
