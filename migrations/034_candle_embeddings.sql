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
