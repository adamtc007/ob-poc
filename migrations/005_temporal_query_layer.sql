-- Migration: Temporal Query Layer
-- Purpose: Enable point-in-time queries for regulatory lookback
--
-- Implements:
--   1. History table for entity_relationships (audit trail)
--   2. Trigger to capture changes before UPDATE/DELETE
--   3. Point-in-time SQL functions for ownership/control queries
--   4. Temporal views for common lookback patterns
--
-- Answers the killer question:
--   "What did the ownership structure look like when we approved the KYC case 18 months ago?"

BEGIN;

-- ============================================================================
-- 1. History table for entity_relationships
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_relationships_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Original row data (all columns from entity_relationships)
    relationship_id UUID NOT NULL,
    from_entity_id UUID NOT NULL,
    to_entity_id UUID NOT NULL,
    relationship_type VARCHAR(30) NOT NULL,
    percentage NUMERIC(5,2),
    ownership_type VARCHAR(30),
    control_type VARCHAR(30),
    trust_role VARCHAR(30),
    interest_type VARCHAR(20),
    effective_from DATE,
    effective_to DATE,
    source VARCHAR(100),
    source_document_ref VARCHAR(255),
    notes TEXT,
    created_at TIMESTAMPTZ,
    created_by UUID,
    updated_at TIMESTAMPTZ,
    trust_interest_type VARCHAR(30),
    trust_class_description TEXT,
    is_regulated BOOLEAN,
    regulatory_jurisdiction VARCHAR(20),

    -- Audit metadata
    operation VARCHAR(10) NOT NULL, -- 'UPDATE' or 'DELETE'
    changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    changed_by UUID, -- Could be populated from session context
    superseded_by UUID, -- For UPDATE: the relationship_id that replaced this
    change_reason TEXT
);

-- Indexes for history queries
CREATE INDEX IF NOT EXISTS idx_rel_history_relationship_id
    ON "ob-poc".entity_relationships_history(relationship_id);
CREATE INDEX IF NOT EXISTS idx_rel_history_changed_at
    ON "ob-poc".entity_relationships_history(changed_at);
CREATE INDEX IF NOT EXISTS idx_rel_history_from_entity
    ON "ob-poc".entity_relationships_history(from_entity_id);
CREATE INDEX IF NOT EXISTS idx_rel_history_to_entity
    ON "ob-poc".entity_relationships_history(to_entity_id);
CREATE INDEX IF NOT EXISTS idx_rel_history_temporal
    ON "ob-poc".entity_relationships_history(effective_from, effective_to);

-- ============================================================================
-- 2. Trigger to capture changes before UPDATE/DELETE
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".entity_relationships_history_trigger()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO "ob-poc".entity_relationships_history (
            relationship_id, from_entity_id, to_entity_id, relationship_type,
            percentage, ownership_type, control_type, trust_role, interest_type,
            effective_from, effective_to, source, source_document_ref, notes,
            created_at, created_by, updated_at,
            trust_interest_type, trust_class_description, is_regulated, regulatory_jurisdiction,
            operation, changed_at
        ) VALUES (
            OLD.relationship_id, OLD.from_entity_id, OLD.to_entity_id, OLD.relationship_type,
            OLD.percentage, OLD.ownership_type, OLD.control_type, OLD.trust_role, OLD.interest_type,
            OLD.effective_from, OLD.effective_to, OLD.source, OLD.source_document_ref, OLD.notes,
            OLD.created_at, OLD.created_by, OLD.updated_at,
            OLD.trust_interest_type, OLD.trust_class_description, OLD.is_regulated, OLD.regulatory_jurisdiction,
            'DELETE', NOW()
        );
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO "ob-poc".entity_relationships_history (
            relationship_id, from_entity_id, to_entity_id, relationship_type,
            percentage, ownership_type, control_type, trust_role, interest_type,
            effective_from, effective_to, source, source_document_ref, notes,
            created_at, created_by, updated_at,
            trust_interest_type, trust_class_description, is_regulated, regulatory_jurisdiction,
            operation, changed_at, superseded_by
        ) VALUES (
            OLD.relationship_id, OLD.from_entity_id, OLD.to_entity_id, OLD.relationship_type,
            OLD.percentage, OLD.ownership_type, OLD.control_type, OLD.trust_role, OLD.interest_type,
            OLD.effective_from, OLD.effective_to, OLD.source, OLD.source_document_ref, OLD.notes,
            OLD.created_at, OLD.created_by, OLD.updated_at,
            OLD.trust_interest_type, OLD.trust_class_description, OLD.is_regulated, OLD.regulatory_jurisdiction,
            'UPDATE', NOW(), NEW.relationship_id
        );
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger (drop first if exists to allow re-run)
DROP TRIGGER IF EXISTS trg_entity_relationships_history ON "ob-poc".entity_relationships;
CREATE TRIGGER trg_entity_relationships_history
    BEFORE UPDATE OR DELETE ON "ob-poc".entity_relationships
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".entity_relationships_history_trigger();

-- ============================================================================
-- 3. Point-in-time SQL functions
-- ============================================================================

-- Get ownership relationships as of a specific date
CREATE OR REPLACE FUNCTION "ob-poc".ownership_as_of(
    p_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    relationship_id UUID,
    from_entity_id UUID,
    from_entity_name VARCHAR(255),
    to_entity_id UUID,
    to_entity_name VARCHAR(255),
    percentage NUMERIC(5,2),
    ownership_type VARCHAR(30),
    effective_from DATE,
    effective_to DATE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.relationship_id,
        r.from_entity_id,
        e_from.name AS from_entity_name,
        r.to_entity_id,
        e_to.name AS to_entity_name,
        r.percentage,
        r.ownership_type,
        r.effective_from,
        r.effective_to
    FROM "ob-poc".entity_relationships r
    JOIN "ob-poc".entities e_from ON r.from_entity_id = e_from.entity_id
    JOIN "ob-poc".entities e_to ON r.to_entity_id = e_to.entity_id
    WHERE r.relationship_type = 'ownership'
      AND (r.from_entity_id = p_entity_id OR r.to_entity_id = p_entity_id)
      AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
      AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date);
END;
$$ LANGUAGE plpgsql STABLE;

-- Get all relationships (ownership + control) for a CBU as of a date
CREATE OR REPLACE FUNCTION "ob-poc".cbu_relationships_as_of(
    p_cbu_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    relationship_id UUID,
    from_entity_id UUID,
    from_entity_name VARCHAR(255),
    to_entity_id UUID,
    to_entity_name VARCHAR(255),
    relationship_type VARCHAR(30),
    percentage NUMERIC(5,2),
    ownership_type VARCHAR(30),
    control_type VARCHAR(30),
    trust_role VARCHAR(30),
    effective_from DATE,
    effective_to DATE
) AS $$
BEGIN
    RETURN QUERY
    WITH cbu_entities AS (
        -- Get all entities linked to this CBU via roles
        SELECT DISTINCT cer.entity_id
        FROM "ob-poc".cbu_entity_roles cer
        WHERE cer.cbu_id = p_cbu_id
    )
    SELECT
        r.relationship_id,
        r.from_entity_id,
        e_from.name AS from_entity_name,
        r.to_entity_id,
        e_to.name AS to_entity_name,
        r.relationship_type,
        r.percentage,
        r.ownership_type,
        r.control_type,
        r.trust_role,
        r.effective_from,
        r.effective_to
    FROM "ob-poc".entity_relationships r
    JOIN "ob-poc".entities e_from ON r.from_entity_id = e_from.entity_id
    JOIN "ob-poc".entities e_to ON r.to_entity_id = e_to.entity_id
    WHERE (r.from_entity_id IN (SELECT entity_id FROM cbu_entities)
           OR r.to_entity_id IN (SELECT entity_id FROM cbu_entities))
      AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
      AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date);
END;
$$ LANGUAGE plpgsql STABLE;

-- Trace ownership chain to UBOs as of a specific date
CREATE OR REPLACE FUNCTION "ob-poc".ubo_chain_as_of(
    p_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE,
    p_threshold NUMERIC(5,2) DEFAULT 25.0
)
RETURNS TABLE (
    chain_path UUID[],
    chain_names TEXT[],
    ultimate_owner_id UUID,
    ultimate_owner_name VARCHAR(255),
    ultimate_owner_type VARCHAR(100),
    effective_percentage NUMERIC(10,4),
    chain_length INT
) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE ownership_chain AS (
        -- Base case: direct owners of the target entity
        SELECT
            ARRAY[r.from_entity_id] AS path,
            ARRAY[e.name::TEXT] AS names,
            r.from_entity_id AS current_entity,
            r.percentage AS cumulative_pct,
            1 AS depth,
            et.type_code AS entity_type
        FROM "ob-poc".entity_relationships r
        JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE r.to_entity_id = p_entity_id
          AND r.relationship_type = 'ownership'
          AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
          AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date)

        UNION ALL

        -- Recursive case: follow the chain upward
        SELECT
            oc.path || r.from_entity_id,
            oc.names || e.name::TEXT,
            r.from_entity_id,
            (oc.cumulative_pct * r.percentage / 100.0)::NUMERIC(10,4),
            oc.depth + 1,
            et.type_code
        FROM ownership_chain oc
        JOIN "ob-poc".entity_relationships r ON r.to_entity_id = oc.current_entity
        JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE r.relationship_type = 'ownership'
          AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
          AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date)
          AND oc.depth < 10 -- Prevent infinite loops
          AND NOT (r.from_entity_id = ANY(oc.path)) -- Prevent cycles
    )
    -- Return chains that end at natural persons (UBOs)
    SELECT
        oc.path,
        oc.names,
        oc.current_entity AS ultimate_owner_id,
        e.name AS ultimate_owner_name,
        et.type_code AS ultimate_owner_type,
        oc.cumulative_pct AS effective_percentage,
        oc.depth AS chain_length
    FROM ownership_chain oc
    JOIN "ob-poc".entities e ON oc.current_entity = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE et.entity_category = 'PERSON'
      AND oc.cumulative_pct >= p_threshold
    ORDER BY oc.cumulative_pct DESC;
END;
$$ LANGUAGE plpgsql STABLE;

-- Get entity roles within a CBU as of a date
CREATE OR REPLACE FUNCTION "ob-poc".cbu_roles_as_of(
    p_cbu_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    entity_id UUID,
    entity_name VARCHAR(255),
    entity_type VARCHAR(100),
    role_name VARCHAR(255),
    effective_from DATE,
    effective_to DATE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        e.entity_id,
        e.name AS entity_name,
        et.type_code AS entity_type,
        r.name AS role_name,
        cer.effective_from,
        cer.effective_to
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    WHERE cer.cbu_id = p_cbu_id
      AND (cer.effective_from IS NULL OR cer.effective_from <= p_as_of_date)
      AND (cer.effective_to IS NULL OR cer.effective_to > p_as_of_date);
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================================================
-- 4. Convenience view for "as of case approval" queries
-- ============================================================================

-- Get the state of a CBU at the time its KYC case was approved
CREATE OR REPLACE FUNCTION "ob-poc".cbu_state_at_approval(
    p_cbu_id UUID
)
RETURNS TABLE (
    case_id UUID,
    approved_at TIMESTAMPTZ,
    entity_id UUID,
    entity_name VARCHAR(255),
    role_name VARCHAR(255),
    ownership_from UUID,
    ownership_percentage NUMERIC(5,2)
) AS $$
DECLARE
    v_approval_date DATE;
    v_case_id UUID;
BEGIN
    -- Find the most recent approved case
    SELECT c.case_id, c.closed_at::DATE
    INTO v_case_id, v_approval_date
    FROM kyc.cases c
    WHERE c.cbu_id = p_cbu_id
      AND c.status = 'APPROVED'
    ORDER BY c.closed_at DESC
    LIMIT 1;

    IF v_case_id IS NULL THEN
        RETURN; -- No approved case found
    END IF;

    RETURN QUERY
    SELECT
        v_case_id,
        (SELECT c.closed_at FROM kyc.cases c WHERE c.case_id = v_case_id),
        e.entity_id,
        e.name AS entity_name,
        r.name AS role_name,
        rel.from_entity_id AS ownership_from,
        rel.percentage AS ownership_percentage
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    LEFT JOIN "ob-poc".entity_relationships rel
        ON rel.to_entity_id = e.entity_id
        AND rel.relationship_type = 'ownership'
        AND (rel.effective_from IS NULL OR rel.effective_from <= v_approval_date)
        AND (rel.effective_to IS NULL OR rel.effective_to > v_approval_date)
    WHERE cer.cbu_id = p_cbu_id
      AND (cer.effective_from IS NULL OR cer.effective_from <= v_approval_date)
      AND (cer.effective_to IS NULL OR cer.effective_to > v_approval_date);
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================================================
-- 5. Add history trigger for cbu_entity_roles as well
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_entity_roles_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Original row data
    cbu_entity_role_id UUID NOT NULL,
    cbu_id UUID NOT NULL,
    entity_id UUID NOT NULL,
    role_id UUID NOT NULL,
    target_entity_id UUID,
    ownership_percentage NUMERIC(5,2),
    effective_from DATE,
    effective_to DATE,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,

    -- Audit metadata
    operation VARCHAR(10) NOT NULL,
    changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    changed_by UUID
);

CREATE INDEX IF NOT EXISTS idx_cer_history_cbu
    ON "ob-poc".cbu_entity_roles_history(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cer_history_entity
    ON "ob-poc".cbu_entity_roles_history(entity_id);
CREATE INDEX IF NOT EXISTS idx_cer_history_changed_at
    ON "ob-poc".cbu_entity_roles_history(changed_at);

CREATE OR REPLACE FUNCTION "ob-poc".cbu_entity_roles_history_trigger()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO "ob-poc".cbu_entity_roles_history (
            cbu_entity_role_id, cbu_id, entity_id, role_id,
            target_entity_id, ownership_percentage,
            effective_from, effective_to, created_at, updated_at,
            operation, changed_at
        ) VALUES (
            OLD.cbu_entity_role_id, OLD.cbu_id, OLD.entity_id, OLD.role_id,
            OLD.target_entity_id, OLD.ownership_percentage,
            OLD.effective_from, OLD.effective_to, OLD.created_at, OLD.updated_at,
            'DELETE', NOW()
        );
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO "ob-poc".cbu_entity_roles_history (
            cbu_entity_role_id, cbu_id, entity_id, role_id,
            target_entity_id, ownership_percentage,
            effective_from, effective_to, created_at, updated_at,
            operation, changed_at
        ) VALUES (
            OLD.cbu_entity_role_id, OLD.cbu_id, OLD.entity_id, OLD.role_id,
            OLD.target_entity_id, OLD.ownership_percentage,
            OLD.effective_from, OLD.effective_to, OLD.created_at, OLD.updated_at,
            'UPDATE', NOW()
        );
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_cbu_entity_roles_history ON "ob-poc".cbu_entity_roles;
CREATE TRIGGER trg_cbu_entity_roles_history
    BEFORE UPDATE OR DELETE ON "ob-poc".cbu_entity_roles
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".cbu_entity_roles_history_trigger();

COMMIT;

-- ============================================================================
-- Usage examples (not part of migration, just documentation)
-- ============================================================================
/*
-- Get ownership for entity as of 6 months ago
SELECT * FROM "ob-poc".ownership_as_of(
    '550e8400-e29b-41d4-a716-446655440000'::UUID,
    CURRENT_DATE - INTERVAL '6 months'
);

-- Get all relationships for a CBU as of case approval date
SELECT * FROM "ob-poc".cbu_relationships_as_of(
    'cbu-uuid-here'::UUID,
    '2024-06-15'::DATE
);

-- Trace UBO chain as of a historical date
SELECT * FROM "ob-poc".ubo_chain_as_of(
    'fund-entity-uuid'::UUID,
    '2023-01-01'::DATE,
    25.0  -- threshold percentage
);

-- Get CBU state at the time of KYC approval
SELECT * FROM "ob-poc".cbu_state_at_approval('cbu-uuid-here'::UUID);

-- Query the history table directly for audit
SELECT * FROM "ob-poc".entity_relationships_history
WHERE relationship_id = 'some-uuid'
ORDER BY changed_at DESC;
*/
