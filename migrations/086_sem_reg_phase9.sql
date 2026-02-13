-- ============================================================================
-- Phase 9: Lineage, Embeddings, Coverage Metrics
-- ============================================================================
-- Three append-only / versioned projection tables for impact analysis,
-- semantic search, and governance dashboards.
-- ============================================================================

-- ── Derivation Edges ─────────────────────────────────────────────────────────
-- Immutable, append-only.  Records which input snapshots produced which output
-- snapshot through a verb execution.

CREATE TABLE IF NOT EXISTS sem_reg.derivation_edges (
    edge_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    input_snapshot_ids UUID[] NOT NULL,
    output_snapshot_id UUID NOT NULL REFERENCES sem_reg.snapshots(snapshot_id),
    verb_fqn        TEXT NOT NULL,
    run_id          UUID,          -- optional link to run_records
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- GIN index on input array for forward-impact queries
CREATE INDEX IF NOT EXISTS idx_derivation_edges_inputs
    ON sem_reg.derivation_edges USING GIN (input_snapshot_ids);

-- B-tree index on output for reverse-provenance queries
CREATE INDEX IF NOT EXISTS idx_derivation_edges_output
    ON sem_reg.derivation_edges (output_snapshot_id);

-- Index on run_id for run-scoped edge lookups
CREATE INDEX IF NOT EXISTS idx_derivation_edges_run
    ON sem_reg.derivation_edges (run_id) WHERE run_id IS NOT NULL;


-- ── Run Records ──────────────────────────────────────────────────────────────
-- Immutable.  One record per plan-step execution capturing timing and context.

CREATE TABLE IF NOT EXISTS sem_reg.run_records (
    run_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id         UUID,          -- optional link to agent_plans
    step_id         UUID,          -- optional link to plan_steps
    verb_fqn        TEXT NOT NULL,
    started_at      TIMESTAMPTZ NOT NULL,
    completed_at    TIMESTAMPTZ NOT NULL,
    duration_ms     BIGINT NOT NULL,
    input_count     INTEGER NOT NULL DEFAULT 0,
    output_count    INTEGER NOT NULL DEFAULT 0,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for plan-scoped run lookups
CREATE INDEX IF NOT EXISTS idx_run_records_plan
    ON sem_reg.run_records (plan_id) WHERE plan_id IS NOT NULL;

-- Add FK from derivation_edges.run_id → run_records after table creation
ALTER TABLE sem_reg.derivation_edges
    ADD CONSTRAINT fk_derivation_edges_run
    FOREIGN KEY (run_id) REFERENCES sem_reg.run_records(run_id);


-- ── Embedding Records ────────────────────────────────────────────────────────
-- Versioned — one embedding per snapshot, updated when definition changes.
-- Staleness detected by comparing version_hash against current snapshot.

CREATE TABLE IF NOT EXISTS sem_reg.embedding_records (
    embedding_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id     UUID NOT NULL REFERENCES sem_reg.snapshots(snapshot_id) UNIQUE,
    object_type     TEXT NOT NULL,
    version_hash    TEXT NOT NULL,
    model_id        TEXT NOT NULL,
    dimensions      INTEGER NOT NULL,
    embedding       JSONB NOT NULL,     -- stored as JSON array of floats
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for staleness checks (snapshot_id is already UNIQUE)
CREATE INDEX IF NOT EXISTS idx_embedding_records_object_type
    ON sem_reg.embedding_records (object_type);


-- ── Convenience Views ────────────────────────────────────────────────────────

-- Stale embeddings view: embeddings whose version_hash doesn't match current snapshot
CREATE OR REPLACE VIEW sem_reg.v_stale_embeddings AS
SELECT
    e.embedding_id,
    e.snapshot_id,
    e.object_type,
    e.version_hash AS embedding_hash,
    md5(s.definition::text) AS current_hash,
    e.model_id,
    e.updated_at AS last_embedded_at
FROM sem_reg.embedding_records e
JOIN sem_reg.snapshots s ON s.snapshot_id = e.snapshot_id
WHERE e.version_hash IS DISTINCT FROM md5(s.definition::text);

-- Lineage summary view: edge counts per verb
CREATE OR REPLACE VIEW sem_reg.v_lineage_summary AS
SELECT
    verb_fqn,
    COUNT(*) AS edge_count,
    MIN(created_at) AS first_edge,
    MAX(created_at) AS last_edge
FROM sem_reg.derivation_edges
GROUP BY verb_fqn
ORDER BY edge_count DESC;

-- Run performance view: average duration by verb
CREATE OR REPLACE VIEW sem_reg.v_run_performance AS
SELECT
    verb_fqn,
    COUNT(*) AS run_count,
    ROUND(AVG(duration_ms)::numeric, 1) AS avg_ms,
    MAX(duration_ms) AS max_ms,
    SUM(input_count) AS total_inputs,
    SUM(output_count) AS total_outputs
FROM sem_reg.run_records
GROUP BY verb_fqn
ORDER BY run_count DESC;
