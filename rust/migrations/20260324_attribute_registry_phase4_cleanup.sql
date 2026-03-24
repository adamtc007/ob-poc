ALTER TABLE "ob-poc".attribute_registry
    ADD COLUMN IF NOT EXISTS sem_reg_snapshot_id uuid,
    ADD COLUMN IF NOT EXISTS is_derived boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS derivation_spec_fqn text,
    ADD COLUMN IF NOT EXISTS evidence_grade text NOT NULL DEFAULT 'none';

ALTER TABLE "ob-poc".attribute_registry
    DROP CONSTRAINT IF EXISTS check_value_type,
    DROP CONSTRAINT IF EXISTS chk_derived_has_spec,
    DROP CONSTRAINT IF EXISTS chk_evidence_grade;

ALTER TABLE "ob-poc".attribute_registry
    ADD CONSTRAINT check_value_type CHECK (
        value_type = ANY (
            ARRAY[
                'string',
                'integer',
                'number',
                'decimal',
                'boolean',
                'date',
                'datetime',
                'timestamp',
                'uuid',
                'email',
                'phone',
                'address',
                'currency',
                'percentage',
                'tax_id',
                'json',
                'enum'
            ]
        )
    ),
    ADD CONSTRAINT chk_derived_has_spec CHECK (
        (NOT is_derived AND derivation_spec_fqn IS NULL)
        OR (is_derived AND derivation_spec_fqn IS NOT NULL)
    ),
    ADD CONSTRAINT chk_evidence_grade CHECK (
        evidence_grade = ANY (
            ARRAY[
                'none',
                'prohibited',
                'allowed_with_constraints',
                'regulatory_evidence'
            ]
        )
    );

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
active_derivation_snapshots AS (
    SELECT DISTINCT ON ((definition->>'fqn'))
        definition->>'fqn' AS fqn,
        snapshot_id,
        definition
    FROM sem_reg.snapshots
    WHERE object_type = 'derivation_spec'::sem_reg.object_type
      AND status = 'active'::sem_reg.snapshot_status
      AND effective_until IS NULL
      AND definition ? 'fqn'
    ORDER BY (definition->>'fqn'), effective_from DESC, created_at DESC
)
UPDATE "ob-poc".attribute_registry ar
SET sem_reg_snapshot_id = COALESCE(
        NULLIF(ar.metadata #>> '{sem_os,snapshot_id}', '')::uuid,
        aas.snapshot_id,
        ar.sem_reg_snapshot_id
    ),
    is_derived = COALESCE(
        (ar.metadata #>> '{sem_os,derived}')::boolean,
        (ar.metadata #>> '{sem_os,lineage_plane}') = 'below_line',
        (aas.definition #>> '{source,derived}')::boolean,
        ar.is_derived,
        false
    ),
    derivation_spec_fqn = COALESCE(
        NULLIF(ar.derivation_spec_fqn, ''),
        NULLIF(ar.metadata #>> '{sem_os,derivation_spec_fqn}', ''),
        CASE
            WHEN COALESCE(
                (ar.metadata #>> '{sem_os,derived}')::boolean,
                (ar.metadata #>> '{sem_os,lineage_plane}') = 'below_line',
                (aas.definition #>> '{source,derived}')::boolean,
                false
            ) THEN COALESCE(NULLIF(aas.definition->>'fqn', ''), ar.id)
            ELSE NULL
        END
    ),
    evidence_grade = COALESCE(
        NULLIF(ar.evidence_grade, ''),
        NULLIF(ar.metadata #>> '{sem_os,evidence_grade}', ''),
        NULLIF(aas.definition->>'evidence_grade', ''),
        'none'
    ),
    metadata = jsonb_set(
        COALESCE(ar.metadata, '{}'::jsonb),
        '{sem_os}',
        COALESCE(ar.metadata->'sem_os', '{}'::jsonb) ||
        jsonb_strip_nulls(
            jsonb_build_object(
                'attribute_fqn', COALESCE(NULLIF(ar.metadata #>> '{sem_os,attribute_fqn}', ''), aas.fqn, ar.id),
                'snapshot_id', COALESCE(NULLIF(ar.metadata #>> '{sem_os,snapshot_id}', ''), aas.snapshot_id::text),
                'derivation_spec_fqn', COALESCE(NULLIF(ar.metadata #>> '{sem_os,derivation_spec_fqn}', ''), ads.fqn),
                'evidence_grade', COALESCE(NULLIF(ar.metadata #>> '{sem_os,evidence_grade}', ''), aas.definition->>'evidence_grade', ar.evidence_grade),
                'derived', COALESCE(
                    (ar.metadata #>> '{sem_os,derived}')::boolean,
                    (ar.metadata #>> '{sem_os,lineage_plane}') = 'below_line',
                    (aas.definition #>> '{source,derived}')::boolean,
                    false
                )
            )
        ),
        true
    )
FROM active_attr_snapshots aas
LEFT JOIN active_derivation_snapshots ads ON ads.fqn = aas.fqn
WHERE COALESCE(ar.metadata #>> '{sem_os,attribute_fqn}', ar.id) = aas.fqn
   OR ar.id = aas.fqn;

ALTER TABLE "ob-poc".attribute_registry
    DROP COLUMN IF EXISTS embedding,
    DROP COLUMN IF EXISTS embedding_model,
    DROP COLUMN IF EXISTS embedding_updated_at,
    DROP COLUMN IF EXISTS reconciliation_rules,
    DROP COLUMN IF EXISTS acceptable_variation_threshold,
    DROP COLUMN IF EXISTS requires_authoritative_source;

CREATE OR REPLACE VIEW "ob-poc".v_attribute_lineage_summary AS
SELECT
    ar.id AS attribute_id,
    ar.display_name,
    ar.category,
    COUNT(DISTINCT CASE WHEN dal.direction IN ('SOURCE', 'BOTH') THEN dal.document_type_id END) AS source_count,
    COUNT(DISTINCT CASE WHEN dal.direction IN ('SINK', 'BOTH') THEN dal.document_type_id END) AS sink_count,
    COUNT(DISTINCT rar.resource_id) AS resource_count,
    BOOL_OR(COALESCE(dal.is_authoritative, false)) AS has_authoritative_source
FROM "ob-poc".attribute_registry ar
LEFT JOIN "ob-poc".document_attribute_links dal ON dal.attribute_id = ar.uuid
LEFT JOIN "ob-poc".resource_attribute_requirements rar ON rar.attribute_id = ar.uuid
GROUP BY ar.id, ar.display_name, ar.category;

CREATE OR REPLACE VIEW "ob-poc".v_attribute_registry_reconciled AS
WITH active_attr_snapshots AS (
    SELECT DISTINCT ON ((definition->>'fqn'))
        definition->>'fqn' AS fqn,
        snapshot_id,
        version_major,
        version_minor,
        status,
        governance_tier,
        definition
    FROM sem_reg.snapshots
    WHERE object_type = 'attribute_def'::sem_reg.object_type
      AND status = 'active'::sem_reg.snapshot_status
      AND effective_until IS NULL
      AND definition ? 'fqn'
    ORDER BY (definition->>'fqn'), effective_from DESC, created_at DESC
),
active_derivation_snapshots AS (
    SELECT DISTINCT ON ((definition->>'fqn'))
        definition->>'fqn' AS fqn,
        snapshot_id,
        definition
    FROM sem_reg.snapshots
    WHERE object_type = 'derivation_spec'::sem_reg.object_type
      AND status = 'active'::sem_reg.snapshot_status
      AND effective_until IS NULL
      AND definition ? 'fqn'
    ORDER BY (definition->>'fqn'), effective_from DESC, created_at DESC
),
observation_counts AS (
    SELECT attribute_id, COUNT(*)::bigint AS active_observations
    FROM "ob-poc".attribute_observations
    WHERE status = 'ACTIVE'
    GROUP BY attribute_id
),
cbu_value_counts AS (
    SELECT attr_id AS attribute_id, COUNT(*)::bigint AS cbu_values
    FROM "ob-poc".cbu_attr_values
    GROUP BY attr_id
),
document_source_counts AS (
    SELECT attribute_id, COUNT(*)::bigint AS document_sources
    FROM "ob-poc".document_attribute_links
    WHERE direction IN ('SOURCE', 'BOTH')
    GROUP BY attribute_id
)
SELECT
    ar.id AS registry_id,
    COALESCE(ar.metadata #>> '{sem_os,attribute_fqn}', aas.fqn, ar.id) AS fqn,
    ar.uuid,
    ar.display_name,
    ar.category,
    ar.value_type,
    ar.domain,
    ar.sem_reg_snapshot_id,
    ar.is_derived,
    ar.derivation_spec_fqn,
    ar.evidence_grade,
    ar.metadata,
    aas.snapshot_id AS attribute_snapshot_id,
    (aas.version_major::text || '.' || aas.version_minor::text) AS attribute_snapshot_version,
    aas.status::text AS attribute_snapshot_status,
    aas.governance_tier::text AS governance_tier,
    aas.definition AS attribute_definition,
    aas.definition->'source' AS attribute_source,
    aas.definition->'constraints' AS attribute_constraints,
    ads.snapshot_id AS derivation_snapshot_id,
    ads.definition AS derivation_definition,
    COALESCE(oc.active_observations, 0) AS active_observations,
    COALESCE(cv.cbu_values, 0) AS cbu_values,
    COALESCE(ds.document_sources, 0) AS document_sources
FROM "ob-poc".attribute_registry ar
LEFT JOIN active_attr_snapshots aas
    ON aas.fqn = COALESCE(ar.metadata #>> '{sem_os,attribute_fqn}', ar.id)
LEFT JOIN active_derivation_snapshots ads
    ON ads.fqn = COALESCE(ar.derivation_spec_fqn, ar.metadata #>> '{sem_os,derivation_spec_fqn}')
LEFT JOIN observation_counts oc ON oc.attribute_id = ar.uuid
LEFT JOIN cbu_value_counts cv ON cv.attribute_id = ar.uuid
LEFT JOIN document_source_counts ds ON ds.attribute_id = ar.uuid;

CREATE OR REPLACE VIEW "ob-poc".v_attribute_reconciliation_summary AS
SELECT
    COUNT(*)::bigint AS total_attributes,
    COUNT(*) FILTER (WHERE sem_reg_snapshot_id IS NOT NULL)::bigint AS bridged_to_sem_reg,
    COUNT(*) FILTER (WHERE is_derived)::bigint AS derived_attributes,
    COUNT(*) FILTER (WHERE is_derived AND derivation_spec_fqn IS NULL)::bigint AS derived_missing_spec,
    COUNT(*) FILTER (WHERE sem_reg_snapshot_id IS NULL)::bigint AS missing_snapshot_link,
    COUNT(*) FILTER (WHERE COALESCE(metadata #>> '{sem_os,attribute_fqn}', '') = '')::bigint AS missing_attribute_fqn,
    COUNT(*) FILTER (WHERE evidence_grade = 'none')::bigint AS evidence_grade_none,
    COUNT(*) FILTER (WHERE evidence_grade <> 'none')::bigint AS evidence_grade_governed
FROM "ob-poc".attribute_registry;
