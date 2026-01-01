-- Migration: Verb Pattern Embeddings for Semantic Voice Matching
-- Uses pgvector for similarity search on voice transcripts
-- Model: all-MiniLM-L6-v2 (384 dimensions)

-- =============================================================================
-- VERB PATTERN EMBEDDINGS TABLE
-- Stores pre-computed embeddings for RAG phrase patterns from verb_rag_metadata
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".verb_pattern_embeddings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The verb this pattern maps to (e.g., "ui.follow-the-rabbit", "ubo.list-owners")
    verb_name VARCHAR(100) NOT NULL,

    -- The original pattern phrase (e.g., "follow the white rabbit", "who owns this")
    pattern_phrase TEXT NOT NULL,

    -- Normalized/lowercase version for exact matching fallback
    pattern_normalized TEXT NOT NULL,

    -- Double Metaphone encoding for phonetic matching
    -- Stores primary and secondary codes as array
    phonetic_codes TEXT[] NOT NULL DEFAULT '{}',

    -- The embedding vector (384 dimensions for all-MiniLM-L6-v2)
    embedding vector(384) NOT NULL,

    -- Category for filtering (navigation, investigation, workflow, etc.)
    category VARCHAR(50) NOT NULL DEFAULT 'navigation',

    -- Whether this is an agent-bound command vs local UI command
    is_agent_bound BOOLEAN NOT NULL DEFAULT false,

    -- Priority for disambiguation (lower = higher priority)
    priority INTEGER NOT NULL DEFAULT 50,

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Ensure no duplicate patterns for the same verb
    UNIQUE(verb_name, pattern_normalized)
);

-- Index for fast vector similarity search using IVFFlat
-- For ~1000 vectors, lists=10 is appropriate (sqrt(n) rule of thumb)
CREATE INDEX IF NOT EXISTS idx_verb_pattern_embedding_ivfflat
ON "ob-poc".verb_pattern_embeddings
USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 10);

-- Index for category filtering
CREATE INDEX IF NOT EXISTS idx_verb_pattern_category
ON "ob-poc".verb_pattern_embeddings(category);

-- Index for agent-bound filtering
CREATE INDEX IF NOT EXISTS idx_verb_pattern_agent_bound
ON "ob-poc".verb_pattern_embeddings(is_agent_bound);

-- GIN index for phonetic code array search
CREATE INDEX IF NOT EXISTS idx_verb_pattern_phonetic
ON "ob-poc".verb_pattern_embeddings
USING gin (phonetic_codes);

-- =============================================================================
-- SEMANTIC MATCH CACHE (Optional - for performance)
-- Caches recent transcript → verb matches to avoid re-embedding
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".semantic_match_cache (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The input transcript (normalized)
    transcript_normalized TEXT NOT NULL UNIQUE,

    -- The matched verb
    matched_verb VARCHAR(100) NOT NULL,

    -- Similarity score (0-1)
    similarity_score REAL NOT NULL,

    -- Match method used
    match_method VARCHAR(20) NOT NULL, -- 'semantic', 'phonetic', 'exact'

    -- Cache metadata
    hit_count INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_accessed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Auto-expire cache entries older than 24 hours (handled by application or cron)
CREATE INDEX IF NOT EXISTS idx_semantic_cache_accessed
ON "ob-poc".semantic_match_cache(last_accessed_at);

-- =============================================================================
-- HELPER FUNCTION: Find top-k similar patterns
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".find_similar_patterns(
    query_embedding vector(384),
    top_k INTEGER DEFAULT 5,
    min_similarity REAL DEFAULT 0.5,
    category_filter VARCHAR(50) DEFAULT NULL,
    agent_bound_filter BOOLEAN DEFAULT NULL
)
RETURNS TABLE (
    verb_name VARCHAR(100),
    pattern_phrase TEXT,
    similarity REAL,
    category VARCHAR(50),
    is_agent_bound BOOLEAN,
    priority INTEGER
)
LANGUAGE SQL
STABLE
AS $$
    SELECT
        vpe.verb_name,
        vpe.pattern_phrase,
        1 - (vpe.embedding <=> query_embedding) AS similarity,
        vpe.category,
        vpe.is_agent_bound,
        vpe.priority
    FROM "ob-poc".verb_pattern_embeddings vpe
    WHERE
        (category_filter IS NULL OR vpe.category = category_filter)
        AND (agent_bound_filter IS NULL OR vpe.is_agent_bound = agent_bound_filter)
        AND 1 - (vpe.embedding <=> query_embedding) >= min_similarity
    ORDER BY
        vpe.embedding <=> query_embedding,
        vpe.priority
    LIMIT top_k;
$$;

-- =============================================================================
-- HELPER FUNCTION: Find patterns by phonetic match
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".find_phonetic_matches(
    query_phonetic_codes TEXT[],
    top_k INTEGER DEFAULT 5
)
RETURNS TABLE (
    verb_name VARCHAR(100),
    pattern_phrase TEXT,
    category VARCHAR(50),
    is_agent_bound BOOLEAN,
    priority INTEGER,
    matching_codes TEXT[]
)
LANGUAGE SQL
STABLE
AS $$
    SELECT
        vpe.verb_name,
        vpe.pattern_phrase,
        vpe.category,
        vpe.is_agent_bound,
        vpe.priority,
        vpe.phonetic_codes & query_phonetic_codes AS matching_codes
    FROM "ob-poc".verb_pattern_embeddings vpe
    WHERE vpe.phonetic_codes && query_phonetic_codes
    ORDER BY
        cardinality(vpe.phonetic_codes & query_phonetic_codes) DESC,
        vpe.priority
    LIMIT top_k;
$$;

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE "ob-poc".verb_pattern_embeddings IS
    'Pre-computed embeddings for voice command patterns. Used for semantic similarity matching of voice transcripts to DSL verbs.';

COMMENT ON COLUMN "ob-poc".verb_pattern_embeddings.embedding IS
    'all-MiniLM-L6-v2 embedding (384 dimensions). Captures semantic meaning of pattern phrase.';

COMMENT ON COLUMN "ob-poc".verb_pattern_embeddings.phonetic_codes IS
    'Double Metaphone codes for phonetic fallback matching. Handles "enhawnce" → "enhance".';

COMMENT ON FUNCTION "ob-poc".find_similar_patterns IS
    'Find top-k verb patterns most semantically similar to query embedding. Uses cosine similarity.';
