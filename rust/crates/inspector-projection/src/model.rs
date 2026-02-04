//! Core model types for Inspector projections.
//!
//! This module defines:
//! - `InspectorProjection` - Top-level envelope
//! - `Node` - Core node structure
//! - `NodeKind` - 20 node type variants
//! - Supporting types for branches, paging, provenance

use crate::node_id::NodeId;
use crate::policy::RenderPolicy;
use crate::ref_value::RefValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ============================================================================
// TOP-LEVEL PROJECTION
// ============================================================================

/// Complete projection document.
///
/// This is the wire format for loading/saving projections.
/// The projection is deterministic: same input + policy = same output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct InspectorProjection {
    /// Envelope metadata (version, hashes, timestamp).
    pub snapshot: SnapshotMeta,

    /// Rendering policy (LOD, depth, filters).
    pub render_policy: RenderPolicy,

    /// UI display hints.
    #[serde(default)]
    pub ui_hints: UiHints,

    /// Root node references by chamber name.
    /// Entry points into the node graph.
    #[serde(default)]
    pub root: BTreeMap<String, RefValue>,

    /// All nodes keyed by NodeId.
    /// Using BTreeMap for deterministic ordering.
    #[serde(default)]
    pub nodes: BTreeMap<NodeId, Node>,
}


impl InspectorProjection {
    /// Create a new empty projection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a `$ref` to a node.
    pub fn resolve_ref(&self, ref_val: &RefValue) -> Option<&Node> {
        self.nodes.get(ref_val.target())
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Insert a node into the projection.
    pub fn insert_node(&mut self, node: Node) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Add a root reference for a chamber.
    pub fn set_root(&mut self, chamber: impl Into<String>, target: NodeId) {
        self.root.insert(chamber.into(), RefValue::new(target));
    }
}

/// Snapshot envelope metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    /// Schema version for compatibility checking.
    pub schema_version: u32,

    /// Hash of the source data (for cache invalidation).
    pub source_hash: String,

    /// Hash of the render policy used.
    pub policy_hash: String,

    /// When the projection was created (ISO8601).
    pub created_at: String,
}

impl Default for SnapshotMeta {
    fn default() -> Self {
        Self {
            schema_version: 1,
            source_hash: String::new(),
            policy_hash: String::new(),
            created_at: String::new(),
        }
    }
}

/// UI display hints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiHints {
    /// Use abbreviated labels where possible.
    #[serde(default = "default_true")]
    pub shorthand_labels: bool,

    /// Show breadcrumb navigation trail.
    #[serde(default = "default_true")]
    pub breadcrumb: bool,

    /// Enable back/forward history navigation.
    #[serde(default = "default_true")]
    pub history: bool,
}

impl Default for UiHints {
    fn default() -> Self {
        Self {
            shorthand_labels: true,
            breadcrumb: true,
            history: true,
        }
    }
}

fn default_true() -> bool {
    true
}

// ============================================================================
// NODE STRUCTURE
// ============================================================================

/// A node in the projection graph.
///
/// Nodes are the atomic units of the projection. Each has:
/// - A unique `id` (NodeId)
/// - A `kind` that determines rendering
/// - Labels for display
/// - Optional branches, links, attributes, provenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier (must match map key).
    pub id: NodeId,

    /// Node type discriminator.
    pub kind: NodeKind,

    /// Short display label (required, max ~60 chars).
    pub label_short: String,

    /// Full display label (shown on expand/detail).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_full: Option<String>,

    /// Icon/glyph identifier (emoji or icon name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph: Option<String>,

    /// Named branches containing `$ref` targets.
    /// For tree-like structures (CBU→members, matrix→slices).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub branches: BTreeMap<String, RefOrList>,

    /// Direct links to related nodes (cross-references).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<RefValue>,

    /// Summary statistics (shown in collapsed state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<NodeSummary>,

    /// Type-specific attributes as key-value pairs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,

    /// Data provenance (sources, confidence, assertions).
    /// REQUIRED for HoldingEdge and ControlEdge kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl Node {
    /// Create a new node with minimal required fields.
    pub fn new(id: NodeId, kind: NodeKind, label_short: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            label_short: label_short.into(),
            label_full: None,
            glyph: None,
            branches: BTreeMap::new(),
            links: Vec::new(),
            summary: None,
            attributes: BTreeMap::new(),
            provenance: None,
        }
    }

    /// Set the full label.
    pub fn with_label_full(mut self, label: impl Into<String>) -> Self {
        self.label_full = Some(label.into());
        self
    }

    /// Set the glyph.
    pub fn with_glyph(mut self, glyph: impl Into<String>) -> Self {
        self.glyph = Some(glyph.into());
        self
    }

    /// Add a branch with a single ref.
    pub fn with_branch(mut self, name: impl Into<String>, target: NodeId) -> Self {
        self.branches
            .insert(name.into(), RefOrList::Single(RefValue::new(target)));
        self
    }

    /// Add a branch with a paged list.
    pub fn with_branch_list(mut self, name: impl Into<String>, list: PagingList) -> Self {
        self.branches.insert(name.into(), RefOrList::List(list));
        self
    }

    /// Add a link to another node.
    pub fn with_link(mut self, target: NodeId) -> Self {
        self.links.push(RefValue::new(target));
        self
    }

    /// Set summary statistics.
    pub fn with_summary(mut self, summary: NodeSummary) -> Self {
        self.summary = Some(summary);
        self
    }

    /// Add an attribute.
    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Set provenance.
    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// Get the default glyph for this node's kind.
    pub fn default_glyph(&self) -> &'static str {
        self.kind.default_glyph()
    }

    /// Check if this node has any children (branches or links).
    pub fn has_children(&self) -> bool {
        !self.branches.is_empty()
    }
}

// ============================================================================
// NODE KIND
// ============================================================================

/// All supported node kinds in the projection.
///
/// Each kind determines:
/// - Default glyph/icon
/// - Required fields (e.g., provenance for edges)
/// - Rendering behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    // CBU Container
    #[serde(rename = "CBU")]
    Cbu,
    #[serde(rename = "MemberList")]
    MemberList,
    #[serde(rename = "Entity")]
    Entity,

    // Product Taxonomy
    #[serde(rename = "ProductTree")]
    ProductTree,
    #[serde(rename = "Product")]
    Product,
    #[serde(rename = "Service")]
    Service,
    #[serde(rename = "Resource")]
    Resource,
    #[serde(rename = "ProductBinding")]
    ProductBinding,

    // Instrument Matrix
    #[serde(rename = "InstrumentMatrix")]
    InstrumentMatrix,
    #[serde(rename = "MatrixSlice")]
    MatrixSlice,
    #[serde(rename = "SparseCellPage")]
    SparseCellPage,

    // Investor/Control Registers
    #[serde(rename = "InvestorRegister")]
    InvestorRegister,
    #[serde(rename = "HoldingEdgeList")]
    HoldingEdgeList,
    #[serde(rename = "HoldingEdge")]
    HoldingEdge,
    #[serde(rename = "ControlRegister")]
    ControlRegister,
    #[serde(rename = "ControlTree")]
    ControlTree,
    #[serde(rename = "ControlNode")]
    ControlNode,
    #[serde(rename = "ControlEdge")]
    ControlEdge,

    // Paging
    #[serde(rename = "PageToken")]
    PageToken,
}

impl NodeKind {
    /// Get the default glyph for this kind.
    pub fn default_glyph(&self) -> &'static str {
        match self {
            Self::Cbu => "🏢",
            Self::MemberList => "👥",
            Self::Entity => "👤",
            Self::ProductTree => "📦",
            Self::Product => "🧱",
            Self::Service => "🔁",
            Self::Resource => "📨",
            Self::ProductBinding => "🔗",
            Self::InstrumentMatrix => "📊",
            Self::MatrixSlice => "📋",
            Self::SparseCellPage => "📄",
            Self::InvestorRegister => "💰",
            Self::HoldingEdgeList => "📈",
            Self::HoldingEdge => "→",
            Self::ControlRegister => "🧬",
            Self::ControlTree => "🌳",
            Self::ControlNode => "●",
            Self::ControlEdge => "⬇",
            Self::PageToken => "📑",
        }
    }

    /// Check if this kind requires provenance.
    pub fn requires_provenance(&self) -> bool {
        matches!(self, Self::HoldingEdge | Self::ControlEdge)
    }

    /// Check if this kind can have child branches.
    pub fn can_have_children(&self) -> bool {
        matches!(
            self,
            Self::Cbu
                | Self::MemberList
                | Self::ProductTree
                | Self::Product
                | Self::Service
                | Self::InstrumentMatrix
                | Self::InvestorRegister
                | Self::HoldingEdgeList
                | Self::ControlRegister
                | Self::ControlTree
                | Self::ControlNode
        )
    }
}

// ============================================================================
// SUPPORTING TYPES
// ============================================================================

/// Either a single ref or a paginated list of refs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RefOrList {
    /// Single reference to a node.
    Single(RefValue),
    /// Paginated list of references.
    List(PagingList),
}

impl RefOrList {
    /// Get all target NodeIds in this branch.
    pub fn targets(&self) -> Vec<&NodeId> {
        match self {
            Self::Single(r) => vec![r.target()],
            Self::List(list) => list.items.iter().map(|r| r.target()).collect(),
        }
    }
}

/// Paginated list of node references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagingList {
    /// Pagination metadata.
    pub paging: PagingInfo,
    /// Items on this page.
    pub items: Vec<RefValue>,
}

impl PagingList {
    /// Create a new paging list.
    pub fn new(items: Vec<RefValue>, limit: usize, next: Option<NodeId>) -> Self {
        Self {
            paging: PagingInfo {
                limit,
                next,
                total: Some(items.len()),
            },
            items,
        }
    }
}

/// Pagination information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagingInfo {
    /// Maximum items per page.
    pub limit: usize,

    /// NodeId of next page (None if last page).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<NodeId>,

    /// Total count (if known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
}

/// Summary statistics for a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    /// Count of child items.
    pub item_count: usize,

    /// Status indicator (e.g., "complete", "pending").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl NodeSummary {
    /// Create a summary with just a count.
    pub fn count(n: usize) -> Self {
        Self {
            item_count: n,
            status: None,
        }
    }

    /// Create a summary with count and status.
    pub fn with_status(n: usize, status: impl Into<String>) -> Self {
        Self {
            item_count: n,
            status: Some(status.into()),
        }
    }
}

/// Data provenance information.
///
/// Required for HoldingEdge and ControlEdge nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// Source systems/documents.
    pub sources: Vec<String>,

    /// When the assertion was made (ISO8601 date).
    pub asserted_at: String,

    /// Confidence score (0.0 to 1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,

    /// Human-readable notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// References to evidence nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<RefValue>,
}

impl Provenance {
    /// Create a new provenance with required fields.
    pub fn new(sources: Vec<String>, asserted_at: impl Into<String>) -> Self {
        Self {
            sources,
            asserted_at: asserted_at.into(),
            confidence: None,
            notes: None,
            evidence_refs: Vec::new(),
        }
    }

    /// Set confidence score.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Set notes.
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let id = NodeId::new("cbu:test").unwrap();
        let node = Node::new(id.clone(), NodeKind::Cbu, "Test CBU")
            .with_glyph("🏢")
            .with_summary(NodeSummary::count(5));

        assert_eq!(node.id, id);
        assert_eq!(node.kind, NodeKind::Cbu);
        assert_eq!(node.label_short, "Test CBU");
        assert_eq!(node.glyph, Some("🏢".to_string()));
        assert_eq!(node.summary.unwrap().item_count, 5);
    }

    #[test]
    fn test_node_kind_glyphs() {
        assert_eq!(NodeKind::Cbu.default_glyph(), "🏢");
        assert_eq!(NodeKind::Entity.default_glyph(), "👤");
        assert_eq!(NodeKind::HoldingEdge.default_glyph(), "→");
    }

    #[test]
    fn test_node_kind_provenance_requirements() {
        assert!(NodeKind::HoldingEdge.requires_provenance());
        assert!(NodeKind::ControlEdge.requires_provenance());
        assert!(!NodeKind::Entity.requires_provenance());
        assert!(!NodeKind::Cbu.requires_provenance());
    }

    #[test]
    fn test_projection_operations() {
        let mut proj = InspectorProjection::new();

        let cbu_id = NodeId::new("cbu:test").unwrap();
        let entity_id = NodeId::new("entity:e1").unwrap();

        let cbu = Node::new(cbu_id.clone(), NodeKind::Cbu, "Test CBU")
            .with_branch("members", entity_id.clone());
        let entity = Node::new(entity_id.clone(), NodeKind::Entity, "Entity 1");

        proj.insert_node(cbu);
        proj.insert_node(entity);
        proj.set_root("cbu", cbu_id.clone());

        assert!(proj.get_node(&cbu_id).is_some());
        assert!(proj.get_node(&entity_id).is_some());
        assert!(proj.root.contains_key("cbu"));
    }

    #[test]
    fn test_ref_or_list_targets() {
        let id1 = NodeId::new("entity:e1").unwrap();
        let id2 = NodeId::new("entity:e2").unwrap();

        let single = RefOrList::Single(RefValue::new(id1.clone()));
        assert_eq!(single.targets().len(), 1);

        let list = RefOrList::List(PagingList::new(
            vec![RefValue::new(id1.clone()), RefValue::new(id2.clone())],
            50,
            None,
        ));
        assert_eq!(list.targets().len(), 2);
    }

    #[test]
    fn test_projection_yaml_roundtrip() {
        let yaml = r#"
snapshot:
  schema_version: 1
  source_hash: "abc123"
  policy_hash: "def456"
  created_at: "2026-02-04T00:00:00Z"
render_policy:
  lod: 2
  max_depth: 3
  max_items_per_list: 50
root:
  cbu:
    $ref: "cbu:test"
nodes:
  "cbu:test":
    id: "cbu:test"
    kind: CBU
    label_short: "Test CBU"
"#;

        let proj: InspectorProjection = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(proj.snapshot.schema_version, 1);
        assert!(proj.root.contains_key("cbu"));
        assert!(proj.nodes.contains_key(&NodeId::new("cbu:test").unwrap()));

        // Roundtrip
        let serialized = serde_yaml::to_string(&proj).unwrap();
        let reparsed: InspectorProjection = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(
            reparsed.snapshot.schema_version,
            proj.snapshot.schema_version
        );
    }
}
