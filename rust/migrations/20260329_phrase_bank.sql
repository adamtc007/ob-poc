-- Migration 20260329: Phrase bank schema — governed phrase storage with collision-safe indexing.

CREATE TABLE IF NOT EXISTS "ob-poc".phrase_bank (
    id                  SERIAL PRIMARY KEY,
    phrase              TEXT NOT NULL,
    verb_fqn            TEXT NOT NULL,
    workspace           TEXT,
    source              TEXT NOT NULL DEFAULT 'governed',
    risk_tier           TEXT NOT NULL DEFAULT 'elevated',
    sem_reg_snapshot_id UUID,
    supersedes_id       INT REFERENCES "ob-poc".phrase_bank(id),
    active              BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- At most one active phrase per (phrase, workspace) pair
CREATE UNIQUE INDEX IF NOT EXISTS idx_phrase_bank_active_unique
    ON "ob-poc".phrase_bank (phrase, COALESCE(workspace, '__global__'))
    WHERE (active = TRUE);

-- Fast lookup for Tier 0
CREATE INDEX IF NOT EXISTS idx_phrase_bank_lookup
    ON "ob-poc".phrase_bank (phrase, workspace)
    WHERE (active = TRUE);

-- Workspace column on verb_pattern_embeddings
ALTER TABLE "ob-poc".verb_pattern_embeddings
    ADD COLUMN IF NOT EXISTS workspace TEXT;

COMMENT ON TABLE "ob-poc".phrase_bank IS 'Governed phrase bank — curated phrase-to-verb mappings with collision safety';
