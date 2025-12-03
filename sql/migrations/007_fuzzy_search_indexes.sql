-- Migration: 007_fuzzy_search_indexes
-- Description: Add pg_trgm extension and GIN indexes for fuzzy text search
-- Used by: Agent context-gathering for CBU/entity name lookups

-- Enable trigram extension for fuzzy text matching
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- =============================================================================
-- GIN INDEXES FOR FUZZY SEARCH
-- =============================================================================

-- CBU name fuzzy search
CREATE INDEX IF NOT EXISTS idx_cbus_name_trgm 
ON "ob-poc".cbus USING GIN (name gin_trgm_ops);

-- Entity base table name search
CREATE INDEX IF NOT EXISTS idx_entities_name_trgm 
ON "ob-poc".entities USING GIN (name gin_trgm_ops);

-- Person name search
CREATE INDEX IF NOT EXISTS idx_proper_persons_first_name_trgm 
ON "ob-poc".entity_proper_persons USING GIN (first_name gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_proper_persons_last_name_trgm 
ON "ob-poc".entity_proper_persons USING GIN (last_name gin_trgm_ops);

-- Company name search
CREATE INDEX IF NOT EXISTS idx_limited_companies_name_trgm 
ON "ob-poc".entity_limited_companies USING GIN (company_name gin_trgm_ops);

-- Partnership name search
CREATE INDEX IF NOT EXISTS idx_partnerships_name_trgm 
ON "ob-poc".entity_partnerships USING GIN (partnership_name gin_trgm_ops);

-- Trust name search
CREATE INDEX IF NOT EXISTS idx_trusts_name_trgm 
ON "ob-poc".entity_trusts USING GIN (trust_name gin_trgm_ops);

-- =============================================================================
-- HELPER FUNCTIONS FOR FUZZY SEARCH
-- =============================================================================

-- Find CBUs by fuzzy name match
CREATE OR REPLACE FUNCTION "ob-poc".fuzzy_find_cbu(
    search_term TEXT,
    min_similarity FLOAT DEFAULT 0.3,
    max_results INT DEFAULT 5
)
RETURNS TABLE (
    cbu_id UUID,
    name VARCHAR(255),
    client_type VARCHAR(100),
    jurisdiction VARCHAR(50),
    similarity_score FLOAT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        c.cbu_id,
        c.name,
        c.client_type,
        c.jurisdiction,
        similarity(c.name, search_term)::FLOAT as similarity_score
    FROM "ob-poc".cbus c
    WHERE c.name % search_term
      AND similarity(c.name, search_term) >= min_similarity
    ORDER BY similarity_score DESC
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql STABLE;

-- Find entities by fuzzy name match
CREATE OR REPLACE FUNCTION "ob-poc".fuzzy_find_entity(
    search_term TEXT,
    min_similarity FLOAT DEFAULT 0.3,
    max_results INT DEFAULT 5
)
RETURNS TABLE (
    entity_id UUID,
    name VARCHAR(255),
    entity_type VARCHAR(255),
    similarity_score FLOAT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        e.entity_id,
        e.name,
        et.name as entity_type,
        similarity(e.name, search_term)::FLOAT as similarity_score
    FROM "ob-poc".entities e
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE e.name % search_term
      AND similarity(e.name, search_term) >= min_similarity
    ORDER BY similarity_score DESC
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql STABLE;

-- Find persons by fuzzy name match
CREATE OR REPLACE FUNCTION "ob-poc".fuzzy_find_person(
    search_term TEXT,
    min_similarity FLOAT DEFAULT 0.3,
    max_results INT DEFAULT 5
)
RETURNS TABLE (
    entity_id UUID,
    proper_person_id UUID,
    first_name VARCHAR(255),
    last_name VARCHAR(255),
    full_name TEXT,
    similarity_score FLOAT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        p.entity_id,
        p.proper_person_id,
        p.first_name,
        p.last_name,
        (p.first_name || ' ' || p.last_name)::TEXT as full_name,
        GREATEST(
            similarity(p.first_name, search_term),
            similarity(p.last_name, search_term),
            similarity(p.first_name || ' ' || p.last_name, search_term)
        )::FLOAT as similarity_score
    FROM "ob-poc".entity_proper_persons p
    WHERE p.first_name % search_term
       OR p.last_name % search_term
       OR (p.first_name || ' ' || p.last_name) % search_term
    ORDER BY similarity_score DESC
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql STABLE;

-- Find companies by fuzzy name match
CREATE OR REPLACE FUNCTION "ob-poc".fuzzy_find_company(
    search_term TEXT,
    min_similarity FLOAT DEFAULT 0.3,
    max_results INT DEFAULT 5
)
RETURNS TABLE (
    entity_id UUID,
    limited_company_id UUID,
    company_name VARCHAR(255),
    jurisdiction VARCHAR(100),
    similarity_score FLOAT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        c.entity_id,
        c.limited_company_id,
        c.company_name,
        c.jurisdiction,
        similarity(c.company_name, search_term)::FLOAT as similarity_score
    FROM "ob-poc".entity_limited_companies c
    WHERE c.company_name % search_term
      AND similarity(c.company_name, search_term) >= min_similarity
    ORDER BY similarity_score DESC
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql STABLE;

-- =============================================================================
-- UNIFIED SEARCH FUNCTION
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fuzzy_search_all(
    search_term TEXT,
    min_similarity FLOAT DEFAULT 0.3,
    max_results INT DEFAULT 10
)
RETURNS TABLE (
    id UUID,
    name TEXT,
    type TEXT,
    subtype TEXT,
    similarity_score FLOAT
) AS $$
BEGIN
    RETURN QUERY
    (
        SELECT 
            c.cbu_id as id,
            c.name::TEXT,
            'cbu'::TEXT as type,
            c.client_type::TEXT as subtype,
            similarity(c.name, search_term)::FLOAT as similarity_score
        FROM "ob-poc".cbus c
        WHERE c.name % search_term
          AND similarity(c.name, search_term) >= min_similarity
    )
    UNION ALL
    (
        SELECT 
            e.entity_id as id,
            e.name::TEXT,
            'entity'::TEXT as type,
            et.name::TEXT as subtype,
            similarity(e.name, search_term)::FLOAT as similarity_score
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE e.name % search_term
          AND similarity(e.name, search_term) >= min_similarity
    )
    ORDER BY similarity_score DESC
    LIMIT max_results;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".fuzzy_find_cbu IS 'Find CBUs by fuzzy name match using trigram similarity';
COMMENT ON FUNCTION "ob-poc".fuzzy_find_entity IS 'Find entities by fuzzy name match using trigram similarity';
COMMENT ON FUNCTION "ob-poc".fuzzy_find_person IS 'Find persons by fuzzy name match (first, last, or full name)';
COMMENT ON FUNCTION "ob-poc".fuzzy_find_company IS 'Find companies by fuzzy name match';
COMMENT ON FUNCTION "ob-poc".fuzzy_search_all IS 'Unified fuzzy search across CBUs and all entity types';
