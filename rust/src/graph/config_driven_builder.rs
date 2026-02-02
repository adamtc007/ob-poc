//! ConfigDrivenGraphBuilder - builds graphs using database-driven configuration
//!
//! This replaces the hardcoded logic in CbuGraphBuilder with config-driven behavior.
//! Node/edge visibility and layout hints are determined by database configuration
//! in the node_types, edge_types, and view_modes tables.
//!
//! ## Key differences from CbuGraphBuilder:
//!
//! 1. **No hardcoded `is_ubo_relevant()` / `is_trading_relevant()`** - visibility is
//!    determined by `show_in_*_view` columns in node_types and edge_types tables.
//!
//! 2. **Configurable edge routing** - edge_types table defines `tier_delta`,
//!    `is_hierarchical`, `layout_direction`, `bundle_group` for layout hints.
//!
//! 3. **View mode configuration** - view_modes table defines which layers are included,
//!    hierarchy edge types, overlay edge types, and root identification rules.
//!
//! 4. **Layout config from DB** - layout parameters like node_separation, tier_height,
//!    animation settings come from layout_config table.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use uuid::Uuid;

use crate::database::{
    EdgeTypeConfig, NodeTypeConfig, ViewConfigService, ViewModeConfig, VisualizationRepository,
};
use crate::graph::types::{
    CbuGraph, EdgeType, GraphEdge, LayerType, LegacyGraphNode, NodeStatus, NodeType,
};

// Re-export as GraphNode for this module
type GraphNode = LegacyGraphNode;

/// Builder for constructing CbuGraph using database-driven configuration
///
/// This builder queries the configuration tables (node_types, edge_types, view_modes)
/// to determine what to include in the graph, rather than using hardcoded logic.
pub struct ConfigDrivenGraphBuilder {
    cbu_id: Uuid,
    /// Stored for debugging/logging (derived sets are used for actual logic)
    _view_mode: String,
    /// Loaded node type configurations
    node_type_configs: HashMap<String, NodeTypeConfig>,
    /// Loaded edge type configurations
    edge_type_configs: HashMap<String, EdgeTypeConfig>,
    /// Stored for potential future use (e.g., layout parameters)
    _view_mode_config: Option<ViewModeConfig>,
    /// Set of node type codes visible in this view
    visible_node_types: HashSet<String>,
    /// Set of edge type codes visible in this view
    visible_edge_types: HashSet<String>,
    /// Hierarchy edge types for layout
    hierarchy_edge_types: HashSet<String>,
    /// Overlay edge types for layout
    overlay_edge_types: HashSet<String>,
}

impl ConfigDrivenGraphBuilder {
    /// Create a new builder for the given CBU and view mode
    ///
    /// The view_mode should match a code in the view_modes table:
    /// - KYC_UBO
    /// - UBO_ONLY
    /// - TRADING
    /// - SERVICE_DELIVERY
    /// - CUSTODY
    /// - FUND_STRUCTURE
    /// - PRODUCTS_ONLY
    /// - COMBINED
    pub async fn new(pool: &sqlx::PgPool, cbu_id: Uuid, view_mode: &str) -> Result<Self> {
        // Load all node type configs
        let node_types = ViewConfigService::get_all_node_types(pool).await?;
        let node_type_configs: HashMap<String, NodeTypeConfig> = node_types
            .into_iter()
            .map(|nt| (nt.node_type_code.clone(), nt))
            .collect();

        // Load all edge type configs
        let edge_types = ViewConfigService::get_all_edge_types(pool).await?;
        let edge_type_configs: HashMap<String, EdgeTypeConfig> = edge_types
            .into_iter()
            .map(|et| (et.edge_type_code.clone(), et))
            .collect();

        // Load view mode config
        let view_mode_config = ViewConfigService::get_view_mode_config(pool, view_mode).await?;

        // Compute visible node types for this view
        let visible_node_types: HashSet<String> =
            ViewConfigService::get_view_node_types(pool, view_mode)
                .await?
                .into_iter()
                .map(|nt| nt.node_type_code)
                .collect();

        // Compute visible edge types for this view
        let visible_edge_types: HashSet<String> =
            ViewConfigService::get_view_edge_types(pool, view_mode)
                .await?
                .into_iter()
                .map(|et| et.edge_type_code)
                .collect();

        // Load hierarchy and overlay edge types
        let hierarchy_edge_types: HashSet<String> =
            ViewConfigService::get_hierarchy_edge_types(pool, view_mode)
                .await?
                .into_iter()
                .collect();

        let overlay_edge_types: HashSet<String> =
            ViewConfigService::get_overlay_edge_types(pool, view_mode)
                .await?
                .into_iter()
                .collect();

        Ok(Self {
            cbu_id,
            _view_mode: view_mode.to_string(),
            node_type_configs,
            edge_type_configs,
            _view_mode_config: view_mode_config,
            visible_node_types,
            visible_edge_types,
            hierarchy_edge_types,
            overlay_edge_types,
        })
    }

    /// Check if a node type is visible in the current view mode
    pub fn is_node_type_visible(&self, type_code: &str) -> bool {
        self.visible_node_types.contains(type_code)
    }

    /// Check if an edge type is visible in the current view mode
    pub fn is_edge_type_visible(&self, type_code: &str) -> bool {
        self.visible_edge_types.contains(type_code)
    }

    /// Get rendering hints for a node type
    pub fn get_node_rendering_hints(&self, type_code: &str) -> Option<NodeRenderingHints> {
        self.node_type_configs.get(type_code).map(|config| {
            use bigdecimal::ToPrimitive;
            NodeRenderingHints {
                icon: config.icon.clone(),
                default_color: config.default_color.clone(),
                default_shape: config.default_shape.clone(),
                default_width: config.default_width.as_ref().and_then(|d| d.to_f32()),
                default_height: config.default_height.as_ref().and_then(|d| d.to_f32()),
                z_order: config.z_order,
            }
        })
    }

    /// Get layout hints for an edge type
    pub fn get_edge_layout_hints(&self, type_code: &str) -> Option<EdgeLayoutHints> {
        self.edge_type_configs
            .get(type_code)
            .map(|config| EdgeLayoutHints {
                tier_delta: config.tier_delta,
                is_hierarchical: config.is_hierarchical,
                layout_direction: config.layout_direction.clone(),
                bundle_group: config.bundle_group.clone(),
                routing_priority: config.routing_priority,
            })
    }

    /// Check if an edge type is hierarchical (affects layout)
    pub fn is_hierarchy_edge(&self, type_code: &str) -> bool {
        self.hierarchy_edge_types.contains(type_code)
    }

    /// Check if an edge type is an overlay (rendered on top of hierarchy)
    pub fn is_overlay_edge(&self, type_code: &str) -> bool {
        self.overlay_edge_types.contains(type_code)
    }

    /// Get the layers to include based on view mode config
    ///
    /// Infers layers from the hierarchy_edge_types and overlay_edge_types in the view mode config.
    /// Falls back to ["core"] if no config is available.
    pub fn get_included_layers(&self) -> Vec<String> {
        // Since view_modes doesn't have an included_layers column,
        // we infer layers from the edge types configured for this view mode.
        // The default includes "core" and any layers implied by visible edge types.
        let mut layers = vec!["core".to_string()];

        // If we have hierarchy or overlay edge types configured, include additional layers
        // Edge type codes are SCREAMING_SNAKE_CASE from database (ob-poc.edge_types)
        if !self.hierarchy_edge_types.is_empty() || !self.overlay_edge_types.is_empty() {
            // Check for custody-related edges
            if self.visible_edge_types.contains("SERVICE_USES_RESOURCE") {
                layers.push("custody".to_string());
            }

            // Check for UBO-related edges
            if self.visible_edge_types.contains("OWNERSHIP")
                || self.visible_edge_types.contains("INDIRECT_OWNERSHIP")
                || self.visible_edge_types.contains("CONTROL")
            {
                layers.push("ubo".to_string());
            }

            // Check for KYC-related edges (currently mapped to SERVICE_USES_RESOURCE)
            // TODO: Add dedicated KYC edge types to database if needed

            // Check for services-related edges
            if self.visible_edge_types.contains("PRODUCT_PROVIDES_SERVICE") {
                layers.push("services".to_string());
            }

            // Check for trading-related edges
            if self.visible_edge_types.contains("CBU_HAS_TRADING_PROFILE")
                || self
                    .visible_edge_types
                    .contains("TRADING_PROFILE_HAS_MATRIX")
                || self.visible_edge_types.contains("MATRIX_INCLUDES_CLASS")
                || self.visible_edge_types.contains("CLASS_TRADED_ON_MARKET")
                || self.visible_edge_types.contains("OTC_WITH_COUNTERPARTY")
                || self.visible_edge_types.contains("OTC_COVERED_BY_ISDA")
                || self.visible_edge_types.contains("ISDA_HAS_CSA")
                || self.visible_edge_types.contains("CBU_IM_MANDATE")
                || self
                    .visible_edge_types
                    .contains("ENTITY_AUTHORIZES_TRADING")
            {
                layers.push("trading".to_string());
            }

            // Check for fund structure edges
            if self.visible_edge_types.contains("FUND_MANAGED_BY")
                || self
                    .visible_edge_types
                    .contains("UMBRELLA_CONTAINS_SUBFUND")
                || self.visible_edge_types.contains("FEEDER_TO_MASTER")
            {
                layers.push("fund_structure".to_string());
            }
        }

        layers
    }

    /// Build the graph from database via VisualizationRepository
    ///
    /// This method loads data and applies config-driven filtering.
    pub async fn build(self, repo: &VisualizationRepository) -> Result<CbuGraph> {
        // Load CBU base record
        let cbu_record = repo
            .get_cbu_basic(self.cbu_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", self.cbu_id))?;

        let mut graph = CbuGraph::with_metadata(
            self.cbu_id,
            cbu_record.name.clone(),
            cbu_record.cbu_category.clone(),
            cbu_record.jurisdiction.clone(),
        );

        // Add CBU root node (always included)
        graph.add_node(self.create_cbu_node(&cbu_record));

        // Get included layers from view mode config
        let included_layers = self.get_included_layers();

        // Load data based on included layers
        if included_layers.contains(&"core".to_string()) {
            self.load_entities(&mut graph, repo).await?;
        }

        if included_layers.contains(&"custody".to_string()) {
            self.load_custody_layer(&mut graph, repo).await?;
        }

        if included_layers.contains(&"kyc".to_string()) {
            self.load_kyc_layer(&mut graph, repo).await?;
        }

        if included_layers.contains(&"ubo".to_string()) {
            self.load_ubo_layer(&mut graph, repo).await?;
        }

        if included_layers.contains(&"services".to_string()) {
            self.load_services_layer(&mut graph, repo).await?;
        }

        if included_layers.contains(&"trading".to_string()) {
            self.load_trading_layer(&mut graph, repo).await?;
        }

        if included_layers.contains(&"fund_structure".to_string()) {
            self.load_fund_structure_layer(&mut graph, repo).await?;
        }

        // Filter nodes and edges by visibility
        self.apply_visibility_filter(&mut graph);

        // Compute final stats and visual hints
        graph.compute_stats();
        graph.compute_visual_hints();
        graph.build_layer_info();

        // Apply config-driven rendering hints
        self.apply_rendering_hints(&mut graph);

        Ok(graph)
    }

    /// Create the CBU root node
    fn create_cbu_node(&self, cbu_record: &crate::database::CbuBasicView) -> GraphNode {
        let hints = self.get_node_rendering_hints("cbu");

        GraphNode {
            id: self.cbu_id.to_string(),
            node_type: NodeType::Cbu,
            layer: LayerType::Core,
            label: cbu_record.name.clone(),
            sublabel: Some(format!(
                "{} / {}",
                cbu_record.jurisdiction.as_deref().unwrap_or("N/A"),
                cbu_record.client_type.as_deref().unwrap_or("N/A")
            )),
            status: NodeStatus::Active,
            parent_id: None,
            roles: Vec::new(),
            role_categories: Vec::new(),
            primary_role: None,
            jurisdiction: cbu_record.jurisdiction.clone(),
            role_priority: None,
            // Apply rendering hints from config
            width: hints.as_ref().and_then(|h| h.default_width),
            height: hints.as_ref().and_then(|h| h.default_height),
            // CBU is a container for entities
            is_container: true,
            contains_type: Some("entity".to_string()),
            data: serde_json::json!({
                "jurisdiction": cbu_record.jurisdiction,
                "client_type": cbu_record.client_type,
                "cbu_category": cbu_record.cbu_category
            }),
            ..Default::default()
        }
    }

    /// Load entities linked to the CBU via cbu_entity_roles
    async fn load_entities(
        &self,
        graph: &mut CbuGraph,
        repo: &VisualizationRepository,
    ) -> Result<()> {
        let entities = repo.get_graph_entities(self.cbu_id).await?;

        for ent in entities {
            let entity_id = ent.entity_id.to_string();

            // Get node type code (map entity_type to node_type code)
            let node_type_code = self.map_entity_type_to_node_type(&ent.entity_type);

            // Skip if node type is not visible in this view
            if !self.is_node_type_visible(&node_type_code) {
                continue;
            }

            // Get rendering hints
            let hints = self.get_node_rendering_hints(&node_type_code);

            // Compute layout behavior from primary role category
            let layout_behavior = ent
                .primary_role_category
                .as_ref()
                .and_then(|cat| cat.parse::<crate::graph::RoleCategory>().ok())
                .map(|cat| format!("{:?}", cat.layout_behavior()).to_lowercase());

            if !graph.has_node(&entity_id) {
                graph.add_node(GraphNode {
                    id: entity_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Core,
                    label: ent.entity_name,
                    sublabel: Some(ent.entity_type),
                    status: NodeStatus::Active,
                    parent_id: None,
                    roles: ent.roles.clone(),
                    role_categories: ent.role_categories.clone(),
                    primary_role: ent.primary_role.clone(),
                    jurisdiction: ent.jurisdiction.clone(),
                    role_priority: ent.role_priority,
                    entity_category: ent.entity_category.clone(),
                    primary_role_category: ent.primary_role_category.clone(),
                    layout_behavior,
                    ubo_treatment: ent.ubo_treatment.clone(),
                    kyc_obligation: ent.kyc_obligation.clone(),
                    person_state: ent.person_state.clone(),
                    width: hints.as_ref().and_then(|h| h.default_width),
                    height: hints.as_ref().and_then(|h| h.default_height),
                    // Entity belongs inside the CBU container
                    container_parent_id: Some(self.cbu_id.to_string()),
                    data: serde_json::json!({
                        "entity_id": ent.entity_id,
                        "entity_category": ent.entity_category,
                        "roles": ent.roles,
                        "role_categories": ent.role_categories,
                        "primary_role": ent.primary_role,
                        "jurisdiction": ent.jurisdiction,
                        "role_priority": ent.role_priority,
                        "primary_role_category": ent.primary_role_category,
                        "ubo_treatment": ent.ubo_treatment,
                        "kyc_obligation": ent.kyc_obligation,
                        "person_state": ent.person_state
                    }),
                    ..Default::default()
                });
            }

            // Add one edge per role (if CBU_ROLE edge type is visible)
            if self.is_edge_type_visible("CBU_ROLE") {
                for role in &ent.roles {
                    graph.add_edge(GraphEdge {
                        id: format!("{}->{}:{}", self.cbu_id, entity_id, role),
                        source: self.cbu_id.to_string(),
                        target: entity_id.clone(),
                        edge_type: EdgeType::HasRole,
                        label: Some(role.clone()),
                    });
                }
            }
        }

        Ok(())
    }

    /// Load custody layer data
    async fn load_custody_layer(
        &self,
        graph: &mut CbuGraph,
        repo: &VisualizationRepository,
    ) -> Result<()> {
        // Track which markets we've created nodes for
        let mut market_nodes: std::collections::HashMap<Uuid, String> =
            std::collections::HashMap::new();

        // Load universe entries
        if self.is_node_type_visible("universe") {
            let universes = repo.get_universes(self.cbu_id).await?;

            for u in universes {
                let universe_id = u.universe_id.to_string();

                // Create market grouping node if visible and not already created
                let market_node_id = if self.is_node_type_visible("market") {
                    if let Some(market_id) = u.market_id {
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            market_nodes.entry(market_id)
                        {
                            let market_node_id = format!("market-{}", market_id);
                            let mic = u.mic.clone().unwrap_or_else(|| "N/A".to_string());
                            let market_name = u.market_name.clone().unwrap_or_else(|| mic.clone());

                            graph.add_node(GraphNode {
                                id: market_node_id.clone(),
                                node_type: NodeType::Market,
                                layer: LayerType::Custody,
                                label: mic.clone(),
                                sublabel: Some(market_name),
                                status: NodeStatus::Active,
                                data: serde_json::json!({ "market_id": market_id }),
                                ..Default::default()
                            });

                            // Edge: CBU → Market
                            if self.is_edge_type_visible("matches") {
                                graph.add_edge(GraphEdge {
                                    id: format!("cbu->{}", market_node_id),
                                    source: self.cbu_id.to_string(),
                                    target: market_node_id.clone(),
                                    edge_type: EdgeType::Matches,
                                    label: None,
                                });
                            }

                            e.insert(market_node_id.clone());
                        }
                        market_nodes.get(&market_id).cloned()
                    } else {
                        None
                    }
                } else {
                    None
                };

                let class_name = u
                    .class_name
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());
                let sublabel = if u.currencies.is_empty() {
                    None
                } else {
                    Some(u.currencies.join(", "))
                };

                graph.add_node(GraphNode {
                    id: universe_id.clone(),
                    node_type: NodeType::Universe,
                    layer: LayerType::Custody,
                    label: class_name,
                    sublabel,
                    status: if u.is_active.unwrap_or(true) {
                        NodeStatus::Active
                    } else {
                        NodeStatus::Suspended
                    },
                    parent_id: market_node_id.clone(),
                    data: serde_json::json!({
                        "instrument_class_id": u.instrument_class_id,
                        "class_name": u.class_name,
                        "market_id": u.market_id,
                        "market_name": u.market_name,
                        "currencies": u.currencies,
                        "settlement_types": u.settlement_types
                    }),
                    ..Default::default()
                });

                // Add edge based on parent
                if self.is_edge_type_visible("matches") {
                    if let Some(ref parent_id) = market_node_id {
                        graph.add_edge(GraphEdge {
                            id: format!("{}->{}", parent_id, universe_id),
                            source: parent_id.clone(),
                            target: universe_id,
                            edge_type: EdgeType::Matches,
                            label: None,
                        });
                    } else {
                        graph.add_edge(GraphEdge {
                            id: format!("cbu->{}", universe_id),
                            source: self.cbu_id.to_string(),
                            target: universe_id,
                            edge_type: EdgeType::Matches,
                            label: None,
                        });
                    }
                }
            }
        }

        // Load SSIs
        if self.is_node_type_visible("ssi") {
            let ssis = repo.get_ssis(self.cbu_id).await?;

            for ssi in ssis {
                let ssi_id = ssi.ssi_id.to_string();

                graph.add_node(GraphNode {
                    id: ssi_id.clone(),
                    node_type: NodeType::Ssi,
                    layer: LayerType::Custody,
                    label: ssi.ssi_name,
                    sublabel: Some(format!(
                        "{} @ {}",
                        ssi.cash_currency.as_deref().unwrap_or("N/A"),
                        ssi.safekeeping_bic.as_deref().unwrap_or("N/A")
                    )),
                    status: match ssi.status.as_deref().unwrap_or("DRAFT") {
                        "ACTIVE" => NodeStatus::Active,
                        "PENDING" => NodeStatus::Pending,
                        "SUSPENDED" => NodeStatus::Suspended,
                        _ => NodeStatus::Draft,
                    },
                    data: serde_json::json!({
                        "ssi_type": ssi.ssi_type,
                        "cash_currency": ssi.cash_currency,
                        "safekeeping_account": ssi.safekeeping_account,
                        "safekeeping_bic": ssi.safekeeping_bic
                    }),
                    ..Default::default()
                });

                // Edge: CBU → SSI
                if self.is_edge_type_visible("settles_at") {
                    graph.add_edge(GraphEdge {
                        id: format!("cbu->{}", ssi_id),
                        source: self.cbu_id.to_string(),
                        target: ssi_id,
                        edge_type: EdgeType::SettlesAt,
                        label: None,
                    });
                }
            }
        }

        // Load booking rules
        if self.is_node_type_visible("booking_rule") {
            let rules = repo.get_booking_rules(self.cbu_id).await?;

            for rule in rules {
                let rule_id = rule.rule_id.to_string();

                graph.add_node(GraphNode {
                    id: rule_id.clone(),
                    node_type: NodeType::BookingRule,
                    layer: LayerType::Custody,
                    label: rule.rule_name,
                    sublabel: Some(format!("Priority {}", rule.priority)),
                    status: if rule.is_active.unwrap_or(true) {
                        NodeStatus::Active
                    } else {
                        NodeStatus::Suspended
                    },
                    data: serde_json::json!({
                        "priority": rule.priority,
                        "instrument_class_id": rule.instrument_class_id,
                        "class_name": rule.class_name
                    }),
                    ..Default::default()
                });

                // Edge: Rule → SSI (routes to)
                if self.is_edge_type_visible("routes_to") {
                    graph.add_edge(GraphEdge {
                        id: format!("{}->{}", rule_id, rule.ssi_id),
                        source: rule_id,
                        target: rule.ssi_id.to_string(),
                        edge_type: EdgeType::RoutesTo,
                        label: None,
                    });
                }
            }
        }

        // Load ISDA agreements
        if self.is_node_type_visible("isda") {
            let isdas = repo.get_isdas(self.cbu_id).await?;

            for isda in isdas {
                let isda_id = isda.isda_id.to_string();
                let counterparty = isda
                    .counterparty_name
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());

                graph.add_node(GraphNode {
                    id: isda_id.clone(),
                    node_type: NodeType::Isda,
                    layer: LayerType::Custody,
                    label: format!("ISDA - {}", counterparty),
                    sublabel: isda.governing_law.as_ref().map(|g| format!("{} law", g)),
                    status: if isda.is_active.unwrap_or(true) {
                        NodeStatus::Active
                    } else {
                        NodeStatus::Suspended
                    },
                    data: serde_json::json!({
                        "counterparty_entity_id": isda.counterparty_entity_id,
                        "counterparty_name": isda.counterparty_name,
                        "governing_law": isda.governing_law
                    }),
                    ..Default::default()
                });

                // Edge: CBU → ISDA
                if self.is_edge_type_visible("covered_by") {
                    graph.add_edge(GraphEdge {
                        id: format!("cbu->{}", isda_id),
                        source: self.cbu_id.to_string(),
                        target: isda_id.clone(),
                        edge_type: EdgeType::CoveredBy,
                        label: None,
                    });
                }

                // Load CSAs for this ISDA
                if self.is_node_type_visible("csa") {
                    let csas = repo.get_csas(isda.isda_id).await?;

                    for csa in csas {
                        let csa_id = csa.csa_id.to_string();

                        graph.add_node(GraphNode {
                            id: csa_id.clone(),
                            node_type: NodeType::Csa,
                            layer: LayerType::Custody,
                            label: format!("CSA ({})", csa.csa_type),
                            sublabel: None,
                            status: if csa.is_active.unwrap_or(true) {
                                NodeStatus::Active
                            } else {
                                NodeStatus::Suspended
                            },
                            data: serde_json::json!({ "csa_type": csa.csa_type }),
                            ..Default::default()
                        });

                        // Edge: ISDA → CSA
                        if self.is_edge_type_visible("secured_by") {
                            graph.add_edge(GraphEdge {
                                id: format!("{}->{}", isda_id, csa_id),
                                source: isda_id.clone(),
                                target: csa_id,
                                edge_type: EdgeType::SecuredBy,
                                label: None,
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Load KYC layer data
    async fn load_kyc_layer(
        &self,
        graph: &mut CbuGraph,
        repo: &VisualizationRepository,
    ) -> Result<()> {
        // Load entity KYC statuses
        if self.is_node_type_visible("verification") {
            let statuses = repo.get_kyc_statuses(self.cbu_id).await?;

            let mut entity_kyc_completions: HashMap<String, i32> = HashMap::new();

            for ks in statuses {
                let status_id = format!("kyc-status-{}", ks.status_id);
                let entity_id_str = ks.entity_id.to_string();

                let completion = match ks.kyc_status.as_deref() {
                    Some("APPROVED") => 100,
                    Some("PENDING_REVIEW") => 80,
                    Some("IN_PROGRESS") => 50,
                    Some("NOT_STARTED") => 0,
                    Some("REJECTED") | Some("EXPIRED") => 100,
                    _ => 0,
                };
                entity_kyc_completions.insert(entity_id_str.clone(), completion);

                graph.add_node(GraphNode {
                    id: status_id.clone(),
                    node_type: NodeType::Verification,
                    layer: LayerType::Kyc,
                    label: format!("KYC: {}", ks.kyc_status.as_deref().unwrap_or("N/A")),
                    sublabel: ks.risk_rating.clone(),
                    status: match ks.kyc_status.as_deref() {
                        Some("APPROVED") => NodeStatus::Active,
                        Some("IN_PROGRESS") | Some("PENDING_REVIEW") => NodeStatus::Pending,
                        Some("REJECTED") | Some("EXPIRED") => NodeStatus::Expired,
                        _ => NodeStatus::Draft,
                    },
                    data: serde_json::json!({
                        "kyc_status": ks.kyc_status,
                        "risk_rating": ks.risk_rating,
                        "next_review_date": ks.next_review_date
                    }),
                    ..Default::default()
                });

                // Edge: Entity → KYC Status
                if self.is_edge_type_visible("validates") {
                    graph.add_edge(GraphEdge {
                        id: format!("{}->{}", ks.entity_id, status_id),
                        source: ks.entity_id.to_string(),
                        target: status_id,
                        edge_type: EdgeType::Validates,
                        label: None,
                    });
                }
            }

            // Update entity nodes with KYC completion
            for node in &mut graph.nodes {
                if node.node_type == NodeType::Entity {
                    if let Some(&completion) = entity_kyc_completions.get(&node.id) {
                        node.kyc_completion = Some(completion);
                    }
                }
            }
        }

        // Load document requirements
        if self.is_node_type_visible("document") {
            let doc_reqs = repo.get_document_requests(self.cbu_id).await?;

            for dr in doc_reqs {
                let doc_req_id = format!("doc-req-{}", dr.request_id);

                graph.add_node(GraphNode {
                    id: doc_req_id.clone(),
                    node_type: NodeType::Document,
                    layer: LayerType::Kyc,
                    label: dr.document_type.clone(),
                    sublabel: dr.status.clone(),
                    status: match dr.status.as_deref() {
                        Some("RECEIVED") | Some("VERIFIED") => NodeStatus::Active,
                        Some("PENDING") => NodeStatus::Pending,
                        Some("REJECTED") => NodeStatus::Expired,
                        _ => NodeStatus::Draft,
                    },
                    data: serde_json::json!({
                        "document_type": dr.document_type,
                        "status": dr.status
                    }),
                    ..Default::default()
                });

                // Edge: Entity → Document Requirement
                if self.is_edge_type_visible("requires") {
                    if let Some(entity_id) = dr.requested_from_entity_id {
                        graph.add_edge(GraphEdge {
                            id: format!("{}->{}", entity_id, doc_req_id),
                            source: entity_id.to_string(),
                            target: doc_req_id,
                            edge_type: EdgeType::Requires,
                            label: None,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Load UBO layer data
    async fn load_ubo_layer(
        &self,
        graph: &mut CbuGraph,
        repo: &VisualizationRepository,
    ) -> Result<()> {
        let edges = repo.get_ubo_edges(self.cbu_id).await?;

        for edge in &edges {
            let from_id = edge.from_entity_id.to_string();
            let to_id = edge.to_entity_id.to_string();

            // Add from_entity node if not present
            if !graph.has_node(&from_id) {
                let sublabel = match edge.edge_type.as_str() {
                    "ownership" => edge.from_category.clone(),
                    "control" => edge.control_role.clone().or(edge.from_category.clone()),
                    "trust_role" => edge.trust_role.clone().or(edge.from_category.clone()),
                    _ => edge.from_category.clone(),
                };

                graph.add_node(GraphNode {
                    id: from_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Ubo,
                    label: edge.from_name.clone(),
                    sublabel,
                    status: match edge.status.as_str() {
                        "proven" => NodeStatus::Active,
                        "pending" => NodeStatus::Pending,
                        "disputed" => NodeStatus::Expired,
                        _ => NodeStatus::Draft,
                    },
                    entity_category: edge.from_category.clone(),
                    verification_status: Some(edge.status.clone()),
                    data: serde_json::json!({
                        "entity_type": edge.from_type_code,
                        "entity_category": edge.from_category
                    }),
                    ..Default::default()
                });
            }

            // Add to_entity node if not present
            if !graph.has_node(&to_id) {
                graph.add_node(GraphNode {
                    id: to_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Ubo,
                    label: edge.to_name.clone(),
                    sublabel: edge.to_category.clone(),
                    status: NodeStatus::Active,
                    entity_category: edge.to_category.clone(),
                    data: serde_json::json!({
                        "entity_type": edge.to_type_code,
                        "entity_category": edge.to_category
                    }),
                    ..Default::default()
                });
            }

            // Determine edge type and check visibility
            // edge_type_code uses database SCREAMING_SNAKE_CASE codes
            let (graph_edge_type, edge_type_code, label) = match edge.edge_type.as_str() {
                "ownership" => {
                    let pct = edge
                        .proven_percentage
                        .as_ref()
                        .or(edge.percentage.as_ref())
                        .or(edge.alleged_percentage.as_ref());
                    let label = pct.map(|p| format!("{}%", p));
                    (EdgeType::Owns, "OWNERSHIP", label)
                }
                "control" => {
                    let label = edge.control_role.clone();
                    (EdgeType::Controls, "CONTROL", label)
                }
                "trust_role" => {
                    let label = edge.trust_role.clone();
                    // Map specific trust roles to their database codes
                    let code = match edge.trust_role.as_deref() {
                        Some("settlor") => "TRUST_SETTLOR",
                        Some("trustee") => "TRUST_TRUSTEE",
                        Some("beneficiary") => "TRUST_BENEFICIARY",
                        Some("protector") => "TRUST_PROTECTOR",
                        _ => "CONTROL",
                    };
                    (EdgeType::Controls, code, label)
                }
                _ => (EdgeType::Owns, "OWNERSHIP", None),
            };

            // Only add edge if visible
            if self.is_edge_type_visible(edge_type_code) {
                let status_label = if edge.status != "proven" {
                    match &label {
                        Some(l) => Some(format!("{} ({})", l, edge.status)),
                        None => Some(format!("({})", edge.status)),
                    }
                } else {
                    label
                };

                graph.add_edge(GraphEdge {
                    id: edge.edge_id.to_string(),
                    source: from_id,
                    target: to_id,
                    edge_type: graph_edge_type,
                    label: status_label,
                });
            }
        }

        Ok(())
    }

    /// Load services layer data
    async fn load_services_layer(
        &self,
        graph: &mut CbuGraph,
        repo: &VisualizationRepository,
    ) -> Result<()> {
        // Load products
        if self.is_node_type_visible("product") {
            let products = repo.get_cbu_products(self.cbu_id).await?;

            for product in &products {
                let product_node_id = format!("product-{}", product.product_id);

                graph.add_node(GraphNode {
                    id: product_node_id.clone(),
                    node_type: NodeType::Product,
                    layer: LayerType::Services,
                    label: product.name.clone(),
                    sublabel: product.product_category.clone(),
                    status: if product.is_active.unwrap_or(true) {
                        NodeStatus::Active
                    } else {
                        NodeStatus::Suspended
                    },
                    data: serde_json::json!({
                        "product_id": product.product_id,
                        "product_code": product.product_code
                    }),
                    ..Default::default()
                });

                // Edge: CBU → Product
                if self.is_edge_type_visible("delivers") {
                    graph.add_edge(GraphEdge {
                        id: format!("cbu->{}", product_node_id),
                        source: self.cbu_id.to_string(),
                        target: product_node_id.clone(),
                        edge_type: EdgeType::Delivers,
                        label: None,
                    });
                }

                // Load services for this product
                if self.is_node_type_visible("service") {
                    let services = repo.get_product_services(product.product_id).await?;

                    for service in &services {
                        let service_node_id = format!("service-{}", service.service_id);

                        if !graph.has_node(&service_node_id) {
                            graph.add_node(GraphNode {
                                id: service_node_id.clone(),
                                node_type: NodeType::Service,
                                layer: LayerType::Services,
                                label: service.name.clone(),
                                sublabel: service.service_category.clone(),
                                status: NodeStatus::Active,
                                data: serde_json::json!({
                                    "service_id": service.service_id,
                                    "service_code": service.service_code,
                                    "is_mandatory": service.is_mandatory
                                }),
                                ..Default::default()
                            });
                        }

                        // Edge: Product → Service
                        if self.is_edge_type_visible("delivers") {
                            graph.add_edge(GraphEdge {
                                id: format!("{}->{}", product_node_id, service_node_id),
                                source: product_node_id.clone(),
                                target: service_node_id.clone(),
                                edge_type: EdgeType::Delivers,
                                label: if service.is_mandatory.unwrap_or(false) {
                                    Some("mandatory".to_string())
                                } else {
                                    None
                                },
                            });
                        }

                        // Load resource types
                        if self.is_node_type_visible("resource") {
                            let resource_types =
                                repo.get_service_resource_types(service.service_id).await?;

                            for rt in &resource_types {
                                let rt_node_id = format!("resource-type-{}", rt.resource_id);

                                if !graph.has_node(&rt_node_id) {
                                    graph.add_node(GraphNode {
                                        id: rt_node_id.clone(),
                                        node_type: NodeType::Resource,
                                        layer: LayerType::Services,
                                        label: rt.name.clone(),
                                        sublabel: rt.resource_type.clone(),
                                        status: if rt.is_active.unwrap_or(true) {
                                            NodeStatus::Active
                                        } else {
                                            NodeStatus::Suspended
                                        },
                                        data: serde_json::json!({
                                            "resource_id": rt.resource_id,
                                            "resource_code": rt.resource_code
                                        }),
                                        ..Default::default()
                                    });
                                }

                                // Edge: Service → Resource Type
                                if self.is_edge_type_visible("delivers") {
                                    graph.add_edge(GraphEdge {
                                        id: format!("{}->{}", service_node_id, rt_node_id),
                                        source: service_node_id.clone(),
                                        target: rt_node_id,
                                        edge_type: EdgeType::Delivers,
                                        label: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Load trading layer data
    ///
    /// Structure:
    /// CBU → TradingProfile → InstrumentMatrix
    ///                           ├── InstrumentClass nodes
    ///                           │     ├── Market nodes (exchange traded)
    ///                           │     └── Counterparty nodes (OTC)
    ///                           │           └── ISDA → CSA
    ///                           └── IM links (entities doing the trading)
    async fn load_trading_layer(
        &self,
        graph: &mut CbuGraph,
        repo: &VisualizationRepository,
    ) -> Result<()> {
        // Note: Uses module-level type aliases (GraphNode = LegacyGraphNode)
        // EdgeType, NodeType, etc. are already imported at module level

        // 1. Load current trading profile (latest version regardless of status)
        let Some(profile) = repo.get_current_trading_profile(self.cbu_id).await? else {
            return Ok(()); // No trading profile = no trading layer
        };

        // 2. Add Trading Profile node
        let profile_node_id = format!("profile-{}", profile.profile_id);
        if self.is_node_type_visible("TRADING_PROFILE") {
            graph.add_node(GraphNode {
                id: profile_node_id.clone(),
                node_type: NodeType::TradingProfile,
                layer: LayerType::Trading,
                label: "Trading Profile".into(),
                sublabel: Some(format!("v{} - {}", profile.version, profile.status)),
                status: match profile.status.as_str() {
                    "ACTIVE" => NodeStatus::Active,
                    "DRAFT" => NodeStatus::Draft,
                    _ => NodeStatus::Suspended,
                },
                ..Default::default()
            });

            // Edge: CBU → Trading Profile
            if self.is_edge_type_visible("CBU_HAS_TRADING_PROFILE") {
                graph.add_edge(GraphEdge {
                    id: format!("cbu->profile-{}", profile.profile_id),
                    source: self.cbu_id.to_string(),
                    target: profile_node_id.clone(),
                    edge_type: EdgeType::HasTradingProfile,
                    label: None,
                });
            }
        }

        // 3. Add Instrument Matrix node (container)
        let matrix_node_id = format!("matrix-{}", profile.profile_id);
        if self.is_node_type_visible("INSTRUMENT_MATRIX") {
            graph.add_node(GraphNode {
                id: matrix_node_id.clone(),
                node_type: NodeType::InstrumentMatrix,
                layer: LayerType::Trading,
                label: "Instrument Matrix".into(),
                sublabel: None,
                status: NodeStatus::Active,
                ..Default::default()
            });

            // Edge: Trading Profile → Matrix
            if self.is_edge_type_visible("TRADING_PROFILE_HAS_MATRIX") {
                graph.add_edge(GraphEdge {
                    id: format!("{}->{}", profile_node_id, matrix_node_id),
                    source: profile_node_id.clone(),
                    target: matrix_node_id.clone(),
                    edge_type: EdgeType::HasMatrix,
                    label: None,
                });
            }
        }

        // 4. Load instrument universe and build class/market/counterparty nodes
        let universe = repo.get_cbu_instrument_universe(self.cbu_id).await?;

        // Group by instrument class
        let mut classes_added: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        let mut markets_added: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        let mut counterparties_added: std::collections::HashSet<Uuid> =
            std::collections::HashSet::new();

        for entry in &universe {
            // Add Instrument Class node (deduplicated)
            let class_node_id = format!("class-{}", entry.instrument_class_id);
            if !classes_added.contains(&entry.instrument_class_id) {
                if self.is_node_type_visible("INSTRUMENT_CLASS") {
                    graph.add_node(GraphNode {
                        id: class_node_id.clone(),
                        node_type: NodeType::InstrumentClass,
                        layer: LayerType::Trading,
                        label: entry.class_name.clone(),
                        sublabel: Some(entry.class_code.clone()),
                        status: NodeStatus::Active,
                        data: serde_json::json!({
                            "class_id": entry.instrument_class_id,
                            "class_code": entry.class_code,
                            "is_otc": entry.is_otc,
                        }),
                        ..Default::default()
                    });

                    // Edge: Matrix → Class
                    if self.is_edge_type_visible("MATRIX_INCLUDES_CLASS") {
                        graph.add_edge(GraphEdge {
                            id: format!("{}->{}", matrix_node_id, class_node_id),
                            source: matrix_node_id.clone(),
                            target: class_node_id.clone(),
                            edge_type: EdgeType::IncludesClass,
                            label: None,
                        });
                    }
                }
                classes_added.insert(entry.instrument_class_id);
            }

            // Add Market node for exchange-traded (deduplicated)
            if let Some(market_id) = entry.market_id {
                if !markets_added.contains(&market_id) {
                    if self.is_node_type_visible("MARKET") {
                        let market_node_id = format!("market-{}", market_id);
                        graph.add_node(GraphNode {
                            id: market_node_id.clone(),
                            node_type: NodeType::Market,
                            layer: LayerType::Trading,
                            label: entry
                                .market_name
                                .clone()
                                .unwrap_or_else(|| "Unknown".into()),
                            sublabel: entry.mic.clone(),
                            status: NodeStatus::Active,
                            data: serde_json::json!({
                                "market_id": market_id,
                                "mic": entry.mic,
                            }),
                            ..Default::default()
                        });

                        // Edge: Class → Market
                        if self.is_edge_type_visible("CLASS_TRADED_ON_MARKET") {
                            graph.add_edge(GraphEdge {
                                id: format!("{}->{}", class_node_id, market_node_id),
                                source: class_node_id.clone(),
                                target: market_node_id,
                                edge_type: EdgeType::TradedOn,
                                label: None,
                            });
                        }
                    }
                    markets_added.insert(market_id);
                }
            }

            // Add OTC Counterparty node (deduplicated)
            if let Some(counterparty_id) = entry.counterparty_id {
                if !counterparties_added.contains(&counterparty_id) {
                    let cp_node_id = format!("counterparty-{}", counterparty_id);
                    // Note: counterparty is an entity, may already exist in graph
                    if !graph.has_node(&cp_node_id) {
                        graph.add_node(GraphNode {
                            id: cp_node_id.clone(),
                            node_type: NodeType::Entity,
                            layer: LayerType::Trading,
                            label: entry
                                .counterparty_name
                                .clone()
                                .unwrap_or_else(|| "Unknown".into()),
                            sublabel: Some("OTC Counterparty".into()),
                            status: NodeStatus::Active,
                            data: serde_json::json!({
                                "entity_id": counterparty_id,
                                "role": "OTC_COUNTERPARTY",
                            }),
                            ..Default::default()
                        });
                    }

                    // Edge: Class → Counterparty (OTC trading relationship)
                    if self.is_edge_type_visible("OTC_WITH_COUNTERPARTY") {
                        graph.add_edge(GraphEdge {
                            id: format!("{}->{}", class_node_id, cp_node_id),
                            source: class_node_id.clone(),
                            target: cp_node_id,
                            edge_type: EdgeType::OtcCounterparty,
                            label: None,
                        });
                    }
                    counterparties_added.insert(counterparty_id);
                }
            }
        }

        // 5. Load ISDA agreements and link to counterparties
        let isdas = repo.get_cbu_isda_agreements(self.cbu_id).await?;
        for isda in isdas {
            let isda_node_id = format!("isda-{}", isda.isda_id);
            let cp_node_id = format!("counterparty-{}", isda.counterparty_entity_id);

            if self.is_node_type_visible("ISDA_AGREEMENT") {
                graph.add_node(GraphNode {
                    id: isda_node_id.clone(),
                    node_type: NodeType::IsdaAgreement,
                    layer: LayerType::Trading,
                    label: format!("ISDA {}", isda.governing_law.as_deref().unwrap_or("?")),
                    sublabel: isda.counterparty_name.clone(),
                    status: NodeStatus::Active,
                    data: serde_json::json!({
                        "isda_id": isda.isda_id,
                        "governing_law": isda.governing_law,
                        "agreement_date": isda.agreement_date,
                    }),
                    ..Default::default()
                });

                // Edge: Counterparty → ISDA
                if self.is_edge_type_visible("OTC_COVERED_BY_ISDA") {
                    graph.add_edge(GraphEdge {
                        id: format!("{}->{}", cp_node_id, isda_node_id),
                        source: cp_node_id,
                        target: isda_node_id.clone(),
                        edge_type: EdgeType::CoveredByIsda,
                        label: None,
                    });
                }
            }

            // 6. Load CSA under ISDA
            if let Some(csa) = repo.get_isda_csa(isda.isda_id).await? {
                let csa_node_id = format!("csa-{}", csa.csa_id);

                if self.is_node_type_visible("CSA_AGREEMENT") {
                    graph.add_node(GraphNode {
                        id: csa_node_id.clone(),
                        node_type: NodeType::CsaAgreement,
                        layer: LayerType::Trading,
                        label: format!("CSA ({})", csa.csa_type),
                        sublabel: None,
                        status: NodeStatus::Active,
                        data: serde_json::json!({
                            "csa_id": csa.csa_id,
                            "csa_type": csa.csa_type,
                            "threshold_amount": csa.threshold_amount,
                            "threshold_currency": csa.threshold_currency,
                        }),
                        ..Default::default()
                    });

                    // Edge: ISDA → CSA
                    if self.is_edge_type_visible("ISDA_HAS_CSA") {
                        graph.add_edge(GraphEdge {
                            id: format!("{}->{}", isda_node_id, csa_node_id),
                            source: isda_node_id,
                            target: csa_node_id,
                            edge_type: EdgeType::HasCsa,
                            label: None,
                        });
                    }
                }
            }
        }

        // 7. Load Investment Managers and link to CBU
        let ims = repo.get_cbu_investment_managers(self.cbu_id).await?;
        for im in ims {
            // IM is an entity - use raw UUID to match entity nodes from load_entities
            let im_entity_node_id = im.entity_id.to_string();

            // Edge: CBU → IM Entity with trading mandate
            if self.is_edge_type_visible("CBU_IM_MANDATE") {
                graph.add_edge(GraphEdge {
                    id: format!("cbu->im-{}", im.entity_id),
                    source: self.cbu_id.to_string(),
                    target: im_entity_node_id,
                    edge_type: EdgeType::ImMandate,
                    label: Some(format!("IM: {}", im.scope_description)),
                });
            }
        }

        Ok(())
    }

    /// Load fund structure layer data
    async fn load_fund_structure_layer(
        &self,
        graph: &mut CbuGraph,
        repo: &VisualizationRepository,
    ) -> Result<()> {
        let edges = repo.get_fund_structure_edges(self.cbu_id).await?;

        for edge in &edges {
            let child_id = edge.child_entity_id.to_string();

            let parent_id = match edge.parent_entity_id {
                Some(id) => id.to_string(),
                None => match &edge.parent_lei {
                    Some(lei) => format!("lei-{}", lei),
                    None => continue,
                },
            };

            // Add child node if not present
            if !graph.has_node(&child_id) {
                graph.add_node(GraphNode {
                    id: child_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Core,
                    label: edge.child_name.clone(),
                    sublabel: edge.child_type_code.clone(),
                    status: NodeStatus::Active,
                    data: serde_json::json!({
                        "entity_type": edge.child_type_code,
                        "source": "fund_structure"
                    }),
                    ..Default::default()
                });
            }

            // Add parent node if not present
            if !graph.has_node(&parent_id) {
                let parent_label = edge.parent_name.clone().unwrap_or_else(|| {
                    edge.parent_lei
                        .clone()
                        .unwrap_or_else(|| "Unknown".to_string())
                });

                graph.add_node(GraphNode {
                    id: parent_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Core,
                    label: parent_label,
                    sublabel: edge.parent_type_code.clone(),
                    status: if edge.relationship_status.as_deref() == Some("ACTIVE") {
                        NodeStatus::Active
                    } else {
                        NodeStatus::Pending
                    },
                    data: serde_json::json!({
                        "entity_type": edge.parent_type_code,
                        "lei": edge.parent_lei,
                        "source": edge.source
                    }),
                    ..Default::default()
                });
            }

            // Determine edge type and code
            let (edge_type, edge_type_code, label) = match edge.relationship_type.as_str() {
                "FUND_MANAGER" => (
                    EdgeType::ManagedBy,
                    "managed_by",
                    Some("managed by".to_string()),
                ),
                "UMBRELLA_FUND" => (
                    EdgeType::Contains,
                    "contains",
                    Some("subfund of".to_string()),
                ),
                "MASTER_FUND" => (
                    EdgeType::FeederTo,
                    "feeder_to",
                    Some("feeds into".to_string()),
                ),
                _ => (EdgeType::ManagedBy, "managed_by", None),
            };

            // Only add edge if visible
            if self.is_edge_type_visible(edge_type_code) {
                graph.add_edge(GraphEdge {
                    id: edge.relationship_id.to_string(),
                    source: child_id,
                    target: parent_id,
                    edge_type,
                    label,
                });
            }
        }

        Ok(())
    }

    /// Apply visibility filter to remove nodes/edges not visible in this view
    fn apply_visibility_filter(&self, graph: &mut CbuGraph) {
        // Filter edges first (nodes may become orphaned)
        graph.edges.retain(|edge| {
            let edge_type_code = self.edge_type_to_code(&edge.edge_type);
            self.is_edge_type_visible(&edge_type_code)
        });

        // Collect connected node IDs
        let connected_node_ids: HashSet<String> = graph
            .edges
            .iter()
            .flat_map(|e| [e.source.clone(), e.target.clone()])
            .collect();

        // Keep CBU and connected nodes
        graph.nodes.retain(|node| {
            node.node_type == NodeType::Cbu || connected_node_ids.contains(&node.id)
        });
    }

    /// Apply rendering hints from config to nodes
    fn apply_rendering_hints(&self, graph: &mut CbuGraph) {
        for node in &mut graph.nodes {
            let type_code = self.node_type_to_code(&node.node_type);
            if let Some(hints) = self.get_node_rendering_hints(&type_code) {
                // Apply default dimensions if not already set
                if node.width.is_none() {
                    node.width = hints.default_width;
                }
                if node.height.is_none() {
                    node.height = hints.default_height;
                }
            }
        }
    }

    /// Map entity_type string to node_type code
    ///
    /// Maps entity types from the entities table to node_type_code values
    /// in the node_types table for visibility filtering.
    fn map_entity_type_to_node_type(&self, entity_type: &str) -> String {
        match entity_type.to_uppercase().as_str() {
            // Natural persons
            "PROPER_PERSON" | "NATURAL_PERSON" | "PERSON" => "ENTITY_PERSON".to_string(),
            // Companies
            "LIMITED_COMPANY" | "COMPANY" | "CORPORATION" | "LLC" | "GMBH" | "SA" | "AG"
            | "PLC" | "LTD" | "INC" => "ENTITY_COMPANY".to_string(),
            // Funds
            "FUND" | "SICAV" | "ICAV" | "OEIC" | "VCC" | "UNIT_TRUST" | "FCP" | "UCITS" | "AIF"
            | "RAIF" => "ENTITY_FUND".to_string(),
            // Partnerships
            "PARTNERSHIP"
            | "LIMITED_PARTNERSHIP"
            | "LP"
            | "LLP"
            | "GP"
            | "SCSP"
            | "SCS"
            | "SCSSP" => "ENTITY_PARTNERSHIP".to_string(),
            // Trusts
            "TRUST" | "DISCRETIONARY_TRUST" | "FIXED_TRUST" | "UNIT_TRUST_STRUCTURE" => {
                "ENTITY_TRUST".to_string()
            }
            // Default to company for unknown types
            _ => "ENTITY_COMPANY".to_string(),
        }
    }

    /// Convert NodeType to type_code string
    fn node_type_to_code(&self, node_type: &NodeType) -> String {
        match node_type {
            NodeType::Cbu => "cbu".to_string(),
            NodeType::Entity => "entity".to_string(),
            NodeType::Market => "market".to_string(),
            NodeType::Universe => "universe".to_string(),
            NodeType::Ssi => "ssi".to_string(),
            NodeType::BookingRule => "booking_rule".to_string(),
            NodeType::Isda => "isda".to_string(),
            NodeType::Csa => "csa".to_string(),
            NodeType::Document => "document".to_string(),
            NodeType::Verification => "verification".to_string(),
            NodeType::Product => "product".to_string(),
            NodeType::Service => "service".to_string(),
            NodeType::Resource => "resource".to_string(),
            NodeType::ShareClass => "share_class".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// Convert EdgeType to database edge_type_code
    /// Uses the centralized EdgeType::to_code() method for consistency with database
    fn edge_type_to_code(&self, edge_type: &EdgeType) -> String {
        edge_type.to_code().to_string()
    }
}

// =============================================================================
// RENDERING HINTS
// =============================================================================

/// Rendering hints for a node type from config
#[derive(Debug, Clone)]
pub struct NodeRenderingHints {
    pub icon: Option<String>,
    pub default_color: Option<String>,
    pub default_shape: Option<String>,
    pub default_width: Option<f32>,
    pub default_height: Option<f32>,
    pub z_order: Option<i32>,
}

/// Layout hints for an edge type from config
#[derive(Debug, Clone)]
pub struct EdgeLayoutHints {
    pub tier_delta: Option<i32>,
    pub is_hierarchical: bool,
    pub layout_direction: Option<String>,
    pub bundle_group: Option<String>,
    pub routing_priority: Option<i32>,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_to_code() {
        let builder = ConfigDrivenGraphBuilder {
            cbu_id: Uuid::now_v7(),
            _view_mode: "KYC_UBO".to_string(),
            node_type_configs: HashMap::new(),
            edge_type_configs: HashMap::new(),
            _view_mode_config: None,
            visible_node_types: HashSet::new(),
            visible_edge_types: HashSet::new(),
            hierarchy_edge_types: HashSet::new(),
            overlay_edge_types: HashSet::new(),
        };

        assert_eq!(builder.node_type_to_code(&NodeType::Cbu), "cbu");
        assert_eq!(builder.node_type_to_code(&NodeType::Entity), "entity");
        assert_eq!(builder.node_type_to_code(&NodeType::Market), "market");
        assert_eq!(builder.node_type_to_code(&NodeType::Ssi), "ssi");
    }

    #[test]
    fn test_edge_type_to_code() {
        let builder = ConfigDrivenGraphBuilder {
            cbu_id: Uuid::now_v7(),
            _view_mode: "KYC_UBO".to_string(),
            node_type_configs: HashMap::new(),
            edge_type_configs: HashMap::new(),
            _view_mode_config: None,
            visible_node_types: HashSet::new(),
            visible_edge_types: HashSet::new(),
            hierarchy_edge_types: HashSet::new(),
            overlay_edge_types: HashSet::new(),
        };

        // Tests use database SCREAMING_SNAKE_CASE codes
        assert_eq!(builder.edge_type_to_code(&EdgeType::HasRole), "CBU_ROLE");
        assert_eq!(builder.edge_type_to_code(&EdgeType::Owns), "OWNERSHIP");
        assert_eq!(builder.edge_type_to_code(&EdgeType::Controls), "CONTROL");
        assert_eq!(
            builder.edge_type_to_code(&EdgeType::ManagedBy),
            "FUND_MANAGED_BY"
        );
    }

    #[test]
    fn test_visibility_checks() {
        let mut visible_nodes = HashSet::new();
        visible_nodes.insert("cbu".to_string());
        visible_nodes.insert("entity".to_string());

        // Use database SCREAMING_SNAKE_CASE codes
        let mut visible_edges = HashSet::new();
        visible_edges.insert("CBU_ROLE".to_string());
        visible_edges.insert("OWNERSHIP".to_string());

        let builder = ConfigDrivenGraphBuilder {
            cbu_id: Uuid::now_v7(),
            _view_mode: "KYC_UBO".to_string(),
            node_type_configs: HashMap::new(),
            edge_type_configs: HashMap::new(),
            _view_mode_config: None,
            visible_node_types: visible_nodes,
            visible_edge_types: visible_edges,
            hierarchy_edge_types: HashSet::new(),
            overlay_edge_types: HashSet::new(),
        };

        assert!(builder.is_node_type_visible("cbu"));
        assert!(builder.is_node_type_visible("entity"));
        assert!(!builder.is_node_type_visible("market"));

        assert!(builder.is_edge_type_visible("CBU_ROLE"));
        assert!(builder.is_edge_type_visible("OWNERSHIP"));
        assert!(!builder.is_edge_type_visible("CONTROL"));
    }
}
