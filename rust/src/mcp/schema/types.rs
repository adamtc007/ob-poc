//! Core types for verb schema definitions (V2 Format)
//!
//! Canonical schema structure:
//! ```yaml
//! verb: view.drill
//! domain: view
//! action: drill
//! aliases: [drill, zoom-in]
//! args:
//!   style: keyworded
//!   required:
//!     entity: { type: entity_name }
//!   optional:
//!     depth: { type: int, default: 1 }
//! positional_sugar: [entity]
//! invocation_phrases: ["drill into entity"]
//! examples: ["(view.drill :entity \"X\")"]
//! doc: "Drill into entity"
//! tier: intent
//! tags: [navigation]
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// V2 CANONICAL TYPES
// ============================================================================

/// Complete specification for a verb (V2 format)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerbSpec {
    /// FQN for V2 format OR name for V1-style registry
    #[serde(alias = "verb")]
    pub name: String,

    /// Domain name
    pub domain: String,

    /// Action name (verb part) - optional for V1 compatibility
    #[serde(default)]
    pub action: String,

    /// Alternative names that resolve to this verb
    #[serde(default)]
    pub aliases: Vec<String>,

    /// Argument schema
    pub args: ArgSchema,

    /// Positional sugar: args that can be provided positionally (max 2)
    #[serde(default)]
    pub positional_sugar: Vec<String>,

    /// Keyword aliases (short → full name)
    #[serde(default)]
    pub keyword_aliases: HashMap<String, String>,

    /// Invocation phrases for semantic matching
    #[serde(default)]
    pub invocation_phrases: Vec<String>,

    /// Example s-expressions
    #[serde(default)]
    pub examples: Vec<String>,

    /// Documentation string
    #[serde(default)]
    pub doc: String,

    /// Verb tier (intent, crud, operational, computed, refdata)
    #[serde(default)]
    pub tier: String,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

impl VerbSpec {
    /// Get FQN (fully qualified name)
    pub fn fqn(&self) -> &str {
        &self.name
    }

    /// Get argument by name (searches both required and optional in V2 format)
    pub fn get_arg(&self, name: &str) -> Option<&ArgDef> {
        self.args.get(name)
    }

    /// Check if argument is required
    pub fn is_required(&self, name: &str) -> bool {
        self.args.required.iter().any(|a| a.name == name)
    }

    /// Get all argument names
    pub fn arg_names(&self) -> impl Iterator<Item = &String> {
        self.args
            .required
            .iter()
            .map(|a| &a.name)
            .chain(self.args.optional.iter().map(|a| &a.name))
    }
}

/// Schema for verb arguments
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArgSchema {
    /// Argument style: keyworded, positional, hybrid
    #[serde(default = "default_style")]
    pub style: String,

    /// Required arguments
    #[serde(default)]
    pub required: Vec<ArgDef>,

    /// Optional arguments (with defaults)
    #[serde(default)]
    pub optional: Vec<ArgDef>,
}

fn default_style() -> String {
    "keyworded".to_string()
}

impl ArgSchema {
    /// Get all arguments
    pub fn all(&self) -> impl Iterator<Item = &ArgDef> {
        self.required.iter().chain(self.optional.iter())
    }

    /// Find an argument by name
    pub fn get(&self, name: &str) -> Option<&ArgDef> {
        self.required
            .iter()
            .find(|a| a.name == name)
            .or_else(|| self.optional.iter().find(|a| a.name == name))
    }
}

/// Argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgDef {
    /// Argument name
    pub name: String,

    /// Argument shape/type
    pub shape: ArgShape,

    /// Default value (for optional args)
    #[serde(default)]
    pub default: Option<serde_json::Value>,

    /// Documentation
    #[serde(default)]
    pub doc: String,

    /// Maps to DB column (if different from name)
    #[serde(default)]
    pub maps_to: Option<String>,

    /// Entity lookup configuration
    #[serde(default)]
    pub lookup: Option<LookupConfig>,
}

impl Default for ArgDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            shape: ArgShape::Str,
            default: None,
            doc: String::new(),
            maps_to: None,
            lookup: None,
        }
    }
}

/// Argument shape/type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ArgShape {
    #[default]
    Str,
    Int,
    Bool,
    Decimal,
    Uuid,
    Date,
    Datetime,
    Json,
    #[serde(rename = "enum")]
    Enum {
        values: Vec<String>,
    },
    #[serde(rename = "entity_ref")]
    EntityRef {
        allowed_kinds: Vec<String>,
    },
    #[serde(rename = "entity_name")]
    EntityName,
    #[serde(rename = "list")]
    List {
        item: Box<ArgShape>,
    },
}

impl ArgShape {
    /// Human-readable type name
    pub fn type_name(&self) -> &str {
        match self {
            ArgShape::Str => "string",
            ArgShape::Int => "integer",
            ArgShape::Bool => "boolean",
            ArgShape::Decimal => "decimal",
            ArgShape::Uuid => "uuid",
            ArgShape::Date => "date",
            ArgShape::Datetime => "datetime",
            ArgShape::Json => "json",
            ArgShape::Enum { .. } => "enum",
            ArgShape::EntityRef { .. } => "entity",
            ArgShape::EntityName => "entity_name",
            ArgShape::List { .. } => "list",
        }
    }

    /// Check if this is an entity reference type
    pub fn is_entity(&self) -> bool {
        matches!(self, ArgShape::EntityRef { .. } | ArgShape::EntityName)
    }

    /// Check if this is an enum type
    pub fn is_enum(&self) -> bool {
        matches!(self, ArgShape::Enum { .. })
    }
}

/// Configuration for entity lookup/resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupConfig {
    pub table: String,
    #[serde(default = "default_schema")]
    pub schema: String,
    #[serde(default)]
    pub entity_type: Option<String>,
    pub search_key: String,
    pub primary_key: String,
    #[serde(default)]
    pub resolution_mode: Option<String>,
}

fn default_schema() -> String {
    "ob-poc".to_string()
}

// ============================================================================
// SOURCE FORMAT TYPES (for parsing existing YAML verb files)
// ============================================================================

/// Schema file containing one or more domains (source format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaFile {
    /// Schema version
    #[serde(default = "default_version")]
    pub version: String,

    /// Domains defined in this file
    #[serde(default)]
    pub domains: HashMap<String, DomainContent>,
}

fn default_version() -> String {
    "1.0".to_string()
}

/// Content of a domain in source format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainContent {
    /// Domain description
    #[serde(default)]
    pub description: String,

    /// Verb definitions
    #[serde(default)]
    pub verbs: HashMap<String, VerbContent>,
}

/// Raw verb content from source YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbContent {
    /// Verb description
    #[serde(default)]
    pub description: String,

    /// Invocation phrases for semantic matching
    #[serde(default)]
    pub invocation_phrases: Vec<String>,

    /// Behavior type
    #[serde(default)]
    pub behavior: String,

    /// Metadata
    #[serde(default)]
    pub metadata: VerbMetadata,

    /// Arguments
    #[serde(default)]
    pub args: Vec<ArgContent>,

    /// Return type
    #[serde(default)]
    pub returns: Option<ReturnsContent>,
}

impl VerbContent {
    /// Convert source format to VerbSpec
    pub fn to_spec(&self, domain: &str, verb_name: &str) -> VerbSpec {
        let fqn = format!("{}.{}", domain, verb_name);

        // Build aliases from invocation phrases (single words only) + verb_name
        let mut aliases: Vec<String> = self
            .invocation_phrases
            .iter()
            .filter(|p| !p.contains(' '))
            .cloned()
            .collect();

        // Always include the verb name as an alias
        if !aliases.contains(&verb_name.to_string()) {
            aliases.push(verb_name.to_string());
        }

        aliases.sort();
        aliases.dedup();

        // Build argument schema
        let mut required = Vec::new();
        let mut optional = Vec::new();

        for arg in &self.args {
            let arg_def = arg.to_arg_def();
            if arg.required {
                required.push(arg_def);
            } else {
                optional.push(arg_def);
            }
        }

        // Compute positional sugar (first 1-2 required args)
        let positional_sugar: Vec<String> = self
            .args
            .iter()
            .filter(|a| a.required)
            .take(2)
            .map(|a| a.name.clone())
            .collect();

        VerbSpec {
            name: fqn,
            domain: domain.to_string(),
            action: verb_name.to_string(),
            aliases,
            args: ArgSchema {
                style: "keyworded".to_string(),
                required,
                optional,
            },
            positional_sugar,
            keyword_aliases: HashMap::new(),
            invocation_phrases: self.invocation_phrases.clone(),
            examples: Vec::new(),
            doc: self.description.clone(),
            tier: self.metadata.tier.clone(),
            tags: self.metadata.tags.clone(),
        }
    }
}

/// Verb metadata from source YAML
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerbMetadata {
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub source_of_truth: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub noun: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub internal: bool,
}

/// Argument content from source YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgContent {
    pub name: String,
    #[serde(rename = "type", default)]
    pub arg_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub valid_values: Option<Vec<String>>,
    #[serde(default)]
    pub maps_to: Option<String>,
    #[serde(default)]
    pub lookup: Option<LookupConfig>,
}

impl ArgContent {
    /// Convert to ArgDef
    pub fn to_arg_def(&self) -> ArgDef {
        let shape = self.to_shape();
        ArgDef {
            name: self.name.clone(),
            shape,
            default: self.default.clone(),
            doc: self.description.clone(),
            maps_to: self.maps_to.clone(),
            lookup: self.lookup.clone(),
        }
    }

    /// Convert arg_type string to ArgShape
    fn to_shape(&self) -> ArgShape {
        // Handle enum with valid_values
        if let Some(values) = &self.valid_values {
            return ArgShape::Enum {
                values: values.clone(),
            };
        }

        // Handle entity reference with lookup
        if self.lookup.is_some() {
            let kinds = self
                .lookup
                .as_ref()
                .and_then(|l| l.entity_type.as_ref())
                .map(|t| vec![t.clone()])
                .unwrap_or_default();
            return ArgShape::EntityRef {
                allowed_kinds: kinds,
            };
        }

        match self.arg_type.as_str() {
            "string" | "str" => ArgShape::Str,
            "integer" | "int" => ArgShape::Int,
            "boolean" | "bool" => ArgShape::Bool,
            "decimal" | "numeric" => ArgShape::Decimal,
            "uuid" => ArgShape::Uuid,
            "date" => ArgShape::Date,
            "datetime" => ArgShape::Datetime,
            "json" | "object" => ArgShape::Json,
            "entity" | "entity_ref" => ArgShape::EntityRef {
                allowed_kinds: vec![],
            },
            "entity_name" => ArgShape::EntityName,
            "string_list" => ArgShape::List {
                item: Box::new(ArgShape::Str),
            },
            "uuid_list" => ArgShape::List {
                item: Box::new(ArgShape::Uuid),
            },
            _ => ArgShape::Str,
        }
    }
}

/// Returns content from source YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnsContent {
    #[serde(rename = "type")]
    pub return_type: String,
    #[serde(default)]
    pub capture: bool,
    #[serde(default)]
    pub description: String,
}

// ============================================================================
// V2 Schema file for output
// ============================================================================

/// V2 schema file containing multiple verb specs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2SchemaFile {
    /// Schema version
    #[serde(default = "v2_version")]
    pub version: String,

    /// Domain name
    pub domain: String,

    /// Domain description
    #[serde(default)]
    pub description: String,

    /// Verb specifications
    pub verbs: Vec<VerbSpec>,
}

fn v2_version() -> String {
    "2.0".to_string()
}
