//! AST Definitions for CBU Model DSL
//!
//! These structures represent the parsed CBU Model specification.
//! They are purely conceptual and do not contain any SQL or database logic.

use serde::{Deserialize, Serialize};

/// Complete CBU Model specification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbuModel {
    /// Unique identifier for this model (e.g., "CBU.GENERIC")
    pub id: String,
    /// Version string (e.g., "1.0")
    pub version: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Entity types this model applies to (e.g., ["Fund", "SPV"])
    pub applies_to: Vec<String>,
    /// Attribute specifications grouped by category
    pub attributes: CbuAttributesSpec,
    /// State machine definition
    pub states: CbuStateMachine,
    /// Role requirements
    pub roles: Vec<CbuRoleSpec>,
}

/// Attribute specifications for a CBU Model
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbuAttributesSpec {
    /// Groups of attributes (e.g., "core", "contact", "ubo")
    pub groups: Vec<CbuAttributeGroup>,
}

impl CbuAttributesSpec {
    /// Get all required attribute IDs across all groups
    pub fn all_required(&self) -> Vec<&str> {
        self.groups
            .iter()
            .flat_map(|g| g.required.iter().map(|s| s.as_str()))
            .collect()
    }

    /// Get all optional attribute IDs across all groups
    pub fn all_optional(&self) -> Vec<&str> {
        self.groups
            .iter()
            .flat_map(|g| g.optional.iter().map(|s| s.as_str()))
            .collect()
    }

    /// Get all attribute IDs (required + optional)
    pub fn all_attributes(&self) -> Vec<&str> {
        let mut attrs = self.all_required();
        attrs.extend(self.all_optional());
        attrs
    }

    /// Find group by name
    pub fn get_group(&self, name: &str) -> Option<&CbuAttributeGroup> {
        self.groups.iter().find(|g| g.name == name)
    }
}

/// A group of related attributes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbuAttributeGroup {
    /// Group name (e.g., "core", "contact", "ubo")
    pub name: String,
    /// Required attribute IDs (dictionary names)
    pub required: Vec<String>,
    /// Optional attribute IDs (dictionary names)
    pub optional: Vec<String>,
}

impl CbuAttributeGroup {
    /// Check if an attribute is in this group
    pub fn contains(&self, attr_id: &str) -> bool {
        self.required.iter().any(|a| a == attr_id) || self.optional.iter().any(|a| a == attr_id)
    }

    /// Check if an attribute is required in this group
    pub fn is_required(&self, attr_id: &str) -> bool {
        self.required.iter().any(|a| a == attr_id)
    }
}

/// State machine definition for CBU lifecycle
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbuStateMachine {
    /// Initial state name
    pub initial: String,
    /// Final/terminal state names
    pub finals: Vec<String>,
    /// All defined states
    pub states: Vec<CbuState>,
    /// Valid transitions between states
    pub transitions: Vec<CbuTransition>,
}

impl CbuStateMachine {
    /// Check if a state is a final state
    pub fn is_final(&self, state: &str) -> bool {
        self.finals.iter().any(|s| s == state)
    }

    /// Get a state definition by name
    pub fn get_state(&self, name: &str) -> Option<&CbuState> {
        self.states.iter().find(|s| s.name == name)
    }

    /// Get all valid transitions from a given state
    pub fn transitions_from(&self, state: &str) -> Vec<&CbuTransition> {
        self.transitions
            .iter()
            .filter(|t| t.from == state)
            .collect()
    }

    /// Check if a transition is valid
    pub fn is_valid_transition(&self, from: &str, to: &str) -> bool {
        self.transitions
            .iter()
            .any(|t| t.from == from && t.to == to)
    }

    /// Get the transition between two states
    pub fn get_transition(&self, from: &str, to: &str) -> Option<&CbuTransition> {
        self.transitions
            .iter()
            .find(|t| t.from == from && t.to == to)
    }

    /// Get all transitions for a given verb
    pub fn transitions_for_verb(&self, verb: &str) -> Vec<&CbuTransition> {
        self.transitions.iter().filter(|t| t.verb == verb).collect()
    }

    /// Find a single transition by verb (returns first match)
    pub fn find_transition_by_verb(&self, verb: &str) -> Option<&CbuTransition> {
        self.transitions.iter().find(|t| t.verb == verb)
    }
}

impl CbuModel {
    /// Get a chunk (attribute group) by name
    pub fn get_chunk(&self, name: &str) -> Option<&CbuAttributeGroup> {
        self.attributes.get_group(name)
    }

    /// Find transition by verb
    pub fn find_transition_by_verb(&self, verb: &str) -> Option<&CbuTransition> {
        self.states.find_transition_by_verb(verb)
    }

    /// Get all attributes required for a set of chunks
    pub fn get_chunk_attributes(&self, chunk_names: &[String]) -> Vec<&str> {
        let mut attrs = Vec::new();
        for name in chunk_names {
            if let Some(chunk) = self.get_chunk(name) {
                attrs.extend(chunk.required.iter().map(|s| s.as_str()));
                attrs.extend(chunk.optional.iter().map(|s| s.as_str()));
            }
        }
        attrs
    }

    /// Get all required attributes for a set of chunks
    pub fn get_chunk_required_attributes(&self, chunk_names: &[String]) -> Vec<&str> {
        let mut attrs = Vec::new();
        for name in chunk_names {
            if let Some(chunk) = self.get_chunk(name) {
                attrs.extend(chunk.required.iter().map(|s| s.as_str()));
            }
        }
        attrs
    }
}

/// A single state in the CBU lifecycle
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbuState {
    /// State name (e.g., "Proposed", "Active")
    pub name: String,
    /// Human-readable description
    pub description: Option<String>,
}

/// A transition between states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbuTransition {
    /// Source state
    pub from: String,
    /// Target state
    pub to: String,
    /// Verb that triggers this transition (e.g., "cbu.submit")
    pub verb: String,
    /// Attribute chunks that must be complete for this transition (e.g., ["core", "contact"])
    pub chunks: Vec<String>,
    /// Attribute IDs that must be present for this transition
    pub preconditions: Vec<String>,
}

impl CbuTransition {
    /// Check if all preconditions are satisfied
    pub fn check_preconditions(&self, present_attrs: &[&str]) -> Vec<&str> {
        self.preconditions
            .iter()
            .filter(|p| !present_attrs.contains(&p.as_str()))
            .map(|s| s.as_str())
            .collect()
    }
}

/// Role requirement specification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbuRoleSpec {
    /// Role name (e.g., "BeneficialOwner")
    pub name: String,
    /// Minimum number of entities with this role
    pub min: u32,
    /// Maximum number of entities with this role (None = unlimited)
    pub max: Option<u32>,
}

impl CbuRoleSpec {
    /// Check if a count satisfies this role's constraints
    pub fn is_satisfied(&self, count: u32) -> bool {
        count >= self.min && self.max.is_none_or(|max| count <= max)
    }

    /// Check if more entities can be added with this role
    pub fn can_add(&self, current_count: u32) -> bool {
        self.max.is_none_or(|max| current_count < max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attributes_spec_all_required() {
        let spec = CbuAttributesSpec {
            groups: vec![
                CbuAttributeGroup {
                    name: "core".to_string(),
                    required: vec!["ATTR1".to_string(), "ATTR2".to_string()],
                    optional: vec!["ATTR3".to_string()],
                },
                CbuAttributeGroup {
                    name: "contact".to_string(),
                    required: vec!["ATTR4".to_string()],
                    optional: vec![],
                },
            ],
        };

        let required = spec.all_required();
        assert_eq!(required.len(), 3);
        assert!(required.contains(&"ATTR1"));
        assert!(required.contains(&"ATTR2"));
        assert!(required.contains(&"ATTR4"));
    }

    #[test]
    fn test_state_machine_transitions() {
        let sm = CbuStateMachine {
            initial: "Proposed".to_string(),
            finals: vec!["Closed".to_string()],
            states: vec![
                CbuState {
                    name: "Proposed".to_string(),
                    description: None,
                },
                CbuState {
                    name: "Active".to_string(),
                    description: None,
                },
                CbuState {
                    name: "Closed".to_string(),
                    description: None,
                },
            ],
            transitions: vec![
                CbuTransition {
                    from: "Proposed".to_string(),
                    to: "Active".to_string(),
                    verb: "cbu.approve".to_string(),
                    chunks: vec!["core".to_string()],
                    preconditions: vec!["ATTR1".to_string()],
                },
                CbuTransition {
                    from: "Active".to_string(),
                    to: "Closed".to_string(),
                    verb: "cbu.close".to_string(),
                    chunks: vec![],
                    preconditions: vec![],
                },
            ],
        };

        assert!(sm.is_valid_transition("Proposed", "Active"));
        assert!(!sm.is_valid_transition("Proposed", "Closed"));
        assert!(sm.is_final("Closed"));
        assert!(!sm.is_final("Active"));
    }

    #[test]
    fn test_transition_preconditions() {
        let transition = CbuTransition {
            from: "A".to_string(),
            to: "B".to_string(),
            verb: "test".to_string(),
            chunks: vec!["core".to_string()],
            preconditions: vec![
                "ATTR1".to_string(),
                "ATTR2".to_string(),
                "ATTR3".to_string(),
            ],
        };

        let missing = transition.check_preconditions(&["ATTR1", "ATTR3"]);
        assert_eq!(missing, vec!["ATTR2"]);
    }

    #[test]
    fn test_role_spec_constraints() {
        let role = CbuRoleSpec {
            name: "BeneficialOwner".to_string(),
            min: 1,
            max: Some(10),
        };

        assert!(!role.is_satisfied(0));
        assert!(role.is_satisfied(1));
        assert!(role.is_satisfied(10));
        assert!(!role.is_satisfied(11));
        assert!(role.can_add(9));
        assert!(!role.can_add(10));
    }
}
