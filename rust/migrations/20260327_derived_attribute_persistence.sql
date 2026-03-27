CREATE TABLE IF NOT EXISTS "ob-poc".derived_attribute_values (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attr_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    entity_id UUID NOT NULL,
    entity_type TEXT NOT NULL,
    value JSONB NOT NULL,
    derivation_spec_fqn TEXT NOT NULL,
    spec_snapshot_id UUID NOT NULL,
    content_hash TEXT NOT NULL,
    input_values JSONB NOT NULL DEFAULT '{}'::jsonb,
    inherited_security_label JSONB,
    dependency_depth INTEGER NOT NULL DEFAULT 0,
    evaluated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    stale BOOLEAN NOT NULL DEFAULT false,
    superseded_by UUID REFERENCES "ob-poc".derived_attribute_values(id),
    superseded_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT chk_derived_attribute_supersession_pair
        CHECK (
            (superseded_by IS NULL AND superseded_at IS NULL)
            OR (superseded_by IS NOT NULL AND superseded_at IS NOT NULL)
        ),
    CONSTRAINT chk_derived_attribute_not_self_superseded
        CHECK (superseded_by IS NULL OR superseded_by <> id)
);

CREATE TABLE IF NOT EXISTS "ob-poc".derived_attribute_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    derived_value_id UUID NOT NULL REFERENCES "ob-poc".derived_attribute_values(id) ON DELETE CASCADE,
    input_kind TEXT NOT NULL CHECK (input_kind IN ('observation', 'derived_value')),
    input_attr_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    input_entity_id UUID NOT NULL,
    input_source_row_id UUID,
    dependency_role TEXT,
    resolved_value JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_derived_attribute_values_current
    ON "ob-poc".derived_attribute_values (entity_type, entity_id, attr_id)
    WHERE superseded_by IS NULL;

CREATE INDEX IF NOT EXISTS idx_derived_attribute_values_stale_queue
    ON "ob-poc".derived_attribute_values (stale, entity_type, dependency_depth, evaluated_at)
    WHERE superseded_by IS NULL AND stale = true;

CREATE INDEX IF NOT EXISTS idx_derived_attribute_values_history
    ON "ob-poc".derived_attribute_values (entity_type, entity_id, attr_id, evaluated_at DESC);

CREATE INDEX IF NOT EXISTS idx_derived_attribute_values_spec_lookup
    ON "ob-poc".derived_attribute_values (derivation_spec_fqn)
    WHERE superseded_by IS NULL;

CREATE INDEX IF NOT EXISTS idx_derived_attribute_values_entity_scope
    ON "ob-poc".derived_attribute_values (entity_type, entity_id)
    WHERE superseded_by IS NULL;

CREATE INDEX IF NOT EXISTS idx_derived_attribute_dependencies_staleness
    ON "ob-poc".derived_attribute_dependencies (input_attr_id, input_entity_id);

CREATE INDEX IF NOT EXISTS idx_derived_attribute_dependencies_value
    ON "ob-poc".derived_attribute_dependencies (derived_value_id);

CREATE INDEX IF NOT EXISTS idx_derived_attribute_dependencies_source_row
    ON "ob-poc".derived_attribute_dependencies (input_source_row_id)
    WHERE input_source_row_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_derived_attribute_dependencies_dedupe
    ON "ob-poc".derived_attribute_dependencies (
        derived_value_id,
        input_kind,
        input_attr_id,
        input_entity_id,
        COALESCE(input_source_row_id, '00000000-0000-0000-0000-000000000000'::uuid),
        COALESCE(dependency_role, '')
    );

CREATE OR REPLACE FUNCTION "ob-poc".validate_derived_attr_id()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM "ob-poc".attribute_registry
        WHERE uuid = NEW.attr_id
          AND is_derived = true
    ) THEN
        RAISE EXCEPTION 'Attribute % is not marked as derived in attribute_registry', NEW.attr_id;
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_validate_derived_attr_id ON "ob-poc".derived_attribute_values;

CREATE TRIGGER trg_validate_derived_attr_id
    BEFORE INSERT ON "ob-poc".derived_attribute_values
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".validate_derived_attr_id();

CREATE OR REPLACE FUNCTION "ob-poc".propagate_observation_staleness()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    UPDATE "ob-poc".derived_attribute_values dav
    SET stale = true
    WHERE dav.superseded_by IS NULL
      AND dav.stale = false
      AND EXISTS (
          SELECT 1
          FROM "ob-poc".derived_attribute_dependencies dad
          WHERE dad.derived_value_id = dav.id
            AND dad.input_attr_id = NEW.attribute_id
            AND dad.input_entity_id = NEW.entity_id
      );

    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION "ob-poc".propagate_derived_chain_staleness(
    p_attr_id UUID,
    p_entity_id UUID
)
RETURNS bigint
LANGUAGE plpgsql
AS $$
DECLARE
    affected bigint;
BEGIN
    UPDATE "ob-poc".derived_attribute_values dav
    SET stale = true
    WHERE dav.superseded_by IS NULL
      AND dav.stale = false
      AND EXISTS (
          SELECT 1
          FROM "ob-poc".derived_attribute_dependencies dad
          WHERE dad.derived_value_id = dav.id
            AND dad.input_attr_id = p_attr_id
            AND dad.input_entity_id = p_entity_id
      );

    GET DIAGNOSTICS affected = ROW_COUNT;
    RETURN affected;
END;
$$;

CREATE OR REPLACE FUNCTION "ob-poc".propagate_spec_staleness(
    p_spec_fqn TEXT,
    p_new_snapshot_id UUID
)
RETURNS bigint
LANGUAGE plpgsql
AS $$
DECLARE
    affected bigint;
BEGIN
    UPDATE "ob-poc".derived_attribute_values
    SET stale = true
    WHERE derivation_spec_fqn = p_spec_fqn
      AND spec_snapshot_id <> p_new_snapshot_id
      AND superseded_by IS NULL
      AND stale = false;

    GET DIAGNOSTICS affected = ROW_COUNT;
    RETURN affected;
END;
$$;

DROP TRIGGER IF EXISTS trg_propagate_observation_staleness ON "ob-poc".attribute_observations;

CREATE TRIGGER trg_propagate_observation_staleness
    AFTER INSERT OR UPDATE OF value_text, value_number, value_boolean, value_date, value_datetime, value_json, status, superseded_at
    ON "ob-poc".attribute_observations
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".propagate_observation_staleness();

CREATE OR REPLACE VIEW "ob-poc".v_derived_latest AS
SELECT
    dav.id,
    dav.attr_id,
    dav.entity_id,
    dav.entity_type,
    dav.value,
    dav.derivation_spec_fqn,
    dav.spec_snapshot_id,
    dav.content_hash,
    dav.input_values,
    dav.inherited_security_label,
    dav.dependency_depth,
    dav.evaluated_at,
    dav.stale,
    dav.superseded_by,
    dav.superseded_at,
    dav.created_at
FROM "ob-poc".derived_attribute_values dav
WHERE dav.superseded_by IS NULL;

CREATE OR REPLACE VIEW "ob-poc".v_derived_current AS
SELECT *
FROM "ob-poc".v_derived_latest
WHERE stale = false;

CREATE OR REPLACE VIEW "ob-poc".v_derived_recompute_queue AS
SELECT
    dav.id,
    dav.attr_id,
    dav.entity_id,
    dav.entity_type,
    dav.value,
    dav.derivation_spec_fqn,
    dav.spec_snapshot_id,
    dav.content_hash,
    dav.input_values,
    dav.inherited_security_label,
    dav.dependency_depth,
    dav.evaluated_at,
    dav.stale,
    dav.superseded_by,
    dav.superseded_at,
    dav.created_at
FROM "ob-poc".v_derived_latest dav
JOIN "ob-poc".attribute_registry ar
  ON ar.uuid = dav.attr_id
WHERE dav.stale = true
ORDER BY
    CASE ar.evidence_grade
        WHEN 'regulatory_evidence' THEN 1
        WHEN 'allowed_with_constraints' THEN 2
        WHEN 'prohibited' THEN 3
        ELSE 4
    END,
    dav.dependency_depth ASC,
    dav.evaluated_at ASC;

CREATE OR REPLACE VIEW "ob-poc".v_cbu_derived_values AS
SELECT
    dav.entity_id AS cbu_id,
    dav.attr_id,
    dav.value,
    'derived'::text AS source,
    COALESCE(
        jsonb_agg(
            DISTINCT jsonb_build_object(
                'type', dad.input_kind,
                'id', dad.input_source_row_id,
                'path', dad.dependency_role,
                'details', jsonb_build_object(
                    'input_attr_id', dad.input_attr_id,
                    'input_entity_id', dad.input_entity_id,
                    'resolved_value', dad.resolved_value
                )
            )
        ) FILTER (WHERE dad.id IS NOT NULL),
        '[]'::jsonb
    ) AS evidence_refs,
    jsonb_build_array(
        jsonb_build_object(
            'rule', format('derivation:%s', dav.derivation_spec_fqn),
            'input', jsonb_build_object(
                'spec_snapshot_id', dav.spec_snapshot_id,
                'input_values', dav.input_values,
                'dependency_depth', dav.dependency_depth
            ),
            'output', jsonb_build_object(
                'value', dav.value,
                'evaluated_at', dav.evaluated_at,
                'inherited_security_label', dav.inherited_security_label
            )
        )
    ) AS explain_refs,
    dav.evaluated_at AS as_of,
    dav.created_at,
    dav.created_at AS updated_at
FROM "ob-poc".v_derived_current dav
LEFT JOIN "ob-poc".derived_attribute_dependencies dad
  ON dad.derived_value_id = dav.id
WHERE dav.entity_type = 'cbu'
GROUP BY dav.id, dav.entity_id, dav.attr_id, dav.value, dav.derivation_spec_fqn,
         dav.spec_snapshot_id, dav.input_values, dav.dependency_depth,
         dav.inherited_security_label, dav.evaluated_at, dav.created_at;

CREATE OR REPLACE VIEW "ob-poc".v_cbu_attr_gaps AS
WITH effective_cbu_values AS (
    SELECT cbu_id, attr_id, value, source, as_of
    FROM "ob-poc".cbu_attr_values
    WHERE source <> 'derived'

    UNION ALL

    SELECT cbu_id, attr_id, value, source, as_of
    FROM "ob-poc".v_cbu_derived_values
)
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    r.attr_id,
    ar.id AS attr_code,
    ar.display_name AS attr_name,
    ar.category AS attr_category,
    r.requirement_strength,
    r.preferred_source,
    r.required_by_srdefs,
    r.conflict,
    (v.value IS NOT NULL) AS has_value,
    v.source AS value_source,
    v.as_of AS value_as_of
FROM "ob-poc".cbu_unified_attr_requirements r
JOIN "ob-poc".cbus c
  ON c.cbu_id = r.cbu_id
JOIN "ob-poc".attribute_registry ar
  ON ar.uuid = r.attr_id
LEFT JOIN effective_cbu_values v
  ON v.cbu_id = r.cbu_id
 AND v.attr_id = r.attr_id
WHERE r.requirement_strength = 'required';

CREATE OR REPLACE VIEW "ob-poc".v_cbu_attr_summary AS
WITH effective_cbu_values AS (
    SELECT cbu_id, attr_id, value
    FROM "ob-poc".cbu_attr_values
    WHERE source <> 'derived'

    UNION ALL

    SELECT cbu_id, attr_id, value
    FROM "ob-poc".v_cbu_derived_values
)
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    COUNT(*) AS total_required,
    COUNT(v.value) AS populated,
    COUNT(*) - COUNT(v.value) AS missing,
    COUNT(*) FILTER (WHERE r.conflict IS NOT NULL) AS conflicts,
    ROUND(((100.0 * COUNT(v.value)::numeric) / NULLIF(COUNT(*), 0)::numeric), 1) AS pct_complete
FROM "ob-poc".cbu_unified_attr_requirements r
JOIN "ob-poc".cbus c
  ON c.cbu_id = r.cbu_id
LEFT JOIN effective_cbu_values v
  ON v.cbu_id = r.cbu_id
 AND v.attr_id = r.attr_id
WHERE r.requirement_strength = 'required'
GROUP BY r.cbu_id, c.name;
