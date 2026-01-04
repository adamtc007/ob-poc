-- Migration: View State Audit Trail
-- Closes the "side door" gap where view state changes were not persisted
-- Ensures full auditability from REPL to execution
-- Baked into the DSL pipeline - not an optional add-on

-- =============================================================================
-- 1. View State Changes Table - Audit trail of all view state mutations
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_view_state_changes (
    change_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Link to execution via idempotency_key (the actual PK of dsl_idempotency)
    idempotency_key TEXT NOT NULL,

    -- Link to session (if available)
    session_id UUID,

    -- What view operation was executed
    verb_name VARCHAR(100) NOT NULL,

    -- The taxonomy context that built this view
    taxonomy_context JSONB NOT NULL,

    -- The selection at time of change
    selection UUID[] NOT NULL DEFAULT '{}',
    selection_count INTEGER GENERATED ALWAYS AS (coalesce(array_length(selection, 1), 0)) STORED,

    -- Refinements applied to narrow/expand selection
    refinements JSONB DEFAULT '[]',

    -- Stack depth for navigation audit
    stack_depth INTEGER NOT NULL DEFAULT 1,

    -- The full view state snapshot (for reconstruction)
    view_state_snapshot JSONB NOT NULL,

    -- Audit metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    audit_user_id UUID,

    -- Foreign keys
    CONSTRAINT fk_idempotency
        FOREIGN KEY (idempotency_key)
        REFERENCES "ob-poc".dsl_idempotency(idempotency_key)
        ON DELETE CASCADE,
    CONSTRAINT fk_session
        FOREIGN KEY (session_id)
        REFERENCES "ob-poc".dsl_sessions(session_id)
        ON DELETE SET NULL
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_view_state_changes_idempotency
    ON "ob-poc".dsl_view_state_changes(idempotency_key);
CREATE INDEX IF NOT EXISTS idx_view_state_changes_session
    ON "ob-poc".dsl_view_state_changes(session_id);
CREATE INDEX IF NOT EXISTS idx_view_state_changes_created
    ON "ob-poc".dsl_view_state_changes(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_view_state_changes_verb
    ON "ob-poc".dsl_view_state_changes(verb_name);

-- GIN index for selection array queries (find changes affecting specific entities)
CREATE INDEX IF NOT EXISTS idx_view_state_changes_selection_gin
    ON "ob-poc".dsl_view_state_changes USING GIN (selection);

-- =============================================================================
-- 2. Ensure dsl_idempotency has view state columns (idempotent)
-- =============================================================================

-- These columns were added in previous migration but ensure they exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_idempotency'
        AND column_name = 'input_view_state'
    ) THEN
        ALTER TABLE "ob-poc".dsl_idempotency ADD COLUMN input_view_state JSONB;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_idempotency'
        AND column_name = 'input_selection'
    ) THEN
        ALTER TABLE "ob-poc".dsl_idempotency ADD COLUMN input_selection UUID[] DEFAULT '{}';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_idempotency'
        AND column_name = 'output_view_state'
    ) THEN
        ALTER TABLE "ob-poc".dsl_idempotency ADD COLUMN output_view_state JSONB;
    END IF;
END $$;

COMMENT ON COLUMN "ob-poc".dsl_idempotency.input_view_state IS
    'View state snapshot before execution - what selection was targeted';
COMMENT ON COLUMN "ob-poc".dsl_idempotency.input_selection IS
    'Selection array before execution - entities affected by batch ops';
COMMENT ON COLUMN "ob-poc".dsl_idempotency.output_view_state IS
    'View state snapshot after execution - result of view.* operations';

-- =============================================================================
-- 3. Add view_state to dsl_sessions for session restore (idempotent)
-- =============================================================================

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_sessions'
        AND column_name = 'current_view_state'
    ) THEN
        ALTER TABLE "ob-poc".dsl_sessions ADD COLUMN current_view_state JSONB;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_sessions'
        AND column_name = 'view_updated_at'
    ) THEN
        ALTER TABLE "ob-poc".dsl_sessions ADD COLUMN view_updated_at TIMESTAMPTZ;
    END IF;
END $$;

COMMENT ON COLUMN "ob-poc".dsl_sessions.current_view_state IS
    'Current view state for session - enables session restore with full context';
COMMENT ON COLUMN "ob-poc".dsl_sessions.view_updated_at IS
    'When view state was last updated';

-- =============================================================================
-- 4. Audit view: Complete execution trace with view context
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_execution_audit_with_view AS
SELECT
    e.idempotency_key,
    e.execution_id,
    e.verb_hash,
    e.verb AS verb_name,
    e.result_id,
    e.created_at AS executed_at,
    e.input_selection,
    coalesce(array_length(e.input_selection, 1), 0) AS selection_count,
    e.input_view_state ->> 'context' AS view_context,
    e.output_view_state IS NOT NULL AS produced_view_state,
    v.domain,
    v.description
FROM "ob-poc".dsl_idempotency e
LEFT JOIN "ob-poc".dsl_verbs v ON e.verb_hash = v.compiled_hash
ORDER BY e.created_at DESC;

COMMENT ON VIEW "ob-poc".v_execution_audit_with_view IS
    'Complete execution audit trail with view state context - shows what was targeted';

-- =============================================================================
-- 5. Audit view: Session view history
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_session_view_history AS
SELECT
    c.session_id,
    c.change_id,
    c.verb_name,
    c.selection_count,
    c.stack_depth,
    c.taxonomy_context ->> 'node_type' AS node_type,
    c.taxonomy_context ->> 'label' AS label,
    c.refinements,
    c.created_at,
    s.status AS session_status,
    s.primary_domain
FROM "ob-poc".dsl_view_state_changes c
LEFT JOIN "ob-poc".dsl_sessions s ON c.session_id = s.session_id
ORDER BY c.session_id, c.created_at DESC;

COMMENT ON VIEW "ob-poc".v_session_view_history IS
    'View state change history per session - shows navigation path through data';

-- =============================================================================
-- 6. Function: Record view state change (called from Rust DSL pipeline)
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".record_view_state_change(
    p_idempotency_key TEXT,
    p_session_id UUID,
    p_verb_name VARCHAR(100),
    p_taxonomy_context JSONB,
    p_selection UUID[],
    p_refinements JSONB,
    p_stack_depth INTEGER,
    p_view_state_snapshot JSONB,
    p_audit_user_id UUID DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_change_id UUID;
BEGIN
    INSERT INTO "ob-poc".dsl_view_state_changes (
        idempotency_key,
        session_id,
        verb_name,
        taxonomy_context,
        selection,
        refinements,
        stack_depth,
        view_state_snapshot,
        audit_user_id
    ) VALUES (
        p_idempotency_key,
        p_session_id,
        p_verb_name,
        p_taxonomy_context,
        p_selection,
        p_refinements,
        p_stack_depth,
        p_view_state_snapshot,
        p_audit_user_id
    ) RETURNING change_id INTO v_change_id;

    -- Also update the session's current view state
    IF p_session_id IS NOT NULL THEN
        UPDATE "ob-poc".dsl_sessions
        SET current_view_state = p_view_state_snapshot,
            view_updated_at = now()
        WHERE session_id = p_session_id;
    END IF;

    RETURN v_change_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".record_view_state_change IS
    'Atomically records view state change and updates session - called from DSL pipeline';
