//! Verb-first onboarding scanner — pure conversion functions.
//!
//! Converts ob-poc verb YAML definitions into Semantic OS typed seed data.
//! All functions here are **pure** (no DB, no I/O). The DB-publishing
//! orchestrator remains in `ob-poc/src/sem_reg/scanner.rs` and delegates
//! to these converters.

use std::collections::BTreeMap;

use dsl_core::config::types::{
    ActionClass as DslActionClass, ArgConfig, CrudOperation, HarmClass as DslHarmClass, VerbConfig,
    VerbMetadata, VerbsConfig,
};

use sem_os_core::{
    attribute_def::{AttributeDataType, AttributeDefBody, AttributeSource},
    entity_type_def::{DbTableMapping, EntityTypeDefBody},
    types::{Classification, EvidenceGrade, HandlingControl, SecurityLabel},
    verb_contract::{
        ActionClass, HarmClass, VerbArgDef, VerbArgLookup, VerbContractBody, VerbContractMetadata,
        VerbCrudMapping, VerbOutput, VerbPrecondition, VerbProducesSpec, VerbReturnSpec,
    },
};

use crate::metadata::DomainMetadata;

/// Convert a `VerbConfig` to a `VerbContractBody`.
pub fn verb_config_to_contract(
    domain: &str,
    action: &str,
    config: &VerbConfig,
) -> VerbContractBody {
    let fqn = format!("{}.{}", domain, action);

    let args: Vec<VerbArgDef> = config
        .args
        .iter()
        .map(|a| VerbArgDef {
            name: a.name.clone(),
            arg_type: to_wire_str(&a.arg_type),
            required: a.required,
            description: a.description.clone(),
            lookup: a.lookup.as_ref().map(|l| {
                let search_key_str = match &l.search_key {
                    dsl_core::config::types::SearchKeyConfig::Simple(s) => Some(s.clone()),
                    dsl_core::config::types::SearchKeyConfig::Composite(c) => {
                        Some(c.primary.clone())
                    }
                };
                VerbArgLookup {
                    table: l.table.clone(),
                    entity_type: l.entity_type.clone().unwrap_or_else(|| l.table.clone()),
                    schema: l.schema.clone(),
                    search_key: search_key_str,
                    primary_key: Some(l.primary_key.clone()),
                }
            }),
            valid_values: a.valid_values.clone(),
            default: a
                .default
                .as_ref()
                .and_then(|v| serde_json::to_value(v).ok()),
        })
        .collect();

    let returns = config.returns.as_ref().map(|r| VerbReturnSpec {
        return_type: to_wire_str(&r.return_type),
        schema: None,
    });

    let produces = config.produces.as_ref().map(|p| VerbProducesSpec {
        entity_type: p.produced_type.clone(),
        resolved: p.resolved,
    });

    let consumes: Vec<String> = config
        .consumes
        .iter()
        .map(|c| c.consumed_type.clone())
        .collect();

    let preconditions = config
        .lifecycle
        .as_ref()
        .map(|lc| {
            let mut pres = Vec::new();
            for req in &lc.requires_states {
                pres.push(VerbPrecondition {
                    kind: "requires_state".into(),
                    value: req.clone(),
                    description: None,
                });
            }
            for check in &lc.precondition_checks {
                pres.push(VerbPrecondition {
                    kind: "precondition_check".into(),
                    value: check.clone(),
                    description: None,
                });
            }
            pres
        })
        .unwrap_or_default();

    let metadata = config.metadata.as_ref().map(|m| VerbContractMetadata {
        tier: m.tier.as_ref().map(to_wire_str),
        source_of_truth: m.source_of_truth.as_ref().map(to_wire_str),
        scope: m.scope.as_ref().map(to_wire_str),
        noun: m.noun.clone(),
        tags: m.tags.clone(),
        subject_kinds: m.subject_kinds.clone(),
        phase_tags: m.phase_tags.clone(),
    });

    let subject_kinds = derive_subject_kinds(domain, config);

    let phase_tags = {
        let explicit = config
            .metadata
            .as_ref()
            .map(|m| m.phase_tags.clone())
            .unwrap_or_default();
        if explicit.is_empty() {
            // Fallback: derive from metadata.tags when phase_tags is empty
            config
                .metadata
                .as_ref()
                .map(|m| m.tags.clone())
                .unwrap_or_default()
        } else {
            explicit
        }
    };

    let harm_class = Some(infer_harm_class(action, config.metadata.as_ref()));

    let action_class = Some(infer_action_class(action, config));

    let precondition_states = config
        .lifecycle
        .as_ref()
        .map(|lc| lc.requires_states.clone())
        .unwrap_or_default();

    let requires_subject = config
        .metadata
        .as_ref()
        .map(|m| m.requires_subject)
        .unwrap_or(true);

    let produces_focus = config
        .metadata
        .as_ref()
        .map(|m| m.produces_focus)
        .unwrap_or(false);

    let crud_mapping = config.crud.as_ref().map(|c| VerbCrudMapping {
        operation: to_wire_str(&c.operation),
        table: c.table.clone(),
        schema: c.schema.clone(),
        key_column: c.key.clone(),
    });

    VerbContractBody {
        fqn,
        domain: domain.to_string(),
        action: action.to_string(),
        description: config.description.clone(),
        behavior: to_wire_str(&config.behavior),
        args,
        returns,
        preconditions,
        postconditions: vec![],
        produces,
        consumes,
        invocation_phrases: config.invocation_phrases.clone(),
        subject_kinds,
        phase_tags,
        harm_class,
        action_class,
        precondition_states,
        requires_subject,
        produces_focus,
        metadata,
        crud_mapping,
        reads_from: vec![],
        writes_to: vec![],
        outputs: config
            .outputs
            .iter()
            .map(|o| VerbOutput {
                field_name: o.name.clone(),
                output_type: o.output_type.clone(),
                entity_kind: o.entity_kind.clone(),
                description: o.description.clone(),
            })
            .collect(),
    }
}

fn to_contract_harm_class(harm_class: DslHarmClass) -> HarmClass {
    match harm_class {
        DslHarmClass::ReadOnly => HarmClass::ReadOnly,
        DslHarmClass::Reversible => HarmClass::Reversible,
        DslHarmClass::Irreversible => HarmClass::Irreversible,
        DslHarmClass::Destructive => HarmClass::Destructive,
    }
}

fn to_contract_action_class(action_class: DslActionClass) -> ActionClass {
    match action_class {
        DslActionClass::List => ActionClass::List,
        DslActionClass::Read => ActionClass::Read,
        DslActionClass::Search => ActionClass::Search,
        DslActionClass::Describe => ActionClass::Describe,
        DslActionClass::Create => ActionClass::Create,
        DslActionClass::Update => ActionClass::Update,
        DslActionClass::Delete => ActionClass::Delete,
        DslActionClass::Assign => ActionClass::Assign,
        DslActionClass::Remove => ActionClass::Remove,
        DslActionClass::Import => ActionClass::Import,
        DslActionClass::Compute => ActionClass::Compute,
        DslActionClass::Review => ActionClass::Review,
        DslActionClass::Approve => ActionClass::Approve,
        DslActionClass::Reject => ActionClass::Reject,
        DslActionClass::Execute => ActionClass::Execute,
    }
}

fn infer_harm_class(verb_name: &str, metadata: Option<&VerbMetadata>) -> HarmClass {
    if let Some(explicit) = metadata.and_then(|metadata| metadata.harm_class) {
        return to_contract_harm_class(explicit);
    }

    let normalized_name = verb_name.to_ascii_lowercase();
    let side_effects = metadata.and_then(|meta| meta.side_effects.as_deref());
    let dangerous = metadata.is_some_and(|meta| meta.dangerous);

    if matches!(side_effects, Some("facts_only")) {
        return HarmClass::ReadOnly;
    }

    if dangerous
        || [
            "purge",
            "destroy",
            "wipe",
            "nuke",
            "truncate",
            "hard-delete",
        ]
        .iter()
        .any(|needle| normalized_name.contains(needle))
    {
        return HarmClass::Destructive;
    }

    if [
        "delete",
        "deactivate",
        "retire",
        "archive",
        "close",
        "publish",
    ]
    .iter()
    .any(|needle| normalized_name.contains(needle))
    {
        return HarmClass::Irreversible;
    }

    if matches!(side_effects, Some("state_write")) {
        return HarmClass::Reversible;
    }

    if normalized_name.starts_with("list")
        || normalized_name.starts_with("show")
        || normalized_name.starts_with("get")
        || normalized_name.starts_with("read")
        || normalized_name.starts_with("search")
        || normalized_name.starts_with("describe")
        || normalized_name.starts_with("trace")
    {
        HarmClass::ReadOnly
    } else {
        HarmClass::Reversible
    }
}

fn infer_action_class(verb_name: &str, config: &VerbConfig) -> ActionClass {
    if let Some(explicit) = config
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.action_class)
    {
        return to_contract_action_class(explicit);
    }

    if let Some(crud) = &config.crud {
        return match crud.operation {
            CrudOperation::Select
            | CrudOperation::ListByFk
            | CrudOperation::ListParties
            | CrudOperation::SelectWithJoin => {
                if verb_name.starts_with("list") {
                    ActionClass::List
                } else {
                    ActionClass::Read
                }
            }
            CrudOperation::Insert | CrudOperation::EntityCreate => ActionClass::Create,
            CrudOperation::Update | CrudOperation::EntityUpsert | CrudOperation::Upsert => {
                ActionClass::Update
            }
            CrudOperation::Delete => ActionClass::Delete,
            CrudOperation::Link | CrudOperation::RoleLink => ActionClass::Assign,
            CrudOperation::Unlink | CrudOperation::RoleUnlink => ActionClass::Remove,
        };
    }

    let normalized_name = verb_name.to_ascii_lowercase();
    let primary = normalized_name
        .split(['-', '.'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(normalized_name.as_str());
    match primary {
        "list" => ActionClass::List,
        "show" | "get" | "read" => ActionClass::Read,
        "search" | "find" | "discover" => ActionClass::Search,
        "describe" => ActionClass::Describe,
        "create" | "ensure" | "upsert" => ActionClass::Create,
        "update" | "edit" | "change" | "set" => ActionClass::Update,
        "delete" | "destroy" | "purge" => ActionClass::Delete,
        "assign" | "bind" | "link" | "subscribe" => ActionClass::Assign,
        "remove" | "unlink" | "unsubscribe" => ActionClass::Remove,
        "import" | "load" | "ingest" => ActionClass::Import,
        "compute" | "calculate" | "analyze" | "analyse" => ActionClass::Compute,
        "review" | "validate" => ActionClass::Review,
        "approve" | "publish" => ActionClass::Approve,
        "reject" => ActionClass::Reject,
        "run" | "execute" => ActionClass::Execute,
        _ => match config
            .metadata
            .as_ref()
            .and_then(|meta| meta.side_effects.as_deref())
        {
            Some("facts_only") => ActionClass::Read,
            _ => ActionClass::Update,
        },
    }
}

/// Infer entity types from verb argument lookup configurations.
pub fn infer_entity_types_from_verbs(verbs_config: &VerbsConfig) -> Vec<EntityTypeDefBody> {
    let mut seen: BTreeMap<String, EntityTypeDefBody> = BTreeMap::new();

    for (domain, domain_config) in &verbs_config.domains {
        for verb_config in domain_config.verbs.values() {
            for arg in &verb_config.args {
                if let Some(lookup) = &arg.lookup {
                    let entity_type_str = lookup.entity_type.as_deref().unwrap_or(&lookup.table);
                    let key = format!("{}.{}", domain, entity_type_str);
                    seen.entry(key.clone()).or_insert_with(|| {
                        let search_key_str = match &lookup.search_key {
                            dsl_core::config::types::SearchKeyConfig::Simple(s) => s.clone(),
                            dsl_core::config::types::SearchKeyConfig::Composite(c) => {
                                c.primary.clone()
                            }
                        };
                        EntityTypeDefBody {
                            fqn: key,
                            name: title_case(entity_type_str),
                            description: format!(
                                "Entity type inferred from {}.{} lookup",
                                domain, entity_type_str
                            ),
                            domain: domain.clone(),
                            db_table: Some(DbTableMapping {
                                schema: lookup.schema.clone().unwrap_or_else(|| "ob-poc".into()),
                                table: lookup.table.clone(),
                                primary_key: lookup.primary_key.clone(),
                                name_column: Some(search_key_str),
                            }),
                            lifecycle_states: vec![],
                            required_attributes: vec![],
                            optional_attributes: vec![],
                            parent_type: None,
                            governance_tier: None,
                            security_classification: None,
                            pii: None,
                            read_by_verbs: vec![],
                            written_by_verbs: vec![],
                        }
                    });
                }
            }
        }
    }

    seen.into_values().collect()
}

/// Infer attributes from verb argument definitions.
///
/// Accepts the inferred entity type defs so that attribute sources can resolve
/// real (schema, table) triples via the resolution chain:
/// 1. Verb CRUD config (most precise — gives exact table + schema)
/// 2. Entity type db_table mapping (from lookup configs)
/// 3. Fallback to None (better than a wrong guess)
pub fn infer_attributes_from_verbs(
    verbs_config: &VerbsConfig,
    entity_type_defs: &[EntityTypeDefBody],
) -> Vec<AttributeDefBody> {
    // Build domain → entity type lookup for step 2 of the resolution chain
    let entity_types_by_domain: BTreeMap<&str, &EntityTypeDefBody> = entity_type_defs
        .iter()
        .map(|et| (et.domain.as_str(), et))
        .collect();

    let mut seen: BTreeMap<String, AttributeDefBody> = BTreeMap::new();

    for (domain, domain_config) in &verbs_config.domains {
        for (action, verb_config) in &domain_config.verbs {
            for arg in &verb_config.args {
                let fqn = format!("{}.{}", domain, arg.name);
                let action = action.clone();
                let domain = domain.clone();

                // Resolution chain: CRUD config → entity type db_table → None
                let (schema, table) = if let Some(crud) = &verb_config.crud {
                    (crud.schema.clone(), crud.table.clone())
                } else if let Some(et) = entity_types_by_domain.get(domain.as_str()) {
                    et.db_table.as_ref().map_or((None, None), |dt| {
                        (Some(dt.schema.clone()), Some(dt.table.clone()))
                    })
                } else {
                    (None, None)
                };

                seen.entry(fqn.clone()).or_insert_with(|| AttributeDefBody {
                    fqn,
                    name: title_case(&arg.name),
                    description: arg.description.clone().unwrap_or_else(|| {
                        format!(
                            "Attribute inferred from {}.{} arg '{}'",
                            domain, action, arg.name
                        )
                    }),
                    domain: domain.clone(),
                    data_type: arg_type_to_attribute_type(arg),
                    evidence_grade: EvidenceGrade::None,
                    source: Some(AttributeSource {
                        producing_verb: Some(format!("{}.{}", domain, action)),
                        schema,
                        table,
                        column: arg.maps_to.clone(),
                        derived: false,
                    }),
                    constraints: None,
                    sinks: vec![],
                });
            }
        }
    }

    seen.into_values().collect()
}

/// Generate auditable `attribute.define` macro calls for a single verb domain.
///
/// This preserves the scanner's inference logic but emits replayable DSL
/// instead of publishing snapshots directly.
///
/// # Examples
///
/// ```
/// use dsl_core::config::types::{DomainConfig, VerbsConfig};
/// use sem_os_obpoc_adapter::scanner::generate_seed_domain_macro_calls;
/// use std::collections::HashMap;
///
/// let config = VerbsConfig {
///     version: "1.0".into(),
///     domains: HashMap::from([(
///         "cbu".into(),
///         DomainConfig {
///             description: "CBU".into(),
///             verbs: HashMap::new(),
///             dynamic_verbs: vec![],
///             invocation_hints: vec![],
///         },
///     )]),
/// };
///
/// let calls = generate_seed_domain_macro_calls(&config, "cbu");
/// assert!(calls.iter().all(|call| call.starts_with("(attribute.define")));
/// ```
pub fn generate_seed_domain_macro_calls(verbs_config: &VerbsConfig, domain: &str) -> Vec<String> {
    let entity_types = infer_entity_types_from_verbs(verbs_config);
    let mut attrs: Vec<_> = infer_attributes_from_verbs(verbs_config, &entity_types)
        .into_iter()
        .filter(|attr| attr.domain == domain)
        .collect();
    attrs.sort_by(|left, right| left.fqn.cmp(&right.fqn));

    attrs.into_iter()
        .map(|attr| {
            format!(
                "(attribute.define :id \"{}\" :display-name \"{}\" :category \"{}\" :value-type \"{}\" :domain \"{}\")",
                escape_dsl_string(&attr.fqn),
                escape_dsl_string(&attr.name),
                inferred_category_for_fqn(&attr.fqn),
                attr.data_type.to_pg_check_value(),
                escape_dsl_string(domain)
            )
        })
        .collect()
}

/// Suggest a security label for a snapshot based on FQN/domain/tag heuristics.
pub fn suggest_security_label(fqn: &str, domain: &str, tags: &[String]) -> SecurityLabel {
    let fqn_lower = fqn.to_lowercase();
    let domain_lower = domain.to_lowercase();
    let tags_lower: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    let pii_patterns = [
        "name",
        "address",
        "dob",
        "date_of_birth",
        "birth_date",
        "ssn",
        "social_security",
        "passport",
        "national_id",
        "tax_id",
        "phone",
        "email",
        "bank_account",
        "iban",
    ];
    let has_pii = pii_patterns.iter().any(|p| fqn_lower.contains(p))
        || tags_lower
            .iter()
            .any(|t| t == "pii" || t == "personal_data");

    let is_sanctions = domain_lower == "sanctions"
        || domain_lower == "screening"
        || tags_lower.iter().any(|t| t == "sanctions");

    let is_financial = matches!(
        domain_lower.as_str(),
        "deal" | "billing" | "rate" | "fee" | "invoice" | "contract"
    ) || tags_lower.iter().any(|t| t == "financial");

    if is_sanctions {
        SecurityLabel {
            classification: Classification::Restricted,
            pii: has_pii,
            jurisdictions: vec![],
            purpose_limitation: vec!["operations".into()],
            handling_controls: vec![HandlingControl::NoExport, HandlingControl::NoLlmExternal],
        }
    } else if has_pii {
        SecurityLabel {
            classification: Classification::Confidential,
            pii: true,
            jurisdictions: vec![],
            purpose_limitation: vec!["operations".into(), "audit".into()],
            handling_controls: vec![HandlingControl::MaskByDefault],
        }
    } else if is_financial {
        SecurityLabel {
            classification: Classification::Confidential,
            pii: false,
            jurisdictions: vec![],
            purpose_limitation: vec![],
            handling_controls: vec![HandlingControl::NoLlmExternal],
        }
    } else {
        SecurityLabel::default()
    }
}

/// Convert all verb configs from a `VerbsConfig` into sorted `VerbContractBody` list.
pub fn scan_verb_contracts(verbs_config: &VerbsConfig) -> Vec<VerbContractBody> {
    let mut contracts = Vec::new();
    for (domain, domain_config) in &verbs_config.domains {
        for (action, verb_config) in &domain_config.verbs {
            contracts.push(verb_config_to_contract(domain, action, verb_config));
        }
    }
    contracts.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    contracts
}

/// Enrich verb contracts with reads_from/writes_to from domain metadata.
///
/// For each verb contract, looks up the verb FQN in domain metadata's
/// `verb_data_footprint` and populates the data linkage fields.
pub fn enrich_verb_contracts(contracts: &mut [VerbContractBody], meta: &DomainMetadata) {
    for contract in contracts.iter_mut() {
        if let Some(footprint) = meta.find_verb_footprint(&contract.fqn) {
            contract.reads_from = footprint.reads.clone();
            contract.writes_to = footprint.writes.clone();
        }
    }
}

/// Enrich entity types with governance metadata and reverse verb index.
///
/// For each entity type, derives the table name from `db_table` (if present)
/// or falls back to a pluralized FQN heuristic. Then populates:
/// - `governance_tier` and `security_classification` from table metadata
/// - `pii` flag from table metadata
/// - `read_by_verbs` / `written_by_verbs` from the reverse verb index
pub fn enrich_entity_types(entity_types: &mut [EntityTypeDefBody], meta: &DomainMetadata) {
    for et in entity_types.iter_mut() {
        // Derive table name from db_table field or FQN heuristic
        let table_name = et
            .db_table
            .as_ref()
            .map(|dt| dt.table.clone())
            .unwrap_or_else(|| format!("{}s", et.fqn));

        // Try schema-qualified lookup first, then unqualified
        let schema = et.db_table.as_ref().map(|dt| dt.schema.as_str());
        let table_lookup = meta.find_table_qualified(schema, &table_name);

        if let Some((_domain, table_meta)) = table_lookup {
            et.governance_tier = Some(to_wire_str(&table_meta.governance_tier));
            et.security_classification = Some(to_wire_str(&table_meta.classification));
            et.pii = Some(table_meta.pii);
        }

        // Populate reverse verb index (which verbs read/write this entity's table)
        let readers = meta.verbs_reading(&table_name);
        if !readers.is_empty() {
            et.read_by_verbs = readers.into_iter().map(String::from).collect();
            et.read_by_verbs.sort();
        }

        let writers = meta.verbs_writing(&table_name);
        if !writers.is_empty() {
            et.written_by_verbs = writers.into_iter().map(String::from).collect();
            et.written_by_verbs.sort();
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

/// Convert a serde-serializable enum to its stable snake_case wire name.
pub fn to_wire_str<T: serde::Serialize>(value: &T) -> String {
    let json = serde_json::to_string(value).unwrap_or_default();
    json.trim_matches('"').to_string()
}

pub fn title_case(s: &str) -> String {
    s.replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn inferred_category_for_fqn(fqn: &str) -> &'static str {
    let lower = fqn.to_ascii_lowercase();
    if lower.contains("risk") {
        "risk"
    } else if lower.contains("ubo") {
        "ubo"
    } else if lower.contains("document") {
        "document"
    } else if lower.contains("fund") {
        "fund"
    } else if lower.contains("entity") {
        "entity"
    } else {
        "compliance"
    }
}

fn escape_dsl_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Map a verb domain name to its primary subject kind.
///
/// This is the lowest-priority heuristic for `subject_kinds` derivation.
/// Only used when `metadata.subject_kinds`, `produces.entity_type`, and
/// arg lookup `entity_type` all yield nothing.
fn domain_to_subject_kind(domain: &str) -> String {
    match domain {
        "cbu" | "cbu-role" => "cbu".into(),
        "entity" | "entity-role" | "party" | "ownership" | "legal-entity" => "entity".into(),
        "kyc" | "kyc-case" | "screening" => "kyc-case".into(),
        "case" | "tollgate" => "kyc-case".into(),
        "deal" => "deal".into(),
        "contract" | "contract-pack" => "contract".into(),
        "billing" => "billing-profile".into(),
        "trading-profile" | "custody" | "ssi" | "mandate" => "trading-profile".into(),
        "fund" => "fund".into(),
        "investor" | "holding" => "investor-register".into(),
        "document" | "requirement" => "document".into(),
        "session" | "view" => "session".into(),
        "gleif" | "research" => "entity".into(),
        "workflow" | "bpmn" => "workflow".into(),
        _ => domain.into(),
    }
}

fn derive_subject_kinds(domain: &str, config: &VerbConfig) -> Vec<String> {
    if let Some(ref meta) = config.metadata {
        if !meta.subject_kinds.is_empty() {
            return dedupe_subject_kinds(
                meta.subject_kinds
                    .iter()
                    .map(|kind| canonicalize_entity_kind(kind))
                    .collect(),
            );
        }
    }

    let mut inferred = Vec::new();

    if let Some(ref produces) = config.produces {
        inferred.push(canonicalize_entity_kind(&produces.produced_type));
    }

    inferred.extend(
        config
            .consumes
            .iter()
            .map(|consume| canonicalize_entity_kind(&consume.consumed_type)),
    );

    inferred.extend(derive_subject_kinds_from_crud(config));

    inferred.extend(
        config
            .args
            .iter()
            .filter(|arg| arg.required || arg.lookup.is_some())
            .filter_map(derive_subject_kind_from_arg),
    );

    if let Some(entity_arg) = config
        .lifecycle
        .as_ref()
        .and_then(|lifecycle| lifecycle.entity_arg.as_deref())
    {
        if let Some(arg) = config.args.iter().find(|arg| arg.name == entity_arg) {
            if let Some(kind) = derive_subject_kind_from_arg(arg) {
                inferred.push(kind);
            }
        }
    }

    if let Some(meta) = &config.metadata {
        inferred.extend(
            meta.noun
                .iter()
                .filter_map(|noun| derive_subject_kind_from_hint(noun)),
        );
        inferred.extend(
            meta.tags
                .iter()
                .filter_map(|tag| derive_subject_kind_from_hint(tag)),
        );
    }

    let inferred = dedupe_subject_kinds(inferred);
    if !inferred.is_empty() {
        return inferred;
    }

    vec![canonicalize_entity_kind(&domain_to_subject_kind(domain))]
}

fn dedupe_subject_kinds(mut kinds: Vec<String>) -> Vec<String> {
    kinds.retain(|kind| !kind.is_empty());
    kinds.sort();
    kinds.dedup();
    kinds
}

fn derive_subject_kinds_from_crud(config: &VerbConfig) -> Vec<String> {
    let Some(ref crud) = config.crud else {
        return Vec::new();
    };

    [
        crud.table.as_deref(),
        crud.base_table.as_deref(),
        crud.extension_table.as_deref(),
        crud.junction.as_deref(),
        crud.primary_table.as_deref(),
        crud.join_table.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(derive_subject_kind_from_hint)
    .collect()
}

fn derive_subject_kind_from_arg(arg: &ArgConfig) -> Option<String> {
    arg.lookup
        .as_ref()
        .and_then(|lookup| {
            lookup
                .entity_type
                .as_deref()
                .filter(|kind| !is_generic_lookup_kind(kind))
                .map(canonicalize_entity_kind)
                .or_else(|| derive_subject_kind_from_hint(&lookup.table))
        })
        .or_else(|| derive_subject_kind_from_arg_name(&arg.name))
}

fn derive_subject_kind_from_arg_name(name: &str) -> Option<String> {
    let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
    let trimmed = normalized
        .trim_end_matches("-id")
        .trim_end_matches("-ref")
        .trim_end_matches("-uuid");
    derive_subject_kind_from_hint(trimmed)
}

fn derive_subject_kind_from_hint(hint: &str) -> Option<String> {
    let normalized = hint.trim().to_ascii_lowercase().replace('_', "-");
    let kind = match normalized.as_str() {
        "cbu" | "cbus" | "client-business-unit" | "client-business-units" | "structure" => "cbu",
        "entity"
        | "entities"
        | "party"
        | "parties"
        | "company"
        | "companies"
        | "person"
        | "people"
        | "legal-entity"
        | "legal-entities"
        | "counterparty"
        | "counterparties"
        | "investment-manager"
        | "investment-managers"
        | "management-company"
        | "management-companies"
        | "depositary"
        | "depositaries" => "entity",
        "deal" | "deals" => "deal",
        "contract" | "contracts" | "contract-pack" | "contract-packs" | "agreement" => "contract",
        "document" | "documents" | "requirement" | "requirements" | "evidence" | "attachments" => {
            "document"
        }
        "trading-profile"
        | "trading-profiles"
        | "mandate"
        | "mandates"
        | "ssi"
        | "custody"
        | "cbu-trading-profiles" => "trading-profile",
        "billing" | "billings" | "billing-profile" | "billing-profiles" | "invoice"
        | "invoices" | "fee" | "fees" => "billing-profile",
        "fund" | "funds" | "sub-fund" | "sub-funds" | "umbrella" | "umbrellas" => "fund",
        "investor" | "investors" | "holding" | "holdings" | "investor-register" => "investor",
        "kyc-case"
        | "kyc"
        | "case"
        | "cases"
        | "tollgate"
        | "tollgate-evaluations"
        | "screening"
        | "screenings" => "kyc-case",
        "session" | "view" => "session",
        "workflow" => "workflow",
        _ => return None,
    };
    Some(canonicalize_entity_kind(kind))
}

fn is_generic_lookup_kind(kind: &str) -> bool {
    matches!(
        canonicalize_entity_kind(kind).as_str(),
        "jurisdiction"
            | "country"
            | "currency"
            | "role"
            | "status"
            | "market"
            | "user"
            | "team"
            | "security"
    )
}

fn canonicalize_entity_kind(kind: &str) -> String {
    let normalized = kind.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "kyc_case" | "case" => "kyc-case".to_string(),
        "client_group" => "client-group".to_string(),
        "legal-entity" | "legal_entity" | "organization" | "org" => "company".to_string(),
        "individual" | "natural_person" => "person".to_string(),
        "client-book" | "client_book" => "client-group".to_string(),
        "investor-register" | "investor_register" => "investor".to_string(),
        "investment-fund" | "umbrella" | "sub-fund" | "compartment" => "fund".to_string(),
        "doc" | "evidence-document" => "document".to_string(),
        "legal-contract" | "agreement" | "msa" => "contract".to_string(),
        "mandate" | "trading-mandate" => "trading-profile".to_string(),
        "deal-record" | "sales-deal" => "deal".to_string(),
        "client-business-unit" | "structure" | "trading-unit" => "cbu".to_string(),
        other => other.to_string(),
    }
}

pub fn arg_type_to_attribute_type(arg: &ArgConfig) -> AttributeDataType {
    match to_wire_str(&arg.arg_type).as_str() {
        "string" => AttributeDataType::String,
        "integer" | "int" => AttributeDataType::Integer,
        "decimal" => AttributeDataType::Decimal,
        "number" | "float" => AttributeDataType::Number,
        "boolean" | "bool" => AttributeDataType::Boolean,
        "uuid" => AttributeDataType::Uuid,
        "date" => AttributeDataType::Date,
        "timestamp" | "datetime" => AttributeDataType::DateTime,
        "email" => AttributeDataType::Email,
        "phone" => AttributeDataType::Phone,
        "address" => AttributeDataType::Address,
        "currency" => AttributeDataType::Currency,
        "percentage" | "percent" => AttributeDataType::Percentage,
        "tax_id" | "taxid" => AttributeDataType::TaxId,
        _ => {
            if let Some(ref valid_values) = arg.valid_values {
                AttributeDataType::Enum(valid_values.clone())
            } else {
                AttributeDataType::String
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::config::types::*;
    use std::collections::HashMap;

    fn sample_verb_config() -> VerbConfig {
        VerbConfig {
            description: "Create a new CBU".into(),
            behavior: VerbBehavior::Plugin,
            crud: None,
            handler: Some("CbuCreateOp".into()),
            graph_query: None,
            durable: None,
            args: vec![
                ArgConfig {
                    name: "name".into(),
                    arg_type: ArgType::String,
                    required: true,
                    maps_to: Some("name".into()),
                    lookup: None,
                    valid_values: None,
                    default: None,
                    description: Some("CBU name".into()),
                    validation: None,
                    fuzzy_check: None,
                    slot_type: None,
                    preferred_roles: vec![],
                },
                ArgConfig {
                    name: "jurisdiction".into(),
                    arg_type: ArgType::String,
                    required: true,
                    maps_to: None,
                    lookup: Some(LookupConfig {
                        table: "master_jurisdictions".into(),
                        entity_type: Some("jurisdiction".into()),
                        schema: Some("ob-poc".into()),
                        search_key: SearchKeyConfig::Simple("jurisdiction_code".into()),
                        primary_key: "jurisdiction_code".into(),
                        resolution_mode: None,
                        scope_key: None,
                        role_filter: None,
                    }),
                    valid_values: None,
                    default: None,
                    description: Some("Jurisdiction code".into()),
                    validation: None,
                    fuzzy_check: None,
                    slot_type: None,
                    preferred_roles: vec![],
                },
            ],
            returns: None,
            produces: Some(VerbProduces {
                produced_type: "cbu".into(),
                subtype: None,
                subtype_from_arg: None,
                resolved: false,
                initial_state: None,
            }),
            consumes: vec![],
            lifecycle: None,
            metadata: None,
            invocation_phrases: vec!["create CBU".into(), "new fund".into()],
            policy: None,
            sentences: None,
            confirm_policy: None,
        }
    }

    #[test]
    fn test_verb_config_to_contract() {
        let config = sample_verb_config();
        let contract = verb_config_to_contract("cbu", "create", &config);

        assert_eq!(contract.fqn, "cbu.create");
        assert_eq!(contract.domain, "cbu");
        assert_eq!(contract.action, "create");
        assert_eq!(contract.args.len(), 2);
        assert_eq!(contract.args[0].name, "name");
        assert!(contract.args[0].required);
        assert!(contract.args[1].lookup.is_some());
        assert_eq!(contract.invocation_phrases.len(), 2);
        assert!(contract.produces.is_some());
        assert_eq!(contract.harm_class, Some(super::HarmClass::Reversible));
        assert_eq!(contract.action_class, Some(super::ActionClass::Create));
        assert!(contract.precondition_states.is_empty());
    }

    #[test]
    fn test_infer_entity_types() {
        let mut domains = HashMap::new();
        domains.insert(
            "cbu".into(),
            DomainConfig {
                description: "CBU domain".into(),
                verbs: {
                    let mut v = HashMap::new();
                    v.insert("create".into(), sample_verb_config());
                    v
                },
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );

        let config = VerbsConfig {
            version: "1.0".into(),
            domains,
        };

        let entity_types = infer_entity_types_from_verbs(&config);
        assert!(!entity_types.is_empty());
        let juris = entity_types.iter().find(|e| e.fqn.contains("jurisdiction"));
        assert!(juris.is_some());
    }

    #[test]
    fn test_infer_attributes() {
        let mut domains = HashMap::new();
        domains.insert(
            "cbu".into(),
            DomainConfig {
                description: "CBU domain".into(),
                verbs: {
                    let mut v = HashMap::new();
                    v.insert("create".into(), sample_verb_config());
                    v
                },
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );

        let config = VerbsConfig {
            version: "1.0".into(),
            domains,
        };

        let entity_types = infer_entity_types_from_verbs(&config);
        let attrs = infer_attributes_from_verbs(&config, &entity_types);
        assert!(!attrs.is_empty());
        let name_attr = attrs.iter().find(|a| a.fqn == "cbu.name");
        assert!(name_attr.is_some());
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("hello_world"), "Hello World");
        assert_eq!(title_case("client-business-unit"), "Client Business Unit");
        assert_eq!(title_case("cbu"), "Cbu");
    }

    #[test]
    fn test_suggest_pii_from_fqn() {
        let label = suggest_security_label("entity.date_of_birth", "entity", &[]);
        assert_eq!(label.classification, Classification::Confidential);
        assert!(label.pii);
        assert!(label
            .handling_controls
            .contains(&HandlingControl::MaskByDefault));
    }

    #[test]
    fn test_suggest_sanctions_domain() {
        let label = suggest_security_label("screening.check_result", "screening", &[]);
        assert_eq!(label.classification, Classification::Restricted);
        assert!(label
            .handling_controls
            .contains(&HandlingControl::NoLlmExternal));
    }

    #[test]
    fn test_suggest_financial_domain() {
        let label = suggest_security_label("deal.rate_value", "deal", &[]);
        assert_eq!(label.classification, Classification::Confidential);
        assert!(!label.pii);
    }

    #[test]
    fn test_suggest_default_label() {
        let label = suggest_security_label("cbu.status", "cbu", &[]);
        assert_eq!(label.classification, Classification::Internal);
        assert!(!label.pii);
        assert!(label.handling_controls.is_empty());
    }

    #[test]
    fn test_domain_to_subject_kind_common() {
        assert_eq!(super::domain_to_subject_kind("cbu"), "cbu");
        assert_eq!(super::domain_to_subject_kind("kyc"), "kyc-case");
        assert_eq!(super::domain_to_subject_kind("deal"), "deal");
        assert_eq!(super::domain_to_subject_kind("session"), "session");
        assert_eq!(super::domain_to_subject_kind("gleif"), "entity");
        assert_eq!(
            super::domain_to_subject_kind("trading-profile"),
            "trading-profile"
        );
    }

    #[test]
    fn test_domain_to_subject_kind_fallback() {
        // Unknown domains just return the domain name
        assert_eq!(
            super::domain_to_subject_kind("unknown-domain"),
            "unknown-domain"
        );
    }

    #[test]
    fn test_subject_kinds_from_produces() {
        let config = sample_verb_config(); // has produces: cbu
        let contract = verb_config_to_contract("cbu", "create", &config);
        assert_eq!(contract.subject_kinds, vec!["cbu".to_string()]);
    }

    #[test]
    fn test_subject_kinds_from_lookup_args() {
        // Verb with no produces, no metadata.subject_kinds, but has lookup args
        let mut config = sample_verb_config();
        config.produces = None;
        config.metadata = None;
        // arg[1] has lookup.entity_type = Some("jurisdiction"), which is treated
        // as generic reference data, so this should fall back to the domain kind.
        let contract = verb_config_to_contract("cbu", "lookup-verb", &config);
        assert_eq!(contract.subject_kinds, vec!["cbu".to_string()]);
    }

    #[test]
    fn test_subject_kinds_from_crud_table() {
        let mut config = sample_verb_config();
        config.produces = None;
        config.metadata = None;
        config.crud = Some(CrudConfig {
            operation: CrudOperation::Insert,
            table: Some("documents".into()),
            schema: Some("ob-poc".into()),
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
            order_by: None,
            set_values: None,
            extension_table_column: None,
            type_id_column: None,
            type_code: None,
        });
        let contract = verb_config_to_contract("research", "store-document", &config);
        assert_eq!(contract.subject_kinds, vec!["document".to_string()]);
    }

    #[test]
    fn test_subject_kinds_domain_fallback() {
        // Verb with no produces, no metadata, no lookup args
        let mut config = sample_verb_config();
        config.produces = None;
        config.metadata = None;
        config.args = vec![ArgConfig {
            name: "value".into(),
            arg_type: ArgType::String,
            required: true,
            maps_to: None,
            lookup: None,
            valid_values: None,
            default: None,
            description: None,
            validation: None,
            fuzzy_check: None,
            slot_type: None,
            preferred_roles: vec![],
        }];
        let contract = verb_config_to_contract("deal", "custom-action", &config);
        // Falls through all the way to domain_to_subject_kind("deal")
        assert_eq!(contract.subject_kinds, vec!["deal".to_string()]);
    }

    #[test]
    fn test_subject_kinds_accumulate_consumes_and_metadata_hints() {
        let mut config = sample_verb_config();
        config.produces = None;
        config.crud = None;
        config.args.clear();
        config.consumes = vec![
            dsl_core::config::types::VerbConsumes {
                arg: "entity-id".into(),
                consumed_type: "entity".into(),
                required: true,
            },
            dsl_core::config::types::VerbConsumes {
                arg: "mandate-id".into(),
                consumed_type: "mandate".into(),
                required: true,
            },
        ];
        config.metadata = Some(VerbMetadata {
            noun: Some("document".into()),
            tags: vec!["fund".into()],
            ..Default::default()
        });

        let contract = verb_config_to_contract("research", "review", &config);
        assert_eq!(
            contract.subject_kinds,
            vec![
                "document".to_string(),
                "entity".to_string(),
                "fund".to_string(),
                "trading-profile".to_string()
            ]
        );
    }

    #[test]
    fn test_subject_kinds_from_entity_arg_name() {
        let mut config = sample_verb_config();
        config.produces = None;
        config.metadata = None;
        config.crud = None;
        config.consumes.clear();
        config.args = vec![ArgConfig {
            name: "contract-id".into(),
            arg_type: ArgType::String,
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
        }];
        config.lifecycle = Some(dsl_core::config::types::VerbLifecycle {
            entity_arg: Some("contract-id".into()),
            ..Default::default()
        });

        let contract = verb_config_to_contract("workflow", "advance", &config);
        assert_eq!(contract.subject_kinds, vec!["contract".to_string()]);
    }

    #[test]
    fn test_phase_tags_from_metadata_tags() {
        // Verb with metadata.tags but empty phase_tags
        let mut config = sample_verb_config();
        config.metadata = Some(VerbMetadata {
            tags: vec!["kyc".into(), "onboarding".into()],
            phase_tags: vec![], // empty — should fallback to tags
            ..Default::default()
        });
        let contract = verb_config_to_contract("kyc", "check", &config);
        assert_eq!(
            contract.phase_tags,
            vec!["kyc".to_string(), "onboarding".to_string()]
        );
    }

    #[test]
    fn test_phase_tags_explicit_takes_precedence() {
        // Verb with both metadata.tags and explicit phase_tags
        let mut config = sample_verb_config();
        config.metadata = Some(VerbMetadata {
            tags: vec!["general".into()],
            phase_tags: vec!["specific-phase".into()], // explicit — takes precedence
            ..Default::default()
        });
        let contract = verb_config_to_contract("cbu", "special", &config);
        assert_eq!(contract.phase_tags, vec!["specific-phase".to_string()]);
    }

    #[test]
    fn test_classification_and_precondition_states_propagate() {
        let mut config = sample_verb_config();
        config.lifecycle = Some(VerbLifecycle {
            entity_arg: None,
            requires_states: vec!["review".into(), "approved".into()],
            transitions_to: None,
            transitions_to_arg: None,
            precondition_checks: vec![],
            writes_tables: vec![],
            reads_tables: vec![],
        });
        config.metadata = Some(VerbMetadata {
            harm_class: Some(DslHarmClass::Irreversible),
            action_class: Some(DslActionClass::Approve),
            ..Default::default()
        });

        let contract = verb_config_to_contract("kyc-case", "approve", &config);

        assert_eq!(contract.harm_class, Some(super::HarmClass::Irreversible));
        assert_eq!(contract.action_class, Some(super::ActionClass::Approve));
        assert_eq!(
            contract.precondition_states,
            vec!["review".to_string(), "approved".to_string()]
        );
    }

    // ── Domain metadata enrichment tests ─────────────────────────

    const ENRICHMENT_YAML: &str = r#"
domains:
  deal:
    description: "Commercial origination"
    tables:
      deals:
        description: "Commercial deal record"
        governance_tier: governed
        classification: confidential
        pii: false
    verb_data_footprint:
      deal.create:
        writes: [deals, deal_events]
        reads: [client_group]
      deal.summary:
        reads: [deals, deal_participants]
  cbu:
    description: "Client Business Unit"
    tables:
      cbus:
        description: "CBU operational container"
        governance_tier: governed
        classification: internal
        pii: false
    verb_data_footprint:
      cbu.create:
        writes: [cbus]
"#;

    fn enrichment_metadata() -> crate::metadata::DomainMetadata {
        crate::metadata::DomainMetadata::from_yaml(ENRICHMENT_YAML).unwrap()
    }

    #[test]
    fn test_enrich_verb_contracts_populates_reads_writes() {
        let meta = enrichment_metadata();
        let mut contracts = vec![
            VerbContractBody {
                fqn: "deal.create".into(),
                domain: "deal".into(),
                action: "create".into(),
                description: "Create deal".into(),
                behavior: "plugin".into(),
                args: vec![],
                returns: None,
                preconditions: vec![],
                postconditions: vec![],
                produces: None,
                consumes: vec![],
                invocation_phrases: vec![],
                subject_kinds: vec![],
                phase_tags: vec![],
                harm_class: None,
                action_class: None,
                precondition_states: vec![],
                requires_subject: true,
                produces_focus: false,
                metadata: None,
                crud_mapping: None,
                reads_from: vec![],
                writes_to: vec![],
                outputs: vec![],
            },
            VerbContractBody {
                fqn: "cbu.create".into(),
                domain: "cbu".into(),
                action: "create".into(),
                description: "Create CBU".into(),
                behavior: "plugin".into(),
                args: vec![],
                returns: None,
                preconditions: vec![],
                postconditions: vec![],
                produces: None,
                consumes: vec![],
                invocation_phrases: vec![],
                subject_kinds: vec![],
                phase_tags: vec![],
                harm_class: None,
                action_class: None,
                precondition_states: vec![],
                requires_subject: true,
                produces_focus: false,
                metadata: None,
                crud_mapping: None,
                reads_from: vec![],
                writes_to: vec![],
                outputs: vec![],
            },
        ];

        enrich_verb_contracts(&mut contracts, &meta);

        // deal.create should read client_group, write deals + deal_events
        let deal = contracts.iter().find(|c| c.fqn == "deal.create").unwrap();
        assert_eq!(deal.reads_from, vec!["client_group"]);
        assert_eq!(deal.writes_to, vec!["deals", "deal_events"]);

        // cbu.create should write cbus
        let cbu = contracts.iter().find(|c| c.fqn == "cbu.create").unwrap();
        assert!(cbu.reads_from.is_empty());
        assert_eq!(cbu.writes_to, vec!["cbus"]);
    }

    #[test]
    fn test_enrich_verb_contracts_no_footprint_leaves_empty() {
        let meta = enrichment_metadata();
        let mut contracts = vec![VerbContractBody {
            fqn: "unknown.verb".into(),
            domain: "unknown".into(),
            action: "verb".into(),
            description: "No footprint".into(),
            behavior: "plugin".into(),
            args: vec![],
            returns: None,
            preconditions: vec![],
            postconditions: vec![],
            produces: None,
            consumes: vec![],
            invocation_phrases: vec![],
            subject_kinds: vec![],
            phase_tags: vec![],
            harm_class: None,
            action_class: None,
            precondition_states: vec![],
            requires_subject: true,
            produces_focus: false,
            metadata: None,
            crud_mapping: None,
            reads_from: vec![],
            writes_to: vec![],
            outputs: vec![],
        }];

        enrich_verb_contracts(&mut contracts, &meta);
        assert!(contracts[0].reads_from.is_empty());
        assert!(contracts[0].writes_to.is_empty());
    }

    #[test]
    fn test_enrich_entity_types_populates_governance() {
        use sem_os_core::entity_type_def::{DbTableMapping, EntityTypeDefBody};

        let meta = enrichment_metadata();
        let mut entity_types = vec![EntityTypeDefBody {
            fqn: "deal".into(),
            name: "Deal".into(),
            description: "Deal entity".into(),
            domain: "deal".into(),
            db_table: Some(DbTableMapping {
                schema: "ob-poc".into(),
                table: "deals".into(),
                primary_key: "deal_id".into(),
                name_column: None,
            }),
            lifecycle_states: vec![],
            required_attributes: vec![],
            optional_attributes: vec![],
            parent_type: None,
            governance_tier: None,
            security_classification: None,
            pii: None,
            read_by_verbs: vec![],
            written_by_verbs: vec![],
        }];

        enrich_entity_types(&mut entity_types, &meta);

        let deal = &entity_types[0];
        assert_eq!(deal.governance_tier.as_deref(), Some("governed"));
        assert_eq!(
            deal.security_classification.as_deref(),
            Some("confidential")
        );
        assert_eq!(deal.pii, Some(false));
    }

    #[test]
    fn test_enrich_entity_types_populates_reverse_verbs() {
        use sem_os_core::entity_type_def::{DbTableMapping, EntityTypeDefBody};

        let meta = enrichment_metadata();
        let mut entity_types = vec![EntityTypeDefBody {
            fqn: "deal".into(),
            name: "Deal".into(),
            description: "Deal entity".into(),
            domain: "deal".into(),
            db_table: Some(DbTableMapping {
                schema: "ob-poc".into(),
                table: "deals".into(),
                primary_key: "deal_id".into(),
                name_column: None,
            }),
            lifecycle_states: vec![],
            required_attributes: vec![],
            optional_attributes: vec![],
            parent_type: None,
            governance_tier: None,
            security_classification: None,
            pii: None,
            read_by_verbs: vec![],
            written_by_verbs: vec![],
        }];

        enrich_entity_types(&mut entity_types, &meta);

        let deal = &entity_types[0];
        // deals is read by deal.summary, written by deal.create
        assert!(deal.read_by_verbs.contains(&"deal.summary".to_string()));
        assert!(deal.written_by_verbs.contains(&"deal.create".to_string()));
        // Results should be sorted
        assert_eq!(deal.read_by_verbs, {
            let mut v = deal.read_by_verbs.clone();
            v.sort();
            v
        });
    }

    #[test]
    fn test_enrich_entity_types_fqn_heuristic() {
        use sem_os_core::entity_type_def::EntityTypeDefBody;

        let meta = enrichment_metadata();
        // Entity type without db_table — uses fqn + "s" heuristic
        let mut entity_types = vec![EntityTypeDefBody {
            fqn: "cbu".into(),
            name: "CBU".into(),
            description: "Client Business Unit".into(),
            domain: "cbu".into(),
            db_table: None, // no db_table → falls back to "cbus"
            lifecycle_states: vec![],
            required_attributes: vec![],
            optional_attributes: vec![],
            parent_type: None,
            governance_tier: None,
            security_classification: None,
            pii: None,
            read_by_verbs: vec![],
            written_by_verbs: vec![],
        }];

        enrich_entity_types(&mut entity_types, &meta);

        let cbu = &entity_types[0];
        // "cbus" found in metadata → governance populated
        assert_eq!(cbu.governance_tier.as_deref(), Some("governed"));
        assert_eq!(cbu.security_classification.as_deref(), Some("internal"));
        // cbus is written by cbu.create
        assert!(cbu.written_by_verbs.contains(&"cbu.create".to_string()));
    }
}
