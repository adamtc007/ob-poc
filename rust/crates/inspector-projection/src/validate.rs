//! Validation for Inspector projections.
//!
//! Implements pre-render validation:
//! 1. Referential integrity (all $refs resolve)
//! 2. Cycle detection (DFS with path tracking)
//! 3. Root validation (all roots exist)
//! 4. Provenance requirements (edges must have provenance)
//! 5. Confidence range validation (0.0-1.0)

use crate::error::ValidationError;
use crate::model::InspectorProjection;
use crate::node_id::NodeId;
use std::collections::HashSet;

/// Maximum supported schema version.
pub const MAX_SCHEMA_VERSION: u32 = 1;

/// Result of validating a projection.
#[derive(Debug, Default)]
pub struct ValidationResult {
    /// Blocking errors (render should not proceed).
    pub errors: Vec<ValidationError>,
    /// Non-blocking warnings (render can proceed).
    pub warnings: Vec<ValidationError>,
}

impl ValidationResult {
    /// Create an empty result.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if validation passed (no blocking errors).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Add an error or warning based on severity.
    pub fn add(&mut self, error: ValidationError) {
        if error.is_blocking() {
            self.errors.push(error);
        } else {
            self.warnings.push(error);
        }
    }

    /// Total count of issues.
    pub fn issue_count(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }
}

/// Validate an Inspector projection.
///
/// Checks:
/// 1. Schema version is supported
/// 2. All root refs exist
/// 3. All node $refs resolve
/// 4. No cycles in the graph
/// 5. Provenance present where required
/// 6. Confidence values in range
///
/// # Example
///
/// ```
/// use inspector_projection::{InspectorProjection, validate};
///
/// let projection = InspectorProjection::default();
/// let result = validate(&projection);
/// if result.is_valid() {
///     println!("Projection is valid!");
/// } else {
///     for error in &result.errors {
///         eprintln!("Error: {}", error);
///     }
/// }
/// ```
pub fn validate(projection: &InspectorProjection) -> ValidationResult {
    let mut result = ValidationResult::new();

    // 1. Schema version
    validate_schema_version(projection, &mut result);

    // 2. Node ID consistency
    validate_node_ids(projection, &mut result);

    // 3. Root references
    validate_roots(projection, &mut result);

    // 4. All $refs resolve
    validate_refs(projection, &mut result);

    // 5. Cycle detection
    validate_no_cycles(projection, &mut result);

    // 6. Provenance requirements
    validate_provenance(projection, &mut result);

    // 7. Confidence ranges
    validate_confidence(projection, &mut result);

    result
}

fn validate_schema_version(projection: &InspectorProjection, result: &mut ValidationResult) {
    if projection.snapshot.schema_version > MAX_SCHEMA_VERSION {
        result.add(ValidationError::UnsupportedSchemaVersion {
            version: projection.snapshot.schema_version,
            max_supported: MAX_SCHEMA_VERSION,
        });
    }
}

fn validate_node_ids(projection: &InspectorProjection, result: &mut ValidationResult) {
    for (key, node) in &projection.nodes {
        if key != &node.id {
            result.add(ValidationError::IdMismatch {
                key: key.clone(),
                node_id: node.id.clone(),
            });
        }
    }
}

fn validate_roots(projection: &InspectorProjection, result: &mut ValidationResult) {
    for (chamber, ref_val) in &projection.root {
        if !projection.nodes.contains_key(ref_val.target()) {
            result.add(ValidationError::MissingRoot {
                chamber: chamber.clone(),
                target: ref_val.target().clone(),
            });
        }
    }
}

fn validate_refs(projection: &InspectorProjection, result: &mut ValidationResult) {
    for (source_id, node) in &projection.nodes {
        // Check branches
        for ref_or_list in node.branches.values() {
            for target in ref_or_list.targets() {
                if !projection.nodes.contains_key(target) {
                    result.add(ValidationError::DanglingRef {
                        source_node: source_id.clone(),
                        target: target.clone(),
                    });
                }
            }
        }

        // Check links
        for link in &node.links {
            if !projection.nodes.contains_key(link.target()) {
                result.add(ValidationError::DanglingRef {
                    source_node: source_id.clone(),
                    target: link.target().clone(),
                });
            }
        }

        // Check provenance evidence refs
        if let Some(ref prov) = node.provenance {
            for evidence in &prov.evidence_refs {
                if !projection.nodes.contains_key(evidence.target()) {
                    result.add(ValidationError::DanglingRef {
                        source_node: source_id.clone(),
                        target: evidence.target().clone(),
                    });
                }
            }
        }
    }
}

fn validate_no_cycles(projection: &InspectorProjection, result: &mut ValidationResult) {
    let mut global_visited: HashSet<NodeId> = HashSet::new();

    // Start DFS from each root
    for ref_val in projection.root.values() {
        let mut path: Vec<NodeId> = Vec::new();
        let mut path_set: HashSet<NodeId> = HashSet::new();

        detect_cycles_dfs(
            ref_val.target(),
            projection,
            &mut global_visited,
            &mut path,
            &mut path_set,
            result,
        );
    }
}

fn detect_cycles_dfs(
    node_id: &NodeId,
    projection: &InspectorProjection,
    global_visited: &mut HashSet<NodeId>,
    path: &mut Vec<NodeId>,
    path_set: &mut HashSet<NodeId>,
    result: &mut ValidationResult,
) {
    // If in current path, it's a cycle
    if path_set.contains(node_id) {
        // Find where the cycle starts
        if let Some(pos) = path.iter().position(|id| id == node_id) {
            let cycle_path: Vec<NodeId> = path[pos..].to_vec();
            result.add(ValidationError::CycleDetected { path: cycle_path });
        }
        return;
    }

    // If already visited in another path, skip
    if global_visited.contains(node_id) {
        return;
    }

    // Mark as in-path
    path.push(node_id.clone());
    path_set.insert(node_id.clone());

    // Visit children
    if let Some(node) = projection.nodes.get(node_id) {
        for ref_or_list in node.branches.values() {
            for target in ref_or_list.targets() {
                detect_cycles_dfs(target, projection, global_visited, path, path_set, result);
            }
        }
    }

    // Remove from path, add to visited
    path.pop();
    path_set.remove(node_id);
    global_visited.insert(node_id.clone());
}

fn validate_provenance(projection: &InspectorProjection, result: &mut ValidationResult) {
    for (node_id, node) in &projection.nodes {
        // Check if provenance is required
        if node.kind.requires_provenance() {
            match &node.provenance {
                None => {
                    result.add(ValidationError::MissingProvenance {
                        node_id: node_id.clone(),
                        kind: node.kind,
                    });
                }
                Some(prov) => {
                    // Check sources non-empty
                    if prov.sources.is_empty() {
                        result.add(ValidationError::EmptyProvenanceSources {
                            node_id: node_id.clone(),
                        });
                    }
                    // Check asserted_at present
                    if prov.asserted_at.is_empty() {
                        result.add(ValidationError::MissingAssertedAt {
                            node_id: node_id.clone(),
                        });
                    }
                }
            }
        }
    }
}

fn validate_confidence(projection: &InspectorProjection, result: &mut ValidationResult) {
    for (node_id, node) in &projection.nodes {
        if let Some(ref prov) = node.provenance {
            if let Some(confidence) = prov.confidence {
                if !(0.0..=1.0).contains(&confidence) {
                    result.add(ValidationError::InvalidConfidence {
                        node_id: node_id.clone(),
                        value: confidence,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Node, NodeKind, Provenance};
    use crate::ref_value::RefValue;

    fn make_projection_with_nodes(nodes: Vec<Node>) -> InspectorProjection {
        let mut proj = InspectorProjection::default();
        for node in nodes {
            proj.nodes.insert(node.id.clone(), node);
        }
        proj
    }

    #[test]
    fn test_valid_projection() {
        let cbu_id = NodeId::new("cbu:test").unwrap();
        let entity_id = NodeId::new("entity:e1").unwrap();

        let cbu = Node::new(cbu_id.clone(), NodeKind::Cbu, "Test CBU")
            .with_branch("members", entity_id.clone());
        let entity = Node::new(entity_id.clone(), NodeKind::Entity, "Entity 1");

        let mut proj = make_projection_with_nodes(vec![cbu, entity]);
        proj.set_root("cbu", cbu_id);

        let result = validate(&proj);
        assert!(result.is_valid(), "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_dangling_ref() {
        let cbu_id = NodeId::new("cbu:test").unwrap();
        let missing_id = NodeId::new("entity:missing").unwrap();

        let cbu = Node::new(cbu_id.clone(), NodeKind::Cbu, "Test CBU")
            .with_branch("members", missing_id.clone());

        let mut proj = make_projection_with_nodes(vec![cbu]);
        proj.set_root("cbu", cbu_id);

        let result = validate(&proj);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::DanglingRef { .. })));
    }

    #[test]
    fn test_missing_root() {
        let proj = InspectorProjection {
            root: {
                let mut r = std::collections::BTreeMap::new();
                r.insert(
                    "cbu".to_string(),
                    RefValue::new(NodeId::new("cbu:missing").unwrap()),
                );
                r
            },
            ..Default::default()
        };

        let result = validate(&proj);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingRoot { .. })));
    }

    #[test]
    fn test_cycle_detection() {
        let a_id = NodeId::new("node:a").unwrap();
        let b_id = NodeId::new("node:b").unwrap();

        let a = Node::new(a_id.clone(), NodeKind::Entity, "A").with_branch("next", b_id.clone());
        let b = Node::new(b_id.clone(), NodeKind::Entity, "B").with_branch("next", a_id.clone());

        let mut proj = make_projection_with_nodes(vec![a, b]);
        proj.set_root("test", a_id);

        let result = validate(&proj);
        // Cycles are warnings, not errors
        assert!(result.is_valid());
        assert!(result
            .warnings
            .iter()
            .any(|e| matches!(e, ValidationError::CycleDetected { .. })));
    }

    #[test]
    fn test_missing_provenance_on_edge() {
        let edge_id = NodeId::new("control:edge:001").unwrap();
        let edge = Node::new(edge_id.clone(), NodeKind::ControlEdge, "50% control");
        // No provenance set

        let mut proj = make_projection_with_nodes(vec![edge]);
        proj.set_root("control", edge_id);

        let result = validate(&proj);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingProvenance { .. })));
    }

    #[test]
    fn test_valid_provenance_on_edge() {
        let edge_id = NodeId::new("control:edge:001").unwrap();
        let edge = Node::new(edge_id.clone(), NodeKind::ControlEdge, "50% control")
            .with_provenance(Provenance::new(vec!["gleif".to_string()], "2026-01-15"));

        let mut proj = make_projection_with_nodes(vec![edge]);
        proj.set_root("control", edge_id);

        let result = validate(&proj);
        assert!(result.is_valid(), "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_empty_provenance_sources() {
        let edge_id = NodeId::new("control:edge:001").unwrap();
        let edge = Node::new(edge_id.clone(), NodeKind::ControlEdge, "50% control")
            .with_provenance(Provenance::new(vec![], "2026-01-15")); // Empty sources

        let mut proj = make_projection_with_nodes(vec![edge]);
        proj.set_root("control", edge_id);

        let result = validate(&proj);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyProvenanceSources { .. })));
    }

    #[test]
    fn test_invalid_confidence() {
        let entity_id = NodeId::new("entity:e1").unwrap();
        let entity = Node::new(entity_id.clone(), NodeKind::Entity, "Entity 1").with_provenance(
            Provenance::new(vec!["test".to_string()], "2026-01-15").with_confidence(1.5),
        ); // Invalid

        let mut proj = make_projection_with_nodes(vec![entity]);
        proj.set_root("test", entity_id);

        let result = validate(&proj);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidConfidence { .. })));
    }

    #[test]
    fn test_id_mismatch() {
        let key_id = NodeId::new("cbu:key").unwrap();
        let node_id = NodeId::new("cbu:different").unwrap();

        let node = Node::new(node_id, NodeKind::Cbu, "Test");

        let mut proj = InspectorProjection::default();
        proj.nodes.insert(key_id.clone(), node);
        proj.set_root("cbu", key_id);

        let result = validate(&proj);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::IdMismatch { .. })));
    }

    #[test]
    fn test_unsupported_schema_version() {
        let mut proj = InspectorProjection::default();
        proj.snapshot.schema_version = 999;

        let result = validate(&proj);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::UnsupportedSchemaVersion { .. })));
    }
}
