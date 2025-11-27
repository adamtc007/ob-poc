-- Migration: Entity Search Indexes with pg_trgm
-- Description: Enable fuzzy text search across entity types for typeahead/autocomplete
-- Date: 2025-11-27

-- =============================================================================
-- Enable pg_trgm extension for fuzzy text search
-- =============================================================================
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- =============================================================================
-- Person search index (combined name)
-- =============================================================================
-- Add computed column for full name search
ALTER TABLE "ob-poc".entity_proper_persons
ADD COLUMN IF NOT EXISTS search_name TEXT
GENERATED ALWAYS AS (
    COALESCE(first_name, '') || ' ' || COALESCE(last_name, '')
) STORED;

-- Trigram index for fuzzy matching
CREATE INDEX IF NOT EXISTS idx_persons_search_name_trgm
ON "ob-poc".entity_proper_persons
USING gin (search_name gin_trgm_ops);

-- Also index individual name parts for prefix search
CREATE INDEX IF NOT EXISTS idx_persons_first_name_trgm
ON "ob-poc".entity_proper_persons
USING gin (first_name gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_persons_last_name_trgm
ON "ob-poc".entity_proper_persons
USING gin (last_name gin_trgm_ops);

-- =============================================================================
-- Company search index
-- =============================================================================
CREATE INDEX IF NOT EXISTS idx_companies_name_trgm
ON "ob-poc".entity_limited_companies
USING gin (company_name gin_trgm_ops);

-- Also registration number for exact lookups
CREATE INDEX IF NOT EXISTS idx_companies_reg_number
ON "ob-poc".entity_limited_companies (registration_number);

-- =============================================================================
-- CBU search index
-- =============================================================================
CREATE INDEX IF NOT EXISTS idx_cbu_name_trgm
ON "ob-poc".cbus
USING gin (name gin_trgm_ops);

-- =============================================================================
-- Trust search index
-- =============================================================================
CREATE INDEX IF NOT EXISTS idx_trusts_name_trgm
ON "ob-poc".entity_trusts
USING gin (trust_name gin_trgm_ops);

-- =============================================================================
-- Unified entity view for cross-type search
-- =============================================================================
CREATE OR REPLACE VIEW "ob-poc".entity_search_view AS

-- Persons
SELECT
    proper_person_id as id,
    'PERSON' as entity_type,
    COALESCE(first_name, '') || ' ' || COALESCE(last_name, '') as display_name,
    nationality as subtitle_1,
    date_of_birth::text as subtitle_2,
    COALESCE(first_name, '') || ' ' || COALESCE(last_name, '') as search_text
FROM "ob-poc".entity_proper_persons
WHERE proper_person_id IS NOT NULL

UNION ALL

-- Companies
SELECT
    limited_company_id as id,
    'COMPANY' as entity_type,
    company_name as display_name,
    jurisdiction as subtitle_1,
    registration_number as subtitle_2,
    company_name as search_text
FROM "ob-poc".entity_limited_companies
WHERE limited_company_id IS NOT NULL

UNION ALL

-- CBUs
SELECT
    cbu_id as id,
    'CBU' as entity_type,
    name as display_name,
    client_type as subtitle_1,
    jurisdiction as subtitle_2,
    name as search_text
FROM "ob-poc".cbus
WHERE cbu_id IS NOT NULL

UNION ALL

-- Trusts
SELECT
    trust_id as id,
    'TRUST' as entity_type,
    trust_name as display_name,
    jurisdiction as subtitle_1,
    NULL as subtitle_2,
    trust_name as search_text
FROM "ob-poc".entity_trusts
WHERE trust_id IS NOT NULL;

-- =============================================================================
-- Verification query (run manually to test)
-- =============================================================================
-- SELECT
--     display_name,
--     entity_type,
--     similarity(search_text, 'john') as score
-- FROM "ob-poc".entity_search_view
-- WHERE search_text % 'john'
-- ORDER BY score DESC
-- LIMIT 10;
