//! Entity taxonomy loader and accessor.
//!
//! Loads entity definitions from `config/ontology/entity_taxonomy.yaml` and provides
//! lookup methods for entity types, FK relationships, and implicit creation config.

use crate::ontology::types::{EntityDef, EntityTaxonomyConfig, FkRelationship};
use std::collections::HashMap;
use std::path::Path;

/// Entity taxonomy loaded from configuration.
#[derive(Debug, Clone)]
pub struct EntityTaxonomy {
    /// Entity definitions keyed by type name
    entities: HashMap<String, EntityDef>,

    /// FK relationships
    fk_relationships: Vec<FkRelationship>,

    /// Index: (parent_category, child_category) -> fk_arg
    fk_index: HashMap<(String, String), String>,

    /// Reference tables
    reference_tables: HashMap<String, crate::ontology::types::ReferenceTableDef>,
}

impl EntityTaxonomy {
    /// Load taxonomy from a YAML file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref())?;
        Self::from_yaml(&content)
    }

    /// Parse taxonomy from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config: EntityTaxonomyConfig = serde_yaml::from_str(yaml)?;

        // Build FK index for quick lookup
        let mut fk_index = HashMap::new();
        for rel in &config.relationships {
            fk_index.insert((rel.parent.clone(), rel.child.clone()), rel.fk_arg.clone());
        }

        Ok(Self {
            entities: config.entities,
            fk_relationships: config.relationships,
            fk_index,
            reference_tables: config.reference_tables,
        })
    }

    /// Get an entity definition by type name.
    pub fn get(&self, entity_type: &str) -> Option<&EntityDef> {
        self.entities.get(entity_type)
    }

    /// Get FK argument for a parent-child relationship.
    ///
    /// This replaces the hardcoded `PARENT_FK_MAP` in execution_plan.rs.
    /// Looks up by exact type match first, then by category.
    pub fn get_fk(&self, parent: &str, child: &str) -> Option<&str> {
        // Try exact match first
        if let Some(fk) = self.fk_index.get(&(parent.to_string(), child.to_string())) {
            return Some(fk.as_str());
        }

        // Try parent type -> child category
        if let Some(child_def) = self.entities.get(child) {
            let child_category = &child_def.category;
            if let Some(fk) = self
                .fk_index
                .get(&(parent.to_string(), child_category.clone()))
            {
                return Some(fk.as_str());
            }
        }

        // Try parent category -> child category
        if let Some(parent_def) = self.entities.get(parent) {
            let parent_category = &parent_def.category;
            if let Some(child_def) = self.entities.get(child) {
                let child_category = &child_def.category;
                if let Some(fk) = self
                    .fk_index
                    .get(&(parent_category.clone(), child_category.clone()))
                {
                    return Some(fk.as_str());
                }
            }
        }

        None
    }

    /// Get all entity type names.
    pub fn entity_types(&self) -> impl Iterator<Item = &str> {
        self.entities.keys().map(|s| s.as_str())
    }

    /// Check if implicit creation is allowed for an entity type.
    pub fn allows_implicit_create(&self, entity_type: &str) -> bool {
        self.entities
            .get(entity_type)
            .and_then(|e| e.implicit_create.as_ref())
            .map(|ic| ic.allowed)
            .unwrap_or(false)
    }

    /// Get the canonical creator verb for an entity type.
    ///
    /// Returns the verb name (e.g., "cbu.create") or a pattern (e.g., "entity.create-{subtype}").
    pub fn canonical_creator(&self, entity_type: &str, subtype: Option<&str>) -> Option<String> {
        let entity = self.entities.get(entity_type)?;
        let ic = entity.implicit_create.as_ref()?;

        if !ic.allowed {
            return None;
        }

        // Direct verb name
        if let Some(verb) = &ic.canonical_verb {
            return Some(verb.clone());
        }

        // Pattern with subtype substitution
        if let Some(pattern) = &ic.canonical_verb_pattern {
            if let Some(st) = subtype {
                return Some(pattern.replace("{subtype}", st));
            }
        }

        None
    }

    /// Get required arguments for implicit creation.
    pub fn implicit_create_required_args(&self, entity_type: &str) -> Vec<&str> {
        self.entities
            .get(entity_type)
            .and_then(|e| e.implicit_create.as_ref())
            .map(|ic| ic.required_args.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get the lifecycle for an entity type.
    pub fn get_lifecycle(
        &self,
        entity_type: &str,
    ) -> Option<&crate::ontology::types::EntityLifecycle> {
        // Handle aliases
        let resolved_type = self
            .entities
            .get(entity_type)
            .and_then(|e| e.alias_for.as_ref())
            .map(|s| s.as_str())
            .unwrap_or(entity_type);

        self.entities
            .get(resolved_type)
            .and_then(|e| e.lifecycle.as_ref())
    }

    /// Get the initial state for an entity type.
    pub fn initial_state(&self, entity_type: &str) -> Option<&str> {
        self.get_lifecycle(entity_type)
            .map(|lc| lc.initial_state.as_str())
    }

    /// Get the status column for an entity type.
    pub fn status_column(&self, entity_type: &str) -> Option<&str> {
        self.get_lifecycle(entity_type)
            .map(|lc| lc.status_column.as_str())
    }

    /// Get all FK relationships.
    pub fn relationships(&self) -> &[FkRelationship] {
        &self.fk_relationships
    }

    /// Get a reference table definition.
    pub fn get_reference_table(
        &self,
        name: &str,
    ) -> Option<&crate::ontology::types::ReferenceTableDef> {
        self.reference_tables.get(name)
    }

    /// Get the DB config for an entity type.
    pub fn get_db_config(
        &self,
        entity_type: &str,
    ) -> Option<&crate::ontology::types::EntityDbConfig> {
        self.entities.get(entity_type).map(|e| &e.db)
    }

    /// Resolve an entity type alias to its canonical type.
    pub fn resolve_alias<'a>(&'a self, entity_type: &'a str) -> &'a str {
        self.entities
            .get(entity_type)
            .and_then(|e| e.alias_for.as_ref())
            .map(|s| s.as_str())
            .unwrap_or(entity_type)
    }

    /// Get the parent type for a subtype (e.g., proper_person -> entity).
    pub fn parent_type(&self, entity_type: &str) -> Option<&str> {
        self.entities
            .get(entity_type)
            .and_then(|e| e.parent_type.as_ref())
            .map(|s| s.as_str())
    }

    /// Check if an entity type is a subtype of another.
    pub fn is_subtype_of(&self, entity_type: &str, parent: &str) -> bool {
        let mut current = entity_type;
        while let Some(p) = self.parent_type(current) {
            if p == parent {
                return true;
            }
            current = p;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_YAML: &str = r#"
version: "1.0"
entities:
  cbu:
    description: "Client Business Unit"
    category: subject
    db:
      schema: ob-poc
      table: cbus
      pk: cbu_id
    lifecycle:
      status_column: status
      states: [DRAFT, ACTIVE]
      transitions:
        - from: DRAFT
          to: [ACTIVE]
      initial_state: DRAFT
    implicit_create:
      allowed: true
      canonical_verb: cbu.create
      required_args: [name, jurisdiction]
  entity:
    description: "Legal/natural person"
    category: entity
    db:
      schema: ob-poc
      table: entities
      pk: entity_id
    implicit_create:
      allowed: true
      canonical_verb_pattern: "entity.create-{subtype}"
      required_args: [name]
relationships:
  - parent: cbu
    child: document
    fk_arg: cbu-id
reference_tables: {}
"#;

    #[test]
    fn test_load_taxonomy() {
        let taxonomy = EntityTaxonomy::from_yaml(TEST_YAML).unwrap();
        assert!(taxonomy.get("cbu").is_some());
        assert!(taxonomy.get("entity").is_some());
    }

    #[test]
    fn test_get_fk() {
        let taxonomy = EntityTaxonomy::from_yaml(TEST_YAML).unwrap();
        assert_eq!(taxonomy.get_fk("cbu", "document"), Some("cbu-id"));
    }

    #[test]
    fn test_canonical_creator() {
        let taxonomy = EntityTaxonomy::from_yaml(TEST_YAML).unwrap();
        assert_eq!(
            taxonomy.canonical_creator("cbu", None),
            Some("cbu.create".to_string())
        );
        assert_eq!(
            taxonomy.canonical_creator("entity", Some("proper_person")),
            Some("entity.create-proper_person".to_string())
        );
    }

    #[test]
    fn test_lifecycle() {
        let taxonomy = EntityTaxonomy::from_yaml(TEST_YAML).unwrap();
        let lifecycle = taxonomy.get_lifecycle("cbu").unwrap();
        assert_eq!(lifecycle.initial_state, "DRAFT");
        assert!(lifecycle.is_valid_transition("DRAFT", "ACTIVE"));
        assert!(!lifecycle.is_valid_transition("ACTIVE", "DRAFT"));
    }
}
