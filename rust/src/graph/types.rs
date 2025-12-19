//! Graph types for CBU visualization
//!
//! These types define the intermediate representation for graph data
//! that is serialized to JSON and consumed by the egui WASM client.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Graph projection of a CBU for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuGraph {
    pub cbu_id: Uuid,
    pub label: String,
    /// CBU category for template selection (FUND_MANDATE, CORPORATE_GROUP, etc.)
    pub cbu_category: Option<String>,
    pub jurisdiction: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub layers: Vec<LayerInfo>,
    pub stats: GraphStats,
}

/// A node in the graph representing an entity, document, or resource
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphNode {
    pub id: String,
    pub node_type: NodeType,
    pub layer: LayerType,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: NodeStatus,
    pub data: serde_json::Value,
    /// Parent node ID for hierarchical grouping (e.g., market groups custody items)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// All roles for this entity (for Entity nodes)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    /// Role categories: OWNERSHIP_CONTROL, TRADING_EXECUTION, BOTH
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_categories: Vec<String>,
    /// Primary role determined by priority (for Entity nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_role: Option<String>,
    /// Jurisdiction code (for Entity nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// Role priority score for layout ordering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_priority: Option<i32>,
    /// Entity category: SHELL (legal vehicles) or PERSON (natural persons)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_category: Option<String>,

    // =========================================================================
    // LAYOUT FIELDS - computed by server-side LayoutEngine
    // =========================================================================
    /// X position (computed by layout engine)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    /// Y position (computed by layout engine)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    /// Node width (computed by layout engine)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    /// Node height (computed by layout engine)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    /// Layout tier (0 = top, higher = lower on screen)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_tier: Option<i32>,

    // =========================================================================
    // VISUAL HINT FIELDS - computed by server-side builder for rendering
    // =========================================================================
    /// Node importance score (0.0 - 1.0) - affects rendered size
    /// Computed based on role priority, UBO status, connection count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,

    /// KYC completion percentage (0-100) for progress bar rendering
    /// Only set for Entity nodes with KYC status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyc_completion: Option<i32>,

    /// Verification status for edge styling
    /// Values: "proven", "pending", "alleged", "disputed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,

    // =========================================================================
    // CONTAINER FIELDS - for nodes that contain browseable children
    // =========================================================================
    /// Is this node a container with browseable contents?
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_container: bool,

    /// Entity type of children (e.g., "investor_holding", "service_resource")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_type: Option<String>,

    /// Number of children (for badge display)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i64>,

    /// EntityGateway nickname for browsing children
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browse_nickname: Option<String>,

    /// Parent key field name for scoped queries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_key: Option<String>,
}

/// Helper for serde skip_serializing_if
fn is_false(b: &bool) -> bool {
    !*b
}

/// Types of nodes in the graph
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    #[default]
    // Core
    Cbu,

    // Custody
    Market, // Grouping node for market
    Universe,
    Ssi,
    BookingRule,
    Isda,
    Csa,
    Subcustodian,

    // KYC
    Document,
    Attribute,
    Verification,

    // UBO
    Entity,
    OwnershipLink,

    // Services
    Product,
    Service,
    Resource,

    // Container types (browseable via slide-in panel)
    ShareClass,      // Contains investor holdings
    ServiceInstance, // Contains resource instances

    // Container item types (for detail views)
    InvestorHolding,
    ServiceResource,
}

/// Layer categories for organizing nodes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayerType {
    #[default]
    Core,
    Custody,
    Kyc,
    Ubo,
    Services,
}

/// Status of a node
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    #[default]
    Active,
    Pending,
    Suspended,
    Expired,
    Draft,
}

/// An edge connecting two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

/// Types of edges representing relationships
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    // =========================================================================
    // CORE RELATIONSHIPS
    // =========================================================================
    /// Entity has a role within a CBU
    HasRole,

    // =========================================================================
    // OWNERSHIP & CONTROL (from entity_relationships table)
    // =========================================================================
    /// Direct or indirect ownership (percentage-based)
    Owns,
    /// Control relationship (voting rights, board control, veto powers)
    Controls,
    /// Trust relationship types
    TrustSettlor,
    TrustTrustee,
    TrustBeneficiary,
    TrustProtector,

    // =========================================================================
    // FUND STRUCTURE (from fund_structure, fund_investments tables)
    // =========================================================================
    /// Management company manages a fund
    ManagedBy,
    /// Administrator provides admin services
    AdministeredBy,
    /// Custodian provides custody services
    CustodiedBy,
    /// Entity uses a product
    UsesProduct,
    /// Feeder fund invests in master fund
    FeederTo,
    /// Fund of funds investment
    InvestsIn,
    /// Umbrella contains subfund
    Contains,
    /// Share class belongs to fund
    ShareClassOf,

    // =========================================================================
    // CUSTODY & SETTLEMENT (from custody schema)
    // =========================================================================
    /// Booking rule routes to SSI
    RoutesTo,
    /// Booking rule matches universe entry
    Matches,
    /// ISDA covers product types
    CoveredBy,
    /// CSA secures ISDA
    SecuredBy,
    /// SSI settles at market/CSD
    SettlesAt,
    /// Subcustodian relationship
    SubcustodianOf,

    // =========================================================================
    // KYC & DOCUMENTS
    // =========================================================================
    /// Document requirement
    Requires,
    /// Document validates attribute
    Validates,
    /// Allegation verified by observation
    VerifiedBy,
    /// Observation contradicts another
    Contradicts,

    // =========================================================================
    // SERVICES & DELIVERY
    // =========================================================================
    /// Service delivers capability
    Delivers,
    /// Resource belongs to service
    BelongsTo,
    /// Service instance provisioned for CBU
    ProvisionedFor,

    // =========================================================================
    // DELEGATION (from delegation_relationships table)
    // =========================================================================
    /// Delegation of authority/responsibility
    DelegatesTo,
}

impl EdgeType {
    /// Get the edge type from a relationship_type string (from entity_relationships table)
    pub fn from_relationship_type(rel_type: &str) -> Option<Self> {
        match rel_type.to_lowercase().as_str() {
            "ownership" => Some(EdgeType::Owns),
            "control" => Some(EdgeType::Controls),
            "trust_settlor" | "settlor" => Some(EdgeType::TrustSettlor),
            "trust_trustee" | "trustee" => Some(EdgeType::TrustTrustee),
            "trust_beneficiary" | "beneficiary" => Some(EdgeType::TrustBeneficiary),
            "trust_protector" | "protector" => Some(EdgeType::TrustProtector),
            _ => None,
        }
    }

    /// Get the edge type from a fund structure relationship type
    pub fn from_fund_structure_type(rel_type: &str) -> Option<Self> {
        match rel_type.to_lowercase().as_str() {
            "contains" => Some(EdgeType::Contains),
            "master_feeder" | "feeder_to" => Some(EdgeType::FeederTo),
            "invests_in" => Some(EdgeType::InvestsIn),
            _ => None,
        }
    }

    /// Check if this edge type represents an ownership/control relationship
    pub fn is_ownership_or_control(&self) -> bool {
        matches!(
            self,
            EdgeType::Owns
                | EdgeType::Controls
                | EdgeType::TrustSettlor
                | EdgeType::TrustTrustee
                | EdgeType::TrustBeneficiary
                | EdgeType::TrustProtector
        )
    }

    /// Check if this edge type represents a fund structure relationship
    pub fn is_fund_structure(&self) -> bool {
        matches!(
            self,
            EdgeType::ManagedBy
                | EdgeType::AdministeredBy
                | EdgeType::CustodiedBy
                | EdgeType::UsesProduct
                | EdgeType::FeederTo
                | EdgeType::InvestsIn
                | EdgeType::Contains
                | EdgeType::ShareClassOf
        )
    }

    /// Check if this edge type represents a custody relationship
    pub fn is_custody(&self) -> bool {
        matches!(
            self,
            EdgeType::RoutesTo
                | EdgeType::Matches
                | EdgeType::CoveredBy
                | EdgeType::SecuredBy
                | EdgeType::SettlesAt
                | EdgeType::SubcustodianOf
        )
    }

    /// Get the display label for this edge type
    pub fn display_label(&self) -> &'static str {
        match self {
            EdgeType::HasRole => "has role",
            EdgeType::Owns => "owns",
            EdgeType::Controls => "controls",
            EdgeType::TrustSettlor => "settlor of",
            EdgeType::TrustTrustee => "trustee of",
            EdgeType::TrustBeneficiary => "beneficiary of",
            EdgeType::TrustProtector => "protector of",
            EdgeType::ManagedBy => "managed by",
            EdgeType::AdministeredBy => "administered by",
            EdgeType::CustodiedBy => "custodied by",
            EdgeType::UsesProduct => "uses",
            EdgeType::FeederTo => "feeds into",
            EdgeType::InvestsIn => "invests in",
            EdgeType::Contains => "contains",
            EdgeType::ShareClassOf => "share class of",
            EdgeType::RoutesTo => "routes to",
            EdgeType::Matches => "matches",
            EdgeType::CoveredBy => "covered by",
            EdgeType::SecuredBy => "secured by",
            EdgeType::SettlesAt => "settles at",
            EdgeType::SubcustodianOf => "subcustodian of",
            EdgeType::Requires => "requires",
            EdgeType::Validates => "validates",
            EdgeType::VerifiedBy => "verified by",
            EdgeType::Contradicts => "contradicts",
            EdgeType::Delivers => "delivers",
            EdgeType::BelongsTo => "belongs to",
            EdgeType::ProvisionedFor => "provisioned for",
            EdgeType::DelegatesTo => "delegates to",
        }
    }
}

/// Information about a layer for UI rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerInfo {
    pub layer_type: LayerType,
    pub label: String,
    pub color: String,
    pub node_count: usize,
    pub visible: bool,
}

/// Statistics about the graph
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_layer: HashMap<String, usize>,
    pub nodes_by_type: HashMap<String, usize>,
}

impl CbuGraph {
    /// Create a new empty graph for a CBU
    pub fn new(cbu_id: Uuid, label: String) -> Self {
        Self {
            cbu_id,
            label,
            cbu_category: None,
            jurisdiction: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            layers: Vec::new(),
            stats: GraphStats::default(),
        }
    }

    /// Create a new graph with category and jurisdiction
    pub fn with_metadata(
        cbu_id: Uuid,
        label: String,
        cbu_category: Option<String>,
        jurisdiction: Option<String>,
    ) -> Self {
        Self {
            cbu_id,
            label,
            cbu_category,
            jurisdiction,
            nodes: Vec::new(),
            edges: Vec::new(),
            layers: Vec::new(),
            stats: GraphStats::default(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.push(node);
    }

    /// Check if a node with the given ID exists
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n.id == id)
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
    }

    /// Compute statistics for the graph
    pub fn compute_stats(&mut self) {
        self.stats.total_nodes = self.nodes.len();
        self.stats.total_edges = self.edges.len();

        self.stats.nodes_by_layer.clear();
        self.stats.nodes_by_type.clear();

        for node in &self.nodes {
            let layer_key = format!("{:?}", node.layer).to_lowercase();
            let type_key = format!("{:?}", node.node_type).to_lowercase();

            *self.stats.nodes_by_layer.entry(layer_key).or_insert(0) += 1;
            *self.stats.nodes_by_type.entry(type_key).or_insert(0) += 1;
        }
    }

    /// Filter graph to products only (removes services, resources, entities)
    /// Used for PRODUCTS_ONLY view mode
    pub fn filter_to_products_only(&mut self) {
        // Keep only CBU and Product nodes
        let kept_node_ids: std::collections::HashSet<String> = self
            .nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::Cbu | NodeType::Product))
            .map(|n| n.id.clone())
            .collect();

        self.nodes
            .retain(|n| matches!(n.node_type, NodeType::Cbu | NodeType::Product));

        // Keep only edges where both source and target are kept
        self.edges
            .retain(|e| kept_node_ids.contains(&e.source) && kept_node_ids.contains(&e.target));
    }

    /// Filter graph to UBO/ownership edges only
    /// Used for UBO_ONLY view mode - shows pure ownership/control graph
    ///
    /// Edge types kept:
    /// - Owns (ownership relationships)
    /// - Controls (control relationships from ubo_edges)
    /// - HasRole WHERE role indicates control (DIRECTOR, CEO, UBO, SHAREHOLDER, etc.)
    ///
    /// Edge types removed:
    /// - HasRole for trading/operational roles (CUSTODIAN, INVESTMENT_MANAGER, etc.)
    /// - Delivers, RoutesTo, etc. (service delivery edges)
    pub fn filter_to_ubo_only(&mut self) {
        // Roles that indicate ownership or control (from OWNERSHIP_CONTROL category)
        let control_roles = [
            "DIRECTOR",
            "NOMINEE_DIRECTOR",
            "INDEPENDENT_TRUSTEE",
            "INTERESTED_TRUSTEE",
            "UBO",
            "BENEFICIAL_OWNER",
            "SHAREHOLDER",
            "SETTLOR",
            "TRUSTEE",
            "BENEFICIARY",
            "PROTECTOR",
            "GENERAL_PARTNER",
            "LIMITED_PARTNER",
            "MANAGING_PARTNER",
            "ASSET_OWNER",
            "AUTHORIZED_SIGNATORY",
            "CHIEF_COMPLIANCE_OFFICER",
            "CONDUCTING_OFFICER",
            "CORPORATE_SECRETARY",
            "HOLDING_COMPANY",
            "SUBSIDIARY",
            "SPONSOR",
            "MANCO",
            "MANAGEMENT_COMPANY",
            // Dual-purpose roles
            "PRINCIPAL",
            "COMMERCIAL_CLIENT",
        ];

        // Keep ownership, control, and control-indicating role edges
        self.edges.retain(|e| match e.edge_type {
            EdgeType::Owns | EdgeType::Controls => true,
            EdgeType::HasRole => {
                // Keep role edge if it's a control-indicating role
                e.label
                    .as_ref()
                    .map(|role| control_roles.contains(&role.as_str()))
                    .unwrap_or(false)
            }
            _ => false,
        });

        // Collect node IDs that are still connected after edge filtering
        let connected_node_ids: std::collections::HashSet<String> = self
            .edges
            .iter()
            .flat_map(|e| [e.source.clone(), e.target.clone()])
            .collect();

        // Keep CBU (root) and any entity connected via ownership/control/role
        self.nodes
            .retain(|n| matches!(n.node_type, NodeType::Cbu) || connected_node_ids.contains(&n.id));
    }

    /// Filter entities to trading execution roles only (removes ownership/control roles)
    /// Used for SERVICE_DELIVERY view mode
    /// Keeps entities with role_category: TRADING_EXECUTION, FUND_OPERATIONS, DISTRIBUTION, etc.
    /// Removes entities with only OWNERSHIP_CONTROL roles
    pub fn filter_to_trading_entities(&mut self) {
        // Categories considered "trading" for service delivery view
        let trading_categories = [
            "TRADING_EXECUTION",
            "FUND_OPERATIONS",
            "DISTRIBUTION",
            "FINANCING",
            "INVESTMENT",
            "BOTH", // Dual-purpose roles like PRINCIPAL
        ];

        // Find entity nodes that have at least one trading role
        let trading_entity_ids: std::collections::HashSet<String> = self
            .nodes
            .iter()
            .filter(|n| {
                n.node_type == NodeType::Entity
                    && n.role_categories
                        .iter()
                        .any(|cat| trading_categories.contains(&cat.as_str()))
            })
            .map(|n| n.id.clone())
            .collect();

        // Remove entities that don't have trading roles
        self.nodes
            .retain(|n| n.node_type != NodeType::Entity || trading_entity_ids.contains(&n.id));

        // Remove edges to/from removed entities
        self.edges.retain(|e| {
            // Keep edge if it's not a HasRole edge OR if the entity is a trading entity
            if e.edge_type != EdgeType::HasRole {
                true
            } else {
                // For HasRole edges, keep only if target is a trading entity
                trading_entity_ids.contains(&e.target)
            }
        });
    }

    /// Compute visual hints (importance, kyc_completion, verification_status)
    /// Call this after all nodes and edges are added
    pub fn compute_visual_hints(&mut self) {
        // Build edge count map for connectivity-based importance
        let mut edge_counts: HashMap<String, usize> = HashMap::new();
        for edge in &self.edges {
            *edge_counts.entry(edge.source.clone()).or_insert(0) += 1;
            *edge_counts.entry(edge.target.clone()).or_insert(0) += 1;
        }

        // Find max edge count for normalization
        let max_edges = edge_counts.values().max().copied().unwrap_or(1) as f32;

        for node in &mut self.nodes {
            // Compute importance based on multiple factors
            let mut importance: f32 = 0.5; // Base importance

            // Factor 1: Node type priority
            importance += match node.node_type {
                NodeType::Cbu => 0.5,     // Root is always important
                NodeType::Entity => 0.3,  // Entities are key
                NodeType::Product => 0.2, // Products matter
                NodeType::Isda => 0.15,   // ISDA agreements
                NodeType::Ssi => 0.1,     // SSIs
                _ => 0.0,
            };

            // Factor 2: Role priority (for entities)
            if let Some(priority) = node.role_priority {
                // Higher priority (lower number) = more important
                // Priority ranges from ~10 (ownership) to ~100 (trading)
                let priority_boost = ((100 - priority) as f32 / 100.0) * 0.2;
                importance += priority_boost;
            }

            // Factor 3: UBO/ownership roles boost importance
            if node
                .role_categories
                .contains(&"OWNERSHIP_CONTROL".to_string())
            {
                importance += 0.1;
            }

            // Factor 4: Connectivity (more connections = more important)
            if let Some(&count) = edge_counts.get(&node.id) {
                let connectivity_boost = (count as f32 / max_edges) * 0.15;
                importance += connectivity_boost;
            }

            // Factor 5: Status affects importance (pending/draft less important)
            match node.status {
                NodeStatus::Active => {} // No change
                NodeStatus::Pending => importance -= 0.1,
                NodeStatus::Draft => importance -= 0.15,
                NodeStatus::Suspended => importance -= 0.2,
                NodeStatus::Expired => importance -= 0.25,
            }

            // Clamp to 0.0 - 1.0
            node.importance = Some(importance.clamp(0.0, 1.0));
        }
    }

    /// Build layer information for UI rendering
    pub fn build_layer_info(&mut self) {
        self.layers = vec![
            LayerInfo {
                layer_type: LayerType::Core,
                label: "Core".to_string(),
                color: "#6B7280".to_string(), // Gray
                node_count: self.stats.nodes_by_layer.get("core").copied().unwrap_or(0),
                visible: true,
            },
            LayerInfo {
                layer_type: LayerType::Custody,
                label: "Custody".to_string(),
                color: "#3B82F6".to_string(), // Blue
                node_count: self
                    .stats
                    .nodes_by_layer
                    .get("custody")
                    .copied()
                    .unwrap_or(0),
                visible: true,
            },
            LayerInfo {
                layer_type: LayerType::Kyc,
                label: "KYC".to_string(),
                color: "#8B5CF6".to_string(), // Purple
                node_count: self.stats.nodes_by_layer.get("kyc").copied().unwrap_or(0),
                visible: false,
            },
            LayerInfo {
                layer_type: LayerType::Ubo,
                label: "UBO".to_string(),
                color: "#10B981".to_string(), // Green
                node_count: self.stats.nodes_by_layer.get("ubo").copied().unwrap_or(0),
                visible: false,
            },
            LayerInfo {
                layer_type: LayerType::Services,
                label: "Services".to_string(),
                color: "#F59E0B".to_string(), // Amber
                node_count: self
                    .stats
                    .nodes_by_layer
                    .get("services")
                    .copied()
                    .unwrap_or(0),
                visible: false,
            },
        ];
    }
}

/// Summary of a CBU for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

// =============================================================================
// LAYOUT OVERRIDES (positions/sizes saved by UI)
// =============================================================================

/// Per-node position offset from template layout
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeOffset {
    pub node_id: String,
    pub dx: f32,
    pub dy: f32,
}

/// Per-node size override
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeSizeOverride {
    pub node_id: String,
    pub w: f32,
    pub h: f32,
}

/// Saved layout overrides for a CBU/view
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct LayoutOverride {
    #[serde(default)]
    pub positions: Vec<NodeOffset>,
    #[serde(default)]
    pub sizes: Vec<NodeSizeOverride>,
}
