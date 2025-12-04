-- Migration: 011_ubo_enhancements.sql
-- Description: UBO chain analysis enhancements - discovery, versioning, snapshots
-- Based on: KYC_DSL_LIFECYCLE_TODO.md Phase 4 + docs_KYC_UBO_DSL_SPEC.md Section 8

-- =============================================================================
-- PART 1: Extend ubo_registry with case/workstream linking and lifecycle fields
-- =============================================================================

-- Add columns to existing ubo_registry table
ALTER TABLE "ob-poc".ubo_registry 
    ADD COLUMN IF NOT EXISTS case_id UUID REFERENCES kyc.cases(case_id),
    ADD COLUMN IF NOT EXISTS workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id),
    ADD COLUMN IF NOT EXISTS discovery_method VARCHAR(30) DEFAULT 'MANUAL',
    ADD COLUMN IF NOT EXISTS superseded_by UUID REFERENCES "ob-poc".ubo_registry(ubo_id),
    ADD COLUMN IF NOT EXISTS superseded_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS closed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS closed_reason VARCHAR(100);

-- Add check constraint for discovery_method
ALTER TABLE "ob-poc".ubo_registry 
    DROP CONSTRAINT IF EXISTS chk_ubo_discovery_method;
ALTER TABLE "ob-poc".ubo_registry 
    ADD CONSTRAINT chk_ubo_discovery_method 
    CHECK (discovery_method IN ('MANUAL', 'INFERRED', 'DOCUMENT', 'REGISTRY', 'SCREENING'));

-- Index for case-based UBO queries
CREATE INDEX IF NOT EXISTS idx_ubo_registry_case_id ON "ob-poc".ubo_registry(case_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_workstream_id ON "ob-poc".ubo_registry(workstream_id);

-- =============================================================================
-- PART 2: UBO Snapshots table for point-in-time ownership state
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".ubo_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    case_id UUID REFERENCES kyc.cases(case_id),
    snapshot_type VARCHAR(30) NOT NULL DEFAULT 'MANUAL',
    snapshot_reason VARCHAR(100),
    -- Denormalized snapshot of UBO state at capture time
    ubos JSONB NOT NULL DEFAULT '[]',
    ownership_chains JSONB NOT NULL DEFAULT '[]',
    control_relationships JSONB NOT NULL DEFAULT '[]',
    total_identified_ownership NUMERIC(5,2),
    has_gaps BOOLEAN DEFAULT false,
    gap_summary TEXT,
    -- Metadata
    captured_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    captured_by VARCHAR(255),
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT chk_snapshot_type CHECK (snapshot_type IN ('MANUAL', 'PERIODIC', 'EVENT_DRIVEN', 'CASE_OPEN', 'CASE_CLOSE'))
);

CREATE INDEX IF NOT EXISTS idx_ubo_snapshots_cbu_id ON "ob-poc".ubo_snapshots(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ubo_snapshots_case_id ON "ob-poc".ubo_snapshots(case_id);
CREATE INDEX IF NOT EXISTS idx_ubo_snapshots_captured_at ON "ob-poc".ubo_snapshots(captured_at DESC);

-- =============================================================================
-- PART 3: Snapshot comparisons for change detection
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".ubo_snapshot_comparisons (
    comparison_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    baseline_snapshot_id UUID NOT NULL REFERENCES "ob-poc".ubo_snapshots(snapshot_id),
    current_snapshot_id UUID NOT NULL REFERENCES "ob-poc".ubo_snapshots(snapshot_id),
    -- Comparison results
    has_changes BOOLEAN NOT NULL DEFAULT false,
    change_summary JSONB NOT NULL DEFAULT '{}',
    -- Specific change arrays
    added_ubos JSONB DEFAULT '[]',
    removed_ubos JSONB DEFAULT '[]',
    changed_ubos JSONB DEFAULT '[]',
    ownership_changes JSONB DEFAULT '[]',
    control_changes JSONB DEFAULT '[]',
    -- Metadata
    compared_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    compared_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT chk_different_snapshots CHECK (baseline_snapshot_id != current_snapshot_id)
);

CREATE INDEX IF NOT EXISTS idx_ubo_comparisons_cbu_id ON "ob-poc".ubo_snapshot_comparisons(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ubo_comparisons_baseline ON "ob-poc".ubo_snapshot_comparisons(baseline_snapshot_id);
CREATE INDEX IF NOT EXISTS idx_ubo_comparisons_current ON "ob-poc".ubo_snapshot_comparisons(current_snapshot_id);

-- =============================================================================
-- PART 4: Ownership chain computation function
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".compute_ownership_chains(
    p_cbu_id UUID,
    p_target_entity_id UUID DEFAULT NULL,
    p_max_depth INTEGER DEFAULT 10
)
RETURNS TABLE (
    chain_id INTEGER,
    ubo_person_id UUID,
    ubo_name TEXT,
    path_entities UUID[],
    path_names TEXT[],
    ownership_percentages NUMERIC[],
    effective_ownership NUMERIC,
    chain_depth INTEGER,
    is_complete BOOLEAN
) AS $$
WITH RECURSIVE ownership_chain AS (
    -- Base case: direct ownership from persons
    SELECT 
        ROW_NUMBER() OVER () as chain_id,
        o.owner_entity_id as current_entity,
        o.owned_entity_id as target_entity,
        ARRAY[o.owner_entity_id] as path,
        ARRAY[COALESCE(e.name, 'Unknown')] as names,
        ARRAY[o.ownership_percentage] as percentages,
        o.ownership_percentage as effective_pct,
        1 as depth,
        et.type_code = 'proper_person' as owner_is_person
    FROM "ob-poc".entity_ownership o
    JOIN "ob-poc".entities e ON o.owner_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    JOIN "ob-poc".cbu_entity_roles cer ON o.owned_entity_id = cer.entity_id
    WHERE cer.cbu_id = p_cbu_id
      AND o.is_active = true
      AND (p_target_entity_id IS NULL OR o.owned_entity_id = p_target_entity_id)
    
    UNION ALL
    
    -- Recursive case: follow ownership chain upward
    SELECT 
        oc.chain_id,
        o.owner_entity_id,
        oc.target_entity,
        oc.path || o.owner_entity_id,
        oc.names || COALESCE(e.name, 'Unknown'),
        oc.percentages || o.ownership_percentage,
        oc.effective_pct * o.ownership_percentage / 100,
        oc.depth + 1,
        et.type_code = 'proper_person'
    FROM ownership_chain oc
    JOIN "ob-poc".entity_ownership o ON o.owned_entity_id = oc.current_entity
    JOIN "ob-poc".entities e ON o.owner_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE o.is_active = true
      AND oc.depth < p_max_depth
      AND NOT o.owner_entity_id = ANY(oc.path) -- Prevent cycles
)
SELECT 
    chain_id::INTEGER,
    current_entity as ubo_person_id,
    names[array_length(names, 1)] as ubo_name,
    path as path_entities,
    names as path_names,
    percentages as ownership_percentages,
    effective_pct as effective_ownership,
    depth as chain_depth,
    owner_is_person as is_complete
FROM ownership_chain
WHERE owner_is_person = true  -- Only return chains that end at a natural person
ORDER BY effective_pct DESC, chain_id;
$$ LANGUAGE SQL STABLE;

-- =============================================================================
-- PART 5: UBO completeness check function
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".check_ubo_completeness(
    p_cbu_id UUID,
    p_threshold NUMERIC DEFAULT 25.0
)
RETURNS TABLE (
    is_complete BOOLEAN,
    total_identified_ownership NUMERIC,
    gap_percentage NUMERIC,
    missing_chains INTEGER,
    ubos_above_threshold INTEGER,
    issues JSONB
) AS $$
DECLARE
    v_total_ownership NUMERIC;
    v_issues JSONB := '[]'::JSONB;
    v_ubos_count INTEGER;
    v_incomplete_chains INTEGER;
BEGIN
    -- Calculate total identified ownership
    SELECT COALESCE(SUM(DISTINCT effective_ownership), 0)
    INTO v_total_ownership
    FROM "ob-poc".compute_ownership_chains(p_cbu_id);
    
    -- Count UBOs above threshold
    SELECT COUNT(DISTINCT ubo_person_id)
    INTO v_ubos_count
    FROM "ob-poc".compute_ownership_chains(p_cbu_id)
    WHERE effective_ownership >= p_threshold;
    
    -- Check for incomplete chains (entities with no further ownership but not persons)
    SELECT COUNT(*)
    INTO v_incomplete_chains
    FROM "ob-poc".entity_ownership o
    JOIN "ob-poc".cbu_entity_roles cer ON o.owned_entity_id = cer.entity_id
    LEFT JOIN "ob-poc".entity_ownership parent ON o.owner_entity_id = parent.owned_entity_id
    JOIN "ob-poc".entities e ON o.owner_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE cer.cbu_id = p_cbu_id
      AND o.is_active = true
      AND parent.ownership_id IS NULL
      AND et.type_code != 'proper_person';
    
    -- Build issues array
    IF v_total_ownership < 100 THEN
        v_issues := v_issues || jsonb_build_object(
            'type', 'OWNERSHIP_GAP',
            'message', format('Only %.2f%% ownership identified', v_total_ownership),
            'gap', 100 - v_total_ownership
        );
    END IF;
    
    IF v_incomplete_chains > 0 THEN
        v_issues := v_issues || jsonb_build_object(
            'type', 'INCOMPLETE_CHAIN',
            'message', format('%s ownership chains end at non-person entities', v_incomplete_chains),
            'count', v_incomplete_chains
        );
    END IF;
    
    RETURN QUERY SELECT
        (v_total_ownership >= 100 AND v_incomplete_chains = 0),
        v_total_ownership,
        GREATEST(0, 100 - v_total_ownership),
        v_incomplete_chains,
        v_ubos_count,
        v_issues;
END;
$$ LANGUAGE plpgsql STABLE;

-- =============================================================================
-- PART 6: Helper function to capture UBO snapshot
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".capture_ubo_snapshot(
    p_cbu_id UUID,
    p_case_id UUID DEFAULT NULL,
    p_snapshot_type VARCHAR(30) DEFAULT 'MANUAL',
    p_reason VARCHAR(100) DEFAULT NULL,
    p_captured_by VARCHAR(255) DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    v_snapshot_id UUID;
    v_ubos JSONB;
    v_chains JSONB;
    v_controls JSONB;
    v_completeness RECORD;
BEGIN
    -- Get current UBOs
    SELECT COALESCE(jsonb_agg(jsonb_build_object(
        'ubo_id', ur.ubo_id,
        'subject_entity_id', ur.subject_entity_id,
        'ubo_person_id', ur.ubo_person_id,
        'relationship_type', ur.relationship_type,
        'qualifying_reason', ur.qualifying_reason,
        'ownership_percentage', ur.ownership_percentage,
        'verification_status', ur.verification_status,
        'risk_rating', ur.risk_rating
    )), '[]'::JSONB)
    INTO v_ubos
    FROM "ob-poc".ubo_registry ur
    WHERE ur.cbu_id = p_cbu_id
      AND ur.superseded_at IS NULL
      AND ur.closed_at IS NULL;
    
    -- Get ownership chains
    SELECT COALESCE(jsonb_agg(jsonb_build_object(
        'ubo_person_id', chain.ubo_person_id,
        'ubo_name', chain.ubo_name,
        'path_entities', chain.path_entities,
        'path_names', chain.path_names,
        'ownership_percentages', chain.ownership_percentages,
        'effective_ownership', chain.effective_ownership,
        'chain_depth', chain.chain_depth
    )), '[]'::JSONB)
    INTO v_chains
    FROM "ob-poc".compute_ownership_chains(p_cbu_id) chain;
    
    -- Get control relationships
    SELECT COALESCE(jsonb_agg(jsonb_build_object(
        'control_id', cr.control_id,
        'controller_entity_id', cr.controller_entity_id,
        'controlled_entity_id', cr.controlled_entity_id,
        'control_type', cr.control_type,
        'description', cr.description
    )), '[]'::JSONB)
    INTO v_controls
    FROM "ob-poc".control_relationships cr
    JOIN "ob-poc".cbu_entity_roles cer ON cr.controlled_entity_id = cer.entity_id
    WHERE cer.cbu_id = p_cbu_id
      AND cr.is_active = true;
    
    -- Check completeness
    SELECT * INTO v_completeness
    FROM "ob-poc".check_ubo_completeness(p_cbu_id);
    
    -- Insert snapshot
    INSERT INTO "ob-poc".ubo_snapshots (
        cbu_id, case_id, snapshot_type, snapshot_reason,
        ubos, ownership_chains, control_relationships,
        total_identified_ownership, has_gaps, gap_summary,
        captured_by
    ) VALUES (
        p_cbu_id, p_case_id, p_snapshot_type, p_reason,
        v_ubos, v_chains, v_controls,
        v_completeness.total_identified_ownership,
        NOT v_completeness.is_complete,
        CASE WHEN NOT v_completeness.is_complete 
             THEN v_completeness.issues::TEXT 
             ELSE NULL END,
        p_captured_by
    ) RETURNING snapshot_id INTO v_snapshot_id;
    
    RETURN v_snapshot_id;
END;
$$ LANGUAGE plpgsql;

-- Grant permissions
GRANT SELECT, INSERT, UPDATE ON "ob-poc".ubo_snapshots TO PUBLIC;
GRANT SELECT, INSERT, UPDATE ON "ob-poc".ubo_snapshot_comparisons TO PUBLIC;
GRANT EXECUTE ON FUNCTION "ob-poc".compute_ownership_chains TO PUBLIC;
GRANT EXECUTE ON FUNCTION "ob-poc".check_ubo_completeness TO PUBLIC;
GRANT EXECUTE ON FUNCTION "ob-poc".capture_ubo_snapshot TO PUBLIC;

COMMENT ON TABLE "ob-poc".ubo_snapshots IS 'Point-in-time snapshots of UBO ownership state for a CBU';
COMMENT ON TABLE "ob-poc".ubo_snapshot_comparisons IS 'Comparisons between UBO snapshots to detect changes';
COMMENT ON FUNCTION "ob-poc".compute_ownership_chains IS 'Recursively computes ownership chains from entities to natural persons';
COMMENT ON FUNCTION "ob-poc".check_ubo_completeness IS 'Validates UBO determination completeness for a CBU';
COMMENT ON FUNCTION "ob-poc".capture_ubo_snapshot IS 'Captures current UBO state as a snapshot';
