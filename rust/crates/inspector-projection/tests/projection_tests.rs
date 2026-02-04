//! Integration tests for inspector-projection crate
//!
//! Tests cover:
//! - NodeId parsing and validation
//! - RefValue serialization
//! - YAML fixture loading
//! - Validation (refs, cycles, provenance, confidence)

use inspector_projection::{
    validate, InspectorProjection, Node, NodeId, NodeKind, Provenance, RefOrList, RefValue,
    RenderPolicy, ValidationError,
};

// ============================================
// NodeId Tests
// ============================================

#[test]
fn test_node_id_valid_simple() {
    let id = NodeId::new("cbu:allianz-ie").unwrap();
    assert_eq!(id.kind(), "cbu");
    assert_eq!(id.qualifier(), "allianz-ie");
    assert_eq!(id.depth(), 1);
}

#[test]
fn test_node_id_valid_with_subpath() {
    let id = NodeId::new("matrix:focus:mic:XLON").unwrap();
    assert_eq!(id.kind(), "matrix");
    assert_eq!(id.qualifier(), "focus:mic:XLON");
    assert_eq!(id.depth(), 3);
}

#[test]
fn test_node_id_preserves_uppercase_mic() {
    // MIC codes like XLON, XETR should be preserved
    let id = NodeId::new("matrixslice:XLON").unwrap();
    assert_eq!(id.as_str(), "matrixslice:XLON");
}

#[test]
fn test_node_id_invalid_uppercase_prefix() {
    // Prefix must be lowercase
    let result = NodeId::new("CBU:allianz");
    assert!(result.is_err());
}

#[test]
fn test_node_id_invalid_no_colon() {
    let result = NodeId::new("invalid");
    assert!(result.is_err());
}

#[test]
fn test_node_id_invalid_empty_qualifier() {
    let result = NodeId::new("cbu:");
    assert!(result.is_err());
}

#[test]
fn test_node_id_child() {
    let parent = NodeId::new("cbu:allianz").unwrap();
    let child = parent.child("members");
    assert_eq!(child.as_str(), "cbu:allianz:members");
}

#[test]
fn test_node_id_parent() {
    let child = NodeId::new("cbu:allianz:members:item1").unwrap();
    let parent = child.parent().unwrap();
    assert_eq!(parent.as_str(), "cbu:allianz:members");
}

#[test]
fn test_node_id_parent_of_root() {
    let root = NodeId::new("cbu:allianz").unwrap();
    assert!(root.parent().is_none());
}

// ============================================
// RefValue Tests
// ============================================

#[test]
fn test_ref_value_serialization() {
    let node_id = NodeId::new("cbu:test").unwrap();
    let ref_val = RefValue::new(node_id);
    let json = serde_json::to_string(&ref_val).unwrap();
    assert_eq!(json, r#"{"$ref":"cbu:test"}"#);
}

#[test]
fn test_ref_value_deserialization() {
    let json = r#"{"$ref":"entity:uuid-123"}"#;
    let ref_val: RefValue = serde_json::from_str(json).unwrap();
    assert_eq!(ref_val.target().as_str(), "entity:uuid-123");
}

// ============================================
// YAML Fixture Loading Tests
// ============================================

const SAMPLE_YAML: &str = include_str!("fixtures/sample.yaml");
const INVALID_YAML: &str = include_str!("fixtures/invalid_refs.yaml");

#[test]
fn test_load_sample_yaml() {
    let projection: InspectorProjection =
        serde_yaml::from_str(SAMPLE_YAML).expect("Failed to parse sample.yaml");

    // Check snapshot metadata
    assert_eq!(projection.snapshot.schema_version, 1);
    assert_eq!(projection.snapshot.source_hash, "abc123def456");

    // Check root exists
    assert!(projection.root.contains_key("cbu"));

    // Check nodes count (should have many nodes)
    assert!(projection.nodes.len() > 10);
}

#[test]
fn test_sample_yaml_has_all_node_kinds() {
    let projection: InspectorProjection = serde_yaml::from_str(SAMPLE_YAML).unwrap();

    // Collect all node kinds present
    let kinds: Vec<NodeKind> = projection.nodes.values().map(|n| n.kind).collect();

    // Check key kinds are present
    assert!(kinds.contains(&NodeKind::Cbu));
    assert!(kinds.contains(&NodeKind::MemberList));
    assert!(kinds.contains(&NodeKind::Entity));
    assert!(kinds.contains(&NodeKind::ProductTree));
    assert!(kinds.contains(&NodeKind::Product));
    assert!(kinds.contains(&NodeKind::InstrumentMatrix));
    assert!(kinds.contains(&NodeKind::MatrixSlice));
    assert!(kinds.contains(&NodeKind::InvestorRegister));
    assert!(kinds.contains(&NodeKind::HoldingEdgeList));
    assert!(kinds.contains(&NodeKind::HoldingEdge));
    assert!(kinds.contains(&NodeKind::ControlRegister));
    assert!(kinds.contains(&NodeKind::ControlTree));
    assert!(kinds.contains(&NodeKind::ControlNode));
    assert!(kinds.contains(&NodeKind::ControlEdge));
}

#[test]
fn test_sample_yaml_validates_successfully() {
    let projection: InspectorProjection = serde_yaml::from_str(SAMPLE_YAML).unwrap();
    let result = validate(&projection);

    // Should have no blocking errors
    let blocking_errors: Vec<_> = result.errors.iter().filter(|e| e.is_blocking()).collect();
    assert!(
        blocking_errors.is_empty(),
        "Unexpected blocking errors: {:?}",
        blocking_errors
    );
}

// ============================================
// Validation Tests
// ============================================

#[test]
fn test_validation_detects_dangling_refs() {
    let projection: InspectorProjection = serde_yaml::from_str(INVALID_YAML).unwrap();
    let result = validate(&projection);

    // Should detect dangling refs
    let dangling_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| matches!(e, ValidationError::DanglingRef { .. }))
        .collect();

    assert!(
        dangling_errors.len() >= 2,
        "Expected at least 2 dangling ref errors, got {:?}",
        dangling_errors
    );
}

#[test]
fn test_validation_detects_missing_provenance() {
    let projection: InspectorProjection = serde_yaml::from_str(INVALID_YAML).unwrap();
    let result = validate(&projection);

    // Should detect missing provenance on HoldingEdge and ControlEdge
    let provenance_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| matches!(e, ValidationError::MissingProvenance { .. }))
        .collect();

    assert!(
        provenance_errors.len() >= 2,
        "Expected at least 2 missing provenance errors, got {:?}",
        provenance_errors
    );
}

#[test]
fn test_validation_detects_invalid_confidence() {
    let projection: InspectorProjection = serde_yaml::from_str(INVALID_YAML).unwrap();
    let result = validate(&projection);

    // Should detect confidence > 1.0 and < 0.0
    let confidence_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| matches!(e, ValidationError::InvalidConfidence { .. }))
        .collect();

    assert!(
        confidence_errors.len() >= 2,
        "Expected at least 2 invalid confidence errors, got {:?}",
        confidence_errors
    );
}

#[test]
fn test_validation_error_codes() {
    // Check error codes match expected format
    let dangling = ValidationError::DanglingRef {
        source_node: NodeId::new("cbu:test").unwrap(),
        target: NodeId::new("entity:missing").unwrap(),
    };
    assert_eq!(dangling.code(), "DANGLING_REF");

    let missing_root = ValidationError::MissingRoot {
        chamber: "cbu".to_string(),
        target: NodeId::new("cbu:missing").unwrap(),
    };
    assert_eq!(missing_root.code(), "MISSING_ROOT");

    let cycle = ValidationError::CycleDetected {
        path: vec![
            NodeId::new("a:1").unwrap(),
            NodeId::new("a:2").unwrap(),
            NodeId::new("a:1").unwrap(),
        ],
    };
    assert_eq!(cycle.code(), "CYCLE_DETECTED");

    let missing_prov = ValidationError::MissingProvenance {
        node_id: NodeId::new("holdingedge:test").unwrap(),
        kind: NodeKind::HoldingEdge,
    };
    assert_eq!(missing_prov.code(), "MISSING_PROVENANCE");
}

#[test]
fn test_cycles_are_warnings_not_blocking() {
    let cycle_error = ValidationError::CycleDetected {
        path: vec![NodeId::new("a:1").unwrap()],
    };
    assert!(
        !cycle_error.is_blocking(),
        "Cycles should be warnings, not blocking errors"
    );
}

// ============================================
// RenderPolicy Tests
// ============================================

#[test]
fn test_render_policy_lod_field_visibility() {
    let policy = RenderPolicy {
        lod: 0,
        ..Default::default()
    };

    // LOD 0: Only glyph and ID
    assert!(policy.field_visible("glyph"));
    assert!(policy.field_visible("id"));
    assert!(!policy.field_visible("label_short"));
    assert!(!policy.field_visible("attributes"));
    assert!(!policy.field_visible("provenance"));
}

#[test]
fn test_render_policy_lod_1_visibility() {
    let policy = RenderPolicy {
        lod: 1,
        ..Default::default()
    };

    // LOD 1: + label_short
    assert!(policy.field_visible("glyph"));
    assert!(policy.field_visible("id"));
    assert!(policy.field_visible("label_short"));
    assert!(!policy.field_visible("label_full"));
    assert!(!policy.field_visible("attributes"));
}

#[test]
fn test_render_policy_lod_2_visibility() {
    let policy = RenderPolicy {
        lod: 2,
        ..Default::default()
    };

    // LOD 2: + tags, summary
    assert!(policy.field_visible("tags"));
    assert!(policy.field_visible("summary"));
    assert!(!policy.field_visible("attributes"));
    assert!(!policy.field_visible("provenance"));
}

#[test]
fn test_render_policy_lod_3_visibility() {
    let policy = RenderPolicy {
        lod: 3,
        ..Default::default()
    };

    // LOD 3: Everything visible
    assert!(policy.field_visible("glyph"));
    assert!(policy.field_visible("label_full"));
    assert!(policy.field_visible("attributes"));
    assert!(policy.field_visible("provenance"));
}

#[test]
fn test_render_policy_hash_changes_with_lod() {
    let policy1 = RenderPolicy {
        lod: 1,
        ..Default::default()
    };
    let policy2 = RenderPolicy {
        lod: 2,
        ..Default::default()
    };

    assert_ne!(
        policy1.policy_hash(),
        policy2.policy_hash(),
        "Different LOD should produce different hash"
    );
}

// ============================================
// Node Builder Tests
// ============================================

#[test]
fn test_node_builder_pattern() {
    let node = Node::new(
        NodeId::new("entity:test").unwrap(),
        NodeKind::Entity,
        "Test Entity",
    )
    .with_label_full("Test Entity Full Name")
    .with_glyph("company");

    assert_eq!(node.label_short, "Test Entity");
    assert_eq!(node.label_full.as_deref(), Some("Test Entity Full Name"));
    assert_eq!(node.glyph.as_deref(), Some("company"));
}

#[test]
fn test_node_with_branches() {
    let node = Node::new(NodeId::new("cbu:test").unwrap(), NodeKind::Cbu, "Test CBU")
        .with_branch("members", NodeId::new("memberlist:test").unwrap());

    assert!(node.branches.contains_key("members"));
    match node.branches.get("members").unwrap() {
        RefOrList::Single(ref_val) => {
            assert_eq!(ref_val.target().as_str(), "memberlist:test");
        }
        _ => panic!("Expected single ref"),
    }
}

#[test]
fn test_node_with_provenance() {
    let node = Node::new(
        NodeId::new("holdingedge:test").unwrap(),
        NodeKind::HoldingEdge,
        "Test Edge",
    )
    .with_provenance(
        Provenance::new(vec!["gleif".to_string()], "2026-01-15").with_confidence(0.95),
    );

    assert!(node.provenance.is_some());
    let prov = node.provenance.unwrap();
    assert_eq!(prov.sources, vec!["gleif"]);
    assert_eq!(prov.confidence, Some(0.95));
}

// ============================================
// Round-trip Tests
// ============================================

#[test]
fn test_projection_json_roundtrip() {
    let projection: InspectorProjection = serde_yaml::from_str(SAMPLE_YAML).unwrap();

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&projection).unwrap();

    // Deserialize back
    let restored: InspectorProjection = serde_json::from_str(&json).unwrap();

    // Check key fields match
    assert_eq!(
        projection.snapshot.schema_version,
        restored.snapshot.schema_version
    );
    assert_eq!(
        projection.snapshot.source_hash,
        restored.snapshot.source_hash
    );
    assert_eq!(projection.nodes.len(), restored.nodes.len());
}

#[test]
fn test_node_id_serde_roundtrip() {
    let id = NodeId::new("matrix:focus:mic:XLON").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    let restored: NodeId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, restored);
}

// ============================================
// Edge Case Tests
// ============================================

#[test]
fn test_empty_projection_validates() {
    let projection = InspectorProjection::default();
    let result = validate(&projection);
    // Empty projection is valid (no roots, no nodes)
    assert!(result.is_valid());
}

#[test]
fn test_node_kind_requires_provenance() {
    assert!(NodeKind::HoldingEdge.requires_provenance());
    assert!(NodeKind::ControlEdge.requires_provenance());
    assert!(!NodeKind::Cbu.requires_provenance());
    assert!(!NodeKind::Entity.requires_provenance());
    assert!(!NodeKind::Product.requires_provenance());
}

#[test]
fn test_node_kind_default_glyphs() {
    assert_eq!(NodeKind::Cbu.default_glyph(), "🏢");
    assert_eq!(NodeKind::Entity.default_glyph(), "👤");
    assert_eq!(NodeKind::HoldingEdge.default_glyph(), "→");
    assert_eq!(NodeKind::ControlEdge.default_glyph(), "⬇");
}
