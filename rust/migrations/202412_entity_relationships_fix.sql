-- ═══════════════════════════════════════════════════════════════════════════
-- FIX SCRIPT for entity_relationships migration
-- ═══════════════════════════════════════════════════════════════════════════

-- Fix the v_ubo_candidates view with proper type casting
DROP VIEW IF EXISTS "ob-poc".v_ubo_candidates;
CREATE VIEW "ob-poc".v_ubo_candidates AS
WITH RECURSIVE ownership_chain AS (
    -- Base: direct ownership edges that are verified for this CBU
    SELECT
        v.cbu_id,
        r.from_entity_id AS owned_entity_id,
        r.to_entity_id AS owner_entity_id,
        COALESCE(v.observed_percentage, v.alleged_percentage, r.percentage)::NUMERIC AS effective_percentage,
        1 AS depth,
        ARRAY[r.from_entity_id, r.to_entity_id] AS path
    FROM "ob-poc".entity_relationships r
    JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
    WHERE r.relationship_type = 'ownership'
      AND v.status IN ('proven', 'alleged', 'pending')
      AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)

    UNION ALL

    -- Recursive: follow the chain upward
    SELECT
        oc.cbu_id,
        oc.owned_entity_id,
        r.to_entity_id AS owner_entity_id,
        (oc.effective_percentage * COALESCE(v.observed_percentage, v.alleged_percentage, r.percentage)::NUMERIC / 100)::NUMERIC AS effective_percentage,
        oc.depth + 1,
        oc.path || r.to_entity_id
    FROM ownership_chain oc
    JOIN "ob-poc".entity_relationships r ON r.from_entity_id = oc.owner_entity_id
    JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
        AND v.cbu_id = oc.cbu_id
    WHERE r.relationship_type = 'ownership'
      AND v.status IN ('proven', 'alleged', 'pending')
      AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
      AND oc.depth < 10
      AND NOT r.to_entity_id = ANY(oc.path)
)
SELECT
    oc.cbu_id,
    oc.owner_entity_id AS entity_id,
    e.name AS entity_name,
    et.entity_category,
    et.name AS entity_type_name,
    SUM(oc.effective_percentage) AS total_effective_percentage,
    et.entity_category = 'PERSON' AS is_natural_person,
    et.entity_category = 'PERSON' AND SUM(oc.effective_percentage) >= 25 AS is_ubo
FROM ownership_chain oc
JOIN "ob-poc".entities e ON e.entity_id = oc.owner_entity_id
JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
GROUP BY oc.cbu_id, oc.owner_entity_id, e.name, et.entity_category, et.name
HAVING SUM(oc.effective_percentage) >= 25 OR et.entity_category = 'PERSON';


-- Drop and recreate the function with consistent parameter name
DROP FUNCTION IF EXISTS "ob-poc".is_natural_person(UUID);
CREATE FUNCTION "ob-poc".is_natural_person(p_entity_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE e.entity_id = p_entity_id
        AND et.entity_category = 'PERSON'
    );
END;
$$ LANGUAGE plpgsql STABLE;


-- Migrate from legacy ownership_relationships (without relationship_id column reference)
INSERT INTO "ob-poc".entity_relationships (
    from_entity_id,
    to_entity_id,
    relationship_type,
    percentage,
    ownership_type,
    effective_from,
    effective_to,
    source,
    created_at
)
SELECT
    owned_entity_id,
    owner_entity_id,
    'ownership',
    ownership_percent,
    COALESCE(ownership_type, 'direct'),
    effective_from,
    effective_to,
    'legacy_migration',
    COALESCE(created_at, NOW())
FROM "ob-poc".ownership_relationships
ON CONFLICT DO NOTHING;


-- Migrate from ubo_edges - insert relationships first
INSERT INTO "ob-poc".entity_relationships (
    from_entity_id,
    to_entity_id,
    relationship_type,
    percentage,
    control_type,
    trust_role,
    interest_type,
    effective_from,
    effective_to,
    source,
    created_at
)
SELECT
    ue.from_entity_id,
    ue.to_entity_id,
    ue.edge_type,
    COALESCE(ue.proven_percentage, ue.alleged_percentage, ue.percentage),
    ue.control_role,
    ue.trust_role,
    ue.interest_type,
    ue.effective_from,
    ue.effective_to,
    COALESCE(ue.allegation_source, 'ubo_edges_migration'),
    COALESCE(ue.created_at, NOW())
FROM "ob-poc".ubo_edges ue
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".entity_relationships er
    WHERE er.from_entity_id = ue.from_entity_id
      AND er.to_entity_id = ue.to_entity_id
      AND er.relationship_type = ue.edge_type
)
ON CONFLICT DO NOTHING;


-- Create verification records WITHOUT the proof_id (skip bad foreign keys)
INSERT INTO "ob-poc".cbu_relationship_verification (
    cbu_id,
    relationship_id,
    alleged_percentage,
    alleged_at,
    observed_percentage,
    status,
    created_at
)
SELECT
    ue.cbu_id,
    er.relationship_id,
    ue.alleged_percentage,
    ue.alleged_at,
    ue.proven_percentage,
    COALESCE(ue.status, 'unverified'),
    COALESCE(ue.created_at, NOW())
FROM "ob-poc".ubo_edges ue
JOIN "ob-poc".entity_relationships er
    ON er.from_entity_id = ue.from_entity_id
   AND er.to_entity_id = ue.to_entity_id
   AND er.relationship_type = ue.edge_type
WHERE ue.cbu_id IS NOT NULL
ON CONFLICT (cbu_id, relationship_id) DO NOTHING;
