//! Core types for CBU Entity Graph visualization
//!
//! These types mirror the server-side graph types but are optimized for UI rendering.

#![allow(dead_code)]

use egui::{Color32, Pos2, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// CBU CATEGORY & TEMPLATES
// =============================================================================

/// CBU category determines which layout template to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CbuCategory {
    #[default]
    FundMandate,
    CorporateGroup,
    InstitutionalAccount,
    RetailClient,
    FamilyTrust,
    InternalTest,
    CorrespondentBank,
}

impl CbuCategory {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "FUND_MANDATE" => Self::FundMandate,
            "CORPORATE_GROUP" => Self::CorporateGroup,
            "INSTITUTIONAL_ACCOUNT" => Self::InstitutionalAccount,
            "RETAIL_CLIENT" => Self::RetailClient,
            "FAMILY_TRUST" => Self::FamilyTrust,
            "INTERNAL_TEST" => Self::InternalTest,
            "CORRESPONDENT_BANK" => Self::CorrespondentBank,
            _ => Self::CorporateGroup, // fallback
        }
    }
}

// =============================================================================
// ROLE TYPES
// =============================================================================

/// Primary role determines slot assignment in templates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrimaryRole {
    // Highest priority - ownership/control
    UltimateBeneficialOwner,
    Shareholder,
    // Management
    ManagementCompany,
    Director,
    Officer,
    // Structure
    Principal,
    Trustee,
    Protector,
    Beneficiary,
    Settlor,
    // Other
    AuthorizedSignatory,
    ContactPerson,
    Unknown,
}

impl PrimaryRole {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().replace('-', "_").as_str() {
            "ULTIMATE_BENEFICIAL_OWNER" | "UBO" => Self::UltimateBeneficialOwner,
            "SHAREHOLDER" => Self::Shareholder,
            "MANAGEMENT_COMPANY" | "MANCO" => Self::ManagementCompany,
            "DIRECTOR" => Self::Director,
            "OFFICER" => Self::Officer,
            "PRINCIPAL" => Self::Principal,
            "TRUSTEE" => Self::Trustee,
            "PROTECTOR" => Self::Protector,
            "BENEFICIARY" => Self::Beneficiary,
            "SETTLOR" => Self::Settlor,
            "AUTHORIZED_SIGNATORY" => Self::AuthorizedSignatory,
            "CONTACT_PERSON" => Self::ContactPerson,
            _ => Self::Unknown,
        }
    }

    /// Priority for layout ordering (higher = more important = placed first)
    pub fn priority(&self) -> i32 {
        match self {
            Self::UltimateBeneficialOwner => 100,
            Self::Shareholder => 90,
            Self::ManagementCompany => 75,
            Self::Director => 70,
            Self::Officer => 65,
            Self::Principal => 60,
            Self::Trustee => 55,
            Self::Protector => 50,
            Self::Beneficiary => 45,
            Self::Settlor => 40,
            Self::AuthorizedSignatory => 30,
            Self::ContactPerson => 20,
            Self::Unknown => 0,
        }
    }
}

// =============================================================================
// ENTITY TYPES
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    #[default]
    Unknown,
    // Entity types
    ProperPerson,
    LimitedCompany,
    Partnership,
    Trust,
    Fund,
    // Service layer types
    Product,
    Service,
    Resource,
}

impl EntityType {
    pub fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.contains("person") {
            Self::ProperPerson
        } else if lower.contains("company") || lower.contains("limited") {
            Self::LimitedCompany
        } else if lower.contains("partner") {
            Self::Partnership
        } else if lower.contains("trust") {
            Self::Trust
        } else if lower.contains("fund") {
            Self::Fund
        } else if lower == "product" {
            Self::Product
        } else if lower == "service" {
            Self::Service
        } else if lower == "resource" {
            Self::Resource
        } else {
            Self::Unknown
        }
    }
}

// =============================================================================
// GRAPH DATA (from server)
// =============================================================================

/// Graph data received from server API
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CbuGraphData {
    pub cbu_id: Uuid,
    pub label: String,
    pub cbu_category: Option<String>,
    pub jurisdiction: Option<String>,
    pub nodes: Vec<GraphNodeData>,
    pub edges: Vec<GraphEdgeData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphNodeData {
    pub id: String,
    pub node_type: String,
    pub layer: String,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: String,
    #[serde(default)]
    pub roles: Vec<String>,
    pub primary_role: Option<String>,
    pub jurisdiction: Option<String>,
    pub role_priority: Option<i32>,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphEdgeData {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub label: Option<String>,
}

// =============================================================================
// UI GRAPH TYPES (computed for rendering)
// =============================================================================

/// A node positioned for rendering
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: String,
    pub entity_type: EntityType,
    pub primary_role: PrimaryRole,
    pub all_roles: Vec<String>,
    pub label: String,
    pub sublabel: Option<String>,
    pub jurisdiction: Option<String>,

    /// Computed position in graph coordinates
    pub position: Pos2,
    /// Size of the node
    pub size: Vec2,
    /// Is this node in the current focus set?
    pub in_focus: bool,
    /// Is this the CBU root node?
    pub is_cbu_root: bool,
    /// Visual style
    pub style: NodeStyle,
}

#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub fill_color: Color32,
    pub border_color: Color32,
    pub text_color: Color32,
    pub border_width: f32,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            fill_color: Color32::from_rgb(55, 65, 81),
            border_color: Color32::from_rgb(107, 114, 128),
            text_color: Color32::WHITE,
            border_width: 2.0,
        }
    }
}

/// An edge positioned for rendering
#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,

    /// Control points for bezier curve (if needed for hop-over)
    pub control_points: Vec<Pos2>,
    /// Is this edge in the current focus set?
    pub in_focus: bool,
    /// Visual style
    pub style: EdgeStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    HasRole,
    Owns,
    Controls,
    Other,
}

impl EdgeType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "has_role" | "hasrole" => Self::HasRole,
            "owns" => Self::Owns,
            "controls" => Self::Controls,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EdgeStyle {
    pub color: Color32,
    pub width: f32,
    pub dashed: bool,
}

impl Default for EdgeStyle {
    fn default() -> Self {
        Self {
            color: Color32::from_rgb(107, 114, 128),
            width: 1.5,
            dashed: false,
        }
    }
}

// =============================================================================
// INVESTOR GROUP (collapsed investors)
// =============================================================================

/// Collapsed group of investors for a share class
#[derive(Debug, Clone)]
pub struct InvestorGroup {
    pub share_class_id: String,
    pub share_class_name: String,
    pub investor_count: usize,
    pub investors: Vec<InvestorSummary>,
    pub position: Pos2,
    pub expanded: bool,
}

#[derive(Debug, Clone)]
pub struct InvestorSummary {
    pub entity_id: String,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub holding_units: Option<f64>,
}

// =============================================================================
// LAYOUT GRAPH (complete positioned graph)
// =============================================================================

/// Complete layout-computed graph ready for rendering
#[derive(Debug, Clone)]
pub struct LayoutGraph {
    pub cbu_id: Uuid,
    pub cbu_category: CbuCategory,
    pub jurisdiction: Option<String>,

    /// All positioned nodes
    pub nodes: HashMap<String, LayoutNode>,
    /// All positioned edges
    pub edges: Vec<LayoutEdge>,
    /// Collapsed investor groups
    pub investor_groups: Vec<InvestorGroup>,

    /// Bounding box of all nodes (for camera fitting)
    pub bounds: egui::Rect,
}

impl Default for LayoutGraph {
    fn default() -> Self {
        Self {
            cbu_id: Uuid::nil(),
            cbu_category: CbuCategory::default(),
            jurisdiction: None,
            nodes: HashMap::new(),
            edges: Vec::new(),
            investor_groups: Vec::new(),
            bounds: egui::Rect::NOTHING,
        }
    }
}

impl LayoutGraph {
    pub fn new(cbu_id: Uuid) -> Self {
        Self {
            cbu_id,
            cbu_category: CbuCategory::default(),
            jurisdiction: None,
            nodes: HashMap::new(),
            edges: Vec::new(),
            investor_groups: Vec::new(),
            bounds: egui::Rect::NOTHING,
        }
    }

    /// Get node by ID
    pub fn get_node(&self, id: &str) -> Option<&LayoutNode> {
        self.nodes.get(id)
    }

    /// Get mutable node by ID
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut LayoutNode> {
        self.nodes.get_mut(id)
    }

    /// Recompute bounds from all nodes
    pub fn recompute_bounds(&mut self) {
        if self.nodes.is_empty() {
            self.bounds = egui::Rect::NOTHING;
            return;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for node in self.nodes.values() {
            let half_size = node.size / 2.0;
            min_x = min_x.min(node.position.x - half_size.x);
            min_y = min_y.min(node.position.y - half_size.y);
            max_x = max_x.max(node.position.x + half_size.x);
            max_y = max_y.max(node.position.y + half_size.y);
        }

        self.bounds = egui::Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y));
    }
}

// =============================================================================
// VIEW MODE - re-exported from graph_view
// =============================================================================

// ViewMode is defined in graph_view.rs and re-exported through mod.rs
