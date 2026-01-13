//! Runtime verb registry built from YAML configuration
//!
//! This replaces the static STANDARD_VERBS array with a dynamic
//! registry that can be reloaded at runtime.
//!
//! Also loads templates from config/verbs/templates/ as first-class
//! language constructs (macros that expand to DSL statements).

#[cfg(feature = "database")]
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tracing::{info, warn};

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::config::types::*;
use crate::templates::{TemplateDefinition, TemplateRegistry};

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
    /// Dataflow: what this verb produces (binding type)
    pub produces: Option<VerbProduces>,
    /// Dataflow: what this verb consumes (required bindings)
    pub consumes: Vec<VerbConsumes>,
    /// Lifecycle constraints and transitions for this verb
    pub lifecycle: Option<VerbLifecycle>,
}

#[derive(Debug, Clone)]
pub enum RuntimeBehavior {
    /// Standard CRUD operation (boxed to reduce enum size)
    Crud(Box<RuntimeCrudConfig>),
    /// Plugin handler (Rust function)
    Plugin(String),
    /// Graph query operation
    GraphQuery(Box<RuntimeGraphQueryConfig>),
}

#[derive(Debug, Clone)]
pub struct RuntimeCrudConfig {
    pub operation: CrudOperation,
    pub table: String,
    pub schema: String,
    pub key: Option<String>,
    pub returning: Option<String>,
    pub conflict_keys: Vec<String>,
    /// Named constraint for ON CONFLICT (used when conflict_keys has computed columns)
    pub conflict_constraint: Option<String>,
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
    pub extension_table: Option<String>,
    /// Explicit type_code for entity_create (e.g., "fund_umbrella")
    /// If not set, derived from verb name
    pub type_code: Option<String>,
    // List operations
    pub order_by: Option<String>,
    // Update with fixed values
    pub set_values: Option<std::collections::HashMap<String, serde_yaml::Value>>,
    pub extension_table_column: Option<String>,
    pub type_id_column: Option<String>,
}

/// Configuration for graph query operations
#[derive(Debug, Clone)]
pub struct RuntimeGraphQueryConfig {
    /// The type of graph query operation
    pub operation: GraphQueryOperation,
    /// Root entity type for the query (e.g., "cbu", "entity")
    pub root_type: Option<String>,
    /// Edge types to include in traversal
    pub edge_types: Vec<String>,
    /// Maximum traversal depth
    pub max_depth: u32,
    /// Default view mode for visualization queries
    pub default_view_mode: Option<String>,
}

use super::config::types::GraphQueryOperation;

#[derive(Debug, Clone)]
pub struct RuntimeArg {
    pub name: String,
    pub arg_type: ArgType,
    pub required: bool,
    pub maps_to: Option<String>,
    pub lookup: Option<LookupConfig>,
    pub valid_values: Option<Vec<String>>,
    pub default: Option<serde_yaml::Value>,
    pub description: Option<String>,
    pub fuzzy_check: Option<FuzzyCheckConfig>,
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
///
/// Contains both verbs and templates, loaded at startup from:
/// - config/verbs/*.yaml (verb definitions)
/// - config/verbs/templates/**/*.yaml (template definitions)
pub struct RuntimeVerbRegistry {
    verbs: HashMap<String, RuntimeVerb>,
    by_domain: HashMap<String, Vec<String>>,
    domains: Vec<String>,
    /// Template registry (macros that expand to DSL statements)
    templates: TemplateRegistry,
}

impl RuntimeVerbRegistry {
    /// Build registry from configuration
    pub fn from_config(config: &VerbsConfig) -> Self {
        Self::from_config_with_templates(config, TemplateRegistry::new())
    }

    /// Build registry from configuration with template directory
    pub fn from_config_and_templates_dir(config: &VerbsConfig, templates_dir: &Path) -> Self {
        let templates = match TemplateRegistry::load_from_dir(templates_dir) {
            Ok(registry) => {
                info!(
                    "Loaded {} templates from {:?}",
                    registry.len(),
                    templates_dir
                );
                registry
            }
            Err(e) => {
                warn!("Failed to load templates from {:?}: {}", templates_dir, e);
                TemplateRegistry::new()
            }
        };
        Self::from_config_with_templates(config, templates)
    }

    /// Build registry from configuration with pre-loaded templates
    pub fn from_config_with_templates(config: &VerbsConfig, templates: TemplateRegistry) -> Self {
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
            templates,
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
                    conflict_constraint: None,
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
                    extension_table: dynamic
                        .crud
                        .as_ref()
                        .and_then(|c| c.extension_table.clone()),
                    // For dynamic verbs, type_code comes from the database row
                    type_code: Some(type_code.clone()),
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
                // Dynamic entity.create-* verbs produce an entity with subtype
                produces: Some(VerbProduces {
                    produced_type: "entity".to_string(),
                    subtype: Some(type_code.clone()),
                    subtype_from_arg: None,
                    resolved: false,
                    initial_state: Some("DRAFT".to_string()),
                }),
                consumes: vec![],
                lifecycle: None, // Dynamic verbs don't have lifecycle constraints by default
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

    #[cfg(any(feature = "database", test))]
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
                // For entity_create/entity_upsert operations, table defaults to base_table
                let table = crud.table.clone().unwrap_or_else(|| {
                    if matches!(
                        crud.operation,
                        CrudOperation::EntityCreate | CrudOperation::EntityUpsert
                    ) {
                        crud.base_table.clone().unwrap_or_default()
                    } else {
                        String::new()
                    }
                });
                RuntimeBehavior::Crud(Box::new(RuntimeCrudConfig {
                    operation: crud.operation,
                    table,
                    schema: crud.schema.clone().unwrap_or_else(|| "ob-poc".to_string()),
                    key: crud.key.clone(),
                    returning: crud.returning.clone(),
                    conflict_keys: crud.conflict_keys.clone().unwrap_or_default(),
                    conflict_constraint: crud.conflict_constraint.clone(),
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
                    extension_table: crud.extension_table.clone(),
                    type_code: crud.type_code.clone(),
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
            (VerbBehavior::GraphQuery, _, _) => {
                let graph_query = config.graph_query.as_ref();
                RuntimeBehavior::GraphQuery(Box::new(RuntimeGraphQueryConfig {
                    operation: graph_query
                        .map(|g| g.operation)
                        .unwrap_or(GraphQueryOperation::View),
                    root_type: graph_query.and_then(|g| g.root_type.clone()),
                    edge_types: graph_query
                        .map(|g| g.edge_types.clone())
                        .unwrap_or_default(),
                    max_depth: graph_query.map(|g| g.max_depth).unwrap_or(10),
                    default_view_mode: graph_query.and_then(|g| g.default_view_mode.clone()),
                }))
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
                    conflict_constraint: None,
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
                    extension_table: None,
                    type_code: None,
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
            produces: config.produces.clone(),
            consumes: config.consumes.clone(),
            lifecycle: config.lifecycle.clone(),
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
            description: arg.description.clone(),
            fuzzy_check: arg.fuzzy_check.clone(),
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

    // =========================================================================
    // TEMPLATE METHODS
    // =========================================================================

    /// Get the template registry
    pub fn templates(&self) -> &TemplateRegistry {
        &self.templates
    }

    /// Get a template by ID
    pub fn get_template(&self, id: &str) -> Option<&TemplateDefinition> {
        self.templates.get(id)
    }

    /// Find templates by tag
    pub fn find_templates_by_tag(&self, tag: &str) -> Vec<&TemplateDefinition> {
        self.templates.find_by_tag(tag)
    }

    /// Find templates that resolve a blocker
    pub fn find_templates_by_blocker(&self, blocker_type: &str) -> Vec<&TemplateDefinition> {
        self.templates.find_by_blocker(blocker_type)
    }

    /// Find templates for a workflow state
    pub fn find_templates_by_workflow_state(
        &self,
        workflow: &str,
        state: &str,
    ) -> Vec<&TemplateDefinition> {
        self.templates.find_by_workflow_state(workflow, state)
    }

    /// Search templates by text
    pub fn search_templates(&self, query: &str) -> Vec<&TemplateDefinition> {
        self.templates.search(query)
    }

    /// List all templates
    pub fn list_templates(&self) -> Vec<&TemplateDefinition> {
        self.templates.list()
    }

    /// Get template count
    pub fn template_count(&self) -> usize {
        self.templates.len()
    }

    // =========================================================================
    // DATAFLOW METHODS
    // =========================================================================

    /// Get what a verb produces (if anything)
    pub fn get_produces(&self, domain: &str, verb: &str) -> Option<&VerbProduces> {
        self.get(domain, verb)?.produces.as_ref()
    }

    /// Get what a verb consumes
    pub fn get_consumes(&self, domain: &str, verb: &str) -> &[VerbConsumes] {
        self.get(domain, verb)
            .map(|v| v.consumes.as_slice())
            .unwrap_or(&[])
    }

    /// Get the expected ref_type for an argument (from lookup config)
    pub fn get_arg_ref_type(&self, domain: &str, verb: &str, arg: &str) -> Option<&str> {
        self.get(domain, verb)?
            .args
            .iter()
            .find(|a| a.name == arg)?
            .lookup
            .as_ref()?
            .entity_type
            .as_deref()
    }

    /// Get the full lookup config for an argument
    ///
    /// This provides access to the SearchKeyConfig including discriminators,
    /// which is needed for disambiguation during resolution.
    pub fn get_arg_lookup(&self, domain: &str, verb: &str, arg: &str) -> Option<&LookupConfig> {
        self.get(domain, verb)?
            .args
            .iter()
            .find(|a| a.name == arg)?
            .lookup
            .as_ref()
    }

    /// Get all verbs that can execute given available binding types
    /// Returns verbs where all required consumes are satisfied
    pub fn verbs_satisfiable_by<'a>(
        &'a self,
        available_types: &'a std::collections::HashSet<String>,
    ) -> impl Iterator<Item = &'a RuntimeVerb> {
        self.verbs.values().filter(move |v| {
            v.consumes
                .iter()
                .all(|c| !c.required || available_types.contains(&c.consumed_type))
        })
    }

    /// Get verbs grouped by satisfaction status
    pub fn verbs_by_satisfaction<'a>(
        &'a self,
        available_types: &'a std::collections::HashSet<String>,
    ) -> (Vec<&'a RuntimeVerb>, Vec<&'a RuntimeVerb>) {
        let mut satisfied = vec![];
        let mut unsatisfied = vec![];

        for verb in self.verbs.values() {
            let all_satisfied = verb
                .consumes
                .iter()
                .all(|c| !c.required || available_types.contains(&c.consumed_type));
            if all_satisfied {
                satisfied.push(verb);
            } else {
                unsatisfied.push(verb);
            }
        }

        (satisfied, unsatisfied)
    }
}

// =============================================================================
// GLOBAL REGISTRY ACCESSOR
// =============================================================================

/// Global runtime registry instance (loaded once from YAML)
static RUNTIME_REGISTRY: OnceLock<RuntimeVerbRegistry> = OnceLock::new();

/// Get or initialize the global runtime verb registry
///
/// Loads from config/verbs/*.yaml and config/verbs/templates/**/*.yaml on first access.
/// Returns an empty registry if loading fails (with warning logged).
pub fn runtime_registry() -> &'static RuntimeVerbRegistry {
    RUNTIME_REGISTRY.get_or_init(|| {
        use super::config::ConfigLoader;

        let loader = ConfigLoader::from_env();
        match loader.load_verbs() {
            Ok(config) => {
                // Load templates from config/verbs/templates/
                let templates_dir = loader.config_dir().join("templates");
                let registry =
                    RuntimeVerbRegistry::from_config_and_templates_dir(&config, &templates_dir);
                info!(
                    "Loaded runtime registry: {} verbs across {} domains, {} templates",
                    registry.len(),
                    registry.domains().len(),
                    registry.template_count()
                );
                registry
            }
            Err(e) => {
                warn!("Failed to load verbs.yaml, using empty registry: {}", e);
                RuntimeVerbRegistry {
                    verbs: HashMap::new(),
                    by_domain: HashMap::new(),
                    domains: vec![],
                    templates: TemplateRegistry::new(),
                }
            }
        }
    })
}

/// Global Arc-wrapped registry for use with PlanningInput
static RUNTIME_REGISTRY_ARC: OnceLock<Arc<RuntimeVerbRegistry>> = OnceLock::new();

/// Get an Arc-wrapped runtime verb registry for use with PlanningInput
///
/// This loads a fresh registry (not sharing the static reference) wrapped in Arc.
/// Use this when you need `Arc<RuntimeVerbRegistry>` for planning operations.
pub fn runtime_registry_arc() -> Arc<RuntimeVerbRegistry> {
    RUNTIME_REGISTRY_ARC
        .get_or_init(|| {
            use super::config::ConfigLoader;

            let loader = ConfigLoader::from_env();
            match loader.load_verbs() {
                Ok(config) => {
                    let templates_dir = loader.config_dir().join("templates");
                    let registry =
                        RuntimeVerbRegistry::from_config_and_templates_dir(&config, &templates_dir);
                    info!(
                        "Loaded Arc runtime registry: {} verbs, {} templates",
                        registry.len(),
                        registry.template_count()
                    );
                    Arc::new(registry)
                }
                Err(e) => {
                    warn!(
                        "Failed to load verbs.yaml for Arc registry, using empty: {}",
                        e
                    );
                    Arc::new(RuntimeVerbRegistry {
                        verbs: HashMap::new(),
                        by_domain: HashMap::new(),
                        domains: vec![],
                        templates: TemplateRegistry::new(),
                    })
                }
            }
        })
        .clone()
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
                produces: None,
                consumes: vec![],
                lifecycle: None,
                graph_query: None,
                handler: None,
                crud: Some(CrudConfig {
                    operation: CrudOperation::Insert,
                    table: Some("cbus".to_string()),
                    schema: Some("ob-poc".to_string()),
                    key: None,
                    returning: Some("cbu_id".to_string()),
                    conflict_keys: None,
                    conflict_constraint: None,
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
                    extension_table: None,
                    order_by: None,
                    set_values: None,
                    extension_table_column: None,
                    type_id_column: None,
                    type_code: None,
                }),
                args: vec![ArgConfig {
                    name: "name".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                    maps_to: Some("name".to_string()),
                    lookup: None,
                    valid_values: None,
                    default: None,
                    description: None,
                    validation: None,
                    fuzzy_check: None,
                }],
                returns: Some(ReturnsConfig {
                    return_type: ReturnTypeConfig::Uuid,
                    name: Some("cbu_id".to_string()),
                    capture: Some(true),
                }),
                metadata: None,
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

    #[test]
    fn test_load_templates_from_directory() {
        use std::path::Path;

        // Load from actual config directory
        let templates_dir = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/config/verbs/templates"
        ));

        if templates_dir.exists() {
            let config = create_test_config();
            let registry =
                RuntimeVerbRegistry::from_config_and_templates_dir(&config, templates_dir);

            // Should have loaded templates
            assert!(
                registry.template_count() > 0,
                "Should have loaded templates"
            );

            // Check specific templates exist
            assert!(
                registry.get_template("onboard-director").is_some(),
                "Should have onboard-director template"
            );
            assert!(
                registry.get_template("create-kyc-case").is_some(),
                "Should have create-kyc-case template"
            );

            // Check template search works
            let director_templates = registry.find_templates_by_tag("director");
            assert!(
                !director_templates.is_empty(),
                "Should find templates by tag"
            );

            // Check primary_entity
            let template = registry.get_template("onboard-director").unwrap();
            assert!(
                template.primary_entity.is_some(),
                "Template should have primary_entity"
            );
            assert!(
                template.is_cbu_scoped(),
                "onboard-director should be CBU scoped"
            );

            println!("Loaded {} templates", registry.template_count());
            for t in registry.list_templates() {
                println!("  - {} ({})", t.template, t.metadata.summary);
            }
        }
    }
}
