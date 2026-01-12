//! Unified Graph Types for CBU + UBO Navigation
//!
//! This module provides a unified `EntityGraph` structure that handles both:
//! - CBU container visualization (existing functionality)
//! - UBO forest navigation (new functionality)
//!
//! ## Key Design Principles
//!
//! 1. **Single Graph, Multiple Views** - One EntityGraph can be viewed as:
//!    - CBU container (entities grouped inside CBU boxes)
//!    - UBO forest (ownership pyramids with control overlays)
//!    - Fund structure (umbrella → subfund trees)
//!
//! 2. **Typed Edges** - Different edge types for different relationships:
//!    - `OwnershipEdge` - percentage-based ownership
//!    - `ControlEdge` - control relationships (board, voting, etc.)
//!    - `FundEdge` - fund structure (contains, feeder, share class)
//!    - `ServiceEdge` - service delivery relationships
//!    - `RoleAssignment` - entity roles within CBUs
//!
//! 3. **Temporal Awareness** - All relationships support effective dates
//!    and can be filtered by `as_of_date`
//!
//! 4. **Server-Side Layout** - Layout positions are computed server-side
//!    and included in the response for the egui client to render

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::str::FromStr;
use uuid::Uuid;

// Re-export navigation types (will be created in navigation.rs)
// pub use crate::graph::navigation::NavigationHistory;

// =============================================================================
// CORE GRAPH STRUCTURE
// =============================================================================

/// Unified entity graph supporting CBU container and UBO forest views
///
/// This is the primary graph structure that replaces the old `CbuGraph`.
/// It can represent:
/// - A single CBU with its entities and relationships
/// - An entire "book" (all CBUs under an ownership apex)
/// - A jurisdiction scope
/// - An entity neighborhood (N hops from a focal entity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityGraph {
    /// All entity nodes, indexed by entity_id
    pub nodes: HashMap<Uuid, GraphNode>,

    /// CBU container nodes (for CBU container view)
    pub cbus: HashMap<Uuid, CbuNode>,

    /// Ownership relationships (percentage-based)
    pub ownership_edges: Vec<OwnershipEdge>,

    /// Control relationships (board, voting, veto)
    pub control_edges: Vec<ControlEdge>,

    /// Fund structure relationships (contains, feeder, share class)
    pub fund_edges: Vec<FundEdge>,

    /// Service delivery relationships
    pub service_edges: Vec<ServiceEdge>,

    /// Role assignments (entity ↔ CBU roles)
    pub role_assignments: Vec<RoleAssignment>,

    /// Current cursor position for navigation
    pub cursor: Option<Uuid>,

    /// Navigation history for back/forward
    pub history: NavigationHistory,

    /// Active filters
    pub filters: GraphFilters,

    /// Current scope (what data is loaded)
    pub scope: GraphScope,

    /// Terminus entities (no parent owners - UBO apex points)
    pub termini: Vec<Uuid>,

    /// Commercial client entities (apex of CBU ownership)
    pub commercial_clients: Vec<Uuid>,

    /// Graph statistics
    pub stats: GraphStats,
}

impl Default for EntityGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityGraph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            cbus: HashMap::new(),
            ownership_edges: Vec::new(),
            control_edges: Vec::new(),
            fund_edges: Vec::new(),
            service_edges: Vec::new(),
            role_assignments: Vec::new(),
            cursor: None,
            history: NavigationHistory::new(),
            filters: GraphFilters::default(),
            scope: GraphScope::Empty,
            termini: Vec::new(),
            commercial_clients: Vec::new(),
            stats: GraphStats::default(),
        }
    }

    /// Create a graph with a specific scope
    pub fn with_scope(scope: GraphScope) -> Self {
        let mut graph = Self::new();
        graph.scope = scope;
        graph
    }

    /// Add an entity node to the graph
    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.insert(node.entity_id, node);
    }

    /// Add a CBU container node
    pub fn add_cbu(&mut self, cbu: CbuNode) {
        self.cbus.insert(cbu.cbu_id, cbu);
    }

    /// Get a node by ID
    pub fn get_node(&self, entity_id: &Uuid) -> Option<&GraphNode> {
        self.nodes.get(entity_id)
    }

    /// Get a mutable node by ID
    pub fn get_node_mut(&mut self, entity_id: &Uuid) -> Option<&mut GraphNode> {
        self.nodes.get_mut(entity_id)
    }

    /// Get a CBU by ID
    pub fn get_cbu(&self, cbu_id: &Uuid) -> Option<&CbuNode> {
        self.cbus.get(cbu_id)
    }

    /// Check if a node exists
    pub fn has_node(&self, entity_id: &Uuid) -> bool {
        self.nodes.contains_key(entity_id)
    }

    /// Add an ownership edge
    pub fn add_ownership_edge(&mut self, edge: OwnershipEdge) {
        self.ownership_edges.push(edge);
    }

    /// Add a control edge
    pub fn add_control_edge(&mut self, edge: ControlEdge) {
        self.control_edges.push(edge);
    }

    /// Add a fund structure edge
    pub fn add_fund_edge(&mut self, edge: FundEdge) {
        self.fund_edges.push(edge);
    }

    /// Add a service edge
    pub fn add_service_edge(&mut self, edge: ServiceEdge) {
        self.service_edges.push(edge);
    }

    /// Add a role assignment
    pub fn add_role_assignment(&mut self, role: RoleAssignment) {
        self.role_assignments.push(role);
    }

    /// Compute statistics for the graph
    pub fn compute_stats(&mut self) {
        let total_edges = self.ownership_edges.len()
            + self.control_edges.len()
            + self.fund_edges.len()
            + self.service_edges.len()
            + self.role_assignments.len();

        self.stats = GraphStats {
            total_nodes: self.nodes.len(),
            total_edges,
            nodes_by_layer: HashMap::new(),
            nodes_by_type: HashMap::new(),
            cbu_count: self.cbus.len(),
            terminus_count: self.termini.len(),
        };

        // Count by entity type
        for node in self.nodes.values() {
            let type_key = format!("{:?}", node.entity_type).to_lowercase();
            *self.stats.nodes_by_type.entry(type_key).or_insert(0) += 1;
        }
    }

    /// Compute depth_from_terminus for all nodes using BFS from termini
    ///
    /// A terminus is a node with no owners (top of ownership chain).
    /// Depth 0 = terminus, depth N = N ownership hops from nearest terminus.
    pub fn compute_depths(&mut self) {
        use std::collections::VecDeque;

        // First, identify termini (nodes with no owners) and reset depths
        self.termini.clear();
        for node in self.nodes.values_mut() {
            if node.owners.is_empty() {
                // Terminus: depth 0
                node.depth_from_terminus = Some(0);
                self.termini.push(node.entity_id);
            } else {
                // Non-terminus: will be computed via BFS
                node.depth_from_terminus = None;
            }
        }

        // BFS from termini to compute depths
        let mut queue: VecDeque<(Uuid, u32)> = self.termini.iter().map(|id| (*id, 0)).collect();

        while let Some((current_id, depth)) = queue.pop_front() {
            // Get entities owned by current node
            let owned_ids: Vec<Uuid> = self
                .nodes
                .get(&current_id)
                .map(|n| n.owned.clone())
                .unwrap_or_default();

            for owned_id in owned_ids {
                if let Some(owned_node) = self.nodes.get_mut(&owned_id) {
                    // Only update if not yet visited or found shorter path
                    let new_depth = depth + 1;
                    if owned_node.depth_from_terminus.is_none()
                        || owned_node.depth_from_terminus.unwrap() > new_depth
                    {
                        owned_node.depth_from_terminus = Some(new_depth);
                        queue.push_back((owned_id, new_depth));
                    }
                }
            }
        }

        // Update stats
        self.stats.terminus_count = self.termini.len();
    }

    /// Rebuild adjacency lists from edges
    ///
    /// Call this after loading edges to populate the owners/owned/controls/controlled_by
    /// vectors in each node.
    pub fn rebuild_adjacency(&mut self) {
        // Clear existing adjacency lists
        for node in self.nodes.values_mut() {
            node.owners.clear();
            node.owned.clear();
            node.controls.clear();
            node.controlled_by.clear();
        }

        // Populate from ownership edges
        for edge in &self.ownership_edges {
            if let Some(owned_node) = self.nodes.get_mut(&edge.to_entity_id) {
                owned_node.owners.push(edge.from_entity_id);
            }
            if let Some(owner_node) = self.nodes.get_mut(&edge.from_entity_id) {
                owner_node.owned.push(edge.to_entity_id);
            }
        }

        // Populate from control edges
        for edge in &self.control_edges {
            if let Some(controlled_node) = self.nodes.get_mut(&edge.controlled_id) {
                controlled_node.controlled_by.push(edge.controller_id);
            }
            if let Some(controller_node) = self.nodes.get_mut(&edge.controller_id) {
                controller_node.controls.push(edge.controlled_id);
            }
        }
    }

    /// Load an EntityGraph from the database based on scope
    ///
    /// This is the main entry point for loading graphs. It delegates to the
    /// GraphRepository based on the scope type and then performs post-processing:
    /// - Rebuilds adjacency lists from edges
    /// - Computes depths from termini
    /// - Computes statistics
    #[cfg(feature = "database")]
    pub async fn load(
        scope: GraphScope,
        repo: &impl crate::database::GraphRepository,
    ) -> anyhow::Result<Self> {
        use chrono::Local;

        let as_of = Local::now().date_naive();

        let mut graph = match &scope {
            GraphScope::SingleCbu { cbu_id, .. } => repo.load_cbu_graph(*cbu_id, as_of).await?,
            GraphScope::Book { apex_entity_id, .. } => {
                repo.load_book_graph(*apex_entity_id, as_of).await?
            }
            GraphScope::Jurisdiction { code } => repo.load_jurisdiction_graph(code, as_of).await?,
            GraphScope::EntityNeighborhood { entity_id, hops } => {
                repo.load_neighborhood_graph(*entity_id, *hops, as_of)
                    .await?
            }
            GraphScope::Empty => return Ok(Self::new()),
            GraphScope::Custom { .. } => {
                return Err(anyhow::anyhow!("Custom scope requires explicit loading"))
            }
        };

        // Set the scope on the loaded graph
        graph.scope = scope;

        // Rebuild adjacency lists from loaded edges
        graph.rebuild_adjacency();

        // Compute depths from termini
        graph.compute_depths();

        // Compute statistics
        graph.compute_stats();

        Ok(graph)
    }

    /// Load with a specific as-of date for temporal queries
    #[cfg(feature = "database")]
    pub async fn load_as_of(
        scope: GraphScope,
        as_of: NaiveDate,
        repo: &impl crate::database::GraphRepository,
    ) -> anyhow::Result<Self> {
        let mut graph = match &scope {
            GraphScope::SingleCbu { cbu_id, .. } => repo.load_cbu_graph(*cbu_id, as_of).await?,
            GraphScope::Book { apex_entity_id, .. } => {
                repo.load_book_graph(*apex_entity_id, as_of).await?
            }
            GraphScope::Jurisdiction { code } => repo.load_jurisdiction_graph(code, as_of).await?,
            GraphScope::EntityNeighborhood { entity_id, hops } => {
                repo.load_neighborhood_graph(*entity_id, *hops, as_of)
                    .await?
            }
            GraphScope::Empty => return Ok(Self::new()),
            GraphScope::Custom { .. } => {
                return Err(anyhow::anyhow!("Custom scope requires explicit loading"))
            }
        };

        graph.scope = scope;
        graph.filters.as_of_date = as_of;
        graph.rebuild_adjacency();
        graph.compute_depths();
        graph.compute_stats();

        Ok(graph)
    }

    // =========================================================================
    // LAYOUT METHODS
    // =========================================================================

    /// Layout configuration constants
    const NODE_SPACING_X: f32 = 180.0;
    const TIER_SPACING_Y: f32 = 120.0;
    const NODE_WIDTH: f32 = 160.0;
    const NODE_HEIGHT: f32 = 60.0;
    const CANVAS_WIDTH: f32 = 1200.0;
    const SHELL_MARGIN_LEFT: f32 = 100.0;
    const PERSON_MARGIN_RIGHT: f32 = 100.0;

    /// Apply layout to the graph based on view mode
    ///
    /// This positions all nodes based on their role categories and the
    /// LayoutBehavior derived from the taxonomy.
    pub fn layout(&mut self, view_mode: &str, orientation: &str) {
        match (
            view_mode.to_uppercase().as_str(),
            orientation.to_uppercase().as_str(),
        ) {
            ("KYC_UBO" | "KYC", "VERTICAL" | "TTB") => self.layout_kyc_ubo_vertical(),
            ("KYC_UBO" | "KYC", "HORIZONTAL" | "LTR") => self.layout_kyc_ubo_horizontal(),
            ("UBO_ONLY" | "UBO", "VERTICAL" | "TTB") => self.layout_ubo_only_vertical(),
            ("UBO_ONLY" | "UBO", _) => self.layout_ubo_only_horizontal(),
            ("BOOK", _) => self.layout_book_view(),
            _ => self.layout_kyc_ubo_vertical(), // Default
        }
    }

    /// Get the layout behavior for a node
    fn get_node_layout_behavior(&self, entity_id: &Uuid) -> LayoutBehavior {
        if let Some(node) = self.nodes.get(entity_id) {
            // First try explicit layout_behavior
            if let Some(ref behavior) = node.layout_behavior {
                return *behavior;
            }
            // Then try to derive from primary_role_category
            if let Some(ref category) = node.primary_role_category {
                return category.layout_behavior();
            }
        }
        LayoutBehavior::Peripheral
    }

    /// Check if a node is a SHELL entity (vs PERSON)
    fn is_shell_entity(&self, entity_id: &Uuid) -> bool {
        self.nodes
            .get(entity_id)
            .map(|n| !n.is_natural_person)
            .unwrap_or(false)
    }

    /// KYC/UBO layout (VERTICAL): Hierarchical by LayoutBehavior with SHELL/PERSON split
    fn layout_kyc_ubo_vertical(&mut self) {
        let center_x = Self::CANVAS_WIDTH / 2.0;

        // Collect nodes by tier based on LayoutBehavior
        let mut tier_cbu: Vec<Uuid> = Vec::new();
        let mut tier_2_shell: Vec<Uuid> = Vec::new();
        let mut tier_2_person: Vec<Uuid> = Vec::new();
        let mut tier_3_shell: Vec<Uuid> = Vec::new();
        let mut tier_3_person: Vec<Uuid> = Vec::new();
        let mut tier_4_shell: Vec<Uuid> = Vec::new();
        let mut tier_4_person: Vec<Uuid> = Vec::new();
        let mut tier_5_shell: Vec<Uuid> = Vec::new();
        let mut tier_5_person: Vec<Uuid> = Vec::new();
        let mut tier_peripheral: Vec<Uuid> = Vec::new();

        // Collect all entity IDs first to avoid borrow issues
        let entity_ids: Vec<Uuid> = self.nodes.keys().copied().collect();

        for entity_id in entity_ids {
            let is_shell = self.is_shell_entity(&entity_id);
            let behavior = self.get_node_layout_behavior(&entity_id);

            match behavior {
                LayoutBehavior::PyramidUp | LayoutBehavior::PyramidDown => {
                    if is_shell {
                        tier_2_shell.push(entity_id);
                    } else {
                        tier_2_person.push(entity_id);
                    }
                }
                LayoutBehavior::TreeDown | LayoutBehavior::Overlay => {
                    if is_shell {
                        tier_3_shell.push(entity_id);
                    } else {
                        tier_3_person.push(entity_id);
                    }
                }
                LayoutBehavior::Satellite | LayoutBehavior::Radial => {
                    if is_shell {
                        tier_4_shell.push(entity_id);
                    } else {
                        tier_4_person.push(entity_id);
                    }
                }
                LayoutBehavior::FlatBottom | LayoutBehavior::FlatRight => {
                    if is_shell {
                        tier_5_shell.push(entity_id);
                    } else {
                        tier_5_person.push(entity_id);
                    }
                }
                LayoutBehavior::Peripheral => {
                    tier_peripheral.push(entity_id);
                }
            }
        }

        // Also collect CBU IDs
        tier_cbu.extend(self.cbus.keys().copied());

        // Layout each tier
        self.layout_tier_centered_cbu(&tier_cbu, 0, center_x);

        // Tier 2: Ownership/Control chains
        let tier_2_y = 2.0 * Self::TIER_SPACING_Y;
        self.layout_tier_left(&tier_2_shell, 2, tier_2_y);
        self.layout_tier_right(&tier_2_person, 2, tier_2_y);

        // Tier 3: Fund structure/Service providers
        let tier_3_y = 3.0 * Self::TIER_SPACING_Y;
        self.layout_tier_left(&tier_3_shell, 3, tier_3_y);
        self.layout_tier_right(&tier_3_person, 3, tier_3_y);

        // Tier 4: Trading/Trust roles
        let tier_4_y = 4.0 * Self::TIER_SPACING_Y;
        self.layout_tier_left(&tier_4_shell, 4, tier_4_y);
        self.layout_tier_right(&tier_4_person, 4, tier_4_y);

        // Tier 5: Investor chain
        let tier_5_y = 5.0 * Self::TIER_SPACING_Y;
        self.layout_tier_left(&tier_5_shell, 5, tier_5_y);
        self.layout_tier_right(&tier_5_person, 5, tier_5_y);

        // Tier 6: Peripheral
        let tier_6_y = 6.0 * Self::TIER_SPACING_Y;
        self.layout_tier_centered(&tier_peripheral, 6, center_x, tier_6_y);
    }

    /// KYC/UBO layout (HORIZONTAL)
    fn layout_kyc_ubo_horizontal(&mut self) {
        // Similar to vertical but with x/y swapped
        // TODO: Implement horizontal variant
        self.layout_kyc_ubo_vertical();
    }

    /// UBO Only layout: Pure ownership/control graph
    fn layout_ubo_only_vertical(&mut self) {
        let center_x = Self::CANVAS_WIDTH / 2.0;

        let mut shells: Vec<Uuid> = Vec::new();
        let mut persons: Vec<Uuid> = Vec::new();

        for (id, node) in &self.nodes {
            if node.is_natural_person {
                persons.push(*id);
            } else {
                shells.push(*id);
            }
        }

        // CBUs at top
        let cbu_ids: Vec<Uuid> = self.cbus.keys().copied().collect();
        self.layout_tier_centered_cbu(&cbu_ids, 0, center_x);

        // Shells below left
        let tier_1_y = Self::TIER_SPACING_Y;
        self.layout_tier_left(&shells, 1, tier_1_y);

        // Persons (UBOs) below right
        let tier_2_y = 2.0 * Self::TIER_SPACING_Y;
        self.layout_tier_right(&persons, 2, tier_2_y);
    }

    /// UBO Only layout (HORIZONTAL)
    fn layout_ubo_only_horizontal(&mut self) {
        // TODO: Implement horizontal variant
        self.layout_ubo_only_vertical();
    }

    /// Book view: Shows all CBUs under an ownership apex
    fn layout_book_view(&mut self) {
        let center_x = Self::CANVAS_WIDTH / 2.0;

        // Layout termini at top
        let termini = self.termini.clone();
        let tier_0_y = Self::TIER_SPACING_Y;
        self.layout_tier_centered(&termini, 0, center_x, tier_0_y);

        // Group remaining nodes by depth
        let mut by_depth: std::collections::BTreeMap<u32, Vec<Uuid>> =
            std::collections::BTreeMap::new();
        for (id, node) in &self.nodes {
            if !self.termini.contains(id) {
                let depth = node.depth_from_terminus.unwrap_or(10);
                by_depth.entry(depth).or_default().push(*id);
            }
        }

        // Layout each depth tier
        for (depth, ids) in by_depth {
            let tier = (depth + 1) as i32;
            let y = (tier + 1) as f32 * Self::TIER_SPACING_Y;
            self.layout_tier_centered(&ids, tier, center_x, y);
        }

        // Layout CBUs below entities
        let max_depth = self
            .nodes
            .values()
            .filter_map(|n| n.depth_from_terminus)
            .max()
            .unwrap_or(0);
        let cbu_ids: Vec<Uuid> = self.cbus.keys().copied().collect();
        let cbu_y = (max_depth + 3) as f32 * Self::TIER_SPACING_Y;
        self.layout_tier_centered_cbu(&cbu_ids, (max_depth + 2) as i32, center_x);

        // Position CBU y separately since layout_tier_centered_cbu doesn't take y
        for cbu_id in &cbu_ids {
            if let Some(cbu) = self.cbus.get_mut(cbu_id) {
                cbu.y = Some(cbu_y);
            }
        }
    }

    /// Layout CBUs centered around x position
    fn layout_tier_centered_cbu(&mut self, cbu_ids: &[Uuid], tier: i32, center_x: f32) {
        if cbu_ids.is_empty() {
            return;
        }

        let total_width = cbu_ids.len() as f32 * Self::NODE_SPACING_X;
        let start_x = center_x - total_width / 2.0 + Self::NODE_SPACING_X / 2.0;
        let y = tier as f32 * Self::TIER_SPACING_Y;

        for (i, cbu_id) in cbu_ids.iter().enumerate() {
            if let Some(cbu) = self.cbus.get_mut(cbu_id) {
                cbu.x = Some(start_x + i as f32 * Self::NODE_SPACING_X);
                cbu.y = Some(y);
                cbu.width = Some(Self::NODE_WIDTH);
                cbu.height = Some(Self::NODE_HEIGHT);
            }
        }
    }

    /// Layout entity nodes centered around x position at given y
    fn layout_tier_centered(&mut self, entity_ids: &[Uuid], tier: i32, center_x: f32, y: f32) {
        if entity_ids.is_empty() {
            return;
        }

        let total_width = entity_ids.len() as f32 * Self::NODE_SPACING_X;
        let start_x = center_x - total_width / 2.0 + Self::NODE_SPACING_X / 2.0;

        for (i, entity_id) in entity_ids.iter().enumerate() {
            if let Some(node) = self.nodes.get_mut(entity_id) {
                node.x = Some(start_x + i as f32 * Self::NODE_SPACING_X);
                node.y = Some(y);
                node.width = Some(Self::NODE_WIDTH);
                node.height = Some(Self::NODE_HEIGHT);
                node.layout_tier = Some(tier);
            }
        }
    }

    /// Layout SHELL nodes on the left side
    fn layout_tier_left(&mut self, entity_ids: &[Uuid], tier: i32, y: f32) {
        if entity_ids.is_empty() {
            return;
        }

        let start_x = Self::SHELL_MARGIN_LEFT;

        for (i, entity_id) in entity_ids.iter().enumerate() {
            if let Some(node) = self.nodes.get_mut(entity_id) {
                node.x = Some(start_x + i as f32 * Self::NODE_SPACING_X);
                node.y = Some(y);
                node.width = Some(Self::NODE_WIDTH);
                node.height = Some(Self::NODE_HEIGHT);
                node.layout_tier = Some(tier);
            }
        }
    }

    /// Layout PERSON nodes on the right side
    fn layout_tier_right(&mut self, entity_ids: &[Uuid], tier: i32, y: f32) {
        if entity_ids.is_empty() {
            return;
        }

        let total_width = entity_ids.len() as f32 * Self::NODE_SPACING_X;
        let ideal_start = Self::CANVAS_WIDTH - Self::PERSON_MARGIN_RIGHT - total_width;
        let start_x = ideal_start.max(Self::CANVAS_WIDTH / 2.0);

        for (i, entity_id) in entity_ids.iter().enumerate() {
            if let Some(node) = self.nodes.get_mut(entity_id) {
                node.x = Some(start_x + i as f32 * Self::NODE_SPACING_X);
                node.y = Some(y);
                node.width = Some(Self::NODE_WIDTH);
                node.height = Some(Self::NODE_HEIGHT);
                node.layout_tier = Some(tier);
            }
        }
    }
}

// =============================================================================
// NODE TYPES
// =============================================================================

/// A node representing an entity in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Entity ID (primary key)
    pub entity_id: Uuid,

    /// Display name
    pub name: String,

    /// Entity type (PROPER_PERSON, LIMITED_COMPANY, FUND, etc.)
    pub entity_type: EntityType,

    /// Jurisdiction code
    pub jurisdiction: Option<String>,

    // =========================================================================
    // ADJACENCY LISTS (populated from edges)
    // =========================================================================
    /// Entities that own this entity
    #[serde(default)]
    pub owners: Vec<Uuid>,

    /// Entities owned by this entity
    #[serde(default)]
    pub owned: Vec<Uuid>,

    /// Entities that control this entity
    #[serde(default)]
    pub controlled_by: Vec<Uuid>,

    /// Entities controlled by this entity
    #[serde(default)]
    pub controls: Vec<Uuid>,

    /// CBUs this entity is a member of
    #[serde(default)]
    pub cbu_memberships: Vec<Uuid>,

    // =========================================================================
    // FUND STRUCTURE RELATIONSHIPS (for Same ManCo / Same SICAV filters)
    // =========================================================================
    /// ManCo (management company) that manages this entity (if fund/subfund)
    #[serde(default)]
    pub manco_id: Option<Uuid>,

    /// SICAV/umbrella this entity belongs to (if subfund)
    #[serde(default)]
    pub sicav_id: Option<Uuid>,

    // =========================================================================
    // CLASSIFICATION
    // =========================================================================
    /// Primary role category for layout (from taxonomy)
    pub primary_role_category: Option<RoleCategory>,

    /// Layout behavior hint (derived from role_category)
    pub layout_behavior: Option<LayoutBehavior>,

    /// UBO treatment code
    pub ubo_treatment: Option<UboTreatment>,

    /// Is this a natural person? (always a terminus in ownership chains)
    pub is_natural_person: bool,

    /// Depth from nearest terminus (0 = terminus itself)
    pub depth_from_terminus: Option<u32>,

    // =========================================================================
    // RENDERING (set by layout engine)
    // =========================================================================
    /// X position
    pub x: Option<f32>,

    /// Y position
    pub y: Option<f32>,

    /// Node width
    pub width: Option<f32>,

    /// Node height
    pub height: Option<f32>,

    /// Is this node currently visible (passes filters)?
    #[serde(default = "default_true")]
    pub visible: bool,

    /// Layout tier (0 = top, higher = lower)
    pub layout_tier: Option<i32>,

    // =========================================================================
    // VISUAL HINTS
    // =========================================================================
    /// Node importance score (0.0 - 1.0)
    pub importance: Option<f32>,

    /// KYC completion percentage (0-100)
    pub kyc_completion: Option<i32>,

    /// Verification status
    pub verification_status: Option<VerificationStatus>,

    /// All roles for this entity across CBUs
    #[serde(default)]
    pub roles: Vec<String>,

    /// Role categories for this entity
    #[serde(default)]
    pub role_categories: Vec<RoleCategory>,

    /// Additional data for rendering
    #[serde(default)]
    pub data: serde_json::Value,
}

fn default_true() -> bool {
    true
}

impl Default for GraphNode {
    fn default() -> Self {
        Self {
            entity_id: Uuid::nil(),
            name: String::new(),
            entity_type: EntityType::Unknown,
            jurisdiction: None,
            owners: Vec::new(),
            owned: Vec::new(),
            controlled_by: Vec::new(),
            controls: Vec::new(),
            cbu_memberships: Vec::new(),
            manco_id: None,
            sicav_id: None,
            primary_role_category: None,
            layout_behavior: None,
            ubo_treatment: None,
            is_natural_person: false,
            depth_from_terminus: None,
            x: None,
            y: None,
            width: None,
            height: None,
            visible: true,
            layout_tier: None,
            importance: None,
            kyc_completion: None,
            verification_status: None,
            roles: Vec::new(),
            role_categories: Vec::new(),
            data: serde_json::Value::Null,
        }
    }
}

impl GraphNode {
    /// Create a new node with minimal required fields
    pub fn new(entity_id: Uuid, name: String, entity_type: EntityType) -> Self {
        Self {
            entity_id,
            name,
            entity_type,
            ..Default::default()
        }
    }

    /// Check if this node is a terminus (no parent owners)
    pub fn is_terminus(&self) -> bool {
        self.owners.is_empty()
    }

    /// Check if this node has children (owns something)
    pub fn has_children(&self) -> bool {
        !self.owned.is_empty()
    }
}

/// A CBU container node (for CBU container view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuNode {
    /// CBU ID
    pub cbu_id: Uuid,

    /// CBU name
    pub name: String,

    /// Jurisdiction code
    pub jurisdiction: Option<String>,

    /// CBU status
    pub status: CbuStatus,

    /// Commercial client entity that owns this CBU's assets
    pub commercial_client_id: Option<Uuid>,

    /// Entity IDs that are members of this CBU
    #[serde(default)]
    pub member_entities: Vec<Uuid>,

    /// Product IDs associated with this CBU
    #[serde(default)]
    pub products: Vec<Uuid>,

    // Layout fields
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,

    /// Is the CBU container expanded in the UI?
    #[serde(default = "default_true")]
    pub expanded: bool,
}

impl CbuNode {
    pub fn new(cbu_id: Uuid, name: String) -> Self {
        Self {
            cbu_id,
            name,
            jurisdiction: None,
            status: CbuStatus::Active,
            commercial_client_id: None,
            member_entities: Vec::new(),
            products: Vec::new(),
            x: None,
            y: None,
            width: None,
            height: None,
            expanded: true,
        }
    }
}

// =============================================================================
// EDGE TYPES
// =============================================================================

/// Ownership edge (percentage-based ownership relationship)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipEdge {
    /// Edge ID
    pub id: Uuid,

    /// Owner entity
    pub from_entity_id: Uuid,

    /// Owned entity
    pub to_entity_id: Uuid,

    /// Ownership percentage (0-100)
    pub percentage: Decimal,

    /// Type of ownership
    pub ownership_type: OwnershipType,

    /// Effective from date
    pub effective_from: Option<NaiveDate>,

    /// Effective to date (None = current)
    pub effective_to: Option<NaiveDate>,

    /// Verification status
    pub verification_status: Option<VerificationStatus>,

    /// Is this edge currently visible (passes filters)?
    #[serde(default = "default_true")]
    pub visible: bool,
}

impl OwnershipEdge {
    pub fn new(
        from_entity_id: Uuid,
        to_entity_id: Uuid,
        percentage: Decimal,
        ownership_type: OwnershipType,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_entity_id,
            to_entity_id,
            percentage,
            ownership_type,
            effective_from: None,
            effective_to: None,
            verification_status: None,
            visible: true,
        }
    }

    /// Check if this edge is effective as of a given date
    pub fn is_effective_as_of(&self, date: NaiveDate) -> bool {
        let from_ok = self.effective_from.is_none_or(|d| d <= date);
        let to_ok = self.effective_to.is_none_or(|d| d >= date);
        from_ok && to_ok
    }
}

/// Control edge (non-ownership control relationship)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlEdge {
    /// Edge ID
    pub id: Uuid,

    /// Controller entity
    pub controller_id: Uuid,

    /// Controlled entity
    pub controlled_id: Uuid,

    /// Type of control
    pub control_type: ControlType,

    /// Effective from date
    pub effective_from: Option<NaiveDate>,

    /// Effective to date
    pub effective_to: Option<NaiveDate>,

    /// Is this edge currently visible?
    #[serde(default = "default_true")]
    pub visible: bool,
}

impl ControlEdge {
    pub fn new(controller_id: Uuid, controlled_id: Uuid, control_type: ControlType) -> Self {
        Self {
            id: Uuid::new_v4(),
            controller_id,
            controlled_id,
            control_type,
            effective_from: None,
            effective_to: None,
            visible: true,
        }
    }

    /// Check if this edge is effective as of a given date
    pub fn is_effective_as_of(&self, date: NaiveDate) -> bool {
        let from_ok = self.effective_from.is_none_or(|d| d <= date);
        let to_ok = self.effective_to.is_none_or(|d| d >= date);
        from_ok && to_ok
    }
}

/// Fund structure edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundEdge {
    /// Edge ID
    pub id: Uuid,

    /// Parent entity (umbrella, master fund, etc.)
    pub parent_id: Uuid,

    /// Child entity (subfund, feeder, share class, etc.)
    pub child_id: Uuid,

    /// Relationship type
    pub relationship_type: FundRelationshipType,

    /// Is this edge currently visible?
    #[serde(default = "default_true")]
    pub visible: bool,
}

impl FundEdge {
    pub fn new(parent_id: Uuid, child_id: Uuid, relationship_type: FundRelationshipType) -> Self {
        Self {
            id: Uuid::new_v4(),
            parent_id,
            child_id,
            relationship_type,
            visible: true,
        }
    }
}

/// Service delivery edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEdge {
    /// Edge ID
    pub id: Uuid,

    /// Source entity (provider or CBU)
    pub source_id: Uuid,

    /// Target entity (service, resource, etc.)
    pub target_id: Uuid,

    /// Relationship type
    pub relationship_type: ServiceRelationshipType,

    /// Is this edge currently visible?
    #[serde(default = "default_true")]
    pub visible: bool,
}

/// Role assignment (entity role within a CBU)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignment {
    /// Assignment ID
    pub id: Uuid,

    /// CBU where role is assigned
    pub cbu_id: Uuid,

    /// Entity with the role
    pub entity_id: Uuid,

    /// Role name
    pub role: String,

    /// Role category from taxonomy
    pub role_category: Option<RoleCategory>,

    /// Ownership percentage (for ownership roles)
    pub ownership_percentage: Option<Decimal>,

    /// Effective from date
    pub effective_from: Option<NaiveDate>,

    /// Effective to date
    pub effective_to: Option<NaiveDate>,

    /// Is this assignment currently visible?
    #[serde(default = "default_true")]
    pub visible: bool,
}

impl RoleAssignment {
    pub fn new(cbu_id: Uuid, entity_id: Uuid, role: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            cbu_id,
            entity_id,
            role,
            role_category: None,
            ownership_percentage: None,
            effective_from: None,
            effective_to: None,
            visible: true,
        }
    }
}

// =============================================================================
// ENUMS
// =============================================================================

/// Entity type classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityType {
    #[default]
    Unknown,
    // Natural persons
    ProperPerson,
    // Legal entities
    LimitedCompany,
    Partnership,
    Trust,
    Llc,
    // Fund types
    Fund,
    Sicav,
    Icav,
    Oeic,
    Vcc,
    UnitTrust,
    Fcp,
    // Management
    Manco,
    Aifm,
    // Other
    ShareClass,
    Product,
    Service,
    Resource,
}

impl FromStr for EntityType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().as_str() {
            "PROPER_PERSON" | "NATURAL_PERSON" | "PERSON" => Self::ProperPerson,
            "LIMITED_COMPANY" | "COMPANY" => Self::LimitedCompany,
            "PARTNERSHIP" | "LIMITED_PARTNERSHIP" => Self::Partnership,
            "TRUST" | "DISCRETIONARY_TRUST" => Self::Trust,
            "LLC" => Self::Llc,
            "FUND" => Self::Fund,
            "SICAV" => Self::Sicav,
            "ICAV" => Self::Icav,
            "OEIC" => Self::Oeic,
            "VCC" => Self::Vcc,
            "UNIT_TRUST" => Self::UnitTrust,
            "FCP" => Self::Fcp,
            "MANCO" | "MANAGEMENT_COMPANY" => Self::Manco,
            "AIFM" => Self::Aifm,
            "SHARE_CLASS" => Self::ShareClass,
            "PRODUCT" => Self::Product,
            "SERVICE" => Self::Service,
            "RESOURCE" => Self::Resource,
            _ => Self::Unknown,
        })
    }
}

impl EntityType {
    /// Check if this is a natural person type
    pub fn is_natural_person(&self) -> bool {
        matches!(self, Self::ProperPerson)
    }

    /// Check if this is a fund type
    pub fn is_fund(&self) -> bool {
        matches!(
            self,
            Self::Fund
                | Self::Sicav
                | Self::Icav
                | Self::Oeic
                | Self::Vcc
                | Self::UnitTrust
                | Self::Fcp
        )
    }
}

// =============================================================================
// ENTITY VERIFICATION STATE
// =============================================================================

/// Person/Entity verification state - progressive refinement from Ghost to Verified
///
/// Ghost entities allow the system to capture "we know someone exists" before
/// we have full identifying attributes. This prevents blocking imports while
/// maintaining explicit tracking of incomplete data.
///
/// Sources that create Ghost entities:
/// - Document extraction ("John Smith mentioned as director")
/// - Ownership chain discovery ("UBO identified but not yet contacted")
/// - Client allegations ("Client says X is a shareholder")
/// - GLEIF parent chains (natural person at terminus)
///
/// ```text
/// Ghost → Identified → Verified
///   │          │           │
///   │          │           └── KYC complete, documents verified
///   │          └── Has identifying attributes (DOB, nationality, etc.)
///   └── Name only, discovered from document/relationship
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PersonState {
    /// Ghost - name only, minimal attributes
    /// Discovered from document mention, ownership chain, or allegation
    /// Cannot complete KYC until identified
    #[default]
    Ghost,
    /// Identified - has identifying attributes (DOB, nationality, residence, ID docs)
    /// Can proceed with KYC screening
    Identified,
    /// Verified - identity confirmed by official documents (passport, license, tax returns)
    Verified,
}

impl FromStr for PersonState {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().as_str() {
            "GHOST" => Self::Ghost,
            "IDENTIFIED" => Self::Identified,
            "VERIFIED" => Self::Verified,
            _ => Self::Ghost, // Default to Ghost for unknown values (safest assumption)
        })
    }
}

impl PersonState {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ghost => "GHOST",
            Self::Identified => "IDENTIFIED",
            Self::Verified => "VERIFIED",
        }
    }

    pub fn is_ghost(&self) -> bool {
        matches!(self, Self::Ghost)
    }

    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
    }

    /// Can this entity proceed to KYC screening?
    /// Ghost entities need identification first.
    pub fn can_screen(&self) -> bool {
        !self.is_ghost()
    }

    /// Can this entity complete KYC?
    /// Only verified entities can complete.
    pub fn can_complete_kyc(&self) -> bool {
        self.is_verified()
    }

    /// Display label for UI
    pub fn display_label(&self) -> &str {
        match self {
            Self::Ghost => "👻 Ghost",
            Self::Identified => "Identified",
            Self::Verified => "✓ Verified",
        }
    }
}

/// Ownership type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OwnershipType {
    #[default]
    Direct,
    Indirect,
    Beneficial,
    Nominee,
}

impl FromStr for OwnershipType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().as_str() {
            "DIRECT" => Self::Direct,
            "INDIRECT" => Self::Indirect,
            "BENEFICIAL" => Self::Beneficial,
            "NOMINEE" => Self::Nominee,
            _ => Self::Direct,
        })
    }
}

/// Control type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControlType {
    #[default]
    VotingRights,
    BoardMember,
    BoardAppointment,
    Veto,
    Executive,
    TrustSettlor,
    TrustTrustee,
    TrustBeneficiary,
    TrustProtector,
}

impl FromStr for ControlType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().as_str() {
            "VOTING_RIGHTS" | "VOTING" => Self::VotingRights,
            "BOARD_MEMBER" | "DIRECTOR" => Self::BoardMember,
            "BOARD_APPOINTMENT" => Self::BoardAppointment,
            "VETO" => Self::Veto,
            "EXECUTIVE" | "CEO" | "CFO" => Self::Executive,
            "TRUST_SETTLOR" | "SETTLOR" => Self::TrustSettlor,
            "TRUST_TRUSTEE" | "TRUSTEE" => Self::TrustTrustee,
            "TRUST_BENEFICIARY" | "BENEFICIARY" => Self::TrustBeneficiary,
            "TRUST_PROTECTOR" | "PROTECTOR" => Self::TrustProtector,
            _ => Self::VotingRights,
        })
    }
}

/// Fund structure relationship type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FundRelationshipType {
    /// Umbrella contains subfund
    #[default]
    Contains,
    /// Feeder invests in master
    FeederTo,
    /// Share class belongs to fund
    ShareClassOf,
    /// Fund of funds investment
    InvestsIn,
    /// Management relationship
    ManagedBy,
    /// Administration relationship
    AdministeredBy,
    /// Custody relationship
    CustodiedBy,
}

impl FromStr for FundRelationshipType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().as_str() {
            "CONTAINS" => Self::Contains,
            "FEEDER_TO" | "MASTER_FEEDER" => Self::FeederTo,
            "SHARE_CLASS_OF" | "SHARE_CLASS" => Self::ShareClassOf,
            "INVESTS_IN" | "FOF" => Self::InvestsIn,
            "MANAGED_BY" => Self::ManagedBy,
            "ADMINISTERED_BY" => Self::AdministeredBy,
            "CUSTODIED_BY" => Self::CustodiedBy,
            _ => Self::Contains,
        })
    }
}

/// Service relationship type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceRelationshipType {
    /// CBU uses product
    UsesProduct,
    /// Product provides service
    ProvidesService,
    /// Service uses resource
    UsesResource,
    /// Resource provisioned for CBU
    ProvisionedFor,
}

/// CBU status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CbuStatus {
    #[default]
    Active,
    Pending,
    Suspended,
    Closed,
}

/// Verification status for edges
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    #[default]
    Unverified,
    Alleged,
    Pending,
    Proven,
    Disputed,
    Waived,
}

impl FromStr for VerificationStatus {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "unverified" => Self::Unverified,
            "alleged" => Self::Alleged,
            "pending" => Self::Pending,
            "proven" | "verified" => Self::Proven,
            "disputed" => Self::Disputed,
            "waived" => Self::Waived,
            _ => Self::Unverified,
        })
    }
}

/// UBO treatment for entities
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UboTreatment {
    /// Natural person - terminus of ownership chain
    Terminus,
    /// Look through this entity to find owners
    LookThrough,
    /// Control prong (not ownership)
    ControlProng,
    /// Not applicable (not in UBO chain)
    NotApplicable,
    /// Public company exemption
    PublicCompanyExemption,
    /// Regulated entity exemption
    RegulatedEntityExemption,
}

impl FromStr for UboTreatment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TERMINUS" => Ok(Self::Terminus),
            "LOOK_THROUGH" => Ok(Self::LookThrough),
            "CONTROL_PRONG" => Ok(Self::ControlProng),
            "NOT_APPLICABLE" => Ok(Self::NotApplicable),
            "PUBLIC_COMPANY_EXEMPTION" => Ok(Self::PublicCompanyExemption),
            "REGULATED_ENTITY_EXEMPTION" => Ok(Self::RegulatedEntityExemption),
            _ => Err(format!("Unknown UBO treatment: {}", s)),
        }
    }
}

// =============================================================================
// ROLE TAXONOMY (from existing types.rs - preserved for compatibility)
// =============================================================================

/// Role category from taxonomy - determines layout behavior
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoleCategory {
    /// Ownership chain - pyramid layout with UBOs at apex
    OwnershipChain,
    /// Control chain - overlay on owned entities
    ControlChain,
    /// Fund structure - tree layout
    FundStructure,
    /// Fund management - satellite around fund
    FundManagement,
    /// Trust roles - radial around trust
    TrustRoles,
    /// Service providers - flat at bottom
    ServiceProvider,
    /// Trading execution - flat at right
    TradingExecution,
    /// Investor chain - pyramid down
    InvestorChain,
    /// Related parties - peripheral
    RelatedParty,
    /// Investment vehicle - pooled funds AO invests in (Umbrella, ETF, Unit Trust)
    /// Layout: TreeDown below AO, shows fund structure
    /// Trading View only (not UBO)
    InvestmentVehicle,
    // Legacy categories
    OwnershipControl,
    Both,
    FundOperations,
    Distribution,
    Financing,
}

impl FromStr for RoleCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "OWNERSHIP_CHAIN" => Ok(Self::OwnershipChain),
            "CONTROL_CHAIN" => Ok(Self::ControlChain),
            "FUND_STRUCTURE" => Ok(Self::FundStructure),
            "FUND_MANAGEMENT" => Ok(Self::FundManagement),
            "TRUST_ROLES" => Ok(Self::TrustRoles),
            "SERVICE_PROVIDER" => Ok(Self::ServiceProvider),
            "TRADING_EXECUTION" => Ok(Self::TradingExecution),
            "INVESTOR_CHAIN" => Ok(Self::InvestorChain),
            "RELATED_PARTY" => Ok(Self::RelatedParty),
            "INVESTMENT_VEHICLE" => Ok(Self::InvestmentVehicle),
            "OWNERSHIP_CONTROL" => Ok(Self::OwnershipControl),
            "BOTH" => Ok(Self::Both),
            "FUND_OPERATIONS" => Ok(Self::FundOperations),
            "DISTRIBUTION" => Ok(Self::Distribution),
            "FINANCING" => Ok(Self::Financing),
            _ => Err(format!("Unknown role category: {}", s)),
        }
    }
}

impl RoleCategory {
    pub fn layout_behavior(&self) -> LayoutBehavior {
        match self {
            Self::OwnershipChain | Self::OwnershipControl => LayoutBehavior::PyramidUp,
            Self::ControlChain => LayoutBehavior::Overlay,
            Self::FundStructure | Self::InvestmentVehicle => LayoutBehavior::TreeDown,
            Self::FundManagement => LayoutBehavior::Satellite,
            Self::TrustRoles => LayoutBehavior::Radial,
            Self::ServiceProvider => LayoutBehavior::FlatBottom,
            Self::TradingExecution | Self::FundOperations | Self::Distribution => {
                LayoutBehavior::FlatRight
            }
            Self::InvestorChain | Self::Financing => LayoutBehavior::PyramidDown,
            Self::RelatedParty => LayoutBehavior::Peripheral,
            Self::Both => LayoutBehavior::Overlay,
        }
    }

    pub fn is_ownership_or_control(&self) -> bool {
        matches!(
            self,
            Self::OwnershipChain
                | Self::ControlChain
                | Self::OwnershipControl
                | Self::TrustRoles
                | Self::Both
        )
    }

    // NOTE: is_ubo_relevant() and is_trading_relevant() removed - replaced by database-driven
    // visibility config in ob-poc.node_types and ob-poc.view_modes tables.
    // See ViewConfigService for the config-driven approach.
}

/// Layout behavior hint for node positioning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutBehavior {
    PyramidUp,
    PyramidDown,
    TreeDown,
    Overlay,
    Satellite,
    Radial,
    FlatBottom,
    FlatRight,
    Peripheral,
}

impl FromStr for LayoutBehavior {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pyramid_up" => Ok(Self::PyramidUp),
            "pyramid_down" => Ok(Self::PyramidDown),
            "tree_down" => Ok(Self::TreeDown),
            "overlay" => Ok(Self::Overlay),
            "satellite" => Ok(Self::Satellite),
            "radial" => Ok(Self::Radial),
            "flat_bottom" => Ok(Self::FlatBottom),
            "flat_right" => Ok(Self::FlatRight),
            "peripheral" => Ok(Self::Peripheral),
            _ => Err(format!("Unknown layout behavior: {}", s)),
        }
    }
}

// =============================================================================
// FILTERS AND SCOPE
// =============================================================================

/// Graph filters for visibility control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphFilters {
    /// Ownership/control prong filter
    pub prong: ProngFilter,

    /// Filter to specific jurisdictions
    pub jurisdictions: Option<Vec<String>>,

    /// Filter to specific fund types
    pub fund_types: Option<Vec<String>>,

    /// Filter to specific entity types
    pub entity_types: Option<Vec<EntityType>>,

    /// Temporal filter - show graph as of this date
    pub as_of_date: NaiveDate,

    /// Minimum ownership percentage to show
    pub min_ownership_pct: Option<Decimal>,

    /// Show only the path to cursor (hide unrelated nodes)
    pub path_only: bool,

    /// Filter to entities managed by this ManCo (management company)
    /// Shows only funds/subfunds that share the same management company
    #[serde(default)]
    pub same_manco_id: Option<Uuid>,

    /// Filter to entities under this SICAV umbrella
    /// Shows only subfunds belonging to the same SICAV/umbrella structure
    #[serde(default)]
    pub same_sicav_id: Option<Uuid>,
}

impl Default for GraphFilters {
    fn default() -> Self {
        Self {
            prong: ProngFilter::Both,
            jurisdictions: None,
            fund_types: None,
            entity_types: None,
            as_of_date: chrono::Local::now().date_naive(),
            min_ownership_pct: None,
            path_only: false,
            same_manco_id: None,
            same_sicav_id: None,
        }
    }
}

/// Ownership/control prong filter
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProngFilter {
    /// Show both ownership and control
    #[default]
    Both,
    /// Show only ownership relationships
    OwnershipOnly,
    /// Show only control relationships
    ControlOnly,
}

/// Scope of the loaded graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphScope {
    /// Empty graph (initial state)
    #[default]
    Empty,

    /// Single CBU container
    SingleCbu { cbu_id: Uuid, cbu_name: String },

    /// Book scope - all CBUs under an ownership apex
    Book {
        apex_entity_id: Uuid,
        apex_name: String,
    },

    /// Jurisdiction scope
    Jurisdiction { code: String },

    /// Entity neighborhood (N hops from focal entity)
    EntityNeighborhood { entity_id: Uuid, hops: u32 },

    /// Custom scope with description
    Custom { description: String },
}

// =============================================================================
// NAVIGATION HISTORY
// =============================================================================

/// Navigation history for back/forward in the graph
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NavigationHistory {
    /// Stack of previously visited entities
    back_stack: Vec<Uuid>,

    /// Stack of entities to visit on "forward"
    forward_stack: Vec<Uuid>,

    /// Maximum history size
    max_size: usize,
}

impl NavigationHistory {
    pub fn new() -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            max_size: 50,
        }
    }

    /// Push current location before navigating away
    pub fn push(&mut self, entity_id: Uuid) {
        self.back_stack.push(entity_id);
        self.forward_stack.clear(); // Forward stack clears on new navigation

        // Limit size
        while self.back_stack.len() > self.max_size {
            self.back_stack.remove(0);
        }
    }

    /// Go back to previous location
    pub fn go_back(&mut self, current: Option<Uuid>) -> Option<Uuid> {
        if let Some(prev) = self.back_stack.pop() {
            if let Some(curr) = current {
                self.forward_stack.push(curr);
            }
            Some(prev)
        } else {
            None
        }
    }

    /// Go forward
    pub fn go_forward(&mut self, current: Option<Uuid>) -> Option<Uuid> {
        if let Some(next) = self.forward_stack.pop() {
            if let Some(curr) = current {
                self.back_stack.push(curr);
            }
            Some(next)
        } else {
            None
        }
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.back_stack.clear();
        self.forward_stack.clear();
    }

    /// Check if we can go back
    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    /// Check if we can go forward
    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }
}

// =============================================================================
// STATISTICS
// =============================================================================

/// Graph statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_layer: HashMap<String, usize>,
    pub nodes_by_type: HashMap<String, usize>,
    pub cbu_count: usize,
    pub terminus_count: usize,
}

// =============================================================================
// VIEW MODES
// =============================================================================

/// View mode for graph rendering
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ViewMode {
    /// CBU as container with entities inside
    #[default]
    CbuContainer,
    /// UBO ownership pyramid with control overlay
    UboForest,
    /// Fund structure tree (umbrella → subfund)
    FundStructure,
    /// Service delivery view
    ServiceDelivery,
    /// Combined view
    Combined,
    // Legacy view modes for backward compatibility
    KycUbo,
    UboOnly,
    ProductsOnly,
    Trading,
}

impl FromStr for ViewMode {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().as_str() {
            "CBU_CONTAINER" => Self::CbuContainer,
            "UBO_FOREST" => Self::UboForest,
            "FUND_STRUCTURE" => Self::FundStructure,
            "SERVICE_DELIVERY" => Self::ServiceDelivery,
            "COMBINED" => Self::Combined,
            "KYC_UBO" => Self::KycUbo,
            "UBO_ONLY" => Self::UboOnly,
            "PRODUCTS_ONLY" => Self::ProductsOnly,
            "TRADING" => Self::Trading,
            _ => Self::CbuContainer,
        })
    }
}

/// Canvas orientation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Orientation {
    #[default]
    Vertical,
    Horizontal,
}

// =============================================================================
// LAYOUT OVERRIDES (preserved from original)
// =============================================================================

/// Per-node position offset from template layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOffset {
    pub node_id: String,
    pub dx: f32,
    pub dy: f32,
}

/// Per-node size override
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSizeOverride {
    pub node_id: String,
    pub w: f32,
    pub h: f32,
}

/// Saved layout overrides for a CBU/view
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayoutOverride {
    #[serde(default)]
    pub positions: Vec<NodeOffset>,
    #[serde(default)]
    pub sizes: Vec<NodeSizeOverride>,
}

// =============================================================================
// BACKWARD COMPATIBILITY - CbuGraph alias
// =============================================================================

/// Legacy CBU graph type - now an alias for EntityGraph
///
/// This type is maintained for backward compatibility with existing code.
/// New code should use `EntityGraph` directly.
pub type CbuGraph = LegacyCbuGraph;

/// Legacy CBU graph structure for backward compatibility
///
/// This wraps EntityGraph to provide the old API shape for existing tests and code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyCbuGraph {
    pub cbu_id: Uuid,
    pub label: String,
    pub cbu_category: Option<String>,
    pub jurisdiction: Option<String>,
    pub nodes: Vec<LegacyGraphNode>,
    pub edges: Vec<LegacyGraphEdge>,
    pub layers: Vec<LayerInfo>,
    pub stats: LegacyGraphStats,
}

/// Legacy graph node for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LegacyGraphNode {
    pub id: String,
    pub node_type: NodeType,
    pub layer: LayerType,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: NodeStatus,
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_categories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_role_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_behavior: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ubo_treatment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyc_obligation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_tier: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyc_completion: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_container: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browse_nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_key: Option<String>,
    /// Person state for proper_person entities: GHOST, IDENTIFIED, or VERIFIED
    /// Ghost entities have minimal info (name only) and render with dashed/faded style
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person_state: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Legacy graph edge for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyGraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

/// Legacy node types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    #[default]
    Cbu,
    Market,
    Universe,
    Ssi,
    BookingRule,
    Isda,
    Csa,
    Subcustodian,
    Document,
    Attribute,
    Verification,
    Entity,
    OwnershipLink,
    Product,
    Service,
    Resource,
    ShareClass,
    ServiceInstance,
    InvestorHolding,
    ServiceResource,
    // Trading view node types
    TradingProfile,
    InstrumentMatrix,
    InstrumentClass,
    IsdaAgreement,
    CsaAgreement,
    // Capital structure node types (Migration 013)
    IssuanceEvent,
    DilutionInstrument,
    OwnershipSnapshot,
    ReconciliationRun,
    SpecialRight,
}

/// Legacy layer types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayerType {
    #[default]
    Core,
    Custody,
    Kyc,
    Ubo,
    Services,
    Trading,
}

/// Legacy node status
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

/// Legacy edge types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    HasRole,
    Owns,
    Controls,
    TrustSettlor,
    TrustTrustee,
    TrustBeneficiary,
    TrustProtector,
    ManagedBy,
    AdministeredBy,
    CustodiedBy,
    UsesProduct,
    FeederTo,
    InvestsIn,
    Contains,
    ShareClassOf,
    RoutesTo,
    Matches,
    CoveredBy,
    SecuredBy,
    SettlesAt,
    SubcustodianOf,
    Requires,
    Validates,
    VerifiedBy,
    Contradicts,
    Delivers,
    BelongsTo,
    ProvisionedFor,
    DelegatesTo,
    /// UBO chain terminus - ownership tracing stops here (public company, government, etc.)
    UboTerminus,
    // Trading view edge types
    /// CBU -> TradingProfile
    HasTradingProfile,
    /// TradingProfile -> InstrumentMatrix
    HasMatrix,
    /// InstrumentMatrix -> InstrumentClass
    IncludesClass,
    /// InstrumentClass -> Market (exchange traded)
    TradedOn,
    /// InstrumentClass -> Entity (OTC counterparty)
    OtcCounterparty,
    /// Entity -> IsdaAgreement
    CoveredByIsda,
    /// IsdaAgreement -> CsaAgreement
    HasCsa,
    /// CBU -> Entity (IM mandate)
    ImMandate,
    /// Undefined/unmapped relationship type (e.g., from GLEIF import)
    /// Allows imports to proceed even when we don't have a specific mapping
    Undefined,
    // Capital structure edge types (Migration 013)
    /// ShareClass -> Entity (issuer)
    IssuedBy,
    /// IssuanceEvent -> ShareClass
    AffectsSupply,
    /// DilutionInstrument -> ShareClass (target class on conversion)
    ConvertsTo,
    /// DilutionInstrument -> Entity (holder)
    GrantedTo,
    /// OwnershipSnapshot -> Entity (owner)
    SnapshotOwner,
    /// OwnershipSnapshot -> Entity (issuer)
    SnapshotIssuer,
    /// SpecialRight -> ShareClass or Entity
    RightAttachedTo,
    /// ReconciliationRun -> OwnershipSnapshot
    ComparedSnapshot,
}

impl EdgeType {
    /// Convert from database edge_type_code (SCREAMING_SNAKE_CASE)
    /// Used by ConfigDrivenGraphBuilder for view configuration filtering
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "CBU_ROLE" => Some(Self::HasRole),
            "OWNERSHIP" => Some(Self::Owns),
            "INDIRECT_OWNERSHIP" => Some(Self::Owns), // Maps to same variant
            "CONTROL" => Some(Self::Controls),
            "BOARD_MEMBER" => Some(Self::Controls), // Control variant
            "TRUST_SETTLOR" => Some(Self::TrustSettlor),
            "TRUST_TRUSTEE" => Some(Self::TrustTrustee),
            "TRUST_BENEFICIARY" => Some(Self::TrustBeneficiary),
            "TRUST_PROTECTOR" => Some(Self::TrustProtector),
            "FUND_MANAGED_BY" => Some(Self::ManagedBy),
            "CBU_USES_PRODUCT" => Some(Self::UsesProduct),
            "FEEDER_TO_MASTER" => Some(Self::FeederTo),
            "INVESTS_IN_VEHICLE" => Some(Self::InvestsIn),
            "UMBRELLA_CONTAINS_SUBFUND" => Some(Self::Contains),
            "FUND_HAS_SHARE_CLASS" => Some(Self::ShareClassOf),
            "PRODUCT_PROVIDES_SERVICE" => Some(Self::Delivers),
            "SERVICE_USES_RESOURCE" => Some(Self::ProvisionedFor),
            "ENTITY_AUTHORIZES_TRADING" => Some(Self::DelegatesTo),
            // Trading view edge types
            "CBU_HAS_TRADING_PROFILE" => Some(Self::HasTradingProfile),
            "TRADING_PROFILE_HAS_MATRIX" => Some(Self::HasMatrix),
            "MATRIX_INCLUDES_CLASS" => Some(Self::IncludesClass),
            "CLASS_TRADED_ON_MARKET" => Some(Self::TradedOn),
            "OTC_WITH_COUNTERPARTY" => Some(Self::OtcCounterparty),
            "OTC_COVERED_BY_ISDA" => Some(Self::CoveredByIsda),
            "ISDA_HAS_CSA" => Some(Self::HasCsa),
            "CBU_IM_MANDATE" => Some(Self::ImMandate),
            "UNDEFINED" => Some(Self::Undefined),
            // Capital structure edge types (Migration 013)
            "ISSUED_BY" => Some(Self::IssuedBy),
            "AFFECTS_SUPPLY" => Some(Self::AffectsSupply),
            "CONVERTS_TO" => Some(Self::ConvertsTo),
            "GRANTED_TO" => Some(Self::GrantedTo),
            "SNAPSHOT_OWNER" => Some(Self::SnapshotOwner),
            "SNAPSHOT_ISSUER" => Some(Self::SnapshotIssuer),
            "RIGHT_ATTACHED_TO" => Some(Self::RightAttachedTo),
            "COMPARED_SNAPSHOT" => Some(Self::ComparedSnapshot),
            _ => None,
        }
    }

    /// Parse edge type from code, returning Undefined for unknown codes.
    /// Use this for GLEIF imports where we want to proceed even without a mapping.
    pub fn from_code_or_undefined(code: &str) -> Self {
        Self::from_code(code).unwrap_or(Self::Undefined)
    }

    /// Convert to database edge_type_code (SCREAMING_SNAKE_CASE)
    /// Used by ConfigDrivenGraphBuilder for view configuration filtering
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::HasRole => "CBU_ROLE",
            Self::Owns => "OWNERSHIP",
            Self::Controls => "CONTROL",
            Self::TrustSettlor => "TRUST_SETTLOR",
            Self::TrustTrustee => "TRUST_TRUSTEE",
            Self::TrustBeneficiary => "TRUST_BENEFICIARY",
            Self::TrustProtector => "TRUST_PROTECTOR",
            Self::ManagedBy => "FUND_MANAGED_BY",
            Self::AdministeredBy => "FUND_MANAGED_BY", // No separate code, use managed
            Self::CustodiedBy => "FUND_MANAGED_BY",    // No separate code, use managed
            Self::UsesProduct => "CBU_USES_PRODUCT",
            Self::FeederTo => "FEEDER_TO_MASTER",
            Self::InvestsIn => "INVESTS_IN_VEHICLE",
            Self::Contains => "UMBRELLA_CONTAINS_SUBFUND",
            Self::ShareClassOf => "FUND_HAS_SHARE_CLASS",
            Self::RoutesTo => "SERVICE_USES_RESOURCE",
            Self::Matches => "SERVICE_USES_RESOURCE",
            Self::CoveredBy => "SERVICE_USES_RESOURCE",
            Self::SecuredBy => "SERVICE_USES_RESOURCE",
            Self::SettlesAt => "SERVICE_USES_RESOURCE",
            Self::SubcustodianOf => "SERVICE_USES_RESOURCE",
            Self::Requires => "SERVICE_USES_RESOURCE",
            Self::Validates => "SERVICE_USES_RESOURCE",
            Self::VerifiedBy => "SERVICE_USES_RESOURCE",
            Self::Contradicts => "SERVICE_USES_RESOURCE",
            Self::Delivers => "PRODUCT_PROVIDES_SERVICE",
            Self::BelongsTo => "CBU_HAS_TRADING_PROFILE",
            Self::ProvisionedFor => "SERVICE_USES_RESOURCE",
            Self::DelegatesTo => "ENTITY_AUTHORIZES_TRADING",
            Self::UboTerminus => "OWNERSHIP", // Terminus is an ownership termination
            // Trading view edge types
            Self::HasTradingProfile => "CBU_HAS_TRADING_PROFILE",
            Self::HasMatrix => "TRADING_PROFILE_HAS_MATRIX",
            Self::IncludesClass => "MATRIX_INCLUDES_CLASS",
            Self::TradedOn => "CLASS_TRADED_ON_MARKET",
            Self::OtcCounterparty => "OTC_WITH_COUNTERPARTY",
            Self::CoveredByIsda => "OTC_COVERED_BY_ISDA",
            Self::HasCsa => "ISDA_HAS_CSA",
            Self::ImMandate => "CBU_IM_MANDATE",
            Self::Undefined => "UNDEFINED",
            // Capital structure edge types (Migration 013)
            Self::IssuedBy => "ISSUED_BY",
            Self::AffectsSupply => "AFFECTS_SUPPLY",
            Self::ConvertsTo => "CONVERTS_TO",
            Self::GrantedTo => "GRANTED_TO",
            Self::SnapshotOwner => "SNAPSHOT_OWNER",
            Self::SnapshotIssuer => "SNAPSHOT_ISSUER",
            Self::RightAttachedTo => "RIGHT_ATTACHED_TO",
            Self::ComparedSnapshot => "COMPARED_SNAPSHOT",
        }
    }

    pub fn from_relationship_type(rel_type: &str) -> Option<Self> {
        match rel_type.to_lowercase().as_str() {
            "ownership" => Some(EdgeType::Owns),
            "control" => Some(EdgeType::Controls),
            "trust_settlor" | "settlor" => Some(EdgeType::TrustSettlor),
            "trust_trustee" | "trustee" => Some(EdgeType::TrustTrustee),
            "trust_beneficiary" | "beneficiary" => Some(EdgeType::TrustBeneficiary),
            "trust_protector" | "protector" => Some(EdgeType::TrustProtector),
            "ubo_terminus" => Some(EdgeType::UboTerminus),
            _ => None,
        }
    }

    pub fn from_fund_structure_type(rel_type: &str) -> Option<Self> {
        match rel_type.to_uppercase().as_str() {
            // GLEIF relationship types from entity_parent_relationships
            "FUND_MANAGER" => Some(EdgeType::ManagedBy),
            "UMBRELLA_FUND" => Some(EdgeType::Contains),
            "MASTER_FUND" => Some(EdgeType::FeederTo),
            // Legacy/internal relationship types
            "CONTAINS" => Some(EdgeType::Contains),
            "MASTER_FEEDER" | "FEEDER_TO" => Some(EdgeType::FeederTo),
            "INVESTS_IN" => Some(EdgeType::InvestsIn),
            "MANAGED_BY" => Some(EdgeType::ManagedBy),
            _ => None,
        }
    }

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

    pub fn is_trading(&self) -> bool {
        matches!(
            self,
            EdgeType::HasTradingProfile
                | EdgeType::HasMatrix
                | EdgeType::IncludesClass
                | EdgeType::TradedOn
                | EdgeType::OtcCounterparty
                | EdgeType::CoveredByIsda
                | EdgeType::HasCsa
                | EdgeType::ImMandate
        )
    }

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
            EdgeType::UboTerminus => "UBO terminus",
            // Trading view edge labels
            EdgeType::HasTradingProfile => "has trading profile",
            EdgeType::HasMatrix => "has matrix",
            EdgeType::IncludesClass => "includes class",
            EdgeType::TradedOn => "traded on",
            EdgeType::OtcCounterparty => "OTC with",
            EdgeType::CoveredByIsda => "covered by ISDA",
            EdgeType::HasCsa => "has CSA",
            EdgeType::ImMandate => "IM mandate",
            EdgeType::Undefined => "related to",
            // Capital structure edge labels (Migration 013)
            EdgeType::IssuedBy => "issued by",
            EdgeType::AffectsSupply => "affects supply",
            EdgeType::ConvertsTo => "converts to",
            EdgeType::GrantedTo => "granted to",
            EdgeType::SnapshotOwner => "snapshot owner",
            EdgeType::SnapshotIssuer => "snapshot issuer",
            EdgeType::RightAttachedTo => "right attached to",
            EdgeType::ComparedSnapshot => "compared snapshot",
        }
    }

    /// Check if this edge type relates to capital structure
    pub fn is_capital_structure(&self) -> bool {
        matches!(
            self,
            EdgeType::IssuedBy
                | EdgeType::AffectsSupply
                | EdgeType::ConvertsTo
                | EdgeType::GrantedTo
                | EdgeType::SnapshotOwner
                | EdgeType::SnapshotIssuer
                | EdgeType::RightAttachedTo
                | EdgeType::ComparedSnapshot
        )
    }
}

/// Layer info for UI rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerInfo {
    pub layer_type: LayerType,
    pub label: String,
    pub color: String,
    pub node_count: usize,
    pub visible: bool,
}

/// Legacy graph stats
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LegacyGraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_layer: HashMap<String, usize>,
    pub nodes_by_type: HashMap<String, usize>,
}

impl LegacyCbuGraph {
    pub fn new(cbu_id: Uuid, label: String) -> Self {
        Self {
            cbu_id,
            label,
            cbu_category: None,
            jurisdiction: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            layers: Vec::new(),
            stats: LegacyGraphStats::default(),
        }
    }

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
            stats: LegacyGraphStats::default(),
        }
    }

    pub fn add_node(&mut self, node: LegacyGraphNode) {
        self.nodes.push(node);
    }

    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n.id == id)
    }

    pub fn add_edge(&mut self, edge: LegacyGraphEdge) {
        self.edges.push(edge);
    }

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

    pub fn filter_to_products_only(&mut self) {
        let kept_node_ids: std::collections::HashSet<String> = self
            .nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::Cbu | NodeType::Product))
            .map(|n| n.id.clone())
            .collect();

        self.nodes
            .retain(|n| matches!(n.node_type, NodeType::Cbu | NodeType::Product));
        self.edges
            .retain(|e| kept_node_ids.contains(&e.source) && kept_node_ids.contains(&e.target));
    }

    // NOTE: filter_to_ubo_only() and filter_to_trading_entities() removed.
    // Filtering is now done at query time by ConfigDrivenGraphBuilder using
    // view_modes.node_types and view_modes.edge_types from the database.

    pub fn compute_visual_hints(&mut self) {
        let mut edge_counts: HashMap<String, usize> = HashMap::new();
        for edge in &self.edges {
            *edge_counts.entry(edge.source.clone()).or_insert(0) += 1;
            *edge_counts.entry(edge.target.clone()).or_insert(0) += 1;
        }

        let max_edges = edge_counts.values().max().copied().unwrap_or(1) as f32;

        for node in &mut self.nodes {
            let mut importance: f32 = 0.5;

            importance += match node.node_type {
                NodeType::Cbu => 0.5,
                NodeType::Entity => 0.3,
                NodeType::Product => 0.2,
                NodeType::Isda => 0.15,
                NodeType::Ssi => 0.1,
                _ => 0.0,
            };

            if let Some(priority) = node.role_priority {
                let priority_boost = ((100 - priority) as f32 / 100.0) * 0.2;
                importance += priority_boost;
            }

            if node
                .role_categories
                .contains(&"OWNERSHIP_CONTROL".to_string())
            {
                importance += 0.1;
            }

            if let Some(&count) = edge_counts.get(&node.id) {
                let connectivity_boost = (count as f32 / max_edges) * 0.15;
                importance += connectivity_boost;
            }

            match node.status {
                NodeStatus::Active => {}
                NodeStatus::Pending => importance -= 0.1,
                NodeStatus::Draft => importance -= 0.15,
                NodeStatus::Suspended => importance -= 0.2,
                NodeStatus::Expired => importance -= 0.25,
            }

            node.importance = Some(importance.clamp(0.0, 1.0));
        }
    }

    pub fn build_layer_info(&mut self) {
        self.layers = vec![
            LayerInfo {
                layer_type: LayerType::Core,
                label: "Core".to_string(),
                color: "#6B7280".to_string(),
                node_count: self.stats.nodes_by_layer.get("core").copied().unwrap_or(0),
                visible: true,
            },
            LayerInfo {
                layer_type: LayerType::Custody,
                label: "Custody".to_string(),
                color: "#3B82F6".to_string(),
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
                color: "#8B5CF6".to_string(),
                node_count: self.stats.nodes_by_layer.get("kyc").copied().unwrap_or(0),
                visible: false,
            },
            LayerInfo {
                layer_type: LayerType::Ubo,
                label: "UBO".to_string(),
                color: "#10B981".to_string(),
                node_count: self.stats.nodes_by_layer.get("ubo").copied().unwrap_or(0),
                visible: false,
            },
            LayerInfo {
                layer_type: LayerType::Services,
                label: "Services".to_string(),
                color: "#F59E0B".to_string(),
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

/// CBU summary for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    /// Template discriminator: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, etc.
    pub cbu_category: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

// Backward compatibility aliases
pub type GraphNodeLegacy = LegacyGraphNode;
pub type GraphEdge = LegacyGraphEdge;
pub type GraphStatsLegacy = LegacyGraphStats;

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_history_push_and_back() {
        let mut history = NavigationHistory::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        // Push locations to back_stack
        history.push(id1);
        history.push(id2);
        history.push(id3);

        assert!(history.can_go_back());
        assert!(!history.can_go_forward());

        // go_back pops from back_stack (returns id3, the last pushed)
        // and pushes current to forward_stack
        let back = history.go_back(Some(Uuid::new_v4())); // current position
        assert_eq!(back, Some(id3));
        assert!(history.can_go_forward());

        // go_forward pops from forward_stack (returns the current we just pushed)
        // Note: This tests the mechanism works, not specific values
        let forward = history.go_forward(Some(id3));
        assert!(forward.is_some());
    }

    #[test]
    fn test_navigation_history_clear() {
        let mut history = NavigationHistory::new();
        history.push(Uuid::new_v4());
        history.push(Uuid::new_v4());

        history.clear();

        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_layout_behavior_from_str() {
        assert!(matches!(
            "pyramid_up".parse::<LayoutBehavior>(),
            Ok(LayoutBehavior::PyramidUp)
        ));
        assert!(matches!(
            "tree_down".parse::<LayoutBehavior>(),
            Ok(LayoutBehavior::TreeDown)
        ));
        assert!(matches!(
            "flat_bottom".parse::<LayoutBehavior>(),
            Ok(LayoutBehavior::FlatBottom)
        ));
        assert!(matches!(
            "flat_right".parse::<LayoutBehavior>(),
            Ok(LayoutBehavior::FlatRight)
        ));
        assert!("unknown_value".parse::<LayoutBehavior>().is_err());
    }

    #[test]
    fn test_entity_type_from_str() {
        assert!(matches!(
            "PROPER_PERSON".parse::<EntityType>(),
            Ok(EntityType::ProperPerson)
        ));
        assert!(matches!(
            "LIMITED_COMPANY".parse::<EntityType>(),
            Ok(EntityType::LimitedCompany)
        ));
        assert!(matches!("FUND".parse::<EntityType>(), Ok(EntityType::Fund)));
        assert!(matches!(
            "TRUST".parse::<EntityType>(),
            Ok(EntityType::Trust)
        ));
        assert!(matches!(
            "PARTNERSHIP".parse::<EntityType>(),
            Ok(EntityType::Partnership)
        ));
        // Unknown values map to Unknown (infallible FromStr)
        assert!(matches!(
            "unknown".parse::<EntityType>(),
            Ok(EntityType::Unknown)
        ));
    }
}
