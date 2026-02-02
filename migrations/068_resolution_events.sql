-- Migration: 068_resolution_events.sql
-- Purpose: Learning loop events for scope resolution
--
-- This table captures user interactions with scope resolution:
-- - What scopes were created
-- - What users selected vs what was offered
-- - When users narrowed, widened, or rejected suggestions
--
-- Used for:
-- 1. Hit-rate analysis (how often top-1 is correct)
-- 2. Learning signal generation (improve future matching)
-- 3. Audit trail (who did what when)

-- =============================================================================
-- RESOLUTION EVENTS TABLE
-- =============================================================================

CREATE TABLE "ob-poc".resolution_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Session context (nullable for batch operations)
    session_id UUID,

    -- Link to snapshot (if event relates to a specific resolution)
    snapshot_id UUID REFERENCES "ob-poc".scope_snapshots(id) ON DELETE SET NULL,

    -- =========================================================================
    -- EVENT CLASSIFICATION
    -- =========================================================================

    event_type TEXT NOT NULL CHECK (event_type IN (
        -- Scope lifecycle events
        'scope_created',       -- New scope resolved (candidates generated)
        'scope_committed',     -- User confirmed scope (frozen to snapshot)
        'scope_rejected',      -- User rejected entire suggestion
        'scope_narrowed',      -- User refined to subset
        'scope_widened',       -- User requested more entities
        'scope_refreshed',     -- User triggered re-resolution

        -- Selection events
        'candidate_selected',  -- User picked specific entity from candidates
        'candidate_deselected', -- User removed entity from selection

        -- Group/anchor events
        'group_anchored',      -- User set client group context
        'group_changed',       -- User switched to different group

        -- Error/recovery events
        'resolution_failed',   -- Resolution produced no matches
        'ambiguity_shown',     -- Disambiguation UI shown to user
        'timeout_expired'      -- User didn't respond to disambiguation
    )),

    -- =========================================================================
    -- EVENT PAYLOAD (type-specific data)
    -- =========================================================================

    -- Flexible payload for event-specific details
    -- Schema depends on event_type:
    --
    -- scope_created:
    --   { "description": "...", "candidate_count": 10, "method": "semantic" }
    --
    -- scope_committed:
    --   { "selected_count": 5, "from_position": [0,1,3], "confidence": 0.92 }
    --
    -- candidate_selected:
    --   { "entity_id": "uuid", "position": 2, "score": 0.87 }
    --
    -- scope_narrowed:
    --   { "filter": {"tags": ["ETF"]}, "before_count": 10, "after_count": 3 }
    --
    -- group_anchored:
    --   { "group_id": "uuid", "group_name": "Allianz" }
    --
    payload JSONB NOT NULL DEFAULT '{}',

    -- =========================================================================
    -- USER CONTEXT
    -- =========================================================================

    -- User who triggered this event (nullable for system events)
    user_id TEXT,

    -- Client IP for security audit (nullable)
    client_ip INET,

    -- =========================================================================
    -- TIMING
    -- =========================================================================

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Session timeline (for UI showing session history)
CREATE INDEX idx_re_session ON "ob-poc".resolution_events(session_id, created_at DESC)
    WHERE session_id IS NOT NULL;

-- Snapshot events (for getting all events for a snapshot)
CREATE INDEX idx_re_snapshot ON "ob-poc".resolution_events(snapshot_id)
    WHERE snapshot_id IS NOT NULL;

-- Event type filtering (for analytics by event type)
CREATE INDEX idx_re_type ON "ob-poc".resolution_events(event_type);

-- Time-based queries (for learning batch jobs)
CREATE INDEX idx_re_created ON "ob-poc".resolution_events(created_at DESC);

-- User activity (for per-user analytics)
CREATE INDEX idx_re_user ON "ob-poc".resolution_events(user_id)
    WHERE user_id IS NOT NULL;

-- Ambiguity analysis (for measuring disambiguation effectiveness)
CREATE INDEX idx_re_ambiguity ON "ob-poc".resolution_events(event_type, created_at DESC)
    WHERE event_type IN ('ambiguity_shown', 'scope_committed', 'scope_rejected');

-- =============================================================================
-- LEARNING ANALYTICS VIEWS
-- =============================================================================

-- View: Daily hit-rate (how often top-1 is accepted)
CREATE OR REPLACE VIEW "ob-poc".v_scope_hit_rate AS
SELECT
    DATE_TRUNC('day', created_at) AS day,
    COUNT(*) FILTER (WHERE event_type = 'scope_created') AS resolutions,
    COUNT(*) FILTER (WHERE event_type = 'scope_committed') AS commits,
    COUNT(*) FILTER (WHERE event_type = 'scope_rejected') AS rejections,
    COUNT(*) FILTER (WHERE event_type = 'scope_narrowed') AS narrows,
    COUNT(*) FILTER (WHERE event_type = 'ambiguity_shown') AS ambiguities,
    -- Hit rate: commits / resolutions
    ROUND(
        COUNT(*) FILTER (WHERE event_type = 'scope_committed')::NUMERIC /
        NULLIF(COUNT(*) FILTER (WHERE event_type = 'scope_created'), 0),
        3
    ) AS hit_rate,
    -- Ambiguity rate: ambiguities / resolutions
    ROUND(
        COUNT(*) FILTER (WHERE event_type = 'ambiguity_shown')::NUMERIC /
        NULLIF(COUNT(*) FILTER (WHERE event_type = 'scope_created'), 0),
        3
    ) AS ambiguity_rate,
    -- Narrow rate: narrows / commits (how often users refine)
    ROUND(
        COUNT(*) FILTER (WHERE event_type = 'scope_narrowed')::NUMERIC /
        NULLIF(COUNT(*) FILTER (WHERE event_type = 'scope_committed'), 0),
        3
    ) AS narrow_rate
FROM "ob-poc".resolution_events
GROUP BY DATE_TRUNC('day', created_at)
ORDER BY day DESC;

-- View: Resolution method effectiveness
CREATE OR REPLACE VIEW "ob-poc".v_resolution_method_stats AS
SELECT
    ss.resolution_method,
    COUNT(DISTINCT ss.id) AS total_snapshots,
    AVG(ss.overall_confidence) AS avg_confidence,
    COUNT(DISTINCT re.id) FILTER (WHERE re.event_type = 'scope_committed') AS commits,
    COUNT(DISTINCT re.id) FILTER (WHERE re.event_type = 'scope_rejected') AS rejections,
    ROUND(
        COUNT(DISTINCT re.id) FILTER (WHERE re.event_type = 'scope_committed')::NUMERIC /
        NULLIF(COUNT(DISTINCT ss.id), 0),
        3
    ) AS commit_rate
FROM "ob-poc".scope_snapshots ss
LEFT JOIN "ob-poc".resolution_events re ON re.snapshot_id = ss.id
WHERE ss.created_at > NOW() - INTERVAL '30 days'
GROUP BY ss.resolution_method
ORDER BY total_snapshots DESC;

-- View: User activity summary
CREATE OR REPLACE VIEW "ob-poc".v_user_resolution_activity AS
SELECT
    user_id,
    COUNT(*) AS total_events,
    COUNT(DISTINCT session_id) AS sessions,
    COUNT(*) FILTER (WHERE event_type = 'scope_committed') AS commits,
    COUNT(*) FILTER (WHERE event_type = 'scope_narrowed') AS narrows,
    COUNT(*) FILTER (WHERE event_type = 'scope_rejected') AS rejections,
    MIN(created_at) AS first_event,
    MAX(created_at) AS last_event
FROM "ob-poc".resolution_events
WHERE user_id IS NOT NULL
  AND created_at > NOW() - INTERVAL '30 days'
GROUP BY user_id
ORDER BY total_events DESC;

-- View: Session resolution timeline
CREATE OR REPLACE VIEW "ob-poc".v_session_resolution_timeline AS
SELECT
    re.session_id,
    re.event_type,
    re.payload,
    re.created_at,
    ss.description AS scope_description,
    ss.entity_count,
    ss.resolution_method,
    ss.overall_confidence
FROM "ob-poc".resolution_events re
LEFT JOIN "ob-poc".scope_snapshots ss ON ss.id = re.snapshot_id
WHERE re.session_id IS NOT NULL
ORDER BY re.session_id, re.created_at;

-- =============================================================================
-- LEARNING SIGNAL EXTRACTION FUNCTION
-- =============================================================================

-- Function: Extract learning signals from recent events
-- Used by batch learning job to identify patterns worth promoting
CREATE OR REPLACE FUNCTION "ob-poc".extract_scope_learning_signals(
    days_back INTEGER DEFAULT 7,
    min_occurrences INTEGER DEFAULT 3
)
RETURNS TABLE (
    description TEXT,
    resolution_method TEXT,
    group_id UUID,
    avg_confidence DECIMAL(3,2),
    commit_count BIGINT,
    reject_count BIGINT,
    narrow_count BIGINT,
    total_occurrences BIGINT,
    success_rate DECIMAL(3,2)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        ss.description,
        ss.resolution_method,
        ss.group_id,
        AVG(ss.overall_confidence)::DECIMAL(3,2) AS avg_confidence,
        COUNT(*) FILTER (WHERE re.event_type = 'scope_committed') AS commit_count,
        COUNT(*) FILTER (WHERE re.event_type = 'scope_rejected') AS reject_count,
        COUNT(*) FILTER (WHERE re.event_type = 'scope_narrowed') AS narrow_count,
        COUNT(DISTINCT ss.id) AS total_occurrences,
        ROUND(
            COUNT(*) FILTER (WHERE re.event_type = 'scope_committed')::NUMERIC /
            NULLIF(COUNT(DISTINCT ss.id), 0),
            2
        )::DECIMAL(3,2) AS success_rate
    FROM "ob-poc".scope_snapshots ss
    LEFT JOIN "ob-poc".resolution_events re ON re.snapshot_id = ss.id
    WHERE ss.created_at > NOW() - (days_back || ' days')::INTERVAL
      AND ss.description IS NOT NULL
    GROUP BY ss.description, ss.resolution_method, ss.group_id
    HAVING COUNT(DISTINCT ss.id) >= min_occurrences
    ORDER BY total_occurrences DESC, success_rate DESC;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE "ob-poc".resolution_events IS
    'Learning loop events for scope resolution. Tracks user interactions for hit-rate analysis and model improvement.';

COMMENT ON COLUMN "ob-poc".resolution_events.event_type IS
    'Type of resolution event. Used for filtering and analytics.';

COMMENT ON COLUMN "ob-poc".resolution_events.payload IS
    'Event-specific data in JSONB. Schema depends on event_type.';

COMMENT ON VIEW "ob-poc".v_scope_hit_rate IS
    'Daily hit-rate metrics. Key KPI: how often top-1 suggestion is accepted.';

COMMENT ON FUNCTION "ob-poc".extract_scope_learning_signals IS
    'Extract patterns worth promoting to learned phrases. Used by batch learning job.';
