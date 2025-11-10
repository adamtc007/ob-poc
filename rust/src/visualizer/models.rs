//! Data Models for DSL Visualizer
//!
//! This module defines the core data structures used throughout the DSL visualizer
//! application. These models represent DSL entries, AST nodes, filter options,
//! and other data structures needed for visualization and interaction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a DSL entry in the browser panel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DSLEntry {
    /// Unique identifier for the DSL instance
    pub id: String,

    /// Human-readable name for the DSL
    pub name: String,

    /// Domain/category of the DSL (e.g., "onboarding", "kyc", "compliance")
    pub domain: String,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Version number
    pub version: i32,

    /// Optional description
    pub description: Option<String>,

    /// Preview of the DSL content (first few lines)
    pub content_preview: String,
}

/// Represents an AST (Abstract Syntax Tree) node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ASTNode {
    /// Unique identifier for this node
    pub id: String,

    /// Display label for the node
    pub label: String,

    /// Type of this AST node
    pub node_type: ASTNodeType,

    /// Child nodes
    pub children: Vec<ASTNode>,

    /// Additional properties/attributes
    pub properties: HashMap<String, String>,

    /// Optional position for graph layout
    pub position: Option<(f32, f32)>,
}

/// Types of AST nodes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ASTNodeType {
    /// Root node of the AST
    Root,
    /// Verb/function call node (e.g., "onboarding.create")
    Verb,
    /// Attribute/parameter node (e.g., ":cbu-id")
    Attribute,
    /// Value node (e.g., string, number, boolean)
    Value,
    /// List/collection node
    List,
}

/// Filter options for the DSL browser
#[derive(Debug, Clone, Default)]
pub struct FilterOptions {
    /// Filter by text search
    pub search_text: Option<String>,

    /// Filter by domain
    pub domain: Option<String>,

    /// Filter by date range
    pub date_range: Option<DateRange>,

    /// Filter by version range
    pub min_version: Option<i32>,
    pub max_version: Option<i32>,

    /// Show only entries with compilation errors
    pub show_errors_only: bool,

    /// Show only entries modified recently
    pub show_recent_only: bool,
}

/// Date range for filtering
#[derive(Debug, Clone)]
pub struct DateRange {
    /// Start date (inclusive)
    pub from: DateTime<Utc>,
    /// End date (inclusive)
    pub to: DateTime<Utc>,
}

/// Visualization options for the AST viewer
#[derive(Debug, Clone)]
pub struct VisualizationOptions {
    /// Layout algorithm to use
    pub layout: LayoutType,

    /// Styling configuration
    pub styling: StylingConfig,

    /// Include compilation information in display
    pub include_compilation_info: bool,

    /// Include domain context information
    pub include_domain_context: bool,

    /// Active filters for node display
    pub filters: NodeFilters,
}

/// Available layout types for AST visualization
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutType {
    /// Traditional tree layout (hierarchical, top-down)
    Tree,
    /// Force-directed graph layout
    Graph,
    /// Compact horizontal layout
    Hierarchical,
    /// Circular layout
    Circular,
    /// Custom layout
    Custom,
}

/// Styling configuration for the visualizer
#[derive(Debug, Clone)]
pub struct StylingConfig {
    /// UI theme (dark/light)
    pub theme: Theme,

    /// Base node size
    pub node_size: f32,

    /// Font size for node labels
    pub font_size: f32,

    /// Color scheme for different node types
    pub color_scheme: ColorScheme,

    /// Line thickness for connections
    pub line_thickness: f32,
}

/// UI theme options
#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    Dark,
    Light,
    Auto,
}

/// Color scheme for node visualization
#[derive(Debug, Clone)]
pub struct ColorScheme {
    /// Color for verb nodes
    pub verb_color: (u8, u8, u8),
    /// Color for attribute nodes
    pub attribute_color: (u8, u8, u8),
    /// Color for value nodes
    pub value_color: (u8, u8, u8),
    /// Color for list nodes
    pub list_color: (u8, u8, u8),
    /// Color for root nodes
    pub root_color: (u8, u8, u8),
    /// Color for selected nodes
    pub selection_color: (u8, u8, u8),
    /// Color for connections/edges
    pub edge_color: (u8, u8, u8),
}

/// Filters for node display in AST viewer
#[derive(Debug, Clone, Default)]
pub struct NodeFilters {
    /// Show only specific node types
    pub show_node_types: Option<Vec<ASTNodeType>>,

    /// Hide nodes with specific labels
    pub hide_labels: Vec<String>,

    /// Show only nodes matching pattern
    pub label_pattern: Option<String>,

    /// Maximum depth to display
    pub max_depth: Option<usize>,

    /// Minimum number of children to show node
    pub min_children: Option<usize>,
}

/// gRPC request/response models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDSLRequest {
    /// Maximum number of entries to return
    pub limit: Option<i32>,
    /// Offset for pagination
    pub offset: Option<i32>,
    /// Domain filter
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDSLResponse {
    /// List of DSL entries
    pub entries: Vec<DSLEntry>,
    /// Total count of available entries
    pub total_count: i32,
    /// Whether there are more results
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetDSLRequest {
    /// ID of the DSL instance to retrieve
    pub dsl_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetDSLResponse {
    /// The DSL entry
    pub entry: DSLEntry,
    /// Full DSL content
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDSLRequest {
    /// DSL content to parse
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDSLResponse {
    /// Parsed AST
    pub ast: ASTNode,
    /// Parsing statistics
    pub stats: ParseStats,
}

/// Statistics from DSL parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseStats {
    /// Total number of nodes in the AST
    pub node_count: usize,
    /// Maximum depth of the AST
    pub max_depth: usize,
    /// Parsing time in milliseconds
    pub parse_time_ms: u64,
    /// Number of errors encountered
    pub error_count: usize,
    /// Number of warnings
    pub warning_count: usize,
}

/// UI state for the application
#[derive(Debug, Clone)]
pub struct UIState {
    /// Currently selected DSL entry ID
    pub selected_dsl_id: Option<String>,

    /// Currently selected AST node ID
    pub selected_node_id: Option<String>,

    /// Zoom level for AST viewer
    pub zoom_level: f32,

    /// Pan offset for AST viewer (x, y)
    pub pan_offset: (f32, f32),

    /// Whether details panel is visible
    pub show_details_panel: bool,

    /// Whether debug information is shown
    pub show_debug_info: bool,

    /// Last error message
    pub last_error: Option<String>,

    /// Loading state
    pub is_loading: bool,
}

/// Performance metrics for the visualizer
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    /// Frames per second
    pub fps: f32,

    /// Time to render last frame (milliseconds)
    pub frame_time_ms: f32,

    /// Time to fetch DSL data (milliseconds)
    pub fetch_time_ms: f32,

    /// Time to parse AST (milliseconds)
    pub parse_time_ms: f32,

    /// Number of visible nodes in current view
    pub visible_nodes: usize,

    /// Memory usage estimate (MB)
    pub memory_usage_mb: f32,
}

/// Connection status for gRPC client
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    /// Not connected
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Successfully connected
    Connected,
    /// Connection failed
    Failed(String),
}

/// Implementation of default values and utility functions
impl Default for DSLEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            domain: String::new(),
            created_at: Utc::now(),
            version: 1,
            description: None,
            content_preview: String::new(),
        }
    }
}

impl Default for ASTNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            node_type: ASTNodeType::Root,
            children: Vec::new(),
            properties: HashMap::new(),
            position: None,
        }
    }
}

impl Default for VisualizationOptions {
    fn default() -> Self {
        Self {
            layout: LayoutType::Tree,
            styling: StylingConfig::default(),
            include_compilation_info: true,
            include_domain_context: true,
            filters: NodeFilters::default(),
        }
    }
}

impl Default for StylingConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            node_size: 60.0,
            font_size: 14.0,
            color_scheme: ColorScheme::default(),
            line_thickness: 1.0,
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            verb_color: (100, 150, 255),      // Blue
            attribute_color: (255, 150, 100), // Orange
            value_color: (150, 255, 100),     // Green
            list_color: (255, 255, 100),      // Yellow
            root_color: (200, 200, 200),      // Gray
            selection_color: (255, 255, 255), // White
            edge_color: (128, 128, 128),      // Gray
        }
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            selected_dsl_id: None,
            selected_node_id: None,
            zoom_level: 1.0,
            pan_offset: (0.0, 0.0),
            show_details_panel: true,
            show_debug_info: false,
            last_error: None,
            is_loading: false,
        }
    }
}

/// Utility functions for working with models
impl DSLEntry {
    /// Create a new DSL entry with required fields
    pub fn new(id: String, name: String, domain: String) -> Self {
        Self {
            id,
            name,
            domain,
            created_at: Utc::now(),
            version: 1,
            description: None,
            content_preview: String::new(),
        }
    }

    /// Check if this entry matches a search query
    pub fn matches_search(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.name.to_lowercase().contains(&query_lower)
            || self.domain.to_lowercase().contains(&query_lower)
            || self
                .description
                .as_ref()
                .map(|d| d.to_lowercase().contains(&query_lower))
                .unwrap_or(false)
            || self.content_preview.to_lowercase().contains(&query_lower)
    }

    /// Get a formatted display name
    pub fn display_name(&self) -> String {
        format!("{} (v{})", self.name, self.version)
    }
}

impl ASTNode {
    /// Create a new AST node
    pub fn new(id: String, label: String, node_type: ASTNodeType) -> Self {
        Self {
            id,
            label,
            node_type,
            children: Vec::new(),
            properties: HashMap::new(),
            position: None,
        }
    }

    /// Add a child node
    pub fn add_child(&mut self, child: ASTNode) {
        self.children.push(child);
    }

    /// Set a property
    pub fn set_property(&mut self, key: String, value: String) {
        self.properties.insert(key, value);
    }

    /// Get total number of nodes (including self and all descendants)
    pub fn total_node_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(|c| c.total_node_count())
            .sum::<usize>()
    }

    /// Get maximum depth of the tree
    pub fn max_depth(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            1 + self
                .children
                .iter()
                .map(|c| c.max_depth())
                .max()
                .unwrap_or(0)
        }
    }

    /// Find a node by ID
    pub fn find_by_id(&self, target_id: &str) -> Option<&ASTNode> {
        if self.id == target_id {
            return Some(self);
        }

        for child in &self.children {
            if let Some(found) = child.find_by_id(target_id) {
                return Some(found);
            }
        }

        None
    }

    /// Collect all nodes of a specific type
    pub fn collect_nodes_by_type(&self, target_type: &ASTNodeType) -> Vec<&ASTNode> {
        let mut result = Vec::new();

        if &self.node_type == target_type {
            result.push(self);
        }

        for child in &self.children {
            result.extend(child.collect_nodes_by_type(target_type));
        }

        result
    }
}

impl PerformanceMetrics {
    /// Update FPS calculation
    pub fn update_fps(&mut self, frame_time: f32) {
        self.frame_time_ms = frame_time;
        if frame_time > 0.0 {
            self.fps = 1000.0 / frame_time;
        }
    }

    /// Check if performance is good
    pub fn is_performance_good(&self) -> bool {
        self.fps >= 30.0 && self.frame_time_ms <= 33.0
    }
}

impl ConnectionStatus {
    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionStatus::Connected)
    }

    /// Get a human-readable status string
    pub fn status_text(&self) -> &str {
        match self {
            ConnectionStatus::Disconnected => "Disconnected",
            ConnectionStatus::Connecting => "Connecting...",
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Failed(_) => "Connection Failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsl_entry_creation() {
        let entry = DSLEntry::new(
            "test-id".to_string(),
            "Test DSL".to_string(),
            "onboarding".to_string(),
        );

        assert_eq!(entry.id, "test-id");
        assert_eq!(entry.name, "Test DSL");
        assert_eq!(entry.domain, "onboarding");
        assert_eq!(entry.version, 1);
    }

    #[test]
    fn test_dsl_entry_search() {
        let entry = DSLEntry {
            id: "1".to_string(),
            name: "Onboarding Process".to_string(),
            domain: "onboarding".to_string(),
            created_at: Utc::now(),
            version: 1,
            description: Some("Customer onboarding workflow".to_string()),
            content_preview: "(onboarding.create :client-id \"test\")".to_string(),
        };

        assert!(entry.matches_search("onboard"));
        assert!(entry.matches_search("customer"));
        assert!(entry.matches_search("client-id"));
        assert!(!entry.matches_search("kyc"));
    }

    #[test]
    fn test_ast_node_creation() {
        let mut root = ASTNode::new("root".to_string(), "Root".to_string(), ASTNodeType::Root);

        let child = ASTNode::new(
            "child1".to_string(),
            "Child 1".to_string(),
            ASTNodeType::Verb,
        );

        root.add_child(child);
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.total_node_count(), 2);
        assert_eq!(root.max_depth(), 2);
    }

    #[test]
    fn test_ast_node_find_by_id() {
        let mut root = ASTNode::new("root".to_string(), "Root".to_string(), ASTNodeType::Root);

        let child = ASTNode::new(
            "child1".to_string(),
            "Child 1".to_string(),
            ASTNodeType::Verb,
        );

        root.add_child(child);

        assert!(root.find_by_id("root").is_some());
        assert!(root.find_by_id("child1").is_some());
        assert!(root.find_by_id("nonexistent").is_none());
    }

    #[test]
    fn test_ast_node_collect_by_type() {
        let mut root = ASTNode::new("root".to_string(), "Root".to_string(), ASTNodeType::Root);

        let verb1 = ASTNode::new("verb1".to_string(), "Verb 1".to_string(), ASTNodeType::Verb);

        let verb2 = ASTNode::new("verb2".to_string(), "Verb 2".to_string(), ASTNodeType::Verb);

        let attr = ASTNode::new(
            "attr1".to_string(),
            "Attr 1".to_string(),
            ASTNodeType::Attribute,
        );

        root.add_child(verb1);
        root.add_child(verb2);
        root.add_child(attr);

        let verbs = root.collect_nodes_by_type(&ASTNodeType::Verb);
        assert_eq!(verbs.len(), 2);

        let roots = root.collect_nodes_by_type(&ASTNodeType::Root);
        assert_eq!(roots.len(), 1);
    }

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::default();

        metrics.update_fps(16.67); // 60 FPS
        assert!(metrics.fps > 59.0 && metrics.fps < 61.0);
        assert!(metrics.is_performance_good());

        metrics.update_fps(100.0); // 10 FPS
        assert!(metrics.fps > 9.0 && metrics.fps < 11.0);
        assert!(!metrics.is_performance_good());
    }

    #[test]
    fn test_connection_status() {
        assert!(!ConnectionStatus::Disconnected.is_connected());
        assert!(!ConnectionStatus::Connecting.is_connected());
        assert!(ConnectionStatus::Connected.is_connected());
        assert!(!ConnectionStatus::Failed("error".to_string()).is_connected());

        assert_eq!(ConnectionStatus::Connected.status_text(), "Connected");
        assert_eq!(ConnectionStatus::Disconnected.status_text(), "Disconnected");
    }
}
