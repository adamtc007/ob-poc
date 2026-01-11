-- ============================================================================
-- Migration 017: Event Infrastructure
-- ============================================================================
--
-- Creates tables for the always-on event capture system (023a).
--
-- Two main tables:
-- 1. events.log - Append-only DSL execution events
-- 2. sessions.log - Conversation context for session replay
--
-- Design principles:
-- - Append-only (no updates for write performance)
-- - Minimal indexes (partitioned by timestamp)
-- - JSONB payload for flexibility
--
-- ============================================================================

-- Create schemas if not exist
CREATE SCHEMA IF NOT EXISTS events;
CREATE SCHEMA IF NOT EXISTS sessions;

-- ============================================================================
-- events.log - DSL Execution Events
-- ============================================================================
--
-- Captures every DSL command execution (success and failure).
-- Used by the Feedback Inspector for failure analysis.

CREATE TABLE IF NOT EXISTS events.log (
    id BIGSERIAL PRIMARY KEY,

    -- When the event occurred
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Session context (nullable for non-session events)
    session_id UUID,

    -- Event type for quick filtering
    event_type TEXT NOT NULL,
    -- Values: 'command_succeeded', 'command_failed', 'session_started', 'session_ended'

    -- Full event payload as JSONB
    payload JSONB NOT NULL,

    -- Constraint on event_type
    CONSTRAINT valid_event_type CHECK (
        event_type IN ('command_succeeded', 'command_failed', 'session_started', 'session_ended')
    )
);

-- Index for time-based queries (partition key for future partitioning)
CREATE INDEX IF NOT EXISTS idx_events_log_timestamp
    ON events.log (timestamp);

-- Index for session lookups
CREATE INDEX IF NOT EXISTS idx_events_log_session
    ON events.log (session_id, timestamp)
    WHERE session_id IS NOT NULL;

-- Index for failure analysis
CREATE INDEX IF NOT EXISTS idx_events_log_failures
    ON events.log (timestamp)
    WHERE event_type = 'command_failed';

-- Comment
COMMENT ON TABLE events.log IS 'Append-only DSL execution events for observability and failure analysis';

-- ============================================================================
-- sessions.log - Conversation Context
-- ============================================================================
--
-- Captures the full conversation context for a session:
-- - User input
-- - Agent thoughts (in agent mode)
-- - DSL commands
-- - Responses
-- - Errors
--
-- This enables session replay and context-aware failure analysis.

CREATE TABLE IF NOT EXISTS sessions.log (
    id BIGSERIAL PRIMARY KEY,

    -- Session this entry belongs to
    session_id UUID NOT NULL,

    -- When this entry was created
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Entry type
    entry_type TEXT NOT NULL,
    -- Values: 'user_input', 'agent_thought', 'dsl_command', 'response', 'error'

    -- The actual content
    content TEXT NOT NULL,

    -- Link to corresponding event (for DSL commands and errors)
    event_id BIGINT REFERENCES events.log(id),

    -- Source of this session
    source TEXT NOT NULL,
    -- Values: 'repl', 'egui', 'mcp', 'api'

    -- Optional metadata (command args, error context, etc.)
    metadata JSONB DEFAULT '{}',

    -- Constraint on entry_type
    CONSTRAINT valid_entry_type CHECK (
        entry_type IN ('user_input', 'agent_thought', 'dsl_command', 'response', 'error')
    ),

    -- Constraint on source
    CONSTRAINT valid_source CHECK (
        source IN ('repl', 'egui', 'mcp', 'api')
    )
);

-- Primary lookup: entries for a session in order
CREATE INDEX IF NOT EXISTS idx_sessions_log_session
    ON sessions.log (session_id, timestamp);

-- Lookup by event_id (for linking events to context)
CREATE INDEX IF NOT EXISTS idx_sessions_log_event
    ON sessions.log (event_id)
    WHERE event_id IS NOT NULL;

-- Comment
COMMENT ON TABLE sessions.log IS 'Conversation context log for session replay and failure analysis';

-- ============================================================================
-- Helper Views
-- ============================================================================

-- View: Recent failures with session context
CREATE OR REPLACE VIEW events.recent_failures AS
SELECT
    e.id AS event_id,
    e.timestamp,
    e.session_id,
    e.payload->>'verb' AS verb,
    e.payload->'error'->>'message' AS error_message,
    e.payload->'error'->>'error_type' AS error_type,
    (e.payload->>'duration_ms')::integer AS duration_ms
FROM events.log e
WHERE e.event_type = 'command_failed'
ORDER BY e.timestamp DESC
LIMIT 100;

COMMENT ON VIEW events.recent_failures IS 'Recent command failures for quick inspection';

-- View: Session summary
CREATE OR REPLACE VIEW events.session_summary AS
SELECT
    session_id,
    MIN(timestamp) AS started_at,
    MAX(timestamp) AS last_activity,
    COUNT(*) FILTER (WHERE event_type = 'command_succeeded') AS success_count,
    COUNT(*) FILTER (WHERE event_type = 'command_failed') AS failure_count,
    COUNT(*) AS total_events
FROM events.log
WHERE session_id IS NOT NULL
GROUP BY session_id
ORDER BY MAX(timestamp) DESC;

COMMENT ON VIEW events.session_summary IS 'Per-session event summary';

-- ============================================================================
-- Maintenance Functions
-- ============================================================================

-- Function to clean old events (call periodically)
CREATE OR REPLACE FUNCTION events.cleanup_old_events(retention_days INTEGER DEFAULT 30)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM events.log
    WHERE timestamp < NOW() - (retention_days || ' days')::INTERVAL;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION events.cleanup_old_events IS 'Delete events older than retention_days (default 30)';

-- Function to clean old session logs
CREATE OR REPLACE FUNCTION sessions.cleanup_old_logs(retention_days INTEGER DEFAULT 30)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM sessions.log
    WHERE timestamp < NOW() - (retention_days || ' days')::INTERVAL;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION sessions.cleanup_old_logs IS 'Delete session logs older than retention_days (default 30)';
