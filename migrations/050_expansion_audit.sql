-- Migration 050: Expansion Audit Trail
--
-- Stores ExpansionReport for audit/replay of DSL template expansion.
-- Each execution of DSL that goes through the expansion stage produces
-- a report that captures deterministic expansion details.
--
-- Key use cases:
-- - Audit trail for batch operations
-- - Replay/debugging of template expansions
-- - Lock derivation history (what entities were locked)
-- - Batch policy determination (atomic vs best_effort)

BEGIN;

-- =============================================================================
-- EXPANSION REPORTS
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".expansion_reports (
    -- Primary key is the expansion_id from ExpansionReport
    expansion_id UUID PRIMARY KEY,

    -- Session context
    session_id UUID NOT NULL,

    -- Source DSL hash (canonical whitespace)
    source_digest VARCHAR(64) NOT NULL,

    -- Expanded DSL hash (canonical whitespace)
    expanded_dsl_digest VARCHAR(64) NOT NULL,

    -- Number of statements after expansion
    expanded_statement_count INTEGER NOT NULL,

    -- Batch policy determined by expansion
    -- atomic = all-or-nothing with advisory locks
    -- best_effort = continue on failure
    batch_policy VARCHAR(20) NOT NULL CHECK (batch_policy IN ('atomic', 'best_effort')),

    -- Derived locks (JSONB array of {entity_type, entity_id, access})
    derived_lock_set JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Template digests used (JSONB array of {name, version, digest})
    template_digests JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Template invocations (JSONB array of TemplateInvocationReport)
    invocations JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Expansion diagnostics (warnings/errors)
    diagnostics JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Timestamps
    expanded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for session lookups
CREATE INDEX IF NOT EXISTS idx_expansion_reports_session
    ON "ob-poc".expansion_reports(session_id);

-- Index for digest lookups (find by source or expanded hash)
CREATE INDEX IF NOT EXISTS idx_expansion_reports_source_digest
    ON "ob-poc".expansion_reports(source_digest);

CREATE INDEX IF NOT EXISTS idx_expansion_reports_expanded_digest
    ON "ob-poc".expansion_reports(expanded_dsl_digest);

-- Index for batch policy analysis
CREATE INDEX IF NOT EXISTS idx_expansion_reports_batch_policy
    ON "ob-poc".expansion_reports(batch_policy);

-- Index for recent expansions
CREATE INDEX IF NOT EXISTS idx_expansion_reports_created
    ON "ob-poc".expansion_reports(created_at DESC);

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE "ob-poc".expansion_reports IS
    'Audit trail for DSL template expansion. Captures deterministic expansion details for replay/debugging.';

COMMENT ON COLUMN "ob-poc".expansion_reports.source_digest IS
    'SHA-256 hash of canonicalized source DSL (whitespace normalized)';

COMMENT ON COLUMN "ob-poc".expansion_reports.expanded_dsl_digest IS
    'SHA-256 hash of canonicalized expanded DSL (whitespace normalized)';

COMMENT ON COLUMN "ob-poc".expansion_reports.derived_lock_set IS
    'Advisory locks derived from template policy + runtime args. Array of {entity_type, entity_id, access}';

COMMENT ON COLUMN "ob-poc".expansion_reports.template_digests IS
    'Templates used in expansion. Array of {name, version, digest}';

COMMENT ON COLUMN "ob-poc".expansion_reports.invocations IS
    'Template invocation details. Array of TemplateInvocationReport';

COMMIT;
