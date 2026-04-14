-- Migration: Add temporal support to compute_ownership_chains function
-- This creates a new version that accepts an as_of_date parameter

-- Drop the existing function first
DROP FUNCTION IF EXISTS "ob-poc".compute_ownership_chains(uuid, uuid, integer);

-- Create the new version with as_of_date parameter
CREATE OR REPLACE FUNCTION "ob-poc".compute_ownership_chains(
    p_cbu_id uuid,
    p_target_entity_id uuid DEFAULT NULL::uuid,
    p_max_depth integer DEFAULT 10,
    p_as_of_date date DEFAULT CURRENT_DATE
) RETURNS TABLE(
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
LANGUAGE plpgsql STABLE
AS $$
BEGIN
    -- Fresh test databases may not have the later relationship tables yet.
    -- Return an empty set until those migrations have been applied.
    IF to_regclass('"ob-poc".entity_relationships') IS NULL
       OR to_regclass('"ob-poc".entities') IS NULL
       OR to_regclass('"ob-poc".entity_types') IS NULL
       OR to_regclass('"ob-poc".cbu_entity_roles') IS NULL THEN
        RETURN;
    END IF;

    RETURN QUERY EXECUTE $query$
    WITH RECURSIVE ownership_chain AS (
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
            SELECT
                r.from_entity_id as parent_entity_id,
                r.to_entity_id as child_entity_id,
                COALESCE(e.name, 'Unknown')::text as entity_name,
                r.percentage::NUMERIC as ownership_pct,
                et.type_code = 'proper_person' as is_person,
                UPPER(r.relationship_type)::text as rel_type,
                r.relationship_type != 'ownership' as is_control
            FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            JOIN "ob-poc".cbu_entity_roles cer ON r.to_entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
              AND (r.effective_from IS NULL OR r.effective_from <= $4)
              AND (r.effective_to IS NULL OR r.effective_to >= $4)
              AND (cer.effective_from IS NULL OR cer.effective_from <= $4)
              AND (cer.effective_to IS NULL OR cer.effective_to >= $4)
              AND ($2 IS NULL OR r.to_entity_id = $2)
        ) base

        UNION ALL

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
            SELECT
                r.from_entity_id as parent_entity_id,
                COALESCE(e.name, 'Unknown')::text as entity_name,
                r.percentage::NUMERIC as ownership_pct,
                et.type_code = 'proper_person' as is_person,
                UPPER(r.relationship_type)::text as rel_type,
                r.relationship_type != 'ownership' as is_control
            FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE r.to_entity_id = oc.current_entity
              AND (r.effective_from IS NULL OR r.effective_from <= $4)
              AND (r.effective_to IS NULL OR r.effective_to >= $4)
              AND NOT (r.from_entity_id = ANY(oc.path))
        ) combined
        WHERE oc.depth < $3
          AND NOT oc.owner_is_person
    )
    SELECT
        oc.chain_id,
        oc.current_entity as ubo_person_id,
        oc.names[array_length(oc.names, 1)] as ubo_name,
        oc.path as path_entities,
        oc.names as path_names,
        oc.percentages as ownership_percentages,
        oc.effective_pct as effective_ownership,
        oc.depth as chain_depth,
        oc.owner_is_person as is_complete,
        oc.rel_types as relationship_types,
        oc.has_control as has_control_path
    FROM ownership_chain oc
    WHERE oc.owner_is_person
       OR oc.depth = $3
    ORDER BY oc.effective_pct DESC NULLS LAST, oc.chain_id
    $query$
    USING p_cbu_id, p_target_entity_id, p_max_depth, p_as_of_date;
END;
$$;

COMMENT ON FUNCTION "ob-poc".compute_ownership_chains(uuid, uuid, integer, date) IS
'Computes ownership and control chains from CBU entities to natural persons.
Supports point-in-time queries via p_as_of_date parameter (defaults to today).
Returns chains with effective ownership percentages and relationship types.';
