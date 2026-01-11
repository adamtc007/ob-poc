-- =============================================================================
-- Migration 019: Session Navigation History Enhancement
-- =============================================================================
-- Purpose: Add current history position tracking for back/forward navigation
--   - Add history_position column to session_scopes (current position in history)
--   - Add active_cbu_ids column for multi-CBU set selection
--   - Create helper functions for back/forward navigation
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. Add history position and multi-CBU tracking to session_scopes
-- -----------------------------------------------------------------------------
ALTER TABLE "ob-poc".session_scopes
ADD COLUMN IF NOT EXISTS history_position INTEGER DEFAULT 0;

ALTER TABLE "ob-poc".session_scopes
ADD COLUMN IF NOT EXISTS active_cbu_ids UUID[] DEFAULT '{}';

COMMENT ON COLUMN "ob-poc".session_scopes.history_position IS
'Current position in history stack. -1 means at latest, >=0 means navigated back.';

COMMENT ON COLUMN "ob-poc".session_scopes.active_cbu_ids IS
'Set of active CBU IDs (0..n) for multi-CBU selection workflows.';

-- -----------------------------------------------------------------------------
-- 2. Helper function to push history entry (called on scope change)
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".push_scope_history(
    p_session_id UUID,
    p_change_source VARCHAR(50) DEFAULT 'dsl',
    p_change_verb VARCHAR(100) DEFAULT NULL
) RETURNS INTEGER AS $$
DECLARE
    v_current_scope "ob-poc".session_scopes;
    v_new_position INTEGER;
    v_snapshot JSONB;
BEGIN
    -- Get current scope
    SELECT * INTO v_current_scope
    FROM "ob-poc".session_scopes
    WHERE session_id = p_session_id;

    IF v_current_scope IS NULL THEN
        RETURN -1;
    END IF;

    -- If we're not at the end of history, truncate forward history
    -- (like when you navigate back then make a new change)
    IF v_current_scope.history_position >= 0 THEN
        DELETE FROM "ob-poc".session_scope_history
        WHERE session_id = p_session_id
          AND position > v_current_scope.history_position;
    END IF;

    -- Get next position
    SELECT COALESCE(MAX(position), -1) + 1 INTO v_new_position
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id;

    -- Build snapshot from current scope
    v_snapshot := jsonb_build_object(
        'scope_type', v_current_scope.scope_type,
        'apex_entity_id', v_current_scope.apex_entity_id,
        'apex_entity_name', v_current_scope.apex_entity_name,
        'cbu_id', v_current_scope.cbu_id,
        'cbu_name', v_current_scope.cbu_name,
        'jurisdiction_code', v_current_scope.jurisdiction_code,
        'focal_entity_id', v_current_scope.focal_entity_id,
        'focal_entity_name', v_current_scope.focal_entity_name,
        'neighborhood_hops', v_current_scope.neighborhood_hops,
        'scope_filters', v_current_scope.scope_filters,
        'cursor_entity_id', v_current_scope.cursor_entity_id,
        'cursor_entity_name', v_current_scope.cursor_entity_name,
        'active_cbu_ids', v_current_scope.active_cbu_ids
    );

    -- Insert history entry
    INSERT INTO "ob-poc".session_scope_history (
        session_id, position, scope_snapshot, change_source, change_verb
    ) VALUES (
        p_session_id, v_new_position, v_snapshot, p_change_source, p_change_verb
    );

    -- Update current position to "at end" (-1)
    UPDATE "ob-poc".session_scopes
    SET history_position = -1,
        updated_at = NOW()
    WHERE session_id = p_session_id;

    RETURN v_new_position;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".push_scope_history IS
'Push current scope state to history stack. Call before making scope changes.';

-- -----------------------------------------------------------------------------
-- 3. Navigate back in history
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".navigate_back(
    p_session_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_current_scope "ob-poc".session_scopes;
    v_current_pos INTEGER;
    v_target_pos INTEGER;
    v_max_pos INTEGER;
    v_snapshot JSONB;
    v_result "ob-poc".session_scopes;
BEGIN
    -- Get current scope and position
    SELECT * INTO v_current_scope
    FROM "ob-poc".session_scopes
    WHERE session_id = p_session_id;

    IF v_current_scope IS NULL THEN
        RETURN NULL;
    END IF;

    -- Get max history position
    SELECT MAX(position) INTO v_max_pos
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id;

    IF v_max_pos IS NULL THEN
        -- No history, return current scope unchanged
        RETURN v_current_scope;
    END IF;

    -- Calculate current effective position
    IF v_current_scope.history_position < 0 THEN
        -- At end of history, need to save current state first
        PERFORM "ob-poc".push_scope_history(p_session_id, 'navigation', 'session.back');
        v_current_pos := v_max_pos + 1;
    ELSE
        v_current_pos := v_current_scope.history_position;
    END IF;

    -- Calculate target position (one step back)
    v_target_pos := v_current_pos - 1;

    IF v_target_pos < 0 THEN
        -- Already at oldest history entry
        RETURN v_current_scope;
    END IF;

    -- Get the snapshot at target position
    SELECT scope_snapshot INTO v_snapshot
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id
      AND position = v_target_pos;

    IF v_snapshot IS NULL THEN
        RETURN v_current_scope;
    END IF;

    -- Restore scope from snapshot
    UPDATE "ob-poc".session_scopes
    SET scope_type = v_snapshot->>'scope_type',
        apex_entity_id = (v_snapshot->>'apex_entity_id')::UUID,
        apex_entity_name = v_snapshot->>'apex_entity_name',
        cbu_id = (v_snapshot->>'cbu_id')::UUID,
        cbu_name = v_snapshot->>'cbu_name',
        jurisdiction_code = v_snapshot->>'jurisdiction_code',
        focal_entity_id = (v_snapshot->>'focal_entity_id')::UUID,
        focal_entity_name = v_snapshot->>'focal_entity_name',
        neighborhood_hops = (v_snapshot->>'neighborhood_hops')::INTEGER,
        scope_filters = COALESCE(v_snapshot->'scope_filters', '{}'),
        cursor_entity_id = (v_snapshot->>'cursor_entity_id')::UUID,
        cursor_entity_name = v_snapshot->>'cursor_entity_name',
        active_cbu_ids = COALESCE(
            ARRAY(SELECT jsonb_array_elements_text(v_snapshot->'active_cbu_ids')::UUID),
            '{}'
        ),
        history_position = v_target_pos,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".navigate_back IS
'Navigate back one step in scope history. Returns updated session_scopes row.';

-- -----------------------------------------------------------------------------
-- 4. Navigate forward in history
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".navigate_forward(
    p_session_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_current_scope "ob-poc".session_scopes;
    v_target_pos INTEGER;
    v_max_pos INTEGER;
    v_snapshot JSONB;
    v_result "ob-poc".session_scopes;
BEGIN
    -- Get current scope and position
    SELECT * INTO v_current_scope
    FROM "ob-poc".session_scopes
    WHERE session_id = p_session_id;

    IF v_current_scope IS NULL THEN
        RETURN NULL;
    END IF;

    -- If already at end of history, nothing to do
    IF v_current_scope.history_position < 0 THEN
        RETURN v_current_scope;
    END IF;

    -- Get max history position
    SELECT MAX(position) INTO v_max_pos
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id;

    -- Calculate target position (one step forward)
    v_target_pos := v_current_scope.history_position + 1;

    IF v_target_pos > v_max_pos THEN
        -- Moving past end of history - mark as "at end"
        UPDATE "ob-poc".session_scopes
        SET history_position = -1,
            updated_at = NOW()
        WHERE session_id = p_session_id
        RETURNING * INTO v_result;
        RETURN v_result;
    END IF;

    -- Get the snapshot at target position
    SELECT scope_snapshot INTO v_snapshot
    FROM "ob-poc".session_scope_history
    WHERE session_id = p_session_id
      AND position = v_target_pos;

    IF v_snapshot IS NULL THEN
        RETURN v_current_scope;
    END IF;

    -- Restore scope from snapshot
    UPDATE "ob-poc".session_scopes
    SET scope_type = v_snapshot->>'scope_type',
        apex_entity_id = (v_snapshot->>'apex_entity_id')::UUID,
        apex_entity_name = v_snapshot->>'apex_entity_name',
        cbu_id = (v_snapshot->>'cbu_id')::UUID,
        cbu_name = v_snapshot->>'cbu_name',
        jurisdiction_code = v_snapshot->>'jurisdiction_code',
        focal_entity_id = (v_snapshot->>'focal_entity_id')::UUID,
        focal_entity_name = v_snapshot->>'focal_entity_name',
        neighborhood_hops = (v_snapshot->>'neighborhood_hops')::INTEGER,
        scope_filters = COALESCE(v_snapshot->'scope_filters', '{}'),
        cursor_entity_id = (v_snapshot->>'cursor_entity_id')::UUID,
        cursor_entity_name = v_snapshot->>'cursor_entity_name',
        active_cbu_ids = COALESCE(
            ARRAY(SELECT jsonb_array_elements_text(v_snapshot->'active_cbu_ids')::UUID),
            '{}'
        ),
        history_position = v_target_pos,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".navigate_forward IS
'Navigate forward one step in scope history. Returns updated session_scopes row.';

-- -----------------------------------------------------------------------------
-- 5. Multi-CBU set operations
-- -----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".add_cbu_to_set(
    p_session_id UUID,
    p_cbu_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET active_cbu_ids = CASE
            WHEN p_cbu_id = ANY(active_cbu_ids) THEN active_cbu_ids  -- Already in set
            ELSE array_append(active_cbu_ids, p_cbu_id)
        END,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "ob-poc".remove_cbu_from_set(
    p_session_id UUID,
    p_cbu_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET active_cbu_ids = array_remove(active_cbu_ids, p_cbu_id),
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "ob-poc".clear_cbu_set(
    p_session_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET active_cbu_ids = '{}',
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "ob-poc".set_cbu_set(
    p_session_id UUID,
    p_cbu_ids UUID[]
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET active_cbu_ids = COALESCE(p_cbu_ids, '{}'),
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".add_cbu_to_set IS 'Add a CBU to the active set';
COMMENT ON FUNCTION "ob-poc".remove_cbu_from_set IS 'Remove a CBU from the active set';
COMMENT ON FUNCTION "ob-poc".clear_cbu_set IS 'Clear the active CBU set';
COMMENT ON FUNCTION "ob-poc".set_cbu_set IS 'Replace the entire active CBU set';

-- -----------------------------------------------------------------------------
-- 6. Update existing scope-setting functions to push history first
-- -----------------------------------------------------------------------------

-- Wrapper for set_scope_galaxy that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_galaxy_with_history(
    p_session_id UUID,
    p_apex_entity_id UUID
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    -- Push current state to history
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-galaxy');
    -- Set new scope
    RETURN "ob-poc".set_scope_galaxy(p_session_id, p_apex_entity_id);
END;
$$ LANGUAGE plpgsql;

-- Wrapper for set_scope_cbu that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_cbu_with_history(
    p_session_id UUID,
    p_cbu_id UUID
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-cbu');
    RETURN "ob-poc".set_scope_cbu(p_session_id, p_cbu_id);
END;
$$ LANGUAGE plpgsql;

-- Wrapper for set_scope_jurisdiction that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_jurisdiction_with_history(
    p_session_id UUID,
    p_jurisdiction_code VARCHAR(10)
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-jurisdiction');
    RETURN "ob-poc".set_scope_jurisdiction(p_session_id, p_jurisdiction_code);
END;
$$ LANGUAGE plpgsql;

-- Wrapper for set_scope_book that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_book_with_history(
    p_session_id UUID,
    p_apex_entity_id UUID,
    p_filters JSONB DEFAULT '{}'
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-book');
    RETURN "ob-poc".set_scope_book(p_session_id, p_apex_entity_id, p_filters);
END;
$$ LANGUAGE plpgsql;

-- Wrapper for set_scope_neighborhood that pushes history
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_neighborhood_with_history(
    p_session_id UUID,
    p_entity_id UUID,
    p_hops INTEGER DEFAULT 2
) RETURNS "ob-poc".session_scopes AS $$
BEGIN
    PERFORM "ob-poc".push_scope_history(p_session_id, 'dsl', 'session.set-neighborhood');
    RETURN "ob-poc".set_scope_neighborhood(p_session_id, p_entity_id, p_hops);
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- DONE
-- =============================================================================
