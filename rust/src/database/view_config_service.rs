//! View Configuration Service
//!
//! Provides access to config-driven visualization settings from database tables:
//! - node_types: Node type definitions with view applicability and rendering hints
//! - edge_types: Edge type definitions with layout hints
//! - view_modes: Per-view configuration with root identification and hierarchy rules
//! - layout_config: Global layout configuration settings
//! - layout_overrides: User-specified node position/size overrides
//! - layout_cache: Cached layout computations
//!
//! This replaces hardcoded Rust logic (is_ubo_relevant, is_trading_relevant) with
//! database configuration. "Config, not code."

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// =============================================================================
// NODE TYPE CONFIGURATION
// =============================================================================

/// Node type configuration from node_types table
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct NodeTypeConfig {
    pub node_type_code: String,
    pub display_name: String,
    pub description: Option<String>,

    // View applicability
    pub show_in_ubo_view: bool,
    pub show_in_trading_view: bool,
    pub show_in_fund_structure_view: bool,
    pub show_in_service_view: bool,
    pub show_in_product_view: bool,

    // Rendering hints
    pub icon: Option<String>,
    pub default_color: Option<String>,
    pub default_shape: Option<String>,

    // Layout hints
    pub default_width: Option<bigdecimal::BigDecimal>,
    pub default_height: Option<bigdecimal::BigDecimal>,
    pub can_be_container: bool,
    pub default_tier: Option<i32>,
    pub importance_weight: Option<bigdecimal::BigDecimal>,

    // Container layout
    pub child_layout_mode: Option<String>,
    pub container_padding: Option<bigdecimal::BigDecimal>,

    // Semantic zoom
    pub collapse_below_zoom: Option<bigdecimal::BigDecimal>,
    pub hide_label_below_zoom: Option<bigdecimal::BigDecimal>,
    pub show_detail_above_zoom: Option<bigdecimal::BigDecimal>,

    // Fan-out control
    pub max_visible_children: Option<i32>,
    pub overflow_behavior: Option<String>,

    // Deduplication
    pub dedupe_mode: Option<String>,
    pub min_separation: Option<bigdecimal::BigDecimal>,

    // Rendering order
    pub z_order: Option<i32>,

    // Semantic flags
    pub is_kyc_subject: bool,
    pub is_structural: bool,
    pub is_operational: bool,
    pub is_trading: bool,

    pub sort_order: Option<i32>,
}

// =============================================================================
// EDGE TYPE CONFIGURATION
// =============================================================================

/// Edge type configuration from edge_types table
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct EdgeTypeConfig {
    pub edge_type_code: String,
    pub display_name: String,
    pub description: Option<String>,

    // Endpoint constraints (JSONB arrays)
    pub from_node_types: serde_json::Value,
    pub to_node_types: serde_json::Value,

    // View applicability
    pub show_in_ubo_view: bool,
    pub show_in_trading_view: bool,
    pub show_in_fund_structure_view: bool,
    pub show_in_service_view: bool,
    pub show_in_product_view: bool,

    // Rendering hints
    pub edge_style: Option<String>,
    pub edge_color: Option<String>,
    pub edge_width: Option<bigdecimal::BigDecimal>,
    pub arrow_style: Option<String>,
    pub shows_percentage: bool,
    pub shows_label: bool,
    pub label_template: Option<String>,
    pub label_position: Option<String>,

    // Layout hints
    pub layout_direction: Option<String>,
    pub tier_delta: Option<i32>,
    pub is_hierarchical: bool,
    pub bundle_group: Option<String>,
    pub routing_priority: Option<i32>,
    pub spring_strength: Option<bigdecimal::BigDecimal>,
    pub ideal_length: Option<bigdecimal::BigDecimal>,

    // Sibling ordering
    pub sibling_sort_key: Option<String>,

    // Anchor points
    pub source_anchor: Option<String>,
    pub target_anchor: Option<String>,

    // Cycle handling
    pub cycle_break_priority: Option<i32>,

    // Multi-parent
    pub is_primary_parent_rule: Option<String>,

    // Parallel edges
    pub parallel_edge_offset: Option<bigdecimal::BigDecimal>,

    // Self-loops
    pub self_loop_radius: Option<bigdecimal::BigDecimal>,
    pub self_loop_position: Option<String>,

    // Rendering order
    pub z_order: Option<i32>,

    // Semantic flags
    pub is_ownership: bool,
    pub is_control: bool,
    pub is_structural: bool,
    pub is_service_delivery: bool,
    pub is_trading: bool,
    pub creates_kyc_obligation: bool,

    // Cardinality
    pub cardinality: Option<String>,

    pub sort_order: Option<i32>,
}

// =============================================================================
// VIEW MODE CONFIGURATION
// =============================================================================

/// View mode configuration from view_modes table
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ViewModeConfig {
    pub view_mode_code: String,
    pub display_name: String,
    pub description: Option<String>,

    // Root identification
    pub root_identification_rule: String,

    // Traversal
    pub primary_traversal_direction: Option<String>,

    // Edge classification (JSONB arrays)
    pub hierarchy_edge_types: serde_json::Value,
    pub overlay_edge_types: serde_json::Value,

    // Layout algorithm
    pub default_algorithm: Option<String>,
    pub algorithm_params: serde_json::Value,

    // Swim lanes
    pub swim_lane_attribute: Option<String>,
    pub swim_lane_direction: Option<String>,

    // Temporal
    pub temporal_axis: Option<String>,
    pub temporal_axis_direction: Option<String>,

    // Grid snapping
    pub snap_to_grid: bool,
    pub grid_size_x: Option<bigdecimal::BigDecimal>,
    pub grid_size_y: Option<bigdecimal::BigDecimal>,

    // Clustering
    pub auto_cluster: bool,
    pub cluster_attribute: Option<String>,
    pub cluster_visual_style: Option<String>,
}

// =============================================================================
// LAYOUT CONFIGURATION
// =============================================================================

/// Layout configuration entry from layout_config table
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct LayoutConfigEntry {
    pub config_key: String,
    pub config_value: serde_json::Value,
    pub description: Option<String>,
}

// =============================================================================
// LAYOUT OVERRIDE
// =============================================================================

/// Layout override for a single node
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct NodeLayoutOverride {
    pub override_id: Uuid,
    pub cbu_id: Uuid,
    pub view_mode: String,
    pub user_id: Option<Uuid>,
    pub node_id: Uuid,
    pub node_type: String,
    pub x_offset: Option<bigdecimal::BigDecimal>,
    pub y_offset: Option<bigdecimal::BigDecimal>,
    pub width_override: Option<bigdecimal::BigDecimal>,
    pub height_override: Option<bigdecimal::BigDecimal>,
    pub pinned: bool,
    pub collapsed: bool,
    pub hidden: bool,
    pub expansion_level: Option<i32>,
}

// =============================================================================
// LAYOUT CACHE
// =============================================================================

/// Cached layout entry
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct LayoutCacheEntry {
    pub cache_id: Uuid,
    pub cbu_id: Uuid,
    pub view_mode: String,
    pub user_id: Option<Uuid>,
    pub input_hash: String,
    pub algorithm_version: Option<String>,
    pub node_positions: serde_json::Value,
    pub edge_paths: serde_json::Value,
    pub bounding_box: Option<serde_json::Value>,
    pub tier_info: Option<serde_json::Value>,
    pub computation_time_ms: Option<i32>,
    pub node_count: Option<i32>,
    pub edge_count: Option<i32>,
    pub computed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
}

// =============================================================================
// VIEW CONFIG SERVICE
// =============================================================================

/// Service for loading view configuration from database
///
/// Replaces hardcoded visibility/layout logic with config-driven approach.
pub struct ViewConfigService;

impl ViewConfigService {
    // =========================================================================
    // NODE TYPE QUERIES
    // =========================================================================

    /// Get all node types
    pub async fn get_all_node_types(pool: &PgPool) -> Result<Vec<NodeTypeConfig>> {
        let node_types = sqlx::query_as::<_, NodeTypeConfig>(
            r#"SELECT
                node_type_code, display_name, description,
                show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
                show_in_service_view, show_in_product_view,
                icon, default_color, default_shape,
                default_width, default_height, can_be_container, default_tier, importance_weight,
                child_layout_mode, container_padding,
                collapse_below_zoom, hide_label_below_zoom, show_detail_above_zoom,
                max_visible_children, overflow_behavior,
                dedupe_mode, min_separation,
                z_order,
                is_kyc_subject, is_structural, is_operational, is_trading,
                sort_order
               FROM "ob-poc".node_types
               ORDER BY sort_order, node_type_code"#,
        )
        .fetch_all(pool)
        .await?;
        Ok(node_types)
    }

    /// Get node types applicable to a specific view mode
    pub async fn get_view_node_types(
        pool: &PgPool,
        view_mode: &str,
    ) -> Result<Vec<NodeTypeConfig>> {
        // Build WHERE clause based on view mode
        let column = match view_mode {
            "UBO" | "KYC_UBO" => "show_in_ubo_view",
            "TRADING" => "show_in_trading_view",
            "FUND_STRUCTURE" => "show_in_fund_structure_view",
            "SERVICE" | "SERVICE_DELIVERY" => "show_in_service_view",
            "PRODUCT" | "PRODUCTS_ONLY" => "show_in_product_view",
            // For unknown views, return all node types
            _ => return Self::get_all_node_types(pool).await,
        };

        let query = format!(
            r#"SELECT
                node_type_code, display_name, description,
                show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
                show_in_service_view, show_in_product_view,
                icon, default_color, default_shape,
                default_width, default_height, can_be_container, default_tier, importance_weight,
                child_layout_mode, container_padding,
                collapse_below_zoom, hide_label_below_zoom, show_detail_above_zoom,
                max_visible_children, overflow_behavior,
                dedupe_mode, min_separation,
                z_order,
                is_kyc_subject, is_structural, is_operational, is_trading,
                sort_order
               FROM "ob-poc".node_types
               WHERE {} = true
               ORDER BY sort_order, node_type_code"#,
            column
        );

        let node_types = sqlx::query_as::<_, NodeTypeConfig>(&query)
            .fetch_all(pool)
            .await?;
        Ok(node_types)
    }

    /// Get a single node type by code
    pub async fn get_node_type(pool: &PgPool, code: &str) -> Result<Option<NodeTypeConfig>> {
        let node_type = sqlx::query_as::<_, NodeTypeConfig>(
            r#"SELECT
                node_type_code, display_name, description,
                show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
                show_in_service_view, show_in_product_view,
                icon, default_color, default_shape,
                default_width, default_height, can_be_container, default_tier, importance_weight,
                child_layout_mode, container_padding,
                collapse_below_zoom, hide_label_below_zoom, show_detail_above_zoom,
                max_visible_children, overflow_behavior,
                dedupe_mode, min_separation,
                z_order,
                is_kyc_subject, is_structural, is_operational, is_trading,
                sort_order
               FROM "ob-poc".node_types
               WHERE node_type_code = $1"#,
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;
        Ok(node_type)
    }

    // =========================================================================
    // EDGE TYPE QUERIES
    // =========================================================================

    /// Get all edge types
    pub async fn get_all_edge_types(pool: &PgPool) -> Result<Vec<EdgeTypeConfig>> {
        let edge_types = sqlx::query_as::<_, EdgeTypeConfig>(
            r#"SELECT
                edge_type_code, display_name, description,
                from_node_types, to_node_types,
                show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
                show_in_service_view, show_in_product_view,
                edge_style, edge_color, edge_width, arrow_style,
                shows_percentage, shows_label, label_template, label_position,
                layout_direction, tier_delta, is_hierarchical, bundle_group, routing_priority,
                spring_strength, ideal_length,
                sibling_sort_key,
                source_anchor, target_anchor,
                cycle_break_priority,
                is_primary_parent_rule,
                parallel_edge_offset,
                self_loop_radius, self_loop_position,
                z_order,
                is_ownership, is_control, is_structural, is_service_delivery, is_trading,
                creates_kyc_obligation,
                cardinality,
                sort_order
               FROM "ob-poc".edge_types
               ORDER BY sort_order, edge_type_code"#,
        )
        .fetch_all(pool)
        .await?;
        Ok(edge_types)
    }

    /// Get edge types applicable to a specific view mode
    pub async fn get_view_edge_types(
        pool: &PgPool,
        view_mode: &str,
    ) -> Result<Vec<EdgeTypeConfig>> {
        // Build WHERE clause based on view mode
        let column = match view_mode {
            "UBO" | "KYC_UBO" => "show_in_ubo_view",
            "TRADING" => "show_in_trading_view",
            "FUND_STRUCTURE" => "show_in_fund_structure_view",
            "SERVICE" | "SERVICE_DELIVERY" => "show_in_service_view",
            "PRODUCT" | "PRODUCTS_ONLY" => "show_in_product_view",
            // For unknown views, return all edge types
            _ => return Self::get_all_edge_types(pool).await,
        };

        let query = format!(
            r#"SELECT
                edge_type_code, display_name, description,
                from_node_types, to_node_types,
                show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
                show_in_service_view, show_in_product_view,
                edge_style, edge_color, edge_width, arrow_style,
                shows_percentage, shows_label, label_template, label_position,
                layout_direction, tier_delta, is_hierarchical, bundle_group, routing_priority,
                spring_strength, ideal_length,
                sibling_sort_key,
                source_anchor, target_anchor,
                cycle_break_priority,
                is_primary_parent_rule,
                parallel_edge_offset,
                self_loop_radius, self_loop_position,
                z_order,
                is_ownership, is_control, is_structural, is_service_delivery, is_trading,
                creates_kyc_obligation,
                cardinality,
                sort_order
               FROM "ob-poc".edge_types
               WHERE {} = true
               ORDER BY sort_order, edge_type_code"#,
            column
        );

        let edge_types = sqlx::query_as::<_, EdgeTypeConfig>(&query)
            .fetch_all(pool)
            .await?;
        Ok(edge_types)
    }

    /// Get a single edge type by code
    pub async fn get_edge_type(pool: &PgPool, code: &str) -> Result<Option<EdgeTypeConfig>> {
        let edge_type = sqlx::query_as::<_, EdgeTypeConfig>(
            r#"SELECT
                edge_type_code, display_name, description,
                from_node_types, to_node_types,
                show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
                show_in_service_view, show_in_product_view,
                edge_style, edge_color, edge_width, arrow_style,
                shows_percentage, shows_label, label_template, label_position,
                layout_direction, tier_delta, is_hierarchical, bundle_group, routing_priority,
                spring_strength, ideal_length,
                sibling_sort_key,
                source_anchor, target_anchor,
                cycle_break_priority,
                is_primary_parent_rule,
                parallel_edge_offset,
                self_loop_radius, self_loop_position,
                z_order,
                is_ownership, is_control, is_structural, is_service_delivery, is_trading,
                creates_kyc_obligation,
                cardinality,
                sort_order
               FROM "ob-poc".edge_types
               WHERE edge_type_code = $1"#,
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;
        Ok(edge_type)
    }

    /// Get hierarchical edge types for a view mode (from view_modes.hierarchy_edge_types)
    pub async fn get_hierarchy_edge_types(pool: &PgPool, view_mode: &str) -> Result<Vec<String>> {
        let result = sqlx::query_scalar::<_, serde_json::Value>(
            r#"SELECT hierarchy_edge_types FROM "ob-poc".view_modes WHERE view_mode_code = $1"#,
        )
        .bind(view_mode)
        .fetch_optional(pool)
        .await?;

        match result {
            Some(json) => {
                let types: Vec<String> = serde_json::from_value(json)?;
                Ok(types)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Get overlay edge types for a view mode (from view_modes.overlay_edge_types)
    pub async fn get_overlay_edge_types(pool: &PgPool, view_mode: &str) -> Result<Vec<String>> {
        let result = sqlx::query_scalar::<_, serde_json::Value>(
            r#"SELECT overlay_edge_types FROM "ob-poc".view_modes WHERE view_mode_code = $1"#,
        )
        .bind(view_mode)
        .fetch_optional(pool)
        .await?;

        match result {
            Some(json) => {
                let types: Vec<String> = serde_json::from_value(json)?;
                Ok(types)
            }
            None => Ok(Vec::new()),
        }
    }

    // =========================================================================
    // VIEW MODE QUERIES
    // =========================================================================

    /// Get all view modes
    pub async fn get_all_view_modes(pool: &PgPool) -> Result<Vec<ViewModeConfig>> {
        let view_modes = sqlx::query_as::<_, ViewModeConfig>(
            r#"SELECT
                view_mode_code, display_name, description,
                root_identification_rule,
                primary_traversal_direction,
                hierarchy_edge_types, overlay_edge_types,
                default_algorithm, algorithm_params,
                swim_lane_attribute, swim_lane_direction,
                temporal_axis, temporal_axis_direction,
                snap_to_grid, grid_size_x, grid_size_y,
                auto_cluster, cluster_attribute, cluster_visual_style
               FROM "ob-poc".view_modes
               ORDER BY view_mode_code"#,
        )
        .fetch_all(pool)
        .await?;
        Ok(view_modes)
    }

    /// Get a single view mode configuration
    pub async fn get_view_mode_config(
        pool: &PgPool,
        view_mode: &str,
    ) -> Result<Option<ViewModeConfig>> {
        let view_mode_config = sqlx::query_as::<_, ViewModeConfig>(
            r#"SELECT
                view_mode_code, display_name, description,
                root_identification_rule,
                primary_traversal_direction,
                hierarchy_edge_types, overlay_edge_types,
                default_algorithm, algorithm_params,
                swim_lane_attribute, swim_lane_direction,
                temporal_axis, temporal_axis_direction,
                snap_to_grid, grid_size_x, grid_size_y,
                auto_cluster, cluster_attribute, cluster_visual_style
               FROM "ob-poc".view_modes
               WHERE view_mode_code = $1"#,
        )
        .bind(view_mode)
        .fetch_optional(pool)
        .await?;
        Ok(view_mode_config)
    }

    // =========================================================================
    // LAYOUT CONFIG QUERIES
    // =========================================================================

    /// Get all layout configuration entries
    pub async fn get_all_layout_config(pool: &PgPool) -> Result<Vec<LayoutConfigEntry>> {
        let configs = sqlx::query_as::<_, LayoutConfigEntry>(
            r#"SELECT config_key, config_value, description
               FROM "ob-poc".layout_config
               ORDER BY config_key"#,
        )
        .fetch_all(pool)
        .await?;
        Ok(configs)
    }

    /// Get a single layout configuration value by key
    pub async fn get_layout_config(pool: &PgPool, key: &str) -> Result<Option<serde_json::Value>> {
        let result = sqlx::query_scalar::<_, serde_json::Value>(
            r#"SELECT config_value FROM "ob-poc".layout_config WHERE config_key = $1"#,
        )
        .bind(key)
        .fetch_optional(pool)
        .await?;
        Ok(result)
    }

    /// Get layout configuration with typed extraction
    pub async fn get_layout_config_typed<T: serde::de::DeserializeOwned>(
        pool: &PgPool,
        key: &str,
    ) -> Result<Option<T>> {
        let result = Self::get_layout_config(pool, key).await?;
        match result {
            Some(json) => {
                let typed: T = serde_json::from_value(json)?;
                Ok(Some(typed))
            }
            None => Ok(None),
        }
    }

    // =========================================================================
    // LAYOUT OVERRIDE QUERIES
    // =========================================================================

    /// Get layout overrides for a CBU and view mode
    pub async fn get_layout_overrides(
        pool: &PgPool,
        cbu_id: Uuid,
        view_mode: &str,
        user_id: Option<Uuid>,
    ) -> Result<Vec<NodeLayoutOverride>> {
        let overrides = sqlx::query_as::<_, NodeLayoutOverride>(
            r#"SELECT
                override_id, cbu_id, view_mode, user_id,
                node_id, node_type,
                x_offset, y_offset,
                width_override, height_override,
                pinned, collapsed, hidden,
                expansion_level
               FROM "ob-poc".layout_overrides
               WHERE cbu_id = $1
                 AND view_mode = $2
                 AND (user_id = $3 OR ($3 IS NULL AND user_id IS NULL))"#,
        )
        .bind(cbu_id)
        .bind(view_mode)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(overrides)
    }

    /// Save or update a layout override for a node
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_layout_override(
        pool: &PgPool,
        cbu_id: Uuid,
        view_mode: &str,
        user_id: Option<Uuid>,
        node_id: Uuid,
        node_type: &str,
        x_offset: f64,
        y_offset: f64,
        width_override: Option<f64>,
        height_override: Option<f64>,
        pinned: bool,
        collapsed: bool,
        hidden: bool,
    ) -> Result<Uuid> {
        let override_id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO "ob-poc".layout_overrides
               (cbu_id, view_mode, user_id, node_id, node_type,
                x_offset, y_offset, width_override, height_override,
                pinned, collapsed, hidden, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now())
               ON CONFLICT (cbu_id, view_mode, node_id, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::UUID))
               DO UPDATE SET
                   x_offset = EXCLUDED.x_offset,
                   y_offset = EXCLUDED.y_offset,
                   width_override = EXCLUDED.width_override,
                   height_override = EXCLUDED.height_override,
                   pinned = EXCLUDED.pinned,
                   collapsed = EXCLUDED.collapsed,
                   hidden = EXCLUDED.hidden,
                   updated_at = now()
               RETURNING override_id"#,
        )
        .bind(cbu_id)
        .bind(view_mode)
        .bind(user_id)
        .bind(node_id)
        .bind(node_type)
        .bind(x_offset)
        .bind(y_offset)
        .bind(width_override)
        .bind(height_override)
        .bind(pinned)
        .bind(collapsed)
        .bind(hidden)
        .fetch_one(pool)
        .await?;
        Ok(override_id)
    }

    /// Delete a layout override
    pub async fn delete_layout_override(pool: &PgPool, override_id: Uuid) -> Result<bool> {
        let result = sqlx::query(r#"DELETE FROM "ob-poc".layout_overrides WHERE override_id = $1"#)
            .bind(override_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Reset all layout overrides for a CBU/view
    pub async fn reset_layout_overrides(
        pool: &PgPool,
        cbu_id: Uuid,
        view_mode: Option<&str>,
        user_id: Option<Uuid>,
    ) -> Result<i64> {
        let count =
            sqlx::query_scalar::<_, i64>(r#"SELECT "ob-poc".reset_layout_overrides($1, $2, $3)"#)
                .bind(cbu_id)
                .bind(view_mode)
                .bind(user_id)
                .fetch_one(pool)
                .await?;
        Ok(count)
    }

    // =========================================================================
    // LAYOUT CACHE QUERIES
    // =========================================================================

    /// Get cached layout for a CBU and view mode
    pub async fn get_layout_cache(
        pool: &PgPool,
        cbu_id: Uuid,
        view_mode: &str,
        user_id: Option<Uuid>,
    ) -> Result<Option<LayoutCacheEntry>> {
        let cache = sqlx::query_as::<_, LayoutCacheEntry>(
            r#"SELECT
                cache_id, cbu_id, view_mode, user_id,
                input_hash, algorithm_version,
                node_positions, edge_paths,
                bounding_box, tier_info,
                computation_time_ms, node_count, edge_count,
                computed_at, valid_until
               FROM "ob-poc".layout_cache
               WHERE cbu_id = $1
                 AND view_mode = $2
                 AND (user_id = $3 OR ($3 IS NULL AND user_id IS NULL))
                 AND (valid_until IS NULL OR valid_until > now())"#,
        )
        .bind(cbu_id)
        .bind(view_mode)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(cache)
    }

    /// Check if cached layout is still valid by comparing input hash
    pub async fn is_cache_valid(
        pool: &PgPool,
        cbu_id: Uuid,
        view_mode: &str,
        user_id: Option<Uuid>,
        expected_hash: &str,
    ) -> Result<bool> {
        let result = sqlx::query_scalar::<_, String>(
            r#"SELECT input_hash FROM "ob-poc".layout_cache
               WHERE cbu_id = $1
                 AND view_mode = $2
                 AND (user_id = $3 OR ($3 IS NULL AND user_id IS NULL))
                 AND (valid_until IS NULL OR valid_until > now())"#,
        )
        .bind(cbu_id)
        .bind(view_mode)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(result.as_deref() == Some(expected_hash))
    }

    /// Save layout cache
    #[allow(clippy::too_many_arguments)]
    pub async fn save_layout_cache(
        pool: &PgPool,
        cbu_id: Uuid,
        view_mode: &str,
        user_id: Option<Uuid>,
        input_hash: &str,
        node_positions: serde_json::Value,
        edge_paths: serde_json::Value,
        bounding_box: Option<serde_json::Value>,
        tier_info: Option<serde_json::Value>,
        computation_time_ms: i32,
        node_count: i32,
        edge_count: i32,
    ) -> Result<Uuid> {
        let cache_id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO "ob-poc".layout_cache
               (cbu_id, view_mode, user_id, input_hash,
                node_positions, edge_paths, bounding_box, tier_info,
                computation_time_ms, node_count, edge_count,
                computed_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now())
               ON CONFLICT (cbu_id, view_mode, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::UUID))
               DO UPDATE SET
                   input_hash = EXCLUDED.input_hash,
                   node_positions = EXCLUDED.node_positions,
                   edge_paths = EXCLUDED.edge_paths,
                   bounding_box = EXCLUDED.bounding_box,
                   tier_info = EXCLUDED.tier_info,
                   computation_time_ms = EXCLUDED.computation_time_ms,
                   node_count = EXCLUDED.node_count,
                   edge_count = EXCLUDED.edge_count,
                   computed_at = now(),
                   valid_until = NULL
               RETURNING cache_id"#,
        )
        .bind(cbu_id)
        .bind(view_mode)
        .bind(user_id)
        .bind(input_hash)
        .bind(node_positions)
        .bind(edge_paths)
        .bind(bounding_box)
        .bind(tier_info)
        .bind(computation_time_ms)
        .bind(node_count)
        .bind(edge_count)
        .fetch_one(pool)
        .await?;
        Ok(cache_id)
    }

    /// Invalidate layout cache for a CBU
    pub async fn invalidate_layout_cache(
        pool: &PgPool,
        cbu_id: Uuid,
        view_mode: Option<&str>,
    ) -> Result<i64> {
        let count =
            sqlx::query_scalar::<_, i64>(r#"SELECT "ob-poc".invalidate_layout_cache($1, $2)"#)
                .bind(cbu_id)
                .bind(view_mode)
                .fetch_one(pool)
                .await?;
        Ok(count)
    }

    // =========================================================================
    // CONVENIENCE METHODS
    // =========================================================================

    /// Check if a node type is visible in a given view mode
    pub fn is_node_type_visible(node_type: &NodeTypeConfig, view_mode: &str) -> bool {
        match view_mode {
            "UBO" | "KYC_UBO" => node_type.show_in_ubo_view,
            "TRADING" => node_type.show_in_trading_view,
            "FUND_STRUCTURE" => node_type.show_in_fund_structure_view,
            "SERVICE" | "SERVICE_DELIVERY" => node_type.show_in_service_view,
            "PRODUCT" | "PRODUCTS_ONLY" => node_type.show_in_product_view,
            // Unknown views show all node types
            _ => true,
        }
    }

    /// Check if an edge type is visible in a given view mode
    pub fn is_edge_type_visible(edge_type: &EdgeTypeConfig, view_mode: &str) -> bool {
        match view_mode {
            "UBO" | "KYC_UBO" => edge_type.show_in_ubo_view,
            "TRADING" => edge_type.show_in_trading_view,
            "FUND_STRUCTURE" => edge_type.show_in_fund_structure_view,
            "SERVICE" | "SERVICE_DELIVERY" => edge_type.show_in_service_view,
            "PRODUCT" | "PRODUCTS_ONLY" => edge_type.show_in_product_view,
            // Unknown views show all edge types
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_visibility() {
        let node_type = NodeTypeConfig {
            node_type_code: "CBU".to_string(),
            display_name: "Client Business Unit".to_string(),
            description: None,
            show_in_ubo_view: true,
            show_in_trading_view: true,
            show_in_fund_structure_view: true,
            show_in_service_view: true,
            show_in_product_view: true,
            icon: None,
            default_color: None,
            default_shape: None,
            default_width: None,
            default_height: None,
            can_be_container: true,
            default_tier: Some(0),
            importance_weight: None,
            child_layout_mode: None,
            container_padding: None,
            collapse_below_zoom: None,
            hide_label_below_zoom: None,
            show_detail_above_zoom: None,
            max_visible_children: None,
            overflow_behavior: None,
            dedupe_mode: None,
            min_separation: None,
            z_order: None,
            is_kyc_subject: false,
            is_structural: false,
            is_operational: false,
            is_trading: false,
            sort_order: None,
        };

        assert!(ViewConfigService::is_node_type_visible(&node_type, "UBO"));
        assert!(ViewConfigService::is_node_type_visible(
            &node_type, "KYC_UBO"
        ));
        assert!(ViewConfigService::is_node_type_visible(
            &node_type, "TRADING"
        ));
        assert!(ViewConfigService::is_node_type_visible(
            &node_type, "UNKNOWN"
        ));
    }

    #[test]
    fn test_edge_type_visibility() {
        let edge_type = EdgeTypeConfig {
            edge_type_code: "OWNERSHIP".to_string(),
            display_name: "Ownership".to_string(),
            description: None,
            from_node_types: serde_json::json!(["ENTITY_PERSON"]),
            to_node_types: serde_json::json!(["ENTITY_COMPANY"]),
            show_in_ubo_view: true,
            show_in_trading_view: false,
            show_in_fund_structure_view: false,
            show_in_service_view: false,
            show_in_product_view: false,
            edge_style: None,
            edge_color: None,
            edge_width: None,
            arrow_style: None,
            shows_percentage: true,
            shows_label: true,
            label_template: None,
            label_position: None,
            layout_direction: None,
            tier_delta: None,
            is_hierarchical: true,
            bundle_group: None,
            routing_priority: None,
            spring_strength: None,
            ideal_length: None,
            sibling_sort_key: None,
            source_anchor: None,
            target_anchor: None,
            cycle_break_priority: None,
            is_primary_parent_rule: None,
            parallel_edge_offset: None,
            self_loop_radius: None,
            self_loop_position: None,
            z_order: None,
            is_ownership: true,
            is_control: false,
            is_structural: false,
            is_service_delivery: false,
            is_trading: false,
            creates_kyc_obligation: true,
            cardinality: None,
            sort_order: None,
        };

        assert!(ViewConfigService::is_edge_type_visible(&edge_type, "UBO"));
        assert!(!ViewConfigService::is_edge_type_visible(
            &edge_type, "TRADING"
        ));
        assert!(ViewConfigService::is_edge_type_visible(
            &edge_type, "UNKNOWN"
        ));
    }
}
