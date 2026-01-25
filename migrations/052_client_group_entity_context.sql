-- Migration 052: Client Group Entity Context
-- Human-to-agent semantic bridge for entity resolution
--
-- Purpose: Enable Candle-assisted fuzzy search from human shorthand to entity_ids
--
-- Layers:
-- 1. client_group_entity: Which entities BELONG to this client universe
-- 2. client_group_entity_tag: Informal shorthand tags (persona-scoped)
-- 3. client_group_entity_tag_embedding: Candle-searchable vectors
--
-- Prerequisites:
-- - pgcrypto extension enabled (gen_random_uuid) — see migration 001
-- - pgvector extension enabled — see migration 037
-- - pg_trgm extension for fuzzy text matching

BEGIN;

-- Ensure pg_trgm is available for trigram fuzzy matching
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- ============================================================================
-- Entity Membership: "These entities belong to this client"
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_entity (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- Membership classification
    membership_type TEXT NOT NULL DEFAULT 'confirmed',
        -- 'confirmed': verified as belonging to client
        -- 'suspected': discovered but unconfirmed (onboarding)
        -- 'historical': formerly belonged, now inactive

    -- Provenance
    added_by TEXT NOT NULL DEFAULT 'manual',
        -- 'manual': human added directly
        -- 'discovery': onboarding discovery process
        -- 'gleif': traced via GLEIF relationship
        -- 'ownership_trace': UBO/ownership analysis
        -- 'user_confirmed': human confirmed suspected

    -- Metadata
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),

    UNIQUE(group_id, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_cge_group ON "ob-poc".client_group_entity(group_id);
CREATE INDEX IF NOT EXISTS idx_cge_entity ON "ob-poc".client_group_entity(entity_id);
CREATE INDEX IF NOT EXISTS idx_cge_membership ON "ob-poc".client_group_entity(group_id, membership_type);

COMMENT ON TABLE "ob-poc".client_group_entity IS
    'Entity membership in client groups. Tracks which entities belong to which client universe.';

-- ============================================================================
-- Shorthand Tags: "What humans call this entity"
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_entity_tag (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- The tag itself
    tag TEXT NOT NULL,              -- "main lux manco", "irish fund", "sicav"
    tag_norm TEXT NOT NULL,         -- normalized: lowercase, trimmed, collapsed spaces

    -- Persona scoping (same entity, different labels for different users)
    persona TEXT,                   -- NULL = universal, or: 'kyc' | 'trading' | 'ops' | 'onboarding'

    -- Provenance
    source TEXT NOT NULL DEFAULT 'manual',
        -- 'manual': human entered
        -- 'user_confirmed': human confirmed during interaction ("yes, I call it that")
        -- 'inferred': system guessed from usage patterns
        -- 'bootstrap': initial seed data

    confidence FLOAT DEFAULT 1.0,   -- 0.0-1.0, lower for inferred

    -- Metadata
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT                 -- user/session that created
);

-- Uniqueness: same tag can't exist twice for same entity+group+persona
-- Must use index because Postgres table constraints can't include expressions like COALESCE
CREATE UNIQUE INDEX IF NOT EXISTS uq_cget_tag
    ON "ob-poc".client_group_entity_tag(group_id, entity_id, tag_norm, COALESCE(persona, ''));

-- Fast lookups
CREATE INDEX IF NOT EXISTS idx_cget_group_entity ON "ob-poc".client_group_entity_tag(group_id, entity_id);
CREATE INDEX IF NOT EXISTS idx_cget_tag_norm ON "ob-poc".client_group_entity_tag(tag_norm);
CREATE INDEX IF NOT EXISTS idx_cget_persona ON "ob-poc".client_group_entity_tag(group_id, persona) WHERE persona IS NOT NULL;

-- Trigram index for fuzzy text search
CREATE INDEX IF NOT EXISTS idx_cget_tag_norm_trgm ON "ob-poc".client_group_entity_tag
    USING gin (tag_norm gin_trgm_ops);

COMMENT ON TABLE "ob-poc".client_group_entity_tag IS
    'Informal shorthand tags for entities within a client context. Persona-scoped, human-think labels.';

COMMENT ON COLUMN "ob-poc".client_group_entity_tag.persona IS
    'NULL = universal tag. Otherwise scoped to persona: kyc, trading, ops, onboarding, etc.';

-- ============================================================================
-- Tag Embeddings: Candle-searchable vectors
-- NOTE: vector(384) is intentionally fixed — this system uses BGE-small-en-v1.5.
-- Changing embedding dimension requires schema + function migration.
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_entity_tag_embedding (
    tag_id UUID NOT NULL REFERENCES "ob-poc".client_group_entity_tag(id) ON DELETE CASCADE,
    embedder_id TEXT NOT NULL,           -- e.g., 'bge-small-en-v1.5'
    pooling TEXT NOT NULL,               -- e.g., 'cls', 'mean'
    normalize BOOLEAN NOT NULL,          -- should always be true for BGE
    dimension INT NOT NULL,              -- e.g., 384
    embedding vector(384) NOT NULL,      -- L2-normalized vector
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (tag_id, embedder_id)
);

-- ANN INDEX: DEFERRED until sufficient data volume
-- At low row counts (<2k tags), exact vector scan is fast enough.
-- Enable this when tag count exceeds ~5k rows:
--
-- CREATE INDEX idx_cgete_embedding ON "ob-poc".client_group_entity_tag_embedding
--     USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
--
-- Or use HNSW (if pgvector >= 0.5.0) for better performance without tuning:
-- CREATE INDEX idx_cgete_embedding ON "ob-poc".client_group_entity_tag_embedding
--     USING hnsw (embedding vector_cosine_ops);

COMMENT ON TABLE "ob-poc".client_group_entity_tag_embedding IS
    'Embeddings for shorthand tags. Enables Candle semantic search: "irish funds" → entity_ids';

-- ============================================================================
-- Helper View: Full tag context for search
-- ============================================================================
DROP VIEW IF EXISTS "ob-poc".v_client_entity_tags;
CREATE VIEW "ob-poc".v_client_entity_tags AS
SELECT
    cget.id AS tag_id,
    cget.tag,
    cget.tag_norm,
    cget.persona,
    cget.confidence,
    cget.source,
    cget.group_id,
    cg.canonical_name AS group_name,
    cget.entity_id,
    e.name::TEXT AS entity_name,  -- explicit cast for varchar(255)
    e.entity_type_id,
    cge.membership_type
FROM "ob-poc".client_group_entity_tag cget
JOIN "ob-poc".client_group cg ON cg.id = cget.group_id
JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
LEFT JOIN "ob-poc".client_group_entity cge
    ON cge.group_id = cget.group_id AND cge.entity_id = cget.entity_id;

-- ============================================================================
-- Function: Search tags by text (exact + fuzzy)
-- Returns entities matching the human shorthand
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".search_entity_tags(
    p_group_id UUID,
    p_query TEXT,
    p_persona TEXT DEFAULT NULL,
    p_limit INT DEFAULT 10,
    p_include_historical BOOLEAN DEFAULT FALSE  -- exclude historical by default
) RETURNS TABLE (
    entity_id UUID,
    entity_name TEXT,
    tag TEXT,
    confidence FLOAT,
    match_type TEXT
) AS $$
DECLARE
    v_query_norm TEXT;
BEGIN
    -- Normalize query
    v_query_norm := lower(trim(regexp_replace(p_query, '\s+', ' ', 'g')));

    RETURN QUERY
    WITH matches AS (
        -- Exact match (highest priority)
        SELECT
            cget.entity_id,
            e.name::TEXT AS entity_name,  -- explicit cast for varchar(255)
            cget.tag,
            cget.confidence,
            'exact'::TEXT AS match_type,
            1 AS priority
        FROM "ob-poc".client_group_entity_tag cget
        -- MEMBERSHIP GATE: only return entities that are members of the group
        JOIN "ob-poc".client_group_entity cge
            ON cge.group_id = cget.group_id AND cge.entity_id = cget.entity_id
        JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
        WHERE cget.group_id = p_group_id
          AND cget.tag_norm = v_query_norm
          AND (p_persona IS NULL OR cget.persona IS NULL OR cget.persona = p_persona)
          -- Exclude historical unless explicitly requested
          AND (p_include_historical OR cge.membership_type != 'historical')

        UNION ALL

        -- Trigram fuzzy match
        SELECT
            cget.entity_id,
            e.name::TEXT AS entity_name,  -- explicit cast for varchar(255)
            cget.tag,
            (similarity(cget.tag_norm, v_query_norm) * cget.confidence)::FLOAT,
            'fuzzy'::TEXT AS match_type,
            2 AS priority
        FROM "ob-poc".client_group_entity_tag cget
        -- MEMBERSHIP GATE: only return entities that are members of the group
        JOIN "ob-poc".client_group_entity cge
            ON cge.group_id = cget.group_id AND cge.entity_id = cget.entity_id
        JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
        WHERE cget.group_id = p_group_id
          AND cget.tag_norm % v_query_norm
          AND cget.tag_norm != v_query_norm  -- exclude exact matches
          AND (p_persona IS NULL OR cget.persona IS NULL OR cget.persona = p_persona)
          -- Exclude historical unless explicitly requested
          AND (p_include_historical OR cge.membership_type != 'historical')
    )
    SELECT DISTINCT ON (m.entity_id)
        m.entity_id,
        m.entity_name,
        m.tag,
        m.confidence,
        m.match_type
    FROM matches m
    ORDER BY m.entity_id, m.priority, m.confidence DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".search_entity_tags IS
    'Search shorthand tags to resolve human language to entity_ids. Used by Candle intent pipeline.';

-- ============================================================================
-- Function: Search tags by embedding (semantic similarity)
-- NOTE: vector(384) is intentionally fixed — this system uses BGE-small-en-v1.5.
-- Changing embedding dimension requires schema + function migration.
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".search_entity_tags_semantic(
    p_group_id UUID,
    p_query_embedding vector(384),  -- 384-dim fixed (BGE-small-en-v1.5)
    p_persona TEXT DEFAULT NULL,
    p_limit INT DEFAULT 10,
    p_min_similarity FLOAT DEFAULT 0.5,
    p_include_historical BOOLEAN DEFAULT FALSE,
    p_embedder_id TEXT DEFAULT 'bge-small-en-v1.5'  -- must match embedding model
) RETURNS TABLE (
    entity_id UUID,
    entity_name TEXT,
    tag TEXT,
    similarity FLOAT,
    match_type TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT ON (cget.entity_id)
        cget.entity_id,
        e.name::TEXT AS entity_name,  -- explicit cast for varchar(255)
        cget.tag,
        (1.0 - (cgete.embedding <=> p_query_embedding))::FLOAT AS similarity,
        'semantic'::TEXT AS match_type
    FROM "ob-poc".client_group_entity_tag_embedding cgete
    JOIN "ob-poc".client_group_entity_tag cget ON cget.id = cgete.tag_id
    -- MEMBERSHIP GATE: only return entities that are members of the group
    JOIN "ob-poc".client_group_entity cge
        ON cge.group_id = cget.group_id AND cge.entity_id = cget.entity_id
    JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
    WHERE cget.group_id = p_group_id
      -- Filter by embedder to avoid dimension/model mismatches
      AND cgete.embedder_id = p_embedder_id
      AND (p_persona IS NULL OR cget.persona IS NULL OR cget.persona = p_persona)
      AND (1.0 - (cgete.embedding <=> p_query_embedding)) >= p_min_similarity
      -- Exclude historical unless explicitly requested
      AND (p_include_historical OR cge.membership_type != 'historical')
    ORDER BY cget.entity_id, (cgete.embedding <=> p_query_embedding)
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".search_entity_tags_semantic IS
    'Semantic search using Candle embeddings. Fallback when text search returns nothing.';

-- ============================================================================
-- Function: Normalize a tag string
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".normalize_tag(p_tag TEXT) RETURNS TEXT AS $$
BEGIN
    RETURN lower(trim(regexp_replace(p_tag, '\s+', ' ', 'g')));
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION "ob-poc".normalize_tag IS 'Normalize tag for consistent matching: lowercase, trim, collapse spaces';

COMMIT;
