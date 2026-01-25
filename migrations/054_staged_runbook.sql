-- Migration 054: Staged Runbook REPL
-- Server-side staging for DSL commands with resolution and DAG analysis
--
-- Purpose: Enable anti-hallucination execution model where:
-- 1. Commands are staged (no side effects)
-- 2. Entity references are resolved to UUIDs via DB search
-- 3. DAG analysis determines execution order
-- 4. Execution only happens on explicit user confirmation
--
-- Depends on:
-- - 052_client_group_entity_context.sql (search_entity_tags functions)
-- - 053_client_group_entity_seed.sql (bootstrap data)

BEGIN;

-- ============================================================================
-- Staged Runbook: Accumulated DSL commands awaiting execution
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".staged_runbook (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Session binding
    -- NOTE: session_id is the stable MCP conversation key.
    -- If agent layer distinguishes threads/channels, add thread_id here.
    session_id TEXT NOT NULL,

    -- Context (copied from session at creation time)
    client_group_id UUID REFERENCES "ob-poc".client_group(id),
    persona TEXT,

    -- State machine
    status TEXT NOT NULL DEFAULT 'building'
        CHECK (status IN ('building', 'ready', 'executing', 'completed', 'aborted')),

    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_sr_session ON "ob-poc".staged_runbook(session_id);
CREATE INDEX IF NOT EXISTS idx_sr_status ON "ob-poc".staged_runbook(status) WHERE status = 'building';

COMMENT ON TABLE "ob-poc".staged_runbook IS
    'Session-scoped staged runbook. Accumulates DSL commands for review before execution.';

-- ============================================================================
-- Staged Commands: Individual DSL lines in the runbook
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".staged_command (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    runbook_id UUID NOT NULL REFERENCES "ob-poc".staged_runbook(id) ON DELETE CASCADE,

    -- Ordering
    source_order INT NOT NULL,              -- user's original insertion order
    dag_order INT,                          -- computed execution order (NULL until ready)

    -- The DSL
    dsl_raw TEXT NOT NULL,                  -- as user/agent provided (may have shorthand)
    dsl_resolved TEXT,                      -- with UUIDs substituted (NULL until resolved)

    -- Metadata
    verb TEXT NOT NULL,                     -- parsed verb (e.g., 'entity.list')
    description TEXT,                       -- human-readable summary
    source_prompt TEXT,                     -- original user utterance

    -- Resolution state
    resolution_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (resolution_status IN ('pending', 'resolved', 'ambiguous', 'failed', 'parse_failed')),
    resolution_error TEXT,                  -- if failed/parse_failed

    -- DAG edges (populated during analysis)
    -- NOTE: $N references are parsed from dsl_raw; no separate output_ref column needed
    depends_on UUID[] DEFAULT '{}',         -- command IDs this depends on

    created_at TIMESTAMPTZ DEFAULT now(),

    UNIQUE(runbook_id, source_order)
);

CREATE INDEX IF NOT EXISTS idx_sc_runbook ON "ob-poc".staged_command(runbook_id);
CREATE INDEX IF NOT EXISTS idx_sc_status ON "ob-poc".staged_command(resolution_status);

COMMENT ON TABLE "ob-poc".staged_command IS
    'Individual DSL command staged for execution. Tracks resolution status and DAG dependencies.';

COMMENT ON COLUMN "ob-poc".staged_command.resolution_status IS
    'pending: not yet resolved, resolved: all refs→UUIDs, ambiguous: needs picker, failed: no matches, parse_failed: syntax error';

-- ============================================================================
-- Resolved Entities: Entity footprint for the runbook
-- These are the CONFIRMED entity UUIDs that will be touched by execution
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".staged_command_entity (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    command_id UUID NOT NULL REFERENCES "ob-poc".staged_command(id) ON DELETE CASCADE,

    -- The resolved entity
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- How it got there
    arg_name TEXT NOT NULL,                 -- which DSL argument (e.g., 'entity-id', 'entity-ids')
    resolution_source TEXT NOT NULL
        CHECK (resolution_source IN ('tag_exact', 'tag_fuzzy', 'tag_semantic', 'direct_uuid', 'picker', 'output_ref')),
    original_ref TEXT,                      -- what user said (e.g., "Irish funds")
    confidence FLOAT,                       -- for fuzzy/semantic matches

    UNIQUE(command_id, entity_id, arg_name)
);

CREATE INDEX IF NOT EXISTS idx_sce_command ON "ob-poc".staged_command_entity(command_id);
CREATE INDEX IF NOT EXISTS idx_sce_entity ON "ob-poc".staged_command_entity(entity_id);

COMMENT ON TABLE "ob-poc".staged_command_entity IS
    'Entity footprint: which entities will be touched by each command. Shows pre-execution impact.';

COMMENT ON COLUMN "ob-poc".staged_command_entity.resolution_source IS
    'tag_exact: exact tag match, tag_fuzzy: trigram fuzzy, tag_semantic: Candle embedding, direct_uuid: user provided UUID, picker: user selected from candidates, output_ref: from previous command output ($N.result)';

-- ============================================================================
-- Picker Candidates: Proposed entities for ambiguous resolution
-- These are the ONLY valid choices for runbook_pick — agent cannot fabricate UUIDs
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".staged_command_candidate (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    command_id UUID NOT NULL REFERENCES "ob-poc".staged_command(id) ON DELETE CASCADE,

    -- The candidate entity
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Match details (from resolution search)
    arg_name TEXT NOT NULL,                 -- which DSL argument this is for
    matched_tag TEXT,                       -- tag that matched
    confidence FLOAT,                       -- match confidence
    match_type TEXT NOT NULL
        CHECK (match_type IN ('tag_exact', 'tag_fuzzy', 'tag_semantic')),

    created_at TIMESTAMPTZ DEFAULT now(),

    UNIQUE(command_id, entity_id, arg_name)
);

CREATE INDEX IF NOT EXISTS idx_scc_command ON "ob-poc".staged_command_candidate(command_id);

COMMENT ON TABLE "ob-poc".staged_command_candidate IS
    'Picker candidates for ambiguous resolution. runbook_pick MUST validate entity_ids against this set. Agent cannot fabricate UUIDs.';

-- ============================================================================
-- View: Full runbook with resolved DSL and entity footprint
-- ============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_staged_runbook AS
SELECT
    sr.id AS runbook_id,
    sr.session_id,
    sr.client_group_id,
    sr.persona,
    sr.status AS runbook_status,
    sr.created_at AS runbook_created_at,
    sr.updated_at AS runbook_updated_at,
    sc.id AS command_id,
    sc.source_order,
    sc.dag_order,
    sc.dsl_raw,
    sc.dsl_resolved,
    sc.verb,
    sc.description,
    sc.source_prompt,
    sc.resolution_status,
    sc.resolution_error,
    sc.depends_on,
    -- Entity footprint as JSON array
    COALESCE(
        (
            SELECT jsonb_agg(jsonb_build_object(
                'entity_id', sce.entity_id,
                'entity_name', e.name,
                'arg_name', sce.arg_name,
                'source', sce.resolution_source,
                'original_ref', sce.original_ref,
                'confidence', sce.confidence
            ) ORDER BY sce.arg_name, e.name)
            FROM "ob-poc".staged_command_entity sce
            JOIN "ob-poc".entities e ON e.entity_id = sce.entity_id
            WHERE sce.command_id = sc.id
        ),
        '[]'::jsonb
    ) AS entity_footprint,
    -- Picker candidates as JSON array (for ambiguous commands)
    COALESCE(
        (
            SELECT jsonb_agg(jsonb_build_object(
                'entity_id', scc.entity_id,
                'entity_name', e.name,
                'arg_name', scc.arg_name,
                'matched_tag', scc.matched_tag,
                'confidence', scc.confidence,
                'match_type', scc.match_type
            ) ORDER BY scc.confidence DESC, e.name)
            FROM "ob-poc".staged_command_candidate scc
            JOIN "ob-poc".entities e ON e.entity_id = scc.entity_id
            WHERE scc.command_id = sc.id
        ),
        '[]'::jsonb
    ) AS candidates
FROM "ob-poc".staged_runbook sr
LEFT JOIN "ob-poc".staged_command sc ON sc.runbook_id = sr.id
ORDER BY sr.id, COALESCE(sc.dag_order, sc.source_order);

COMMENT ON VIEW "ob-poc".v_staged_runbook IS
    'Full runbook state with commands, entity footprint, and picker candidates. Primary query for MCP runbook_show.';

-- ============================================================================
-- View: Runbook summary statistics
-- ============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_runbook_summary AS
SELECT
    sr.id AS runbook_id,
    sr.session_id,
    sr.status,
    sr.client_group_id,
    sr.persona,
    COUNT(sc.id) AS command_count,
    COUNT(CASE WHEN sc.resolution_status = 'resolved' THEN 1 END) AS resolved_count,
    COUNT(CASE WHEN sc.resolution_status = 'pending' THEN 1 END) AS pending_count,
    COUNT(CASE WHEN sc.resolution_status = 'ambiguous' THEN 1 END) AS ambiguous_count,
    COUNT(CASE WHEN sc.resolution_status IN ('failed', 'parse_failed') THEN 1 END) AS failed_count,
    -- Distinct entities touched
    (
        SELECT COUNT(DISTINCT sce.entity_id)
        FROM "ob-poc".staged_command sc2
        JOIN "ob-poc".staged_command_entity sce ON sce.command_id = sc2.id
        WHERE sc2.runbook_id = sr.id
    ) AS entity_footprint_size,
    sr.created_at,
    sr.updated_at
FROM "ob-poc".staged_runbook sr
LEFT JOIN "ob-poc".staged_command sc ON sc.runbook_id = sr.id
GROUP BY sr.id;

COMMENT ON VIEW "ob-poc".v_runbook_summary IS
    'Runbook statistics for MCP events and UI status bar.';

-- ============================================================================
-- Function: Get or create runbook for session
-- Ensures exactly one active runbook per session
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".get_or_create_runbook(
    p_session_id TEXT,
    p_client_group_id UUID DEFAULT NULL,
    p_persona TEXT DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_runbook_id UUID;
BEGIN
    -- Try to get existing active runbook
    SELECT id INTO v_runbook_id
    FROM "ob-poc".staged_runbook
    WHERE session_id = p_session_id
      AND status = 'building'
    ORDER BY created_at DESC
    LIMIT 1;

    -- If none exists, create one
    IF v_runbook_id IS NULL THEN
        INSERT INTO "ob-poc".staged_runbook (session_id, client_group_id, persona)
        VALUES (p_session_id, p_client_group_id, p_persona)
        RETURNING id INTO v_runbook_id;
    END IF;

    RETURN v_runbook_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".get_or_create_runbook IS
    'Get existing active runbook or create new one. Ensures one active runbook per session.';

-- ============================================================================
-- Function: Stage a command (insert with next source_order)
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".stage_command(
    p_runbook_id UUID,
    p_dsl_raw TEXT,
    p_verb TEXT,
    p_description TEXT DEFAULT NULL,
    p_source_prompt TEXT DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_command_id UUID;
    v_next_order INT;
BEGIN
    -- Get next source_order
    SELECT COALESCE(MAX(source_order), 0) + 1 INTO v_next_order
    FROM "ob-poc".staged_command
    WHERE runbook_id = p_runbook_id;

    -- Insert command
    INSERT INTO "ob-poc".staged_command (
        runbook_id, source_order, dsl_raw, verb, description, source_prompt
    )
    VALUES (
        p_runbook_id, v_next_order, p_dsl_raw, p_verb, p_description, p_source_prompt
    )
    RETURNING id INTO v_command_id;

    -- Update runbook timestamp
    UPDATE "ob-poc".staged_runbook
    SET updated_at = now()
    WHERE id = p_runbook_id;

    RETURN v_command_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".stage_command IS
    'Stage a new command with automatic source_order assignment.';

-- ============================================================================
-- Function: Remove command and cascade dependents
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".remove_command(
    p_command_id UUID
) RETURNS TABLE (removed_id UUID, was_dependent BOOLEAN) AS $$
DECLARE
    v_runbook_id UUID;
BEGIN
    -- Get runbook ID
    SELECT runbook_id INTO v_runbook_id
    FROM "ob-poc".staged_command
    WHERE id = p_command_id;

    IF v_runbook_id IS NULL THEN
        RAISE EXCEPTION 'Command % not found', p_command_id;
    END IF;

    -- Return the command and all dependents
    RETURN QUERY
    WITH RECURSIVE dependents AS (
        -- Base: the command being removed
        SELECT id, FALSE AS was_dependent
        FROM "ob-poc".staged_command
        WHERE id = p_command_id

        UNION ALL

        -- Recursive: commands that depend on already-selected commands
        SELECT sc.id, TRUE AS was_dependent
        FROM "ob-poc".staged_command sc
        JOIN dependents d ON p_command_id = ANY(sc.depends_on)
        WHERE sc.runbook_id = v_runbook_id
    )
    SELECT d.id, d.was_dependent FROM dependents d;

    -- Delete (CASCADE handles staged_command_entity and staged_command_candidate)
    DELETE FROM "ob-poc".staged_command
    WHERE id IN (
        SELECT d.id FROM (
            WITH RECURSIVE dependents AS (
                SELECT id FROM "ob-poc".staged_command WHERE id = p_command_id
                UNION ALL
                SELECT sc.id
                FROM "ob-poc".staged_command sc
                JOIN dependents d ON p_command_id = ANY(sc.depends_on)
                WHERE sc.runbook_id = v_runbook_id
            )
            SELECT id FROM dependents
        ) d
    );

    -- Update runbook timestamp
    UPDATE "ob-poc".staged_runbook
    SET updated_at = now()
    WHERE id = v_runbook_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".remove_command IS
    'Remove a command and cascade-remove all commands that depend on it.';

-- ============================================================================
-- Function: Abort runbook (clear all commands, set status)
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".abort_runbook(
    p_runbook_id UUID
) RETURNS BOOLEAN AS $$
BEGIN
    -- Delete all commands (CASCADE handles children)
    DELETE FROM "ob-poc".staged_command WHERE runbook_id = p_runbook_id;

    -- Update status
    UPDATE "ob-poc".staged_runbook
    SET status = 'aborted', updated_at = now()
    WHERE id = p_runbook_id;

    RETURN FOUND;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".abort_runbook IS
    'Clear all staged commands and mark runbook as aborted.';

-- ============================================================================
-- Function: Check if runbook is ready for execution
-- Returns blocking commands if not ready
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".check_runbook_ready(
    p_runbook_id UUID
) RETURNS TABLE (
    is_ready BOOLEAN,
    blocking_command_id UUID,
    blocking_source_order INT,
    blocking_status TEXT,
    blocking_error TEXT
) AS $$
BEGIN
    -- Check for any non-resolved commands
    RETURN QUERY
    SELECT
        NOT EXISTS (
            SELECT 1 FROM "ob-poc".staged_command
            WHERE runbook_id = p_runbook_id
              AND resolution_status != 'resolved'
        ) AS is_ready,
        sc.id AS blocking_command_id,
        sc.source_order AS blocking_source_order,
        sc.resolution_status AS blocking_status,
        sc.resolution_error AS blocking_error
    FROM "ob-poc".staged_command sc
    WHERE sc.runbook_id = p_runbook_id
      AND sc.resolution_status != 'resolved'
    ORDER BY sc.source_order;

    -- If no blocking commands, return a single row with is_ready = true
    IF NOT FOUND THEN
        RETURN QUERY
        SELECT TRUE, NULL::UUID, NULL::INT, NULL::TEXT, NULL::TEXT;
    END IF;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".check_runbook_ready IS
    'Server-side readiness gate. Returns blocking commands if runbook cannot execute.';

-- ============================================================================
-- Function: Validate picker selection against stored candidates
-- CRITICAL: Prevents agent from fabricating entity_ids
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".validate_picker_selection(
    p_command_id UUID,
    p_entity_ids UUID[]
) RETURNS TABLE (
    is_valid BOOLEAN,
    invalid_entity_id UUID,
    error_message TEXT
) AS $$
DECLARE
    v_invalid UUID;
    v_valid_candidates UUID[];
BEGIN
    -- Get valid candidates for this command
    SELECT array_agg(entity_id) INTO v_valid_candidates
    FROM "ob-poc".staged_command_candidate
    WHERE command_id = p_command_id;

    IF v_valid_candidates IS NULL OR array_length(v_valid_candidates, 1) IS NULL THEN
        RETURN QUERY SELECT FALSE, NULL::UUID, 'No candidates stored for this command'::TEXT;
        RETURN;
    END IF;

    -- Check each selected entity_id is in the candidate set
    FOREACH v_invalid IN ARRAY p_entity_ids
    LOOP
        IF NOT (v_invalid = ANY(v_valid_candidates)) THEN
            RETURN QUERY SELECT
                FALSE,
                v_invalid,
                format('Entity %s not in stored candidate set from ResolutionAmbiguous event', v_invalid)::TEXT;
            RETURN;
        END IF;
    END LOOP;

    -- All valid
    RETURN QUERY SELECT TRUE, NULL::UUID, NULL::TEXT;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".validate_picker_selection IS
    'CRITICAL: Validates picker entity_ids against stored candidates. Prevents agent from fabricating UUIDs.';

-- ============================================================================
-- Trigger: Update runbook timestamp on command changes
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".trg_update_runbook_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE "ob-poc".staged_runbook
    SET updated_at = now()
    WHERE id = COALESCE(NEW.runbook_id, OLD.runbook_id);
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_staged_command_update_runbook ON "ob-poc".staged_command;
CREATE TRIGGER trg_staged_command_update_runbook
    AFTER INSERT OR UPDATE OR DELETE ON "ob-poc".staged_command
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".trg_update_runbook_timestamp();

COMMIT;
