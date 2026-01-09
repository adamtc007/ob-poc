//! Core types for CBU Entity Graph visualization
//!
//! These types mirror the server-side graph types but are optimized for UI rendering.

#![allow(dead_code)]

use egui::{Color32, Pos2, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::spatial::SpatialIndex;

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

impl std::str::FromStr for CbuCategory {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().as_str() {
            "FUND_MANDATE" => Self::FundMandate,
            "CORPORATE_GROUP" => Self::CorporateGroup,
            "INSTITUTIONAL_ACCOUNT" => Self::InstitutionalAccount,
            "RETAIL_CLIENT" => Self::RetailClient,
            "FAMILY_TRUST" => Self::FamilyTrust,
            "INTERNAL_TEST" => Self::InternalTest,
            "CORRESPONDENT_BANK" => Self::CorrespondentBank,
            _ => Self::CorporateGroup, // fallback
        })
    }
}

// =============================================================================
// ROLE TYPES
// =============================================================================

/// Primary role determines slot assignment in templates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrimaryRole {
    // Highest priority - ownership/control (KYC view)
    UltimateBeneficialOwner,
    BeneficialOwner,
    Shareholder,
    GeneralPartner,
    LimitedPartner,
    // Governance (KYC view)
    Director,
    Officer,
    ConductingOfficer,
    ChiefComplianceOfficer,
    Trustee,
    Protector,
    Beneficiary,
    Settlor,
    // Fund structure - trading entities (Service Delivery view)
    Principal,
    AssetOwner,
    MasterFund,
    FeederFund,
    SegregatedPortfolio,
    ManagementCompany,
    InvestmentManager,
    InvestmentAdvisor,
    Sponsor,
    // Service providers (Service Delivery view)
    Administrator,
    Custodian,
    Depositary,
    TransferAgent,
    Distributor,
    PrimeBroker,
    Auditor,
    LegalCounsel,
    // Other
    AuthorizedSignatory,
    ContactPerson,
    CommercialClient,
    Unknown,
}

impl std::str::FromStr for PrimaryRole {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().replace('-', "_").as_str() {
            // Ownership/Control
            "ULTIMATE_BENEFICIAL_OWNER" | "UBO" => Self::UltimateBeneficialOwner,
            "BENEFICIAL_OWNER" => Self::BeneficialOwner,
            "SHAREHOLDER" => Self::Shareholder,
            "GENERAL_PARTNER" | "GP" => Self::GeneralPartner,
            "LIMITED_PARTNER" | "LP" => Self::LimitedPartner,
            // Governance
            "DIRECTOR" => Self::Director,
            "OFFICER" => Self::Officer,
            "CONDUCTING_OFFICER" => Self::ConductingOfficer,
            "CHIEF_COMPLIANCE_OFFICER" | "CCO" => Self::ChiefComplianceOfficer,
            "TRUSTEE" => Self::Trustee,
            "PROTECTOR" => Self::Protector,
            "BENEFICIARY" => Self::Beneficiary,
            "SETTLOR" => Self::Settlor,
            // Fund structure
            "PRINCIPAL" => Self::Principal,
            "ASSET_OWNER" => Self::AssetOwner,
            "MASTER_FUND" => Self::MasterFund,
            "FEEDER_FUND" => Self::FeederFund,
            "SEGREGATED_PORTFOLIO" => Self::SegregatedPortfolio,
            "MANAGEMENT_COMPANY" | "MANCO" => Self::ManagementCompany,
            "INVESTMENT_MANAGER" => Self::InvestmentManager,
            "INVESTMENT_ADVISOR" => Self::InvestmentAdvisor,
            "SPONSOR" => Self::Sponsor,
            // Service providers
            "ADMINISTRATOR" => Self::Administrator,
            "CUSTODIAN" => Self::Custodian,
            "DEPOSITARY" => Self::Depositary,
            "TRANSFER_AGENT" => Self::TransferAgent,
            "DISTRIBUTOR" => Self::Distributor,
            "PRIME_BROKER" => Self::PrimeBroker,
            "AUDITOR" => Self::Auditor,
            "LEGAL_COUNSEL" => Self::LegalCounsel,
            // Other
            "AUTHORIZED_SIGNATORY" => Self::AuthorizedSignatory,
            "CONTACT_PERSON" => Self::ContactPerson,
            "COMMERCIAL_CLIENT" => Self::CommercialClient,
            _ => Self::Unknown,
        })
    }
}

impl PrimaryRole {
    /// Priority for layout ordering (higher = more important = placed first)
    pub fn priority(&self) -> i32 {
        match self {
            // Ownership/control - highest priority
            Self::UltimateBeneficialOwner => 100,
            Self::BeneficialOwner => 95,
            Self::Shareholder => 90,
            Self::GeneralPartner => 88,
            Self::LimitedPartner => 85,
            // Governance
            Self::Director => 70,
            Self::Officer => 65,
            Self::ConductingOfficer => 64,
            Self::ChiefComplianceOfficer => 63,
            Self::Trustee => 55,
            Self::Protector => 50,
            Self::Beneficiary => 45,
            Self::Settlor => 40,
            // Fund structure - trading entities
            Self::Principal => 80,
            Self::AssetOwner => 78,
            Self::MasterFund => 76,
            Self::FeederFund => 74,
            Self::SegregatedPortfolio => 72,
            Self::ManagementCompany => 75,
            Self::InvestmentManager => 73,
            Self::InvestmentAdvisor => 71,
            Self::Sponsor => 69,
            Self::CommercialClient => 77,
            // Service providers
            Self::Administrator => 50,
            Self::Custodian => 48,
            Self::Depositary => 47,
            Self::TransferAgent => 46,
            Self::Distributor => 45,
            Self::PrimeBroker => 44,
            Self::Auditor => 42,
            Self::LegalCounsel => 40,
            // Other
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
    // Trading layer types
    TradingProfile,
    InstrumentMatrix,
    InstrumentClass,
    Market,
    Counterparty,
    IsdaAgreement,
    CsaAgreement,
}

impl std::str::FromStr for EntityType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        Ok(if lower.contains("person") {
            Self::ProperPerson
        } else if lower.contains("company") || lower.contains("limited") {
            Self::LimitedCompany
        } else if lower.contains("partner") {
            Self::Partnership
        } else if lower.contains("trust") {
            Self::Trust
        } else if lower.contains("fund") && !lower.contains("instrument") {
            Self::Fund
        } else if lower == "product" {
            Self::Product
        } else if lower == "service" {
            Self::Service
        } else if lower == "resource" {
            Self::Resource
        // Trading layer types - exact matches
        } else if lower == "trading_profile" || lower == "tradingprofile" {
            Self::TradingProfile
        } else if lower == "instrument_matrix" || lower == "instrumentmatrix" {
            Self::InstrumentMatrix
        } else if lower == "instrument_class" || lower == "instrumentclass" {
            Self::InstrumentClass
        } else if lower == "market" {
            Self::Market
        } else if lower == "counterparty" {
            Self::Counterparty
        } else if lower == "isda_agreement" || lower == "isdaagreement" || lower == "isda" {
            Self::IsdaAgreement
        } else if lower == "csa_agreement" || lower == "csaagreement" || lower == "csa" {
            Self::CsaAgreement
        } else {
            Self::Unknown
        })
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
    /// Role categories: OWNERSHIP_CONTROL, TRADING_EXECUTION, BOTH
    #[serde(default)]
    pub role_categories: Vec<String>,
    pub primary_role: Option<String>,
    pub jurisdiction: Option<String>,
    pub role_priority: Option<i32>,
    #[serde(default)]
    pub data: serde_json::Value,
    /// Server-computed x position (optional, client may recompute)
    #[serde(default)]
    pub x: Option<f64>,
    /// Server-computed y position (optional, client may recompute)
    #[serde(default)]
    pub y: Option<f64>,

    // =========================================================================
    // VISUAL HINTS - from server
    // =========================================================================
    /// Node importance score (0.0 - 1.0) - affects rendered size
    #[serde(default)]
    pub importance: Option<f32>,

    /// Depth in ownership hierarchy
    #[serde(default)]
    pub hierarchy_depth: Option<i32>,

    /// KYC completion percentage (0-100)
    #[serde(default)]
    pub kyc_completion: Option<i32>,

    /// Verification summary
    #[serde(default)]
    pub verification_summary: Option<VerificationSummary>,

    /// Whether this node needs attention
    #[serde(default)]
    pub needs_attention: bool,

    /// Entity category: PERSON or SHELL
    #[serde(default)]
    pub entity_category: Option<String>,

    /// Person state: GHOST, IDENTIFIED, or VERIFIED
    /// Ghost entities have minimal info (name only) and render with dashed/faded style
    #[serde(default)]
    pub person_state: Option<String>,

    // =========================================================================
    // CONTAINER FIELDS - for nodes that contain browseable children
    // =========================================================================
    /// Whether this node is a container (can be double-clicked to browse)
    #[serde(default)]
    pub is_container: bool,

    /// Type of items this container holds (e.g., "investor_holding", "resource_instance")
    #[serde(default)]
    pub contains_type: Option<String>,

    /// Number of child items (for badge display)
    #[serde(default)]
    pub child_count: Option<i64>,

    /// EntityGateway nickname for searching children
    #[serde(default)]
    pub browse_nickname: Option<String>,

    /// Parent key for scoped queries (e.g., cbu_id)
    #[serde(default)]
    pub parent_key: Option<String>,
}

/// Verification status summary for entity relationships
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct VerificationSummary {
    pub total_edges: i32,
    pub proven_edges: i32,
    pub alleged_edges: i32,
    pub disputed_edges: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphEdgeData {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub label: Option<String>,

    // =========================================================================
    // VISUAL HINTS
    // =========================================================================
    /// Ownership percentage (0-100) - affects edge thickness
    #[serde(default)]
    pub weight: Option<f32>,

    /// Verification status - affects line style
    #[serde(default)]
    pub verification_status: Option<String>,
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

    /// Base position from template layout
    pub base_position: Pos2,
    /// User offset from base position
    pub offset: Vec2,
    /// Computed position in graph coordinates (base_position + offset)
    pub position: Pos2,
    /// Base size from template
    pub base_size: Vec2,
    /// User size override
    pub size_override: Option<Vec2>,
    /// Current size (size_override or base_size)
    pub size: Vec2,
    /// Is this node in the current focus set?
    pub in_focus: bool,
    /// Is this the CBU root node?
    pub is_cbu_root: bool,
    /// Visual style
    pub style: NodeStyle,

    // =========================================================================
    // VISUAL HINTS - from server, used for enhanced rendering
    // =========================================================================
    /// Node importance score (0.0 - 1.0) - affects rendered size
    pub importance: f32,

    /// Depth in ownership hierarchy (0 = root, 1+ = chain depth)
    pub hierarchy_depth: i32,

    /// KYC completion percentage (0-100) - affects fill pattern
    pub kyc_completion: Option<i32>,

    /// Verification summary for entity relationships
    pub verification_summary: Option<VerificationSummary>,

    /// Whether this node needs attention (has issues/gaps)
    pub needs_attention: bool,

    /// Entity category: "PERSON" or "SHELL"
    pub entity_category: Option<String>,

    /// Person state: GHOST, IDENTIFIED, or VERIFIED
    /// Ghost entities render with dashed borders and faded fill
    pub person_state: Option<String>,

    // =========================================================================
    // CONTAINER FIELDS - for nodes that contain browseable children
    // =========================================================================
    /// Whether this node is a container (can be double-clicked to browse)
    pub is_container: bool,

    /// Type of items this container holds (e.g., "investor_holding", "resource_instance")
    pub contains_type: Option<String>,

    /// Number of child items (for badge display)
    pub child_count: Option<i64>,

    /// EntityGateway nickname for searching children
    pub browse_nickname: Option<String>,

    /// Parent key for scoped queries (e.g., cbu_id)
    pub parent_key: Option<String>,

    // =========================================================================
    // CONTAINER PARENT - for nodes that visually belong inside a container
    // =========================================================================
    /// ID of the container node this node belongs to (for visual grouping)
    /// Used in SERVICE_DELIVERY view to show entities inside the CBU container
    pub container_parent_id: Option<String>,
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

    // =========================================================================
    // VISUAL HINTS
    // =========================================================================
    /// Ownership percentage (0-100) - affects edge thickness
    pub weight: Option<f32>,

    /// Verification status - affects line style
    /// Values: "proven", "alleged", "disputed", "pending"
    pub verification_status: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    // Core edge types
    HasRole,
    Owns,
    Controls,
    /// UBO chain terminus - ownership tracing stops here (public company, government, etc.)
    UboTerminus,
    // Service layer edge types
    UsesProduct,
    DeliversService,
    ProvidesResource,
    // Trading layer edge types
    HasTradingProfile,
    HasMatrix,
    IncludesClass,
    TradedOn,
    OtcCounterparty,
    CoveredByIsda,
    HasCsa,
    ImMandate,
    Other,
}

impl std::str::FromStr for EdgeType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().replace('-', "_").as_str() {
            "has_role" | "hasrole" => Self::HasRole,
            "owns" => Self::Owns,
            "controls" => Self::Controls,
            "ubo_terminus" | "uboterminus" => Self::UboTerminus,
            // Service layer
            "uses_product" | "usesproduct" => Self::UsesProduct,
            "delivers_service" | "deliversservice" => Self::DeliversService,
            "provides_resource" | "providesresource" => Self::ProvidesResource,
            // Trading layer
            "has_trading_profile" | "hastradingprofile" => Self::HasTradingProfile,
            "has_matrix" | "hasmatrix" => Self::HasMatrix,
            "includes_class" | "includesclass" => Self::IncludesClass,
            "traded_on" | "tradedon" => Self::TradedOn,
            "otc_counterparty" | "otccounterparty" => Self::OtcCounterparty,
            "covered_by_isda" | "coveredbyisda" => Self::CoveredByIsda,
            "has_csa" | "hascsa" => Self::HasCsa,
            "im_mandate" | "immandate" => Self::ImMandate,
            _ => Self::Other,
        })
    }
}

impl EdgeType {
    /// Whether this edge type should render with dashed style
    pub fn is_dashed(&self) -> bool {
        matches!(
            self,
            EdgeType::OtcCounterparty | EdgeType::ImMandate | EdgeType::CoveredByIsda
        )
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

    /// R-tree spatial index for O(log n) hit testing
    spatial_index: SpatialIndex,
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
            spatial_index: SpatialIndex::new(),
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
            spatial_index: SpatialIndex::new(),
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

    /// Recompute bounds and spatial index from all nodes
    pub fn recompute_bounds(&mut self) {
        if self.nodes.is_empty() {
            self.bounds = egui::Rect::NOTHING;
            self.spatial_index = SpatialIndex::new();
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

        // Rebuild spatial index for O(log n) hit testing
        self.rebuild_spatial_index();
    }

    /// Rebuild the spatial index from current node positions
    fn rebuild_spatial_index(&mut self) {
        self.spatial_index = SpatialIndex::from_nodes(self.nodes.values().map(|node| {
            // Use max dimension as radius for circular hit area
            let radius = node.size.x.max(node.size.y) / 2.0;
            super::spatial::SpatialNode::new(
                node.id.clone(),
                [node.position.x, node.position.y],
                radius,
            )
        }));
    }

    /// O(log n) hit test using spatial index
    /// Returns the ID of the node at the given world position, if any
    pub fn hit_test(&self, world_pos: Pos2, threshold: f32) -> Option<&str> {
        self.spatial_index
            .hit_test([world_pos.x, world_pos.y], threshold)
            .map(|node| node.id.as_str())
    }

    /// O(log n) hit test that also checks if point is inside the node's rect
    /// More precise than circular hit test for rectangular nodes
    pub fn hit_test_rect(&self, world_pos: Pos2) -> Option<&str> {
        // First use spatial index to find candidates quickly
        // Use a reasonable threshold to find nearby nodes
        let threshold = 100.0; // Generous threshold to catch nearby nodes

        // Get nearest node from spatial index
        if let Some(spatial_node) = self
            .spatial_index
            .hit_test([world_pos.x, world_pos.y], threshold)
        {
            // Verify with precise rect check
            if let Some(node) = self.nodes.get(&spatial_node.id) {
                let node_rect = egui::Rect::from_center_size(node.position, node.size);
                if node_rect.contains(world_pos) {
                    return Some(&node.id);
                }
            }
        }

        // Fallback: check if we're inside any node's rect that spatial index might have missed
        // This handles edge cases where the nearest circular approximation doesn't match
        for node in self.nodes.values() {
            let node_rect = egui::Rect::from_center_size(node.position, node.size);
            if node_rect.contains(world_pos) {
                return Some(&node.id);
            }
        }

        None
    }
}

// =============================================================================
// VIEW MODE - re-exported from graph_view
// =============================================================================

// ViewMode is defined in graph_view.rs and re-exported through mod.rs

// =============================================================================
// LAYOUT OVERRIDES
// =============================================================================

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct NodeOffset {
    pub node_id: String,
    pub dx: f32,
    pub dy: f32,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct NodeSizeOverride {
    pub node_id: String,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct LayoutOverride {
    #[serde(default)]
    pub positions: Vec<NodeOffset>,
    #[serde(default)]
    pub sizes: Vec<NodeSizeOverride>,
}

impl LayoutGraph {
    /// Apply position/size overrides (offsets relative to template positions)
    pub fn apply_overrides(&mut self, overrides: &LayoutOverride) -> usize {
        let mut applied = 0;
        for off in &overrides.positions {
            if let Some(node) = self.nodes.get_mut(&off.node_id) {
                node.offset = Vec2::new(off.dx, off.dy);
                node.position = node.base_position + node.offset;
                applied += 1;
            }
        }
        for sz in &overrides.sizes {
            if let Some(node) = self.nodes.get_mut(&sz.node_id) {
                let new_size = Vec2::new(sz.w, sz.h);
                node.size_override = Some(new_size);
                node.size = new_size;
                applied += 1;
            }
        }
        if applied > 0 {
            self.recompute_bounds();
        }
        applied
    }
}

// =============================================================================
// CONVERSION FROM SHARED API TYPES
// =============================================================================

impl From<ob_poc_types::CbuGraphResponse> for CbuGraphData {
    fn from(resp: ob_poc_types::CbuGraphResponse) -> Self {
        Self {
            cbu_id: resp.cbu_id.parse().unwrap_or_default(),
            label: resp.label,
            cbu_category: resp.cbu_category,
            jurisdiction: resp.jurisdiction,
            nodes: resp.nodes.into_iter().map(|n| n.into()).collect(),
            edges: resp.edges.into_iter().map(|e| e.into()).collect(),
        }
    }
}

impl From<ob_poc_types::GraphNode> for GraphNodeData {
    fn from(node: ob_poc_types::GraphNode) -> Self {
        Self {
            id: node.id,
            node_type: node.node_type,
            layer: node.layer,
            label: node.label,
            sublabel: node.sublabel,
            status: node.status,
            roles: node.roles,
            role_categories: node.role_categories,
            primary_role: node.primary_role,
            jurisdiction: node.jurisdiction,
            role_priority: node.role_priority,
            data: node.data.unwrap_or_default(),
            x: node.x,
            y: node.y,
            // Visual hints
            importance: node.importance,
            hierarchy_depth: node.hierarchy_depth,
            kyc_completion: node.kyc_completion,
            verification_summary: node.verification_summary.map(|v| VerificationSummary {
                total_edges: v.total_edges,
                proven_edges: v.proven_edges,
                alleged_edges: v.alleged_edges,
                disputed_edges: v.disputed_edges,
            }),
            needs_attention: node.needs_attention,
            entity_category: node.entity_category,
            person_state: node.person_state,
            // Container fields
            is_container: node.is_container,
            contains_type: node.contains_type,
            child_count: node.child_count,
            browse_nickname: node.browse_nickname,
            parent_key: node.parent_key,
        }
    }
}

impl From<ob_poc_types::GraphEdge> for GraphEdgeData {
    fn from(edge: ob_poc_types::GraphEdge) -> Self {
        Self {
            id: edge.id,
            source: edge.source,
            target: edge.target,
            edge_type: edge.edge_type,
            label: edge.label,
            // Visual hints
            weight: edge.weight,
            verification_status: edge.verification_status,
        }
    }
}
