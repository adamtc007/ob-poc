//! Verb-first onboarding scanner.
//!
//! Scans existing verb YAML definitions and bootstraps VerbContract, EntityTypeDef,
//! and AttributeDef snapshots into the semantic registry.
//!
//! The scanner is **idempotent**: it checks if an FQN already exists before publishing.

use std::collections::BTreeMap;

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use dsl_core::config::loader::ConfigLoader;
use dsl_core::config::types::{ArgConfig, VerbConfig, VerbsConfig};

use super::{
    attribute_def::{AttributeDataType, AttributeDefBody, AttributeSource},
    entity_type_def::{DbTableMapping, EntityTypeDefBody},
    store::SnapshotStore,
    types::{ObjectType, SnapshotMeta},
    verb_contract::{
        VerbArgDef, VerbArgLookup, VerbContractBody, VerbContractMetadata, VerbPrecondition,
        VerbProducesSpec, VerbReturnSpec,
    },
};

/// Summary report from a scan run.
#[derive(Debug, Default)]
pub struct ScanReport {
    pub verb_contracts_published: usize,
    pub verb_contracts_skipped: usize,
    pub entity_types_published: usize,
    pub entity_types_skipped: usize,
    pub attributes_published: usize,
    pub attributes_skipped: usize,
}

impl std::fmt::Display for ScanReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Scan Report:")?;
        writeln!(
            f,
            "  Verb contracts:  {} published, {} skipped (already exist)",
            self.verb_contracts_published, self.verb_contracts_skipped
        )?;
        writeln!(
            f,
            "  Entity types:    {} published, {} skipped",
            self.entity_types_published, self.entity_types_skipped
        )?;
        writeln!(
            f,
            "  Attributes:      {} published, {} skipped",
            self.attributes_published, self.attributes_skipped
        )?;
        let total =
            self.verb_contracts_published + self.entity_types_published + self.attributes_published;
        write!(f, "  Total new snapshots: {}", total)
    }
}

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
            arg_type: format!("{:?}", a.arg_type).to_lowercase(),
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
        return_type: format!("{:?}", r.return_type).to_lowercase(),
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

    // Extract preconditions from lifecycle
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
        tier: m.tier.as_ref().map(|t| format!("{:?}", t).to_lowercase()),
        source_of_truth: m
            .source_of_truth
            .as_ref()
            .map(|s| format!("{:?}", s).to_lowercase()),
        scope: m.scope.as_ref().map(|s| format!("{:?}", s).to_lowercase()),
        noun: m.noun.clone(),
        tags: m.tags.clone(),
    });

    VerbContractBody {
        fqn,
        domain: domain.to_string(),
        action: action.to_string(),
        description: config.description.clone(),
        behavior: format!("{:?}", config.behavior).to_lowercase(),
        args,
        returns,
        preconditions,
        postconditions: vec![],
        produces,
        consumes,
        invocation_phrases: config.invocation_phrases.clone(),
        metadata,
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
pub fn infer_attributes_from_verbs(verbs_config: &VerbsConfig) -> Vec<AttributeDefBody> {
    let mut seen: BTreeMap<String, AttributeDefBody> = BTreeMap::new();

    for (domain, domain_config) in &verbs_config.domains {
        for (action, verb_config) in &domain_config.verbs {
            for arg in &verb_config.args {
                let fqn = format!("{}.{}", domain, arg.name);
                let action = action.clone();
                let domain = domain.clone();
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
                        table: arg.maps_to.as_ref().map(|_| domain.clone()),
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

/// Run the full onboarding scan.
///
/// If `dry_run` is true, reports counts without writing to the database.
pub async fn run_onboarding_scan(
    pool: &PgPool,
    dry_run: bool,
    verbose: bool,
) -> Result<ScanReport> {
    let loader = ConfigLoader::from_env();
    let verbs_config = loader.load_verbs()?;
    let mut report = ScanReport::default();

    // 1. Convert verb configs to contracts
    let mut contracts = Vec::new();
    for (domain, domain_config) in &verbs_config.domains {
        for (action, verb_config) in &domain_config.verbs {
            contracts.push(verb_config_to_contract(domain, action, verb_config));
        }
    }
    contracts.sort_by(|a, b| a.fqn.cmp(&b.fqn));

    if verbose {
        println!("Found {} verb definitions in YAML", contracts.len());
    }

    // 2. Infer entity types
    let entity_types = infer_entity_types_from_verbs(&verbs_config);
    if verbose {
        println!(
            "Inferred {} entity types from verb lookups",
            entity_types.len()
        );
    }

    // 3. Infer attributes
    let attributes = infer_attributes_from_verbs(&verbs_config);
    if verbose {
        println!("Inferred {} attributes from verb args", attributes.len());
    }

    if dry_run {
        report.verb_contracts_published = contracts.len();
        report.entity_types_published = entity_types.len();
        report.attributes_published = attributes.len();
        println!("\n[DRY RUN] Would publish:");
        println!("  {} verb contracts", contracts.len());
        println!("  {} entity types", entity_types.len());
        println!("  {} attributes", attributes.len());
        return Ok(report);
    }

    // 4. Create a snapshot set for the entire scan
    let set_id =
        SnapshotStore::create_snapshot_set(pool, Some("onboarding-scan"), "scanner").await?;

    // 5. Publish verb contracts (idempotent by FQN)
    for contract in &contracts {
        let existing = SnapshotStore::find_active_by_definition_field(
            pool,
            ObjectType::VerbContract,
            "fqn",
            &contract.fqn,
        )
        .await?;

        if existing.is_some() {
            report.verb_contracts_skipped += 1;
            if verbose {
                println!("  SKIP verb: {} (already exists)", contract.fqn);
            }
            continue;
        }

        let object_id = Uuid::new_v4();
        let meta = SnapshotMeta::new_operational(ObjectType::VerbContract, object_id, "scanner");
        let definition = serde_json::to_value(contract)?;
        SnapshotStore::insert_snapshot(pool, &meta, &definition, Some(set_id)).await?;
        report.verb_contracts_published += 1;

        if verbose {
            println!("  NEW  verb: {}", contract.fqn);
        }
    }

    // 6. Publish entity types (idempotent by FQN)
    for entity_type in &entity_types {
        let existing = SnapshotStore::find_active_by_definition_field(
            pool,
            ObjectType::EntityTypeDef,
            "fqn",
            &entity_type.fqn,
        )
        .await?;

        if existing.is_some() {
            report.entity_types_skipped += 1;
            if verbose {
                println!("  SKIP entity type: {} (already exists)", entity_type.fqn);
            }
            continue;
        }

        let object_id = Uuid::new_v4();
        let meta = SnapshotMeta::new_operational(ObjectType::EntityTypeDef, object_id, "scanner");
        let definition = serde_json::to_value(entity_type)?;
        SnapshotStore::insert_snapshot(pool, &meta, &definition, Some(set_id)).await?;
        report.entity_types_published += 1;

        if verbose {
            println!("  NEW  entity type: {}", entity_type.fqn);
        }
    }

    // 7. Publish attributes (idempotent by FQN)
    for attr in &attributes {
        let existing = SnapshotStore::find_active_by_definition_field(
            pool,
            ObjectType::AttributeDef,
            "fqn",
            &attr.fqn,
        )
        .await?;

        if existing.is_some() {
            report.attributes_skipped += 1;
            continue;
        }

        let object_id = Uuid::new_v4();
        let meta = SnapshotMeta::new_operational(ObjectType::AttributeDef, object_id, "scanner");
        let definition = serde_json::to_value(attr)?;
        SnapshotStore::insert_snapshot(pool, &meta, &definition, Some(set_id)).await?;
        report.attributes_published += 1;
    }

    Ok(report)
}

// ── Security label heuristic ──────────────────────────────────

/// Suggest a security label for an existing snapshot based on heuristics.
///
/// Uses FQN patterns, domain, and tags to assign a reasonable default:
/// - PII patterns (name, address, dob, ssn, passport) → Confidential + PII
/// - Sanctions domain → Restricted + NoLlmExternal
/// - Financial domain (deal, billing, rate) → Confidential
/// - Default → Internal, no special handling
pub fn suggest_security_label(
    fqn: &str,
    domain: &str,
    tags: &[String],
) -> super::types::SecurityLabel {
    use super::types::{Classification, HandlingControl, SecurityLabel};

    let fqn_lower = fqn.to_lowercase();
    let domain_lower = domain.to_lowercase();
    let tags_lower: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    // PII patterns in FQN or tags
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

    // Sanctions domain
    let is_sanctions = domain_lower == "sanctions"
        || domain_lower == "screening"
        || tags_lower.iter().any(|t| t == "sanctions");

    // Financial domain
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

// ── Helpers ───────────────────────────────────────────────────

fn title_case(s: &str) -> String {
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

fn arg_type_to_attribute_type(arg: &ArgConfig) -> AttributeDataType {
    match format!("{:?}", arg.arg_type).to_lowercase().as_str() {
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
        // Should find jurisdiction from the lookup config
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

        let attrs = infer_attributes_from_verbs(&config);
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

    // ── suggest_security_label tests ──────────────────────────

    #[test]
    fn test_suggest_pii_from_fqn() {
        let label = suggest_security_label("entity.date_of_birth", "entity", &[]);
        assert_eq!(
            label.classification,
            super::super::types::Classification::Confidential
        );
        assert!(label.pii);
        assert!(label
            .handling_controls
            .contains(&super::super::types::HandlingControl::MaskByDefault));
    }

    #[test]
    fn test_suggest_sanctions_domain() {
        let label = suggest_security_label("screening.check_result", "screening", &[]);
        assert_eq!(
            label.classification,
            super::super::types::Classification::Restricted
        );
        assert!(label
            .handling_controls
            .contains(&super::super::types::HandlingControl::NoLlmExternal));
    }

    #[test]
    fn test_suggest_financial_domain() {
        let label = suggest_security_label("deal.rate_value", "deal", &[]);
        assert_eq!(
            label.classification,
            super::super::types::Classification::Confidential
        );
        assert!(!label.pii);
    }

    #[test]
    fn test_suggest_default_label() {
        let label = suggest_security_label("cbu.status", "cbu", &[]);
        assert_eq!(
            label.classification,
            super::super::types::Classification::Internal
        );
        assert!(!label.pii);
        assert!(label.handling_controls.is_empty());
    }
}
