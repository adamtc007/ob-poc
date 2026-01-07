# Layout Configuration Implementation

Implement the taxonomy layout configuration system - YAML schema and Rust types.

## Files to Create

### YAML Configuration Files

1. `rust/config/taxonomies/ubo_ownership.yaml`
2. `rust/config/taxonomies/entity_universe.yaml`
3. `rust/config/taxonomies/trading_instruments.yaml`
4. `rust/config/layout_config.yaml` (master config)

### Rust Type Files

1. `rust/crates/ob-poc-types/src/layout/config.rs`
2. `rust/crates/ob-poc-types/src/layout/config_loader.rs`

---

## YAML Configurations

### ubo_ownership.yaml
```yaml
taxonomy: ubo_ownership
version: "1.0"

layout:
  strategy: pyramid
  direction: top_down
  level_spacing: 120
  sibling_spacing: 80
  pyramid_expansion: 1.2

rank_rules:
  ULTIMATE_BENEFICIAL_OWNER:
    rank: 0
    anchor: top_center
  INTERMEDIATE_HOLDING:
    rank: derived
  HOLDING_COMPANY:
    rank: derived
  SUBJECT_ENTITY:
    rank: leaf
  PERSON:
    rank: contextual
  DIRECTOR:
    rank: contextual
  SIGNATORY:
    rank: contextual

edge_topology:
  OWNS:
    direction: above_to_below
    rank_delta: 1
  BENEFICIAL_OWNER:
    direction: above_to_below
    rank_delta: 1
  CONTROLS:
    direction: above_to_below
    rank_delta: 1
  SHAREHOLDER:
    direction: above_to_below
    rank_delta: 1
  DIRECTOR_OF:
    direction: horizontal
    rank_delta: 0
  SIGNATORY_OF:
    direction: horizontal
    rank_delta: 0
  RELATED_TO:
    direction: horizontal
    rank_delta: 0

floating_zone:
  position: right_gutter
  layout: vertical_stack
  max_width: 200
  label: "Unlinked Persons"

node_styles:
  ULTIMATE_BENEFICIAL_OWNER:
    color: "#E8B04A"
    shape: diamond
    size: [80, 60]
  HOLDING_COMPANY:
    color: "#4A90D9"
    shape: rectangle
    size: [120, 50]
  SUBJECT_ENTITY:
    color: "#6BBF6B"
    shape: rectangle
    size: [140, 60]
  PERSON:
    color: "#9B59B6"
    shape: circle
    size: [40, 40]
```

### entity_universe.yaml
```yaml
taxonomy: entity_universe
version: "1.0"

layout:
  strategy: solar_system
  center: focal_entity
  ring_spacing: 100
  rotation: organic

rings:
  - name: core
    filter:
      role_in: ["UBO", "DIRECTOR", "SIGNATORY", "BENEFICIAL_OWNER"]
    radius: 150
    style: evenly

  - name: inner
    filter:
      edge_type_in: ["OWNS", "CONTROLS", "SHAREHOLDER"]
    radius: 280
    style: evenly

  - name: outer
    filter:
      edge_type_in: ["RELATED_TO", "ASSOCIATED", "AFFILIATED"]
    radius: 420
    style: clustered

  - name: asteroid_belt
    filter: floating
    radius: 580
    style: scattered

center_selection:
  primary: focal_entity
  fallback: highest_degree

node_styles:
  CBU:
    color: "#2ECC71"
    shape: circle
    size: [60, 60]
  PERSON:
    color: "#9B59B6"
    shape: circle
    size: [30, 30]
  default:
    color: "#7F8C8D"
    shape: circle
    size: [40, 40]
```

### trading_instruments.yaml
```yaml
taxonomy: trading_instruments
version: "1.0"

layout:
  strategy: matrix
  rows_by: instrument_type
  cols_by: currency
  cell_padding: 20
  header_height: 40

row_order:
  - EQUITY
  - BOND
  - DERIVATIVE
  - FUND
  - OTHER

col_order:
  - USD
  - EUR
  - GBP
  - JPY
  - OTHER

node_styles:
  EQUITY:
    color: "#3498DB"
  BOND:
    color: "#27AE60"
  DERIVATIVE:
    color: "#E74C3C"
  FUND:
    color: "#9B59B6"
  default:
    color: "#7F8C8D"
```

### layout_config.yaml (Master Config)
```yaml
mass_weights:
  cbu: 100
  person: 10
  holding: 20
  edge: 5
  floating: 15

mass_thresholds:
  astro_threshold: 500
  hybrid_threshold: 100

density_rules:
  - threshold:
      gt: 20
      entity_type: visible_cbu
    mode: astro_overview
    node_rendering: compact_dot

  - threshold:
      min: 5
      max: 20
      entity_type: visible_cbu
    mode: astro_clustered
    node_rendering: labeled_circle
    cluster_by: sector

  - threshold:
      lt: 5
      entity_type: visible_cbu
    mode: hybrid_drilldown
    node_rendering: expanded_taxonomy
    expand_taxonomy: ubo_ownership

  - threshold: single
    mode: full_detail
    node_rendering: full_taxonomy_pyramid
    show_floating_persons: true

transitions:
  duration_ms: 400
  debounce_ms: 300
  easing: ease_out_cubic

visibility:
  min_node_size_px: 20

confirmation:
  mass_threshold: 100
```

---

## Rust Configuration Types

### config.rs
```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct TaxonomyLayoutConfig {
    pub taxonomy: String,
    #[serde(default)]
    pub version: String,
    pub layout: LayoutSpec,
    #[serde(default)]
    pub rank_rules: HashMap<String, RankRule>,
    #[serde(default)]
    pub edge_topology: HashMap<String, EdgeTopology>,
    #[serde(default)]
    pub floating_zone: Option<FloatingZoneSpec>,
    #[serde(default)]
    pub node_styles: HashMap<String, NodeStyleSpec>,
    #[serde(default)]
    pub rings: Vec<RingDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayoutSpec {
    pub strategy: LayoutStrategy,
    #[serde(flatten)]
    pub params: LayoutParams,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutStrategy {
    Pyramid,
    SolarSystem,
    Matrix,
    ForceDirected,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LayoutParams {
    Pyramid(PyramidConfig),
    SolarSystem(SolarSystemConfig),
    Matrix(MatrixConfig),
    ForceDirected(ForceDirectedConfig),
}

// Pyramid Configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PyramidConfig {
    #[serde(default = "default_direction")]
    pub direction: Direction,
    #[serde(default = "default_level_spacing")]
    pub level_spacing: f32,
    #[serde(default = "default_sibling_spacing")]
    pub sibling_spacing: f32,
    #[serde(default = "default_pyramid_expansion")]
    pub pyramid_expansion: f32,
}

fn default_direction() -> Direction { Direction::TopDown }
fn default_level_spacing() -> f32 { 120.0 }
fn default_sibling_spacing() -> f32 { 80.0 }
fn default_pyramid_expansion() -> f32 { 1.2 }

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    TopDown,
    BottomUp,
    LeftToRight,
    RightToLeft,
}

// Solar System Configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SolarSystemConfig {
    #[serde(default)]
    pub center: CenterSelection,
    #[serde(default = "default_ring_spacing")]
    pub ring_spacing: f32,
    #[serde(default)]
    pub rotation: RotationStrategy,
}

fn default_ring_spacing() -> f32 { 100.0 }

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CenterSelection {
    #[default]
    FocalEntity,
    HighestDegree,
    SelectedNode,
    ByNodeType(String),
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RotationStrategy {
    #[default]
    Organic,
    FixedSectors,
    Alphabetical,
    ByAttribute(String),
}

// Ring Definition
#[derive(Debug, Clone, Deserialize)]
pub struct RingDefinition {
    pub name: String,
    pub filter: RingFilter,
    pub radius: f32,
    #[serde(default)]
    pub style: RingStyle,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RingFilter {
    RoleIn { role_in: Vec<String> },
    EdgeTypeIn { edge_type_in: Vec<String> },
    HopDistance { hop_distance: u32 },
    Floating,
    Custom(String),
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RingStyle {
    #[default]
    Evenly,
    Clustered,
    Scattered,
}

// Matrix Configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MatrixConfig {
    pub rows_by: String,
    pub cols_by: String,
    #[serde(default = "default_cell_padding")]
    pub cell_padding: f32,
    #[serde(default)]
    pub row_order: Vec<String>,
    #[serde(default)]
    pub col_order: Vec<String>,
}

fn default_cell_padding() -> f32 { 20.0 }

// Force Directed Configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ForceDirectedConfig {
    #[serde(default = "default_damping")]
    pub damping: f32,
    #[serde(default = "default_iterations")]
    pub iterations: u32,
}

fn default_damping() -> f32 { 0.85 }
fn default_iterations() -> u32 { 100 }

// Rank Rules
#[derive(Debug, Clone, Deserialize)]
pub struct RankRule {
    pub rank: RankAssignment,
    #[serde(default)]
    pub anchor: Option<Anchor>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RankAssignment {
    Fixed(u32),
    Named(RankName),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankName {
    Derived,
    Leaf,
    Contextual,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    TopCenter,
    TopLeft,
    TopRight,
    BottomCenter,
    Center,
}

// Edge Topology
#[derive(Debug, Clone, Deserialize)]
pub struct EdgeTopology {
    pub direction: TopologyDirection,
    #[serde(default)]
    pub rank_delta: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyDirection {
    AboveToBelow,
    BelowToAbove,
    Horizontal,
    None,
}

// Floating Zone
#[derive(Debug, Clone, Deserialize)]
pub struct FloatingZoneSpec {
    pub position: FloatingPosition,
    pub layout: FloatingLayout,
    #[serde(default = "default_floating_width")]
    pub max_width: f32,
    #[serde(default)]
    pub label: Option<String>,
}

fn default_floating_width() -> f32 { 200.0 }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloatingPosition {
    RightGutter,
    LeftGutter,
    BottomDock,
    TopDock,
    SatelliteOrbit,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloatingLayout {
    VerticalStack,
    HorizontalStack,
    Grid,
    Scattered,
}

// Node Style
#[derive(Debug, Clone, Deserialize)]
pub struct NodeStyleSpec {
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub shape: Option<NodeShape>,
    #[serde(default)]
    pub size: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeShape {
    Circle,
    Rectangle,
    Diamond,
    Hexagon,
}
```

### config_loader.rs
```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct TaxonomyConfigLoader {
    config_dir: PathBuf,
    cache: RwLock<HashMap<String, TaxonomyLayoutConfig>>,
}

impl TaxonomyConfigLoader {
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
            cache: RwLock::new(HashMap::new()),
        }
    }
    
    pub fn load(&self, taxonomy_name: &str) -> Result<TaxonomyLayoutConfig, ConfigError> {
        // Check cache first
        if let Some(config) = self.cache.read().unwrap().get(taxonomy_name) {
            return Ok(config.clone());
        }
        
        // Load from file
        let path = self.config_dir.join(format!("{}.yaml", taxonomy_name));
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| ConfigError::FileNotFound(path.clone(), e))?;
        
        let config: TaxonomyLayoutConfig = serde_yaml::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(path.clone(), e))?;
        
        // Cache it
        self.cache.write().unwrap().insert(taxonomy_name.to_string(), config.clone());
        
        Ok(config)
    }
    
    pub fn reload(&self, taxonomy_name: &str) -> Result<TaxonomyLayoutConfig, ConfigError> {
        self.cache.write().unwrap().remove(taxonomy_name);
        self.load(taxonomy_name)
    }
    
    pub fn list_available(&self) -> Vec<String> {
        std::fs::read_dir(&self.config_dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension()? == "yaml" {
                    path.file_stem()?.to_str().map(String::from)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Debug)]
pub enum ConfigError {
    FileNotFound(PathBuf, std::io::Error),
    ParseError(PathBuf, serde_yaml::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileNotFound(path, e) => write!(f, "Config not found: {:?} - {}", path, e),
            Self::ParseError(path, e) => write!(f, "Parse error in {:?} - {}", path, e),
        }
    }
}
```

## Dependencies

Add to `Cargo.toml`:
```toml
serde_yaml = "0.9"
```

## Acceptance Criteria

- [ ] All YAML files created in config/taxonomies/
- [ ] Rust types match YAML schema
- [ ] Serde deserializes without errors
- [ ] Config loader caches loaded configs
- [ ] Hot-reload support via reload()
- [ ] list_available() returns taxonomy names
