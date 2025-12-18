//! Token type definitions
//!
//! Tokens bridge data entities to visual representation.
//! Definitions are loaded from YAML configuration.
//!
//! This is the 2D/egui version - no 3D, no GPU instancing.

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
    #[serde(default)]
    pub category: TokenCategory,

    /// Visual appearance
    pub visual: TokenVisual,

    /// Interaction behaviors
    #[serde(default)]
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

    /// Status -> color mapping
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IconDef {
    /// Unicode glyph (for quick rendering)
    #[serde(default)]
    pub glyph: Option<String>,

    /// Icon name from icon set (e.g., "user", "building", "folder")
    #[serde(default)]
    pub icon_name: Option<String>,

    /// Fallback text if icon unavailable
    #[serde(default = "default_fallback")]
    pub fallback: String,
}

fn default_fallback() -> String {
    "?".to_string()
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

    /// Convert highlight color to egui Color32
    pub fn highlight_color32(&self) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(
            self.highlight_color[0],
            self.highlight_color[1],
            self.highlight_color[2],
            self.highlight_color[3],
        )
    }
}

impl Default for TokenVisual {
    fn default() -> Self {
        Self {
            icon: IconDef::default(),
            base_color: [128, 128, 128, 255],
            highlight_color: [180, 180, 180, 255],
            status_colors: HashMap::new(),
            base_size: 32.0,
        }
    }
}

// =============================================================================
// INTERACTION RULES
// =============================================================================

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

// =============================================================================
// DETAIL TEMPLATE
// =============================================================================

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

// =============================================================================
// DEFAULT UNKNOWN TOKEN
// =============================================================================

impl TokenDefinition {
    /// Create default token for unknown types
    pub fn default_unknown() -> Self {
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
