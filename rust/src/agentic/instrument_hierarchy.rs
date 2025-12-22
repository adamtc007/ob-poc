//! Instrument Class Hierarchy Configuration
//!
//! Loads and provides access to the hierarchical instrument classification.
//! When a user says "fixed income", we expand to specific instrument class codes.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Root configuration for instrument hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentHierarchyConfig {
    pub version: String,
    pub description: String,
    pub instrument_hierarchy: HashMap<String, InstrumentNode>,
    #[serde(default)]
    pub shorthand_expansions: HashMap<String, ShorthandExpansion>,
}

/// A node in the instrument hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentNode {
    pub name: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub cfi_prefix: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub children: Vec<String>,
    #[serde(default)]
    pub requires_isda: bool,
    #[serde(default)]
    pub isda_asset_class: Option<String>,
    #[serde(default)]
    pub isda_product: Option<String>,
    #[serde(default)]
    pub smpg_category: Option<String>,
}

/// Shorthand expansion definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShorthandExpansion {
    #[serde(default)]
    pub expands_to: ShorthandTarget,
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub context_dependent: bool,
    #[serde(default)]
    pub contexts: HashMap<String, String>,
}

/// Target for shorthand expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ShorthandTarget {
    Special(String),
    List(Vec<String>),
}

impl Default for ShorthandTarget {
    fn default() -> Self {
        ShorthandTarget::List(Vec::new())
    }
}

impl InstrumentHierarchyConfig {
    /// Load from a YAML file
    pub fn load_from_file(path: &Path) -> Result<Self, InstrumentHierarchyError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| InstrumentHierarchyError::IoError {
                path: path.display().to_string(),
                source: e,
            })?;

        Self::load_from_str(&content)
    }

    /// Load from a YAML string
    pub fn load_from_str(yaml: &str) -> Result<Self, InstrumentHierarchyError> {
        serde_yaml::from_str(yaml).map_err(|e| InstrumentHierarchyError::ParseError(e.to_string()))
    }

    /// Expand a category/name to a list of instrument class codes
    pub fn expand_category(&self, category: &str) -> Option<Vec<String>> {
        // First check shorthand expansions
        if let Some(shorthand) = self.shorthand_expansions.get(category) {
            return Some(self.expand_shorthand(shorthand));
        }

        // Try to find by node name or alias
        let category_lower = category.to_lowercase();
        for (node_id, node) in &self.instrument_hierarchy {
            if node_id.to_lowercase() == category_lower
                || node.name.to_lowercase() == category_lower
            {
                return Some(self.expand_node(node_id));
            }
            for alias in &node.aliases {
                if alias.to_lowercase() == category_lower {
                    return Some(self.expand_node(node_id));
                }
            }
        }

        None
    }

    /// Expand a shorthand expression
    fn expand_shorthand(&self, shorthand: &ShorthandExpansion) -> Vec<String> {
        match &shorthand.expands_to {
            ShorthandTarget::Special(s) if s == "_ALL_IN_UNIVERSE" => {
                // Return all leaf codes
                self.all_leaf_codes()
            }
            ShorthandTarget::Special(_) => Vec::new(),
            ShorthandTarget::List(items) => {
                let mut result = Vec::new();
                for item in items {
                    // Each item could be a category or a code
                    if let Some(node) = self.instrument_hierarchy.get(item) {
                        if let Some(code) = &node.code {
                            result.push(code.clone());
                        } else {
                            result.extend(self.expand_node(item));
                        }
                    } else {
                        // Assume it's a code directly
                        result.push(item.clone());
                    }
                }
                result
            }
        }
    }

    /// Expand a node to all its leaf codes
    fn expand_node(&self, node_id: &str) -> Vec<String> {
        let mut codes = HashSet::new();
        self.collect_codes(node_id, &mut codes);
        let mut result: Vec<_> = codes.into_iter().collect();
        result.sort();
        result
    }

    /// Recursively collect all codes from a node and its children
    fn collect_codes(&self, node_id: &str, codes: &mut HashSet<String>) {
        if let Some(node) = self.instrument_hierarchy.get(node_id) {
            // If this node has a code, add it
            if let Some(code) = &node.code {
                codes.insert(code.clone());
            }

            // Recursively process children
            for child_id in &node.children {
                self.collect_codes(child_id, codes);
            }
        }
    }

    /// Get all leaf instrument codes
    pub fn all_leaf_codes(&self) -> Vec<String> {
        let mut codes = HashSet::new();
        for node in self.instrument_hierarchy.values() {
            if let Some(code) = &node.code {
                codes.insert(code.clone());
            }
        }
        let mut result: Vec<_> = codes.into_iter().collect();
        result.sort();
        result
    }

    /// Get a node by its ID
    pub fn get_node(&self, node_id: &str) -> Option<&InstrumentNode> {
        self.instrument_hierarchy.get(node_id)
    }

    /// Get a node by its code
    pub fn get_node_by_code(&self, code: &str) -> Option<&InstrumentNode> {
        self.instrument_hierarchy
            .values()
            .find(|n| n.code.as_deref() == Some(code))
    }

    /// Check if an instrument requires ISDA
    pub fn requires_isda(&self, code: &str) -> bool {
        self.get_node_by_code(code)
            .map(|n| n.requires_isda)
            .unwrap_or(false)
    }

    /// Get the ISDA asset class for an instrument
    pub fn get_isda_asset_class(&self, code: &str) -> Option<&str> {
        // First check the code's node
        if let Some(node) = self.get_node_by_code(code) {
            if let Some(asset_class) = &node.isda_asset_class {
                return Some(asset_class);
            }
        }

        // Walk up the hierarchy to find the asset class
        for node in self.instrument_hierarchy.values() {
            if node
                .children
                .iter()
                .any(|c| self.get_node(c).and_then(|n| n.code.as_deref()) == Some(code))
            {
                if let Some(asset_class) = &node.isda_asset_class {
                    return Some(asset_class);
                }
            }
        }

        None
    }

    /// Check if a string matches any category name or alias
    pub fn is_category(&self, text: &str) -> bool {
        let text_lower = text.to_lowercase();

        // Check shorthand
        if self.shorthand_expansions.contains_key(text) {
            return true;
        }

        // Check nodes
        for node in self.instrument_hierarchy.values() {
            if node.name.to_lowercase() == text_lower {
                return true;
            }
            for alias in &node.aliases {
                if alias.to_lowercase() == text_lower {
                    return true;
                }
            }
        }

        false
    }

    /// Find parent categories for a given code
    pub fn get_parents(&self, code: &str) -> Vec<&str> {
        let mut parents = Vec::new();

        for (node_id, node) in &self.instrument_hierarchy {
            // Check if this node is a direct parent
            for child_id in &node.children {
                if let Some(child_node) = self.get_node(child_id) {
                    if child_node.code.as_deref() == Some(code) {
                        parents.push(node_id.as_str());
                    }
                }
                // Also check if the child_id itself is the code we're looking for
                if child_id == code {
                    parents.push(node_id.as_str());
                }
            }
        }

        parents
    }
}

/// Errors that can occur when loading instrument hierarchy
#[derive(Debug, thiserror::Error)]
pub enum InstrumentHierarchyError {
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

    const SAMPLE_HIERARCHY: &str = r#"
version: "1.0"
description: "Test hierarchy"
instrument_hierarchy:
  root:
    name: "All Instruments"
    children:
      - listed_securities
      - fixed_income
  listed_securities:
    name: "Listed Securities"
    aliases:
      - "equities"
      - "stocks"
    children:
      - equity
      - etf
  equity:
    name: "Equity"
    code: "EQUITY"
    cfi_prefix: "ES"
    aliases:
      - "stocks"
      - "shares"
  etf:
    name: "ETF"
    code: "ETF"
  fixed_income:
    name: "Fixed Income"
    aliases:
      - "bonds"
    children:
      - govt_bond
      - corp_bond
  govt_bond:
    name: "Government Bonds"
    code: "GOVT_BOND"
    aliases:
      - "govvies"
  corp_bond:
    name: "Corporate Bonds"
    code: "CORP_BOND"
  derivatives:
    name: "Derivatives"
    requires_isda: true
    isda_asset_class: "RATES"
    children:
      - irs
  irs:
    name: "Interest Rate Swaps"
    code: "IRS"
    requires_isda: true
    isda_product: "IRSwap"
shorthand_expansions:
  "everything":
    expands_to: "_ALL_IN_UNIVERSE"
  "investment grade":
    expands_to:
      - GOVT_BOND
      - CORP_BOND
"#;

    #[test]
    fn test_parse_hierarchy() {
        let config = InstrumentHierarchyConfig::load_from_str(SAMPLE_HIERARCHY).unwrap();
        assert_eq!(config.version, "1.0");
    }

    #[test]
    fn test_expand_category() {
        let config = InstrumentHierarchyConfig::load_from_str(SAMPLE_HIERARCHY).unwrap();
        let codes = config.expand_category("fixed_income").unwrap();
        assert!(codes.contains(&"GOVT_BOND".to_string()));
        assert!(codes.contains(&"CORP_BOND".to_string()));
        assert!(!codes.contains(&"EQUITY".to_string()));
    }

    #[test]
    fn test_expand_by_alias() {
        let config = InstrumentHierarchyConfig::load_from_str(SAMPLE_HIERARCHY).unwrap();
        let codes = config.expand_category("bonds").unwrap();
        assert!(codes.contains(&"GOVT_BOND".to_string()));
    }

    #[test]
    fn test_expand_shorthand() {
        let config = InstrumentHierarchyConfig::load_from_str(SAMPLE_HIERARCHY).unwrap();
        let codes = config.expand_category("investment grade").unwrap();
        assert_eq!(codes.len(), 2);
        assert!(codes.contains(&"GOVT_BOND".to_string()));
    }

    #[test]
    fn test_expand_everything() {
        let config = InstrumentHierarchyConfig::load_from_str(SAMPLE_HIERARCHY).unwrap();
        let codes = config.expand_category("everything").unwrap();
        assert!(codes.len() >= 4); // All leaf codes
    }

    #[test]
    fn test_requires_isda() {
        let config = InstrumentHierarchyConfig::load_from_str(SAMPLE_HIERARCHY).unwrap();
        assert!(config.requires_isda("IRS"));
        assert!(!config.requires_isda("EQUITY"));
    }

    #[test]
    fn test_is_category() {
        let config = InstrumentHierarchyConfig::load_from_str(SAMPLE_HIERARCHY).unwrap();
        assert!(config.is_category("Fixed Income"));
        assert!(config.is_category("bonds")); // alias
        assert!(config.is_category("everything")); // shorthand
        assert!(!config.is_category("UNKNOWN"));
    }
}
