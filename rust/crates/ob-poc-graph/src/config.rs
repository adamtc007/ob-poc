//! Graph Configuration (Policy-Driven)
//!
//! Centralized configuration for LOD, layout, animation, rendering, and navigation.
//! Loaded from `config/graph_settings.yaml` at startup.
//!
//! # Policy Hashing
//!
//! Policy changes produce a new `policy_hash`, which invalidates cached snapshots.
//! Use `GraphConfig::policy_hash()` to get the current hash.
//!
//! # Usage
//!
//! ```rust,no_run
//! use ob_poc_graph::config::GraphConfig;
//!
//! // Load from default path (may fail if file not found)
//! let config = GraphConfig::load_default().expect("config file");
//!
//! // Access LOD thresholds
//! let threshold = config.lod.thresholds.compact;
//!
//! // Get policy hash for cache key
//! let hash = config.policy_hash();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::OnceLock;

/// Global config instance
static GLOBAL_CONFIG: OnceLock<GraphConfig> = OnceLock::new();

/// Get the global graph config (loads default on first access)
pub fn global_config() -> &'static GraphConfig {
    GLOBAL_CONFIG.get_or_init(|| {
        GraphConfig::load_default().unwrap_or_else(|e| {
            eprintln!(
                "Warning: Failed to load graph_settings.yaml: {}. Using defaults.",
                e
            );
            GraphConfig::default()
        })
    })
}

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphConfig {
    pub policy: PolicyMetadata,
    pub lod: LodConfig,
    pub budgets: BudgetConfig,
    pub flyover: FlyoverConfig,
    pub structural: StructuralConfig,
    pub camera: CameraConfig,
    pub focus: FocusConfig,
    pub label_cache: LabelCacheConfig,
    pub spatial_index: SpatialIndexConfig,
    pub layout: LayoutConfig,
    pub viewport: ViewportConfig,
    pub animation: AnimationConfig,
    pub rendering: RenderingConfig,
    pub colors: ColorConfig,
    pub debug: DebugConfig,
    pub clamps: ClampConfig,
}

impl GraphConfig {
    /// Load config from the default path (config/graph_settings.yaml)
    pub fn load_default() -> Result<Self, ConfigError> {
        let paths = [
            "config/graph_settings.yaml",
            "../config/graph_settings.yaml",
            "rust/config/graph_settings.yaml",
        ];

        for path in paths {
            if Path::new(path).exists() {
                return Self::load_from_path(path);
            }
        }

        Err(ConfigError::NotFound("graph_settings.yaml".to_string()))
    }

    /// Load config from a specific path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content =
            std::fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::Io(e.to_string()))?;
        Self::parse(&content)
    }

    /// Parse config from YAML string
    pub fn parse(yaml: &str) -> Result<Self, ConfigError> {
        serde_yaml::from_str(yaml).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Compute policy hash for cache invalidation
    ///
    /// Changes to the config produce a different hash, which should
    /// invalidate cached WorldSnapshots.
    ///
    /// Uses deterministic fields only (excludes HashMap fields which have
    /// non-deterministic iteration order).
    pub fn policy_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Policy metadata
        self.policy.version.hash(&mut hasher);
        self.policy.name.hash(&mut hasher);
        self.policy.variant.hash(&mut hasher);

        // LOD tiers (deterministic struct fields)
        self.lod.tiers.icon.zoom_max.to_bits().hash(&mut hasher);
        self.lod.tiers.label.zoom_max.to_bits().hash(&mut hasher);
        self.lod.tiers.extended.zoom_max.to_bits().hash(&mut hasher);
        self.lod.tiers.full.zoom_max.to_bits().hash(&mut hasher);

        // LOD thresholds
        self.lod.thresholds.micro.to_bits().hash(&mut hasher);
        self.lod.thresholds.icon.to_bits().hash(&mut hasher);
        self.lod.thresholds.compact.to_bits().hash(&mut hasher);
        self.lod.thresholds.standard.to_bits().hash(&mut hasher);

        // Budgets
        self.budgets.label_budget_count.hash(&mut hasher);
        self.budgets.full_budget_count.hash(&mut hasher);
        self.budgets
            .shape_budget_ms_per_frame
            .to_bits()
            .hash(&mut hasher);

        // Flyover
        self.flyover.dwell_ticks.hash(&mut hasher);
        self.flyover.settle_duration_s.to_bits().hash(&mut hasher);

        // Layout node
        self.layout.node.width.to_bits().hash(&mut hasher);
        self.layout.node.height.to_bits().hash(&mut hasher);

        // Spatial index default
        self.spatial_index
            .default_cell_size
            .to_bits()
            .hash(&mut hasher);

        // Clamps
        self.clamps.max_nodes_visible.hash(&mut hasher);
        self.clamps.max_snapshot_bytes.hash(&mut hasher);

        hasher.finish()
    }

    /// Get schema version for compatibility checking
    pub fn schema_version(&self) -> u32 {
        self.policy.version
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // LOD tiers must have ascending zoom_max
        if self.lod.tiers.icon.zoom_max >= self.lod.tiers.label.zoom_max {
            errors.push("LOD: icon zoom_max must be < label zoom_max".to_string());
        }
        if self.lod.tiers.label.zoom_max >= self.lod.tiers.extended.zoom_max {
            errors.push("LOD: label zoom_max must be < extended zoom_max".to_string());
        }
        if self.lod.tiers.extended.zoom_max >= self.lod.tiers.full.zoom_max {
            errors.push("LOD: extended zoom_max must be < full zoom_max".to_string());
        }

        // Hysteresis must be positive
        for (name, tier) in [
            ("icon", &self.lod.tiers.icon),
            ("label", &self.lod.tiers.label),
            ("extended", &self.lod.tiers.extended),
            ("full", &self.lod.tiers.full),
        ] {
            if tier.hysteresis < 0.0 {
                errors.push(format!("LOD: {} hysteresis must be >= 0", name));
            }
        }

        // Legacy thresholds must be ascending
        if self.lod.thresholds.micro >= self.lod.thresholds.icon {
            errors.push("LOD: micro threshold must be < icon threshold".to_string());
        }
        if self.lod.thresholds.icon >= self.lod.thresholds.compact {
            errors.push("LOD: icon threshold must be < compact threshold".to_string());
        }
        if self.lod.thresholds.compact >= self.lod.thresholds.standard {
            errors.push("LOD: compact threshold must be < standard threshold".to_string());
        }

        // Budget constraints
        if self.budgets.label_budget_count == 0 {
            errors.push("Budgets: label_budget_count must be > 0".to_string());
        }
        if self.budgets.full_budget_count == 0 {
            errors.push("Budgets: full_budget_count must be > 0".to_string());
        }
        if self.budgets.shape_budget_ms_per_frame <= 0.0 {
            errors.push("Budgets: shape_budget_ms_per_frame must be > 0".to_string());
        }

        // Flyover constraints
        if self.flyover.dwell_ticks == 0 {
            errors.push("Flyover: dwell_ticks must be > 0".to_string());
        }
        if self.flyover.settle_duration_s <= 0.0 {
            errors.push("Flyover: settle_duration_s must be > 0".to_string());
        }

        // Camera constraints
        if self.camera.snap_epsilon <= 0.0 {
            errors.push("Camera: snap_epsilon must be > 0".to_string());
        }

        // Label cache constraints
        if self.label_cache.max_entries == 0 {
            errors.push("LabelCache: max_entries must be > 0".to_string());
        }
        if self.label_cache.width_quantization == 0 {
            errors.push("LabelCache: width_quantization must be > 0".to_string());
        }

        // Layout constraints
        if self.layout.node.width <= 0.0 {
            errors.push("Layout: node width must be positive".to_string());
        }
        if self.layout.node.height <= 0.0 {
            errors.push("Layout: node height must be positive".to_string());
        }
        if self.layout.node.min_scale <= 0.0 || self.layout.node.min_scale > 1.0 {
            errors.push("Layout: node min_scale must be in (0, 1]".to_string());
        }
        if self.layout.node.max_scale < 1.0 {
            errors.push("Layout: node max_scale must be >= 1.0".to_string());
        }

        // Viewport constraints
        if self.viewport.min_auto_zoom <= 0.0 {
            errors.push("Viewport: min_auto_zoom must be positive".to_string());
        }
        if self.viewport.max_auto_zoom <= self.viewport.min_auto_zoom {
            errors.push("Viewport: max_auto_zoom must be > min_auto_zoom".to_string());
        }
        if self.viewport.fit_margin <= 0.0 || self.viewport.fit_margin > 1.0 {
            errors.push("Viewport: fit_margin must be in (0, 1]".to_string());
        }

        // Animation spring validation
        for (name, spring) in &self.animation.springs {
            if spring.stiffness <= 0.0 {
                errors.push(format!(
                    "Animation: spring '{}' stiffness must be positive",
                    name
                ));
            }
            if spring.damping <= 0.0 {
                errors.push(format!(
                    "Animation: spring '{}' damping must be positive",
                    name
                ));
            }
        }

        // Clamp constraints
        if self.clamps.max_nodes_visible == 0 {
            errors.push("Clamps: max_nodes_visible must be > 0".to_string());
        }
        if self.clamps.max_snapshot_bytes == 0 {
            errors.push("Clamps: max_snapshot_bytes must be > 0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// =============================================================================
// POLICY METADATA
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyMetadata {
    pub version: u32,
    pub name: String,
    pub variant: String,
}

impl Default for PolicyMetadata {
    fn default() -> Self {
        Self {
            version: 1,
            name: "default".to_string(),
            variant: "baseline".to_string(),
        }
    }
}

// =============================================================================
// LOD CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LodConfig {
    pub tiers: LodTierConfig,
    pub thresholds: LodThresholds,
    #[serde(default)]
    pub manual_cycle_order: Vec<String>,
    pub density: DensityConfig,
    pub compression: CompressionThresholds,
}

impl Default for LodConfig {
    fn default() -> Self {
        Self {
            tiers: LodTierConfig::default(),
            thresholds: LodThresholds::default(),
            manual_cycle_order: vec![
                "icon".to_string(),
                "label".to_string(),
                "extended".to_string(),
                "full".to_string(),
            ],
            density: DensityConfig::default(),
            compression: CompressionThresholds::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LodTierConfig {
    pub icon: LodTier,
    pub label: LodTier,
    pub extended: LodTier,
    pub full: LodTier,
}

impl Default for LodTierConfig {
    fn default() -> Self {
        Self {
            icon: LodTier {
                zoom_max: 0.8,
                hysteresis: 0.08,
            },
            label: LodTier {
                zoom_max: 1.5,
                hysteresis: 0.10,
            },
            extended: LodTier {
                zoom_max: 3.0,
                hysteresis: 0.12,
            },
            full: LodTier {
                zoom_max: 999.0,
                hysteresis: 0.15,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LodTier {
    pub zoom_max: f32,
    pub hysteresis: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LodThresholds {
    pub micro: f32,
    pub icon: f32,
    pub compact: f32,
    pub standard: f32,
}

impl Default for LodThresholds {
    fn default() -> Self {
        Self {
            micro: 20.0,
            icon: 40.0,
            compact: 70.0,
            standard: 120.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DensityConfig {
    pub reference_viewport_area: f32,
    pub base: f32,
    pub weight: f32,
}

impl Default for DensityConfig {
    fn default() -> Self {
        Self {
            reference_viewport_area: 480_000.0,
            base: 20.0,
            weight: 0.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionThresholds {
    pub icon: f32,
    pub compact: f32,
    pub standard: f32,
}

impl Default for CompressionThresholds {
    fn default() -> Self {
        Self {
            icon: 0.8,
            compact: 0.5,
            standard: 0.2,
        }
    }
}

// =============================================================================
// BUDGET CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub icons_unlimited: bool,
    pub label_budget_count: usize,
    pub full_budget_count: usize,
    pub shape_budget_ms_per_frame: f32,
    pub visible_query_budget_ms: f32,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            icons_unlimited: true,
            label_budget_count: 250,
            full_budget_count: 20,
            shape_budget_ms_per_frame: 3.0,
            visible_query_budget_ms: 2.0,
        }
    }
}

// =============================================================================
// FLYOVER CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlyoverConfig {
    pub dwell_ticks: u32,
    pub settle_duration_s: f32,
    pub easing: String,
    pub phases: FlyoverPhaseConfig,
    pub mode_defaults: FlyoverModeDefaults,
}

impl Default for FlyoverConfig {
    fn default() -> Self {
        Self {
            dwell_ticks: 30,
            settle_duration_s: 0.2,
            easing: "smoothstep".to_string(),
            phases: FlyoverPhaseConfig::default(),
            mode_defaults: FlyoverModeDefaults::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlyoverPhaseConfig {
    pub moving: PhaseSettings,
    pub settling: PhaseSettings,
    pub focused: PhaseSettings,
}

impl Default for FlyoverPhaseConfig {
    fn default() -> Self {
        Self {
            moving: PhaseSettings {
                selection_lod: "icon".to_string(),
                siblings_lod: "icon".to_string(),
                shaping_allowed: false,
            },
            settling: PhaseSettings {
                selection_lod: "label".to_string(),
                siblings_lod: "icon".to_string(),
                shaping_allowed: true,
            },
            focused: PhaseSettings {
                selection_lod: "full".to_string(),
                siblings_lod: "label".to_string(),
                shaping_allowed: true,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSettings {
    pub selection_lod: String,
    pub siblings_lod: String,
    pub shaping_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlyoverModeDefaults {
    pub spatial: ModeTimings,
    pub structural: ModeTimings,
}

impl Default for FlyoverModeDefaults {
    fn default() -> Self {
        Self {
            spatial: ModeTimings {
                dwell_ticks: 30,
                settle_duration_s: 0.2,
            },
            structural: ModeTimings {
                dwell_ticks: 20,
                settle_duration_s: 0.15,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeTimings {
    pub dwell_ticks: u32,
    pub settle_duration_s: f32,
}

// =============================================================================
// STRUCTURAL CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralConfig {
    pub density_cutover: StructuralDensityCutover,
    pub max_labels_per_cluster: usize,
}

impl Default for StructuralConfig {
    fn default() -> Self {
        Self {
            density_cutover: StructuralDensityCutover::default(),
            max_labels_per_cluster: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralDensityCutover {
    pub icon_only: usize,
    pub labels: usize,
}

impl Default for StructuralDensityCutover {
    fn default() -> Self {
        Self {
            icon_only: 50,
            labels: 10,
        }
    }
}

// =============================================================================
// CAMERA CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub pan_speed: f32,
    pub zoom_speed: f32,
    pub snap_epsilon: f32,
    pub focus_padding: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            pan_speed: 1.0,
            zoom_speed: 1.0,
            snap_epsilon: 0.001,
            focus_padding: 50.0,
        }
    }
}

// =============================================================================
// FOCUS CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusConfig {
    pub selection_priority: Vec<String>,
    pub neighbor_ring_size: usize,
    pub prefetch_radius_cells: usize,
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            selection_priority: vec![
                "selected".to_string(),
                "hovered".to_string(),
                "recent".to_string(),
            ],
            neighbor_ring_size: 3,
            prefetch_radius_cells: 2,
        }
    }
}

// =============================================================================
// LABEL CACHE CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelCacheConfig {
    pub max_entries: usize,
    pub width_quantization: u16,
    pub eviction: String,
}

impl Default for LabelCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 500,
            width_quantization: 10,
            eviction: "lru".to_string(),
        }
    }
}

// =============================================================================
// SPATIAL INDEX CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialIndexConfig {
    pub default_cell_size: f32,
    #[serde(default)]
    pub chamber_overrides: HashMap<String, ChamberSpatialConfig>,
}

impl Default for SpatialIndexConfig {
    fn default() -> Self {
        let mut overrides = HashMap::new();
        overrides.insert(
            "instrument_matrix".to_string(),
            ChamberSpatialConfig { cell_size: 50.0 },
        );
        overrides.insert(
            "cbu_graph".to_string(),
            ChamberSpatialConfig { cell_size: 150.0 },
        );

        Self {
            default_cell_size: 100.0,
            chamber_overrides: overrides,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChamberSpatialConfig {
    pub cell_size: f32,
}

// =============================================================================
// LAYOUT CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayoutConfig {
    pub node: NodeConfig,
    pub spacing: SpacingConfig,
    pub tiers: TierConfig,
    pub ubo_tiers: UboTierConfig,
    pub trading: TradingLayoutConfig,
    pub container: ContainerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub width: f32,
    pub height: f32,
    pub min_scale: f32,
    pub max_scale: f32,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            width: 160.0,
            height: 70.0,
            min_scale: 0.7,
            max_scale: 1.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingConfig {
    pub horizontal: f32,
    pub vertical: f32,
}

impl Default for SpacingConfig {
    fn default() -> Self {
        Self {
            horizontal: 40.0,
            vertical: 120.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    pub cbu: f32,
    pub structure: f32,
    pub officers: f32,
    pub ubo: f32,
    pub investors: f32,
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            cbu: 0.0,
            structure: 150.0,
            officers: 300.0,
            ubo: 450.0,
            investors: 600.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboTierConfig {
    pub person: f32,
    pub shell_base: f32,
    pub cbu: f32,
}

impl Default for UboTierConfig {
    fn default() -> Self {
        Self {
            person: 0.0,
            shell_base: 150.0,
            cbu: 450.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingLayoutConfig {
    pub gap_above: f32,
    pub gap_below: f32,
    pub tier_spacing: f32,
}

impl Default for TradingLayoutConfig {
    fn default() -> Self {
        Self {
            gap_above: 100.0,
            gap_below: 100.0,
            tier_spacing: 80.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub padding: f32,
    pub header_height: f32,
    pub corner_radius: f32,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            padding: 40.0,
            header_height: 36.0,
            corner_radius: 8.0,
        }
    }
}

// =============================================================================
// VIEWPORT CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportConfig {
    pub fit_margin: f32,
    pub min_auto_zoom: f32,
    pub max_auto_zoom: f32,
    pub max_visible_nodes: usize,
    pub max_visible_clusters: usize,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            fit_margin: 0.9,
            min_auto_zoom: 0.1,
            max_auto_zoom: 2.0,
            max_visible_nodes: 200,
            max_visible_clusters: 50,
        }
    }
}

// =============================================================================
// ANIMATION CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    pub springs: HashMap<String, SpringConfigYaml>,
    pub default_hold_ms: u64,
    pub scale_pulse_peak: f32,
    pub scale_settle_factor: f32,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        // Damping ratio: 1.0 = critically damped (smooth, no overshoot)
        //                < 1.0 = underdamped (bouncy, overshoots)
        //                > 1.0 = overdamped (sluggish)
        let mut springs = HashMap::new();
        springs.insert(
            "fast".to_string(),
            SpringConfigYaml {
                stiffness: 300.0,
                damping: 1.0,
            },
        );
        springs.insert(
            "medium".to_string(),
            SpringConfigYaml {
                stiffness: 150.0,
                damping: 1.0,
            },
        );
        springs.insert(
            "slow".to_string(),
            SpringConfigYaml {
                stiffness: 80.0,
                damping: 1.0,
            },
        );
        springs.insert(
            "bouncy".to_string(),
            SpringConfigYaml {
                stiffness: 200.0,
                damping: 0.6,
            },
        );
        springs.insert(
            "instant".to_string(),
            SpringConfigYaml {
                stiffness: 500.0,
                damping: 1.0,
            },
        );
        springs.insert(
            "snappy".to_string(),
            SpringConfigYaml {
                stiffness: 300.0,
                damping: 1.25,
            },
        );
        springs.insert(
            "organic".to_string(),
            SpringConfigYaml {
                stiffness: 180.0,
                damping: 0.85,
            },
        );
        springs.insert(
            "gentle".to_string(),
            SpringConfigYaml {
                stiffness: 120.0,
                damping: 1.0,
            },
        );
        springs.insert(
            "camera".to_string(),
            SpringConfigYaml {
                stiffness: 150.0,
                damping: 1.1,
            },
        );
        springs.insert(
            "agent_ui".to_string(),
            SpringConfigYaml {
                stiffness: 200.0,
                damping: 1.0,
            },
        );
        springs.insert(
            "autopilot".to_string(),
            SpringConfigYaml {
                stiffness: 120.0,
                damping: 0.95,
            },
        );
        springs.insert(
            "pulse".to_string(),
            SpringConfigYaml {
                stiffness: 60.0,
                damping: 0.8,
            },
        );

        Self {
            springs,
            default_hold_ms: 100,
            scale_pulse_peak: 1.03,
            scale_settle_factor: 0.3,
        }
    }
}

impl AnimationConfig {
    /// Get a spring config by name, falling back to "medium" if not found
    pub fn spring(&self, name: &str) -> SpringConfigYaml {
        self.springs.get(name).cloned().unwrap_or(SpringConfigYaml {
            stiffness: 150.0,
            damping: 1.0,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SpringConfigYaml {
    pub stiffness: f32,
    pub damping: f32,
}

// =============================================================================
// RENDERING CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderingConfig {
    pub edges: EdgeConfig,
    pub blur_opacity: f32,
    pub board_control: BoardControlConfig,
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            edges: EdgeConfig::default(),
            blur_opacity: 0.25,
            board_control: BoardControlConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeConfig {
    pub bezier_segments: usize,
    pub arrow_size: f32,
    pub label_zoom_threshold: f32,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            bezier_segments: 20,
            arrow_size: 8.0,
            label_zoom_threshold: 0.6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardControlConfig {
    pub node_width: f32,
    pub node_height: f32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub corner_radius: f32,
}

impl Default for BoardControlConfig {
    fn default() -> Self {
        Self {
            node_width: 180.0,
            node_height: 80.0,
            h_spacing: 60.0,
            v_spacing: 140.0,
            corner_radius: 8.0,
        }
    }
}

// =============================================================================
// COLOR CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ColorConfig {
    pub risk: RiskColors,
    pub kyc: KycColors,
    pub entity: EntityColors,
    pub sun: SunColors,
}

/// RGB color as [r, g, b] array
pub type Rgb = [u8; 3];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskColors {
    pub standard: Rgb,
    pub low: Rgb,
    pub medium: Rgb,
    pub high: Rgb,
    pub prohibited: Rgb,
    pub unrated: Rgb,
}

impl Default for RiskColors {
    fn default() -> Self {
        Self {
            standard: [76, 175, 80],
            low: [139, 195, 74],
            medium: [255, 193, 7],
            high: [255, 87, 34],
            prohibited: [33, 33, 33],
            unrated: [158, 158, 158],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycColors {
    pub complete: Rgb,
    pub partial: Rgb,
    pub draft: Rgb,
    pub pending: Rgb,
    pub overdue: Rgb,
}

impl Default for KycColors {
    fn default() -> Self {
        Self {
            complete: [76, 175, 80],
            partial: [255, 193, 7],
            draft: [158, 158, 158],
            pending: [66, 165, 245],
            overdue: [244, 67, 54],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityColors {
    pub shell: Rgb,
    pub person: Rgb,
    pub product: Rgb,
    pub service: Rgb,
}

impl Default for EntityColors {
    fn default() -> Self {
        Self {
            shell: [66, 165, 245],
            person: [102, 187, 106],
            product: [255, 167, 38],
            service: [171, 71, 188],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SunColors {
    pub core: Rgb,
    pub glow: Rgb,
}

impl Default for SunColors {
    fn default() -> Self {
        Self {
            core: [255, 215, 0],
            glow: [255, 200, 50],
        }
    }
}

// =============================================================================
// DEBUG CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DebugConfig {
    pub overlay: DebugOverlayConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugOverlayConfig {
    pub enabled: bool,
    pub show_hashes: bool,
    pub show_phase: bool,
    pub show_lod_counts: bool,
    pub show_cache_stats: bool,
    pub show_timings: bool,
}

impl Default for DebugOverlayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_hashes: true,
            show_phase: true,
            show_lod_counts: true,
            show_cache_stats: true,
            show_timings: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub stage_timings_enabled: bool,
    pub snapshot_size_enabled: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            stage_timings_enabled: true,
            snapshot_size_enabled: true,
        }
    }
}

// =============================================================================
// CLAMP CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClampConfig {
    pub max_nodes_visible: usize,
    pub max_chambers_loaded: usize,
    pub max_snapshot_bytes: usize,
}

impl Default for ClampConfig {
    fn default() -> Self {
        Self {
            max_nodes_visible: 5000,
            max_chambers_loaded: 50,
            max_snapshot_bytes: 50_000_000,
        }
    }
}

// =============================================================================
// ERROR TYPE
// =============================================================================

#[derive(Debug)]
pub enum ConfigError {
    NotFound(String),
    Io(String),
    Parse(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound(path) => write!(f, "Config file not found: {}", path),
            ConfigError::Io(e) => write!(f, "IO error: {}", e),
            ConfigError::Parse(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = GraphConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_policy_hash_changes_on_modification() {
        let config1 = GraphConfig::default();
        let mut config2 = GraphConfig::default();
        config2.budgets.label_budget_count = 100;

        assert_ne!(config1.policy_hash(), config2.policy_hash());
    }

    #[test]
    fn test_policy_hash_stable_for_same_config() {
        // Note: HashMap iteration order is non-deterministic, so we test
        // that the same config instance produces stable hash on multiple calls
        let config = GraphConfig::default();
        let hash1 = config.policy_hash();
        let hash2 = config.policy_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_parse_extended_yaml() {
        let yaml = r#"
policy:
  version: 1
  name: "test"
  variant: "dev"
lod:
  tiers:
    icon:
      zoom_max: 0.9
      hysteresis: 0.1
    label:
      zoom_max: 1.6
      hysteresis: 0.12
    extended:
      zoom_max: 3.5
      hysteresis: 0.14
    full:
      zoom_max: 999.0
      hysteresis: 0.18
  thresholds:
    micro: 15.0
    icon: 35.0
    compact: 65.0
    standard: 110.0
  manual_cycle_order: [icon, label, full]
  density:
    reference_viewport_area: 480000.0
    base: 25.0
    weight: 0.4
  compression:
    icon: 0.85
    compact: 0.55
    standard: 0.25
budgets:
  icons_unlimited: true
  label_budget_count: 200
  full_budget_count: 15
  shape_budget_ms_per_frame: 2.5
  visible_query_budget_ms: 1.5
flyover:
  dwell_ticks: 25
  settle_duration_s: 0.25
  easing: "cubic"
  phases:
    moving:
      selection_lod: icon
      siblings_lod: icon
      shaping_allowed: false
    settling:
      selection_lod: label
      siblings_lod: icon
      shaping_allowed: true
    focused:
      selection_lod: full
      siblings_lod: label
      shaping_allowed: true
  mode_defaults:
    spatial:
      dwell_ticks: 25
      settle_duration_s: 0.25
    structural:
      dwell_ticks: 18
      settle_duration_s: 0.12
structural:
  density_cutover:
    icon_only: 40
    labels: 8
  max_labels_per_cluster: 25
camera:
  pan_speed: 1.2
  zoom_speed: 1.1
  snap_epsilon: 0.002
  focus_padding: 60.0
focus:
  selection_priority: [selected, hovered]
  neighbor_ring_size: 4
  prefetch_radius_cells: 3
label_cache:
  max_entries: 600
  width_quantization: 12
  eviction: "lru"
spatial_index:
  default_cell_size: 120.0
  chamber_overrides: {}
layout:
  node:
    width: 180.0
    height: 80.0
    min_scale: 0.6
    max_scale: 1.4
  spacing:
    horizontal: 50.0
    vertical: 130.0
  tiers:
    cbu: 0.0
    structure: 160.0
    officers: 320.0
    ubo: 480.0
    investors: 640.0
  ubo_tiers:
    person: 0.0
    shell_base: 160.0
    cbu: 480.0
  trading:
    gap_above: 110.0
    gap_below: 110.0
    tier_spacing: 90.0
  container:
    padding: 45.0
    header_height: 40.0
    corner_radius: 10.0
viewport:
  fit_margin: 0.85
  min_auto_zoom: 0.15
  max_auto_zoom: 2.5
  max_visible_nodes: 250
  max_visible_clusters: 60
animation:
  springs:
    camera:
      stiffness: 150.0
      damping: 1.1
  default_hold_ms: 120
  scale_pulse_peak: 1.05
  scale_settle_factor: 0.35
rendering:
  edges:
    bezier_segments: 25
    arrow_size: 10.0
    label_zoom_threshold: 0.5
  blur_opacity: 0.3
  board_control:
    node_width: 200.0
    node_height: 90.0
    h_spacing: 70.0
    v_spacing: 150.0
    corner_radius: 10.0
colors:
  risk:
    standard: [80, 180, 85]
    low: [140, 200, 75]
    medium: [255, 195, 10]
    high: [255, 90, 35]
    prohibited: [35, 35, 35]
    unrated: [160, 160, 160]
  kyc:
    complete: [80, 180, 85]
    partial: [255, 195, 10]
    draft: [160, 160, 160]
    pending: [70, 170, 250]
    overdue: [245, 70, 55]
  entity:
    shell: [70, 170, 250]
    person: [105, 190, 110]
    product: [255, 170, 40]
    service: [175, 75, 190]
  sun:
    core: [255, 220, 5]
    glow: [255, 205, 55]
debug:
  overlay:
    enabled: true
    show_hashes: true
    show_phase: true
    show_lod_counts: true
    show_cache_stats: true
    show_timings: true
  metrics:
    stage_timings_enabled: true
    snapshot_size_enabled: true
clamps:
  max_nodes_visible: 4000
  max_chambers_loaded: 40
  max_snapshot_bytes: 40000000
"#;

        let config = GraphConfig::parse(yaml).expect("Failed to parse YAML");
        assert_eq!(config.policy.name, "test");
        assert_eq!(config.lod.tiers.icon.zoom_max, 0.9);
        assert_eq!(config.budgets.label_budget_count, 200);
        assert_eq!(config.flyover.dwell_ticks, 25);
        assert_eq!(config.structural.density_cutover.icon_only, 40);
        assert_eq!(config.camera.pan_speed, 1.2);
        assert_eq!(config.focus.neighbor_ring_size, 4);
        assert_eq!(config.label_cache.max_entries, 600);
        assert!(config.debug.overlay.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_budget() {
        let mut config = GraphConfig::default();
        config.budgets.label_budget_count = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("label_budget_count"));
    }

    #[test]
    fn test_invalid_flyover() {
        let mut config = GraphConfig::default();
        config.flyover.dwell_ticks = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("dwell_ticks"));
    }

    #[test]
    fn test_spring_lookup() {
        let config = GraphConfig::default();
        let camera = config.animation.spring("camera");
        assert_eq!(camera.stiffness, 150.0);
        assert_eq!(camera.damping, 1.1);

        // Fallback for unknown - uses medium defaults
        let unknown = config.animation.spring("nonexistent");
        assert_eq!(unknown.stiffness, 150.0);
        assert_eq!(unknown.damping, 1.0);
    }
}
