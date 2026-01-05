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
    /// Full template with discriminators (e.g., "{first_name} {last_name} ({nationality}, b.{birth_year})")
    #[serde(default)]
    pub display_template_full: Option<String>,
    /// Index mode: trigram for names, exact for codes
    #[serde(default)]
    pub index_mode: IndexMode,
    /// Optional WHERE clause filter (e.g., "is_active = true")
    #[serde(default)]
    pub filter: Option<String>,
    /// Composite search schema (s-expression) for disambiguation
    /// e.g., "(search_name (nationality :selectivity 0.7) (date_of_birth :selectivity 0.95))"
    #[serde(default)]
    pub composite_search: Option<String>,
    /// Available search keys for this entity
    pub search_keys: Vec<SearchKeyConfig>,
    /// Discriminator fields for filtering/scoring (used with composite_search)
    #[serde(default)]
    pub discriminators: Vec<EntityDiscriminatorConfig>,
    /// Sharding configuration
    #[serde(default)]
    pub shard: Option<ShardConfig>,
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

/// Entity-level discriminator configuration (from entity_index.yaml)
/// Similar to DiscriminatorConfig but with additional match mode for dates
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityDiscriminatorConfig {
    /// Name of the discriminator (used in API/display)
    pub name: String,
    /// Database column name
    pub column: String,
    /// Selectivity score (0.0-1.0, higher = more unique)
    #[serde(default = "default_selectivity")]
    pub selectivity: f32,
    /// Match mode for special types (e.g., "year_or_exact" for dates)
    #[serde(default)]
    pub match_mode: Option<String>,
}

/// Match mode for date discriminators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateMatchMode {
    /// Exact date match required
    Exact,
    /// Year match only (for approximate DOB)
    YearOnly,
    /// Match if year matches OR exact date matches (progressive refinement)
    YearOrExact,
}

impl std::str::FromStr for DateMatchMode {
    type Err = std::convert::Infallible;

    /// Parse from string, defaulting to Exact if unrecognized
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "year_only" | "year-only" => Self::YearOnly,
            "year_or_exact" | "year-or-exact" => Self::YearOrExact,
            _ => Self::Exact,
        })
    }
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

    /// Get a discriminator by name
    pub fn get_discriminator(&self, name: &str) -> Option<&EntityDiscriminatorConfig> {
        self.discriminators.iter().find(|d| d.name == name)
    }

    /// Check if this entity has composite search enabled
    pub fn has_composite_search(&self) -> bool {
        self.composite_search.is_some() && !self.discriminators.is_empty()
    }

    /// Get all column names needed for this entity
    pub fn all_columns(&self) -> Vec<&str> {
        let mut cols: Vec<&str> = vec![&self.return_key];

        // Add search key columns
        for key in &self.search_keys {
            if !cols.contains(&key.column.as_str()) {
                cols.push(&key.column);
            }
        }

        // Add discriminator columns
        for disc in &self.discriminators {
            if !cols.contains(&disc.column.as_str()) {
                cols.push(&disc.column);
            }
        }

        // Add columns from display template if present
        if let Some(template) = &self.display_template {
            Self::extract_template_columns(template, &mut cols);
        }

        // Add columns from full display template if present
        if let Some(template) = &self.display_template_full {
            Self::extract_template_columns(template, &mut cols);
        }

        cols
    }

    /// Extract column names from a template string like "{first_name} {last_name}"
    fn extract_template_columns<'a>(template: &'a str, cols: &mut Vec<&'a str>) {
        for cap in template.split('{').skip(1) {
            if let Some(col) = cap.split('}').next() {
                // Handle special computed columns like birth_year
                let col = col.trim();
                if !col.is_empty() && !cols.contains(&col) {
                    cols.push(col);
                }
            }
        }
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
        assert!(person.shard.as_ref().map(|s| s.enabled).unwrap_or(false));
    }

    #[test]
    fn test_all_columns() {
        let entity = EntityConfig {
            nickname: "person".to_string(),
            source_table: "persons".to_string(),
            return_key: "person_id".to_string(),
            display_template: Some("{first_name} {last_name}".to_string()),
            display_template_full: None,
            index_mode: IndexMode::Trigram,
            filter: None,
            composite_search: None,
            search_keys: vec![SearchKeyConfig {
                name: "name".to_string(),
                column: "search_name".to_string(),
                default: true,
            }],
            discriminators: vec![],
            shard: None,
        };

        let cols = entity.all_columns();
        assert!(cols.contains(&"person_id"));
        assert!(cols.contains(&"search_name"));
        assert!(cols.contains(&"first_name"));
        assert!(cols.contains(&"last_name"));
    }

    #[test]
    fn test_all_columns_with_discriminators() {
        let entity = EntityConfig {
            nickname: "person".to_string(),
            source_table: "persons".to_string(),
            return_key: "entity_id".to_string(),
            display_template: Some("{first_name} {last_name}".to_string()),
            display_template_full: Some(
                "{first_name} {last_name} ({nationality}, b.{birth_year})".to_string(),
            ),
            index_mode: IndexMode::Trigram,
            filter: None,
            composite_search: Some(
                "(search_name (nationality :selectivity 0.7) (date_of_birth :selectivity 0.95))"
                    .to_string(),
            ),
            search_keys: vec![SearchKeyConfig {
                name: "name".to_string(),
                column: "search_name".to_string(),
                default: true,
            }],
            discriminators: vec![
                EntityDiscriminatorConfig {
                    name: "nationality".to_string(),
                    column: "nationality".to_string(),
                    selectivity: 0.7,
                    match_mode: None,
                },
                EntityDiscriminatorConfig {
                    name: "date_of_birth".to_string(),
                    column: "date_of_birth".to_string(),
                    selectivity: 0.95,
                    match_mode: Some("year_or_exact".to_string()),
                },
            ],
            shard: Some(ShardConfig {
                enabled: true,
                prefix_len: 1,
            }),
        };

        let cols = entity.all_columns();
        // Core columns
        assert!(cols.contains(&"entity_id"));
        assert!(cols.contains(&"search_name"));
        // Template columns
        assert!(cols.contains(&"first_name"));
        assert!(cols.contains(&"last_name"));
        // Discriminator columns
        assert!(cols.contains(&"nationality"));
        assert!(cols.contains(&"date_of_birth"));
        // Full template columns
        assert!(cols.contains(&"birth_year"));

        // Check composite search is enabled
        assert!(entity.has_composite_search());

        // Check discriminator lookup
        let dob = entity.get_discriminator("date_of_birth").unwrap();
        assert_eq!(dob.selectivity, 0.95);
        assert_eq!(dob.match_mode.as_deref(), Some("year_or_exact"));
    }

    #[test]
    fn test_date_match_mode_parsing() {
        assert_eq!(
            "exact".parse::<DateMatchMode>().unwrap(),
            DateMatchMode::Exact
        );
        assert_eq!(
            "year_only".parse::<DateMatchMode>().unwrap(),
            DateMatchMode::YearOnly
        );
        assert_eq!(
            "year-only".parse::<DateMatchMode>().unwrap(),
            DateMatchMode::YearOnly
        );
        assert_eq!(
            "year_or_exact".parse::<DateMatchMode>().unwrap(),
            DateMatchMode::YearOrExact
        );
        assert_eq!(
            "year-or-exact".parse::<DateMatchMode>().unwrap(),
            DateMatchMode::YearOrExact
        );
        assert_eq!(
            "unknown".parse::<DateMatchMode>().unwrap(),
            DateMatchMode::Exact
        ); // default
    }
}
