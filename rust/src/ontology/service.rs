//! OntologyService - Global singleton for ontology access.
//!
//! Provides a thread-safe, lazily-loaded global accessor for the entity taxonomy
//! and verb lifecycle information.

use crate::dsl_v2::config::types::VerbLifecycle;
use crate::ontology::taxonomy::EntityTaxonomy;
use crate::ontology::types::{EntityDef, EntityLifecycle};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// Global ontology instance.
static ONTOLOGY: OnceLock<Arc<OntologyService>> = OnceLock::new();

/// Service for accessing entity taxonomy and lifecycle information.
#[derive(Debug)]
pub struct OntologyService {
    taxonomy: EntityTaxonomy,
}

impl OntologyService {
    /// Load ontology from the default config path.
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_path();
        let taxonomy = EntityTaxonomy::load(&config_path)?;
        Ok(Self { taxonomy })
    }

    /// Load ontology from a specific path.
    pub fn load_from<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let taxonomy = EntityTaxonomy::load(path)?;
        Ok(Self { taxonomy })
    }

    /// Get the global ontology service instance.
    ///
    /// Lazily loads from the default config path on first access.
    pub fn global() -> &'static Arc<OntologyService> {
        ONTOLOGY.get_or_init(|| {
            Arc::new(
                Self::load().expect(
                    "Failed to load entity taxonomy from config/ontology/entity_taxonomy.yaml",
                ),
            )
        })
    }

    /// Initialize the global ontology with a custom instance.
    ///
    /// This is useful for testing with custom configurations.
    /// Returns error if already initialized.
    pub fn init_global(service: OntologyService) -> Result<(), OntologyService> {
        ONTOLOGY
            .set(Arc::new(service))
            .map_err(|arc| Arc::try_unwrap(arc).unwrap_or_else(|_| panic!("Failed to unwrap Arc")))
    }

    /// Get the default config path for entity taxonomy.
    fn config_path() -> PathBuf {
        // Check environment variable first
        if let Ok(dir) = std::env::var("DSL_CONFIG_DIR") {
            return PathBuf::from(dir).join("ontology/entity_taxonomy.yaml");
        }

        // Try relative to current directory
        let relative = PathBuf::from("config/ontology/entity_taxonomy.yaml");
        if relative.exists() {
            return relative;
        }

        // Try from rust/ subdirectory (when running from project root)
        let from_root = PathBuf::from("rust/config/ontology/entity_taxonomy.yaml");
        if from_root.exists() {
            return from_root;
        }

        // Default to relative path (will error on load if not found)
        relative
    }

    // =========================================================================
    // Entity accessors
    // =========================================================================

    /// Get an entity definition by type name.
    pub fn get_entity(&self, entity_type: &str) -> Option<&EntityDef> {
        self.taxonomy.get(entity_type)
    }

    /// Get the lifecycle for an entity type.
    pub fn get_lifecycle(&self, entity_type: &str) -> Option<&EntityLifecycle> {
        self.taxonomy.get_lifecycle(entity_type)
    }

    /// Get FK argument for parent-child relationship.
    pub fn get_fk(&self, parent: &str, child: &str) -> Option<&str> {
        self.taxonomy.get_fk(parent, child)
    }

    /// Check if implicit creation is allowed for an entity type.
    pub fn allows_implicit_create(&self, entity_type: &str) -> bool {
        self.taxonomy.allows_implicit_create(entity_type)
    }

    /// Get the canonical creator verb for an entity type.
    pub fn canonical_creator(&self, entity_type: &str, subtype: Option<&str>) -> Option<String> {
        self.taxonomy.canonical_creator(entity_type, subtype)
    }

    /// Get the taxonomy (for advanced use cases).
    pub fn taxonomy(&self) -> &EntityTaxonomy {
        &self.taxonomy
    }

    // =========================================================================
    // Verb lifecycle accessors
    // =========================================================================

    /// Get verb lifecycle configuration from the runtime registry.
    ///
    /// This bridges to the existing verb YAML config loaded by RuntimeVerbRegistry.
    pub fn get_verb_lifecycle(&self, domain: &str, verb: &str) -> Option<VerbLifecycle> {
        // The verb lifecycle is stored in the verb YAML and loaded by RuntimeVerbRegistry.
        // We access it through the runtime registry to avoid duplicating the verb config loading.
        use crate::dsl_v2::runtime_registry;

        runtime_registry()
            .get(domain, verb)
            .and_then(|rv| rv.lifecycle.clone())
    }

    /// Check if a verb is a canonical creator for an entity type.
    pub fn is_canonical_creator(&self, domain: &str, verb: &str, entity_type: &str) -> bool {
        let full_verb = format!("{}.{}", domain, verb);

        // Check direct match
        if let Some(canonical) = self.taxonomy.canonical_creator(entity_type, None) {
            if canonical == full_verb {
                return true;
            }
        }

        // Check pattern match for subtypes
        if let Some(entity) = self.taxonomy.get(entity_type) {
            if let Some(ic) = &entity.implicit_create {
                if let Some(pattern) = &ic.canonical_verb_pattern {
                    // Pattern like "entity.create-{subtype}" matches "entity.create-proper_person"
                    let prefix = pattern.replace("{subtype}", "");
                    if full_verb.starts_with(&prefix) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Get entity types that this verb can create.
    pub fn verb_produces_entity_type(&self, domain: &str, verb: &str) -> Option<String> {
        use crate::dsl_v2::runtime_registry;

        runtime_registry()
            .get(domain, verb)
            .and_then(|rv| rv.produces.as_ref())
            .map(|p| p.produced_type.clone())
    }

    /// Get all entity type names.
    pub fn entity_types(&self) -> impl Iterator<Item = &str> {
        self.taxonomy.entity_types()
    }

    /// Resolve an alias to its canonical entity type.
    pub fn resolve_alias<'a>(&'a self, entity_type: &'a str) -> &'a str {
        self.taxonomy.resolve_alias(entity_type)
    }

    /// Get the initial state for an entity type.
    pub fn initial_state(&self, entity_type: &str) -> Option<&str> {
        self.taxonomy.initial_state(entity_type)
    }

    /// Check if a state transition is valid.
    pub fn is_valid_transition(&self, entity_type: &str, from_state: &str, to_state: &str) -> bool {
        self.taxonomy
            .get_lifecycle(entity_type)
            .map(|lc| lc.is_valid_transition(from_state, to_state))
            .unwrap_or(true) // No lifecycle = any transition allowed
    }

    /// Get valid next states for an entity.
    pub fn valid_next_states(&self, entity_type: &str, from_state: &str) -> Vec<&str> {
        self.taxonomy
            .get_lifecycle(entity_type)
            .map(|lc| lc.valid_next_states(from_state))
            .unwrap_or_default()
    }
}

/// Get the global ontology service.
///
/// This is the primary way to access ontology information throughout the codebase.
pub fn ontology() -> &'static OntologyService {
    OntologyService::global()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_resolution() {
        // Just test that the function doesn't panic
        let _path = OntologyService::config_path();
    }
}
