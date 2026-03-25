COPY (
    WITH active_attr AS (
        SELECT DISTINCT ON ((definition->>'fqn'))
            definition->>'fqn' AS fqn,
            definition->>'domain' AS domain,
            definition->>'name' AS name
        FROM sem_reg.snapshots
        WHERE object_type = 'attribute_def'::sem_reg.object_type
          AND status = 'active'
          AND effective_until IS NULL
          AND definition ? 'fqn'
        ORDER BY (definition->>'fqn'), effective_from DESC, created_at DESC
    )
    SELECT
        ar.id AS registry_id,
        ar.display_name,
        ar.category,
        ar.value_type,
        ar.domain,
        ar.uuid,
        ar.metadata,
        (
            SELECT string_agg(a.fqn, ' | ' ORDER BY a.fqn)
            FROM active_attr a
            WHERE (a.domain = ar.domain OR a.domain = ar.category)
              AND a.name ILIKE '%' || split_part(ar.id, '.', array_length(string_to_array(ar.id, '.'), 1)) || '%'
        ) AS candidate_fqns,
        CASE
            WHEN ar.domain IS NULL THEN 'NEEDS_DOMAIN: assign domain before reconciliation'
            WHEN ar.id NOT LIKE 'attr.%' AND ar.id NOT LIKE '%.%' THEN 'NEEDS_FQN: bare id needs domain.name convention'
            ELSE 'NEEDS_REVIEW: ambiguous mapping'
        END AS action_needed
    FROM "ob-poc".attribute_registry ar
    WHERE ar.sem_reg_snapshot_id IS NULL
      AND COALESCE(ar.metadata #>> '{sem_os,reconciliation_status}', '') <> 'out_of_scope'
    ORDER BY action_needed, ar.category, ar.id
) TO STDOUT WITH (FORMAT csv, DELIMITER E'\t', HEADER true);
