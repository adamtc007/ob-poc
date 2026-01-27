//! V2 Verb Registry Loader
//!
//! Loads verb definitions from the compiled V2 registry.json artifact.
//! This is the new canonical source of verb schemas, replacing V1 YAML parsing.
//!
//! # Migration Path
//!
//! ```text
//! V1 (deprecated):  config/verbs/*.yaml → RuntimeVerbRegistry
//! V2 (canonical):   config/verb_schemas/registry.json → RuntimeVerbRegistry
//! ```
//!
//! The V2 format provides:
//! - Pre-generated invocation phrases (deterministic, no LLM)
//! - Positional sugar definitions
//! - Aliases with collision detection
//! - Inline arg schemas (HashMap style)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

use super::config::types::{ArgType, LookupConfig, ReturnTypeConfig, SearchKeyConfig};
use super::runtime_registry::{
    RuntimeArg, RuntimeBehavior, RuntimeReturn, RuntimeVerb, RuntimeVerbRegistry,
};
use crate::templates::TemplateRegistry;

// =============================================================================
// V2 SCHEMA TYPES (mirrors xtask/src/verb_migrate.rs)
// =============================================================================

/// V2 Registry file (compiled artifact)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2Registry {
    pub version: String,
    pub generated: String,
    pub verb_count: usize,
    pub alias_count: usize,
    pub collisions: usize,
    pub verbs: Vec<V2VerbSpec>,
}

/// V2 Verb specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2VerbSpec {
    pub verb: String,
    pub domain: String,
    pub action: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub args: V2ArgSchema,
    #[serde(default)]
    pub positional_sugar: Vec<String>,
    #[serde(default)]
    pub invocation_phrases: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub doc: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2ArgSchema {
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default)]
    pub required: HashMap<String, V2ArgType>,
    #[serde(default)]
    pub optional: HashMap<String, V2ArgType>,
}

fn default_style() -> String {
    "keyworded".to_string()
}

impl Default for V2ArgSchema {
    fn default() -> Self {
        Self {
            style: "keyworded".to_string(),
            required: HashMap::new(),
            optional: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2ArgType {
    #[serde(rename = "type")]
    pub typ: String,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub values: Option<Vec<String>>,
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
}

// =============================================================================
// V2 REGISTRY LOADER
// =============================================================================

/// Load V2 registry from compiled JSON artifact
pub fn load_v2_registry(path: &Path) -> Result<V2Registry> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read V2 registry from {:?}", path))?;

    let registry: V2Registry = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse V2 registry JSON from {:?}", path))?;

    info!(
        "Loaded V2 registry: {} verbs, {} aliases, {} collisions (generated: {})",
        registry.verb_count, registry.alias_count, registry.collisions, registry.generated
    );

    Ok(registry)
}

/// Convert V2 registry to RuntimeVerbRegistry
///
/// This allows the existing DSL execution pipeline to work unchanged,
/// while using V2 schemas as the source of truth.
pub fn v2_to_runtime_registry(v2: &V2Registry) -> RuntimeVerbRegistry {
    v2_to_runtime_registry_with_templates(v2, TemplateRegistry::new())
}

/// Convert V2 registry to RuntimeVerbRegistry with templates
pub fn v2_to_runtime_registry_with_templates(
    v2: &V2Registry,
    templates: TemplateRegistry,
) -> RuntimeVerbRegistry {
    let mut verbs = HashMap::new();
    let mut by_domain: HashMap<String, Vec<String>> = HashMap::new();

    for spec in &v2.verbs {
        let runtime_verb = v2_spec_to_runtime_verb(spec);
        let full_name = runtime_verb.full_name.clone();
        let domain = runtime_verb.domain.clone();

        verbs.insert(full_name.clone(), runtime_verb);
        by_domain.entry(domain).or_default().push(full_name);
    }

    // Sort domain lists
    for list in by_domain.values_mut() {
        list.sort();
        list.dedup();
    }

    let mut domains: Vec<String> = by_domain.keys().cloned().collect();
    domains.sort();

    RuntimeVerbRegistry::from_parts(verbs, by_domain, domains, templates)
}

/// Convert a single V2 spec to RuntimeVerb
fn v2_spec_to_runtime_verb(spec: &V2VerbSpec) -> RuntimeVerb {
    // Convert args
    let mut args = Vec::new();

    // Add required args
    for (name, arg_type) in &spec.args.required {
        args.push(v2_arg_to_runtime_arg(name, arg_type, true));
    }

    // Add optional args
    for (name, arg_type) in &spec.args.optional {
        args.push(v2_arg_to_runtime_arg(name, arg_type, false));
    }

    // Determine behavior (default to Plugin since V2 doesn't store behavior details)
    // The actual CRUD/Plugin/GraphQuery config comes from the V1 YAML merged later
    let behavior = RuntimeBehavior::Plugin(spec.action.replace('-', "_"));

    RuntimeVerb {
        domain: spec.domain.clone(),
        verb: spec.action.clone(),
        full_name: spec.verb.clone(),
        description: spec.doc.clone(),
        behavior,
        args,
        returns: RuntimeReturn {
            return_type: ReturnTypeConfig::Void,
            name: None,
            capture: false,
        },
        produces: None,
        consumes: vec![],
        lifecycle: None,
        policy: None,
    }
}

/// Convert V2 arg to RuntimeArg
fn v2_arg_to_runtime_arg(name: &str, arg_type: &V2ArgType, required: bool) -> RuntimeArg {
    let (rust_type, lookup) = convert_v2_type(&arg_type.typ);

    // Convert default value from JSON to YAML
    let default = arg_type
        .default
        .as_ref()
        .and_then(|v| serde_yaml::to_value(v).ok());

    RuntimeArg {
        name: name.to_string(),
        arg_type: rust_type,
        required,
        maps_to: Some(name.to_string()),
        lookup,
        valid_values: arg_type.values.clone(),
        default,
        description: None,
        fuzzy_check: None,
    }
}

/// Convert V2 type string to ArgType and optional LookupConfig
fn convert_v2_type(typ: &str) -> (ArgType, Option<LookupConfig>) {
    match typ {
        "str" | "string" => (ArgType::String, None),
        "int" | "integer" => (ArgType::Integer, None),
        "bool" | "boolean" => (ArgType::Boolean, None),
        "uuid" => (ArgType::Uuid, None),
        "decimal" | "numeric" => (ArgType::Decimal, None),
        "date" => (ArgType::Date, None),
        "datetime" | "timestamp" => (ArgType::Timestamp, None),
        "json" | "object" => (ArgType::Json, None),
        "list" => (ArgType::StringList, None),
        "uuid_list" | "uuid_array" => (ArgType::UuidList, None),
        "enum" => (ArgType::String, None), // Enum stored as string, valid_values constrains
        "entity_ref" => (ArgType::Uuid, None),
        "entity_name" => {
            // Entity name implies lookup
            let lookup = LookupConfig {
                table: "entities".to_string(),
                schema: Some("ob-poc".to_string()),
                search_key: SearchKeyConfig::Simple("name".to_string()),
                primary_key: "entity_id".to_string(),
                entity_type: Some("entity".to_string()),
                resolution_mode: None,
                scope_key: None,
                role_filter: None,
            };
            (ArgType::String, Some(lookup))
        }
        _ => {
            warn!("Unknown V2 arg type '{}', defaulting to String", typ);
            (ArgType::String, None)
        }
    }
}

// =============================================================================
// V2 INVOCATION PHRASE EXTRACTION
// =============================================================================

/// Extract all invocation phrases from V2 registry
///
/// Returns a map of verb_full_name -> invocation_phrases for syncing to database.
pub fn extract_invocation_phrases(v2: &V2Registry) -> HashMap<String, Vec<String>> {
    v2.verbs
        .iter()
        .map(|spec| (spec.verb.clone(), spec.invocation_phrases.clone()))
        .collect()
}

/// Extract aliases with collision information
///
/// Returns (alias -> [verbs]) map. Collisions are where len > 1.
pub fn extract_aliases(v2: &V2Registry) -> HashMap<String, Vec<String>> {
    let mut alias_map: HashMap<String, Vec<String>> = HashMap::new();

    for spec in &v2.verbs {
        // Add verb name as alias
        if let Some(action) = spec.verb.split('.').next_back() {
            alias_map
                .entry(action.to_lowercase())
                .or_default()
                .push(spec.verb.clone());
        }

        // Add explicit aliases
        for alias in &spec.aliases {
            alias_map
                .entry(alias.to_lowercase())
                .or_default()
                .push(spec.verb.clone());
        }
    }

    // Dedupe
    for verbs in alias_map.values_mut() {
        verbs.sort();
        verbs.dedup();
    }

    alias_map
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_v2_registry() -> V2Registry {
        V2Registry {
            version: "2.0".to_string(),
            generated: "2026-01-27T00:00:00Z".to_string(),
            verb_count: 2,
            alias_count: 3,
            collisions: 0,
            verbs: vec![
                V2VerbSpec {
                    verb: "cbu.create".to_string(),
                    domain: "cbu".to_string(),
                    action: "create".to_string(),
                    aliases: vec!["create".to_string(), "add".to_string()],
                    args: V2ArgSchema {
                        style: "keyworded".to_string(),
                        required: {
                            let mut m = HashMap::new();
                            m.insert(
                                "name".to_string(),
                                V2ArgType {
                                    typ: "str".to_string(),
                                    default: None,
                                    values: None,
                                    kinds: None,
                                },
                            );
                            m
                        },
                        optional: HashMap::new(),
                    },
                    positional_sugar: vec!["name".to_string()],
                    invocation_phrases: vec![
                        "create a cbu".to_string(),
                        "onboard a client".to_string(),
                        "add cbu".to_string(),
                    ],
                    examples: vec!["(cbu.create :name \"Acme\")".to_string()],
                    doc: "Create a new CBU".to_string(),
                    tier: "crud".to_string(),
                    tags: vec!["cbu".to_string()],
                },
                V2VerbSpec {
                    verb: "session.load-galaxy".to_string(),
                    domain: "session".to_string(),
                    action: "load-galaxy".to_string(),
                    aliases: vec!["load-galaxy".to_string()],
                    args: V2ArgSchema {
                        style: "keyworded".to_string(),
                        required: HashMap::new(),
                        optional: {
                            let mut m = HashMap::new();
                            m.insert(
                                "apex-name".to_string(),
                                V2ArgType {
                                    typ: "entity_name".to_string(),
                                    default: None,
                                    values: None,
                                    kinds: None,
                                },
                            );
                            m
                        },
                    },
                    positional_sugar: vec!["apex-name".to_string()],
                    invocation_phrases: vec![
                        "load the allianz book".to_string(),
                        "open galaxy".to_string(),
                    ],
                    examples: vec!["(session.load-galaxy :apex-name \"Allianz\")".to_string()],
                    doc: "Load CBUs under apex entity".to_string(),
                    tier: "intent".to_string(),
                    tags: vec!["session".to_string(), "navigation".to_string()],
                },
            ],
        }
    }

    #[test]
    fn test_v2_to_runtime_registry() {
        let v2 = create_test_v2_registry();
        let registry = v2_to_runtime_registry(&v2);

        assert_eq!(registry.len(), 2);
        assert!(registry.contains("cbu", "create"));
        assert!(registry.contains("session", "load-galaxy"));
    }

    #[test]
    fn test_extract_invocation_phrases() {
        let v2 = create_test_v2_registry();
        let phrases = extract_invocation_phrases(&v2);

        assert_eq!(phrases.len(), 2);
        assert!(phrases
            .get("cbu.create")
            .unwrap()
            .contains(&"create a cbu".to_string()));
        assert!(phrases
            .get("session.load-galaxy")
            .unwrap()
            .contains(&"load the allianz book".to_string()));
    }

    #[test]
    fn test_extract_aliases() {
        let v2 = create_test_v2_registry();
        let aliases = extract_aliases(&v2);

        // "create" should map to cbu.create
        assert!(aliases
            .get("create")
            .unwrap()
            .contains(&"cbu.create".to_string()));
        // "add" should also map to cbu.create
        assert!(aliases
            .get("add")
            .unwrap()
            .contains(&"cbu.create".to_string()));
    }

    #[test]
    fn test_convert_v2_type() {
        assert!(matches!(convert_v2_type("str").0, ArgType::String));
        assert!(matches!(convert_v2_type("int").0, ArgType::Integer));
        assert!(matches!(convert_v2_type("uuid").0, ArgType::Uuid));
        assert!(matches!(convert_v2_type("entity_name").0, ArgType::String));

        // entity_name should have lookup
        let (_, lookup) = convert_v2_type("entity_name");
        assert!(lookup.is_some());
    }
}
