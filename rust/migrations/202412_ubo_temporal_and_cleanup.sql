-- UBO Temporal Columns and Legacy Table Cleanup
-- Adds effective_from/effective_to to ubo_edges for temporal ownership tracking
-- Prepares for deprecation of legacy tables: ownership_relationships, control_relationships, ubo_registry

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 1: Add temporal columns to ubo_edges
-- ═══════════════════════════════════════════════════════════════════════════

-- Add temporal validity columns (allows tracking historical ownership changes)
ALTER TABLE "ob-poc".ubo_edges
ADD COLUMN IF NOT EXISTS effective_from DATE,
ADD COLUMN IF NOT EXISTS effective_to DATE;

-- Add check constraint for temporal validity
ALTER TABLE "ob-poc".ubo_edges
ADD CONSTRAINT chk_ubo_edges_temporal
CHECK (effective_to IS NULL OR effective_from IS NULL OR effective_to >= effective_from);

-- Index for temporal queries
CREATE INDEX IF NOT EXISTS idx_ubo_edges_temporal
ON "ob-poc".ubo_edges(cbu_id, effective_from, effective_to);

-- View for current (non-expired) edges only
CREATE OR REPLACE VIEW "ob-poc".ubo_edges_current AS
SELECT *
FROM "ob-poc".ubo_edges
WHERE effective_to IS NULL OR effective_to >= CURRENT_DATE;

COMMENT ON COLUMN "ob-poc".ubo_edges.effective_from IS 'Date this relationship became effective (NULL = since inception)';
COMMENT ON COLUMN "ob-poc".ubo_edges.effective_to IS 'Date this relationship ended (NULL = still current)';
COMMENT ON VIEW "ob-poc".ubo_edges_current IS 'View of current (non-expired) ownership/control edges';

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 2: Data migration from legacy tables (if needed)
-- Only runs if legacy tables have data not yet in ubo_edges
-- ═══════════════════════════════════════════════════════════════════════════

-- Migrate ownership_relationships to ubo_edges (if any exist)
INSERT INTO "ob-poc".ubo_edges (
    cbu_id,
    from_entity_id,
    to_entity_id,
    edge_type,
    percentage,
    effective_from,
    effective_to,
    status,
    created_at
)
SELECT
    o.cbu_id,
    o.owner_entity_id AS from_entity_id,
    o.owned_entity_id AS to_entity_id,
    'ownership' AS edge_type,
    o.percentage,
    o.effective_from,
    o.effective_to,
    CASE WHEN o.is_verified THEN 'proven' ELSE 'alleged' END AS status,
    o.created_at
FROM "ob-poc".ownership_relationships o
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".ubo_edges e
    WHERE e.cbu_id = o.cbu_id
      AND e.from_entity_id = o.owner_entity_id
      AND e.to_entity_id = o.owned_entity_id
      AND e.edge_type = 'ownership'
)
ON CONFLICT (cbu_id, from_entity_id, to_entity_id, edge_type) DO NOTHING;

-- Migrate control_relationships to ubo_edges (if any exist)
INSERT INTO "ob-poc".ubo_edges (
    cbu_id,
    from_entity_id,
    to_entity_id,
    edge_type,
    control_role,
    effective_from,
    effective_to,
    status,
    created_at
)
SELECT
    c.cbu_id,
    c.controller_entity_id AS from_entity_id,
    c.controlled_entity_id AS to_entity_id,
    'control' AS edge_type,
    c.control_type AS control_role,
    c.effective_from,
    c.effective_to,
    'proven' AS status,  -- Control relationships were typically proven
    c.created_at
FROM "ob-poc".control_relationships c
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".ubo_edges e
    WHERE e.cbu_id = c.cbu_id
      AND e.from_entity_id = c.controller_entity_id
      AND e.to_entity_id = c.controlled_entity_id
      AND e.edge_type = 'control'
)
ON CONFLICT (cbu_id, from_entity_id, to_entity_id, edge_type) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 3: Mark legacy tables as deprecated (do NOT drop yet)
-- Keep tables for backwards compatibility during migration period
-- ═══════════════════════════════════════════════════════════════════════════

COMMENT ON TABLE "ob-poc".ownership_relationships IS 'DEPRECATED: Use ubo_edges with edge_type=ownership instead';
COMMENT ON TABLE "ob-poc".control_relationships IS 'DEPRECATED: Use ubo_edges with edge_type=control instead';
COMMENT ON TABLE "ob-poc".ubo_registry IS 'DEPRECATED: UBO status now derived from ubo_edges + entity_workstreams';

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 4: Add helper function for UBO qualification check
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION "ob-poc".is_natural_person(entity_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE e.entity_id = is_natural_person.entity_id
          AND et.entity_category = 'PERSON'
    );
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".is_natural_person IS 'Returns true if entity is a natural person (PERSON category)';

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 5: View for UBO candidates (natural persons with >= 25% ownership)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW "ob-poc".ubo_candidates AS
WITH RECURSIVE ownership_chain AS (
    -- Base case: direct ownership from natural persons
    SELECT
        e.cbu_id,
        e.from_entity_id AS person_id,
        e.to_entity_id AS owned_entity_id,
        e.percentage AS effective_percentage,
        1 AS depth,
        ARRAY[e.from_entity_id, e.to_entity_id] AS path
    FROM "ob-poc".ubo_edges e
    WHERE e.edge_type = 'ownership'
      AND e.status = 'proven'
      AND (e.effective_to IS NULL OR e.effective_to >= CURRENT_DATE)
      AND "ob-poc".is_natural_person(e.from_entity_id)

    UNION ALL

    -- Recursive: chain through shell entities
    SELECT
        oc.cbu_id,
        oc.person_id,
        e.to_entity_id AS owned_entity_id,
        (oc.effective_percentage * e.percentage / 100.0)::DECIMAL(5,2) AS effective_percentage,
        oc.depth + 1 AS depth,
        oc.path || e.to_entity_id
    FROM ownership_chain oc
    JOIN "ob-poc".ubo_edges e ON e.from_entity_id = oc.owned_entity_id
    WHERE e.edge_type = 'ownership'
      AND e.status = 'proven'
      AND e.cbu_id = oc.cbu_id
      AND (e.effective_to IS NULL OR e.effective_to >= CURRENT_DATE)
      AND e.to_entity_id != ALL(oc.path)  -- Prevent cycles
      AND oc.depth < 10  -- Max depth to prevent infinite recursion
)
SELECT
    oc.cbu_id,
    oc.person_id,
    p.name AS person_name,
    oc.owned_entity_id AS subject_entity_id,
    s.name AS subject_entity_name,
    SUM(oc.effective_percentage) AS total_ownership_percentage,
    MAX(oc.depth) AS max_chain_depth
FROM ownership_chain oc
JOIN "ob-poc".entities p ON p.entity_id = oc.person_id
JOIN "ob-poc".entities s ON s.entity_id = oc.owned_entity_id
GROUP BY oc.cbu_id, oc.person_id, p.name, oc.owned_entity_id, s.name
HAVING SUM(oc.effective_percentage) >= 25.0;

COMMENT ON VIEW "ob-poc".ubo_candidates IS 'Natural persons with >= 25% effective ownership (computed from proven edges)';
