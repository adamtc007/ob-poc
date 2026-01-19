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
