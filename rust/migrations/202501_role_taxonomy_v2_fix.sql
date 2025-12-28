-- ═══════════════════════════════════════════════════════════════════════════
-- ROLE TAXONOMY V2 - FIX MIGRATION
-- ═══════════════════════════════════════════════════════════════════════════
-- Fixes:
--   1. View column naming (primary_layout → primary_layout_category)
--   2. Add missing primary_role_category to view
--   3. Unique constraints for idempotent upserts
--   4. Missing columns in cbu_entity_roles (authority_limit, etc.)
--   5. Missing columns in entity_relationships (trust fields, control_type)
-- ═══════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 1: UNIQUE CONSTRAINTS FOR IDEMPOTENCY
-- ═══════════════════════════════════════════════════════════════════════════

-- Prevent duplicate role assignments (same entity, same role, same CBU)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_cbu_entity_role'
    ) THEN
        ALTER TABLE "ob-poc".cbu_entity_roles
        ADD CONSTRAINT uq_cbu_entity_role
        UNIQUE (cbu_id, entity_id, role_id);
    END IF;
END $$;

-- Prevent duplicate entity relationships
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_entity_relationship'
    ) THEN
        ALTER TABLE "ob-poc".entity_relationships
        ADD CONSTRAINT uq_entity_relationship
        UNIQUE (from_entity_id, to_entity_id, relationship_type);
    END IF;
END $$;

-- Prevent duplicate UBO edges (if table exists)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables
               WHERE table_schema = 'ob-poc' AND table_name = 'ubo_edges') THEN
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint
            WHERE conname = 'uq_ubo_edge'
        ) THEN
            ALTER TABLE "ob-poc".ubo_edges
            ADD CONSTRAINT uq_ubo_edge
            UNIQUE (cbu_id, from_entity_id, to_entity_id, edge_type);
        END IF;
    END IF;
END $$;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 2: ADD MISSING COLUMNS TO cbu_entity_roles
-- ═══════════════════════════════════════════════════════════════════════════

DO $$
BEGIN
    -- Core role assignment columns
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles'
                   AND column_name = 'target_entity_id') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN target_entity_id UUID REFERENCES "ob-poc".entities(entity_id);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles'
                   AND column_name = 'ownership_percentage') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN ownership_percentage DECIMAL(5,2);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles'
                   AND column_name = 'effective_from') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN effective_from DATE;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles'
                   AND column_name = 'effective_to') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN effective_to DATE;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles'
                   AND column_name = 'updated_at') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN updated_at TIMESTAMPTZ DEFAULT NOW();
    END IF;

    -- Authority columns for signatories
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles'
                   AND column_name = 'authority_limit') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN authority_limit DECIMAL(18,2);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles'
                   AND column_name = 'authority_currency') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN authority_currency VARCHAR(3) DEFAULT 'USD';
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbu_entity_roles'
                   AND column_name = 'requires_co_signatory') THEN
        ALTER TABLE "ob-poc".cbu_entity_roles ADD COLUMN requires_co_signatory BOOLEAN DEFAULT FALSE;
    END IF;
END $$;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 3: ADD MISSING COLUMNS TO entity_relationships
-- ═══════════════════════════════════════════════════════════════════════════

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships'
                   AND column_name = 'trust_interest_type') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN trust_interest_type VARCHAR(30);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships'
                   AND column_name = 'trust_class_description') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN trust_class_description TEXT;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships'
                   AND column_name = 'is_regulated') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN is_regulated BOOLEAN DEFAULT TRUE;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships'
                   AND column_name = 'regulatory_jurisdiction') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN regulatory_jurisdiction VARCHAR(20);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships'
                   AND column_name = 'control_type') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN control_type VARCHAR(30);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships'
                   AND column_name = 'ownership_type') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN ownership_type VARCHAR(20);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'entity_relationships'
                   AND column_name = 'updated_at') THEN
        ALTER TABLE "ob-poc".entity_relationships ADD COLUMN updated_at TIMESTAMPTZ DEFAULT NOW();
    END IF;
END $$;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 4: FIX v_cbu_entity_with_roles VIEW
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW "ob-poc".v_cbu_entity_with_roles AS
WITH role_data AS (
    SELECT
        cer.cbu_id,
        cer.entity_id,
        e.name AS entity_name,
        et.type_code AS entity_type,
        et.entity_category,
        COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) AS jurisdiction,
        r.name AS role_name,
        r.role_category,
        r.layout_category,
        r.display_priority,
        r.ubo_treatment,
        r.requires_percentage,
        r.kyc_obligation
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
    LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
    LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
    LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
)
SELECT
    cbu_id,
    entity_id,
    entity_name,
    entity_type,
    entity_category,
    jurisdiction,
    -- Aggregate roles
    array_agg(role_name ORDER BY display_priority DESC) AS roles,
    array_agg(DISTINCT role_category) FILTER (WHERE role_category IS NOT NULL) AS role_categories,
    array_agg(DISTINCT layout_category) FILTER (WHERE layout_category IS NOT NULL) AS layout_categories,
    -- Primary role (highest priority)
    (array_agg(role_name ORDER BY display_priority DESC))[1] AS primary_role,
    -- Primary role category (FIXED: was missing)
    (array_agg(role_category ORDER BY display_priority DESC) FILTER (WHERE role_category IS NOT NULL))[1] AS primary_role_category,
    -- Primary layout behavior (FIXED: renamed from primary_layout)
    (array_agg(layout_category ORDER BY display_priority DESC) FILTER (WHERE layout_category IS NOT NULL))[1] AS primary_layout_category,
    -- Max priority for sorting
    max(display_priority) AS max_role_priority,
    -- UBO treatment (most restrictive)
    CASE
        WHEN 'ALWAYS_UBO' = ANY(array_agg(ubo_treatment)) THEN 'ALWAYS_UBO'
        WHEN 'TERMINUS' = ANY(array_agg(ubo_treatment)) THEN 'TERMINUS'
        WHEN 'CONTROL_PRONG' = ANY(array_agg(ubo_treatment)) THEN 'CONTROL_PRONG'
        WHEN 'BY_PERCENTAGE' = ANY(array_agg(ubo_treatment)) THEN 'BY_PERCENTAGE'
        WHEN 'LOOK_THROUGH' = ANY(array_agg(ubo_treatment)) THEN 'LOOK_THROUGH'
        ELSE 'NOT_APPLICABLE'
    END AS effective_ubo_treatment,
    -- KYC obligation (most stringent)
    CASE
        WHEN 'FULL_KYC' = ANY(array_agg(kyc_obligation)) THEN 'FULL_KYC'
        WHEN 'SCREEN_AND_ID' = ANY(array_agg(kyc_obligation)) THEN 'SCREEN_AND_ID'
        WHEN 'SIMPLIFIED' = ANY(array_agg(kyc_obligation)) THEN 'SIMPLIFIED'
        WHEN 'SCREEN_ONLY' = ANY(array_agg(kyc_obligation)) THEN 'SCREEN_ONLY'
        ELSE 'RECORD_ONLY'
    END AS effective_kyc_obligation
FROM role_data
GROUP BY cbu_id, entity_id, entity_name, entity_type, entity_category, jurisdiction;

COMMENT ON VIEW "ob-poc".v_cbu_entity_with_roles IS
'Aggregated view of entities with their roles, categories, and effective KYC/UBO treatment.
Fixed in V2.1: Added primary_role_category, renamed primary_layout to primary_layout_category.';
