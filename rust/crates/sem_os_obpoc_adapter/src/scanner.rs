//! Verb-first onboarding scanner — pure conversion functions.
//!
//! Converts ob-poc verb YAML definitions into Semantic OS typed seed data.
//! All functions here are **pure** (no DB, no I/O). The DB-publishing
//! orchestrator remains in `ob-poc/src/sem_reg/scanner.rs` and delegates
//! to these converters.

use std::collections::BTreeMap;

use dsl_core::config::types::{ArgConfig, VerbConfig, VerbsConfig};

use sem_os_core::{
    attribute_def::{AttributeDataType, AttributeDefBody, AttributeSource},
    entity_type_def::{DbTableMapping, EntityTypeDefBody},
    types::{Classification, HandlingControl, SecurityLabel},
    verb_contract::{
        VerbArgDef, VerbArgLookup, VerbContractBody, VerbContractMetadata, VerbCrudMapping,
        VerbPrecondition, VerbProducesSpec, VerbReturnSpec,
    },
};

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

    let subject_kinds = config
        .metadata
        .as_ref()
        .map(|m| m.subject_kinds.clone())
        .filter(|sk| !sk.is_empty())
        .unwrap_or_else(|| {
            config
                .produces
                .as_ref()
                .map(|p| vec![p.produced_type.clone()])
                .unwrap_or_default()
        });

    let phase_tags = config
        .metadata
        .as_ref()
        .map(|m| m.phase_tags.clone())
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
        requires_subject,
        produces_focus,
        metadata,
        crud_mapping,
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
                    et.db_table
                        .as_ref()
                        .map_or((None, None), |dt| (Some(dt.schema.clone()), Some(dt.table.clone())))
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

pub fn arg_type_to_attribute_type(arg: &ArgConfig) -> AttributeDataType {
    match to_wire_str(&arg.arg_type).as_str() {
        "string" => AttributeDataType::String,
        "integer" | "int" => AttributeDataType::Integer,
        "decimal" | "number" | "float" => AttributeDataType::Decimal,
        "boolean" | "bool" => AttributeDataType::Boolean,
        "uuid" => AttributeDataType::Uuid,
        "date" => AttributeDataType::Date,
        "timestamp" => AttributeDataType::Timestamp,
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
}
