-- ═══════════════════════════════════════════════════════════════════════════
-- VISUALIZATION LAYOUT SUPPORT - Phase 2
-- ═══════════════════════════════════════════════════════════════════════════
--
-- This migration adds layout support tables:
--   1. layout_overrides - User-specified node positions (pinned, hidden, resized)
--   2. layout_cache - Incremental layout computation cache
--   3. layout_config - Global layout configuration settings
--
-- These tables support the egui graph widget's layout persistence and
-- incremental update capabilities.
--
-- Depends on: 20260105_visualization_config.sql (Phase 1)
--
-- ═══════════════════════════════════════════════════════════════════════════


-- ═══════════════════════════════════════════════════════════════════════════
-- 1. LAYOUT OVERRIDES TABLE
-- ═══════════════════════════════════════════════════════════════════════════
--
-- Stores user-specified modifications to node layout:
--   - Position offsets from computed layout
--   - Size overrides
--   - Pinned nodes (don't move during layout)
--   - Collapsed containers
--   - Hidden nodes
--
-- One record per (cbu_id, view_mode, node_id) combination.
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".layout_overrides (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- CONTEXT
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    view_mode VARCHAR(30) NOT NULL REFERENCES "ob-poc".view_modes(view_mode_code),
    user_id UUID,  -- Optional: per-user overrides (NULL = shared)

    -- NODE IDENTIFICATION
    node_id UUID NOT NULL,
    node_type VARCHAR(30) NOT NULL,

    -- POSITION OVERRIDES (offset from computed position)
    x_offset NUMERIC(8,2) DEFAULT 0.0,
    y_offset NUMERIC(8,2) DEFAULT 0.0,

    -- SIZE OVERRIDES (NULL = use default from node_types)
    width_override NUMERIC(6,1),
    height_override NUMERIC(6,1),

    -- STATE FLAGS
    pinned BOOLEAN DEFAULT false,      -- Don't move during layout recalc
    collapsed BOOLEAN DEFAULT false,   -- Container is collapsed
    hidden BOOLEAN DEFAULT false,      -- Node is hidden from view

    -- EXPANSION STATE (for containers)
    expansion_level INTEGER DEFAULT 1, -- How many levels deep to expand

    -- AUDIT
    created_by VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),

    -- UNIQUE CONSTRAINT
    UNIQUE(cbu_id, view_mode, node_id, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::UUID))
);

COMMENT ON TABLE "ob-poc".layout_overrides IS
'User-specified layout modifications for graph nodes. Supports position pinning, resizing, collapsing, and hiding.';

COMMENT ON COLUMN "ob-poc".layout_overrides.x_offset IS
'Horizontal offset from computed layout position. Applied after layout algorithm runs.';

COMMENT ON COLUMN "ob-poc".layout_overrides.y_offset IS
'Vertical offset from computed layout position. Applied after layout algorithm runs.';

COMMENT ON COLUMN "ob-poc".layout_overrides.pinned IS
'If true, node position is absolute (not relative to layout). Drag operations set this to true.';

COMMENT ON COLUMN "ob-poc".layout_overrides.collapsed IS
'If true and node is a container, children are hidden and node shows collapse indicator.';

COMMENT ON COLUMN "ob-poc".layout_overrides.expansion_level IS
'For containers: 0=show node only, 1=show immediate children, 2+=show deeper levels.';

-- Indexes for fast lookup
CREATE INDEX IF NOT EXISTS idx_layout_overrides_cbu_view
    ON "ob-poc".layout_overrides(cbu_id, view_mode);

CREATE INDEX IF NOT EXISTS idx_layout_overrides_node
    ON "ob-poc".layout_overrides(node_id);

CREATE INDEX IF NOT EXISTS idx_layout_overrides_user
    ON "ob-poc".layout_overrides(user_id) WHERE user_id IS NOT NULL;


-- ═══════════════════════════════════════════════════════════════════════════
-- 2. LAYOUT CACHE TABLE
-- ═══════════════════════════════════════════════════════════════════════════
--
-- Caches computed layout for incremental updates:
--   - Stores computed node positions and edge paths
--   - Keyed by hash of input data (nodes + edges + overrides)
--   - Invalidated when input changes
--
-- Enables:
--   - Fast initial render (use cached positions)
--   - Incremental layout (only recalculate changed portions)
--   - Animation from old to new positions
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".layout_cache (
    cache_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- CACHE KEY
    cbu_id UUID NOT NULL,
    view_mode VARCHAR(30) NOT NULL,
    user_id UUID,  -- Optional: per-user cache (NULL = shared)

    -- INPUT HASH (SHA-256 of nodes + edges + overrides)
    input_hash VARCHAR(64) NOT NULL,

    -- LAYOUT VERSION (for tracking algorithm changes)
    algorithm_version VARCHAR(20) DEFAULT 'v1.0',

    -- COMPUTED LAYOUT
    node_positions JSONB NOT NULL,
    -- Format: { "node_id": { "x": 100.0, "y": 200.0, "width": 160.0, "height": 60.0, "tier": 1 }, ... }

    edge_paths JSONB NOT NULL,
    -- Format: { "edge_id": { "path": [[x1,y1], [x2,y2], ...], "label_pos": [x,y] }, ... }

    -- LAYOUT METADATA
    bounding_box JSONB,
    -- Format: { "min_x": 0, "min_y": 0, "max_x": 800, "max_y": 600 }

    tier_info JSONB,
    -- Format: { "tier_count": 4, "tier_positions": [0, 120, 240, 360] }

    -- PERFORMANCE METRICS
    computation_time_ms INTEGER,
    node_count INTEGER,
    edge_count INTEGER,

    -- VALIDITY
    computed_at TIMESTAMPTZ DEFAULT now(),
    valid_until TIMESTAMPTZ,  -- Optional expiration

    -- UNIQUE CONSTRAINT
    UNIQUE(cbu_id, view_mode, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::UUID))
);

COMMENT ON TABLE "ob-poc".layout_cache IS
'Cached layout computations for fast rendering and incremental updates. Invalidated when input_hash changes.';

COMMENT ON COLUMN "ob-poc".layout_cache.input_hash IS
'SHA-256 hash of normalized input data. Used to detect when layout needs recomputation.';

COMMENT ON COLUMN "ob-poc".layout_cache.node_positions IS
'JSONB map of node_id to computed position {x, y, width, height, tier}. Used for initial render.';

COMMENT ON COLUMN "ob-poc".layout_cache.edge_paths IS
'JSONB map of edge_id to path points and label position. Supports curved/orthogonal routing.';

COMMENT ON COLUMN "ob-poc".layout_cache.tier_info IS
'Computed tier information for hierarchical layouts. Used for tier-based node positioning.';

-- Indexes for fast lookup
CREATE INDEX IF NOT EXISTS idx_layout_cache_lookup
    ON "ob-poc".layout_cache(cbu_id, view_mode);

CREATE INDEX IF NOT EXISTS idx_layout_cache_hash
    ON "ob-poc".layout_cache(input_hash);

CREATE INDEX IF NOT EXISTS idx_layout_cache_valid
    ON "ob-poc".layout_cache(valid_until) WHERE valid_until IS NOT NULL;


-- ═══════════════════════════════════════════════════════════════════════════
-- 3. LAYOUT CONFIG TABLE
-- ═══════════════════════════════════════════════════════════════════════════
--
-- Global layout configuration settings:
--   - Aspect ratio constraints
--   - Node separation minimums
--   - Edge crossing penalties
--   - Animation timing
--   - Label rendering settings
--
-- Key-value store with JSONB values for flexibility.
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".layout_config (
    config_key VARCHAR(50) PRIMARY KEY,
    config_value JSONB NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE "ob-poc".layout_config IS
'Global layout configuration settings. Key-value store with JSONB values.';

-- SEED CONFIGURATION VALUES
INSERT INTO "ob-poc".layout_config (config_key, config_value, description) VALUES

-- ASPECT RATIO
('aspect_ratio', '{
    "min": 0.5,
    "max": 2.0,
    "preferred": 1.2,
    "enforce": false
}'::JSONB,
'Overall graph aspect ratio constraints. Used by layout algorithm to prefer certain shapes.'),

-- NODE SEPARATION
('node_separation', '{
    "unconnected": 40.0,
    "sibling": 20.0,
    "same_tier": 30.0,
    "container_padding": 20.0
}'::JSONB,
'Minimum distance between nodes in different contexts.'),

-- TIER SPACING
('tier_spacing', '{
    "default": 120.0,
    "compact": 80.0,
    "expanded": 160.0,
    "auto_adjust": true
}'::JSONB,
'Vertical spacing between tiers in hierarchical layouts.'),

-- EDGE CROSSING
('edge_crossing', '{
    "penalty_same_bundle": 100,
    "penalty_different_bundle": 10,
    "max_iterations": 50,
    "optimize": true
}'::JSONB,
'Edge crossing optimization parameters. Higher penalties result in fewer crossings.'),

-- ANIMATION
('animation', '{
    "node_move_ms": 300,
    "node_appear_ms": 200,
    "node_disappear_ms": 150,
    "zoom_ms": 200,
    "pan_ms": 150,
    "easing": "ease-out"
}'::JSONB,
'Animation timing for graph transitions. Used by egui/WASM renderer.'),

-- EDGE LABELS
('edge_labels', '{
    "collision_detection": true,
    "collision_offset": 15.0,
    "max_length": 20,
    "truncate_suffix": "...",
    "font_size": 11.0
}'::JSONB,
'Edge label rendering configuration.'),

-- EDGE ROUTING
('edge_routing', '{
    "style": "orthogonal",
    "corner_radius": 8.0,
    "bundle_separation": 5.0,
    "avoid_nodes": true,
    "self_loop_radius": 30.0
}'::JSONB,
'Edge path routing algorithm settings.'),

-- ZOOM LEVELS
('zoom_levels', '{
    "min": 0.1,
    "max": 3.0,
    "default": 1.0,
    "fit_padding": 40.0,
    "step": 0.1
}'::JSONB,
'Camera zoom constraints and defaults.'),

-- CONTAINER EXPANSION
('container_expansion', '{
    "auto_expand_threshold": 5,
    "default_expansion_level": 1,
    "show_child_count": true,
    "collapse_indicator_size": 16.0
}'::JSONB,
'Container node expansion behavior.'),

-- SELECTION
('selection', '{
    "multi_select": true,
    "highlight_connected": true,
    "fade_unselected": true,
    "fade_opacity": 0.3
}'::JSONB,
'Node selection behavior.'),

-- PERFORMANCE
('performance', '{
    "max_visible_nodes": 200,
    "lod_threshold": 100,
    "cache_ttl_seconds": 3600,
    "debounce_layout_ms": 100
}'::JSONB,
'Performance tuning parameters.')

ON CONFLICT (config_key) DO UPDATE SET
    config_value = EXCLUDED.config_value,
    description = EXCLUDED.description,
    updated_at = now();


-- ═══════════════════════════════════════════════════════════════════════════
-- 4. HELPER FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════

-- Function to get layout config value
CREATE OR REPLACE FUNCTION "ob-poc".get_layout_config(p_key VARCHAR)
RETURNS JSONB AS $$
    SELECT config_value FROM "ob-poc".layout_config WHERE config_key = p_key;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION "ob-poc".get_layout_config IS
'Get a layout configuration value by key. Returns JSONB.';


-- Function to invalidate layout cache for a CBU
CREATE OR REPLACE FUNCTION "ob-poc".invalidate_layout_cache(
    p_cbu_id UUID,
    p_view_mode VARCHAR DEFAULT NULL
)
RETURNS INTEGER AS $$
DECLARE
    v_count INTEGER;
BEGIN
    IF p_view_mode IS NULL THEN
        DELETE FROM "ob-poc".layout_cache WHERE cbu_id = p_cbu_id;
    ELSE
        DELETE FROM "ob-poc".layout_cache
        WHERE cbu_id = p_cbu_id AND view_mode = p_view_mode;
    END IF;
    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".invalidate_layout_cache IS
'Invalidate layout cache for a CBU. Pass view_mode to invalidate only that view, or NULL for all views.';


-- Function to reset layout overrides for a CBU
CREATE OR REPLACE FUNCTION "ob-poc".reset_layout_overrides(
    p_cbu_id UUID,
    p_view_mode VARCHAR DEFAULT NULL,
    p_user_id UUID DEFAULT NULL
)
RETURNS INTEGER AS $$
DECLARE
    v_count INTEGER;
BEGIN
    DELETE FROM "ob-poc".layout_overrides
    WHERE cbu_id = p_cbu_id
      AND (p_view_mode IS NULL OR view_mode = p_view_mode)
      AND (p_user_id IS NULL OR user_id = p_user_id);
    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".reset_layout_overrides IS
'Reset layout overrides for a CBU. Pass view_mode and/or user_id to limit scope.';


-- ═══════════════════════════════════════════════════════════════════════════
-- 5. TRIGGER TO INVALIDATE CACHE ON DATA CHANGES
-- ═══════════════════════════════════════════════════════════════════════════

-- Trigger function to invalidate cache when underlying data changes
CREATE OR REPLACE FUNCTION "ob-poc".trigger_invalidate_layout_cache()
RETURNS TRIGGER AS $$
BEGIN
    -- For entity_relationships changes, find affected CBUs
    IF TG_TABLE_NAME = 'entity_relationships' THEN
        -- Find CBUs that include either entity
        DELETE FROM "ob-poc".layout_cache lc
        WHERE lc.cbu_id IN (
            SELECT DISTINCT cer.cbu_id
            FROM "ob-poc".cbu_entity_roles cer
            WHERE cer.entity_id IN (
                COALESCE(NEW.from_entity_id, OLD.from_entity_id),
                COALESCE(NEW.to_entity_id, OLD.to_entity_id)
            )
        );
    END IF;

    -- For cbu_entity_roles changes
    IF TG_TABLE_NAME = 'cbu_entity_roles' THEN
        DELETE FROM "ob-poc".layout_cache
        WHERE cbu_id = COALESCE(NEW.cbu_id, OLD.cbu_id);
    END IF;

    -- For cbu_products changes
    IF TG_TABLE_NAME = 'cbu_products' THEN
        DELETE FROM "ob-poc".layout_cache
        WHERE cbu_id = COALESCE(NEW.cbu_id, OLD.cbu_id);
    END IF;

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Apply triggers to key tables
DROP TRIGGER IF EXISTS trg_invalidate_cache_entity_relationships ON "ob-poc".entity_relationships;
CREATE TRIGGER trg_invalidate_cache_entity_relationships
    AFTER INSERT OR UPDATE OR DELETE ON "ob-poc".entity_relationships
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".trigger_invalidate_layout_cache();

DROP TRIGGER IF EXISTS trg_invalidate_cache_cbu_entity_roles ON "ob-poc".cbu_entity_roles;
CREATE TRIGGER trg_invalidate_cache_cbu_entity_roles
    AFTER INSERT OR UPDATE OR DELETE ON "ob-poc".cbu_entity_roles
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".trigger_invalidate_layout_cache();

DROP TRIGGER IF EXISTS trg_invalidate_cache_cbu_products ON "ob-poc".cbu_products;
CREATE TRIGGER trg_invalidate_cache_cbu_products
    AFTER INSERT OR UPDATE OR DELETE ON "ob-poc".cbu_products
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".trigger_invalidate_layout_cache();


-- ═══════════════════════════════════════════════════════════════════════════
-- 6. VIEW FOR LAYOUT OVERRIDE STATUS
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW "ob-poc".v_layout_override_summary AS
SELECT
    lo.cbu_id,
    lo.view_mode,
    lo.user_id,
    COUNT(*) AS total_overrides,
    COUNT(*) FILTER (WHERE lo.pinned) AS pinned_count,
    COUNT(*) FILTER (WHERE lo.collapsed) AS collapsed_count,
    COUNT(*) FILTER (WHERE lo.hidden) AS hidden_count,
    COUNT(*) FILTER (WHERE lo.width_override IS NOT NULL OR lo.height_override IS NOT NULL) AS resized_count,
    MAX(lo.updated_at) AS last_updated
FROM "ob-poc".layout_overrides lo
GROUP BY lo.cbu_id, lo.view_mode, lo.user_id;

COMMENT ON VIEW "ob-poc".v_layout_override_summary IS
'Summary of layout overrides per CBU/view/user. Useful for UI to show customization status.';


-- ═══════════════════════════════════════════════════════════════════════════
-- FINAL COMMENTS
-- ═══════════════════════════════════════════════════════════════════════════

COMMENT ON SCHEMA "ob-poc" IS
'OB-POC schema with config-driven visualization. Phase 2 adds layout persistence and caching.';
