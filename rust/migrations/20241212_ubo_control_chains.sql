-- Migration: Add control relationships to UBO chain computation
-- Date: 2024-12-12
-- Purpose: Extend UBO analysis to include control relationships (voting rights, board control, etc.)
--          Control relationships are UBO extensions per AML/KYC regulatory guidance

-- Drop the old function
DROP FUNCTION IF EXISTS "ob-poc".compute_ownership_chains(uuid, uuid, integer);

-- Create new function that traces both ownership AND control relationships
CREATE OR REPLACE FUNCTION "ob-poc".compute_ownership_chains(
    p_cbu_id uuid,
    p_target_entity_id uuid DEFAULT NULL::uuid,
    p_max_depth integer DEFAULT 10
)
RETURNS TABLE(
    chain_id integer,
    ubo_person_id uuid,
    ubo_name text,
    path_entities uuid[],
    path_names text[],
    ownership_percentages numeric[],
    effective_ownership numeric,
    chain_depth integer,
    is_complete boolean,
    relationship_types text[],
    has_control_path boolean
)
LANGUAGE sql STABLE
AS $$
WITH RECURSIVE ownership_chain AS (
    -- Base case: direct relationships from entities to CBU-linked entities
    -- Combines both ownership and control in a single base case
    SELECT
        ROW_NUMBER() OVER ()::INTEGER as chain_id,
        base.parent_entity_id as current_entity,
        base.child_entity_id as target_entity,
        ARRAY[base.parent_entity_id] as path,
        ARRAY[base.entity_name] as names,
        ARRAY[base.ownership_pct] as percentages,
        base.ownership_pct as effective_pct,
        1 as depth,
        base.is_person as owner_is_person,
        ARRAY[base.rel_type] as rel_types,
        base.is_control as has_control
    FROM (
        -- Ownership relationships
        SELECT
            o.owner_entity_id as parent_entity_id,
            o.owned_entity_id as child_entity_id,
            COALESCE(e.name, 'Unknown')::text as entity_name,
            o.ownership_percent::NUMERIC as ownership_pct,
            et.type_code = 'proper_person' as is_person,
            'OWNERSHIP'::text as rel_type,
            false as is_control
        FROM "ob-poc".ownership_relationships o
        JOIN "ob-poc".entities e ON o.owner_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        JOIN "ob-poc".cbu_entity_roles cer ON o.owned_entity_id = cer.entity_id
        WHERE cer.cbu_id = p_cbu_id
          AND (o.effective_to IS NULL OR o.effective_to > CURRENT_DATE)
          AND (p_target_entity_id IS NULL OR o.owned_entity_id = p_target_entity_id)

        UNION ALL

        -- Control relationships
        SELECT
            c.controller_entity_id as parent_entity_id,
            c.controlled_entity_id as child_entity_id,
            COALESCE(e.name, 'Unknown')::text as entity_name,
            NULL::NUMERIC as ownership_pct,
            et.type_code = 'proper_person' as is_person,
            c.control_type::text as rel_type,
            true as is_control
        FROM "ob-poc".control_relationships c
        JOIN "ob-poc".entities e ON c.controller_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        JOIN "ob-poc".cbu_entity_roles cer ON c.controlled_entity_id = cer.entity_id
        WHERE cer.cbu_id = p_cbu_id
          AND c.is_active = true
          AND (c.effective_to IS NULL OR c.effective_to > CURRENT_DATE)
          AND (p_target_entity_id IS NULL OR c.controlled_entity_id = p_target_entity_id)
    ) base

    UNION ALL

    -- Recursive case: follow chain upward using CROSS JOIN LATERAL
    SELECT
        oc.chain_id,
        combined.parent_entity_id,
        oc.target_entity,
        oc.path || combined.parent_entity_id,
        oc.names || combined.entity_name,
        oc.percentages || combined.ownership_pct,
        CASE
            WHEN oc.effective_pct IS NOT NULL AND combined.ownership_pct IS NOT NULL
            THEN (oc.effective_pct * combined.ownership_pct / 100)::NUMERIC
            ELSE oc.effective_pct
        END,
        oc.depth + 1,
        combined.is_person,
        oc.rel_types || combined.rel_type,
        oc.has_control OR combined.is_control
    FROM ownership_chain oc
    CROSS JOIN LATERAL (
        -- Ownership relationships
        SELECT
            o.owner_entity_id as parent_entity_id,
            COALESCE(e.name, 'Unknown')::text as entity_name,
            o.ownership_percent::NUMERIC as ownership_pct,
            et.type_code = 'proper_person' as is_person,
            'OWNERSHIP'::text as rel_type,
            false as is_control
        FROM "ob-poc".ownership_relationships o
        JOIN "ob-poc".entities e ON o.owner_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE o.owned_entity_id = oc.current_entity
          AND (o.effective_to IS NULL OR o.effective_to > CURRENT_DATE)
          AND NOT o.owner_entity_id = ANY(oc.path)

        UNION ALL

        -- Control relationships
        SELECT
            c.controller_entity_id as parent_entity_id,
            COALESCE(e.name, 'Unknown')::text as entity_name,
            NULL::NUMERIC as ownership_pct,
            et.type_code = 'proper_person' as is_person,
            c.control_type::text as rel_type,
            true as is_control
        FROM "ob-poc".control_relationships c
        JOIN "ob-poc".entities e ON c.controller_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE c.controlled_entity_id = oc.current_entity
          AND c.is_active = true
          AND (c.effective_to IS NULL OR c.effective_to > CURRENT_DATE)
          AND NOT c.controller_entity_id = ANY(oc.path)
    ) combined
    WHERE oc.depth < p_max_depth
)
SELECT
    chain_id,
    current_entity as ubo_person_id,
    names[array_length(names, 1)] as ubo_name,
    path as path_entities,
    names as path_names,
    percentages as ownership_percentages,
    effective_pct as effective_ownership,
    depth as chain_depth,
    owner_is_person as is_complete,
    rel_types as relationship_types,
    has_control as has_control_path
FROM ownership_chain
WHERE owner_is_person = true
ORDER BY
    CASE WHEN effective_pct IS NOT NULL THEN effective_pct ELSE 0 END DESC,
    chain_id;
$$;

-- Grant permissions
GRANT EXECUTE ON FUNCTION "ob-poc".compute_ownership_chains(uuid, uuid, integer) TO PUBLIC;

COMMENT ON FUNCTION "ob-poc".compute_ownership_chains IS
'Computes ownership and control chains from CBU entities to ultimate beneficial owners (UBOs).
Traces both:
- Ownership relationships (percentage-based ownership)
- Control relationships (voting rights, board control, veto powers, etc.)

Control relationships are UBO extensions per AML/KYC regulatory guidance - a person may be
a beneficial owner through control even without direct ownership percentage.

Returns all chains terminating at natural persons (proper_person entity type).
The has_control_path flag indicates if the chain includes any control relationships.';
