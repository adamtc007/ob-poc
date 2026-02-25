-- Migration 081: Semantic Registry Phase 2 — Taxonomy, Membership, View Definition views
--
-- Adds convenience views for Phase 2 registry object types:
--   - v_active_taxonomy_defs
--   - v_active_taxonomy_nodes
--   - v_active_membership_rules
--   - v_active_view_defs
--
-- All views extract typed fields from the JSONB definition column
-- in sem_reg.snapshots, filtered to active snapshots only.

-- ────────────────────────────────────────────────────────────────
-- 1. Taxonomy Definition view
-- ────────────────────────────────────────────────────────────────
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

-- ────────────────────────────────────────────────────────────────
-- 2. Taxonomy Node view
-- ────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW sem_reg.v_active_taxonomy_nodes AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'           AS fqn,
    s.definition->>'name'          AS name,
    s.definition->>'description'   AS description,
    s.definition->>'taxonomy_fqn'  AS taxonomy_fqn,
    s.definition->>'parent_fqn'    AS parent_fqn,
    (s.definition->>'sort_order')::int AS sort_order,
    s.definition->'labels'         AS labels
FROM sem_reg.snapshots s
WHERE s.object_type = 'taxonomy_node'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 3. Membership Rule view
-- ────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW sem_reg.v_active_membership_rules AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'            AS fqn,
    s.definition->>'name'           AS name,
    s.definition->>'description'    AS description,
    s.definition->>'taxonomy_fqn'   AS taxonomy_fqn,
    s.definition->>'node_fqn'       AS node_fqn,
    s.definition->>'membership_kind' AS membership_kind,
    s.definition->>'target_type'    AS target_type,
    s.definition->>'target_fqn'     AS target_fqn,
    s.definition->'conditions'      AS conditions
FROM sem_reg.snapshots s
WHERE s.object_type = 'membership_rule'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 4. View Definition view
-- ────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW sem_reg.v_active_view_defs AS
SELECT
    s.snapshot_id,
    s.object_id,
    s.version_major,
    s.governance_tier,
    s.trust_class,
    s.created_by,
    s.effective_from,
    s.definition->>'fqn'           AS fqn,
    s.definition->>'name'          AS name,
    s.definition->>'description'   AS description,
    s.definition->>'domain'        AS domain,
    s.definition->>'base_entity_type' AS base_entity_type,
    s.definition->'columns'        AS columns,
    s.definition->'filters'        AS filters,
    s.definition->'sort_order'     AS sort_order
FROM sem_reg.snapshots s
WHERE s.object_type = 'view_def'
  AND s.status = 'active'
  AND s.effective_until IS NULL;

-- ────────────────────────────────────────────────────────────────
-- 5. FQN lookup indexes for Phase 2 types
-- ────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_snapshots_taxonomy_def_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'taxonomy_def'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_taxonomy_node_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'taxonomy_node'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_membership_rule_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'membership_rule'
      AND status = 'active'
      AND effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshots_view_def_fqn
    ON sem_reg.snapshots ((definition->>'fqn'))
    WHERE object_type = 'view_def'
      AND status = 'active'
      AND effective_until IS NULL;

-- Index for taxonomy node parent traversal
CREATE INDEX IF NOT EXISTS idx_snapshots_taxonomy_node_parent
    ON sem_reg.snapshots ((definition->>'taxonomy_fqn'), (definition->>'parent_fqn'))
    WHERE object_type = 'taxonomy_node'
      AND status = 'active'
      AND effective_until IS NULL;

-- Index for membership rule target lookup
CREATE INDEX IF NOT EXISTS idx_snapshots_membership_target
    ON sem_reg.snapshots ((definition->>'target_fqn'))
    WHERE object_type = 'membership_rule'
      AND status = 'active'
      AND effective_until IS NULL;
