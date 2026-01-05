-- Migration: Audit Trail Improvements
-- Based on peer review feedback:
-- 1. Transaction boundary audit - atomic execution + view state recording
-- 2. Source attribution columns for provenance tracking

-- =============================================================================
-- 1. Add source attribution columns to dsl_idempotency
-- =============================================================================

DO $$
BEGIN
    -- Source of the execution (where did this request originate?)
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_idempotency'
        AND column_name = 'source'
    ) THEN
        ALTER TABLE "ob-poc".dsl_idempotency
        ADD COLUMN source VARCHAR(30) DEFAULT 'unknown';
    END IF;

    -- Request/correlation ID for distributed tracing
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_idempotency'
        AND column_name = 'request_id'
    ) THEN
        ALTER TABLE "ob-poc".dsl_idempotency
        ADD COLUMN request_id UUID;
    END IF;

    -- Actor ID (user or system that initiated)
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_idempotency'
        AND column_name = 'actor_id'
    ) THEN
        ALTER TABLE "ob-poc".dsl_idempotency
        ADD COLUMN actor_id UUID;
    END IF;

    -- Actor type for polymorphic actors
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_idempotency'
        AND column_name = 'actor_type'
    ) THEN
        ALTER TABLE "ob-poc".dsl_idempotency
        ADD COLUMN actor_type VARCHAR(20) DEFAULT 'user';
    END IF;
END $$;

-- Create index for request_id lookups (distributed tracing)
CREATE INDEX IF NOT EXISTS idx_idempotency_request_id
    ON "ob-poc".dsl_idempotency(request_id)
    WHERE request_id IS NOT NULL;

-- Create index for actor lookups (audit by user)
CREATE INDEX IF NOT EXISTS idx_idempotency_actor
    ON "ob-poc".dsl_idempotency(actor_id, actor_type)
    WHERE actor_id IS NOT NULL;

COMMENT ON COLUMN "ob-poc".dsl_idempotency.source IS
    'Origin of execution: api, cli, mcp, repl, batch, test, migration';
COMMENT ON COLUMN "ob-poc".dsl_idempotency.request_id IS
    'Correlation ID for distributed tracing - groups related executions';
COMMENT ON COLUMN "ob-poc".dsl_idempotency.actor_id IS
    'ID of user or system that initiated this execution';
COMMENT ON COLUMN "ob-poc".dsl_idempotency.actor_type IS
    'Type of actor: user, system, agent, service';

-- =============================================================================
-- 2. Add source attribution to dsl_view_state_changes
-- =============================================================================

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_view_state_changes'
        AND column_name = 'source'
    ) THEN
        ALTER TABLE "ob-poc".dsl_view_state_changes
        ADD COLUMN source VARCHAR(30) DEFAULT 'unknown';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'dsl_view_state_changes'
        AND column_name = 'request_id'
    ) THEN
        ALTER TABLE "ob-poc".dsl_view_state_changes
        ADD COLUMN request_id UUID;
    END IF;
END $$;

COMMENT ON COLUMN "ob-poc".dsl_view_state_changes.source IS
    'Origin of view state change: api, cli, mcp, repl, batch, test';
COMMENT ON COLUMN "ob-poc".dsl_view_state_changes.request_id IS
    'Correlation ID for distributed tracing';

-- =============================================================================
-- 3. Atomic execution+view state recording function
-- =============================================================================

-- This function atomically records both the idempotency entry AND the view state change
-- ensuring they are committed together or not at all.
CREATE OR REPLACE FUNCTION "ob-poc".record_execution_with_view_state(
    -- Idempotency params
    p_idempotency_key TEXT,
    p_execution_id UUID,
    p_statement_index INTEGER,
    p_verb VARCHAR(200),
    p_args_hash VARCHAR(64),
    p_result_type VARCHAR(20),
    p_result_id UUID DEFAULT NULL,
    p_result_json JSONB DEFAULT NULL,
    p_result_affected BIGINT DEFAULT NULL,
    p_verb_hash BYTEA DEFAULT NULL,
    -- Source attribution
    p_source VARCHAR(30) DEFAULT 'unknown',
    p_request_id UUID DEFAULT NULL,
    p_actor_id UUID DEFAULT NULL,
    p_actor_type VARCHAR(20) DEFAULT 'user',
    -- View state params (all optional - only populated for view.* operations)
    p_session_id UUID DEFAULT NULL,
    p_view_taxonomy_context JSONB DEFAULT NULL,
    p_view_selection UUID[] DEFAULT NULL,
    p_view_refinements JSONB DEFAULT NULL,
    p_view_stack_depth INTEGER DEFAULT NULL,
    p_view_state_snapshot JSONB DEFAULT NULL
) RETURNS TABLE(
    idempotency_key TEXT,
    view_state_change_id UUID,
    was_cached BOOLEAN
) AS $$
DECLARE
    v_existing_key TEXT;
    v_change_id UUID;
BEGIN
    -- Check if already executed (idempotency check)
    SELECT i.idempotency_key INTO v_existing_key
    FROM "ob-poc".dsl_idempotency i
    WHERE i.idempotency_key = p_idempotency_key;

    IF v_existing_key IS NOT NULL THEN
        -- Already executed - return cached indicator
        RETURN QUERY SELECT v_existing_key, NULL::UUID, TRUE;
        RETURN;
    END IF;

    -- Record idempotency entry with source attribution
    INSERT INTO "ob-poc".dsl_idempotency (
        idempotency_key,
        execution_id,
        statement_index,
        verb,
        args_hash,
        result_type,
        result_id,
        result_json,
        result_affected,
        verb_hash,
        source,
        request_id,
        actor_id,
        actor_type
    ) VALUES (
        p_idempotency_key,
        p_execution_id,
        p_statement_index,
        p_verb,
        p_args_hash,
        p_result_type,
        p_result_id,
        p_result_json,
        p_result_affected,
        p_verb_hash,
        p_source,
        p_request_id,
        p_actor_id,
        p_actor_type
    );

    -- If view state provided, record it atomically
    IF p_view_state_snapshot IS NOT NULL THEN
        INSERT INTO "ob-poc".dsl_view_state_changes (
            idempotency_key,
            session_id,
            verb_name,
            taxonomy_context,
            selection,
            refinements,
            stack_depth,
            view_state_snapshot,
            source,
            request_id,
            audit_user_id
        ) VALUES (
            p_idempotency_key,
            p_session_id,
            p_verb,
            p_view_taxonomy_context,
            COALESCE(p_view_selection, '{}'),
            COALESCE(p_view_refinements, '[]'::jsonb),
            COALESCE(p_view_stack_depth, 1),
            p_view_state_snapshot,
            p_source,
            p_request_id,
            p_actor_id
        ) RETURNING change_id INTO v_change_id;

        -- Also update session's current view state
        IF p_session_id IS NOT NULL THEN
            UPDATE "ob-poc".dsl_sessions
            SET current_view_state = p_view_state_snapshot,
                view_updated_at = now()
            WHERE session_id = p_session_id;
        END IF;
    END IF;

    RETURN QUERY SELECT p_idempotency_key, v_change_id, FALSE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".record_execution_with_view_state IS
    'Atomically records execution result and view state change in single transaction';

-- =============================================================================
-- 4. Update v_execution_audit_with_view to include source attribution
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
    e.source,
    e.request_id,
    e.actor_id,
    e.actor_type,
    v.domain,
    v.description
FROM "ob-poc".dsl_idempotency e
LEFT JOIN "ob-poc".dsl_verbs v ON e.verb_hash = v.compiled_hash
ORDER BY e.created_at DESC;

COMMENT ON VIEW "ob-poc".v_execution_audit_with_view IS
    'Complete execution audit trail with view state and source attribution';

-- =============================================================================
-- 5. Audit query: Find all executions for a request (distributed tracing)
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_request_execution_trace AS
SELECT
    e.request_id,
    e.idempotency_key,
    e.execution_id,
    e.statement_index,
    e.verb,
    e.result_type,
    e.result_id,
    e.source,
    e.actor_id,
    e.actor_type,
    e.created_at,
    v.change_id AS view_state_change_id,
    v.selection_count AS view_selection_count
FROM "ob-poc".dsl_idempotency e
LEFT JOIN "ob-poc".dsl_view_state_changes v
    ON e.idempotency_key = v.idempotency_key
WHERE e.request_id IS NOT NULL
ORDER BY e.request_id, e.created_at;

COMMENT ON VIEW "ob-poc".v_request_execution_trace IS
    'Trace all executions and view state changes for a single request';
