-- Migration: Trading View Configuration
-- Adds missing node types and edge types for the Trading View
-- These enable visualization of: Instrument Classes, Markets, ISDA/CSA agreements

-- =============================================================================
-- TRADING VIEW NODE TYPES
-- =============================================================================

INSERT INTO "ob-poc".node_types (
    node_type_code, display_name, description,
    show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
    show_in_service_view, show_in_product_view,
    icon, default_color, default_shape,
    default_width, default_height, can_be_container, default_tier,
    is_trading, sort_order
) VALUES
-- Instrument class (EQUITY, FIXED_INCOME, DERIVATIVES, etc.)
('INSTRUMENT_CLASS', 'Instrument Class', 'Asset class category (Equity, Fixed Income, Derivatives)',
 false, true, false, false, false,
 'layers', '#7C3AED', 'RECTANGLE',
 160.0, 50.0, true, 3,
 true, 220),

-- Market / Exchange (via MIC code)
('MARKET', 'Market', 'Exchange or trading venue (MIC)',
 false, true, false, false, false,
 'landmark', '#059669', 'RECTANGLE',
 140.0, 50.0, false, 4,
 true, 230),

-- ISDA Master Agreement
('ISDA_AGREEMENT', 'ISDA Agreement', 'OTC Master Agreement',
 false, true, false, false, false,
 'file-signature', '#DC2626', 'RECTANGLE',
 160.0, 50.0, true, 4,
 true, 240),

-- Credit Support Annex
('CSA_AGREEMENT', 'CSA', 'Credit Support Annex (collateral)',
 false, true, false, false, false,
 'shield-check', '#EA580C', 'RECTANGLE',
 140.0, 40.0, false, 5,
 true, 250)

ON CONFLICT (node_type_code) DO UPDATE SET
    show_in_trading_view = EXCLUDED.show_in_trading_view,
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    is_trading = EXCLUDED.is_trading;

-- =============================================================================
-- TRADING VIEW EDGE TYPES
-- =============================================================================

INSERT INTO "ob-poc".edge_types (
    edge_type_code, display_name, description,
    from_node_types, to_node_types,
    show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
    show_in_service_view, show_in_product_view,
    edge_style, edge_color, edge_width, arrow_style,
    layout_direction, tier_delta, is_hierarchical,
    is_trading, sort_order
) VALUES
-- Matrix -> Instrument Class
('MATRIX_INCLUDES_CLASS', 'Includes Class', 'Matrix includes instrument class',
 '["INSTRUMENT_MATRIX"]'::JSONB, '["INSTRUMENT_CLASS"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#7C3AED', 1.5, 'SINGLE',
 'DOWN', 1, true,
 true, 202),

-- Instrument Class -> Market (exchange traded)
('CLASS_TRADED_ON_MARKET', 'Traded On', 'Instrument class traded on market',
 '["INSTRUMENT_CLASS"]'::JSONB, '["MARKET"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#059669', 1.0, 'SINGLE',
 'DOWN', 1, true,
 true, 203),

-- Instrument Class -> Counterparty Entity (OTC)
('OTC_WITH_COUNTERPARTY', 'OTC With', 'OTC trading relationship',
 '["INSTRUMENT_CLASS"]'::JSONB, '["ENTITY_COMPANY"]'::JSONB,
 false, true, false, false, false,
 'DASHED', '#DC2626', 1.5, 'SINGLE',
 'DOWN', 1, true,
 true, 204),

-- Counterparty Entity -> ISDA
('OTC_COVERED_BY_ISDA', 'Covered By ISDA', 'OTC trades governed by ISDA',
 '["ENTITY_COMPANY"]'::JSONB, '["ISDA_AGREEMENT"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#DC2626', 1.5, 'SINGLE',
 'RIGHT', 0, false,
 true, 205),

-- ISDA -> CSA
('ISDA_HAS_CSA', 'Has CSA', 'ISDA includes collateral annex',
 '["ISDA_AGREEMENT"]'::JSONB, '["CSA_AGREEMENT"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#EA580C', 1.0, 'SINGLE',
 'DOWN', 1, true,
 true, 206),

-- CBU -> Investment Manager Entity (trading mandate)
('CBU_IM_MANDATE', 'IM Mandate', 'Investment manager trading mandate',
 '["CBU"]'::JSONB, '["ENTITY_COMPANY"]'::JSONB,
 false, true, false, false, false,
 'DASHED', '#8B5CF6', 2.0, 'DOUBLE',
 'RIGHT', 0, false,
 true, 210)

ON CONFLICT (edge_type_code) DO UPDATE SET
    show_in_trading_view = EXCLUDED.show_in_trading_view,
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    is_trading = EXCLUDED.is_trading;

-- =============================================================================
-- UNDEFINED EDGE TYPE (for GLEIF imports with unmapped relationship types)
-- =============================================================================

INSERT INTO "ob-poc".edge_types (
    edge_type_code, display_name, description,
    from_node_types, to_node_types,
    show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
    show_in_service_view, show_in_product_view,
    edge_style, edge_color, edge_width, arrow_style,
    layout_direction, tier_delta, is_hierarchical,
    is_trading, sort_order
) VALUES
-- Undefined/unmapped relationship - shown in all views to ensure visibility
('UNDEFINED', 'Related To', 'Unmapped relationship from external source (e.g., GLEIF)',
 '[]'::JSONB, '[]'::JSONB,
 true, true, true, true, true,
 'DOTTED', '#9CA3AF', 1.0, 'SINGLE',
 'DOWN', 0, false,
 false, 999)
ON CONFLICT (edge_type_code) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    show_in_ubo_view = EXCLUDED.show_in_ubo_view,
    show_in_trading_view = EXCLUDED.show_in_trading_view;

-- =============================================================================
-- UPDATE VIEW MODE: Update TRADING view mode with correct edge types
-- =============================================================================

UPDATE "ob-poc".view_modes SET
    display_name = 'Trading',
    description = 'Trading authorization matrix, markets, ISDA/CSA agreements',
    hierarchy_edge_types = '["CBU_HAS_TRADING_PROFILE", "TRADING_PROFILE_HAS_MATRIX", "MATRIX_INCLUDES_CLASS", "CLASS_TRADED_ON_MARKET", "OTC_COVERED_BY_ISDA", "ISDA_HAS_CSA"]'::JSONB,
    overlay_edge_types = '["OTC_WITH_COUNTERPARTY", "CBU_IM_MANDATE", "ENTITY_AUTHORIZES_TRADING"]'::JSONB
WHERE view_mode_code = 'TRADING';
