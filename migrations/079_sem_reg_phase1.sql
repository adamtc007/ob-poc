-- Migration 079: Semantic Registry Phase 1 — Convenience Views
--
-- Creates views that extract typed fields from the JSONB definition column
-- for the three core registry types: attribute_def, entity_type_def, verb_contract.
-- Also adds a registry statistics view.

-- ── Active Attribute Definitions ──────────────────────────────

CREATE OR REPLACE VIEW sem_reg.v_active_attribute_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.version_minor,
    s.governance_tier,
    s.trust_class,
    s.security_label,
    s.created_by,
    s.effective_from,
    s.definition->>'name'                             AS attr_name,
    s.definition->>'fqn'                              AS fqn,
    s.definition->>'description'                      AS description,
    s.definition->>'data_type'                        AS data_type,
    s.definition->>'domain'                           AS domain,
    s.definition->'source'                            AS source,
    s.definition->'constraints'                       AS constraints,
    s.definition->'sinks'                             AS sinks,
    s.definition
FROM sem_reg.snapshots s
WHERE s.object_type = 'attribute_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

COMMENT ON VIEW sem_reg.v_active_attribute_defs IS
    'Active attribute definitions with extracted JSONB fields';

-- ── Active Entity Type Definitions ────────────────────────────

CREATE OR REPLACE VIEW sem_reg.v_active_entity_type_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.version_minor,
    s.governance_tier,
    s.trust_class,
    s.security_label,
    s.created_by,
    s.effective_from,
    s.definition->>'name'                             AS type_name,
    s.definition->>'fqn'                              AS fqn,
    s.definition->>'description'                      AS description,
    s.definition->>'domain'                           AS domain,
    s.definition->'db_table'                          AS db_table,
    s.definition->'lifecycle_states'                  AS lifecycle_states,
    s.definition->'required_attributes'               AS required_attributes,
    s.definition->'optional_attributes'               AS optional_attributes,
    s.definition
FROM sem_reg.snapshots s
WHERE s.object_type = 'entity_type_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

COMMENT ON VIEW sem_reg.v_active_entity_type_defs IS
    'Active entity type definitions with extracted JSONB fields';

-- ── Active Verb Contracts ─────────────────────────────────────

CREATE OR REPLACE VIEW sem_reg.v_active_verb_contracts AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.version_minor,
    s.governance_tier,
    s.trust_class,
    s.security_label,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'                              AS fqn,
    s.definition->>'domain'                           AS domain,
    s.definition->>'action'                           AS action,
    s.definition->>'description'                      AS description,
    s.definition->>'behavior'                         AS behavior,
    s.definition->'args'                              AS args,
    s.definition->'returns'                           AS returns,
    s.definition->'preconditions'                     AS preconditions,
    s.definition->'postconditions'                    AS postconditions,
    s.definition->'produces'                          AS produces,
    s.definition->'consumes'                          AS consumes,
    s.definition->'invocation_phrases'                AS invocation_phrases,
    s.definition
FROM sem_reg.snapshots s
WHERE s.object_type = 'verb_contract'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

COMMENT ON VIEW sem_reg.v_active_verb_contracts IS
    'Active verb contracts with extracted JSONB fields';

-- ── Registry Statistics ───────────────────────────────────────

CREATE OR REPLACE VIEW sem_reg.v_registry_stats AS
SELECT
    object_type,
    COUNT(*) AS active_count,
    COUNT(DISTINCT object_id) AS unique_objects,
    MIN(effective_from) AS earliest,
    MAX(effective_from) AS latest
FROM sem_reg.snapshots
WHERE status = 'active'
  AND effective_until IS NULL
GROUP BY object_type
ORDER BY object_type;

COMMENT ON VIEW sem_reg.v_registry_stats IS
    'Summary statistics for all active registry objects by type';

-- ── FQN lookup indexes ────────────────────────────────────────
-- These accelerate the common `find_active_by_definition_field` queries.

CREATE INDEX IF NOT EXISTS idx_snapshots_attr_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'attribute_def'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_entity_type_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'entity_type_def'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_verb_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'verb_contract'
      AND status = 'active'
      AND effective_until IS NULL;
