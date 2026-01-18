-- Migration: 035_session_sheet.sql
-- Purpose: Add REPL session state machine and DSL sheet support
-- Depends: 023_sessions_persistence.sql

-- =============================================================================
-- EXTEND SESSIONS TABLE
-- =============================================================================

-- Add columns for REPL state machine
ALTER TABLE "ob-poc".sessions
ADD COLUMN IF NOT EXISTS repl_state TEXT DEFAULT 'empty',
ADD COLUMN IF NOT EXISTS scope_dsl TEXT[] DEFAULT '{}',
ADD COLUMN IF NOT EXISTS template_dsl TEXT,
ADD COLUMN IF NOT EXISTS target_entity_type TEXT,
ADD COLUMN IF NOT EXISTS intent_confirmed BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS sheet JSONB;

-- Index for querying sessions by REPL state
CREATE INDEX IF NOT EXISTS idx_sessions_repl_state ON "ob-poc".sessions(repl_state);

-- Comment on new columns
COMMENT ON COLUMN "ob-poc".sessions.repl_state IS 'REPL state machine: empty, scoped, templated, generated, parsed, resolving, ready, executing, executed';
COMMENT ON COLUMN "ob-poc".sessions.scope_dsl IS 'DSL commands that defined the current scope (for audit/replay)';
COMMENT ON COLUMN "ob-poc".sessions.template_dsl IS 'Template DSL before expansion (unpopulated intent)';
COMMENT ON COLUMN "ob-poc".sessions.target_entity_type IS 'Entity type for template expansion (e.g., cbu)';
COMMENT ON COLUMN "ob-poc".sessions.intent_confirmed IS 'Whether user confirmed the intent';
COMMENT ON COLUMN "ob-poc".sessions.sheet IS 'Generated DSL sheet with statements, DAG phases, and execution status';

-- =============================================================================
-- SHEET EXECUTION AUDIT TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".sheet_execution_audit (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL,
    sheet_id UUID NOT NULL,

    -- Source tracking
    scope_dsl TEXT[] NOT NULL DEFAULT '{}',
    template_dsl TEXT,
    source_statements TEXT[] NOT NULL DEFAULT '{}',

    -- DAG analysis
    phase_count INTEGER NOT NULL DEFAULT 0,
    statement_count INTEGER NOT NULL DEFAULT 0,
    dag_analysis JSONB,

    -- Execution result
    overall_status TEXT NOT NULL,  -- success, failed, rolled_back
    phases_completed INTEGER NOT NULL DEFAULT 0,
    result JSONB NOT NULL,

    -- Timing
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,

    -- User tracking
    submitted_by TEXT,

    CONSTRAINT fk_session FOREIGN KEY (session_id)
        REFERENCES "ob-poc".sessions(id) ON DELETE CASCADE
);

-- Indexes for audit queries
CREATE INDEX IF NOT EXISTS idx_sheet_audit_session ON "ob-poc".sheet_execution_audit(session_id);
CREATE INDEX IF NOT EXISTS idx_sheet_audit_submitted ON "ob-poc".sheet_execution_audit(submitted_at);
CREATE INDEX IF NOT EXISTS idx_sheet_audit_status ON "ob-poc".sheet_execution_audit(overall_status);

-- Comment on audit table
COMMENT ON TABLE "ob-poc".sheet_execution_audit IS 'Audit trail of DSL sheet executions for debugging and compliance';
