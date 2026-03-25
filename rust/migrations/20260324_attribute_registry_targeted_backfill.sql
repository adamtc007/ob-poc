WITH active_attr_snapshots AS (
    SELECT DISTINCT ON ((definition->>'fqn'))
        definition->>'fqn' AS fqn,
        snapshot_id,
        definition
    FROM sem_reg.snapshots
    WHERE object_type = 'attribute_def'::sem_reg.object_type
      AND status = 'active'::sem_reg.snapshot_status
      AND effective_until IS NULL
      AND definition ? 'fqn'
    ORDER BY (definition->>'fqn'), effective_from DESC, created_at DESC
),
safe_candidates AS (
    SELECT
        ar.id,
        ar.uuid,
        regexp_replace(regexp_replace(ar.id, '^attr\.', ''), '_', '-', 'g') AS candidate_fqn,
        'drop_attr_prefix'::text AS strategy
    FROM "ob-poc".attribute_registry ar
    WHERE ar.sem_reg_snapshot_id IS NULL

    UNION ALL

    SELECT
        ar.id,
        ar.uuid,
        regexp_replace(
            regexp_replace(ar.id, '^entity\.(company|person|partnership|trust)\.', 'entity.'),
            '_',
            '-',
            'g'
        ) AS candidate_fqn,
        'entity_subtype_collapse'::text AS strategy
    FROM "ob-poc".attribute_registry ar
    WHERE ar.sem_reg_snapshot_id IS NULL
      AND ar.id ~ '^entity\.(company|person|partnership|trust)\.'
),
matched AS (
    SELECT
        sc.id,
        sc.uuid,
        sc.candidate_fqn,
        sc.strategy,
        aas.snapshot_id,
        aas.definition
    FROM safe_candidates sc
    JOIN active_attr_snapshots aas
      ON aas.fqn = sc.candidate_fqn
),
deduped AS (
    SELECT DISTINCT ON (id)
        id,
        uuid,
        candidate_fqn,
        strategy,
        snapshot_id,
        definition
    FROM matched
    ORDER BY id, strategy
)
UPDATE "ob-poc".attribute_registry ar
SET sem_reg_snapshot_id = d.snapshot_id,
    is_derived = COALESCE((d.definition #>> '{source,derived}')::boolean, false),
    derivation_spec_fqn = CASE
        WHEN COALESCE((d.definition #>> '{source,derived}')::boolean, false)
            THEN d.definition->>'fqn'
        ELSE NULL
    END,
    evidence_grade = COALESCE(NULLIF(d.definition->>'evidence_grade', ''), ar.evidence_grade, 'none'),
    metadata = jsonb_set(
        COALESCE(ar.metadata, '{}'::jsonb),
        '{sem_os}',
        COALESCE(ar.metadata->'sem_os', '{}'::jsonb) ||
        jsonb_strip_nulls(
            jsonb_build_object(
                'attribute_fqn', d.candidate_fqn,
                'snapshot_id', d.snapshot_id::text,
                'evidence_grade', COALESCE(NULLIF(d.definition->>'evidence_grade', ''), ar.evidence_grade, 'none'),
                'derived', COALESCE((d.definition #>> '{source,derived}')::boolean, false),
                'backfill_strategy', d.strategy
            )
        ),
        true
    ),
    updated_at = NOW()
FROM deduped d
WHERE ar.id = d.id
  AND ar.sem_reg_snapshot_id IS NULL;
