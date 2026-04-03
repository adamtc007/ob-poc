//! Bootstrap membership rules for taxonomy overlap in context resolution.
//!
//! These rules activate the KYC view/verb filtering chain by:
//! - assigning the CBU subject entity type into the KYC domain taxonomy
//! - assigning the KYC view into the same taxonomy
//! - assigning the KYC verb slice into the same taxonomy

use anyhow::Result;
use sem_os_core::verb_contract::VerbContractBody;
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_reg::{
    ids::{definition_hash, object_id_for},
    store::SnapshotStore,
    types::{ChangeType, GovernanceTier, ObjectType, SnapshotMeta},
    MembershipKind, MembershipRuleBody,
};

const KYC_TAXONOMY_FQN: &str = "taxonomy.domain";
const KYC_NODE_FQN: &str = "taxonomy.domain.kyc";
const KYC_VIEW_FQN: &str = "view.kyc-case";
const KYC_SUBJECT_ENTITY_FQN: &str = "entity.cbu";
const GOVERNED_VERB_DOMAINS: &[&str] = &["kyc-case", "entity-workstream", "ubo.registry"];

/// Report from membership rule seeding.
#[derive(Debug, Default)]
pub struct MembershipRuleSeedReport {
    pub membership_rules_published: usize,
    pub membership_rules_skipped: usize,
    pub membership_rules_updated: usize,
}

impl std::fmt::Display for MembershipRuleSeedReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Membership rules: {} published, {} updated, {} skipped",
            self.membership_rules_published,
            self.membership_rules_updated,
            self.membership_rules_skipped,
        )
    }
}

pub async fn seed_kyc_membership_rules(
    pool: &PgPool,
    set_id: Uuid,
    dry_run: bool,
    verbose: bool,
    contracts: &[VerbContractBody],
) -> Result<MembershipRuleSeedReport> {
    let mut report = MembershipRuleSeedReport::default();
    let rules = core_membership_rules(contracts);

    if dry_run {
        report.membership_rules_published = rules.len();
        if verbose {
            for rule in &rules {
                println!("  [DRY] membership: {}", rule.fqn);
            }
        }
        return Ok(report);
    }

    for rule in &rules {
        publish_idempotent(
            pool,
            rule,
            set_id,
            verbose,
            &mut report.membership_rules_published,
            &mut report.membership_rules_updated,
            &mut report.membership_rules_skipped,
        )
        .await?;
    }

    Ok(report)
}

fn core_membership_rules(contracts: &[VerbContractBody]) -> Vec<MembershipRuleBody> {
    let mut rules = vec![
        direct_membership(
            "membership.entity.cbu.in-kyc-domain",
            "CBU Subject In KYC Domain",
            "Allows CBU subjects to overlap with KYC views and verbs",
            "entity_type_def",
            KYC_SUBJECT_ENTITY_FQN,
        ),
        direct_membership(
            "membership.view.kyc-case.in-kyc-domain",
            "KYC Case View In KYC Domain",
            "Links the KYC case view into the KYC domain taxonomy",
            "view_def",
            KYC_VIEW_FQN,
        ),
    ];

    for contract in contracts {
        if is_governed_kyc_domain(&contract.domain) {
            let fqn = format!("membership.{}.in-kyc-domain", contract.fqn);
            rules.push(direct_membership(
                &fqn,
                &format!("{} In KYC Domain", contract.fqn),
                "Links KYC verb contracts into the KYC domain taxonomy",
                "verb_contract",
                &contract.fqn,
            ));
        }
    }

    rules
}

fn direct_membership(
    fqn: &str,
    name: &str,
    description: &str,
    target_type: &str,
    target_fqn: &str,
) -> MembershipRuleBody {
    MembershipRuleBody {
        fqn: fqn.to_string(),
        name: name.to_string(),
        description: Some(description.to_string()),
        taxonomy_fqn: KYC_TAXONOMY_FQN.to_string(),
        node_fqn: KYC_NODE_FQN.to_string(),
        membership_kind: MembershipKind::Direct,
        target_type: target_type.to_string(),
        target_fqn: target_fqn.to_string(),
        conditions: vec![],
    }
}

fn is_governed_kyc_domain(domain: &str) -> bool {
    GOVERNED_VERB_DOMAINS.contains(&domain)
}

async fn publish_idempotent(
    pool: &PgPool,
    rule: &MembershipRuleBody,
    set_id: Uuid,
    verbose: bool,
    published: &mut usize,
    updated: &mut usize,
    skipped: &mut usize,
) -> Result<()> {
    let existing = SnapshotStore::find_active_by_definition_field(
        pool,
        ObjectType::MembershipRule,
        "fqn",
        &rule.fqn,
    )
    .await?;

    let object_id = object_id_for(ObjectType::MembershipRule, &rule.fqn);
    let definition = serde_json::to_value(rule)?;
    let new_hash = definition_hash(&definition);

    if let Some(existing_row) = existing {
        let old_hash = definition_hash(&existing_row.definition);
        if old_hash == new_hash {
            *skipped += 1;
            if verbose {
                println!("  SKIP membership: {} (unchanged)", rule.fqn);
            }
        } else {
            let mut meta = SnapshotMeta::new_at_tier(
                GovernanceTier::Governed,
                ObjectType::MembershipRule,
                object_id,
                "seed",
            );
            meta.predecessor_id = Some(existing_row.snapshot_id);
            meta.version_major = existing_row.version_major;
            meta.version_minor = existing_row.version_minor + 1;
            meta.change_type = ChangeType::NonBreaking;
            meta.change_rationale = Some("Seed definition update".into());
            SnapshotStore::publish_snapshot(pool, &meta, &definition, Some(set_id)).await?;
            *updated += 1;
            if verbose {
                println!("  UPD  membership: {} (definition changed)", rule.fqn);
            }
        }
    } else {
        let meta = SnapshotMeta::new_at_tier(
            GovernanceTier::Governed,
            ObjectType::MembershipRule,
            object_id,
            "seed",
        );
        SnapshotStore::insert_snapshot(pool, &meta, &definition, Some(set_id)).await?;
        *published += 1;
        if verbose {
            println!("  NEW  membership: {}", rule.fqn);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_membership_rules_include_subject_and_view() {
        let rules = core_membership_rules(&[]);
        assert!(rules
            .iter()
            .any(|rule| rule.target_fqn == KYC_SUBJECT_ENTITY_FQN));
        assert!(rules.iter().any(|rule| rule.target_fqn == KYC_VIEW_FQN));
    }

    #[test]
    fn test_core_membership_rules_include_governed_verb_domains() {
        let rules = core_membership_rules(&[
            VerbContractBody {
                fqn: "kyc-case.create".into(),
                domain: "kyc-case".into(),
                action: "create".into(),
                description: "Create".into(),
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
                produces_shared_facts: vec![],
            },
            VerbContractBody {
                fqn: "cbu.create".into(),
                domain: "cbu".into(),
                action: "create".into(),
                description: "Create".into(),
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
                produces_shared_facts: vec![],
            },
        ]);

        assert!(rules
            .iter()
            .any(|rule| rule.target_fqn == "kyc-case.create"));
        assert!(!rules.iter().any(|rule| rule.target_fqn == "cbu.create"));
    }
}
