//! Token Registry - loads and provides access to token definitions
//!
//! The registry loads token configurations from YAML and provides
//! fast lookup by type ID with alias support.

use std::collections::HashMap;
use std::sync::Arc;

use super::types::{TokenDefinition, TokenTypeId};

/// Registry of all token type definitions
pub struct TokenRegistry {
    /// Token definitions by type ID
    definitions: HashMap<TokenTypeId, Arc<TokenDefinition>>,

    /// Entity type -> Token type mapping (for aliases)
    type_aliases: HashMap<String, TokenTypeId>,

    /// Default token for unknown types
    default_token: Arc<TokenDefinition>,
}

/// Configuration file structure
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenConfig {
    /// Version for compatibility checking
    #[serde(default = "default_version")]
    pub version: String,

    /// Token definitions
    pub tokens: Vec<TokenDefinition>,

    /// Entity type aliases (maps multiple entity types to one token)
    #[serde(default)]
    pub aliases: HashMap<TokenTypeId, Vec<String>>,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl TokenRegistry {
    /// Create empty registry with just the default token
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            type_aliases: HashMap::new(),
            default_token: Arc::new(TokenDefinition::default_unknown()),
        }
    }

    /// Load registry from YAML string
    pub fn from_yaml(yaml_str: &str) -> Result<Self, String> {
        let config: TokenConfig =
            serde_yaml::from_str(yaml_str).map_err(|e| format!("Failed to parse YAML: {}", e))?;
        Self::from_config(config)
    }

    /// Build registry from parsed config
    pub fn from_config(config: TokenConfig) -> Result<Self, String> {
        let mut definitions = HashMap::new();
        let mut type_aliases = HashMap::new();

        for def in config.tokens {
            let type_id = def.type_id.clone();
            definitions.insert(type_id.clone(), Arc::new(def));
        }

        // Build alias map
        for (token_type, aliases) in config.aliases {
            for alias in aliases {
                type_aliases.insert(alias, token_type.clone());
            }
        }

        let default_token = Arc::new(TokenDefinition::default_unknown());

        Ok(Self {
            definitions,
            type_aliases,
            default_token,
        })
    }

    /// Get token definition by type ID
    pub fn get(&self, type_id: &str) -> Arc<TokenDefinition> {
        // Check direct match first
        if let Some(def) = self.definitions.get(type_id) {
            return def.clone();
        }

        // Check aliases
        if let Some(aliased_type) = self.type_aliases.get(type_id) {
            if let Some(def) = self.definitions.get(aliased_type) {
                return def.clone();
            }
        }

        // Fall back to default
        self.default_token.clone()
    }

    /// Check if a type ID exists (directly or via alias)
    pub fn contains(&self, type_id: &str) -> bool {
        self.definitions.contains_key(type_id) || self.type_aliases.contains_key(type_id)
    }

    /// List all registered type IDs (not including aliases)
    pub fn type_ids(&self) -> impl Iterator<Item = &TokenTypeId> {
        self.definitions.keys()
    }

    /// Get all container tokens
    pub fn containers(&self) -> impl Iterator<Item = &Arc<TokenDefinition>> {
        self.definitions.values().filter(|d| d.is_container)
    }

    /// Get count of registered tokens
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

impl Default for TokenRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// EMBEDDED DEFAULT CONFIG
// =============================================================================

/// Default token configuration YAML (embedded for WASM compatibility)
pub const DEFAULT_TOKEN_CONFIG: &str = include_str!("default_tokens.yaml");

impl TokenRegistry {
    /// Load from embedded default configuration
    pub fn load_defaults() -> Result<Self, String> {
        Self::from_yaml(DEFAULT_TOKEN_CONFIG)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_loads_defaults() {
        let registry = TokenRegistry::load_defaults().expect("Failed to load defaults");
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_unknown_type_returns_default() {
        let registry = TokenRegistry::new();
        let token = registry.get("nonexistent_type");
        assert_eq!(token.type_id, "_unknown");
    }

    #[test]
    fn test_alias_resolution() {
        let yaml = r#"
version: "1.0"
aliases:
  entity:
    - proper_person
    - limited_company
tokens:
  - type_id: entity
    label: "Entity"
    visual:
      icon:
        fallback: "E"
      base_color: [128, 128, 128, 255]
      highlight_color: [180, 180, 180, 255]
"#;
        let registry = TokenRegistry::from_yaml(yaml).expect("Failed to parse");

        // Direct lookup
        let token = registry.get("entity");
        assert_eq!(token.label, "Entity");

        // Alias lookup
        let token = registry.get("proper_person");
        assert_eq!(token.label, "Entity");

        let token = registry.get("limited_company");
        assert_eq!(token.label, "Entity");
    }
}
