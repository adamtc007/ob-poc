//! Compiler configuration.

use crate::{DEFAULT_GRID_CELL_SIZE, DEFAULT_VIEWPORT_PADDING, MAX_ENTITIES_PER_CHAMBER};
use serde::{Deserialize, Serialize};

/// Configuration for the snapshot compiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerConfig {
    /// Layout configuration.
    pub layout: LayoutConfig,
    /// Chamber configuration.
    pub chamber: ChamberConfig,
    /// Whether to validate output.
    pub validate_output: bool,
    /// Whether to compute grid spatial index.
    pub compute_grid: bool,
    /// Schema version for output snapshots.
    pub schema_version: u32,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            layout: LayoutConfig::default(),
            chamber: ChamberConfig::default(),
            validate_output: true,
            compute_grid: true,
            schema_version: 1,
        }
    }
}

/// Layout algorithm configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Layout algorithm to use.
    pub algorithm: LayoutAlgorithmConfig,
    /// Viewport width.
    pub viewport_width: f32,
    /// Viewport height.
    pub viewport_height: f32,
    /// Padding around edges.
    pub padding: f32,
    /// Node spacing (minimum distance between nodes).
    pub node_spacing: f32,
    /// Level spacing (for hierarchical layouts).
    pub level_spacing: f32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            algorithm: LayoutAlgorithmConfig::Tree,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            padding: DEFAULT_VIEWPORT_PADDING,
            node_spacing: 80.0,
            level_spacing: 120.0,
        }
    }
}

/// Layout algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LayoutAlgorithmConfig {
    /// Tree layout (hierarchical).
    #[default]
    Tree,
    /// Force-directed layout.
    Force,
    /// Grid layout.
    Grid,
    /// Radial/orbital layout.
    Radial,
    /// Matrix layout (for dense connections).
    Matrix,
}

/// Chamber configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChamberConfig {
    /// Maximum entities per chamber.
    pub max_entities: usize,
    /// Grid cell size for spatial indexing.
    pub grid_cell_size: f32,
    /// Whether to create sub-chambers for large graphs.
    pub auto_split: bool,
    /// Minimum entities to trigger split.
    pub split_threshold: usize,
}

impl Default for ChamberConfig {
    fn default() -> Self {
        Self {
            max_entities: MAX_ENTITIES_PER_CHAMBER,
            grid_cell_size: DEFAULT_GRID_CELL_SIZE,
            auto_split: true,
            split_threshold: 5000,
        }
    }
}

impl CompilerConfig {
    /// Create a minimal config for small graphs.
    pub fn minimal() -> Self {
        Self {
            layout: LayoutConfig::default(),
            chamber: ChamberConfig {
                max_entities: 1000,
                grid_cell_size: 25.0,
                auto_split: false,
                split_threshold: 500,
            },
            validate_output: false,
            compute_grid: false,
            schema_version: 1,
        }
    }

    /// Create config for large graphs.
    pub fn large_graph() -> Self {
        Self {
            layout: LayoutConfig {
                algorithm: LayoutAlgorithmConfig::Force,
                viewport_width: 4000.0,
                viewport_height: 3000.0,
                ..Default::default()
            },
            chamber: ChamberConfig {
                max_entities: 50_000,
                grid_cell_size: 100.0,
                auto_split: true,
                split_threshold: 10_000,
            },
            validate_output: true,
            compute_grid: true,
            schema_version: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = CompilerConfig::default();
        assert!(config.validate_output);
        assert!(config.compute_grid);
        assert_eq!(config.schema_version, 1);
    }

    #[test]
    fn minimal_config() {
        let config = CompilerConfig::minimal();
        assert!(!config.validate_output);
        assert!(!config.compute_grid);
        assert!(config.chamber.max_entities < MAX_ENTITIES_PER_CHAMBER);
    }

    #[test]
    fn large_graph_config() {
        let config = CompilerConfig::large_graph();
        assert!(config.chamber.max_entities > MAX_ENTITIES_PER_CHAMBER);
        assert!(config.chamber.auto_split);
    }
}
