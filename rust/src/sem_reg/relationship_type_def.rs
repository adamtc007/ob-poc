//! Relationship type definition body — the typed JSONB content for `ObjectType::RelationshipTypeDef`.

use serde::{Deserialize, Serialize};

/// The JSONB body stored in `definition` for relationship type definitions.
///
/// Describes a typed relationship between two entity types, with cardinality
/// and optional inverse/constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipTypeDefBody {
    /// Fully qualified name, e.g. "relationship.ownership", "relationship.custody_of"
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description of this relationship type
    pub description: String,
    /// Domain this relationship belongs to
    pub domain: String,
    /// Source entity type FQN (the "from" side)
    pub source_entity_type_fqn: String,
    /// Target entity type FQN (the "to" side)
    pub target_entity_type_fqn: String,
    /// Cardinality of the relationship
    pub cardinality: RelationshipCardinality,
    /// FQN of the inverse relationship (if bidirectional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse_fqn: Option<String>,
    /// Additional constraints on this relationship type
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
}

/// Cardinality of a relationship between entity types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipCardinality {
    OneToOne,
    OneToMany,
    ManyToMany,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relationship_type_def_serde() {
        let body = RelationshipTypeDefBody {
            fqn: "relationship.ownership".into(),
            name: "Ownership".into(),
            description: "Ownership relationship between entities".into(),
            domain: "relationship".into(),
            source_entity_type_fqn: "entity.legal_entity".into(),
            target_entity_type_fqn: "entity.fund".into(),
            cardinality: RelationshipCardinality::OneToMany,
            inverse_fqn: Some("relationship.owned_by".into()),
            constraints: vec!["ownership_pct <= 100".into()],
        };

        let json = serde_json::to_value(&body).unwrap();
        let back: RelationshipTypeDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, "relationship.ownership");
        assert_eq!(back.source_entity_type_fqn, "entity.legal_entity");
        assert_eq!(back.target_entity_type_fqn, "entity.fund");
        assert_eq!(back.cardinality, RelationshipCardinality::OneToMany);
        assert_eq!(back.inverse_fqn.as_deref(), Some("relationship.owned_by"));
        assert_eq!(back.constraints.len(), 1);
    }

    #[test]
    fn test_minimal_relationship_type_def() {
        let body = RelationshipTypeDefBody {
            fqn: "relationship.controls".into(),
            name: "Controls".into(),
            description: "Control relationship".into(),
            domain: "relationship".into(),
            source_entity_type_fqn: "entity.person".into(),
            target_entity_type_fqn: "entity.legal_entity".into(),
            cardinality: RelationshipCardinality::ManyToMany,
            inverse_fqn: None,
            constraints: vec![],
        };

        let json = serde_json::to_value(&body).unwrap();
        let back: RelationshipTypeDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, "relationship.controls");
        assert!(back.inverse_fqn.is_none());
        assert!(back.constraints.is_empty());
    }

    #[test]
    fn test_cardinality_serde_snake_case() {
        let json = serde_json::json!("one_to_one");
        let card: RelationshipCardinality = serde_json::from_value(json).unwrap();
        assert_eq!(card, RelationshipCardinality::OneToOne);

        let json = serde_json::json!("many_to_many");
        let card: RelationshipCardinality = serde_json::from_value(json).unwrap();
        assert_eq!(card, RelationshipCardinality::ManyToMany);
    }
}
