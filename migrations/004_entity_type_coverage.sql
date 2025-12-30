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
