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
