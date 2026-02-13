//! Entity type definition body — the typed JSONB content for `ObjectType::EntityTypeDef`.

use serde::{Deserialize, Serialize};

/// The JSONB body stored in `definition` for entity type definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeDefBody {
    /// Fully qualified name, e.g. "entity.fund", "entity.person"
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description of this entity type
    pub description: String,
    /// Domain this entity type belongs to
    pub domain: String,
    /// Database table mapping (where instances live)
    #[serde(default)]
    pub db_table: Option<DbTableMapping>,
    /// Lifecycle states this entity type supports
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle_states: Vec<LifecycleStateDef>,
    /// Required attribute FQNs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_attributes: Vec<String>,
    /// Optional attribute FQNs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_attributes: Vec<String>,
    /// Parent entity type FQN (for type hierarchy)
    #[serde(default)]
    pub parent_type: Option<String>,
}

/// Database table mapping for an entity type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTableMapping {
    /// Schema name (e.g. "ob-poc", "kyc")
    pub schema: String,
    /// Table name
    pub table: String,
    /// Primary key column name
    pub primary_key: String,
    /// Name column (for display/search)
    #[serde(default)]
    pub name_column: Option<String>,
}

/// A state in the entity lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleStateDef {
    /// State name (e.g. "draft", "active", "archived")
    pub name: String,
    /// Description of this state
    #[serde(default)]
    pub description: Option<String>,
    /// Valid transitions FROM this state
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<LifecycleTransition>,
    /// Whether this is a terminal state
    #[serde(default)]
    pub terminal: bool,
}

/// A valid transition between lifecycle states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleTransition {
    /// Target state name
    pub to: String,
    /// Verb that triggers this transition
    #[serde(default)]
    pub trigger_verb: Option<String>,
    /// Guard condition (human-readable)
    #[serde(default)]
    pub guard: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_def_serde() {
        let body = EntityTypeDefBody {
            fqn: "entity.fund".into(),
            name: "Fund".into(),
            description: "An investment fund entity".into(),
            domain: "entity".into(),
            db_table: Some(DbTableMapping {
                schema: "ob-poc".into(),
                table: "entity_funds".into(),
                primary_key: "entity_id".into(),
                name_column: Some("name".into()),
            }),
            lifecycle_states: vec![
                LifecycleStateDef {
                    name: "draft".into(),
                    description: Some("Initial state".into()),
                    transitions: vec![LifecycleTransition {
                        to: "active".into(),
                        trigger_verb: Some("entity.activate".into()),
                        guard: None,
                    }],
                    terminal: false,
                },
                LifecycleStateDef {
                    name: "active".into(),
                    description: Some("Active fund".into()),
                    transitions: vec![],
                    terminal: false,
                },
            ],
            required_attributes: vec!["entity.name".into(), "entity.jurisdiction".into()],
            optional_attributes: vec!["entity.lei_code".into()],
            parent_type: Some("entity.legal_entity".into()),
        };

        let json = serde_json::to_value(&body).unwrap();
        let back: EntityTypeDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, "entity.fund");
        assert_eq!(back.lifecycle_states.len(), 2);
        assert_eq!(back.required_attributes.len(), 2);
    }

    #[test]
    fn test_minimal_entity_type_def() {
        let body = EntityTypeDefBody {
            fqn: "entity.person".into(),
            name: "Person".into(),
            description: "A natural person".into(),
            domain: "entity".into(),
            db_table: None,
            lifecycle_states: vec![],
            required_attributes: vec![],
            optional_attributes: vec![],
            parent_type: None,
        };

        let json = serde_json::to_value(&body).unwrap();
        let back: EntityTypeDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, "entity.person");
        assert!(back.db_table.is_none());
    }
}
