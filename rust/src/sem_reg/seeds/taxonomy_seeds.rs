//! Bootstrap taxonomy definitions and nodes for the semantic registry.
//!
//! Seeds core taxonomies derived from the existing domain structure:
//! - Entity classification (by entity kind)
//! - Jurisdiction (geographic regions)
//! - Instrument classification (asset classes)
//! - Governance tier (governed vs operational)
//! - Domain (verb/attribute domains)

use std::collections::BTreeMap;

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_reg::{
    ids::{definition_hash, object_id_for},
    store::SnapshotStore,
    taxonomy_def::{TaxonomyDefBody, TaxonomyNodeBody},
    types::{ChangeType, ObjectType, SnapshotMeta},
};

/// Report from taxonomy seeding.
#[derive(Debug, Default)]
pub struct TaxonomySeedReport {
    pub taxonomies_published: usize,
    pub taxonomies_skipped: usize,
    pub taxonomies_updated: usize,
    pub nodes_published: usize,
    pub nodes_skipped: usize,
    pub nodes_updated: usize,
}

impl std::fmt::Display for TaxonomySeedReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Taxonomies: {} published, {} updated, {} skipped | Nodes: {} published, {} updated, {} skipped",
            self.taxonomies_published,
            self.taxonomies_updated,
            self.taxonomies_skipped,
            self.nodes_published,
            self.nodes_updated,
            self.nodes_skipped,
        )
    }
}

/// Core taxonomy definitions to bootstrap.
fn core_taxonomies() -> Vec<(TaxonomyDefBody, Vec<TaxonomyNodeBody>)> {
    vec![
        // 1. Entity classification taxonomy
        entity_classification_taxonomy(),
        // 2. Jurisdiction taxonomy
        jurisdiction_taxonomy(),
        // 3. Instrument classification taxonomy
        instrument_taxonomy(),
        // 4. Governance tier taxonomy
        governance_tier_taxonomy(),
        // 5. Domain taxonomy
        domain_taxonomy(),
    ]
}

fn entity_classification_taxonomy() -> (TaxonomyDefBody, Vec<TaxonomyNodeBody>) {
    let tax = TaxonomyDefBody {
        fqn: "taxonomy.entity-classification".into(),
        name: "Entity Classification".into(),
        description: "Classifies entities by their kind (person, fund, legal entity, etc.)".into(),
        domain: "entity".into(),
        root_node_fqn: Some("taxonomy.entity-classification.root".into()),
        max_depth: Some(3),
        classification_axis: Some("entity_kind".into()),
    };

    let nodes = vec![
        node(
            "taxonomy.entity-classification.root",
            "All Entities",
            &tax.fqn,
            None,
            0,
        ),
        node(
            "taxonomy.entity-classification.legal-entity",
            "Legal Entity",
            &tax.fqn,
            Some("taxonomy.entity-classification.root"),
            1,
        ),
        node(
            "taxonomy.entity-classification.natural-person",
            "Natural Person",
            &tax.fqn,
            Some("taxonomy.entity-classification.root"),
            2,
        ),
        node(
            "taxonomy.entity-classification.fund",
            "Fund",
            &tax.fqn,
            Some("taxonomy.entity-classification.legal-entity"),
            1,
        ),
        node(
            "taxonomy.entity-classification.bank",
            "Bank",
            &tax.fqn,
            Some("taxonomy.entity-classification.legal-entity"),
            2,
        ),
        node(
            "taxonomy.entity-classification.custodian",
            "Custodian",
            &tax.fqn,
            Some("taxonomy.entity-classification.legal-entity"),
            3,
        ),
    ];

    (tax, nodes)
}

fn jurisdiction_taxonomy() -> (TaxonomyDefBody, Vec<TaxonomyNodeBody>) {
    let tax = TaxonomyDefBody {
        fqn: "taxonomy.jurisdiction".into(),
        name: "Jurisdiction".into(),
        description:
            "Geographic jurisdiction classification for regulatory and operational context".into(),
        domain: "jurisdiction".into(),
        root_node_fqn: Some("taxonomy.jurisdiction.root".into()),
        max_depth: Some(3),
        classification_axis: Some("jurisdiction".into()),
    };

    let nodes = vec![
        node("taxonomy.jurisdiction.root", "Global", &tax.fqn, None, 0),
        node(
            "taxonomy.jurisdiction.europe",
            "Europe",
            &tax.fqn,
            Some("taxonomy.jurisdiction.root"),
            1,
        ),
        node(
            "taxonomy.jurisdiction.americas",
            "Americas",
            &tax.fqn,
            Some("taxonomy.jurisdiction.root"),
            2,
        ),
        node(
            "taxonomy.jurisdiction.apac",
            "Asia Pacific",
            &tax.fqn,
            Some("taxonomy.jurisdiction.root"),
            3,
        ),
        node(
            "taxonomy.jurisdiction.lu",
            "Luxembourg",
            &tax.fqn,
            Some("taxonomy.jurisdiction.europe"),
            1,
        ),
        node(
            "taxonomy.jurisdiction.ie",
            "Ireland",
            &tax.fqn,
            Some("taxonomy.jurisdiction.europe"),
            2,
        ),
        node(
            "taxonomy.jurisdiction.de",
            "Germany",
            &tax.fqn,
            Some("taxonomy.jurisdiction.europe"),
            3,
        ),
        node(
            "taxonomy.jurisdiction.uk",
            "United Kingdom",
            &tax.fqn,
            Some("taxonomy.jurisdiction.europe"),
            4,
        ),
        node(
            "taxonomy.jurisdiction.us",
            "United States",
            &tax.fqn,
            Some("taxonomy.jurisdiction.americas"),
            1,
        ),
    ];

    (tax, nodes)
}

fn instrument_taxonomy() -> (TaxonomyDefBody, Vec<TaxonomyNodeBody>) {
    let tax = TaxonomyDefBody {
        fqn: "taxonomy.instrument-class".into(),
        name: "Instrument Classification".into(),
        description: "Classification of financial instruments by asset class".into(),
        domain: "trading".into(),
        root_node_fqn: Some("taxonomy.instrument-class.root".into()),
        max_depth: Some(3),
        classification_axis: Some("instrument_class".into()),
    };

    let nodes = vec![
        node(
            "taxonomy.instrument-class.root",
            "All Instruments",
            &tax.fqn,
            None,
            0,
        ),
        node(
            "taxonomy.instrument-class.equity",
            "Equity",
            &tax.fqn,
            Some("taxonomy.instrument-class.root"),
            1,
        ),
        node(
            "taxonomy.instrument-class.fixed-income",
            "Fixed Income",
            &tax.fqn,
            Some("taxonomy.instrument-class.root"),
            2,
        ),
        node(
            "taxonomy.instrument-class.derivatives",
            "Derivatives",
            &tax.fqn,
            Some("taxonomy.instrument-class.root"),
            3,
        ),
        node(
            "taxonomy.instrument-class.otc",
            "OTC",
            &tax.fqn,
            Some("taxonomy.instrument-class.derivatives"),
            1,
        ),
        node(
            "taxonomy.instrument-class.listed",
            "Listed",
            &tax.fqn,
            Some("taxonomy.instrument-class.derivatives"),
            2,
        ),
    ];

    (tax, nodes)
}

fn governance_tier_taxonomy() -> (TaxonomyDefBody, Vec<TaxonomyNodeBody>) {
    let tax = TaxonomyDefBody {
        fqn: "taxonomy.governance-tier".into(),
        name: "Governance Tier".into(),
        description: "Classifies objects by governance rigour (governed vs operational)".into(),
        domain: "governance".into(),
        root_node_fqn: Some("taxonomy.governance-tier.root".into()),
        max_depth: Some(2),
        classification_axis: Some("governance_tier".into()),
    };

    let nodes = vec![
        node(
            "taxonomy.governance-tier.root",
            "All Tiers",
            &tax.fqn,
            None,
            0,
        ),
        node(
            "taxonomy.governance-tier.governed",
            "Governed",
            &tax.fqn,
            Some("taxonomy.governance-tier.root"),
            1,
        ),
        node(
            "taxonomy.governance-tier.operational",
            "Operational",
            &tax.fqn,
            Some("taxonomy.governance-tier.root"),
            2,
        ),
    ];

    (tax, nodes)
}

fn domain_taxonomy() -> (TaxonomyDefBody, Vec<TaxonomyNodeBody>) {
    let tax = TaxonomyDefBody {
        fqn: "taxonomy.domain".into(),
        name: "Domain".into(),
        description: "Classification by business domain".into(),
        domain: "system".into(),
        root_node_fqn: Some("taxonomy.domain.root".into()),
        max_depth: Some(2),
        classification_axis: Some("domain".into()),
    };

    let nodes = vec![
        node("taxonomy.domain.root", "All Domains", &tax.fqn, None, 0),
        node(
            "taxonomy.domain.cbu",
            "Client Business Unit",
            &tax.fqn,
            Some("taxonomy.domain.root"),
            1,
        ),
        node(
            "taxonomy.domain.entity",
            "Entity",
            &tax.fqn,
            Some("taxonomy.domain.root"),
            2,
        ),
        node(
            "taxonomy.domain.kyc",
            "KYC",
            &tax.fqn,
            Some("taxonomy.domain.root"),
            3,
        ),
        node(
            "taxonomy.domain.trading",
            "Trading",
            &tax.fqn,
            Some("taxonomy.domain.root"),
            4,
        ),
        node(
            "taxonomy.domain.custody",
            "Custody",
            &tax.fqn,
            Some("taxonomy.domain.root"),
            5,
        ),
        node(
            "taxonomy.domain.deal",
            "Deal",
            &tax.fqn,
            Some("taxonomy.domain.root"),
            6,
        ),
        node(
            "taxonomy.domain.billing",
            "Billing",
            &tax.fqn,
            Some("taxonomy.domain.root"),
            7,
        ),
    ];

    (tax, nodes)
}

/// Helper to build a taxonomy node.
fn node(
    fqn: &str,
    name: &str,
    taxonomy_fqn: &str,
    parent_fqn: Option<&str>,
    sort_order: i32,
) -> TaxonomyNodeBody {
    TaxonomyNodeBody {
        fqn: fqn.into(),
        name: name.into(),
        description: None,
        taxonomy_fqn: taxonomy_fqn.into(),
        parent_fqn: parent_fqn.map(Into::into),
        sort_order,
        labels: BTreeMap::new(),
    }
}

/// Seed core taxonomies into the registry.
///
/// Uses the same idempotent publish pattern as the scanner:
/// - Check by FQN, compare hash, publish/update/skip accordingly.
pub async fn seed_taxonomies(
    pool: &PgPool,
    set_id: Uuid,
    dry_run: bool,
    verbose: bool,
) -> Result<TaxonomySeedReport> {
    let mut report = TaxonomySeedReport::default();
    let taxonomies = core_taxonomies();

    if dry_run {
        let tax_count = taxonomies.len();
        let node_count: usize = taxonomies.iter().map(|(_, nodes)| nodes.len()).sum();
        report.taxonomies_published = tax_count;
        report.nodes_published = node_count;
        if verbose {
            for (tax, nodes) in &taxonomies {
                println!("  [DRY] taxonomy: {} ({} nodes)", tax.fqn, nodes.len());
            }
        }
        return Ok(report);
    }

    for (tax, nodes) in &taxonomies {
        // Publish taxonomy definition
        publish_idempotent(
            pool,
            ObjectType::TaxonomyDef,
            &tax.fqn,
            &serde_json::to_value(tax)?,
            set_id,
            verbose,
            "taxonomy",
            &mut report.taxonomies_published,
            &mut report.taxonomies_updated,
            &mut report.taxonomies_skipped,
        )
        .await?;

        // Publish nodes
        for n in nodes {
            publish_idempotent(
                pool,
                ObjectType::TaxonomyNode,
                &n.fqn,
                &serde_json::to_value(n)?,
                set_id,
                verbose,
                "node",
                &mut report.nodes_published,
                &mut report.nodes_updated,
                &mut report.nodes_skipped,
            )
            .await?;
        }
    }

    Ok(report)
}

/// Idempotent publish: check FQN, compare hash, publish/update/skip.
#[allow(clippy::too_many_arguments)]
async fn publish_idempotent(
    pool: &PgPool,
    object_type: ObjectType,
    fqn: &str,
    definition: &serde_json::Value,
    set_id: Uuid,
    verbose: bool,
    label: &str,
    published: &mut usize,
    updated: &mut usize,
    skipped: &mut usize,
) -> Result<()> {
    let existing =
        SnapshotStore::find_active_by_definition_field(pool, object_type, "fqn", fqn).await?;

    let object_id = object_id_for(object_type, fqn);
    let new_hash = definition_hash(definition);

    if let Some(existing_row) = existing {
        let old_hash = definition_hash(&existing_row.definition);
        if old_hash == new_hash {
            *skipped += 1;
            if verbose {
                println!("  SKIP {}: {} (unchanged)", label, fqn);
            }
        } else {
            let mut meta = SnapshotMeta::new_operational(object_type, object_id, "seed");
            meta.predecessor_id = Some(existing_row.snapshot_id);
            meta.version_major = existing_row.version_major;
            meta.version_minor = existing_row.version_minor + 1;
            meta.change_type = ChangeType::NonBreaking;
            meta.change_rationale = Some("Seed definition update".into());
            SnapshotStore::publish_snapshot(pool, &meta, definition, Some(set_id)).await?;
            *updated += 1;
            if verbose {
                println!("  UPD  {}: {} (definition changed)", label, fqn);
            }
        }
    } else {
        let meta = SnapshotMeta::new_operational(object_type, object_id, "seed");
        SnapshotStore::insert_snapshot(pool, &meta, definition, Some(set_id)).await?;
        *published += 1;
        if verbose {
            println!("  NEW  {}: {}", label, fqn);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_taxonomies_well_formed() {
        let taxonomies = core_taxonomies();
        assert_eq!(taxonomies.len(), 5, "Expected 5 core taxonomies");

        for (tax, nodes) in &taxonomies {
            // Each taxonomy has a valid FQN
            assert!(tax.fqn.starts_with("taxonomy."), "Bad FQN: {}", tax.fqn);
            assert!(!tax.name.is_empty());
            assert!(!tax.description.is_empty());

            // Each node references the parent taxonomy
            for n in nodes {
                assert_eq!(
                    n.taxonomy_fqn, tax.fqn,
                    "Node {} has wrong taxonomy_fqn",
                    n.fqn
                );
                assert!(n.fqn.starts_with("taxonomy."), "Bad node FQN: {}", n.fqn);
            }

            // Root node exists (first node, no parent)
            if let Some(root) = nodes.first() {
                assert!(
                    root.parent_fqn.is_none(),
                    "Root node {} should have no parent",
                    root.fqn
                );
            }
        }
    }

    #[test]
    fn test_entity_classification_nodes() {
        let (tax, nodes) = entity_classification_taxonomy();
        assert_eq!(tax.fqn, "taxonomy.entity-classification");
        assert_eq!(nodes.len(), 6);

        // Check hierarchy: fund is under legal-entity
        let fund = nodes.iter().find(|n| n.fqn.ends_with(".fund")).unwrap();
        assert_eq!(
            fund.parent_fqn.as_deref(),
            Some("taxonomy.entity-classification.legal-entity")
        );
    }

    #[test]
    fn test_jurisdiction_nodes() {
        let (tax, nodes) = jurisdiction_taxonomy();
        assert_eq!(tax.fqn, "taxonomy.jurisdiction");
        assert!(nodes.len() >= 9);

        // Luxembourg is under Europe
        let lu = nodes.iter().find(|n| n.fqn.ends_with(".lu")).unwrap();
        assert_eq!(
            lu.parent_fqn.as_deref(),
            Some("taxonomy.jurisdiction.europe")
        );
    }

    #[test]
    fn test_instrument_nodes() {
        let (_, nodes) = instrument_taxonomy();
        // OTC is under derivatives
        let otc = nodes.iter().find(|n| n.fqn.ends_with(".otc")).unwrap();
        assert_eq!(
            otc.parent_fqn.as_deref(),
            Some("taxonomy.instrument-class.derivatives")
        );
    }

    #[test]
    fn test_taxonomy_serde_round_trip() {
        let (tax, nodes) = entity_classification_taxonomy();
        let json = serde_json::to_value(&tax).unwrap();
        let back: TaxonomyDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, tax.fqn);

        for n in &nodes {
            let json = serde_json::to_value(n).unwrap();
            let back: TaxonomyNodeBody = serde_json::from_value(json).unwrap();
            assert_eq!(back.fqn, n.fqn);
        }
    }
}
