//! Intent Taxonomy Configuration
//!
//! Loads and provides access to the intent taxonomy YAML configuration.
//! The taxonomy defines all recognizable intents, their trigger phrases,
//! required/optional entities, and default inference rules.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// The root intent taxonomy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentTaxonomy {
    pub version: String,
    pub description: String,
    pub intent_taxonomy: HashMap<String, DomainConfig>,
    #[serde(default)]
    pub intent_relationships: IntentRelationships,
    #[serde(default)]
    pub confidence_thresholds: ConfidenceThresholds,
}

/// Configuration for a domain (e.g., trading_matrix, compound)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConfig {
    pub description: String,
    #[serde(default)]
    pub intents: Vec<IntentDefinition>,
    #[serde(flatten)]
    pub subdomains: HashMap<String, SubdomainConfig>,
}

/// Configuration for a subdomain (e.g., investment_manager, pricing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubdomainConfig {
    pub description: String,
    #[serde(default)]
    pub intents: Vec<IntentDefinition>,
}

/// Definition of a single intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentDefinition {
    pub intent: String,
    pub description: String,
    #[serde(default)]
    pub canonical_verb: Option<String>,
    #[serde(default)]
    pub trigger_phrases: Vec<String>,
    #[serde(default)]
    pub required_entities: Vec<String>,
    #[serde(default)]
    pub optional_entities: Vec<String>,
    #[serde(default)]
    pub default_inferences: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub examples: Vec<IntentExample>,
    #[serde(default)]
    pub is_query: bool,
    #[serde(default)]
    pub confirmation_required: bool,
    #[serde(default)]
    pub expands_to: Vec<String>,
    #[serde(default)]
    pub action: Option<String>,
}

/// Example usage of an intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentExample {
    pub input: String,
    #[serde(default)]
    pub entities: HashMap<String, serde_json::Value>,
}

/// Intent relationships for context-aware classification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntentRelationships {
    #[serde(default)]
    pub natural_followups: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub mutually_exclusive: Vec<Vec<String>>,
}

/// Confidence thresholds for intent classification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfidenceThresholds {
    #[serde(default)]
    pub defaults: ThresholdConfig,
    #[serde(default)]
    pub overrides: HashMap<String, ThresholdConfig>,
}

/// Threshold configuration for a single intent or defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    #[serde(default = "default_execute_threshold")]
    pub execute_threshold: f32,
    #[serde(default = "default_confirm_threshold")]
    pub confirm_threshold: f32,
    #[serde(default = "default_suggest_threshold")]
    pub suggest_threshold: f32,
    #[serde(default)]
    pub clarify_threshold: f32,
}

fn default_execute_threshold() -> f32 {
    0.85
}
fn default_confirm_threshold() -> f32 {
    0.65
}
fn default_suggest_threshold() -> f32 {
    0.45
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            execute_threshold: default_execute_threshold(),
            confirm_threshold: default_confirm_threshold(),
            suggest_threshold: default_suggest_threshold(),
            clarify_threshold: 0.0,
        }
    }
}

impl IntentTaxonomy {
    /// Load taxonomy from a YAML file
    pub fn load_from_file(path: &Path) -> Result<Self, TaxonomyError> {
        let content = std::fs::read_to_string(path).map_err(|e| TaxonomyError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;

        Self::load_from_str(&content)
    }

    /// Load taxonomy from a YAML string
    pub fn load_from_str(yaml: &str) -> Result<Self, TaxonomyError> {
        serde_yaml::from_str(yaml).map_err(|e| TaxonomyError::ParseError(e.to_string()))
    }

    /// Get all intents flattened into a list
    pub fn all_intents(&self) -> Vec<&IntentDefinition> {
        let mut intents = Vec::new();

        for domain in self.intent_taxonomy.values() {
            intents.extend(domain.intents.iter());
            for subdomain in domain.subdomains.values() {
                intents.extend(subdomain.intents.iter());
            }
        }

        intents
    }

    /// Find an intent by its ID
    pub fn get_intent(&self, intent_id: &str) -> Option<&IntentDefinition> {
        self.all_intents()
            .into_iter()
            .find(|i| i.intent == intent_id)
    }

    /// Get the canonical verb for an intent
    pub fn get_canonical_verb(&self, intent_id: &str) -> Option<&str> {
        self.get_intent(intent_id)
            .and_then(|i| i.canonical_verb.as_deref())
    }

    /// Check if intent B is a natural followup of intent A
    pub fn is_natural_followup(&self, from_intent: &str, to_intent: &str) -> bool {
        self.intent_relationships
            .natural_followups
            .get(from_intent)
            .map(|followups| followups.contains(&to_intent.to_string()))
            .unwrap_or(false)
    }

    /// Get confidence thresholds for an intent
    pub fn get_thresholds(&self, intent_id: &str) -> &ThresholdConfig {
        self.confidence_thresholds
            .overrides
            .get(intent_id)
            .unwrap_or(&self.confidence_thresholds.defaults)
    }

    /// Get all trigger phrases for an intent (for embedding)
    pub fn get_trigger_phrases(&self, intent_id: &str) -> Vec<&str> {
        self.get_intent(intent_id)
            .map(|i| i.trigger_phrases.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
}

/// Errors that can occur when loading taxonomy
#[derive(Debug, thiserror::Error)]
pub enum TaxonomyError {
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

    const SAMPLE_TAXONOMY: &str = r#"
version: "1.0"
description: "Test taxonomy"
intent_taxonomy:
  trading_matrix:
    description: "Trading matrix domain"
    investment_manager:
      description: "IM subdomain"
      intents:
        - intent: im_assign
          description: "Assign IM"
          canonical_verb: investment-manager.assign
          trigger_phrases:
            - "add {manager} as investment manager"
            - "{manager} will handle {scope}"
          required_entities:
            - manager_name_or_lei
          optional_entities:
            - scope_markets
          default_inferences:
            priority: 100
intent_relationships:
  natural_followups:
    im_assign:
      - im_assign
      - pricing_set
confidence_thresholds:
  defaults:
    execute_threshold: 0.85
    confirm_threshold: 0.65
"#;

    #[test]
    fn test_parse_taxonomy() {
        let taxonomy = IntentTaxonomy::load_from_str(SAMPLE_TAXONOMY).unwrap();
        assert_eq!(taxonomy.version, "1.0");
    }

    #[test]
    fn test_get_all_intents() {
        let taxonomy = IntentTaxonomy::load_from_str(SAMPLE_TAXONOMY).unwrap();
        let intents = taxonomy.all_intents();
        assert!(!intents.is_empty());
        assert_eq!(intents[0].intent, "im_assign");
    }

    #[test]
    fn test_get_intent() {
        let taxonomy = IntentTaxonomy::load_from_str(SAMPLE_TAXONOMY).unwrap();
        let intent = taxonomy.get_intent("im_assign").unwrap();
        assert_eq!(
            intent.canonical_verb.as_deref(),
            Some("investment-manager.assign")
        );
    }

    #[test]
    fn test_natural_followups() {
        let taxonomy = IntentTaxonomy::load_from_str(SAMPLE_TAXONOMY).unwrap();
        assert!(taxonomy.is_natural_followup("im_assign", "pricing_set"));
        assert!(!taxonomy.is_natural_followup("im_assign", "unknown"));
    }

    #[test]
    fn test_get_thresholds() {
        let taxonomy = IntentTaxonomy::load_from_str(SAMPLE_TAXONOMY).unwrap();
        let thresholds = taxonomy.get_thresholds("im_assign");
        assert_eq!(thresholds.execute_threshold, 0.85);
    }
}
