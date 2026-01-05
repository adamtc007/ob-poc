-- ═══════════════════════════════════════════════════════════════════════════
-- VISUALIZATION CONFIG SCHEMA - Phase 1
-- ═══════════════════════════════════════════════════════════════════════════
--
-- This migration establishes config-driven visualization tables:
--   1. node_types - Node type definitions with view applicability
--   2. edge_types - Edge type definitions with layout hints
--   3. view_modes - Per-view configuration
--   4. v_all_edges - Unified edges view
--
-- Replaces hardcoded Rust logic (is_ubo_relevant, is_trading_relevant) with
-- database configuration. "Config, not code."
--
-- ═══════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════
-- 1. NODE TYPES TABLE
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".node_types (
    node_type_code VARCHAR(30) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    description TEXT,

    -- VIEW APPLICABILITY (replaces hardcoded Rust filtering)
    show_in_ubo_view BOOLEAN DEFAULT false,
    show_in_trading_view BOOLEAN DEFAULT false,
    show_in_fund_structure_view BOOLEAN DEFAULT false,
    show_in_service_view BOOLEAN DEFAULT false,
    show_in_product_view BOOLEAN DEFAULT false,

    -- RENDERING HINTS
    icon VARCHAR(50),
    default_color VARCHAR(30),
    default_shape VARCHAR(30) DEFAULT 'RECTANGLE',

    -- LAYOUT HINTS
    default_width NUMERIC(6,1) DEFAULT 160.0,
    default_height NUMERIC(6,1) DEFAULT 60.0,
    can_be_container BOOLEAN DEFAULT false,
    default_tier INTEGER,
    importance_weight NUMERIC(3,2) DEFAULT 1.0,

    -- CONTAINER LAYOUT
    child_layout_mode VARCHAR(20) DEFAULT 'VERTICAL',
    container_padding NUMERIC(5,1) DEFAULT 20.0,

    -- SEMANTIC ZOOM
    collapse_below_zoom NUMERIC(3,2) DEFAULT 0.3,
    hide_label_below_zoom NUMERIC(3,2) DEFAULT 0.2,
    show_detail_above_zoom NUMERIC(3,2) DEFAULT 0.7,

    -- FAN-OUT CONTROL
    max_visible_children INTEGER DEFAULT 20,
    overflow_behavior VARCHAR(20) DEFAULT 'COLLAPSE',

    -- DEDUPLICATION
    dedupe_mode VARCHAR(20) DEFAULT 'SINGLE',
    min_separation NUMERIC(5,1) DEFAULT 20.0,

    -- RENDERING ORDER
    z_order INTEGER DEFAULT 100,

    -- SEMANTIC FLAGS
    is_kyc_subject BOOLEAN DEFAULT false,
    is_structural BOOLEAN DEFAULT false,
    is_operational BOOLEAN DEFAULT false,
    is_trading BOOLEAN DEFAULT false,

    sort_order INTEGER DEFAULT 100,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE "ob-poc".node_types IS
'Config-driven node type definitions with view applicability and layout hints. Replaces hardcoded Rust enums.';

-- SEED NODE TYPES
INSERT INTO "ob-poc".node_types (
    node_type_code, display_name, description,
    show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
    show_in_service_view, show_in_product_view,
    icon, default_color, default_shape,
    default_width, default_height, can_be_container, default_tier, importance_weight,
    is_kyc_subject, is_structural, is_operational, is_trading,
    z_order, sort_order
) VALUES
-- Core CBU
('CBU', 'Client Business Unit', 'The commercial client container',
 true, true, true, true, true,
 'building', '#3B82F6', 'RECTANGLE',
 200.0, 80.0, true, 0, 1.0,
 false, false, false, false,
 150, 0),

-- Entity types (map to entity_types.entity_category)
('ENTITY_PERSON', 'Natural Person', 'Individual / natural person',
 true, true, false, false, false,
 'user', '#10B981', 'ELLIPSE',
 140.0, 60.0, false, NULL, 0.9,
 true, false, false, false,
 100, 10),

('ENTITY_COMPANY', 'Company', 'Legal entity - company/corporation',
 true, true, true, false, false,
 'building-2', '#6366F1', 'RECTANGLE',
 160.0, 60.0, false, NULL, 0.8,
 true, false, false, false,
 100, 11),

('ENTITY_FUND', 'Fund', 'Investment fund entity',
 true, true, true, false, false,
 'pie-chart', '#8B5CF6', 'RECTANGLE',
 160.0, 60.0, true, NULL, 0.85,
 true, true, false, false,
 100, 12),

('ENTITY_TRUST', 'Trust', 'Trust structure',
 true, false, false, false, false,
 'shield', '#EC4899', 'DIAMOND',
 140.0, 70.0, false, NULL, 0.8,
 true, false, false, false,
 100, 13),

('ENTITY_PARTNERSHIP', 'Partnership', 'LP/LLP/GP structure',
 true, true, false, false, false,
 'users', '#F59E0B', 'RECTANGLE',
 160.0, 60.0, false, NULL, 0.8,
 true, false, false, false,
 100, 14),

-- Service delivery chain
('PRODUCT', 'Product', 'Service product offering',
 false, false, false, true, true,
 'box', '#22C55E', 'RECTANGLE',
 160.0, 50.0, false, 1, 0.7,
 false, false, true, false,
 100, 100),

('SERVICE', 'Service', 'Service delivered by product',
 false, false, false, true, false,
 'cog', '#64748B', 'RECTANGLE',
 140.0, 50.0, false, 2, 0.6,
 false, false, true, false,
 100, 110),

('RESOURCE', 'Resource', 'Resource instance for service',
 false, false, false, true, false,
 'database', '#94A3B8', 'RECTANGLE',
 140.0, 50.0, false, 3, 0.5,
 false, false, true, false,
 100, 120),

-- Trading world
('TRADING_PROFILE', 'Trading Profile', 'Trading authorization configuration',
 false, true, false, false, false,
 'chart-bar', '#0EA5E9', 'RECTANGLE',
 180.0, 60.0, true, 1, 0.8,
 false, false, false, true,
 100, 200),

('INSTRUMENT_MATRIX', 'Instrument Matrix', 'Authorized instrument classes/markets',
 false, true, false, false, false,
 'grid', '#0284C7', 'RECTANGLE',
 200.0, 100.0, false, 2, 0.7,
 false, false, false, true,
 100, 210),

-- Fund structure
('UMBRELLA_FUND', 'Umbrella Fund', 'Umbrella containing subfunds',
 false, false, true, false, false,
 'layers', '#7C3AED', 'RECTANGLE',
 180.0, 70.0, true, 0, 0.9,
 false, true, false, false,
 150, 300),

('SHARE_CLASS', 'Share Class', 'Fund share class',
 false, false, true, false, false,
 'tag', '#A78BFA', 'RECTANGLE',
 120.0, 40.0, false, 2, 0.5,
 false, true, false, false,
 100, 310)

ON CONFLICT (node_type_code) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    show_in_ubo_view = EXCLUDED.show_in_ubo_view,
    show_in_trading_view = EXCLUDED.show_in_trading_view,
    show_in_fund_structure_view = EXCLUDED.show_in_fund_structure_view,
    show_in_service_view = EXCLUDED.show_in_service_view,
    show_in_product_view = EXCLUDED.show_in_product_view,
    icon = EXCLUDED.icon,
    default_color = EXCLUDED.default_color,
    default_shape = EXCLUDED.default_shape,
    default_width = EXCLUDED.default_width,
    default_height = EXCLUDED.default_height,
    can_be_container = EXCLUDED.can_be_container,
    default_tier = EXCLUDED.default_tier,
    importance_weight = EXCLUDED.importance_weight,
    is_kyc_subject = EXCLUDED.is_kyc_subject,
    is_structural = EXCLUDED.is_structural,
    is_operational = EXCLUDED.is_operational,
    is_trading = EXCLUDED.is_trading,
    z_order = EXCLUDED.z_order,
    sort_order = EXCLUDED.sort_order,
    updated_at = now();


-- ═══════════════════════════════════════════════════════════════════════════
-- 2. EDGE TYPES TABLE
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".edge_types (
    edge_type_code VARCHAR(50) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    description TEXT,

    -- ENDPOINT CONSTRAINTS
    from_node_types JSONB NOT NULL,
    to_node_types JSONB NOT NULL,

    -- VIEW APPLICABILITY (replaces hardcoded Rust filtering)
    show_in_ubo_view BOOLEAN DEFAULT false,
    show_in_trading_view BOOLEAN DEFAULT false,
    show_in_fund_structure_view BOOLEAN DEFAULT false,
    show_in_service_view BOOLEAN DEFAULT false,
    show_in_product_view BOOLEAN DEFAULT false,

    -- RENDERING HINTS
    edge_style VARCHAR(30) DEFAULT 'SOLID',
    edge_color VARCHAR(30),
    edge_width NUMERIC(3,1) DEFAULT 1.0,
    arrow_style VARCHAR(30) DEFAULT 'SINGLE',
    shows_percentage BOOLEAN DEFAULT false,
    shows_label BOOLEAN DEFAULT true,
    label_template VARCHAR(100),
    label_position VARCHAR(20) DEFAULT 'MIDDLE',

    -- LAYOUT HINTS
    layout_direction VARCHAR(20) DEFAULT 'DOWN',
    tier_delta INTEGER DEFAULT 1,
    is_hierarchical BOOLEAN DEFAULT true,
    bundle_group VARCHAR(30),
    routing_priority INTEGER DEFAULT 50,
    spring_strength NUMERIC(4,3) DEFAULT 1.0,
    ideal_length NUMERIC(6,1) DEFAULT 100.0,

    -- SIBLING ORDERING
    sibling_sort_key VARCHAR(30) DEFAULT 'PERCENTAGE_DESC',

    -- ANCHOR POINTS
    source_anchor VARCHAR(20) DEFAULT 'AUTO',
    target_anchor VARCHAR(20) DEFAULT 'AUTO',

    -- CYCLE HANDLING
    cycle_break_priority INTEGER DEFAULT 50,

    -- MULTI-PARENT
    is_primary_parent_rule VARCHAR(50) DEFAULT 'HIGHEST_PERCENTAGE',

    -- PARALLEL EDGES
    parallel_edge_offset NUMERIC(4,1) DEFAULT 15.0,

    -- SELF-LOOPS
    self_loop_radius NUMERIC(4,1) DEFAULT 30.0,
    self_loop_position VARCHAR(20) DEFAULT 'TOP_RIGHT',

    -- RENDERING ORDER
    z_order INTEGER DEFAULT 50,

    -- SEMANTIC FLAGS
    is_ownership BOOLEAN DEFAULT false,
    is_control BOOLEAN DEFAULT false,
    is_structural BOOLEAN DEFAULT false,
    is_service_delivery BOOLEAN DEFAULT false,
    is_trading BOOLEAN DEFAULT false,
    creates_kyc_obligation BOOLEAN DEFAULT false,

    -- CARDINALITY
    cardinality VARCHAR(10) DEFAULT '1:N',

    sort_order INTEGER DEFAULT 100,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE "ob-poc".edge_types IS
'Config-driven edge type definitions with view applicability and layout hints. Replaces hardcoded relationship handling.';

-- SEED EDGE TYPES
INSERT INTO "ob-poc".edge_types (
    edge_type_code, display_name, description,
    from_node_types, to_node_types,
    show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
    show_in_service_view, show_in_product_view,
    edge_style, edge_color, edge_width, arrow_style, shows_percentage, shows_label, label_template,
    layout_direction, tier_delta, is_hierarchical, bundle_group, routing_priority,
    is_ownership, is_control, is_structural, is_service_delivery, is_trading, creates_kyc_obligation,
    cardinality, sort_order
) VALUES

-- ═══════════════════════════════════════════════════════════════════════════
-- OWNERSHIP CHAIN (UBO View)
-- ═══════════════════════════════════════════════════════════════════════════
('OWNERSHIP', 'Ownership', 'Equity/ownership interest',
 '["ENTITY_PERSON", "ENTITY_COMPANY", "ENTITY_FUND", "ENTITY_TRUST", "ENTITY_PARTNERSHIP"]'::JSONB,
 '["ENTITY_COMPANY", "ENTITY_FUND", "ENTITY_TRUST", "ENTITY_PARTNERSHIP"]'::JSONB,
 true, false, false, false, false,
 'SOLID', NULL, 2.0, 'SINGLE', true, true, '{percentage}%',
 'UP', 1, true, 'ownership', 100,
 true, false, false, false, false, true,
 'N:1', 10),

('INDIRECT_OWNERSHIP', 'Indirect Ownership', 'Beneficial ownership through chain',
 '["ENTITY_PERSON", "ENTITY_COMPANY"]'::JSONB,
 '["ENTITY_COMPANY", "ENTITY_FUND"]'::JSONB,
 true, false, false, false, false,
 'DASHED', '#9CA3AF', 1.5, 'SINGLE', true, true, '{percentage}% indirect',
 'UP', 2, false, 'ownership', 80,
 true, false, false, false, false, true,
 'N:1', 15),

-- ═══════════════════════════════════════════════════════════════════════════
-- CONTROL CHAIN (UBO + Trading Views)
-- ═══════════════════════════════════════════════════════════════════════════
('CONTROL', 'Control', 'Control without ownership (voting, board)',
 '["ENTITY_PERSON", "ENTITY_COMPANY"]'::JSONB,
 '["ENTITY_COMPANY", "ENTITY_FUND", "ENTITY_TRUST"]'::JSONB,
 true, true, false, false, false,
 'DASHED', '#F97316', 1.5, 'SINGLE', false, true, 'controls',
 'UP', 1, false, 'control', 90,
 false, true, false, false, false, true,
 'N:M', 20),

('BOARD_MEMBER', 'Board Member', 'Director/board membership',
 '["ENTITY_PERSON"]'::JSONB,
 '["ENTITY_COMPANY", "ENTITY_FUND"]'::JSONB,
 true, true, false, false, false,
 'DOTTED', '#F97316', 1.0, 'NONE', false, true, 'director',
 'BIDIRECTIONAL', 0, false, 'control', 70,
 false, true, false, false, false, true,
 'N:M', 25),

-- ═══════════════════════════════════════════════════════════════════════════
-- TRUST ROLES (UBO View)
-- ═══════════════════════════════════════════════════════════════════════════
('TRUST_SETTLOR', 'Trust Settlor', 'Settlor of trust',
 '["ENTITY_PERSON", "ENTITY_COMPANY"]'::JSONB,
 '["ENTITY_TRUST"]'::JSONB,
 true, false, false, false, false,
 'SOLID', '#EC4899', 2.0, 'SINGLE', false, true, 'settlor',
 'UP', 1, true, 'trust', 95,
 false, true, false, false, false, true,
 '1:N', 30),

('TRUST_TRUSTEE', 'Trust Trustee', 'Trustee of trust',
 '["ENTITY_PERSON", "ENTITY_COMPANY"]'::JSONB,
 '["ENTITY_TRUST"]'::JSONB,
 true, true, false, false, false,
 'SOLID', '#EC4899', 1.5, 'DOUBLE', false, true, 'trustee',
 'BIDIRECTIONAL', 0, false, 'trust', 85,
 false, true, false, false, false, true,
 'N:M', 31),

('TRUST_BENEFICIARY', 'Trust Beneficiary', 'Beneficiary of trust',
 '["ENTITY_TRUST"]'::JSONB,
 '["ENTITY_PERSON", "ENTITY_COMPANY"]'::JSONB,
 true, false, false, false, false,
 'SOLID', '#EC4899', 1.5, 'SINGLE', true, true, 'beneficiary',
 'DOWN', 1, true, 'trust', 85,
 true, false, false, false, false, true,
 '1:N', 32),

('TRUST_PROTECTOR', 'Trust Protector', 'Protector of trust',
 '["ENTITY_PERSON", "ENTITY_COMPANY"]'::JSONB,
 '["ENTITY_TRUST"]'::JSONB,
 true, false, false, false, false,
 'DOTTED', '#EC4899', 1.0, 'SINGLE', false, true, 'protector',
 'BIDIRECTIONAL', 0, false, 'trust', 60,
 false, true, false, false, false, false,
 'N:M', 33),

-- ═══════════════════════════════════════════════════════════════════════════
-- CBU ↔ ENTITY ROLES (UBO + Trading Views)
-- ═══════════════════════════════════════════════════════════════════════════
('CBU_ROLE', 'CBU Role', 'Entity role assignment within CBU',
 '["CBU"]'::JSONB,
 '["ENTITY_PERSON", "ENTITY_COMPANY", "ENTITY_FUND", "ENTITY_TRUST", "ENTITY_PARTNERSHIP"]'::JSONB,
 true, true, false, false, false,
 'SOLID', '#3B82F6', 1.5, 'SINGLE', false, true, '{role}',
 'DOWN', 1, true, NULL, 100,
 false, false, false, false, false, false,
 '1:N', 50),

-- ═══════════════════════════════════════════════════════════════════════════
-- SERVICE DELIVERY CHAIN (Service View)
-- ═══════════════════════════════════════════════════════════════════════════
('CBU_USES_PRODUCT', 'Uses Product', 'CBU subscribes to product',
 '["CBU"]'::JSONB,
 '["PRODUCT"]'::JSONB,
 false, false, false, true, true,
 'SOLID', '#22C55E', 2.0, 'SINGLE', false, true, NULL,
 'DOWN', 1, true, 'service', 100,
 false, false, false, true, false, false,
 '1:N', 100),

('PRODUCT_PROVIDES_SERVICE', 'Provides Service', 'Product delivers service',
 '["PRODUCT"]'::JSONB,
 '["SERVICE"]'::JSONB,
 false, false, false, true, true,
 'SOLID', '#22C55E', 1.5, 'SINGLE', false, true, NULL,
 'DOWN', 1, true, 'service', 90,
 false, false, false, true, false, false,
 '1:N', 110),

('SERVICE_USES_RESOURCE', 'Uses Resource', 'Service requires resource',
 '["SERVICE"]'::JSONB,
 '["RESOURCE"]'::JSONB,
 false, false, false, true, false,
 'SOLID', '#64748B', 1.0, 'SINGLE', false, true, NULL,
 'DOWN', 1, true, 'service', 80,
 false, false, false, true, false, false,
 '1:N', 120),

-- ═══════════════════════════════════════════════════════════════════════════
-- TRADING WORLD (Trading View)
-- ═══════════════════════════════════════════════════════════════════════════
('CBU_HAS_TRADING_PROFILE', 'Has Trading Profile', 'CBU trading configuration',
 '["CBU"]'::JSONB,
 '["TRADING_PROFILE"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#0EA5E9', 2.0, 'SINGLE', false, false, NULL,
 'DOWN', 1, true, 'trading', 100,
 false, false, false, false, true, false,
 '1:N', 200),

('TRADING_PROFILE_HAS_MATRIX', 'Has Matrix', 'Trading profile instrument authorization',
 '["TRADING_PROFILE"]'::JSONB,
 '["INSTRUMENT_MATRIX"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#0EA5E9', 1.5, 'SINGLE', false, false, NULL,
 'DOWN', 1, true, 'trading', 90,
 false, false, false, false, true, false,
 '1:1', 210),

('ENTITY_AUTHORIZES_TRADING', 'Authorizes Trading', 'Entity authorized for trading profile',
 '["ENTITY_PERSON", "ENTITY_COMPANY"]'::JSONB,
 '["TRADING_PROFILE"]'::JSONB,
 false, true, false, false, false,
 'DASHED', '#0EA5E9', 1.0, 'SINGLE', false, true, '{role}',
 'BIDIRECTIONAL', 0, false, 'trading', 70,
 false, false, false, false, true, false,
 'N:M', 220),

-- ═══════════════════════════════════════════════════════════════════════════
-- FUND STRUCTURE (Fund Structure View)
-- ═══════════════════════════════════════════════════════════════════════════
('UMBRELLA_CONTAINS_SUBFUND', 'Contains Subfund', 'Umbrella contains subfund',
 '["UMBRELLA_FUND", "ENTITY_FUND"]'::JSONB,
 '["ENTITY_FUND"]'::JSONB,
 false, false, true, false, false,
 'SOLID', '#7C3AED', 2.0, 'SINGLE', false, true, NULL,
 'DOWN', 1, true, 'fund_structure', 100,
 false, false, true, false, false, false,
 '1:N', 300),

('FUND_HAS_SHARE_CLASS', 'Has Share Class', 'Fund share class',
 '["ENTITY_FUND"]'::JSONB,
 '["SHARE_CLASS"]'::JSONB,
 false, false, true, false, false,
 'SOLID', '#A78BFA', 1.0, 'SINGLE', false, true, NULL,
 'DOWN', 1, true, 'fund_structure', 80,
 false, false, true, false, false, false,
 '1:N', 310),

('FEEDER_TO_MASTER', 'Feeder To Master', 'Feeder invests in master fund',
 '["ENTITY_FUND"]'::JSONB,
 '["ENTITY_FUND"]'::JSONB,
 false, false, true, false, false,
 'SOLID', '#7C3AED', 2.0, 'SINGLE', true, true, '{percentage}%',
 'UP', 1, true, 'fund_structure', 95,
 true, false, true, false, false, false,
 'N:1', 320),

('FUND_MANAGED_BY', 'Managed By', 'Fund management company',
 '["ENTITY_FUND"]'::JSONB,
 '["ENTITY_COMPANY"]'::JSONB,
 false, true, true, false, false,
 'SOLID', '#6366F1', 1.5, 'SINGLE', false, true, 'ManCo',
 'BIDIRECTIONAL', 0, false, 'fund_management', 85,
 false, false, true, false, false, false,
 'N:1', 330),

-- ═══════════════════════════════════════════════════════════════════════════
-- INVESTMENT VEHICLES (Trading View)
-- ═══════════════════════════════════════════════════════════════════════════
('INVESTS_IN_VEHICLE', 'Invests In', 'Asset owner invests in external pooled fund',
 '["ENTITY_COMPANY", "ENTITY_FUND"]'::JSONB,
 '["ENTITY_FUND"]'::JSONB,
 false, true, false, false, false,
 'DASHED', '#22C55E', 1.5, 'SINGLE', true, true, '{percentage}%',
 'DOWN', 1, true, 'investment', 75,
 true, false, false, false, true, false,
 '1:N', 400)

ON CONFLICT (edge_type_code) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    from_node_types = EXCLUDED.from_node_types,
    to_node_types = EXCLUDED.to_node_types,
    show_in_ubo_view = EXCLUDED.show_in_ubo_view,
    show_in_trading_view = EXCLUDED.show_in_trading_view,
    show_in_fund_structure_view = EXCLUDED.show_in_fund_structure_view,
    show_in_service_view = EXCLUDED.show_in_service_view,
    show_in_product_view = EXCLUDED.show_in_product_view,
    edge_style = EXCLUDED.edge_style,
    edge_color = EXCLUDED.edge_color,
    edge_width = EXCLUDED.edge_width,
    arrow_style = EXCLUDED.arrow_style,
    shows_percentage = EXCLUDED.shows_percentage,
    shows_label = EXCLUDED.shows_label,
    label_template = EXCLUDED.label_template,
    layout_direction = EXCLUDED.layout_direction,
    tier_delta = EXCLUDED.tier_delta,
    is_hierarchical = EXCLUDED.is_hierarchical,
    bundle_group = EXCLUDED.bundle_group,
    routing_priority = EXCLUDED.routing_priority,
    is_ownership = EXCLUDED.is_ownership,
    is_control = EXCLUDED.is_control,
    is_structural = EXCLUDED.is_structural,
    is_service_delivery = EXCLUDED.is_service_delivery,
    is_trading = EXCLUDED.is_trading,
    creates_kyc_obligation = EXCLUDED.creates_kyc_obligation,
    cardinality = EXCLUDED.cardinality,
    sort_order = EXCLUDED.sort_order,
    updated_at = now();


-- ═══════════════════════════════════════════════════════════════════════════
-- 3. VIEW MODES TABLE
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".view_modes (
    view_mode_code VARCHAR(30) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    description TEXT,

    -- ROOT IDENTIFICATION
    root_identification_rule VARCHAR(50) NOT NULL,
    -- Values: 'CBU', 'TERMINUS_ENTITIES', 'APEX_ENTITY', 'UMBRELLA_FUNDS'

    -- TRAVERSAL
    primary_traversal_direction VARCHAR(10) DEFAULT 'DOWN',

    -- EDGE CLASSIFICATION
    hierarchy_edge_types JSONB NOT NULL,
    overlay_edge_types JSONB DEFAULT '[]'::JSONB,

    -- LAYOUT ALGORITHM
    default_algorithm VARCHAR(30) DEFAULT 'HIERARCHICAL',
    algorithm_params JSONB DEFAULT '{}'::JSONB,

    -- SWIM LANES
    swim_lane_attribute VARCHAR(50),
    swim_lane_direction VARCHAR(10) DEFAULT 'VERTICAL',

    -- TEMPORAL
    temporal_axis VARCHAR(30),
    temporal_axis_direction VARCHAR(10) DEFAULT 'HORIZONTAL',

    -- GRID SNAPPING
    snap_to_grid BOOLEAN DEFAULT false,
    grid_size_x NUMERIC(5,1) DEFAULT 20.0,
    grid_size_y NUMERIC(5,1) DEFAULT 20.0,

    -- CLUSTERING
    auto_cluster BOOLEAN DEFAULT false,
    cluster_attribute VARCHAR(50),
    cluster_visual_style VARCHAR(20) DEFAULT 'BACKGROUND',

    created_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE "ob-poc".view_modes IS
'Per-view configuration including root identification, hierarchy edges, and layout algorithm.';

-- SEED VIEW MODES
INSERT INTO "ob-poc".view_modes VALUES
('UBO', 'UBO View', 'Beneficial ownership chains from terminus entities',
 'TERMINUS_ENTITIES', 'DOWN',
 '["OWNERSHIP", "TRUST_SETTLOR", "TRUST_BENEFICIARY"]'::JSONB,
 '["CONTROL", "BOARD_MEMBER", "TRUST_TRUSTEE", "TRUST_PROTECTOR"]'::JSONB,
 'HIERARCHICAL', '{"level_separation": 120, "node_separation": 160}'::JSONB,
 NULL, 'VERTICAL', NULL, 'HORIZONTAL', false, 20.0, 20.0, false, NULL, 'BACKGROUND'),

('KYC_UBO', 'KYC/UBO View', 'Full KYC with ownership from CBU perspective',
 'CBU', 'DOWN',
 '["CBU_ROLE", "OWNERSHIP"]'::JSONB,
 '["CONTROL"]'::JSONB,
 'HIERARCHICAL', '{"level_separation": 120, "node_separation": 160}'::JSONB,
 NULL, 'VERTICAL', NULL, 'HORIZONTAL', false, 20.0, 20.0, false, NULL, 'BACKGROUND'),

('TRADING', 'Trading View', 'Trading entities and authorization',
 'CBU', 'DOWN',
 '["CBU_ROLE", "CBU_HAS_TRADING_PROFILE", "TRADING_PROFILE_HAS_MATRIX"]'::JSONB,
 '["ENTITY_AUTHORIZES_TRADING", "CONTROL"]'::JSONB,
 'TIERED', '{"container_padding": 20}'::JSONB,
 NULL, 'VERTICAL', NULL, 'HORIZONTAL', false, 20.0, 20.0, false, NULL, 'BACKGROUND'),

('SERVICE', 'Service View', 'Product/service delivery chain',
 'CBU', 'DOWN',
 '["CBU_USES_PRODUCT", "PRODUCT_PROVIDES_SERVICE", "SERVICE_USES_RESOURCE"]'::JSONB,
 '[]'::JSONB,
 'HIERARCHICAL', '{"level_separation": 80, "node_separation": 140}'::JSONB,
 NULL, 'VERTICAL', NULL, 'HORIZONTAL', false, 20.0, 20.0, false, NULL, 'BACKGROUND'),

('FUND_STRUCTURE', 'Fund Structure View', 'Umbrella/subfund hierarchy',
 'UMBRELLA_FUNDS', 'DOWN',
 '["UMBRELLA_CONTAINS_SUBFUND", "FUND_HAS_SHARE_CLASS", "FEEDER_TO_MASTER"]'::JSONB,
 '["FUND_MANAGED_BY"]'::JSONB,
 'HIERARCHICAL', '{"level_separation": 100, "node_separation": 180}'::JSONB,
 'jurisdiction', 'VERTICAL', NULL, 'HORIZONTAL', false, 20.0, 20.0, true, 'jurisdiction', 'BACKGROUND'),

('BOOK', 'Book View', 'All CBUs under commercial client apex',
 'APEX_ENTITY', 'DOWN',
 '["OWNERSHIP"]'::JSONB,
 '[]'::JSONB,
 'HIERARCHICAL', '{"level_separation": 150, "node_separation": 200}'::JSONB,
 NULL, 'VERTICAL', NULL, 'HORIZONTAL', false, 20.0, 20.0, false, NULL, 'BACKGROUND'),

-- Legacy mappings for existing code
('SERVICE_DELIVERY', 'Service Delivery View', 'Alias for SERVICE view',
 'CBU', 'DOWN',
 '["CBU_USES_PRODUCT", "PRODUCT_PROVIDES_SERVICE", "SERVICE_USES_RESOURCE"]'::JSONB,
 '[]'::JSONB,
 'HIERARCHICAL', '{"level_separation": 80, "node_separation": 140}'::JSONB,
 NULL, 'VERTICAL', NULL, 'HORIZONTAL', false, 20.0, 20.0, false, NULL, 'BACKGROUND'),

('PRODUCTS_ONLY', 'Products Only View', 'Products with no service/resource expansion',
 'CBU', 'DOWN',
 '["CBU_USES_PRODUCT"]'::JSONB,
 '[]'::JSONB,
 'HIERARCHICAL', '{"level_separation": 80, "node_separation": 140}'::JSONB,
 NULL, 'VERTICAL', NULL, 'HORIZONTAL', false, 20.0, 20.0, false, NULL, 'BACKGROUND')

ON CONFLICT (view_mode_code) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    root_identification_rule = EXCLUDED.root_identification_rule,
    primary_traversal_direction = EXCLUDED.primary_traversal_direction,
    hierarchy_edge_types = EXCLUDED.hierarchy_edge_types,
    overlay_edge_types = EXCLUDED.overlay_edge_types,
    default_algorithm = EXCLUDED.default_algorithm,
    algorithm_params = EXCLUDED.algorithm_params;


-- ═══════════════════════════════════════════════════════════════════════════
-- 4. ADD VIEW FLAGS TO EXISTING role_categories
-- ═══════════════════════════════════════════════════════════════════════════

ALTER TABLE "ob-poc".role_categories
    ADD COLUMN IF NOT EXISTS show_in_ubo_view BOOLEAN DEFAULT true,
    ADD COLUMN IF NOT EXISTS show_in_trading_view BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS show_in_fund_structure_view BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS show_in_service_view BOOLEAN DEFAULT false;

UPDATE "ob-poc".role_categories SET
    show_in_ubo_view = CASE WHEN category_code IN
        ('OWNERSHIP_CHAIN', 'CONTROL_CHAIN', 'TRUST_ROLES', 'INVESTOR_CHAIN', 'RELATED_PARTY')
        THEN true ELSE false END,
    show_in_trading_view = CASE WHEN category_code IN
        ('CONTROL_CHAIN', 'FUND_MANAGEMENT', 'FUND_STRUCTURE', 'TRADING_EXECUTION', 'SERVICE_PROVIDER')
        THEN true ELSE false END,
    show_in_fund_structure_view = CASE WHEN category_code IN
        ('FUND_STRUCTURE', 'FUND_MANAGEMENT')
        THEN true ELSE false END,
    show_in_service_view = CASE WHEN category_code IN
        ('SERVICE_PROVIDER')
        THEN true ELSE false END;


-- ═══════════════════════════════════════════════════════════════════════════
-- 5. UNIFIED EDGES VIEW
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW "ob-poc".v_all_edges AS

-- Entity ownership/control/trust relationships
SELECT
    er.relationship_id AS edge_id,
    UPPER(er.relationship_type) AS edge_type_code,
    er.from_entity_id AS from_node_id,
    'ENTITY' AS from_node_type,
    er.to_entity_id AS to_node_id,
    'ENTITY' AS to_node_type,
    er.percentage,
    NULL::VARCHAR AS role_name,
    er.effective_from,
    er.effective_to,
    er.created_at
FROM "ob-poc".entity_relationships er
WHERE er.effective_to IS NULL OR er.effective_to > CURRENT_DATE

UNION ALL

-- CBU ↔ Entity role assignments
SELECT
    cer.cbu_entity_role_id AS edge_id,
    'CBU_ROLE' AS edge_type_code,
    cer.cbu_id AS from_node_id,
    'CBU' AS from_node_type,
    cer.entity_id AS to_node_id,
    'ENTITY' AS to_node_type,
    cer.ownership_percentage AS percentage,
    r.name AS role_name,
    cer.effective_from,
    cer.effective_to,
    cer.created_at
FROM "ob-poc".cbu_entity_roles cer
JOIN "ob-poc".roles r ON cer.role_id = r.role_id
WHERE cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE

UNION ALL

-- CBU ↔ Product linkage
SELECT
    cp.cbu_product_id AS edge_id,
    'CBU_USES_PRODUCT' AS edge_type_code,
    cp.cbu_id AS from_node_id,
    'CBU' AS from_node_type,
    cp.product_id AS to_node_id,
    'PRODUCT' AS to_node_type,
    NULL AS percentage,
    NULL AS role_name,
    NULL AS effective_from,
    NULL AS effective_to,
    cp.created_at
FROM "ob-poc".cbu_products cp

UNION ALL

-- Product ↔ Service linkage
SELECT
    ps.product_service_id AS edge_id,
    'PRODUCT_PROVIDES_SERVICE' AS edge_type_code,
    ps.product_id AS from_node_id,
    'PRODUCT' AS from_node_type,
    ps.service_id AS to_node_id,
    'SERVICE' AS to_node_type,
    NULL AS percentage,
    NULL AS role_name,
    NULL AS effective_from,
    NULL AS effective_to,
    ps.created_at
FROM "ob-poc".product_services ps

UNION ALL

-- Service ↔ Resource linkage
SELECT
    src.service_resource_id AS edge_id,
    'SERVICE_USES_RESOURCE' AS edge_type_code,
    src.service_id AS from_node_id,
    'SERVICE' AS from_node_type,
    src.resource_type_id AS to_node_id,
    'RESOURCE' AS to_node_type,
    NULL AS percentage,
    NULL AS role_name,
    NULL AS effective_from,
    NULL AS effective_to,
    src.created_at
FROM "ob-poc".service_resource_capabilities src

UNION ALL

-- Trading profile linkage (if exists)
SELECT
    tp.profile_id AS edge_id,
    'CBU_HAS_TRADING_PROFILE' AS edge_type_code,
    tp.cbu_id AS from_node_id,
    'CBU' AS from_node_type,
    tp.profile_id AS to_node_id,
    'TRADING_PROFILE' AS to_node_type,
    NULL AS percentage,
    NULL AS role_name,
    NULL AS effective_from,
    NULL AS effective_to,
    tp.created_at
FROM "ob-poc".cbu_trading_profiles tp
WHERE tp.status = 'ACTIVE'

UNION ALL

-- Fund structure relationships
SELECT
    fs.structure_id AS edge_id,
    CASE fs.relationship_type
        WHEN 'CONTAINS' THEN 'UMBRELLA_CONTAINS_SUBFUND'
        WHEN 'MASTER_FEEDER' THEN 'FEEDER_TO_MASTER'
        ELSE fs.relationship_type
    END AS edge_type_code,
    fs.parent_entity_id AS from_node_id,
    'ENTITY_FUND' AS from_node_type,
    fs.child_entity_id AS to_node_id,
    'ENTITY_FUND' AS to_node_type,
    NULL AS percentage,
    NULL AS role_name,
    fs.effective_from,
    fs.effective_to,
    fs.created_at
FROM "ob-poc".fund_structure fs
WHERE fs.effective_to IS NULL OR fs.effective_to > CURRENT_DATE

UNION ALL

-- Share class relationships
SELECT
    esc.entity_id AS edge_id,
    'FUND_HAS_SHARE_CLASS' AS edge_type_code,
    esc.parent_fund_id AS from_node_id,
    'ENTITY_FUND' AS from_node_type,
    esc.entity_id AS to_node_id,
    'SHARE_CLASS' AS to_node_type,
    NULL AS percentage,
    NULL AS role_name,
    NULL AS effective_from,
    NULL AS effective_to,
    esc.created_at
FROM "ob-poc".entity_share_classes esc
WHERE esc.parent_fund_id IS NOT NULL;

COMMENT ON VIEW "ob-poc".v_all_edges IS
'Unified view of all edges across different source tables. Query this with edge_type filter for view-specific graphs.';


-- ═══════════════════════════════════════════════════════════════════════════
-- 6. CREATE INDEXES
-- ═══════════════════════════════════════════════════════════════════════════

CREATE INDEX IF NOT EXISTS idx_node_types_ubo ON "ob-poc".node_types(show_in_ubo_view) WHERE show_in_ubo_view = true;
CREATE INDEX IF NOT EXISTS idx_node_types_trading ON "ob-poc".node_types(show_in_trading_view) WHERE show_in_trading_view = true;
CREATE INDEX IF NOT EXISTS idx_node_types_service ON "ob-poc".node_types(show_in_service_view) WHERE show_in_service_view = true;

CREATE INDEX IF NOT EXISTS idx_edge_types_ubo ON "ob-poc".edge_types(show_in_ubo_view) WHERE show_in_ubo_view = true;
CREATE INDEX IF NOT EXISTS idx_edge_types_trading ON "ob-poc".edge_types(show_in_trading_view) WHERE show_in_trading_view = true;
CREATE INDEX IF NOT EXISTS idx_edge_types_service ON "ob-poc".edge_types(show_in_service_view) WHERE show_in_service_view = true;


-- ═══════════════════════════════════════════════════════════════════════════
-- COMMENTS
-- ═══════════════════════════════════════════════════════════════════════════

COMMENT ON COLUMN "ob-poc".node_types.show_in_ubo_view IS
'If true, nodes of this type appear in UBO/KYC views. Replaces hardcoded is_ubo_relevant().';

COMMENT ON COLUMN "ob-poc".node_types.show_in_trading_view IS
'If true, nodes of this type appear in Trading views. Replaces hardcoded is_trading_relevant().';

COMMENT ON COLUMN "ob-poc".edge_types.is_hierarchical IS
'If true, this edge type contributes to tier computation in hierarchical layout.';

COMMENT ON COLUMN "ob-poc".edge_types.tier_delta IS
'How many tiers down (positive) or up (negative) the target is from source. Used by layout engine.';

COMMENT ON COLUMN "ob-poc".edge_types.bundle_group IS
'Edges in same bundle group are routed together to reduce visual clutter.';

COMMENT ON COLUMN "ob-poc".view_modes.root_identification_rule IS
'How to identify root nodes: CBU (CBU node), TERMINUS_ENTITIES (natural persons), APEX_ENTITY (top of chain), UMBRELLA_FUNDS (umbrella funds).';

COMMENT ON COLUMN "ob-poc".view_modes.hierarchy_edge_types IS
'Edge types that define the primary hierarchy for layout. Array of edge_type_codes.';

COMMENT ON COLUMN "ob-poc".view_modes.overlay_edge_types IS
'Edge types that overlay on the hierarchy (control, trustee). Not used for tier computation.';
