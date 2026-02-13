//! Taxonomy definition and node types for the semantic registry.
//!
//! A taxonomy is a hierarchical classification scheme. Each taxonomy
//! has a root node and an ordered tree of child nodes. Membership rules
//! (see `membership.rs`) bind attributes, verbs, and entity types to
//! taxonomy nodes for classification.

use serde::{Deserialize, Serialize};

/// Body for a taxonomy definition snapshot.
///
/// Taxonomies organise registry objects into hierarchical classification
/// axes (e.g., "KYC Risk Tier", "Instrument Asset Class", "Jurisdiction").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyDefBody {
    /// Fully qualified name, e.g. `"risk.kyc-tier"`
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Owning domain
    pub domain: String,
    /// FQN of the root node in this taxonomy
    #[serde(default)]
    pub root_node_fqn: Option<String>,
    /// Maximum permitted depth (None = unlimited)
    #[serde(default)]
    pub max_depth: Option<u32>,
    /// Classification axis label (for UI grouping)
    #[serde(default)]
    pub classification_axis: Option<String>,
}

/// Body for a taxonomy node snapshot.
///
/// Nodes form the tree structure within a taxonomy. Each node has
/// an optional parent (root nodes have `parent_fqn: None`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyNodeBody {
    /// Fully qualified name, e.g. `"risk.kyc-tier.high"`
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// FQN of the owning taxonomy
    pub taxonomy_fqn: String,
    /// FQN of the parent node (None for root nodes)
    #[serde(default)]
    pub parent_fqn: Option<String>,
    /// Sort order within siblings
    #[serde(default)]
    pub sort_order: i32,
    /// Arbitrary key-value labels for this node
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_taxonomy_def_serde() {
        let body = TaxonomyDefBody {
            fqn: "risk.kyc-tier".into(),
            name: "KYC Risk Tier".into(),
            description: "Risk classification for KYC cases".into(),
            domain: "risk".into(),
            root_node_fqn: Some("risk.kyc-tier.root".into()),
            max_depth: Some(3),
            classification_axis: Some("risk".into()),
        };
        let json = serde_json::to_value(&body).unwrap();
        let round: TaxonomyDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(round.fqn, "risk.kyc-tier");
        assert_eq!(round.max_depth, Some(3));
    }

    #[test]
    fn test_taxonomy_node_serde() {
        let body = TaxonomyNodeBody {
            fqn: "risk.kyc-tier.high".into(),
            name: "High Risk".into(),
            description: Some("High-risk KYC classification".into()),
            taxonomy_fqn: "risk.kyc-tier".into(),
            parent_fqn: Some("risk.kyc-tier.root".into()),
            sort_order: 1,
            labels: [("colour".into(), "red".into())].into_iter().collect(),
        };
        let json = serde_json::to_value(&body).unwrap();
        let round: TaxonomyNodeBody = serde_json::from_value(json).unwrap();
        assert_eq!(round.fqn, "risk.kyc-tier.high");
        assert_eq!(round.parent_fqn, Some("risk.kyc-tier.root".into()));
        assert_eq!(round.labels.get("colour"), Some(&"red".to_string()));
    }

    #[test]
    fn test_root_node_has_no_parent() {
        let root = TaxonomyNodeBody {
            fqn: "risk.kyc-tier.root".into(),
            name: "Root".into(),
            description: None,
            taxonomy_fqn: "risk.kyc-tier".into(),
            parent_fqn: None,
            sort_order: 0,
            labels: Default::default(),
        };
        assert!(root.parent_fqn.is_none());
    }
}
