# TODO: CBU Token System - Entity-to-Render Pipeline

## ⛔ MANDATORY FIRST STEP

**Read these files:**
- `/EGUI-RULES.md` - UI patterns
- `/rust/src/graph/types.rs` - Current graph model
- `/rust/config/ontology/entity_taxonomy.yaml` - Entity definitions

**Dependencies:**
- Builds on: `TODO-CBU-VISUALIZATION-ANIMATION.md` (springs, camera)
- Builds on: `TODO-CBU-CONTAINER-ARCHITECTURE.md` (containers, lazy load)

---

## Overview

The Token System is the **bridge** between data entities and rendered visuals.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   DATA ENTITIES              TOKENS                    RENDERED OUTPUT      │
│   (what exists)              (how it behaves)          (what you see)       │
│                                                                             │
│   ┌─────────────┐           ┌─────────────┐           ┌─────────────┐      │
│   │ Shareholder │ ────────▶ │ Shareholder │ ────────▶ │ 👤 / Card / │      │
│   │ entity_id   │  TokenMap │ Token       │  Render   │ Detail View │      │
│   │ name        │           │ - visual    │  Pipeline │             │      │
│   │ holding     │           │ - LOD rules │           │ 60fps       │      │
│   │ jurisdiction│           │ - hit rules │           │             │      │
│   └─────────────┘           │ - behaviors │           └─────────────┘      │
│                             └─────────────┘                                 │
│                                                                             │
│   Tokens are NOT data. Tokens are RENDER INSTRUCTIONS.                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Principle:** Every CBU entity maps to exactly one Token. The Token defines:
- **Visual:** What it looks like at different distances
- **Behavior:** What happens when you interact with it
- **Physics:** How it moves, animates, responds to camera
- **Hit Testing:** Whether/how it can be selected

---

## Part 1: Core Types

### 1.1 Token Definition

**File:** `crates/ob-poc-ui/src/tokens/types.rs`

```rust
//! Token type definitions for the CBU visualization pipeline
//!
//! Tokens bridge data entities to rendered visuals. Every entity
//! maps to a token which defines how it appears and behaves.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a token type (matches entity type codes)
pub type TokenTypeId = String;

/// Unique identifier for a token instance
pub type TokenId = String;

/// Complete definition of a token type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDefinition {
    /// Unique type identifier (e.g., "shareholder", "fund", "document")
    pub type_id: TokenTypeId,
    
    /// Human-readable label
    pub label: String,
    
    /// Category for grouping (e.g., "entity", "container", "edge")
    pub category: TokenCategory,
    
    /// Visual appearance definition
    pub visual: TokenVisual,
    
    /// Level-of-detail rules (distance → representation)
    pub lod_rules: Vec<LodRule>,
    
    /// Interaction behaviors
    pub interactions: InteractionRules,
    
    /// Detail view template (when fully expanded)
    pub detail_template: Option<DetailTemplate>,
    
    /// Whether this token type represents a container
    pub is_container: bool,
    
    /// For containers: what token type it contains
    pub contains_type: Option<TokenTypeId>,
}

/// Token category
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenCategory {
    /// Core business unit
    Cbu,
    /// Legal or natural person entity
    Entity,
    /// Container that holds other tokens (silo)
    Container,
    /// Item within a container
    ContainerItem,
    /// Product, service, or resource
    Product,
    /// Document or artifact
    Document,
    /// Relationship edge
    Edge,
    /// Grouping or cluster
    Group,
}

/// Visual appearance definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenVisual {
    /// Icon definition
    pub icon: IconDef,
    
    /// Base color (RGBA 0-1)
    pub base_color: [f32; 4],
    
    /// Highlight color when selected/hovered
    pub highlight_color: [f32; 4],
    
    /// Status color mapping (status_code → color)
    #[serde(default)]
    pub status_colors: HashMap<String, [f32; 4]>,
    
    /// Shape for 3D rendering
    pub shape: TokenShape,
    
    /// Default size (before LOD scaling)
    pub base_size: f32,
}

/// Icon definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconDef {
    /// Unicode glyph (for quick rendering)
    pub glyph: Option<String>,
    
    /// Icon name from icon set (e.g., "lucide:user")
    pub icon_name: Option<String>,
    
    /// Fallback text if icon unavailable
    pub fallback: String,
}

/// Shape for 3D rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenShape {
    /// 2D billboard (always faces camera)
    Billboard,
    /// 3D sphere
    Sphere,
    /// 3D box/cube
    Box,
    /// Cylinder (for containers/silos)
    Cylinder { open_top: bool },
    /// Custom mesh
    Mesh { mesh_id: String },
}
```

### 1.2 Level-of-Detail Rules

```rust
/// Level-of-detail rule - defines representation at a distance range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LodRule {
    /// Minimum camera distance for this LOD
    pub min_distance: f32,
    
    /// Maximum camera distance (use f32::INFINITY for "and beyond")
    pub max_distance: f32,
    
    /// What to render at this distance
    pub representation: LodRepresentation,
    
    /// Is hit-testing enabled at this LOD?
    pub hittable: bool,
    
    /// Label visibility at this LOD
    pub label_visibility: LabelVisibility,
    
    /// Scale factor applied at this LOD
    pub scale_factor: f32,
}

/// What to render at a given LOD
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LodRepresentation {
    /// Invisible (culled)
    Hidden,
    
    /// Single colored point
    Dot {
        /// Size in pixels (before distance scaling)
        size: f32,
    },
    
    /// Small icon (billboard sprite)
    Icon {
        /// Scale relative to base size
        scale: f32,
    },
    
    /// Card with icon and label
    Card {
        width: f32,
        height: f32,
        /// Show status indicator
        show_status: bool,
    },
    
    /// Full detail panel
    Detail,
    
    /// Container visualization (silo/tube)
    Container {
        /// Show count badge
        show_count: bool,
        /// Render child preview
        show_preview: bool,
    },
}

/// Label visibility options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelVisibility {
    /// Never show label
    Hidden,
    /// Show on hover only
    OnHover,
    /// Always show, but truncate
    Truncated { max_chars: usize },
    /// Always show full label
    Always,
}

impl LodRule {
    /// Check if a distance falls within this rule's range
    pub fn matches_distance(&self, distance: f32) -> bool {
        distance >= self.min_distance && distance < self.max_distance
    }
}
```

### 1.3 Interaction Rules

```rust
/// Interaction behavior definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionRules {
    /// Behavior when cursor hovers over token
    pub on_hover: HoverBehavior,
    
    /// Behavior on single click
    pub on_click: ClickBehavior,
    
    /// Behavior on double-click
    pub on_double_click: ClickBehavior,
    
    /// Behavior on right-click (context menu)
    pub context_menu: Option<ContextMenu>,
    
    /// Keyboard shortcuts when selected
    #[serde(default)]
    pub keyboard_shortcuts: Vec<KeyboardShortcut>,
    
    /// Drag behavior
    pub draggable: bool,
    
    /// Can receive drops
    pub drop_target: bool,
}

/// Hover behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HoverBehavior {
    /// No hover effect
    None,
    
    /// Visual highlight only
    Highlight,
    
    /// Show tooltip
    Tooltip {
        /// Template string with {field} placeholders
        template: String,
        /// Delay before showing (ms)
        delay_ms: u32,
    },
    
    /// Show preview panel
    Preview {
        /// Delay before showing (ms)
        delay_ms: u32,
    },
    
    /// Highlight connected tokens
    HighlightConnected,
}

/// Click behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickBehavior {
    /// No action
    None,
    
    /// Select this token
    Select,
    
    /// Toggle selection (multi-select)
    ToggleSelect,
    
    /// Expand inline (for containers)
    Expand,
    
    /// Drill down (navigate into, push to drill stack)
    DrillDown,
    
    /// Focus camera on this token
    Focus,
    
    /// Open detail panel
    OpenDetail,
    
    /// Navigate to related view
    Navigate { target: String },
    
    /// Custom action (handled by action system)
    Custom { action_id: String },
}

/// Context menu definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenu {
    pub items: Vec<ContextMenuItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenuItem {
    /// Unique action identifier
    pub action_id: String,
    /// Display label
    pub label: String,
    /// Icon (optional)
    pub icon: Option<String>,
    /// Keyboard shortcut hint
    pub shortcut: Option<String>,
    /// Condition for enabling (optional)
    pub enabled_when: Option<String>,
    /// Submenu (for nested menus)
    pub submenu: Option<Vec<ContextMenuItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardShortcut {
    /// Key combination (e.g., "Enter", "Ctrl+D", "Delete")
    pub key: String,
    /// Action to trigger
    pub action: ClickBehavior,
}
```

### 1.4 Detail Template

```rust
/// Template for rendering detail view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailTemplate {
    /// Sections in the detail view
    pub sections: Vec<DetailSection>,
    
    /// Actions available in detail view
    pub actions: Vec<DetailAction>,
    
    /// Related items to show
    pub related: Vec<RelatedSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailSection {
    /// Section title
    pub title: String,
    
    /// Fields to display
    pub fields: Vec<DetailField>,
    
    /// Collapsible?
    pub collapsible: bool,
    
    /// Initially collapsed?
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailField {
    /// Data key (path into entity data)
    pub key: String,
    
    /// Display label
    pub label: String,
    
    /// Format type
    pub format: FieldFormat,
    
    /// Condition for showing (optional)
    pub show_when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldFormat {
    Text,
    Number { decimals: u8 },
    Currency { symbol: String },
    Percent { decimals: u8 },
    Date { format: String },
    DateTime { format: String },
    StatusBadge,
    Link,
    EntityRef,
    List,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailAction {
    pub action_id: String,
    pub label: String,
    pub icon: Option<String>,
    pub style: ActionStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStyle {
    Primary,
    Secondary,
    Danger,
    Link,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedSection {
    /// Title for the related section
    pub title: String,
    /// Edge type to follow
    pub edge_type: String,
    /// Direction (outgoing, incoming, both)
    pub direction: EdgeDirection,
    /// Maximum items to show
    pub max_items: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeDirection {
    Outgoing,
    Incoming,
    Both,
}
```

### 1.5 Tasks - Core Types

- [ ] Create `crates/ob-poc-ui/src/tokens/` module
- [ ] Create `types.rs` with all type definitions
- [ ] Implement `TokenDefinition` struct
- [ ] Implement `LodRule` with distance matching
- [ ] Implement `InteractionRules` struct
- [ ] Implement `DetailTemplate` struct
- [ ] Add serde serialization/deserialization
- [ ] Unit tests for LOD distance matching

---

## Part 2: Token Registry

### 2.1 Registry Structure

**File:** `crates/ob-poc-ui/src/tokens/registry.rs`

```rust
//! Token Registry - manages token type definitions
//!
//! The registry is loaded from YAML config and provides
//! fast lookup of token definitions by type ID.

use std::collections::HashMap;
use std::sync::Arc;
use crate::tokens::types::*;

/// Registry of all token type definitions
pub struct TokenRegistry {
    /// Token definitions by type ID
    definitions: HashMap<TokenTypeId, Arc<TokenDefinition>>,
    
    /// Entity type → Token type mapping
    entity_type_map: HashMap<String, TokenTypeId>,
    
    /// Default token for unknown types
    default_token: Arc<TokenDefinition>,
}

impl TokenRegistry {
    /// Load registry from YAML configuration
    pub fn from_yaml(yaml_str: &str) -> Result<Self, TokenError> {
        let config: TokenConfig = serde_yaml::from_str(yaml_str)?;
        Self::from_config(config)
    }
    
    /// Load registry from config file path
    pub fn from_file(path: &std::path::Path) -> Result<Self, TokenError> {
        let yaml_str = std::fs::read_to_string(path)?;
        Self::from_yaml(&yaml_str)
    }
    
    /// Build registry from parsed config
    pub fn from_config(config: TokenConfig) -> Result<Self, TokenError> {
        let mut definitions = HashMap::new();
        let mut entity_type_map = HashMap::new();
        
        for def in config.tokens {
            let type_id = def.type_id.clone();
            
            // Build entity type mapping
            if let Some(ref entity_types) = config.entity_mappings.get(&type_id) {
                for entity_type in entity_types {
                    entity_type_map.insert(entity_type.clone(), type_id.clone());
                }
            }
            
            definitions.insert(type_id, Arc::new(def));
        }
        
        // Create default token for unknown types
        let default_token = Arc::new(TokenDefinition::default_unknown());
        
        Ok(Self {
            definitions,
            entity_type_map,
            default_token,
        })
    }
    
    /// Get token definition by type ID
    pub fn get(&self, type_id: &str) -> Arc<TokenDefinition> {
        self.definitions
            .get(type_id)
            .cloned()
            .unwrap_or_else(|| self.default_token.clone())
    }
    
    /// Get token definition for an entity type
    pub fn for_entity_type(&self, entity_type: &str) -> Arc<TokenDefinition> {
        if let Some(token_type) = self.entity_type_map.get(entity_type) {
            self.get(token_type)
        } else {
            self.default_token.clone()
        }
    }
    
    /// Get all container token types
    pub fn containers(&self) -> impl Iterator<Item = &Arc<TokenDefinition>> {
        self.definitions.values().filter(|d| d.is_container)
    }
    
    /// Get all token types in a category
    pub fn by_category(&self, category: TokenCategory) -> impl Iterator<Item = &Arc<TokenDefinition>> {
        self.definitions.values().filter(move |d| d.category == category)
    }
    
    /// Check if a type ID exists
    pub fn contains(&self, type_id: &str) -> bool {
        self.definitions.contains_key(type_id)
    }
    
    /// List all registered type IDs
    pub fn type_ids(&self) -> impl Iterator<Item = &TokenTypeId> {
        self.definitions.keys()
    }
}

/// Configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenConfig {
    /// Version for compatibility
    pub version: String,
    
    /// Token definitions
    pub tokens: Vec<TokenDefinition>,
    
    /// Entity type → Token type mappings
    #[serde(default)]
    pub entity_mappings: HashMap<TokenTypeId, Vec<String>>,
}

impl TokenDefinition {
    /// Create a default token for unknown types
    pub fn default_unknown() -> Self {
        Self {
            type_id: "_unknown".to_string(),
            label: "Unknown".to_string(),
            category: TokenCategory::Entity,
            visual: TokenVisual {
                icon: IconDef {
                    glyph: Some("?".to_string()),
                    icon_name: Some("lucide:help-circle".to_string()),
                    fallback: "?".to_string(),
                },
                base_color: [0.5, 0.5, 0.5, 1.0],
                highlight_color: [0.7, 0.7, 0.7, 1.0],
                status_colors: HashMap::new(),
                shape: TokenShape::Billboard,
                base_size: 30.0,
            },
            lod_rules: vec![
                LodRule {
                    min_distance: 0.0,
                    max_distance: f32::INFINITY,
                    representation: LodRepresentation::Icon { scale: 1.0 },
                    hittable: true,
                    label_visibility: LabelVisibility::OnHover,
                    scale_factor: 1.0,
                },
            ],
            interactions: InteractionRules {
                on_hover: HoverBehavior::Highlight,
                on_click: ClickBehavior::Select,
                on_double_click: ClickBehavior::OpenDetail,
                context_menu: None,
                keyboard_shortcuts: vec![],
                draggable: false,
                drop_target: false,
            },
            detail_template: None,
            is_container: false,
            contains_type: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Invalid token definition: {0}")]
    InvalidDefinition(String),
}
```

### 2.2 Tasks - Token Registry

- [ ] Create `registry.rs`
- [ ] Implement `TokenRegistry` struct
- [ ] Implement YAML loading
- [ ] Implement entity type mapping
- [ ] Implement default unknown token
- [ ] Add category/container queries
- [ ] Unit tests for registry lookup

---

## Part 3: Token Instance

### 3.1 Runtime Token

**File:** `crates/ob-poc-ui/src/tokens/instance.rs`

```rust
//! Token Instance - runtime representation of a token
//!
//! A TokenInstance is created for each entity in the scene.
//! It holds the entity data reference + current visual state.

use std::sync::Arc;
use crate::tokens::types::*;
use crate::animation::SpringVec3;

/// Runtime token instance
pub struct TokenInstance {
    /// Unique instance ID (usually matches entity ID)
    pub id: TokenId,
    
    /// Reference to token definition
    pub definition: Arc<TokenDefinition>,
    
    /// Reference to source entity data
    pub entity_data: Arc<serde_json::Value>,
    
    /// Current position in world space (animated)
    pub position: SpringVec3,
    
    /// Current scale (animated, includes LOD factor)
    pub scale: f32,
    
    /// Current opacity (animated, for fade in/out)
    pub opacity: f32,
    
    /// Current LOD level
    pub current_lod: usize,
    
    /// Visual state
    pub visual_state: TokenVisualState,
    
    /// Interaction state
    pub interaction_state: TokenInteractionState,
    
    /// Bounding box (for hit testing)
    pub bounds: BoundingBox,
    
    /// Parent token ID (for hierarchical containment)
    pub parent_id: Option<TokenId>,
    
    /// Child token IDs (for containers)
    pub child_ids: Vec<TokenId>,
    
    /// Is this token currently visible?
    pub visible: bool,
    
    /// Custom data for specialized rendering
    pub render_data: Option<RenderData>,
}

/// Visual state flags
#[derive(Debug, Clone, Default)]
pub struct TokenVisualState {
    /// Is token selected?
    pub selected: bool,
    
    /// Is cursor hovering?
    pub hovered: bool,
    
    /// Is token highlighted (e.g., search result)?
    pub highlighted: bool,
    
    /// Is token dimmed (e.g., not matching filter)?
    pub dimmed: bool,
    
    /// Is token expanded (for containers)?
    pub expanded: bool,
    
    /// Animation state (for pulsing, glowing, etc.)
    pub animation_phase: f32,
    
    /// Status code (for status-based coloring)
    pub status: Option<String>,
}

/// Interaction state
#[derive(Debug, Clone, Default)]
pub struct TokenInteractionState {
    /// Time cursor has been hovering (for delayed tooltips)
    pub hover_time: f32,
    
    /// Is context menu open?
    pub context_menu_open: bool,
    
    /// Is being dragged?
    pub dragging: bool,
    
    /// Drag offset from token center
    pub drag_offset: Option<[f32; 3]>,
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundingBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl BoundingBox {
    pub fn new(center: [f32; 3], half_extents: [f32; 3]) -> Self {
        Self {
            min: [
                center[0] - half_extents[0],
                center[1] - half_extents[1],
                center[2] - half_extents[2],
            ],
            max: [
                center[0] + half_extents[0],
                center[1] + half_extents[1],
                center[2] + half_extents[2],
            ],
        }
    }
    
    pub fn contains_point(&self, point: [f32; 3]) -> bool {
        point[0] >= self.min[0] && point[0] <= self.max[0] &&
        point[1] >= self.min[1] && point[1] <= self.max[1] &&
        point[2] >= self.min[2] && point[2] <= self.max[2]
    }
    
    pub fn center(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) / 2.0,
            (self.min[1] + self.max[1]) / 2.0,
            (self.min[2] + self.max[2]) / 2.0,
        ]
    }
}

/// Custom render data for specialized token types
#[derive(Debug, Clone)]
pub enum RenderData {
    /// Container/silo specific data
    Container {
        /// Child count (for badge)
        child_count: i64,
        /// Silo depth (for 3D rendering)
        silo_depth: f32,
        /// View depth (how far into silo camera is)
        view_depth: f32,
        /// Loaded child range
        loaded_range: (i64, i64),
    },
    
    /// Edge specific data
    Edge {
        /// Source token ID
        source_id: TokenId,
        /// Target token ID
        target_id: TokenId,
        /// Control points for curved edges
        control_points: Vec<[f32; 3]>,
        /// Edge weight (for thickness)
        weight: f32,
        /// Is edge bidirectional?
        bidirectional: bool,
    },
    
    /// Group/cluster specific data
    Group {
        /// Member token IDs
        member_ids: Vec<TokenId>,
        /// Convex hull points
        hull_points: Vec<[f32; 2]>,
    },
}

impl TokenInstance {
    /// Create a new token instance from entity data
    pub fn from_entity(
        id: TokenId,
        definition: Arc<TokenDefinition>,
        entity_data: Arc<serde_json::Value>,
        position: [f32; 3],
    ) -> Self {
        let base_size = definition.visual.base_size;
        
        Self {
            id,
            definition,
            entity_data,
            position: SpringVec3::new(position[0], position[1], position[2]),
            scale: 1.0,
            opacity: 1.0,
            current_lod: 0,
            visual_state: TokenVisualState::default(),
            interaction_state: TokenInteractionState::default(),
            bounds: BoundingBox::new(position, [base_size / 2.0; 3]),
            parent_id: None,
            child_ids: Vec::new(),
            visible: true,
            render_data: None,
        }
    }
    
    /// Update LOD based on camera distance
    pub fn update_lod(&mut self, camera_distance: f32) {
        for (i, rule) in self.definition.lod_rules.iter().enumerate() {
            if rule.matches_distance(camera_distance) {
                self.current_lod = i;
                self.scale = rule.scale_factor;
                return;
            }
        }
    }
    
    /// Get current LOD rule
    pub fn current_lod_rule(&self) -> &LodRule {
        &self.definition.lod_rules[self.current_lod]
    }
    
    /// Is this token hittable at current LOD?
    pub fn is_hittable(&self) -> bool {
        self.visible && self.current_lod_rule().hittable
    }
    
    /// Get display label (with truncation if needed)
    pub fn display_label(&self) -> Option<String> {
        match self.current_lod_rule().label_visibility {
            LabelVisibility::Hidden => None,
            LabelVisibility::OnHover if !self.visual_state.hovered => None,
            LabelVisibility::Truncated { max_chars } => {
                let label = self.get_label();
                if label.len() > max_chars {
                    Some(format!("{}...", &label[..max_chars.saturating_sub(3)]))
                } else {
                    Some(label)
                }
            }
            _ => Some(self.get_label()),
        }
    }
    
    /// Extract label from entity data
    fn get_label(&self) -> String {
        self.entity_data
            .get("name")
            .or_else(|| self.entity_data.get("label"))
            .and_then(|v| v.as_str())
            .unwrap_or(&self.id)
            .to_string()
    }
    
    /// Get current color (considering state)
    pub fn current_color(&self) -> [f32; 4] {
        if self.visual_state.selected || self.visual_state.hovered {
            self.definition.visual.highlight_color
        } else if self.visual_state.dimmed {
            let mut color = self.definition.visual.base_color;
            color[3] *= 0.3;
            color
        } else if let Some(ref status) = self.visual_state.status {
            self.definition.visual.status_colors
                .get(status)
                .copied()
                .unwrap_or(self.definition.visual.base_color)
        } else {
            self.definition.visual.base_color
        }
    }
    
    /// Update bounding box based on current position and scale
    pub fn update_bounds(&mut self) {
        let pos = self.position.get();
        let half_size = self.definition.visual.base_size * self.scale / 2.0;
        self.bounds = BoundingBox::new(
            [pos.0, pos.1, pos.2],
            [half_size, half_size, half_size],
        );
    }
}
```

### 3.2 Tasks - Token Instance

- [ ] Create `instance.rs`
- [ ] Implement `TokenInstance` struct
- [ ] Implement `TokenVisualState`
- [ ] Implement `TokenInteractionState`
- [ ] Implement `BoundingBox` with point containment
- [ ] Implement LOD update logic
- [ ] Implement label truncation
- [ ] Implement color state logic
- [ ] Unit tests for LOD transitions

---

## Part 4: Spatial Index

### 4.1 Spatial Index for Hit Testing

**File:** `crates/ob-poc-ui/src/tokens/spatial.rs`

```rust
//! Spatial Index for fast token lookup
//!
//! Uses a grid-based spatial hash for efficient:
//! - Hit testing (ray-cast, point query)
//! - Visibility culling (frustum query)
//! - Proximity queries (radius search)

use std::collections::{HashMap, HashSet};
use crate::tokens::instance::{TokenInstance, TokenId, BoundingBox};

/// Spatial hash grid for token lookup
pub struct SpatialIndex {
    /// Grid cell size
    cell_size: f32,
    
    /// Grid cells: cell_key → set of token IDs
    cells: HashMap<CellKey, HashSet<TokenId>>,
    
    /// Token ID → cell keys (for fast removal)
    token_cells: HashMap<TokenId, Vec<CellKey>>,
}

/// Grid cell key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellKey {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Ray for hit testing
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
}

/// Hit test result
#[derive(Debug, Clone)]
pub struct HitResult {
    pub token_id: TokenId,
    pub hit_point: [f32; 3],
    pub distance: f32,
}

/// View frustum for culling
#[derive(Debug, Clone)]
pub struct Frustum {
    /// Near plane distance
    pub near: f32,
    /// Far plane distance
    pub far: f32,
    /// Field of view (radians)
    pub fov: f32,
    /// Aspect ratio
    pub aspect: f32,
    /// Camera position
    pub camera_pos: [f32; 3],
    /// Camera forward direction
    pub camera_forward: [f32; 3],
}

impl SpatialIndex {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
            token_cells: HashMap::new(),
        }
    }
    
    /// Insert a token into the index
    pub fn insert(&mut self, token: &TokenInstance) {
        let cell_keys = self.get_cell_keys(&token.bounds);
        
        for key in &cell_keys {
            self.cells
                .entry(*key)
                .or_insert_with(HashSet::new)
                .insert(token.id.clone());
        }
        
        self.token_cells.insert(token.id.clone(), cell_keys);
    }
    
    /// Remove a token from the index
    pub fn remove(&mut self, token_id: &TokenId) {
        if let Some(cell_keys) = self.token_cells.remove(token_id) {
            for key in cell_keys {
                if let Some(cell) = self.cells.get_mut(&key) {
                    cell.remove(token_id);
                    if cell.is_empty() {
                        self.cells.remove(&key);
                    }
                }
            }
        }
    }
    
    /// Update a token's position in the index
    pub fn update(&mut self, token: &TokenInstance) {
        self.remove(&token.id);
        self.insert(token);
    }
    
    /// Ray-cast to find the nearest hit token
    pub fn ray_cast(&self, ray: &Ray, tokens: &HashMap<TokenId, TokenInstance>) -> Option<HitResult> {
        let mut best_hit: Option<HitResult> = None;
        let mut checked: HashSet<TokenId> = HashSet::new();
        
        // Walk ray through grid cells
        let mut t = 0.0f32;
        let max_distance = 10000.0; // Maximum ray distance
        
        while t < max_distance {
            let point = [
                ray.origin[0] + ray.direction[0] * t,
                ray.origin[1] + ray.direction[1] * t,
                ray.origin[2] + ray.direction[2] * t,
            ];
            
            let cell_key = self.point_to_cell(point);
            
            if let Some(token_ids) = self.cells.get(&cell_key) {
                for token_id in token_ids {
                    if checked.contains(token_id) {
                        continue;
                    }
                    checked.insert(token_id.clone());
                    
                    if let Some(token) = tokens.get(token_id) {
                        if !token.is_hittable() {
                            continue;
                        }
                        
                        if let Some(hit_point) = self.ray_box_intersection(ray, &token.bounds) {
                            let distance = self.distance(ray.origin, hit_point);
                            
                            if best_hit.is_none() || distance < best_hit.as_ref().unwrap().distance {
                                best_hit = Some(HitResult {
                                    token_id: token_id.clone(),
                                    hit_point,
                                    distance,
                                });
                            }
                        }
                    }
                }
            }
            
            // Step to next cell
            t += self.cell_size * 0.5;
        }
        
        best_hit
    }
    
    /// Query tokens within a radius
    pub fn query_radius(&self, center: [f32; 3], radius: f32) -> Vec<TokenId> {
        let mut results = Vec::new();
        let mut checked: HashSet<TokenId> = HashSet::new();
        
        // Calculate cell range
        let min_cell = self.point_to_cell([
            center[0] - radius,
            center[1] - radius,
            center[2] - radius,
        ]);
        let max_cell = self.point_to_cell([
            center[0] + radius,
            center[1] + radius,
            center[2] + radius,
        ]);
        
        // Check all cells in range
        for x in min_cell.x..=max_cell.x {
            for y in min_cell.y..=max_cell.y {
                for z in min_cell.z..=max_cell.z {
                    let key = CellKey { x, y, z };
                    if let Some(token_ids) = self.cells.get(&key) {
                        for token_id in token_ids {
                            if !checked.contains(token_id) {
                                checked.insert(token_id.clone());
                                results.push(token_id.clone());
                            }
                        }
                    }
                }
            }
        }
        
        results
    }
    
    /// Query tokens visible in frustum
    pub fn query_frustum(&self, frustum: &Frustum) -> Vec<TokenId> {
        // Simplified: return all tokens within frustum bounds
        // Full implementation would check frustum planes
        let radius = frustum.far;
        self.query_radius(frustum.camera_pos, radius)
    }
    
    /// Get cell key for a point
    fn point_to_cell(&self, point: [f32; 3]) -> CellKey {
        CellKey {
            x: (point[0] / self.cell_size).floor() as i32,
            y: (point[1] / self.cell_size).floor() as i32,
            z: (point[2] / self.cell_size).floor() as i32,
        }
    }
    
    /// Get all cell keys that a bounding box overlaps
    fn get_cell_keys(&self, bounds: &BoundingBox) -> Vec<CellKey> {
        let min_cell = self.point_to_cell(bounds.min);
        let max_cell = self.point_to_cell(bounds.max);
        
        let mut keys = Vec::new();
        for x in min_cell.x..=max_cell.x {
            for y in min_cell.y..=max_cell.y {
                for z in min_cell.z..=max_cell.z {
                    keys.push(CellKey { x, y, z });
                }
            }
        }
        keys
    }
    
    /// Ray-box intersection test
    fn ray_box_intersection(&self, ray: &Ray, bounds: &BoundingBox) -> Option<[f32; 3]> {
        let inv_dir = [
            1.0 / ray.direction[0],
            1.0 / ray.direction[1],
            1.0 / ray.direction[2],
        ];
        
        let t1 = (bounds.min[0] - ray.origin[0]) * inv_dir[0];
        let t2 = (bounds.max[0] - ray.origin[0]) * inv_dir[0];
        let t3 = (bounds.min[1] - ray.origin[1]) * inv_dir[1];
        let t4 = (bounds.max[1] - ray.origin[1]) * inv_dir[1];
        let t5 = (bounds.min[2] - ray.origin[2]) * inv_dir[2];
        let t6 = (bounds.max[2] - ray.origin[2]) * inv_dir[2];
        
        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));
        
        if tmax >= tmin && tmax >= 0.0 {
            let t = if tmin >= 0.0 { tmin } else { tmax };
            Some([
                ray.origin[0] + ray.direction[0] * t,
                ray.origin[1] + ray.direction[1] * t,
                ray.origin[2] + ray.direction[2] * t,
            ])
        } else {
            None
        }
    }
    
    fn distance(&self, a: [f32; 3], b: [f32; 3]) -> f32 {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let dz = b[2] - a[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}
```

### 4.2 Tasks - Spatial Index

- [ ] Create `spatial.rs`
- [ ] Implement `SpatialIndex` with grid hashing
- [ ] Implement `insert`, `remove`, `update`
- [ ] Implement `ray_cast` for hit testing
- [ ] Implement `query_radius` for proximity
- [ ] Implement `query_frustum` for visibility culling
- [ ] Implement ray-box intersection
- [ ] Unit tests for spatial queries
- [ ] Benchmark with 10,000 tokens

---

## Part 5: LOD Manager

### 5.1 LOD Update System

**File:** `crates/ob-poc-ui/src/tokens/lod.rs`

```rust
//! LOD Manager - updates token representations based on camera distance
//!
//! Efficiently batches LOD updates and manages transitions.

use std::collections::HashMap;
use crate::tokens::instance::{TokenInstance, TokenId};
use crate::animation::SpringF32;

/// LOD Manager
pub struct LodManager {
    /// Camera position for distance calculations
    camera_position: [f32; 3],
    
    /// LOD transition animations
    transitions: HashMap<TokenId, LodTransition>,
    
    /// Hysteresis thresholds (prevent LOD thrashing)
    hysteresis_factor: f32,
}

/// LOD transition animation
struct LodTransition {
    from_lod: usize,
    to_lod: usize,
    progress: SpringF32,
}

impl LodManager {
    pub fn new() -> Self {
        Self {
            camera_position: [0.0, 0.0, 0.0],
            transitions: HashMap::new(),
            hysteresis_factor: 1.1, // 10% hysteresis
        }
    }
    
    /// Update camera position
    pub fn set_camera_position(&mut self, position: [f32; 3]) {
        self.camera_position = position;
    }
    
    /// Update LOD for a single token
    pub fn update_token(&mut self, token: &mut TokenInstance) {
        let distance = self.distance_to_token(token);
        
        // Find appropriate LOD level
        let mut new_lod = 0;
        for (i, rule) in token.definition.lod_rules.iter().enumerate() {
            if rule.matches_distance(distance) {
                new_lod = i;
                break;
            }
        }
        
        // Apply hysteresis to prevent thrashing
        if new_lod != token.current_lod {
            let current_rule = &token.definition.lod_rules[token.current_lod];
            let should_transition = if new_lod > token.current_lod {
                // Moving to lower detail - use hysteresis
                distance > current_rule.max_distance * self.hysteresis_factor
            } else {
                // Moving to higher detail - use hysteresis
                distance < current_rule.min_distance / self.hysteresis_factor
            };
            
            if should_transition {
                self.start_transition(&token.id, token.current_lod, new_lod);
                token.current_lod = new_lod;
                token.scale = token.definition.lod_rules[new_lod].scale_factor;
            }
        }
    }
    
    /// Update LOD for all tokens
    pub fn update_all(&mut self, tokens: &mut HashMap<TokenId, TokenInstance>) {
        for token in tokens.values_mut() {
            self.update_token(token);
        }
        
        // Update transition animations
        self.update_transitions();
    }
    
    /// Start a LOD transition animation
    fn start_transition(&mut self, token_id: &TokenId, from: usize, to: usize) {
        let mut progress = SpringF32::new(0.0);
        progress.set_target(1.0);
        
        self.transitions.insert(token_id.clone(), LodTransition {
            from_lod: from,
            to_lod: to,
            progress,
        });
    }
    
    /// Update transition animations
    fn update_transitions(&mut self) {
        self.transitions.retain(|_, transition| {
            transition.progress.is_animating()
        });
    }
    
    /// Get transition progress for a token (0.0 = from_lod, 1.0 = to_lod)
    pub fn get_transition_progress(&self, token_id: &TokenId) -> Option<f32> {
        self.transitions.get(token_id).map(|t| t.progress.get())
    }
    
    /// Calculate distance from camera to token
    fn distance_to_token(&self, token: &TokenInstance) -> f32 {
        let pos = token.position.get();
        let dx = pos.0 - self.camera_position[0];
        let dy = pos.1 - self.camera_position[1];
        let dz = pos.2 - self.camera_position[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
    
    /// Tick animations
    pub fn tick(&mut self, dt: f32) {
        for transition in self.transitions.values_mut() {
            transition.progress.tick(dt);
        }
    }
}
```

### 5.2 Tasks - LOD Manager

- [ ] Create `lod.rs`
- [ ] Implement `LodManager`
- [ ] Implement hysteresis to prevent LOD thrashing
- [ ] Implement LOD transition animations
- [ ] Implement batch update for all tokens
- [ ] Unit tests for LOD transitions

---

## Part 6: Instance Batching

### 6.1 Instance Buffer Manager

**File:** `crates/ob-poc-ui/src/tokens/instancing.rs`

```rust
//! Instance Buffer Manager - GPU batching for efficient rendering
//!
//! Groups tokens by type and LOD for instanced rendering.

use std::collections::HashMap;
use crate::tokens::types::*;
use crate::tokens::instance::{TokenInstance, TokenId};

/// Instance data for GPU upload
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct InstanceData {
    /// Position (xyz) + scale (w)
    pub position_scale: [f32; 4],
    /// Color (rgba)
    pub color: [f32; 4],
    /// Flags: selected(1), hovered(2), highlighted(4), dimmed(8)
    pub flags: u32,
    /// UV offset for texture atlas (if using)
    pub uv_offset: [f32; 2],
    /// Padding for alignment
    pub _padding: [f32; 1],
}

/// Instance batch for a specific token type + LOD
pub struct InstanceBatch {
    /// Token type ID
    pub token_type: TokenTypeId,
    
    /// LOD level
    pub lod_level: usize,
    
    /// Representation at this LOD
    pub representation: LodRepresentation,
    
    /// Instance data array
    pub instances: Vec<InstanceData>,
    
    /// Token ID → instance index mapping
    pub token_indices: HashMap<TokenId, usize>,
    
    /// Dirty flag (needs re-upload)
    pub dirty: bool,
}

/// Manages all instance batches
pub struct InstanceManager {
    /// Batches keyed by (type, lod)
    batches: HashMap<(TokenTypeId, usize), InstanceBatch>,
}

impl InstanceManager {
    pub fn new() -> Self {
        Self {
            batches: HashMap::new(),
        }
    }
    
    /// Update instances from token collection
    pub fn update(&mut self, tokens: &HashMap<TokenId, TokenInstance>) {
        // Clear all batches
        for batch in self.batches.values_mut() {
            batch.instances.clear();
            batch.token_indices.clear();
            batch.dirty = true;
        }
        
        // Rebuild from tokens
        for token in tokens.values() {
            if !token.visible {
                continue;
            }
            
            let key = (token.definition.type_id.clone(), token.current_lod);
            
            // Get or create batch
            let batch = self.batches.entry(key.clone()).or_insert_with(|| {
                InstanceBatch {
                    token_type: token.definition.type_id.clone(),
                    lod_level: token.current_lod,
                    representation: token.current_lod_rule().representation.clone(),
                    instances: Vec::new(),
                    token_indices: HashMap::new(),
                    dirty: true,
                }
            });
            
            // Add instance data
            let pos = token.position.get();
            let color = token.current_color();
            
            let instance = InstanceData {
                position_scale: [pos.0, pos.1, pos.2, token.scale],
                color,
                flags: self.encode_flags(&token.visual_state),
                uv_offset: [0.0, 0.0], // TODO: texture atlas
                _padding: [0.0],
            };
            
            let index = batch.instances.len();
            batch.instances.push(instance);
            batch.token_indices.insert(token.id.clone(), index);
        }
        
        // Remove empty batches
        self.batches.retain(|_, batch| !batch.instances.is_empty());
    }
    
    /// Get all batches for rendering
    pub fn batches(&self) -> impl Iterator<Item = &InstanceBatch> {
        self.batches.values()
    }
    
    /// Get batches that need GPU upload
    pub fn dirty_batches(&mut self) -> impl Iterator<Item = &mut InstanceBatch> {
        self.batches.values_mut().filter(|b| b.dirty)
    }
    
    /// Mark batch as uploaded
    pub fn mark_clean(&mut self, token_type: &TokenTypeId, lod: usize) {
        if let Some(batch) = self.batches.get_mut(&(token_type.clone(), lod)) {
            batch.dirty = false;
        }
    }
    
    /// Encode visual state flags
    fn encode_flags(&self, state: &crate::tokens::instance::TokenVisualState) -> u32 {
        let mut flags = 0u32;
        if state.selected { flags |= 1; }
        if state.hovered { flags |= 2; }
        if state.highlighted { flags |= 4; }
        if state.dimmed { flags |= 8; }
        if state.expanded { flags |= 16; }
        flags
    }
}
```

### 6.2 Tasks - Instance Batching

- [ ] Create `instancing.rs`
- [ ] Implement `InstanceData` struct (GPU-compatible)
- [ ] Implement `InstanceBatch` struct
- [ ] Implement `InstanceManager`
- [ ] Implement batch grouping by (type, lod)
- [ ] Implement dirty tracking for GPU upload
- [ ] Benchmark with 10,000 instances

---

## Part 7: Token YAML Configuration

### 7.1 Configuration File

**File:** `rust/config/visualization/tokens.yaml`

```yaml
# Token Type Definitions
# ======================
# Defines how each entity type renders and behaves in the visualization.

version: "1.0"

# Entity type → Token type mappings
entity_mappings:
  shareholder:
    - investor_holding
  fund:
    - fund
    - umbrella_fund
  share_class:
    - share_class
  entity:
    - proper_person
    - limited_company
    - partnership
    - trust
  document:
    - document
    - certificate
  product:
    - custody
    - fund_accounting
    - trading
  service:
    - safekeeping
    - settlement
    - nav_calculation
  resource:
    - vault
    - ssi
    - pricing_feed

# Token definitions
tokens:
  # ==========================================================================
  # SHAREHOLDER (Container Item)
  # ==========================================================================
  - type_id: shareholder
    label: "Shareholder"
    category: container_item
    is_container: false
    
    visual:
      icon:
        glyph: "👤"
        icon_name: "lucide:user"
        fallback: "U"
      base_color: [0.4, 0.6, 0.9, 1.0]
      highlight_color: [0.6, 0.8, 1.0, 1.0]
      status_colors:
        VERIFIED: [0.3, 0.8, 0.4, 1.0]
        PENDING: [0.9, 0.7, 0.2, 1.0]
        BLOCKED: [0.9, 0.3, 0.3, 1.0]
      shape: billboard
      base_size: 30.0
    
    lod_rules:
      # Detail view (very close)
      - min_distance: 0
        max_distance: 50
        representation: detail
        hittable: true
        label_visibility: always
        scale_factor: 1.0
        
      # Card view (close)
      - min_distance: 50
        max_distance: 150
        representation:
          card:
            width: 100
            height: 50
            show_status: true
        hittable: true
        label_visibility:
          truncated:
            max_chars: 15
        scale_factor: 0.8
        
      # Icon view (medium)
      - min_distance: 150
        max_distance: 400
        representation:
          icon:
            scale: 1.0
        hittable: true
        label_visibility: on_hover
        scale_factor: 0.6
        
      # Dot view (far)
      - min_distance: 400
        max_distance: .inf
        representation:
          dot:
            size: 4.0
        hittable: false
        label_visibility: hidden
        scale_factor: 0.3
    
    interactions:
      on_hover:
        tooltip:
          template: |
            {name}
            {jurisdiction} | {investor_type}
            ${holding_value:,.0f} ({percentage:.2f}%)
          delay_ms: 300
      on_click: select
      on_double_click: open_detail
      context_menu:
        items:
          - action_id: view_entity
            label: "View Entity Profile"
            icon: "lucide:user"
          - action_id: view_documents
            label: "View Documents"
            icon: "lucide:folder"
          - action_id: view_kyc
            label: "KYC History"
            icon: "lucide:shield"
      draggable: false
      drop_target: false
    
    detail_template:
      sections:
        - title: "Holding Details"
          collapsible: false
          fields:
            - key: holding_value
              label: "Value"
              format:
                currency:
                  symbol: "$"
            - key: units
              label: "Units"
              format:
                number:
                  decimals: 2
            - key: percentage
              label: "% of Class"
              format:
                percent:
                  decimals: 2
            - key: acquisition_date
              label: "Acquired"
              format:
                date:
                  format: "%Y-%m-%d"
        - title: "Investor Profile"
          collapsible: true
          fields:
            - key: investor_type
              label: "Type"
              format: text
            - key: jurisdiction
              label: "Jurisdiction"
              format: text
            - key: kyc_status
              label: "KYC Status"
              format: status_badge
      actions:
        - action_id: view_entity
          label: "View Entity"
          style: primary
        - action_id: view_documents
          label: "Documents"
          style: secondary

  # ==========================================================================
  # SHARE CLASS (Container)
  # ==========================================================================
  - type_id: share_class
    label: "Share Class"
    category: container
    is_container: true
    contains_type: shareholder
    
    visual:
      icon:
        glyph: "📊"
        icon_name: "lucide:bar-chart"
        fallback: "SC"
      base_color: [0.3, 0.7, 0.5, 1.0]
      highlight_color: [0.5, 0.9, 0.7, 1.0]
      status_colors:
        ACTIVE: [0.3, 0.8, 0.4, 1.0]
        SOFT_CLOSED: [0.9, 0.7, 0.2, 1.0]
        HARD_CLOSED: [0.9, 0.3, 0.3, 1.0]
      shape:
        cylinder:
          open_top: true
      base_size: 60.0
    
    lod_rules:
      # Container detail (close - can dive in)
      - min_distance: 0
        max_distance: 100
        representation:
          container:
            show_count: true
            show_preview: true
        hittable: true
        label_visibility: always
        scale_factor: 1.0
        
      # Container icon (medium)
      - min_distance: 100
        max_distance: 300
        representation:
          container:
            show_count: true
            show_preview: false
        hittable: true
        label_visibility:
          truncated:
            max_chars: 10
        scale_factor: 0.7
        
      # Icon only (far)
      - min_distance: 300
        max_distance: .inf
        representation:
          icon:
            scale: 0.5
        hittable: true
        label_visibility: on_hover
        scale_factor: 0.4
    
    interactions:
      on_hover:
        tooltip:
          template: |
            {share_class_code} ({currency})
            {child_count:,} investors
            AUM: ${aggregates.total_aum:,.0f}
          delay_ms: 200
      on_click: select
      on_double_click: drill_down
      context_menu:
        items:
          - action_id: expand
            label: "Expand Here"
            icon: "lucide:maximize"
          - action_id: drill_down
            label: "Dive In"
            icon: "lucide:arrow-down"
          - action_id: view_summary
            label: "View Summary"
            icon: "lucide:pie-chart"
      draggable: false
      drop_target: false

  # ==========================================================================
  # FUND (Container of Share Classes)
  # ==========================================================================
  - type_id: fund
    label: "Fund"
    category: container
    is_container: true
    contains_type: share_class
    
    visual:
      icon:
        glyph: "🏛️"
        icon_name: "lucide:landmark"
        fallback: "F"
      base_color: [0.9, 0.7, 0.3, 1.0]
      highlight_color: [1.0, 0.85, 0.5, 1.0]
      shape:
        cylinder:
          open_top: true
      base_size: 80.0
    
    lod_rules:
      - min_distance: 0
        max_distance: 200
        representation:
          container:
            show_count: true
            show_preview: true
        hittable: true
        label_visibility: always
        scale_factor: 1.0
        
      - min_distance: 200
        max_distance: .inf
        representation:
          icon:
            scale: 0.6
        hittable: true
        label_visibility: on_hover
        scale_factor: 0.5
    
    interactions:
      on_hover: highlight_connected
      on_click: select
      on_double_click: drill_down

  # ==========================================================================
  # ENTITY (Person/Company)
  # ==========================================================================
  - type_id: entity
    label: "Entity"
    category: entity
    is_container: false
    
    visual:
      icon:
        glyph: "🏢"
        icon_name: "lucide:building"
        fallback: "E"
      base_color: [0.5, 0.5, 0.7, 1.0]
      highlight_color: [0.7, 0.7, 0.9, 1.0]
      status_colors:
        ACTIVE: [0.3, 0.8, 0.4, 1.0]
        INACTIVE: [0.5, 0.5, 0.5, 1.0]
      shape: billboard
      base_size: 50.0
    
    lod_rules:
      - min_distance: 0
        max_distance: 100
        representation:
          card:
            width: 120
            height: 60
            show_status: true
        hittable: true
        label_visibility: always
        scale_factor: 1.0
        
      - min_distance: 100
        max_distance: 300
        representation:
          icon:
            scale: 1.0
        hittable: true
        label_visibility: on_hover
        scale_factor: 0.7
        
      - min_distance: 300
        max_distance: .inf
        representation:
          dot:
            size: 6.0
        hittable: false
        label_visibility: hidden
        scale_factor: 0.3
    
    interactions:
      on_hover:
        tooltip:
          template: |
            {name}
            {entity_type} | {jurisdiction}
          delay_ms: 300
      on_click: select
      on_double_click: open_detail

  # ==========================================================================
  # OWNERSHIP EDGE
  # ==========================================================================
  - type_id: ownership_edge
    label: "Ownership"
    category: edge
    is_container: false
    
    visual:
      base_color: [0.4, 0.4, 0.4, 1.0]
      highlight_color: [0.8, 0.6, 0.2, 1.0]
      status_colors:
        proven: [0.3, 0.8, 0.4, 1.0]
        alleged: [0.5, 0.5, 0.5, 1.0]
        disputed: [0.9, 0.3, 0.3, 1.0]
      shape: billboard  # N/A for edges
      base_size: 2.0  # Line width
    
    lod_rules:
      - min_distance: 0
        max_distance: 200
        representation:
          card:
            width: 40
            height: 20
            show_status: true
        hittable: true
        label_visibility: always
        scale_factor: 1.0
        
      - min_distance: 200
        max_distance: .inf
        representation: hidden
        hittable: false
        label_visibility: hidden
        scale_factor: 0.5
    
    interactions:
      on_hover:
        tooltip:
          template: |
            {source_name} → {target_name}
            {percentage:.1f}% ownership
            Status: {verification_status}
          delay_ms: 200
      on_click: select
      on_double_click: none
```

### 7.2 Tasks - Configuration

- [ ] Create `rust/config/visualization/tokens.yaml`
- [ ] Define shareholder token
- [ ] Define share_class token (container)
- [ ] Define fund token (container)
- [ ] Define entity token (multiple subtypes)
- [ ] Define ownership_edge token
- [ ] Define product/service/resource tokens
- [ ] Add entity_mappings section
- [ ] Validate YAML loads correctly

---

## Part 8: Integration

### 8.1 Token System Facade

**File:** `crates/ob-poc-ui/src/tokens/mod.rs`

```rust
//! Token System - Entity-to-Render Pipeline
//!
//! The token system bridges data entities to rendered visuals.

pub mod types;
pub mod registry;
pub mod instance;
pub mod spatial;
pub mod lod;
pub mod instancing;

use std::collections::HashMap;
use std::sync::Arc;

pub use types::*;
pub use registry::TokenRegistry;
pub use instance::{TokenInstance, TokenId};
pub use spatial::{SpatialIndex, Ray, HitResult};
pub use lod::LodManager;
pub use instancing::InstanceManager;

/// Complete token system
pub struct TokenSystem {
    /// Token type registry
    pub registry: TokenRegistry,
    
    /// All token instances
    pub tokens: HashMap<TokenId, TokenInstance>,
    
    /// Spatial index for hit testing
    pub spatial: SpatialIndex,
    
    /// LOD manager
    pub lod: LodManager,
    
    /// Instance buffer manager
    pub instances: InstanceManager,
}

impl TokenSystem {
    /// Create a new token system
    pub fn new(registry: TokenRegistry) -> Self {
        Self {
            registry,
            tokens: HashMap::new(),
            spatial: SpatialIndex::new(100.0), // 100 unit grid cells
            lod: LodManager::new(),
            instances: InstanceManager::new(),
        }
    }
    
    /// Load token system from config file
    pub fn from_config_file(path: &std::path::Path) -> Result<Self, registry::TokenError> {
        let registry = TokenRegistry::from_file(path)?;
        Ok(Self::new(registry))
    }
    
    /// Add an entity to the token system
    pub fn add_entity(
        &mut self,
        entity_id: &str,
        entity_type: &str,
        entity_data: serde_json::Value,
        position: [f32; 3],
    ) {
        let definition = self.registry.for_entity_type(entity_type);
        let token = TokenInstance::from_entity(
            entity_id.to_string(),
            definition,
            Arc::new(entity_data),
            position,
        );
        
        self.spatial.insert(&token);
        self.tokens.insert(entity_id.to_string(), token);
    }
    
    /// Remove an entity from the token system
    pub fn remove_entity(&mut self, entity_id: &str) {
        self.spatial.remove(&entity_id.to_string());
        self.tokens.remove(entity_id);
    }
    
    /// Update camera position (for LOD calculations)
    pub fn set_camera_position(&mut self, position: [f32; 3]) {
        self.lod.set_camera_position(position);
    }
    
    /// Ray-cast for hit testing
    pub fn ray_cast(&self, ray: &Ray) -> Option<HitResult> {
        self.spatial.ray_cast(ray, &self.tokens)
    }
    
    /// Get token by ID
    pub fn get_token(&self, token_id: &str) -> Option<&TokenInstance> {
        self.tokens.get(token_id)
    }
    
    /// Get mutable token by ID
    pub fn get_token_mut(&mut self, token_id: &str) -> Option<&mut TokenInstance> {
        self.tokens.get_mut(token_id)
    }
    
    /// Update every frame
    pub fn tick(&mut self, dt: f32) {
        // Update animations
        for token in self.tokens.values_mut() {
            token.position.tick(dt);
        }
        
        // Update LOD
        self.lod.tick(dt);
        self.lod.update_all(&mut self.tokens);
        
        // Rebuild instance buffers
        self.instances.update(&self.tokens);
    }
    
    /// Select a token
    pub fn select(&mut self, token_id: &str) {
        // Deselect all
        for token in self.tokens.values_mut() {
            token.visual_state.selected = false;
        }
        
        // Select target
        if let Some(token) = self.tokens.get_mut(token_id) {
            token.visual_state.selected = true;
        }
    }
    
    /// Set hover state
    pub fn set_hovered(&mut self, token_id: Option<&str>) {
        for token in self.tokens.values_mut() {
            token.visual_state.hovered = false;
            token.interaction_state.hover_time = 0.0;
        }
        
        if let Some(id) = token_id {
            if let Some(token) = self.tokens.get_mut(id) {
                token.visual_state.hovered = true;
            }
        }
    }
}
```

### 8.2 Tasks - Integration

- [ ] Create `mod.rs` with public exports
- [ ] Implement `TokenSystem` facade
- [ ] Implement `add_entity` / `remove_entity`
- [ ] Implement `ray_cast` for hit testing
- [ ] Implement `tick` for frame updates
- [ ] Implement selection/hover state management
- [ ] Integration test: load config → add entities → ray-cast → verify

---

## Summary: Complete Token System

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TOKEN SYSTEM COMPONENTS                                                    │
│  ═══════════════════════                                                    │
│                                                                             │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                   │
│  │   types.rs  │     │ registry.rs │     │ instance.rs │                   │
│  │             │     │             │     │             │                   │
│  │ TokenDef    │────▶│ TokenReg    │────▶│TokenInstance│                   │
│  │ LodRule     │     │ YAML load   │     │ Visual state│                   │
│  │ Interactions│     │ Type lookup │     │ Bounds      │                   │
│  └─────────────┘     └─────────────┘     └──────┬──────┘                   │
│                                                  │                          │
│                                                  ▼                          │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                   │
│  │  spatial.rs │◀────│   lod.rs    │────▶│instancing.rs│                   │
│  │             │     │             │     │             │                   │
│  │ Grid hash   │     │ LOD manager │     │ GPU batches │                   │
│  │ Ray-cast    │     │ Hysteresis  │     │ Instance buf│                   │
│  │ Hit test    │     │ Transitions │     │ Dirty track │                   │
│  └─────────────┘     └─────────────┘     └─────────────┘                   │
│                                                                             │
│                              │                                              │
│                              ▼                                              │
│                      ┌─────────────┐                                       │
│                      │   mod.rs    │                                       │
│                      │             │                                       │
│                      │TokenSystem  │ ← Facade for all components           │
│                      │  tick()     │                                       │
│                      │  ray_cast() │                                       │
│                      │  select()   │                                       │
│                      └─────────────┘                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Order

1. **Part 1: Core Types** - TokenDefinition, LodRule, InteractionRules
2. **Part 2: Token Registry** - YAML loading, lookup
3. **Part 3: Token Instance** - Runtime representation
4. **Part 7: YAML Config** - Define actual token types
5. **Part 4: Spatial Index** - Hit testing
6. **Part 5: LOD Manager** - Distance-based updates
7. **Part 6: Instance Batching** - GPU efficiency
8. **Part 8: Integration** - TokenSystem facade

---

## Success Criteria

- [ ] YAML config loads without errors
- [ ] Entity types correctly map to token types
- [ ] LOD transitions are smooth (no flickering)
- [ ] Ray-cast finds correct token
- [ ] 10,000 tokens render at 60fps
- [ ] Hover/select states update correctly
- [ ] Container tokens can drill down
- [ ] Detail templates render correctly

---

## References

- ECS pattern: https://en.wikipedia.org/wiki/Entity_component_system
- Spatial hashing: https://www.gamedev.net/tutorials/programming/general-and-gameplay-programming/spatial-hashing-r2697/
- GPU instancing: https://learnopengl.com/Advanced-OpenGL/Instancing
- Game LOD: https://docs.unrealengine.com/en-US/RenderingAndGraphics/Visibility-Culling/VisibilityCulling/

---

*Token System: The bridge between data and visuals.*
