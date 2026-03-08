//! Verb metadata index for Phase 2 Coder resolution.
//!
//! This is a read-only projection over verb YAML and runtime registry data.
//! It precomputes plane, polarity, action tags, and argument names so the
//! Coder can rank verbs without embedding search.

use std::collections::HashMap;

use anyhow::Result;
use dsl_core::config::loader::ConfigLoader;
use dsl_core::config::types::{CrudOperation, VerbConfig, VerbMetadata, VerbTier, VerbsConfig};

use crate::dsl_v2::runtime_registry::RuntimeVerbRegistry;

use super::{IntentPolarity, ObservationPlane};

/// Precomputed metadata for one verb.
#[derive(Debug, Clone)]
pub struct VerbMeta {
    pub fqn: String,
    pub domain: String,
    pub verb_name: String,
    pub polarity: IntentPolarity,
    pub side_effects: Option<String>,
    pub planes: Vec<ObservationPlane>,
    pub action_tags: Vec<String>,
    pub param_names: Vec<String>,
    pub required_params: Vec<String>,
    pub description: String,
}

/// Read-only index over all configured verbs.
#[derive(Debug, Clone, Default)]
pub struct VerbMetadataIndex {
    by_fqn: HashMap<String, VerbMeta>,
}

impl VerbMetadataIndex {
    /// Build the index from loaded verb configuration.
    ///
    /// # Examples
    /// ```ignore
    /// use dsl_core::config::loader::ConfigLoader;
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let config = ConfigLoader::from_env().load_verbs()?;
    /// let index = VerbMetadataIndex::from_config(&config);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn from_config(config: &VerbsConfig) -> Self {
        let mut by_fqn = HashMap::new();

        for (domain, domain_config) in &config.domains {
            for (verb_name, verb_config) in &domain_config.verbs {
                let meta = Self::build_meta(domain, verb_name, verb_config);
                by_fqn.insert(meta.fqn.clone(), meta);
            }
        }

        Self { by_fqn }
    }

    /// Build the index from the default runtime configuration.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let index = VerbMetadataIndex::load()?;
    /// println!("{}", index.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load() -> Result<Self> {
        let config = ConfigLoader::from_env().load_verbs()?;
        Ok(Self::from_config(&config))
    }

    /// Return metadata for a verb FQN.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let index = VerbMetadataIndex::load()?;
    /// let meta = index.get("cbu.create");
    /// assert!(meta.is_some());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn get(&self, fqn: &str) -> Option<&VerbMeta> {
        self.by_fqn.get(fqn)
    }

    /// Iterate over all verb metadata rows.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let index = VerbMetadataIndex::load()?;
    /// for meta in index.iter() {
    ///     println!("{}", meta.fqn);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &VerbMeta> {
        self.by_fqn.values()
    }

    /// Iterate only verbs that are safe to serve directly.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let index = VerbMetadataIndex::load()?;
    /// assert!(index.facts_only_verbs().count() > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn facts_only_verbs(&self) -> impl Iterator<Item = &VerbMeta> {
        self.iter()
            .filter(|meta| meta.side_effects.as_deref() == Some("facts_only"))
    }

    /// Iterate only verbs that mutate state and require confirmation.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let index = VerbMetadataIndex::load()?;
    /// assert!(index.state_write_verbs().count() > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn state_write_verbs(&self) -> impl Iterator<Item = &VerbMeta> {
        self.iter()
            .filter(|meta| meta.side_effects.as_deref() == Some("state_write"))
    }

    /// Query verbs by plane, polarity, and optional domain hint.
    ///
    /// When `domain_hint` is `None` or empty, this returns every verb matching
    /// the requested plane and polarity. When a domain hint is present, verbs
    /// also match on prefix overlap and action-tag overlap so Sage noun hints do
    /// not have to be exact domain-prefix matches.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::{IntentPolarity, ObservationPlane};
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let index = VerbMetadataIndex::load()?;
    /// let matches = index.query(
    ///     ObservationPlane::Instance,
    ///     IntentPolarity::Write,
    ///     Some("cbu"),
    /// );
    /// assert!(!matches.is_empty());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn query(
        &self,
        plane: ObservationPlane,
        polarity: IntentPolarity,
        domain_hint: Option<&str>,
    ) -> Vec<&VerbMeta> {
        self.iter()
            .filter(|meta| meta.planes.contains(&plane))
            .filter(|meta| meta.polarity == polarity || polarity == IntentPolarity::Ambiguous)
            .filter(|meta| self.matches_domain(meta, domain_hint))
            .collect()
    }

    /// Number of indexed verbs.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let index = VerbMetadataIndex::load()?;
    /// assert!(index.len() > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn len(&self) -> usize {
        self.by_fqn.len()
    }

    /// Whether the index is empty.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    ///
    /// let index = VerbMetadataIndex::default();
    /// assert!(index.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.by_fqn.is_empty()
    }

    fn matches_domain(&self, verb: &VerbMeta, domain_hint: Option<&str>) -> bool {
        let Some(hint) = domain_hint
            .map(str::trim)
            .filter(|hint| !hint.is_empty() && *hint != "unknown")
        else {
            return true;
        };

        let hint = hint.to_ascii_lowercase();
        let domain = verb.domain.to_ascii_lowercase();
        if domain == hint || domain.starts_with(&hint) || hint.starts_with(&domain) {
            return true;
        }

        verb.action_tags
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case(&hint))
    }

    fn build_meta(domain: &str, verb_name: &str, config: &VerbConfig) -> VerbMeta {
        let fqn = format!("{}.{}", domain, verb_name);
        let polarity = classify_polarity(domain, verb_name, config);
        let planes = classify_planes(domain, config.metadata.as_ref());
        let action_tags = action_tags(domain, verb_name, config, polarity);
        let param_names = config.args.iter().map(|arg| arg.name.clone()).collect();
        let required_params = config
            .args
            .iter()
            .filter(|arg| arg.required)
            .map(|arg| arg.name.clone())
            .collect();

        VerbMeta {
            fqn,
            domain: domain.to_string(),
            verb_name: verb_name.to_string(),
            polarity,
            side_effects: config
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.side_effects.clone()),
            planes,
            action_tags,
            param_names,
            required_params,
            description: config.description.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_test_map(by_fqn: HashMap<String, VerbMeta>) -> Self {
        Self { by_fqn }
    }
}

fn classify_polarity(domain: &str, verb_name: &str, config: &VerbConfig) -> IntentPolarity {
    let read_prefixes = [
        "list",
        "show",
        "get",
        "read",
        "search",
        "find",
        "trace",
        "discover",
        "describe",
        "report",
        "analyze",
        "analyse",
        "compute",
        "calculate",
        "for-",
        "who-",
        "identify",
        "missing",
        "timeline",
        "catalog",
        "coverage-matrix",
        "case-status",
        "check-readiness",
    ];

    if read_prefixes
        .iter()
        .any(|prefix| verb_name.starts_with(prefix))
    {
        return IntentPolarity::Read;
    }

    if let Some(crud) = &config.crud {
        return match crud.operation {
            CrudOperation::Select
            | CrudOperation::ListByFk
            | CrudOperation::ListParties
            | CrudOperation::SelectWithJoin => IntentPolarity::Read,
            CrudOperation::Insert
            | CrudOperation::Update
            | CrudOperation::Delete
            | CrudOperation::Upsert
            | CrudOperation::Link
            | CrudOperation::Unlink
            | CrudOperation::RoleLink
            | CrudOperation::RoleUnlink
            | CrudOperation::EntityCreate
            | CrudOperation::EntityUpsert => IntentPolarity::Write,
        };
    }

    if matches!(
        config.metadata.as_ref().and_then(|m| m.tier),
        Some(VerbTier::Diagnostics)
    ) {
        return IntentPolarity::Read;
    }

    if matches!(domain, "view" | "session")
        && (verb_name.starts_with("load-") || verb_name == "universe")
    {
        return IntentPolarity::Read;
    }

    IntentPolarity::Write
}

fn classify_planes(domain: &str, metadata: Option<&VerbMetadata>) -> Vec<ObservationPlane> {
    let mut planes = Vec::new();

    if domain == "schema" || domain == "struct" || domain == "registry" {
        planes.push(ObservationPlane::Structure);
    }
    if matches!(
        domain,
        "registry" | "changeset" | "governance" | "focus" | "audit"
    ) {
        planes.push(ObservationPlane::Registry);
    }
    if planes.is_empty() {
        planes.push(ObservationPlane::Instance);
    }

    if let Some(metadata) = metadata {
        if matches!(metadata.tier, Some(VerbTier::Reference))
            && !planes.contains(&ObservationPlane::Structure)
        {
            planes.push(ObservationPlane::Structure);
        }
        if matches!(metadata.tier, Some(VerbTier::Diagnostics))
            && domain == "registry"
            && !planes.contains(&ObservationPlane::Registry)
        {
            planes.push(ObservationPlane::Registry);
        }
    }

    planes.sort_by_key(|plane| match plane {
        ObservationPlane::Instance => 0,
        ObservationPlane::Structure => 1,
        ObservationPlane::Registry => 2,
    });
    planes.dedup();
    planes
}

fn action_tags(
    domain: &str,
    verb_name: &str,
    config: &VerbConfig,
    polarity: IntentPolarity,
) -> Vec<String> {
    let mut tags = Vec::new();

    tags.push(verb_name.to_string());
    tags.push(domain.to_string());

    for part in verb_name.split(['.', '-']) {
        if !part.is_empty() {
            tags.push(part.to_string());
        }
    }

    if let Some(metadata) = &config.metadata {
        if let Some(noun) = &metadata.noun {
            tags.push(noun.clone());
        }
        tags.extend(metadata.tags.iter().cloned());
    }

    if let Some(crud) = &config.crud {
        let crud_tag = match crud.operation {
            CrudOperation::Insert | CrudOperation::EntityCreate => "create",
            CrudOperation::Select
            | CrudOperation::ListByFk
            | CrudOperation::ListParties
            | CrudOperation::SelectWithJoin => "read",
            CrudOperation::Update | CrudOperation::EntityUpsert | CrudOperation::Upsert => "update",
            CrudOperation::Delete => "delete",
            CrudOperation::Link | CrudOperation::RoleLink => "assign",
            CrudOperation::Unlink | CrudOperation::RoleUnlink => "remove",
        };
        tags.push(crud_tag.to_string());
    }

    tags.push(match polarity {
        IntentPolarity::Read => "read".to_string(),
        IntentPolarity::Write => "write".to_string(),
        IntentPolarity::Ambiguous => "ambiguous".to_string(),
    });

    for word in config
        .description
        .split(|ch: char| !ch.is_ascii_alphanumeric())
    {
        let word = word.trim().to_ascii_lowercase();
        if word.len() >= 3 {
            tags.push(word);
        }
    }

    tags.sort();
    tags.dedup();
    tags
}

/// Compare the metadata index size to the runtime registry size.
///
/// # Examples
/// ```ignore
/// use ob_poc::sage::verb_index::runtime_registry_parity;
///
/// let (index_count, registry_count) = runtime_registry_parity()?;
/// assert_eq!(index_count, registry_count);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn runtime_registry_parity() -> Result<(usize, usize)> {
    let config = ConfigLoader::from_env().load_verbs()?;
    let index = VerbMetadataIndex::from_config(&config);
    let registry = RuntimeVerbRegistry::from_config(&config);
    Ok((index.len(), registry.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::config::types::{ArgConfig, ArgType, DomainConfig, VerbBehavior};

    fn sample_config() -> VerbsConfig {
        let mut domains = HashMap::new();
        let mut cbu_verbs = HashMap::new();
        cbu_verbs.insert(
            "list".to_string(),
            VerbConfig {
                description: "List CBUs".to_string(),
                behavior: VerbBehavior::Crud,
                crud: Some(dsl_core::config::types::CrudConfig {
                    operation: CrudOperation::Select,
                    table: Some("cbus".to_string()),
                    schema: Some("ob-poc".to_string()),
                    key: None,
                    returning: None,
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
                    type_code: None,
                    order_by: None,
                    set_values: None,
                    extension_table_column: None,
                    type_id_column: None,
                }),
                handler: None,
                graph_query: None,
                durable: None,
                args: vec![ArgConfig {
                    name: "client-id".to_string(),
                    arg_type: ArgType::Uuid,
                    required: false,
                    maps_to: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    description: None,
                    validation: None,
                    fuzzy_check: None,
                    slot_type: None,
                    preferred_roles: vec![],
                }],
                returns: None,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                metadata: Some(VerbMetadata {
                    tier: Some(VerbTier::Diagnostics),
                    noun: Some("cbu".to_string()),
                    ..VerbMetadata::default()
                }),
                invocation_phrases: vec![],
                policy: None,
                sentences: None,
                confirm_policy: None,
            },
        );
        let mut registry_verbs = HashMap::new();
        registry_verbs.insert(
            "list-entities".to_string(),
            VerbConfig {
                description: "List registry entities".to_string(),
                behavior: VerbBehavior::Plugin,
                crud: None,
                handler: Some("registry.list_entities".to_string()),
                graph_query: None,
                durable: None,
                args: vec![],
                returns: None,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                metadata: Some(VerbMetadata {
                    tier: Some(VerbTier::Diagnostics),
                    noun: Some("registry".to_string()),
                    ..VerbMetadata::default()
                }),
                invocation_phrases: vec![],
                policy: None,
                sentences: None,
                confirm_policy: None,
            },
        );
        domains.insert(
            "cbu".to_string(),
            DomainConfig {
                description: "CBU verbs".to_string(),
                verbs: cbu_verbs,
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );
        domains.insert(
            "registry".to_string(),
            DomainConfig {
                description: "Registry verbs".to_string(),
                verbs: registry_verbs,
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );
        VerbsConfig {
            version: "1.0".to_string(),
            domains,
        }
    }

    #[test]
    fn index_classifies_polarity_from_crud_and_prefix() {
        let index = VerbMetadataIndex::from_config(&sample_config());
        assert_eq!(
            index.get("cbu.list").unwrap().polarity,
            IntentPolarity::Read
        );
        assert_eq!(
            index.get("registry.list-entities").unwrap().polarity,
            IntentPolarity::Read
        );
    }

    #[test]
    fn index_classifies_planes_from_domain() {
        let index = VerbMetadataIndex::from_config(&sample_config());
        assert_eq!(
            index.get("cbu.list").unwrap().planes,
            vec![ObservationPlane::Instance]
        );
        assert_eq!(
            index.get("registry.list-entities").unwrap().planes,
            vec![ObservationPlane::Structure, ObservationPlane::Registry]
        );
    }

    #[test]
    fn index_captures_params_and_required_params() {
        let index = VerbMetadataIndex::from_config(&sample_config());
        let meta = index.get("cbu.list").unwrap();
        assert_eq!(meta.param_names, vec!["client-id".to_string()]);
        assert!(meta.required_params.is_empty());
        assert!(meta.action_tags.iter().any(|tag| tag == "cbu"));
    }

    #[test]
    fn query_without_domain_hint_returns_plane_and_polarity_matches() {
        let index = VerbMetadataIndex::from_config(&sample_config());
        let matches = index.query(ObservationPlane::Instance, IntentPolarity::Read, None);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].fqn, "cbu.list");
    }

    #[test]
    fn query_matches_domain_prefix_and_tags() {
        let mut config = sample_config();
        let screening_domain = config.domains.entry("case-screening".to_string()).or_insert(
            DomainConfig {
                description: "Screening verbs".to_string(),
                verbs: HashMap::new(),
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );
        screening_domain.verbs.insert(
            "list".to_string(),
            VerbConfig {
                description: "List screening cases".to_string(),
                behavior: VerbBehavior::Plugin,
                crud: None,
                handler: Some("case-screening.list".to_string()),
                graph_query: None,
                durable: None,
                args: vec![],
                returns: None,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                metadata: Some(VerbMetadata {
                    noun: Some("screening".to_string()),
                    tags: vec!["screening".to_string()],
                    ..VerbMetadata::default()
                }),
                invocation_phrases: vec![],
                policy: None,
                sentences: None,
                confirm_policy: None,
            },
        );

        let index = VerbMetadataIndex::from_config(&config);
        let matches = index.query(
            ObservationPlane::Instance,
            IntentPolarity::Read,
            Some("screening"),
        );
        assert!(matches.iter().any(|meta| meta.domain == "case-screening"));
    }

    #[test]
    fn real_config_matches_runtime_registry_count() {
        let (index_count, registry_count) = runtime_registry_parity().unwrap();
        assert_eq!(index_count, registry_count);
        assert!(index_count > 1000);
    }
}
