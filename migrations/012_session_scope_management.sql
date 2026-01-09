-- =============================================================================
-- Migration 012: Session Scope Management
-- =============================================================================
-- Purpose: Persistent storage for user session scope state
--   - Scope type (galaxy, book, cbu, jurisdiction, neighborhood)
--   - Scope parameters (apex entity, CBU, jurisdiction code, etc.)
--   - Cursor (focused entity within scope)
--   - History for back/forward navigation
--   - Bookmarks for saved scopes
--
-- Integrates with: UnifiedSessionContext, ViewState, AgentGraphContext
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. Session Scopes Table (current scope state per session)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS "ob-poc".session_scopes (
    session_scope_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Session identity (browser/REPL session)
    session_id UUID NOT NULL,
    user_id UUID,  -- Optional: for user-specific persistence

    -- Scope type discriminator (matches GraphScope enum)
    scope_type VARCHAR(50) NOT NULL DEFAULT 'empty',

    -- Scope parameters (only one set populated based on scope_type)
    -- For 'galaxy' / 'book': the apex entity
    apex_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    apex_entity_name VARCHAR(255),

    -- For 'cbu': single CBU focus
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    cbu_name VARCHAR(255),

    -- For 'jurisdiction': jurisdiction filter
    jurisdiction_code VARCHAR(10),

    -- For 'neighborhood': entity + hop count
    focal_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    focal_entity_name VARCHAR(255),
    neighborhood_hops INTEGER DEFAULT 2,

    -- For 'book': additional filters (JSONB for flexibility)
    -- e.g., {"jurisdictions": ["LU", "IE"], "entity_types": ["fund", "subfund"]}
    scope_filters JSONB DEFAULT '{}',

    -- Cursor: currently focused entity within scope
    cursor_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    cursor_entity_name VARCHAR(255),

    -- Scope statistics (cached for display)
    total_entities INTEGER DEFAULT 0,
    total_cbus INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ DEFAULT NOW() + INTERVAL '24 hours',

    -- Unique: one active scope per session
    UNIQUE(session_id)
);

COMMENT ON TABLE "ob-poc".session_scopes IS
'Persistent storage for user session scope state (galaxy, book, CBU, jurisdiction, neighborhood)';

COMMENT ON COLUMN "ob-poc".session_scopes.scope_type IS
'Discriminator: empty, galaxy, book, cbu, jurisdiction, neighborhood, custom';

COMMENT ON COLUMN "ob-poc".session_scopes.scope_filters IS
'Additional filters for book scope: jurisdictions[], entity_types[], etc.';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_session_scopes_session ON "ob-poc".session_scopes(session_id);
CREATE INDEX IF NOT EXISTS idx_session_scopes_user ON "ob-poc".session_scopes(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_session_scopes_expires ON "ob-poc".session_scopes(expires_at);

-- -----------------------------------------------------------------------------
-- 2. Session Scope History (for back/forward navigation)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS "ob-poc".session_scope_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL,

    -- Position in history (0 = oldest)
    position INTEGER NOT NULL,

    -- Snapshot of scope at this point
    scope_snapshot JSONB NOT NULL,

    -- What triggered this history entry
    change_source VARCHAR(50) NOT NULL DEFAULT 'dsl',
    change_verb VARCHAR(100),  -- e.g., 'session.set-cbu'

    -- Timestamp
    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- Composite index for efficient history navigation
    UNIQUE(session_id, position)
);

COMMENT ON TABLE "ob-poc".session_scope_history IS
'Navigation history for back/forward in session scope';

COMMENT ON COLUMN "ob-poc".session_scope_history.change_source IS
'dsl, api, lexicon, navigation, system';

CREATE INDEX IF NOT EXISTS idx_scope_history_session ON "ob-poc".session_scope_history(session_id, position DESC);

-- -----------------------------------------------------------------------------
-- 3. Session Bookmarks (named saved scopes)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS "ob-poc".session_bookmarks (
    bookmark_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Owner (can be session-specific or user-specific)
    session_id UUID,
    user_id UUID,

    -- Bookmark name
    name VARCHAR(100) NOT NULL,
    description TEXT,

    -- Scope snapshot
    scope_snapshot JSONB NOT NULL,

    -- Metadata
    icon VARCHAR(50),  -- emoji or icon name
    color VARCHAR(20),  -- for UI highlighting

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    use_count INTEGER DEFAULT 0
);

-- Unique name per user (or per session if no user) - use unique index instead of constraint
CREATE UNIQUE INDEX IF NOT EXISTS idx_bookmarks_unique_name
ON "ob-poc".session_bookmarks(COALESCE(user_id, session_id), name);

COMMENT ON TABLE "ob-poc".session_bookmarks IS
'Named saved scopes for quick navigation';

CREATE INDEX IF NOT EXISTS idx_bookmarks_user ON "ob-poc".session_bookmarks(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_bookmarks_session ON "ob-poc".session_bookmarks(session_id) WHERE session_id IS NOT NULL;

-- -----------------------------------------------------------------------------
-- 4. Helper Functions
-- -----------------------------------------------------------------------------

-- Get or create session scope
CREATE OR REPLACE FUNCTION "ob-poc".get_or_create_session_scope(
    p_session_id UUID,
    p_user_id UUID DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_scope_id UUID;
BEGIN
    -- Try to find existing
    SELECT session_scope_id INTO v_scope_id
    FROM "ob-poc".session_scopes
    WHERE session_id = p_session_id;

    IF v_scope_id IS NULL THEN
        -- Create new empty scope
        INSERT INTO "ob-poc".session_scopes (session_id, user_id, scope_type)
        VALUES (p_session_id, p_user_id, 'empty')
        RETURNING session_scope_id INTO v_scope_id;
    ELSE
        -- Extend expiry
        UPDATE "ob-poc".session_scopes
        SET expires_at = NOW() + INTERVAL '24 hours',
            updated_at = NOW()
        WHERE session_scope_id = v_scope_id;
    END IF;

    RETURN v_scope_id;
END;
$$ LANGUAGE plpgsql;

-- Set scope to galaxy (all CBUs under apex)
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_galaxy(
    p_session_id UUID,
    p_apex_entity_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_apex_name VARCHAR(255);
    v_cbu_count INTEGER;
    v_entity_count INTEGER;
BEGIN
    -- Get apex name
    SELECT name INTO v_apex_name
    FROM "ob-poc".entities WHERE entity_id = p_apex_entity_id;

    -- Count CBUs under this apex (via commercial_client_entity_id)
    SELECT COUNT(*) INTO v_cbu_count
    FROM "ob-poc".cbus
    WHERE commercial_client_entity_id = p_apex_entity_id;

    -- Estimate entity count (CBUs * avg entities per CBU)
    -- For now, just use CBU count * 10 as estimate
    v_entity_count := v_cbu_count * 10;

    -- Upsert scope
    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        apex_entity_id, apex_entity_name,
        total_cbus, total_entities,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'galaxy',
        p_apex_entity_id, v_apex_name,
        v_cbu_count, v_entity_count,
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'galaxy',
        apex_entity_id = p_apex_entity_id,
        apex_entity_name = v_apex_name,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = NULL,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        scope_filters = '{}',
        total_cbus = v_cbu_count,
        total_entities = v_entity_count,
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set scope to book (filtered subset of galaxy)
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_book(
    p_session_id UUID,
    p_apex_entity_id UUID,
    p_filters JSONB DEFAULT '{}'
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_apex_name VARCHAR(255);
    v_cbu_count INTEGER;
BEGIN
    SELECT name INTO v_apex_name
    FROM "ob-poc".entities WHERE entity_id = p_apex_entity_id;

    -- Count CBUs matching filters
    -- For now, count all under apex (filter logic in application layer)
    SELECT COUNT(*) INTO v_cbu_count
    FROM "ob-poc".cbus
    WHERE commercial_client_entity_id = p_apex_entity_id;

    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        apex_entity_id, apex_entity_name,
        scope_filters, total_cbus,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'book',
        p_apex_entity_id, v_apex_name,
        p_filters, v_cbu_count,
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'book',
        apex_entity_id = p_apex_entity_id,
        apex_entity_name = v_apex_name,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = NULL,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        scope_filters = p_filters,
        total_cbus = v_cbu_count,
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set scope to single CBU
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_cbu(
    p_session_id UUID,
    p_cbu_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_cbu_name VARCHAR(255);
    v_entity_count INTEGER;
BEGIN
    SELECT name INTO v_cbu_name
    FROM "ob-poc".cbus WHERE cbu_id = p_cbu_id;

    -- Count entities in this CBU's ownership structure
    -- Simplified: count direct ownership relationships
    SELECT COUNT(DISTINCT e.entity_id) INTO v_entity_count
    FROM "ob-poc".entities e
    JOIN "ob-poc".entity_relationships er ON e.entity_id = er.from_entity_id OR e.entity_id = er.to_entity_id
    WHERE er.from_entity_id IN (SELECT entity_id FROM "ob-poc".cbus WHERE cbu_id = p_cbu_id)
       OR er.to_entity_id IN (SELECT entity_id FROM "ob-poc".cbus WHERE cbu_id = p_cbu_id);

    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        cbu_id, cbu_name,
        total_cbus, total_entities,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'cbu',
        p_cbu_id, v_cbu_name,
        1, COALESCE(v_entity_count, 0),
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'cbu',
        apex_entity_id = NULL,
        apex_entity_name = NULL,
        cbu_id = p_cbu_id,
        cbu_name = v_cbu_name,
        jurisdiction_code = NULL,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        scope_filters = '{}',
        total_cbus = 1,
        total_entities = COALESCE(v_entity_count, 0),
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set scope to jurisdiction
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_jurisdiction(
    p_session_id UUID,
    p_jurisdiction_code VARCHAR(10)
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_cbu_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO v_cbu_count
    FROM "ob-poc".cbus
    WHERE jurisdiction = p_jurisdiction_code;

    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        jurisdiction_code,
        total_cbus,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'jurisdiction',
        p_jurisdiction_code,
        v_cbu_count,
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'jurisdiction',
        apex_entity_id = NULL,
        apex_entity_name = NULL,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = p_jurisdiction_code,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        scope_filters = '{}',
        total_cbus = v_cbu_count,
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set scope to entity neighborhood
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_neighborhood(
    p_session_id UUID,
    p_entity_id UUID,
    p_hops INTEGER DEFAULT 2
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_entity_name VARCHAR(255);
BEGIN
    SELECT name INTO v_entity_name
    FROM "ob-poc".entities WHERE entity_id = p_entity_id;

    INSERT INTO "ob-poc".session_scopes (
        session_id, scope_type,
        focal_entity_id, focal_entity_name,
        neighborhood_hops,
        updated_at, expires_at
    ) VALUES (
        p_session_id, 'neighborhood',
        p_entity_id, v_entity_name,
        p_hops,
        NOW(), NOW() + INTERVAL '24 hours'
    )
    ON CONFLICT (session_id) DO UPDATE SET
        scope_type = 'neighborhood',
        apex_entity_id = NULL,
        apex_entity_name = NULL,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = NULL,
        focal_entity_id = p_entity_id,
        focal_entity_name = v_entity_name,
        neighborhood_hops = p_hops,
        scope_filters = '{}',
        updated_at = NOW(),
        expires_at = NOW() + INTERVAL '24 hours'
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Set cursor (focus entity within scope)
CREATE OR REPLACE FUNCTION "ob-poc".set_scope_cursor(
    p_session_id UUID,
    p_entity_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
    v_entity_name VARCHAR(255);
BEGIN
    SELECT name INTO v_entity_name
    FROM "ob-poc".entities WHERE entity_id = p_entity_id;

    UPDATE "ob-poc".session_scopes
    SET cursor_entity_id = p_entity_id,
        cursor_entity_name = v_entity_name,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Clear scope (reset to empty)
CREATE OR REPLACE FUNCTION "ob-poc".clear_scope(
    p_session_id UUID
) RETURNS "ob-poc".session_scopes AS $$
DECLARE
    v_result "ob-poc".session_scopes;
BEGIN
    UPDATE "ob-poc".session_scopes
    SET scope_type = 'empty',
        apex_entity_id = NULL,
        apex_entity_name = NULL,
        cbu_id = NULL,
        cbu_name = NULL,
        jurisdiction_code = NULL,
        focal_entity_id = NULL,
        focal_entity_name = NULL,
        neighborhood_hops = NULL,
        scope_filters = '{}',
        cursor_entity_id = NULL,
        cursor_entity_name = NULL,
        total_entities = 0,
        total_cbus = 0,
        updated_at = NOW()
    WHERE session_id = p_session_id
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- -----------------------------------------------------------------------------
-- 5. Views
-- -----------------------------------------------------------------------------

-- Current scope with enriched entity names
CREATE OR REPLACE VIEW "ob-poc".v_current_session_scope AS
SELECT
    ss.session_scope_id,
    ss.session_id,
    ss.user_id,
    ss.scope_type,

    -- Scope parameters
    ss.apex_entity_id,
    ss.apex_entity_name,
    ss.cbu_id,
    ss.cbu_name,
    ss.jurisdiction_code,
    ss.focal_entity_id,
    ss.focal_entity_name,
    ss.neighborhood_hops,
    ss.scope_filters,

    -- Cursor
    ss.cursor_entity_id,
    ss.cursor_entity_name,

    -- Stats
    ss.total_entities,
    ss.total_cbus,

    -- Display string
    CASE ss.scope_type
        WHEN 'empty' THEN 'No scope set'
        WHEN 'galaxy' THEN 'Galaxy: ' || ss.apex_entity_name || ' (' || ss.total_cbus || ' CBUs)'
        WHEN 'book' THEN 'Book: ' || ss.apex_entity_name || ' (filtered)'
        WHEN 'cbu' THEN 'CBU: ' || ss.cbu_name
        WHEN 'jurisdiction' THEN 'Jurisdiction: ' || ss.jurisdiction_code || ' (' || ss.total_cbus || ' CBUs)'
        WHEN 'neighborhood' THEN 'Neighborhood: ' || ss.focal_entity_name || ' (' || ss.neighborhood_hops || ' hops)'
        ELSE ss.scope_type
    END AS scope_display,

    -- Cursor display
    CASE
        WHEN ss.cursor_entity_id IS NOT NULL
        THEN '@ ' || ss.cursor_entity_name
        ELSE NULL
    END AS cursor_display,

    -- Timestamps
    ss.created_at,
    ss.updated_at,
    ss.expires_at,

    -- Is expired?
    ss.expires_at < NOW() AS is_expired

FROM "ob-poc".session_scopes ss;

COMMENT ON VIEW "ob-poc".v_current_session_scope IS
'Current session scope with display strings and enriched entity names';

-- -----------------------------------------------------------------------------
-- 6. Cleanup Job (delete expired scopes)
-- -----------------------------------------------------------------------------

-- Function to clean up expired sessions
CREATE OR REPLACE FUNCTION "ob-poc".cleanup_expired_session_scopes()
RETURNS INTEGER AS $$
DECLARE
    v_deleted INTEGER;
BEGIN
    -- Delete expired scopes
    WITH deleted AS (
        DELETE FROM "ob-poc".session_scopes
        WHERE expires_at < NOW()
        RETURNING session_id
    )
    SELECT COUNT(*) INTO v_deleted FROM deleted;

    -- Delete orphaned history
    DELETE FROM "ob-poc".session_scope_history h
    WHERE NOT EXISTS (
        SELECT 1 FROM "ob-poc".session_scopes s
        WHERE s.session_id = h.session_id
    );

    RETURN v_deleted;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- DONE
-- =============================================================================
