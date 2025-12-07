//! Runtime verb registry built from YAML configuration
//!
//! This replaces the static STANDARD_VERBS array with a dynamic
//! registry that can be reloaded at runtime.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tracing::{info, warn};

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::config::types::*;

// =============================================================================
// RUNTIME VERB DEFINITION
// =============================================================================

/// Runtime verb definition (built from YAML config)
#[derive(Debug, Clone)]
pub struct RuntimeVerb {
    pub domain: String,
    pub verb: String,
    pub full_name: String,
    pub description: String,
    pub behavior: RuntimeBehavior,
    pub args: Vec<RuntimeArg>,
    pub returns: RuntimeReturn,
}

#[derive(Debug, Clone)]
pub enum RuntimeBehavior {
    /// Standard CRUD operation (boxed to reduce enum size)
    Crud(Box<RuntimeCrudConfig>),
    /// Plugin handler (Rust function)
    Plugin(String),
}

#[derive(Debug, Clone)]
pub struct RuntimeCrudConfig {
    pub operation: CrudOperation,
    pub table: String,
    pub schema: String,
    pub key: Option<String>,
    pub returning: Option<String>,
    pub conflict_keys: Vec<String>,
    // Junction config
    pub junction: Option<String>,
    pub from_col: Option<String>,
    pub to_col: Option<String>,
    pub role_table: Option<String>,
    pub role_col: Option<String>,
    pub fk_col: Option<String>,
    pub filter_col: Option<String>,
    // Join config
    pub primary_table: Option<String>,
    pub join_table: Option<String>,
    pub join_col: Option<String>,
    // Entity create config
    pub base_table: Option<String>,
    // List operations
    pub order_by: Option<String>,
    // Update with fixed values
    pub set_values: Option<std::collections::HashMap<String, serde_yaml::Value>>,
    pub extension_table_column: Option<String>,
    pub type_id_column: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeArg {
    pub name: String,
    pub arg_type: ArgType,
    pub required: bool,
    pub maps_to: Option<String>,
    pub lookup: Option<LookupConfig>,
    pub valid_values: Option<Vec<String>>,
    pub default: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone)]
pub struct RuntimeReturn {
    pub return_type: ReturnTypeConfig,
    pub name: Option<String>,
    pub capture: bool,
}

// =============================================================================
// RUNTIME REGISTRY
// =============================================================================

/// Runtime verb registry - can be hot-reloaded
pub struct RuntimeVerbRegistry {
    verbs: HashMap<String, RuntimeVerb>,
    by_domain: HashMap<String, Vec<String>>,
    domains: Vec<String>,
}

impl RuntimeVerbRegistry {
    /// Build registry from configuration
    pub fn from_config(config: &VerbsConfig) -> Self {
        let mut verbs = HashMap::new();
        let mut by_domain: HashMap<String, Vec<String>> = HashMap::new();

        // Process each domain
        for (domain_name, domain_config) in &config.domains {
            // Process static verbs
            for (verb_name, verb_config) in &domain_config.verbs {
                let full_name = format!("{}.{}", domain_name, verb_name);

                let runtime_verb = Self::build_verb(domain_name, verb_name, verb_config);

                verbs.insert(full_name.clone(), runtime_verb);
                by_domain
                    .entry(domain_name.clone())
                    .or_default()
                    .push(full_name);
            }
        }

        // Sort domain lists
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

    /// Build registry with dynamic verbs from database
    #[cfg(feature = "database")]
    pub async fn from_config_with_db(config: &VerbsConfig, pool: &PgPool) -> Result<Self> {
        let mut registry = Self::from_config(config);

        // Process dynamic verbs
        for (domain_name, domain_config) in &config.domains {
            for dynamic in &domain_config.dynamic_verbs {
                registry
                    .expand_dynamic_verbs(domain_name, dynamic, pool)
                    .await?;
            }
        }

        Ok(registry)
    }

    #[cfg(feature = "database")]
    async fn expand_dynamic_verbs(
        &mut self,
        domain: &str,
        dynamic: &DynamicVerbConfig,
        pool: &PgPool,
    ) -> Result<()> {
        let source = dynamic
            .source
            .as_ref()
            .ok_or_else(|| anyhow!("Dynamic verb requires source config"))?;

        let schema = source.schema.as_deref().unwrap_or("ob-poc");

        // Query entity types from database
        let query = format!(
            r#"SELECT {} FROM "{}".{}"#,
            source.type_code_column, schema, source.table
        );

        let rows: Vec<(String,)> = sqlx::query_as(&query).fetch_all(pool).await?;
        let row_count = rows.len();

        for (type_code,) in rows {
            let verb_name = dynamic.pattern.replace("{type_code}", &type_code);
            let verb_name = Self::transform_name(&verb_name, source.transform.as_deref());
            let full_name = format!("{}.{}", domain, verb_name);

            // Build verb from dynamic config
            let runtime_verb = RuntimeVerb {
                domain: domain.to_string(),
                verb: verb_name.clone(),
                full_name: full_name.clone(),
                description: format!("Create {} entity", type_code),
                behavior: RuntimeBehavior::Crud(Box::new(RuntimeCrudConfig {
                    operation: dynamic
                        .crud
                        .as_ref()
                        .map(|c| c.operation)
                        .unwrap_or(CrudOperation::EntityCreate),
                    table: dynamic
                        .crud
                        .as_ref()
                        .and_then(|c| c.base_table.clone())
                        .unwrap_or_else(|| "entities".to_string()),
                    schema: schema.to_string(),
                    key: None,
                    returning: Some("entity_id".to_string()),
                    conflict_keys: vec![],
                    junction: None,
                    from_col: None,
                    to_col: None,
                    role_table: None,
                    role_col: None,
                    fk_col: None,
                    order_by: None,
                    set_values: None,
                    filter_col: None,
                    primary_table: None,
                    join_table: None,
                    join_col: None,
                    base_table: Some("entities".to_string()),
                    extension_table_column: dynamic
                        .crud
                        .as_ref()
                        .and_then(|c| c.extension_table_column.clone()),
                    type_id_column: dynamic.crud.as_ref().and_then(|c| c.type_id_column.clone()),
                })),
                args: dynamic.base_args.iter().map(Self::convert_arg).collect(),
                returns: RuntimeReturn {
                    return_type: ReturnTypeConfig::Uuid,
                    name: Some("entity_id".to_string()),
                    capture: true,
                },
            };

            self.verbs.insert(full_name.clone(), runtime_verb);
            self.by_domain
                .entry(domain.to_string())
                .or_default()
                .push(full_name);
        }

        info!("Expanded {} dynamic verbs for domain {}", row_count, domain);
        Ok(())
    }

    fn transform_name(name: &str, transform: Option<&str>) -> String {
        match transform {
            Some("kebab_case") => name.to_lowercase().replace('_', "-"),
            Some("snake_case") => name.to_lowercase().replace('-', "_"),
            _ => name.to_string(),
        }
    }

    fn build_verb(domain: &str, verb: &str, config: &VerbConfig) -> RuntimeVerb {
        let behavior = match (&config.behavior, &config.crud, &config.handler) {
            (VerbBehavior::Crud, Some(crud), _) => {
                RuntimeBehavior::Crud(Box::new(RuntimeCrudConfig {
                    operation: crud.operation,
                    table: crud.table.clone().unwrap_or_default(),
                    schema: crud.schema.clone().unwrap_or_else(|| "ob-poc".to_string()),
                    key: crud.key.clone(),
                    returning: crud.returning.clone(),
                    conflict_keys: crud.conflict_keys.clone().unwrap_or_default(),
                    junction: crud.junction.clone(),
                    from_col: crud.from_col.clone(),
                    to_col: crud.to_col.clone(),
                    role_table: crud.role_table.clone(),
                    role_col: crud.role_col.clone(),
                    fk_col: crud.fk_col.clone(),
                    filter_col: crud.filter_col.clone(),
                    primary_table: crud.primary_table.clone(),
                    join_table: crud.join_table.clone(),
                    join_col: crud.join_col.clone(),
                    base_table: crud.base_table.clone(),
                    order_by: crud.order_by.clone(),
                    set_values: crud.set_values.clone(),
                    extension_table_column: crud.extension_table_column.clone(),
                    type_id_column: crud.type_id_column.clone(),
                }))
            }
            (VerbBehavior::Plugin, _, Some(handler)) => RuntimeBehavior::Plugin(handler.clone()),
            (VerbBehavior::Plugin, _, None) => {
                warn!(
                    "Plugin verb {}.{} missing handler, using verb name",
                    domain, verb
                );
                RuntimeBehavior::Plugin(verb.replace('-', "_"))
            }
            _ => {
                warn!(
                    "Verb {}.{} has no valid behavior config, defaulting to empty CRUD",
                    domain, verb
                );
                RuntimeBehavior::Crud(Box::new(RuntimeCrudConfig {
                    operation: CrudOperation::Select,
                    table: String::new(),
                    schema: "ob-poc".to_string(),
                    key: None,
                    returning: None,
                    conflict_keys: vec![],
                    junction: None,
                    from_col: None,
                    to_col: None,
                    role_table: None,
                    role_col: None,
                    fk_col: None,
                    filter_col: None,
                    primary_table: None,
                    join_table: None,
                    join_col: None,
                    base_table: None,
                    order_by: None,
                    set_values: None,
                    extension_table_column: None,
                    type_id_column: None,
                }))
            }
        };

        RuntimeVerb {
            domain: domain.to_string(),
            verb: verb.to_string(),
            full_name: format!("{}.{}", domain, verb),
            description: config.description.clone(),
            behavior,
            args: config.args.iter().map(Self::convert_arg).collect(),
            returns: config
                .returns
                .as_ref()
                .map(|r| RuntimeReturn {
                    return_type: r.return_type,
                    name: r.name.clone(),
                    capture: r.capture.unwrap_or(false),
                })
                .unwrap_or(RuntimeReturn {
                    return_type: ReturnTypeConfig::Void,
                    name: None,
                    capture: false,
                }),
        }
    }

    fn convert_arg(arg: &ArgConfig) -> RuntimeArg {
        RuntimeArg {
            name: arg.name.clone(),
            arg_type: arg.arg_type,
            required: arg.required,
            maps_to: arg.maps_to.clone(),
            lookup: arg.lookup.clone(),
            valid_values: arg.valid_values.clone(),
            default: arg.default.clone(),
        }
    }

    // =========================================================================
    // LOOKUP METHODS
    // =========================================================================

    pub fn get(&self, domain: &str, verb: &str) -> Option<&RuntimeVerb> {
        let key = format!("{}.{}", domain, verb);
        self.verbs.get(&key)
    }

    pub fn get_by_name(&self, full_name: &str) -> Option<&RuntimeVerb> {
        self.verbs.get(full_name)
    }

    pub fn verbs_for_domain(&self, domain: &str) -> Vec<&RuntimeVerb> {
        self.by_domain
            .get(domain)
            .map(|keys| keys.iter().filter_map(|k| self.verbs.get(k)).collect())
            .unwrap_or_default()
    }

    pub fn domains(&self) -> &[String] {
        &self.domains
    }

    pub fn all_verbs(&self) -> impl Iterator<Item = &RuntimeVerb> {
        self.verbs.values()
    }

    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    pub fn contains(&self, domain: &str, verb: &str) -> bool {
        self.get(domain, verb).is_some()
    }
}

// =============================================================================
// GLOBAL REGISTRY ACCESSOR
// =============================================================================

/// Global runtime registry instance (loaded once from YAML)
static RUNTIME_REGISTRY: OnceLock<RuntimeVerbRegistry> = OnceLock::new();

/// Get or initialize the global runtime verb registry
///
/// Loads from config/verbs.yaml on first access. Returns an empty registry
/// if loading fails (with warning logged).
pub fn runtime_registry() -> &'static RuntimeVerbRegistry {
    RUNTIME_REGISTRY.get_or_init(|| {
        use super::config::ConfigLoader;

        let loader = ConfigLoader::from_env();
        match loader.load_verbs() {
            Ok(config) => {
                let registry = RuntimeVerbRegistry::from_config(&config);
                info!(
                    "Loaded runtime verb registry: {} verbs across {} domains",
                    registry.len(),
                    registry.domains().len()
                );
                registry
            }
            Err(e) => {
                warn!("Failed to load verbs.yaml, using empty registry: {}", e);
                RuntimeVerbRegistry {
                    verbs: HashMap::new(),
                    by_domain: HashMap::new(),
                    domains: vec![],
                }
            }
        }
    })
}

// =============================================================================
// THREAD-SAFE WRAPPER
// =============================================================================

/// Thread-safe wrapper for hot-reloadable registry
#[derive(Clone)]
pub struct SharedVerbRegistry {
    inner: Arc<RwLock<RuntimeVerbRegistry>>,
}

impl SharedVerbRegistry {
    pub fn new(registry: RuntimeVerbRegistry) -> Self {
        Self {
            inner: Arc::new(RwLock::new(registry)),
        }
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, RuntimeVerbRegistry> {
        self.inner.read().await
    }

    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, RuntimeVerbRegistry> {
        self.inner.write().await
    }

    pub fn clone_inner(&self) -> Arc<RwLock<RuntimeVerbRegistry>> {
        self.inner.clone()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> VerbsConfig {
        let mut domains = HashMap::new();

        let mut cbu_verbs = HashMap::new();
        cbu_verbs.insert(
            "create".to_string(),
            VerbConfig {
                description: "Create a CBU".to_string(),
                behavior: VerbBehavior::Crud,
                handler: None,
                crud: Some(CrudConfig {
                    operation: CrudOperation::Insert,
                    table: Some("cbus".to_string()),
                    schema: Some("ob-poc".to_string()),
                    key: None,
                    returning: Some("cbu_id".to_string()),
                    conflict_keys: None,
                    junction: None,
                    from_col: None,
                    to_col: None,
                    role_table: None,
                    role_col: None,
                    fk_col: None,
                    filter_col: None,
                    primary_table: None,
                    join_table: None,
                    join_col: None,
                    base_table: None,
                    order_by: None,
                    set_values: None,
                    extension_table_column: None,
                    type_id_column: None,
                }),
                args: vec![ArgConfig {
                    name: "name".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                    maps_to: Some("name".to_string()),
                    lookup: None,
                    valid_values: None,
                    default: None,
                }],
                returns: Some(ReturnsConfig {
                    return_type: ReturnTypeConfig::Uuid,
                    name: Some("cbu_id".to_string()),
                    capture: Some(true),
                }),
            },
        );

        domains.insert(
            "cbu".to_string(),
            DomainConfig {
                description: "CBU operations".to_string(),
                verbs: cbu_verbs,
                dynamic_verbs: vec![],
            },
        );

        VerbsConfig {
            version: "1.0".to_string(),
            domains,
        }
    }

    #[test]
    fn test_build_registry() {
        let config = create_test_config();
        let registry = RuntimeVerbRegistry::from_config(&config);

        assert_eq!(registry.len(), 1);
        assert!(registry.contains("cbu", "create"));
    }

    #[test]
    fn test_get_verb() {
        let config = create_test_config();
        let registry = RuntimeVerbRegistry::from_config(&config);

        let verb = registry.get("cbu", "create").unwrap();
        assert_eq!(verb.domain, "cbu");
        assert_eq!(verb.verb, "create");
        assert_eq!(verb.description, "Create a CBU");
    }

    #[test]
    fn test_domains() {
        let config = create_test_config();
        let registry = RuntimeVerbRegistry::from_config(&config);

        let domains = registry.domains();
        assert!(domains.contains(&"cbu".to_string()));
    }

    #[test]
    fn test_transform_name() {
        assert_eq!(
            RuntimeVerbRegistry::transform_name("PROPER_PERSON", Some("kebab_case")),
            "proper-person"
        );
        assert_eq!(
            RuntimeVerbRegistry::transform_name("proper-person", Some("snake_case")),
            "proper_person"
        );
        assert_eq!(
            RuntimeVerbRegistry::transform_name("unchanged", None),
            "unchanged"
        );
    }
}
