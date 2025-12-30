//! Server-side layout engine for CBU graph visualization
//!
//! Computes x, y positions for graph nodes based on view mode and graph semantics.
//! The UI receives pre-positioned nodes and just renders them.
//!
//! ## Role Taxonomy V2
//!
//! Layout decisions are driven by `RoleCategory` and `LayoutBehavior` rather than
//! numeric role_priority values. Each entity's `primary_role_category` determines
//! its `layout_behavior` which controls placement:
//!
//! | LayoutBehavior | Placement |
//! |----------------|-----------|
//! | PyramidUp      | Tier 2-3, ownership chain (SHELL left, PERSON right) |
//! | PyramidDown    | Tier 2-3, control chain (SHELL left, PERSON right) |
//! | TreeDown       | Tier 3, fund structure hierarchy |
//! | Overlay        | Tier 3, service providers overlay on structure |
//! | Satellite      | Tier 4, trading/execution entities |
//! | FlatBottom     | Tier 5, investor chain |
//! | Peripheral     | Edge, related parties |

// Use legacy types for backward compatibility during transition to EntityGraph
use super::types::{CbuGraph, LayoutBehavior, LegacyGraphNode, NodeType, RoleCategory};

// Re-export as GraphNode for this module
type GraphNode = LegacyGraphNode;

/// View modes determine which layers are visible and how they're laid out
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// KYC/UBO view: Entity hierarchy by role priority, ownership chains, KYC status
    #[default]
    KycUbo,
    /// UBO Only view: Pure ownership/control graph - no roles, no products
    /// Shows: CBU → ownership_relationships → control_relationships → natural persons
    UboOnly,
    /// Products Only view: CBU → Products (simple, clean view)
    ProductsOnly,
    /// Service Delivery view: Products → Services → Resources + Trading entities
    /// Shows: CBU → Products → Services → Resources, plus entities with TRADING_EXECUTION roles
    ServiceDelivery,
    /// Trading view: CBU as container with trading entities inside (Asset Owner, IM, ManCo, etc.)
    /// No products, no services - just the trading entities wrapped by the CBU
    Trading,
}

impl ViewMode {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "UBO_ONLY" | "UBO" | "OWNERSHIP" => ViewMode::UboOnly,
            "PRODUCTS_ONLY" | "PRODUCTS" => ViewMode::ProductsOnly,
            "SERVICE_DELIVERY" | "SERVICES" => ViewMode::ServiceDelivery,
            "TRADING" => ViewMode::Trading,
            _ => ViewMode::KycUbo, // Default
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ViewMode::KycUbo => "KYC_UBO",
            ViewMode::UboOnly => "UBO_ONLY",
            ViewMode::ProductsOnly => "PRODUCTS_ONLY",
            ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
            ViewMode::Trading => "TRADING",
        }
    }
}

/// Layout orientation determines flow direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// Horizontal: tiers flow left-to-right, splits are top/bottom
    #[default]
    Horizontal,
    /// Vertical: tiers flow top-to-bottom, splits are left/right
    Vertical,
}

impl Orientation {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "HORIZONTAL" | "LTR" | "LEFT_TO_RIGHT" => Orientation::Horizontal,
            "VERTICAL" | "TTB" | "TOP_TO_BOTTOM" => Orientation::Vertical,
            _ => Orientation::Vertical, // Default to vertical (more natural for trees)
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Orientation::Horizontal => "HORIZONTAL",
            Orientation::Vertical => "VERTICAL",
        }
    }
}

/// Layout configuration constants
pub struct LayoutConfig {
    /// Horizontal spacing between nodes
    pub node_spacing_x: f32,
    /// Vertical spacing between tiers
    pub tier_spacing_y: f32,
    /// Default node width
    pub node_width: f32,
    /// Default node height
    pub node_height: f32,
    /// Canvas width for centering
    pub canvas_width: f32,
    /// Left margin for SHELL nodes
    pub shell_margin_left: f32,
    /// Right margin for PERSON nodes
    pub person_margin_right: f32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            node_spacing_x: 180.0,
            tier_spacing_y: 120.0,
            node_width: 160.0,
            node_height: 60.0,
            canvas_width: 1200.0,
            shell_margin_left: 100.0,
            person_margin_right: 100.0,
        }
    }
}

/// Layout engine that computes node positions based on view mode and orientation
pub struct LayoutEngine {
    config: LayoutConfig,
    view_mode: ViewMode,
    orientation: Orientation,
}

impl LayoutEngine {
    pub fn new(view_mode: ViewMode) -> Self {
        Self {
            config: LayoutConfig::default(),
            view_mode,
            orientation: Orientation::default(),
        }
    }

    pub fn with_orientation(view_mode: ViewMode, orientation: Orientation) -> Self {
        Self {
            config: LayoutConfig::default(),
            view_mode,
            orientation,
        }
    }

    pub fn with_config(
        view_mode: ViewMode,
        orientation: Orientation,
        config: LayoutConfig,
    ) -> Self {
        Self {
            config,
            view_mode,
            orientation,
        }
    }

    /// Get the layout behavior for a node, using the new taxonomy fields
    /// with fallback to legacy role_priority for backward compatibility
    fn get_layout_behavior(&self, node: &GraphNode) -> LayoutBehavior {
        // First, try to get behavior from the node's layout_behavior field (set by builder)
        if let Some(ref behavior_str) = node.layout_behavior {
            if let Ok(behavior) = behavior_str.parse::<LayoutBehavior>() {
                return behavior;
            }
        }

        // Second, try to derive from primary_role_category
        if let Some(ref category_str) = node.primary_role_category {
            if let Ok(category) = category_str.parse::<RoleCategory>() {
                return category.layout_behavior();
            }
        }

        // Fallback: use legacy role_priority for backward compatibility
        let priority = node.role_priority.unwrap_or(0);
        if priority >= 100 {
            // OWNERSHIP_CONTROL legacy category
            LayoutBehavior::PyramidUp
        } else if priority >= 50 {
            // BOTH legacy category (directors, signatories)
            LayoutBehavior::Overlay
        } else if priority >= 10 {
            // TRADING_EXECUTION legacy category
            LayoutBehavior::Satellite
        } else {
            // Unknown/peripheral
            LayoutBehavior::Peripheral
        }
    }

    /// Apply layout to the graph, computing x/y positions for all nodes
    pub fn layout(&self, graph: &mut CbuGraph) {
        match (self.view_mode, self.orientation) {
            (ViewMode::KycUbo, Orientation::Vertical) => self.layout_kyc_ubo_vertical(graph),
            (ViewMode::KycUbo, Orientation::Horizontal) => self.layout_kyc_ubo_horizontal(graph),
            (ViewMode::UboOnly, Orientation::Vertical) => self.layout_ubo_only_vertical(graph),
            (ViewMode::UboOnly, Orientation::Horizontal) => self.layout_ubo_only_horizontal(graph),
            (ViewMode::ProductsOnly, Orientation::Vertical) => {
                self.layout_products_only_vertical(graph)
            }
            (ViewMode::ProductsOnly, Orientation::Horizontal) => {
                self.layout_products_only_horizontal(graph)
            }
            (ViewMode::ServiceDelivery, Orientation::Vertical) => {
                self.layout_service_delivery_vertical(graph)
            }
            (ViewMode::ServiceDelivery, Orientation::Horizontal) => {
                self.layout_service_delivery_horizontal(graph)
            }
            (ViewMode::Trading, Orientation::Vertical) => self.layout_trading_vertical(graph),
            (ViewMode::Trading, Orientation::Horizontal) => self.layout_trading_horizontal(graph),
        }
    }

    /// Trading layout (VERTICAL): CBU at top, trading entities arranged below
    /// CBU acts as container, entities inside (Asset Owner, IM, ManCo, etc.)
    fn layout_trading_vertical(&self, graph: &mut CbuGraph) {
        let center_x = self.config.canvas_width / 2.0;
        let mut y = self.config.tier_spacing_y;

        // Find CBU node
        let cbu_idx = graph
            .nodes
            .iter()
            .position(|n| n.node_type == NodeType::Cbu);

        // Position CBU at top center
        if let Some(idx) = cbu_idx {
            graph.nodes[idx].x = Some(center_x);
            graph.nodes[idx].y = Some(y);
        }

        y += self.config.tier_spacing_y * 1.5;

        // Collect trading entities (non-CBU, non-product)
        let trading_entities: Vec<usize> = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.node_type != NodeType::Cbu && n.node_type != NodeType::Product)
            .map(|(i, _)| i)
            .collect();

        // Arrange trading entities in a grid
        let cols = 3;
        let spacing_x = self.config.node_spacing_x * 1.5;
        let start_x = center_x - (cols as f32 - 1.0) * spacing_x / 2.0;

        for (i, &idx) in trading_entities.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            graph.nodes[idx].x = Some(start_x + col as f32 * spacing_x);
            graph.nodes[idx].y = Some(y + row as f32 * self.config.tier_spacing_y);
        }
    }

    /// Trading layout (HORIZONTAL): CBU at left, trading entities to the right
    fn layout_trading_horizontal(&self, graph: &mut CbuGraph) {
        let center_y = self.config.canvas_width / 2.0; // Use same config for centering
        let mut x = self.config.tier_spacing_y;

        // Find CBU node
        let cbu_idx = graph
            .nodes
            .iter()
            .position(|n| n.node_type == NodeType::Cbu);

        // Position CBU at left center
        if let Some(idx) = cbu_idx {
            graph.nodes[idx].x = Some(x);
            graph.nodes[idx].y = Some(center_y);
        }

        x += self.config.tier_spacing_y * 1.5;

        // Collect trading entities
        let trading_entities: Vec<usize> = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.node_type != NodeType::Cbu && n.node_type != NodeType::Product)
            .map(|(i, _)| i)
            .collect();

        // Arrange in rows
        let rows = 3;
        let spacing_y = self.config.node_spacing_x * 1.5;
        let start_y = center_y - (rows as f32 - 1.0) * spacing_y / 2.0;

        for (i, &idx) in trading_entities.iter().enumerate() {
            let row = i % rows;
            let col = i / rows;
            graph.nodes[idx].x = Some(x + col as f32 * self.config.tier_spacing_y);
            graph.nodes[idx].y = Some(start_y + row as f32 * spacing_y);
        }
    }

    /// KYC/UBO layout (VERTICAL - top to bottom): Hierarchical by LayoutBehavior with SHELL/PERSON split
    ///
    /// Uses RoleCategory → LayoutBehavior mapping for tier placement:
    ///
    /// ```text
    /// Tier 0: CBU (center)
    /// Tier 1: Products (center-right, if any)
    /// Tier 2: PyramidUp/PyramidDown (ownership/control chains) - SHELL left, PERSON right
    /// Tier 3: TreeDown/Overlay (fund structure, service providers) - SHELL left, PERSON right
    /// Tier 4: Satellite/Radial (trading, trust roles) - SHELL left, PERSON right
    /// Tier 5: FlatBottom/FlatRight (investor chain) - SHELL left, PERSON right
    /// Tier 6: Peripheral (related parties) and other nodes
    /// ```
    fn layout_kyc_ubo_vertical(&self, graph: &mut CbuGraph) {
        let center_x = self.config.canvas_width / 2.0;

        // Collect nodes by tier based on LayoutBehavior
        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut tier_1: Vec<usize> = Vec::new(); // Products
        let mut tier_2_shell: Vec<usize> = Vec::new(); // PyramidUp/PyramidDown + SHELL
        let mut tier_2_person: Vec<usize> = Vec::new(); // PyramidUp/PyramidDown + PERSON
        let mut tier_3_shell: Vec<usize> = Vec::new(); // TreeDown/Overlay + SHELL
        let mut tier_3_person: Vec<usize> = Vec::new(); // TreeDown/Overlay + PERSON
        let mut tier_4_shell: Vec<usize> = Vec::new(); // Satellite/Radial + SHELL
        let mut tier_4_person: Vec<usize> = Vec::new(); // Satellite/Radial + PERSON
        let mut tier_5_shell: Vec<usize> = Vec::new(); // FlatBottom/FlatRight + SHELL
        let mut tier_5_person: Vec<usize> = Vec::new(); // FlatBottom/FlatRight + PERSON
        let mut tier_6: Vec<usize> = Vec::new(); // Peripheral and other nodes

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Product => tier_1.push(idx),
                NodeType::Entity => {
                    let is_shell = node.entity_category.as_deref() == Some("SHELL");

                    // Get layout behavior from the node, or derive from role category
                    let behavior = self.get_layout_behavior(node);

                    match behavior {
                        LayoutBehavior::PyramidUp | LayoutBehavior::PyramidDown => {
                            // Ownership/control chains - highest tier
                            if is_shell {
                                tier_2_shell.push(idx);
                            } else {
                                tier_2_person.push(idx);
                            }
                        }
                        LayoutBehavior::TreeDown | LayoutBehavior::Overlay => {
                            // Fund structure, service providers
                            if is_shell {
                                tier_3_shell.push(idx);
                            } else {
                                tier_3_person.push(idx);
                            }
                        }
                        LayoutBehavior::Satellite | LayoutBehavior::Radial => {
                            // Trading execution, trust roles
                            if is_shell {
                                tier_4_shell.push(idx);
                            } else {
                                tier_4_person.push(idx);
                            }
                        }
                        LayoutBehavior::FlatBottom | LayoutBehavior::FlatRight => {
                            // Investor chain
                            if is_shell {
                                tier_5_shell.push(idx);
                            } else {
                                tier_5_person.push(idx);
                            }
                        }
                        LayoutBehavior::Peripheral => {
                            // Related parties, edge entities
                            tier_6.push(idx);
                        }
                    }
                }
                NodeType::Service | NodeType::Resource => {
                    // Skip in KYC view, or place below products
                    tier_6.push(idx);
                }
                _ => tier_6.push(idx),
            }
        }

        // Layout each tier
        self.layout_tier_centered(&mut graph.nodes, &tier_0, 0, center_x);
        self.layout_tier_centered(&mut graph.nodes, &tier_1, 1, center_x + 200.0);

        // Tier 2: Ownership/Control chains (PyramidUp/PyramidDown)
        let tier_2_y = 2.0 * self.config.tier_spacing_y;
        self.layout_tier_left(&mut graph.nodes, &tier_2_shell, 2, tier_2_y);
        self.layout_tier_right(&mut graph.nodes, &tier_2_person, 2, tier_2_y);

        // Tier 3: Fund structure/Service providers (TreeDown/Overlay)
        let tier_3_y = 3.0 * self.config.tier_spacing_y;
        self.layout_tier_left(&mut graph.nodes, &tier_3_shell, 3, tier_3_y);
        self.layout_tier_right(&mut graph.nodes, &tier_3_person, 3, tier_3_y);

        // Tier 4: Trading/Trust roles (Satellite/Radial)
        let tier_4_y = 4.0 * self.config.tier_spacing_y;
        self.layout_tier_left(&mut graph.nodes, &tier_4_shell, 4, tier_4_y);
        self.layout_tier_right(&mut graph.nodes, &tier_4_person, 4, tier_4_y);

        // Tier 5: Investor chain (FlatBottom/FlatRight)
        let tier_5_y = 5.0 * self.config.tier_spacing_y;
        self.layout_tier_left(&mut graph.nodes, &tier_5_shell, 5, tier_5_y);
        self.layout_tier_right(&mut graph.nodes, &tier_5_person, 5, tier_5_y);

        // Tier 6: Peripheral and other nodes
        self.layout_tier_centered(&mut graph.nodes, &tier_6, 6, center_x);
    }

    /// UBO Only layout (VERTICAL): Pure ownership/control graph
    ///
    /// Shows ownership chains from CBU's subject entities up to natural person UBOs.
    /// No role-based connections, no products/services.
    ///
    /// ```text
    /// Tier 0: CBU (center)
    /// Tier 1: Direct owned entities (SHELL left, PERSON right)
    /// Tier 2: Intermediate owners (SHELL left, PERSON right)
    /// Tier 3+: Ultimate beneficial owners (natural persons, right side)
    /// ```
    fn layout_ubo_only_vertical(&self, graph: &mut CbuGraph) {
        let center_x = self.config.canvas_width / 2.0;

        // Collect nodes - in UBO view, entities come from ownership graph
        // We tier by entity_category (SHELL vs PERSON) since ownership flows upward
        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut shells: Vec<usize> = Vec::new(); // SHELL entities (intermediate)
        let mut persons: Vec<usize> = Vec::new(); // PERSON entities (UBOs)

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Entity => {
                    if node.entity_category.as_deref() == Some("PERSON") {
                        persons.push(idx);
                    } else {
                        shells.push(idx);
                    }
                }
                // Skip all other node types in UBO view
                _ => {}
            }
        }

        // Layout: CBU at top, shells below left, persons below right
        self.layout_tier_centered(&mut graph.nodes, &tier_0, 0, center_x);

        // Tier 1: Shell entities (ownership intermediaries) - left side
        let tier_1_y = self.config.tier_spacing_y;
        self.layout_tier_left(&mut graph.nodes, &shells, 1, tier_1_y);

        // Tier 2: Person entities (UBOs) - right side
        let tier_2_y = 2.0 * self.config.tier_spacing_y;
        self.layout_tier_right(&mut graph.nodes, &persons, 2, tier_2_y);
    }

    /// UBO Only layout (HORIZONTAL): Pure ownership/control graph
    fn layout_ubo_only_horizontal(&self, graph: &mut CbuGraph) {
        let center_y = 300.0;

        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut shells: Vec<usize> = Vec::new(); // SHELL entities
        let mut persons: Vec<usize> = Vec::new(); // PERSON entities

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Entity => {
                    if node.entity_category.as_deref() == Some("PERSON") {
                        persons.push(idx);
                    } else {
                        shells.push(idx);
                    }
                }
                _ => {}
            }
        }

        // Horizontal: CBU left, shells middle (top), persons right (bottom)
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_0, 0, center_y);

        let tier_1_x = self.config.node_spacing_x + 100.0;
        self.layout_tier_horizontal_top(&mut graph.nodes, &shells, 1, tier_1_x);

        let tier_2_x = 2.0 * self.config.node_spacing_x + 100.0;
        self.layout_tier_horizontal_bottom(&mut graph.nodes, &persons, 2, tier_2_x);
    }

    /// Products Only layout (VERTICAL): Simple CBU → Products view
    /// Suppresses services and resources for a cleaner view
    fn layout_products_only_vertical(&self, graph: &mut CbuGraph) {
        let center_x = self.config.canvas_width / 2.0;

        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut tier_1: Vec<usize> = Vec::new(); // Products
        let mut tier_2: Vec<usize> = Vec::new(); // Entities (optional context)

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Product => tier_1.push(idx),
                NodeType::Entity => tier_2.push(idx),
                // Skip services, resources, and other nodes entirely
                _ => {}
            }
        }

        self.layout_tier_centered(&mut graph.nodes, &tier_0, 0, center_x);
        self.layout_tier_centered(&mut graph.nodes, &tier_1, 1, center_x);
        self.layout_tier_centered(&mut graph.nodes, &tier_2, 2, center_x);
    }

    /// Products Only layout (HORIZONTAL): Simple CBU → Products view
    fn layout_products_only_horizontal(&self, graph: &mut CbuGraph) {
        let center_y = 300.0;

        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut tier_1: Vec<usize> = Vec::new(); // Products
        let mut tier_2: Vec<usize> = Vec::new(); // Entities

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Product => tier_1.push(idx),
                NodeType::Entity => tier_2.push(idx),
                _ => {}
            }
        }

        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_0, 0, center_y);
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_1, 1, center_y);
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_2, 2, center_y);
    }

    /// Service Delivery layout (VERTICAL): Tree from CBU → Products → Services → Resources
    fn layout_service_delivery_vertical(&self, graph: &mut CbuGraph) {
        let center_x = self.config.canvas_width / 2.0;

        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut tier_1: Vec<usize> = Vec::new(); // Products
        let mut tier_2: Vec<usize> = Vec::new(); // Services
        let mut tier_3: Vec<usize> = Vec::new(); // Resources
        let mut tier_4: Vec<usize> = Vec::new(); // Entities (minimal in this view)

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Product => tier_1.push(idx),
                NodeType::Service => tier_2.push(idx),
                NodeType::Resource => tier_3.push(idx),
                NodeType::Entity => tier_4.push(idx),
                _ => {} // Skip custody, KYC nodes in this view
            }
        }

        self.layout_tier_centered(&mut graph.nodes, &tier_0, 0, center_x);
        self.layout_tier_centered(&mut graph.nodes, &tier_1, 1, center_x);
        self.layout_tier_centered(&mut graph.nodes, &tier_2, 2, center_x);
        self.layout_tier_centered(&mut graph.nodes, &tier_3, 3, center_x);
        self.layout_tier_centered(&mut graph.nodes, &tier_4, 4, center_x);
    }

    /// KYC/UBO layout (HORIZONTAL - left to right): Tiers flow horizontally, splits are top/bottom
    ///
    /// Uses RoleCategory → LayoutBehavior mapping for tier placement:
    ///
    /// ```text
    /// Tier 0: CBU (left, center-y)
    /// Tier 1: Products (next column)
    /// Tier 2: PyramidUp/PyramidDown (ownership/control) - SHELL top, PERSON bottom
    /// Tier 3: TreeDown/Overlay (fund structure, service providers) - SHELL top, PERSON bottom
    /// Tier 4: Satellite/Radial (trading, trust roles) - SHELL top, PERSON bottom
    /// Tier 5: FlatBottom/FlatRight (investor chain) - SHELL top, PERSON bottom
    /// Tier 6: Peripheral (related parties) and other nodes
    /// ```
    fn layout_kyc_ubo_horizontal(&self, graph: &mut CbuGraph) {
        let center_y = 300.0; // Vertical center for the layout

        // Collect nodes by tier based on LayoutBehavior (same logic as vertical)
        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut tier_1: Vec<usize> = Vec::new(); // Products
        let mut tier_2_shell: Vec<usize> = Vec::new(); // PyramidUp/PyramidDown + SHELL
        let mut tier_2_person: Vec<usize> = Vec::new(); // PyramidUp/PyramidDown + PERSON
        let mut tier_3_shell: Vec<usize> = Vec::new(); // TreeDown/Overlay + SHELL
        let mut tier_3_person: Vec<usize> = Vec::new(); // TreeDown/Overlay + PERSON
        let mut tier_4_shell: Vec<usize> = Vec::new(); // Satellite/Radial + SHELL
        let mut tier_4_person: Vec<usize> = Vec::new(); // Satellite/Radial + PERSON
        let mut tier_5_shell: Vec<usize> = Vec::new(); // FlatBottom/FlatRight + SHELL
        let mut tier_5_person: Vec<usize> = Vec::new(); // FlatBottom/FlatRight + PERSON
        let mut tier_6: Vec<usize> = Vec::new(); // Peripheral and other nodes

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Product => tier_1.push(idx),
                NodeType::Entity => {
                    let is_shell = node.entity_category.as_deref() == Some("SHELL");
                    let behavior = self.get_layout_behavior(node);

                    match behavior {
                        LayoutBehavior::PyramidUp | LayoutBehavior::PyramidDown => {
                            if is_shell {
                                tier_2_shell.push(idx);
                            } else {
                                tier_2_person.push(idx);
                            }
                        }
                        LayoutBehavior::TreeDown | LayoutBehavior::Overlay => {
                            if is_shell {
                                tier_3_shell.push(idx);
                            } else {
                                tier_3_person.push(idx);
                            }
                        }
                        LayoutBehavior::Satellite | LayoutBehavior::Radial => {
                            if is_shell {
                                tier_4_shell.push(idx);
                            } else {
                                tier_4_person.push(idx);
                            }
                        }
                        LayoutBehavior::FlatBottom | LayoutBehavior::FlatRight => {
                            if is_shell {
                                tier_5_shell.push(idx);
                            } else {
                                tier_5_person.push(idx);
                            }
                        }
                        LayoutBehavior::Peripheral => {
                            tier_6.push(idx);
                        }
                    }
                }
                NodeType::Service | NodeType::Resource => tier_6.push(idx),
                _ => tier_6.push(idx),
            }
        }

        // Horizontal layout: x increases with tier, y splits for shell/person
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_0, 0, center_y);
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_1, 1, center_y - 80.0);

        // Tier 2: Ownership/Control chains (PyramidUp/PyramidDown) - shells top, persons bottom
        let tier_2_x = 2.0 * self.config.node_spacing_x + 100.0;
        self.layout_tier_horizontal_top(&mut graph.nodes, &tier_2_shell, 2, tier_2_x);
        self.layout_tier_horizontal_bottom(&mut graph.nodes, &tier_2_person, 2, tier_2_x);

        // Tier 3: Fund structure/Service providers (TreeDown/Overlay)
        let tier_3_x = 3.0 * self.config.node_spacing_x + 100.0;
        self.layout_tier_horizontal_top(&mut graph.nodes, &tier_3_shell, 3, tier_3_x);
        self.layout_tier_horizontal_bottom(&mut graph.nodes, &tier_3_person, 3, tier_3_x);

        // Tier 4: Trading/Trust roles (Satellite/Radial)
        let tier_4_x = 4.0 * self.config.node_spacing_x + 100.0;
        self.layout_tier_horizontal_top(&mut graph.nodes, &tier_4_shell, 4, tier_4_x);
        self.layout_tier_horizontal_bottom(&mut graph.nodes, &tier_4_person, 4, tier_4_x);

        // Tier 5: Investor chain (FlatBottom/FlatRight)
        let tier_5_x = 5.0 * self.config.node_spacing_x + 100.0;
        self.layout_tier_horizontal_top(&mut graph.nodes, &tier_5_shell, 5, tier_5_x);
        self.layout_tier_horizontal_bottom(&mut graph.nodes, &tier_5_person, 5, tier_5_x);

        // Tier 6: Peripheral and other nodes - far right
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_6, 6, center_y);
    }

    /// Service Delivery layout (HORIZONTAL): Tree flows left to right
    fn layout_service_delivery_horizontal(&self, graph: &mut CbuGraph) {
        let center_y = 300.0;

        let mut tier_0: Vec<usize> = Vec::new();
        let mut tier_1: Vec<usize> = Vec::new();
        let mut tier_2: Vec<usize> = Vec::new();
        let mut tier_3: Vec<usize> = Vec::new();
        let mut tier_4: Vec<usize> = Vec::new();

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Product => tier_1.push(idx),
                NodeType::Service => tier_2.push(idx),
                NodeType::Resource => tier_3.push(idx),
                NodeType::Entity => tier_4.push(idx),
                _ => {}
            }
        }

        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_0, 0, center_y);
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_1, 1, center_y);
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_2, 2, center_y);
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_3, 3, center_y);
        self.layout_tier_horizontal_centered(&mut graph.nodes, &tier_4, 4, center_y);
    }

    /// Layout nodes centered around a given x position
    fn layout_tier_centered(
        &self,
        nodes: &mut [GraphNode],
        indices: &[usize],
        tier: i32,
        center_x: f32,
    ) {
        if indices.is_empty() {
            return;
        }

        let total_width = indices.len() as f32 * self.config.node_spacing_x;
        let start_x = center_x - total_width / 2.0 + self.config.node_spacing_x / 2.0;
        let y = tier as f32 * self.config.tier_spacing_y;

        for (i, &idx) in indices.iter().enumerate() {
            let node = &mut nodes[idx];
            node.x = Some(start_x + i as f32 * self.config.node_spacing_x);
            node.y = Some(y);
            node.width = Some(self.config.node_width);
            node.height = Some(self.config.node_height);
            node.layout_tier = Some(tier);
        }
    }

    /// Layout SHELL nodes on the left side
    fn layout_tier_left(&self, nodes: &mut [GraphNode], indices: &[usize], tier: i32, y: f32) {
        if indices.is_empty() {
            return;
        }

        let start_x = self.config.shell_margin_left;

        for (i, &idx) in indices.iter().enumerate() {
            let node = &mut nodes[idx];
            node.x = Some(start_x + i as f32 * self.config.node_spacing_x);
            node.y = Some(y);
            node.width = Some(self.config.node_width);
            node.height = Some(self.config.node_height);
            node.layout_tier = Some(tier);
        }
    }

    /// Layout PERSON nodes on the right side
    fn layout_tier_right(&self, nodes: &mut [GraphNode], indices: &[usize], tier: i32, y: f32) {
        if indices.is_empty() {
            return;
        }

        // Calculate start position: right-aligned with margin
        // Ensure we don't go negative - start at center if too many nodes
        let total_width = indices.len() as f32 * self.config.node_spacing_x;
        let ideal_start = self.config.canvas_width - self.config.person_margin_right - total_width;
        let start_x = ideal_start.max(self.config.canvas_width / 2.0); // Don't go left of center

        for (i, &idx) in indices.iter().enumerate() {
            let node = &mut nodes[idx];
            node.x = Some(start_x + i as f32 * self.config.node_spacing_x);
            node.y = Some(y);
            node.width = Some(self.config.node_width);
            node.height = Some(self.config.node_height);
            node.layout_tier = Some(tier);
        }
    }

    // ========== HORIZONTAL LAYOUT HELPERS ==========

    /// Layout nodes centered around a given y position (horizontal mode)
    fn layout_tier_horizontal_centered(
        &self,
        nodes: &mut [GraphNode],
        indices: &[usize],
        tier: i32,
        center_y: f32,
    ) {
        if indices.is_empty() {
            return;
        }

        let x = tier as f32 * self.config.node_spacing_x + 100.0;
        let total_height = indices.len() as f32 * self.config.tier_spacing_y;
        let start_y = center_y - total_height / 2.0 + self.config.tier_spacing_y / 2.0;

        for (i, &idx) in indices.iter().enumerate() {
            let node = &mut nodes[idx];
            node.x = Some(x);
            node.y = Some(start_y + i as f32 * self.config.tier_spacing_y);
            node.width = Some(self.config.node_width);
            node.height = Some(self.config.node_height);
            node.layout_tier = Some(tier);
        }
    }

    /// Layout SHELL nodes on top (horizontal mode)
    fn layout_tier_horizontal_top(
        &self,
        nodes: &mut [GraphNode],
        indices: &[usize],
        tier: i32,
        x: f32,
    ) {
        if indices.is_empty() {
            return;
        }

        let start_y = 50.0; // Top margin

        for (i, &idx) in indices.iter().enumerate() {
            let node = &mut nodes[idx];
            node.x = Some(x);
            node.y = Some(start_y + i as f32 * self.config.tier_spacing_y);
            node.width = Some(self.config.node_width);
            node.height = Some(self.config.node_height);
            node.layout_tier = Some(tier);
        }
    }

    /// Layout PERSON nodes on bottom (horizontal mode)
    fn layout_tier_horizontal_bottom(
        &self,
        nodes: &mut [GraphNode],
        indices: &[usize],
        tier: i32,
        x: f32,
    ) {
        if indices.is_empty() {
            return;
        }

        let canvas_height = 600.0;
        let total_height = indices.len() as f32 * self.config.tier_spacing_y;
        let start_y = (canvas_height - 50.0 - total_height).max(canvas_height / 2.0);

        for (i, &idx) in indices.iter().enumerate() {
            let node = &mut nodes[idx];
            node.x = Some(x);
            node.y = Some(start_y + i as f32 * self.config.tier_spacing_y);
            node.width = Some(self.config.node_width);
            node.height = Some(self.config.node_height);
            node.layout_tier = Some(tier);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_parse() {
        assert_eq!(ViewMode::parse("KYC_UBO"), ViewMode::KycUbo);
        assert_eq!(ViewMode::parse("kyc_ubo"), ViewMode::KycUbo);
        assert_eq!(
            ViewMode::parse("SERVICE_DELIVERY"),
            ViewMode::ServiceDelivery
        );
        assert_eq!(ViewMode::parse("services"), ViewMode::ServiceDelivery);
        assert_eq!(ViewMode::parse("PRODUCTS_ONLY"), ViewMode::ProductsOnly);
        assert_eq!(ViewMode::parse("CUSTODY"), ViewMode::KycUbo); // Custody removed, defaults to KycUbo
        assert_eq!(ViewMode::parse("unknown"), ViewMode::KycUbo); // Default
    }

    #[test]
    fn test_orientation_parse() {
        assert_eq!(Orientation::parse("VERTICAL"), Orientation::Vertical);
        assert_eq!(Orientation::parse("vertical"), Orientation::Vertical);
        assert_eq!(Orientation::parse("HORIZONTAL"), Orientation::Horizontal);
        assert_eq!(Orientation::parse("LTR"), Orientation::Horizontal);
        assert_eq!(Orientation::parse("TTB"), Orientation::Vertical);
        assert_eq!(Orientation::parse("unknown"), Orientation::Vertical); // Default
    }
}
