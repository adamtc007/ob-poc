# TODO: CBU Token System (Slim)

## ⛔ MANDATORY FIRST STEP

**Read these files before starting:**
- `/EGUI-RULES.md` - UI patterns and constraints
- `/rust/src/graph/types.rs` - Current graph model
- `/rust/config/ontology/entity_taxonomy.yaml` - Entity definitions

**Prerequisite:** Complete `TODO-CBU-CONTAINERS.md` first. This TODO enhances 
the container system with YAML-driven configuration.

---

## Overview

The Token System provides a **configuration layer** between data entities and 
their visual representation. Instead of hardcoding icons, colors, and behaviors 
in Rust, they're defined in YAML and looked up at runtime.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  WITHOUT TOKENS                         WITH TOKENS                         │
│  ═══════════════                        ═══════════════                     │
│                                                                             │
│  match entity_type {                    let token = registry.get(type);    │
│    "shareholder" => Color::BLUE,        token.visual.base_color            │
│    "fund" => Color::GOLD,                                                   │
│    _ => Color::GRAY,                    // One lookup, all config from YAML│
│  }                                                                          │
│                                                                             │
│  Hardcoded in Rust                      Configurable in YAML               │
│  Change = recompile                     Change = reload config             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**This is the 2D/egui version.** No 3D, no GPU instancing, no wgpu.

---

## Part 1: Core Types

### 1.1 Token Definition

**File:** `rust/src/tokens/types.rs` (new file)

```rust
//! Token type definitions
//!
//! Tokens bridge data entities to visual representation.
//! Definitions are loaded from YAML configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a token type (matches entity type codes)
pub type TokenTypeId = String;

/// Complete definition of a token type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDefinition {
    /// Unique type identifier (e.g., "shareholder", "fund", "document")
    pub type_id: TokenTypeId,
    
    /// Human-readable label
    pub label: String,
    
    /// Category for grouping
    pub category: TokenCategory,
    
    /// Visual appearance
    pub visual: TokenVisual,
    
    /// Interaction behaviors
    pub interactions: InteractionRules,
    
    /// Detail view template
    #[serde(default)]
    pub detail_template: Option<DetailTemplate>,
    
    /// Is this a container type?
    #[serde(default)]
    pub is_container: bool,
    
    /// For containers: EntityGateway nickname for browsing
    #[serde(default)]
    pub browse_nickname: Option<String>,
}

/// Token category
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TokenCategory {
    #[default]
    Entity,
    Container,
    ContainerItem,
    Product,
    Document,
    Edge,
}

/// Visual appearance definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenVisual {
    /// Icon definition
    pub icon: IconDef,
    
    /// Base color [r, g, b, a] 0-255
    pub base_color: [u8; 4],
    
    /// Highlight color when selected/hovered
    pub highlight_color: [u8; 4],
    
    /// Status → color mapping
    #[serde(default)]
    pub status_colors: HashMap<String, [u8; 4]>,
    
    /// Default size in pixels
    #[serde(default = "default_size")]
    pub base_size: f32,
}

fn default_size() -> f32 {
    32.0
}

/// Icon definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconDef {
    /// Unicode glyph (for quick rendering)
    #[serde(default)]
    pub glyph: Option<String>,
    
    /// Icon name from icon set (e.g., "user", "building", "folder")
    #[serde(default)]
    pub icon_name: Option<String>,
    
    /// Fallback text if icon unavailable
    pub fallback: String,
}

impl TokenVisual {
    /// Get color for a status, falling back to base color
    pub fn color_for_status(&self, status: Option<&str>) -> [u8; 4] {
        status
            .and_then(|s| self.status_colors.get(s))
            .copied()
            .unwrap_or(self.base_color)
    }
    
    /// Convert color to egui Color32
    pub fn base_color32(&self) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(
            self.base_color[0],
            self.base_color[1],
            self.base_color[2],
            self.base_color[3],
        )
    }
    
    /// Convert status color to egui Color32
    pub fn status_color32(&self, status: Option<&str>) -> egui::Color32 {
        let c = self.color_for_status(status);
        egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
    }
}
```

### 1.2 Interaction Rules

```rust
/// Interaction behavior definitions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InteractionRules {
    /// Behavior when cursor hovers
    #[serde(default)]
    pub on_hover: HoverBehavior,
    
    /// Behavior on single click
    #[serde(default)]
    pub on_click: ClickBehavior,
    
    /// Behavior on double-click
    #[serde(default)]
    pub on_double_click: ClickBehavior,
    
    /// Context menu items
    #[serde(default)]
    pub context_actions: Vec<ContextAction>,
}

/// Hover behavior
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HoverBehavior {
    #[default]
    None,
    Highlight,
    Tooltip {
        /// Template with {field} placeholders
        template: String,
    },
}

/// Click behavior
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClickBehavior {
    #[default]
    None,
    Select,
    Focus,
    OpenDetail,
    OpenContainer,
    Custom {
        action_id: String,
    },
}

/// Context menu action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAction {
    pub action_id: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
}
```

### 1.3 Detail Template

```rust
/// Template for rendering detail view
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DetailTemplate {
    /// Sections in the detail view
    #[serde(default)]
    pub sections: Vec<DetailSection>,
    
    /// Actions available in detail view
    #[serde(default)]
    pub actions: Vec<DetailAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailSection {
    /// Section title
    pub title: String,
    
    /// Fields to display
    #[serde(default)]
    pub fields: Vec<DetailField>,
    
    /// Initially collapsed?
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailField {
    /// Data key (path into entity data)
    pub key: String,
    
    /// Display label
    pub label: String,
    
    /// Format type
    #[serde(default)]
    pub format: FieldFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FieldFormat {
    #[default]
    Text,
    Number {
        #[serde(default)]
        decimals: u8,
    },
    Currency {
        #[serde(default = "default_currency")]
        symbol: String,
    },
    Percent {
        #[serde(default = "default_decimals")]
        decimals: u8,
    },
    Date {
        #[serde(default = "default_date_format")]
        format: String,
    },
    StatusBadge,
}

fn default_currency() -> String {
    "$".to_string()
}

fn default_decimals() -> u8 {
    2
}

fn default_date_format() -> String {
    "%Y-%m-%d".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailAction {
    pub action_id: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub style: ActionStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActionStyle {
    #[default]
    Secondary,
    Primary,
    Danger,
}
```

### 1.4 Tasks - Core Types

- [ ] Create `rust/src/tokens/` module directory
- [ ] Create `rust/src/tokens/mod.rs` with public exports
- [ ] Create `rust/src/tokens/types.rs` with all type definitions
- [ ] Add `tokens` module to `rust/src/lib.rs`
- [ ] Add serde dependency if not present
- [ ] Unit tests for color conversion helpers

---

## Part 2: Token Registry

### 2.1 Registry Implementation

**File:** `rust/src/tokens/registry.rs`

```rust
//! Token Registry - loads and provides access to token definitions

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use anyhow::{Context, Result};

use super::types::{TokenDefinition, TokenTypeId};

/// Registry of all token type definitions
pub struct TokenRegistry {
    /// Token definitions by type ID
    definitions: HashMap<TokenTypeId, Arc<TokenDefinition>>,
    
    /// Entity type → Token type mapping (for aliases)
    type_aliases: HashMap<String, TokenTypeId>,
    
    /// Default token for unknown types
    default_token: Arc<TokenDefinition>,
}

/// Configuration file structure
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenConfig {
    /// Version for compatibility checking
    #[serde(default = "default_version")]
    pub version: String,
    
    /// Token definitions
    pub tokens: Vec<TokenDefinition>,
    
    /// Entity type aliases (maps multiple entity types to one token)
    #[serde(default)]
    pub aliases: HashMap<TokenTypeId, Vec<String>>,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl TokenRegistry {
    /// Create empty registry with just the default token
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            type_aliases: HashMap::new(),
            default_token: Arc::new(TokenDefinition::default_unknown()),
        }
    }
    
    /// Load registry from YAML file
    pub fn from_file(path: &Path) -> Result<Self> {
        let yaml_str = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read token config: {:?}", path))?;
        Self::from_yaml(&yaml_str)
    }
    
    /// Load registry from YAML string
    pub fn from_yaml(yaml_str: &str) -> Result<Self> {
        let config: TokenConfig = serde_yaml::from_str(yaml_str)
            .context("Failed to parse token config YAML")?;
        Self::from_config(config)
    }
    
    /// Build registry from parsed config
    pub fn from_config(config: TokenConfig) -> Result<Self> {
        let mut definitions = HashMap::new();
        let mut type_aliases = HashMap::new();
        
        for def in config.tokens {
            let type_id = def.type_id.clone();
            definitions.insert(type_id.clone(), Arc::new(def));
        }
        
        // Build alias map
        for (token_type, aliases) in config.aliases {
            for alias in aliases {
                type_aliases.insert(alias, token_type.clone());
            }
        }
        
        let default_token = Arc::new(TokenDefinition::default_unknown());
        
        Ok(Self {
            definitions,
            type_aliases,
            default_token,
        })
    }
    
    /// Get token definition by type ID
    pub fn get(&self, type_id: &str) -> Arc<TokenDefinition> {
        // Check direct match first
        if let Some(def) = self.definitions.get(type_id) {
            return def.clone();
        }
        
        // Check aliases
        if let Some(aliased_type) = self.type_aliases.get(type_id) {
            if let Some(def) = self.definitions.get(aliased_type) {
                return def.clone();
            }
        }
        
        // Fall back to default
        self.default_token.clone()
    }
    
    /// Check if a type ID exists (directly or via alias)
    pub fn contains(&self, type_id: &str) -> bool {
        self.definitions.contains_key(type_id) || self.type_aliases.contains_key(type_id)
    }
    
    /// List all registered type IDs (not including aliases)
    pub fn type_ids(&self) -> impl Iterator<Item = &TokenTypeId> {
        self.definitions.keys()
    }
    
    /// Get all container tokens
    pub fn containers(&self) -> impl Iterator<Item = &Arc<TokenDefinition>> {
        self.definitions.values().filter(|d| d.is_container)
    }
}

impl Default for TokenRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenDefinition {
    /// Create default token for unknown types
    pub fn default_unknown() -> Self {
        use super::types::*;
        
        Self {
            type_id: "_unknown".to_string(),
            label: "Unknown".to_string(),
            category: TokenCategory::Entity,
            visual: TokenVisual {
                icon: IconDef {
                    glyph: Some("?".to_string()),
                    icon_name: Some("help-circle".to_string()),
                    fallback: "?".to_string(),
                },
                base_color: [128, 128, 128, 255],
                highlight_color: [180, 180, 180, 255],
                status_colors: HashMap::new(),
                base_size: 32.0,
            },
            interactions: InteractionRules::default(),
            detail_template: None,
            is_container: false,
            browse_nickname: None,
        }
    }
}
```

### 2.2 Tasks - Registry

- [ ] Create `rust/src/tokens/registry.rs`
- [ ] Implement `TokenRegistry` struct
- [ ] Implement YAML loading with error handling
- [ ] Implement alias resolution
- [ ] Implement default unknown token
- [ ] Unit tests for registry lookup
- [ ] Unit tests for alias resolution

---

## Part 3: YAML Configuration

### 3.1 Token Definitions File

**File:** `rust/config/tokens/tokens.yaml`

```yaml
# Token Type Definitions
# ======================
# Defines visual appearance and behavior for each entity type.

version: "1.0"

# Aliases: map multiple entity types to one token definition
aliases:
  entity:
    - proper_person
    - limited_company
    - partnership
    - trust
  investor:
    - investor_holding
    - shareholder

tokens:
  # ===========================================================================
  # CBU - Client Business Unit
  # ===========================================================================
  - type_id: cbu
    label: "CBU"
    category: container
    is_container: false
    
    visual:
      icon:
        glyph: "🏢"
        icon_name: "building"
        fallback: "C"
      base_color: [64, 156, 255, 255]   # Blue
      highlight_color: [100, 180, 255, 255]
      status_colors:
        ACTIVE: [77, 204, 102, 255]     # Green
        DRAFT: [180, 180, 180, 255]     # Gray
        PENDING: [230, 179, 51, 255]    # Amber
        SUSPENDED: [230, 77, 77, 255]   # Red
      base_size: 48.0
    
    interactions:
      on_hover:
        tooltip:
          template: "{name}\n{jurisdiction} | {status}"
      on_click: select
      on_double_click: open_detail
      context_actions:
        - action_id: view_graph
          label: "View Graph"
          icon: "git-branch"
        - action_id: view_documents
          label: "Documents"
          icon: "folder"
        - action_id: view_kyc
          label: "KYC Status"
          icon: "shield"
    
    detail_template:
      sections:
        - title: "Overview"
          fields:
            - key: name
              label: "Name"
              format: text
            - key: jurisdiction
              label: "Jurisdiction"
              format: text
            - key: client_type
              label: "Type"
              format: text
            - key: status
              label: "Status"
              format: status_badge
      actions:
        - action_id: edit
          label: "Edit"
          style: primary
        - action_id: view_full
          label: "Full Profile"
          style: secondary

  # ===========================================================================
  # SHARE CLASS - Container for investors
  # ===========================================================================
  - type_id: share_class
    label: "Share Class"
    category: container
    is_container: true
    browse_nickname: "INVESTOR_HOLDING"
    
    visual:
      icon:
        glyph: "📊"
        icon_name: "bar-chart"
        fallback: "SC"
      base_color: [77, 179, 128, 255]   # Teal
      highlight_color: [102, 204, 153, 255]
      status_colors:
        ACTIVE: [77, 204, 102, 255]
        SOFT_CLOSED: [230, 179, 51, 255]
        HARD_CLOSED: [230, 77, 77, 255]
      base_size: 40.0
    
    interactions:
      on_hover:
        tooltip:
          template: "{share_class_code} ({currency})\n{investor_count} investors | {total_aum}"
      on_click: select
      on_double_click: open_container
      context_actions:
        - action_id: browse_investors
          label: "Browse Investors"
          icon: "users"
        - action_id: view_summary
          label: "View Summary"
          icon: "pie-chart"
    
    detail_template:
      sections:
        - title: "Share Class"
          fields:
            - key: share_class_code
              label: "Code"
              format: text
            - key: share_class_name
              label: "Name"
              format: text
            - key: currency
              label: "Currency"
              format: text
            - key: status
              label: "Status"
              format: status_badge
        - title: "Statistics"
          fields:
            - key: investor_count
              label: "Investors"
              format:
                number:
                  decimals: 0
            - key: total_aum
              label: "Total AUM"
              format:
                currency:
                  symbol: "$"

  # ===========================================================================
  # INVESTOR HOLDING - Container item
  # ===========================================================================
  - type_id: investor_holding
    label: "Investor"
    category: container_item
    
    visual:
      icon:
        glyph: "👤"
        icon_name: "user"
        fallback: "I"
      base_color: [102, 153, 230, 255]  # Light blue
      highlight_color: [128, 179, 255, 255]
      status_colors:
        VERIFIED: [77, 204, 102, 255]
        PENDING: [230, 179, 51, 255]
        IN_PROGRESS: [102, 179, 230, 255]
        EXPIRED: [180, 180, 180, 255]
        BLOCKED: [230, 77, 77, 255]
      base_size: 32.0
    
    interactions:
      on_hover:
        tooltip:
          template: "{investor_name}\n{jurisdiction} | {investor_type}\n{holding_value} ({percentage}%)"
      on_click: select
      on_double_click: open_detail
      context_actions:
        - action_id: view_entity
          label: "View Entity"
          icon: "user"
        - action_id: view_documents
          label: "Documents"
          icon: "folder"
        - action_id: view_kyc
          label: "KYC History"
          icon: "shield"
    
    detail_template:
      sections:
        - title: "Holding"
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
        - title: "Investor"
          collapsed: true
          fields:
            - key: investor_name
              label: "Name"
              format: text
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

  # ===========================================================================
  # ENTITY - Legal/Natural persons
  # ===========================================================================
  - type_id: entity
    label: "Entity"
    category: entity
    
    visual:
      icon:
        glyph: "🏛️"
        icon_name: "landmark"
        fallback: "E"
      base_color: [128, 128, 179, 255]  # Purple-gray
      highlight_color: [153, 153, 204, 255]
      status_colors:
        ACTIVE: [77, 204, 102, 255]
        INACTIVE: [180, 180, 180, 255]
      base_size: 36.0
    
    interactions:
      on_hover:
        tooltip:
          template: "{name}\n{entity_type} | {jurisdiction}"
      on_click: select
      on_double_click: open_detail
      context_actions:
        - action_id: view_profile
          label: "View Profile"
          icon: "user"
        - action_id: view_documents
          label: "Documents"
          icon: "folder"
        - action_id: view_relationships
          label: "Relationships"
          icon: "git-branch"
    
    detail_template:
      sections:
        - title: "Entity"
          fields:
            - key: name
              label: "Name"
              format: text
            - key: entity_type
              label: "Type"
              format: text
            - key: jurisdiction
              label: "Jurisdiction"
              format: text
            - key: registration_number
              label: "Reg. Number"
              format: text

  # ===========================================================================
  # SERVICE RESOURCE - Container item for services
  # ===========================================================================
  - type_id: service_resource
    label: "Resource"
    category: container_item
    
    visual:
      icon:
        glyph: "⚙️"
        icon_name: "settings"
        fallback: "R"
      base_color: [179, 128, 102, 255]  # Brown
      highlight_color: [204, 153, 128, 255]
      status_colors:
        ACTIVE: [77, 204, 102, 255]
        PENDING: [230, 179, 51, 255]
        INACTIVE: [180, 180, 180, 255]
      base_size: 28.0
    
    interactions:
      on_hover:
        tooltip:
          template: "{resource_name}\n{resource_type_code} | {status}"
      on_click: select
      on_double_click: open_detail
      context_actions:
        - action_id: view_config
          label: "View Config"
          icon: "settings"
        - action_id: view_logs
          label: "Activity Log"
          icon: "list"
    
    detail_template:
      sections:
        - title: "Resource"
          fields:
            - key: resource_name
              label: "Name"
              format: text
            - key: resource_type_code
              label: "Type"
              format: text
            - key: resource_identifier
              label: "Identifier"
              format: text
            - key: status
              label: "Status"
              format: status_badge

  # ===========================================================================
  # DOCUMENT
  # ===========================================================================
  - type_id: document
    label: "Document"
    category: document
    
    visual:
      icon:
        glyph: "📄"
        icon_name: "file-text"
        fallback: "D"
      base_color: [179, 179, 128, 255]  # Olive
      highlight_color: [204, 204, 153, 255]
      status_colors:
        VALID: [77, 204, 102, 255]
        PENDING_REVIEW: [230, 179, 51, 255]
        EXPIRED: [230, 77, 77, 255]
        REJECTED: [180, 180, 180, 255]
      base_size: 28.0
    
    interactions:
      on_hover:
        tooltip:
          template: "{document_type}\n{status} | {expiry_date}"
      on_click: select
      on_double_click: open_detail
      context_actions:
        - action_id: view_document
          label: "View Document"
          icon: "eye"
        - action_id: download
          label: "Download"
          icon: "download"
```

### 3.2 Tasks - Configuration

- [ ] Create `rust/config/tokens/` directory
- [ ] Create `tokens.yaml` with all token definitions
- [ ] Define CBU token
- [ ] Define share_class token (container)
- [ ] Define investor_holding token
- [ ] Define entity token with aliases
- [ ] Define service_resource token
- [ ] Define document token
- [ ] Test YAML loads without errors

---

## Part 4: Integration with UI

### 4.1 Add Registry to App State

**File:** Update wherever app state is initialized

```rust
use crate::tokens::TokenRegistry;

pub struct AppState {
    // ... existing fields ...
    
    /// Token registry for visual configuration
    pub token_registry: TokenRegistry,
}

impl AppState {
    pub fn new() -> Self {
        // Load token registry
        let token_registry = TokenRegistry::from_file(
            Path::new("config/tokens/tokens.yaml")
        ).unwrap_or_else(|e| {
            tracing::warn!("Failed to load token config: {}, using defaults", e);
            TokenRegistry::new()
        });
        
        Self {
            // ... existing fields ...
            token_registry,
        }
    }
}
```

### 4.2 Update ContainerBrowsePanel to Use Tokens

**File:** `crates/ob-poc-ui/src/panels/container_browse.rs`

Replace hardcoded colors with token lookups:

```rust
impl ContainerBrowsePanel {
    /// Render a browse item using token configuration
    fn render_item(
        &self,
        ui: &mut Ui,
        item: &BrowseItemView,
        registry: &TokenRegistry,
    ) {
        let token = registry.get(&item.entity_type);
        
        // Use token colors
        let status_color = token.visual.status_color32(Some(&item.status));
        
        ui.horizontal(|ui| {
            // Status indicator using token color
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(4.0, 40.0),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(rect, 2.0, status_color);
            
            ui.add_space(8.0);
            
            // Icon from token
            if let Some(ref glyph) = token.visual.icon.glyph {
                ui.label(glyph);
            }
            
            // Content
            ui.vertical(|ui| {
                ui.label(RichText::new(&item.display).strong());
                ui.label(RichText::new(&item.sublabel).small().weak());
            });
        });
    }
    
    /// Build tooltip from token template
    fn build_tooltip(&self, item: &BrowseItemView, registry: &TokenRegistry) -> String {
        let token = registry.get(&item.entity_type);
        
        if let HoverBehavior::Tooltip { ref template } = token.interactions.on_hover {
            // Replace {field} placeholders with actual values
            let mut result = template.clone();
            for (key, value) in &item.fields {
                result = result.replace(&format!("{{{}}}", key), value);
            }
            result
        } else {
            item.display.clone()
        }
    }
}
```

### 4.3 Update Graph View to Use Tokens

**File:** `crates/ob-poc-ui/src/views/graph_view.rs`

```rust
impl GraphView {
    /// Render a node using token configuration
    fn render_node(
        &self,
        ui: &mut Ui,
        node: &GraphNode,
        registry: &TokenRegistry,
    ) {
        let token = registry.get(&node.node_type.to_string());
        
        // Node color from token (with status)
        let color = token.visual.status_color32(
            node.data.get("status").and_then(|v| v.as_str())
        );
        
        // Size from token
        let size = token.visual.base_size;
        
        // Icon from token
        let icon = token.visual.icon.glyph.as_deref()
            .unwrap_or(&token.visual.icon.fallback);
        
        // Render node...
    }
    
    /// Handle node click using token behavior
    fn handle_node_click(&mut self, node: &GraphNode, double_click: bool, registry: &TokenRegistry) {
        let token = registry.get(&node.node_type.to_string());
        
        let behavior = if double_click {
            &token.interactions.on_double_click
        } else {
            &token.interactions.on_click
        };
        
        match behavior {
            ClickBehavior::Select => self.select_node(&node.id),
            ClickBehavior::Focus => self.focus_on_node(&node.id),
            ClickBehavior::OpenDetail => self.open_detail_panel(&node.id),
            ClickBehavior::OpenContainer => {
                if let Some(ref nickname) = token.browse_nickname {
                    self.open_container_panel(node, nickname);
                }
            }
            ClickBehavior::Custom { action_id } => {
                self.handle_custom_action(action_id, node);
            }
            ClickBehavior::None => {}
        }
    }
}
```

### 4.4 Tasks - Integration

- [ ] Add TokenRegistry to app state
- [ ] Load registry at startup with fallback
- [ ] Update ContainerBrowsePanel to use token colors
- [ ] Update ContainerBrowsePanel to use token tooltips
- [ ] Update graph node rendering to use tokens
- [ ] Update click handling to use token behaviors
- [ ] Test token-driven rendering works

---

## Part 5: Detail View Renderer

### 5.1 Detail Panel Component

**File:** `crates/ob-poc-ui/src/panels/detail_panel.rs` (new file)

```rust
//! Detail Panel - renders entity details using token templates

use egui::{Ui, RichText};
use crate::tokens::{TokenRegistry, DetailTemplate, DetailSection, FieldFormat};

/// Render detail view for an entity using its token template
pub fn render_detail(
    ui: &mut Ui,
    entity_type: &str,
    data: &serde_json::Value,
    registry: &TokenRegistry,
) {
    let token = registry.get(entity_type);
    
    // Header with icon and title
    ui.horizontal(|ui| {
        if let Some(ref glyph) = token.visual.icon.glyph {
            ui.label(RichText::new(glyph).size(24.0));
        }
        
        let title = data.get("name")
            .or_else(|| data.get("label"))
            .and_then(|v| v.as_str())
            .unwrap_or(&token.label);
        ui.heading(title);
    });
    
    ui.separator();
    
    // Render sections from template
    if let Some(ref template) = token.detail_template {
        for section in &template.sections {
            render_section(ui, section, data);
        }
        
        // Actions
        if !template.actions.is_empty() {
            ui.separator();
            ui.horizontal(|ui| {
                for action in &template.actions {
                    let button = match action.style {
                        ActionStyle::Primary => egui::Button::new(&action.label),
                        ActionStyle::Danger => egui::Button::new(
                            RichText::new(&action.label).color(egui::Color32::RED)
                        ),
                        _ => egui::Button::new(&action.label),
                    };
                    
                    if ui.add(button).clicked() {
                        // Emit action event
                    }
                }
            });
        }
    }
}

fn render_section(ui: &mut Ui, section: &DetailSection, data: &serde_json::Value) {
    egui::CollapsingHeader::new(&section.title)
        .default_open(!section.collapsed)
        .show(ui, |ui| {
            egui::Grid::new(&section.title)
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for field in &section.fields {
                        ui.label(&field.label);
                        
                        let value = data.get(&field.key);
                        let formatted = format_value(value, &field.format);
                        
                        match &field.format {
                            FieldFormat::StatusBadge => {
                                render_status_badge(ui, &formatted);
                            }
                            _ => {
                                ui.label(&formatted);
                            }
                        }
                        
                        ui.end_row();
                    }
                });
        });
}

fn format_value(value: Option<&serde_json::Value>, format: &FieldFormat) -> String {
    let Some(v) = value else {
        return "—".to_string();
    };
    
    match format {
        FieldFormat::Text => {
            v.as_str().unwrap_or("—").to_string()
        }
        FieldFormat::Number { decimals } => {
            if let Some(n) = v.as_f64() {
                format!("{:.prec$}", n, prec = *decimals as usize)
            } else {
                v.to_string()
            }
        }
        FieldFormat::Currency { symbol } => {
            if let Some(n) = v.as_f64() {
                format!("{}{:,.2}", symbol, n)
            } else {
                v.to_string()
            }
        }
        FieldFormat::Percent { decimals } => {
            if let Some(n) = v.as_f64() {
                format!("{:.prec$}%", n, prec = *decimals as usize)
            } else {
                v.to_string()
            }
        }
        FieldFormat::Date { format: fmt } => {
            v.as_str().unwrap_or("—").to_string()
            // TODO: parse and reformat date
        }
        FieldFormat::StatusBadge => {
            v.as_str().unwrap_or("UNKNOWN").to_string()
        }
    }
}

fn render_status_badge(ui: &mut Ui, status: &str) {
    let color = match status {
        "ACTIVE" | "VERIFIED" | "VALID" => egui::Color32::from_rgb(77, 204, 102),
        "PENDING" | "PENDING_REVIEW" | "IN_PROGRESS" => egui::Color32::from_rgb(230, 179, 51),
        "BLOCKED" | "EXPIRED" | "REJECTED" | "SUSPENDED" => egui::Color32::from_rgb(230, 77, 77),
        _ => egui::Color32::GRAY,
    };
    
    let (rect, _) = ui.allocate_exact_size(egui::vec2(80.0, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, color);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        status,
        egui::FontId::proportional(11.0),
        egui::Color32::WHITE,
    );
}
```

### 5.2 Tasks - Detail View

- [ ] Create `detail_panel.rs`
- [ ] Implement section rendering
- [ ] Implement field formatting (text, number, currency, percent, date)
- [ ] Implement status badge rendering
- [ ] Implement action buttons
- [ ] Wire into container browse panel
- [ ] Test with different entity types

---

## Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  IMPLEMENTATION ORDER                                                       │
│                                                                             │
│  1. Part 1: Core types (TokenDefinition, Visual, Interactions)             │
│  2. Part 2: Token Registry (YAML loading, lookup)                          │
│  3. Part 3: YAML configuration (define all token types)                    │
│  4. Part 4: Integration (wire registry into UI components)                 │
│  5. Part 5: Detail view renderer                                           │
│                                                                             │
│  ESTIMATED: 2 days                                                          │
│                                                                             │
│  DEPENDS ON: TODO-CBU-CONTAINERS.md (should be complete first)             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## What This Enables

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  BEFORE TOKENS                          AFTER TOKENS                        │
│  ═════════════                          ════════════                        │
│                                                                             │
│  Hardcoded status colors                YAML: status_colors: {...}         │
│  Hardcoded icons                        YAML: icon: { glyph: "👤" }        │
│  Hardcoded tooltips                     YAML: on_hover: tooltip: "..."     │
│  Hardcoded detail fields                YAML: detail_template: {...}       │
│  Hardcoded click behavior               YAML: on_click: open_container     │
│                                                                             │
│  Change = edit Rust + recompile         Change = edit YAML + reload        │
│                                                                             │
│  New entity type = new code             New entity type = new YAML block   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Success Criteria

- [ ] Token registry loads from YAML without errors
- [ ] Unknown entity types fall back to default token
- [ ] Container browse panel uses token colors for status badges
- [ ] Graph view uses token icons and colors
- [ ] Click behaviors match token configuration
- [ ] Detail view renders from token template
- [ ] Adding new entity type requires only YAML changes

---

*Tokens: configuration over code.*
