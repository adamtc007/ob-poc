//! Market Regions Configuration
//!
//! Loads and provides access to market region expansion mappings.
//! When a user says "European equities", we expand to specific market MIC codes.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Root configuration for market regions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRegionsConfig {
    pub version: String,
    pub description: String,
    pub market_regions: HashMap<String, RegionDefinition>,
    #[serde(default)]
    pub market_timezones: HashMap<String, String>,
    #[serde(default)]
    pub market_csds: HashMap<String, String>,
}

/// Definition of a market region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionDefinition {
    pub description: String,
    #[serde(default)]
    pub markets: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub union_of: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub expands_to: Option<String>,
}

impl MarketRegionsConfig {
    /// Load from a YAML file
    pub fn load_from_file(path: &Path) -> Result<Self, MarketRegionsError> {
        let content = std::fs::read_to_string(path).map_err(|e| MarketRegionsError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;

        Self::load_from_str(&content)
    }

    /// Load from a YAML string
    pub fn load_from_str(yaml: &str) -> Result<Self, MarketRegionsError> {
        serde_yaml::from_str(yaml).map_err(|e| MarketRegionsError::ParseError(e.to_string()))
    }

    /// Expand a region name to a list of market MIC codes
    pub fn expand_region(&self, region_name: &str) -> Option<Vec<String>> {
        // First, try to find by exact name
        if let Some(region) = self.market_regions.get(region_name) {
            return Some(self.expand_region_def(region));
        }

        // Try to find by alias (case-insensitive)
        let region_lower = region_name.to_lowercase();
        for (name, region) in &self.market_regions {
            if name.to_lowercase() == region_lower {
                return Some(self.expand_region_def(region));
            }
            for alias in &region.aliases {
                if alias.to_lowercase() == region_lower {
                    return Some(self.expand_region_def(region));
                }
            }
        }

        None
    }

    /// Expand a region definition to MIC codes
    fn expand_region_def(&self, region: &RegionDefinition) -> Vec<String> {
        // Check for special expansion
        if let Some(expands_to) = &region.expands_to {
            if expands_to == "ALL_MARKETS" {
                return self.all_markets();
            }
        }

        let mut markets = HashSet::new();

        // Add direct markets
        for mic in &region.markets {
            markets.insert(mic.clone());
        }

        // Add markets from union regions
        for union_region in &region.union_of {
            if let Some(union_markets) = self.expand_region(union_region) {
                markets.extend(union_markets);
            }
        }

        // Remove excluded regions
        for exclude_region in &region.exclude {
            if let Some(exclude_markets) = self.expand_region(exclude_region) {
                for mic in exclude_markets {
                    markets.remove(&mic);
                }
            }
        }

        let mut result: Vec<_> = markets.into_iter().collect();
        result.sort();
        result
    }

    /// Get all market MIC codes
    pub fn all_markets(&self) -> Vec<String> {
        let mut markets = HashSet::new();
        for region in self.market_regions.values() {
            for mic in &region.markets {
                markets.insert(mic.clone());
            }
        }
        let mut result: Vec<_> = markets.into_iter().collect();
        result.sort();
        result
    }

    /// Get the timezone for a market
    pub fn get_timezone(&self, mic: &str) -> Option<&str> {
        self.market_timezones.get(mic).map(|s| s.as_str())
    }

    /// Get the CSD BIC for a market
    pub fn get_csd(&self, mic: &str) -> Option<&str> {
        self.market_csds.get(mic).map(|s| s.as_str())
    }

    /// Find which region(s) a market belongs to
    pub fn regions_for_market(&self, mic: &str) -> Vec<&str> {
        let mut regions = Vec::new();
        for (name, region) in &self.market_regions {
            if region.markets.contains(&mic.to_string()) {
                regions.push(name.as_str());
            }
        }
        regions
    }

    /// Check if a string matches any region name or alias
    pub fn is_region(&self, text: &str) -> bool {
        let text_lower = text.to_lowercase();
        for (name, region) in &self.market_regions {
            if name.to_lowercase() == text_lower {
                return true;
            }
            for alias in &region.aliases {
                if alias.to_lowercase() == text_lower {
                    return true;
                }
            }
        }
        false
    }
}

/// Errors that can occur when loading market regions
#[derive(Debug, thiserror::Error)]
pub enum MarketRegionsError {
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

    const SAMPLE_REGIONS: &str = r#"
version: "1.0"
description: "Test regions"
market_regions:
  European:
    description: "European markets"
    markets:
      - XLON
      - XETR
      - XPAR
    aliases:
      - "Europe"
      - "EU"
  US:
    description: "US markets"
    markets:
      - XNYS
      - XNAS
    aliases:
      - "American"
  North_American:
    description: "US and Canada"
    union_of:
      - US
    markets:
      - XTSE
    aliases:
      - "NA"
  Global:
    description: "All markets"
    expands_to: ALL_MARKETS
    aliases:
      - "worldwide"
market_timezones:
  XNYS: "America/New_York"
  XLON: "Europe/London"
market_csds:
  XNYS: "DTCYUS33"
  XLON: "CABOROCP"
"#;

    #[test]
    fn test_parse_regions() {
        let config = MarketRegionsConfig::load_from_str(SAMPLE_REGIONS).unwrap();
        assert_eq!(config.version, "1.0");
    }

    #[test]
    fn test_expand_region() {
        let config = MarketRegionsConfig::load_from_str(SAMPLE_REGIONS).unwrap();
        let markets = config.expand_region("European").unwrap();
        assert!(markets.contains(&"XLON".to_string()));
        assert!(markets.contains(&"XETR".to_string()));
        assert!(!markets.contains(&"XNYS".to_string()));
    }

    #[test]
    fn test_expand_by_alias() {
        let config = MarketRegionsConfig::load_from_str(SAMPLE_REGIONS).unwrap();
        let markets = config.expand_region("EU").unwrap();
        assert!(markets.contains(&"XLON".to_string()));
    }

    #[test]
    fn test_expand_union() {
        let config = MarketRegionsConfig::load_from_str(SAMPLE_REGIONS).unwrap();
        let markets = config.expand_region("North_American").unwrap();
        assert!(markets.contains(&"XNYS".to_string())); // From US
        assert!(markets.contains(&"XTSE".to_string())); // Direct
    }

    #[test]
    fn test_expand_global() {
        let config = MarketRegionsConfig::load_from_str(SAMPLE_REGIONS).unwrap();
        let markets = config.expand_region("Global").unwrap();
        assert!(markets.len() >= 4); // Should have all markets
    }

    #[test]
    fn test_get_timezone() {
        let config = MarketRegionsConfig::load_from_str(SAMPLE_REGIONS).unwrap();
        assert_eq!(config.get_timezone("XNYS"), Some("America/New_York"));
        assert_eq!(config.get_timezone("UNKNOWN"), None);
    }

    #[test]
    fn test_is_region() {
        let config = MarketRegionsConfig::load_from_str(SAMPLE_REGIONS).unwrap();
        assert!(config.is_region("European"));
        assert!(config.is_region("europe")); // Case-insensitive
        assert!(config.is_region("EU")); // Alias
        assert!(!config.is_region("XNYS")); // Not a region
    }
}
