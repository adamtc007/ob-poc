-- Migration: 001_consolidate_cbu_category_constraint.sql
-- Purpose: Consolidate duplicate CBU category CHECK constraints into a single constraint
--
-- Problem: The cbus table has TWO CHECK constraints on cbu_category with different values:
--   - cbus_category_check includes FAMILY_TRUST but not INTERNAL_TEST
--   - chk_cbu_category includes INTERNAL_TEST but not FAMILY_TRUST
--
-- This migration consolidates them into a single constraint with all valid values.

BEGIN;

-- Drop both existing constraints
ALTER TABLE "ob-poc".cbus DROP CONSTRAINT IF EXISTS cbus_category_check;
ALTER TABLE "ob-poc".cbus DROP CONSTRAINT IF EXISTS chk_cbu_category;

-- Add single consolidated constraint with all valid values
ALTER TABLE "ob-poc".cbus ADD CONSTRAINT chk_cbu_category CHECK (
  cbu_category IS NULL OR cbu_category IN (
    'FUND_MANDATE',
    'CORPORATE_GROUP',
    'INSTITUTIONAL_ACCOUNT',
    'RETAIL_CLIENT',
    'FAMILY_TRUST',
    'INTERNAL_TEST',
    'CORRESPONDENT_BANK'
  )
);

-- Update column comment to reflect all valid values
COMMENT ON COLUMN "ob-poc".cbus.cbu_category IS
  'Template discriminator for visualization layout: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, RETAIL_CLIENT, FAMILY_TRUST, INTERNAL_TEST, CORRESPONDENT_BANK';

COMMIT;
-- Add unique constraint for movement idempotency
-- Conflict key: (holding_id, trade_date, reference)
-- This enables retry-safe fund transactions

ALTER TABLE kyc.movements 
ADD CONSTRAINT movements_holding_trade_date_reference_key 
UNIQUE (holding_id, trade_date, reference);

-- Make reference required (NOT NULL) for idempotency
-- Existing NULL references will need to be backfilled first
-- ALTER TABLE kyc.movements ALTER COLUMN reference SET NOT NULL;

COMMENT ON CONSTRAINT movements_holding_trade_date_reference_key ON kyc.movements IS 
'Idempotency key for movement transactions. Same holding + trade_date + reference = same transaction.';
-- Add unique constraint for screening idempotency
-- Conflict key: (workstream_id, screening_type)
-- One screening of each type per workstream

ALTER TABLE kyc.screenings 
ADD CONSTRAINT screenings_workstream_type_key 
UNIQUE (workstream_id, screening_type);

COMMENT ON CONSTRAINT screenings_workstream_type_key ON kyc.screenings IS 
'Idempotency key for screenings. One screening per type per workstream.';
-- Migration: Entity Type Coverage Enhancement
-- Purpose: Add missing entity types for financial services KYC/custody domain
--
-- Adds:
--   - FOUNDATION (Stiftung, Anstalt - common in DE/AT/LI wealth planning)
--   - GOVERNMENT_ENTITY (Sovereign, central bank - for SWF ownership chains)
--   - SPV (Special Purpose Vehicle - securitization, structured products)
--   - COOPERATIVE (Credit unions, agricultural co-ops)
--
-- Deprecates (marks for removal, keeps for backward compat):
--   - management_company (use LIMITED_COMPANY_* + MANAGEMENT_COMPANY role)
--   - depositary (use LIMITED_COMPANY_* + DEPOSITARY role)
--   - fund_administrator (use LIMITED_COMPANY_* + FUND_ADMINISTRATOR role)
--
-- Fixes:
--   - Sets entity_category consistently across all types
--   - Adds table_name mappings for dynamic verb generation

BEGIN;

-- ============================================================================
-- 1. Create extension tables for new entity types
-- ============================================================================

-- Foundation extension table (Stiftung, Anstalt, etc.)
CREATE TABLE IF NOT EXISTS "ob-poc".entity_foundations (
    foundation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    foundation_name VARCHAR(255) NOT NULL,
    foundation_type VARCHAR(50), -- STIFTUNG, ANSTALT, PRIVATE_FOUNDATION, CHARITABLE_FOUNDATION
    jurisdiction VARCHAR(100) NOT NULL,
    registration_number VARCHAR(100),
    establishment_date DATE,
    foundation_purpose TEXT,
    governing_law VARCHAR(100),
    registered_address TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT entity_foundations_entity_id_uniq UNIQUE (entity_id)
);

CREATE INDEX IF NOT EXISTS idx_foundations_name_trgm
    ON "ob-poc".entity_foundations USING gin (foundation_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_foundations_jurisdiction
    ON "ob-poc".entity_foundations(jurisdiction);
CREATE INDEX IF NOT EXISTS idx_foundations_entity_id
    ON "ob-poc".entity_foundations(entity_id);

-- Government entity extension table (Sovereign, central bank, SWF, etc.)
CREATE TABLE IF NOT EXISTS "ob-poc".entity_government (
    government_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    entity_name VARCHAR(255) NOT NULL,
    government_type VARCHAR(50) NOT NULL, -- SOVEREIGN, CENTRAL_BANK, SWF, STATE_OWNED_ENTERPRISE, SUPRANATIONAL
    country_code VARCHAR(3), -- ISO 3166-1 alpha-3
    governing_authority VARCHAR(255),
    establishment_date DATE,
    registered_address TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT entity_government_entity_id_uniq UNIQUE (entity_id)
);

CREATE INDEX IF NOT EXISTS idx_government_name_trgm
    ON "ob-poc".entity_government USING gin (entity_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_government_country
    ON "ob-poc".entity_government(country_code);
CREATE INDEX IF NOT EXISTS idx_government_type
    ON "ob-poc".entity_government(government_type);
CREATE INDEX IF NOT EXISTS idx_government_entity_id
    ON "ob-poc".entity_government(entity_id);

-- Cooperative extension table
CREATE TABLE IF NOT EXISTS "ob-poc".entity_cooperatives (
    cooperative_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    cooperative_name VARCHAR(255) NOT NULL,
    cooperative_type VARCHAR(50), -- CREDIT_UNION, AGRICULTURAL, HOUSING, WORKER, CONSUMER
    jurisdiction VARCHAR(100),
    registration_number VARCHAR(100),
    formation_date DATE,
    member_count INTEGER,
    registered_address TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT entity_cooperatives_entity_id_uniq UNIQUE (entity_id)
);

CREATE INDEX IF NOT EXISTS idx_cooperatives_name_trgm
    ON "ob-poc".entity_cooperatives USING gin (cooperative_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_cooperatives_jurisdiction
    ON "ob-poc".entity_cooperatives(jurisdiction);
CREATE INDEX IF NOT EXISTS idx_cooperatives_entity_id
    ON "ob-poc".entity_cooperatives(entity_id);

-- ============================================================================
-- 2. Add new entity types
-- ============================================================================

-- Add FOUNDATION type
INSERT INTO "ob-poc".entity_types (type_code, name, entity_category, table_name)
VALUES ('FOUNDATION', 'Foundation/Stiftung', 'SHELL', 'entity_foundations')
ON CONFLICT (type_code) DO UPDATE SET
    name = EXCLUDED.name,
    entity_category = EXCLUDED.entity_category,
    table_name = EXCLUDED.table_name;

-- Add GOVERNMENT_ENTITY type
INSERT INTO "ob-poc".entity_types (type_code, name, entity_category, table_name)
VALUES ('GOVERNMENT_ENTITY', 'Government/Sovereign Entity', 'SHELL', 'entity_government')
ON CONFLICT (type_code) DO UPDATE SET
    name = EXCLUDED.name,
    entity_category = EXCLUDED.entity_category,
    table_name = EXCLUDED.table_name;

-- Add SPV type (uses limited_companies table - it's a company with special purpose)
INSERT INTO "ob-poc".entity_types (type_code, name, entity_category, table_name)
VALUES ('SPV', 'Special Purpose Vehicle', 'SHELL', 'entity_limited_companies')
ON CONFLICT (type_code) DO UPDATE SET
    name = EXCLUDED.name,
    entity_category = EXCLUDED.entity_category,
    table_name = EXCLUDED.table_name;

-- Add COOPERATIVE type
INSERT INTO "ob-poc".entity_types (type_code, name, entity_category, table_name)
VALUES ('COOPERATIVE', 'Cooperative', 'SHELL', 'entity_cooperatives')
ON CONFLICT (type_code) DO UPDATE SET
    name = EXCLUDED.name,
    entity_category = EXCLUDED.entity_category,
    table_name = EXCLUDED.table_name;

-- ============================================================================
-- 3. Mark deprecated entity types (add deprecated flag, don't delete)
-- ============================================================================

-- First, add deprecated column if it doesn't exist
ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS deprecated BOOLEAN DEFAULT FALSE;

ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS deprecation_note TEXT;

-- Mark service provider entity types as deprecated
-- These should use LIMITED_COMPANY_* + role assignment instead
UPDATE "ob-poc".entity_types
SET
    deprecated = TRUE,
    deprecation_note = 'Use LIMITED_COMPANY_* with MANAGEMENT_COMPANY role instead'
WHERE type_code = 'management_company';

UPDATE "ob-poc".entity_types
SET
    deprecated = TRUE,
    deprecation_note = 'Use LIMITED_COMPANY_* with DEPOSITARY role instead'
WHERE type_code = 'depositary';

UPDATE "ob-poc".entity_types
SET
    deprecated = TRUE,
    deprecation_note = 'Use LIMITED_COMPANY_* with FUND_ADMINISTRATOR role instead'
WHERE type_code = 'fund_administrator';

-- ============================================================================
-- 4. Fix entity_category consistency
-- ============================================================================

-- Ensure all types have proper categories
UPDATE "ob-poc".entity_types SET entity_category = 'PERSON'
WHERE type_code LIKE 'PROPER_PERSON%' AND (entity_category IS NULL OR entity_category != 'PERSON');

UPDATE "ob-poc".entity_types SET entity_category = 'SHELL'
WHERE type_code LIKE 'LIMITED_COMPANY%' AND (entity_category IS NULL OR entity_category != 'SHELL');

UPDATE "ob-poc".entity_types SET entity_category = 'SHELL'
WHERE type_code = 'limited_company' AND entity_category IS NULL;

UPDATE "ob-poc".entity_types SET entity_category = 'SHELL'
WHERE type_code LIKE 'PARTNERSHIP%' AND (entity_category IS NULL OR entity_category != 'SHELL');

UPDATE "ob-poc".entity_types SET entity_category = 'SHELL'
WHERE type_code LIKE 'TRUST%' AND (entity_category IS NULL OR entity_category != 'SHELL');

UPDATE "ob-poc".entity_types SET entity_category = 'SHELL'
WHERE type_code LIKE 'fund%' AND (entity_category IS NULL OR entity_category != 'SHELL');

-- ============================================================================
-- 5. Ensure table_name is set for all types (required for dynamic verb gen)
-- ============================================================================

UPDATE "ob-poc".entity_types SET table_name = 'entity_proper_persons'
WHERE type_code LIKE 'PROPER_PERSON%' AND table_name IS NULL;

UPDATE "ob-poc".entity_types SET table_name = 'entity_limited_companies'
WHERE (type_code LIKE 'LIMITED_COMPANY%' OR type_code = 'limited_company') AND table_name IS NULL;

UPDATE "ob-poc".entity_types SET table_name = 'entity_partnerships'
WHERE type_code LIKE 'PARTNERSHIP%' AND table_name IS NULL;

UPDATE "ob-poc".entity_types SET table_name = 'entity_trusts'
WHERE type_code LIKE 'TRUST%' AND table_name IS NULL;

UPDATE "ob-poc".entity_types SET table_name = 'entity_funds'
WHERE type_code IN ('fund_umbrella', 'fund_subfund', 'fund_standalone', 'fund_master', 'fund_feeder')
AND table_name IS NULL;

UPDATE "ob-poc".entity_types SET table_name = 'entity_share_classes'
WHERE type_code = 'fund_share_class' AND table_name IS NULL;

-- ============================================================================
-- 6. Add roles if they don't exist (for the deprecated types migration path)
-- ============================================================================

INSERT INTO "ob-poc".roles (name, description, role_category)
VALUES
    ('MANAGEMENT_COMPANY', 'Fund management company (UCITS ManCo, AIFM)', 'FUND_MANAGEMENT'),
    ('DEPOSITARY', 'Fund depositary/trustee', 'SERVICE_PROVIDER'),
    ('FUND_ADMINISTRATOR', 'Fund administrator', 'SERVICE_PROVIDER')
ON CONFLICT (name) DO NOTHING;

COMMIT;

-- ============================================================================
-- Verification query (run after migration)
-- ============================================================================
-- SELECT type_code, name, entity_category, table_name, deprecated, deprecation_note
-- FROM "ob-poc".entity_types
-- ORDER BY
--     CASE WHEN deprecated THEN 1 ELSE 0 END,
--     entity_category,
--     type_code;
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
-- GLEIF Entity Enhancement Migration
-- Adds GLEIF-specific fields and relationship tracking for LEI → UBO pipeline
-- Phase 1: Corporate Layer

BEGIN;

-- ============================================================================
-- 1. Add LEI and GLEIF columns to entity_limited_companies
-- ============================================================================

ALTER TABLE "ob-poc".entity_limited_companies
ADD COLUMN IF NOT EXISTS lei VARCHAR(20) UNIQUE,
ADD COLUMN IF NOT EXISTS gleif_status VARCHAR(20),
ADD COLUMN IF NOT EXISTS gleif_category VARCHAR(50),
ADD COLUMN IF NOT EXISTS gleif_subcategory VARCHAR(50),
ADD COLUMN IF NOT EXISTS legal_form_code VARCHAR(10),
ADD COLUMN IF NOT EXISTS legal_form_text VARCHAR(200),
ADD COLUMN IF NOT EXISTS gleif_validation_level VARCHAR(30),
ADD COLUMN IF NOT EXISTS gleif_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS gleif_next_renewal DATE,
ADD COLUMN IF NOT EXISTS direct_parent_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS ultimate_parent_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS entity_creation_date DATE,
ADD COLUMN IF NOT EXISTS headquarters_address TEXT,
ADD COLUMN IF NOT EXISTS headquarters_city VARCHAR(200),
ADD COLUMN IF NOT EXISTS headquarters_country VARCHAR(3),
ADD COLUMN IF NOT EXISTS fund_manager_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS umbrella_fund_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS master_fund_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS is_fund BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS fund_type VARCHAR(30),
ADD COLUMN IF NOT EXISTS gleif_direct_parent_exception VARCHAR(50),
ADD COLUMN IF NOT EXISTS gleif_ultimate_parent_exception VARCHAR(50),
ADD COLUMN IF NOT EXISTS ubo_status VARCHAR(30) DEFAULT 'PENDING';

-- Create index on LEI
CREATE INDEX IF NOT EXISTS idx_limited_companies_lei
ON "ob-poc".entity_limited_companies(lei) WHERE lei IS NOT NULL;

-- Index for parent LEI lookups
CREATE INDEX IF NOT EXISTS idx_limited_companies_direct_parent
ON "ob-poc".entity_limited_companies(direct_parent_lei) WHERE direct_parent_lei IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_limited_companies_ultimate_parent
ON "ob-poc".entity_limited_companies(ultimate_parent_lei) WHERE ultimate_parent_lei IS NOT NULL;

COMMENT ON COLUMN "ob-poc".entity_limited_companies.gleif_direct_parent_exception IS
'GLEIF Level 2 reporting exception for direct parent: NO_KNOWN_PERSON, NATURAL_PERSONS, NON_CONSOLIDATING, etc.';

COMMENT ON COLUMN "ob-poc".entity_limited_companies.gleif_ultimate_parent_exception IS
'GLEIF Level 2 reporting exception for ultimate parent';

COMMENT ON COLUMN "ob-poc".entity_limited_companies.ubo_status IS
'UBO discovery status: PENDING, DISCOVERED, PUBLIC_FLOAT, EXEMPT, MANUAL_REQUIRED';

-- ============================================================================
-- 2. Create entity_names table (alternative names, trading names, etc.)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_names (
    name_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    name_type VARCHAR(50) NOT NULL,  -- LEGAL, TRADING, TRANSLITERATED, HISTORICAL
    name TEXT NOT NULL,
    language VARCHAR(10),
    is_primary BOOLEAN DEFAULT FALSE,
    effective_from DATE,
    effective_to DATE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_name_type CHECK (
        name_type IN ('LEGAL', 'TRADING', 'TRANSLITERATED', 'HISTORICAL', 'ALTERNATIVE', 'SHORT')
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_names_entity
ON "ob-poc".entity_names(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_names_search
ON "ob-poc".entity_names USING gin(to_tsvector('english', name));

COMMENT ON TABLE "ob-poc".entity_names IS
'Alternative names for entities from GLEIF otherNames and transliteratedOtherNames fields';

-- ============================================================================
-- 3. Create entity_addresses table (structured address data)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_addresses (
    address_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    address_type VARCHAR(50) NOT NULL,  -- LEGAL, HEADQUARTERS, BRANCH, ALTERNATIVE
    language VARCHAR(10),
    address_lines TEXT[],
    city VARCHAR(200),
    region VARCHAR(50),           -- ISO 3166-2
    country VARCHAR(3) NOT NULL,  -- ISO 3166-1 alpha-2
    postal_code VARCHAR(50),
    is_primary BOOLEAN DEFAULT FALSE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_address_type CHECK (
        address_type IN ('LEGAL', 'HEADQUARTERS', 'BRANCH', 'ALTERNATIVE', 'TRANSLITERATED')
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_addresses_entity
ON "ob-poc".entity_addresses(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_addresses_country
ON "ob-poc".entity_addresses(country);

COMMENT ON TABLE "ob-poc".entity_addresses IS
'Structured address data from GLEIF legalAddress, headquartersAddress, otherAddresses';

-- ============================================================================
-- 4. Create entity_identifiers table (LEI, BIC, ISIN, etc.)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    identifier_type VARCHAR(30) NOT NULL,  -- LEI, BIC, ISIN, CIK, REG_NUM, MIC
    identifier_value VARCHAR(100) NOT NULL,
    issuing_authority VARCHAR(100),
    is_primary BOOLEAN DEFAULT FALSE,
    valid_from DATE,
    valid_until DATE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_identifier_type CHECK (
        identifier_type IN ('LEI', 'BIC', 'ISIN', 'CIK', 'MIC', 'REG_NUM', 'FIGI', 'CUSIP', 'SEDOL')
    ),
    UNIQUE(entity_id, identifier_type, identifier_value)
);

CREATE INDEX IF NOT EXISTS idx_entity_identifiers_lookup
ON "ob-poc".entity_identifiers(identifier_type, identifier_value);

COMMENT ON TABLE "ob-poc".entity_identifiers IS
'Cross-reference identifiers from GLEIF (LEI, BIC mappings, etc.) and other sources';

-- ============================================================================
-- 5. Create entity_parent_relationships table (ownership chains)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_parent_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    parent_entity_id UUID REFERENCES "ob-poc".entities(entity_id),  -- NULL if parent not in our system
    parent_lei VARCHAR(20),  -- Store even if parent not in our system
    parent_name TEXT,        -- Denormalized for display
    relationship_type VARCHAR(50) NOT NULL,  -- DIRECT_PARENT, ULTIMATE_PARENT
    accounting_standard VARCHAR(20),  -- IFRS, US_GAAP, etc.
    relationship_start DATE,
    relationship_end DATE,
    relationship_status VARCHAR(30) DEFAULT 'ACTIVE',
    validation_source VARCHAR(50),
    validation_reference TEXT,
    source VARCHAR(50) DEFAULT 'GLEIF',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_relationship_type CHECK (
        relationship_type IN ('DIRECT_PARENT', 'ULTIMATE_PARENT', 'FUND_MANAGER',
                              'UMBRELLA_FUND', 'MASTER_FUND', 'BRANCH_OF')
    ),
    UNIQUE(child_entity_id, parent_lei, relationship_type)
);

CREATE INDEX IF NOT EXISTS idx_entity_parents_child
ON "ob-poc".entity_parent_relationships(child_entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_parents_parent
ON "ob-poc".entity_parent_relationships(parent_entity_id) WHERE parent_entity_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_entity_parents_parent_lei
ON "ob-poc".entity_parent_relationships(parent_lei) WHERE parent_lei IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_entity_parents_type
ON "ob-poc".entity_parent_relationships(relationship_type);

COMMENT ON TABLE "ob-poc".entity_parent_relationships IS
'Corporate ownership relationships from GLEIF Level 2 data - direct and ultimate parents';

-- ============================================================================
-- 6. Create entity_lifecycle_events table (corporate actions)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_lifecycle_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL,  -- CHANGE_LEGAL_NAME, MERGER, DISSOLUTION, etc.
    event_status VARCHAR(30),         -- PENDING, COMPLETED
    effective_date DATE,
    recorded_date DATE,
    affected_fields JSONB,            -- What changed
    old_values JSONB,
    new_values JSONB,
    successor_lei VARCHAR(20),
    successor_name TEXT,
    validation_documents VARCHAR(50),
    validation_reference TEXT,
    source VARCHAR(50) DEFAULT 'GLEIF',
    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_event_type CHECK (
        event_type IN ('CHANGE_LEGAL_NAME', 'CHANGE_LEGAL_ADDRESS', 'CHANGE_HQ_ADDRESS',
                       'CHANGE_LEGAL_FORM', 'MERGER', 'SPIN_OFF', 'ACQUISITION',
                       'DISSOLUTION', 'BANKRUPTCY', 'DEREGISTRATION', 'RELOCATION')
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_events_entity
ON "ob-poc".entity_lifecycle_events(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_events_type
ON "ob-poc".entity_lifecycle_events(event_type);

CREATE INDEX IF NOT EXISTS idx_entity_events_date
ON "ob-poc".entity_lifecycle_events(effective_date DESC);

COMMENT ON TABLE "ob-poc".entity_lifecycle_events IS
'Corporate lifecycle events from GLEIF eventGroups - name changes, mergers, etc.';

-- ============================================================================
-- 7. Create gleif_sync_log table (track GLEIF data freshness)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".gleif_sync_log (
    sync_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    lei VARCHAR(20),
    sync_type VARCHAR(30) NOT NULL,  -- FULL, INCREMENTAL, RELATIONSHIP
    sync_status VARCHAR(30) NOT NULL,  -- SUCCESS, FAILED, PARTIAL
    records_fetched INTEGER DEFAULT 0,
    records_updated INTEGER DEFAULT 0,
    records_created INTEGER DEFAULT 0,
    error_message TEXT,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    CONSTRAINT valid_sync_status CHECK (
        sync_status IN ('SUCCESS', 'FAILED', 'PARTIAL', 'IN_PROGRESS')
    )
);

CREATE INDEX IF NOT EXISTS idx_gleif_sync_entity
ON "ob-poc".gleif_sync_log(entity_id) WHERE entity_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_gleif_sync_lei
ON "ob-poc".gleif_sync_log(lei) WHERE lei IS NOT NULL;

COMMENT ON TABLE "ob-poc".gleif_sync_log IS
'Audit log for GLEIF data synchronization operations';

COMMIT;
-- ============================================================================
-- Migration 007: BODS UBO Layer
-- Beneficial Ownership Data Standard tables for UBO discovery
-- ============================================================================

-- ============================================================================
-- BODS Entity Statements (companies/trusts from beneficial ownership registers)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_entity_statements (
    statement_id VARCHAR(100) PRIMARY KEY,  -- BODS statement ID
    entity_type VARCHAR(50),                -- registeredEntity, legalEntity, arrangement
    name TEXT,
    jurisdiction VARCHAR(10),

    -- Identifiers (denormalized for query performance)
    lei VARCHAR(20),                        -- LEI if present
    company_number VARCHAR(100),
    opencorporates_id VARCHAR(200),

    -- Full identifiers array
    identifiers JSONB,

    -- Metadata
    source_register VARCHAR(100),           -- UK_PSC, DENMARK_CVR, etc.
    statement_date DATE,
    source_url TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_entity_type CHECK (
        entity_type IN ('registeredEntity', 'legalEntity', 'arrangement',
                        'anonymousEntity', 'unknownEntity', 'state', 'stateBody')
    )
);

CREATE INDEX IF NOT EXISTS idx_bods_entity_lei
ON "ob-poc".bods_entity_statements(lei) WHERE lei IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_bods_entity_company_num
ON "ob-poc".bods_entity_statements(company_number) WHERE company_number IS NOT NULL;

-- ============================================================================
-- BODS Person Statements (natural persons - the actual UBOs)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_person_statements (
    statement_id VARCHAR(100) PRIMARY KEY,  -- BODS statement ID
    person_type VARCHAR(50),                -- knownPerson, anonymousPerson, unknownPerson

    -- Name (primary)
    full_name TEXT,
    given_name VARCHAR(200),
    family_name VARCHAR(200),

    -- All names (JSONB for aliases, maiden names, etc.)
    names JSONB,

    -- Demographics
    birth_date DATE,
    birth_date_precision VARCHAR(20),       -- exact, month, year
    death_date DATE,

    -- Location
    nationalities VARCHAR(10)[],            -- ISO country codes
    country_of_residence VARCHAR(10),
    addresses JSONB,

    -- Tax identifiers (if disclosed)
    tax_residencies VARCHAR(10)[],

    -- Metadata
    source_register VARCHAR(100),
    statement_date DATE,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_person_type CHECK (
        person_type IN ('knownPerson', 'anonymousPerson', 'unknownPerson')
    )
);

CREATE INDEX IF NOT EXISTS idx_bods_person_name
ON "ob-poc".bods_person_statements USING gin(to_tsvector('english', full_name));

-- ============================================================================
-- BODS Ownership/Control Statements (the relationships)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_ownership_statements (
    statement_id VARCHAR(100) PRIMARY KEY,

    -- Subject (the entity being owned/controlled)
    subject_entity_statement_id VARCHAR(100),
    subject_lei VARCHAR(20),
    subject_name TEXT,

    -- Interested Party (the owner - can be person or entity)
    interested_party_type VARCHAR(20),      -- person, entity
    interested_party_statement_id VARCHAR(100),
    interested_party_name TEXT,

    -- Ownership details
    ownership_type VARCHAR(50),             -- shareholding, votingRights, appointmentOfBoard, etc.
    share_min DECIMAL,
    share_max DECIMAL,
    share_exact DECIMAL,
    is_direct BOOLEAN,

    -- Control details
    control_types VARCHAR(50)[],

    -- Validity
    start_date DATE,
    end_date DATE,

    -- Metadata
    source_register VARCHAR(100),
    statement_date DATE,
    source_description TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_bods_ownership_subject
ON "ob-poc".bods_ownership_statements(subject_entity_statement_id);

CREATE INDEX IF NOT EXISTS idx_bods_ownership_subject_lei
ON "ob-poc".bods_ownership_statements(subject_lei) WHERE subject_lei IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_bods_ownership_interested
ON "ob-poc".bods_ownership_statements(interested_party_statement_id);

-- ============================================================================
-- Link table: Connect our entities to BODS statements
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_bods_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    bods_entity_statement_id VARCHAR(100) REFERENCES "ob-poc".bods_entity_statements(statement_id),
    match_method VARCHAR(50),               -- LEI, COMPANY_NUMBER, NAME_MATCH
    match_confidence DECIMAL,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(entity_id, bods_entity_statement_id)
);

-- ============================================================================
-- UBO Summary View (denormalized for quick access)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_ubos (
    ubo_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- UBO details
    person_statement_id VARCHAR(100),
    person_name TEXT,
    nationalities VARCHAR(10)[],
    country_of_residence VARCHAR(10),

    -- Ownership chain
    ownership_chain JSONB,                  -- Array of intermediate entities
    chain_depth INTEGER,

    -- Ownership percentage (aggregated)
    ownership_min DECIMAL,
    ownership_max DECIMAL,
    ownership_exact DECIMAL,

    -- Control types
    control_types VARCHAR(50)[],
    is_direct BOOLEAN,

    -- Status
    ubo_type VARCHAR(30),                   -- NATURAL_PERSON, PUBLIC_FLOAT, UNKNOWN
    confidence_level VARCHAR(20),

    -- Source tracking
    source VARCHAR(50),                     -- BODS, GLEIF, MANUAL
    source_register VARCHAR(100),
    discovered_at TIMESTAMPTZ DEFAULT NOW(),
    verified_at TIMESTAMPTZ,
    verified_by VARCHAR(255),

    CONSTRAINT valid_ubo_type CHECK (
        ubo_type IN ('NATURAL_PERSON', 'PUBLIC_FLOAT', 'STATE_OWNED',
                     'WIDELY_HELD', 'UNKNOWN', 'EXEMPT')
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_ubos_entity
ON "ob-poc".entity_ubos(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_ubos_person
ON "ob-poc".entity_ubos(person_statement_id) WHERE person_statement_id IS NOT NULL;

-- ============================================================================
-- Comments for documentation
-- ============================================================================

COMMENT ON TABLE "ob-poc".bods_entity_statements IS 'BODS entity statements from beneficial ownership registers (UK PSC, etc.)';
COMMENT ON TABLE "ob-poc".bods_person_statements IS 'BODS person statements - natural persons who are UBOs';
COMMENT ON TABLE "ob-poc".bods_ownership_statements IS 'BODS ownership/control statements linking persons to entities';
COMMENT ON TABLE "ob-poc".entity_bods_links IS 'Links our entities to BODS entity statements';
COMMENT ON TABLE "ob-poc".entity_ubos IS 'Denormalized UBO summary for quick access';

COMMENT ON COLUMN "ob-poc".bods_entity_statements.lei IS 'LEI identifier if present - join key to GLEIF data';
COMMENT ON COLUMN "ob-poc".bods_person_statements.birth_date_precision IS 'Precision of birth date: exact, month, or year';
COMMENT ON COLUMN "ob-poc".entity_ubos.ownership_chain IS 'JSON array of intermediate entities in ownership chain';
COMMENT ON COLUMN "ob-poc".entity_ubos.ubo_type IS 'Type: NATURAL_PERSON, PUBLIC_FLOAT, STATE_OWNED, WIDELY_HELD, UNKNOWN, EXEMPT';
-- Migration 008: KYC Control Enhancement - Roles and Validation
-- Phase A from TODO-KYC-CONTROL-ENHANCEMENT.md
-- Adds executive, LLP, and control-specific roles plus role-entity-type constraints

-- ============================================================================
-- A.1: Add C-Suite & Executive Roles
-- ============================================================================

INSERT INTO "ob-poc".roles (name, description, role_category, layout_category, ubo_treatment, natural_person_only, display_priority, sort_order)
VALUES
  ('CEO', 'Chief Executive Officer - operational control', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 10, 10),
  ('CFO', 'Chief Financial Officer - financial control', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 11, 11),
  ('CIO', 'Chief Investment Officer - investment decisions', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 12, 12),
  ('COO', 'Chief Operating Officer - operations', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 13, 13),
  ('CRO', 'Chief Risk Officer - risk management', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 14, 14),
  ('CCO', 'Chief Compliance Officer - compliance oversight', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 15, 15),
  ('MANAGING_DIRECTOR', 'Managing Director - senior management', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 20, 20),
  ('CHAIRMAN', 'Board Chairman - board control', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 5, 5),
  ('EXECUTIVE_DIRECTOR', 'Executive board member - management + board', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 25, 25),
  ('NON_EXECUTIVE_DIRECTOR', 'Non-executive board member - oversight only', 'CONTROL_CHAIN', 'Overlay', 'OVERSIGHT', true, 26, 26)
ON CONFLICT (name) DO UPDATE SET
  description = EXCLUDED.description,
  role_category = EXCLUDED.role_category,
  layout_category = EXCLUDED.layout_category,
  ubo_treatment = EXCLUDED.ubo_treatment,
  natural_person_only = EXCLUDED.natural_person_only,
  display_priority = EXCLUDED.display_priority,
  sort_order = EXCLUDED.sort_order,
  updated_at = now();

-- ============================================================================
-- A.2: Add LLP-Specific Roles
-- ============================================================================

INSERT INTO "ob-poc".roles (name, description, role_category, layout_category, ubo_treatment, requires_percentage, display_priority, sort_order)
VALUES
  ('DESIGNATED_MEMBER', 'LLP designated member - statutory signatory with filing duties', 'OWNERSHIP_CHAIN', 'PyramidUp', 'BENEFICIAL_OWNER', true, 30, 30),
  ('MEMBER', 'LLP member - ownership interest without designated status', 'OWNERSHIP_CHAIN', 'PyramidUp', 'BENEFICIAL_OWNER', true, 31, 31)
ON CONFLICT (name) DO UPDATE SET
  description = EXCLUDED.description,
  role_category = EXCLUDED.role_category,
  layout_category = EXCLUDED.layout_category,
  ubo_treatment = EXCLUDED.ubo_treatment,
  requires_percentage = EXCLUDED.requires_percentage,
  display_priority = EXCLUDED.display_priority,
  sort_order = EXCLUDED.sort_order,
  updated_at = now();

-- ============================================================================
-- A.3: Add Control-Specific Roles
-- ============================================================================

INSERT INTO "ob-poc".roles (name, description, role_category, layout_category, ubo_treatment, natural_person_only, display_priority, sort_order)
VALUES
  ('CONTROLLER', 'De facto control without ownership (SHA provisions, veto rights)', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', false, 35, 35),
  ('POWER_OF_ATTORNEY', 'Legal representative - can act for entity', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 36, 36),
  ('APPOINTOR', 'Trust role - can appoint/remove trustees', 'TRUST_ROLES', 'Radial', 'CONTROL_PERSON', false, 40, 40),
  ('ENFORCER', 'Trust role - charitable/purpose trust oversight', 'TRUST_ROLES', 'Radial', 'OVERSIGHT', false, 41, 41),
  ('PORTFOLIO_MANAGER', 'Day-to-day investment decisions', 'FUND_MANAGEMENT', 'Overlay', 'CONTROL_PERSON', true, 45, 45),
  ('KEY_PERSON', 'Subject to key-man clause provisions', 'FUND_MANAGEMENT', 'Overlay', 'KEY_PERSON', true, 46, 46),
  ('INVESTMENT_COMMITTEE_MEMBER', 'Member of investment committee', 'FUND_MANAGEMENT', 'Overlay', 'OVERSIGHT', true, 47, 47),
  ('CONDUCTING_OFFICER', 'CSSF conducting officer (Luxembourg funds)', 'FUND_MANAGEMENT', 'Overlay', 'CONTROL_PERSON', true, 48, 48)
ON CONFLICT (name) DO UPDATE SET
  description = EXCLUDED.description,
  role_category = EXCLUDED.role_category,
  layout_category = EXCLUDED.layout_category,
  ubo_treatment = EXCLUDED.ubo_treatment,
  natural_person_only = EXCLUDED.natural_person_only,
  display_priority = EXCLUDED.display_priority,
  sort_order = EXCLUDED.sort_order,
  updated_at = now();

-- ============================================================================
-- A.4: Role-to-Entity-Type Validation Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".role_applicable_entity_types (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  role_id UUID NOT NULL REFERENCES "ob-poc".roles(role_id) ON DELETE CASCADE,
  entity_type_code VARCHAR(100) NOT NULL,
  is_required BOOLEAN DEFAULT false,
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  UNIQUE(role_id, entity_type_code)
);

COMMENT ON TABLE "ob-poc".role_applicable_entity_types IS
  'Constrains which roles can be assigned to which entity types. E.g., TRUSTEE only for trust entities, GP only for partnerships.';
COMMENT ON COLUMN "ob-poc".role_applicable_entity_types.is_required IS
  'If true, this role MUST be assigned to an entity of this type (e.g., every trust must have a TRUSTEE)';

-- Index for quick lookup by entity type
CREATE INDEX IF NOT EXISTS idx_role_applicable_entity_type
  ON "ob-poc".role_applicable_entity_types(entity_type_code);

-- ============================================================================
-- Populate role-entity-type constraints
-- ============================================================================

-- Trust roles - only applicable to trust entities
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type_code, is_required, notes)
SELECT r.role_id, et.entity_type_code, et.is_required, et.notes
FROM "ob-poc".roles r
CROSS JOIN (VALUES
  ('TRUSTEE', 'TRUST_DISCRETIONARY', true, 'Every discretionary trust requires at least one trustee'),
  ('TRUSTEE', 'TRUST_FIXED_INTEREST', true, 'Every fixed interest trust requires at least one trustee'),
  ('TRUSTEE', 'TRUST_UNIT', true, 'Every unit trust requires at least one trustee'),
  ('TRUSTEE', 'TRUST_CHARITABLE', true, 'Every charitable trust requires at least one trustee'),
  ('SETTLOR', 'TRUST_DISCRETIONARY', false, NULL),
  ('SETTLOR', 'TRUST_FIXED_INTEREST', false, NULL),
  ('SETTLOR', 'TRUST_UNIT', false, NULL),
  ('SETTLOR', 'TRUST_CHARITABLE', false, NULL),
  ('BENEFICIARY', 'TRUST_DISCRETIONARY', false, NULL),
  ('BENEFICIARY', 'TRUST_FIXED_INTEREST', false, NULL),
  ('BENEFICIARY', 'TRUST_UNIT', false, NULL),
  ('BENEFICIARY', 'TRUST_CHARITABLE', false, NULL),
  ('PROTECTOR', 'TRUST_DISCRETIONARY', false, NULL),
  ('PROTECTOR', 'TRUST_FIXED_INTEREST', false, NULL),
  ('PROTECTOR', 'TRUST_UNIT', false, NULL),
  ('PROTECTOR', 'TRUST_CHARITABLE', false, NULL),
  ('APPOINTOR', 'TRUST_DISCRETIONARY', false, NULL),
  ('APPOINTOR', 'TRUST_FIXED_INTEREST', false, NULL),
  ('APPOINTOR', 'TRUST_UNIT', false, NULL),
  ('APPOINTOR', 'TRUST_CHARITABLE', false, NULL),
  ('ENFORCER', 'TRUST_CHARITABLE', false, 'Enforcer role specific to charitable/purpose trusts')
) AS et(role_name, entity_type_code, is_required, notes)
WHERE r.name = et.role_name
ON CONFLICT (role_id, entity_type_code) DO UPDATE SET
  is_required = EXCLUDED.is_required,
  notes = EXCLUDED.notes;

-- Partnership roles - only applicable to partnership entities
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type_code, is_required, notes)
SELECT r.role_id, et.entity_type_code, et.is_required, et.notes
FROM "ob-poc".roles r
CROSS JOIN (VALUES
  ('GENERAL_PARTNER', 'PARTNERSHIP_GENERAL', true, 'GP required for general partnership'),
  ('GENERAL_PARTNER', 'PARTNERSHIP_LIMITED', true, 'GP required for limited partnership'),
  ('LIMITED_PARTNER', 'PARTNERSHIP_LIMITED', false, NULL),
  ('MANAGING_PARTNER', 'PARTNERSHIP_GENERAL', false, NULL),
  ('MANAGING_PARTNER', 'PARTNERSHIP_LIMITED', false, NULL),
  ('MANAGING_PARTNER', 'PARTNERSHIP_LLP', false, NULL),
  ('DESIGNATED_MEMBER', 'PARTNERSHIP_LLP', true, 'LLP requires at least two designated members'),
  ('MEMBER', 'PARTNERSHIP_LLP', false, NULL)
) AS et(role_name, entity_type_code, is_required, notes)
WHERE r.name = et.role_name
ON CONFLICT (role_id, entity_type_code) DO UPDATE SET
  is_required = EXCLUDED.is_required,
  notes = EXCLUDED.notes;

-- Corporate roles - applicable to companies
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type_code, is_required, notes)
SELECT r.role_id, et.entity_type_code, et.is_required, et.notes
FROM "ob-poc".roles r
CROSS JOIN (VALUES
  ('DIRECTOR', 'LIMITED_COMPANY', true, 'Company requires at least one director'),
  ('DIRECTOR', 'PRIVATE_LIMITED_COMPANY', true, NULL),
  ('DIRECTOR', 'PUBLIC_LIMITED_COMPANY', true, NULL),
  ('CHAIRMAN', 'LIMITED_COMPANY', false, NULL),
  ('CHAIRMAN', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('CHAIRMAN', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('EXECUTIVE_DIRECTOR', 'LIMITED_COMPANY', false, NULL),
  ('EXECUTIVE_DIRECTOR', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('EXECUTIVE_DIRECTOR', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('NON_EXECUTIVE_DIRECTOR', 'LIMITED_COMPANY', false, NULL),
  ('NON_EXECUTIVE_DIRECTOR', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('NON_EXECUTIVE_DIRECTOR', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('CEO', 'LIMITED_COMPANY', false, NULL),
  ('CEO', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('CEO', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('CFO', 'LIMITED_COMPANY', false, NULL),
  ('CFO', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('CFO', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('SHAREHOLDER', 'LIMITED_COMPANY', false, NULL),
  ('SHAREHOLDER', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('SHAREHOLDER', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('COMPANY_SECRETARY', 'LIMITED_COMPANY', false, NULL),
  ('COMPANY_SECRETARY', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('COMPANY_SECRETARY', 'PUBLIC_LIMITED_COMPANY', true, 'Public companies require a company secretary')
) AS et(role_name, entity_type_code, is_required, notes)
WHERE r.name = et.role_name
ON CONFLICT (role_id, entity_type_code) DO UPDATE SET
  is_required = EXCLUDED.is_required,
  notes = EXCLUDED.notes;

-- Fund roles - applicable to fund entities
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type_code, is_required, notes)
SELECT r.role_id, et.entity_type_code, et.is_required, et.notes
FROM "ob-poc".roles r
CROSS JOIN (VALUES
  ('INVESTMENT_MANAGER', 'FUND_SICAV', false, NULL),
  ('INVESTMENT_MANAGER', 'FUND_ICAV', false, NULL),
  ('INVESTMENT_MANAGER', 'FUND_OEIC', false, NULL),
  ('INVESTMENT_MANAGER', 'FUND_FCP', false, NULL),
  ('MANAGEMENT_COMPANY', 'FUND_SICAV', true, 'UCITS/AIF requires authorized ManCo'),
  ('MANAGEMENT_COMPANY', 'FUND_ICAV', true, NULL),
  ('MANAGEMENT_COMPANY', 'FUND_OEIC', true, NULL),
  ('MANAGEMENT_COMPANY', 'FUND_FCP', true, NULL),
  ('DEPOSITARY', 'FUND_SICAV', true, 'Regulated funds require depositary'),
  ('DEPOSITARY', 'FUND_ICAV', true, NULL),
  ('DEPOSITARY', 'FUND_OEIC', true, NULL),
  ('DEPOSITARY', 'FUND_FCP', true, NULL),
  ('PORTFOLIO_MANAGER', 'FUND_SICAV', false, NULL),
  ('PORTFOLIO_MANAGER', 'FUND_ICAV', false, NULL),
  ('PORTFOLIO_MANAGER', 'FUND_OEIC', false, NULL),
  ('PORTFOLIO_MANAGER', 'FUND_FCP', false, NULL),
  ('CONDUCTING_OFFICER', 'FUND_SICAV', false, 'Luxembourg CSSF requirement'),
  ('CONDUCTING_OFFICER', 'FUND_FCP', false, NULL),
  ('KEY_PERSON', 'FUND_SICAV', false, NULL),
  ('KEY_PERSON', 'FUND_ICAV', false, NULL),
  ('KEY_PERSON', 'FUND_OEIC', false, NULL),
  ('KEY_PERSON', 'FUND_FCP', false, NULL)
) AS et(role_name, entity_type_code, is_required, notes)
WHERE r.name = et.role_name
ON CONFLICT (role_id, entity_type_code) DO UPDATE SET
  is_required = EXCLUDED.is_required,
  notes = EXCLUDED.notes;

-- ============================================================================
-- Create view for role applicability lookup
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_role_applicability AS
SELECT
  r.role_id,
  r.name AS role_name,
  r.description AS role_description,
  r.role_category,
  r.ubo_treatment,
  r.natural_person_only,
  r.legal_entity_only,
  COALESCE(
    array_agg(DISTINCT raet.entity_type_code) FILTER (WHERE raet.entity_type_code IS NOT NULL),
    ARRAY[]::varchar[]
  ) AS applicable_entity_types,
  COALESCE(
    array_agg(DISTINCT raet.entity_type_code) FILTER (WHERE raet.is_required = true),
    ARRAY[]::varchar[]
  ) AS required_for_entity_types
FROM "ob-poc".roles r
LEFT JOIN "ob-poc".role_applicable_entity_types raet ON r.role_id = raet.role_id
WHERE r.is_active = true
GROUP BY r.role_id, r.name, r.description, r.role_category, r.ubo_treatment, r.natural_person_only, r.legal_entity_only;

COMMENT ON VIEW "ob-poc".v_role_applicability IS
  'Aggregated view of roles with their applicable entity types for validation';

-- ============================================================================
-- Create function to validate role assignment
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".validate_role_for_entity_type(
  p_role_id UUID,
  p_entity_type_code VARCHAR
) RETURNS BOOLEAN AS $$
DECLARE
  v_has_constraints BOOLEAN;
  v_is_applicable BOOLEAN;
BEGIN
  -- Check if this role has any entity type constraints
  SELECT EXISTS(
    SELECT 1 FROM "ob-poc".role_applicable_entity_types
    WHERE role_id = p_role_id
  ) INTO v_has_constraints;

  -- If no constraints, role is applicable to all entity types
  IF NOT v_has_constraints THEN
    RETURN true;
  END IF;

  -- Check if entity type is in the allowed list
  SELECT EXISTS(
    SELECT 1 FROM "ob-poc".role_applicable_entity_types
    WHERE role_id = p_role_id
    AND entity_type_code = p_entity_type_code
  ) INTO v_is_applicable;

  RETURN v_is_applicable;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".validate_role_for_entity_type IS
  'Validates whether a role can be assigned to an entity of the given type. Returns true if allowed.';
-- Migration 009: KYC Control Enhancement - Schema Extensions
-- Phase B from TODO-KYC-CONTROL-ENHANCEMENT.md
-- Extends share_classes and adds board, trust, partnership, tollgate tables

-- ============================================================================
-- B.1: Extend share_classes for Capital Structure
-- ============================================================================

-- Add corporate capital structure columns to existing share_classes table
ALTER TABLE kyc.share_classes
  ADD COLUMN IF NOT EXISTS share_type VARCHAR(50),
  ADD COLUMN IF NOT EXISTS authorized_shares NUMERIC(20,0),
  ADD COLUMN IF NOT EXISTS issued_shares NUMERIC(20,0),
  ADD COLUMN IF NOT EXISTS voting_rights_per_share NUMERIC(10,4) DEFAULT 1.0,
  ADD COLUMN IF NOT EXISTS par_value NUMERIC(20,6),
  ADD COLUMN IF NOT EXISTS par_value_currency VARCHAR(3),
  ADD COLUMN IF NOT EXISTS dividend_rights BOOLEAN DEFAULT true,
  ADD COLUMN IF NOT EXISTS liquidation_preference NUMERIC(20,2),
  ADD COLUMN IF NOT EXISTS conversion_ratio NUMERIC(10,4),
  ADD COLUMN IF NOT EXISTS is_convertible BOOLEAN DEFAULT false,
  ADD COLUMN IF NOT EXISTS issuer_entity_id UUID REFERENCES "ob-poc".entities(entity_id);

COMMENT ON COLUMN kyc.share_classes.share_type IS 'ORDINARY, PREFERENCE_A, PREFERENCE_B, DEFERRED, REDEEMABLE, GROWTH, MANAGEMENT, CONVERTIBLE';
COMMENT ON COLUMN kyc.share_classes.authorized_shares IS 'Maximum shares authorized in articles/charter';
COMMENT ON COLUMN kyc.share_classes.issued_shares IS 'Actually issued shares - SUM(holdings.units) must equal this for reconciliation';
COMMENT ON COLUMN kyc.share_classes.voting_rights_per_share IS 'Votes per share (0 = non-voting, >1 = super-voting)';
COMMENT ON COLUMN kyc.share_classes.par_value IS 'Nominal/par value per share';
COMMENT ON COLUMN kyc.share_classes.liquidation_preference IS 'Priority claim amount on liquidation (typically for preference shares)';
COMMENT ON COLUMN kyc.share_classes.issuer_entity_id IS 'The entity (company) that issued these shares';

-- Add constraint for share_type
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.check_constraints
    WHERE constraint_name = 'chk_share_type'
    AND constraint_schema = 'kyc'
  ) THEN
    ALTER TABLE kyc.share_classes
      ADD CONSTRAINT chk_share_type CHECK (
        share_type IS NULL OR share_type IN (
          'ORDINARY', 'PREFERENCE_A', 'PREFERENCE_B', 'DEFERRED',
          'REDEEMABLE', 'GROWTH', 'MANAGEMENT', 'CONVERTIBLE',
          'COMMON', 'PREFERRED', 'RESTRICTED', 'FOUNDERS'
        )
      );
  END IF;
END $$;

-- Index for capital structure queries
CREATE INDEX IF NOT EXISTS idx_share_classes_issuer ON kyc.share_classes(issuer_entity_id);
CREATE INDEX IF NOT EXISTS idx_share_classes_type ON kyc.share_classes(share_type) WHERE share_type IS NOT NULL;

-- ============================================================================
-- B.2: Capital Structure View
-- ============================================================================

CREATE OR REPLACE VIEW kyc.v_capital_structure AS
SELECT
  sc.id AS share_class_id,
  sc.cbu_id,
  sc.issuer_entity_id,
  sc.name AS share_class_name,
  sc.share_type,
  sc.class_category,
  sc.authorized_shares,
  sc.issued_shares,
  sc.voting_rights_per_share,
  sc.par_value,
  sc.par_value_currency,
  sc.dividend_rights,
  sc.liquidation_preference,
  h.id AS holding_id,
  h.investor_entity_id,
  h.units,
  h.cost_basis,
  h.status AS holding_status,
  -- Ownership calculation
  CASE
    WHEN sc.issued_shares > 0 AND sc.issued_shares IS NOT NULL
    THEN ROUND((h.units / sc.issued_shares) * 100, 4)
    ELSE 0
  END AS ownership_pct,
  -- Voting rights calculation
  h.units * COALESCE(sc.voting_rights_per_share, 1) AS holder_voting_rights,
  -- Total voting rights for this share class
  sc.issued_shares * COALESCE(sc.voting_rights_per_share, 1) AS total_class_voting_rights,
  -- Voting percentage
  CASE
    WHEN sc.issued_shares > 0 AND sc.issued_shares IS NOT NULL AND sc.voting_rights_per_share > 0
    THEN ROUND((h.units / sc.issued_shares) * 100, 4)
    ELSE 0
  END AS voting_pct,
  -- Entity details
  e.name AS investor_name,
  et.type_code AS investor_entity_type,
  ie.name AS issuer_name,
  iet.type_code AS issuer_entity_type
FROM kyc.share_classes sc
LEFT JOIN kyc.holdings h ON h.share_class_id = sc.id AND h.status = 'active'
LEFT JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
LEFT JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entities ie ON ie.entity_id = sc.issuer_entity_id
LEFT JOIN "ob-poc".entity_types iet ON ie.entity_type_id = iet.entity_type_id
WHERE sc.class_category = 'CORPORATE' OR sc.share_type IS NOT NULL;

COMMENT ON VIEW kyc.v_capital_structure IS
  'Computed ownership and voting percentages from corporate share registry. Join with share_classes and holdings.';

-- ============================================================================
-- B.3: Board Compositions Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.board_compositions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
  entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  person_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  role_id UUID NOT NULL REFERENCES "ob-poc".roles(role_id),
  appointed_by_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
  appointment_date DATE,
  resignation_date DATE,
  is_active BOOLEAN DEFAULT true,
  appointment_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  appointment_source VARCHAR(50),
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_board_dates CHECK (resignation_date IS NULL OR resignation_date >= appointment_date),
  CONSTRAINT chk_appointment_source CHECK (
    appointment_source IS NULL OR appointment_source IN (
      'ARTICLES', 'SHA', 'BOARD_RESOLUTION', 'SHAREHOLDER_RESOLUTION',
      'REGULATOR_APPROVAL', 'COURT_ORDER', 'OTHER'
    )
  )
);

COMMENT ON TABLE kyc.board_compositions IS
  'Directors and officers appointed to entity boards with appointment chain for control analysis';
COMMENT ON COLUMN kyc.board_compositions.entity_id IS 'The entity (company/fund) whose board this is';
COMMENT ON COLUMN kyc.board_compositions.person_entity_id IS 'The person appointed to the board position';
COMMENT ON COLUMN kyc.board_compositions.appointed_by_entity_id IS 'Entity that exercised appointment right (for SHA-based appointments)';
COMMENT ON COLUMN kyc.board_compositions.appointment_source IS 'Legal basis for the appointment';

CREATE INDEX IF NOT EXISTS idx_board_entity ON kyc.board_compositions(entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_board_person ON kyc.board_compositions(person_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_board_appointer ON kyc.board_compositions(appointed_by_entity_id) WHERE appointed_by_entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_board_cbu ON kyc.board_compositions(cbu_id);

-- Unique constraint: same person can't hold same role on same entity board twice (while active)
CREATE UNIQUE INDEX IF NOT EXISTS idx_board_unique_active
  ON kyc.board_compositions(entity_id, person_entity_id, role_id)
  WHERE is_active = true;

-- ============================================================================
-- B.4: Appointment Rights Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.appointment_rights (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
  target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  right_type VARCHAR(50) NOT NULL,
  appointable_role_id UUID REFERENCES "ob-poc".roles(role_id),
  max_appointments INTEGER,
  current_appointments INTEGER DEFAULT 0,
  source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  source_clause TEXT,
  effective_from DATE,
  effective_to DATE,
  is_active BOOLEAN DEFAULT true,
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_right_type CHECK (right_type IN (
    'APPOINT_DIRECTOR', 'REMOVE_DIRECTOR', 'APPOINT_AND_REMOVE',
    'VETO_APPOINTMENT', 'CONSENT_REQUIRED', 'OBSERVER_SEAT',
    'APPOINT_CHAIRMAN', 'APPOINT_CEO', 'APPOINT_AUDITOR'
  )),
  CONSTRAINT chk_appointments CHECK (
    max_appointments IS NULL OR current_appointments <= max_appointments
  )
);

COMMENT ON TABLE kyc.appointment_rights IS
  'SHA/articles provisions granting board appointment/removal rights - key for control analysis';
COMMENT ON COLUMN kyc.appointment_rights.target_entity_id IS 'Entity whose board can be affected by this right';
COMMENT ON COLUMN kyc.appointment_rights.holder_entity_id IS 'Entity holding/exercising the appointment right';
COMMENT ON COLUMN kyc.appointment_rights.source_clause IS 'Reference to specific clause in SHA/articles (e.g., "Clause 5.2(a)")';
COMMENT ON COLUMN kyc.appointment_rights.max_appointments IS 'Maximum number of directors this right allows appointing';

CREATE INDEX IF NOT EXISTS idx_appt_rights_target ON kyc.appointment_rights(target_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_appt_rights_holder ON kyc.appointment_rights(holder_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_appt_rights_cbu ON kyc.appointment_rights(cbu_id);

-- Unique constraint: one right type per holder-target pair
CREATE UNIQUE INDEX IF NOT EXISTS idx_appt_rights_unique
  ON kyc.appointment_rights(target_entity_id, holder_entity_id, right_type)
  WHERE is_active = true;

-- ============================================================================
-- B.5: Trust Provisions Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.trust_provisions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
  trust_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  provision_type VARCHAR(50) NOT NULL,
  holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
  beneficiary_class TEXT,
  interest_percentage NUMERIC(8,4),
  discretion_level VARCHAR(30),
  vesting_conditions TEXT,
  vesting_date DATE,
  source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  source_clause TEXT,
  is_active BOOLEAN DEFAULT true,
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_provision_type CHECK (provision_type IN (
    'INCOME_BENEFICIARY', 'CAPITAL_BENEFICIARY', 'DISCRETIONARY_BENEFICIARY',
    'REMAINDER_BENEFICIARY', 'CONTINGENT_BENEFICIARY', 'DEFAULT_BENEFICIARY',
    'APPOINTOR_POWER', 'PROTECTOR_POWER', 'TRUSTEE_REMOVAL',
    'TRUST_VARIATION', 'ACCUMULATION_POWER', 'ADVANCEMENT_POWER',
    'INVESTMENT_DIRECTION', 'DISTRIBUTION_DIRECTION', 'ADD_BENEFICIARY',
    'EXCLUDE_BENEFICIARY', 'RESERVED_POWER'
  )),
  CONSTRAINT chk_discretion CHECK (discretion_level IS NULL OR discretion_level IN (
    'ABSOLUTE', 'LIMITED', 'NONE', 'FETTERED'
  )),
  CONSTRAINT chk_interest_pct CHECK (
    interest_percentage IS NULL OR (interest_percentage >= 0 AND interest_percentage <= 100)
  )
);

COMMENT ON TABLE kyc.trust_provisions IS
  'Trust deed provisions affecting control and beneficial interest - key for trust UBO analysis';
COMMENT ON COLUMN kyc.trust_provisions.trust_entity_id IS 'The trust entity this provision belongs to';
COMMENT ON COLUMN kyc.trust_provisions.holder_entity_id IS 'Entity holding this provision/right (NULL for class beneficiaries)';
COMMENT ON COLUMN kyc.trust_provisions.beneficiary_class IS 'Description of beneficiary class if not specific entity (e.g., "descendants of the settlor")';
COMMENT ON COLUMN kyc.trust_provisions.discretion_level IS 'Level of trustee discretion: ABSOLUTE=full, LIMITED=within parameters, NONE=mandatory, FETTERED=restricted';
COMMENT ON COLUMN kyc.trust_provisions.interest_percentage IS 'Fixed interest percentage for fixed interest trusts';

CREATE INDEX IF NOT EXISTS idx_trust_prov_trust ON kyc.trust_provisions(trust_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_trust_prov_holder ON kyc.trust_provisions(holder_entity_id) WHERE holder_entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_trust_prov_cbu ON kyc.trust_provisions(cbu_id);
CREATE INDEX IF NOT EXISTS idx_trust_prov_type ON kyc.trust_provisions(provision_type);

-- ============================================================================
-- B.6: Partnership Capital Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.partnership_capital (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
  partnership_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  partner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  partner_type VARCHAR(30) NOT NULL,
  capital_commitment NUMERIC(20,2),
  capital_contributed NUMERIC(20,2) DEFAULT 0,
  capital_returned NUMERIC(20,2) DEFAULT 0,
  unfunded_commitment NUMERIC(20,2) GENERATED ALWAYS AS (
    COALESCE(capital_commitment, 0) - COALESCE(capital_contributed, 0) + COALESCE(capital_returned, 0)
  ) STORED,
  profit_share_pct NUMERIC(8,4),
  loss_share_pct NUMERIC(8,4),
  management_fee_share_pct NUMERIC(8,4),
  carried_interest_pct NUMERIC(8,4),
  management_rights BOOLEAN DEFAULT false,
  voting_pct NUMERIC(8,4),
  admission_date DATE,
  withdrawal_date DATE,
  is_active BOOLEAN DEFAULT true,
  source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_partner_type CHECK (partner_type IN ('GP', 'LP', 'MEMBER', 'FOUNDING_PARTNER', 'SPECIAL_LP')),
  CONSTRAINT chk_capital CHECK (
    capital_contributed IS NULL OR capital_commitment IS NULL OR
    capital_contributed <= capital_commitment
  ),
  CONSTRAINT chk_percentages CHECK (
    (profit_share_pct IS NULL OR (profit_share_pct >= 0 AND profit_share_pct <= 100)) AND
    (loss_share_pct IS NULL OR (loss_share_pct >= 0 AND loss_share_pct <= 100)) AND
    (voting_pct IS NULL OR (voting_pct >= 0 AND voting_pct <= 100))
  ),
  UNIQUE(partnership_entity_id, partner_entity_id)
);

COMMENT ON TABLE kyc.partnership_capital IS
  'Partnership capital accounts and profit/loss allocation for LP/GP structures';
COMMENT ON COLUMN kyc.partnership_capital.partner_type IS 'GP=General Partner (control+liability), LP=Limited Partner (passive), MEMBER=LLP member';
COMMENT ON COLUMN kyc.partnership_capital.management_rights IS 'Whether partner has management rights (always true for GP)';
COMMENT ON COLUMN kyc.partnership_capital.unfunded_commitment IS 'Computed: commitment - contributed + returned';
COMMENT ON COLUMN kyc.partnership_capital.carried_interest_pct IS 'GP carried interest percentage (typically 20%)';

CREATE INDEX IF NOT EXISTS idx_partnership_cap_partnership ON kyc.partnership_capital(partnership_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_partnership_cap_partner ON kyc.partnership_capital(partner_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_partnership_cap_cbu ON kyc.partnership_capital(cbu_id);
CREATE INDEX IF NOT EXISTS idx_partnership_cap_type ON kyc.partnership_capital(partner_type);

-- ============================================================================
-- B.7: Tollgate Evaluation Tables
-- ============================================================================

-- Tollgate threshold configuration
CREATE TABLE IF NOT EXISTS kyc.tollgate_thresholds (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  threshold_name VARCHAR(100) NOT NULL UNIQUE,
  metric_type VARCHAR(50) NOT NULL,
  comparison VARCHAR(10) NOT NULL DEFAULT 'GTE',
  threshold_value NUMERIC(10,4),
  weight NUMERIC(5,2) DEFAULT 1.0,
  is_blocking BOOLEAN DEFAULT false,
  applies_to_case_types VARCHAR(50)[] DEFAULT ARRAY['NEW_CLIENT', 'PERIODIC_REVIEW', 'EVENT_DRIVEN'],
  description TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_metric_type CHECK (metric_type IN (
    'OWNERSHIP_VERIFIED_PCT', 'CONTROL_VERIFIED_PCT', 'UBO_COVERAGE_PCT',
    'DOC_COMPLETENESS_PCT', 'SCREENING_CLEAR_PCT', 'RED_FLAG_COUNT',
    'ALLEGATION_UNRESOLVED_COUNT', 'DAYS_SINCE_REFRESH', 'ENTITY_KYC_COMPLETE_PCT',
    'HIGH_RISK_ENTITY_COUNT', 'OPEN_DISCREPANCY_COUNT', 'EVIDENCE_FRESHNESS_DAYS'
  )),
  CONSTRAINT chk_comparison CHECK (comparison IN ('GT', 'GTE', 'LT', 'LTE', 'EQ', 'NEQ'))
);

COMMENT ON TABLE kyc.tollgate_thresholds IS
  'Configurable thresholds for tollgate pass/fail decisions in KYC workflow';
COMMENT ON COLUMN kyc.tollgate_thresholds.comparison IS 'Comparison operator: GT(>), GTE(>=), LT(<), LTE(<=), EQ(=), NEQ(!=)';
COMMENT ON COLUMN kyc.tollgate_thresholds.is_blocking IS 'If true, failing this threshold blocks case progression';
COMMENT ON COLUMN kyc.tollgate_thresholds.applies_to_case_types IS 'Case types this threshold applies to';

-- Populate default thresholds
INSERT INTO kyc.tollgate_thresholds (threshold_name, metric_type, comparison, threshold_value, is_blocking, weight, description)
VALUES
  ('ownership_minimum', 'OWNERSHIP_VERIFIED_PCT', 'GTE', 95.0, true, 1.0, 'Minimum verified ownership percentage'),
  ('control_minimum', 'CONTROL_VERIFIED_PCT', 'GTE', 100.0, true, 1.0, 'All control vectors must be verified'),
  ('ubo_coverage', 'UBO_COVERAGE_PCT', 'GTE', 100.0, true, 1.0, 'All UBOs must be identified'),
  ('doc_completeness', 'DOC_COMPLETENESS_PCT', 'GTE', 90.0, false, 0.8, 'Target document collection rate'),
  ('entity_kyc_complete', 'ENTITY_KYC_COMPLETE_PCT', 'GTE', 100.0, true, 1.0, 'All entities must complete KYC'),
  ('screening_clear', 'SCREENING_CLEAR_PCT', 'GTE', 100.0, true, 1.0, 'All screenings must be clear or escalated'),
  ('red_flag_limit', 'RED_FLAG_COUNT', 'LTE', 0, false, 0.9, 'Maximum unaddressed red flags'),
  ('allegation_limit', 'ALLEGATION_UNRESOLVED_COUNT', 'LTE', 0, true, 1.0, 'No unresolved ownership allegations'),
  ('discrepancy_limit', 'OPEN_DISCREPANCY_COUNT', 'LTE', 0, false, 0.7, 'No open discrepancies'),
  ('high_risk_limit', 'HIGH_RISK_ENTITY_COUNT', 'LTE', 0, false, 0.8, 'High risk entities require escalation'),
  ('evidence_freshness', 'EVIDENCE_FRESHNESS_DAYS', 'LTE', 365, false, 0.5, 'Evidence should be less than 1 year old')
ON CONFLICT (threshold_name) DO UPDATE SET
  metric_type = EXCLUDED.metric_type,
  comparison = EXCLUDED.comparison,
  threshold_value = EXCLUDED.threshold_value,
  is_blocking = EXCLUDED.is_blocking,
  weight = EXCLUDED.weight,
  description = EXCLUDED.description,
  updated_at = now();

-- Tollgate evaluations
CREATE TABLE IF NOT EXISTS kyc.tollgate_evaluations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  evaluation_type VARCHAR(30) NOT NULL,
  evaluated_at TIMESTAMPTZ DEFAULT now(),
  evaluated_by VARCHAR(100),
  overall_result VARCHAR(20) NOT NULL,
  score NUMERIC(5,2),
  metrics JSONB NOT NULL DEFAULT '{}',
  threshold_results JSONB NOT NULL DEFAULT '{}',
  blocking_failures TEXT[],
  warnings TEXT[],
  override_id UUID,
  notes TEXT,
  CONSTRAINT chk_eval_type CHECK (evaluation_type IN (
    'DISCOVERY_COMPLETE', 'EVIDENCE_COMPLETE', 'VERIFICATION_COMPLETE',
    'DECISION_READY', 'PERIODIC_REVIEW', 'EVENT_TRIGGERED'
  )),
  CONSTRAINT chk_result CHECK (overall_result IN ('PASS', 'FAIL', 'CONDITIONAL', 'OVERRIDE', 'PENDING'))
);

COMMENT ON TABLE kyc.tollgate_evaluations IS
  'Point-in-time tollgate evaluation results for KYC cases';
COMMENT ON COLUMN kyc.tollgate_evaluations.metrics IS
  'JSON object with all computed metrics at evaluation time';
COMMENT ON COLUMN kyc.tollgate_evaluations.threshold_results IS
  'JSON object with per-threshold pass/fail results';
COMMENT ON COLUMN kyc.tollgate_evaluations.blocking_failures IS
  'Array of threshold names that caused blocking failure';

CREATE INDEX IF NOT EXISTS idx_tollgate_case ON kyc.tollgate_evaluations(case_id);
CREATE INDEX IF NOT EXISTS idx_tollgate_cbu ON kyc.tollgate_evaluations(cbu_id);
CREATE INDEX IF NOT EXISTS idx_tollgate_result ON kyc.tollgate_evaluations(overall_result);
CREATE INDEX IF NOT EXISTS idx_tollgate_type ON kyc.tollgate_evaluations(evaluation_type);
CREATE INDEX IF NOT EXISTS idx_tollgate_date ON kyc.tollgate_evaluations(evaluated_at DESC);

-- Tollgate overrides (management override of failed tollgate)
CREATE TABLE IF NOT EXISTS kyc.tollgate_overrides (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  evaluation_id UUID NOT NULL REFERENCES kyc.tollgate_evaluations(id) ON DELETE CASCADE,
  override_reason TEXT NOT NULL,
  approved_by VARCHAR(100) NOT NULL,
  approved_at TIMESTAMPTZ DEFAULT now(),
  approval_authority VARCHAR(50) NOT NULL,
  conditions TEXT,
  expiry_date DATE,
  is_active BOOLEAN DEFAULT true,
  review_required_by DATE,
  created_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_authority CHECK (approval_authority IN (
    'ANALYST', 'SENIOR_ANALYST', 'TEAM_LEAD', 'COMPLIANCE_OFFICER',
    'SENIOR_COMPLIANCE', 'MLRO', 'EXECUTIVE', 'BOARD'
  ))
);

COMMENT ON TABLE kyc.tollgate_overrides IS
  'Management overrides for tollgate failures with audit trail';
COMMENT ON COLUMN kyc.tollgate_overrides.approval_authority IS 'Level of authority that approved the override';
COMMENT ON COLUMN kyc.tollgate_overrides.conditions IS 'Any conditions attached to the override';
COMMENT ON COLUMN kyc.tollgate_overrides.review_required_by IS 'Date by which the override must be reviewed';

CREATE INDEX IF NOT EXISTS idx_override_eval ON kyc.tollgate_overrides(evaluation_id);
CREATE INDEX IF NOT EXISTS idx_override_active ON kyc.tollgate_overrides(is_active) WHERE is_active = true;
-- ============================================================================
-- Migration: 010_bods_gleif_integration.sql
-- Purpose: BODS 0.4 + GLEIF Deep Integration
-- Date: 2025-01-08
--
-- Reference: ai-thoughts/012-bods-gleif-integration-TODO.md
--
-- This migration:
-- 1. EXTENDS entity_identifiers with BODS/LEI validation columns
-- 2. Creates gleif_relationships table (corporate hierarchy, SEPARATE from UBO)
-- 3. Creates bods_interest_types codelist (22 standard types)
-- 4. Extends entity_relationships with BODS fields
-- 5. Creates person_pep_status table (detailed PEP tracking)
-- 6. Creates bods_entity_types codelist
-- 7. Adds BODS columns to entities table
-- ============================================================================

BEGIN;

-- Record migration (check if already recorded)
INSERT INTO "ob-poc".schema_changes (change_type, description, script_name)
SELECT 'ENHANCEMENT', 'BODS 0.4 + GLEIF Deep Integration', '010_bods_gleif_integration.sql'
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".schema_changes WHERE script_name = '010_bods_gleif_integration.sql'
);

-- ============================================================================
-- 1. EXTEND ENTITY_IDENTIFIERS (Already exists - add BODS columns)
-- ============================================================================
-- Existing columns: identifier_id, entity_id, identifier_type, identifier_value,
--                   issuing_authority, is_primary, valid_from, valid_until, source, created_at

-- Add BODS/LEI-specific columns
ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS scheme_name VARCHAR(100);  -- Human readable: 'Global LEI System'

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS uri VARCHAR(500);  -- Full URI (e.g., GLEIF API URL)

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS is_validated BOOLEAN DEFAULT false;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS validated_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS validation_source VARCHAR(100);  -- 'GLEIF_API', 'MANUAL'

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS validation_details JSONB DEFAULT '{}';

-- LEI-specific fields
ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_status VARCHAR(30);  -- 'ISSUED', 'LAPSED', 'RETIRED'

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_next_renewal DATE;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_managing_lou VARCHAR(100);

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_initial_registration DATE;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_last_update TIMESTAMPTZ;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

-- Create index on identifier_type for LEI lookups
CREATE INDEX IF NOT EXISTS idx_entity_identifiers_lei
    ON "ob-poc".entity_identifiers(identifier_value) WHERE identifier_type = 'LEI';

CREATE INDEX IF NOT EXISTS idx_entity_identifiers_lei_status
    ON "ob-poc".entity_identifiers(lei_status) WHERE identifier_type = 'LEI';

COMMENT ON TABLE "ob-poc".entity_identifiers IS 'Unified identifier storage for entities. LEI is the global spine, supports any identifier scheme.';

-- ============================================================================
-- 2. GLEIF RELATIONSHIPS (Corporate Hierarchy - SEPARATE FROM UBO)
-- ============================================================================
-- CRITICAL: This stores GLEIF corporate structure (accounting consolidation)
-- NOT the same as beneficial ownership (KYC). Keep semantics separate!

CREATE TABLE IF NOT EXISTS "ob-poc".gleif_relationships (
    gleif_rel_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Parent entity (owner in GLEIF consolidation terms)
    parent_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    parent_lei VARCHAR(20) NOT NULL,

    -- Child entity (owned in GLEIF consolidation terms)
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    child_lei VARCHAR(20) NOT NULL,

    -- GLEIF relationship semantics
    relationship_type VARCHAR(50) NOT NULL,  -- IS_DIRECTLY_CONSOLIDATED_BY, IS_ULTIMATELY_CONSOLIDATED_BY, IS_FUND_MANAGED_BY
    relationship_status VARCHAR(30),         -- ACTIVE, INACTIVE
    relationship_qualifier VARCHAR(50),

    -- Ownership from GLEIF (for consolidation, NOT UBO)
    ownership_percentage NUMERIC(5,2),
    ownership_percentage_min NUMERIC(5,2),
    ownership_percentage_max NUMERIC(5,2),

    -- Accounting consolidation info
    accounting_standard VARCHAR(50),  -- IFRS, US_GAAP, OTHER

    -- Temporal
    start_date DATE,
    end_date DATE,

    -- Source tracking
    gleif_record_id VARCHAR(100),
    gleif_registration_status VARCHAR(30),
    fetched_at TIMESTAMPTZ DEFAULT NOW(),
    raw_data JSONB,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Natural key
    UNIQUE(parent_lei, child_lei, relationship_type)
);

CREATE INDEX IF NOT EXISTS idx_gleif_rel_parent ON "ob-poc".gleif_relationships(parent_entity_id);
CREATE INDEX IF NOT EXISTS idx_gleif_rel_child ON "ob-poc".gleif_relationships(child_entity_id);
CREATE INDEX IF NOT EXISTS idx_gleif_rel_parent_lei ON "ob-poc".gleif_relationships(parent_lei);
CREATE INDEX IF NOT EXISTS idx_gleif_rel_child_lei ON "ob-poc".gleif_relationships(child_lei);
CREATE INDEX IF NOT EXISTS idx_gleif_rel_type ON "ob-poc".gleif_relationships(relationship_type);

COMMENT ON TABLE "ob-poc".gleif_relationships IS 'GLEIF corporate hierarchy (consolidation). SEPARATE from entity_relationships (UBO/KYC). GLEIF = accounting, UBO = beneficial ownership.';

-- ============================================================================
-- 3. BODS INTEREST TYPES (Codelist - 22 Standard Types)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_interest_types (
    type_code VARCHAR(50) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    category VARCHAR(30) NOT NULL,  -- ownership, control, trust, contractual, nominee, unknown
    description TEXT,
    bods_standard BOOLEAN DEFAULT true,
    requires_percentage BOOLEAN DEFAULT false,
    display_order INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Populate BODS 0.4 standard interest types
INSERT INTO "ob-poc".bods_interest_types (type_code, display_name, category, requires_percentage, display_order) VALUES
-- Ownership interests
('shareholding', 'Shareholding', 'ownership', true, 1),
('votingRights', 'Voting Rights', 'ownership', true, 2),
('rightsToSurplusAssetsOnDissolution', 'Rights to Surplus Assets on Dissolution', 'ownership', true, 3),
('rightsToProfitOrIncome', 'Rights to Profit or Income', 'ownership', true, 4),
-- Control interests
('appointmentOfBoard', 'Appointment of Board', 'control', false, 10),
('otherInfluenceOrControl', 'Other Influence or Control', 'control', false, 11),
('seniorManagingOfficial', 'Senior Managing Official', 'control', false, 12),
('controlViaCompanyRulesOrArticles', 'Control via Company Rules/Articles', 'control', false, 13),
('controlByLegalFramework', 'Control by Legal Framework', 'control', false, 14),
('boardMember', 'Board Member', 'control', false, 15),
('boardChair', 'Board Chair', 'control', false, 16),
-- Trust interests
('settlor', 'Settlor', 'trust', false, 20),
('trustee', 'Trustee', 'trust', false, 21),
('protector', 'Protector', 'trust', false, 22),
('beneficiaryOfLegalArrangement', 'Beneficiary of Legal Arrangement', 'trust', true, 23),
('enjoymentAndUseOfAssets', 'Enjoyment and Use of Assets', 'trust', false, 24),
('rightToProfitOrIncomeFromAssets', 'Right to Profit/Income from Assets', 'trust', true, 25),
-- Contractual interests
('rightsGrantedByContract', 'Rights Granted by Contract', 'contractual', false, 30),
('conditionalRightsGrantedByContract', 'Conditional Rights Granted by Contract', 'contractual', false, 31),
-- Nominee interests
('nominee', 'Nominee', 'nominee', false, 40),
('nominator', 'Nominator', 'nominee', false, 41),
-- Unknown
('unknownInterest', 'Unknown Interest', 'unknown', false, 50),
('unpublishedInterest', 'Unpublished Interest', 'unknown', false, 51)
ON CONFLICT (type_code) DO NOTHING;

COMMENT ON TABLE "ob-poc".bods_interest_types IS 'BODS 0.4 standard interest types (22 types). Codelist for entity_relationships.interest_type.';

-- ============================================================================
-- 4. BODS ENTITY TYPES (Codelist)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_entity_types (
    type_code VARCHAR(30) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    description TEXT,
    subtypes JSONB DEFAULT '[]',
    display_order INTEGER DEFAULT 0
);

INSERT INTO "ob-poc".bods_entity_types (type_code, display_name, subtypes, display_order) VALUES
('registeredEntity', 'Registered Entity', '["other"]', 1),
('legalEntity', 'Legal Entity', '["trust", "other"]', 2),
('arrangement', 'Arrangement', '["trust", "nomination", "other"]', 3),
('anonymousEntity', 'Anonymous Entity', '["other"]', 4),
('unknownEntity', 'Unknown Entity', '["other"]', 5),
('state', 'State', '["other"]', 6),
('stateBody', 'State Body', '["governmentDepartment", "stateAgency", "other"]', 7)
ON CONFLICT (type_code) DO NOTHING;

COMMENT ON TABLE "ob-poc".bods_entity_types IS 'BODS 0.4 entity type codelist.';

-- ============================================================================
-- 5. EXTEND ENTITY_RELATIONSHIPS FOR BODS ALIGNMENT
-- ============================================================================

-- Add interest_type column if not exists, or widen if it exists
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS interest_type VARCHAR(50);

-- Drop ALL dependent views before altering column type
DROP VIEW IF EXISTS "ob-poc".entity_relationships_current;
DROP VIEW IF EXISTS "ob-poc".cbu_ownership_graph;

-- Widen existing interest_type column to accommodate BODS codes (beneficiaryOfLegalArrangement = 28 chars)
ALTER TABLE "ob-poc".entity_relationships
ALTER COLUMN interest_type TYPE VARCHAR(50);

-- Also widen the history table column
ALTER TABLE "ob-poc".entity_relationships_history
ALTER COLUMN interest_type TYPE VARCHAR(50);

-- Recreate entity_relationships_current view
CREATE OR REPLACE VIEW "ob-poc".entity_relationships_current AS
SELECT relationship_id,
    from_entity_id,
    to_entity_id,
    relationship_type,
    percentage,
    ownership_type,
    control_type,
    trust_role,
    interest_type,
    effective_from,
    effective_to,
    source,
    source_document_ref,
    notes,
    created_at,
    created_by,
    updated_at
FROM "ob-poc".entity_relationships
WHERE effective_to IS NULL OR effective_to > CURRENT_DATE;

-- Recreate cbu_ownership_graph view
CREATE OR REPLACE VIEW "ob-poc".cbu_ownership_graph AS
SELECT v.cbu_id,
    r.relationship_id,
    r.from_entity_id,
    e_from.name AS from_entity_name,
    et_from.entity_category AS from_entity_category,
    r.to_entity_id,
    e_to.name AS to_entity_name,
    et_to.entity_category AS to_entity_category,
    r.relationship_type,
    r.percentage,
    r.ownership_type,
    r.control_type,
    r.trust_role,
    r.interest_type,
    v.status AS verification_status,
    v.alleged_percentage,
    v.observed_percentage,
    v.proof_document_id,
    r.effective_from,
    r.effective_to
FROM "ob-poc".entity_relationships r
JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
LEFT JOIN "ob-poc".entities e_from ON e_from.entity_id = r.from_entity_id
LEFT JOIN "ob-poc".entity_types et_from ON et_from.entity_type_id = e_from.entity_type_id
LEFT JOIN "ob-poc".entities e_to ON e_to.entity_id = r.to_entity_id
LEFT JOIN "ob-poc".entity_types et_to ON et_to.entity_type_id = e_to.entity_type_id
WHERE r.effective_to IS NULL OR r.effective_to > CURRENT_DATE;

-- Direct or indirect ownership/control
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS direct_or_indirect VARCHAR(10) DEFAULT 'unknown';

-- Share ranges (BODS allows min/max, not just exact)
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS share_minimum NUMERIC(5,2);

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS share_maximum NUMERIC(5,2);

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS share_exclusive_minimum BOOLEAN DEFAULT false;

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS share_exclusive_maximum BOOLEAN DEFAULT false;

-- Component tracking for indirect chains
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS is_component BOOLEAN DEFAULT false;

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS component_of_relationship_id UUID;

-- Statement tracking
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS statement_date DATE;

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS replaces_relationship_id UUID;

-- Index for interest type
CREATE INDEX IF NOT EXISTS idx_entity_rel_interest_type
    ON "ob-poc".entity_relationships(interest_type);

CREATE INDEX IF NOT EXISTS idx_entity_rel_component_of
    ON "ob-poc".entity_relationships(component_of_relationship_id)
    WHERE is_component = true;

-- ============================================================================
-- 6. PERSON PEP STATUS
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".person_pep_status (
    pep_status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    status VARCHAR(20) NOT NULL CHECK (status IN ('isPep', 'isNotPep', 'unknown')),
    reason TEXT,
    jurisdiction VARCHAR(10),
    position_held TEXT,
    position_level VARCHAR(30),
    start_date DATE,
    end_date DATE,

    source_type VARCHAR(50),
    source_reference TEXT,
    screening_id UUID,

    verified_at TIMESTAMPTZ,
    verified_by VARCHAR(255),
    verification_notes TEXT,
    pep_risk_level VARCHAR(20),

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_person_pep_entity ON "ob-poc".person_pep_status(person_entity_id);
CREATE INDEX IF NOT EXISTS idx_person_pep_status ON "ob-poc".person_pep_status(status);
-- Note: Partial index for active PEP status - using end_date IS NULL for currently active
-- Runtime filtering on end_date > CURRENT_DATE should be done in queries
CREATE INDEX IF NOT EXISTS idx_person_pep_active
    ON "ob-poc".person_pep_status(person_entity_id)
    WHERE status = 'isPep' AND end_date IS NULL;

COMMENT ON TABLE "ob-poc".person_pep_status IS 'BODS-compliant PEP status tracking.';

-- ============================================================================
-- 7. ADD BODS COLUMNS TO ENTITIES TABLE
-- ============================================================================

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS bods_entity_type VARCHAR(30);

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS bods_entity_subtype VARCHAR(30);

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS founding_date DATE;

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS dissolution_date DATE;

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS is_publicly_listed BOOLEAN DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_entities_bods_type ON "ob-poc".entities(bods_entity_type);

-- ============================================================================
-- 8. HELPER VIEWS
-- ============================================================================

-- View: Entities with their LEIs
CREATE OR REPLACE VIEW "ob-poc".v_entities_with_lei AS
SELECT
    e.entity_id,
    e.name,
    e.bods_entity_type,
    et.type_code AS entity_type_code,
    ei.identifier_value AS lei,
    ei.lei_status,
    ei.lei_next_renewal,
    ei.is_validated AS lei_validated
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
LEFT JOIN "ob-poc".entity_identifiers ei ON ei.entity_id = e.entity_id AND ei.identifier_type = 'LEI';

-- View: UBO interests with BODS type info
CREATE OR REPLACE VIEW "ob-poc".v_ubo_interests AS
SELECT
    er.relationship_id,
    er.from_entity_id AS interested_party_id,
    owner.name AS interested_party_name,
    er.to_entity_id AS subject_id,
    subject.name AS subject_name,
    er.interest_type,
    bit.display_name AS interest_type_display,
    bit.category AS interest_category,
    er.direct_or_indirect,
    COALESCE(er.percentage, er.share_minimum, er.share_maximum) AS ownership_share,
    er.share_minimum,
    er.share_maximum,
    er.effective_from,
    er.effective_to,
    er.is_component,
    er.component_of_relationship_id
FROM "ob-poc".entity_relationships er
JOIN "ob-poc".entities owner ON owner.entity_id = er.from_entity_id
JOIN "ob-poc".entities subject ON subject.entity_id = er.to_entity_id
LEFT JOIN "ob-poc".bods_interest_types bit ON bit.type_code = er.interest_type
WHERE er.relationship_type IN ('ownership', 'control', 'trust_role')
  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE);

-- View: GLEIF corporate hierarchy
CREATE OR REPLACE VIEW "ob-poc".v_gleif_hierarchy AS
SELECT
    gr.gleif_rel_id,
    gr.parent_entity_id,
    parent.name AS parent_name,
    gr.parent_lei,
    gr.child_entity_id,
    child.name AS child_name,
    gr.child_lei,
    gr.relationship_type,
    gr.relationship_status,
    gr.ownership_percentage,
    gr.accounting_standard,
    gr.start_date,
    gr.end_date
FROM "ob-poc".gleif_relationships gr
JOIN "ob-poc".entities parent ON parent.entity_id = gr.parent_entity_id
JOIN "ob-poc".entities child ON child.entity_id = gr.child_entity_id
WHERE gr.relationship_status = 'ACTIVE' OR gr.relationship_status IS NULL;

-- ============================================================================
-- 9. BACKFILL: Map existing relationship_type to interest_type
-- ============================================================================

UPDATE "ob-poc".entity_relationships
SET interest_type = CASE
    WHEN relationship_type = 'ownership' AND (ownership_type = 'DIRECT' OR ownership_type IS NULL) THEN 'shareholding'
    WHEN relationship_type = 'ownership' AND ownership_type = 'VOTING' THEN 'votingRights'
    WHEN relationship_type = 'ownership' AND ownership_type = 'BENEFICIAL' THEN 'shareholding'
    WHEN relationship_type = 'control' AND control_type = 'board_member' THEN 'boardMember'
    WHEN relationship_type = 'control' AND control_type = 'executive' THEN 'seniorManagingOfficial'
    WHEN relationship_type = 'control' AND control_type = 'voting_rights' THEN 'votingRights'
    WHEN relationship_type = 'control' THEN 'otherInfluenceOrControl'
    WHEN relationship_type = 'trust_role' AND trust_role = 'settlor' THEN 'settlor'
    WHEN relationship_type = 'trust_role' AND trust_role = 'trustee' THEN 'trustee'
    WHEN relationship_type = 'trust_role' AND trust_role = 'beneficiary' THEN 'beneficiaryOfLegalArrangement'
    WHEN relationship_type = 'trust_role' AND trust_role = 'protector' THEN 'protector'
    ELSE NULL
END
WHERE interest_type IS NULL;

UPDATE "ob-poc".entity_relationships
SET direct_or_indirect = CASE
    WHEN ownership_type = 'DIRECT' THEN 'direct'
    WHEN ownership_type = 'INDIRECT' THEN 'indirect'
    ELSE 'unknown'
END
WHERE direct_or_indirect = 'unknown' OR direct_or_indirect IS NULL;

COMMIT;
-- =============================================================================
-- Migration 011: Investor Register with Full Lifecycle
-- =============================================================================
-- Purpose: Dual-purpose investor register supporting:
--   Use Case A: Transfer Agency KYC-as-a-Service (client's end investors)
--   Use Case B: UBO Intra-Group Holdings (≥25% = UBO candidate)
--
-- Replaces: 011_clearstream_investor_views.sql (provider-agnostic now)
-- =============================================================================

-- Drop old Clearstream-specific views (will be replaced with generic versions)
DROP VIEW IF EXISTS kyc.v_clearstream_register CASCADE;
DROP VIEW IF EXISTS kyc.v_clearstream_movements CASCADE;
DROP VIEW IF EXISTS kyc.v_bods_ownership_statements CASCADE;
DROP VIEW IF EXISTS kyc.v_share_class_summary CASCADE;
DROP VIEW IF EXISTS kyc.v_investor_portfolio CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_entity_identifier_xref CASCADE;

-- =============================================================================
-- PHASE 1: INVESTORS TABLE & LIFECYCLE STATE MACHINE
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1.1 Investors Table (links entity to investor-specific data)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS kyc.investors (
    investor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Link to entity (person or company)
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Investor classification
    investor_type VARCHAR(50) NOT NULL,
    investor_category VARCHAR(50),

    -- Lifecycle state (the investor's journey)
    lifecycle_state VARCHAR(50) NOT NULL DEFAULT 'ENQUIRY',
    lifecycle_state_at TIMESTAMPTZ DEFAULT NOW(),
    lifecycle_notes TEXT,

    -- KYC status (separate from lifecycle - an investor can be SUBSCRIBED but KYC_EXPIRED)
    kyc_status VARCHAR(50) NOT NULL DEFAULT 'NOT_STARTED',
    kyc_case_id UUID,  -- Current/latest KYC case
    kyc_approved_at TIMESTAMPTZ,
    kyc_expires_at TIMESTAMPTZ,
    kyc_risk_rating VARCHAR(20),

    -- Tax & regulatory
    tax_status VARCHAR(50),
    tax_jurisdiction VARCHAR(10),
    fatca_status VARCHAR(50),
    crs_status VARCHAR(50),

    -- Eligibility & restrictions
    eligible_fund_types TEXT[],  -- Array of fund types investor can access
    restricted_jurisdictions TEXT[],

    -- Data source tracking
    provider VARCHAR(50) DEFAULT 'MANUAL',
    provider_reference VARCHAR(100),
    provider_sync_at TIMESTAMPTZ,

    -- Context
    owning_cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),  -- Which client "owns" this investor

    -- Rejection/suspension tracking
    rejection_reason TEXT,
    suspended_reason TEXT,
    pre_suspension_state VARCHAR(50),  -- State before suspension (for reinstatement)
    suspended_at TIMESTAMPTZ,
    offboard_reason TEXT,
    offboarded_at TIMESTAMPTZ,

    -- Subscription tracking
    first_subscription_at TIMESTAMPTZ,
    redemption_type VARCHAR(50),  -- FULL, PARTIAL (when in REDEEMING state)

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Unique: one investor record per entity per owning client
    UNIQUE(entity_id, owning_cbu_id)
);

COMMENT ON TABLE kyc.investors IS
'Investor register linking entities to investor-specific lifecycle and KYC status';

COMMENT ON COLUMN kyc.investors.investor_type IS
'RETAIL, PROFESSIONAL, INSTITUTIONAL, NOMINEE, INTRA_GROUP';

COMMENT ON COLUMN kyc.investors.investor_category IS
'HIGH_NET_WORTH, PENSION_FUND, INSURANCE, SOVEREIGN_WEALTH, FAMILY_OFFICE, CORPORATE, INDIVIDUAL';

COMMENT ON COLUMN kyc.investors.lifecycle_state IS
'ENQUIRY, PENDING_DOCUMENTS, KYC_IN_PROGRESS, KYC_APPROVED, KYC_REJECTED, ELIGIBLE_TO_SUBSCRIBE, SUBSCRIBED, ACTIVE_HOLDER, REDEEMING, OFFBOARDED, SUSPENDED, BLOCKED';

COMMENT ON COLUMN kyc.investors.kyc_status IS
'NOT_STARTED, IN_PROGRESS, APPROVED, REJECTED, EXPIRED, REFRESH_REQUIRED';

COMMENT ON COLUMN kyc.investors.provider IS
'CLEARSTREAM, EUROCLEAR, CSV_IMPORT, API_FEED, MANUAL';

COMMENT ON COLUMN kyc.investors.owning_cbu_id IS
'The BNY client (fund manager) who owns this investor relationship';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_investors_entity ON kyc.investors(entity_id);
CREATE INDEX IF NOT EXISTS idx_investors_lifecycle ON kyc.investors(lifecycle_state);
CREATE INDEX IF NOT EXISTS idx_investors_kyc_status ON kyc.investors(kyc_status);
CREATE INDEX IF NOT EXISTS idx_investors_owning_cbu ON kyc.investors(owning_cbu_id);
CREATE INDEX IF NOT EXISTS idx_investors_provider ON kyc.investors(provider, provider_reference);
CREATE INDEX IF NOT EXISTS idx_investors_kyc_expires ON kyc.investors(kyc_expires_at) WHERE kyc_expires_at IS NOT NULL;

-- -----------------------------------------------------------------------------
-- 1.2 Lifecycle State Transitions (validation table)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS kyc.investor_lifecycle_transitions (
    from_state VARCHAR(50) NOT NULL,
    to_state VARCHAR(50) NOT NULL,
    requires_kyc_approved BOOLEAN DEFAULT false,
    requires_document TEXT,  -- Document type required for transition
    auto_trigger VARCHAR(100),  -- Event that auto-triggers this transition
    PRIMARY KEY (from_state, to_state)
);

COMMENT ON TABLE kyc.investor_lifecycle_transitions IS
'Valid state transitions for investor lifecycle with requirements';

-- Insert valid transitions
INSERT INTO kyc.investor_lifecycle_transitions (from_state, to_state, requires_kyc_approved, auto_trigger) VALUES
-- Initial journey
('ENQUIRY', 'PENDING_DOCUMENTS', false, NULL),
('PENDING_DOCUMENTS', 'KYC_IN_PROGRESS', false, 'ALL_DOCS_RECEIVED'),
('KYC_IN_PROGRESS', 'KYC_APPROVED', false, 'KYC_CASE_APPROVED'),
('KYC_IN_PROGRESS', 'KYC_REJECTED', false, 'KYC_CASE_REJECTED'),
('KYC_APPROVED', 'ELIGIBLE_TO_SUBSCRIBE', true, NULL),

-- Subscription journey
('ELIGIBLE_TO_SUBSCRIBE', 'SUBSCRIBED', true, 'FIRST_SUBSCRIPTION'),
('SUBSCRIBED', 'ACTIVE_HOLDER', true, 'SUBSCRIPTION_SETTLED'),

-- Exit journey
('ACTIVE_HOLDER', 'REDEEMING', true, 'FULL_REDEMPTION_REQUESTED'),
('REDEEMING', 'OFFBOARDED', false, 'REDEMPTION_SETTLED'),

-- Exceptional states
('ACTIVE_HOLDER', 'SUSPENDED', false, NULL),
('SUSPENDED', 'ACTIVE_HOLDER', true, NULL),
('ACTIVE_HOLDER', 'BLOCKED', false, NULL),
('SUBSCRIBED', 'BLOCKED', false, NULL),
('ELIGIBLE_TO_SUBSCRIBE', 'SUSPENDED', false, NULL),
('SUSPENDED', 'ELIGIBLE_TO_SUBSCRIBE', true, NULL),

-- Re-engagement
('KYC_REJECTED', 'PENDING_DOCUMENTS', false, NULL),
('OFFBOARDED', 'ENQUIRY', false, NULL)

ON CONFLICT DO NOTHING;

-- -----------------------------------------------------------------------------
-- 1.3 Lifecycle Transition Validation Trigger
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION kyc.validate_investor_lifecycle_transition()
RETURNS TRIGGER AS $$
BEGIN
    -- Allow if transition is valid
    IF EXISTS (
        SELECT 1 FROM kyc.investor_lifecycle_transitions
        WHERE from_state = OLD.lifecycle_state
          AND to_state = NEW.lifecycle_state
    ) THEN
        NEW.lifecycle_state_at := NOW();
        NEW.updated_at := NOW();
        RETURN NEW;
    END IF;

    -- Reject invalid transition
    RAISE EXCEPTION 'Invalid lifecycle transition from % to % for investor %',
        OLD.lifecycle_state, NEW.lifecycle_state, OLD.investor_id;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_validate_investor_lifecycle ON kyc.investors;
CREATE TRIGGER trg_validate_investor_lifecycle
    BEFORE UPDATE OF lifecycle_state ON kyc.investors
    FOR EACH ROW
    WHEN (OLD.lifecycle_state IS DISTINCT FROM NEW.lifecycle_state)
    EXECUTE FUNCTION kyc.validate_investor_lifecycle_transition();

-- -----------------------------------------------------------------------------
-- 1.4 Investor Lifecycle Audit Trail
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS kyc.investor_lifecycle_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investor_id UUID NOT NULL REFERENCES kyc.investors(investor_id),
    from_state VARCHAR(50),
    to_state VARCHAR(50) NOT NULL,
    transitioned_at TIMESTAMPTZ DEFAULT NOW(),
    triggered_by VARCHAR(100),  -- User, system event, or auto-trigger
    notes TEXT,
    metadata JSONB
);

COMMENT ON TABLE kyc.investor_lifecycle_history IS
'Audit trail of all investor lifecycle state changes';

CREATE INDEX IF NOT EXISTS idx_investor_lifecycle_history ON kyc.investor_lifecycle_history(investor_id, transitioned_at DESC);

-- Trigger to log lifecycle changes
CREATE OR REPLACE FUNCTION kyc.log_investor_lifecycle_change()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO kyc.investor_lifecycle_history (
        investor_id, from_state, to_state, triggered_by, notes
    ) VALUES (
        NEW.investor_id, OLD.lifecycle_state, NEW.lifecycle_state,
        current_setting('app.current_user', true),
        NEW.lifecycle_notes
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_log_investor_lifecycle ON kyc.investors;
CREATE TRIGGER trg_log_investor_lifecycle
    AFTER UPDATE OF lifecycle_state ON kyc.investors
    FOR EACH ROW
    EXECUTE FUNCTION kyc.log_investor_lifecycle_change();

-- =============================================================================
-- PHASE 2: ENHANCED HOLDINGS TABLE
-- =============================================================================

-- Add investor_id column to link holdings to investor records
ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS investor_id UUID REFERENCES kyc.investors(investor_id);

-- Add holding_status for lifecycle tracking (separate from old 'status' column)
ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS holding_status VARCHAR(50) DEFAULT 'ACTIVE';

-- Add provider tracking columns
ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS provider VARCHAR(50) DEFAULT 'MANUAL';

ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS provider_reference VARCHAR(100);

ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS provider_sync_at TIMESTAMPTZ;

-- Add usage_type to distinguish TA vs UBO holdings
ALTER TABLE kyc.holdings
ADD COLUMN IF NOT EXISTS usage_type VARCHAR(20) DEFAULT 'TA';

COMMENT ON COLUMN kyc.holdings.investor_id IS
'Link to investor record (for TA use case - may be NULL for legacy data)';

COMMENT ON COLUMN kyc.holdings.holding_status IS
'PENDING, ACTIVE, SUSPENDED, CLOSED';

COMMENT ON COLUMN kyc.holdings.usage_type IS
'TA (Transfer Agency - client investors) or UBO (intra-group ownership)';

COMMENT ON COLUMN kyc.holdings.provider IS
'Data source: CLEARSTREAM, EUROCLEAR, CSV_IMPORT, API_FEED, MANUAL';

-- Indexes for new columns
CREATE INDEX IF NOT EXISTS idx_holdings_investor ON kyc.holdings(investor_id);
CREATE INDEX IF NOT EXISTS idx_holdings_usage_type ON kyc.holdings(usage_type);
CREATE INDEX IF NOT EXISTS idx_holdings_provider ON kyc.holdings(provider, provider_reference);
CREATE INDEX IF NOT EXISTS idx_holdings_status ON kyc.holdings(holding_status);

-- =============================================================================
-- PHASE 3: ENHANCED MOVEMENTS TABLE
-- =============================================================================

-- Update movement_type constraint to include PE/VC and lifecycle types
ALTER TABLE kyc.movements
DROP CONSTRAINT IF EXISTS movements_movement_type_check;

ALTER TABLE kyc.movements
ADD CONSTRAINT movements_movement_type_check CHECK (
    movement_type IN (
        -- Standard movements
        'subscription', 'redemption', 'transfer_in', 'transfer_out',
        'dividend', 'adjustment',
        -- PE/VC specific
        'commitment', 'capital_call', 'distribution', 'recallable',
        -- Lifecycle events
        'initial_subscription', 'additional_subscription',
        'partial_redemption', 'full_redemption',
        -- Corporate actions
        'stock_split', 'merger', 'spinoff'
    )
);

-- Add PE-specific columns
ALTER TABLE kyc.movements
ADD COLUMN IF NOT EXISTS commitment_id UUID;  -- Links calls/distributions to original commitment

ALTER TABLE kyc.movements
ADD COLUMN IF NOT EXISTS call_number INTEGER;  -- For capital calls: 1st, 2nd, etc.

ALTER TABLE kyc.movements
ADD COLUMN IF NOT EXISTS distribution_type VARCHAR(50);  -- INCOME, CAPITAL, RETURN_OF_CAPITAL

COMMENT ON COLUMN kyc.movements.commitment_id IS
'For capital_call and distribution: links to the original commitment movement';

COMMENT ON COLUMN kyc.movements.call_number IS
'For capital_call: sequence number (1st call, 2nd call, etc.)';

COMMENT ON COLUMN kyc.movements.distribution_type IS
'For distributions: INCOME, CAPITAL_GAIN, RETURN_OF_CAPITAL, RECALLABLE';

-- Index for commitment tracking
CREATE INDEX IF NOT EXISTS idx_movements_commitment ON kyc.movements(commitment_id) WHERE commitment_id IS NOT NULL;

-- =============================================================================
-- PHASE 4: VIEWS FOR DUAL USE CASES
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 4.1 Transfer Agency Investor View (Use Case A)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_ta_investors AS
SELECT
    -- Investor details
    i.investor_id,
    i.entity_id,
    e.name AS investor_name,
    et.type_code AS entity_type,
    pp.nationality AS investor_country,
    i.investor_type,
    i.investor_category,

    -- Lifecycle
    i.lifecycle_state,
    i.lifecycle_state_at,

    -- KYC
    i.kyc_status,
    i.kyc_case_id,
    i.kyc_approved_at,
    i.kyc_expires_at,
    i.kyc_risk_rating,

    -- Tax & regulatory
    i.tax_status,
    i.tax_jurisdiction,
    i.fatca_status,
    i.crs_status,

    -- Eligibility
    i.eligible_fund_types,
    i.restricted_jurisdictions,

    -- Owning client
    i.owning_cbu_id,
    c.name AS owning_client_name,

    -- Holdings summary
    COALESCE(hs.holding_count, 0) AS holding_count,
    COALESCE(hs.total_value, 0) AS total_value,

    -- Identifiers
    lei.id AS lei,
    tax_id.id AS tax_id,

    -- Provider
    i.provider,
    i.provider_reference,
    i.provider_sync_at,

    -- Timestamps
    i.created_at,
    i.updated_at

FROM kyc.investors i
JOIN "ob-poc".entities e ON i.entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".cbus c ON i.owning_cbu_id = c.cbu_id
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id AND lei.scheme = 'LEI'
LEFT JOIN "ob-poc".entity_identifiers tax_id
    ON e.entity_id = tax_id.entity_id AND tax_id.scheme = 'tax_id'
LEFT JOIN LATERAL (
    SELECT
        COUNT(*) AS holding_count,
        SUM(h.units * COALESCE(sc.nav_per_share, 0)) AS total_value
    FROM kyc.holdings h
    JOIN kyc.share_classes sc ON h.share_class_id = sc.id
    WHERE h.investor_id = i.investor_id
      AND h.holding_status = 'ACTIVE'
) hs ON true;

COMMENT ON VIEW kyc.v_ta_investors IS
'Transfer Agency view: Client investors with lifecycle state, KYC status, and holdings summary';

-- -----------------------------------------------------------------------------
-- 4.2 UBO-Qualified Holdings View (Use Case B)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_ubo_holdings AS
SELECT
    -- Holding details
    h.id AS holding_id,
    h.share_class_id,
    h.investor_entity_id,
    h.units,
    h.acquisition_date,
    h.usage_type,
    h.provider,
    h.provider_reference,

    -- Share class context
    sc.isin,
    sc.name AS share_class_name,
    sc.cbu_id AS fund_cbu_id,
    c.name AS fund_name,

    -- Entity being owned (the fund entity)
    sc.entity_id AS owned_entity_id,

    -- Investor/owner details
    e.name AS owner_name,
    et.type_code AS owner_entity_type,
    COALESCE(pp.nationality, lc.jurisdiction) AS owner_country,

    -- Ownership percentage
    ROUND((h.units / NULLIF(total.total_units, 0)) * 100, 4) AS ownership_percentage,

    -- UBO qualification
    CASE
        WHEN total.total_units > 0 AND (h.units / total.total_units) >= 0.25
        THEN true ELSE false
    END AS is_ubo_qualified,

    -- UBO type determination
    CASE
        WHEN et.type_code IN ('proper_person', 'natural_person') THEN 'DIRECT_UBO'
        ELSE 'REQUIRES_GLEIF_TRACE'
    END AS ubo_determination,

    -- LEI for corporate tracing
    lei.id AS owner_lei

FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id AND lei.scheme = 'LEI'
CROSS JOIN LATERAL (
    SELECT COALESCE(SUM(h2.units), 0) AS total_units
    FROM kyc.holdings h2
    WHERE h2.share_class_id = sc.id
      AND COALESCE(h2.holding_status, h2.status) = 'active'
) total
WHERE COALESCE(h.holding_status, h.status) = 'active';

COMMENT ON VIEW kyc.v_ubo_holdings IS
'Holdings view for UBO discovery. Shows ownership percentage and UBO qualification.';

-- -----------------------------------------------------------------------------
-- 4.3 Investor Register Summary View (Provider-Agnostic)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_investor_register AS
SELECT
    -- Share class (the "register" is per share class)
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    sc.currency,
    sc.nav_per_share,
    sc.nav_date,

    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    c.jurisdiction AS fund_jurisdiction,

    -- Investor
    h.id AS holding_id,
    i.investor_id,
    e.entity_id AS investor_entity_id,
    e.name AS investor_name,
    et.type_code AS investor_entity_type,
    COALESCE(pp.nationality, lc.jurisdiction) AS investor_country,
    i.investor_type,
    i.investor_category,

    -- Lifecycle & KYC
    i.lifecycle_state,
    i.kyc_status,
    i.kyc_risk_rating,

    -- Position data
    h.units AS holding_quantity,
    h.cost_basis,
    h.acquisition_date AS registration_date,
    COALESCE(h.holding_status, h.status) AS holding_status,

    -- Computed values
    h.units * COALESCE(sc.nav_per_share, 0) AS market_value,
    ROUND((h.units / NULLIF(total.total_units, 0)) * 100, 4) AS ownership_percentage,

    -- Identifiers
    lei.id AS investor_lei,
    clr.id AS clearstream_ref,

    -- Provider tracking
    COALESCE(h.provider, 'MANUAL') AS provider,
    h.provider_reference,
    h.provider_sync_at,

    -- Timestamps
    h.created_at AS holding_created_at,
    h.updated_at AS holding_updated_at

FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN kyc.investors i ON h.investor_id = i.investor_id
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id AND lei.scheme = 'LEI'
LEFT JOIN "ob-poc".entity_identifiers clr
    ON e.entity_id = clr.entity_id AND clr.scheme = 'CLEARSTREAM_KV'
CROSS JOIN LATERAL (
    SELECT COALESCE(SUM(h2.units), 0) AS total_units
    FROM kyc.holdings h2
    WHERE h2.share_class_id = sc.id
      AND COALESCE(h2.holding_status, h2.status) = 'active'
) total
WHERE COALESCE(h.holding_status, h.status) = 'active';

COMMENT ON VIEW kyc.v_investor_register IS
'Provider-agnostic investor register with holdings, identifiers, and ownership percentages';

-- -----------------------------------------------------------------------------
-- 4.4 Movement Report View (Provider-Agnostic)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_movements AS
SELECT
    -- Movement Details
    m.id AS movement_id,
    m.reference AS trans_ref,
    m.movement_type,
    m.units,
    m.price_per_unit,
    m.amount,
    m.currency,
    m.trade_date,
    m.settlement_date,
    m.status AS movement_status,
    m.notes,

    -- PE/VC specific
    m.commitment_id,
    m.call_number,
    m.distribution_type,

    -- Holding Context
    h.id AS holding_id,
    h.units AS current_holding_units,
    COALESCE(h.provider, 'MANUAL') AS provider,

    -- Share Class
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,

    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,

    -- Investor
    e.entity_id AS investor_entity_id,
    e.name AS investor_name,
    i.investor_id,
    i.lifecycle_state,
    clr_id.id AS clearstream_reference,
    lei.id AS investor_lei,

    -- Timestamps
    m.created_at,
    m.updated_at

FROM kyc.movements m
JOIN kyc.holdings h ON m.holding_id = h.id
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
LEFT JOIN kyc.investors i ON h.investor_id = i.investor_id

-- Clearstream KV reference
LEFT JOIN "ob-poc".entity_identifiers clr_id
    ON e.entity_id = clr_id.entity_id
    AND clr_id.scheme = 'CLEARSTREAM_KV'

-- LEI
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id
    AND lei.scheme = 'LEI';

COMMENT ON VIEW kyc.v_movements IS
'Movement/transaction log with investor and fund context (provider-agnostic)';

-- -----------------------------------------------------------------------------
-- 4.5 Share Class Summary View
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_share_class_summary AS
SELECT
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    sc.currency,
    sc.nav_per_share,
    sc.nav_date,
    sc.fund_type,
    sc.fund_structure,
    sc.investor_eligibility,
    sc.status,

    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    c.jurisdiction AS fund_jurisdiction,

    -- Aggregates
    COALESCE(stats.investor_count, 0) AS investor_count,
    COALESCE(stats.total_units, 0) AS total_units,
    CASE
        WHEN sc.nav_per_share IS NOT NULL
        THEN COALESCE(stats.total_units, 0) * sc.nav_per_share
        ELSE NULL
    END AS assets_under_management,

    -- Movement activity (last 30 days)
    COALESCE(activity.subscription_count, 0) AS subscriptions_30d,
    COALESCE(activity.redemption_count, 0) AS redemptions_30d,
    COALESCE(activity.net_flow_units, 0) AS net_flow_units_30d

FROM kyc.share_classes sc
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id

-- Holding statistics
LEFT JOIN LATERAL (
    SELECT
        COUNT(DISTINCT h.investor_entity_id) AS investor_count,
        SUM(h.units) AS total_units
    FROM kyc.holdings h
    WHERE h.share_class_id = sc.id
    AND COALESCE(h.holding_status, h.status) = 'active'
) stats ON true

-- Recent activity
LEFT JOIN LATERAL (
    SELECT
        COUNT(*) FILTER (WHERE m.movement_type IN ('subscription', 'initial_subscription', 'additional_subscription')) AS subscription_count,
        COUNT(*) FILTER (WHERE m.movement_type IN ('redemption', 'partial_redemption', 'full_redemption')) AS redemption_count,
        COALESCE(SUM(CASE
            WHEN m.movement_type IN ('subscription', 'initial_subscription', 'additional_subscription', 'transfer_in') THEN m.units
            WHEN m.movement_type IN ('redemption', 'partial_redemption', 'full_redemption', 'transfer_out') THEN -m.units
            ELSE 0
        END), 0) AS net_flow_units
    FROM kyc.movements m
    JOIN kyc.holdings h ON m.holding_id = h.id
    WHERE h.share_class_id = sc.id
    AND m.trade_date >= CURRENT_DATE - INTERVAL '30 days'
    AND m.status IN ('confirmed', 'settled')
) activity ON true;

COMMENT ON VIEW kyc.v_share_class_summary IS
'Share class summary with investor counts, AUM, and recent activity metrics';

-- -----------------------------------------------------------------------------
-- 4.6 Investor Portfolio View
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW kyc.v_investor_portfolio AS
SELECT
    -- Investor
    e.entity_id AS investor_entity_id,
    e.name AS investor_name,
    et.type_code AS investor_type,
    COALESCE(pp.nationality, lc.jurisdiction) AS investor_country,

    -- Investor lifecycle (if available)
    i.investor_id,
    i.lifecycle_state,
    i.kyc_status,
    i.kyc_risk_rating,

    -- Identifiers
    lei.id AS investor_lei,
    clr_id.id AS clearstream_reference,

    -- Holding
    h.id AS holding_id,
    h.units,
    h.cost_basis,
    h.acquisition_date,
    COALESCE(h.holding_status, h.status) AS holding_status,
    COALESCE(h.provider, 'MANUAL') AS provider,

    -- Share Class
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    sc.currency,
    sc.nav_per_share,
    sc.nav_date,

    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    c.jurisdiction AS fund_jurisdiction,

    -- Computed values
    CASE
        WHEN sc.nav_per_share IS NOT NULL
        THEN h.units * sc.nav_per_share
        ELSE NULL
    END AS market_value,

    CASE
        WHEN h.cost_basis IS NOT NULL AND sc.nav_per_share IS NOT NULL
        THEN (h.units * sc.nav_per_share) - h.cost_basis
        ELSE NULL
    END AS unrealized_pnl

FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
JOIN kyc.holdings h ON e.entity_id = h.investor_entity_id
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN kyc.investors i ON h.investor_id = i.investor_id

-- LEI
LEFT JOIN "ob-poc".entity_identifiers lei
    ON e.entity_id = lei.entity_id
    AND lei.scheme = 'LEI'

-- Clearstream reference
LEFT JOIN "ob-poc".entity_identifiers clr_id
    ON e.entity_id = clr_id.entity_id
    AND clr_id.scheme = 'CLEARSTREAM_KV'

WHERE COALESCE(h.holding_status, h.status) = 'active';

COMMENT ON VIEW kyc.v_investor_portfolio IS
'Investor portfolio view showing all holdings across funds with market values';

-- -----------------------------------------------------------------------------
-- 4.7 Identifier Cross-Reference View
-- -----------------------------------------------------------------------------
CREATE OR REPLACE VIEW "ob-poc".v_entity_identifier_xref AS
SELECT
    e.entity_id,
    e.name AS entity_name,
    et.type_code AS entity_type,
    COALESCE(pp.nationality, lc.jurisdiction) AS country_code,

    -- Pivot common identifier schemes
    MAX(CASE WHEN ei.scheme = 'LEI' THEN ei.id END) AS lei,
    MAX(CASE WHEN ei.scheme = 'LEI' THEN ei.lei_status END) AS lei_status,
    MAX(CASE WHEN ei.scheme = 'CLEARSTREAM_KV' THEN ei.id END) AS clearstream_kv,
    MAX(CASE WHEN ei.scheme = 'CLEARSTREAM_ACCT' THEN ei.id END) AS clearstream_account,
    MAX(CASE WHEN ei.scheme = 'EUROCLEAR' THEN ei.id END) AS euroclear_id,
    MAX(CASE WHEN ei.scheme = 'company_register' THEN ei.id END) AS company_register_id,
    MAX(CASE WHEN ei.scheme = 'tax_id' THEN ei.id END) AS tax_id,
    MAX(CASE WHEN ei.scheme = 'ISIN' THEN ei.id END) AS isin,

    -- Count of all identifiers
    COUNT(ei.identifier_id) AS identifier_count,

    -- Validation status
    BOOL_OR(ei.is_validated) AS has_validated_identifier

FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN "ob-poc".entity_identifiers ei ON e.entity_id = ei.entity_id
GROUP BY e.entity_id, e.name, et.type_code, pp.nationality, lc.jurisdiction;

COMMENT ON VIEW "ob-poc".v_entity_identifier_xref IS
'Cross-reference view of all entity identifiers (LEI, Clearstream, Euroclear, Tax ID, etc.)';

-- =============================================================================
-- PHASE 5: BODS EXPORT VIEW
-- =============================================================================

-- Unified BODS Ownership Statements from all sources
CREATE OR REPLACE VIEW kyc.v_bods_ownership_statements AS

-- Source 1: Investor Register holdings (qualified UBOs)
SELECT
    'ooc-holding-' || h.holding_id::text AS statement_id,
    'ownershipOrControlStatement' AS statement_type,
    h.isin AS subject_identifier,
    h.fund_name AS subject_name,
    h.owner_name AS interested_party_name,
    h.owner_lei AS interested_party_lei,
    'shareholding' AS interest_type,
    'direct' AS interest_directness,
    h.units AS share_exact,
    h.ownership_percentage,
    h.is_ubo_qualified AS beneficial_ownership_or_control,
    h.acquisition_date AS interest_start_date,
    NULL::DATE AS interest_end_date,
    COALESCE(h.provider, 'MANUAL') AS source_type,
    h.provider_reference AS source_reference,
    CURRENT_DATE AS statement_date
FROM kyc.v_ubo_holdings h
WHERE h.is_ubo_qualified = true

UNION ALL

-- Source 2: Direct entity_relationships (not from holdings)
SELECT
    'ooc-rel-' || er.relationship_id::text AS statement_id,
    'ownershipOrControlStatement' AS statement_type,
    NULL AS subject_identifier,
    subject_e.name AS subject_name,
    owner_e.name AS interested_party_name,
    owner_lei.id AS interested_party_lei,
    COALESCE(er.interest_type, 'shareholding') AS interest_type,
    COALESCE(er.direct_or_indirect, 'direct') AS interest_directness,
    NULL::NUMERIC AS share_exact,
    er.percentage AS ownership_percentage,
    er.percentage >= 25 AS beneficial_ownership_or_control,
    er.effective_from AS interest_start_date,
    er.effective_to AS interest_end_date,
    COALESCE(er.source, 'MANUAL') AS source_type,
    er.relationship_id::text AS source_reference,
    CURRENT_DATE AS statement_date
FROM "ob-poc".entity_relationships er
JOIN "ob-poc".entities owner_e ON er.from_entity_id = owner_e.entity_id
JOIN "ob-poc".entities subject_e ON er.to_entity_id = subject_e.entity_id
LEFT JOIN "ob-poc".entity_identifiers owner_lei
    ON owner_e.entity_id = owner_lei.entity_id AND owner_lei.scheme = 'LEI'
WHERE er.relationship_type = 'ownership'
  AND er.source != 'INVESTOR_REGISTER'  -- Avoid double-count with holdings
  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE);

COMMENT ON VIEW kyc.v_bods_ownership_statements IS
'BODS 0.4 Ownership-or-Control Statement format for regulatory reporting (unified from all sources)';

-- =============================================================================
-- PHASE 6: UBO SYNC TRIGGER (Holdings ≥25% → entity_relationships)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.sync_holding_to_ubo_relationship()
RETURNS TRIGGER AS $$
DECLARE
    v_total_units NUMERIC;
    v_ownership_pct NUMERIC;
    v_fund_entity_id UUID;
BEGIN
    -- Get total units for percentage calculation
    SELECT COALESCE(SUM(units), 0) INTO v_total_units
    FROM kyc.holdings
    WHERE share_class_id = NEW.share_class_id
      AND COALESCE(holding_status, status) = 'active';

    -- Calculate ownership percentage
    IF v_total_units > 0 THEN
        v_ownership_pct := (NEW.units / v_total_units) * 100;
    ELSE
        v_ownership_pct := 0;
    END IF;

    -- Get fund entity ID from share class
    SELECT entity_id INTO v_fund_entity_id
    FROM kyc.share_classes WHERE id = NEW.share_class_id;

    -- Create/update ownership relationship if ≥25% and fund entity exists
    IF v_ownership_pct >= 25 AND v_fund_entity_id IS NOT NULL THEN
        INSERT INTO "ob-poc".entity_relationships (
            from_entity_id, to_entity_id, relationship_type,
            percentage, ownership_type, interest_type, direct_or_indirect,
            effective_from, source, notes
        ) VALUES (
            NEW.investor_entity_id, v_fund_entity_id, 'ownership',
            v_ownership_pct, 'DIRECT', 'shareholding', 'direct',
            COALESCE(NEW.acquisition_date, CURRENT_DATE),
            'INVESTOR_REGISTER',
            'Synced from holding ' || NEW.id::text
        )
        ON CONFLICT (from_entity_id, to_entity_id, relationship_type)
        WHERE effective_to IS NULL
        DO UPDATE SET
            percentage = EXCLUDED.percentage,
            updated_at = NOW(),
            notes = EXCLUDED.notes;
    ELSE
        -- Remove relationship if dropped below 25%
        UPDATE "ob-poc".entity_relationships
        SET effective_to = CURRENT_DATE,
            updated_at = NOW()
        WHERE from_entity_id = NEW.investor_entity_id
          AND to_entity_id = v_fund_entity_id
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
          AND effective_to IS NULL;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_sync_holding_to_ubo ON kyc.holdings;
CREATE TRIGGER trg_sync_holding_to_ubo
    AFTER INSERT OR UPDATE OF units, holding_status, status ON kyc.holdings
    FOR EACH ROW
    EXECUTE FUNCTION kyc.sync_holding_to_ubo_relationship();

COMMENT ON FUNCTION kyc.sync_holding_to_ubo_relationship() IS
'Syncs holdings ≥25% to entity_relationships for UBO discovery';

-- =============================================================================
-- PHASE 7: ADDITIONAL INDEXES FOR PERFORMANCE
-- =============================================================================

-- Index for identifier lookups (including Euroclear)
CREATE INDEX IF NOT EXISTS idx_entity_identifiers_provider
ON "ob-poc".entity_identifiers(scheme, id)
WHERE scheme IN ('CLEARSTREAM_KV', 'CLEARSTREAM_ACCT', 'EUROCLEAR');

-- Index for share class by ISIN
CREATE INDEX IF NOT EXISTS idx_share_classes_isin
ON kyc.share_classes(isin)
WHERE isin IS NOT NULL;

-- Index for active holdings with usage type
CREATE INDEX IF NOT EXISTS idx_holdings_active_usage
ON kyc.holdings(share_class_id, investor_entity_id, usage_type)
WHERE COALESCE(holding_status, status) = 'active';

-- Index for movement lookups by date and type
CREATE INDEX IF NOT EXISTS idx_movements_trade_date
ON kyc.movements(trade_date, movement_type, status);

-- Index for KYC expiring soon
CREATE INDEX IF NOT EXISTS idx_investors_kyc_expiring
ON kyc.investors(kyc_expires_at)
WHERE kyc_status = 'APPROVED' AND kyc_expires_at IS NOT NULL;

-- =============================================================================
-- DONE
-- =============================================================================
-- =============================================================================
-- Migration 012: Session Scope Management
-- =============================================================================
-- Purpose: Persistent storage for user session scope state
--   - Scope type (galaxy, book, cbu, jurisdiction, neighborhood)
--   - Scope parameters (apex entity, CBU, jurisdiction code, etc.)
--   - Cursor (focused entity within scope)
--   - History for back/forward navigation
--   - Bookmarks for saved scopes
--
-- Integrates with: UnifiedSessionContext, ViewState, AgentGraphContext
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. Session Scopes Table (current scope state per session)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS "ob-poc".session_scopes (
    session_scope_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Session identity (browser/REPL session)
    session_id UUID NOT NULL,
    user_id UUID,  -- Optional: for user-specific persistence

    -- Scope type discriminator (matches GraphScope enum)
    scope_type VARCHAR(50) NOT NULL DEFAULT 'empty',

    -- Scope parameters (only one set populated based on scope_type)
    -- For 'galaxy' / 'book': the apex entity
    apex_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    apex_entity_name VARCHAR(255),

    -- For 'cbu': single CBU focus
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    cbu_name VARCHAR(255),

    -- For 'jurisdiction': jurisdiction filter
    jurisdiction_code VARCHAR(10),

    -- For 'neighborhood': entity + hop count
    focal_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    focal_entity_name VARCHAR(255),
    neighborhood_hops INTEGER DEFAULT 2,

    -- For 'book': additional filters (JSONB for flexibility)
    -- e.g., {"jurisdictions": ["LU", "IE"], "entity_types": ["fund", "subfund"]}
    scope_filters JSONB DEFAULT '{}',

    -- Cursor: currently focused entity within scope
    cursor_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    cursor_entity_name VARCHAR(255),

    -- Scope statistics (cached for display)
    total_entities INTEGER DEFAULT 0,
    total_cbus INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ DEFAULT NOW() + INTERVAL '24 hours',

    -- Unique: one active scope per session
    UNIQUE(session_id)
);

COMMENT ON TABLE "ob-poc".session_scopes IS
'Persistent storage for user session scope state (galaxy, book, CBU, jurisdiction, neighborhood)';

COMMENT ON COLUMN "ob-poc".session_scopes.scope_type IS
'Discriminator: empty, galaxy, book, cbu, jurisdiction, neighborhood, custom';

COMMENT ON COLUMN "ob-poc".session_scopes.scope_filters IS
'Additional filters for book scope: jurisdictions[], entity_types[], etc.';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_session_scopes_session ON "ob-poc".session_scopes(session_id);
CREATE INDEX IF NOT EXISTS idx_session_scopes_user ON "ob-poc".session_scopes(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_session_scopes_expires ON "ob-poc".session_scopes(expires_at);

-- -----------------------------------------------------------------------------
-- 2. Session Scope History (for back/forward navigation)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS "ob-poc".session_scope_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL,

    -- Position in history (0 = oldest)
    position INTEGER NOT NULL,

    -- Snapshot of scope at this point
    scope_snapshot JSONB NOT NULL,

    -- What triggered this history entry
    change_source VARCHAR(50) NOT NULL DEFAULT 'dsl',
    change_verb VARCHAR(100),  -- e.g., 'session.set-cbu'

    -- Timestamp
    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- Composite index for efficient history navigation
    UNIQUE(session_id, position)
);

COMMENT ON TABLE "ob-poc".session_scope_history IS
'Navigation history for back/forward in session scope';

COMMENT ON COLUMN "ob-poc".session_scope_history.change_source IS
'dsl, api, lexicon, navigation, system';

CREATE INDEX IF NOT EXISTS idx_scope_history_session ON "ob-poc".session_scope_history(session_id, position DESC);

-- -----------------------------------------------------------------------------
-- 3. Session Bookmarks (named saved scopes)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS "ob-poc".session_bookmarks (
    bookmark_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Owner (can be session-specific or user-specific)
    session_id UUID,
    user_id UUID,

    -- Bookmark name
    name VARCHAR(100) NOT NULL,
    description TEXT,

    -- Scope snapshot
    scope_snapshot JSONB NOT NULL,

    -- Metadata
    icon VARCHAR(50),  -- emoji or icon name
    color VARCHAR(20),  -- for UI highlighting

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    use_count INTEGER DEFAULT 0
);

-- Unique name per user (or per session if no user) - use unique index instead of constraint
CREATE UNIQUE INDEX IF NOT EXISTS idx_bookmarks_unique_name
ON "ob-poc".session_bookmarks(COALESCE(user_id, session_id), name);

COMMENT ON TABLE "ob-poc".session_bookmarks IS
'Named saved scopes for quick navigation';

CREATE INDEX IF NOT EXISTS idx_bookmarks_user ON "ob-poc".session_bookmarks(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_bookmarks_session ON "ob-poc".session_bookmarks(session_id) WHERE session_id IS NOT NULL;

-- -----------------------------------------------------------------------------
-- 4. Helper Functions
-- -----------------------------------------------------------------------------

-- Get or create session scope
CREATE OR REPLACE FUNCTION "ob-poc".get_or_create_session_scope(
    p_session_id UUID,
    p_user_id UUID DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_scope_id UUID;
BEGIN
    -- Try to find existing
    SELECT session_scope_id INTO v_scope_id
    FROM "ob-poc".session_scopes
    WHERE session_id = p_session_id;

    IF v_scope_id IS NULL THEN
        -- Create new empty scope
        INSERT INTO "ob-poc".session_scopes (session_id, user_id, scope_type)
        VALUES (p_session_id, p_user_id, 'empty')
        RETURNING session_scope_id INTO v_scope_id;
    ELSE
        -- Extend expiry
        UPDATE "ob-poc".session_scopes
        SET expires_at = NOW() + INTERVAL '24 hours',
            updated_at = NOW()
        WHERE session_scope_id = v_scope_id;
    END IF;

    RETURN v_scope_id;
END;
$$ LANGUAGE plpgsql;

-- Set scope to galaxy (all CBUs under apex)
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_galaxy(
    p_session_id UUID,
    p_apex_entity_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_apex_name VARCHAR(255);
    v_cbu_count INTEGER;
    v_entity_count INTEGER;
BEGIN
    -- Get apex name
    SELECT name INTO v_apex_name
    FROM "ob-poc".entities WHERE entity_id = p_apex_entity_id;

    -- Count CBUs under this apex (via commercial_client_entity_id)
    SELECT COUNT(*) INTO v_cbu_count
    FROM "ob-poc".cbus
    WHERE commercial_client_entity_id = p_apex_entity_id;

    -- Estimate entity count (CBUs * avg entities per CBU)
    -- For now, just use CBU count * 10 as estimate
    v_entity_count := v_cbu_count * 10;

    -- Upsert scope
    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        apex_entity_id, apex_entity_name,
        total_cbus, total_entities,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'galaxy',
        p_apex_entity_id, v_apex_name,
        v_cbu_count, v_entity_count,
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'galaxy',
        apex_entity_id = p_apex_entity_id,
        apex_entity_name = v_apex_name,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = NULL,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        scope_filters = '{}',
        total_cbus = v_cbu_count,
        total_entities = v_entity_count,
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set scope to book (filtered subset of galaxy)
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_book(
    p_session_id UUID,
    p_apex_entity_id UUID,
    p_filters JSONB DEFAULT '{}'
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_apex_name VARCHAR(255);
    v_cbu_count INTEGER;
BEGIN
    SELECT name INTO v_apex_name
    FROM "ob-poc".entities WHERE entity_id = p_apex_entity_id;

    -- Count CBUs matching filters
    -- For now, count all under apex (filter logic in application layer)
    SELECT COUNT(*) INTO v_cbu_count
    FROM "ob-poc".cbus
    WHERE commercial_client_entity_id = p_apex_entity_id;

    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        apex_entity_id, apex_entity_name,
        scope_filters, total_cbus,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'book',
        p_apex_entity_id, v_apex_name,
        p_filters, v_cbu_count,
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'book',
        apex_entity_id = p_apex_entity_id,
        apex_entity_name = v_apex_name,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = NULL,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        scope_filters = p_filters,
        total_cbus = v_cbu_count,
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set scope to single CBU
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_cbu(
    p_session_id UUID,
    p_cbu_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_cbu_name VARCHAR(255);
    v_entity_count INTEGER;
BEGIN
    SELECT name INTO v_cbu_name
    FROM "ob-poc".cbus WHERE cbu_id = p_cbu_id;

    -- Count entities in this CBU's ownership structure
    -- Simplified: count direct ownership relationships
    SELECT COUNT(DISTINCT e.entity_id) INTO v_entity_count
    FROM "ob-poc".entities e
    JOIN "ob-poc".entity_relationships er ON e.entity_id = er.from_entity_id OR e.entity_id = er.to_entity_id
    WHERE er.from_entity_id IN (SELECT entity_id FROM "ob-poc".cbus WHERE cbu_id = p_cbu_id)
       OR er.to_entity_id IN (SELECT entity_id FROM "ob-poc".cbus WHERE cbu_id = p_cbu_id);

    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        cbu_id, cbu_name,
        total_cbus, total_entities,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'cbu',
        p_cbu_id, v_cbu_name,
        1, COALESCE(v_entity_count, 0),
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'cbu',
        apex_entity_id = NULL,
        apex_entity_name = NULL,
        cbu_id = p_cbu_id,
        cbu_name = v_cbu_name,
        jurisdiction_code = NULL,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        scope_filters = '{}',
        total_cbus = 1,
        total_entities = COALESCE(v_entity_count, 0),
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set scope to jurisdiction
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_jurisdiction(
    p_session_id UUID,
    p_jurisdiction_code VARCHAR(10)
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_cbu_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO v_cbu_count
    FROM "ob-poc".cbus
    WHERE jurisdiction = p_jurisdiction_code;

    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        jurisdiction_code,
        total_cbus,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'jurisdiction',
        p_jurisdiction_code,
        v_cbu_count,
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'jurisdiction',
        apex_entity_id = NULL,
        apex_entity_name = NULL,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = p_jurisdiction_code,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        scope_filters = '{}',
        total_cbus = v_cbu_count,
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set scope to entity neighborhood
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_neighborhood(
    p_session_id UUID,
    p_entity_id UUID,
    p_hops INTEGER DEFAULT 2
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_entity_name VARCHAR(255);
BEGIN
    SELECT name INTO v_entity_name
    FROM "ob-poc".entities WHERE entity_id = p_entity_id;

    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        focal_entity_id, focal_entity_name,
        neighborhood_hops,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'neighborhood',
        p_entity_id, v_entity_name,
        p_hops,
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'neighborhood',
        apex_entity_id = NULL,
        apex_entity_name = NULL,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = NULL,
        focal_entity_id = p_entity_id,
        focal_entity_name = v_entity_name,
        neighborhood_hops = p_hops,
        scope_filters = '{}',
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set cursor (focus entity within scope)
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_cursor(
    p_session_id UUID,
    p_entity_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_entity_name VARCHAR(255);
BEGIN
    SELECT name INTO v_entity_name
    FROM "ob-poc".entities WHERE entity_id = p_entity_id;

    UPDATE "ob-poc".session_scopes
    SET cursor_entity_id = p_entity_id,
        cursor_entity_name = v_entity_name,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Clear scope (reset to empty)
CREATE OR REPLACE FUNCTION "ob-poc".clear_scope(
    p_session_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET scope_type = 'empty',
        apex_entity_id = NULL,
        apex_entity_name = NULL,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = NULL,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        neighborhood_hops = NULL,
        scope_filters = '{}',
        cursor_entity_id = NULL,
        cursor_entity_name = NULL,
        total_entities = 0,
        total_cbus = 0,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- -----------------------------------------------------------------------------
-- 5. Views
-- -----------------------------------------------------------------------------

-- Current scope with enriched entity names
CREATE OR REPLACE VIEW "ob-poc".v_current_session_scope AS
SELECT
    ss.session_scope_id,
    ss.session_id,
    ss.user_id,
    ss.scope_type,

    -- Scope parameters
    ss.apex_entity_id,
    ss.apex_entity_name,
    ss.cbu_id,
    ss.cbu_name,
    ss.jurisdiction_code,
    ss.focal_entity_id,
    ss.focal_entity_name,
    ss.neighborhood_hops,
    ss.scope_filters,

    -- Cursor
    ss.cursor_entity_id,
    ss.cursor_entity_name,

    -- Stats
    ss.total_entities,
    ss.total_cbus,

    -- Display string
    CASE ss.scope_type
        WHEN 'empty' THEN 'No scope set'
        WHEN 'galaxy' THEN 'Galaxy: ' || ss.apex_entity_name || ' (' || ss.total_cbus || ' CBUs)'
        WHEN 'book' THEN 'Book: ' || ss.apex_entity_name || ' (filtered)'
        WHEN 'cbu' THEN 'CBU: ' || ss.cbu_name
        WHEN 'jurisdiction' THEN 'Jurisdiction: ' || ss.jurisdiction_code || ' (' || ss.total_cbus || ' CBUs)'
        WHEN 'neighborhood' THEN 'Neighborhood: ' || ss.focal_entity_name || ' (' || ss.neighborhood_hops || ' hops)'
        ELSE ss.scope_type
    END AS scope_display,

    -- Cursor display
    CASE
        WHEN ss.cursor_entity_id IS NOT NULL
        THEN '@ ' || ss.cursor_entity_name
        ELSE NULL
    END AS cursor_display,

    -- Timestamps
    ss.created_at,
    ss.updated_at,
    ss.expires_at,

    -- Is expired?
    ss.expires_at < NOW() AS is_expired

FROM "ob-poc".session_scopes ss;

COMMENT ON VIEW "ob-poc".v_current_session_scope IS
'Current session scope with display strings and enriched entity names';

-- -----------------------------------------------------------------------------
-- 6. Cleanup Job (delete expired scopes)
-- -----------------------------------------------------------------------------

-- Function to clean up expired sessions
CREATE OR REPLACE FUNCTION "ob-poc".cleanup_expired_session_scopes()
RETURNS INTEGER AS $$
DECLARE
    v_deleted INTEGER;
BEGIN
    -- Delete expired scopes
    WITH deleted AS (
        DELETE FROM "ob-poc".session_scopes
        WHERE expires_at < NOW()
        RETURNING session_id
    )
    SELECT COUNT(*) INTO v_deleted FROM deleted;

    -- Delete orphaned history
    DELETE FROM "ob-poc".session_scope_history h
    WHERE NOT EXISTS (
        SELECT 1 FROM "ob-poc".session_scopes s
        WHERE s.session_id = h.session_id
    );

    RETURN v_deleted;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- DONE
-- =============================================================================
-- Migration 013: Capital Structure & Ownership Model
-- Completes GLEIF/BODS/Register triangulation for ownership reconciliation
--
-- This migration extends the capital structure from migration 009 with:
-- - Instrument identifier schemes (ISIN, SEDOL, CUSIP, etc.)
-- - Share class identifiers (many-to-one)
-- - Share class supply tracking (current state)
-- - Issuance events ledger (append-only supply changes)
-- - Dilution instruments (options, warrants, SAFEs, convertibles)
-- - Issuer control configuration (jurisdiction thresholds)
-- - Special rights (board seats, vetos - unified class/holder)
-- - Ownership snapshots (computed from register, imported from BODS/GLEIF)
-- - Reconciliation framework (compare sources, track findings)

-- ============================================================================
-- 1.1: Instrument Identifier Schemes
-- ============================================================================
-- Handles both listed (ISIN, SEDOL) and private (INTERNAL, FUND_ADMIN) securities

CREATE TABLE IF NOT EXISTS kyc.instrument_identifier_schemes (
    scheme_code VARCHAR(20) PRIMARY KEY,
    scheme_name VARCHAR(100) NOT NULL,
    issuing_authority VARCHAR(100),
    format_regex VARCHAR(200),
    is_global BOOLEAN DEFAULT false,
    validation_url VARCHAR(500),
    display_order INTEGER DEFAULT 100,
    created_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE kyc.instrument_identifier_schemes IS
    'Reference table for security identifier types (ISIN, SEDOL, CUSIP, INTERNAL, etc.)';

-- Seed data
INSERT INTO kyc.instrument_identifier_schemes (scheme_code, scheme_name, issuing_authority, format_regex, is_global, display_order) VALUES
    ('ISIN', 'International Securities Identification Number', 'ISO 6166', '^[A-Z]{2}[A-Z0-9]{9}[0-9]$', true, 1),
    ('SEDOL', 'Stock Exchange Daily Official List', 'LSE', '^[B-DF-HJ-NP-TV-Z0-9]{7}$', false, 2),
    ('CUSIP', 'Committee on Uniform Securities Identification', 'CUSIP Global Services', '^[0-9A-Z]{9}$', false, 3),
    ('FIGI', 'Financial Instrument Global Identifier', 'Bloomberg', '^BBG[A-Z0-9]{9}$', true, 4),
    ('LEI', 'Legal Entity Identifier', 'GLEIF', '^[A-Z0-9]{20}$', true, 5),
    ('INTERNAL', 'Internal Reference', NULL, NULL, false, 99),
    ('FUND_ADMIN', 'Fund Administrator ID', NULL, NULL, false, 10),
    ('REGISTRY', 'Share Registry Number', NULL, NULL, false, 11),
    ('TA_REF', 'Transfer Agent Reference', NULL, NULL, false, 12)
ON CONFLICT (scheme_code) DO NOTHING;

-- ============================================================================
-- 1.2: Share Class Identifiers (many-to-one)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.share_class_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_class_id UUID NOT NULL REFERENCES kyc.share_classes(id) ON DELETE CASCADE,
    scheme_code VARCHAR(20) NOT NULL REFERENCES kyc.instrument_identifier_schemes(scheme_code),
    identifier_value VARCHAR(100) NOT NULL,
    is_primary BOOLEAN DEFAULT false,
    valid_from DATE DEFAULT CURRENT_DATE,
    valid_to DATE,  -- NULL = current
    source VARCHAR(50),  -- GLEIF, BLOOMBERG, MANUAL, FUND_ADMIN
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT uq_share_class_scheme_value UNIQUE (share_class_id, scheme_code, identifier_value)
);

-- Only one primary identifier per share class at a time
CREATE UNIQUE INDEX IF NOT EXISTS idx_share_class_primary_identifier
    ON kyc.share_class_identifiers(share_class_id)
    WHERE is_primary = true AND valid_to IS NULL;

CREATE INDEX IF NOT EXISTS idx_share_class_identifiers_lookup
    ON kyc.share_class_identifiers(scheme_code, identifier_value)
    WHERE valid_to IS NULL;

COMMENT ON TABLE kyc.share_class_identifiers IS
    'Security identifiers for share classes. Every class has at least INTERNAL. External IDs (ISIN, etc.) optional.';

-- ============================================================================
-- 1.3: Extend share_classes for control computation
-- ============================================================================
-- Migration 009 added basic columns. We extend with additional control-related fields.

ALTER TABLE kyc.share_classes
    -- Instrument classification (more specific than share_type)
    ADD COLUMN IF NOT EXISTS instrument_kind VARCHAR(30) DEFAULT 'FUND_UNIT',

    -- Voting rights (use existing voting_rights_per_share, add cap/threshold)
    ADD COLUMN IF NOT EXISTS votes_per_unit NUMERIC(10,4) DEFAULT 1.0,
    ADD COLUMN IF NOT EXISTS voting_cap_pct NUMERIC(5,2),
    ADD COLUMN IF NOT EXISTS voting_threshold_pct NUMERIC(5,2),

    -- Economic rights
    ADD COLUMN IF NOT EXISTS economic_per_unit NUMERIC(10,4) DEFAULT 1.0,
    ADD COLUMN IF NOT EXISTS dividend_rate NUMERIC(10,4),
    ADD COLUMN IF NOT EXISTS liquidation_rank INTEGER DEFAULT 100,

    -- Conversion (for convertibles, warrants)
    ADD COLUMN IF NOT EXISTS converts_to_share_class_id UUID REFERENCES kyc.share_classes(id),
    ADD COLUMN IF NOT EXISTS conversion_ratio_num NUMERIC(10,4),
    ADD COLUMN IF NOT EXISTS conversion_price NUMERIC(20,6),

    -- LP/PE specific
    ADD COLUMN IF NOT EXISTS commitment_currency VARCHAR(3),
    ADD COLUMN IF NOT EXISTS vintage_year INTEGER,
    ADD COLUMN IF NOT EXISTS is_carried_interest BOOLEAN DEFAULT false;

-- Add constraint for instrument_kind
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
        WHERE constraint_name = 'chk_instrument_kind'
        AND constraint_schema = 'kyc'
    ) THEN
        ALTER TABLE kyc.share_classes
            ADD CONSTRAINT chk_instrument_kind CHECK (
                instrument_kind IS NULL OR instrument_kind IN (
                    'ORDINARY_EQUITY', 'PREFERENCE_EQUITY', 'DEFERRED_EQUITY',
                    'FUND_UNIT', 'FUND_SHARE', 'LP_INTEREST', 'GP_INTEREST',
                    'DEBT', 'CONVERTIBLE', 'WARRANT', 'OTHER'
                )
            );
    END IF;
END $$;

COMMENT ON COLUMN kyc.share_classes.votes_per_unit IS
    '0 = non-voting, 1 = standard, >1 = super-voting (founder shares)';
COMMENT ON COLUMN kyc.share_classes.instrument_kind IS
    'Determines calculation method for ownership/control derivation';
COMMENT ON COLUMN kyc.share_classes.liquidation_rank IS
    'Priority in liquidation. Lower = more senior. 100 = common equity.';

-- ============================================================================
-- 1.4: Share Class Supply (current state - materialized for fast reads)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.share_class_supply (
    supply_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_class_id UUID NOT NULL REFERENCES kyc.share_classes(id) ON DELETE CASCADE,

    -- Supply figures
    authorized_units NUMERIC(20,6),
    issued_units NUMERIC(20,6) NOT NULL DEFAULT 0,
    outstanding_units NUMERIC(20,6) NOT NULL DEFAULT 0,
    treasury_units NUMERIC(20,6) DEFAULT 0,
    reserved_units NUMERIC(20,6) DEFAULT 0,

    -- As-of tracking
    as_of_date DATE NOT NULL DEFAULT CURRENT_DATE,
    as_of_event_id UUID,

    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT uq_supply_class_date UNIQUE (share_class_id, as_of_date)
);

CREATE INDEX IF NOT EXISTS idx_supply_class ON kyc.share_class_supply(share_class_id);
CREATE INDEX IF NOT EXISTS idx_supply_date ON kyc.share_class_supply(as_of_date DESC);

COMMENT ON TABLE kyc.share_class_supply IS
    'Current supply state per share class. Source of truth for denominators in control computation.';

-- Trigger to update totals when supply changes
CREATE OR REPLACE FUNCTION kyc.fn_update_supply_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_supply_timestamp ON kyc.share_class_supply;
CREATE TRIGGER trg_supply_timestamp
    BEFORE UPDATE ON kyc.share_class_supply
    FOR EACH ROW EXECUTE FUNCTION kyc.fn_update_supply_timestamp();

-- ============================================================================
-- 1.5: Issuance Events (append-only supply ledger)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.issuance_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What
    share_class_id UUID NOT NULL REFERENCES kyc.share_classes(id),
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Event type
    event_type VARCHAR(30) NOT NULL,

    -- Quantities
    units_delta NUMERIC(20,6) NOT NULL,

    -- For splits/consolidations
    ratio_from INTEGER,
    ratio_to INTEGER,

    -- Pricing
    price_per_unit NUMERIC(20,6),
    price_currency VARCHAR(3),
    total_amount NUMERIC(20,2),

    -- Dates
    effective_date DATE NOT NULL,
    announcement_date DATE,
    record_date DATE,

    -- Status
    status VARCHAR(20) DEFAULT 'EFFECTIVE',

    -- Provenance
    board_resolution_ref VARCHAR(100),
    regulatory_filing_ref VARCHAR(100),
    source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),

    -- Audit
    created_by VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT now(),
    notes TEXT,

    CONSTRAINT chk_event_type CHECK (event_type IN (
        'INITIAL_ISSUE', 'NEW_ISSUE', 'STOCK_SPLIT', 'BONUS_ISSUE',
        'CANCELLATION', 'BUYBACK', 'CONSOLIDATION',
        'TREASURY_RELEASE', 'TREASURY_TRANSFER',
        'MERGER_IN', 'MERGER_OUT', 'SPINOFF', 'CONVERSION'
    )),
    CONSTRAINT chk_event_status CHECK (status IN (
        'DRAFT', 'PENDING_APPROVAL', 'EFFECTIVE', 'REVERSED', 'CANCELLED'
    )),
    CONSTRAINT chk_split_ratio CHECK (
        (event_type NOT IN ('STOCK_SPLIT', 'CONSOLIDATION')) OR
        (ratio_from IS NOT NULL AND ratio_to IS NOT NULL AND ratio_from > 0 AND ratio_to > 0)
    )
);

CREATE INDEX IF NOT EXISTS idx_issuance_class ON kyc.issuance_events(share_class_id, effective_date);
CREATE INDEX IF NOT EXISTS idx_issuance_issuer ON kyc.issuance_events(issuer_entity_id);
CREATE INDEX IF NOT EXISTS idx_issuance_status ON kyc.issuance_events(status) WHERE status = 'EFFECTIVE';

COMMENT ON TABLE kyc.issuance_events IS
    'Append-only ledger of supply changes. Source for computing share_class_supply at any as-of date.';

-- ============================================================================
-- 1.6: Dilution Instruments (Options, Warrants, Convertibles, SAFEs)
-- ============================================================================
-- Tracks potential future dilution from options, warrants, convertibles, SAFEs.
-- Required for FULLY_DILUTED basis computation.

CREATE TABLE IF NOT EXISTS kyc.dilution_instruments (
    instrument_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What company/fund this dilutes
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- What share class it converts INTO (the diluted class)
    converts_to_share_class_id UUID REFERENCES kyc.share_classes(id),

    -- Instrument type
    instrument_type VARCHAR(30) NOT NULL,

    -- Who holds this instrument
    holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Quantities
    units_granted NUMERIC(20,6) NOT NULL,
    units_exercised NUMERIC(20,6) DEFAULT 0,
    units_forfeited NUMERIC(20,6) DEFAULT 0,

    -- Conversion terms
    conversion_ratio NUMERIC(10,4) DEFAULT 1.0,
    exercise_price NUMERIC(20,6),
    exercise_currency VARCHAR(3),

    -- For SAFEs/Convertible Notes
    valuation_cap NUMERIC(20,2),
    discount_pct NUMERIC(5,2),
    principal_amount NUMERIC(20,2),

    -- Exercisability
    vesting_start_date DATE,
    vesting_end_date DATE,
    vesting_cliff_months INTEGER,
    exercisable_from DATE,
    expiration_date DATE,

    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE',

    -- Provenance
    grant_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    board_approval_ref VARCHAR(100),
    plan_name VARCHAR(100),

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    notes TEXT,

    CONSTRAINT chk_instrument_type CHECK (instrument_type IN (
        'STOCK_OPTION',
        'WARRANT',
        'CONVERTIBLE_NOTE',
        'SAFE',
        'CONVERTIBLE_PREFERRED',
        'RSU',
        'PHANTOM_STOCK',
        'SAR',
        'OTHER'
    )),
    CONSTRAINT chk_dilution_status CHECK (status IN (
        'ACTIVE', 'EXERCISED', 'EXPIRED', 'FORFEITED', 'CANCELLED'
    )),
    CONSTRAINT chk_units_positive CHECK (units_granted > 0),
    CONSTRAINT chk_exercised_lte_granted CHECK (units_exercised <= units_granted)
);

CREATE INDEX IF NOT EXISTS idx_dilution_issuer ON kyc.dilution_instruments(issuer_entity_id)
    WHERE status = 'ACTIVE';
CREATE INDEX IF NOT EXISTS idx_dilution_converts_to ON kyc.dilution_instruments(converts_to_share_class_id);
CREATE INDEX IF NOT EXISTS idx_dilution_holder ON kyc.dilution_instruments(holder_entity_id)
    WHERE holder_entity_id IS NOT NULL;

COMMENT ON TABLE kyc.dilution_instruments IS
    'Options, warrants, convertibles, SAFEs that may dilute existing shareholders. Required for FULLY_DILUTED computation.';

-- ============================================================================
-- 1.7: Dilution Exercise Events (audit trail)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.dilution_exercise_events (
    exercise_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instrument_id UUID NOT NULL REFERENCES kyc.dilution_instruments(instrument_id),

    -- Exercise details
    units_exercised NUMERIC(20,6) NOT NULL,
    exercise_date DATE NOT NULL,
    exercise_price_paid NUMERIC(20,6),

    -- Resulting shares
    shares_issued NUMERIC(20,6) NOT NULL,
    resulting_holding_id UUID REFERENCES kyc.holdings(id),

    -- For cashless exercise
    is_cashless BOOLEAN DEFAULT false,
    shares_withheld_for_tax NUMERIC(20,6),

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_exercise_instrument ON kyc.dilution_exercise_events(instrument_id);

COMMENT ON TABLE kyc.dilution_exercise_events IS
    'Audit trail of option/warrant exercises. Links to resulting holdings.';

-- ============================================================================
-- 1.8: Issuer Control Configuration
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.issuer_control_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Thresholds (jurisdiction/articles dependent)
    control_threshold_pct NUMERIC(5,2) DEFAULT 50.00,
    significant_threshold_pct NUMERIC(5,2) DEFAULT 25.00,
    material_threshold_pct NUMERIC(5,2) DEFAULT 10.00,
    disclosure_threshold_pct NUMERIC(5,2) DEFAULT 5.00,

    -- Basis for computation
    control_basis VARCHAR(20) DEFAULT 'VOTES',
    disclosure_basis VARCHAR(20) DEFAULT 'ECONOMIC',
    voting_basis VARCHAR(20) DEFAULT 'OUTSTANDING',

    -- Jurisdiction rules
    jurisdiction VARCHAR(10),
    applies_voting_caps BOOLEAN DEFAULT false,

    -- Temporal
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,

    created_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT chk_control_basis CHECK (control_basis IN ('VOTES', 'ECONOMIC', 'UNITS')),
    CONSTRAINT chk_disclosure_basis CHECK (disclosure_basis IN ('VOTES', 'ECONOMIC', 'UNITS')),
    CONSTRAINT chk_voting_basis CHECK (voting_basis IN (
        'ISSUED', 'OUTSTANDING', 'FULLY_DILUTED', 'EXERCISABLE'
    )),
    CONSTRAINT uq_issuer_control_config UNIQUE (issuer_entity_id, effective_from)
);

CREATE INDEX IF NOT EXISTS idx_control_config_issuer ON kyc.issuer_control_config(issuer_entity_id)
    WHERE effective_to IS NULL;

COMMENT ON TABLE kyc.issuer_control_config IS
    'Jurisdiction/articles-specific thresholds for control determination per issuer.';

-- ============================================================================
-- 1.9: Special Rights (class-level OR holder-level)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.special_rights (
    right_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope: exactly one of these must be set
    share_class_id UUID REFERENCES kyc.share_classes(id),
    holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Always required
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Right type
    right_type VARCHAR(30) NOT NULL,

    -- Conditions
    threshold_pct NUMERIC(5,2),
    threshold_basis VARCHAR(20),
    requires_class_vote BOOLEAN DEFAULT false,

    -- For board rights
    board_seats INTEGER,
    board_seat_type VARCHAR(20),

    -- Provenance
    source_type VARCHAR(20),
    source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    source_clause_ref VARCHAR(50),

    -- Temporal
    effective_from DATE,
    effective_to DATE,

    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT chk_right_scope CHECK (
        (share_class_id IS NOT NULL AND holder_entity_id IS NULL) OR
        (share_class_id IS NULL AND holder_entity_id IS NOT NULL)
    ),
    CONSTRAINT chk_right_type CHECK (right_type IN (
        'BOARD_APPOINTMENT', 'BOARD_OBSERVER', 'VETO_MA', 'VETO_FUNDRAISE',
        'VETO_DIVIDEND', 'VETO_LIQUIDATION', 'ANTI_DILUTION', 'DRAG_ALONG',
        'TAG_ALONG', 'FIRST_REFUSAL', 'REDEMPTION', 'CONVERSION_TRIGGER',
        'PROTECTIVE_PROVISION', 'INFORMATION_RIGHTS', 'OTHER'
    )),
    CONSTRAINT chk_source_type CHECK (source_type IS NULL OR source_type IN (
        'ARTICLES', 'SHA', 'SIDE_LETTER', 'BOARD_RESOLUTION', 'INVESTMENT_AGREEMENT'
    )),
    CONSTRAINT chk_board_seat_type CHECK (board_seat_type IS NULL OR board_seat_type IN (
        'DIRECTOR', 'OBSERVER', 'ALTERNATE', 'CHAIRMAN'
    ))
);

CREATE INDEX IF NOT EXISTS idx_special_rights_class ON kyc.special_rights(share_class_id)
    WHERE share_class_id IS NOT NULL AND effective_to IS NULL;
CREATE INDEX IF NOT EXISTS idx_special_rights_holder ON kyc.special_rights(holder_entity_id)
    WHERE holder_entity_id IS NOT NULL AND effective_to IS NULL;
CREATE INDEX IF NOT EXISTS idx_special_rights_issuer ON kyc.special_rights(issuer_entity_id);

COMMENT ON TABLE kyc.special_rights IS
    'Control rights not reducible to voting percentage. Attached to either share class or specific holder.';

-- ============================================================================
-- 1.10: Ownership Snapshots (the bridge)
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.ownership_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The relationship
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    owner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    share_class_id UUID REFERENCES kyc.share_classes(id),

    -- Temporal
    as_of_date DATE NOT NULL,

    -- Ownership basis
    basis VARCHAR(20) NOT NULL,

    -- The numbers
    units NUMERIC(20,6),
    percentage NUMERIC(8,4),
    percentage_min NUMERIC(8,4),
    percentage_max NUMERIC(8,4),

    -- Denominator (for audit)
    numerator NUMERIC(20,6),
    denominator NUMERIC(20,6),

    -- Provenance
    derived_from VARCHAR(20) NOT NULL,

    -- Source references
    source_holding_ids UUID[],
    source_bods_statement_id VARCHAR(100),
    source_gleif_rel_id UUID,
    source_document_id UUID,

    -- Flags
    is_direct BOOLEAN DEFAULT true,
    is_aggregated BOOLEAN DEFAULT false,
    confidence VARCHAR(20) DEFAULT 'HIGH',

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    superseded_at TIMESTAMPTZ,
    superseded_by UUID REFERENCES kyc.ownership_snapshots(snapshot_id),

    CONSTRAINT chk_snapshot_basis CHECK (basis IN (
        'UNITS', 'VOTES', 'ECONOMIC', 'CAPITAL', 'DECLARED'
    )),
    CONSTRAINT chk_snapshot_source CHECK (derived_from IN (
        'REGISTER', 'BODS', 'GLEIF', 'PSC', 'MANUAL', 'INFERRED'
    )),
    CONSTRAINT chk_snapshot_confidence CHECK (confidence IN (
        'HIGH', 'MEDIUM', 'LOW', 'UNVERIFIED'
    ))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_snapshot_current ON kyc.ownership_snapshots(
    issuer_entity_id, owner_entity_id, COALESCE(share_class_id, '00000000-0000-0000-0000-000000000000'::uuid), as_of_date, basis, derived_from
) WHERE superseded_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_snapshot_issuer ON kyc.ownership_snapshots(issuer_entity_id, as_of_date)
    WHERE superseded_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_snapshot_owner ON kyc.ownership_snapshots(owner_entity_id, as_of_date)
    WHERE superseded_at IS NULL;

COMMENT ON TABLE kyc.ownership_snapshots IS
    'Computed ownership positions from register, or imported from BODS/GLEIF. Bridge for reconciliation.';

-- ============================================================================
-- 1.11: Reconciliation Framework
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.ownership_reconciliation_runs (
    run_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    as_of_date DATE NOT NULL,
    basis VARCHAR(20) NOT NULL,

    -- What we're comparing
    source_a VARCHAR(20) NOT NULL,
    source_b VARCHAR(20) NOT NULL,

    -- Config
    tolerance_bps INTEGER DEFAULT 100,

    -- Results
    status VARCHAR(20) DEFAULT 'RUNNING',
    total_entities INTEGER,
    matched_count INTEGER,
    mismatched_count INTEGER,
    missing_in_a_count INTEGER,
    missing_in_b_count INTEGER,

    -- Audit
    started_at TIMESTAMPTZ DEFAULT now(),
    completed_at TIMESTAMPTZ,
    triggered_by VARCHAR(100),
    notes TEXT,

    CONSTRAINT chk_recon_status CHECK (status IN (
        'RUNNING', 'COMPLETED', 'FAILED', 'CANCELLED'
    ))
);

CREATE TABLE IF NOT EXISTS kyc.ownership_reconciliation_findings (
    finding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES kyc.ownership_reconciliation_runs(run_id) ON DELETE CASCADE,

    -- The entity being reconciled
    owner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Comparison
    source_a_pct NUMERIC(8,4),
    source_b_pct NUMERIC(8,4),
    delta_bps INTEGER,

    -- Finding type
    finding_type VARCHAR(30) NOT NULL,
    severity VARCHAR(10),

    -- Resolution
    resolution_status VARCHAR(20) DEFAULT 'OPEN',
    resolution_notes TEXT,
    resolved_by VARCHAR(100),
    resolved_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT chk_finding_type CHECK (finding_type IN (
        'MATCH', 'MISMATCH', 'MISSING_IN_REGISTER', 'MISSING_IN_EXTERNAL',
        'ENTITY_NOT_MAPPED', 'BASIS_MISMATCH'
    )),
    CONSTRAINT chk_finding_severity CHECK (severity IS NULL OR severity IN (
        'INFO', 'WARN', 'ERROR', 'CRITICAL'
    )),
    CONSTRAINT chk_resolution_status CHECK (resolution_status IN (
        'OPEN', 'ACKNOWLEDGED', 'INVESTIGATING', 'RESOLVED', 'FALSE_POSITIVE'
    ))
);

CREATE INDEX IF NOT EXISTS idx_recon_findings_run ON kyc.ownership_reconciliation_findings(run_id);
CREATE INDEX IF NOT EXISTS idx_recon_findings_open ON kyc.ownership_reconciliation_findings(resolution_status)
    WHERE resolution_status = 'OPEN';

-- ============================================================================
-- 1.12: BODS Interest Type Mapping
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.bods_right_type_mapping (
    bods_interest_type VARCHAR(50) PRIMARY KEY,
    maps_to_right_type VARCHAR(30),
    maps_to_control BOOLEAN DEFAULT false,
    maps_to_voting BOOLEAN DEFAULT false,
    maps_to_economic BOOLEAN DEFAULT false,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);

INSERT INTO kyc.bods_right_type_mapping (bods_interest_type, maps_to_right_type, maps_to_control, maps_to_voting, maps_to_economic, notes) VALUES
    ('shareholding', NULL, false, true, true, 'Maps to voting/economic pct computation'),
    ('voting-rights', NULL, true, true, false, 'Maps to voting pct computation'),
    ('right-to-share-in-surplus-assets', NULL, false, false, true, 'Economic only'),
    ('right-to-appoint-and-remove-directors', 'BOARD_APPOINTMENT', true, false, false, NULL),
    ('right-to-appoint-and-remove-members', 'BOARD_APPOINTMENT', true, false, false, NULL),
    ('right-to-exercise-significant-influence-or-control', 'PROTECTIVE_PROVISION', true, false, false, NULL),
    ('rights-under-a-shareholders-agreement', NULL, true, false, false, 'Needs manual review'),
    ('rights-under-articles-of-association', NULL, true, false, false, 'Needs manual review'),
    ('rights-under-a-contract', NULL, false, false, false, 'Needs manual review'),
    ('rights-under-the-law', NULL, false, false, false, 'Jurisdiction specific')
ON CONFLICT (bods_interest_type) DO NOTHING;

COMMENT ON TABLE kyc.bods_right_type_mapping IS
    'Maps BODS 0.4 interest types to special_rights.right_type for reconciliation.';

-- ============================================================================
-- 2.1: Function - Compute supply at any as-of date from events ledger
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_share_class_supply_at(
    p_share_class_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    share_class_id UUID,
    authorized_units NUMERIC,
    issued_units NUMERIC,
    outstanding_units NUMERIC,
    treasury_units NUMERIC,
    total_votes NUMERIC,
    total_economic NUMERIC,
    as_of_date DATE
) AS $$
DECLARE
    v_votes_per_unit NUMERIC;
    v_economic_per_unit NUMERIC;
    v_authorized NUMERIC;
    v_issued NUMERIC := 0;
    v_treasury NUMERIC := 0;
BEGIN
    -- Get share class attributes
    SELECT
        COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1),
        COALESCE(sc.economic_per_unit, 1),
        sc.authorized_shares
    INTO v_votes_per_unit, v_economic_per_unit, v_authorized
    FROM kyc.share_classes sc
    WHERE sc.id = p_share_class_id;

    -- Compute issued from events up to as_of
    SELECT COALESCE(SUM(
        CASE
            WHEN ie.event_type IN ('INITIAL_ISSUE', 'NEW_ISSUE', 'BONUS_ISSUE', 'MERGER_IN', 'TREASURY_RELEASE', 'CONVERSION')
                THEN ie.units_delta
            WHEN ie.event_type IN ('CANCELLATION', 'BUYBACK', 'MERGER_OUT')
                THEN -ABS(ie.units_delta)
            WHEN ie.event_type = 'STOCK_SPLIT'
                THEN (SELECT COALESCE(SUM(ie2.units_delta), 0) FROM kyc.issuance_events ie2
                      WHERE ie2.share_class_id = p_share_class_id
                      AND ie2.effective_date < ie.effective_date
                      AND ie2.status = 'EFFECTIVE') * (ie.ratio_to::NUMERIC / ie.ratio_from - 1)
            ELSE 0
        END
    ), 0)
    INTO v_issued
    FROM kyc.issuance_events ie
    WHERE ie.share_class_id = p_share_class_id
      AND ie.effective_date <= p_as_of
      AND ie.status = 'EFFECTIVE';

    -- Compute treasury
    SELECT COALESCE(SUM(
        CASE
            WHEN ie.event_type = 'BUYBACK' THEN ie.units_delta
            WHEN ie.event_type IN ('TREASURY_RELEASE', 'TREASURY_TRANSFER') THEN -ie.units_delta
            ELSE 0
        END
    ), 0)
    INTO v_treasury
    FROM kyc.issuance_events ie
    WHERE ie.share_class_id = p_share_class_id
      AND ie.effective_date <= p_as_of
      AND ie.status = 'EFFECTIVE';

    RETURN QUERY SELECT
        p_share_class_id,
        v_authorized,
        v_issued,
        v_issued - v_treasury,
        v_treasury,
        v_issued * COALESCE(v_votes_per_unit, 1),
        v_issued * COALESCE(v_economic_per_unit, 1),
        p_as_of;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION kyc.fn_share_class_supply_at IS
    'Compute supply at any as-of date from the issuance events ledger.';

-- ============================================================================
-- 2.2: Function - Compute diluted supply (FULLY_DILUTED / EXERCISABLE)
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_diluted_supply_at(
    p_share_class_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE,
    p_basis TEXT DEFAULT 'FULLY_DILUTED'
)
RETURNS TABLE (
    share_class_id UUID,
    issued_units NUMERIC,
    outstanding_units NUMERIC,
    dilution_units NUMERIC,
    fully_diluted_units NUMERIC,
    total_votes NUMERIC,
    total_economic NUMERIC,
    dilution_source_count INTEGER,
    as_of_date DATE
) AS $$
DECLARE
    v_base RECORD;
    v_dilution NUMERIC := 0;
    v_dilution_count INTEGER := 0;
    v_votes_per_unit NUMERIC;
    v_economic_per_unit NUMERIC;
BEGIN
    -- Get base supply from issuance events
    SELECT * INTO v_base
    FROM kyc.fn_share_class_supply_at(p_share_class_id, p_as_of);

    -- Get voting/economic multipliers
    SELECT COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1), COALESCE(sc.economic_per_unit, 1)
    INTO v_votes_per_unit, v_economic_per_unit
    FROM kyc.share_classes sc
    WHERE sc.id = p_share_class_id;

    -- Compute dilution from instruments that convert INTO this share class
    IF p_basis = 'FULLY_DILUTED' THEN
        -- All outstanding instruments (vested or not)
        SELECT
            COALESCE(SUM((di.units_granted - di.units_exercised - di.units_forfeited) * di.conversion_ratio), 0),
            COUNT(*)
        INTO v_dilution, v_dilution_count
        FROM kyc.dilution_instruments di
        WHERE di.converts_to_share_class_id = p_share_class_id
          AND di.status = 'ACTIVE'
          AND (di.expiration_date IS NULL OR di.expiration_date > p_as_of);

    ELSIF p_basis = 'EXERCISABLE' THEN
        -- Only currently exercisable instruments
        SELECT
            COALESCE(SUM((di.units_granted - di.units_exercised - di.units_forfeited) * di.conversion_ratio), 0),
            COUNT(*)
        INTO v_dilution, v_dilution_count
        FROM kyc.dilution_instruments di
        WHERE di.converts_to_share_class_id = p_share_class_id
          AND di.status = 'ACTIVE'
          AND (di.exercisable_from IS NULL OR di.exercisable_from <= p_as_of)
          AND (di.expiration_date IS NULL OR di.expiration_date > p_as_of);
    ELSE
        -- Default to no dilution for ISSUED/OUTSTANDING
        v_dilution := 0;
        v_dilution_count := 0;
    END IF;

    RETURN QUERY SELECT
        p_share_class_id,
        v_base.issued_units,
        v_base.outstanding_units,
        v_dilution,
        v_base.outstanding_units + v_dilution,
        (v_base.outstanding_units + v_dilution) * COALESCE(v_votes_per_unit, 1),
        (v_base.outstanding_units + v_dilution) * COALESCE(v_economic_per_unit, 1),
        v_dilution_count,
        p_as_of;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION kyc.fn_diluted_supply_at IS
    'Compute supply including potential dilution from options/warrants/convertibles.
     FULLY_DILUTED = all outstanding instruments. EXERCISABLE = only currently exercisable.';

-- ============================================================================
-- 2.3: Function - Holder control position
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_holder_control_position(
    p_issuer_entity_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE,
    p_basis TEXT DEFAULT 'VOTES'
)
RETURNS TABLE (
    issuer_entity_id UUID,
    issuer_name TEXT,
    holder_entity_id UUID,
    holder_name TEXT,
    holder_type TEXT,
    holder_units NUMERIC,
    holder_votes NUMERIC,
    holder_economic NUMERIC,
    total_issuer_votes NUMERIC,
    total_issuer_economic NUMERIC,
    voting_pct NUMERIC,
    economic_pct NUMERIC,
    control_threshold_pct NUMERIC,
    significant_threshold_pct NUMERIC,
    has_control BOOLEAN,
    has_significant_influence BOOLEAN,
    has_board_rights BOOLEAN,
    board_seats INTEGER
) AS $$
BEGIN
    RETURN QUERY
    WITH issuer_supply AS (
        -- Aggregate supply across all share classes for issuer
        SELECT
            SUM(COALESCE(scs.issued_units, sc.issued_shares, 0) * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1)) AS total_votes,
            SUM(COALESCE(scs.issued_units, sc.issued_shares, 0) * COALESCE(sc.economic_per_unit, 1)) AS total_economic
        FROM kyc.share_classes sc
        LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
            AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id AND as_of_date <= p_as_of)
        WHERE sc.issuer_entity_id = p_issuer_entity_id
    ),
    holder_positions AS (
        -- Aggregate holdings per holder across all classes
        SELECT
            h.investor_entity_id,
            SUM(h.units) AS units,
            SUM(h.units * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1)) AS votes,
            SUM(h.units * COALESCE(sc.economic_per_unit, 1)) AS economic
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        WHERE sc.issuer_entity_id = p_issuer_entity_id
          AND h.status = 'active'
        GROUP BY h.investor_entity_id
    ),
    holder_rights AS (
        -- Check for board appointment rights
        SELECT
            sr.holder_entity_id,
            COALESCE(SUM(sr.board_seats), 0) AS board_seats
        FROM kyc.special_rights sr
        WHERE sr.issuer_entity_id = p_issuer_entity_id
          AND sr.holder_entity_id IS NOT NULL
          AND sr.right_type = 'BOARD_APPOINTMENT'
          AND (sr.effective_to IS NULL OR sr.effective_to > p_as_of)
          AND (sr.effective_from IS NULL OR sr.effective_from <= p_as_of)
        GROUP BY sr.holder_entity_id
    ),
    config AS (
        SELECT
            COALESCE(icc.control_threshold_pct, 50) AS control_threshold,
            COALESCE(icc.significant_threshold_pct, 25) AS significant_threshold
        FROM kyc.issuer_control_config icc
        WHERE icc.issuer_entity_id = p_issuer_entity_id
          AND (icc.effective_to IS NULL OR icc.effective_to > p_as_of)
          AND icc.effective_from <= p_as_of
        ORDER BY icc.effective_from DESC
        LIMIT 1
    )
    SELECT
        p_issuer_entity_id,
        ie.name::TEXT,
        hp.investor_entity_id,
        he.name::TEXT,
        het.type_code::TEXT,
        hp.units,
        hp.votes,
        hp.economic,
        isu.total_votes,
        isu.total_economic,
        CASE WHEN isu.total_votes > 0 THEN ROUND((hp.votes / isu.total_votes) * 100, 4) ELSE 0 END,
        CASE WHEN isu.total_economic > 0 THEN ROUND((hp.economic / isu.total_economic) * 100, 4) ELSE 0 END,
        COALESCE(cfg.control_threshold, 50),
        COALESCE(cfg.significant_threshold, 25),
        CASE WHEN isu.total_votes > 0 AND (hp.votes / isu.total_votes) * 100 > COALESCE(cfg.control_threshold, 50) THEN true ELSE false END,
        CASE WHEN isu.total_votes > 0 AND (hp.votes / isu.total_votes) * 100 > COALESCE(cfg.significant_threshold, 25) THEN true ELSE false END,
        COALESCE(hr.board_seats, 0) > 0,
        COALESCE(hr.board_seats, 0)::INTEGER
    FROM holder_positions hp
    CROSS JOIN issuer_supply isu
    LEFT JOIN config cfg ON true
    LEFT JOIN holder_rights hr ON hr.holder_entity_id = hp.investor_entity_id
    JOIN "ob-poc".entities ie ON ie.entity_id = p_issuer_entity_id
    JOIN "ob-poc".entities he ON he.entity_id = hp.investor_entity_id
    LEFT JOIN "ob-poc".entity_types het ON he.entity_type_id = het.entity_type_id
    ORDER BY hp.votes DESC;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION kyc.fn_holder_control_position IS
    'Compute holder control positions for an issuer including voting %, economic %, control flags, and board rights.';

-- ============================================================================
-- 2.4: Function - Derive ownership snapshots from register
-- ============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_derive_ownership_snapshots(
    p_issuer_entity_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE
)
RETURNS INTEGER AS $$
DECLARE
    v_count INTEGER := 0;
BEGIN
    -- Supersede existing register-derived snapshots for this issuer/date
    UPDATE kyc.ownership_snapshots
    SET superseded_at = now()
    WHERE issuer_entity_id = p_issuer_entity_id
      AND as_of_date = p_as_of
      AND derived_from = 'REGISTER'
      AND superseded_at IS NULL;

    -- Insert VOTES basis snapshots
    INSERT INTO kyc.ownership_snapshots (
        issuer_entity_id, owner_entity_id, share_class_id, as_of_date,
        basis, units, percentage, numerator, denominator,
        derived_from, is_direct, is_aggregated, confidence
    )
    SELECT
        p_issuer_entity_id,
        hcp.holder_entity_id,
        NULL,  -- Aggregated across classes
        p_as_of,
        'VOTES',
        hcp.holder_units,
        hcp.voting_pct,
        hcp.holder_votes,
        hcp.total_issuer_votes,
        'REGISTER',
        true,
        true,
        'HIGH'
    FROM kyc.fn_holder_control_position(p_issuer_entity_id, p_as_of, 'VOTES') hcp
    WHERE hcp.holder_votes > 0;

    GET DIAGNOSTICS v_count = ROW_COUNT;

    -- Insert ECONOMIC basis snapshots
    INSERT INTO kyc.ownership_snapshots (
        issuer_entity_id, owner_entity_id, share_class_id, as_of_date,
        basis, units, percentage, numerator, denominator,
        derived_from, is_direct, is_aggregated, confidence
    )
    SELECT
        p_issuer_entity_id,
        hcp.holder_entity_id,
        NULL,
        p_as_of,
        'ECONOMIC',
        hcp.holder_units,
        hcp.economic_pct,
        hcp.holder_economic,
        hcp.total_issuer_economic,
        'REGISTER',
        true,
        true,
        'HIGH'
    FROM kyc.fn_holder_control_position(p_issuer_entity_id, p_as_of, 'ECONOMIC') hcp
    WHERE hcp.holder_economic > 0;

    GET DIAGNOSTICS v_count = v_count + ROW_COUNT;

    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_derive_ownership_snapshots IS
    'Derive ownership snapshots from register holdings for an issuer at a given date. Returns count of snapshots created.';

-- ============================================================================
-- 2.5: Enhanced Capital Structure View (extends B.2 from migration 009)
-- ============================================================================

CREATE OR REPLACE VIEW kyc.v_capital_structure_extended AS
SELECT
    sc.id AS share_class_id,
    sc.cbu_id,
    sc.issuer_entity_id,
    sc.name AS share_class_name,
    sc.share_type,
    COALESCE(sc.instrument_kind, 'FUND_UNIT') AS instrument_kind,
    sc.class_category,
    sc.authorized_shares,
    COALESCE(scs.issued_units, sc.issued_shares) AS issued_shares,
    COALESCE(scs.outstanding_units, sc.issued_shares) AS outstanding_shares,
    COALESCE(scs.treasury_units, 0) AS treasury_shares,
    COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1) AS votes_per_unit,
    COALESCE(sc.economic_per_unit, 1) AS economic_per_unit,
    sc.par_value,
    sc.par_value_currency,
    sc.dividend_rights,
    sc.liquidation_preference,
    sc.liquidation_rank,
    -- Primary identifier
    sci.scheme_code AS primary_id_scheme,
    sci.identifier_value AS primary_id_value,
    -- Ownership data
    h.id AS holding_id,
    h.investor_entity_id,
    h.units,
    h.cost_basis,
    h.status AS holding_status,
    -- Ownership calculation
    CASE
        WHEN COALESCE(scs.issued_units, sc.issued_shares, 0) > 0
        THEN ROUND((h.units / COALESCE(scs.issued_units, sc.issued_shares)) * 100, 4)
        ELSE 0
    END AS ownership_pct,
    -- Voting rights calculation
    h.units * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1) AS holder_voting_rights,
    COALESCE(scs.issued_units, sc.issued_shares, 0) * COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1) AS total_class_voting_rights,
    -- Entity details
    e.name AS investor_name,
    et.type_code AS investor_entity_type,
    ie.name AS issuer_name,
    iet.type_code AS issuer_entity_type
FROM kyc.share_classes sc
LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
    AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id)
LEFT JOIN kyc.share_class_identifiers sci ON sci.share_class_id = sc.id AND sci.is_primary = true AND sci.valid_to IS NULL
LEFT JOIN kyc.holdings h ON h.share_class_id = sc.id AND h.status = 'active'
LEFT JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
LEFT JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entities ie ON ie.entity_id = sc.issuer_entity_id
LEFT JOIN "ob-poc".entity_types iet ON ie.entity_type_id = iet.entity_type_id;

COMMENT ON VIEW kyc.v_capital_structure_extended IS
    'Extended capital structure view with supply tracking, identifiers, and computed ownership/voting percentages.';

-- ============================================================================
-- 2.6: Dilution Summary View
-- ============================================================================

CREATE OR REPLACE VIEW kyc.v_dilution_summary AS
SELECT
    di.issuer_entity_id,
    ie.name AS issuer_name,
    di.converts_to_share_class_id,
    sc.name AS target_share_class_name,
    di.instrument_type,
    di.status,
    COUNT(*) AS instrument_count,
    SUM(di.units_granted) AS total_granted,
    SUM(di.units_exercised) AS total_exercised,
    SUM(di.units_forfeited) AS total_forfeited,
    SUM(di.units_granted - di.units_exercised - di.units_forfeited) AS total_outstanding,
    SUM((di.units_granted - di.units_exercised - di.units_forfeited) * di.conversion_ratio) AS potential_dilution_shares,
    -- As percentage of current outstanding
    CASE
        WHEN COALESCE(scs.outstanding_units, sc.issued_shares, 0) > 0 THEN
            ROUND(
                SUM((di.units_granted - di.units_exercised - di.units_forfeited) * di.conversion_ratio)
                / COALESCE(scs.outstanding_units, sc.issued_shares) * 100,
                2
            )
        ELSE 0
    END AS dilution_pct
FROM kyc.dilution_instruments di
JOIN "ob-poc".entities ie ON ie.entity_id = di.issuer_entity_id
LEFT JOIN kyc.share_classes sc ON sc.id = di.converts_to_share_class_id
LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
    AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id)
WHERE di.status = 'ACTIVE'
GROUP BY
    di.issuer_entity_id, ie.name,
    di.converts_to_share_class_id, sc.name,
    di.instrument_type, di.status,
    scs.outstanding_units, sc.issued_shares;

COMMENT ON VIEW kyc.v_dilution_summary IS
    'Summary of dilution instruments by issuer and type, showing potential dilution impact.';

-- ============================================================================
-- Done
-- ============================================================================
-- Migration: 014_capital_idempotency_keys.sql
-- Purpose: Add idempotency keys for transactional safety on complex capital operations
-- Date: 2026-01-10

-- =============================================================================
-- IDEMPOTENCY KEYS
-- Prevent duplicate operations from retries (network failures, client retries)
-- =============================================================================

-- Add idempotency_key to issuance_events (for splits, consolidations)
ALTER TABLE kyc.issuance_events
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(100) UNIQUE;

-- Add idempotency_key to dilution_exercise_events (for option exercises)
ALTER TABLE kyc.dilution_exercise_events
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(100) UNIQUE;

-- Create index for fast idempotency lookups
CREATE INDEX IF NOT EXISTS idx_issuance_idempotency
    ON kyc.issuance_events(idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_exercise_idempotency
    ON kyc.dilution_exercise_events(idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- =============================================================================
-- HELPER FUNCTION: Convert UUID to advisory lock ID
-- PostgreSQL advisory locks use bigint, so we hash the UUID
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.uuid_to_lock_id(p_uuid UUID)
RETURNS BIGINT AS $$
BEGIN
    -- Use hashtext on the UUID string to get a stable bigint
    RETURN ('x' || substr(md5(p_uuid::text), 1, 16))::bit(64)::bigint;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION kyc.uuid_to_lock_id IS
'Convert UUID to bigint for use with pg_advisory_xact_lock. Uses MD5 hash for stability.';
-- =============================================================================
-- Migration 015: Research Workflows
-- =============================================================================
-- Implements the "Bounded Non-Determinism" pattern for research workflows:
-- - Phase 1 (LLM exploration) decisions are audited in research_decisions
-- - Phase 2 (DSL execution) actions are audited in research_actions
-- - Corrections track when wrong identifiers were selected
-- - Anomalies flag post-import validation issues
-- =============================================================================

-- =============================================================================
-- RESEARCH DECISIONS (Phase 1 Audit Trail)
-- =============================================================================
-- Captures the non-deterministic search and selection process.
-- Every time an LLM picks an identifier (LEI, company number, etc.), log it here.

CREATE TABLE IF NOT EXISTS kyc.research_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What triggered this research
    trigger_id UUID,  -- Optional link to ownership_research_triggers if exists
    target_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Search context
    search_query TEXT NOT NULL,
    search_context JSONB,  -- {jurisdiction, entity_type, parent_context, etc.}

    -- Source used
    source_provider VARCHAR(30) NOT NULL,  -- gleif, companies_house, sec, orbis, etc.

    -- Candidates found
    candidates_found JSONB NOT NULL DEFAULT '[]',  -- [{key, name, score, metadata}]
    candidates_count INTEGER NOT NULL DEFAULT 0,

    -- Selection
    selected_key VARCHAR(100),  -- LEI, company number, CIK, BvD ID, etc.
    selected_key_type VARCHAR(20),  -- LEI, COMPANY_NUMBER, CIK, BVD_ID
    selection_confidence DECIMAL(3,2),  -- 0.00 - 1.00
    selection_reasoning TEXT NOT NULL,

    -- Decision type
    decision_type VARCHAR(20) NOT NULL,

    -- Verification
    auto_selected BOOLEAN NOT NULL DEFAULT false,
    verified_by UUID,  -- User who confirmed (if not auto)
    verified_at TIMESTAMPTZ,

    -- Link to resulting action
    resulting_action_id UUID,  -- Points to research_actions if executed

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    session_id UUID,

    CONSTRAINT chk_decision_type CHECK (
        decision_type IN (
            'AUTO_SELECTED',     -- High confidence, proceeded automatically
            'USER_SELECTED',     -- User picked from candidates
            'USER_CONFIRMED',    -- Auto-selected but user confirmed
            'NO_MATCH',          -- No suitable candidates found
            'AMBIGUOUS',         -- Multiple candidates, awaiting user input
            'REJECTED'           -- User rejected suggested match
        )
    ),
    CONSTRAINT chk_source_provider CHECK (
        source_provider IN (
            'gleif', 'companies_house', 'sec', 'orbis',
            'open_corporates', 'screening', 'manual', 'document'
        )
    )
);

CREATE INDEX idx_research_decisions_target ON kyc.research_decisions(target_entity_id);
CREATE INDEX idx_research_decisions_type ON kyc.research_decisions(decision_type);
CREATE INDEX idx_research_decisions_source ON kyc.research_decisions(source_provider);
CREATE INDEX idx_research_decisions_created ON kyc.research_decisions(created_at);

COMMENT ON TABLE kyc.research_decisions IS
'Audit trail for Phase 1 (LLM exploration) decisions. Captures the non-deterministic
search and selection process for later review and correction.';

-- =============================================================================
-- RESEARCH ACTIONS (Phase 2 Audit Trail)
-- =============================================================================
-- Every import/update via research verbs is logged here.
-- Links back to the decision that provided the identifier.

CREATE TABLE IF NOT EXISTS kyc.research_actions (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What entity this affects
    target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Link to decision that triggered this
    decision_id UUID REFERENCES kyc.research_decisions(decision_id),

    -- Action details
    action_type VARCHAR(50) NOT NULL,  -- IMPORT_HIERARCHY, IMPORT_PSC, ENRICH, etc.
    source_provider VARCHAR(30) NOT NULL,
    source_key VARCHAR(100) NOT NULL,  -- The identifier used
    source_key_type VARCHAR(20) NOT NULL,

    -- DSL verb executed
    verb_domain VARCHAR(30) NOT NULL,
    verb_name VARCHAR(50) NOT NULL,
    verb_args JSONB NOT NULL DEFAULT '{}',

    -- Outcome
    success BOOLEAN NOT NULL,

    -- Changes made (if successful)
    entities_created INTEGER DEFAULT 0,
    entities_updated INTEGER DEFAULT 0,
    relationships_created INTEGER DEFAULT 0,
    fields_updated JSONB,  -- [{entity_id, field, old_value, new_value}]

    -- Errors (if failed)
    error_code VARCHAR(50),
    error_message TEXT,

    -- Performance
    duration_ms INTEGER,
    api_calls_made INTEGER,

    -- Audit
    executed_at TIMESTAMPTZ DEFAULT NOW(),
    executed_by UUID,
    session_id UUID,

    -- Rollback support
    is_rolled_back BOOLEAN DEFAULT false,
    rolled_back_at TIMESTAMPTZ,
    rolled_back_by UUID,
    rollback_reason TEXT
);

CREATE INDEX idx_research_actions_target ON kyc.research_actions(target_entity_id);
CREATE INDEX idx_research_actions_decision ON kyc.research_actions(decision_id);
CREATE INDEX idx_research_actions_verb ON kyc.research_actions(verb_domain, verb_name);
CREATE INDEX idx_research_actions_executed ON kyc.research_actions(executed_at);
CREATE INDEX idx_research_actions_success ON kyc.research_actions(success);

COMMENT ON TABLE kyc.research_actions IS
'Audit trail for Phase 2 (DSL execution). Every import/update via research verbs
is logged here with full details for reproducibility and rollback.';

-- Update research_decisions with resulting action link
-- (Can't add FK constraint due to circular reference, but we track the link)

-- =============================================================================
-- RESEARCH CORRECTIONS
-- =============================================================================
-- Tracks when a wrong identifier was selected and corrected.

CREATE TABLE IF NOT EXISTS kyc.research_corrections (
    correction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What's being corrected
    original_decision_id UUID NOT NULL REFERENCES kyc.research_decisions(decision_id),
    original_action_id UUID REFERENCES kyc.research_actions(action_id),

    -- Correction details
    correction_type VARCHAR(20) NOT NULL,

    -- Wrong selection
    wrong_key VARCHAR(100),
    wrong_key_type VARCHAR(20),

    -- Correct selection
    correct_key VARCHAR(100),
    correct_key_type VARCHAR(20),

    -- New action (if re-imported)
    new_action_id UUID REFERENCES kyc.research_actions(action_id),

    -- Why
    correction_reason TEXT NOT NULL,

    -- Who/when
    corrected_at TIMESTAMPTZ DEFAULT NOW(),
    corrected_by UUID NOT NULL,

    CONSTRAINT chk_correction_type CHECK (
        correction_type IN (
            'WRONG_ENTITY',       -- Selected wrong entity entirely
            'WRONG_JURISDICTION', -- Right name, wrong jurisdiction
            'STALE_DATA',         -- Data was outdated
            'MERGE_REQUIRED',     -- Need to merge with existing
            'UNLINK'              -- Remove incorrect link
        )
    )
);

CREATE INDEX idx_corrections_decision ON kyc.research_corrections(original_decision_id);
CREATE INDEX idx_corrections_corrected ON kyc.research_corrections(corrected_at);

COMMENT ON TABLE kyc.research_corrections IS
'Tracks corrections when Phase 1 selected the wrong identifier.
Supports learning and audit trail for regulatory inquiries.';

-- =============================================================================
-- RESEARCH ANOMALIES
-- =============================================================================
-- Post-import validation flags anomalies for review.

CREATE TABLE IF NOT EXISTS kyc.research_anomalies (
    anomaly_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What action triggered this
    action_id UUID NOT NULL REFERENCES kyc.research_actions(action_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Anomaly details
    rule_code VARCHAR(50) NOT NULL,
    severity VARCHAR(10) NOT NULL,
    description TEXT NOT NULL,

    -- Context
    expected_value TEXT,
    actual_value TEXT,

    -- Resolution
    status VARCHAR(20) DEFAULT 'OPEN',
    resolution TEXT,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,

    -- Audit
    detected_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT chk_anomaly_severity CHECK (severity IN ('ERROR', 'WARNING', 'INFO')),
    CONSTRAINT chk_anomaly_status CHECK (status IN ('OPEN', 'ACKNOWLEDGED', 'RESOLVED', 'FALSE_POSITIVE'))
);

CREATE INDEX idx_anomalies_action ON kyc.research_anomalies(action_id);
CREATE INDEX idx_anomalies_entity ON kyc.research_anomalies(entity_id);
CREATE INDEX idx_anomalies_status ON kyc.research_anomalies(status);

COMMENT ON TABLE kyc.research_anomalies IS
'Post-import validation anomalies. Flags issues like jurisdiction mismatch,
circular ownership, or duplicate entities for human review.';

-- =============================================================================
-- RESEARCH CONFIDENCE CONFIG
-- =============================================================================
-- Configurable thresholds for auto-proceed vs ambiguous vs reject.

CREATE TABLE IF NOT EXISTS kyc.research_confidence_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope (global or per-source)
    source_provider VARCHAR(30),  -- NULL = global default

    -- Thresholds
    auto_proceed_threshold DECIMAL(3,2) DEFAULT 0.90,
    ambiguous_threshold DECIMAL(3,2) DEFAULT 0.70,
    reject_threshold DECIMAL(3,2) DEFAULT 0.50,

    -- Behavior
    require_human_checkpoint BOOLEAN DEFAULT false,
    checkpoint_contexts TEXT[],  -- {'NEW_CLIENT', 'MATERIAL_HOLDING', 'HIGH_RISK'}

    -- Limits
    max_auto_imports_per_session INTEGER DEFAULT 50,
    max_chain_depth INTEGER DEFAULT 10,

    -- Active
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,

    CONSTRAINT uq_confidence_source UNIQUE (source_provider, effective_from)
);

-- Seed defaults
INSERT INTO kyc.research_confidence_config (
    source_provider, auto_proceed_threshold, ambiguous_threshold, require_human_checkpoint
) VALUES
    (NULL, 0.90, 0.70, false),           -- Global default
    ('gleif', 0.92, 0.75, false),        -- GLEIF is authoritative, high bar
    ('companies_house', 0.88, 0.70, false),
    ('orbis', 0.85, 0.65, false),        -- Commercial, slightly lower
    ('screening', 0.00, 0.00, true)      -- Always human checkpoint for screening
ON CONFLICT DO NOTHING;

COMMENT ON TABLE kyc.research_confidence_config IS
'Configurable thresholds for confidence-based routing. Determines when to
auto-proceed, ask user to disambiguate, or reject matches.';

-- =============================================================================
-- OUTREACH REQUESTS
-- =============================================================================
-- Track requests sent to counterparties for information.

CREATE TABLE IF NOT EXISTS kyc.outreach_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Target
    target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Request type
    request_type VARCHAR(30) NOT NULL,

    -- Recipient
    recipient_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    recipient_email VARCHAR(255),
    recipient_name VARCHAR(255),

    -- Status
    status VARCHAR(20) DEFAULT 'DRAFT',

    -- Timing
    deadline_date DATE,
    sent_at TIMESTAMPTZ,
    reminder_sent_at TIMESTAMPTZ,

    -- Response
    response_type VARCHAR(30),
    response_received_at TIMESTAMPTZ,
    response_document_id UUID,

    -- Notes
    request_notes TEXT,
    resolution_notes TEXT,

    -- Link to research trigger
    trigger_id UUID,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID,
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT chk_request_type CHECK (
        request_type IN (
            'NOMINEE_DISCLOSURE',
            'UBO_DECLARATION',
            'SHARE_REGISTER',
            'BOARD_COMPOSITION',
            'BENEFICIAL_OWNERSHIP',
            'GENERAL_INQUIRY'
        )
    ),
    CONSTRAINT chk_request_status CHECK (
        status IN ('DRAFT', 'PENDING', 'SENT', 'REMINDED', 'RESPONDED', 'CLOSED', 'EXPIRED')
    ),
    CONSTRAINT chk_response_type CHECK (
        response_type IS NULL OR response_type IN (
            'FULL_DISCLOSURE', 'PARTIAL_DISCLOSURE', 'DECLINED', 'NO_RESPONSE'
        )
    )
);

CREATE INDEX idx_outreach_target ON kyc.outreach_requests(target_entity_id);
CREATE INDEX idx_outreach_status ON kyc.outreach_requests(status);
CREATE INDEX idx_outreach_deadline ON kyc.outreach_requests(deadline_date);

COMMENT ON TABLE kyc.outreach_requests IS
'Tracks outreach requests to counterparties for ownership disclosures,
UBO declarations, and other information gathering.';

-- =============================================================================
-- VIEWS FOR REPORTING
-- =============================================================================

-- Research activity summary by entity
CREATE OR REPLACE VIEW kyc.v_research_activity AS
SELECT
    e.entity_id,
    e.name AS entity_name,
    COUNT(DISTINCT d.decision_id) AS decision_count,
    COUNT(DISTINCT a.action_id) AS action_count,
    COUNT(DISTINCT c.correction_id) AS correction_count,
    COUNT(DISTINCT an.anomaly_id) FILTER (WHERE an.status = 'OPEN') AS open_anomalies,
    MAX(a.executed_at) AS last_research_action
FROM "ob-poc".entities e
LEFT JOIN kyc.research_decisions d ON d.target_entity_id = e.entity_id
LEFT JOIN kyc.research_actions a ON a.target_entity_id = e.entity_id
LEFT JOIN kyc.research_corrections c ON c.original_decision_id = d.decision_id
LEFT JOIN kyc.research_anomalies an ON an.entity_id = e.entity_id
GROUP BY e.entity_id, e.name;

COMMENT ON VIEW kyc.v_research_activity IS
'Summary of research activity per entity for monitoring and reporting.';

-- Pending decisions requiring user input
CREATE OR REPLACE VIEW kyc.v_pending_decisions AS
SELECT
    d.decision_id,
    d.target_entity_id,
    e.name AS entity_name,
    d.search_query,
    d.source_provider,
    d.candidates_count,
    d.candidates_found,
    d.decision_type,
    d.created_at
FROM kyc.research_decisions d
JOIN "ob-poc".entities e ON e.entity_id = d.target_entity_id
WHERE d.decision_type = 'AMBIGUOUS'
  AND d.verified_by IS NULL
ORDER BY d.created_at;

COMMENT ON VIEW kyc.v_pending_decisions IS
'Decisions awaiting user disambiguation or confirmation.';
-- ============================================================================
-- Migration 017: Event Infrastructure
-- ============================================================================
--
-- Creates tables for the always-on event capture system (023a).
--
-- Two main tables:
-- 1. events.log - Append-only DSL execution events
-- 2. sessions.log - Conversation context for session replay
--
-- Design principles:
-- - Append-only (no updates for write performance)
-- - Minimal indexes (partitioned by timestamp)
-- - JSONB payload for flexibility
--
-- ============================================================================

-- Create schemas if not exist
CREATE SCHEMA IF NOT EXISTS events;
CREATE SCHEMA IF NOT EXISTS sessions;

-- ============================================================================
-- events.log - DSL Execution Events
-- ============================================================================
--
-- Captures every DSL command execution (success and failure).
-- Used by the Feedback Inspector for failure analysis.

CREATE TABLE IF NOT EXISTS events.log (
    id BIGSERIAL PRIMARY KEY,

    -- When the event occurred
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Session context (nullable for non-session events)
    session_id UUID,

    -- Event type for quick filtering
    event_type TEXT NOT NULL,
    -- Values: 'command_succeeded', 'command_failed', 'session_started', 'session_ended'

    -- Full event payload as JSONB
    payload JSONB NOT NULL,

    -- Constraint on event_type
    CONSTRAINT valid_event_type CHECK (
        event_type IN ('command_succeeded', 'command_failed', 'session_started', 'session_ended')
    )
);

-- Index for time-based queries (partition key for future partitioning)
CREATE INDEX IF NOT EXISTS idx_events_log_timestamp
    ON events.log (timestamp);

-- Index for session lookups
CREATE INDEX IF NOT EXISTS idx_events_log_session
    ON events.log (session_id, timestamp)
    WHERE session_id IS NOT NULL;

-- Index for failure analysis
CREATE INDEX IF NOT EXISTS idx_events_log_failures
    ON events.log (timestamp)
    WHERE event_type = 'command_failed';

-- Comment
COMMENT ON TABLE events.log IS 'Append-only DSL execution events for observability and failure analysis';

-- ============================================================================
-- sessions.log - Conversation Context
-- ============================================================================
--
-- Captures the full conversation context for a session:
-- - User input
-- - Agent thoughts (in agent mode)
-- - DSL commands
-- - Responses
-- - Errors
--
-- This enables session replay and context-aware failure analysis.

CREATE TABLE IF NOT EXISTS sessions.log (
    id BIGSERIAL PRIMARY KEY,

    -- Session this entry belongs to
    session_id UUID NOT NULL,

    -- When this entry was created
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Entry type
    entry_type TEXT NOT NULL,
    -- Values: 'user_input', 'agent_thought', 'dsl_command', 'response', 'error'

    -- The actual content
    content TEXT NOT NULL,

    -- Link to corresponding event (for DSL commands and errors)
    event_id BIGINT REFERENCES events.log(id),

    -- Source of this session
    source TEXT NOT NULL,
    -- Values: 'repl', 'egui', 'mcp', 'api'

    -- Optional metadata (command args, error context, etc.)
    metadata JSONB DEFAULT '{}',

    -- Constraint on entry_type
    CONSTRAINT valid_entry_type CHECK (
        entry_type IN ('user_input', 'agent_thought', 'dsl_command', 'response', 'error')
    ),

    -- Constraint on source
    CONSTRAINT valid_source CHECK (
        source IN ('repl', 'egui', 'mcp', 'api')
    )
);

-- Primary lookup: entries for a session in order
CREATE INDEX IF NOT EXISTS idx_sessions_log_session
    ON sessions.log (session_id, timestamp);

-- Lookup by event_id (for linking events to context)
CREATE INDEX IF NOT EXISTS idx_sessions_log_event
    ON sessions.log (event_id)
    WHERE event_id IS NOT NULL;

-- Comment
COMMENT ON TABLE sessions.log IS 'Conversation context log for session replay and failure analysis';

-- ============================================================================
-- Helper Views
-- ============================================================================

-- View: Recent failures with session context
CREATE OR REPLACE VIEW events.recent_failures AS
SELECT
    e.id AS event_id,
    e.timestamp,
    e.session_id,
    e.payload->>'verb' AS verb,
    e.payload->'error'->>'message' AS error_message,
    e.payload->'error'->>'error_type' AS error_type,
    (e.payload->>'duration_ms')::integer AS duration_ms
FROM events.log e
WHERE e.event_type = 'command_failed'
ORDER BY e.timestamp DESC
LIMIT 100;

COMMENT ON VIEW events.recent_failures IS 'Recent command failures for quick inspection';

-- View: Session summary
CREATE OR REPLACE VIEW events.session_summary AS
SELECT
    session_id,
    MIN(timestamp) AS started_at,
    MAX(timestamp) AS last_activity,
    COUNT(*) FILTER (WHERE event_type = 'command_succeeded') AS success_count,
    COUNT(*) FILTER (WHERE event_type = 'command_failed') AS failure_count,
    COUNT(*) AS total_events
FROM events.log
WHERE session_id IS NOT NULL
GROUP BY session_id
ORDER BY MAX(timestamp) DESC;

COMMENT ON VIEW events.session_summary IS 'Per-session event summary';

-- ============================================================================
-- Maintenance Functions
-- ============================================================================

-- Function to clean old events (call periodically)
CREATE OR REPLACE FUNCTION events.cleanup_old_events(retention_days INTEGER DEFAULT 30)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM events.log
    WHERE timestamp < NOW() - (retention_days || ' days')::INTERVAL;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION events.cleanup_old_events IS 'Delete events older than retention_days (default 30)';

-- Function to clean old session logs
CREATE OR REPLACE FUNCTION sessions.cleanup_old_logs(retention_days INTEGER DEFAULT 30)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM sessions.log
    WHERE timestamp < NOW() - (retention_days || ' days')::INTERVAL;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION sessions.cleanup_old_logs IS 'Delete session logs older than retention_days (default 30)';
-- 018: Feedback Inspector Schema
-- Implements 023b: On-demand failure analysis, repro generation, audit trail

-- Create feedback schema
CREATE SCHEMA IF NOT EXISTS feedback;

-- =============================================================================
-- ENUMS
-- =============================================================================

-- Error type classification
CREATE TYPE feedback.error_type AS ENUM (
    -- Transient (runtime retry candidates)
    'TIMEOUT',
    'RATE_LIMITED',
    'CONNECTION_RESET',
    'SERVICE_UNAVAILABLE',
    'POOL_EXHAUSTED',

    -- Schema/contract issues (code fix required)
    'ENUM_DRIFT',
    'SCHEMA_DRIFT',

    -- Code bugs (investigation needed)
    'PARSE_ERROR',
    'HANDLER_PANIC',
    'HANDLER_ERROR',
    'DSL_PARSE_ERROR',

    -- External API changes
    'API_ENDPOINT_MOVED',
    'API_AUTH_CHANGED',
    'VALIDATION_FAILED',

    -- Catch-all
    'UNKNOWN'
);

-- Remediation path
CREATE TYPE feedback.remediation_path AS ENUM (
    'RUNTIME',    -- Can be retried/recovered at runtime
    'CODE',       -- Requires code change
    'LOG_ONLY'    -- Just log, no action needed
);

-- Issue lifecycle status
CREATE TYPE feedback.issue_status AS ENUM (
    -- Initial states
    'NEW',
    'RUNTIME_RESOLVED',
    'RUNTIME_ESCALATED',

    -- Repro states
    'REPRO_GENERATED',
    'REPRO_VERIFIED',
    'TODO_CREATED',

    -- Fix states
    'IN_PROGRESS',
    'FIX_COMMITTED',
    'FIX_VERIFIED',

    -- Deployment states
    'DEPLOYED_STAGING',
    'DEPLOYED_PROD',
    'RESOLVED',

    -- Terminal states
    'WONT_FIX',
    'DUPLICATE',
    'INVALID'
);

-- Actor types for audit trail
CREATE TYPE feedback.actor_type AS ENUM (
    'SYSTEM',
    'MCP_AGENT',
    'REPL_USER',
    'EGUI_USER',
    'CI_PIPELINE',
    'CLAUDE_CODE',
    'CRON_JOB'
);

-- Audit actions
CREATE TYPE feedback.audit_action AS ENUM (
    -- Creation
    'CAPTURED',
    'CLASSIFIED',
    'DEDUPLICATED',

    -- Runtime handling
    'RUNTIME_ATTEMPT',
    'RUNTIME_SUCCESS',
    'RUNTIME_EXHAUSTED',

    -- Repro workflow
    'REPRO_GENERATED',
    'REPRO_VERIFIED_FAILS',
    'REPRO_VERIFICATION_FAILED',

    -- TODO workflow
    'TODO_CREATED',
    'TODO_ASSIGNED',
    'FIX_COMMITTED',

    -- Verification
    'REPRO_VERIFIED_PASSES',
    'DEPLOYED',
    'SEMANTIC_REPLAY_PASSED',
    'SEMANTIC_REPLAY_FAILED',

    -- Terminal
    'RESOLVED',
    'MARKED_WONT_FIX',
    'MARKED_DUPLICATE',
    'REOPENED',
    'COMMENT_ADDED'
);

-- =============================================================================
-- TABLES
-- =============================================================================

-- Main failure table (deduplicated by fingerprint)
CREATE TABLE feedback.failures (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fingerprint     TEXT NOT NULL UNIQUE,
    fingerprint_version SMALLINT NOT NULL DEFAULT 1,

    -- Classification
    error_type      feedback.error_type NOT NULL,
    remediation_path feedback.remediation_path NOT NULL,

    -- Status
    status          feedback.issue_status NOT NULL DEFAULT 'NEW',

    -- Source info
    verb            TEXT NOT NULL,
    source          TEXT,  -- e.g., "gleif", "lbr", null for internal

    -- Error details (redacted)
    error_message   TEXT NOT NULL,
    error_context   JSONB,  -- Redacted context for debugging

    -- Session context (what was user trying to do?)
    user_intent     TEXT,
    command_sequence TEXT[],  -- Recent commands leading to failure

    -- Repro info
    repro_type      TEXT,  -- 'golden_json', 'dsl_scenario', 'unit_test'
    repro_path      TEXT,  -- Path to generated test
    repro_verified  BOOLEAN DEFAULT FALSE,

    -- Fix info
    fix_commit      TEXT,
    fix_notes       TEXT,

    -- Counts
    occurrence_count INTEGER NOT NULL DEFAULT 1,

    -- Timestamps
    first_seen_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at     TIMESTAMPTZ,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Individual occurrences (each time we see this fingerprint)
CREATE TABLE feedback.occurrences (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    failure_id      UUID NOT NULL REFERENCES feedback.failures(id) ON DELETE CASCADE,

    -- Event reference
    event_id        UUID,  -- Reference to events.log if stored there
    event_timestamp TIMESTAMPTZ NOT NULL,

    -- Session info
    session_id      UUID,

    -- Execution context
    verb            TEXT NOT NULL,
    duration_ms     BIGINT,

    -- Error snapshot (redacted)
    error_message   TEXT NOT NULL,
    error_backtrace TEXT,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Full audit trail
CREATE TABLE feedback.audit_log (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    failure_id      UUID NOT NULL REFERENCES feedback.failures(id) ON DELETE CASCADE,

    -- Action
    action          feedback.audit_action NOT NULL,

    -- Actor
    actor_type      feedback.actor_type NOT NULL,
    actor_id        TEXT,  -- e.g., session ID, user ID, CI job ID

    -- Details
    details         JSONB,

    -- Evidence (for verification actions)
    evidence        TEXT,  -- e.g., test output
    evidence_hash   TEXT,  -- SHA256 of large evidence

    -- Previous state (for state transitions)
    previous_status feedback.issue_status,
    new_status      feedback.issue_status,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Fingerprint lookup (most common query)
CREATE INDEX idx_failures_fingerprint ON feedback.failures(fingerprint);

-- Status-based queries
CREATE INDEX idx_failures_status ON feedback.failures(status);
CREATE INDEX idx_failures_status_error_type ON feedback.failures(status, error_type);

-- Time-based queries
CREATE INDEX idx_failures_last_seen ON feedback.failures(last_seen_at DESC);
CREATE INDEX idx_failures_first_seen ON feedback.failures(first_seen_at DESC);

-- Source/verb queries
CREATE INDEX idx_failures_verb ON feedback.failures(verb);
CREATE INDEX idx_failures_source ON feedback.failures(source) WHERE source IS NOT NULL;

-- Occurrences
CREATE INDEX idx_occurrences_failure_id ON feedback.occurrences(failure_id);
CREATE INDEX idx_occurrences_session_id ON feedback.occurrences(session_id) WHERE session_id IS NOT NULL;
CREATE INDEX idx_occurrences_timestamp ON feedback.occurrences(event_timestamp DESC);

-- Audit log
CREATE INDEX idx_audit_failure_id ON feedback.audit_log(failure_id);
CREATE INDEX idx_audit_action ON feedback.audit_log(action);
CREATE INDEX idx_audit_created_at ON feedback.audit_log(created_at DESC);

-- =============================================================================
-- VIEWS
-- =============================================================================

-- Active issues needing attention
CREATE VIEW feedback.active_issues AS
SELECT
    f.id,
    f.fingerprint,
    f.error_type,
    f.remediation_path,
    f.status,
    f.verb,
    f.source,
    f.error_message,
    f.user_intent,
    f.occurrence_count,
    f.first_seen_at,
    f.last_seen_at,
    f.repro_verified
FROM feedback.failures f
WHERE f.status NOT IN ('RESOLVED', 'WONT_FIX', 'DUPLICATE', 'INVALID')
ORDER BY
    CASE f.remediation_path
        WHEN 'CODE' THEN 1
        WHEN 'RUNTIME' THEN 2
        ELSE 3
    END,
    f.occurrence_count DESC,
    f.last_seen_at DESC;

-- Issues ready for TODO generation (verified repro but no TODO yet)
CREATE VIEW feedback.ready_for_todo AS
SELECT
    f.id,
    f.fingerprint,
    f.error_type,
    f.verb,
    f.source,
    f.error_message,
    f.user_intent,
    f.repro_path,
    f.occurrence_count
FROM feedback.failures f
WHERE f.status = 'REPRO_VERIFIED'
  AND f.repro_verified = TRUE
ORDER BY f.occurrence_count DESC;

-- Recent audit activity
CREATE VIEW feedback.recent_activity AS
SELECT
    a.id,
    a.failure_id,
    f.fingerprint,
    a.action,
    a.actor_type,
    a.actor_id,
    a.previous_status,
    a.new_status,
    a.created_at
FROM feedback.audit_log a
JOIN feedback.failures f ON f.id = a.failure_id
ORDER BY a.created_at DESC
LIMIT 100;

-- =============================================================================
-- FUNCTIONS
-- =============================================================================

-- Update timestamps trigger
CREATE OR REPLACE FUNCTION feedback.update_timestamps()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER failures_update_timestamps
    BEFORE UPDATE ON feedback.failures
    FOR EACH ROW
    EXECUTE FUNCTION feedback.update_timestamps();

-- Record occurrence and update failure counts
CREATE OR REPLACE FUNCTION feedback.record_occurrence(
    p_fingerprint TEXT,
    p_event_id UUID,
    p_event_timestamp TIMESTAMPTZ,
    p_session_id UUID,
    p_verb TEXT,
    p_duration_ms BIGINT,
    p_error_message TEXT,
    p_error_backtrace TEXT
) RETURNS UUID AS $$
DECLARE
    v_failure_id UUID;
    v_occurrence_id UUID;
BEGIN
    -- Get failure ID
    SELECT id INTO v_failure_id
    FROM feedback.failures
    WHERE fingerprint = p_fingerprint;

    IF v_failure_id IS NULL THEN
        RAISE EXCEPTION 'Failure not found for fingerprint: %', p_fingerprint;
    END IF;

    -- Insert occurrence
    INSERT INTO feedback.occurrences (
        failure_id, event_id, event_timestamp, session_id,
        verb, duration_ms, error_message, error_backtrace
    ) VALUES (
        v_failure_id, p_event_id, p_event_timestamp, p_session_id,
        p_verb, p_duration_ms, p_error_message, p_error_backtrace
    ) RETURNING id INTO v_occurrence_id;

    -- Update failure counts
    UPDATE feedback.failures
    SET occurrence_count = occurrence_count + 1,
        last_seen_at = p_event_timestamp
    WHERE id = v_failure_id;

    RETURN v_occurrence_id;
END;
$$ LANGUAGE plpgsql;

-- Cleanup old resolved issues (keep 90 days)
CREATE OR REPLACE FUNCTION feedback.cleanup_old_resolved()
RETURNS INTEGER AS $$
DECLARE
    v_deleted INTEGER;
BEGIN
    DELETE FROM feedback.failures
    WHERE status = 'RESOLVED'
      AND resolved_at < NOW() - INTERVAL '90 days';

    GET DIAGNOSTICS v_deleted = ROW_COUNT;
    RETURN v_deleted;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON SCHEMA feedback IS 'Feedback Inspector: failure analysis, repro generation, audit trail';
COMMENT ON TABLE feedback.failures IS 'Deduplicated failure records, keyed by fingerprint';
COMMENT ON TABLE feedback.occurrences IS 'Individual occurrences of each failure';
COMMENT ON TABLE feedback.audit_log IS 'Full audit trail of all state transitions';
COMMENT ON VIEW feedback.active_issues IS 'Issues needing attention, prioritized by remediation path';
COMMENT ON VIEW feedback.ready_for_todo IS 'Issues with verified repro, ready for TODO generation';
-- =============================================================================
-- Migration 019: Session Navigation History Enhancement
-- =============================================================================
-- Purpose: Add current history position tracking for back/forward navigation
--   - Add history_position column to session_scopes (current position in history)
--   - Add active_cbu_ids column for multi-CBU set selection
--   - Create helper functions for back/forward navigation
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. Add history position and multi-CBU tracking to session_scopes
-- -----------------------------------------------------------------------------
ALTER TABLE "ob-poc".session_scopes
ADD COLUMN IF NOT EXISTS history_position INTEGER DEFAULT 0;

ALTER TABLE "ob-poc".session_scopes
ADD COLUMN IF NOT EXISTS active_cbu_ids UUID[] DEFAULT '{}';

COMMENT ON COLUMN "ob-poc".session_scopes.history_position IS
'Current position in history stack. -1 means at latest, >=0 means navigated back.';

COMMENT ON COLUMN "ob-poc".session_scopes.active_cbu_ids IS
'Set of active CBU IDs (0..n) for multi-CBU selection workflows.';

-- -----------------------------------------------------------------------------
-- 2. Helper function to push history entry (called on scope change)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".push_scope_history(
    p_session_id UUID,
    p_change_source VARCHAR(50) DEFAULT 'dsl',
    p_change_verb VARCHAR(100) DEFAULT NULL
) RETURNS INTEGER AS $$
DECLARE
    v_current_scope "ob-poc".session_scopes;
    v_new_position INTEGER;
    v_snapshot JSONB;
BEGIN
    -- Get current scope
    SELECT * INTO v_current_scope
    FROM "ob-poc".session_scopes
    WHERE session_id = p_session_id;

    IF v_current_scope IS NULL THEN
        RETURN -1;
    END IF;

    -- If we're not at the end of history, truncate forward history
    -- (like when you navigate back then make a new change)
    IF v_current_scope.history_position >= 0 THEN
        DELETE FROM "ob-poc".session_scope_history
        WHERE session_id = p_session_id
          AND position > v_current_scope.history_position;
    END IF;

    -- Get next position
    SELECT COALESCE(MAX(position), -1) + 1 INTO v_new_position
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id;

    -- Build snapshot from current scope
    v_snapshot := jsonb_build_object(
        'scope_type', v_current_scope.scope_type,
        'apex_entity_id', v_current_scope.apex_entity_id,
        'apex_entity_name', v_current_scope.apex_entity_name,
        'cbu_id', v_current_scope.cbu_id,
        'cbu_name', v_current_scope.cbu_name,
        'jurisdiction_code', v_current_scope.jurisdiction_code,
        'focal_entity_id', v_current_scope.focal_entity_id,
        'focal_entity_name', v_current_scope.focal_entity_name,
        'neighborhood_hops', v_current_scope.neighborhood_hops,
        'scope_filters', v_current_scope.scope_filters,
        'cursor_entity_id', v_current_scope.cursor_entity_id,
        'cursor_entity_name', v_current_scope.cursor_entity_name,
        'active_cbu_ids', v_current_scope.active_cbu_ids
    );

    -- Insert history entry
    INSERT INTO "ob-poc".session_scope_history (
        session_id, position, scope_snapshot, change_source, change_verb
    ) VALUES (
        p_session_id, v_new_position, v_snapshot, p_change_source, p_change_verb
    );

    -- Update current position to "at end" (-1)
    UPDATE "ob-poc".session_scopes
    SET history_position = -1,
        updated_at = NOW()
    WHERE session_id = p_session_id;

    RETURN v_new_position;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".push_scope_history IS
'Push current scope state to history stack. Call before making scope changes.';

-- -----------------------------------------------------------------------------
-- 3. Navigate back in history
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".navigate_back(
    p_session_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_current_scope "ob-poc".session_scopes;
    v_current_pos INTEGER;
    v_target_pos INTEGER;
    v_max_pos INTEGER;
    v_snapshot JSONB;
    v_result "ob-poc".session_scopes;
BEGIN
    -- Get current scope and position
    SELECT * INTO v_current_scope
    FROM "ob-poc".session_scopes
    WHERE session_id = p_session_id;

    IF v_current_scope IS NULL THEN
        RETURN NULL;
    END IF;

    -- Get max history position
    SELECT MAX(position) INTO v_max_pos
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id;

    IF v_max_pos IS NULL THEN
        -- No history, return current scope unchanged
        RETURN v_current_scope;
    END IF;

    -- Calculate current effective position
    IF v_current_scope.history_position < 0 THEN
        -- At end of history, need to save current state first
        PERFORM "ob-poc".push_scope_history(p_session_id, 'navigation', 'session.back');
        v_current_pos := v_max_pos + 1;
    ELSE
        v_current_pos := v_current_scope.history_position;
    END IF;

    -- Calculate target position (one step back)
    v_target_pos := v_current_pos - 1;

    IF v_target_pos < 0 THEN
        -- Already at oldest history entry
        RETURN v_current_scope;
    END IF;

    -- Get the snapshot at target position
    SELECT scope_snapshot INTO v_snapshot
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id
      AND position = v_target_pos;

    IF v_snapshot IS NULL THEN
        RETURN v_current_scope;
    END IF;

    -- Restore scope from snapshot
    UPDATE "ob-poc".session_scopes
    SET scope_type = v_snapshot->>'scope_type',
        apex_entity_id = (v_snapshot->>'apex_entity_id')::UUID,
        apex_entity_name = v_snapshot->>'apex_entity_name',
        cbu_id = (v_snapshot->>'cbu_id')::UUID,
        cbu_name = v_snapshot->>'cbu_name',
        jurisdiction_code = v_snapshot->>'jurisdiction_code',
        focal_entity_id = (v_snapshot->>'focal_entity_id')::UUID,
        focal_entity_name = v_snapshot->>'focal_entity_name',
        neighborhood_hops = (v_snapshot->>'neighborhood_hops')::INTEGER,
        scope_filters = COALESCE(v_snapshot->'scope_filters', '{}'),
        cursor_entity_id = (v_snapshot->>'cursor_entity_id')::UUID,
        cursor_entity_name = v_snapshot->>'cursor_entity_name',
        active_cbu_ids = COALESCE(
            ARRAY(SELECT jsonb_array_elements_text(v_snapshot->'active_cbu_ids')::UUID),
            '{}'
        ),
        history_position = v_target_pos,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".navigate_back IS
'Navigate back one step in scope history. Returns updated session_scopes row.';

-- -----------------------------------------------------------------------------
-- 4. Navigate forward in history
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".navigate_forward(
    p_session_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_current_scope "ob-poc".session_scopes;
    v_target_pos INTEGER;
    v_max_pos INTEGER;
    v_snapshot JSONB;
    v_result "ob-poc".session_scopes;
BEGIN
    -- Get current scope and position
    SELECT * INTO v_current_scope
    FROM "ob-poc".session_scopes
    WHERE session_id = p_session_id;

    IF v_current_scope IS NULL THEN
        RETURN NULL;
    END IF;

    -- If already at end of history, nothing to do
    IF v_current_scope.history_position < 0 THEN
        RETURN v_current_scope;
    END IF;

    -- Get max history position
    SELECT MAX(position) INTO v_max_pos
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id;

    -- Calculate target position (one step forward)
    v_target_pos := v_current_scope.history_position + 1;

    IF v_target_pos > v_max_pos THEN
        -- Moving past end of history - mark as "at end"
        UPDATE "ob-poc".session_scopes
        SET history_position = -1,
            updated_at = NOW()
        WHERE session_id = p_session_id
        RETURNING * INTO v_result;
        RETURN v_result;
    END IF;

    -- Get the snapshot at target position
    SELECT scope_snapshot INTO v_snapshot
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id
      AND position = v_target_pos;

    IF v_snapshot IS NULL THEN
        RETURN v_current_scope;
    END IF;

    -- Restore scope from snapshot
    UPDATE "ob-poc".session_scopes
    SET scope_type = v_snapshot->>'scope_type',
        apex_entity_id = (v_snapshot->>'apex_entity_id')::UUID,
        apex_entity_name = v_snapshot->>'apex_entity_name',
        cbu_id = (v_snapshot->>'cbu_id')::UUID,
        cbu_name = v_snapshot->>'cbu_name',
        jurisdiction_code = v_snapshot->>'jurisdiction_code',
        focal_entity_id = (v_snapshot->>'focal_entity_id')::UUID,
        focal_entity_name = v_snapshot->>'focal_entity_name',
        neighborhood_hops = (v_snapshot->>'neighborhood_hops')::INTEGER,
        scope_filters = COALESCE(v_snapshot->'scope_filters', '{}'),
        cursor_entity_id = (v_snapshot->>'cursor_entity_id')::UUID,
        cursor_entity_name = v_snapshot->>'cursor_entity_name',
        active_cbu_ids = COALESCE(
            ARRAY(SELECT jsonb_array_elements_text(v_snapshot->'active_cbu_ids')::UUID),
            '{}'
        ),
        history_position = v_target_pos,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".navigate_forward IS
'Navigate forward one step in scope history. Returns updated session_scopes row.';

-- -----------------------------------------------------------------------------
-- 5. Multi-CBU set operations
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".add_cbu_to_set(
    p_session_id UUID,
    p_cbu_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET active_cbu_ids = CASE
            WHEN p_cbu_id = ANY(active_cbu_ids) THEN active_cbu_ids  -- Already in set
            ELSE array_append(active_cbu_ids, p_cbu_id)
        END,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "ob-poc".remove_cbu_from_set(
    p_session_id UUID,
    p_cbu_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET active_cbu_ids = array_remove(active_cbu_ids, p_cbu_id),
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "ob-poc".clear_cbu_set(
    p_session_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET active_cbu_ids = '{}',
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "ob-poc".set_cbu_set(
    p_session_id UUID,
    p_cbu_ids UUID[]
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET active_cbu_ids = COALESCE(p_cbu_ids, '{}'),
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".add_cbu_to_set IS 'Add a CBU to the active set';
COMMENT ON FUNCTION "ob-poc".remove_cbu_from_set IS 'Remove a CBU from the active set';
COMMENT ON FUNCTION "ob-poc".clear_cbu_set IS 'Clear the active CBU set';
COMMENT ON FUNCTION "ob-poc".set_cbu_set IS 'Replace the entire active CBU set';

-- -----------------------------------------------------------------------------
-- 6. Update existing scope-setting functions to push history first
-- -----------------------------------------------------------------------------

-- Wrapper for set_scope_galaxy that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_galaxy_with_history(
    p_session_id UUID,
    p_apex_entity_id UUID
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    -- Push current state to history
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-galaxy');
    -- Set new scope
    RETURN "ob-poc".set_scope_galaxy(p_session_id, p_apex_entity_id);
END;
$$ LANGUAGE plpgsql;

-- Wrapper for set_scope_cbu that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_cbu_with_history(
    p_session_id UUID,
    p_cbu_id UUID
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-cbu');
    RETURN "ob-poc".set_scope_cbu(p_session_id, p_cbu_id);
END;
$$ LANGUAGE plpgsql;

-- Wrapper for set_scope_jurisdiction that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_jurisdiction_with_history(
    p_session_id UUID,
    p_jurisdiction_code VARCHAR(10)
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-jurisdiction');
    RETURN "ob-poc".set_scope_jurisdiction(p_session_id, p_jurisdiction_code);
END;
$$ LANGUAGE plpgsql;

-- Wrapper for set_scope_book that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_book_with_history(
    p_session_id UUID,
    p_apex_entity_id UUID,
    p_filters JSONB DEFAULT '{}'
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-book');
    RETURN "ob-poc".set_scope_book(p_session_id, p_apex_entity_id, p_filters);
END;
$$ LANGUAGE plpgsql;

-- Wrapper for set_scope_neighborhood that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_neighborhood_with_history(
    p_session_id UUID,
    p_entity_id UUID,
    p_hops INTEGER DEFAULT 2
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-neighborhood');
    RETURN "ob-poc".set_scope_neighborhood(p_session_id, p_entity_id, p_hops);
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- DONE
-- =============================================================================
-- Migration 020: Trading Profile Materialization Audit Trail
-- ============================================================================
-- Tracks materialization events: when trading profile documents are projected
-- to operational tables (universe, SSIs, booking rules, ISDA/CSA).
-- ============================================================================

-- Materialization audit log
-- Records each time trading-profile:materialize is executed
CREATE TABLE IF NOT EXISTS "ob-poc".trading_profile_materializations (
    materialization_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id),

    -- What was materialized
    sections_materialized TEXT[] NOT NULL DEFAULT '{}',

    -- Record counts by table
    records_created JSONB NOT NULL DEFAULT '{}',
    records_updated JSONB NOT NULL DEFAULT '{}',
    records_deleted JSONB NOT NULL DEFAULT '{}',

    -- Performance tracking
    duration_ms INTEGER NOT NULL DEFAULT 0,

    -- Audit fields
    materialized_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    materialized_by TEXT,

    -- Error tracking (null = success)
    error_message TEXT,

    -- Session context (if materialized within a session)
    session_id UUID
);

-- Index for querying materializations by profile
CREATE INDEX IF NOT EXISTS idx_materializations_profile_id
    ON "ob-poc".trading_profile_materializations(profile_id);

-- Index for time-based queries
CREATE INDEX IF NOT EXISTS idx_materializations_at
    ON "ob-poc".trading_profile_materializations(materialized_at DESC);

-- Comment
COMMENT ON TABLE "ob-poc".trading_profile_materializations IS
    'Audit trail for trading-profile:materialize operations - tracks when documents are projected to operational tables';
-- ============================================================================
-- Migration 021: Corporate Actions Schema
-- ============================================================================
-- Part of trading matrix pivot (032-corporate-actions-integration.md)
-- Creates tables for CA preferences, instruction windows, and SSI mappings

-- Reference catalog: CA event types (global)
CREATE TABLE IF NOT EXISTS custody.ca_event_types (
    event_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_code TEXT NOT NULL UNIQUE,
    event_name TEXT NOT NULL,
    category TEXT NOT NULL CHECK (category IN (
        'INCOME', 'REORGANIZATION', 'VOLUNTARY', 'MANDATORY', 'INFORMATION'
    )),
    is_elective BOOLEAN NOT NULL DEFAULT false,
    default_election TEXT CHECK (default_election IN (
        'CASH', 'STOCK', 'ROLLOVER', 'LAPSE', 'DECLINE', 'NO_ACTION'
    )),
    iso_event_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- CBU-specific CA preferences
CREATE TABLE IF NOT EXISTS custody.cbu_ca_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    processing_mode TEXT NOT NULL CHECK (processing_mode IN (
        'AUTO_INSTRUCT', 'MANUAL', 'DEFAULT_ONLY', 'THRESHOLD'
    )),
    default_election TEXT,
    threshold_value NUMERIC(18,4),
    threshold_currency TEXT,
    notification_email TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, instrument_class_id)
);

-- Instruction windows (deadline rules)
CREATE TABLE IF NOT EXISTS custody.cbu_ca_instruction_windows (
    window_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID REFERENCES custody.ca_event_types(event_type_id),
    market_id UUID REFERENCES custody.markets(market_id),
    cutoff_days_before INTEGER NOT NULL,
    warning_days INTEGER DEFAULT 3,
    escalation_days INTEGER DEFAULT 1,
    escalation_contact TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, market_id)
);

-- CA proceeds SSI mapping
CREATE TABLE IF NOT EXISTS custody.cbu_ca_ssi_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID REFERENCES custody.ca_event_types(event_type_id),
    currency TEXT NOT NULL,
    proceeds_type TEXT NOT NULL CHECK (proceeds_type IN ('CASH', 'STOCK')),
    ssi_id UUID NOT NULL REFERENCES custody.cbu_ssi(ssi_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, currency, proceeds_type)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_ca_preferences_cbu ON custody.cbu_ca_preferences(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ca_windows_cbu ON custody.cbu_ca_instruction_windows(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ca_ssi_cbu ON custody.cbu_ca_ssi_mappings(cbu_id);

-- ============================================================================
-- ISO 15022 Corporate Action Event Types (CAEV)
-- Complete reference catalog per SWIFT/DTCC/SMPG standards (53 codes)
-- ============================================================================

-- INCOME EVENTS (8)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('DVCA', 'Cash Dividend', 'INCOME', false, 'CASH', 'DVCA'),
    ('DVSE', 'Stock Dividend', 'INCOME', false, 'STOCK', 'DVSE'),
    ('DVOP', 'Dividend Option', 'INCOME', true, 'CASH', 'DVOP'),
    ('INTR', 'Interest Payment', 'INCOME', false, 'CASH', 'INTR'),
    ('CAPD', 'Capital Distribution', 'INCOME', false, 'CASH', 'CAPD'),
    ('CAPG', 'Capital Gains Distribution', 'INCOME', false, 'CASH', 'CAPG'),
    ('DRIP', 'Dividend Reinvestment Plan', 'INCOME', true, 'STOCK', 'DRIP'),
    ('PINK', 'Interest Payment in Kind', 'INCOME', false, 'STOCK', 'PINK')
ON CONFLICT (event_code) DO NOTHING;

-- REORGANIZATION EVENTS (11)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('MRGR', 'Merger', 'REORGANIZATION', false, NULL, 'MRGR'),
    ('SPLF', 'Stock Split (Forward)', 'REORGANIZATION', false, NULL, 'SPLF'),
    ('SPLR', 'Reverse Stock Split', 'REORGANIZATION', false, NULL, 'SPLR'),
    ('BONU', 'Bonus Issue/Capitalisation Issue', 'REORGANIZATION', false, 'STOCK', 'BONU'),
    ('EXOF', 'Exchange Offer', 'REORGANIZATION', true, 'DECLINE', 'EXOF'),
    ('CONS', 'Consent', 'REORGANIZATION', true, NULL, 'CONS'),
    ('CONV', 'Conversion', 'REORGANIZATION', true, 'STOCK', 'CONV'),
    ('PARI', 'Pari-Passu', 'REORGANIZATION', false, NULL, 'PARI'),
    ('REDO', 'Redenomination', 'REORGANIZATION', false, NULL, 'REDO'),
    ('DECR', 'Decrease in Value', 'REORGANIZATION', false, NULL, 'DECR'),
    ('SOFF', 'Spin-Off', 'REORGANIZATION', false, 'STOCK', 'SOFF')
ON CONFLICT (event_code) DO NOTHING;

-- VOLUNTARY EVENTS (7)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('RHTS', 'Rights Issue', 'VOLUNTARY', true, 'LAPSE', 'RHTS'),
    ('RHDI', 'Rights Distribution', 'VOLUNTARY', false, NULL, 'RHDI'),
    ('TEND', 'Tender/Takeover Offer', 'VOLUNTARY', true, 'DECLINE', 'TEND'),
    ('BIDS', 'Repurchase Offer/Issuer Bid', 'VOLUNTARY', true, 'CASH', 'BIDS'),
    ('BPUT', 'Put Redemption', 'VOLUNTARY', true, 'CASH', 'BPUT'),
    ('EXWA', 'Exercise of Warrants', 'VOLUNTARY', true, NULL, 'EXWA'),
    ('NOOF', 'Non-Official Offer', 'VOLUNTARY', true, 'DECLINE', 'NOOF')
ON CONFLICT (event_code) DO NOTHING;

-- REDEMPTION/MANDATORY EVENTS (6)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('REDM', 'Final Maturity/Redemption', 'MANDATORY', false, 'CASH', 'REDM'),
    ('MCAL', 'Full Call/Early Redemption', 'MANDATORY', false, 'CASH', 'MCAL'),
    ('PCAL', 'Partial Redemption (Nominal Reduction)', 'MANDATORY', false, 'CASH', 'PCAL'),
    ('PRED', 'Partial Redemption (No Nominal Change)', 'MANDATORY', false, 'CASH', 'PRED'),
    ('DRAW', 'Drawing', 'MANDATORY', false, 'CASH', 'DRAW'),
    ('PDEF', 'Prerefunding', 'MANDATORY', false, NULL, 'PDEF')
ON CONFLICT (event_code) DO NOTHING;

-- MEETINGS & INFORMATION EVENTS (6)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('OMET', 'Ordinary General Meeting', 'INFORMATION', false, NULL, 'OMET'),
    ('XMET', 'Extraordinary General Meeting', 'INFORMATION', false, NULL, 'XMET'),
    ('BMET', 'Bondholder Meeting', 'INFORMATION', false, NULL, 'BMET'),
    ('CMET', 'Court Meeting', 'INFORMATION', false, NULL, 'CMET'),
    ('INFO', 'Information Only', 'INFORMATION', false, NULL, 'INFO'),
    ('DSCL', 'Disclosure', 'INFORMATION', false, NULL, 'DSCL')
ON CONFLICT (event_code) DO NOTHING;

-- CREDIT/DEFAULT EVENTS (4)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('DFLT', 'Bond Default', 'MANDATORY', false, NULL, 'DFLT'),
    ('CREV', 'Credit Event', 'MANDATORY', false, NULL, 'CREV'),
    ('BRUP', 'Bankruptcy', 'MANDATORY', false, NULL, 'BRUP'),
    ('LIQU', 'Liquidation', 'MANDATORY', false, 'CASH', 'LIQU')
ON CONFLICT (event_code) DO NOTHING;

-- OTHER EVENTS (11)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('ATTI', 'Attachment', 'MANDATORY', false, NULL, 'ATTI'),
    ('CERT', 'Certification of Beneficial Ownership', 'VOLUNTARY', true, NULL, 'CERT'),
    ('CHAN', 'Change (Name/Domicile/etc)', 'MANDATORY', false, NULL, 'CHAN'),
    ('DETI', 'Detachment of Warrants', 'MANDATORY', false, NULL, 'DETI'),
    ('DRCA', 'Non-Eligible Securities Cash Distribution', 'MANDATORY', false, 'CASH', 'DRCA'),
    ('PPMT', 'Installment Call', 'MANDATORY', false, 'CASH', 'PPMT'),
    ('REMK', 'Remarketing Agreement', 'VOLUNTARY', true, NULL, 'REMK'),
    ('TREC', 'Tax Reclaim', 'VOLUNTARY', true, 'CASH', 'TREC'),
    ('WTRC', 'Withholding Tax Relief Certification', 'VOLUNTARY', true, NULL, 'WTRC'),
    ('ACCU', 'Accumulation', 'MANDATORY', false, NULL, 'ACCU'),
    ('CAPI', 'Capitalisation', 'MANDATORY', false, NULL, 'CAPI'),
    ('OTHR', 'Other (Unclassified)', 'INFORMATION', false, NULL, 'OTHR')
ON CONFLICT (event_code) DO NOTHING;

COMMENT ON TABLE custody.ca_event_types IS 'Reference catalog of corporate action event types';
COMMENT ON TABLE custody.cbu_ca_preferences IS 'CBU-specific CA processing preferences (written by materialize)';
COMMENT ON TABLE custody.cbu_ca_instruction_windows IS 'CBU deadline/cutoff rules for CA instructions';
COMMENT ON TABLE custody.cbu_ca_ssi_mappings IS 'Which SSI receives CA proceeds (cash/stock) per currency';
-- Migration: 022_control_edges.sql
-- Purpose: Control edges with BODS/GLEIF/PSC standards alignment + derived board controller
--
-- Key concepts:
-- 1. control_edges: Ownership/voting/board control edges with standard xrefs
-- 2. cbu_board_controller: Materialized derived edge (computed, not hand-authored)
-- 3. board_control_evidence: Audit trail of evidence used in derivation
--
-- Standards alignment:
-- - BODS: https://standard.openownership.org/en/0.3.0/schema/reference.html#interest
-- - GLEIF RR: https://www.gleif.org/en/about-lei/common-data-file-format
-- - UK PSC: https://www.gov.uk/guidance/people-with-significant-control-pscs

SET search_path TO "ob-poc", public;

-- ============================================================================
-- CONTROL EDGES TABLE
-- Stores ownership/voting/control relationships with standard cross-references
-- ============================================================================

CREATE TABLE IF NOT EXISTS control_edges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    to_entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,

    -- Our canonical edge type (aligned to BODS interest types where applicable)
    edge_type TEXT NOT NULL,

    -- Standard cross-references
    bods_interest_type TEXT,           -- e.g., 'shareholding', 'voting-rights', 'appointment-of-board'
    gleif_relationship_type TEXT,      -- e.g., 'IS_DIRECTLY_CONSOLIDATED_BY', 'IS_FUND_MANAGED_BY'
    psc_category TEXT,                 -- e.g., 'ownership-of-shares-25-to-50', 'appoints-majority-of-board'

    -- Quantitative
    percentage DECIMAL(5,2),           -- NULL if boolean/qualitative
    is_direct BOOLEAN DEFAULT true,    -- direct vs indirect holding

    -- Qualitative flags
    is_beneficial BOOLEAN DEFAULT false,  -- beneficial vs legal ownership
    is_legal BOOLEAN DEFAULT true,        -- legal ownership (vs purely economic)

    -- Share class specifics (for voting vs economic split)
    share_class_id UUID,                  -- FK to share_classes if applicable
    votes_per_share DECIMAL(10,4),        -- voting power per unit

    -- Provenance
    source_document_id UUID,              -- FK to documents if we have the source doc
    source_register TEXT,                 -- 'gleif', 'uk-psc', 'lux-rbe', 'sec-13d', 'manual'
    source_reference TEXT,                -- External ID in source register
    effective_date DATE,
    end_date DATE,                        -- NULL = still active

    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    created_by TEXT,

    -- Constraints
    CONSTRAINT valid_edge_type CHECK (edge_type IN (
        -- Ownership/Voting (BODS-aligned)
        'HOLDS_SHARES',
        'HOLDS_VOTING_RIGHTS',

        -- Board control (BODS-aligned)
        'APPOINTS_BOARD',
        'EXERCISES_INFLUENCE',
        'IS_SENIOR_MANAGER',

        -- Trust arrangements (BODS-aligned)
        'IS_SETTLOR',
        'IS_TRUSTEE',
        'IS_PROTECTOR',
        'IS_BENEFICIARY',

        -- Economic rights (BODS-aligned)
        'HAS_DISSOLUTION_RIGHTS',
        'HAS_PROFIT_RIGHTS',

        -- GLEIF hierarchy
        'CONSOLIDATED_BY',
        'ULTIMATELY_CONSOLIDATED_BY',
        'MANAGED_BY',
        'SUBFUND_OF',
        'FEEDS_INTO'
    )),

    CONSTRAINT valid_bods_interest CHECK (bods_interest_type IS NULL OR bods_interest_type IN (
        'shareholding',
        'voting-rights',
        'appointment-of-board',
        'other-influence-or-control',
        'senior-managing-official',
        'settlor-of-trust',
        'trustee-of-trust',
        'protector-of-trust',
        'beneficiary-of-trust',
        'rights-to-surplus-assets-on-dissolution',
        'rights-to-profit-or-income'
    )),

    CONSTRAINT valid_gleif_relationship CHECK (gleif_relationship_type IS NULL OR gleif_relationship_type IN (
        'IS_DIRECTLY_CONSOLIDATED_BY',
        'IS_ULTIMATELY_CONSOLIDATED_BY',
        'IS_FUND_MANAGED_BY',
        'IS_SUBFUND_OF',
        'IS_FEEDER_TO'
    )),

    CONSTRAINT valid_psc_category CHECK (psc_category IS NULL OR psc_category IN (
        'ownership-of-shares-25-to-50',
        'ownership-of-shares-50-to-75',
        'ownership-of-shares-75-to-100',
        'voting-rights-25-to-50',
        'voting-rights-50-to-75',
        'voting-rights-75-to-100',
        'appoints-majority-of-board',
        'significant-influence-or-control'
    )),

    CONSTRAINT valid_percentage CHECK (percentage IS NULL OR (percentage >= 0 AND percentage <= 100))
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_control_edges_from ON control_edges(from_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_edges_to ON control_edges(to_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_edges_type ON control_edges(edge_type);
CREATE INDEX IF NOT EXISTS idx_control_edges_effective ON control_edges(effective_date) WHERE end_date IS NULL;
CREATE INDEX IF NOT EXISTS idx_control_edges_source ON control_edges(source_register);

-- Unique constraint: only one active edge of same type between same entities
CREATE UNIQUE INDEX IF NOT EXISTS idx_control_edges_unique_active
ON control_edges(from_entity_id, to_entity_id, edge_type)
WHERE end_date IS NULL;

-- ============================================================================
-- CBU BOARD CONTROLLER TABLE
-- Materialized derived edge: who controls the board of a CBU's entities
-- This is COMPUTED by the rules engine, not hand-authored
-- ============================================================================

CREATE TABLE IF NOT EXISTS cbu_board_controller (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,

    -- The computed controller (NULL = no single controller identified)
    controller_entity_id UUID REFERENCES entities(entity_id),
    controller_name TEXT,  -- Cached for display

    -- Derivation method (which rule fired)
    method TEXT NOT NULL CHECK (method IN (
        'board_appointment_rights',  -- Rule A: explicit appointment rights
        'voting_rights_majority',    -- Rule B: >50% voting power
        'special_instrument',        -- Rule C: golden share, GP, trustee
        'mixed',                     -- Multiple rules contributed
        'no_single_controller'       -- Rule D: no entity meets threshold
    )),

    -- Confidence in the derivation
    confidence TEXT NOT NULL CHECK (confidence IN ('high', 'medium', 'low')),

    -- Composite score (0.0 - 1.0)
    score DECIMAL(3,2) NOT NULL CHECK (score >= 0 AND score <= 1),

    -- As-of date for the computation
    as_of DATE NOT NULL,

    -- Full explanation payload (JSON)
    -- Contains: candidates, evidence_refs, data_gaps, scoring breakdown
    explanation JSONB NOT NULL DEFAULT '{}',

    -- When this was computed
    computed_at TIMESTAMPTZ DEFAULT NOW(),

    -- Who/what triggered the computation
    computed_by TEXT,  -- 'system', 'user:uuid', 'edge_change:uuid'

    -- One derived controller per CBU
    CONSTRAINT unique_cbu_board_controller UNIQUE(cbu_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_board_controller_controller ON cbu_board_controller(controller_entity_id);
CREATE INDEX IF NOT EXISTS idx_cbu_board_controller_method ON cbu_board_controller(method);
CREATE INDEX IF NOT EXISTS idx_cbu_board_controller_confidence ON cbu_board_controller(confidence);

-- ============================================================================
-- BOARD CONTROL EVIDENCE TABLE
-- Audit trail of evidence used in board control derivation
-- ============================================================================

CREATE TABLE IF NOT EXISTS board_control_evidence (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_board_controller_id UUID NOT NULL REFERENCES cbu_board_controller(id) ON DELETE CASCADE,

    -- Evidence source type
    source_type TEXT NOT NULL CHECK (source_type IN (
        'gleif_rr',           -- GLEIF Relationship Record
        'bods_statement',     -- BODS ownership statement
        'investor_register',  -- Share register entry
        'governance_doc',     -- Articles of association, shareholder agreement
        'special_instrument', -- Golden share, GP/LP agreement, trust deed
        'manual_entry'        -- User-entered override with justification
    )),

    -- Reference to source
    source_id TEXT NOT NULL,          -- Document ID, register entry ID, external ref
    source_register TEXT,             -- 'gleif', 'uk-psc', 'lux-rbe', etc.

    -- What this evidence shows
    description TEXT NOT NULL,

    -- Evidence details (JSON for flexibility)
    details JSONB DEFAULT '{}',

    -- As-of date for this evidence
    as_of DATE,

    -- When captured
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_board_control_evidence_controller ON board_control_evidence(cbu_board_controller_id);
CREATE INDEX IF NOT EXISTS idx_board_control_evidence_source ON board_control_evidence(source_type);

-- ============================================================================
-- CBU CONTROL ANCHORS TABLE
-- Links CBU to entities that bridge into ownership/control graph
-- These are the "portal" entities - clicking navigates to control sphere
-- ============================================================================

CREATE TABLE IF NOT EXISTS cbu_control_anchors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,

    -- Role of this anchor
    anchor_role TEXT NOT NULL CHECK (anchor_role IN (
        'governance',  -- ManCo, board oversight - who controls the board
        'sponsor',     -- Parent group, ultimate controller
        'issuer'       -- Fund legal entity itself (the thing that's owned)
    )),

    -- Cached display info
    display_name TEXT,
    jurisdiction TEXT,

    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Each entity can only have one role per CBU
    CONSTRAINT unique_cbu_anchor_role UNIQUE(cbu_id, entity_id, anchor_role)
);

CREATE INDEX IF NOT EXISTS idx_cbu_control_anchors_cbu ON cbu_control_anchors(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_control_anchors_entity ON cbu_control_anchors(entity_id);

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to auto-set BODS interest type from edge_type
CREATE OR REPLACE FUNCTION set_bods_interest_type()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.bods_interest_type IS NULL THEN
        NEW.bods_interest_type := CASE NEW.edge_type
            WHEN 'HOLDS_SHARES' THEN 'shareholding'
            WHEN 'HOLDS_VOTING_RIGHTS' THEN 'voting-rights'
            WHEN 'APPOINTS_BOARD' THEN 'appointment-of-board'
            WHEN 'EXERCISES_INFLUENCE' THEN 'other-influence-or-control'
            WHEN 'IS_SENIOR_MANAGER' THEN 'senior-managing-official'
            WHEN 'IS_SETTLOR' THEN 'settlor-of-trust'
            WHEN 'IS_TRUSTEE' THEN 'trustee-of-trust'
            WHEN 'IS_PROTECTOR' THEN 'protector-of-trust'
            WHEN 'IS_BENEFICIARY' THEN 'beneficiary-of-trust'
            WHEN 'HAS_DISSOLUTION_RIGHTS' THEN 'rights-to-surplus-assets-on-dissolution'
            WHEN 'HAS_PROFIT_RIGHTS' THEN 'rights-to-profit-or-income'
            ELSE NULL
        END;
    END IF;

    -- Auto-set GLEIF relationship type
    IF NEW.gleif_relationship_type IS NULL THEN
        NEW.gleif_relationship_type := CASE NEW.edge_type
            WHEN 'CONSOLIDATED_BY' THEN 'IS_DIRECTLY_CONSOLIDATED_BY'
            WHEN 'ULTIMATELY_CONSOLIDATED_BY' THEN 'IS_ULTIMATELY_CONSOLIDATED_BY'
            WHEN 'MANAGED_BY' THEN 'IS_FUND_MANAGED_BY'
            WHEN 'SUBFUND_OF' THEN 'IS_SUBFUND_OF'
            WHEN 'FEEDS_INTO' THEN 'IS_FEEDER_TO'
            ELSE NULL
        END;
    END IF;

    -- Auto-set PSC category based on edge type + percentage
    IF NEW.psc_category IS NULL AND NEW.percentage IS NOT NULL THEN
        NEW.psc_category := CASE
            WHEN NEW.edge_type = 'HOLDS_SHARES' AND NEW.percentage > 75 THEN 'ownership-of-shares-75-to-100'
            WHEN NEW.edge_type = 'HOLDS_SHARES' AND NEW.percentage > 50 THEN 'ownership-of-shares-50-to-75'
            WHEN NEW.edge_type = 'HOLDS_SHARES' AND NEW.percentage > 25 THEN 'ownership-of-shares-25-to-50'
            WHEN NEW.edge_type = 'HOLDS_VOTING_RIGHTS' AND NEW.percentage > 75 THEN 'voting-rights-75-to-100'
            WHEN NEW.edge_type = 'HOLDS_VOTING_RIGHTS' AND NEW.percentage > 50 THEN 'voting-rights-50-to-75'
            WHEN NEW.edge_type = 'HOLDS_VOTING_RIGHTS' AND NEW.percentage > 25 THEN 'voting-rights-25-to-50'
            WHEN NEW.edge_type = 'APPOINTS_BOARD' AND NEW.percentage > 50 THEN 'appoints-majority-of-board'
            WHEN NEW.edge_type = 'EXERCISES_INFLUENCE' THEN 'significant-influence-or-control'
            ELSE NULL
        END;
    END IF;

    NEW.updated_at := NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_control_edges_set_standards ON control_edges;
CREATE TRIGGER trg_control_edges_set_standards
    BEFORE INSERT OR UPDATE ON control_edges
    FOR EACH ROW
    EXECUTE FUNCTION set_bods_interest_type();

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TABLE control_edges IS 'Ownership/voting/control edges with BODS/GLEIF/PSC standards alignment';
COMMENT ON TABLE cbu_board_controller IS 'Materialized derived edge: computed board controller per CBU (not hand-authored)';
COMMENT ON TABLE board_control_evidence IS 'Audit trail of evidence used in board control derivation';
COMMENT ON TABLE cbu_control_anchors IS 'Portal entities linking CBU to ownership/control graph';

COMMENT ON COLUMN control_edges.edge_type IS 'Canonical edge type aligned to BODS interest types';
COMMENT ON COLUMN control_edges.bods_interest_type IS 'BODS interest type (auto-set from edge_type)';
COMMENT ON COLUMN control_edges.gleif_relationship_type IS 'GLEIF RR type for legal entity hierarchy';
COMMENT ON COLUMN control_edges.psc_category IS 'UK PSC category (auto-set from edge_type + percentage)';

COMMENT ON COLUMN cbu_board_controller.method IS 'Which derivation rule fired: A=appointment, B=voting, C=special, D=none';
COMMENT ON COLUMN cbu_board_controller.explanation IS 'Full derivation audit trail: candidates, evidence, data gaps, scoring';
-- Migration: 023_sessions_persistence.sql
-- Purpose: Simplified session persistence for CBU session state
--
-- Design: Memory is truth, DB is backup
-- - All mutations happen in-memory, instant
-- - DB saves are fire-and-forget background tasks
-- - Load from DB only at startup, with timeout fallback
-- - If DB fails, session just won't survive refresh

-- =============================================================================
-- SESSIONS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Optional user association (NULL for anonymous sessions)
    user_id UUID,

    -- Optional friendly name
    name TEXT,

    -- Core state: the set of loaded CBU IDs
    cbu_ids UUID[] NOT NULL DEFAULT '{}',

    -- Undo stack: array of previous states (each state is array of UUIDs)
    -- Stored as JSONB for flexible serialization
    history JSONB NOT NULL DEFAULT '[]',

    -- Redo stack: array of future states (cleared on new action)
    future JSONB NOT NULL DEFAULT '[]',

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Auto-expiry: sessions expire after 7 days of inactivity
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '7 days'
);

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Find sessions by user
CREATE INDEX IF NOT EXISTS idx_sessions_user
    ON "ob-poc".sessions(user_id)
    WHERE user_id IS NOT NULL;

-- Cleanup expired sessions
CREATE INDEX IF NOT EXISTS idx_sessions_expires
    ON "ob-poc".sessions(expires_at);

-- Find recent sessions
CREATE INDEX IF NOT EXISTS idx_sessions_updated
    ON "ob-poc".sessions(updated_at DESC);

-- =============================================================================
-- AUTO-EXTEND EXPIRY ON ACTIVITY
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".extend_session_expiry()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.expires_at = NOW() + INTERVAL '7 days';
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS session_activity ON "ob-poc".sessions;

CREATE TRIGGER session_activity
    BEFORE UPDATE ON "ob-poc".sessions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".extend_session_expiry();

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE "ob-poc".sessions IS
'Simplified CBU session persistence. Memory is truth, DB is backup.';

COMMENT ON COLUMN "ob-poc".sessions.cbu_ids IS
'Set of CBU IDs currently loaded in this session';

COMMENT ON COLUMN "ob-poc".sessions.history IS
'Undo stack: JSON array of previous states, each state is array of UUID strings';

COMMENT ON COLUMN "ob-poc".sessions.future IS
'Redo stack: JSON array of future states, cleared on new action';

COMMENT ON COLUMN "ob-poc".sessions.expires_at IS
'Session expires 7 days after last activity. Auto-extended on update.';
-- Migration 024: Service Intents + SRDEF Enhancement
--
-- Adds:
-- 1. service_intents table - captures what CBU wants (product + service + options)
-- 2. Enhances service_resource_types with SRDEF identity columns
-- 3. Discovery reason tracking for audit trail
--
-- Part of CBU Resource Pipeline implementation

-- =============================================================================
-- 1. SERVICE INTENTS TABLE
-- =============================================================================
-- Captures a CBU's subscription to a product/service combination with options.
-- This is the INPUT to the resource discovery engine.

CREATE TABLE IF NOT EXISTS "ob-poc".service_intents (
    intent_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id),

    -- Service configuration options (markets, SSI mode, channels, etc.)
    options JSONB NOT NULL DEFAULT '{}',

    -- Lifecycle
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'suspended', 'cancelled')),

    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by TEXT,

    -- One intent per CBU/product/service combination
    UNIQUE(cbu_id, product_id, service_id)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_service_intents_cbu
    ON "ob-poc".service_intents(cbu_id);
CREATE INDEX IF NOT EXISTS idx_service_intents_status
    ON "ob-poc".service_intents(cbu_id, status) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_service_intents_product
    ON "ob-poc".service_intents(product_id);

-- Updated_at trigger
CREATE OR REPLACE FUNCTION "ob-poc".update_service_intents_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_service_intents_updated ON "ob-poc".service_intents;
CREATE TRIGGER trg_service_intents_updated
    BEFORE UPDATE ON "ob-poc".service_intents
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_service_intents_timestamp();

COMMENT ON TABLE "ob-poc".service_intents IS
    'CBU subscription to product/service combinations. Input to resource discovery.';
COMMENT ON COLUMN "ob-poc".service_intents.options IS
    'Service configuration: markets, SSI mode, channels, counterparties, etc.';

-- =============================================================================
-- 2. SRDEF IDENTITY ON SERVICE_RESOURCE_TYPES
-- =============================================================================
-- Add SRDEF (ServiceResourceDefinition) identity columns to existing table.
-- SRDEF ID format: SRDEF::<APP>::<Kind>::<Purpose>

-- Add srdef_id as computed column for canonical identity
ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS srdef_id TEXT GENERATED ALWAYS AS (
    'SRDEF::' ||
    COALESCE(owner, 'UNKNOWN') || '::' ||
    COALESCE(resource_type, 'Resource') || '::' ||
    COALESCE(resource_code, resource_id::text)
) STORED;

-- Provisioning strategy: how to obtain this resource
ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS provisioning_strategy TEXT DEFAULT 'create'
    CHECK (provisioning_strategy IN ('create', 'request', 'discover'));

-- Resource purpose: what this resource is for (more semantic than resource_type)
ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS resource_purpose TEXT;

-- Index on srdef_id for fast lookups
CREATE INDEX IF NOT EXISTS idx_service_resource_types_srdef
    ON "ob-poc".service_resource_types(srdef_id);

COMMENT ON COLUMN "ob-poc".service_resource_types.srdef_id IS
    'Canonical SRDEF identity: SRDEF::<APP>::<Kind>::<Purpose>';
COMMENT ON COLUMN "ob-poc".service_resource_types.provisioning_strategy IS
    'How to obtain: create (we create it), request (ask owner), discover (find existing)';
COMMENT ON COLUMN "ob-poc".service_resource_types.resource_purpose IS
    'Semantic purpose: custody_securities, swift_messaging, iam_access, etc.';

-- =============================================================================
-- 3. DISCOVERY REASONS TABLE
-- =============================================================================
-- Tracks WHY a particular SRDEF was discovered for a CBU.
-- This is the OUTPUT of the resource discovery engine, providing audit trail.

CREATE TABLE IF NOT EXISTS "ob-poc".srdef_discovery_reasons (
    discovery_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    srdef_id TEXT NOT NULL,
    resource_type_id UUID REFERENCES "ob-poc".service_resource_types(resource_id),

    -- Which intent(s) triggered this discovery
    triggered_by_intents JSONB NOT NULL DEFAULT '[]',  -- array of intent_ids

    -- Discovery reasoning
    discovery_rule TEXT NOT NULL,  -- rule name that matched
    discovery_reason JSONB NOT NULL DEFAULT '{}',  -- detailed explanation

    -- For parameterized resources (per-market, per-currency, etc.)
    parameters JSONB DEFAULT '{}',

    -- Lifecycle
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    superseded_at TIMESTAMPTZ  -- set when re-discovery replaces this
);

-- Partial unique index for active discoveries (only one active per CBU/SRDEF/params)
CREATE UNIQUE INDEX IF NOT EXISTS idx_srdef_discovery_active
    ON "ob-poc".srdef_discovery_reasons(cbu_id, srdef_id, parameters)
    WHERE superseded_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_srdef_discovery_cbu
    ON "ob-poc".srdef_discovery_reasons(cbu_id) WHERE superseded_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_srdef_discovery_srdef
    ON "ob-poc".srdef_discovery_reasons(srdef_id);

COMMENT ON TABLE "ob-poc".srdef_discovery_reasons IS
    'Audit trail: why each SRDEF was discovered for a CBU. Output of discovery engine.';
COMMENT ON COLUMN "ob-poc".srdef_discovery_reasons.parameters IS
    'For parameterized resources: {market_id: ..., currency: ...}';

-- =============================================================================
-- 4. ENHANCE RESOURCE_ATTRIBUTE_REQUIREMENTS
-- =============================================================================
-- Add source policy and constraint columns for SRDEF attribute profiles

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS requirement_type TEXT DEFAULT 'required'
    CHECK (requirement_type IN ('required', 'optional', 'conditional'));

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS source_policy JSONB DEFAULT '["derived", "entity", "cbu", "document", "manual"]';

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS constraints JSONB DEFAULT '{}';

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS evidence_policy JSONB DEFAULT '{}';

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS condition_expression TEXT;

COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.requirement_type IS
    'required=must have, optional=nice to have, conditional=depends on condition_expression';
COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.source_policy IS
    'Ordered list of acceptable sources for this attribute value';
COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.constraints IS
    'Type/range/regex/enum constraints for validation';
COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.evidence_policy IS
    'What evidence is required: {requires_document: true, min_confidence: 0.9}';

-- =============================================================================
-- 5. SERVICE INTENT OPTIONS SCHEMA (for reference)
-- =============================================================================
-- Document expected structure of service_intents.options JSONB

COMMENT ON COLUMN "ob-poc".service_intents.options IS
$comment$
Service configuration options. Expected structure varies by service:

Custody/Settlement:
{
  "markets": ["XNAS", "XNYS", "XLON"],
  "currencies": ["USD", "GBP", "EUR"],
  "ssi_mode": "standing" | "per_trade",
  "counterparties": ["uuid1", "uuid2"]
}

Trading:
{
  "instrument_classes": ["equity", "fixed_income"],
  "execution_venues": ["XNAS", "XNYS"],
  "order_types": ["market", "limit"]
}

Reporting:
{
  "report_types": ["position", "transaction", "valuation"],
  "frequency": "daily" | "weekly" | "monthly",
  "format": "pdf" | "csv" | "xml"
}
$comment$;

-- =============================================================================
-- 6. VIEW: Active Service Intents with Product/Service Names
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_service_intents_active AS
SELECT
    si.intent_id,
    si.cbu_id,
    c.name AS cbu_name,
    si.product_id,
    p.name AS product_name,
    p.product_code,
    si.service_id,
    s.name AS service_name,
    s.service_code,
    si.options,
    si.status,
    si.created_at,
    si.updated_at
FROM "ob-poc".service_intents si
JOIN "ob-poc".cbus c ON c.cbu_id = si.cbu_id
JOIN "ob-poc".products p ON p.product_id = si.product_id
JOIN "ob-poc".services s ON s.service_id = si.service_id
WHERE si.status = 'active';

COMMENT ON VIEW "ob-poc".v_service_intents_active IS
    'Active service intents with resolved product/service names';
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
-- Migration 026: Provisioning Ledger (Append-Only)
--
-- Adds:
-- 1. provisioning_requests - append-only log of provisioning requests
-- 2. provisioning_events - append-only log of events from owner systems
-- 3. Enhances cbu_resource_instances with owner response columns
--
-- These tables are APPEND-ONLY for audit compliance.
-- Updates/deletes are prevented by triggers.
--
-- Part of CBU Resource Pipeline implementation

-- =============================================================================
-- 1. PROVISIONING REQUESTS (Append-Only)
-- =============================================================================
-- One row per provisioning request to an owner system.
-- Status can be updated via a separate status tracking mechanism.

CREATE TABLE IF NOT EXISTS "ob-poc".provisioning_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What we're provisioning for
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    srdef_id TEXT NOT NULL,
    instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),

    -- Who requested it
    requested_by TEXT NOT NULL DEFAULT 'system'
        CHECK (requested_by IN ('agent', 'user', 'system')),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Full request payload (attrs snapshot, bind_to, evidence)
    request_payload JSONB NOT NULL,

    -- Status tracking (updated via events, not direct UPDATE)
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'sent', 'ack', 'completed', 'failed', 'cancelled')),

    -- Owner system info
    owner_system TEXT NOT NULL,  -- app mnemonic (CUSTODY, SWIFT, IAM, etc.)
    owner_ticket_id TEXT,  -- external ticket/reference number

    -- For parameterized resources
    parameters JSONB DEFAULT '{}'
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_provisioning_requests_cbu
    ON "ob-poc".provisioning_requests(cbu_id);
CREATE INDEX IF NOT EXISTS idx_provisioning_requests_status
    ON "ob-poc".provisioning_requests(status) WHERE status IN ('queued', 'sent', 'ack');
CREATE INDEX IF NOT EXISTS idx_provisioning_requests_srdef
    ON "ob-poc".provisioning_requests(srdef_id);
CREATE INDEX IF NOT EXISTS idx_provisioning_requests_instance
    ON "ob-poc".provisioning_requests(instance_id) WHERE instance_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".provisioning_requests IS
    'Append-only log of provisioning requests to owner systems.';
COMMENT ON COLUMN "ob-poc".provisioning_requests.request_payload IS
    'Full request snapshot: {attrs: {...}, bind_to: {...}, evidence_refs: [...]}';
COMMENT ON COLUMN "ob-poc".provisioning_requests.owner_system IS
    'Owner app mnemonic: CUSTODY, SWIFT, IAM, TRADING, etc.';

-- =============================================================================
-- 2. PROVISIONING EVENTS (Append-Only)
-- =============================================================================
-- Log of all events related to provisioning requests.
-- Includes outbound (REQUEST_SENT) and inbound (ACK, RESULT, ERROR) events.

CREATE TABLE IF NOT EXISTS "ob-poc".provisioning_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES "ob-poc".provisioning_requests(request_id),

    -- When and what direction
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    direction TEXT NOT NULL CHECK (direction IN ('OUT', 'IN')),

    -- Event type
    kind TEXT NOT NULL
        CHECK (kind IN ('REQUEST_SENT', 'ACK', 'RESULT', 'ERROR', 'STATUS', 'RETRY')),

    -- Full event payload
    payload JSONB NOT NULL,

    -- Content hash for deduplication (SHA256 of payload)
    content_hash TEXT
);

-- Unique index on hash for deduplication
CREATE UNIQUE INDEX IF NOT EXISTS idx_provisioning_events_hash
    ON "ob-poc".provisioning_events(content_hash)
    WHERE content_hash IS NOT NULL;

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_provisioning_events_request
    ON "ob-poc".provisioning_events(request_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_provisioning_events_kind
    ON "ob-poc".provisioning_events(kind);

COMMENT ON TABLE "ob-poc".provisioning_events IS
    'Append-only event log for provisioning requests. Supports idempotent webhook processing.';
COMMENT ON COLUMN "ob-poc".provisioning_events.direction IS
    'OUT = we sent to owner, IN = owner sent to us';
COMMENT ON COLUMN "ob-poc".provisioning_events.content_hash IS
    'SHA256 hash of payload for deduplication. Prevents duplicate webhook processing.';

-- =============================================================================
-- 3. APPEND-ONLY ENFORCEMENT
-- =============================================================================
-- Prevent UPDATE and DELETE on append-only tables.

CREATE OR REPLACE FUNCTION "ob-poc".prevent_modify_append_only()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Table % is append-only. UPDATE and DELETE are not allowed.', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

-- Apply to provisioning_requests
DROP TRIGGER IF EXISTS trg_provisioning_requests_immutable ON "ob-poc".provisioning_requests;
CREATE TRIGGER trg_provisioning_requests_immutable
    BEFORE UPDATE OR DELETE ON "ob-poc".provisioning_requests
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".prevent_modify_append_only();

-- Apply to provisioning_events
DROP TRIGGER IF EXISTS trg_provisioning_events_immutable ON "ob-poc".provisioning_events;
CREATE TRIGGER trg_provisioning_events_immutable
    BEFORE UPDATE OR DELETE ON "ob-poc".provisioning_events
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".prevent_modify_append_only();

-- =============================================================================
-- 4. UPDATE provisioning_requests.status VIA EVENTS
-- =============================================================================
-- Since we can't UPDATE directly, we need a function that:
-- 1. Inserts a STATUS event
-- 2. Uses a view or computed column for current status

-- Actually, we'll allow status updates on provisioning_requests since it's
-- tracking workflow state, not audit data. Let's remove the trigger and
-- add a softer constraint.

DROP TRIGGER IF EXISTS trg_provisioning_requests_immutable ON "ob-poc".provisioning_requests;

-- Instead, add an audit column for status changes
ALTER TABLE "ob-poc".provisioning_requests
ADD COLUMN IF NOT EXISTS status_changed_at TIMESTAMPTZ;

-- Trigger to track status changes
CREATE OR REPLACE FUNCTION "ob-poc".track_provisioning_status_change()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status IS DISTINCT FROM NEW.status THEN
        NEW.status_changed_at = NOW();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_provisioning_requests_status ON "ob-poc".provisioning_requests;
CREATE TRIGGER trg_provisioning_requests_status
    BEFORE UPDATE ON "ob-poc".provisioning_requests
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".track_provisioning_status_change();

-- =============================================================================
-- 5. ENHANCE cbu_resource_instances
-- =============================================================================
-- Add columns for owner response data

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS resource_url TEXT;

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS owner_ticket_id TEXT;

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS last_request_id UUID
    REFERENCES "ob-poc".provisioning_requests(request_id);

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS last_event_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS srdef_id TEXT;

COMMENT ON COLUMN "ob-poc".cbu_resource_instances.resource_url IS
    'URL to access this resource in the owner system';
COMMENT ON COLUMN "ob-poc".cbu_resource_instances.owner_ticket_id IS
    'External ticket/reference from owner system';
COMMENT ON COLUMN "ob-poc".cbu_resource_instances.last_request_id IS
    'Most recent provisioning request for this instance';
COMMENT ON COLUMN "ob-poc".cbu_resource_instances.srdef_id IS
    'SRDEF that this instance fulfills';

-- =============================================================================
-- 6. CANONICAL PROVISIONING RESULT PAYLOAD
-- =============================================================================
-- Document the expected structure of RESULT events

COMMENT ON COLUMN "ob-poc".provisioning_events.payload IS
$comment$
Event payload structure varies by kind:

REQUEST_SENT:
{
  "srdef_id": "SRDEF::CUSTODY::Account::custody_securities",
  "attrs": {"market_id": "...", "currency": "USD"},
  "bind_to": {"entity_id": "..."},
  "idempotency_key": "..."
}

ACK:
{
  "owner_ticket_id": "INC12345",
  "estimated_completion": "2024-01-15T10:00:00Z"
}

RESULT (success):
{
  "status": "active",
  "srid": "SR::CUSTODY::Account::ACCT-12345678",
  "native_key": "ACCT-12345678",
  "native_key_type": "AccountNo",
  "resource_url": "https://custody.internal/accounts/ACCT-12345678",
  "owner_ticket_id": "INC12345"
}

RESULT (failure):
{
  "status": "failed",
  "explain": {
    "message": "Account creation rejected: duplicate SSI",
    "codes": ["DUPLICATE_SSI", "VALIDATION_ERROR"]
  }
}

ERROR:
{
  "error_code": "TIMEOUT",
  "message": "Request timed out after 30s",
  "retryable": true
}
$comment$;

-- =============================================================================
-- 7. VIEW: Pending Provisioning Requests
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_provisioning_pending AS
SELECT
    pr.request_id,
    pr.cbu_id,
    c.name AS cbu_name,
    pr.srdef_id,
    pr.status,
    pr.owner_system,
    pr.owner_ticket_id,
    pr.requested_at,
    pr.status_changed_at,
    pr.parameters,
    (SELECT COUNT(*) FROM "ob-poc".provisioning_events pe WHERE pe.request_id = pr.request_id) AS event_count,
    (SELECT MAX(occurred_at) FROM "ob-poc".provisioning_events pe WHERE pe.request_id = pr.request_id) AS last_event_at
FROM "ob-poc".provisioning_requests pr
JOIN "ob-poc".cbus c ON c.cbu_id = pr.cbu_id
WHERE pr.status IN ('queued', 'sent', 'ack')
ORDER BY pr.requested_at ASC;

COMMENT ON VIEW "ob-poc".v_provisioning_pending IS
    'Provisioning requests that are not yet completed or failed.';

-- =============================================================================
-- 8. FUNCTION: Process Provisioning Result (Idempotent)
-- =============================================================================
-- Called by webhook handler. Idempotent via content_hash.

CREATE OR REPLACE FUNCTION "ob-poc".process_provisioning_result(
    p_request_id UUID,
    p_payload JSONB,
    p_content_hash TEXT DEFAULT NULL
) RETURNS TABLE(
    event_id UUID,
    was_duplicate BOOLEAN,
    new_status TEXT
) AS $$
DECLARE
    v_event_id UUID;
    v_existing_event UUID;
    v_new_status TEXT;
    v_instance_id UUID;
    v_result_status TEXT;
BEGIN
    -- Check for duplicate via hash
    IF p_content_hash IS NOT NULL THEN
        SELECT pe.event_id INTO v_existing_event
        FROM "ob-poc".provisioning_events pe
        WHERE pe.content_hash = p_content_hash;

        IF v_existing_event IS NOT NULL THEN
            RETURN QUERY SELECT v_existing_event, TRUE, NULL::TEXT;
            RETURN;
        END IF;
    END IF;

    -- Insert event
    INSERT INTO "ob-poc".provisioning_events (request_id, direction, kind, payload, content_hash)
    VALUES (p_request_id, 'IN', 'RESULT', p_payload, p_content_hash)
    RETURNING provisioning_events.event_id INTO v_event_id;

    -- Extract result status
    v_result_status := p_payload->>'status';

    -- Map to request status
    v_new_status := CASE v_result_status
        WHEN 'active' THEN 'completed'
        WHEN 'pending' THEN 'ack'
        WHEN 'rejected' THEN 'failed'
        WHEN 'failed' THEN 'failed'
        ELSE 'ack'
    END;

    -- Update request status
    UPDATE "ob-poc".provisioning_requests
    SET status = v_new_status,
        owner_ticket_id = COALESCE(p_payload->>'owner_ticket_id', owner_ticket_id)
    WHERE request_id = p_request_id;

    -- If completed, update the resource instance
    IF v_new_status = 'completed' THEN
        SELECT pr.instance_id INTO v_instance_id
        FROM "ob-poc".provisioning_requests pr
        WHERE pr.request_id = p_request_id;

        IF v_instance_id IS NOT NULL THEN
            UPDATE "ob-poc".cbu_resource_instances
            SET status = 'ACTIVE',
                resource_url = p_payload->>'resource_url',
                owner_ticket_id = p_payload->>'owner_ticket_id',
                instance_identifier = COALESCE(p_payload->>'native_key', instance_identifier),
                last_request_id = p_request_id,
                last_event_at = NOW(),
                activated_at = NOW(),
                updated_at = NOW()
            WHERE instance_id = v_instance_id;
        END IF;
    END IF;

    RETURN QUERY SELECT v_event_id, FALSE, v_new_status;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".process_provisioning_result IS
    'Idempotent webhook handler for provisioning results. Uses content_hash for deduplication.';
-- Migration 027: Service Readiness (Derived)
--
-- Adds:
-- 1. cbu_service_readiness - derived table tracking "good to transact" status
-- 2. Views for readiness reporting
-- 3. Functions for readiness computation
--
-- This table is DERIVED and can be rebuilt from:
-- - service_intents
-- - srdef_discovery_reasons
-- - cbu_resource_instances
-- - cbu_unified_attr_requirements / cbu_attr_values
--
-- Part of CBU Resource Pipeline implementation

-- =============================================================================
-- 1. CBU SERVICE READINESS TABLE
-- =============================================================================
-- Materialized readiness status per CBU/product/service combination.
-- Rebuilt by the readiness computation engine.

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_service_readiness (
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id),

    -- Readiness status
    status TEXT NOT NULL DEFAULT 'blocked'
        CHECK (status IN ('ready', 'blocked', 'partial')),

    -- Why is it blocked? Array of blocking reasons
    blocking_reasons JSONB NOT NULL DEFAULT '[]',

    -- What SRDEFs are required for this service?
    required_srdefs JSONB NOT NULL DEFAULT '[]',

    -- What resource instances are active?
    active_srids JSONB NOT NULL DEFAULT '[]',

    -- Computed at
    as_of TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Recomputation tracking
    last_recomputed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    recomputation_trigger TEXT,  -- what triggered recomputation

    PRIMARY KEY (cbu_id, product_id, service_id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cbu_service_readiness_status
    ON "ob-poc".cbu_service_readiness(cbu_id, status);
CREATE INDEX IF NOT EXISTS idx_cbu_service_readiness_blocked
    ON "ob-poc".cbu_service_readiness(cbu_id) WHERE status = 'blocked';
CREATE INDEX IF NOT EXISTS idx_cbu_service_readiness_ready
    ON "ob-poc".cbu_service_readiness(cbu_id) WHERE status = 'ready';

COMMENT ON TABLE "ob-poc".cbu_service_readiness IS
    'Derived readiness status per CBU service. Rebuildable from intents + instances + attrs.';

-- =============================================================================
-- 2. BLOCKING REASON SCHEMA
-- =============================================================================
-- Document the structure of blocking_reasons array

COMMENT ON COLUMN "ob-poc".cbu_service_readiness.blocking_reasons IS
$comment$
Array of blocking reasons. Each element:
{
  "type": "missing_srdef" | "pending_provisioning" | "failed_provisioning" |
          "missing_attrs" | "attr_conflict" | "dependency_not_ready",
  "srdef_id": "SRDEF::...",          -- which SRDEF is affected
  "details": {                        -- type-specific details
    // For missing_attrs:
    "missing_attr_ids": ["uuid1", "uuid2"],
    "missing_attr_names": ["market_scope", "settlement_currency"],

    // For pending_provisioning:
    "request_id": "uuid",
    "status": "queued" | "sent" | "ack",
    "owner_system": "CUSTODY",
    "requested_at": "2024-01-10T...",

    // For failed_provisioning:
    "request_id": "uuid",
    "error_message": "...",
    "error_codes": ["..."],

    // For dependency_not_ready:
    "depends_on_srdef": "SRDEF::...",
    "dependency_status": "blocked"
  },
  "explain": "Human-readable explanation"
}
$comment$;

-- =============================================================================
-- 3. VIEW: Readiness Dashboard
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_service_readiness_dashboard AS
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    r.product_id,
    p.name AS product_name,
    p.product_code,
    r.service_id,
    s.name AS service_name,
    s.service_code,
    r.status,
    jsonb_array_length(r.blocking_reasons) AS blocking_count,
    jsonb_array_length(r.required_srdefs) AS required_srdef_count,
    jsonb_array_length(r.active_srids) AS active_instance_count,
    r.as_of,
    r.last_recomputed_at
FROM "ob-poc".cbu_service_readiness r
JOIN "ob-poc".cbus c ON c.cbu_id = r.cbu_id
JOIN "ob-poc".products p ON p.product_id = r.product_id
JOIN "ob-poc".services s ON s.service_id = r.service_id
ORDER BY
    r.status DESC,  -- blocked first
    c.name, p.name, s.name;

COMMENT ON VIEW "ob-poc".v_service_readiness_dashboard IS
    'Dashboard view of service readiness across all CBUs.';

-- =============================================================================
-- 4. VIEW: CBU Readiness Summary
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cbu_readiness_summary AS
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    COUNT(*) AS total_services,
    COUNT(*) FILTER (WHERE r.status = 'ready') AS ready_count,
    COUNT(*) FILTER (WHERE r.status = 'partial') AS partial_count,
    COUNT(*) FILTER (WHERE r.status = 'blocked') AS blocked_count,
    ROUND(100.0 * COUNT(*) FILTER (WHERE r.status = 'ready') / NULLIF(COUNT(*), 0), 1) AS pct_ready
FROM "ob-poc".cbu_service_readiness r
JOIN "ob-poc".cbus c ON c.cbu_id = r.cbu_id
GROUP BY r.cbu_id, c.name
ORDER BY pct_ready ASC, c.name;

COMMENT ON VIEW "ob-poc".v_cbu_readiness_summary IS
    'Summary of readiness per CBU across all services.';

-- =============================================================================
-- 5. VIEW: Blocking Reasons Expanded
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_blocking_reasons_expanded AS
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    r.product_id,
    p.name AS product_name,
    r.service_id,
    s.name AS service_name,
    br->>'type' AS blocking_type,
    br->>'srdef_id' AS srdef_id,
    br->>'explain' AS explanation,
    br->'details' AS details
FROM "ob-poc".cbu_service_readiness r
JOIN "ob-poc".cbus c ON c.cbu_id = r.cbu_id
JOIN "ob-poc".products p ON p.product_id = r.product_id
JOIN "ob-poc".services s ON s.service_id = r.service_id
CROSS JOIN LATERAL jsonb_array_elements(r.blocking_reasons) AS br
WHERE r.status IN ('blocked', 'partial')
ORDER BY c.name, p.name, s.name;

COMMENT ON VIEW "ob-poc".v_blocking_reasons_expanded IS
    'Flattened view of all blocking reasons for analysis.';

-- =============================================================================
-- 6. FUNCTION: Compute Service Readiness (Stub)
-- =============================================================================
-- Main computation logic lives in Rust. This is a stub for manual invocation.

CREATE OR REPLACE FUNCTION "ob-poc".compute_service_readiness(p_cbu_id UUID)
RETURNS TABLE(
    product_id UUID,
    service_id UUID,
    status TEXT,
    blocking_count INT
) AS $$
BEGIN
    -- Stub implementation
    -- Real logic is in Rust: ReadinessEngine::compute_for_cbu()

    RETURN QUERY
    SELECT
        si.product_id,
        si.service_id,
        'blocked'::TEXT AS status,
        0 AS blocking_count
    FROM "ob-poc".service_intents si
    WHERE si.cbu_id = p_cbu_id
    AND si.status = 'active';
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".compute_service_readiness IS
    'Stub for readiness computation. Actual implementation in Rust ReadinessEngine.';

-- =============================================================================
-- 7. FUNCTION: Check Single SRDEF Readiness
-- =============================================================================
-- Check if a single SRDEF is ready (all attrs satisfied, instance active)

CREATE OR REPLACE FUNCTION "ob-poc".check_srdef_readiness(
    p_cbu_id UUID,
    p_srdef_id TEXT
) RETURNS TABLE(
    is_ready BOOLEAN,
    missing_attrs UUID[],
    instance_status TEXT,
    blocking_reason TEXT
) AS $$
DECLARE
    v_missing UUID[];
    v_instance_status TEXT;
    v_is_ready BOOLEAN := TRUE;
    v_blocking_reason TEXT;
BEGIN
    -- Check for missing required attributes
    SELECT ARRAY_AGG(r.attr_id) INTO v_missing
    FROM "ob-poc".cbu_unified_attr_requirements r
    LEFT JOIN "ob-poc".cbu_attr_values v ON v.cbu_id = r.cbu_id AND v.attr_id = r.attr_id
    WHERE r.cbu_id = p_cbu_id
    AND r.requirement_strength = 'required'
    AND p_srdef_id = ANY(SELECT jsonb_array_elements_text(r.required_by_srdefs))
    AND v.value IS NULL;

    IF v_missing IS NOT NULL AND array_length(v_missing, 1) > 0 THEN
        v_is_ready := FALSE;
        v_blocking_reason := 'Missing required attributes';
    END IF;

    -- Check instance status
    SELECT ri.status INTO v_instance_status
    FROM "ob-poc".cbu_resource_instances ri
    WHERE ri.cbu_id = p_cbu_id
    AND ri.srdef_id = p_srdef_id
    ORDER BY ri.created_at DESC
    LIMIT 1;

    IF v_instance_status IS NULL THEN
        v_is_ready := FALSE;
        v_blocking_reason := COALESCE(v_blocking_reason || '; ', '') || 'No instance provisioned';
    ELSIF v_instance_status != 'ACTIVE' THEN
        v_is_ready := FALSE;
        v_blocking_reason := COALESCE(v_blocking_reason || '; ', '') || 'Instance status: ' || v_instance_status;
    END IF;

    RETURN QUERY SELECT v_is_ready, v_missing, v_instance_status, v_blocking_reason;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".check_srdef_readiness IS
    'Check if a single SRDEF is ready for a CBU. Returns missing attrs and instance status.';

-- =============================================================================
-- 8. TRIGGER: Auto-invalidate readiness on instance change
-- =============================================================================
-- When a resource instance status changes, mark related readiness as stale.

ALTER TABLE "ob-poc".cbu_service_readiness
ADD COLUMN IF NOT EXISTS is_stale BOOLEAN DEFAULT FALSE;

CREATE OR REPLACE FUNCTION "ob-poc".invalidate_readiness_on_instance_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Mark readiness as stale when instance status changes
    IF TG_OP = 'UPDATE' AND OLD.status IS DISTINCT FROM NEW.status THEN
        UPDATE "ob-poc".cbu_service_readiness
        SET is_stale = TRUE
        WHERE cbu_id = NEW.cbu_id;
    ELSIF TG_OP = 'INSERT' THEN
        UPDATE "ob-poc".cbu_service_readiness
        SET is_stale = TRUE
        WHERE cbu_id = NEW.cbu_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_instance_invalidates_readiness ON "ob-poc".cbu_resource_instances;
CREATE TRIGGER trg_instance_invalidates_readiness
    AFTER INSERT OR UPDATE ON "ob-poc".cbu_resource_instances
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".invalidate_readiness_on_instance_change();

COMMENT ON COLUMN "ob-poc".cbu_service_readiness.is_stale IS
    'Set TRUE when underlying data changes. Triggers recomputation.';
-- Migration 028: Investor Role Profiles
--
-- Purpose: Add issuer-scoped holder role metadata to:
-- 1. Prevent pooled vehicles (FoF, master pools, nominees) from being misclassified as UBO
-- 2. Control look-through policy per holder-issuer relationship
-- 3. Support temporal versioning for point-in-time queries
--
-- Design: "Same entity, different treatment" - AllianzLife can be an end-investor in Fund A
-- but a master pool operator for Fund B, with different UBO eligibility and look-through rules.

-- =============================================================================
-- INVESTOR ROLE PROFILES TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.investor_role_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Relationship scope
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    share_class_id UUID NULL REFERENCES kyc.share_classes(id) ON DELETE SET NULL,

    -- Role classification
    role_type VARCHAR(50) NOT NULL,

    -- Look-through policy
    lookthrough_policy VARCHAR(30) NOT NULL DEFAULT 'NONE',

    -- Holder affiliation (intra-group vs external)
    holder_affiliation VARCHAR(20) NOT NULL DEFAULT 'UNKNOWN',

    -- BO data availability flag
    beneficial_owner_data_available BOOLEAN NOT NULL DEFAULT false,

    -- UBO eligibility (false = never create UBO edges for this holder)
    is_ubo_eligible BOOLEAN NOT NULL DEFAULT true,

    -- Optional group container (for intra-group holders)
    group_container_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,
    group_label TEXT NULL,

    -- Temporal versioning (effective_from/effective_to pattern)
    effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_to DATE NULL,  -- NULL = current/active version

    -- Audit
    source VARCHAR(50) DEFAULT 'MANUAL',
    source_reference TEXT NULL,
    notes TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by VARCHAR(100) NULL,

    -- Role type enum
    CONSTRAINT chk_role_type CHECK (role_type IN (
        'END_INVESTOR',         -- Ultimate beneficial owner candidate
        'NOMINEE',              -- Holding on behalf of others
        'OMNIBUS',              -- Omnibus account (multiple underlying)
        'INTERMEDIARY_FOF',     -- Fund-of-funds intermediary
        'MASTER_POOL',          -- Master pooling vehicle
        'INTRA_GROUP_POOL',     -- Intra-group pooling (same corporate group)
        'TREASURY',             -- Group treasury function
        'CUSTODIAN',            -- Custodial holding
        'OTHER'
    )),

    -- Lookthrough policy enum
    CONSTRAINT chk_lookthrough CHECK (lookthrough_policy IN (
        'NONE',                 -- Do not look through (treat as leaf)
        'ON_DEMAND',            -- Look through only when explicitly requested
        'AUTO_IF_DATA',         -- Automatic look-through if BO data available
        'ALWAYS'                -- Always look through regardless of data
    )),

    -- Holder affiliation enum
    CONSTRAINT chk_holder_affiliation CHECK (holder_affiliation IN (
        'INTRA_GROUP',          -- Same corporate group as issuer
        'EXTERNAL',             -- External third-party investor
        'MIXED',                -- Hybrid (both intra-group and external)
        'UNKNOWN'               -- Not yet classified
    ))
);

-- Comments
COMMENT ON TABLE kyc.investor_role_profiles IS
'Issuer-scoped holder role metadata. Controls UBO eligibility and look-through policy per holder-issuer relationship.';

COMMENT ON COLUMN kyc.investor_role_profiles.role_type IS
'END_INVESTOR (UBO candidate), NOMINEE, OMNIBUS, INTERMEDIARY_FOF (fund-of-funds), MASTER_POOL, INTRA_GROUP_POOL, TREASURY, CUSTODIAN, OTHER';

COMMENT ON COLUMN kyc.investor_role_profiles.lookthrough_policy IS
'NONE (treat as leaf), ON_DEMAND (explicit request), AUTO_IF_DATA (if BO data available), ALWAYS (regardless of data)';

COMMENT ON COLUMN kyc.investor_role_profiles.holder_affiliation IS
'INTRA_GROUP (same corporate group), EXTERNAL (third-party), MIXED (hybrid), UNKNOWN';

COMMENT ON COLUMN kyc.investor_role_profiles.is_ubo_eligible IS
'If false, UBO sync trigger will never create ownership edges for this holder, regardless of percentage';

COMMENT ON COLUMN kyc.investor_role_profiles.effective_from IS
'Start date for this role profile version. Enables point-in-time queries for mid-year reclassifications.';

COMMENT ON COLUMN kyc.investor_role_profiles.effective_to IS
'End date for this role profile version. NULL means current/active version.';

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Lookup by issuer (most common query pattern)
CREATE INDEX IF NOT EXISTS idx_role_profiles_issuer
    ON kyc.investor_role_profiles(issuer_entity_id);

-- Lookup by holder
CREATE INDEX IF NOT EXISTS idx_role_profiles_holder
    ON kyc.investor_role_profiles(holder_entity_id);

-- Lookup by group container
CREATE INDEX IF NOT EXISTS idx_role_profiles_group
    ON kyc.investor_role_profiles(group_container_entity_id)
    WHERE group_container_entity_id IS NOT NULL;

-- Fast lookup for current/active profiles
CREATE INDEX IF NOT EXISTS idx_role_profiles_active
    ON kyc.investor_role_profiles(issuer_entity_id, holder_entity_id)
    WHERE effective_to IS NULL;

-- Point-in-time queries
CREATE INDEX IF NOT EXISTS idx_role_profiles_temporal
    ON kyc.investor_role_profiles(issuer_entity_id, holder_entity_id, effective_from, effective_to);

-- Unique constraint: only one active (effective_to IS NULL) profile per issuer+holder+share_class
-- Using partial unique index since PostgreSQL doesn't allow COALESCE in unique constraints
CREATE UNIQUE INDEX IF NOT EXISTS idx_role_profiles_unique_active
    ON kyc.investor_role_profiles(issuer_entity_id, holder_entity_id, COALESCE(share_class_id, '00000000-0000-0000-0000-000000000000'::uuid))
    WHERE effective_to IS NULL;

-- =============================================================================
-- HELPER FUNCTION: Get current role profile
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.get_current_role_profile(
    p_issuer_entity_id UUID,
    p_holder_entity_id UUID,
    p_share_class_id UUID DEFAULT NULL
) RETURNS kyc.investor_role_profiles AS $$
    SELECT *
    FROM kyc.investor_role_profiles
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_to IS NULL
    LIMIT 1;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.get_current_role_profile IS
'Get the current (active) role profile for a holder-issuer relationship';

-- =============================================================================
-- HELPER FUNCTION: Get role profile as of date
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.get_role_profile_as_of(
    p_issuer_entity_id UUID,
    p_holder_entity_id UUID,
    p_as_of_date DATE,
    p_share_class_id UUID DEFAULT NULL
) RETURNS kyc.investor_role_profiles AS $$
    SELECT *
    FROM kyc.investor_role_profiles
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_from <= p_as_of_date
      AND (effective_to IS NULL OR effective_to > p_as_of_date)
    ORDER BY effective_from DESC
    LIMIT 1;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.get_role_profile_as_of IS
'Get the role profile that was active as of a specific date (point-in-time query)';

-- =============================================================================
-- HELPER FUNCTION: Close current profile and create new version
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.upsert_role_profile(
    p_issuer_entity_id UUID,
    p_holder_entity_id UUID,
    p_role_type VARCHAR(50),
    p_lookthrough_policy VARCHAR(30) DEFAULT 'NONE',
    p_holder_affiliation VARCHAR(20) DEFAULT 'UNKNOWN',
    p_beneficial_owner_data_available BOOLEAN DEFAULT false,
    p_is_ubo_eligible BOOLEAN DEFAULT true,
    p_share_class_id UUID DEFAULT NULL,
    p_group_container_entity_id UUID DEFAULT NULL,
    p_group_label TEXT DEFAULT NULL,
    p_effective_from DATE DEFAULT CURRENT_DATE,
    p_source VARCHAR(50) DEFAULT 'MANUAL',
    p_source_reference TEXT DEFAULT NULL,
    p_notes TEXT DEFAULT NULL,
    p_created_by VARCHAR(100) DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_new_id UUID;
BEGIN
    -- Close any existing active profile
    UPDATE kyc.investor_role_profiles
    SET effective_to = p_effective_from,
        updated_at = now()
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_to IS NULL;

    -- Insert new version
    INSERT INTO kyc.investor_role_profiles (
        issuer_entity_id,
        holder_entity_id,
        share_class_id,
        role_type,
        lookthrough_policy,
        holder_affiliation,
        beneficial_owner_data_available,
        is_ubo_eligible,
        group_container_entity_id,
        group_label,
        effective_from,
        source,
        source_reference,
        notes,
        created_by
    ) VALUES (
        p_issuer_entity_id,
        p_holder_entity_id,
        p_share_class_id,
        p_role_type,
        p_lookthrough_policy,
        p_holder_affiliation,
        p_beneficial_owner_data_available,
        p_is_ubo_eligible,
        p_group_container_entity_id,
        p_group_label,
        p_effective_from,
        p_source,
        p_source_reference,
        p_notes,
        p_created_by
    )
    RETURNING id INTO v_new_id;

    RETURN v_new_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.upsert_role_profile IS
'Create or update a role profile with temporal versioning. Closes existing active profile and creates new version.';

-- =============================================================================
-- VIEW: Current role profiles (convenience view)
-- =============================================================================

CREATE OR REPLACE VIEW kyc.v_current_role_profiles AS
SELECT
    rp.*,
    issuer.name AS issuer_name,
    holder.name AS holder_name,
    gc.name AS group_container_name
FROM kyc.investor_role_profiles rp
JOIN "ob-poc".entities issuer ON rp.issuer_entity_id = issuer.entity_id
JOIN "ob-poc".entities holder ON rp.holder_entity_id = holder.entity_id
LEFT JOIN "ob-poc".entities gc ON rp.group_container_entity_id = gc.entity_id
WHERE rp.effective_to IS NULL;

COMMENT ON VIEW kyc.v_current_role_profiles IS
'Current (active) role profiles with entity names resolved';

-- =============================================================================
-- UPDATE TRIGGER for updated_at
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.update_role_profile_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_role_profile_updated ON kyc.investor_role_profiles;
CREATE TRIGGER trg_role_profile_updated
    BEFORE UPDATE ON kyc.investor_role_profiles
    FOR EACH ROW
    EXECUTE FUNCTION kyc.update_role_profile_timestamp();
-- Migration 029: Patch UBO sync trigger to respect usage_type and role profiles
--
-- Fixes: The original trigger in migration 011 creates UBO edges for ALL holdings ≥25%,
-- but pooled vehicles (FoF, master pools, nominees) should NOT create UBO edges.
--
-- This patch adds:
-- 1. Check usage_type = 'UBO' (skip TA holdings)
-- 2. Check investor_role_profiles.is_ubo_eligible (skip ineligible holders)
-- 3. Default-deny for known pooled vehicle role types

-- =============================================================================
-- PATCHED UBO SYNC TRIGGER
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.sync_holding_to_ubo_relationship()
RETURNS TRIGGER AS $$
DECLARE
    v_total_units NUMERIC;
    v_ownership_pct NUMERIC;
    v_fund_entity_id UUID;
    v_is_ubo_eligible BOOLEAN;
    v_role_type VARCHAR(50);
BEGIN
    -- NEW CHECK 1: Only sync UBO holdings, skip TA holdings
    -- TA (Transfer Agency) holdings are for client KYC, not UBO tracking
    IF COALESCE(NEW.usage_type, 'TA') != 'UBO' THEN
        RETURN NEW;
    END IF;

    -- Get total units for percentage calculation
    SELECT COALESCE(SUM(units), 0) INTO v_total_units
    FROM kyc.holdings
    WHERE share_class_id = NEW.share_class_id
      AND COALESCE(holding_status, status) = 'active';

    -- Calculate ownership percentage
    IF v_total_units > 0 THEN
        v_ownership_pct := (NEW.units / v_total_units) * 100;
    ELSE
        v_ownership_pct := 0;
    END IF;

    -- Get fund entity ID from share class
    SELECT entity_id INTO v_fund_entity_id
    FROM kyc.share_classes WHERE id = NEW.share_class_id;

    -- NEW CHECK 2: Check investor role profile for UBO eligibility
    SELECT is_ubo_eligible, role_type
    INTO v_is_ubo_eligible, v_role_type
    FROM kyc.investor_role_profiles
    WHERE holder_entity_id = NEW.investor_entity_id
      AND issuer_entity_id = v_fund_entity_id
      AND effective_to IS NULL  -- Current version only
    LIMIT 1;

    -- If role profile exists and is_ubo_eligible = false, skip
    IF v_is_ubo_eligible = false THEN
        RETURN NEW;
    END IF;

    -- NEW CHECK 3: Default-deny for pooled vehicle role types even without explicit profile
    -- These role types typically should not create UBO edges
    IF v_role_type IN ('NOMINEE', 'OMNIBUS', 'INTERMEDIARY_FOF', 'MASTER_POOL', 'INTRA_GROUP_POOL', 'CUSTODIAN') THEN
        -- Only create UBO edge if explicitly marked as eligible (handled above)
        -- Since we got here, is_ubo_eligible is either NULL or TRUE
        -- For pooled vehicles, require explicit TRUE, not just NULL
        IF v_is_ubo_eligible IS NULL THEN
            RETURN NEW;  -- Skip if no explicit eligibility set for pooled vehicles
        END IF;
    END IF;

    -- Create/update ownership relationship if ≥25% and fund entity exists
    IF v_ownership_pct >= 25 AND v_fund_entity_id IS NOT NULL THEN
        INSERT INTO "ob-poc".entity_relationships (
            from_entity_id, to_entity_id, relationship_type,
            percentage, ownership_type, interest_type, direct_or_indirect,
            effective_from, source, notes
        ) VALUES (
            NEW.investor_entity_id, v_fund_entity_id, 'ownership',
            v_ownership_pct, 'DIRECT', 'shareholding', 'direct',
            COALESCE(NEW.acquisition_date, CURRENT_DATE),
            'INVESTOR_REGISTER',
            'Synced from UBO holding ' || NEW.id::text
        )
        ON CONFLICT (from_entity_id, to_entity_id, relationship_type)
        WHERE effective_to IS NULL
        DO UPDATE SET
            percentage = EXCLUDED.percentage,
            updated_at = NOW(),
            notes = EXCLUDED.notes;
    ELSE
        -- Remove relationship if dropped below 25%
        UPDATE "ob-poc".entity_relationships
        SET effective_to = CURRENT_DATE,
            updated_at = NOW()
        WHERE from_entity_id = NEW.investor_entity_id
          AND to_entity_id = v_fund_entity_id
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
          AND effective_to IS NULL;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Comment explaining the changes
COMMENT ON FUNCTION kyc.sync_holding_to_ubo_relationship() IS
'Sync holdings to UBO ownership edges. PATCHED in migration 029 to:
1. Only sync usage_type=UBO (skip TA holdings)
2. Respect investor_role_profiles.is_ubo_eligible
3. Default-deny for pooled vehicle role types (NOMINEE, FOF, MASTER_POOL, etc.)';

-- =============================================================================
-- NOTES FOR FUTURE REFERENCE
-- =============================================================================
-- The trigger is already attached to kyc.holdings from migration 011:
--   CREATE TRIGGER trg_sync_holding_to_ubo
--   AFTER INSERT OR UPDATE OF units, holding_status, status ON kyc.holdings
--   FOR EACH ROW EXECUTE FUNCTION kyc.sync_holding_to_ubo_relationship();
--
-- This migration only updates the function body, not the trigger itself.
-- No need to drop/recreate the trigger.
-- Migration 030: Fund Vehicle Taxonomy
--
-- Purpose: Add explicit metadata for fund structures to support:
-- - FoF/umbrella/master pool representation (Allianz-style structures)
-- - Compartment/sleeve modeling without inventing fake legal entities
-- - Instrument type classification for holdings
--
-- This enables proper representation of multi-tier fund structures without
-- misclassifying pooled vehicles as UBOs.

-- =============================================================================
-- FUND VEHICLES TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.fund_vehicles (
    fund_entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- Vehicle classification
    vehicle_type VARCHAR(30) NOT NULL,

    -- Umbrella relationship (NULL if standalone fund)
    umbrella_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,
    is_umbrella BOOLEAN NOT NULL DEFAULT false,

    -- Domicile and management
    domicile_country CHAR(2) NULL,
    manager_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,

    -- Flexible metadata
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by VARCHAR(100) NULL,

    -- Vehicle type enum (expanded for broader use)
    CONSTRAINT chk_vehicle_type CHECK (vehicle_type IN (
        'SCSP',           -- Luxembourg SCSp (Société en Commandite Spéciale)
        'SICAV_RAIF',     -- Luxembourg SICAV-RAIF (Reserved Alternative Investment Fund)
        'SICAV_SIF',      -- Luxembourg SICAV-SIF (Specialized Investment Fund)
        'SIF',            -- Luxembourg SIF (standalone)
        'SICAV_UCITS',    -- UCITS umbrella fund
        'FCP',            -- Fonds Commun de Placement
        'LLC',            -- US LLC
        'LP',             -- Limited Partnership (generic)
        'TRUST',          -- Unit trust structure
        'OEIC',           -- UK Open-Ended Investment Company
        'ETF',            -- Exchange-traded fund
        'REIT',           -- Real Estate Investment Trust
        'BDC',            -- Business Development Company
        'OTHER'
    ))
);

COMMENT ON TABLE kyc.fund_vehicles IS
'Fund vehicle metadata for fund structures (FoF/umbrella/master pool). Links to entities table.';

COMMENT ON COLUMN kyc.fund_vehicles.vehicle_type IS
'Luxembourg: SCSP, SICAV_RAIF, SICAV_SIF, SIF, FCP. Generic: LP, LLC, TRUST, OEIC, ETF, REIT, BDC, OTHER';

COMMENT ON COLUMN kyc.fund_vehicles.umbrella_entity_id IS
'Parent umbrella fund (if this is a sub-fund/compartment). NULL for standalone funds.';

COMMENT ON COLUMN kyc.fund_vehicles.is_umbrella IS
'True if this fund is an umbrella containing compartments/sub-funds';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_fund_vehicles_umbrella
    ON kyc.fund_vehicles(umbrella_entity_id) WHERE umbrella_entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_fund_vehicles_manager
    ON kyc.fund_vehicles(manager_entity_id) WHERE manager_entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_fund_vehicles_type
    ON kyc.fund_vehicles(vehicle_type);

-- =============================================================================
-- FUND COMPARTMENTS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.fund_compartments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Parent umbrella fund
    umbrella_fund_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- Compartment identification
    compartment_code TEXT NOT NULL,
    compartment_name TEXT NULL,

    -- Optional: link to entity if compartment has separate legal identity
    compartment_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,

    -- Flexible metadata
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Unique compartment per umbrella
    CONSTRAINT uq_compartment UNIQUE (umbrella_fund_entity_id, compartment_code)
);

COMMENT ON TABLE kyc.fund_compartments IS
'Compartments/sleeves under umbrella funds. May or may not have separate legal identity.';

COMMENT ON COLUMN kyc.fund_compartments.compartment_entity_id IS
'Optional link to entity if compartment has separate legal identity (e.g., separate LEI)';

-- Index
CREATE INDEX IF NOT EXISTS idx_fund_compartments_umbrella
    ON kyc.fund_compartments(umbrella_fund_entity_id);

-- =============================================================================
-- EXTEND SHARE_CLASSES WITH INSTRUMENT_TYPE
-- =============================================================================

ALTER TABLE kyc.share_classes
ADD COLUMN IF NOT EXISTS instrument_type VARCHAR(30) DEFAULT 'SHARES';

COMMENT ON COLUMN kyc.share_classes.instrument_type IS
'UNITS, SHARES, LP_INTEREST, PARTNERSHIP_INTEREST, NOMINEE_POSITION, TRACKING_SHARES, CARRIED_INTEREST';

-- Add compartment link to share classes
ALTER TABLE kyc.share_classes
ADD COLUMN IF NOT EXISTS compartment_id UUID NULL REFERENCES kyc.fund_compartments(id) ON DELETE SET NULL;

COMMENT ON COLUMN kyc.share_classes.compartment_id IS
'Optional link to fund compartment (for umbrella funds with compartment-specific share classes)';

-- Index
CREATE INDEX IF NOT EXISTS idx_share_classes_compartment
    ON kyc.share_classes(compartment_id) WHERE compartment_id IS NOT NULL;

-- =============================================================================
-- VIEW: Fund vehicle summary
-- =============================================================================

CREATE OR REPLACE VIEW kyc.v_fund_vehicle_summary AS
SELECT
    fv.fund_entity_id,
    e.name AS fund_name,
    fv.vehicle_type,
    fv.is_umbrella,
    fv.domicile_country,
    umbrella.name AS umbrella_name,
    manager.name AS manager_name,
    (SELECT COUNT(*) FROM kyc.fund_compartments fc WHERE fc.umbrella_fund_entity_id = fv.fund_entity_id) AS compartment_count,
    (SELECT COUNT(*) FROM kyc.share_classes sc WHERE sc.entity_id = fv.fund_entity_id) AS share_class_count,
    fv.meta,
    fv.created_at
FROM kyc.fund_vehicles fv
JOIN "ob-poc".entities e ON fv.fund_entity_id = e.entity_id
LEFT JOIN "ob-poc".entities umbrella ON fv.umbrella_entity_id = umbrella.entity_id
LEFT JOIN "ob-poc".entities manager ON fv.manager_entity_id = manager.entity_id;

COMMENT ON VIEW kyc.v_fund_vehicle_summary IS
'Fund vehicles with resolved entity names and aggregate counts';

-- =============================================================================
-- UPDATE TRIGGER for updated_at
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.update_fund_vehicle_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_fund_vehicle_updated ON kyc.fund_vehicles;
CREATE TRIGGER trg_fund_vehicle_updated
    BEFORE UPDATE ON kyc.fund_vehicles
    FOR EACH ROW
    EXECUTE FUNCTION kyc.update_fund_vehicle_timestamp();

DROP TRIGGER IF EXISTS trg_fund_compartment_updated ON kyc.fund_compartments;
CREATE TRIGGER trg_fund_compartment_updated
    BEFORE UPDATE ON kyc.fund_compartments
    FOR EACH ROW
    EXECUTE FUNCTION kyc.update_fund_vehicle_timestamp();
-- Migration 031: Economic Look-Through Function
--
-- Purpose: Bounded recursive computation of economic exposure through fund chains.
-- Features:
-- - Configurable depth limit (default 6)
-- - Minimum percentage threshold (stops when ownership drops below threshold)
-- - Cycle detection (prevents infinite loops in malformed data)
-- - Respects role profiles (stops at END_INVESTOR, looks through INTERMEDIARY_FOF, etc.)
-- - Explicit stop condition precedence for debugging
--
-- Design rationale: Store direct edges only, compute look-through on-demand.
-- Materializing implied edges is unmaintainable at scale (1K investors × 200 SPVs = explosion).

-- =============================================================================
-- VIEW: Direct economic edges (single-hop ownership for recursive base case)
-- =============================================================================

CREATE OR REPLACE VIEW kyc.v_economic_edges_direct AS
SELECT
    os.owner_entity_id AS from_entity_id,
    os.issuer_entity_id AS to_entity_id,
    os.percentage AS pct_of_to,
    COALESCE(sc.instrument_type, 'SHARES') AS instrument_type,
    os.share_class_id,
    fv.vehicle_type,
    os.basis,
    'OWNERSHIP_SNAPSHOT' AS source,
    os.as_of_date
FROM kyc.ownership_snapshots os
LEFT JOIN kyc.share_classes sc ON os.share_class_id = sc.id
LEFT JOIN kyc.fund_vehicles fv ON os.issuer_entity_id = fv.fund_entity_id
WHERE os.basis = 'ECONOMIC'
  AND os.is_direct = true
  AND os.superseded_at IS NULL;

COMMENT ON VIEW kyc.v_economic_edges_direct IS
'Direct economic ownership edges (single-hop). Base case for recursive look-through.';

-- =============================================================================
-- FUNCTION: Bounded economic look-through with cycle detection
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_compute_economic_exposure(
    p_root_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE,
    p_max_depth INT DEFAULT 6,
    p_min_pct NUMERIC DEFAULT 0.0001,
    p_max_rows INT DEFAULT 200,
    -- Explicit stop condition config
    p_stop_on_no_bo_data BOOLEAN DEFAULT true,
    p_stop_on_policy_none BOOLEAN DEFAULT true
) RETURNS TABLE (
    root_entity_id UUID,
    leaf_entity_id UUID,
    leaf_name TEXT,
    cumulative_pct NUMERIC,
    depth INT,
    path_entities UUID[],
    path_names TEXT[],
    stopped_reason TEXT  -- Why traversal stopped at this leaf
) AS $$
WITH RECURSIVE exposure_tree AS (
    -- Base case: direct holdings from root
    SELECT
        p_root_entity_id AS root_id,
        e.to_entity_id AS current_id,
        ent.name AS current_name,
        e.pct_of_to::NUMERIC AS cumulative_pct,  -- Cast to generic NUMERIC for recursive union
        1 AS depth,
        ARRAY[p_root_entity_id, e.to_entity_id] AS path,
        ARRAY[
            (SELECT name FROM "ob-poc".entities WHERE entity_id = p_root_entity_id)::TEXT,
            ent.name::TEXT
        ]::TEXT[] AS path_names,
        CASE
            WHEN rp.lookthrough_policy = 'NONE' AND p_stop_on_policy_none THEN 'POLICY_NONE'
            WHEN rp.beneficial_owner_data_available = false AND p_stop_on_no_bo_data THEN 'NO_BO_DATA'
            WHEN rp.role_type = 'END_INVESTOR' THEN 'END_INVESTOR'
            ELSE NULL
        END AS stop_reason
    FROM kyc.v_economic_edges_direct e
    JOIN "ob-poc".entities ent ON e.to_entity_id = ent.entity_id
    LEFT JOIN kyc.investor_role_profiles rp
        ON rp.holder_entity_id = e.from_entity_id
        AND rp.issuer_entity_id = e.to_entity_id
        AND rp.effective_to IS NULL
    WHERE e.from_entity_id = p_root_entity_id
      AND e.as_of_date <= p_as_of_date

    UNION ALL

    -- Recursive case: traverse deeper
    SELECT
        t.root_id,
        e.to_entity_id,
        ent.name,
        (t.cumulative_pct * (e.pct_of_to::NUMERIC / 100.0))::NUMERIC,
        t.depth + 1,
        t.path || e.to_entity_id,
        t.path_names || ent.name::TEXT,
        CASE
            -- STOP CONDITION PRECEDENCE:
            -- 1. Cycle detection (highest priority - prevents infinite loops)
            WHEN e.to_entity_id = ANY(t.path) THEN 'CYCLE_DETECTED'
            -- 2. Depth limit
            WHEN t.depth + 1 >= p_max_depth THEN 'MAX_DEPTH'
            -- 3. Percentage threshold
            WHEN (t.cumulative_pct * (e.pct_of_to::NUMERIC / 100.0)) < p_min_pct THEN 'BELOW_MIN_PCT'
            -- 4. End investor role
            WHEN rp.role_type = 'END_INVESTOR' THEN 'END_INVESTOR'
            -- 5. Lookthrough policy
            WHEN rp.lookthrough_policy = 'NONE' AND p_stop_on_policy_none THEN 'POLICY_NONE'
            -- 6. BO data availability
            WHEN rp.beneficial_owner_data_available = false AND p_stop_on_no_bo_data THEN 'NO_BO_DATA'
            ELSE NULL
        END AS stop_reason
    FROM exposure_tree t
    JOIN kyc.v_economic_edges_direct e ON e.from_entity_id = t.current_id
    JOIN "ob-poc".entities ent ON e.to_entity_id = ent.entity_id
    LEFT JOIN kyc.investor_role_profiles rp
        ON rp.holder_entity_id = e.from_entity_id
        AND rp.issuer_entity_id = e.to_entity_id
        AND rp.effective_to IS NULL
    WHERE t.stop_reason IS NULL  -- Only continue if not stopped
      AND t.depth < p_max_depth
      AND t.cumulative_pct >= p_min_pct
      AND e.as_of_date <= p_as_of_date
      -- CYCLE DETECTION: prevent visiting same node twice in path
      AND NOT (e.to_entity_id = ANY(t.path))
)
SELECT
    root_id AS root_entity_id,
    current_id AS leaf_entity_id,
    current_name AS leaf_name,
    cumulative_pct,
    depth,
    path AS path_entities,
    path_names,
    COALESCE(stop_reason, 'LEAF_NODE') AS stopped_reason
FROM exposure_tree
WHERE stop_reason IS NOT NULL  -- Only return nodes where traversal stopped
   OR NOT EXISTS (  -- Or true leaf nodes with no further edges
        SELECT 1 FROM kyc.v_economic_edges_direct e2
        WHERE e2.from_entity_id = exposure_tree.current_id
          AND e2.as_of_date <= p_as_of_date
   )
ORDER BY cumulative_pct DESC
LIMIT p_max_rows;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.fn_compute_economic_exposure IS
'Bounded look-through computation for economic exposure.

Parameters:
- p_root_entity_id: Starting entity (typically a fund)
- p_as_of_date: Point-in-time query date
- p_max_depth: Maximum traversal depth (default 6)
- p_min_pct: Stop when cumulative ownership drops below this (default 0.01%)
- p_max_rows: Limit result set size (default 200)
- p_stop_on_no_bo_data: Stop at holders without BO data
- p_stop_on_policy_none: Stop at holders with NONE lookthrough policy

Stop condition precedence (debugging aid):
1. CYCLE_DETECTED - prevents infinite loops in malformed data
2. MAX_DEPTH - hard depth limit
3. BELOW_MIN_PCT - percentage threshold
4. END_INVESTOR - role profile marks as end of chain
5. POLICY_NONE - role profile says no lookthrough
6. NO_BO_DATA - beneficial owner data unavailable
7. LEAF_NODE - no further edges to traverse';

-- =============================================================================
-- FUNCTION: Get economic exposure summary for an issuer (aggregated view)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_economic_exposure_summary(
    p_issuer_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE,
    p_threshold_pct NUMERIC DEFAULT 5.0
) RETURNS TABLE (
    investor_entity_id UUID,
    investor_name TEXT,
    direct_pct NUMERIC,
    lookthrough_pct NUMERIC,
    is_above_threshold BOOLEAN,
    role_type VARCHAR(50),
    depth INT,
    stop_reason TEXT
) AS $$
WITH direct_holdings AS (
    -- Direct ownership from ownership_snapshots
    SELECT
        os.owner_entity_id,
        e.name AS owner_name,
        os.percentage AS direct_pct,
        rp.role_type
    FROM kyc.ownership_snapshots os
    JOIN "ob-poc".entities e ON os.owner_entity_id = e.entity_id
    LEFT JOIN kyc.investor_role_profiles rp
        ON rp.holder_entity_id = os.owner_entity_id
        AND rp.issuer_entity_id = os.issuer_entity_id
        AND rp.effective_to IS NULL
    WHERE os.issuer_entity_id = p_issuer_entity_id
      AND os.basis = 'ECONOMIC'
      AND os.is_direct = true
      AND os.as_of_date <= p_as_of_date
      AND os.superseded_at IS NULL
),
lookthrough AS (
    -- Look-through from each direct holder
    SELECT
        dh.owner_entity_id AS top_holder_id,
        lt.leaf_entity_id,
        lt.leaf_name,
        lt.cumulative_pct * (dh.direct_pct / 100.0) AS effective_pct,
        lt.depth,
        lt.stopped_reason
    FROM direct_holdings dh
    CROSS JOIN LATERAL kyc.fn_compute_economic_exposure(
        dh.owner_entity_id,
        p_as_of_date,
        6,     -- max_depth
        0.01,  -- min_pct (0.01%)
        50     -- max_rows per holder
    ) lt
    WHERE dh.role_type IN ('INTERMEDIARY_FOF', 'MASTER_POOL', 'OMNIBUS', 'NOMINEE')
       OR dh.role_type IS NULL  -- Default: try look-through
)
SELECT
    COALESCE(lt.leaf_entity_id, dh.owner_entity_id) AS investor_entity_id,
    COALESCE(lt.leaf_name, dh.owner_name) AS investor_name,
    dh.direct_pct,
    COALESCE(lt.effective_pct, dh.direct_pct) AS lookthrough_pct,
    COALESCE(lt.effective_pct, dh.direct_pct) >= p_threshold_pct AS is_above_threshold,
    dh.role_type,
    COALESCE(lt.depth, 0) AS depth,
    lt.stopped_reason AS stop_reason
FROM direct_holdings dh
LEFT JOIN lookthrough lt ON lt.top_holder_id = dh.owner_entity_id
ORDER BY COALESCE(lt.effective_pct, dh.direct_pct) DESC;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.fn_economic_exposure_summary IS
'Aggregated economic exposure view for an issuer. Shows both direct and look-through percentages.
Use p_threshold_pct to filter for significant holders (default 5%).';

-- =============================================================================
-- VIEW: Issuer control thresholds configuration
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.issuer_control_config (
    issuer_entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    disclosure_threshold_pct NUMERIC NOT NULL DEFAULT 5.0,
    material_threshold_pct NUMERIC NOT NULL DEFAULT 10.0,
    significant_threshold_pct NUMERIC NOT NULL DEFAULT 25.0,
    control_threshold_pct NUMERIC NOT NULL DEFAULT 50.0,
    lookthrough_depth_limit INT NOT NULL DEFAULT 6,
    lookthrough_min_pct NUMERIC NOT NULL DEFAULT 0.01,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE kyc.issuer_control_config IS
'Per-issuer configuration for control thresholds and look-through parameters.
Defaults match common regulatory requirements (5% disclosure, 25% significant, 50% control).';

-- Trigger for updated_at
CREATE OR REPLACE FUNCTION kyc.update_issuer_control_config_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_issuer_control_config_updated ON kyc.issuer_control_config;
CREATE TRIGGER trg_issuer_control_config_updated
    BEFORE UPDATE ON kyc.issuer_control_config
    FOR EACH ROW
    EXECUTE FUNCTION kyc.update_issuer_control_config_timestamp();
-- Agent Learning Infrastructure
-- Enables continuous improvement from user interactions

-- Schema for agent learning
CREATE SCHEMA IF NOT EXISTS agent;

-- =============================================================================
-- LEARNED ENTITY ALIASES
-- =============================================================================
-- When users refer to entities by non-canonical names, learn the mapping
-- e.g., "Barclays" → "Barclays PLC", "DB" → "Deutsche Bank AG"

CREATE TABLE agent.entity_aliases (
    id              BIGSERIAL PRIMARY KEY,
    alias           TEXT NOT NULL,                    -- User's term ("Barclays")
    canonical_name  TEXT NOT NULL,                    -- System name ("Barclays PLC")
    entity_id       UUID REFERENCES "ob-poc".entities(entity_id),
    confidence      DECIMAL(3,2) DEFAULT 1.0,         -- 0.00-1.00
    occurrence_count INT DEFAULT 1,                   -- Times seen
    source          TEXT DEFAULT 'user_correction',  -- user_correction, threshold_auto, manual
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(alias, canonical_name)
);

CREATE INDEX idx_entity_aliases_alias ON agent.entity_aliases(LOWER(alias));
CREATE INDEX idx_entity_aliases_entity ON agent.entity_aliases(entity_id);

-- =============================================================================
-- LEARNED LEXICON TOKENS
-- =============================================================================
-- New vocabulary learned from user inputs
-- e.g., "counterparty" → EntityType, "ISDA" → ProductType

CREATE TABLE agent.lexicon_tokens (
    id              BIGSERIAL PRIMARY KEY,
    token           TEXT NOT NULL,                    -- The word/phrase
    token_type      TEXT NOT NULL,                    -- Verb, Entity, Prep, etc.
    token_subtype   TEXT,                             -- More specific classification
    occurrence_count INT DEFAULT 1,
    confidence      DECIMAL(3,2) DEFAULT 1.0,
    source          TEXT DEFAULT 'user_correction',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(token, token_type)
);

CREATE INDEX idx_lexicon_tokens_token ON agent.lexicon_tokens(LOWER(token));

-- =============================================================================
-- LEARNED INVOCATION PHRASES
-- =============================================================================
-- Natural language phrases that map to DSL verbs
-- e.g., "set up an ISDA" → isda.create

CREATE TABLE agent.invocation_phrases (
    id              BIGSERIAL PRIMARY KEY,
    phrase          TEXT NOT NULL,                    -- User's phrase
    verb            TEXT NOT NULL,                    -- DSL verb (domain.verb)
    confidence      DECIMAL(3,2) DEFAULT 1.0,
    occurrence_count INT DEFAULT 1,
    source          TEXT DEFAULT 'user_correction',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(phrase, verb)
);

CREATE INDEX idx_invocation_phrases_phrase ON agent.invocation_phrases
    USING gin(to_tsvector('english', phrase));

-- =============================================================================
-- AGENT EVENTS (for analysis, not hot path)
-- =============================================================================
-- Captures intent resolution flow for learning analysis

CREATE TABLE agent.events (
    id              BIGSERIAL PRIMARY KEY,
    session_id      UUID,
    timestamp       TIMESTAMPTZ DEFAULT NOW(),
    event_type      TEXT NOT NULL,                    -- prompt_sent, intent_extracted, etc.

    -- Event-specific payload
    user_message    TEXT,                             -- Original user input
    parsed_intents  JSONB,                            -- Extracted intents
    selected_verb   TEXT,                             -- Chosen DSL verb
    generated_dsl   TEXT,                             -- DSL output

    -- Correction tracking
    was_corrected   BOOLEAN DEFAULT FALSE,
    corrected_dsl   TEXT,                             -- User's correction
    correction_type TEXT,                             -- verb_change, entity_change, arg_change

    -- Resolution details
    entities_resolved JSONB,                          -- Entity resolution attempts
    resolution_failures JSONB,                        -- What failed to resolve

    -- Outcome
    execution_success BOOLEAN,
    error_message   TEXT,

    -- Metadata
    duration_ms     INT,
    llm_model       TEXT,
    llm_tokens_used INT
);

CREATE INDEX idx_agent_events_session ON agent.events(session_id);
CREATE INDEX idx_agent_events_timestamp ON agent.events(timestamp);
CREATE INDEX idx_agent_events_type ON agent.events(event_type);
CREATE INDEX idx_agent_events_corrected ON agent.events(was_corrected) WHERE was_corrected = TRUE;
CREATE INDEX idx_agent_events_verb ON agent.events(selected_verb);

-- =============================================================================
-- LEARNING CANDIDATES (queued for review or auto-apply)
-- =============================================================================

CREATE TABLE agent.learning_candidates (
    id              BIGSERIAL PRIMARY KEY,
    fingerprint     TEXT NOT NULL UNIQUE,             -- Dedup key
    learning_type   TEXT NOT NULL,                    -- entity_alias, lexicon_token, invocation_phrase, prompt_change

    -- What to learn
    input_pattern   TEXT NOT NULL,                    -- What user said
    suggested_output TEXT NOT NULL,                   -- What we should map to

    -- Evidence
    occurrence_count INT DEFAULT 1,
    first_seen      TIMESTAMPTZ DEFAULT NOW(),
    last_seen       TIMESTAMPTZ DEFAULT NOW(),
    example_events  BIGINT[],                         -- References to agent.events

    -- Risk assessment
    risk_level      TEXT DEFAULT 'low',               -- low, medium, high
    auto_applicable BOOLEAN DEFAULT FALSE,

    -- Status
    status          TEXT DEFAULT 'pending',           -- pending, approved, rejected, applied
    reviewed_by     TEXT,
    reviewed_at     TIMESTAMPTZ,
    applied_at      TIMESTAMPTZ,

    -- Metadata
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_learning_candidates_status ON agent.learning_candidates(status);
CREATE INDEX idx_learning_candidates_type ON agent.learning_candidates(learning_type);
CREATE INDEX idx_learning_candidates_auto ON agent.learning_candidates(auto_applicable)
    WHERE auto_applicable = TRUE AND status = 'pending';

-- =============================================================================
-- LEARNING AUDIT LOG
-- =============================================================================

CREATE TABLE agent.learning_audit (
    id              BIGSERIAL PRIMARY KEY,
    timestamp       TIMESTAMPTZ DEFAULT NOW(),
    action          TEXT NOT NULL,                    -- applied, rejected, reverted
    learning_type   TEXT NOT NULL,
    learning_id     BIGINT,                           -- Reference to applied learning
    candidate_id    BIGINT REFERENCES agent.learning_candidates(id),
    actor           TEXT NOT NULL,                    -- system_auto, system_threshold, user:xxx
    details         JSONB,

    -- For rollback
    previous_state  JSONB,
    can_rollback    BOOLEAN DEFAULT TRUE
);

CREATE INDEX idx_learning_audit_timestamp ON agent.learning_audit(timestamp);
CREATE INDEX idx_learning_audit_type ON agent.learning_audit(learning_type);

-- =============================================================================
-- HELPER FUNCTIONS
-- =============================================================================

-- Increment occurrence count or insert new alias
CREATE OR REPLACE FUNCTION agent.upsert_entity_alias(
    p_alias TEXT,
    p_canonical_name TEXT,
    p_entity_id UUID DEFAULT NULL,
    p_source TEXT DEFAULT 'user_correction'
) RETURNS BIGINT AS $$
DECLARE
    v_id BIGINT;
BEGIN
    INSERT INTO agent.entity_aliases (alias, canonical_name, entity_id, source)
    VALUES (LOWER(TRIM(p_alias)), p_canonical_name, p_entity_id, p_source)
    ON CONFLICT (alias, canonical_name) DO UPDATE SET
        occurrence_count = agent.entity_aliases.occurrence_count + 1,
        updated_at = NOW()
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Increment occurrence count or insert new lexicon token
CREATE OR REPLACE FUNCTION agent.upsert_lexicon_token(
    p_token TEXT,
    p_token_type TEXT,
    p_token_subtype TEXT DEFAULT NULL,
    p_source TEXT DEFAULT 'user_correction'
) RETURNS BIGINT AS $$
DECLARE
    v_id BIGINT;
BEGIN
    INSERT INTO agent.lexicon_tokens (token, token_type, token_subtype, source)
    VALUES (LOWER(TRIM(p_token)), p_token_type, p_token_subtype, p_source)
    ON CONFLICT (token, token_type) DO UPDATE SET
        occurrence_count = agent.lexicon_tokens.occurrence_count + 1,
        updated_at = NOW()
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Get learning candidates ready for auto-apply (3+ occurrences, low risk)
CREATE OR REPLACE FUNCTION agent.get_auto_applicable_candidates()
RETURNS TABLE (
    id BIGINT,
    learning_type TEXT,
    input_pattern TEXT,
    suggested_output TEXT,
    occurrence_count INT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        lc.id,
        lc.learning_type,
        lc.input_pattern,
        lc.suggested_output,
        lc.occurrence_count
    FROM agent.learning_candidates lc
    WHERE lc.status = 'pending'
      AND lc.auto_applicable = TRUE
      AND lc.occurrence_count >= 3
      AND lc.risk_level = 'low'
    ORDER BY lc.occurrence_count DESC;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE agent.entity_aliases IS 'Learned mappings from user terms to canonical entity names';
COMMENT ON TABLE agent.lexicon_tokens IS 'Vocabulary learned from user inputs for intent parsing';
COMMENT ON TABLE agent.invocation_phrases IS 'Natural language phrases mapped to DSL verbs';
COMMENT ON TABLE agent.events IS 'Agent interaction events for learning analysis';
COMMENT ON TABLE agent.learning_candidates IS 'Pending learnings awaiting approval or auto-apply';
COMMENT ON TABLE agent.learning_audit IS 'Audit trail of all learning applications and reversions';

COMMENT ON SCHEMA agent IS 'Continuous learning infrastructure for agent intent resolution';
-- ESPER Learned Aliases
-- Stores user-learned phrase→command mappings for navigation commands.
-- Integrates with the existing agent learning infrastructure.

CREATE TABLE IF NOT EXISTS agent.esper_aliases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The phrase that triggered this learning (normalized to lowercase)
    phrase TEXT NOT NULL,

    -- The command key this phrase maps to (e.g., "zoom_in", "scale_universe")
    command_key TEXT NOT NULL,

    -- How many times this phrase→command mapping was observed
    occurrence_count INT NOT NULL DEFAULT 1,

    -- Confidence score (0.00-1.00), increases with occurrences
    confidence DECIMAL(3,2) NOT NULL DEFAULT 0.50,

    -- Whether this alias has been auto-approved (after 3x threshold)
    auto_approved BOOLEAN NOT NULL DEFAULT FALSE,

    -- Source of the learning
    source TEXT NOT NULL DEFAULT 'user_correction',

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Each phrase can only map to one command
    UNIQUE(phrase, command_key)
);

-- Index for fast phrase lookup during warmup
CREATE INDEX IF NOT EXISTS idx_esper_aliases_phrase
    ON agent.esper_aliases(LOWER(phrase));

-- Index for loading approved aliases at startup
CREATE INDEX IF NOT EXISTS idx_esper_aliases_approved
    ON agent.esper_aliases(auto_approved)
    WHERE auto_approved = true;

-- Function to record/update an ESPER alias
CREATE OR REPLACE FUNCTION agent.upsert_esper_alias(
    p_phrase TEXT,
    p_command_key TEXT,
    p_source TEXT DEFAULT 'user_correction'
) RETURNS agent.esper_aliases AS $$
DECLARE
    v_result agent.esper_aliases;
    v_threshold INT := 3;  -- Auto-approve after 3 occurrences
BEGIN
    INSERT INTO agent.esper_aliases (phrase, command_key, source)
    VALUES (LOWER(TRIM(p_phrase)), p_command_key, p_source)
    ON CONFLICT (phrase, command_key) DO UPDATE SET
        occurrence_count = agent.esper_aliases.occurrence_count + 1,
        confidence = LEAST(1.0, agent.esper_aliases.confidence + 0.15),
        auto_approved = CASE
            WHEN agent.esper_aliases.occurrence_count + 1 >= v_threshold THEN TRUE
            ELSE agent.esper_aliases.auto_approved
        END,
        updated_at = NOW()
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON TABLE agent.esper_aliases IS
    'Learned phrase→command mappings for ESPER navigation commands';
COMMENT ON COLUMN agent.esper_aliases.phrase IS
    'User phrase (normalized to lowercase)';
COMMENT ON COLUMN agent.esper_aliases.command_key IS
    'ESPER command key from config (e.g., zoom_in, scale_universe)';
COMMENT ON COLUMN agent.esper_aliases.auto_approved IS
    'True if alias passed 3x occurrence threshold';
-- Migration: 033_learning_embeddings.sql
-- Adds pgvector embeddings to learning tables for semantic matching
-- Depends on: 032_agent_learning.sql

-- Ensure pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Add embedding columns to existing learning tables
ALTER TABLE agent.invocation_phrases
ADD COLUMN IF NOT EXISTS embedding vector(1536),
ADD COLUMN IF NOT EXISTS embedding_model TEXT DEFAULT 'text-embedding-3-small';

ALTER TABLE agent.entity_aliases
ADD COLUMN IF NOT EXISTS embedding vector(1536),
ADD COLUMN IF NOT EXISTS embedding_model TEXT DEFAULT 'text-embedding-3-small';

-- Phrase blocklist for negative feedback
CREATE TABLE IF NOT EXISTS agent.phrase_blocklist (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phrase TEXT NOT NULL,
    blocked_verb TEXT NOT NULL,
    embedding vector(1536),
    embedding_model TEXT DEFAULT 'text-embedding-3-small',
    reason TEXT,
    source TEXT DEFAULT 'explicit_feedback',
    user_id UUID,  -- NULL = global, set = user-specific
    created_at TIMESTAMPTZ DEFAULT now(),
    expires_at TIMESTAMPTZ,  -- Optional expiry

    UNIQUE(phrase, blocked_verb, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid))
);

-- User-specific learned phrases (separate from global)
CREATE TABLE IF NOT EXISTS agent.user_learned_phrases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    phrase TEXT NOT NULL,
    verb TEXT NOT NULL,
    embedding vector(1536),
    embedding_model TEXT DEFAULT 'text-embedding-3-small',
    occurrence_count INT DEFAULT 1,
    confidence REAL DEFAULT 1.0,
    source TEXT DEFAULT 'explicit_feedback',
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ,

    UNIQUE(user_id, phrase)
);

-- IVFFlat indexes for similarity search (use after 1000+ rows)
-- For smaller datasets, exact search is faster
CREATE INDEX IF NOT EXISTS idx_invocation_phrases_embedding
ON agent.invocation_phrases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

CREATE INDEX IF NOT EXISTS idx_entity_aliases_embedding
ON agent.entity_aliases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

CREATE INDEX IF NOT EXISTS idx_blocklist_embedding
ON agent.phrase_blocklist
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 50);

CREATE INDEX IF NOT EXISTS idx_user_phrases_embedding
ON agent.user_learned_phrases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Standard indexes
CREATE INDEX IF NOT EXISTS idx_user_phrases_user
ON agent.user_learned_phrases(user_id);

CREATE INDEX IF NOT EXISTS idx_blocklist_verb
ON agent.phrase_blocklist(blocked_verb);

CREATE INDEX IF NOT EXISTS idx_blocklist_user
ON agent.phrase_blocklist(user_id) WHERE user_id IS NOT NULL;

-- Function to search learned phrases by semantic similarity
CREATE OR REPLACE FUNCTION agent.search_learned_phrases_semantic(
    query_embedding vector(1536),
    similarity_threshold REAL DEFAULT 0.80,
    max_results INT DEFAULT 5
)
RETURNS TABLE (
    phrase TEXT,
    verb TEXT,
    similarity REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        ip.phrase,
        ip.verb,
        (1 - (ip.embedding <=> query_embedding))::REAL as similarity
    FROM agent.invocation_phrases ip
    WHERE ip.embedding IS NOT NULL
      AND (1 - (ip.embedding <=> query_embedding)) > similarity_threshold
    ORDER BY ip.embedding <=> query_embedding
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to search user-specific phrases
CREATE OR REPLACE FUNCTION agent.search_user_phrases_semantic(
    p_user_id UUID,
    query_embedding vector(1536),
    similarity_threshold REAL DEFAULT 0.80,
    max_results INT DEFAULT 5
)
RETURNS TABLE (
    phrase TEXT,
    verb TEXT,
    confidence REAL,
    similarity REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        up.phrase,
        up.verb,
        up.confidence,
        (1 - (up.embedding <=> query_embedding))::REAL as similarity
    FROM agent.user_learned_phrases up
    WHERE up.user_id = p_user_id
      AND up.embedding IS NOT NULL
      AND (1 - (up.embedding <=> query_embedding)) > similarity_threshold
    ORDER BY up.embedding <=> query_embedding
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to check blocklist with semantic matching
CREATE OR REPLACE FUNCTION agent.is_phrase_blocked(
    p_verb TEXT,
    query_embedding vector(1536),
    p_user_id UUID DEFAULT NULL,
    similarity_threshold REAL DEFAULT 0.75
)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM agent.phrase_blocklist bl
        WHERE bl.blocked_verb = p_verb
          AND (bl.user_id IS NULL OR bl.user_id = p_user_id)
          AND (bl.expires_at IS NULL OR bl.expires_at > now())
          AND bl.embedding IS NOT NULL
          AND (1 - (bl.embedding <=> query_embedding)) > similarity_threshold
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Add confidence column to learning_candidates if not exists
ALTER TABLE agent.learning_candidates
ADD COLUMN IF NOT EXISTS confidence REAL DEFAULT 1.0;

-- Comment on tables
COMMENT ON TABLE agent.phrase_blocklist IS 'Negative feedback: phrases that should NOT map to specific verbs';
COMMENT ON TABLE agent.user_learned_phrases IS 'User-specific phrase→verb mappings with confidence decay';
COMMENT ON COLUMN agent.phrase_blocklist.embedding IS 'pgvector embedding for semantic blocklist matching';
COMMENT ON COLUMN agent.user_learned_phrases.confidence IS 'Confidence score (0.1-1.0), decays on wrong selection, boosts on correct';
-- Migration: 034_candle_embeddings.sql
-- Migrate from OpenAI 1536-dim to Candle 384-dim embeddings
-- Model: all-MiniLM-L6-v2 (local, no API key required)
--
-- This is a DESTRUCTIVE migration - embeddings must be regenerated.
-- Run backfill_candle_embeddings binary after applying.

-- Step 1: Drop old IVFFlat indexes (they're dimension-specific)
DROP INDEX IF EXISTS agent.idx_invocation_phrases_embedding;
DROP INDEX IF EXISTS agent.idx_entity_aliases_embedding;
DROP INDEX IF EXISTS agent.idx_blocklist_embedding;
DROP INDEX IF EXISTS agent.idx_user_phrases_embedding;

-- Step 2: Drop and recreate embedding columns with new dimension
-- We drop and add to change the vector dimension (ALTER TYPE doesn't work for vectors)

-- invocation_phrases
ALTER TABLE agent.invocation_phrases DROP COLUMN IF EXISTS embedding;
ALTER TABLE agent.invocation_phrases DROP COLUMN IF EXISTS embedding_model;
ALTER TABLE agent.invocation_phrases ADD COLUMN embedding vector(384);
ALTER TABLE agent.invocation_phrases ADD COLUMN embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2';

-- entity_aliases
ALTER TABLE agent.entity_aliases DROP COLUMN IF EXISTS embedding;
ALTER TABLE agent.entity_aliases DROP COLUMN IF EXISTS embedding_model;
ALTER TABLE agent.entity_aliases ADD COLUMN embedding vector(384);
ALTER TABLE agent.entity_aliases ADD COLUMN embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2';

-- phrase_blocklist
ALTER TABLE agent.phrase_blocklist DROP COLUMN IF EXISTS embedding;
ALTER TABLE agent.phrase_blocklist DROP COLUMN IF EXISTS embedding_model;
ALTER TABLE agent.phrase_blocklist ADD COLUMN embedding vector(384);
ALTER TABLE agent.phrase_blocklist ADD COLUMN embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2';

-- user_learned_phrases
ALTER TABLE agent.user_learned_phrases DROP COLUMN IF EXISTS embedding;
ALTER TABLE agent.user_learned_phrases DROP COLUMN IF EXISTS embedding_model;
ALTER TABLE agent.user_learned_phrases ADD COLUMN embedding vector(384);
ALTER TABLE agent.user_learned_phrases ADD COLUMN embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2';

-- Step 3: Recreate IVFFlat indexes for 384-dim vectors
CREATE INDEX idx_invocation_phrases_embedding
ON agent.invocation_phrases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

CREATE INDEX idx_entity_aliases_embedding
ON agent.entity_aliases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

CREATE INDEX idx_blocklist_embedding
ON agent.phrase_blocklist
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 50);

CREATE INDEX idx_user_phrases_embedding
ON agent.user_learned_phrases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Step 4: Update search functions for 384-dim

-- Search learned phrases by semantic similarity
CREATE OR REPLACE FUNCTION agent.search_learned_phrases_semantic(
    query_embedding vector(384),
    similarity_threshold REAL DEFAULT 0.75,
    max_results INT DEFAULT 5
)
RETURNS TABLE (
    phrase TEXT,
    verb TEXT,
    similarity REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        ip.phrase,
        ip.verb,
        (1 - (ip.embedding <=> query_embedding))::REAL as similarity
    FROM agent.invocation_phrases ip
    WHERE ip.embedding IS NOT NULL
      AND (1 - (ip.embedding <=> query_embedding)) > similarity_threshold
    ORDER BY ip.embedding <=> query_embedding
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql;

-- Search user-specific phrases by semantic similarity
CREATE OR REPLACE FUNCTION agent.search_user_phrases_semantic(
    p_user_id UUID,
    query_embedding vector(384),
    similarity_threshold REAL DEFAULT 0.75,
    max_results INT DEFAULT 5
)
RETURNS TABLE (
    phrase TEXT,
    verb TEXT,
    confidence REAL,
    similarity REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        up.phrase,
        up.verb,
        up.confidence,
        (1 - (up.embedding <=> query_embedding))::REAL as similarity
    FROM agent.user_learned_phrases up
    WHERE up.user_id = p_user_id
      AND up.embedding IS NOT NULL
      AND (1 - (up.embedding <=> query_embedding)) > similarity_threshold
    ORDER BY up.embedding <=> query_embedding
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql;

-- Check blocklist by semantic similarity
CREATE OR REPLACE FUNCTION agent.check_blocklist_semantic(
    query_embedding vector(384),
    verb_to_check TEXT,
    p_user_id UUID DEFAULT NULL,
    similarity_threshold REAL DEFAULT 0.85
)
RETURNS BOOLEAN AS $$
DECLARE
    is_blocked BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1
        FROM agent.phrase_blocklist pb
        WHERE pb.blocked_verb = verb_to_check
          AND pb.embedding IS NOT NULL
          AND (pb.user_id IS NULL OR pb.user_id = p_user_id)
          AND (pb.expires_at IS NULL OR pb.expires_at > now())
          AND (1 - (pb.embedding <=> query_embedding)) > similarity_threshold
    ) INTO is_blocked;

    RETURN is_blocked;
END;
$$ LANGUAGE plpgsql;

-- Update semantic_verb_patterns if it exists (voice pipeline)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables
               WHERE table_schema = 'ob-poc'
               AND table_name = 'semantic_verb_patterns') THEN
        -- This table should already use 384-dim from ob-semantic-matcher
        -- Just verify or add comment
        COMMENT ON TABLE "ob-poc".semantic_verb_patterns IS
            'Voice pipeline verb patterns. Embeddings: 384-dim all-MiniLM-L6-v2 (Candle)';
    END IF;
END $$;

-- Add migration metadata comment
COMMENT ON SCHEMA agent IS
    'Agent learning schema. Embeddings: 384-dim all-MiniLM-L6-v2 (Candle, local)';
-- Migration: 035_session_sheet.sql
-- Purpose: Add REPL session state machine and DSL sheet support
-- Depends: 023_sessions_persistence.sql

-- =============================================================================
-- EXTEND SESSIONS TABLE
-- =============================================================================

-- Add columns for REPL state machine
ALTER TABLE "ob-poc".sessions
ADD COLUMN IF NOT EXISTS repl_state TEXT DEFAULT 'empty',
ADD COLUMN IF NOT EXISTS scope_dsl TEXT[] DEFAULT '{}',
ADD COLUMN IF NOT EXISTS template_dsl TEXT,
ADD COLUMN IF NOT EXISTS target_entity_type TEXT,
ADD COLUMN IF NOT EXISTS intent_confirmed BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS sheet JSONB;

-- Index for querying sessions by REPL state
CREATE INDEX IF NOT EXISTS idx_sessions_repl_state ON "ob-poc".sessions(repl_state);

-- Comment on new columns
COMMENT ON COLUMN "ob-poc".sessions.repl_state IS 'REPL state machine: empty, scoped, templated, generated, parsed, resolving, ready, executing, executed';
COMMENT ON COLUMN "ob-poc".sessions.scope_dsl IS 'DSL commands that defined the current scope (for audit/replay)';
COMMENT ON COLUMN "ob-poc".sessions.template_dsl IS 'Template DSL before expansion (unpopulated intent)';
COMMENT ON COLUMN "ob-poc".sessions.target_entity_type IS 'Entity type for template expansion (e.g., cbu)';
COMMENT ON COLUMN "ob-poc".sessions.intent_confirmed IS 'Whether user confirmed the intent';
COMMENT ON COLUMN "ob-poc".sessions.sheet IS 'Generated DSL sheet with statements, DAG phases, and execution status';

-- =============================================================================
-- SHEET EXECUTION AUDIT TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".sheet_execution_audit (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL,
    sheet_id UUID NOT NULL,

    -- Source tracking
    scope_dsl TEXT[] NOT NULL DEFAULT '{}',
    template_dsl TEXT,
    source_statements TEXT[] NOT NULL DEFAULT '{}',

    -- DAG analysis
    phase_count INTEGER NOT NULL DEFAULT 0,
    statement_count INTEGER NOT NULL DEFAULT 0,
    dag_analysis JSONB,

    -- Execution result
    overall_status TEXT NOT NULL,  -- success, failed, rolled_back
    phases_completed INTEGER NOT NULL DEFAULT 0,
    result JSONB NOT NULL,

    -- Timing
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,

    -- User tracking
    submitted_by TEXT,

    CONSTRAINT fk_session FOREIGN KEY (session_id)
        REFERENCES "ob-poc".sessions(id) ON DELETE CASCADE
);

-- Indexes for audit queries
CREATE INDEX IF NOT EXISTS idx_sheet_audit_session ON "ob-poc".sheet_execution_audit(session_id);
CREATE INDEX IF NOT EXISTS idx_sheet_audit_submitted ON "ob-poc".sheet_execution_audit(submitted_at);
CREATE INDEX IF NOT EXISTS idx_sheet_audit_status ON "ob-poc".sheet_execution_audit(overall_status);

-- Comment on audit table
COMMENT ON TABLE "ob-poc".sheet_execution_audit IS 'Audit trail of DSL sheet executions for debugging and compliance';
-- Migration: 037_candle_pipeline_complete.sql
-- Completes the Candle semantic pipeline infrastructure
--
-- Architecture:
--   SOURCE OF TRUTH: "ob-poc".dsl_verbs.intent_patterns (synced from YAML)
--   DERIVED CACHE:   "ob-poc".verb_pattern_embeddings (populated by populate_embeddings)
--   LEARNING LOOP:   Adds patterns to dsl_verbs.intent_patterns → re-run populate_embeddings
--
-- This migration:
--   1. Ensures verb_pattern_embeddings has correct schema
--   2. Creates view for easy pattern extraction
--   3. Creates function to add learned patterns back to dsl_verbs
--   4. Adds user-specific phrase learning table (agent schema)

-- =============================================================================
-- ENSURE verb_pattern_embeddings HAS CORRECT SCHEMA
-- =============================================================================
-- This is the derived lookup cache with embeddings

-- Add missing columns if needed
ALTER TABLE "ob-poc".verb_pattern_embeddings
    ADD COLUMN IF NOT EXISTS match_method TEXT DEFAULT 'semantic';

-- Ensure index exists for semantic search
CREATE INDEX IF NOT EXISTS idx_verb_pattern_embeddings_semantic
    ON "ob-poc".verb_pattern_embeddings
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- =============================================================================
-- VIEW: EXTRACT PATTERNS FROM dsl_verbs (SOURCE OF TRUTH)
-- =============================================================================
-- Used by populate_embeddings to read patterns from DB

CREATE OR REPLACE VIEW "ob-poc".v_verb_intent_patterns AS
SELECT
    v.full_name as verb_full_name,
    unnest(v.intent_patterns) as pattern,
    v.category,
    CASE
        WHEN v.category IN ('investigation', 'screening', 'kyc_workflow') THEN true
        ELSE false
    END as is_agent_bound,
    50 as priority  -- Default priority, can be overridden
FROM "ob-poc".dsl_verbs v
WHERE v.intent_patterns IS NOT NULL
  AND array_length(v.intent_patterns, 1) > 0;

COMMENT ON VIEW "ob-poc".v_verb_intent_patterns IS
    'Flattened view of intent patterns from dsl_verbs - used by populate_embeddings';

-- =============================================================================
-- FUNCTION: ADD LEARNED PATTERN TO dsl_verbs
-- =============================================================================
-- Learning loop calls this to persist new patterns discovered from user feedback

CREATE OR REPLACE FUNCTION "ob-poc".add_learned_pattern(
    p_verb_full_name TEXT,
    p_pattern TEXT
) RETURNS BOOLEAN AS $$
DECLARE
    v_updated BOOLEAN := false;
BEGIN
    -- Add pattern if verb exists and pattern not already present
    UPDATE "ob-poc".dsl_verbs
    SET intent_patterns = array_append(
            COALESCE(intent_patterns, ARRAY[]::text[]),
            p_pattern
        ),
        updated_at = NOW()
    WHERE full_name = p_verb_full_name
      AND NOT (p_pattern = ANY(COALESCE(intent_patterns, ARRAY[]::text[])));

    v_updated := FOUND;
    RETURN v_updated;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".add_learned_pattern IS
    'Add a learned pattern to dsl_verbs.intent_patterns - called by learning loop';

-- =============================================================================
-- FUNCTION: BOOTSTRAP PATTERNS FROM VERB METADATA
-- =============================================================================
-- For verbs without intent_patterns, generate deterministic patterns from metadata

CREATE OR REPLACE FUNCTION "ob-poc".bootstrap_verb_patterns() RETURNS INT AS $$
DECLARE
    v_count INT := 0;
    v_rec RECORD;
BEGIN
    FOR v_rec IN
        SELECT verb_id, full_name, verb_name, domain, description
        FROM "ob-poc".dsl_verbs
        WHERE intent_patterns IS NULL
           OR array_length(intent_patterns, 1) = 0
           OR array_length(intent_patterns, 1) IS NULL
    LOOP
        -- Pattern A: verb name with spaces (e.g., "create cbu", "assign role")
        UPDATE "ob-poc".dsl_verbs
        SET intent_patterns = ARRAY[
            -- Pattern A: verb tokens
            replace(v_rec.verb_name, '-', ' '),
            -- Pattern B: domain.verb description
            v_rec.full_name || ' - ' || COALESCE(v_rec.description, ''),
            -- Pattern C: natural language form
            'when user wants to ' || COALESCE(v_rec.description, replace(v_rec.verb_name, '-', ' '))
        ],
        updated_at = NOW()
        WHERE verb_id = v_rec.verb_id;

        v_count := v_count + 1;
    END LOOP;

    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".bootstrap_verb_patterns IS
    'Generate initial intent_patterns for verbs that have none - deterministic from metadata';

-- =============================================================================
-- SEMANTIC SEARCH FUNCTION
-- =============================================================================
-- Primary entry point for verb discovery via embeddings

CREATE OR REPLACE FUNCTION "ob-poc".search_verbs_semantic(
    p_query_embedding vector(384),
    p_similarity_threshold REAL DEFAULT 0.70,
    p_max_results INT DEFAULT 5
)
RETURNS TABLE (
    verb_name TEXT,
    pattern_phrase TEXT,
    similarity REAL,
    category TEXT,
    is_agent_bound BOOLEAN,
    match_method TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        vpe.verb_name,
        vpe.pattern_phrase,
        (1 - (vpe.embedding <=> p_query_embedding))::REAL as similarity,
        vpe.category,
        vpe.is_agent_bound,
        COALESCE(vpe.match_method, 'semantic')::TEXT as match_method
    FROM "ob-poc".verb_pattern_embeddings vpe
    WHERE vpe.embedding IS NOT NULL
      AND (1 - (vpe.embedding <=> p_query_embedding)) > p_similarity_threshold
    ORDER BY vpe.embedding <=> p_query_embedding
    LIMIT p_max_results;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".search_verbs_semantic IS
    'Semantic verb discovery - returns ranked matches from verb_pattern_embeddings';

-- =============================================================================
-- USER-SPECIFIC LEARNED PHRASES (AGENT SCHEMA)
-- =============================================================================
-- Per-user phrase learning without polluting global vocabulary

CREATE TABLE IF NOT EXISTS agent.user_learned_phrases (
    id              BIGSERIAL PRIMARY KEY,
    user_id         UUID NOT NULL,
    phrase          TEXT NOT NULL,
    verb            TEXT NOT NULL,
    confidence      NUMERIC(3,2) DEFAULT 1.0,
    occurrence_count INT DEFAULT 1,
    embedding       vector(384),
    embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2',
    source          TEXT DEFAULT 'user_correction',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(user_id, phrase)
);

CREATE INDEX IF NOT EXISTS idx_user_learned_phrases_user
    ON agent.user_learned_phrases(user_id);
CREATE INDEX IF NOT EXISTS idx_user_learned_phrases_embedding
    ON agent.user_learned_phrases
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- =============================================================================
-- PHRASE BLOCKLIST
-- =============================================================================
-- Negative examples to prevent semantic false positives

CREATE TABLE IF NOT EXISTS agent.phrase_blocklist (
    id              BIGSERIAL PRIMARY KEY,
    phrase          TEXT NOT NULL,
    blocked_verb    TEXT NOT NULL,
    user_id         UUID,                   -- NULL = global blocklist
    reason          TEXT,
    embedding       vector(384),
    embedding_model TEXT DEFAULT 'all-MiniLM-L6-v2',
    expires_at      TIMESTAMPTZ,            -- NULL = permanent
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_phrase_blocklist_unique
    ON agent.phrase_blocklist(phrase, blocked_verb, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid));
CREATE INDEX IF NOT EXISTS idx_phrase_blocklist_embedding
    ON agent.phrase_blocklist
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 50);

-- =============================================================================
-- COMBINED SEARCH: USER + GLOBAL
-- =============================================================================
-- Searches user phrases first, then falls back to global verb_pattern_embeddings

CREATE OR REPLACE FUNCTION "ob-poc".search_verbs_with_user(
    p_query_embedding vector(384),
    p_user_id UUID,
    p_similarity_threshold REAL DEFAULT 0.70,
    p_max_results INT DEFAULT 5
)
RETURNS TABLE (
    verb_name TEXT,
    pattern_phrase TEXT,
    similarity REAL,
    source TEXT,
    is_agent_bound BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    -- User-specific phrases (higher priority)
    SELECT
        ulp.verb as verb_name,
        ulp.phrase as pattern_phrase,
        (1 - (ulp.embedding <=> p_query_embedding))::REAL as similarity,
        'user_learned'::TEXT as source,
        true as is_agent_bound
    FROM agent.user_learned_phrases ulp
    WHERE ulp.user_id = p_user_id
      AND ulp.embedding IS NOT NULL
      AND (1 - (ulp.embedding <=> p_query_embedding)) > p_similarity_threshold

    UNION ALL

    -- Global patterns
    SELECT
        vpe.verb_name,
        vpe.pattern_phrase,
        (1 - (vpe.embedding <=> p_query_embedding))::REAL as similarity,
        'global'::TEXT as source,
        vpe.is_agent_bound
    FROM "ob-poc".verb_pattern_embeddings vpe
    WHERE vpe.embedding IS NOT NULL
      AND (1 - (vpe.embedding <=> p_query_embedding)) > p_similarity_threshold

    ORDER BY similarity DESC
    LIMIT p_max_results;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- BLOCKLIST CHECK
-- =============================================================================

CREATE OR REPLACE FUNCTION agent.is_verb_blocked(
    p_query_embedding vector(384),
    p_verb TEXT,
    p_user_id UUID DEFAULT NULL,
    p_similarity_threshold REAL DEFAULT 0.85
) RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM agent.phrase_blocklist pb
        WHERE pb.blocked_verb = p_verb
          AND pb.embedding IS NOT NULL
          AND (pb.user_id IS NULL OR pb.user_id = p_user_id)
          AND (pb.expires_at IS NULL OR pb.expires_at > NOW())
          AND (1 - (pb.embedding <=> p_query_embedding)) > p_similarity_threshold
    );
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- STATS VIEW
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_verb_embedding_stats AS
SELECT
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs) as total_verbs,
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs
     WHERE intent_patterns IS NOT NULL AND array_length(intent_patterns, 1) > 0) as verbs_with_patterns,
    (SELECT COUNT(*) FROM "ob-poc".verb_pattern_embeddings) as total_embeddings,
    (SELECT COUNT(*) FROM "ob-poc".verb_pattern_embeddings WHERE embedding IS NOT NULL) as embeddings_populated,
    (SELECT COUNT(DISTINCT verb_name) FROM "ob-poc".verb_pattern_embeddings) as unique_verbs_embedded;

COMMENT ON VIEW "ob-poc".v_verb_embedding_stats IS
    'Statistics on verb pattern embedding coverage';

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE agent.user_learned_phrases IS
    'Per-user phrase → verb mappings with Candle embeddings (384-dim)';
COMMENT ON TABLE agent.phrase_blocklist IS
    'Negative examples to prevent semantic false positives';
-- Migration 038: Split YAML and learned intent patterns
--
-- PROBLEM: sync_invocation_phrases overwrites intent_patterns on startup,
--          which deletes learned patterns added by the learning loop.
--
-- SOLUTION: Add yaml_intent_patterns column for YAML-sourced patterns.
--           Learning loop continues to use intent_patterns (which becomes learned-only).
--           View v_verb_intent_patterns unions both for embedding.
--
-- FLOW:
--   YAML invocation_phrases → dsl_verbs.yaml_intent_patterns (startup sync)
--   Learning loop → dsl_verbs.intent_patterns (learned patterns)
--   v_verb_intent_patterns → UNION of both → populate_embeddings

-- 1. Add yaml_intent_patterns column
ALTER TABLE "ob-poc".dsl_verbs
ADD COLUMN IF NOT EXISTS yaml_intent_patterns text[] DEFAULT ARRAY[]::text[];

COMMENT ON COLUMN "ob-poc".dsl_verbs.yaml_intent_patterns IS
    'Intent patterns from YAML invocation_phrases - synced on startup, safe to overwrite';

COMMENT ON COLUMN "ob-poc".dsl_verbs.intent_patterns IS
    'Learned intent patterns from feedback loop - NOT overwritten on startup';

-- 2. Migrate existing intent_patterns to yaml_intent_patterns (one-time)
-- This preserves current patterns as the YAML baseline
UPDATE "ob-poc".dsl_verbs
SET yaml_intent_patterns = COALESCE(intent_patterns, ARRAY[]::text[])
WHERE yaml_intent_patterns IS NULL OR yaml_intent_patterns = ARRAY[]::text[];

-- 3. Dedupe: remove from intent_patterns anything that's now in yaml_intent_patterns
-- This preserves only "true learned deltas" in intent_patterns
UPDATE "ob-poc".dsl_verbs
SET intent_patterns = (
  SELECT COALESCE(array_agg(DISTINCT p), ARRAY[]::text[])
  FROM unnest(COALESCE(intent_patterns, ARRAY[]::text[])) p
  WHERE NOT (p = ANY(COALESCE(yaml_intent_patterns, ARRAY[]::text[])))
)
WHERE intent_patterns IS NOT NULL
  AND array_length(intent_patterns, 1) > 0;

-- 4. Drop and recreate view to UNION both pattern sources
DROP VIEW IF EXISTS "ob-poc".v_verb_intent_patterns CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_verb_embedding_stats CASCADE;

CREATE VIEW "ob-poc".v_verb_intent_patterns AS
SELECT
    v.full_name as verb_full_name,
    pattern,
    v.category,
    CASE
        WHEN v.category IN ('investigation', 'screening', 'kyc_workflow') THEN true
        ELSE false
    END as is_agent_bound,
    1 as priority,
    'yaml' as source
FROM "ob-poc".dsl_verbs v
CROSS JOIN LATERAL unnest(v.yaml_intent_patterns) as pattern
WHERE v.yaml_intent_patterns IS NOT NULL
  AND array_length(v.yaml_intent_patterns, 1) > 0

UNION ALL

SELECT
    v.full_name as verb_full_name,
    pattern,
    v.category,
    CASE
        WHEN v.category IN ('investigation', 'screening', 'kyc_workflow') THEN true
        ELSE false
    END as is_agent_bound,
    2 as priority,  -- Learned patterns get higher priority
    'learned' as source
FROM "ob-poc".dsl_verbs v
CROSS JOIN LATERAL unnest(v.intent_patterns) as pattern
WHERE v.intent_patterns IS NOT NULL
  AND array_length(v.intent_patterns, 1) > 0;

COMMENT ON VIEW "ob-poc".v_verb_intent_patterns IS
    'Flattened view of all intent patterns (YAML + learned) for embedding population';

-- 5. Recreate stats view with new columns
CREATE VIEW "ob-poc".v_verb_embedding_stats AS
SELECT
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs) as total_verbs,
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs
     WHERE (yaml_intent_patterns IS NOT NULL AND array_length(yaml_intent_patterns, 1) > 0)
        OR (intent_patterns IS NOT NULL AND array_length(intent_patterns, 1) > 0)) as verbs_with_patterns,
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs
     WHERE yaml_intent_patterns IS NOT NULL AND array_length(yaml_intent_patterns, 1) > 0) as verbs_with_yaml_patterns,
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs
     WHERE intent_patterns IS NOT NULL AND array_length(intent_patterns, 1) > 0) as verbs_with_learned_patterns,
    (SELECT COUNT(*) FROM "ob-poc".verb_pattern_embeddings WHERE embedding IS NOT NULL) as total_embeddings,
    (SELECT COUNT(DISTINCT verb_name) FROM "ob-poc".verb_pattern_embeddings) as unique_verbs_embedded;

COMMENT ON VIEW "ob-poc".v_verb_embedding_stats IS
    'Statistics for verb embedding coverage - split by YAML vs learned patterns';

-- 6. Update add_learned_pattern to ensure it goes to intent_patterns (not yaml)
-- (Already correct - appends to intent_patterns)

-- 7. Add index for yaml_intent_patterns lookups
CREATE INDEX IF NOT EXISTS idx_dsl_verbs_yaml_patterns
ON "ob-poc".dsl_verbs USING GIN (yaml_intent_patterns)
WHERE yaml_intent_patterns IS NOT NULL;
-- Migration 039: Link dsl_generation_log to intent_feedback for learning loop
--
-- Purpose: Enable the learning loop to correlate verb matches with DSL execution outcomes
--
-- Architecture:
--   intent_feedback: captures phrase → verb match (learning signal)
--   dsl_generation_log: captures LLM → DSL generation (audit trail)
--
--   This migration adds:
--   1. FK from dsl_generation_log → intent_feedback (optional link)
--   2. Execution outcome columns to dsl_generation_log
--   3. Index for learning queries that join both tables

-- ============================================================================
-- 1. Add intent_feedback_id FK to dsl_generation_log
-- ============================================================================

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS intent_feedback_id BIGINT;

-- FK constraint (nullable - not all DSL generations come from chat)
ALTER TABLE "ob-poc".dsl_generation_log
ADD CONSTRAINT fk_generation_log_feedback
FOREIGN KEY (intent_feedback_id)
REFERENCES "ob-poc".intent_feedback(id)
ON DELETE SET NULL;

COMMENT ON COLUMN "ob-poc".dsl_generation_log.intent_feedback_id IS
'Links to intent_feedback for learning loop. NULL for direct DSL execution without chat.';

-- ============================================================================
-- 2. Add execution outcome columns to dsl_generation_log
-- ============================================================================

-- Execution status enum
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'execution_status') THEN
        CREATE TYPE "ob-poc".execution_status AS ENUM (
            'pending',      -- DSL generated, not yet executed
            'executed',     -- Successfully executed
            'failed',       -- Execution error
            'cancelled',    -- User cancelled before execution
            'skipped'       -- Skipped (e.g., dependency failed)
        );
    END IF;
END
$$;

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS execution_status "ob-poc".execution_status DEFAULT 'pending';

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS execution_error TEXT;

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS executed_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS affected_entity_ids UUID[];

COMMENT ON COLUMN "ob-poc".dsl_generation_log.execution_status IS
'Outcome of DSL execution: pending, executed, failed, cancelled, skipped';

COMMENT ON COLUMN "ob-poc".dsl_generation_log.execution_error IS
'Error message if execution_status = failed';

COMMENT ON COLUMN "ob-poc".dsl_generation_log.executed_at IS
'Timestamp when DSL was executed (NULL if pending/cancelled)';

COMMENT ON COLUMN "ob-poc".dsl_generation_log.affected_entity_ids IS
'Entity UUIDs created/modified by this DSL execution';

-- ============================================================================
-- 3. Indexes for learning queries
-- ============================================================================

-- Index for joining feedback to generation log
CREATE INDEX IF NOT EXISTS idx_generation_log_feedback
ON "ob-poc".dsl_generation_log(intent_feedback_id)
WHERE intent_feedback_id IS NOT NULL;

-- Index for execution status queries
CREATE INDEX IF NOT EXISTS idx_generation_log_exec_status
ON "ob-poc".dsl_generation_log(execution_status);

-- Index for failed executions (high-value learning signal)
CREATE INDEX IF NOT EXISTS idx_generation_log_failures
ON "ob-poc".dsl_generation_log(created_at)
WHERE execution_status = 'failed';

-- ============================================================================
-- 4. Learning analysis view
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_learning_feedback AS
SELECT
    f.id as feedback_id,
    f.interaction_id,
    f.session_id,
    f.user_input,
    f.input_source,
    f.matched_verb,
    f.match_score,
    f.match_confidence,
    f.alternatives,
    f.outcome as feedback_outcome,
    f.outcome_verb,
    f.created_at as feedback_at,

    g.log_id as generation_log_id,
    g.user_intent as generation_intent,
    g.final_valid_dsl,
    g.model_used,
    g.total_attempts,
    g.success as generation_success,
    g.execution_status,
    g.execution_error,
    g.executed_at,
    g.affected_entity_ids,
    g.total_latency_ms,
    g.total_input_tokens,
    g.total_output_tokens,

    -- Learning signals
    CASE
        WHEN f.outcome = 'executed' AND g.execution_status = 'executed' THEN 'success'
        WHEN f.outcome = 'executed' AND g.execution_status = 'failed' THEN 'false_positive'
        WHEN f.outcome = 'selected_alt' THEN 'wrong_match'
        WHEN f.outcome = 'corrected' THEN 'correction_needed'
        WHEN f.outcome = 'abandoned' THEN 'no_match'
        ELSE 'pending'
    END as learning_signal,

    -- Time metrics
    EXTRACT(EPOCH FROM (g.executed_at - f.created_at)) * 1000 as phrase_to_execution_ms

FROM "ob-poc".intent_feedback f
LEFT JOIN "ob-poc".dsl_generation_log g ON g.intent_feedback_id = f.id
ORDER BY f.created_at DESC;

COMMENT ON VIEW "ob-poc".v_learning_feedback IS
'Unified view joining feedback capture with DSL generation outcomes for learning analysis';

-- ============================================================================
-- 5. Learning summary stats view
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_learning_stats AS
SELECT
    DATE(created_at) as date,
    COUNT(*) as total_interactions,
    COUNT(*) FILTER (WHERE outcome = 'executed') as executed,
    COUNT(*) FILTER (WHERE outcome = 'selected_alt') as selected_alternative,
    COUNT(*) FILTER (WHERE outcome = 'corrected') as corrected,
    COUNT(*) FILTER (WHERE outcome = 'abandoned') as abandoned,
    COUNT(*) FILTER (WHERE outcome IS NULL) as pending,

    -- Match quality
    AVG(match_score) FILTER (WHERE outcome = 'executed') as avg_success_score,
    AVG(match_score) FILTER (WHERE outcome IN ('selected_alt', 'corrected')) as avg_failure_score,

    -- Confidence distribution
    COUNT(*) FILTER (WHERE match_confidence = 'high') as high_confidence,
    COUNT(*) FILTER (WHERE match_confidence = 'medium') as medium_confidence,
    COUNT(*) FILTER (WHERE match_confidence = 'low') as low_confidence,
    COUNT(*) FILTER (WHERE match_confidence = 'none') as no_match

FROM "ob-poc".intent_feedback
GROUP BY DATE(created_at)
ORDER BY date DESC;

COMMENT ON VIEW "ob-poc".v_learning_stats IS
'Daily learning statistics for monitoring feedback loop health';
-- Migration 040: DSL Diff Tracking for Learning Loop
--
-- Captures the diff between agent-generated DSL and what the user actually executed.
-- This is valuable correction signal for improving the agent.

-- Add DSL tracking columns to intent_feedback
ALTER TABLE "ob-poc".intent_feedback
ADD COLUMN IF NOT EXISTS generated_dsl TEXT,
ADD COLUMN IF NOT EXISTS final_dsl TEXT,
ADD COLUMN IF NOT EXISTS user_edits JSONB;

COMMENT ON COLUMN "ob-poc".intent_feedback.generated_dsl IS 'DSL as generated by agent (before user edits)';
COMMENT ON COLUMN "ob-poc".intent_feedback.final_dsl IS 'DSL as actually executed (after user edits)';
COMMENT ON COLUMN "ob-poc".intent_feedback.user_edits IS 'Array of {field, from, to} edit records capturing what user changed';

-- Add direct_dsl to match_method enum if not exists
DO $$
BEGIN
    -- Check if direct_dsl already exists in the enum
    IF NOT EXISTS (
        SELECT 1 FROM pg_enum
        WHERE enumlabel = 'direct_dsl'
        AND enumtypid = (SELECT oid FROM pg_type WHERE typname = 'match_method' AND typnamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'ob-poc'))
    ) THEN
        ALTER TYPE "ob-poc".match_method ADD VALUE IF NOT EXISTS 'direct_dsl';
    END IF;
END$$;

-- Update the learning feedback view to include DSL diff info
CREATE OR REPLACE VIEW "ob-poc".v_learning_feedback AS
SELECT
    f.id,
    f.interaction_id,
    f.session_id,
    f.input_phrase,
    f.input_source,
    f.matched_verb,
    f.match_method,
    f.similarity_score,
    f.domain_context,
    f.outcome,
    f.generated_dsl,
    f.final_dsl,
    f.user_edits,
    f.created_at,
    g.execution_status,
    g.execution_error,
    g.executed_at,
    -- Learning signal derivation
    CASE
        WHEN f.outcome = 'executed' AND f.user_edits IS NULL THEN 'success'
        WHEN f.outcome = 'executed' AND f.user_edits IS NOT NULL THEN 'correction'
        WHEN f.outcome = 'wrong_match' THEN 'wrong_match'
        WHEN f.outcome = 'rejected' THEN 'rejection'
        WHEN f.matched_verb IS NULL THEN 'no_match'
        WHEN g.execution_status = 'failed' THEN 'false_positive'
        ELSE 'pending'
    END AS learning_signal,
    -- Was DSL edited?
    CASE
        WHEN f.generated_dsl IS NOT NULL AND f.final_dsl IS NOT NULL
             AND f.generated_dsl != f.final_dsl THEN true
        ELSE false
    END AS was_edited,
    -- Time from phrase to execution
    CASE
        WHEN g.executed_at IS NOT NULL THEN
            EXTRACT(EPOCH FROM (g.executed_at - f.created_at)) * 1000
        ELSE NULL
    END AS phrase_to_execution_ms
FROM "ob-poc".intent_feedback f
LEFT JOIN "ob-poc".dsl_generation_log g ON g.intent_feedback_id = f.id;

-- Index for finding corrections (edited DSL)
CREATE INDEX IF NOT EXISTS idx_intent_feedback_has_edits
ON "ob-poc".intent_feedback ((user_edits IS NOT NULL))
WHERE user_edits IS NOT NULL;

-- View for analyzing user corrections
CREATE OR REPLACE VIEW "ob-poc".v_user_corrections AS
SELECT
    f.matched_verb,
    f.input_phrase,
    f.generated_dsl,
    f.final_dsl,
    f.user_edits,
    jsonb_array_length(f.user_edits) AS edit_count,
    f.outcome,
    f.created_at
FROM "ob-poc".intent_feedback f
WHERE f.user_edits IS NOT NULL
  AND jsonb_array_length(f.user_edits) > 0
ORDER BY f.created_at DESC;

COMMENT ON VIEW "ob-poc".v_user_corrections IS 'Shows cases where users edited agent-generated DSL - valuable for learning';
-- Migration 040: Primary Governance Controller Model
--
-- Purpose: Enable governance-controller-centric CBU grouping and shareholding traversal
--
-- Key Concepts:
--   1. Primary Governance Controller = Entity that controls a CBU via board appointment rights
--   2. CBU Group = Collection of CBUs under same governance controller (the "Allianz Lux Book")
--   3. Holding Control Link = Shareholding that confers control (≥ threshold)
--
-- Signal Priority (deterministic):
--   1. Board appointment rights via control share class (primary)
--   2. MANAGEMENT_COMPANY role assignment (fallback)
--   3. GLEIF IS_FUND_MANAGED_BY (fallback)
--
-- Design Principles:
--   - Control share class = who appoints the board (canonical definition)
--   - Class-level board rights flow to holders of that class
--   - Single deterministic winner per CBU (tie-break by seats, then %, then UUID)
--   - Groups are derivable from data, not manually maintained

BEGIN;

-- =============================================================================
-- 1. CBU GROUPS (Governance-controller-anchored collections)
-- =============================================================================
-- A "group" or "book" is a logical collection of CBUs sharing a governance controller.
-- This provides the "Allianz Lux Book" concept.

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_groups (
    group_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The entity that anchors this group (governance controller or ManCo)
    manco_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Group metadata
    group_name VARCHAR(255) NOT NULL,
    group_code VARCHAR(50),  -- Short code like "ALLIANZ_LUX"
    group_type VARCHAR(30) NOT NULL DEFAULT 'GOVERNANCE_BOOK',

    -- Jurisdiction scope (optional - a controller might have multiple books per jurisdiction)
    jurisdiction VARCHAR(10),

    -- Ultimate parent (for display - e.g., Allianz SE)
    ultimate_parent_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Description
    description TEXT,

    -- Auto-derived flag (true = computed from governance controller, false = manually created)
    is_auto_derived BOOLEAN DEFAULT true,

    -- Temporal
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    created_by VARCHAR(100),

    CONSTRAINT chk_group_type CHECK (group_type IN (
        'GOVERNANCE_BOOK',      -- Computed from board appointment / control signals
        'MANCO_BOOK',           -- Standard ManCo management group (fallback)
        'CORPORATE_GROUP',      -- Corporate entity group (non-fund)
        'INVESTMENT_MANAGER',   -- Grouped by IM rather than ManCo
        'UMBRELLA_SICAV',       -- Sub-funds of a SICAV umbrella
        'CUSTOM'                -- Manual grouping
    )),

    -- One active group per controller per jurisdiction
    UNIQUE NULLS NOT DISTINCT (manco_entity_id, jurisdiction, effective_to)
);

CREATE INDEX IF NOT EXISTS idx_cbu_groups_manco
    ON "ob-poc".cbu_groups(manco_entity_id);
CREATE INDEX IF NOT EXISTS idx_cbu_groups_active
    ON "ob-poc".cbu_groups(manco_entity_id) WHERE effective_to IS NULL;

COMMENT ON TABLE "ob-poc".cbu_groups IS
    'Governance-controller-anchored CBU groups ("books"). Enables querying all CBUs under a controller.';

-- =============================================================================
-- 2. CBU GROUP MEMBERSHIP (link CBU to group)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_group_members (
    membership_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    group_id UUID NOT NULL REFERENCES "ob-poc".cbu_groups(group_id) ON DELETE CASCADE,
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,

    -- How was this membership determined?
    source VARCHAR(30) NOT NULL DEFAULT 'GOVERNANCE_CONTROLLER',

    -- Order within group (for display)
    display_order INTEGER DEFAULT 0,

    -- Temporal
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT chk_membership_source CHECK (source IN (
        'GOVERNANCE_CONTROLLER', -- Computed from board appointment / control signals
        'MANCO_ROLE',            -- From cbu_entity_roles MANAGEMENT_COMPANY
        'GLEIF_MANAGED',         -- From gleif_relationships IS_FUND_MANAGED_BY
        'SHAREHOLDING',          -- From controlling shareholding
        'MANUAL'                 -- Manually assigned
    )),

    -- One active membership per CBU per group
    UNIQUE NULLS NOT DISTINCT (group_id, cbu_id, effective_to)
);

CREATE INDEX IF NOT EXISTS idx_cbu_group_members_group
    ON "ob-poc".cbu_group_members(group_id);
CREATE INDEX IF NOT EXISTS idx_cbu_group_members_cbu
    ON "ob-poc".cbu_group_members(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_group_members_active
    ON "ob-poc".cbu_group_members(cbu_id) WHERE effective_to IS NULL;

COMMENT ON TABLE "ob-poc".cbu_group_members IS
    'CBU membership in groups. One CBU can belong to one group at a time (active).';

-- =============================================================================
-- 3. HOLDING CONTROL LINKS (shareholdings that confer control)
-- =============================================================================
-- Materializes the "controlling interest" relationships derived from holdings.
-- Answers: "Which entities control which other entities via shareholding?"

CREATE TABLE IF NOT EXISTS kyc.holding_control_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The holder (controller)
    holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- The issuer (controlled entity)
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- The share class(es) that establish control (nullable = aggregated across classes)
    share_class_id UUID REFERENCES kyc.share_classes(id),

    -- Control metrics (aggregated if share_class_id is NULL)
    total_units NUMERIC(20,6),
    voting_pct NUMERIC(8,4),
    economic_pct NUMERIC(8,4),

    -- Control classification
    control_type VARCHAR(30) NOT NULL,

    -- Threshold used (for audit)
    threshold_pct NUMERIC(5,2),

    -- Is this a direct holding or computed through chain?
    is_direct BOOLEAN DEFAULT true,
    chain_depth INTEGER DEFAULT 1,  -- 1 = direct, 2+ = indirect

    -- Source holdings (for traceability)
    source_holding_ids UUID[],

    -- Temporal
    as_of_date DATE NOT NULL DEFAULT CURRENT_DATE,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT chk_control_type CHECK (control_type IN (
        'CONTROLLING',           -- ≥ 50% (or issuer-specific control threshold)
        'SIGNIFICANT_INFLUENCE', -- ≥ 25% (or issuer-specific significant threshold)
        'MATERIAL',              -- ≥ 10% (or issuer-specific material threshold)
        'NOTIFIABLE',            -- ≥ 5% (or issuer-specific disclosure threshold)
        'MINORITY'               -- < disclosure threshold but tracked
    ))
);

CREATE INDEX IF NOT EXISTS idx_control_links_holder
    ON kyc.holding_control_links(holder_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_links_issuer
    ON kyc.holding_control_links(issuer_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_links_controlling
    ON kyc.holding_control_links(holder_entity_id)
    WHERE control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE');
CREATE INDEX IF NOT EXISTS idx_control_links_date
    ON kyc.holding_control_links(as_of_date DESC);

COMMENT ON TABLE kyc.holding_control_links IS
    'Materialized control relationships derived from shareholdings. Enables efficient graph traversal.';

-- =============================================================================
-- 4. FUNCTION: Holder control position (FIXED - class-level board rights flow to holders)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_holder_control_position(
    p_issuer_entity_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE,
    p_basis TEXT DEFAULT 'VOTES'
)
RETURNS TABLE (
    issuer_entity_id UUID,
    issuer_name TEXT,
    holder_entity_id UUID,
    holder_name TEXT,
    holder_type TEXT,
    holder_units NUMERIC,
    holder_votes NUMERIC,
    holder_economic NUMERIC,
    total_issuer_votes NUMERIC,
    total_issuer_economic NUMERIC,
    voting_pct NUMERIC,
    economic_pct NUMERIC,
    control_threshold_pct NUMERIC,
    significant_threshold_pct NUMERIC,
    has_control BOOLEAN,
    has_significant_influence BOOLEAN,
    has_board_rights BOOLEAN,
    board_seats INTEGER
) AS $$
BEGIN
    RETURN QUERY
    WITH issuer_supply AS (
        -- Aggregate supply across all share classes for issuer (as-of)
        SELECT
            SUM(COALESCE(scs.issued_units, 0)
                * COALESCE(sc.votes_per_unit, 1)) AS total_votes,
            SUM(COALESCE(scs.issued_units, 0)
                * COALESCE(sc.economic_per_unit, 1)) AS total_economic
        FROM kyc.share_classes sc
        LEFT JOIN LATERAL (
            SELECT scs2.*
            FROM kyc.share_class_supply scs2
            WHERE scs2.share_class_id = sc.id
              AND scs2.as_of_date <= p_as_of
            ORDER BY scs2.as_of_date DESC
            LIMIT 1
        ) scs ON true
        WHERE sc.issuer_entity_id = p_issuer_entity_id
    ),
    holder_positions AS (
        -- Aggregate holdings per holder across all classes
        SELECT
            h.investor_entity_id,
            SUM(h.units) AS units,
            SUM(h.units * COALESCE(sc.votes_per_unit, 1)) AS votes,
            SUM(h.units * COALESCE(sc.economic_per_unit, 1)) AS economic
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        WHERE sc.issuer_entity_id = p_issuer_entity_id
          AND h.status = 'active'
        GROUP BY h.investor_entity_id
    ),

    -- -----------------------------
    -- Board rights: holder-attached
    -- -----------------------------
    holder_specific_rights AS (
        SELECT
            sr.holder_entity_id,
            COALESCE(SUM(COALESCE(sr.board_seats, 1)), 0) AS board_seats
        FROM kyc.special_rights sr
        WHERE sr.issuer_entity_id = p_issuer_entity_id
          AND sr.holder_entity_id IS NOT NULL
          AND sr.right_type = 'BOARD_APPOINTMENT'
          AND (sr.effective_to IS NULL OR sr.effective_to > p_as_of)
          AND (sr.effective_from IS NULL OR sr.effective_from <= p_as_of)
        GROUP BY sr.holder_entity_id
    ),

    -- --------------------------------------
    -- Board rights: class-attached allocation
    -- Deterministic v1 policy:
    --   - determine eligible holders of that class
    --   - allocate ALL seats to the single top eligible holder
    --     (highest pct_of_class; tie-breaker holder UUID)
    -- --------------------------------------
    class_rights AS (
        SELECT
            sr.right_id,
            sr.share_class_id,
            COALESCE(sr.board_seats, 1) AS board_seats,
            sr.threshold_pct,
            sr.threshold_basis
        FROM kyc.special_rights sr
        WHERE sr.issuer_entity_id = p_issuer_entity_id
          AND sr.share_class_id IS NOT NULL
          AND sr.right_type = 'BOARD_APPOINTMENT'
          AND (sr.effective_to IS NULL OR sr.effective_to > p_as_of)
          AND (sr.effective_from IS NULL OR sr.effective_from <= p_as_of)
    ),
    class_supply AS (
        SELECT
            sc.id AS share_class_id,
            COALESCE(scs.outstanding_units, scs.issued_units, 0) AS class_units
        FROM kyc.share_classes sc
        LEFT JOIN LATERAL (
            SELECT scs2.*
            FROM kyc.share_class_supply scs2
            WHERE scs2.share_class_id = sc.id
              AND scs2.as_of_date <= p_as_of
            ORDER BY scs2.as_of_date DESC
            LIMIT 1
        ) scs ON true
        WHERE sc.issuer_entity_id = p_issuer_entity_id
    ),
    holders_in_class AS (
        SELECT
            h.share_class_id,
            h.investor_entity_id AS holder_entity_id,
            SUM(h.units) AS holder_units_in_class,
            cs.class_units,
            CASE
                WHEN COALESCE(cs.class_units, 0) > 0
                THEN (SUM(h.units) / cs.class_units) * 100
                ELSE 0
            END AS pct_of_class
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        JOIN class_supply cs ON cs.share_class_id = h.share_class_id
        WHERE sc.issuer_entity_id = p_issuer_entity_id
          AND h.status = 'active'
        GROUP BY h.share_class_id, h.investor_entity_id, cs.class_units
    ),
    class_right_candidates AS (
        SELECT
            cr.right_id,
            cr.share_class_id,
            cr.board_seats,
            hic.holder_entity_id,
            hic.pct_of_class,
            CASE
                WHEN cr.threshold_pct IS NULL THEN true
                -- treat null/UNITS/CLASS_UNITS/VOTES the same at class scope (v1)
                WHEN COALESCE(cr.threshold_basis, 'UNITS') IN ('UNITS','CLASS_UNITS','VOTES')
                     AND hic.pct_of_class >= cr.threshold_pct THEN true
                ELSE false
            END AS is_eligible
        FROM class_rights cr
        JOIN holders_in_class hic ON hic.share_class_id = cr.share_class_id
    ),
    class_rights_allocated AS (
        SELECT
            x.holder_entity_id AS alloc_holder_entity_id,
            SUM(x.board_seats) AS board_seats
        FROM (
            SELECT
                crc.holder_entity_id,
                crc.board_seats,
                ROW_NUMBER() OVER (
                    PARTITION BY crc.right_id
                    ORDER BY crc.is_eligible DESC, crc.pct_of_class DESC, crc.holder_entity_id ASC
                ) AS rn
            FROM class_right_candidates crc
            WHERE crc.is_eligible = true
        ) x
        WHERE x.rn = 1
        GROUP BY x.holder_entity_id
    ),

    -- -----------------------------
    -- Unified holder_rights
    -- -----------------------------
    holder_rights AS (
        SELECT
            u.hr_holder_entity_id AS rights_holder_entity_id,
            SUM(u.board_seats) AS board_seats
        FROM (
            SELECT hsr.holder_entity_id AS hr_holder_entity_id, hsr.board_seats FROM holder_specific_rights hsr
            UNION ALL
            SELECT cra.alloc_holder_entity_id AS hr_holder_entity_id, cra.board_seats FROM class_rights_allocated cra
        ) u
        GROUP BY u.hr_holder_entity_id
    ),

    config AS (
        SELECT
            COALESCE(icc.control_threshold_pct, 50) AS control_threshold,
            COALESCE(icc.significant_threshold_pct, 25) AS significant_threshold
        FROM kyc.issuer_control_config icc
        WHERE icc.issuer_entity_id = p_issuer_entity_id
          AND (icc.effective_to IS NULL OR icc.effective_to > p_as_of)
          AND icc.effective_from <= p_as_of
        ORDER BY icc.effective_from DESC
        LIMIT 1
    ),
    -- Combine holders from positions (have holdings) and rights (may have board rights without holdings)
    all_holders AS (
        SELECT investor_entity_id AS holder_entity_id, units, votes, economic
        FROM holder_positions
        UNION
        SELECT rights_holder_entity_id AS holder_entity_id, 0::NUMERIC, 0::NUMERIC, 0::NUMERIC
        FROM holder_rights
        WHERE rights_holder_entity_id NOT IN (SELECT investor_entity_id FROM holder_positions)
    )
    SELECT
        p_issuer_entity_id,
        ie.name::TEXT,
        ah.holder_entity_id,
        he.name::TEXT,
        het.type_code::TEXT,
        ah.units,
        ah.votes,
        ah.economic,
        isu.total_votes,
        isu.total_economic,
        CASE WHEN isu.total_votes > 0 THEN ROUND((ah.votes / isu.total_votes) * 100, 4) ELSE 0 END,
        CASE WHEN isu.total_economic > 0 THEN ROUND((ah.economic / isu.total_economic) * 100, 4) ELSE 0 END,
        COALESCE(cfg.control_threshold, 50),
        COALESCE(cfg.significant_threshold, 25),
        CASE WHEN isu.total_votes > 0 AND (ah.votes / isu.total_votes) * 100 > COALESCE(cfg.control_threshold, 50) THEN true ELSE false END,
        CASE WHEN isu.total_votes > 0 AND (ah.votes / isu.total_votes) * 100 > COALESCE(cfg.significant_threshold, 25) THEN true ELSE false END,
        COALESCE(hr.board_seats, 0) > 0,
        COALESCE(hr.board_seats, 0)::INTEGER
    FROM all_holders ah
    CROSS JOIN issuer_supply isu
    LEFT JOIN config cfg ON true
    LEFT JOIN holder_rights hr ON hr.rights_holder_entity_id = ah.holder_entity_id
    JOIN "ob-poc".entities ie ON ie.entity_id = p_issuer_entity_id
    JOIN "ob-poc".entities he ON he.entity_id = ah.holder_entity_id
    LEFT JOIN "ob-poc".entity_types het ON he.entity_type_id = het.entity_type_id
    ORDER BY ah.votes DESC;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION kyc.fn_holder_control_position IS
    'Compute holder control positions including class-level board appointment rights flowing to holders.';

-- =============================================================================
-- 5. FUNCTION: Primary governance controller (single deterministic winner per issuer)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_primary_governance_controller(
    p_issuer_entity_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    issuer_entity_id UUID,
    primary_controller_entity_id UUID,
    governance_controller_entity_id UUID,
    basis TEXT,
    board_seats INTEGER,
    voting_pct NUMERIC,
    economic_pct NUMERIC,
    has_control BOOLEAN,
    has_significant_influence BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    WITH ranked AS (
        SELECT
            hcp.*,
            ROW_NUMBER() OVER (
                ORDER BY
                    -- 1) board appointment rights first
                    hcp.has_board_rights DESC,
                    hcp.board_seats DESC,
                    -- 2) then voting control flags
                    hcp.has_control DESC,
                    hcp.has_significant_influence DESC,
                    -- 3) then raw % (stable)
                    hcp.voting_pct DESC,
                    hcp.economic_pct DESC,
                    hcp.holder_entity_id ASC
            ) AS rn
        FROM kyc.fn_holder_control_position(p_issuer_entity_id, p_as_of, 'VOTES') hcp
        WHERE (hcp.has_board_rights = true)
           OR (hcp.has_control = true)
           OR (hcp.has_significant_influence = true)
    ),
    winner AS (
        SELECT *
        FROM ranked
        WHERE rn = 1
    ),
    role_profile AS (
        SELECT rp.group_container_entity_id
        FROM kyc.investor_role_profiles rp
        JOIN winner w ON w.holder_entity_id = rp.holder_entity_id
        WHERE rp.issuer_entity_id = p_issuer_entity_id
          AND rp.effective_from <= p_as_of
          AND (rp.effective_to IS NULL OR rp.effective_to > p_as_of)
        ORDER BY rp.effective_from DESC
        LIMIT 1
    )
    SELECT
        p_issuer_entity_id,
        w.holder_entity_id AS primary_controller_entity_id,
        COALESCE(rp.group_container_entity_id, w.holder_entity_id) AS governance_controller_entity_id,
        CASE
            WHEN w.has_board_rights THEN 'BOARD_APPOINTMENT'
            WHEN w.has_control THEN 'VOTING_CONTROL'
            WHEN w.has_significant_influence THEN 'SIGNIFICANT_INFLUENCE'
            ELSE 'NONE'
        END AS basis,
        w.board_seats,
        w.voting_pct,
        w.economic_pct,
        w.has_control,
        w.has_significant_influence
    FROM winner w
    LEFT JOIN role_profile rp ON true;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION kyc.fn_primary_governance_controller IS
    'Return single deterministic primary governance controller for an issuer based on board rights > voting control > significant influence.';

-- =============================================================================
-- 6. FUNCTION: Compute holding control links (FIXED - correct denominator calculation)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_compute_control_links(
    p_issuer_entity_id UUID DEFAULT NULL,
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS INTEGER AS $$
DECLARE
    v_count INTEGER := 0;
BEGIN
    DELETE FROM kyc.holding_control_links
    WHERE as_of_date = p_as_of_date
      AND (p_issuer_entity_id IS NULL OR issuer_entity_id = p_issuer_entity_id);

    INSERT INTO kyc.holding_control_links (
        holder_entity_id,
        issuer_entity_id,
        share_class_id,
        total_units,
        voting_pct,
        economic_pct,
        control_type,
        threshold_pct,
        is_direct,
        chain_depth,
        source_holding_ids,
        as_of_date
    )
    WITH issuer_thresholds AS (
        SELECT
            icc.issuer_entity_id,
            COALESCE(icc.control_threshold_pct, 50.00) AS control_threshold,
            COALESCE(icc.significant_threshold_pct, 25.00) AS significant_threshold,
            COALESCE(icc.material_threshold_pct, 10.00) AS material_threshold,
            COALESCE(icc.disclosure_threshold_pct, 5.00) AS disclosure_threshold
        FROM kyc.issuer_control_config icc
        WHERE (icc.effective_to IS NULL OR icc.effective_to > p_as_of_date)
          AND icc.effective_from <= p_as_of_date
    ),
    issuer_denoms AS (
        SELECT
            sc.issuer_entity_id,
            SUM(COALESCE(scs.issued_units, 0)
                * COALESCE(sc.votes_per_unit, 1)) AS total_votes,
            SUM(COALESCE(scs.issued_units, 0)
                * COALESCE(sc.economic_per_unit, 1)) AS total_economic
        FROM kyc.share_classes sc
        LEFT JOIN LATERAL (
            SELECT scs2.*
            FROM kyc.share_class_supply scs2
            WHERE scs2.share_class_id = sc.id
              AND scs2.as_of_date <= p_as_of_date
            ORDER BY scs2.as_of_date DESC
            LIMIT 1
        ) scs ON true
        WHERE (p_issuer_entity_id IS NULL OR sc.issuer_entity_id = p_issuer_entity_id)
        GROUP BY sc.issuer_entity_id
    ),
    holder_totals AS (
        SELECT
            h.investor_entity_id AS holder_entity_id,
            sc.issuer_entity_id,
            h.share_class_id,
            SUM(h.units) AS total_units,
            SUM(h.units * COALESCE(sc.votes_per_unit, 1)) AS holder_votes,
            SUM(h.units * COALESCE(sc.economic_per_unit, 1)) AS holder_economic,
            ARRAY_AGG(h.id) AS source_holding_ids
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        WHERE h.status = 'active'
          AND (p_issuer_entity_id IS NULL OR sc.issuer_entity_id = p_issuer_entity_id)
        GROUP BY h.investor_entity_id, sc.issuer_entity_id, h.share_class_id
    )
    SELECT
        ht.holder_entity_id,
        ht.issuer_entity_id,
        ht.share_class_id,
        ht.total_units,
        CASE WHEN COALESCE(id.total_votes, 0) > 0
             THEN (ht.holder_votes / id.total_votes) * 100 ELSE 0 END AS voting_pct,
        CASE WHEN COALESCE(id.total_economic, 0) > 0
             THEN (ht.holder_economic / id.total_economic) * 100 ELSE 0 END AS economic_pct,
        CASE
            WHEN COALESCE(
                CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
                CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
                0
            ) >= COALESCE(it.control_threshold, 50) THEN 'CONTROLLING'
            WHEN COALESCE(
                CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
                CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
                0
            ) >= COALESCE(it.significant_threshold, 25) THEN 'SIGNIFICANT_INFLUENCE'
            WHEN COALESCE(
                CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
                CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
                0
            ) >= COALESCE(it.material_threshold, 10) THEN 'MATERIAL'
            WHEN COALESCE(
                CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
                CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
                0
            ) >= COALESCE(it.disclosure_threshold, 5) THEN 'NOTIFIABLE'
            ELSE 'MINORITY'
        END AS control_type,
        COALESCE(it.control_threshold, 50.00),
        true,
        1,
        ht.source_holding_ids,
        p_as_of_date
    FROM holder_totals ht
    JOIN issuer_denoms id ON id.issuer_entity_id = ht.issuer_entity_id
    LEFT JOIN issuer_thresholds it ON it.issuer_entity_id = ht.issuer_entity_id
    WHERE COALESCE(
        CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
        CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
        0
    ) >= COALESCE(it.disclosure_threshold, 5);

    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_compute_control_links IS
    'Compute control links from holdings with correct as-of denominator calculation.';

-- =============================================================================
-- 7. VIEW: ManCo Group Summary
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_manco_group_summary AS
SELECT
    g.group_id,
    g.group_name,
    g.group_code,
    g.group_type,
    g.manco_entity_id,
    me.name AS manco_name,
    g.jurisdiction,
    g.ultimate_parent_entity_id,
    upe.name AS ultimate_parent_name,
    COUNT(DISTINCT gm.cbu_id) AS cbu_count,
    ARRAY_AGG(DISTINCT c.name ORDER BY c.name) AS cbu_names,
    g.effective_from,
    g.is_auto_derived
FROM "ob-poc".cbu_groups g
JOIN "ob-poc".entities me ON me.entity_id = g.manco_entity_id
LEFT JOIN "ob-poc".entities upe ON upe.entity_id = g.ultimate_parent_entity_id
LEFT JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id
    AND (gm.effective_to IS NULL OR gm.effective_to > CURRENT_DATE)
LEFT JOIN "ob-poc".cbus c ON c.cbu_id = gm.cbu_id
WHERE g.effective_to IS NULL
GROUP BY g.group_id, g.group_name, g.group_code, g.group_type, g.manco_entity_id, me.name,
         g.jurisdiction, g.ultimate_parent_entity_id, upe.name,
         g.effective_from, g.is_auto_derived;

COMMENT ON VIEW "ob-poc".v_manco_group_summary IS
    'Summary of governance controller groups with CBU counts and names.';

-- =============================================================================
-- 8. VIEW: CBUs by controller with control chain
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cbus_by_manco AS
SELECT
    g.manco_entity_id,
    me.name AS manco_name,
    c.cbu_id,
    c.name AS cbu_name,
    c.cbu_category,
    c.jurisdiction,
    gm.source AS membership_source,
    -- Get controlling shareholder of the CBU's fund entity (if any)
    hcl.holder_entity_id AS controlling_holder_id,
    che.name AS controlling_holder_name,
    hcl.voting_pct AS controlling_voting_pct,
    hcl.control_type
FROM "ob-poc".cbu_groups g
JOIN "ob-poc".entities me ON me.entity_id = g.manco_entity_id
JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id
    AND (gm.effective_to IS NULL OR gm.effective_to > CURRENT_DATE)
JOIN "ob-poc".cbus c ON c.cbu_id = gm.cbu_id
-- Get the fund entity for the CBU (commercial_client_entity_id)
LEFT JOIN kyc.holding_control_links hcl ON hcl.issuer_entity_id = c.commercial_client_entity_id
    AND hcl.control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE')
    AND hcl.as_of_date = (SELECT MAX(as_of_date) FROM kyc.holding_control_links)
LEFT JOIN "ob-poc".entities che ON che.entity_id = hcl.holder_entity_id
WHERE g.effective_to IS NULL
ORDER BY g.manco_entity_id, c.name;

COMMENT ON VIEW "ob-poc".v_cbus_by_manco IS
    'All CBUs grouped by governance controller with controlling shareholder information.';

-- =============================================================================
-- 9. FUNCTION: Get all CBUs in a controller group
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fn_get_manco_group_cbus(
    p_manco_entity_id UUID
)
RETURNS TABLE (
    cbu_id UUID,
    cbu_name TEXT,
    cbu_category TEXT,
    jurisdiction VARCHAR(10),
    fund_entity_id UUID,
    fund_entity_name TEXT,
    membership_source VARCHAR(30)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        c.cbu_id,
        c.name::TEXT,
        c.cbu_category::TEXT,
        c.jurisdiction,
        c.commercial_client_entity_id,
        fe.name::TEXT,
        gm.source
    FROM "ob-poc".cbu_groups g
    JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id
        AND (gm.effective_to IS NULL OR gm.effective_to > CURRENT_DATE)
    JOIN "ob-poc".cbus c ON c.cbu_id = gm.cbu_id
    LEFT JOIN "ob-poc".entities fe ON fe.entity_id = c.commercial_client_entity_id
    WHERE g.manco_entity_id = p_manco_entity_id
      AND (g.effective_to IS NULL OR g.effective_to > CURRENT_DATE)
    ORDER BY c.name;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".fn_get_manco_group_cbus IS
    'Get all CBUs managed by a specific governance controller.';

-- =============================================================================
-- 10. FUNCTION: Find controller for a CBU
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fn_get_cbu_manco(
    p_cbu_id UUID
)
RETURNS TABLE (
    manco_entity_id UUID,
    manco_name TEXT,
    manco_lei VARCHAR(20),
    group_id UUID,
    group_name TEXT,
    group_type TEXT,
    source VARCHAR(30)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        g.manco_entity_id,
        me.name::TEXT,
        em.lei,
        g.group_id,
        g.group_name::TEXT,
        g.group_type::TEXT,
        gm.source
    FROM "ob-poc".cbu_group_members gm
    JOIN "ob-poc".cbu_groups g ON g.group_id = gm.group_id
        AND (g.effective_to IS NULL OR g.effective_to > CURRENT_DATE)
    JOIN "ob-poc".entities me ON me.entity_id = g.manco_entity_id
    LEFT JOIN "ob-poc".entity_manco em ON em.entity_id = g.manco_entity_id
    WHERE gm.cbu_id = p_cbu_id
      AND (gm.effective_to IS NULL OR gm.effective_to > CURRENT_DATE);
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".fn_get_cbu_manco IS
    'Get the governance controller for a specific CBU.';

-- =============================================================================
-- 11. FUNCTION: Get control chain for a controller group
-- =============================================================================
-- Trace the shareholding control chain upward to find ultimate controller.

CREATE OR REPLACE FUNCTION "ob-poc".fn_manco_group_control_chain(
    p_manco_entity_id UUID,
    p_max_depth INTEGER DEFAULT 5
)
RETURNS TABLE (
    depth INTEGER,
    entity_id UUID,
    entity_name TEXT,
    entity_type TEXT,
    controlled_by_entity_id UUID,
    controlled_by_name TEXT,
    control_type VARCHAR(30),
    voting_pct NUMERIC(8,4),
    is_ultimate_controller BOOLEAN
) AS $$
WITH RECURSIVE control_chain AS (
    -- Base: Start with the controller itself
    SELECT
        1 AS depth,
        e.entity_id,
        e.name::TEXT AS entity_name,
        et.type_code::TEXT AS entity_type,
        NULL::UUID AS controlled_by_entity_id,
        NULL::TEXT AS controlled_by_name,
        NULL::VARCHAR(30) AS control_type,
        NULL::NUMERIC(8,4) AS voting_pct
    FROM "ob-poc".entities e
    LEFT JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
    WHERE e.entity_id = p_manco_entity_id

    UNION ALL

    -- Recursive: Find who controls each entity via shareholding
    SELECT
        cc.depth + 1,
        hcl.holder_entity_id,
        he.name::TEXT,
        het.type_code::TEXT,
        cc.entity_id,
        cc.entity_name,
        hcl.control_type,
        hcl.voting_pct
    FROM control_chain cc
    JOIN kyc.holding_control_links hcl ON hcl.issuer_entity_id = cc.entity_id
        AND hcl.control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE')
        AND hcl.as_of_date = (SELECT MAX(as_of_date) FROM kyc.holding_control_links)
    JOIN "ob-poc".entities he ON he.entity_id = hcl.holder_entity_id
    LEFT JOIN "ob-poc".entity_types het ON het.entity_type_id = he.entity_type_id
    WHERE cc.depth < p_max_depth
)
SELECT
    cc.depth,
    cc.entity_id,
    cc.entity_name,
    cc.entity_type,
    cc.controlled_by_entity_id,
    cc.controlled_by_name,
    cc.control_type,
    cc.voting_pct,
    -- Is ultimate controller if no one controls this entity
    NOT EXISTS (
        SELECT 1 FROM kyc.holding_control_links hcl2
        WHERE hcl2.issuer_entity_id = cc.entity_id
          AND hcl2.control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE')
    ) AS is_ultimate_controller
FROM control_chain cc
ORDER BY cc.depth;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION "ob-poc".fn_manco_group_control_chain IS
    'Trace the shareholding control chain upward from a controller to find ultimate parent.';

-- =============================================================================
-- 12. FUNCTION: Derive CBU groups from governance controller (with MANCO_ROLE fallback)
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fn_derive_cbu_groups(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    groups_created INTEGER,
    memberships_created INTEGER
) AS $$
DECLARE
    v_groups_created INTEGER := 0;
    v_memberships_created INTEGER := 0;
BEGIN
    -- Create temp table for chosen anchors (avoids repeating CTE)
    CREATE TEMP TABLE IF NOT EXISTS _chosen_anchors ON COMMIT DROP AS
    WITH cbu_issuer AS (
        -- Get issuer entity from SICAV role (the fund entity), or share_classes, or commercial_client
        SELECT
            c.cbu_id,
            c.jurisdiction,
            COALESCE(
                -- Priority 1: SICAV role = the fund entity that is the issuer
                (SELECT cer.entity_id FROM "ob-poc".cbu_entity_roles cer
                 JOIN "ob-poc".roles r ON r.role_id = cer.role_id AND r.name = 'SICAV'
                 WHERE cer.cbu_id = c.cbu_id
                   AND (cer.effective_to IS NULL OR cer.effective_to > p_as_of_date)
                 ORDER BY cer.effective_from DESC LIMIT 1),
                -- Priority 2: share_class issuer_entity_id
                (SELECT sc.issuer_entity_id FROM kyc.share_classes sc
                 WHERE sc.cbu_id = c.cbu_id AND sc.issuer_entity_id IS NOT NULL
                 ORDER BY sc.issuer_entity_id LIMIT 1),
                -- Priority 3: commercial_client_entity_id (fallback)
                c.commercial_client_entity_id
            ) AS issuer_entity_id
        FROM "ob-poc".cbus c
    ),
    computed_controller AS (
        SELECT
            ci.cbu_id,
            ci.jurisdiction,
            pc.governance_controller_entity_id AS anchor_entity_id,
            'GOVERNANCE_CONTROLLER'::varchar AS source,
            1 AS precedence
        FROM cbu_issuer ci
        JOIN LATERAL kyc.fn_primary_governance_controller(ci.issuer_entity_id, p_as_of_date) pc ON true
        WHERE pc.governance_controller_entity_id IS NOT NULL
          AND pc.basis <> 'NONE'
    ),
    manco_role AS (
        SELECT
            cer.cbu_id,
            c.jurisdiction,
            cer.entity_id AS anchor_entity_id,
            'MANCO_ROLE'::varchar AS source,
            2 AS precedence
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".roles r ON r.role_id = cer.role_id
        JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
        WHERE r.name = 'MANAGEMENT_COMPANY'
          AND (cer.effective_to IS NULL OR cer.effective_to > p_as_of_date)
    ),
    candidates AS (
        SELECT * FROM computed_controller
        UNION ALL
        SELECT * FROM manco_role
    )
    SELECT DISTINCT ON (cbu_id)
        cbu_id,
        jurisdiction,
        anchor_entity_id,
        source
    FROM candidates
    ORDER BY cbu_id, precedence ASC, anchor_entity_id ASC;

    -- Create groups (set-based)
    -- Use DISTINCT ON to ensure one group_type per anchor+jurisdiction (prefer GOVERNANCE_CONTROLLER)
    WITH desired_groups AS (
        SELECT DISTINCT ON (ch.anchor_entity_id, ch.jurisdiction)
            ch.anchor_entity_id,
            ch.jurisdiction,
            CASE WHEN ch.source = 'GOVERNANCE_CONTROLLER' THEN 'GOVERNANCE_BOOK' ELSE 'MANCO_BOOK' END AS group_type
        FROM _chosen_anchors ch
        ORDER BY ch.anchor_entity_id, ch.jurisdiction,
                 CASE WHEN ch.source = 'GOVERNANCE_CONTROLLER' THEN 1 ELSE 2 END
    )
    INSERT INTO "ob-poc".cbu_groups (
        manco_entity_id,
        group_name,
        group_code,
        group_type,
        jurisdiction,
        is_auto_derived,
        effective_from
    )
    SELECT
        dg.anchor_entity_id,
        e.name || ' Book',
        UPPER(REPLACE(SUBSTRING(e.name FROM 1 FOR 20), ' ', '_')) || COALESCE('_' || dg.jurisdiction, ''),
        dg.group_type,
        dg.jurisdiction,
        true,
        p_as_of_date
    FROM desired_groups dg
    JOIN "ob-poc".entities e ON e.entity_id = dg.anchor_entity_id
    ON CONFLICT (manco_entity_id, jurisdiction, effective_to)
    DO UPDATE SET updated_at = now();

    GET DIAGNOSTICS v_groups_created = ROW_COUNT;

    -- Close prior active memberships for these CBUs if they point elsewhere
    WITH chosen_groups AS (
        SELECT
            ch.cbu_id,
            ch.source,
            g.group_id
        FROM _chosen_anchors ch
        JOIN "ob-poc".cbu_groups g
          ON g.manco_entity_id = ch.anchor_entity_id
         AND (g.jurisdiction = ch.jurisdiction OR (g.jurisdiction IS NULL AND ch.jurisdiction IS NULL))
         AND g.effective_to IS NULL
    )
    UPDATE "ob-poc".cbu_group_members gm
    SET effective_to = p_as_of_date
    FROM chosen_groups cg
    WHERE gm.cbu_id = cg.cbu_id
      AND gm.effective_to IS NULL
      AND gm.group_id <> cg.group_id;

    -- Insert/refresh active membership (update source if changed)
    INSERT INTO "ob-poc".cbu_group_members (group_id, cbu_id, source, effective_from)
    SELECT
        g.group_id,
        ch.cbu_id,
        ch.source,
        p_as_of_date
    FROM _chosen_anchors ch
    JOIN "ob-poc".cbu_groups g
      ON g.manco_entity_id = ch.anchor_entity_id
     AND (g.jurisdiction = ch.jurisdiction OR (g.jurisdiction IS NULL AND ch.jurisdiction IS NULL))
     AND g.effective_to IS NULL
    ON CONFLICT (group_id, cbu_id, effective_to)
    DO UPDATE SET source = EXCLUDED.source
    WHERE cbu_group_members.source <> EXCLUDED.source;

    GET DIAGNOSTICS v_memberships_created = ROW_COUNT;

    DROP TABLE IF EXISTS _chosen_anchors;

    RETURN QUERY SELECT v_groups_created, v_memberships_created;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".fn_derive_cbu_groups IS
    'Auto-derive CBU groups using governance controller (board appointment/control) with fallback to MANAGEMENT_COMPANY role.';

COMMIT;

-- =============================================================================
-- USAGE EXAMPLES
-- =============================================================================
/*
-- 1. Derive CBU groups from governance controllers (with MANCO_ROLE fallback)
SELECT * FROM "ob-poc".fn_derive_cbu_groups();

-- 2. Compute control links from holdings
SELECT kyc.fn_compute_control_links(NULL, CURRENT_DATE);

-- 3. View all governance controller groups with CBU counts
SELECT * FROM "ob-poc".v_manco_group_summary;

-- 4. Get all CBUs for a specific controller
SELECT * FROM "ob-poc".fn_get_manco_group_cbus('controller-entity-uuid-here');

-- 5. Find which controller manages a CBU
SELECT * FROM "ob-poc".fn_get_cbu_manco('cbu-uuid-here');

-- 6. Get control chain for a controller group (who controls the controller?)
SELECT * FROM "ob-poc".fn_manco_group_control_chain('controller-entity-uuid-here');

-- 7. Get primary governance controller for an issuer
SELECT * FROM kyc.fn_primary_governance_controller('issuer-entity-uuid-here');

-- 8. View all CBUs by controller with controlling shareholders
SELECT * FROM "ob-poc".v_cbus_by_manco
WHERE manco_name ILIKE '%Allianz%'
ORDER BY manco_name, cbu_name;
*/
-- Migration 041: Governance Controller Bridge Functions
--
-- Purpose: Bridge existing data sources into the governance controller model
--
-- Bridges:
--   1. MANCO_ROLE → special_rights BOARD_APPOINTMENT (immediate - we have 525 roles)
--   2. GLEIF IS_FUND_MANAGED_BY → special_rights BOARD_APPOINTMENT (when relationships imported)
--   3. BODS ownership → kyc.holdings (when BODS data imported)
--
-- All bridges are idempotent (safe to re-run)

BEGIN;

-- =============================================================================
-- 1. BRIDGE: MANAGEMENT_COMPANY role → BOARD_APPOINTMENT special rights
-- =============================================================================
-- Rationale: If an entity is assigned MANAGEMENT_COMPANY role on a CBU, they
-- effectively control the fund's governance. This creates a synthetic board
-- appointment right so the governance controller logic fires.

CREATE OR REPLACE FUNCTION kyc.fn_bridge_manco_role_to_board_rights(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    rights_created INTEGER,
    rights_updated INTEGER
) AS $$
DECLARE
    v_created INTEGER := 0;
    v_updated INTEGER := 0;
BEGIN
    -- Insert BOARD_APPOINTMENT rights for MANAGEMENT_COMPANY roles
    -- Issuer = entity with SICAV role (the fund)
    -- Holder = entity with MANAGEMENT_COMPANY role (the ManCo)
    -- Use DISTINCT ON to deduplicate when same manco manages multiple CBUs for same fund
    WITH manco_fund_pairs AS (
        SELECT DISTINCT ON (sicav_cer.entity_id, manco_cer.entity_id)
            manco_cer.cbu_id,
            manco_cer.entity_id AS manco_entity_id,
            sicav_cer.entity_id AS fund_entity_id,
            manco_cer.effective_from,
            manco_cer.effective_to
        FROM "ob-poc".cbu_entity_roles manco_cer
        JOIN "ob-poc".roles manco_r ON manco_r.role_id = manco_cer.role_id AND manco_r.name = 'MANAGEMENT_COMPANY'
        -- Join to SICAV role on same CBU to get the fund entity
        JOIN "ob-poc".cbu_entity_roles sicav_cer ON sicav_cer.cbu_id = manco_cer.cbu_id
        JOIN "ob-poc".roles sicav_r ON sicav_r.role_id = sicav_cer.role_id AND sicav_r.name = 'SICAV'
        WHERE (manco_cer.effective_to IS NULL OR manco_cer.effective_to > p_as_of_date)
          -- Ensure fund and manco are different entities
          AND sicav_cer.entity_id <> manco_cer.entity_id
        ORDER BY sicav_cer.entity_id, manco_cer.entity_id, manco_cer.effective_from
    )
    INSERT INTO kyc.special_rights (
        issuer_entity_id,
        holder_entity_id,
        right_type,
        board_seats,
        effective_from,
        effective_to,
        notes
    )
    SELECT
        mfp.fund_entity_id,
        mfp.manco_entity_id,
        'BOARD_APPOINTMENT',
        1,  -- Default 1 seat for ManCo-derived rights
        COALESCE(mfp.effective_from, p_as_of_date),
        mfp.effective_to,
        'Auto-generated from MANAGEMENT_COMPANY role assignment'
    FROM manco_fund_pairs mfp
    ON CONFLICT (issuer_entity_id, holder_entity_id, right_type, share_class_id)
    WHERE share_class_id IS NULL
    DO UPDATE SET
        effective_to = EXCLUDED.effective_to
    WHERE kyc.special_rights.effective_to IS DISTINCT FROM EXCLUDED.effective_to;

    GET DIAGNOSTICS v_created = ROW_COUNT;

    RETURN QUERY SELECT v_created, v_updated;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_bridge_manco_role_to_board_rights IS
    'Bridge MANAGEMENT_COMPANY role assignments to BOARD_APPOINTMENT special rights for governance controller.';

-- =============================================================================
-- 2. BRIDGE: GLEIF fund manager relationships → BOARD_APPOINTMENT special rights
-- =============================================================================
-- When GLEIF IS_FUND_MANAGED_BY relationships are imported, bridge them to
-- board appointment rights.

CREATE OR REPLACE FUNCTION kyc.fn_bridge_gleif_fund_manager_to_board_rights(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    rights_created INTEGER,
    rights_updated INTEGER
) AS $$
DECLARE
    v_created INTEGER := 0;
    v_updated INTEGER := 0;
BEGIN
    -- Bridge from entity_parent_relationships where relationship_type indicates fund management
    WITH gleif_fund_managers AS (
        SELECT
            epr.child_entity_id AS fund_entity_id,  -- The fund
            epr.parent_entity_id AS manager_entity_id,  -- The manager
            epr.created_at::date AS effective_from
        FROM "ob-poc".entity_parent_relationships epr
        WHERE epr.relationship_type IN ('IS_FUND_MANAGED_BY', 'IS_FUND-MANAGED_BY', 'FUND_MANAGER')
          AND epr.parent_entity_id IS NOT NULL
          AND epr.source = 'GLEIF'
    )
    INSERT INTO kyc.special_rights (
        issuer_entity_id,
        holder_entity_id,
        right_type,
        board_seats,
        effective_from,
        notes
    )
    SELECT
        gfm.fund_entity_id,
        gfm.manager_entity_id,
        'BOARD_APPOINTMENT',
        1,
        COALESCE(gfm.effective_from, p_as_of_date),
        'Auto-generated from GLEIF IS_FUND_MANAGED_BY relationship'
    FROM gleif_fund_managers gfm
    WHERE gfm.fund_entity_id IS NOT NULL
      AND gfm.manager_entity_id IS NOT NULL
    ON CONFLICT (issuer_entity_id, holder_entity_id, right_type, share_class_id)
    WHERE share_class_id IS NULL
    DO NOTHING;

    GET DIAGNOSTICS v_created = ROW_COUNT;

    RETURN QUERY SELECT v_created, v_updated;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_bridge_gleif_fund_manager_to_board_rights IS
    'Bridge GLEIF IS_FUND_MANAGED_BY relationships to BOARD_APPOINTMENT special rights.';

-- =============================================================================
-- 3. BRIDGE: BODS ownership statements → kyc.holdings
-- =============================================================================
-- When BODS data is imported, convert ownership statements into holdings.
-- This enables the governance controller to compute control from actual ownership %.

CREATE OR REPLACE FUNCTION kyc.fn_bridge_bods_to_holdings(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    holdings_created INTEGER,
    holdings_updated INTEGER,
    entities_linked INTEGER
) AS $$
DECLARE
    v_created INTEGER := 0;
    v_updated INTEGER := 0;
    v_linked INTEGER := 0;
BEGIN
    -- First, link BODS entity statements to our entities via LEI
    INSERT INTO "ob-poc".entity_bods_links (entity_id, bods_entity_statement_id, match_method, match_confidence)
    SELECT DISTINCT
        elc.entity_id,
        bes.statement_id,
        'LEI',
        1.0
    FROM "ob-poc".bods_entity_statements bes
    JOIN "ob-poc".entity_limited_companies elc ON elc.lei = bes.lei
    WHERE bes.lei IS NOT NULL
    ON CONFLICT (entity_id, bods_entity_statement_id) DO NOTHING;

    GET DIAGNOSTICS v_linked = ROW_COUNT;

    -- Now bridge BODS ownership statements to holdings
    -- This requires:
    --   1. Subject entity linked to our entities
    --   2. Interested party entity linked to our entities
    --   3. A share class exists for the subject entity
    WITH bods_ownership AS (
        SELECT
            bos.statement_id,
            subj_link.entity_id AS issuer_entity_id,
            party_link.entity_id AS holder_entity_id,
            COALESCE(bos.share_exact, (bos.share_min + bos.share_max) / 2) AS ownership_pct,
            bos.share_min,
            bos.share_max,
            bos.is_direct,
            bos.start_date,
            bos.end_date
        FROM "ob-poc".bods_ownership_statements bos
        -- Link subject to our entity
        JOIN "ob-poc".entity_bods_links subj_link
            ON subj_link.bods_entity_statement_id = bos.subject_entity_statement_id
        -- Link interested party to our entity
        JOIN "ob-poc".entity_bods_links party_link
            ON party_link.bods_entity_statement_id = bos.interested_party_statement_id
        WHERE bos.ownership_type IN ('shareholding', 'voting-rights', 'ownership-of-shares')
          AND (bos.end_date IS NULL OR bos.end_date > p_as_of_date)
    ),
    -- Find or create default share class for each issuer
    issuer_share_classes AS (
        SELECT DISTINCT ON (bo.issuer_entity_id)
            bo.issuer_entity_id,
            sc.id AS share_class_id,
            COALESCE(scs.outstanding_units, scs.issued_units, 1000000) AS total_units
        FROM bods_ownership bo
        JOIN kyc.share_classes sc ON sc.issuer_entity_id = bo.issuer_entity_id
        LEFT JOIN LATERAL (
            SELECT * FROM kyc.share_class_supply scs
            WHERE scs.share_class_id = sc.id
            ORDER BY scs.as_of_date DESC
            LIMIT 1
        ) scs ON true
        ORDER BY bo.issuer_entity_id, sc.created_at
    )
    INSERT INTO kyc.holdings (
        share_class_id,
        investor_entity_id,
        units,
        cost_basis,
        acquisition_date,
        status,
        source,
        notes
    )
    SELECT
        isc.share_class_id,
        bo.holder_entity_id,
        -- Convert ownership % to units based on total supply
        (bo.ownership_pct / 100.0) * isc.total_units,
        0,  -- No cost basis from BODS
        COALESCE(bo.start_date, p_as_of_date),
        'active',
        'BODS_BRIDGE',
        'Auto-generated from BODS ownership statement ' || bo.statement_id
    FROM bods_ownership bo
    JOIN issuer_share_classes isc ON isc.issuer_entity_id = bo.issuer_entity_id
    ON CONFLICT (share_class_id, investor_entity_id)
    DO UPDATE SET
        units = EXCLUDED.units
    WHERE kyc.holdings.units <> EXCLUDED.units;

    GET DIAGNOSTICS v_created = ROW_COUNT;

    RETURN QUERY SELECT v_created, v_updated, v_linked;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_bridge_bods_to_holdings IS
    'Bridge BODS ownership statements to kyc.holdings for governance controller computation.';

-- =============================================================================
-- 4. MASTER BRIDGE: Run all bridges in sequence
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_run_governance_bridges(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    bridge_name TEXT,
    records_affected INTEGER
) AS $$
DECLARE
    v_manco_created INTEGER;
    v_manco_updated INTEGER;
    v_gleif_created INTEGER;
    v_gleif_updated INTEGER;
    v_bods_created INTEGER;
    v_bods_updated INTEGER;
    v_bods_linked INTEGER;
BEGIN
    -- Run ManCo role bridge
    SELECT * INTO v_manco_created, v_manco_updated
    FROM kyc.fn_bridge_manco_role_to_board_rights(p_as_of_date);

    RETURN QUERY SELECT 'manco_role_to_board_rights'::TEXT, v_manco_created;

    -- Run GLEIF fund manager bridge
    SELECT * INTO v_gleif_created, v_gleif_updated
    FROM kyc.fn_bridge_gleif_fund_manager_to_board_rights(p_as_of_date);

    RETURN QUERY SELECT 'gleif_fund_manager_to_board_rights'::TEXT, v_gleif_created;

    -- Run BODS bridge
    SELECT * INTO v_bods_created, v_bods_updated, v_bods_linked
    FROM kyc.fn_bridge_bods_to_holdings(p_as_of_date);

    RETURN QUERY SELECT 'bods_to_holdings'::TEXT, v_bods_created;
    RETURN QUERY SELECT 'bods_entity_links'::TEXT, v_bods_linked;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_run_governance_bridges IS
    'Run all governance bridges: ManCo roles, GLEIF fund managers, BODS ownership.';

-- =============================================================================
-- 5. Add unique constraint to special_rights if missing
-- =============================================================================
-- Need this for ON CONFLICT to work

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_special_rights_holder_issuer_type_class'
    ) THEN
        ALTER TABLE kyc.special_rights
        ADD CONSTRAINT uq_special_rights_holder_issuer_type_class
        UNIQUE NULLS NOT DISTINCT (issuer_entity_id, holder_entity_id, right_type, share_class_id);
    END IF;
END $$;

COMMIT;

-- =============================================================================
-- USAGE
-- =============================================================================
/*
-- Run all bridges (idempotent - safe to re-run)
SELECT * FROM kyc.fn_run_governance_bridges();

-- Then derive CBU groups (now governance controller should fire)
SELECT * FROM "ob-poc".fn_derive_cbu_groups();

-- Check results
SELECT * FROM "ob-poc".v_manco_group_summary;

-- Verify special_rights were created
SELECT notes, COUNT(*) FROM kyc.special_rights GROUP BY notes;
*/
-- Migration 042: Seed ManCo Holdings for Governance Controller Testing
--
-- Purpose: Create holdings showing ManCo ownership of SICAV share classes
-- This enables the governance controller to detect board appointment rights via ownership
--
-- Idempotent: Uses ON CONFLICT DO NOTHING

BEGIN;

-- Create holdings showing Allianz Global Investors GmbH owns 100% of each SICAV's default share class
-- This gives the ManCo voting control which triggers board appointment right detection

INSERT INTO kyc.holdings (
    id,
    share_class_id,
    investor_entity_id,
    units,
    status,
    acquisition_date,
    created_at,
    updated_at
)
SELECT
    gen_random_uuid(),
    sc.id AS share_class_id,
    '4f463925-53f4-4a71-aabe-65584074db6b'::uuid AS investor_entity_id,  -- Allianz Global Investors GmbH
    1000000 AS units,  -- Match the supply we created (100% ownership)
    'active' AS status,
    CURRENT_DATE AS acquisition_date,
    NOW() AS created_at,
    NOW() AS updated_at
FROM kyc.share_classes sc
WHERE sc.name LIKE '%Default Class%'
  AND sc.issuer_entity_id IN (
      -- Get SICAV entities from Allianz CBUs
      SELECT DISTINCT cer.entity_id
      FROM "ob-poc".cbu_entity_roles cer
      JOIN "ob-poc".roles r ON r.role_id = cer.role_id AND r.name = 'SICAV'
      JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
      JOIN "ob-poc".entities apex ON apex.entity_id = c.commercial_client_entity_id
      WHERE apex.name ILIKE '%allianz%'
  )
ON CONFLICT (share_class_id, investor_entity_id) DO NOTHING;

COMMIT;

-- Verify holdings were created
-- SELECT COUNT(*) as holdings_count, investor_entity_id
-- FROM kyc.holdings
-- WHERE investor_entity_id = '4f463925-53f4-4a71-aabe-65584074db6b'
-- GROUP BY investor_entity_id;
-- Migration 043: Feedback loop promotion infrastructure
-- Adds success tracking, quality gates, and collision detection for pattern learning
--
-- Created: 2026-01-21
-- Purpose: Implement staged promotion pipeline with quality guardrails

-- ============================================================================
-- 1. Add success tracking columns to learning_candidates
-- ============================================================================

ALTER TABLE agent.learning_candidates
ADD COLUMN IF NOT EXISTS success_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS total_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS last_success_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS domain_hint TEXT,
ADD COLUMN IF NOT EXISTS collision_safe BOOLEAN,
ADD COLUMN IF NOT EXISTS collision_check_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS collision_verb TEXT;

COMMENT ON COLUMN agent.learning_candidates.success_count IS
    'Count of successful outcomes (executed + DSL succeeded, or user selected this verb)';
COMMENT ON COLUMN agent.learning_candidates.total_count IS
    'Total signals received (success + failure)';
COMMENT ON COLUMN agent.learning_candidates.collision_safe IS
    'Whether pattern passed semantic collision check (NULL = not checked)';
COMMENT ON COLUMN agent.learning_candidates.collision_verb IS
    'If collision detected, which verb it conflicts with';

-- ============================================================================
-- 2. Stopwords table for quality filtering
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent.stopwords (
    word TEXT PRIMARY KEY,
    category TEXT DEFAULT 'generic'  -- 'generic', 'polite', 'filler'
);

COMMENT ON TABLE agent.stopwords IS 'Common words that should not dominate learning patterns';

-- Seed common stopwords
INSERT INTO agent.stopwords (word, category) VALUES
    ('the', 'generic'), ('a', 'generic'), ('an', 'generic'),
    ('please', 'polite'), ('can', 'polite'), ('could', 'polite'),
    ('you', 'polite'), ('would', 'polite'), ('help', 'polite'),
    ('me', 'generic'), ('i', 'generic'), ('my', 'generic'),
    ('want', 'filler'), ('need', 'filler'), ('like', 'filler'),
    ('to', 'generic'), ('for', 'generic'), ('with', 'generic'),
    ('this', 'generic'), ('that', 'generic'), ('it', 'generic'),
    ('do', 'filler'), ('make', 'filler'), ('get', 'filler'),
    ('just', 'filler'), ('now', 'filler'), ('here', 'filler'),
    ('of', 'generic'), ('in', 'generic'), ('on', 'generic'),
    ('at', 'generic'), ('by', 'generic'), ('is', 'generic'),
    ('are', 'generic'), ('was', 'generic'), ('be', 'generic'),
    ('have', 'generic'), ('has', 'generic'), ('had', 'generic'),
    ('show', 'filler'), ('give', 'filler'), ('tell', 'filler')
ON CONFLICT (word) DO NOTHING;

-- ============================================================================
-- 3. Function: Record learning signal with success tracking
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.record_learning_signal(
    p_phrase TEXT,
    p_verb TEXT,
    p_is_success BOOLEAN,
    p_signal_type TEXT,  -- 'executed', 'selected_alt', 'corrected'
    p_domain_hint TEXT DEFAULT NULL
) RETURNS BIGINT AS $$
DECLARE
    v_normalized TEXT;
    v_fingerprint TEXT;
    v_word_count INT;
    v_stopword_ratio REAL;
    v_id BIGINT;
BEGIN
    -- Normalize phrase
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));
    v_fingerprint := md5(v_normalized || '|' || p_verb);

    -- Quality gate: word count (3-15 words)
    v_word_count := array_length(string_to_array(v_normalized, ' '), 1);
    IF v_word_count IS NULL OR v_word_count < 3 OR v_word_count > 15 THEN
        RETURN NULL;  -- Reject too short or too long
    END IF;

    -- Quality gate: stopword ratio (reject if >70% stopwords)
    SELECT
        COALESCE(COUNT(*) FILTER (WHERE s.word IS NOT NULL)::real / NULLIF(v_word_count, 0), 0)
    INTO v_stopword_ratio
    FROM unnest(string_to_array(v_normalized, ' ')) AS w(word)
    LEFT JOIN agent.stopwords s ON s.word = w.word;

    IF v_stopword_ratio > 0.70 THEN
        RETURN NULL;  -- Reject (too generic)
    END IF;

    -- Upsert candidate
    INSERT INTO agent.learning_candidates (
        fingerprint,
        learning_type,
        input_pattern,
        suggested_output,
        occurrence_count,
        success_count,
        total_count,
        first_seen,
        last_seen,
        last_success_at,
        domain_hint,
        status
    ) VALUES (
        v_fingerprint,
        'invocation_phrase',
        v_normalized,
        p_verb,
        1,
        CASE WHEN p_is_success THEN 1 ELSE 0 END,
        1,
        NOW(),
        NOW(),
        CASE WHEN p_is_success THEN NOW() ELSE NULL END,
        COALESCE(p_domain_hint, (
            SELECT category FROM "ob-poc".dsl_verbs WHERE full_name = p_verb
        )),
        'pending'
    )
    ON CONFLICT (fingerprint) DO UPDATE SET
        occurrence_count = agent.learning_candidates.occurrence_count + 1,
        success_count = agent.learning_candidates.success_count +
            CASE WHEN p_is_success THEN 1 ELSE 0 END,
        total_count = agent.learning_candidates.total_count + 1,
        last_seen = NOW(),
        last_success_at = CASE
            WHEN p_is_success THEN NOW()
            ELSE agent.learning_candidates.last_success_at
        END,
        -- Reset collision check if we get new signals (may have changed)
        collision_safe = NULL,
        collision_check_at = NULL
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.record_learning_signal IS
    'Record a learning signal with quality gates. Returns NULL if phrase rejected.';

-- ============================================================================
-- 4. Function: Check collision with existing patterns (basic, semantic done in Rust)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.check_pattern_collision_basic(
    p_candidate_id BIGINT
) RETURNS BOOLEAN AS $$
DECLARE
    v_phrase TEXT;
    v_verb TEXT;
BEGIN
    -- Get candidate details
    SELECT input_pattern, suggested_output
    INTO v_phrase, v_verb
    FROM agent.learning_candidates
    WHERE id = p_candidate_id;

    IF v_phrase IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Check if phrase already exists for this verb (exact match)
    IF EXISTS (
        SELECT 1 FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_name = v_verb
          AND pattern_normalized = v_phrase
    ) THEN
        -- Already a pattern, mark as duplicate
        UPDATE agent.learning_candidates
        SET status = 'duplicate',
            collision_safe = FALSE,
            collision_check_at = NOW()
        WHERE id = p_candidate_id;
        RETURN FALSE;
    END IF;

    -- Basic check passed; semantic check done in Rust
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 5. Function: Get promotable candidates
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_promotable_candidates(
    p_min_occurrences INT DEFAULT 5,
    p_min_success_rate REAL DEFAULT 0.80,
    p_min_age_hours INT DEFAULT 24,
    p_limit INT DEFAULT 50
)
RETURNS TABLE (
    id BIGINT,
    phrase TEXT,
    verb TEXT,
    occurrence_count INT,
    success_count INT,
    total_count INT,
    success_rate REAL,
    domain_hint TEXT,
    first_seen TIMESTAMPTZ,
    age_hours REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        lc.id,
        lc.input_pattern as phrase,
        lc.suggested_output as verb,
        lc.occurrence_count,
        lc.success_count,
        lc.total_count,
        (lc.success_count::real / NULLIF(lc.total_count, 0))::real as success_rate,
        lc.domain_hint,
        lc.first_seen,
        (EXTRACT(EPOCH FROM (NOW() - lc.first_seen)) / 3600)::real as age_hours
    FROM agent.learning_candidates lc
    WHERE lc.status = 'pending'
      AND lc.learning_type = 'invocation_phrase'
      -- Occurrence threshold
      AND lc.occurrence_count >= p_min_occurrences
      -- Success rate threshold (avoid division by zero)
      AND lc.total_count > 0
      AND (lc.success_count::real / lc.total_count) >= p_min_success_rate
      -- Age threshold (cool-down)
      AND lc.first_seen < NOW() - make_interval(hours => p_min_age_hours)
      -- Collision check passed (or not yet checked - Rust will check)
      AND (lc.collision_safe IS NULL OR lc.collision_safe = TRUE)
      -- Not blocklisted
      AND NOT EXISTS (
          SELECT 1 FROM agent.phrase_blocklist bl
          WHERE bl.blocked_verb = lc.suggested_output
            AND lower(bl.phrase) = lc.input_pattern
            AND (bl.expires_at IS NULL OR bl.expires_at > NOW())
      )
    ORDER BY
        lc.occurrence_count DESC,
        (lc.success_count::real / NULLIF(lc.total_count, 0)) DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.get_promotable_candidates IS
    'Get candidates ready for automatic promotion (meet all quality thresholds)';

-- ============================================================================
-- 6. Function: Get candidates needing manual review
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_review_candidates(
    p_min_occurrences INT DEFAULT 3,
    p_min_age_days INT DEFAULT 7,
    p_limit INT DEFAULT 100
)
RETURNS TABLE (
    id BIGINT,
    phrase TEXT,
    verb TEXT,
    occurrence_count INT,
    success_count INT,
    total_count INT,
    success_rate REAL,
    domain_hint TEXT,
    first_seen TIMESTAMPTZ,
    last_seen TIMESTAMPTZ,
    collision_verb TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        lc.id,
        lc.input_pattern as phrase,
        lc.suggested_output as verb,
        lc.occurrence_count,
        lc.success_count,
        lc.total_count,
        CASE WHEN lc.total_count > 0
             THEN (lc.success_count::real / lc.total_count)
             ELSE 0 END as success_rate,
        lc.domain_hint,
        lc.first_seen,
        lc.last_seen,
        lc.collision_verb
    FROM agent.learning_candidates lc
    WHERE lc.status = 'pending'
      AND lc.learning_type = 'invocation_phrase'
      AND lc.occurrence_count >= p_min_occurrences
      AND lc.first_seen < NOW() - make_interval(days => p_min_age_days)
      -- Either failed auto-promotion criteria or collision detected
      AND (
          -- Low success rate
          (lc.total_count > 0 AND (lc.success_count::real / lc.total_count) < 0.80)
          -- Collision detected
          OR lc.collision_safe = FALSE
          -- Not enough occurrences for auto-promote but old enough for review
          OR lc.occurrence_count < 5
      )
    ORDER BY lc.occurrence_count DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.get_review_candidates IS
    'Get candidates that need manual review (failed auto-promotion but have signal)';

-- ============================================================================
-- 7. Function: Apply promotion (with audit)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.apply_promotion(
    p_candidate_id BIGINT,
    p_actor TEXT DEFAULT 'system_auto'
) RETURNS BOOLEAN AS $$
DECLARE
    v_phrase TEXT;
    v_verb TEXT;
    v_added BOOLEAN;
BEGIN
    -- Get candidate
    SELECT input_pattern, suggested_output
    INTO v_phrase, v_verb
    FROM agent.learning_candidates
    WHERE id = p_candidate_id
      AND status = 'pending';

    IF v_phrase IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Add to dsl_verbs.intent_patterns using existing function
    SELECT "ob-poc".add_learned_pattern(v_verb, v_phrase) INTO v_added;

    IF v_added THEN
        -- Update candidate status
        UPDATE agent.learning_candidates
        SET status = 'applied',
            applied_at = NOW()
        WHERE id = p_candidate_id;

        -- Audit log
        INSERT INTO agent.learning_audit (
            action, learning_type, candidate_id, actor, details
        ) VALUES (
            'applied',
            'invocation_phrase',
            p_candidate_id,
            p_actor,
            jsonb_build_object(
                'phrase', v_phrase,
                'verb', v_verb
            )
        );

        RETURN TRUE;
    END IF;

    RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.apply_promotion IS
    'Promote a learning candidate to dsl_verbs.intent_patterns with audit trail';

-- ============================================================================
-- 8. Function: Reject a candidate (add to blocklist)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.reject_candidate(
    p_candidate_id BIGINT,
    p_reason TEXT,
    p_actor TEXT DEFAULT 'manual_review'
) RETURNS BOOLEAN AS $$
DECLARE
    v_phrase TEXT;
    v_verb TEXT;
BEGIN
    -- Get candidate
    SELECT input_pattern, suggested_output
    INTO v_phrase, v_verb
    FROM agent.learning_candidates
    WHERE id = p_candidate_id
      AND status = 'pending';

    IF v_phrase IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Add to blocklist
    INSERT INTO agent.phrase_blocklist (phrase, blocked_verb, reason)
    VALUES (v_phrase, v_verb, p_reason)
    ON CONFLICT DO NOTHING;

    -- Update candidate status
    UPDATE agent.learning_candidates
    SET status = 'rejected',
        reviewed_by = p_actor,
        reviewed_at = NOW()
    WHERE id = p_candidate_id;

    -- Audit log
    INSERT INTO agent.learning_audit (
        action, learning_type, candidate_id, actor, details
    ) VALUES (
        'rejected',
        'invocation_phrase',
        p_candidate_id,
        p_actor,
        jsonb_build_object(
            'phrase', v_phrase,
            'verb', v_verb,
            'reason', p_reason
        )
    );

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.reject_candidate IS
    'Reject a learning candidate and add to blocklist';

-- ============================================================================
-- 9. Function: Expire pending outcomes
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.expire_pending_outcomes(
    p_older_than_minutes INT DEFAULT 30
) RETURNS INT AS $$
DECLARE
    v_count INT;
BEGIN
    UPDATE "ob-poc".intent_feedback
    SET outcome = 'abandoned'
    WHERE outcome IS NULL
      AND created_at < NOW() - make_interval(mins => p_older_than_minutes);

    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.expire_pending_outcomes IS
    'Mark stale pending outcomes as abandoned (no user action after N minutes)';

-- ============================================================================
-- 10. Metrics views
-- ============================================================================

-- Weekly learning health dashboard
CREATE OR REPLACE VIEW agent.v_learning_health_weekly AS
SELECT
    DATE_TRUNC('week', f.created_at) as week,
    COUNT(*) as total_interactions,
    COUNT(*) FILTER (WHERE v.learning_signal = 'success') as successes,
    COUNT(*) FILTER (WHERE v.learning_signal IN ('wrong_match', 'correction_needed')) as corrections,
    COUNT(*) FILTER (WHERE v.learning_signal = 'no_match') as no_matches,
    COUNT(*) FILTER (WHERE v.learning_signal = 'false_positive') as false_positives,

    -- Hit rates
    ROUND(100.0 * COUNT(*) FILTER (WHERE v.learning_signal = 'success') /
          NULLIF(COUNT(*), 0), 1) as top1_hit_rate_pct,

    -- Scores
    ROUND(AVG(f.match_score) FILTER (WHERE v.learning_signal = 'success')::numeric, 3) as avg_success_score,
    ROUND(AVG(f.match_score) FILTER (WHERE v.learning_signal IN ('wrong_match', 'correction_needed'))::numeric, 3) as avg_correction_score,

    -- Confidence distribution
    COUNT(*) FILTER (WHERE f.match_confidence = 'high') as high_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'medium') as medium_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'low') as low_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'none') as no_match_confidence

FROM "ob-poc".intent_feedback f
LEFT JOIN "ob-poc".v_learning_feedback v ON v.feedback_id = f.id
WHERE f.created_at > NOW() - INTERVAL '12 weeks'
GROUP BY 1
ORDER BY 1 DESC;

COMMENT ON VIEW agent.v_learning_health_weekly IS
    'Weekly metrics for learning pipeline health monitoring';

-- Candidate pipeline status summary
CREATE OR REPLACE VIEW agent.v_candidate_pipeline AS
SELECT
    status,
    COUNT(*) as count,
    ROUND(AVG(occurrence_count)::numeric, 1) as avg_occurrences,
    ROUND(AVG(CASE WHEN total_count > 0
              THEN success_count::real / total_count
              ELSE 0 END)::numeric, 2) as avg_success_rate,
    MIN(first_seen) as oldest,
    MAX(last_seen) as newest
FROM agent.learning_candidates
WHERE learning_type = 'invocation_phrase'
GROUP BY status
ORDER BY count DESC;

COMMENT ON VIEW agent.v_candidate_pipeline IS
    'Summary of candidate statuses in the learning pipeline';

-- Top pending candidates (for quick review)
CREATE OR REPLACE VIEW agent.v_top_pending_candidates AS
SELECT
    id,
    input_pattern as phrase,
    suggested_output as verb,
    occurrence_count,
    success_count,
    total_count,
    CASE WHEN total_count > 0
         THEN ROUND((success_count::real / total_count)::numeric, 2)
         ELSE 0 END as success_rate,
    domain_hint,
    collision_safe,
    collision_verb,
    first_seen,
    last_seen,
    EXTRACT(DAY FROM (NOW() - first_seen))::int as age_days
FROM agent.learning_candidates
WHERE status = 'pending'
  AND learning_type = 'invocation_phrase'
ORDER BY occurrence_count DESC, success_rate DESC
LIMIT 100;

COMMENT ON VIEW agent.v_top_pending_candidates IS
    'Top 100 pending candidates by occurrence count';

-- ============================================================================
-- 11. Indexes for performance
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_learning_candidates_promotable
ON agent.learning_candidates(occurrence_count DESC, status)
WHERE status = 'pending' AND learning_type = 'invocation_phrase';

CREATE INDEX IF NOT EXISTS idx_learning_candidates_fingerprint
ON agent.learning_candidates(fingerprint);

CREATE INDEX IF NOT EXISTS idx_learning_candidates_success_tracking
ON agent.learning_candidates(total_count, success_count)
WHERE status = 'pending';

-- ============================================================================
-- 12. Grant permissions (if using role-based access)
-- ============================================================================

-- Ensure functions are accessible
GRANT EXECUTE ON FUNCTION agent.record_learning_signal TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.get_promotable_candidates TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.get_review_candidates TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.apply_promotion TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.reject_candidate TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.expire_pending_outcomes TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.check_pattern_collision_basic TO PUBLIC;

-- Done!
-- Migration 044: Agent teaching mechanism
-- Direct phrase→verb teaching that bypasses candidate staging
--
-- Created: 2026-01-21
-- Purpose: Allow explicit teaching of phrase→verb mappings

-- ============================================================================
-- 1. Teaching function (trusted source, no staging)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.teach_phrase(
    p_phrase TEXT,
    p_verb TEXT,
    p_source TEXT DEFAULT 'direct_teaching'
) RETURNS BOOLEAN AS $$
DECLARE
    v_normalized TEXT;
    v_added BOOLEAN;
    v_word_count INT;
BEGIN
    -- Normalize phrase
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));

    -- Basic validation: not empty
    IF v_normalized = '' OR v_normalized IS NULL THEN
        RAISE EXCEPTION 'Phrase cannot be empty';
    END IF;

    -- Basic validation: verb exists
    IF NOT EXISTS (SELECT 1 FROM "ob-poc".dsl_verbs WHERE full_name = p_verb) THEN
        RAISE EXCEPTION 'Unknown verb: %. Use verbs.list to see available verbs.', p_verb;
    END IF;

    -- Word count check (warn but don't block for teaching)
    v_word_count := array_length(string_to_array(v_normalized, ' '), 1);
    IF v_word_count < 2 THEN
        RAISE WARNING 'Very short phrase (% words) - may cause false positives', v_word_count;
    END IF;

    -- Add to dsl_verbs.intent_patterns using existing function
    SELECT "ob-poc".add_learned_pattern(p_verb, v_normalized) INTO v_added;

    IF v_added THEN
        -- Audit the teaching
        INSERT INTO agent.learning_audit (
            action,
            learning_type,
            actor,
            details
        ) VALUES (
            'taught',
            'invocation_phrase',
            p_source,
            jsonb_build_object(
                'phrase', v_normalized,
                'verb', p_verb,
                'word_count', v_word_count
            )
        );
    END IF;

    RETURN v_added;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.teach_phrase IS
    'Directly teach a phrase→verb mapping. Bypasses candidate staging (trusted source).';

-- ============================================================================
-- 2. Batch teaching function
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.teach_phrases_batch(
    p_phrases JSONB,  -- Array of {phrase, verb} objects
    p_source TEXT DEFAULT 'batch_teaching'
) RETURNS TABLE (
    phrase TEXT,
    verb TEXT,
    success BOOLEAN,
    message TEXT
) AS $$
DECLARE
    v_item JSONB;
    v_phrase TEXT;
    v_verb TEXT;
    v_added BOOLEAN;
BEGIN
    FOR v_item IN SELECT * FROM jsonb_array_elements(p_phrases)
    LOOP
        v_phrase := v_item->>'phrase';
        v_verb := v_item->>'verb';

        BEGIN
            SELECT agent.teach_phrase(v_phrase, v_verb, p_source) INTO v_added;

            phrase := v_phrase;
            verb := v_verb;
            success := v_added;
            message := CASE
                WHEN v_added THEN 'Learned'
                ELSE 'Already exists'
            END;
            RETURN NEXT;

        EXCEPTION WHEN OTHERS THEN
            phrase := v_phrase;
            verb := v_verb;
            success := FALSE;
            message := SQLERRM;
            RETURN NEXT;
        END;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.teach_phrases_batch IS
    'Batch teach multiple phrase→verb mappings from JSON array.';

-- ============================================================================
-- 3. Function: Unteach a pattern (with audit)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.unteach_phrase(
    p_phrase TEXT,
    p_verb TEXT,
    p_reason TEXT DEFAULT NULL,
    p_actor TEXT DEFAULT 'manual'
) RETURNS BOOLEAN AS $$
DECLARE
    v_normalized TEXT;
    v_removed BOOLEAN := FALSE;
BEGIN
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));

    -- Remove from dsl_verbs.intent_patterns
    UPDATE "ob-poc".dsl_verbs
    SET intent_patterns = array_remove(intent_patterns, v_normalized),
        updated_at = NOW()
    WHERE full_name = p_verb
      AND v_normalized = ANY(intent_patterns);

    v_removed := FOUND;

    IF v_removed THEN
        -- Remove from embeddings cache
        DELETE FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_name = p_verb
          AND pattern_normalized = v_normalized;

        -- Audit
        INSERT INTO agent.learning_audit (
            action, learning_type, actor, details
        ) VALUES (
            'untaught',
            'invocation_phrase',
            p_actor,
            jsonb_build_object(
                'phrase', v_normalized,
                'verb', p_verb,
                'reason', p_reason
            )
        );
    END IF;

    RETURN v_removed;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.unteach_phrase IS
    'Remove a taught pattern (with audit). Use when a pattern causes problems.';

-- ============================================================================
-- 4. View: Recently taught patterns
-- ============================================================================

CREATE OR REPLACE VIEW agent.v_recently_taught AS
SELECT
    la.id,
    la.details->>'phrase' as phrase,
    la.details->>'verb' as verb,
    la.actor as source,
    la.timestamp as taught_at,
    -- Check if embedding exists yet
    EXISTS (
        SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
        WHERE vpe.verb_name = la.details->>'verb'
          AND vpe.pattern_normalized = la.details->>'phrase'
          AND vpe.embedding IS NOT NULL
    ) as has_embedding
FROM agent.learning_audit la
WHERE la.action = 'taught'
  AND la.learning_type = 'invocation_phrase'
ORDER BY la.timestamp DESC
LIMIT 100;

COMMENT ON VIEW agent.v_recently_taught IS
    'Recently taught patterns with embedding status. Run populate_embeddings to activate patterns without embeddings.';

-- ============================================================================
-- 5. Function: Get patterns pending embedding
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_taught_pending_embeddings()
RETURNS TABLE (
    verb TEXT,
    phrase TEXT,
    taught_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        la.details->>'verb' as verb,
        la.details->>'phrase' as phrase,
        la.timestamp as taught_at
    FROM agent.learning_audit la
    WHERE la.action = 'taught'
      AND la.learning_type = 'invocation_phrase'
      AND NOT EXISTS (
          SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
          WHERE vpe.verb_name = la.details->>'verb'
            AND vpe.pattern_normalized = la.details->>'phrase'
            AND vpe.embedding IS NOT NULL
      )
    ORDER BY la.timestamp DESC;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.get_taught_pending_embeddings IS
    'Get taught patterns that are awaiting populate_embeddings to create vectors.';

-- ============================================================================
-- 6. Teaching stats view
-- ============================================================================

CREATE OR REPLACE VIEW agent.v_teaching_stats AS
SELECT
    DATE_TRUNC('day', la.timestamp) as day,
    la.actor as source,
    COUNT(*) FILTER (WHERE la.action = 'taught') as patterns_taught,
    COUNT(*) FILTER (WHERE la.action = 'untaught') as patterns_untaught,
    COUNT(DISTINCT la.details->>'verb') as verbs_affected
FROM agent.learning_audit la
WHERE la.action IN ('taught', 'untaught')
  AND la.learning_type = 'invocation_phrase'
  AND la.timestamp > NOW() - INTERVAL '30 days'
GROUP BY 1, 2
ORDER BY 1 DESC, 3 DESC;

COMMENT ON VIEW agent.v_teaching_stats IS
    'Teaching activity over the last 30 days by source.';

-- ============================================================================
-- 7. Grant permissions
-- ============================================================================

GRANT EXECUTE ON FUNCTION agent.teach_phrase TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.teach_phrases_batch TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.unteach_phrase TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.get_taught_pending_embeddings TO PUBLIC;
-- Migration: 045_legal_contracts
-- Legal contracts with product-level rate cards
-- Join key: client_label (same as cbus.client_label, entities.client_label)

-- Master contract table
CREATE TABLE IF NOT EXISTS "ob-poc".legal_contracts (
    contract_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_label VARCHAR(100) NOT NULL,
    contract_reference VARCHAR(100),  -- External contract number
    effective_date DATE NOT NULL,
    termination_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (status IN ('DRAFT', 'ACTIVE', 'TERMINATED', 'EXPIRED')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_legal_contracts_client_label
    ON "ob-poc".legal_contracts(client_label);

CREATE INDEX IF NOT EXISTS idx_legal_contracts_status
    ON "ob-poc".legal_contracts(status);

-- Contract products with rate cards
CREATE TABLE IF NOT EXISTS "ob-poc".contract_products (
    contract_id UUID NOT NULL REFERENCES "ob-poc".legal_contracts(contract_id) ON DELETE CASCADE,
    product_code VARCHAR(50) NOT NULL,
    rate_card_id UUID,
    effective_date DATE,
    termination_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (contract_id, product_code)
);

CREATE INDEX IF NOT EXISTS idx_contract_products_product_code
    ON "ob-poc".contract_products(product_code);

-- Rate cards (reference table)
CREATE TABLE IF NOT EXISTS "ob-poc".rate_cards (
    rate_card_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    currency VARCHAR(3) DEFAULT 'USD',
    effective_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- View: contracts with products
CREATE OR REPLACE VIEW "ob-poc".v_contract_summary AS
SELECT
    c.contract_id,
    c.client_label,
    c.contract_reference,
    c.effective_date,
    c.status,
    COUNT(cp.product_code) as product_count,
    ARRAY_AGG(cp.product_code ORDER BY cp.product_code) FILTER (WHERE cp.product_code IS NOT NULL) as products
FROM "ob-poc".legal_contracts c
LEFT JOIN "ob-poc".contract_products cp ON cp.contract_id = c.contract_id
GROUP BY c.contract_id, c.client_label, c.contract_reference, c.effective_date, c.status;

-- CBU subscriptions to contract+product (the onboarding gate)
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_subscriptions (
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    contract_id UUID NOT NULL,
    product_code VARCHAR(50) NOT NULL,
    subscribed_at TIMESTAMPTZ DEFAULT NOW(),
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (status IN ('PENDING', 'ACTIVE', 'SUSPENDED', 'TERMINATED')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (cbu_id, contract_id, product_code),
    FOREIGN KEY (contract_id, product_code) REFERENCES "ob-poc".contract_products(contract_id, product_code) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_cbu_subscriptions_contract
    ON "ob-poc".cbu_subscriptions(contract_id, product_code);

-- View: CBU with subscriptions
CREATE OR REPLACE VIEW "ob-poc".v_cbu_subscriptions AS
SELECT
    s.cbu_id,
    c.name as cbu_name,
    c.client_label,
    lc.contract_id,
    lc.contract_reference,
    s.product_code,
    cp.rate_card_id,
    s.status as subscription_status,
    s.subscribed_at
FROM "ob-poc".cbu_subscriptions s
JOIN "ob-poc".cbus c ON c.cbu_id = s.cbu_id
JOIN "ob-poc".legal_contracts lc ON lc.contract_id = s.contract_id
JOIN "ob-poc".contract_products cp ON cp.contract_id = s.contract_id AND cp.product_code = s.product_code;

-- Seed sample data for Allianz
INSERT INTO "ob-poc".legal_contracts (client_label, contract_reference, effective_date, status)
VALUES ('allianz', 'MSA-ALZ-2020-001', '2020-01-01', 'ACTIVE')
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".legal_contracts (client_label, contract_reference, effective_date, status)
VALUES ('bridgewater', 'MSA-BW-2021-001', '2021-06-01', 'ACTIVE')
ON CONFLICT DO NOTHING;
-- Migration 046: Remove client_label columns
-- 
-- Rationale: client_label was a "magic string" shortcut that doesn't scale.
-- Instead, use proper GROUP entity taxonomy with entity resolution.
-- The session bootstrap flow + trigger phrases handle disambiguation.

BEGIN;

-- Drop dependent views first
DROP VIEW IF EXISTS "ob-poc".entity_search_view CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_cbu_subscriptions CASCADE;

-- Drop client_label from entities
ALTER TABLE "ob-poc".entities DROP COLUMN IF EXISTS client_label;

-- Drop client_label from CBUs  
ALTER TABLE "ob-poc".cbus DROP COLUMN IF EXISTS client_label;

-- Drop any indexes on client_label (may not exist)
DROP INDEX IF EXISTS "ob-poc".idx_entities_client_label;
DROP INDEX IF EXISTS "ob-poc".idx_cbus_client_label;

-- Recreate entity_search_view without client_label
CREATE OR REPLACE VIEW "ob-poc".entity_search_view AS
SELECT 
    entity_id,
    name,
    entity_type_id,
    external_id,
    bods_entity_type,
    bods_entity_subtype,
    founding_date,
    dissolution_date,
    is_publicly_listed,
    created_at,
    updated_at,
    -- Search vector for full-text search
    to_tsvector('english', 
        COALESCE(name, '') || ' ' || 
        COALESCE(external_id, '')
    ) AS search_vector
FROM "ob-poc".entities;

-- Recreate v_cbu_subscriptions without client_label (use contract join)
CREATE OR REPLACE VIEW "ob-poc".v_cbu_subscriptions AS
SELECT 
    s.cbu_id,
    c.name AS cbu_name,
    lc.client_label AS contract_client,
    lc.contract_id,
    s.product_code,
    s.subscribed_at,
    cp.rate_card_id,
    rc.name AS rate_card_name,
    rc.currency AS rate_card_currency
FROM "ob-poc".cbu_subscriptions s
JOIN "ob-poc".cbus c ON s.cbu_id = c.cbu_id
JOIN "ob-poc".legal_contracts lc ON s.contract_id = lc.contract_id
JOIN "ob-poc".contract_products cp ON s.contract_id = cp.contract_id AND s.product_code = cp.product_code
LEFT JOIN "ob-poc".rate_cards rc ON cp.rate_card_id = rc.rate_card_id;

COMMENT ON TABLE "ob-poc".entities IS 'Entities use GROUP taxonomy for client hierarchy. Resolution via EntityGateway.';
COMMENT ON TABLE "ob-poc".cbus IS 'CBUs link to ManCo entities via share_links. Scope via GROUP apex entity.';

COMMIT;
-- Migration 047: Client Group Tables
-- Two-stage resolution: nickname → group_id → anchor_entity_id
--
-- Design:
-- - client_group: Virtual entity representing client brand/nickname
-- - client_group_alias: Multiple aliases per group for fuzzy matching
-- - client_group_alias_embedding: Versioned embeddings with model metadata
-- - client_group_anchor: Role-based mapping to real entities

BEGIN;

-- ============================================================================
-- Client Group (virtual entity for nicknames/brands)
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_name TEXT NOT NULL,
    short_code TEXT UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE "ob-poc".client_group IS 'Virtual entity representing client brand/nickname groups';

-- ============================================================================
-- Aliases (multiple per group, for fuzzy matching)
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_alias (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    alias_norm TEXT NOT NULL,  -- normalized: lowercase, trimmed
    source TEXT DEFAULT 'manual',
    confidence FLOAT DEFAULT 1.0,
    is_primary BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(group_id, alias_norm)
);

CREATE INDEX IF NOT EXISTS idx_cga_alias_norm ON "ob-poc".client_group_alias(alias_norm);
CREATE INDEX IF NOT EXISTS idx_cga_group_id ON "ob-poc".client_group_alias(group_id);

-- ============================================================================
-- Embeddings with versioning support
-- Composite PK allows multiple embeddings per alias (different models/pooling)
-- Contract: all embeddings are L2-normalized for proper cosine distance
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_alias_embedding (
    alias_id UUID NOT NULL REFERENCES "ob-poc".client_group_alias(id) ON DELETE CASCADE,
    embedder_id TEXT NOT NULL,           -- e.g., 'bge-small-en-v1.5'
    pooling TEXT NOT NULL,               -- e.g., 'cls', 'mean'
    normalize BOOLEAN NOT NULL,          -- should always be true for BGE
    dimension INT NOT NULL,              -- e.g., 384
    embedding vector(384) NOT NULL,      -- L2-normalized vector
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (alias_id, embedder_id)
);

COMMENT ON TABLE "ob-poc".client_group_alias_embedding IS
    'Embeddings must be L2-normalized. Query embeddings must also be normalized for correct cosine distance.';

-- IVFFlat index for approximate nearest neighbor search
-- Note: Run ANALYZE client_group_alias_embedding after bulk inserts for good recall
CREATE INDEX IF NOT EXISTS idx_cgae_embedding ON "ob-poc".client_group_alias_embedding
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 10);

-- ============================================================================
-- Anchor mappings (group → real entities, role-based)
-- Jurisdiction uses empty string '' for "no jurisdiction" to enable unique constraint
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_anchor (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,
    anchor_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    anchor_role TEXT NOT NULL,           -- 'ultimate_parent', 'governance_controller', etc.
    jurisdiction TEXT NOT NULL DEFAULT '',  -- empty string = no jurisdiction filter
    confidence FLOAT DEFAULT 1.0,
    priority INTEGER DEFAULT 0,          -- higher = preferred
    valid_from DATE,
    valid_to DATE,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(group_id, anchor_role, anchor_entity_id, jurisdiction)
);

CREATE INDEX IF NOT EXISTS idx_cga_anchor_group_role ON "ob-poc".client_group_anchor(group_id, anchor_role);
CREATE INDEX IF NOT EXISTS idx_cga_anchor_entity ON "ob-poc".client_group_anchor(anchor_entity_id);

COMMENT ON COLUMN "ob-poc".client_group_anchor.jurisdiction IS
    'Empty string means "applies to all jurisdictions". Specific jurisdiction takes precedence over empty.';

-- ============================================================================
-- Anchor role reference (for documentation/validation)
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_anchor_role (
    role_code TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    default_for_domains TEXT[]  -- which verb domains use this role by default
);

INSERT INTO "ob-poc".client_group_anchor_role (role_code, description, default_for_domains) VALUES
    ('ultimate_parent', 'UBO top-level parent (ownership apex)', ARRAY['ubo', 'ownership']),
    ('governance_controller', 'Operational/board control entity (ManCo equivalent)', ARRAY['session', 'cbu', 'view']),
    ('book_controller', 'Regional book controller', ARRAY['view']),
    ('operating_controller', 'Day-to-day operations controller', ARRAY['contract', 'service']),
    ('regulatory_anchor', 'Primary regulated entity for compliance', ARRAY['kyc', 'screening'])
ON CONFLICT (role_code) DO NOTHING;

-- ============================================================================
-- Helper view for alias search with group info
-- ============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_client_group_aliases AS
SELECT
    cga.id AS alias_id,
    cga.alias,
    cga.alias_norm,
    cga.is_primary,
    cga.confidence AS alias_confidence,
    cg.id AS group_id,
    cg.canonical_name,
    cg.short_code
FROM "ob-poc".client_group_alias cga
JOIN "ob-poc".client_group cg ON cg.id = cga.group_id;

-- ============================================================================
-- Helper view for anchor resolution with deterministic ordering
-- ============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_client_group_anchors AS
SELECT
    cga.group_id,
    cga.anchor_role,
    cga.anchor_entity_id,
    cga.jurisdiction,
    cga.confidence,
    cga.priority,
    cga.valid_from,
    cga.valid_to,
    e.name AS entity_name,
    e.entity_type_id
FROM "ob-poc".client_group_anchor cga
JOIN "ob-poc".entities e ON e.entity_id = cga.anchor_entity_id
WHERE (cga.valid_from IS NULL OR cga.valid_from <= CURRENT_DATE)
  AND (cga.valid_to IS NULL OR cga.valid_to >= CURRENT_DATE);

-- ============================================================================
-- Function: Resolve client group to anchor entity
-- Uses deterministic ordering: exact jurisdiction → global → priority → confidence → uuid
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".resolve_client_group_anchor(
    p_group_id UUID,
    p_anchor_role TEXT,
    p_jurisdiction TEXT DEFAULT ''
) RETURNS TABLE (
    anchor_entity_id UUID,
    entity_name TEXT,
    jurisdiction TEXT,
    confidence FLOAT,
    match_type TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        cga.anchor_entity_id,
        e.name::TEXT AS entity_name,
        cga.jurisdiction,
        cga.confidence,
        CASE
            WHEN cga.jurisdiction = p_jurisdiction AND p_jurisdiction != '' THEN 'exact_jurisdiction'
            WHEN cga.jurisdiction = '' THEN 'global_fallback'
            ELSE 'other'
        END AS match_type
    FROM "ob-poc".client_group_anchor cga
    JOIN "ob-poc".entities e ON e.entity_id = cga.anchor_entity_id
    WHERE cga.group_id = p_group_id
      AND cga.anchor_role = p_anchor_role
      AND (cga.valid_from IS NULL OR cga.valid_from <= CURRENT_DATE)
      AND (cga.valid_to IS NULL OR cga.valid_to >= CURRENT_DATE)
      AND (
          cga.jurisdiction = p_jurisdiction  -- exact match
          OR (p_jurisdiction = '' AND cga.jurisdiction = '')  -- no jurisdiction requested, match global
          OR (p_jurisdiction != '' AND cga.jurisdiction = '')  -- specific requested, fallback to global
      )
    ORDER BY
        CASE WHEN cga.jurisdiction = p_jurisdiction AND p_jurisdiction != '' THEN 0 ELSE 1 END,  -- exact jurisdiction first
        cga.priority DESC,                                    -- then priority
        cga.confidence DESC,                                  -- then confidence
        cga.anchor_entity_id                                  -- stable tie-breaker
    LIMIT 1;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".resolve_client_group_anchor IS
    'Resolve client group to anchor entity with deterministic ordering';

COMMIT;
-- ============================================================================
-- Client Group Bootstrap Data
-- Seeds: Allianz (full), Aviva (partial), test groups
-- ============================================================================

-- ============================================================================
-- Allianz Group (comprehensive - has entities in DB)
-- ============================================================================
INSERT INTO "ob-poc".client_group (id, canonical_name, short_code, description) VALUES
    ('11111111-1111-1111-1111-111111111111', 'Allianz Global Investors', 'AGI', 'Allianz asset management arm')
ON CONFLICT (id) DO UPDATE SET
    canonical_name = EXCLUDED.canonical_name,
    short_code = EXCLUDED.short_code,
    description = EXCLUDED.description;

-- Allianz aliases (common nicknames and variations)
INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, is_primary, source, confidence) VALUES
    ('11111111-1111-1111-1111-111111111111', 'Allianz Global Investors', 'allianz global investors', true, 'bootstrap', 1.0),
    ('11111111-1111-1111-1111-111111111111', 'Allianz', 'allianz', false, 'bootstrap', 1.0),
    ('11111111-1111-1111-1111-111111111111', 'AGI', 'agi', false, 'bootstrap', 0.95),
    ('11111111-1111-1111-1111-111111111111', 'AllianzGI', 'allianzgi', false, 'bootstrap', 0.98),
    ('11111111-1111-1111-1111-111111111111', 'Allianz GI', 'allianz gi', false, 'bootstrap', 0.98),
    ('11111111-1111-1111-1111-111111111111', 'Allianz Asset Management', 'allianz asset management', false, 'bootstrap', 0.90)
ON CONFLICT (group_id, alias_norm) DO NOTHING;

-- Allianz anchor mappings
-- Allianz SE = ultimate parent (7b6942b5-10e9-425f-b8c9-5a674a7d0701)
-- Allianz Global Investors Holdings GmbH = governance controller (084d316f-fa4e-42f0-ac39-1b01a3fbdf27)
INSERT INTO "ob-poc".client_group_anchor (group_id, anchor_entity_id, anchor_role, jurisdiction, priority, confidence, notes) VALUES
    -- Ultimate parent (global)
    ('11111111-1111-1111-1111-111111111111', '7b6942b5-10e9-425f-b8c9-5a674a7d0701', 'ultimate_parent', '', 10, 1.0, 'Allianz SE - Group apex'),
    -- Governance controller (global fallback)
    ('11111111-1111-1111-1111-111111111111', '084d316f-fa4e-42f0-ac39-1b01a3fbdf27', 'governance_controller', '', 10, 1.0, 'AGI Holdings GmbH - Global ManCo'),
    -- Book controller (global - same as governance for now)
    ('11111111-1111-1111-1111-111111111111', '084d316f-fa4e-42f0-ac39-1b01a3fbdf27', 'book_controller', '', 10, 1.0, 'AGI Holdings GmbH')
ON CONFLICT (group_id, anchor_role, anchor_entity_id, jurisdiction) DO NOTHING;

-- ============================================================================
-- Aviva Group (has some entities)
-- ============================================================================
INSERT INTO "ob-poc".client_group (id, canonical_name, short_code, description) VALUES
    ('22222222-2222-2222-2222-222222222222', 'Aviva Investors', 'AVIVA', 'Aviva asset management arm')
ON CONFLICT (id) DO UPDATE SET
    canonical_name = EXCLUDED.canonical_name,
    short_code = EXCLUDED.short_code,
    description = EXCLUDED.description;

-- Aviva aliases
INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, is_primary, source, confidence) VALUES
    ('22222222-2222-2222-2222-222222222222', 'Aviva Investors', 'aviva investors', true, 'bootstrap', 1.0),
    ('22222222-2222-2222-2222-222222222222', 'Aviva', 'aviva', false, 'bootstrap', 1.0),
    ('22222222-2222-2222-2222-222222222222', 'AI', 'ai', false, 'bootstrap', 0.7),  -- Lower confidence - ambiguous
    ('22222222-2222-2222-2222-222222222222', 'Aviva IM', 'aviva im', false, 'bootstrap', 0.95)
ON CONFLICT (group_id, alias_norm) DO NOTHING;

-- Aviva anchor mappings
-- Using Aviva Investors (5db4b67a-d500-4093-a3b6-a25c7bc0595a) as governance controller
-- Using Aviva Investors Global (8e2b1b10-a73c-4687-b218-e9283b22f940) as book controller
INSERT INTO "ob-poc".client_group_anchor (group_id, anchor_entity_id, anchor_role, jurisdiction, priority, confidence, notes) VALUES
    -- Ultimate parent (using Aviva Investors for now - no plc in DB)
    ('22222222-2222-2222-2222-222222222222', '5db4b67a-d500-4093-a3b6-a25c7bc0595a', 'ultimate_parent', '', 5, 0.9, 'Aviva Investors - placeholder apex'),
    -- Governance controller
    ('22222222-2222-2222-2222-222222222222', '5db4b67a-d500-4093-a3b6-a25c7bc0595a', 'governance_controller', '', 10, 1.0, 'Aviva Investors'),
    -- Luxembourg-specific controller
    ('22222222-2222-2222-2222-222222222222', 'f1fc872d-1ce2-478c-9a87-c0acf7f22a74', 'governance_controller', 'LU', 20, 1.0, 'Aviva Investors Luxembourg'),
    -- Book controller
    ('22222222-2222-2222-2222-222222222222', '8e2b1b10-a73c-4687-b218-e9283b22f940', 'book_controller', '', 10, 1.0, 'Aviva Investors Global')
ON CONFLICT (group_id, anchor_role, anchor_entity_id, jurisdiction) DO NOTHING;

-- ============================================================================
-- BlackRock Group (minimal - only one entity in DB)
-- ============================================================================
INSERT INTO "ob-poc".client_group (id, canonical_name, short_code, description) VALUES
    ('33333333-3333-3333-3333-333333333333', 'BlackRock', 'BLK', 'BlackRock asset management')
ON CONFLICT (id) DO UPDATE SET
    canonical_name = EXCLUDED.canonical_name,
    short_code = EXCLUDED.short_code,
    description = EXCLUDED.description;

-- BlackRock aliases
INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, is_primary, source, confidence) VALUES
    ('33333333-3333-3333-3333-333333333333', 'BlackRock', 'blackrock', true, 'bootstrap', 1.0),
    ('33333333-3333-3333-3333-333333333333', 'BLK', 'blk', false, 'bootstrap', 0.95),
    ('33333333-3333-3333-3333-333333333333', 'Black Rock', 'black rock', false, 'bootstrap', 0.90)
ON CONFLICT (group_id, alias_norm) DO NOTHING;

-- BlackRock anchor mapping (only has Transition Management entity)
INSERT INTO "ob-poc".client_group_anchor (group_id, anchor_entity_id, anchor_role, jurisdiction, priority, confidence, notes) VALUES
    ('33333333-3333-3333-3333-333333333333', '5598c9bf-8508-4f07-b484-aa78f296a09a', 'governance_controller', '', 5, 0.7, 'BlackRock Transition Management - placeholder')
ON CONFLICT (group_id, anchor_role, anchor_entity_id, jurisdiction) DO NOTHING;

-- ============================================================================
-- Test Group (for disambiguation testing)
-- ============================================================================
INSERT INTO "ob-poc".client_group (id, canonical_name, short_code, description) VALUES
    ('44444444-4444-4444-4444-444444444444', 'Aberdeen Global Infrastructure', 'AGI-ABER', 'Aberdeen infrastructure fund')
ON CONFLICT (id) DO UPDATE SET
    canonical_name = EXCLUDED.canonical_name,
    short_code = EXCLUDED.short_code,
    description = EXCLUDED.description;

-- Aberdeen aliases (shares "AGI" with Allianz for disambiguation testing)
INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, is_primary, source, confidence) VALUES
    ('44444444-4444-4444-4444-444444444444', 'Aberdeen Global Infrastructure', 'aberdeen global infrastructure', true, 'bootstrap', 1.0),
    ('44444444-4444-4444-4444-444444444444', 'Aberdeen', 'aberdeen', false, 'bootstrap', 0.95),
    ('44444444-4444-4444-4444-444444444444', 'AGI', 'agi', false, 'bootstrap', 0.80)  -- Same as Allianz - tests disambiguation
ON CONFLICT (group_id, alias_norm) DO NOTHING;

-- ============================================================================
-- Verify data
-- ============================================================================
DO $$
DECLARE
    group_count INT;
    alias_count INT;
    anchor_count INT;
BEGIN
    SELECT COUNT(*) INTO group_count FROM "ob-poc".client_group;
    SELECT COUNT(*) INTO alias_count FROM "ob-poc".client_group_alias;
    SELECT COUNT(*) INTO anchor_count FROM "ob-poc".client_group_anchor;

    RAISE NOTICE 'Client group seed complete: % groups, % aliases, % anchors',
        group_count, alias_count, anchor_count;
END $$;
-- Migration 049: Workflow Task Queue & Document Entity
-- Implements async task return path for workflow engine
-- Design doc: TODO-WORKFLOW-TASK-QUEUE.md (peer-reviewed)

-- ============================================================================
-- SECTION 1: Rejection Reason Codes (reference data)
-- ============================================================================

-- Standardized rejection reasons - drives client messaging
CREATE TABLE IF NOT EXISTS "ob-poc".rejection_reason_codes (
    code TEXT PRIMARY KEY,
    category TEXT NOT NULL,           -- 'quality', 'mismatch', 'validity', 'data', 'format', 'authenticity'
    client_message TEXT NOT NULL,     -- User-facing message
    ops_message TEXT NOT NULL,        -- Internal ops message
    next_action TEXT NOT NULL,        -- What to do next
    is_retryable BOOLEAN DEFAULT true -- Can client retry with different upload?
);

-- Quality issues
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('UNREADABLE',      'quality', 'Document image is too blurry to read', 'OCR failed - image quality', 'Please re-upload a clear, high-resolution image', true),
('CUTOFF',          'quality', 'Part of the document is cut off', 'Incomplete capture', 'Ensure all four corners are visible in the image', true),
('GLARE',           'quality', 'Glare obscures important information', 'Light reflection on document', 'Avoid flash and direct lighting when photographing', true),
('LOW_RESOLUTION',  'quality', 'Image resolution too low', 'Below minimum DPI', 'Upload a higher resolution scan (300 DPI minimum)', true)
ON CONFLICT (code) DO NOTHING;

-- Wrong document
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('WRONG_DOC_TYPE',  'mismatch', 'This is not the requested document type', 'Doc type mismatch', 'Please upload the correct document type', true),
('WRONG_PERSON',    'mismatch', 'Document belongs to a different person', 'Name/subject mismatch', 'Upload document for the correct person', true),
('SAMPLE_DOC',      'mismatch', 'This appears to be a sample or specimen', 'Specimen/sample detected', 'Please upload your actual document', true)
ON CONFLICT (code) DO NOTHING;

-- Validity issues
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('EXPIRED',         'validity', 'Document has expired', 'Past expiry date', 'Please provide a current, valid document', true),
('NOT_YET_VALID',   'validity', 'Document is not yet valid', 'Future valid_from date', 'Please provide a currently valid document', true),
('UNDATED',         'validity', 'Document has no issue or expiry date', 'Missing dates', 'Please provide a dated document', true)
ON CONFLICT (code) DO NOTHING;

-- Data issues
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('DOB_MISMATCH',    'data', 'Date of birth does not match our records', 'DOB mismatch vs entity', 'Please verify the correct document or contact support', false),
('NAME_MISMATCH',   'data', 'Name does not match our records', 'Name mismatch vs entity', 'Please verify spelling or provide supporting name change document', false),
('ADDRESS_MISMATCH','data', 'Address does not match declared address', 'Address mismatch', 'Please provide proof of address at declared address', true)
ON CONFLICT (code) DO NOTHING;

-- Format issues
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('UNSUPPORTED_FORMAT', 'format', 'File format not supported', 'Invalid file type', 'Please upload PDF, JPEG, or PNG', true),
('PASSWORD_PROTECTED', 'format', 'Document is password protected', 'Cannot open file', 'Please upload an unprotected version', true),
('CORRUPTED',       'format', 'File appears to be corrupted', 'Cannot read file', 'Please re-upload or try a different file', true)
ON CONFLICT (code) DO NOTHING;

-- Authenticity (careful with wording - don't accuse)
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('SUSPECTED_ALTERATION', 'authenticity', 'Document requires additional verification', 'Possible tampering detected', 'Our team will contact you for verification', false),
('INCONSISTENT_FONTS',   'authenticity', 'Document requires additional verification', 'Font inconsistency detected', 'Our team will contact you for verification', false)
ON CONFLICT (code) DO NOTHING;

-- ============================================================================
-- SECTION 2: Workflow Pending Tasks (outbound tracking)
-- Must be created BEFORE document_requirements due to FK
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".workflow_pending_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Links to workflow
    instance_id UUID NOT NULL REFERENCES "ob-poc".workflow_instances(instance_id),
    blocker_type TEXT NOT NULL,
    blocker_key TEXT,

    -- What was invoked (source of truth - don't trust external)
    verb TEXT NOT NULL,
    args JSONB,

    -- Expected results (multi-result support)
    expected_cargo_count INT DEFAULT 1,  -- How many results expected
    received_cargo_count INT DEFAULT 0,  -- How many completed with cargo
    failed_count INT DEFAULT 0,          -- How many failed/expired

    -- State
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'partial', 'completed', 'failed', 'expired', 'cancelled')),

    -- Timing
    created_at TIMESTAMPTZ DEFAULT now(),
    expires_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    -- Errors (last error for display)
    last_error TEXT
);

CREATE INDEX IF NOT EXISTS idx_pending_tasks_instance
    ON "ob-poc".workflow_pending_tasks(instance_id);
CREATE INDEX IF NOT EXISTS idx_pending_tasks_status
    ON "ob-poc".workflow_pending_tasks(status)
    WHERE status IN ('pending', 'partial');

-- ============================================================================
-- SECTION 3: Document Requirements (Layer A: what we need)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".document_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope (workflow or entity level)
    workflow_instance_id UUID REFERENCES "ob-poc".workflow_instances(instance_id),
    subject_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    subject_cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),

    -- What's required
    doc_type TEXT NOT NULL,           -- 'passport', 'proof_of_address', 'articles_of_incorporation'
    required_state TEXT NOT NULL DEFAULT 'verified'
        CHECK (required_state IN ('received', 'verified')),

    -- Current status
    status TEXT NOT NULL DEFAULT 'missing'
        CHECK (status IN ('missing', 'requested', 'received', 'in_qa', 'verified', 'rejected', 'expired', 'waived')),

    -- Retry tracking
    attempt_count INT DEFAULT 0,
    max_attempts INT DEFAULT 3,
    current_task_id UUID REFERENCES "ob-poc".workflow_pending_tasks(task_id),

    -- Latest document (populated after documents table created)
    latest_document_id UUID,
    latest_version_id UUID,

    -- Rejection details (last failure - for messaging)
    last_rejection_code TEXT REFERENCES "ob-poc".rejection_reason_codes(code),
    last_rejection_reason TEXT,       -- Optional free-text override

    -- Timing
    due_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    satisfied_at TIMESTAMPTZ,         -- When status reached required_state

    -- Uniqueness: one requirement per doc_type per subject per workflow
    UNIQUE NULLS NOT DISTINCT (workflow_instance_id, subject_entity_id, doc_type)
);

-- Find unsatisfied requirements for a workflow
CREATE INDEX IF NOT EXISTS idx_doc_req_workflow_status
    ON "ob-poc".document_requirements(workflow_instance_id, status)
    WHERE status NOT IN ('verified', 'waived');

-- Find requirements for an entity
CREATE INDEX IF NOT EXISTS idx_doc_req_subject
    ON "ob-poc".document_requirements(subject_entity_id, doc_type);

-- Find requirements with active outreach tasks
CREATE INDEX IF NOT EXISTS idx_doc_req_task
    ON "ob-poc".document_requirements(current_task_id)
    WHERE current_task_id IS NOT NULL;

-- ============================================================================
-- SECTION 4: Documents (Layer B: logical identity)
-- ============================================================================

-- Stable identity for "passport for person X" - multiple versions live under this
CREATE TABLE IF NOT EXISTS "ob-poc".documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Classification
    document_type TEXT NOT NULL,      -- 'passport', 'subscription_form', 'lei_record'

    -- Relationships
    subject_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    subject_cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    parent_document_id UUID REFERENCES "ob-poc".documents(document_id),

    -- Requirement linkage (which requirement this satisfies)
    requirement_id UUID REFERENCES "ob-poc".document_requirements(requirement_id),

    -- Provenance
    source TEXT NOT NULL,             -- 'upload', 'ocr', 'api', 'gleif', 'workflow'
    source_ref TEXT,                  -- External system ID

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT
);

-- Indexes for lookups
CREATE INDEX IF NOT EXISTS idx_documents_subject_type
    ON "ob-poc".documents(subject_entity_id, document_type);
CREATE INDEX IF NOT EXISTS idx_documents_requirement
    ON "ob-poc".documents(requirement_id)
    WHERE requirement_id IS NOT NULL;

-- Add FK constraints now that documents table exists
ALTER TABLE "ob-poc".document_requirements
    ADD CONSTRAINT fk_doc_req_latest_doc
    FOREIGN KEY (latest_document_id)
    REFERENCES "ob-poc".documents(document_id);

-- ============================================================================
-- SECTION 5: Document Versions (Layer C: immutable submissions)
-- ============================================================================

-- Each upload/submission is a new immutable version
CREATE TABLE IF NOT EXISTS "ob-poc".document_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".documents(document_id),
    version_no INT NOT NULL DEFAULT 1,

    -- Content type
    content_type TEXT NOT NULL,       -- MIME: 'application/json', 'image/jpeg', 'application/pdf'

    -- Content (at least one required)
    structured_data JSONB,            -- Parsed JSON/YAML
    blob_ref TEXT,                    -- Pointer to binary: 's3://bucket/key', 'file:///path'
    ocr_extracted JSONB,              -- Indexed fields from OCR/extraction

    -- Workflow linkage (which task produced this version)
    task_id UUID REFERENCES "ob-poc".workflow_pending_tasks(task_id),

    -- Verification status (on VERSION, not document - each submission verified separately)
    verification_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (verification_status IN ('pending', 'in_qa', 'verified', 'rejected')),

    -- Rejection details (if rejected)
    rejection_code TEXT REFERENCES "ob-poc".rejection_reason_codes(code),
    rejection_reason TEXT,            -- Optional free-text override/detail

    -- Verification audit
    verified_by TEXT,
    verified_at TIMESTAMPTZ,

    -- Validity period (from document content)
    valid_from DATE,
    valid_to DATE,

    -- Quality metrics (from OCR/extraction pipeline)
    quality_score NUMERIC(5,2),       -- 0.00 to 100.00
    extraction_confidence NUMERIC(5,2),

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT,

    UNIQUE(document_id, version_no),
    CONSTRAINT version_has_content CHECK (
        structured_data IS NOT NULL OR blob_ref IS NOT NULL
    )
);

-- Find latest version for a document
CREATE INDEX IF NOT EXISTS idx_doc_versions_document
    ON "ob-poc".document_versions(document_id, version_no DESC);

-- Find versions by verification status
CREATE INDEX IF NOT EXISTS idx_doc_versions_status
    ON "ob-poc".document_versions(verification_status, created_at)
    WHERE verification_status IN ('pending', 'in_qa');

-- Find versions by task
CREATE INDEX IF NOT EXISTS idx_doc_versions_task
    ON "ob-poc".document_versions(task_id)
    WHERE task_id IS NOT NULL;

-- GIN indexes for content search
CREATE INDEX IF NOT EXISTS idx_doc_versions_structured
    ON "ob-poc".document_versions USING gin(structured_data jsonb_path_ops)
    WHERE structured_data IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_doc_versions_ocr
    ON "ob-poc".document_versions USING gin(ocr_extracted jsonb_path_ops)
    WHERE ocr_extracted IS NOT NULL;

-- Add FK constraint for latest_version_id
ALTER TABLE "ob-poc".document_requirements
    ADD CONSTRAINT fk_doc_req_latest_version
    FOREIGN KEY (latest_version_id)
    REFERENCES "ob-poc".document_versions(version_id);

-- ============================================================================
-- SECTION 6: Document Events (audit trail)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".document_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".documents(document_id),
    version_id UUID REFERENCES "ob-poc".document_versions(version_id),

    -- Event type
    event_type TEXT NOT NULL,         -- 'created', 'version_uploaded', 'verified', 'rejected', 'expired'

    -- Event details
    old_status TEXT,
    new_status TEXT,
    rejection_code TEXT,
    notes TEXT,

    -- Actor
    actor TEXT,                       -- 'system', 'qa_user@example.com', 'api:gleif'

    occurred_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_doc_events_document
    ON "ob-poc".document_events(document_id, occurred_at DESC);

-- ============================================================================
-- SECTION 7: Task Result Queue (inbound results)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".task_result_queue (
    id BIGSERIAL PRIMARY KEY,

    -- Routing (by UUID, not string)
    task_id UUID NOT NULL,

    -- Outcome
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'expired')),
    error TEXT,

    -- Cargo is always a POINTER (URI)
    cargo_type TEXT,              -- 'document', 'entity', 'screening', 'bundle'
    cargo_ref TEXT,               -- URI: 'document://ob-poc/uuid' or 'version://ob-poc/uuid'

    -- Raw payload for audit/debugging (original webhook body)
    payload JSONB,

    -- Queue management
    queued_at TIMESTAMPTZ DEFAULT now(),
    processed_at TIMESTAMPTZ,

    -- Retry handling
    retry_count INT DEFAULT 0,
    last_error TEXT,

    -- Deduplication: idempotency_key scoped to task (not global)
    idempotency_key TEXT NOT NULL
);

-- Primary deduplication: unique per task
CREATE UNIQUE INDEX IF NOT EXISTS idx_task_result_queue_idempotency
    ON "ob-poc".task_result_queue(task_id, idempotency_key);

-- Secondary dedupe for multi-result safety (backup protection)
CREATE UNIQUE INDEX IF NOT EXISTS idx_task_result_queue_dedupe
    ON "ob-poc".task_result_queue(task_id, cargo_ref, status)
    WHERE cargo_ref IS NOT NULL;

-- Optimized index for queue pop (partial index on unprocessed)
CREATE INDEX IF NOT EXISTS idx_task_result_queue_pending
    ON "ob-poc".task_result_queue(id)
    WHERE processed_at IS NULL;

-- Lookup by task
CREATE INDEX IF NOT EXISTS idx_task_result_queue_task
    ON "ob-poc".task_result_queue(task_id);

-- ============================================================================
-- SECTION 8: Dead Letter Queue
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".task_result_dlq (
    id BIGSERIAL PRIMARY KEY,
    original_id BIGINT NOT NULL,
    task_id UUID NOT NULL,
    status TEXT NOT NULL,
    cargo_type TEXT,
    cargo_ref TEXT,
    error TEXT,
    payload JSONB,
    retry_count INT,
    queued_at TIMESTAMPTZ,
    dead_lettered_at TIMESTAMPTZ DEFAULT now(),
    failure_reason TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_task_result_dlq_task
    ON "ob-poc".task_result_dlq(task_id);

-- ============================================================================
-- SECTION 9: Task Events History (permanent audit trail)
-- ============================================================================

-- Permanent record of all task events (queue is ephemeral, this is audit)
CREATE TABLE IF NOT EXISTS "ob-poc".workflow_task_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES "ob-poc".workflow_pending_tasks(task_id),

    -- Event type: 'created', 'result_received', 'completed', 'failed', 'expired', 'cancelled'
    event_type TEXT NOT NULL,

    -- Result details (for result_received events)
    result_status TEXT,           -- 'completed', 'failed', 'expired'
    cargo_type TEXT,
    cargo_ref TEXT,
    error TEXT,

    -- Raw payload for audit (original webhook body)
    payload JSONB,

    -- Source tracking
    source TEXT,                  -- 'webhook', 'internal', 'timeout_job'
    idempotency_key TEXT,         -- From the original request

    -- Timing
    occurred_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_task_events_task
    ON "ob-poc".workflow_task_events(task_id);
CREATE INDEX IF NOT EXISTS idx_task_events_type
    ON "ob-poc".workflow_task_events(event_type, occurred_at);

-- ============================================================================
-- SECTION 10: Helper Views
-- ============================================================================

-- View: Requirements with latest version status
CREATE OR REPLACE VIEW "ob-poc".v_requirements_with_latest_version AS
SELECT
    dr.requirement_id,
    dr.workflow_instance_id,
    dr.subject_entity_id,
    dr.subject_cbu_id,
    dr.doc_type,
    dr.required_state,
    dr.status AS requirement_status,
    dr.attempt_count,
    dr.max_attempts,
    dr.current_task_id,
    dr.last_rejection_code,
    dr.last_rejection_reason,
    dr.due_date,
    dr.satisfied_at,
    d.document_id,
    d.source AS document_source,
    dv.version_id,
    dv.version_no,
    dv.verification_status AS version_status,
    dv.rejection_code AS version_rejection_code,
    dv.verified_at,
    dv.valid_from,
    dv.valid_to,
    CASE
        WHEN dv.valid_to IS NOT NULL AND dv.valid_to < CURRENT_DATE THEN true
        ELSE false
    END AS is_expired
FROM "ob-poc".document_requirements dr
LEFT JOIN "ob-poc".documents d ON dr.latest_document_id = d.document_id
LEFT JOIN "ob-poc".document_versions dv ON dr.latest_version_id = dv.version_id;

-- View: Pending tasks with cargo summary
CREATE OR REPLACE VIEW "ob-poc".v_pending_tasks_summary AS
SELECT
    pt.task_id,
    pt.instance_id,
    pt.blocker_type,
    pt.verb,
    pt.status,
    pt.expected_cargo_count,
    pt.received_cargo_count,
    pt.failed_count,
    pt.created_at,
    pt.expires_at,
    pt.completed_at,
    pt.last_error,
    wi.workflow_id,
    wi.current_state AS workflow_state
FROM "ob-poc".workflow_pending_tasks pt
JOIN "ob-poc".workflow_instances wi ON pt.instance_id = wi.instance_id;

-- View: Documents with current status (joined)
CREATE OR REPLACE VIEW "ob-poc".v_documents_with_status AS
SELECT
    d.document_id,
    d.document_type,
    d.subject_entity_id,
    d.subject_cbu_id,
    d.requirement_id,
    d.source,
    d.source_ref,
    d.created_at,
    latest.version_id AS latest_version_id,
    latest.version_no AS latest_version_no,
    latest.verification_status AS latest_status,
    latest.verified_at,
    latest.valid_from,
    latest.valid_to
FROM "ob-poc".documents d
LEFT JOIN LATERAL (
    SELECT version_id, version_no, verification_status, verified_at, valid_from, valid_to
    FROM "ob-poc".document_versions dv
    WHERE dv.document_id = d.document_id
    ORDER BY version_no DESC
    LIMIT 1
) latest ON true;

-- ============================================================================
-- SECTION 11: Helper Functions
-- ============================================================================

-- Function to get next version number for a document
CREATE OR REPLACE FUNCTION "ob-poc".get_next_document_version(p_document_id UUID)
RETURNS INT AS $$
    SELECT COALESCE(MAX(version_no), 0) + 1
    FROM "ob-poc".document_versions
    WHERE document_id = p_document_id;
$$ LANGUAGE SQL;

-- Function to update requirement status when version status changes
CREATE OR REPLACE FUNCTION "ob-poc".fn_sync_requirement_from_version()
RETURNS TRIGGER AS $$
DECLARE
    v_requirement_id UUID;
    v_required_state TEXT;
    v_new_req_status TEXT;
BEGIN
    -- Find the requirement via document
    SELECT d.requirement_id, dr.required_state
    INTO v_requirement_id, v_required_state
    FROM "ob-poc".documents d
    JOIN "ob-poc".document_requirements dr ON d.requirement_id = dr.requirement_id
    WHERE d.document_id = NEW.document_id;

    IF v_requirement_id IS NULL THEN
        RETURN NEW;
    END IF;

    -- Map version status to requirement status
    v_new_req_status := CASE NEW.verification_status
        WHEN 'pending' THEN 'received'
        WHEN 'in_qa' THEN 'in_qa'
        WHEN 'verified' THEN 'verified'
        WHEN 'rejected' THEN 'rejected'
    END;

    -- Update requirement
    UPDATE "ob-poc".document_requirements
    SET
        status = v_new_req_status,
        latest_version_id = NEW.version_id,
        updated_at = now(),
        -- Set satisfied_at when we reach the required state
        satisfied_at = CASE
            WHEN v_new_req_status = 'verified' OR
                 (v_required_state = 'received' AND v_new_req_status IN ('received', 'in_qa', 'verified'))
            THEN COALESCE(satisfied_at, now())
            ELSE satisfied_at
        END,
        -- Copy rejection details if rejected
        last_rejection_code = CASE
            WHEN NEW.verification_status = 'rejected' THEN NEW.rejection_code
            ELSE last_rejection_code
        END,
        last_rejection_reason = CASE
            WHEN NEW.verification_status = 'rejected' THEN NEW.rejection_reason
            ELSE last_rejection_reason
        END
    WHERE requirement_id = v_requirement_id;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to sync requirement status from version changes
DROP TRIGGER IF EXISTS tr_sync_requirement_from_version ON "ob-poc".document_versions;
CREATE TRIGGER tr_sync_requirement_from_version
    AFTER INSERT OR UPDATE OF verification_status
    ON "ob-poc".document_versions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".fn_sync_requirement_from_version();

-- Function to create document event on status change
CREATE OR REPLACE FUNCTION "ob-poc".fn_document_version_event()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO "ob-poc".document_events
            (document_id, version_id, event_type, new_status, actor)
        VALUES
            (NEW.document_id, NEW.version_id, 'version_uploaded', NEW.verification_status, NEW.created_by);
    ELSIF OLD.verification_status != NEW.verification_status THEN
        INSERT INTO "ob-poc".document_events
            (document_id, version_id, event_type, old_status, new_status, rejection_code, actor)
        VALUES
            (NEW.document_id, NEW.version_id,
             CASE NEW.verification_status
                WHEN 'verified' THEN 'verified'
                WHEN 'rejected' THEN 'rejected'
                ELSE 'status_changed'
             END,
             OLD.verification_status, NEW.verification_status, NEW.rejection_code, NEW.verified_by);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS tr_document_version_event ON "ob-poc".document_versions;
CREATE TRIGGER tr_document_version_event
    AFTER INSERT OR UPDATE OF verification_status
    ON "ob-poc".document_versions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".fn_document_version_event();

-- Grant permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".rejection_reason_codes TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".workflow_pending_tasks TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".document_requirements TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".documents TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".document_versions TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".document_events TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".task_result_queue TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".task_result_dlq TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".workflow_task_events TO public;
GRANT USAGE ON SEQUENCE "ob-poc".task_result_queue_id_seq TO public;
GRANT USAGE ON SEQUENCE "ob-poc".task_result_dlq_id_seq TO public;
-- Migration 050: Expansion Audit Trail
--
-- Stores ExpansionReport for audit/replay of DSL template expansion.
-- Each execution of DSL that goes through the expansion stage produces
-- a report that captures deterministic expansion details.
--
-- Key use cases:
-- - Audit trail for batch operations
-- - Replay/debugging of template expansions
-- - Lock derivation history (what entities were locked)
-- - Batch policy determination (atomic vs best_effort)

BEGIN;

-- =============================================================================
-- EXPANSION REPORTS
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".expansion_reports (
    -- Primary key is the expansion_id from ExpansionReport
    expansion_id UUID PRIMARY KEY,

    -- Session context
    session_id UUID NOT NULL,

    -- Source DSL hash (canonical whitespace)
    source_digest VARCHAR(64) NOT NULL,

    -- Expanded DSL hash (canonical whitespace)
    expanded_dsl_digest VARCHAR(64) NOT NULL,

    -- Number of statements after expansion
    expanded_statement_count INTEGER NOT NULL,

    -- Batch policy determined by expansion
    -- atomic = all-or-nothing with advisory locks
    -- best_effort = continue on failure
    batch_policy VARCHAR(20) NOT NULL CHECK (batch_policy IN ('atomic', 'best_effort')),

    -- Derived locks (JSONB array of {entity_type, entity_id, access})
    derived_lock_set JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Template digests used (JSONB array of {name, version, digest})
    template_digests JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Template invocations (JSONB array of TemplateInvocationReport)
    invocations JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Expansion diagnostics (warnings/errors)
    diagnostics JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Timestamps
    expanded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for session lookups
CREATE INDEX IF NOT EXISTS idx_expansion_reports_session
    ON "ob-poc".expansion_reports(session_id);

-- Index for digest lookups (find by source or expanded hash)
CREATE INDEX IF NOT EXISTS idx_expansion_reports_source_digest
    ON "ob-poc".expansion_reports(source_digest);

CREATE INDEX IF NOT EXISTS idx_expansion_reports_expanded_digest
    ON "ob-poc".expansion_reports(expanded_dsl_digest);

-- Index for batch policy analysis
CREATE INDEX IF NOT EXISTS idx_expansion_reports_batch_policy
    ON "ob-poc".expansion_reports(batch_policy);

-- Index for recent expansions
CREATE INDEX IF NOT EXISTS idx_expansion_reports_created
    ON "ob-poc".expansion_reports(created_at DESC);

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE "ob-poc".expansion_reports IS
    'Audit trail for DSL template expansion. Captures deterministic expansion details for replay/debugging.';

COMMENT ON COLUMN "ob-poc".expansion_reports.source_digest IS
    'SHA-256 hash of canonicalized source DSL (whitespace normalized)';

COMMENT ON COLUMN "ob-poc".expansion_reports.expanded_dsl_digest IS
    'SHA-256 hash of canonicalized expanded DSL (whitespace normalized)';

COMMENT ON COLUMN "ob-poc".expansion_reports.derived_lock_set IS
    'Advisory locks derived from template policy + runtime args. Array of {entity_type, entity_id, access}';

COMMENT ON COLUMN "ob-poc".expansion_reports.template_digests IS
    'Templates used in expansion. Array of {name, version, digest}';

COMMENT ON COLUMN "ob-poc".expansion_reports.invocations IS
    'Template invocation details. Array of TemplateInvocationReport';

COMMIT;
-- Adversarial Verification Model Tables
-- Part of Phase 2-3 implementation

--
-- Pattern detection audit trail
--
CREATE TABLE IF NOT EXISTS "ob-poc".detected_patterns (
    pattern_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    pattern_type varchar(50) NOT NULL,
    severity varchar(20) NOT NULL,
    description text NOT NULL,
    involved_entities uuid[] NOT NULL,
    evidence jsonb,
    status varchar(20) DEFAULT 'DETECTED'::varchar NOT NULL,
    detected_at timestamptz DEFAULT now() NOT NULL,
    resolved_at timestamptz,
    resolved_by varchar(100),
    resolution_notes text,
    CONSTRAINT detected_patterns_pkey PRIMARY KEY (pattern_id),
    CONSTRAINT detected_patterns_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT detected_patterns_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id),
    CONSTRAINT detected_patterns_pattern_type_check CHECK (pattern_type IN ('CIRCULAR_OWNERSHIP', 'LAYERING', 'NOMINEE_USAGE', 'OPACITY_JURISDICTION', 'REGISTRY_MISMATCH', 'OWNERSHIP_GAPS', 'RECENT_RESTRUCTURING', 'ROLE_CONCENTRATION')),
    CONSTRAINT detected_patterns_severity_check CHECK (severity IN ('INFO', 'LOW', 'MEDIUM', 'HIGH', 'CRITICAL')),
    CONSTRAINT detected_patterns_status_check CHECK (status IN ('DETECTED', 'INVESTIGATING', 'RESOLVED', 'FALSE_POSITIVE'))
);

CREATE INDEX IF NOT EXISTS idx_detected_patterns_cbu ON "ob-poc".detected_patterns(cbu_id);
CREATE INDEX IF NOT EXISTS idx_detected_patterns_type ON "ob-poc".detected_patterns(pattern_type);
CREATE INDEX IF NOT EXISTS idx_detected_patterns_status ON "ob-poc".detected_patterns(status);

COMMENT ON TABLE "ob-poc".detected_patterns IS 'Audit trail for adversarial pattern detection (circular ownership, layering, nominee usage, etc.)';

--
-- Challenge/response workflow for adversarial verification
--
CREATE TABLE IF NOT EXISTS "ob-poc".verification_challenges (
    challenge_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    entity_id uuid,
    allegation_id uuid,
    observation_id uuid,
    challenge_type varchar(30) NOT NULL,
    challenge_reason text NOT NULL,
    severity varchar(20) NOT NULL,
    status varchar(20) DEFAULT 'OPEN'::varchar NOT NULL,
    response_text text,
    response_evidence_ids uuid[],
    raised_at timestamptz DEFAULT now() NOT NULL,
    raised_by varchar(100),
    responded_at timestamptz,
    resolved_at timestamptz,
    resolved_by varchar(100),
    resolution_type varchar(30),
    resolution_notes text,
    CONSTRAINT verification_challenges_pkey PRIMARY KEY (challenge_id),
    CONSTRAINT verification_challenges_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT verification_challenges_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id),
    CONSTRAINT verification_challenges_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id),
    CONSTRAINT verification_challenges_allegation_id_fkey FOREIGN KEY (allegation_id) REFERENCES "ob-poc".client_allegations(allegation_id),
    CONSTRAINT verification_challenges_observation_id_fkey FOREIGN KEY (observation_id) REFERENCES "ob-poc".attribute_observations(observation_id),
    CONSTRAINT verification_challenges_type_check CHECK (challenge_type IN ('INCONSISTENCY', 'LOW_CONFIDENCE', 'MISSING_CORROBORATION', 'PATTERN_DETECTED', 'EVASION_SIGNAL', 'REGISTRY_MISMATCH')),
    CONSTRAINT verification_challenges_severity_check CHECK (severity IN ('INFO', 'LOW', 'MEDIUM', 'HIGH', 'CRITICAL')),
    CONSTRAINT verification_challenges_status_check CHECK (status IN ('OPEN', 'RESPONDED', 'RESOLVED', 'ESCALATED')),
    CONSTRAINT verification_challenges_resolution_type_check CHECK (resolution_type IS NULL OR resolution_type IN ('ACCEPTED', 'REJECTED', 'WAIVED', 'ESCALATED'))
);

CREATE INDEX IF NOT EXISTS idx_verification_challenges_cbu ON "ob-poc".verification_challenges(cbu_id);
CREATE INDEX IF NOT EXISTS idx_verification_challenges_status ON "ob-poc".verification_challenges(status);
CREATE INDEX IF NOT EXISTS idx_verification_challenges_case ON "ob-poc".verification_challenges(case_id);

COMMENT ON TABLE "ob-poc".verification_challenges IS 'Challenge/response workflow for adversarial verification - tracks formal challenges requiring client response';

--
-- Risk-based escalation routing
--
CREATE TABLE IF NOT EXISTS "ob-poc".verification_escalations (
    escalation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    challenge_id uuid,
    escalation_level varchar(30) NOT NULL,
    escalation_reason text NOT NULL,
    risk_indicators jsonb,
    status varchar(20) DEFAULT 'PENDING'::varchar NOT NULL,
    decision varchar(20),
    decision_notes text,
    escalated_at timestamptz DEFAULT now() NOT NULL,
    escalated_by varchar(100),
    decided_at timestamptz,
    decided_by varchar(100),
    CONSTRAINT verification_escalations_pkey PRIMARY KEY (escalation_id),
    CONSTRAINT verification_escalations_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT verification_escalations_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id),
    CONSTRAINT verification_escalations_challenge_id_fkey FOREIGN KEY (challenge_id) REFERENCES "ob-poc".verification_challenges(challenge_id),
    CONSTRAINT verification_escalations_level_check CHECK (escalation_level IN ('SENIOR_ANALYST', 'COMPLIANCE_OFFICER', 'MLRO', 'COMMITTEE')),
    CONSTRAINT verification_escalations_status_check CHECK (status IN ('PENDING', 'UNDER_REVIEW', 'DECIDED')),
    CONSTRAINT verification_escalations_decision_check CHECK (decision IS NULL OR decision IN ('APPROVE', 'REJECT', 'REQUIRE_MORE_INFO', 'ESCALATE_FURTHER'))
);

CREATE INDEX IF NOT EXISTS idx_verification_escalations_cbu ON "ob-poc".verification_escalations(cbu_id);
CREATE INDEX IF NOT EXISTS idx_verification_escalations_status ON "ob-poc".verification_escalations(status);
CREATE INDEX IF NOT EXISTS idx_verification_escalations_level ON "ob-poc".verification_escalations(escalation_level);

COMMENT ON TABLE "ob-poc".verification_escalations IS 'Risk-based escalation routing for verification challenges requiring higher authority review';
