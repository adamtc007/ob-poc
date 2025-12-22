//! Entity Types Configuration
//!
//! Loads and provides access to entity type definitions for extraction.
//! Each entity type defines patterns for recognition, normalization rules,
//! and validation constraints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Root configuration for entity types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypesConfig {
    pub version: String,
    pub description: String,
    pub entity_types: HashMap<String, EntityTypeDefinition>,
    #[serde(default)]
    pub extraction_config: ExtractionConfig,
}

/// Definition of a single entity type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeDefinition {
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub patterns: Vec<PatternDefinition>,
    #[serde(default)]
    pub normalization: NormalizationConfig,
    #[serde(default)]
    pub validation: ValidationConfig,
    #[serde(default)]
    pub context_defaults: Vec<String>,
    #[serde(default)]
    pub components: Option<CompositeComponents>,
    #[serde(default)]
    pub examples: Vec<EntityExample>,
}

/// Pattern definition for entity recognition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternDefinition {
    #[serde(rename = "type")]
    pub pattern_type: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub regex: Option<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub mappings: HashMap<String, String>,
    #[serde(default)]
    pub valid_values: Vec<String>,
    #[serde(default)]
    pub fuzzy_match: bool,
    #[serde(default)]
    pub min_similarity: Option<f32>,
    #[serde(default)]
    pub requires_context: bool,
    #[serde(default)]
    pub expands_to: Option<String>,
    #[serde(default)]
    pub expansion_config: Option<String>,
    #[serde(default)]
    pub validation: Option<String>,
    #[serde(default)]
    pub normalization: Option<HashMap<String, serde_json::Value>>,
}

/// Normalization configuration for an entity type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NormalizationConfig {
    #[serde(default)]
    pub lookup_table: Option<String>,
    #[serde(default)]
    pub primary_key: Option<String>,
    #[serde(default)]
    pub fuzzy_match: bool,
    #[serde(default)]
    pub fuzzy_threshold: Option<f32>,
    #[serde(default)]
    pub case_insensitive: bool,
    #[serde(default)]
    pub uppercase: bool,
    #[serde(default)]
    pub hierarchy_aware: bool,
    #[serde(rename = "type", default)]
    pub value_type: Option<String>,
    #[serde(default)]
    pub remove_commas: bool,
    #[serde(default)]
    pub divide_by_100: bool,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub timezone_context: bool,
    #[serde(default)]
    pub pad_to_11: bool,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Validation configuration for an entity type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationConfig {
    #[serde(default)]
    pub must_exist: bool,
    #[serde(default)]
    pub resolution_required: bool,
    #[serde(default)]
    pub allow_create: bool,
    #[serde(default)]
    pub lookup_table: Option<String>,
    #[serde(default)]
    pub must_be_one_of: Vec<String>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

/// Components for composite entity types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeComponents {
    #[serde(default)]
    pub markets: Option<ComponentDef>,
    #[serde(default)]
    pub instruments: Option<ComponentDef>,
    #[serde(default)]
    pub currencies: Option<ComponentDef>,
    #[serde(default)]
    pub exclude: Option<ComponentDef>,
}

/// Definition of a composite component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDef {
    #[serde(rename = "type")]
    pub component_type: String,
    #[serde(default)]
    pub element_type: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub description: Option<String>,
}

/// Example for an entity type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExample {
    pub text: String,
    #[serde(default)]
    pub parsed: HashMap<String, serde_json::Value>,
}

/// Extraction configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionConfig {
    #[serde(default)]
    pub extraction_order: Vec<String>,
    #[serde(default)]
    pub context_free: Vec<String>,
    #[serde(default)]
    pub context_required: Vec<String>,
    #[serde(default)]
    pub expansions: HashMap<String, String>,
}

impl EntityTypesConfig {
    /// Load from a YAML file
    pub fn load_from_file(path: &Path) -> Result<Self, EntityTypesError> {
        let content = std::fs::read_to_string(path).map_err(|e| EntityTypesError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;

        Self::load_from_str(&content)
    }

    /// Load from a YAML string
    pub fn load_from_str(yaml: &str) -> Result<Self, EntityTypesError> {
        serde_yaml::from_str(yaml).map_err(|e| EntityTypesError::ParseError(e.to_string()))
    }

    /// Get an entity type definition by name
    pub fn get_entity_type(&self, name: &str) -> Option<&EntityTypeDefinition> {
        self.entity_types.get(name)
    }

    /// Get all entity types in extraction order
    pub fn extraction_ordered(&self) -> Vec<(&str, &EntityTypeDefinition)> {
        let mut result = Vec::new();

        for type_name in &self.extraction_config.extraction_order {
            if let Some(def) = self.entity_types.get(type_name) {
                result.push((type_name.as_str(), def));
            }
        }

        // Add any types not in the explicit order
        for (name, def) in &self.entity_types {
            if !self.extraction_config.extraction_order.contains(name) {
                result.push((name.as_str(), def));
            }
        }

        result
    }

    /// Check if an entity type is context-free (can be extracted without context)
    pub fn is_context_free(&self, type_name: &str) -> bool {
        self.extraction_config
            .context_free
            .contains(&type_name.to_string())
    }

    /// Check if an entity type requires context
    pub fn requires_context(&self, type_name: &str) -> bool {
        self.extraction_config
            .context_required
            .contains(&type_name.to_string())
    }

    /// Get the expansion config file for a type
    pub fn get_expansion_config(&self, expansion_key: &str) -> Option<&str> {
        self.extraction_config
            .expansions
            .get(expansion_key)
            .map(|s| s.as_str())
    }
}

impl EntityTypeDefinition {
    /// Get all regex patterns for this entity type
    pub fn get_regex_patterns(&self) -> Vec<&str> {
        self.patterns
            .iter()
            .filter_map(|p| p.regex.as_deref())
            .collect()
    }

    /// Get all static mappings (name -> code) for this entity type
    pub fn get_mappings(&self) -> HashMap<&str, &str> {
        let mut result = HashMap::new();
        for pattern in &self.patterns {
            for (key, value) in &pattern.mappings {
                result.insert(key.as_str(), value.as_str());
            }
        }
        result
    }

    /// Get all valid values for enum-like types
    pub fn get_valid_values(&self) -> Vec<&str> {
        self.patterns
            .iter()
            .flat_map(|p| p.valid_values.iter().map(|s| s.as_str()))
            .collect()
    }

    /// Check if this type supports fuzzy matching
    pub fn supports_fuzzy_match(&self) -> bool {
        self.normalization.fuzzy_match || self.patterns.iter().any(|p| p.fuzzy_match)
    }

    /// Get the lookup table for normalization/validation
    pub fn get_lookup_table(&self) -> Option<&str> {
        self.normalization
            .lookup_table
            .as_deref()
            .or(self.validation.lookup_table.as_deref())
    }
}

/// Errors that can occur when loading entity types
#[derive(Debug, thiserror::Error)]
pub enum EntityTypesError {
    #[error("Failed to read file {path}: {source}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse YAML: {0}")]
    ParseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ENTITY_TYPES: &str = r#"
version: "1.0"
description: "Test entity types"
entity_types:
  currency:
    description: "Currency code"
    category: "reference_data"
    patterns:
      - type: ISO_CODE
        regex: "[A-Z]{3}"
        examples:
          - "USD"
          - "EUR"
      - type: NAME
        mappings:
          "dollars": USD
          "euros": EUR
    normalization:
      uppercase: true
      lookup_table: currencies
    validation:
      must_exist: true
      lookup_table: custody.currencies
  amount:
    description: "Monetary amount"
    category: "numeric"
    patterns:
      - type: WITH_UNIT
        regex: "(\\d+)\\s*(k|m|bn)?"
        normalization:
          k: 1000
          m: 1000000
    normalization:
      type: decimal
      remove_commas: true
extraction_config:
  extraction_order:
    - currency
    - amount
  context_free:
    - currency
"#;

    #[test]
    fn test_parse_entity_types() {
        let config = EntityTypesConfig::load_from_str(SAMPLE_ENTITY_TYPES).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.entity_types.len(), 2);
    }

    #[test]
    fn test_get_entity_type() {
        let config = EntityTypesConfig::load_from_str(SAMPLE_ENTITY_TYPES).unwrap();
        let currency = config.get_entity_type("currency").unwrap();
        assert_eq!(currency.category, "reference_data");
    }

    #[test]
    fn test_extraction_order() {
        let config = EntityTypesConfig::load_from_str(SAMPLE_ENTITY_TYPES).unwrap();
        let ordered = config.extraction_ordered();
        assert_eq!(ordered[0].0, "currency");
        assert_eq!(ordered[1].0, "amount");
    }

    #[test]
    fn test_get_mappings() {
        let config = EntityTypesConfig::load_from_str(SAMPLE_ENTITY_TYPES).unwrap();
        let currency = config.get_entity_type("currency").unwrap();
        let mappings = currency.get_mappings();
        assert_eq!(mappings.get("dollars"), Some(&"USD"));
    }

    #[test]
    fn test_is_context_free() {
        let config = EntityTypesConfig::load_from_str(SAMPLE_ENTITY_TYPES).unwrap();
        assert!(config.is_context_free("currency"));
        assert!(!config.is_context_free("amount"));
    }
}
