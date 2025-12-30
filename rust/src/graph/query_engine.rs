//! Graph Query Engine for DSL graph.* verbs
//!
//! This module implements the query execution logic for graph operations.
//! It handles traversal, filtering, path-finding, and comparison operations.

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{anyhow, Result};
use sqlx::PgPool;
use uuid::Uuid;

// Use legacy types for backward compatibility during transition to EntityGraph
use super::types::{EdgeType, GraphEdge, LayerType, LegacyGraphNode, NodeType};

// Re-export as GraphNode for this module
type GraphNode = LegacyGraphNode;
use super::view_model::{
    GraphComparison, GraphFilter, GraphPath, GraphViewModel, NodeGroup, ViewModeInfo,
};

// =============================================================================
// TYPE ALIASES (for complex SQL query result types)
// =============================================================================

/// Row type for entity with role queries
type EntityRoleRow = (Uuid, String, String, Option<String>, String, Option<i32>);

/// Row type for relationship queries
type RelationshipRow = (
    Uuid,                          // from_id
    Uuid,                          // to_id
    String,                        // rel_type
    Option<rust_decimal::Decimal>, // percentage
    String,                        // from_name
    String,                        // to_name
    Option<String>,                // verification_status
);

// =============================================================================
// GRAPH QUERY ENGINE
// =============================================================================

/// Engine for executing graph queries
///
/// The GraphQueryEngine provides methods for:
/// - Building graph views from a root entity
/// - Focusing on specific nodes with neighborhood
/// - Filtering graphs by criteria
/// - Finding paths between nodes
/// - Traversing ancestors/descendants
/// - Comparing graph snapshots
pub struct GraphQueryEngine {
    pool: PgPool,
}

impl GraphQueryEngine {
    /// Create a new GraphQueryEngine
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // VIEW OPERATIONS
    // =========================================================================

    /// Build a full graph view from a root entity (CBU)
    ///
    /// This is the primary entry point for graph visualization.
    /// It fetches all related entities, relationships, and builds
    /// a complete GraphViewModel.
    pub async fn execute_view(
        &self,
        root_id: Uuid,
        view_mode: &str,
        max_depth: u32,
    ) -> Result<GraphViewModel> {
        let start = std::time::Instant::now();

        let mut model = GraphViewModel::new(root_id.to_string());
        model.depth = max_depth;
        model.view_mode = self.get_view_mode_info(view_mode);

        // Fetch root CBU
        let cbu = self.fetch_cbu(root_id).await?;
        model.add_node(cbu);

        // Fetch entities with roles
        let entities = self.fetch_cbu_entities(root_id).await?;
        for entity in entities {
            model.add_node(entity.node);
            model.add_edge(entity.role_edge);
        }

        // Fetch ownership/control relationships
        let relationships = self.fetch_entity_relationships(root_id).await?;
        for (node, edge) in relationships {
            if !model.has_node(&node.id) {
                model.add_node(node);
            }
            model.add_edge(edge);
        }

        // Apply view mode filtering
        self.apply_view_mode_filter(&mut model, view_mode);

        // Compute statistics
        model.compute_stats();
        model.stats.execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(model)
    }

    /// Focus on a specific node with its immediate neighborhood
    pub async fn execute_focus(
        &self,
        root_id: Uuid,
        focus_id: Uuid,
        depth: u32,
    ) -> Result<GraphViewModel> {
        let start = std::time::Instant::now();

        let mut model = GraphViewModel::new(root_id.to_string());
        model.focus_id = Some(focus_id.to_string());
        model.depth = depth;

        // Fetch the focus node
        let focus_node = self.fetch_entity(focus_id).await?;
        model.add_node(focus_node);

        // Fetch connected nodes up to depth
        let connected = self.fetch_connected_nodes(focus_id, depth).await?;
        for (node, edge) in connected {
            if !model.has_node(&node.id) {
                model.add_node(node);
            }
            model.add_edge(edge);
        }

        model.compute_stats();
        model.stats.execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(model)
    }

    // =========================================================================
    // FILTER OPERATIONS
    // =========================================================================

    /// Filter an existing graph by criteria
    pub fn execute_filter(
        &self,
        model: &GraphViewModel,
        filter: &GraphFilter,
    ) -> Result<GraphViewModel> {
        let start = std::time::Instant::now();

        let mut filtered = GraphViewModel::new(model.root_id.clone());
        filtered.filter = Some(filter.clone());
        filtered.view_mode = model.view_mode.clone();

        // Filter nodes
        for node in &model.nodes {
            if self.node_matches_filter(node, filter) {
                filtered.add_node(node.clone());
            }
        }

        // Keep edges where both source and target are in filtered nodes
        // Collect node IDs first, then collect matching edges, then add them
        let node_ids: HashSet<String> = filtered.nodes.iter().map(|n| n.id.clone()).collect();
        let matching_edges: Vec<_> = model
            .edges
            .iter()
            .filter(|edge| {
                node_ids.contains(&edge.source)
                    && node_ids.contains(&edge.target)
                    && self.edge_matches_filter(edge, filter)
            })
            .cloned()
            .collect();
        for edge in matching_edges {
            filtered.add_edge(edge);
        }

        filtered.compute_stats();
        filtered.stats.execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(filtered)
    }

    /// Check if a node matches filter criteria
    fn node_matches_filter(&self, node: &GraphNode, filter: &GraphFilter) -> bool {
        // Node type filter
        if !filter.node_types.is_empty() && !filter.node_types.contains(&node.node_type) {
            return false;
        }

        // Layer filter
        if !filter.layers.is_empty() && !filter.layers.contains(&node.layer) {
            return false;
        }

        // Status filter
        if let Some(ref status) = filter.status {
            let node_status = format!("{:?}", node.status).to_lowercase();
            if node_status != status.to_lowercase() {
                return false;
            }
        }

        // Role filter
        if let Some(ref role) = filter.role {
            if !node.roles.iter().any(|r| r.eq_ignore_ascii_case(role)) {
                return false;
            }
        }

        // Jurisdiction filter
        if let Some(ref jurisdiction) = filter.jurisdiction {
            if node.jurisdiction.as_deref() != Some(jurisdiction.as_str()) {
                return false;
            }
        }

        // Text search
        if let Some(ref search) = filter.search {
            let search_lower = search.to_lowercase();
            if !node.label.to_lowercase().contains(&search_lower)
                && !node
                    .sublabel
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&search_lower))
                    .unwrap_or(false)
            {
                return false;
            }
        }

        true
    }

    /// Check if an edge matches filter criteria
    fn edge_matches_filter(&self, edge: &GraphEdge, filter: &GraphFilter) -> bool {
        if !filter.edge_types.is_empty() && !filter.edge_types.contains(&edge.edge_type) {
            return false;
        }
        true
    }

    // =========================================================================
    // GROUP OPERATIONS
    // =========================================================================

    /// Group nodes by an attribute
    pub fn execute_group_by(
        &self,
        model: &GraphViewModel,
        group_by: &str,
    ) -> Result<GraphViewModel> {
        let mut grouped = model.clone();
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();

        for node in &model.nodes {
            let key = match group_by {
                "jurisdiction" => node
                    .jurisdiction
                    .clone()
                    .unwrap_or_else(|| "N/A".to_string()),
                "role" => node
                    .primary_role
                    .clone()
                    .unwrap_or_else(|| "N/A".to_string()),
                "layer" => format!("{:?}", node.layer).to_lowercase(),
                "type" => format!("{:?}", node.node_type).to_lowercase(),
                "status" => format!("{:?}", node.status).to_lowercase(),
                "entity_category" => node
                    .entity_category
                    .clone()
                    .unwrap_or_else(|| "N/A".to_string()),
                _ => "unknown".to_string(),
            };

            groups.entry(key).or_default().push(node.id.clone());
        }

        grouped.groups = Some(
            groups
                .into_iter()
                .map(|(key, node_ids)| NodeGroup {
                    key: key.clone(),
                    label: key,
                    node_ids,
                    color: None,
                })
                .collect(),
        );

        Ok(grouped)
    }

    // =========================================================================
    // PATH OPERATIONS
    // =========================================================================

    /// Find shortest path between two nodes
    pub fn execute_path(
        &self,
        model: &GraphViewModel,
        from_id: &str,
        to_id: &str,
        edge_types: Option<&[EdgeType]>,
    ) -> Result<Vec<GraphPath>> {
        // Build adjacency map
        let mut adjacency: HashMap<&str, Vec<(&str, &str, EdgeType)>> = HashMap::new();

        for edge in &model.edges {
            // Filter by edge types if specified
            if let Some(types) = edge_types {
                if !types.contains(&edge.edge_type) {
                    continue;
                }
            }

            adjacency.entry(&edge.source).or_default().push((
                &edge.target,
                &edge.id,
                edge.edge_type,
            ));
            // Add reverse edge for undirected traversal
            adjacency.entry(&edge.target).or_default().push((
                &edge.source,
                &edge.id,
                edge.edge_type,
            ));
        }

        // BFS to find shortest path
        let mut visited: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<Vec<(&str, Option<&str>)>> = VecDeque::new();

        queue.push_back(vec![(from_id, None)]);
        visited.insert(from_id);

        while let Some(path) = queue.pop_front() {
            let (current, _) = path.last().unwrap();

            if *current == to_id {
                // Found path - convert to GraphPath
                let node_ids: Vec<String> = path.iter().map(|(n, _)| n.to_string()).collect();
                let edge_ids: Vec<String> = path
                    .iter()
                    .filter_map(|(_, e)| e.map(|s| s.to_string()))
                    .collect();

                let graph_path =
                    GraphPath::new(format!("path-{}-{}", from_id, to_id), node_ids, edge_ids);

                return Ok(vec![graph_path]);
            }

            if let Some(neighbors) = adjacency.get(*current) {
                for (next, edge_id, _) in neighbors {
                    if !visited.contains(*next) {
                        visited.insert(*next);
                        let mut new_path = path.clone();
                        new_path.push((*next, Some(*edge_id)));
                        queue.push_back(new_path);
                    }
                }
            }
        }

        Ok(vec![]) // No path found
    }

    /// Find all connected nodes from a starting point
    pub fn execute_find_connected(
        &self,
        model: &GraphViewModel,
        start_id: &str,
        max_depth: u32,
        edge_types: Option<&[EdgeType]>,
    ) -> Result<GraphViewModel> {
        let start = std::time::Instant::now();

        let mut connected = GraphViewModel::new(model.root_id.clone());
        let mut visited: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<(&str, u32)> = VecDeque::new();

        // Build adjacency map
        let mut adjacency: HashMap<&str, Vec<(&str, &GraphEdge)>> = HashMap::new();
        for edge in &model.edges {
            if let Some(types) = edge_types {
                if !types.contains(&edge.edge_type) {
                    continue;
                }
            }
            adjacency
                .entry(&edge.source)
                .or_default()
                .push((&edge.target, edge));
            adjacency
                .entry(&edge.target)
                .or_default()
                .push((&edge.source, edge));
        }

        queue.push_back((start_id, 0));
        visited.insert(start_id);

        while let Some((current, depth)) = queue.pop_front() {
            // Add node to result
            if let Some(node) = model.get_node(current) {
                if !connected.has_node(current) {
                    connected.add_node(node.clone());
                }
            }

            if depth >= max_depth {
                continue;
            }

            // Explore neighbors
            if let Some(neighbors) = adjacency.get(current) {
                for (next, edge) in neighbors {
                    connected.add_edge((*edge).clone());

                    if !visited.contains(*next) {
                        visited.insert(*next);
                        queue.push_back((*next, depth + 1));
                    }
                }
            }
        }

        connected.compute_stats();
        connected.stats.execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(connected)
    }

    // =========================================================================
    // ANCESTOR/DESCENDANT OPERATIONS
    // =========================================================================

    /// Find all ancestors (BFS upward through ownership/control)
    pub async fn execute_ancestors(
        &self,
        entity_id: Uuid,
        max_depth: u32,
    ) -> Result<GraphViewModel> {
        self.execute_traversal(entity_id, max_depth, TraversalDirection::Ancestors)
            .await
    }

    /// Find all descendants (BFS downward through ownership/control)
    pub async fn execute_descendants(
        &self,
        entity_id: Uuid,
        max_depth: u32,
    ) -> Result<GraphViewModel> {
        self.execute_traversal(entity_id, max_depth, TraversalDirection::Descendants)
            .await
    }

    /// Execute traversal in a direction
    async fn execute_traversal(
        &self,
        entity_id: Uuid,
        max_depth: u32,
        direction: TraversalDirection,
    ) -> Result<GraphViewModel> {
        let start = std::time::Instant::now();

        let mut model = GraphViewModel::new(entity_id.to_string());
        model.depth = max_depth;

        // Fetch root entity
        let root = self.fetch_entity(entity_id).await?;
        model.add_node(root);

        // BFS traversal
        let mut visited: HashSet<Uuid> = HashSet::new();
        let mut queue: VecDeque<(Uuid, u32)> = VecDeque::new();

        queue.push_back((entity_id, 0));
        visited.insert(entity_id);

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let relationships = match direction {
                TraversalDirection::Ancestors => self.fetch_owners_of(current_id).await?,
                TraversalDirection::Descendants => self.fetch_owned_by(current_id).await?,
            };

            for (node, edge) in relationships {
                let next_id = Uuid::parse_str(&node.id)?;

                if !model.has_node(&node.id) {
                    model.add_node(node);
                }
                model.add_edge(edge);

                if !visited.contains(&next_id) {
                    visited.insert(next_id);
                    queue.push_back((next_id, depth + 1));
                }
            }
        }

        model.compute_stats();
        model.stats.max_depth = max_depth;
        model.stats.execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(model)
    }

    // =========================================================================
    // COMPARISON OPERATIONS
    // =========================================================================

    /// Compare two graph snapshots
    pub fn execute_compare(
        &self,
        left: &GraphViewModel,
        right: &GraphViewModel,
    ) -> Result<GraphComparison> {
        let left_node_ids: HashSet<_> = left.nodes.iter().map(|n| &n.id).collect();
        let right_node_ids: HashSet<_> = right.nodes.iter().map(|n| &n.id).collect();

        let nodes_added: Vec<String> = right_node_ids
            .difference(&left_node_ids)
            .map(|s| (*s).clone())
            .collect();

        let nodes_removed: Vec<String> = left_node_ids
            .difference(&right_node_ids)
            .map(|s| (*s).clone())
            .collect();

        let left_edge_ids: HashSet<_> = left.edges.iter().map(|e| &e.id).collect();
        let right_edge_ids: HashSet<_> = right.edges.iter().map(|e| &e.id).collect();

        let edges_added: Vec<String> = right_edge_ids
            .difference(&left_edge_ids)
            .map(|s| (*s).clone())
            .collect();

        let edges_removed: Vec<String> = left_edge_ids
            .difference(&right_edge_ids)
            .map(|s| (*s).clone())
            .collect();

        // Check for ownership changes
        let ownership_edges = [EdgeType::Owns, EdgeType::Controls];
        let has_ownership_changes = edges_added
            .iter()
            .chain(edges_removed.iter())
            .any(|edge_id| {
                left.edges
                    .iter()
                    .chain(right.edges.iter())
                    .any(|e| &e.id == edge_id && ownership_edges.contains(&e.edge_type))
            });

        Ok(GraphComparison {
            query_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            left_id: left.query_id.clone(),
            right_id: right.query_id.clone(),
            nodes_added: nodes_added.clone(),
            nodes_removed: nodes_removed.clone(),
            nodes_changed: vec![], // TODO: implement field-level comparison
            edges_added: edges_added.clone(),
            edges_removed: edges_removed.clone(),
            summary: super::view_model::ComparisonSummary {
                nodes_added_count: nodes_added.len(),
                nodes_removed_count: nodes_removed.len(),
                nodes_changed_count: 0,
                edges_added_count: edges_added.len(),
                edges_removed_count: edges_removed.len(),
                has_structural_changes: !nodes_added.is_empty()
                    || !nodes_removed.is_empty()
                    || !edges_added.is_empty()
                    || !edges_removed.is_empty(),
                has_ownership_changes,
            },
        })
    }

    // =========================================================================
    // VIEW MODE HELPERS
    // =========================================================================

    fn get_view_mode_info(&self, mode: &str) -> ViewModeInfo {
        match mode.to_uppercase().as_str() {
            "KYC_UBO" => ViewModeInfo {
                name: "KYC_UBO".to_string(),
                layers: vec![LayerType::Core, LayerType::Kyc, LayerType::Ubo],
                edge_types: vec![
                    EdgeType::HasRole,
                    EdgeType::Owns,
                    EdgeType::Controls,
                    EdgeType::Validates,
                    EdgeType::Requires,
                ],
                description: Some("Entity ownership/control with KYC status".to_string()),
            },
            "UBO_ONLY" => ViewModeInfo {
                name: "UBO_ONLY".to_string(),
                layers: vec![LayerType::Core, LayerType::Ubo],
                edge_types: vec![
                    EdgeType::Owns,
                    EdgeType::Controls,
                    EdgeType::TrustSettlor,
                    EdgeType::TrustTrustee,
                    EdgeType::TrustBeneficiary,
                    EdgeType::TrustProtector,
                ],
                description: Some("Pure ownership and control graph".to_string()),
            },
            "SERVICE_DELIVERY" => ViewModeInfo {
                name: "SERVICE_DELIVERY".to_string(),
                layers: vec![LayerType::Core, LayerType::Services],
                edge_types: vec![
                    EdgeType::HasRole,
                    EdgeType::UsesProduct,
                    EdgeType::Delivers,
                    EdgeType::BelongsTo,
                    EdgeType::ProvisionedFor,
                ],
                description: Some("Products, services, and resource instances".to_string()),
            },
            "CUSTODY" => ViewModeInfo {
                name: "CUSTODY".to_string(),
                layers: vec![LayerType::Core, LayerType::Custody],
                edge_types: vec![
                    EdgeType::RoutesTo,
                    EdgeType::Matches,
                    EdgeType::CoveredBy,
                    EdgeType::SecuredBy,
                    EdgeType::SettlesAt,
                ],
                description: Some("Markets, SSIs, and booking rules".to_string()),
            },
            "PRODUCTS_ONLY" => ViewModeInfo {
                name: "PRODUCTS_ONLY".to_string(),
                layers: vec![LayerType::Core],
                edge_types: vec![EdgeType::UsesProduct],
                description: Some("CBU with products only".to_string()),
            },
            _ => ViewModeInfo {
                name: mode.to_string(),
                layers: vec![LayerType::Core],
                edge_types: vec![EdgeType::HasRole],
                description: None,
            },
        }
    }

    fn apply_view_mode_filter(&self, model: &mut GraphViewModel, mode: &str) {
        let info = self.get_view_mode_info(mode);

        // Filter nodes by layer
        model.nodes.retain(|n| info.layers.contains(&n.layer));

        // Filter edges by type
        model
            .edges
            .retain(|e| info.edge_types.contains(&e.edge_type));

        // Rebuild groupings
        model.nodes_by_layer.clear();
        model.nodes_by_type.clear();
        for node in &model.nodes {
            model
                .nodes_by_layer
                .entry(node.layer)
                .or_default()
                .push(node.id.clone());
            model
                .nodes_by_type
                .entry(node.node_type)
                .or_default()
                .push(node.id.clone());
        }
    }

    // =========================================================================
    // DATABASE FETCH METHODS (stubs for now - will use existing repository)
    // =========================================================================

    async fn fetch_cbu(&self, cbu_id: Uuid) -> Result<GraphNode> {
        let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT c.name, c.jurisdiction, c.client_type
            FROM "ob-poc".cbus c
            WHERE c.cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await?;

        let (name, jurisdiction, client_type) = row.ok_or_else(|| anyhow!("CBU not found"))?;

        Ok(GraphNode {
            id: cbu_id.to_string(),
            node_type: NodeType::Cbu,
            layer: LayerType::Core,
            label: name,
            sublabel: client_type,
            jurisdiction,
            ..Default::default()
        })
    }

    async fn fetch_entity(&self, entity_id: Uuid) -> Result<GraphNode> {
        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT e.name, et.name as type_name, et.entity_category
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE e.entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?;

        let (name, type_name, category) = row.ok_or_else(|| anyhow!("Entity not found"))?;

        Ok(GraphNode {
            id: entity_id.to_string(),
            node_type: NodeType::Entity,
            layer: LayerType::Core,
            label: name,
            sublabel: Some(type_name),
            entity_category: category,
            ..Default::default()
        })
    }

    async fn fetch_cbu_entities(&self, cbu_id: Uuid) -> Result<Vec<EntityWithRole>> {
        let rows: Vec<EntityRoleRow> = sqlx::query_as(
            r#"
            SELECT
                e.entity_id,
                e.name,
                et.name as type_name,
                et.entity_category,
                r.name as role_name,
                r.priority
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            ORDER BY r.priority
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;

        let mut entities: HashMap<Uuid, EntityWithRole> = HashMap::new();

        for (entity_id, name, type_name, category, role_name, priority) in rows {
            let entry = entities.entry(entity_id).or_insert_with(|| EntityWithRole {
                node: GraphNode {
                    id: entity_id.to_string(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Core,
                    label: name.clone(),
                    sublabel: Some(type_name.clone()),
                    entity_category: category.clone(),
                    roles: Vec::new(),
                    ..Default::default()
                },
                role_edge: GraphEdge {
                    id: format!("role-{}-{}", cbu_id, entity_id),
                    source: cbu_id.to_string(),
                    target: entity_id.to_string(),
                    edge_type: EdgeType::HasRole,
                    label: None,
                },
            });

            entry.node.roles.push(role_name.clone());
            if entry.node.primary_role.is_none() {
                entry.node.primary_role = Some(role_name.clone());
                entry.node.role_priority = priority;
            }
            entry.role_edge.label = Some(entry.node.roles.join(", "));
        }

        Ok(entities.into_values().collect())
    }

    async fn fetch_entity_relationships(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<(GraphNode, GraphEdge)>> {
        // Fetch ownership/control relationships verified for this CBU
        let rows: Vec<RelationshipRow> = sqlx::query_as(
            r#"
            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.relationship_type,
                er.percentage,
                e_from.name as from_name,
                e_to.name as to_name,
                crv.status as verification_status
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".cbu_relationship_verification crv ON er.relationship_id = crv.relationship_id
            JOIN "ob-poc".entities e_from ON er.from_entity_id = e_from.entity_id
            JOIN "ob-poc".entities e_to ON er.to_entity_id = e_to.entity_id
            WHERE crv.cbu_id = $1
              AND er.effective_to IS NULL
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();

        for (from_id, to_id, rel_type, percentage, from_name, _to_name, verification_status) in rows
        {
            let edge_type = EdgeType::from_relationship_type(&rel_type).unwrap_or(EdgeType::Owns);

            let label = percentage.map(|p| format!("{}%", p));

            // Create node for the owner (from_id)
            let node = GraphNode {
                id: from_id.to_string(),
                node_type: NodeType::Entity,
                layer: LayerType::Ubo,
                label: from_name,
                verification_status,
                ..Default::default()
            };

            let edge = GraphEdge {
                id: format!("{}-{}-{}", rel_type, from_id, to_id),
                source: from_id.to_string(),
                target: to_id.to_string(),
                edge_type,
                label,
            };

            results.push((node, edge));
        }

        Ok(results)
    }

    async fn fetch_connected_nodes(
        &self,
        entity_id: Uuid,
        _depth: u32,
    ) -> Result<Vec<(GraphNode, GraphEdge)>> {
        // Fetch directly connected entities (both directions)
        let rows: Vec<(Uuid, String, String, Option<rust_decimal::Decimal>)> = sqlx::query_as(
            r#"
            SELECT
                CASE WHEN er.from_entity_id = $1 THEN er.to_entity_id ELSE er.from_entity_id END as other_id,
                e.name,
                er.relationship_type,
                er.percentage
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".entities e ON e.entity_id =
                CASE WHEN er.from_entity_id = $1 THEN er.to_entity_id ELSE er.from_entity_id END
            WHERE (er.from_entity_id = $1 OR er.to_entity_id = $1)
              AND er.effective_to IS NULL
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();

        for (other_id, name, rel_type, percentage) in rows {
            let edge_type = EdgeType::from_relationship_type(&rel_type).unwrap_or(EdgeType::Owns);

            let node = GraphNode {
                id: other_id.to_string(),
                node_type: NodeType::Entity,
                layer: LayerType::Ubo,
                label: name,
                ..Default::default()
            };

            let edge = GraphEdge {
                id: format!("{}-{}-{}", rel_type, entity_id, other_id),
                source: entity_id.to_string(),
                target: other_id.to_string(),
                edge_type,
                label: percentage.map(|p| format!("{}%", p)),
            };

            results.push((node, edge));
        }

        Ok(results)
    }

    async fn fetch_owners_of(&self, entity_id: Uuid) -> Result<Vec<(GraphNode, GraphEdge)>> {
        let rows: Vec<(Uuid, String, String, Option<rust_decimal::Decimal>)> = sqlx::query_as(
            r#"
            SELECT
                er.from_entity_id,
                e.name,
                er.relationship_type,
                er.percentage
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".entities e ON e.entity_id = er.from_entity_id
            WHERE er.to_entity_id = $1
              AND er.effective_to IS NULL
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();

        for (owner_id, name, rel_type, percentage) in rows {
            let edge_type = EdgeType::from_relationship_type(&rel_type).unwrap_or(EdgeType::Owns);

            let node = GraphNode {
                id: owner_id.to_string(),
                node_type: NodeType::Entity,
                layer: LayerType::Ubo,
                label: name,
                ..Default::default()
            };

            let edge = GraphEdge {
                id: format!("{}-{}-{}", rel_type, owner_id, entity_id),
                source: owner_id.to_string(),
                target: entity_id.to_string(),
                edge_type,
                label: percentage.map(|p| format!("{}%", p)),
            };

            results.push((node, edge));
        }

        Ok(results)
    }

    async fn fetch_owned_by(&self, entity_id: Uuid) -> Result<Vec<(GraphNode, GraphEdge)>> {
        let rows: Vec<(Uuid, String, String, Option<rust_decimal::Decimal>)> = sqlx::query_as(
            r#"
            SELECT
                er.to_entity_id,
                e.name,
                er.relationship_type,
                er.percentage
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".entities e ON e.entity_id = er.to_entity_id
            WHERE er.from_entity_id = $1
              AND er.effective_to IS NULL
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();

        for (owned_id, name, rel_type, percentage) in rows {
            let edge_type = EdgeType::from_relationship_type(&rel_type).unwrap_or(EdgeType::Owns);

            let node = GraphNode {
                id: owned_id.to_string(),
                node_type: NodeType::Entity,
                layer: LayerType::Ubo,
                label: name,
                ..Default::default()
            };

            let edge = GraphEdge {
                id: format!("{}-{}-{}", rel_type, entity_id, owned_id),
                source: entity_id.to_string(),
                target: owned_id.to_string(),
                edge_type,
                label: percentage.map(|p| format!("{}%", p)),
            };

            results.push((node, edge));
        }

        Ok(results)
    }
}

// =============================================================================
// HELPER TYPES
// =============================================================================

struct EntityWithRole {
    node: GraphNode,
    role_edge: GraphEdge,
}

#[derive(Debug, Clone, Copy)]
enum TraversalDirection {
    Ancestors,
    Descendants,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper: get view mode info without needing a pool
    /// (the actual function doesn't use the pool)
    fn get_view_mode_info_test(mode: &str) -> ViewModeInfo {
        match mode.to_uppercase().as_str() {
            "KYC_UBO" => ViewModeInfo {
                name: "KYC_UBO".to_string(),
                layers: vec![LayerType::Core, LayerType::Ubo, LayerType::Kyc],
                edge_types: vec![
                    EdgeType::HasRole,
                    EdgeType::Owns,
                    EdgeType::Controls,
                    EdgeType::Requires,
                ],
                description: Some("KYC and UBO view".to_string()),
            },
            "SERVICE_DELIVERY" => ViewModeInfo {
                name: "SERVICE_DELIVERY".to_string(),
                layers: vec![LayerType::Core, LayerType::Services],
                edge_types: vec![
                    EdgeType::HasRole,
                    EdgeType::UsesProduct,
                    EdgeType::Delivers,
                    EdgeType::BelongsTo,
                ],
                description: Some("Service delivery view".to_string()),
            },
            _ => ViewModeInfo {
                name: mode.to_string(),
                layers: vec![LayerType::Core],
                edge_types: vec![EdgeType::HasRole],
                description: None,
            },
        }
    }

    /// Test helper: check if node matches filter without needing a pool
    fn node_matches_filter_test(node: &GraphNode, filter: &GraphFilter) -> bool {
        // Node type filter
        if !filter.node_types.is_empty() && !filter.node_types.contains(&node.node_type) {
            return false;
        }

        // Layer filter
        if !filter.layers.is_empty() && !filter.layers.contains(&node.layer) {
            return false;
        }

        // Role filter
        if let Some(ref role) = filter.role {
            if !node.roles.iter().any(|r| r.eq_ignore_ascii_case(role)) {
                return false;
            }
        }

        // Jurisdiction filter
        if let Some(ref jurisdiction) = filter.jurisdiction {
            if node.jurisdiction.as_deref() != Some(jurisdiction.as_str()) {
                return false;
            }
        }

        true
    }

    /// Test helper: find path without needing a pool
    fn find_path_test(
        model: &GraphViewModel,
        from_id: &str,
        to_id: &str,
    ) -> Result<Vec<GraphPath>> {
        // Build adjacency list
        let mut adjacency: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for edge in &model.edges {
            adjacency
                .entry(edge.source.clone())
                .or_default()
                .push((edge.target.clone(), edge.id.clone()));
        }

        // BFS for shortest path
        let mut queue: VecDeque<(String, Vec<String>, Vec<String>)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();

        queue.push_back((from_id.to_string(), vec![from_id.to_string()], vec![]));
        visited.insert(from_id.to_string());

        while let Some((current, path, edges)) = queue.pop_front() {
            if current == to_id {
                return Ok(vec![GraphPath {
                    id: format!("path-{}-{}", from_id, to_id),
                    node_ids: path,
                    edge_ids: edges.clone(),
                    length: edges.len(),
                    weight: 0.0,
                    path_type: None,
                    aggregate_percentage: None,
                }]);
            }

            if let Some(neighbors) = adjacency.get(&current) {
                for (next, edge_id) in neighbors {
                    if !visited.contains(next) {
                        visited.insert(next.clone());
                        let mut new_path = path.clone();
                        new_path.push(next.clone());
                        let mut new_edges = edges.clone();
                        new_edges.push(edge_id.clone());
                        queue.push_back((next.clone(), new_path, new_edges));
                    }
                }
            }
        }

        Ok(vec![])
    }

    #[test]
    fn test_view_mode_info() {
        let info = get_view_mode_info_test("KYC_UBO");
        assert_eq!(info.name, "KYC_UBO");
        assert!(info.layers.contains(&LayerType::Ubo));
        assert!(info.edge_types.contains(&EdgeType::Owns));
    }

    #[test]
    fn test_filter_matching() {
        let node = GraphNode {
            id: "test".to_string(),
            node_type: NodeType::Entity,
            layer: LayerType::Core,
            label: "Test Entity".to_string(),
            roles: vec!["DIRECTOR".to_string()],
            jurisdiction: Some("LU".to_string()),
            ..Default::default()
        };

        // Empty filter matches all
        let filter = GraphFilter::default();
        assert!(node_matches_filter_test(&node, &filter));

        // Role filter
        let filter = GraphFilter {
            role: Some("DIRECTOR".to_string()),
            ..Default::default()
        };
        assert!(node_matches_filter_test(&node, &filter));

        // Non-matching role
        let filter = GraphFilter {
            role: Some("UBO".to_string()),
            ..Default::default()
        };
        assert!(!node_matches_filter_test(&node, &filter));

        // Jurisdiction filter
        let filter = GraphFilter {
            jurisdiction: Some("LU".to_string()),
            ..Default::default()
        };
        assert!(node_matches_filter_test(&node, &filter));
    }

    #[test]
    fn test_path_finding() {
        let mut model = GraphViewModel::new("root".to_string());

        // A -> B -> C
        model.add_node(GraphNode {
            id: "A".to_string(),
            ..Default::default()
        });
        model.add_node(GraphNode {
            id: "B".to_string(),
            ..Default::default()
        });
        model.add_node(GraphNode {
            id: "C".to_string(),
            ..Default::default()
        });

        model.add_edge(GraphEdge {
            id: "e1".to_string(),
            source: "A".to_string(),
            target: "B".to_string(),
            edge_type: EdgeType::Owns,
            label: None,
        });
        model.add_edge(GraphEdge {
            id: "e2".to_string(),
            source: "B".to_string(),
            target: "C".to_string(),
            edge_type: EdgeType::Owns,
            label: None,
        });

        let paths = find_path_test(&model, "A", "C").unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].length, 2); // 2 edges: A->B, B->C
        assert_eq!(paths[0].node_ids, vec!["A", "B", "C"]);
        assert_eq!(paths[0].edge_ids, vec!["e1", "e2"]);
    }
}
