//! Verb-first onboarding scanner.
//!
//! Scans existing verb YAML definitions and bootstraps VerbContract, EntityTypeDef,
//! and AttributeDef snapshots into the semantic registry.
//!
//! The scanner is **idempotent**: it checks if an FQN already exists before publishing.
//!
//! Pure conversion functions (verb_config_to_contract, infer_entity_types_from_verbs,
//! infer_attributes_from_verbs, suggest_security_label) are delegated to
//! `sem_os_obpoc_adapter::scanner`. This module owns the DB-dependent orchestrator.

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use dsl_core::config::loader::ConfigLoader;

use super::{
    ids::{definition_hash, object_id_for},
    store::SnapshotStore,
    types::{ChangeType, ObjectType, SnapshotMeta},
};

// Re-export pure conversion functions from the adapter so existing call sites work.
// Note: suggest_security_label is NOT re-exported because it returns sem_os_core types
// which differ from ob-poc's sqlx-annotated types. A local wrapper is provided below.
pub use sem_os_obpoc_adapter::scanner::{
    arg_type_to_attribute_type, infer_attributes_from_verbs, infer_entity_types_from_verbs,
    scan_verb_contracts, title_case, to_wire_str, verb_config_to_contract,
};

/// Suggest a security label for a snapshot based on FQN/domain/tag heuristics.
///
/// Delegates to the adapter's pure implementation, then converts to ob-poc types.
pub fn suggest_security_label(
    fqn: &str,
    domain: &str,
    tags: &[String],
) -> super::types::SecurityLabel {
    let adapter_label = sem_os_obpoc_adapter::scanner::suggest_security_label(fqn, domain, tags);

    // Convert sem_os_core types → ob-poc types (structurally identical, different crate paths)
    use super::types::{Classification, HandlingControl, SecurityLabel};

    let classification = match adapter_label.classification {
        sem_os_core::types::Classification::Public => Classification::Public,
        sem_os_core::types::Classification::Internal => Classification::Internal,
        sem_os_core::types::Classification::Confidential => Classification::Confidential,
        sem_os_core::types::Classification::Restricted => Classification::Restricted,
    };

    let handling_controls = adapter_label
        .handling_controls
        .into_iter()
        .map(|hc| match hc {
            sem_os_core::types::HandlingControl::MaskByDefault => HandlingControl::MaskByDefault,
            sem_os_core::types::HandlingControl::NoExport => HandlingControl::NoExport,
            sem_os_core::types::HandlingControl::NoLlmExternal => HandlingControl::NoLlmExternal,
            sem_os_core::types::HandlingControl::DualControl => HandlingControl::DualControl,
            sem_os_core::types::HandlingControl::SecureViewerOnly => {
                HandlingControl::SecureViewerOnly
            }
        })
        .collect();

    SecurityLabel {
        classification,
        pii: adapter_label.pii,
        jurisdictions: adapter_label.jurisdictions,
        purpose_limitation: adapter_label.purpose_limitation,
        handling_controls,
    }
}

/// Summary report from a scan run.
#[derive(Debug, Default)]
pub struct ScanReport {
    pub verb_contracts_published: usize,
    pub verb_contracts_skipped: usize,
    pub verb_contracts_updated: usize,
    pub entity_types_published: usize,
    pub entity_types_skipped: usize,
    pub entity_types_updated: usize,
    pub attributes_published: usize,
    pub attributes_skipped: usize,
    pub attributes_updated: usize,
    pub taxonomies_published: usize,
    pub taxonomies_skipped: usize,
    pub taxonomies_updated: usize,
    pub taxonomy_nodes_published: usize,
    pub taxonomy_nodes_skipped: usize,
    pub taxonomy_nodes_updated: usize,
    pub views_published: usize,
    pub views_skipped: usize,
    pub views_updated: usize,
    pub policies_published: usize,
    pub policies_skipped: usize,
    pub policies_updated: usize,
    pub derivation_specs_published: usize,
    pub derivation_specs_skipped: usize,
    pub derivation_specs_updated: usize,
}

impl std::fmt::Display for ScanReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Scan Report:")?;
        writeln!(
            f,
            "  Verb contracts:  {} published, {} updated, {} skipped",
            self.verb_contracts_published, self.verb_contracts_updated, self.verb_contracts_skipped
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
        writeln!(
            f,
            "  Taxonomies:      {} published, {} updated, {} skipped",
            self.taxonomies_published, self.taxonomies_updated, self.taxonomies_skipped
        )?;
        writeln!(
            f,
            "  Taxonomy nodes:  {} published, {} updated, {} skipped",
            self.taxonomy_nodes_published, self.taxonomy_nodes_updated, self.taxonomy_nodes_skipped
        )?;
        writeln!(
            f,
            "  Views:           {} published, {} updated, {} skipped",
            self.views_published, self.views_updated, self.views_skipped
        )?;
        writeln!(
            f,
            "  Policies:        {} published, {} updated, {} skipped",
            self.policies_published, self.policies_updated, self.policies_skipped
        )?;
        writeln!(
            f,
            "  Derivation specs:{} published, {} updated, {} skipped",
            self.derivation_specs_published,
            self.derivation_specs_updated,
            self.derivation_specs_skipped
        )?;
        let total = self.verb_contracts_published
            + self.entity_types_published
            + self.attributes_published
            + self.taxonomies_published
            + self.taxonomy_nodes_published
            + self.views_published
            + self.policies_published
            + self.derivation_specs_published;
        write!(f, "  Total new snapshots: {}", total)
    }
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

    // 1. Convert verb configs to contracts (delegates to adapter)
    let contracts = scan_verb_contracts(&verbs_config);

    if verbose {
        println!("Found {} verb definitions in YAML", contracts.len());
    }

    // 2. Infer entity types (delegates to adapter)
    let entity_types = infer_entity_types_from_verbs(&verbs_config);
    if verbose {
        println!(
            "Inferred {} entity types from verb lookups",
            entity_types.len()
        );
    }

    // 3. Infer attributes (delegates to adapter)
    let attributes = infer_attributes_from_verbs(&verbs_config);
    if verbose {
        println!("Inferred {} attributes from verb args", attributes.len());
    }

    if dry_run {
        report.verb_contracts_published = contracts.len();
        report.entity_types_published = entity_types.len();
        report.attributes_published = attributes.len();
        // Seed reports in dry_run mode
        let tax_report = super::seeds::seed_taxonomies(pool, Uuid::nil(), true, verbose).await?;
        report.taxonomies_published = tax_report.taxonomies_published;
        report.taxonomy_nodes_published = tax_report.nodes_published;
        let view_report = super::seeds::seed_views(pool, Uuid::nil(), true, verbose).await?;
        report.views_published = view_report.views_published;
        let policy_report = super::seeds::seed_policies(pool, Uuid::nil(), true, verbose).await?;
        report.policies_published = policy_report.policies_published;
        let derivation_report =
            super::seeds::seed_derivation_specs(pool, Uuid::nil(), true, verbose).await?;
        report.derivation_specs_published = derivation_report.derivations_published;
        println!("\n[DRY RUN] Would publish:");
        println!("  {} verb contracts", contracts.len());
        println!("  {} entity types", entity_types.len());
        println!("  {} attributes", attributes.len());
        println!(
            "  {} taxonomies ({} nodes)",
            report.taxonomies_published, report.taxonomy_nodes_published
        );
        println!("  {} views", report.views_published);
        println!("  {} policies", report.policies_published);
        println!("  {} derivation specs", report.derivation_specs_published);
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

        let object_id = object_id_for(ObjectType::VerbContract, &contract.fqn);
        let definition = serde_json::to_value(contract)?;
        let new_hash = definition_hash(&definition);

        if let Some(existing_row) = existing {
            let old_hash = definition_hash(&existing_row.definition);
            if old_hash == new_hash {
                report.verb_contracts_skipped += 1;
                if verbose {
                    println!("  SKIP verb: {} (unchanged)", contract.fqn);
                }
            } else {
                // Definition changed — publish successor snapshot
                let mut meta =
                    SnapshotMeta::new_operational(ObjectType::VerbContract, object_id, "scanner");
                meta.predecessor_id = Some(existing_row.snapshot_id);
                meta.version_major = existing_row.version_major;
                meta.version_minor = existing_row.version_minor + 1;
                meta.change_type = ChangeType::NonBreaking;
                meta.change_rationale = Some("Scanner drift update".into());
                SnapshotStore::publish_snapshot(pool, &meta, &definition, Some(set_id)).await?;
                report.verb_contracts_updated += 1;
                if verbose {
                    println!("  UPD  verb: {} (definition changed)", contract.fqn);
                }
            }
            continue;
        }

        let meta = SnapshotMeta::new_operational(ObjectType::VerbContract, object_id, "scanner");
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

        let object_id = object_id_for(ObjectType::EntityTypeDef, &entity_type.fqn);
        let definition = serde_json::to_value(entity_type)?;
        let new_hash = definition_hash(&definition);

        if let Some(existing_row) = existing {
            let old_hash = definition_hash(&existing_row.definition);
            if old_hash == new_hash {
                report.entity_types_skipped += 1;
                if verbose {
                    println!("  SKIP entity type: {} (unchanged)", entity_type.fqn);
                }
            } else {
                let mut meta =
                    SnapshotMeta::new_operational(ObjectType::EntityTypeDef, object_id, "scanner");
                meta.predecessor_id = Some(existing_row.snapshot_id);
                meta.version_major = existing_row.version_major;
                meta.version_minor = existing_row.version_minor + 1;
                meta.change_type = ChangeType::NonBreaking;
                meta.change_rationale = Some("Scanner drift update".into());
                SnapshotStore::publish_snapshot(pool, &meta, &definition, Some(set_id)).await?;
                report.entity_types_updated += 1;
                if verbose {
                    println!(
                        "  UPD  entity type: {} (definition changed)",
                        entity_type.fqn
                    );
                }
            }
            continue;
        }

        let meta = SnapshotMeta::new_operational(ObjectType::EntityTypeDef, object_id, "scanner");
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

        let object_id = object_id_for(ObjectType::AttributeDef, &attr.fqn);
        let definition = serde_json::to_value(attr)?;
        let new_hash = definition_hash(&definition);

        if let Some(existing_row) = existing {
            let old_hash = definition_hash(&existing_row.definition);
            if old_hash == new_hash {
                report.attributes_skipped += 1;
            } else {
                let mut meta =
                    SnapshotMeta::new_operational(ObjectType::AttributeDef, object_id, "scanner");
                meta.predecessor_id = Some(existing_row.snapshot_id);
                meta.version_major = existing_row.version_major;
                meta.version_minor = existing_row.version_minor + 1;
                meta.change_type = ChangeType::NonBreaking;
                meta.change_rationale = Some("Scanner drift update".into());
                SnapshotStore::publish_snapshot(pool, &meta, &definition, Some(set_id)).await?;
                report.attributes_updated += 1;
            }
            continue;
        }

        let meta = SnapshotMeta::new_operational(ObjectType::AttributeDef, object_id, "scanner");
        SnapshotStore::insert_snapshot(pool, &meta, &definition, Some(set_id)).await?;
        report.attributes_published += 1;
    }

    // 8. Seed taxonomies
    if verbose {
        println!("\nSeeding taxonomies...");
    }
    let tax_report = super::seeds::seed_taxonomies(pool, set_id, dry_run, verbose).await?;
    report.taxonomies_published = tax_report.taxonomies_published;
    report.taxonomies_skipped = tax_report.taxonomies_skipped;
    report.taxonomies_updated = tax_report.taxonomies_updated;
    report.taxonomy_nodes_published = tax_report.nodes_published;
    report.taxonomy_nodes_skipped = tax_report.nodes_skipped;
    report.taxonomy_nodes_updated = tax_report.nodes_updated;

    // 9. Seed views
    if verbose {
        println!("\nSeeding views...");
    }
    let view_report = super::seeds::seed_views(pool, set_id, dry_run, verbose).await?;
    report.views_published = view_report.views_published;
    report.views_skipped = view_report.views_skipped;
    report.views_updated = view_report.views_updated;

    // 10. Seed policies
    if verbose {
        println!("\nSeeding policies...");
    }
    let policy_report = super::seeds::seed_policies(pool, set_id, dry_run, verbose).await?;
    report.policies_published = policy_report.policies_published;
    report.policies_skipped = policy_report.policies_skipped;
    report.policies_updated = policy_report.policies_updated;

    // 11. Seed derivation specs
    if verbose {
        println!("\nSeeding derivation specs...");
    }
    let derivation_report =
        super::seeds::seed_derivation_specs(pool, set_id, dry_run, verbose).await?;
    report.derivation_specs_published = derivation_report.derivations_published;
    report.derivation_specs_skipped = derivation_report.derivations_skipped;
    report.derivation_specs_updated = derivation_report.derivations_updated;

    Ok(report)
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
