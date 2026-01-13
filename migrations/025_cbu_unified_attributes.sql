-- Migration 025: CBU Unified Attribute Requirements + Values
--
-- Adds:
-- 1. cbu_unified_attr_requirements - rolled up attribute requirements per CBU
-- 2. cbu_attr_values - CBU-level attribute values (populated from various sources)
--
-- These tables are DERIVED from the discovery engine output.
-- They can be rebuilt by re-running the rollup algorithm.
--
-- Part of CBU Resource Pipeline implementation

-- =============================================================================
-- 1. CBU UNIFIED ATTRIBUTE REQUIREMENTS
-- =============================================================================
-- Roll-up of all attribute requirements across all discovered SRDEFs for a CBU.
-- De-duped by attr_id, with merged constraints and requirement strength.

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_unified_attr_requirements (
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    attr_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid) ON DELETE CASCADE,

    -- Merged requirement (required dominates optional)
    requirement_strength TEXT NOT NULL DEFAULT 'required'
        CHECK (requirement_strength IN ('required', 'optional', 'conditional')),

    -- Merged constraints from all SRDEFs requiring this attribute
    merged_constraints JSONB NOT NULL DEFAULT '{}',

    -- Preferred population source (ordered by priority from source_policy)
    preferred_source TEXT
        CHECK (preferred_source IN ('derived', 'entity', 'cbu', 'document', 'manual', 'external')),

    -- Which SRDEFs require this attribute (for explainability)
    required_by_srdefs JSONB NOT NULL DEFAULT '[]',

    -- Conflict detection: set if constraint merge was impossible
    conflict JSONB,  -- null = no conflict, else {type: "...", details: "..."}

    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (cbu_id, attr_id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cbu_unified_attr_cbu
    ON "ob-poc".cbu_unified_attr_requirements(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_unified_attr_required
    ON "ob-poc".cbu_unified_attr_requirements(cbu_id, requirement_strength)
    WHERE requirement_strength = 'required';
CREATE INDEX IF NOT EXISTS idx_cbu_unified_attr_conflicts
    ON "ob-poc".cbu_unified_attr_requirements(cbu_id)
    WHERE conflict IS NOT NULL;

-- Updated_at trigger
CREATE OR REPLACE FUNCTION "ob-poc".update_cbu_unified_attr_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_cbu_unified_attr_updated ON "ob-poc".cbu_unified_attr_requirements;
CREATE TRIGGER trg_cbu_unified_attr_updated
    BEFORE UPDATE ON "ob-poc".cbu_unified_attr_requirements
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_cbu_unified_attr_timestamp();

COMMENT ON TABLE "ob-poc".cbu_unified_attr_requirements IS
    'Rolled-up attribute requirements per CBU. Derived from discovered SRDEFs. Rebuildable.';
COMMENT ON COLUMN "ob-poc".cbu_unified_attr_requirements.required_by_srdefs IS
    'Array of srdef_ids that require this attribute. For explainability.';
COMMENT ON COLUMN "ob-poc".cbu_unified_attr_requirements.conflict IS
    'Non-null if constraints from multiple SRDEFs could not be merged.';

-- =============================================================================
-- 2. CBU ATTRIBUTE VALUES
-- =============================================================================
-- CBU-level attribute values populated from various sources.
-- Separate from resource_instance_attributes which are per-instance.

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_attr_values (
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    attr_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid) ON DELETE CASCADE,

    -- The actual value (JSONB for flexibility)
    value JSONB NOT NULL,

    -- Where did this value come from?
    source TEXT NOT NULL
        CHECK (source IN ('derived', 'entity', 'cbu', 'document', 'manual', 'external')),

    -- Evidence trail
    evidence_refs JSONB NOT NULL DEFAULT '[]',  -- [{type: "document", id: "..."}, ...]

    -- Explainability (how was this derived/sourced?)
    explain_refs JSONB NOT NULL DEFAULT '[]',

    -- Temporal
    as_of TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (cbu_id, attr_id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cbu_attr_values_cbu
    ON "ob-poc".cbu_attr_values(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_attr_values_source
    ON "ob-poc".cbu_attr_values(cbu_id, source);

-- Updated_at trigger
CREATE OR REPLACE FUNCTION "ob-poc".update_cbu_attr_values_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_cbu_attr_values_updated ON "ob-poc".cbu_attr_values;
CREATE TRIGGER trg_cbu_attr_values_updated
    BEFORE UPDATE ON "ob-poc".cbu_attr_values
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_cbu_attr_values_timestamp();

COMMENT ON TABLE "ob-poc".cbu_attr_values IS
    'CBU-level attribute values. Populated by population engine from various sources.';
COMMENT ON COLUMN "ob-poc".cbu_attr_values.evidence_refs IS
    'Array of evidence references: [{type: "document", id: "uuid"}, {type: "entity_field", path: "..."}, ...]';
COMMENT ON COLUMN "ob-poc".cbu_attr_values.explain_refs IS
    'How was this value derived? [{rule: "...", input: "...", output: "..."}]';

-- =============================================================================
-- 3. VIEW: CBU Attribute Gaps
-- =============================================================================
-- Shows required attributes that are missing values for a CBU.

CREATE OR REPLACE VIEW "ob-poc".v_cbu_attr_gaps AS
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
    v.value IS NOT NULL AS has_value,
    v.source AS value_source,
    v.as_of AS value_as_of
FROM "ob-poc".cbu_unified_attr_requirements r
JOIN "ob-poc".cbus c ON c.cbu_id = r.cbu_id
JOIN "ob-poc".attribute_registry ar ON ar.uuid = r.attr_id
LEFT JOIN "ob-poc".cbu_attr_values v ON v.cbu_id = r.cbu_id AND v.attr_id = r.attr_id
WHERE r.requirement_strength = 'required'
ORDER BY r.cbu_id, ar.category, ar.display_name;

COMMENT ON VIEW "ob-poc".v_cbu_attr_gaps IS
    'Required attributes per CBU with gap analysis (has_value = false means missing).';

-- =============================================================================
-- 4. VIEW: CBU Attribute Summary
-- =============================================================================
-- Summary statistics per CBU.

CREATE OR REPLACE VIEW "ob-poc".v_cbu_attr_summary AS
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    COUNT(*) AS total_required,
    COUNT(v.value) AS populated,
    COUNT(*) - COUNT(v.value) AS missing,
    COUNT(*) FILTER (WHERE r.conflict IS NOT NULL) AS conflicts,
    ROUND(100.0 * COUNT(v.value) / NULLIF(COUNT(*), 0), 1) AS pct_complete
FROM "ob-poc".cbu_unified_attr_requirements r
JOIN "ob-poc".cbus c ON c.cbu_id = r.cbu_id
LEFT JOIN "ob-poc".cbu_attr_values v ON v.cbu_id = r.cbu_id AND v.attr_id = r.attr_id
WHERE r.requirement_strength = 'required'
GROUP BY r.cbu_id, c.name
ORDER BY pct_complete ASC, c.name;

COMMENT ON VIEW "ob-poc".v_cbu_attr_summary IS
    'Attribute population summary per CBU. Shows completion percentage.';

-- =============================================================================
-- 5. FUNCTION: Rebuild CBU Unified Requirements
-- =============================================================================
-- Utility function to rebuild unified requirements from discovery reasons.
-- Called by the rollup engine, can also be invoked manually.

CREATE OR REPLACE FUNCTION "ob-poc".rebuild_cbu_unified_requirements(p_cbu_id UUID)
RETURNS TABLE(
    attrs_added INT,
    attrs_updated INT,
    attrs_removed INT
) AS $$
DECLARE
    v_added INT := 0;
    v_updated INT := 0;
    v_removed INT := 0;
BEGIN
    -- This is a stub. The actual rollup logic lives in Rust.
    -- This function is here for future use or manual invocation.

    -- For now, just return zeros
    RETURN QUERY SELECT v_added, v_updated, v_removed;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".rebuild_cbu_unified_requirements IS
    'Stub function for rebuilding unified attr requirements. Actual logic in Rust rollup engine.';
