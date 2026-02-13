-- Migration 084: Semantic Registry Phase 7 — Context Resolution convenience views
--
-- No new tables — context resolution is a computed projection over existing snapshots.
-- These views accelerate the resolution engine's common query patterns.

-- ── View: Active memberships by subject entity type ──────────
-- Fast taxonomy overlap lookup for context resolution Step 3.
-- Given a subject's entity type FQN, find which taxonomy nodes it belongs to.
CREATE OR REPLACE VIEW sem_reg.v_active_memberships_by_subject AS
SELECT
    mr.snapshot_id       AS membership_snapshot_id,
    mr.object_id         AS membership_id,
    mr.definition->>'taxonomy_fqn'     AS taxonomy_fqn,
    mr.definition->>'node_fqn'         AS node_fqn,
    mr.definition->>'membership_kind'  AS membership_kind,
    mr.definition->>'target_type'      AS target_type,
    mr.definition->>'target_fqn'       AS target_fqn,
    mr.governance_tier,
    mr.trust_class,
    mr.effective_from
FROM sem_reg.snapshots mr
WHERE mr.object_type = 'membership_rule'
  AND mr.status = 'active'
  AND mr.effective_until IS NULL
ORDER BY mr.definition->>'taxonomy_fqn', mr.definition->>'node_fqn';

COMMENT ON VIEW sem_reg.v_active_memberships_by_subject IS
  'Active membership rules flattened for fast taxonomy overlap lookups during context resolution';

-- ── View: Verb precondition status ───────────────────────────
-- Aggregates verb contracts with their precondition definitions
-- for evaluability checking in context resolution Step 8.
CREATE OR REPLACE VIEW sem_reg.v_verb_precondition_status AS
SELECT
    vc.snapshot_id       AS verb_snapshot_id,
    vc.object_id         AS verb_id,
    vc.definition->>'fqn'           AS verb_fqn,
    vc.definition->>'domain'        AS verb_domain,
    vc.definition->>'action'        AS verb_action,
    vc.governance_tier,
    vc.trust_class,
    jsonb_array_length(COALESCE(vc.definition->'preconditions', '[]'::jsonb)) AS precondition_count,
    vc.definition->'preconditions'  AS preconditions,
    vc.effective_from
FROM sem_reg.snapshots vc
WHERE vc.object_type = 'verb_contract'
  AND vc.status = 'active'
  AND vc.effective_until IS NULL
ORDER BY vc.definition->>'fqn';

COMMENT ON VIEW sem_reg.v_verb_precondition_status IS
  'Active verb contracts with precondition counts for resolution engine evaluability checks';

-- ── View: View-attribute cross-reference ─────────────────────
-- Maps ViewDef columns to their referenced attribute FQNs.
-- Used in resolution Steps 4-7 to derive prominence weights.
CREATE OR REPLACE VIEW sem_reg.v_view_attribute_columns AS
SELECT
    vd.snapshot_id       AS view_snapshot_id,
    vd.object_id         AS view_id,
    vd.definition->>'fqn'  AS view_fqn,
    vd.definition->>'domain' AS view_domain,
    col.value->>'attribute_fqn' AS attribute_fqn,
    col.value->>'label'         AS column_label,
    COALESCE((col.value->>'visible')::boolean, true) AS visible,
    col.ordinality              AS column_position
FROM sem_reg.snapshots vd,
     jsonb_array_elements(vd.definition->'columns') WITH ORDINALITY AS col(value, ordinality)
WHERE vd.object_type = 'view_def'
  AND vd.status = 'active'
  AND vd.effective_until IS NULL
ORDER BY vd.definition->>'fqn', col.ordinality;

COMMENT ON VIEW sem_reg.v_view_attribute_columns IS
  'Flattened view columns with position for attribute prominence ranking';

-- ── Index: Speed up membership target lookups ────────────────
-- The resolution engine frequently queries memberships by target_fqn.
CREATE INDEX IF NOT EXISTS idx_snapshots_membership_target_fqn
    ON sem_reg.snapshots ((definition->>'target_fqn'))
    WHERE object_type = 'membership_rule'
      AND status = 'active'
      AND effective_until IS NULL;

-- ── Index: Speed up verb domain lookups ──────────────────────
-- Context resolution filters verbs by domain frequently.
CREATE INDEX IF NOT EXISTS idx_snapshots_verb_domain
    ON sem_reg.snapshots ((definition->>'domain'))
    WHERE object_type = 'verb_contract'
      AND status = 'active'
      AND effective_until IS NULL;
