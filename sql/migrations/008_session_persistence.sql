-- Session persistence for incremental DSL execution
-- Tracks agent sessions, their state, and executed DSL snapshots

-- =============================================================================
-- SESSION STATE TABLE
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_sessions (
    session_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Session lifecycle
    status              VARCHAR(20) NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'completed', 'aborted', 'expired', 'error')),
    
    -- Domain context (detected from executed DSL)
    primary_domain      VARCHAR(30),
    cbu_id              UUID REFERENCES "ob-poc".cbus(cbu_id),
    kyc_case_id         UUID REFERENCES kyc.cases(case_id),
    onboarding_request_id UUID REFERENCES "ob-poc".onboarding_requests(request_id),
    
    -- All named bindings from successful executions
    named_refs          JSONB NOT NULL DEFAULT '{}',
    
    -- Session metadata
    client_type         VARCHAR(50),
    jurisdiction        VARCHAR(10),
    
    -- Timing
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_activity_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at          TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '24 hours'),
    completed_at        TIMESTAMPTZ,
    
    -- Error tracking
    error_count         INTEGER NOT NULL DEFAULT 0,
    last_error          TEXT,
    last_error_at       TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_dsl_sessions_status ON "ob-poc".dsl_sessions(status);
CREATE INDEX IF NOT EXISTS idx_dsl_sessions_expires ON "ob-poc".dsl_sessions(expires_at) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_dsl_sessions_cbu ON "ob-poc".dsl_sessions(cbu_id) WHERE cbu_id IS NOT NULL;

-- =============================================================================
-- DSL SNAPSHOTS TABLE (successful executions only)
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_snapshots (
    snapshot_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id          UUID NOT NULL REFERENCES "ob-poc".dsl_sessions(session_id) ON DELETE CASCADE,
    
    version             INTEGER NOT NULL,
    dsl_source          TEXT NOT NULL,
    dsl_checksum        VARCHAR(64) NOT NULL,
    
    success             BOOLEAN NOT NULL DEFAULT true,
    bindings_captured   JSONB NOT NULL DEFAULT '{}',
    entities_created    JSONB NOT NULL DEFAULT '[]',
    domains_used        TEXT[] NOT NULL DEFAULT '{}',
    
    executed_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    execution_ms        INTEGER,
    
    UNIQUE (session_id, version)
);

CREATE INDEX IF NOT EXISTS idx_dsl_snapshots_session ON "ob-poc".dsl_snapshots(session_id);

-- =============================================================================
-- SESSION EVENTS TABLE (audit log)
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_session_events (
    event_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id          UUID NOT NULL REFERENCES "ob-poc".dsl_sessions(session_id) ON DELETE CASCADE,
    
    event_type          VARCHAR(30) NOT NULL
                        CHECK (event_type IN (
                            'created', 'execute_started', 'execute_success', 'execute_failed',
                            'validation_error', 'timeout', 'aborted', 'expired', 'completed',
                            'binding_added', 'domain_detected', 'error_recovered'
                        )),
    
    dsl_source          TEXT,
    error_message       TEXT,
    metadata            JSONB NOT NULL DEFAULT '{}',
    occurred_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_dsl_session_events_session ON "ob-poc".dsl_session_events(session_id);

-- =============================================================================
-- SESSION LOCKS TABLE (for timeout detection)
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_session_locks (
    session_id          UUID PRIMARY KEY REFERENCES "ob-poc".dsl_sessions(session_id) ON DELETE CASCADE,
    locked_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    lock_timeout_at     TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '30 seconds'),
    operation           VARCHAR(50) NOT NULL
);

-- =============================================================================
-- HELPER FUNCTIONS
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".cleanup_expired_sessions()
RETURNS INTEGER AS $$
DECLARE
    cleaned INTEGER;
BEGIN
    UPDATE "ob-poc".dsl_sessions
    SET status = 'expired'
    WHERE status = 'active' AND expires_at < now();
    GET DIAGNOSTICS cleaned = ROW_COUNT;
    RETURN cleaned;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "ob-poc".abort_hung_sessions()
RETURNS INTEGER AS $$
DECLARE
    aborted INTEGER;
BEGIN
    UPDATE "ob-poc".dsl_sessions s
    SET status = 'error',
        last_error = 'Session timed out during operation: ' || l.operation,
        last_error_at = now()
    FROM "ob-poc".dsl_session_locks l
    WHERE s.session_id = l.session_id
      AND l.lock_timeout_at < now()
      AND s.status = 'active';
    GET DIAGNOSTICS aborted = ROW_COUNT;
    DELETE FROM "ob-poc".dsl_session_locks WHERE lock_timeout_at < now();
    RETURN aborted;
END;
$$ LANGUAGE plpgsql;
