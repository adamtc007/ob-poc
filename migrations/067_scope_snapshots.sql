-- Migration: 067_scope_snapshots.sql
-- Purpose: Immutable scope resolution snapshots for deterministic replay and learning
--
-- This table stores the result of scope resolution, enabling:
-- 1. Deterministic replay (same program, same results)
-- 2. Learning loop (track what users selected vs what was offered)
-- 3. Audit trail (who resolved what scope when)
--
-- Key invariant: Snapshots are IMMUTABLE after creation (enforced by trigger)

-- =============================================================================
-- SCOPE SNAPSHOTS TABLE
-- =============================================================================

CREATE TABLE "ob-poc".scope_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- =========================================================================
    -- DESCRIPTOR (what was requested)
    -- =========================================================================

    -- Client group context (required - scope is always within a group)
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,

    -- Natural language description of what was requested
    -- e.g., "Irish ETF funds", "Luxembourg ManCos"
    description TEXT,

    -- Filter criteria applied (JSONB for flexibility)
    -- e.g., {"jurisdiction": "IE", "tags": ["ETF", "FUND"], "roles": ["custodian"]}
    filter_applied JSONB,

    -- Maximum entities requested (enforced policy limit)
    limit_requested INTEGER NOT NULL CHECK (limit_requested > 0 AND limit_requested <= 1000),

    -- Resolution mode that was used
    -- strict: fail if ambiguous
    -- interactive: prompt for clarification
    -- greedy: take top-1 (audit logged)
    mode TEXT NOT NULL DEFAULT 'interactive'
        CHECK (mode IN ('strict', 'interactive', 'greedy')),

    -- =========================================================================
    -- RESOLVED LIST (deterministic output)
    -- =========================================================================

    -- Ordered array of resolved entity IDs
    -- Order: score DESC, entity_id ASC (for determinism)
    selected_entity_ids UUID[] NOT NULL,

    -- Computed count (for quick queries without array_length)
    entity_count INTEGER GENERATED ALWAYS AS (array_length(selected_entity_ids, 1)) STORED,

    -- =========================================================================
    -- SCORING SUMMARY (for learning/debug)
    -- =========================================================================

    -- Top-k candidates with scores (even those not selected)
    -- Format: [{"entity_id": "uuid", "name": "...", "score": 0.95, "method": "exact"}, ...]
    top_k_candidates JSONB NOT NULL DEFAULT '[]',

    -- Primary method that produced the match
    -- exact_phrase: direct match on scope_phrases
    -- role_tag: matched via role_tags
    -- semantic: Candle embedding similarity
    -- authoritative: direct entity attribute query
    -- fuzzy_text: trigram/fuzzy text matching via search_entity_tags()
    -- semantic_fallback: semantic search fell back to fuzzy
    -- semantic_enhanced_fuzzy: semantic mode using enhanced fuzzy search
    -- narrowed: created via scope.narrow from existing scope
    -- union: created via scope.union combining multiple scopes
    resolution_method TEXT NOT NULL CHECK (resolution_method IN (
        'exact_phrase', 'role_tag', 'semantic', 'authoritative', 'mixed',
        'fuzzy_text', 'semantic_fallback', 'semantic_enhanced_fuzzy',
        'narrowed', 'union'
    )),

    -- Overall confidence (0.00 - 1.00)
    overall_confidence DECIMAL(3,2) CHECK (overall_confidence >= 0 AND overall_confidence <= 1),

    -- =========================================================================
    -- INDEX FINGERPRINTS (drift detection)
    -- =========================================================================

    -- Embedding model version used for semantic search
    embedder_version TEXT,

    -- Hash of role_tags state at resolution time (for drift detection)
    role_tags_hash TEXT,

    -- =========================================================================
    -- LINEAGE (for scope.refresh)
    -- =========================================================================

    -- Parent snapshot if this was created via scope.refresh
    parent_snapshot_id UUID REFERENCES "ob-poc".scope_snapshots(id),

    -- =========================================================================
    -- AUDIT
    -- =========================================================================

    -- Session that created this snapshot (nullable for batch/runbook)
    session_id UUID,

    -- Creation timestamp
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Who/what created this: 'agent', 'user', 'runbook:{id}', 'batch:{id}'
    created_by TEXT NOT NULL DEFAULT 'agent',

    -- =========================================================================
    -- CONSTRAINTS
    -- =========================================================================

    -- Array must be non-empty (a scope with 0 entities is invalid)
    CONSTRAINT chk_ss_nonempty CHECK (array_length(selected_entity_ids, 1) > 0)
);

-- =============================================================================
-- IMMUTABILITY TRIGGER
-- =============================================================================

-- Snapshots are IMMUTABLE after creation - this is a core invariant
-- for deterministic replay. Updates are forbidden.

CREATE OR REPLACE FUNCTION "ob-poc".prevent_snapshot_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'scope_snapshots are immutable after creation. Create a new snapshot via scope.refresh instead.';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER snapshot_immutable
    BEFORE UPDATE ON "ob-poc".scope_snapshots
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".prevent_snapshot_update();

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Session-based lookup (for UI showing session's scope history)
CREATE INDEX idx_ss_session ON "ob-poc".scope_snapshots(session_id)
    WHERE session_id IS NOT NULL;

-- Group-based lookup (for analytics by client)
CREATE INDEX idx_ss_group ON "ob-poc".scope_snapshots(group_id);

-- Recent snapshots (for cleanup/retention policies)
CREATE INDEX idx_ss_created ON "ob-poc".scope_snapshots(created_at DESC);

-- Parent chain navigation (for scope.refresh lineage)
CREATE INDEX idx_ss_parent ON "ob-poc".scope_snapshots(parent_snapshot_id)
    WHERE parent_snapshot_id IS NOT NULL;

-- Method distribution (for learning analytics)
CREATE INDEX idx_ss_method ON "ob-poc".scope_snapshots(resolution_method);

-- =============================================================================
-- HELPER VIEWS
-- =============================================================================

-- View: Recent scope resolutions with entity names
CREATE OR REPLACE VIEW "ob-poc".v_recent_scope_snapshots AS
SELECT
    ss.id,
    ss.description,
    ss.entity_count,
    ss.resolution_method,
    ss.overall_confidence,
    ss.mode,
    ss.created_at,
    ss.created_by,
    cg.canonical_name AS group_name,
    ss.session_id
FROM "ob-poc".scope_snapshots ss
JOIN "ob-poc".client_group cg ON cg.id = ss.group_id
ORDER BY ss.created_at DESC
LIMIT 100;

-- View: Scope resolution quality metrics
CREATE OR REPLACE VIEW "ob-poc".v_scope_resolution_quality AS
SELECT
    DATE_TRUNC('day', created_at) AS day,
    resolution_method,
    COUNT(*) AS total_resolutions,
    AVG(overall_confidence) AS avg_confidence,
    AVG(entity_count) AS avg_entity_count,
    COUNT(*) FILTER (WHERE overall_confidence >= 0.90) AS high_confidence_count,
    COUNT(*) FILTER (WHERE overall_confidence < 0.70) AS low_confidence_count
FROM "ob-poc".scope_snapshots
GROUP BY DATE_TRUNC('day', created_at), resolution_method
ORDER BY day DESC, resolution_method;

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE "ob-poc".scope_snapshots IS
    'Immutable snapshots of scope resolution results. Used for deterministic replay and learning.';

COMMENT ON COLUMN "ob-poc".scope_snapshots.selected_entity_ids IS
    'Ordered array of resolved entity IDs. Order: score DESC, entity_id ASC for determinism.';

COMMENT ON COLUMN "ob-poc".scope_snapshots.top_k_candidates IS
    'Full top-k candidates with scores for learning. Format: [{entity_id, name, score, method}, ...]';

COMMENT ON COLUMN "ob-poc".scope_snapshots.parent_snapshot_id IS
    'Parent snapshot if created via scope.refresh. Enables lineage tracking.';

COMMENT ON TRIGGER snapshot_immutable ON "ob-poc".scope_snapshots IS
    'Enforces immutability invariant. Snapshots cannot be modified after creation.';
