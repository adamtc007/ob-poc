-- Migration: 025_csg_similarity_cache.sql
-- Purpose: Create semantic similarity cache table for CSG suggestions
-- Part of CSG Linter implementation for business rule validation

BEGIN;

-- ============================================
-- CSG_SEMANTIC_SIMILARITY_CACHE
-- ============================================
-- Pre-computed similarity scores for fast suggestions
-- Complements the existing rag_embeddings table

CREATE TABLE IF NOT EXISTS "ob-poc".csg_semantic_similarity_cache (
    cache_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Source item
    source_type VARCHAR(50) NOT NULL,  -- 'document_type', 'attribute', 'entity_type'
    source_code VARCHAR(100) NOT NULL,

    -- Target item
    target_type VARCHAR(50) NOT NULL,
    target_code VARCHAR(100) NOT NULL,

    -- Similarity metrics
    cosine_similarity FLOAT NOT NULL,
    levenshtein_distance INTEGER,
    semantic_relatedness FLOAT,  -- From knowledge graph if available

    -- Context
    relationship_type VARCHAR(50),  -- 'alternative', 'complement', 'parent', 'child'

    -- Cache management
    computed_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ DEFAULT NOW() + INTERVAL '7 days',

    UNIQUE(source_type, source_code, target_type, target_code)
);

CREATE INDEX IF NOT EXISTS idx_similarity_source
ON "ob-poc".csg_semantic_similarity_cache(source_type, source_code);

CREATE INDEX IF NOT EXISTS idx_similarity_target
ON "ob-poc".csg_semantic_similarity_cache(target_type, target_code);

CREATE INDEX IF NOT EXISTS idx_similarity_score
ON "ob-poc".csg_semantic_similarity_cache(cosine_similarity DESC);

CREATE INDEX IF NOT EXISTS idx_similarity_expires
ON "ob-poc".csg_semantic_similarity_cache(expires_at);

-- ============================================
-- FUNCTION: Refresh similarity cache for document types
-- ============================================

CREATE OR REPLACE FUNCTION "ob-poc".refresh_document_type_similarities()
RETURNS void AS $$
BEGIN
    -- Delete expired entries
    DELETE FROM "ob-poc".csg_semantic_similarity_cache
    WHERE expires_at < NOW();

    -- Only proceed if vector extension is available
    IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        RAISE NOTICE 'pgvector extension not installed, skipping similarity refresh';
        RETURN;
    END IF;

    -- Insert new similarities based on embeddings
    INSERT INTO "ob-poc".csg_semantic_similarity_cache
        (source_type, source_code, target_type, target_code,
         cosine_similarity, relationship_type, computed_at, expires_at)
    SELECT
        'document_type', dt1.type_code,
        'document_type', dt2.type_code,
        1 - (dt1.embedding <=> dt2.embedding) as similarity,
        'alternative',
        NOW(),
        NOW() + INTERVAL '7 days'
    FROM "ob-poc".document_types dt1
    CROSS JOIN "ob-poc".document_types dt2
    WHERE dt1.type_code != dt2.type_code
      AND dt1.embedding IS NOT NULL
      AND dt2.embedding IS NOT NULL
      AND 1 - (dt1.embedding <=> dt2.embedding) > 0.5
    ON CONFLICT (source_type, source_code, target_type, target_code)
    DO UPDATE SET
        cosine_similarity = EXCLUDED.cosine_similarity,
        computed_at = NOW(),
        expires_at = NOW() + INTERVAL '7 days';
END;
$$ LANGUAGE plpgsql;

COMMIT;
