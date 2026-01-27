-- Verb centroid vectors for semantic lane optimization
-- One stable "prototype" vector per verb (mean of normalized phrase embeddings)
--
-- This enables two-stage semantic search:
-- 1. Query ~500 centroids to get top-25 verb candidates (fast, stable)
-- 2. Refine with pattern-level matches within shortlist (precise, evidenced)

CREATE TABLE IF NOT EXISTS "ob-poc".verb_centroids (
    verb_name TEXT PRIMARY KEY,
    embedding VECTOR(384) NOT NULL,
    phrase_count INT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- IVFFlat index for fast cosine similarity search
-- lists = 100 is appropriate for ~500-1000 verbs
CREATE INDEX IF NOT EXISTS idx_verb_centroids_embedding_ivfflat
    ON "ob-poc".verb_centroids
    USING ivfflat (embedding vector_cosine_ops)
    WITH (lists = 100);

COMMENT ON TABLE "ob-poc".verb_centroids IS
    'Centroid vectors per verb - mean of normalized phrase embeddings for stable semantic matching';

COMMENT ON COLUMN "ob-poc".verb_centroids.verb_name IS 'Fully qualified verb name (e.g., cbu.create)';
COMMENT ON COLUMN "ob-poc".verb_centroids.embedding IS '384-dim BGE centroid vector (normalized mean of phrase embeddings)';
COMMENT ON COLUMN "ob-poc".verb_centroids.phrase_count IS 'Number of phrases averaged into this centroid';
COMMENT ON COLUMN "ob-poc".verb_centroids.updated_at IS 'Last centroid recomputation timestamp';
