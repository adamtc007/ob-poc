//! Server-side layout engine for CBU graph visualization
//!
//! Computes x, y positions for graph nodes based on view mode and graph semantics.
//! The UI receives pre-positioned nodes and just renders them.

use super::types::{CbuGraph, GraphNode, NodeType};

/// View modes determine which layers are visible and how they're laid out
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// KYC/UBO view: Entity hierarchy by role priority, ownership chains
    #[default]
    KycUbo,
    /// Service Delivery view: Products → Services → Resources tree
    ServiceDelivery,
    /// Custody view: Markets → Universe/SSI/Rules
    Custody,
}

impl ViewMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "SERVICE_DELIVERY" | "SERVICES" => ViewMode::ServiceDelivery,
            "CUSTODY" => ViewMode::Custody,
            _ => ViewMode::KycUbo, // Default
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ViewMode::KycUbo => "KYC_UBO",
            ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
            ViewMode::Custody => "CUSTODY",
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

/// Layout engine that computes node positions based on view mode
pub struct LayoutEngine {
    config: LayoutConfig,
    view_mode: ViewMode,
}

impl LayoutEngine {
    pub fn new(view_mode: ViewMode) -> Self {
        Self {
            config: LayoutConfig::default(),
            view_mode,
        }
    }

    pub fn with_config(view_mode: ViewMode, config: LayoutConfig) -> Self {
        Self { config, view_mode }
    }

    /// Apply layout to the graph, computing x/y positions for all nodes
    pub fn layout(&self, graph: &mut CbuGraph) {
        match self.view_mode {
            ViewMode::KycUbo => self.layout_kyc_ubo(graph),
            ViewMode::ServiceDelivery => self.layout_service_delivery(graph),
            ViewMode::Custody => self.layout_custody(graph),
        }
    }

    /// KYC/UBO layout: Hierarchical by role priority with SHELL/PERSON split
    ///
    /// ```text
    /// Tier 0: CBU (center)
    /// Tier 1: Products (center-right, if any)
    /// Tier 2: OWNERSHIP_CONTROL entities (SHELL left, PERSON right)
    /// Tier 3: BOTH entities (SHELL left, PERSON right)
    /// Tier 4: TRADING_EXECUTION entities (SHELL left, PERSON right)
    /// ```
    fn layout_kyc_ubo(&self, graph: &mut CbuGraph) {
        let center_x = self.config.canvas_width / 2.0;

        // Collect nodes by tier
        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut tier_1: Vec<usize> = Vec::new(); // Products
        let mut tier_2_shell: Vec<usize> = Vec::new(); // OWNERSHIP_CONTROL + SHELL
        let mut tier_2_person: Vec<usize> = Vec::new(); // OWNERSHIP_CONTROL + PERSON
        let mut tier_3_shell: Vec<usize> = Vec::new(); // BOTH + SHELL
        let mut tier_3_person: Vec<usize> = Vec::new(); // BOTH + PERSON
        let mut tier_4_shell: Vec<usize> = Vec::new(); // TRADING + SHELL
        let mut tier_4_person: Vec<usize> = Vec::new(); // TRADING + PERSON
        let mut other: Vec<usize> = Vec::new(); // KYC, documents, etc.

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Product => tier_1.push(idx),
                NodeType::Entity => {
                    let priority = node.role_priority.unwrap_or(0);
                    let is_shell = node.entity_category.as_deref() == Some("SHELL");

                    if priority >= 100 {
                        // OWNERSHIP_CONTROL
                        if is_shell {
                            tier_2_shell.push(idx);
                        } else {
                            tier_2_person.push(idx);
                        }
                    } else if priority >= 50 {
                        // BOTH
                        if is_shell {
                            tier_3_shell.push(idx);
                        } else {
                            tier_3_person.push(idx);
                        }
                    } else {
                        // TRADING_EXECUTION or unknown
                        if is_shell {
                            tier_4_shell.push(idx);
                        } else {
                            tier_4_person.push(idx);
                        }
                    }
                }
                NodeType::Service | NodeType::Resource => {
                    // Skip in KYC view, or place below products
                    other.push(idx);
                }
                _ => other.push(idx),
            }
        }

        // Layout each tier
        self.layout_tier_centered(&mut graph.nodes, &tier_0, 0, center_x);
        self.layout_tier_centered(&mut graph.nodes, &tier_1, 1, center_x + 200.0);

        // Tier 2: OWNERSHIP_CONTROL - shells left, persons right
        let tier_2_y = 2.0 * self.config.tier_spacing_y;
        self.layout_tier_left(&mut graph.nodes, &tier_2_shell, 2, tier_2_y);
        self.layout_tier_right(&mut graph.nodes, &tier_2_person, 2, tier_2_y);

        // Tier 3: BOTH
        let tier_3_y = 3.0 * self.config.tier_spacing_y;
        self.layout_tier_left(&mut graph.nodes, &tier_3_shell, 3, tier_3_y);
        self.layout_tier_right(&mut graph.nodes, &tier_3_person, 3, tier_3_y);

        // Tier 4: TRADING_EXECUTION
        let tier_4_y = 4.0 * self.config.tier_spacing_y;
        self.layout_tier_left(&mut graph.nodes, &tier_4_shell, 4, tier_4_y);
        self.layout_tier_right(&mut graph.nodes, &tier_4_person, 4, tier_4_y);

        // Other nodes (KYC, documents) - place at bottom
        self.layout_tier_centered(&mut graph.nodes, &other, 5, center_x);
    }

    /// Service Delivery layout: Tree from CBU → Products → Services → Resources
    fn layout_service_delivery(&self, graph: &mut CbuGraph) {
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

    /// Custody layout: Markets as columns, Universe/SSI/Rules under each
    fn layout_custody(&self, graph: &mut CbuGraph) {
        let center_x = self.config.canvas_width / 2.0;

        let mut tier_0: Vec<usize> = Vec::new(); // CBU
        let mut markets: Vec<usize> = Vec::new();
        let mut universe: Vec<usize> = Vec::new();
        let mut ssis: Vec<usize> = Vec::new();
        let mut rules: Vec<usize> = Vec::new();
        let mut isda: Vec<usize> = Vec::new();
        let mut csa: Vec<usize> = Vec::new();

        for (idx, node) in graph.nodes.iter().enumerate() {
            match node.node_type {
                NodeType::Cbu => tier_0.push(idx),
                NodeType::Market => markets.push(idx),
                NodeType::Universe => universe.push(idx),
                NodeType::Ssi => ssis.push(idx),
                NodeType::BookingRule => rules.push(idx),
                NodeType::Isda => isda.push(idx),
                NodeType::Csa => csa.push(idx),
                _ => {} // Skip entities, services in custody view
            }
        }

        self.layout_tier_centered(&mut graph.nodes, &tier_0, 0, center_x);
        self.layout_tier_centered(&mut graph.nodes, &markets, 1, center_x);
        self.layout_tier_centered(&mut graph.nodes, &universe, 2, center_x - 150.0);
        self.layout_tier_centered(&mut graph.nodes, &ssis, 2, center_x + 150.0);
        self.layout_tier_centered(&mut graph.nodes, &rules, 3, center_x);
        self.layout_tier_centered(&mut graph.nodes, &isda, 4, center_x - 100.0);
        self.layout_tier_centered(&mut graph.nodes, &csa, 4, center_x + 100.0);
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

        let start_x = self.config.canvas_width
            - self.config.person_margin_right
            - (indices.len() as f32 * self.config.node_spacing_x);

        for (i, &idx) in indices.iter().enumerate() {
            let node = &mut nodes[idx];
            node.x = Some(start_x + i as f32 * self.config.node_spacing_x);
            node.y = Some(y);
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
    fn test_view_mode_from_str() {
        assert_eq!(ViewMode::from_str("KYC_UBO"), ViewMode::KycUbo);
        assert_eq!(ViewMode::from_str("kyc_ubo"), ViewMode::KycUbo);
        assert_eq!(
            ViewMode::from_str("SERVICE_DELIVERY"),
            ViewMode::ServiceDelivery
        );
        assert_eq!(ViewMode::from_str("services"), ViewMode::ServiceDelivery);
        assert_eq!(ViewMode::from_str("CUSTODY"), ViewMode::Custody);
        assert_eq!(ViewMode::from_str("unknown"), ViewMode::KycUbo); // Default
    }
}
