//! Unified Verb Registry
//!
//! Single source of truth for all verbs in the DSL system.
//! Loads from RuntimeVerbRegistry (YAML config) and custom operations.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    UnifiedVerbRegistry                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Sources:                                                    │
//! │  ├── YAML verbs (from RuntimeVerbRegistry)                  │
//! │  └── Custom ops (plugins defined in this module)            │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::OnceLock;

use super::config::types::{LookupConfig, VerbConsumes, VerbProduces};
use super::runtime_registry::{runtime_registry, RuntimeBehavior};

// =============================================================================
// TYPES
// =============================================================================

/// How a verb is executed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbBehavior {
    /// Standard CRUD operation (generic executor)
    Crud,
    /// Custom operation with specialized handler
    CustomOp,
    /// Composite operation (expands to multiple steps)
    Composite,
    /// Graph query operation (GraphQueryExecutor)
    GraphQuery,
}

/// Argument definition for unified verbs
#[derive(Debug, Clone)]
pub struct ArgDef {
    pub name: String,
    pub arg_type: String,
    pub required: bool,
    pub description: String,
    /// Lookup configuration for entity/reference data resolution
    /// Contains: entity_type (nickname), search_key, primary_key
    pub lookup: Option<LookupConfig>,
}

/// Unified verb definition combining CRUD and custom ops
#[derive(Debug, Clone)]
pub struct UnifiedVerbDef {
    pub domain: String,
    pub verb: String,
    pub description: String,
    pub args: Vec<ArgDef>,
    pub behavior: VerbBehavior,
    /// For custom ops, the handler ID
    pub custom_op_id: Option<String>,
    pub produces: Option<VerbProduces>,
    pub consumes: Vec<VerbConsumes>,
}

impl UnifiedVerbDef {
    pub fn consumes(&self) -> &[VerbConsumes] {
        &self.consumes
    }

    /// Full verb name: "domain.verb"
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.domain, self.verb)
    }

    /// Check if verb accepts a given argument key
    pub fn accepts_arg(&self, key: &str) -> bool {
        self.args.iter().any(|a| a.name == key)
    }

    /// Get required arguments
    pub fn required_args(&self) -> Vec<&ArgDef> {
        self.args.iter().filter(|a| a.required).collect()
    }

    /// Get required argument names
    pub fn required_arg_names(&self) -> Vec<&str> {
        self.args
            .iter()
            .filter(|a| a.required)
            .map(|a| a.name.as_str())
            .collect()
    }

    /// Get optional argument names
    pub fn optional_arg_names(&self) -> Vec<&str> {
        self.args
            .iter()
            .filter(|a| !a.required)
            .map(|a| a.name.as_str())
            .collect()
    }
}

// =============================================================================
// REGISTRY
// =============================================================================

/// The unified verb registry - singleton
static UNIFIED_REGISTRY: OnceLock<UnifiedVerbRegistry> = OnceLock::new();

pub struct UnifiedVerbRegistry {
    /// All verbs indexed by "domain.verb"
    verbs: HashMap<String, UnifiedVerbDef>,
    /// Verbs grouped by domain
    by_domain: HashMap<String, Vec<String>>,
    /// All domain names (sorted)
    domains: Vec<String>,
}

impl UnifiedVerbRegistry {
    /// Get the global registry instance
    pub fn global() -> &'static UnifiedVerbRegistry {
        UNIFIED_REGISTRY.get_or_init(Self::build)
    }

    /// Build the registry from RuntimeVerbRegistry
    fn build() -> Self {
        let mut verbs = HashMap::new();
        let mut by_domain: HashMap<String, Vec<String>> = HashMap::new();

        // Load from RuntimeVerbRegistry (YAML config)
        let runtime_reg = runtime_registry();

        for runtime_verb in runtime_reg.all_verbs() {
            let key = runtime_verb.full_name.clone();

            // Convert RuntimeArg to ArgDef (preserving lookup config)
            let args: Vec<ArgDef> = runtime_verb
                .args
                .iter()
                .map(|a| ArgDef {
                    name: a.name.clone(),
                    arg_type: format!("{:?}", a.arg_type),
                    required: a.required,
                    description: String::new(),
                    lookup: a.lookup.clone(),
                })
                .collect();

            // Determine behavior from RuntimeBehavior
            let (behavior, custom_op_id) = match &runtime_verb.behavior {
                RuntimeBehavior::Crud(_) => (VerbBehavior::Crud, None),
                RuntimeBehavior::Plugin(handler) => (VerbBehavior::CustomOp, Some(handler.clone())),
                RuntimeBehavior::GraphQuery(_) => (VerbBehavior::GraphQuery, None),
            };

            let unified = UnifiedVerbDef {
                domain: runtime_verb.domain.clone(),
                verb: runtime_verb.verb.clone(),
                description: runtime_verb.description.clone(),
                args,
                behavior,
                custom_op_id,
                produces: runtime_verb.produces.clone(),
                consumes: runtime_verb.consumes.clone(),
            };

            verbs.insert(key.clone(), unified);
            by_domain
                .entry(runtime_verb.domain.clone())
                .or_default()
                .push(key);
        }

        // Sort domain verb lists
        for list in by_domain.values_mut() {
            list.sort();
            list.dedup();
        }

        let mut domains: Vec<String> = by_domain.keys().cloned().collect();
        domains.sort();

        Self {
            verbs,
            by_domain,
            domains,
        }
    }

    /// Look up a verb by domain and verb name
    pub fn get(&self, domain: &str, verb: &str) -> Option<&UnifiedVerbDef> {
        let key = format!("{}.{}", domain, verb);
        self.verbs.get(&key)
    }

    /// Look up by full name "domain.verb"
    pub fn get_by_name(&self, full_name: &str) -> Option<&UnifiedVerbDef> {
        self.verbs.get(full_name)
    }

    /// Get all verbs for a domain
    pub fn verbs_for_domain(&self, domain: &str) -> Vec<&UnifiedVerbDef> {
        self.by_domain
            .get(domain)
            .map(|keys| keys.iter().filter_map(|k| self.verbs.get(k)).collect())
            .unwrap_or_default()
    }

    /// Get all domain names
    pub fn domains(&self) -> &[String] {
        &self.domains
    }

    /// Get all verbs
    pub fn all_verbs(&self) -> impl Iterator<Item = &UnifiedVerbDef> {
        self.verbs.values()
    }

    /// Total verb count
    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    /// Check if a verb exists
    pub fn contains(&self, domain: &str, verb: &str) -> bool {
        self.get(domain, verb).is_some()
    }

    /// Get what a verb produces (delegates to RuntimeVerbRegistry)
    /// Returns the entity type and optional subtype from verb YAML config
    pub fn get_produces(
        &self,
        domain: &str,
        verb: &str,
    ) -> Option<&super::config::types::VerbProduces> {
        runtime_registry().get_produces(domain, verb)
    }

    /// Get a verb from the runtime registry (for full metadata access)
    pub fn get_runtime_verb(
        &self,
        domain: &str,
        verb: &str,
    ) -> Option<&super::runtime_registry::RuntimeVerb> {
        runtime_registry().get(domain, verb)
    }
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Get the global registry
pub fn registry() -> &'static UnifiedVerbRegistry {
    UnifiedVerbRegistry::global()
}

/// Look up a verb (convenience function)
pub fn find_unified_verb(domain: &str, verb: &str) -> Option<&'static UnifiedVerbDef> {
    registry().get(domain, verb)
}

/// Check if verb exists (convenience function)
pub fn verb_exists(domain: &str, verb: &str) -> bool {
    registry().contains(domain, verb)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_loads() {
        let reg = UnifiedVerbRegistry::global();
        assert!(!reg.is_empty(), "Registry should have verbs");
    }

    #[test]
    fn test_crud_verb_exists() {
        let reg = registry();
        let verb = reg.get("cbu", "create");
        assert!(verb.is_some(), "cbu.create should exist");
        assert_eq!(verb.unwrap().behavior, VerbBehavior::Crud);
    }

    #[test]
    fn test_custom_op_exists() {
        let reg = registry();
        let verb = reg.get("document", "catalog");
        assert!(verb.is_some(), "document.catalog should exist");
        assert_eq!(verb.unwrap().behavior, VerbBehavior::CustomOp);
    }

    #[test]
    fn test_domains_list() {
        let reg = registry();
        let domains = reg.domains();
        assert!(domains.contains(&"cbu".to_string()));
    }

    #[test]
    fn test_full_name() {
        let reg = registry();
        if let Some(verb) = reg.get("cbu", "create") {
            assert_eq!(verb.full_name(), "cbu.create");
        }
    }
}
