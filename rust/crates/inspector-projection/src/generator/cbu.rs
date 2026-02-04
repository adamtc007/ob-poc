//! CBU generator - transforms `CbuGraphResponse` into projection nodes.
//!
//! Creates:
//! - CBU root node with branches for members, products, matrix
//! - MemberList node with paged entity refs
//! - Entity nodes with roles, attributes

use crate::model::{
    InspectorProjection, Node, NodeKind, NodeSummary, PagingList, Provenance, RefOrList,
    SnapshotMeta, UiHints,
};
use crate::node_id::NodeId;
use crate::policy::RenderPolicy;
use crate::ref_value::RefValue;
use std::collections::BTreeMap;

/// Generator that transforms `CbuGraphResponse` into an `InspectorProjection`.
///
/// The generator creates a hierarchical structure:
/// ```text
/// cbu:{cbu_id}
/// ├── members (MemberList)
/// │   ├── entity:{entity_id_1}
/// │   ├── entity:{entity_id_2}
/// │   └── ...
/// └── products (ProductTree) [if products exist]
///     └── ...
/// ```
#[derive(Debug, Default)]
pub struct CbuGenerator {
    /// Include edge nodes (holding/control edges).
    include_edges: bool,
}

impl CbuGenerator {
    /// Create a new CBU generator with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure whether to include edge nodes.
    pub fn with_edges(mut self, include: bool) -> Self {
        self.include_edges = include;
        self
    }

    /// Generate a projection from a `CbuGraphResponse`.
    ///
    /// This is the main entry point. It accepts the loosely-typed API response
    /// and builds a strongly-typed projection.
    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        &self,
        cbu_id: &str,
        cbu_label: &str,
        cbu_category: Option<&str>,
        jurisdiction: Option<&str>,
        nodes: &[GraphNodeInput],
        edges: &[GraphEdgeInput],
        policy: &RenderPolicy,
    ) -> InspectorProjection {
        let mut projection = InspectorProjection {
            snapshot: self.build_snapshot_meta(cbu_id),
            render_policy: policy.clone(),
            ui_hints: UiHints::default(),
            root: BTreeMap::new(),
            nodes: BTreeMap::new(),
        };

        // Create CBU root node
        let cbu_node_id = NodeId::new(format!("cbu:{}", cbu_id)).expect("valid cbu node id");
        let mut cbu_node =
            Node::new(cbu_node_id.clone(), NodeKind::Cbu, cbu_label).with_glyph("🏢");

        if let Some(full_label) = Self::build_full_label(cbu_label, cbu_category, jurisdiction) {
            cbu_node = cbu_node.with_label_full(full_label);
        }

        // Add CBU attributes
        if let Some(cat) = cbu_category {
            cbu_node = cbu_node.with_attribute("cbu_category", cat);
        }
        if let Some(jur) = jurisdiction {
            cbu_node = cbu_node.with_attribute("jurisdiction", jur);
        }

        // Separate entity nodes from other nodes
        let entity_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| n.node_type == "entity" || n.layer == "entity")
            .collect();

        // Create member list node and entity nodes
        if !entity_nodes.is_empty() {
            let (member_list_node, entity_projection_nodes) =
                self.build_member_list(cbu_id, &entity_nodes, policy);

            let member_list_id = member_list_node.id.clone();

            // Add members branch to CBU
            cbu_node = cbu_node.with_branch("members", member_list_id.clone());

            // Add member list and entities to projection
            projection.insert_node(member_list_node);
            for entity_node in entity_projection_nodes {
                projection.insert_node(entity_node);
            }
        }

        // Build control/holding edges if enabled
        if self.include_edges && !edges.is_empty() {
            let (register_node, edge_nodes) = self.build_control_register(cbu_id, edges, policy);
            let register_id = register_node.id.clone();

            cbu_node = cbu_node.with_branch("registers", register_id);
            projection.insert_node(register_node);
            for edge_node in edge_nodes {
                projection.insert_node(edge_node);
            }
        }

        // Add summary to CBU node
        cbu_node = cbu_node.with_summary(NodeSummary::count(entity_nodes.len()));

        // Insert CBU root node and set root reference
        projection.insert_node(cbu_node);
        projection.set_root("cbu", cbu_node_id);

        projection
    }

    /// Build snapshot metadata.
    fn build_snapshot_meta(&self, cbu_id: &str) -> SnapshotMeta {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        cbu_id.hash(&mut hasher);
        let source_hash = format!("{:016x}", hasher.finish());

        SnapshotMeta {
            schema_version: 1,
            source_hash,
            policy_hash: String::new(), // Will be set by caller if needed
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Build a full label from components.
    fn build_full_label(
        label: &str,
        category: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Option<String> {
        let mut parts = vec![label.to_string()];
        if let Some(cat) = category {
            parts.push(format!("({})", cat));
        }
        if let Some(jur) = jurisdiction {
            parts.push(format!("[{}]", jur));
        }
        if parts.len() > 1 {
            Some(parts.join(" "))
        } else {
            None
        }
    }

    /// Build member list and entity nodes.
    fn build_member_list(
        &self,
        cbu_id: &str,
        entities: &[&GraphNodeInput],
        policy: &RenderPolicy,
    ) -> (Node, Vec<Node>) {
        let member_list_id =
            NodeId::new(format!("memberlist:{}", cbu_id)).expect("valid memberlist id");

        let mut entity_nodes = Vec::new();
        let mut entity_refs = Vec::new();

        // Limit entities based on policy
        let max_items = policy.max_items_per_list;
        let limited_entities = if entities.len() > max_items {
            &entities[..max_items]
        } else {
            entities
        };

        for entity in limited_entities {
            let entity_id = NodeId::new(format!("entity:{}", entity.id)).expect("valid entity id");
            let entity_node = self.build_entity_node(&entity_id, entity, policy);
            entity_refs.push(RefValue::new(entity_id));
            entity_nodes.push(entity_node);
        }

        // Create paging info
        let next_token = if entities.len() > max_items {
            Some(
                NodeId::new(format!("pagetoken:{}:{}", cbu_id, max_items))
                    .expect("valid pagetoken id"),
            )
        } else {
            None
        };

        let paging_list = PagingList::new(entity_refs, max_items, next_token);

        let member_list_node =
            Node::new(member_list_id, NodeKind::MemberList, "Members").with_glyph("👥");

        // We need to set branches properly - create a MemberList with the entities branch
        let mut node = member_list_node;
        node.branches
            .insert("entities".to_string(), RefOrList::List(paging_list));
        node = node.with_summary(NodeSummary::count(entities.len()));

        (node, entity_nodes)
    }

    /// Build a single entity node.
    fn build_entity_node(
        &self,
        id: &NodeId,
        input: &GraphNodeInput,
        _policy: &RenderPolicy,
    ) -> Node {
        let glyph = match input.node_type.as_str() {
            "proper_person" | "natural_person" => "👤",
            "limited_company" | "company" => "🏛️",
            "trust" | "foundation" => "📜",
            "fund" => "💼",
            _ => "👤",
        };

        let mut node = Node::new(id.clone(), NodeKind::Entity, &input.label).with_glyph(glyph);

        // Add sublabel if present
        if let Some(ref sublabel) = input.sublabel {
            node = node.with_label_full(format!("{} - {}", input.label, sublabel));
        }

        // Add entity attributes
        node = node.with_attribute("entity_type", input.node_type.as_str());

        if let Some(ref jur) = input.jurisdiction {
            node = node.with_attribute("jurisdiction", jur.as_str());
        }

        if !input.roles.is_empty() {
            node = node.with_attribute("roles", serde_json::json!(input.roles));
        }

        if let Some(ref primary_role) = input.primary_role {
            node = node.with_attribute("primary_role", primary_role.as_str());
        }

        if let Some(ref status) = input.status {
            node = node.with_attribute("status", status.as_str());
        }

        if let Some(pct) = input.ownership_pct {
            node = node.with_attribute("ownership_pct", pct);
        }

        node
    }

    /// Build control register and edge nodes.
    fn build_control_register(
        &self,
        cbu_id: &str,
        edges: &[GraphEdgeInput],
        policy: &RenderPolicy,
    ) -> (Node, Vec<Node>) {
        let register_id =
            NodeId::new(format!("controlregister:{}", cbu_id)).expect("valid register id");

        let mut edge_nodes = Vec::new();
        let mut edge_refs = Vec::new();

        let max_items = policy.max_items_per_list;
        let limited_edges = if edges.len() > max_items {
            &edges[..max_items]
        } else {
            edges
        };

        for (idx, edge) in limited_edges.iter().enumerate() {
            let edge_id =
                NodeId::new(format!("controledge:{}:{}", cbu_id, idx)).expect("valid edge id");

            let edge_node = self.build_edge_node(&edge_id, edge);
            edge_refs.push(RefValue::new(edge_id));
            edge_nodes.push(edge_node);
        }

        let next_token = if edges.len() > max_items {
            Some(
                NodeId::new(format!("pagetoken:edges:{}:{}", cbu_id, max_items))
                    .expect("valid pagetoken id"),
            )
        } else {
            None
        };

        let paging_list = PagingList::new(edge_refs, max_items, next_token);

        let mut register_node =
            Node::new(register_id, NodeKind::ControlRegister, "Control Register").with_glyph("🧬");

        register_node
            .branches
            .insert("edges".to_string(), RefOrList::List(paging_list));
        register_node = register_node.with_summary(NodeSummary::count(edges.len()));

        (register_node, edge_nodes)
    }

    /// Build a single edge node (ControlEdge).
    fn build_edge_node(&self, id: &NodeId, input: &GraphEdgeInput) -> Node {
        let label = input
            .label
            .clone()
            .unwrap_or_else(|| format!("{} → {}", input.source, input.target));

        let mut node = Node::new(id.clone(), NodeKind::ControlEdge, &label).with_glyph("⬇");

        // Add edge attributes
        node = node.with_attribute("source", input.source.as_str());
        node = node.with_attribute("target", input.target.as_str());
        node = node.with_attribute("edge_type", input.edge_type.as_str());

        if let Some(weight) = input.weight {
            node = node.with_attribute("weight", weight);
        }

        if let Some(ref status) = input.verification_status {
            node = node.with_attribute("verification_status", status.as_str());
        }

        // Control edges require provenance
        let provenance = Provenance::new(
            vec!["cbu_graph".to_string()],
            chrono::Utc::now().to_rfc3339(),
        )
        .with_confidence(input.weight.map(|w| w as f64 / 100.0).unwrap_or(1.0));
        node = node.with_provenance(provenance);

        node
    }
}

// ============================================================================
// INPUT TYPES (mirror CbuGraphResponse fields)
// ============================================================================

/// Input graph node (mirrors `GraphNode` from ob-poc-types).
#[derive(Debug, Clone)]
pub struct GraphNodeInput {
    pub id: String,
    pub node_type: String,
    pub layer: String,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: Option<String>,
    pub roles: Vec<String>,
    pub primary_role: Option<String>,
    pub jurisdiction: Option<String>,
    pub ownership_pct: Option<f64>,
}

impl GraphNodeInput {
    /// Create from a `GraphNode` (ob-poc-types).
    pub fn from_graph_node(node: &ob_poc_types::GraphNode) -> Self {
        Self {
            id: node.id.clone(),
            node_type: node.node_type.clone(),
            layer: node.layer.clone(),
            label: node.label.clone(),
            sublabel: node.sublabel.clone(),
            status: Some(node.status.clone()),
            roles: node.roles.clone(),
            primary_role: node.primary_role.clone(),
            jurisdiction: node.jurisdiction.clone(),
            ownership_pct: node.ownership_pct,
        }
    }
}

/// Input graph edge (mirrors `GraphEdge` from ob-poc-types).
#[derive(Debug, Clone)]
pub struct GraphEdgeInput {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub label: Option<String>,
    pub weight: Option<f32>,
    pub verification_status: Option<String>,
}

impl GraphEdgeInput {
    /// Create from a `GraphEdge` (ob-poc-types).
    pub fn from_graph_edge(edge: &ob_poc_types::GraphEdge) -> Self {
        Self {
            id: edge.id.clone(),
            source: edge.source.clone(),
            target: edge.target.clone(),
            edge_type: edge.edge_type.clone(),
            label: edge.label.clone(),
            weight: edge.weight,
            verification_status: edge.verification_status.clone(),
        }
    }
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Generate a projection directly from a `CbuGraphResponse`.
pub fn generate_from_cbu_graph(
    response: &ob_poc_types::CbuGraphResponse,
    policy: &RenderPolicy,
) -> InspectorProjection {
    let nodes: Vec<GraphNodeInput> = response
        .nodes
        .iter()
        .map(GraphNodeInput::from_graph_node)
        .collect();

    let edges: Vec<GraphEdgeInput> = response
        .edges
        .iter()
        .map(GraphEdgeInput::from_graph_edge)
        .collect();

    CbuGenerator::new().with_edges(true).generate(
        &response.cbu_id,
        &response.label,
        response.cbu_category.as_deref(),
        response.jurisdiction.as_deref(),
        &nodes,
        &edges,
        policy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::validate;

    #[test]
    fn test_cbu_generator_basic() {
        let nodes = vec![
            GraphNodeInput {
                id: "entity-001".to_string(),
                node_type: "proper_person".to_string(),
                layer: "entity".to_string(),
                label: "John Smith".to_string(),
                sublabel: Some("Director".to_string()),
                status: Some("active".to_string()),
                roles: vec!["DIRECTOR".to_string()],
                primary_role: Some("DIRECTOR".to_string()),
                jurisdiction: Some("IE".to_string()),
                ownership_pct: None,
            },
            GraphNodeInput {
                id: "entity-002".to_string(),
                node_type: "limited_company".to_string(),
                layer: "entity".to_string(),
                label: "Acme Holdings Ltd".to_string(),
                sublabel: None,
                status: Some("active".to_string()),
                roles: vec!["SHAREHOLDER".to_string()],
                primary_role: Some("SHAREHOLDER".to_string()),
                jurisdiction: Some("UK".to_string()),
                ownership_pct: Some(75.0),
            },
        ];

        let policy = RenderPolicy::default();
        let projection = CbuGenerator::new().generate(
            "cbu-001",
            "Test Fund SICAV",
            Some("SICAV"),
            Some("LU"),
            &nodes,
            &[],
            &policy,
        );

        // Validate the projection
        let result = validate(&projection);
        assert!(
            result.errors.is_empty(),
            "Validation errors: {:?}",
            result.errors
        );

        // Check structure
        assert!(projection.root.contains_key("cbu"));

        let cbu_id = NodeId::new("cbu:cbu-001").unwrap();
        let cbu_node = projection.get_node(&cbu_id).expect("CBU node should exist");
        assert_eq!(cbu_node.kind, NodeKind::Cbu);
        assert_eq!(cbu_node.label_short, "Test Fund SICAV");
        assert!(cbu_node.branches.contains_key("members"));

        // Check member list
        let member_list_id = NodeId::new("memberlist:cbu-001").unwrap();
        let member_list = projection
            .get_node(&member_list_id)
            .expect("MemberList should exist");
        assert_eq!(member_list.kind, NodeKind::MemberList);

        // Check entities exist
        let entity1_id = NodeId::new("entity:entity-001").unwrap();
        let entity1 = projection
            .get_node(&entity1_id)
            .expect("Entity 1 should exist");
        assert_eq!(entity1.kind, NodeKind::Entity);
        assert_eq!(entity1.label_short, "John Smith");
    }

    #[test]
    fn test_cbu_generator_with_edges() {
        let nodes = vec![GraphNodeInput {
            id: "entity-001".to_string(),
            node_type: "proper_person".to_string(),
            layer: "entity".to_string(),
            label: "John Smith".to_string(),
            sublabel: None,
            status: Some("active".to_string()),
            roles: vec![],
            primary_role: None,
            jurisdiction: None,
            ownership_pct: None,
        }];

        let edges = vec![GraphEdgeInput {
            id: "edge-001".to_string(),
            source: "entity-001".to_string(),
            target: "cbu-001".to_string(),
            edge_type: "controls".to_string(),
            label: Some("Controls via voting rights".to_string()),
            weight: Some(51.0),
            verification_status: Some("proven".to_string()),
        }];

        let policy = RenderPolicy::default();
        let projection = CbuGenerator::new().with_edges(true).generate(
            "cbu-001",
            "Test Fund",
            None,
            None,
            &nodes,
            &edges,
            &policy,
        );

        // Validate
        let result = validate(&projection);
        assert!(
            result.errors.is_empty(),
            "Validation errors: {:?}",
            result.errors
        );

        // Check CBU has registers branch
        let cbu_id = NodeId::new("cbu:cbu-001").unwrap();
        let cbu_node = projection.get_node(&cbu_id).unwrap();
        assert!(cbu_node.branches.contains_key("registers"));

        // Check control register exists
        let register_id = NodeId::new("controlregister:cbu-001").unwrap();
        let register = projection
            .get_node(&register_id)
            .expect("ControlRegister should exist");
        assert_eq!(register.kind, NodeKind::ControlRegister);

        // Check edge exists with provenance
        let edge_id = NodeId::new("controledge:cbu-001:0").unwrap();
        let edge = projection
            .get_node(&edge_id)
            .expect("ControlEdge should exist");
        assert_eq!(edge.kind, NodeKind::ControlEdge);
        assert!(
            edge.provenance.is_some(),
            "ControlEdge must have provenance"
        );
    }

    #[test]
    fn test_cbu_generator_pagination() {
        // Create more entities than max_items_per_list
        let nodes: Vec<GraphNodeInput> = (0..100)
            .map(|i| GraphNodeInput {
                id: format!("entity-{:03}", i),
                node_type: "proper_person".to_string(),
                layer: "entity".to_string(),
                label: format!("Person {}", i),
                sublabel: None,
                status: Some("active".to_string()),
                roles: vec![],
                primary_role: None,
                jurisdiction: None,
                ownership_pct: None,
            })
            .collect();

        let mut policy = RenderPolicy::default();
        policy.max_items_per_list = 20;

        let projection =
            CbuGenerator::new().generate("cbu-001", "Large Fund", None, None, &nodes, &[], &policy);

        // Check paging
        let member_list_id = NodeId::new("memberlist:cbu-001").unwrap();
        let member_list = projection.get_node(&member_list_id).unwrap();

        if let Some(RefOrList::List(paging_list)) = member_list.branches.get("entities") {
            assert_eq!(paging_list.items.len(), 20);
            assert!(paging_list.paging.next.is_some());
            assert_eq!(paging_list.paging.total, Some(20)); // Items in this page
        } else {
            panic!("Expected paging list for entities branch");
        }

        // Projection should only have 20 entity nodes + memberlist + cbu = 22 total
        assert_eq!(projection.nodes.len(), 22);
    }

    #[test]
    fn test_entity_node_attributes() {
        let nodes = vec![GraphNodeInput {
            id: "e1".to_string(),
            node_type: "limited_company".to_string(),
            layer: "entity".to_string(),
            label: "Mega Corp".to_string(),
            sublabel: Some("Holding Company".to_string()),
            status: Some("active".to_string()),
            roles: vec!["SHAREHOLDER".to_string(), "CONTROLLER".to_string()],
            primary_role: Some("SHAREHOLDER".to_string()),
            jurisdiction: Some("DE".to_string()),
            ownership_pct: Some(100.0),
        }];

        let policy = RenderPolicy::default();
        let projection =
            CbuGenerator::new().generate("cbu-001", "Test", None, None, &nodes, &[], &policy);

        let entity_id = NodeId::new("entity:e1").unwrap();
        let entity = projection.get_node(&entity_id).unwrap();

        assert_eq!(
            entity.attributes.get("entity_type"),
            Some(&serde_json::json!("limited_company"))
        );
        assert_eq!(
            entity.attributes.get("jurisdiction"),
            Some(&serde_json::json!("DE"))
        );
        assert_eq!(
            entity.attributes.get("ownership_pct"),
            Some(&serde_json::json!(100.0))
        );
        assert_eq!(
            entity.attributes.get("roles"),
            Some(&serde_json::json!(["SHAREHOLDER", "CONTROLLER"]))
        );
    }
}
