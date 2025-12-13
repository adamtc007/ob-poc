//! Entity metadata configuration parsing
//!
//! Loads entity index configuration from YAML and provides
//! strongly-typed access to entity definitions.
//!
//! Supports two configuration sources:
//! 1. `entity_index.yaml` - Direct EntityGateway configuration
//! 2. Verb YAML lookup blocks - Extracted from DSL verb definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root configuration structure for the entity gateway
#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    pub refresh: RefreshConfig,
    pub database: DatabaseConfig,
    pub entities: HashMap<String, EntityConfig>,
}

/// Configuration for index refresh behavior
#[derive(Debug, Clone, Deserialize)]
pub struct RefreshConfig {
    pub interval_secs: u64,
    pub startup_mode: StartupMode,
}

/// Startup mode for initial index population
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupMode {
    /// Load indexes asynchronously (server starts immediately)
    Async,
    /// Load indexes synchronously (server waits until ready)
    Sync,
}

/// Database connection configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub connection_string_env: String,
}

/// Index mode determines how search terms are tokenized
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IndexMode {
    /// Trigram tokenization for fuzzy substring search (names, descriptions)
    #[default]
    Trigram,
    /// Standard word tokenization for prefix/exact match (codes, enums)
    Exact,
}

/// Configuration for a single entity type
#[derive(Debug, Clone, Deserialize)]
pub struct EntityConfig {
    /// Short name for API calls (e.g., "person", "fund")
    pub nickname: String,
    /// Fully qualified table name (e.g., "\"ob-poc\".entity_proper_persons")
    pub source_table: String,
    /// Column name of the primary key to return
    pub return_key: String,
    /// Template for display value (e.g., "{first_name} {last_name}")
    #[serde(default)]
    pub display_template: Option<String>,
    /// Index mode: trigram for names, exact for codes
    #[serde(default)]
    pub index_mode: IndexMode,
    /// Optional WHERE clause filter (e.g., "is_active = true")
    #[serde(default)]
    pub filter: Option<String>,
    /// Available search keys for this entity
    pub search_keys: Vec<SearchKeyConfig>,
    /// Sharding configuration
    pub shard: ShardConfig,
}

/// Configuration for a search key (simple single-column)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchKeyConfig {
    /// Name of the search key (used in API)
    pub name: String,
    /// Database column name
    pub column: String,
    /// Whether this is the default search key
    #[serde(default)]
    pub default: bool,
}

/// Configuration for a composite search key (multi-column for disambiguation)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompositeSearchKeyConfig {
    /// Name of the composite key (used in API)
    pub name: String,
    /// Primary search column (main fuzzy match)
    pub primary_column: String,
    /// Columns to combine for composite value
    pub columns: Vec<String>,
    /// Separator between columns (default: " ")
    #[serde(default = "default_separator")]
    pub separator: String,
    /// Discriminator fields for disambiguation
    #[serde(default)]
    pub discriminators: Vec<DiscriminatorConfig>,
    /// Whether this is the default search key
    #[serde(default)]
    pub default: bool,
}

fn default_separator() -> String {
    " ".to_string()
}

/// Discriminator field for composite search disambiguation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiscriminatorConfig {
    /// Database column name
    pub column: String,
    /// Selectivity score (0.0-1.0, higher = more unique)
    #[serde(default = "default_selectivity")]
    pub selectivity: f32,
}

fn default_selectivity() -> f32 {
    0.5
}

/// Unified search key variant (simple or composite)
#[derive(Debug, Clone)]
pub enum SearchKeyVariant<'a> {
    Simple(&'a SearchKeyConfig),
    Composite(&'a CompositeSearchKeyConfig),
}

/// Sharding configuration for an entity index
#[derive(Debug, Clone, Deserialize)]
pub struct ShardConfig {
    /// Whether sharding is enabled
    pub enabled: bool,
    /// Number of characters to use for shard prefix
    #[serde(default)]
    pub prefix_len: usize,
}

impl EntityConfig {
    /// Get the default search key for this entity
    pub fn default_search_key(&self) -> &SearchKeyConfig {
        self.search_keys
            .iter()
            .find(|k| k.default)
            .unwrap_or(&self.search_keys[0])
    }

    /// Get a search key by name
    pub fn get_search_key(&self, name: &str) -> Option<&SearchKeyConfig> {
        self.search_keys.iter().find(|k| k.name == name)
    }

    /// Get all column names needed for this entity
    pub fn all_columns(&self) -> Vec<&str> {
        let mut cols: Vec<&str> = vec![&self.return_key];

        for key in &self.search_keys {
            if !cols.contains(&key.column.as_str()) {
                cols.push(&key.column);
            }
        }

        // Add columns from display template if present
        if let Some(template) = &self.display_template {
            // Extract {column_name} patterns
            for cap in template.split('{').skip(1) {
                if let Some(col) = cap.split('}').next() {
                    if !cols.contains(&col) {
                        cols.push(col);
                    }
                }
            }
        }

        cols
    }
}

impl GatewayConfig {
    /// Load configuration from a YAML file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: GatewayConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration from a YAML string
    pub fn from_yaml(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config: GatewayConfig = serde_yaml::from_str(content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let yaml = r#"
refresh:
  interval_secs: 300
  startup_mode: async

database:
  connection_string_env: "DATABASE_URL"

entities:
  person:
    nickname: "person"
    source_table: "\"ob-poc\".entity_proper_persons"
    return_key: "proper_person_id"
    display_template: "{first_name} {last_name}"
    search_keys:
      - name: "name"
        column: "search_name"
        default: true
      - name: "id_document"
        column: "id_document_number"
    shard:
      enabled: true
      prefix_len: 1
"#;

        let config = GatewayConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.refresh.interval_secs, 300);
        assert!(matches!(config.refresh.startup_mode, StartupMode::Async));

        let person = config.entities.get("person").unwrap();
        assert_eq!(person.nickname, "person");
        assert_eq!(person.default_search_key().name, "name");
        assert!(person.shard.enabled);
    }

    #[test]
    fn test_all_columns() {
        let entity = EntityConfig {
            nickname: "person".to_string(),
            source_table: "persons".to_string(),
            return_key: "person_id".to_string(),
            display_template: Some("{first_name} {last_name}".to_string()),
            index_mode: IndexMode::Trigram,
            filter: None,
            search_keys: vec![SearchKeyConfig {
                name: "name".to_string(),
                column: "search_name".to_string(),
                default: true,
            }],
            shard: ShardConfig {
                enabled: false,
                prefix_len: 0,
            },
        };

        let cols = entity.all_columns();
        assert!(cols.contains(&"person_id"));
        assert!(cols.contains(&"search_name"));
        assert!(cols.contains(&"first_name"));
        assert!(cols.contains(&"last_name"));
    }
}
