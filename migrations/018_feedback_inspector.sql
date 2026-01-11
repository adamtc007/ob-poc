-- 018: Feedback Inspector Schema
-- Implements 023b: On-demand failure analysis, repro generation, audit trail

-- Create feedback schema
CREATE SCHEMA IF NOT EXISTS feedback;

-- =============================================================================
-- ENUMS
-- =============================================================================

-- Error type classification
CREATE TYPE feedback.error_type AS ENUM (
    -- Transient (runtime retry candidates)
    'TIMEOUT',
    'RATE_LIMITED',
    'CONNECTION_RESET',
    'SERVICE_UNAVAILABLE',
    'POOL_EXHAUSTED',

    -- Schema/contract issues (code fix required)
    'ENUM_DRIFT',
    'SCHEMA_DRIFT',

    -- Code bugs (investigation needed)
    'PARSE_ERROR',
    'HANDLER_PANIC',
    'HANDLER_ERROR',
    'DSL_PARSE_ERROR',

    -- External API changes
    'API_ENDPOINT_MOVED',
    'API_AUTH_CHANGED',
    'VALIDATION_FAILED',

    -- Catch-all
    'UNKNOWN'
);

-- Remediation path
CREATE TYPE feedback.remediation_path AS ENUM (
    'RUNTIME',    -- Can be retried/recovered at runtime
    'CODE',       -- Requires code change
    'LOG_ONLY'    -- Just log, no action needed
);

-- Issue lifecycle status
CREATE TYPE feedback.issue_status AS ENUM (
    -- Initial states
    'NEW',
    'RUNTIME_RESOLVED',
    'RUNTIME_ESCALATED',

    -- Repro states
    'REPRO_GENERATED',
    'REPRO_VERIFIED',
    'TODO_CREATED',

    -- Fix states
    'IN_PROGRESS',
    'FIX_COMMITTED',
    'FIX_VERIFIED',

    -- Deployment states
    'DEPLOYED_STAGING',
    'DEPLOYED_PROD',
    'RESOLVED',

    -- Terminal states
    'WONT_FIX',
    'DUPLICATE',
    'INVALID'
);

-- Actor types for audit trail
CREATE TYPE feedback.actor_type AS ENUM (
    'SYSTEM',
    'MCP_AGENT',
    'REPL_USER',
    'EGUI_USER',
    'CI_PIPELINE',
    'CLAUDE_CODE',
    'CRON_JOB'
);

-- Audit actions
CREATE TYPE feedback.audit_action AS ENUM (
    -- Creation
    'CAPTURED',
    'CLASSIFIED',
    'DEDUPLICATED',

    -- Runtime handling
    'RUNTIME_ATTEMPT',
    'RUNTIME_SUCCESS',
    'RUNTIME_EXHAUSTED',

    -- Repro workflow
    'REPRO_GENERATED',
    'REPRO_VERIFIED_FAILS',
    'REPRO_VERIFICATION_FAILED',

    -- TODO workflow
    'TODO_CREATED',
    'TODO_ASSIGNED',
    'FIX_COMMITTED',

    -- Verification
    'REPRO_VERIFIED_PASSES',
    'DEPLOYED',
    'SEMANTIC_REPLAY_PASSED',
    'SEMANTIC_REPLAY_FAILED',

    -- Terminal
    'RESOLVED',
    'MARKED_WONT_FIX',
    'MARKED_DUPLICATE',
    'REOPENED',
    'COMMENT_ADDED'
);

-- =============================================================================
-- TABLES
-- =============================================================================

-- Main failure table (deduplicated by fingerprint)
CREATE TABLE feedback.failures (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fingerprint     TEXT NOT NULL UNIQUE,
    fingerprint_version SMALLINT NOT NULL DEFAULT 1,

    -- Classification
    error_type      feedback.error_type NOT NULL,
    remediation_path feedback.remediation_path NOT NULL,

    -- Status
    status          feedback.issue_status NOT NULL DEFAULT 'NEW',

    -- Source info
    verb            TEXT NOT NULL,
    source          TEXT,  -- e.g., "gleif", "lbr", null for internal

    -- Error details (redacted)
    error_message   TEXT NOT NULL,
    error_context   JSONB,  -- Redacted context for debugging

    -- Session context (what was user trying to do?)
    user_intent     TEXT,
    command_sequence TEXT[],  -- Recent commands leading to failure

    -- Repro info
    repro_type      TEXT,  -- 'golden_json', 'dsl_scenario', 'unit_test'
    repro_path      TEXT,  -- Path to generated test
    repro_verified  BOOLEAN DEFAULT FALSE,

    -- Fix info
    fix_commit      TEXT,
    fix_notes       TEXT,

    -- Counts
    occurrence_count INTEGER NOT NULL DEFAULT 1,

    -- Timestamps
    first_seen_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at     TIMESTAMPTZ,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Individual occurrences (each time we see this fingerprint)
CREATE TABLE feedback.occurrences (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    failure_id      UUID NOT NULL REFERENCES feedback.failures(id) ON DELETE CASCADE,

    -- Event reference
    event_id        UUID,  -- Reference to events.log if stored there
    event_timestamp TIMESTAMPTZ NOT NULL,

    -- Session info
    session_id      UUID,

    -- Execution context
    verb            TEXT NOT NULL,
    duration_ms     BIGINT,

    -- Error snapshot (redacted)
    error_message   TEXT NOT NULL,
    error_backtrace TEXT,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Full audit trail
CREATE TABLE feedback.audit_log (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    failure_id      UUID NOT NULL REFERENCES feedback.failures(id) ON DELETE CASCADE,

    -- Action
    action          feedback.audit_action NOT NULL,

    -- Actor
    actor_type      feedback.actor_type NOT NULL,
    actor_id        TEXT,  -- e.g., session ID, user ID, CI job ID

    -- Details
    details         JSONB,

    -- Evidence (for verification actions)
    evidence        TEXT,  -- e.g., test output
    evidence_hash   TEXT,  -- SHA256 of large evidence

    -- Previous state (for state transitions)
    previous_status feedback.issue_status,
    new_status      feedback.issue_status,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Fingerprint lookup (most common query)
CREATE INDEX idx_failures_fingerprint ON feedback.failures(fingerprint);

-- Status-based queries
CREATE INDEX idx_failures_status ON feedback.failures(status);
CREATE INDEX idx_failures_status_error_type ON feedback.failures(status, error_type);

-- Time-based queries
CREATE INDEX idx_failures_last_seen ON feedback.failures(last_seen_at DESC);
CREATE INDEX idx_failures_first_seen ON feedback.failures(first_seen_at DESC);

-- Source/verb queries
CREATE INDEX idx_failures_verb ON feedback.failures(verb);
CREATE INDEX idx_failures_source ON feedback.failures(source) WHERE source IS NOT NULL;

-- Occurrences
CREATE INDEX idx_occurrences_failure_id ON feedback.occurrences(failure_id);
CREATE INDEX idx_occurrences_session_id ON feedback.occurrences(session_id) WHERE session_id IS NOT NULL;
CREATE INDEX idx_occurrences_timestamp ON feedback.occurrences(event_timestamp DESC);

-- Audit log
CREATE INDEX idx_audit_failure_id ON feedback.audit_log(failure_id);
CREATE INDEX idx_audit_action ON feedback.audit_log(action);
CREATE INDEX idx_audit_created_at ON feedback.audit_log(created_at DESC);

-- =============================================================================
-- VIEWS
-- =============================================================================

-- Active issues needing attention
CREATE VIEW feedback.active_issues AS
SELECT
    f.id,
    f.fingerprint,
    f.error_type,
    f.remediation_path,
    f.status,
    f.verb,
    f.source,
    f.error_message,
    f.user_intent,
    f.occurrence_count,
    f.first_seen_at,
    f.last_seen_at,
    f.repro_verified
FROM feedback.failures f
WHERE f.status NOT IN ('RESOLVED', 'WONT_FIX', 'DUPLICATE', 'INVALID')
ORDER BY
    CASE f.remediation_path
        WHEN 'CODE' THEN 1
        WHEN 'RUNTIME' THEN 2
        ELSE 3
    END,
    f.occurrence_count DESC,
    f.last_seen_at DESC;

-- Issues ready for TODO generation (verified repro but no TODO yet)
CREATE VIEW feedback.ready_for_todo AS
SELECT
    f.id,
    f.fingerprint,
    f.error_type,
    f.verb,
    f.source,
    f.error_message,
    f.user_intent,
    f.repro_path,
    f.occurrence_count
FROM feedback.failures f
WHERE f.status = 'REPRO_VERIFIED'
  AND f.repro_verified = TRUE
ORDER BY f.occurrence_count DESC;

-- Recent audit activity
CREATE VIEW feedback.recent_activity AS
SELECT
    a.id,
    a.failure_id,
    f.fingerprint,
    a.action,
    a.actor_type,
    a.actor_id,
    a.previous_status,
    a.new_status,
    a.created_at
FROM feedback.audit_log a
JOIN feedback.failures f ON f.id = a.failure_id
ORDER BY a.created_at DESC
LIMIT 100;

-- =============================================================================
-- FUNCTIONS
-- =============================================================================

-- Update timestamps trigger
CREATE OR REPLACE FUNCTION feedback.update_timestamps()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER failures_update_timestamps
    BEFORE UPDATE ON feedback.failures
    FOR EACH ROW
    EXECUTE FUNCTION feedback.update_timestamps();

-- Record occurrence and update failure counts
CREATE OR REPLACE FUNCTION feedback.record_occurrence(
    p_fingerprint TEXT,
    p_event_id UUID,
    p_event_timestamp TIMESTAMPTZ,
    p_session_id UUID,
    p_verb TEXT,
    p_duration_ms BIGINT,
    p_error_message TEXT,
    p_error_backtrace TEXT
) RETURNS UUID AS $$
DECLARE
    v_failure_id UUID;
    v_occurrence_id UUID;
BEGIN
    -- Get failure ID
    SELECT id INTO v_failure_id
    FROM feedback.failures
    WHERE fingerprint = p_fingerprint;

    IF v_failure_id IS NULL THEN
        RAISE EXCEPTION 'Failure not found for fingerprint: %', p_fingerprint;
    END IF;

    -- Insert occurrence
    INSERT INTO feedback.occurrences (
        failure_id, event_id, event_timestamp, session_id,
        verb, duration_ms, error_message, error_backtrace
    ) VALUES (
        v_failure_id, p_event_id, p_event_timestamp, p_session_id,
        p_verb, p_duration_ms, p_error_message, p_error_backtrace
    ) RETURNING id INTO v_occurrence_id;

    -- Update failure counts
    UPDATE feedback.failures
    SET occurrence_count = occurrence_count + 1,
        last_seen_at = p_event_timestamp
    WHERE id = v_failure_id;

    RETURN v_occurrence_id;
END;
$$ LANGUAGE plpgsql;

-- Cleanup old resolved issues (keep 90 days)
CREATE OR REPLACE FUNCTION feedback.cleanup_old_resolved()
RETURNS INTEGER AS $$
DECLARE
    v_deleted INTEGER;
BEGIN
    DELETE FROM feedback.failures
    WHERE status = 'RESOLVED'
      AND resolved_at < NOW() - INTERVAL '90 days';

    GET DIAGNOSTICS v_deleted = ROW_COUNT;
    RETURN v_deleted;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON SCHEMA feedback IS 'Feedback Inspector: failure analysis, repro generation, audit trail';
COMMENT ON TABLE feedback.failures IS 'Deduplicated failure records, keyed by fingerprint';
COMMENT ON TABLE feedback.occurrences IS 'Individual occurrences of each failure';
COMMENT ON TABLE feedback.audit_log IS 'Full audit trail of all state transitions';
COMMENT ON VIEW feedback.active_issues IS 'Issues needing attention, prioritized by remediation path';
COMMENT ON VIEW feedback.ready_for_todo IS 'Issues with verified repro, ready for TODO generation';
