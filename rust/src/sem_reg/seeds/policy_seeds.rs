//! Bootstrap policy rule definitions for the semantic registry.
//!
//! Seeds core policy rules derived from existing ABAC patterns and governance
//! requirements:
//! - Read-only viewer access
//! - Full operator access
//! - PEP enhanced due diligence
//! - Proof-class evidence governance
//! - PII masking enforcement
//! - Sanctions restricted access
//! - Governed-tier approval requirement
//! - Review cycle compliance

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_reg::{
    ids::{definition_hash, object_id_for},
    policy_rule::{PolicyAction, PolicyPredicate, PolicyRuleBody},
    store::SnapshotStore,
    types::{ChangeType, ObjectType, SnapshotMeta},
};

/// Report from policy seeding.
#[derive(Debug, Default)]
pub struct PolicySeedReport {
    pub policies_published: usize,
    pub policies_skipped: usize,
    pub policies_updated: usize,
}

impl std::fmt::Display for PolicySeedReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Policies: {} published, {} updated, {} skipped",
            self.policies_published, self.policies_updated, self.policies_skipped,
        )
    }
}

/// Core policy rules to bootstrap.
fn core_policies() -> Vec<PolicyRuleBody> {
    vec![
        viewer_read_only_policy(),
        operator_full_access_policy(),
        pep_enhanced_dd_policy(),
        proof_evidence_governance_policy(),
        pii_masking_policy(),
        sanctions_restricted_policy(),
        governed_approval_policy(),
        review_cycle_policy(),
    ]
}

/// Viewers can only read — no publish, no modify.
fn viewer_read_only_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "access.viewer-read-only".into(),
        name: "Viewer Read-Only".into(),
        description: "Viewers have read-only access to the registry. Publish and modify operations are blocked.".into(),
        domain: "access".into(),
        scope: Some("global".into()),
        priority: 10,
        predicates: vec![PolicyPredicate {
            kind: "actor_role".into(),
            field: "actor.role".into(),
            operator: "eq".into(),
            value: serde_json::json!("viewer"),
        }],
        actions: vec![PolicyAction {
            kind: "restrict_access".into(),
            params: serde_json::json!({
                "allowed_operations": ["read", "search", "resolve_context"],
                "denied_operations": ["publish", "modify", "delete"]
            }),
            description: Some("Restrict viewers to read-only operations".into()),
        }],
        enabled: true,
    }
}

/// Operators have full access to the registry.
fn operator_full_access_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "access.operator-full-access".into(),
        name: "Operator Full Access".into(),
        description: "Operators have full access to all registry operations.".into(),
        domain: "access".into(),
        scope: Some("global".into()),
        priority: 20,
        predicates: vec![PolicyPredicate {
            kind: "actor_role".into(),
            field: "actor.role".into(),
            operator: "in".into(),
            value: serde_json::json!(["operator", "admin"]),
        }],
        actions: vec![PolicyAction {
            kind: "allow_access".into(),
            params: serde_json::json!({
                "allowed_operations": ["read", "search", "resolve_context", "publish", "modify"]
            }),
            description: Some("Grant operators full registry access".into()),
        }],
        enabled: true,
    }
}

/// PEP entities require enhanced due diligence.
fn pep_enhanced_dd_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "kyc.pep-enhanced-dd".into(),
        name: "PEP Enhanced Due Diligence".into(),
        description:
            "Politically exposed persons require enhanced due diligence checks and evidence.".into(),
        domain: "kyc".into(),
        scope: Some("entity_type".into()),
        priority: 10,
        predicates: vec![PolicyPredicate {
            kind: "attribute_value".into(),
            field: "entity.pep_status".into(),
            operator: "eq".into(),
            value: serde_json::json!("active"),
        }],
        actions: vec![
            PolicyAction {
                kind: "require_evidence".into(),
                params: serde_json::json!({
                    "evidence_type": "enhanced_dd_pack",
                    "freshness_days": 90
                }),
                description: Some("Require enhanced due diligence evidence pack".into()),
            },
            PolicyAction {
                kind: "flag_review".into(),
                params: serde_json::json!({
                    "review_type": "compliance_officer",
                    "priority": "high"
                }),
                description: Some("Flag for compliance officer review".into()),
            },
        ],
        enabled: true,
    }
}

/// Proof-class attributes must have governed-tier evidence.
fn proof_evidence_governance_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "governance.proof-evidence-required".into(),
        name: "Proof Evidence Governance".into(),
        description:
            "Attributes with Proof trust class require governed-tier evidence with valid freshness."
                .into(),
        domain: "governance".into(),
        scope: Some("global".into()),
        priority: 5,
        predicates: vec![PolicyPredicate {
            kind: "trust_class".into(),
            field: "snapshot.trust_class".into(),
            operator: "eq".into(),
            value: serde_json::json!("proof"),
        }],
        actions: vec![PolicyAction {
            kind: "require_evidence".into(),
            params: serde_json::json!({
                "governance_tier": "governed",
                "min_evidence_grade": "proof"
            }),
            description: Some(
                "Proof-class snapshots must have governed-tier evidence backing them".into(),
            ),
        }],
        enabled: true,
    }
}

/// PII attributes must be masked by default.
fn pii_masking_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "security.pii-masking".into(),
        name: "PII Masking Enforcement".into(),
        description: "Attributes marked as PII must be masked in non-operational contexts.".into(),
        domain: "security".into(),
        scope: Some("global".into()),
        priority: 15,
        predicates: vec![PolicyPredicate {
            kind: "security_label".into(),
            field: "snapshot.security_label.pii".into(),
            operator: "eq".into(),
            value: serde_json::json!(true),
        }],
        actions: vec![PolicyAction {
            kind: "restrict_access".into(),
            params: serde_json::json!({
                "handling_control": "mask_by_default",
                "exempt_purposes": ["operations", "audit", "compliance"]
            }),
            description: Some("Mask PII data unless accessor has an exempt purpose".into()),
        }],
        enabled: true,
    }
}

/// Sanctions-related data has restricted access.
fn sanctions_restricted_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "security.sanctions-restricted".into(),
        name: "Sanctions Restricted Access".into(),
        description: "Sanctions screening data is restricted to authorized personnel and purposes."
            .into(),
        domain: "security".into(),
        scope: Some("domain".into()),
        priority: 5,
        predicates: vec![PolicyPredicate {
            kind: "attribute_value".into(),
            field: "snapshot.domain".into(),
            operator: "in".into(),
            value: serde_json::json!(["sanctions", "screening"]),
        }],
        actions: vec![
            PolicyAction {
                kind: "restrict_access".into(),
                params: serde_json::json!({
                    "allowed_purposes": ["sanctions_screening", "compliance", "audit"],
                    "denied_purposes": ["analytics", "reporting"],
                    "no_llm_external": true
                }),
                description: Some("Restrict sanctions data to authorized purposes only".into()),
            },
            PolicyAction {
                kind: "require_approval".into(),
                params: serde_json::json!({
                    "approver_role": "compliance_officer"
                }),
                description: Some(
                    "Require compliance officer approval for sanctions data access".into(),
                ),
            },
        ],
        enabled: true,
    }
}

/// Governed-tier objects require approval before activation.
fn governed_approval_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "governance.governed-approval-required".into(),
        name: "Governed Approval Required".into(),
        description:
            "Objects at the Governed tier must go through approval workflow before activation."
                .into(),
        domain: "governance".into(),
        scope: Some("global".into()),
        priority: 10,
        predicates: vec![PolicyPredicate {
            kind: "governance_tier".into(),
            field: "snapshot.governance_tier".into(),
            operator: "eq".into(),
            value: serde_json::json!("governed"),
        }],
        actions: vec![PolicyAction {
            kind: "require_approval".into(),
            params: serde_json::json!({
                "approver_roles": ["governance_lead", "domain_steward"],
                "auto_approve_operational": true
            }),
            description: Some(
                "Governed-tier snapshots need approval from governance lead or domain steward"
                    .into(),
            ),
        }],
        enabled: true,
    }
}

/// Governed objects must comply with review cycles.
fn review_cycle_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "governance.review-cycle-compliance".into(),
        name: "Review Cycle Compliance".into(),
        description: "Governed objects must be reviewed within their designated review cycle period. Stale objects are flagged for review.".into(),
        domain: "governance".into(),
        scope: Some("global".into()),
        priority: 20,
        predicates: vec![
            PolicyPredicate {
                kind: "governance_tier".into(),
                field: "snapshot.governance_tier".into(),
                operator: "eq".into(),
                value: serde_json::json!("governed"),
            },
        ],
        actions: vec![PolicyAction {
            kind: "flag_review".into(),
            params: serde_json::json!({
                "review_cycle_days": 365,
                "warning_days_before": 30,
                "escalation_days_after": 14
            }),
            description: Some(
                "Flag governed objects for review when approaching or exceeding review deadline"
                    .into(),
            ),
        }],
        enabled: true,
    }
}

/// Seed core policy rules into the registry.
///
/// Uses the same idempotent publish pattern as other seeds.
pub async fn seed_policies(
    pool: &PgPool,
    set_id: Uuid,
    dry_run: bool,
    verbose: bool,
) -> Result<PolicySeedReport> {
    let mut report = PolicySeedReport::default();
    let policies = core_policies();

    if dry_run {
        report.policies_published = policies.len();
        if verbose {
            for p in &policies {
                println!("  [DRY] policy: {}", p.fqn);
            }
        }
        return Ok(report);
    }

    for policy in &policies {
        publish_idempotent(
            pool,
            &policy.fqn,
            &serde_json::to_value(policy)?,
            set_id,
            verbose,
            &mut report.policies_published,
            &mut report.policies_updated,
            &mut report.policies_skipped,
        )
        .await?;
    }

    Ok(report)
}

/// Idempotent publish for policy rules.
#[allow(clippy::too_many_arguments)]
async fn publish_idempotent(
    pool: &PgPool,
    fqn: &str,
    definition: &serde_json::Value,
    set_id: Uuid,
    verbose: bool,
    published: &mut usize,
    updated: &mut usize,
    skipped: &mut usize,
) -> Result<()> {
    let existing =
        SnapshotStore::find_active_by_definition_field(pool, ObjectType::PolicyRule, "fqn", fqn)
            .await?;

    let object_id = object_id_for(ObjectType::PolicyRule, fqn);
    let new_hash = definition_hash(definition);

    if let Some(existing_row) = existing {
        let old_hash = definition_hash(&existing_row.definition);
        if old_hash == new_hash {
            *skipped += 1;
            if verbose {
                println!("  SKIP policy: {} (unchanged)", fqn);
            }
        } else {
            let mut meta = SnapshotMeta::new_operational(ObjectType::PolicyRule, object_id, "seed");
            meta.predecessor_id = Some(existing_row.snapshot_id);
            meta.version_major = existing_row.version_major;
            meta.version_minor = existing_row.version_minor + 1;
            meta.change_type = ChangeType::NonBreaking;
            meta.change_rationale = Some("Seed definition update".into());
            SnapshotStore::publish_snapshot(pool, &meta, definition, Some(set_id)).await?;
            *updated += 1;
            if verbose {
                println!("  UPD  policy: {} (definition changed)", fqn);
            }
        }
    } else {
        let meta = SnapshotMeta::new_operational(ObjectType::PolicyRule, object_id, "seed");
        SnapshotStore::insert_snapshot(pool, &meta, definition, Some(set_id)).await?;
        *published += 1;
        if verbose {
            println!("  NEW  policy: {}", fqn);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_policies_well_formed() {
        let policies = core_policies();
        assert_eq!(policies.len(), 8, "Expected 8 core policies");

        for p in &policies {
            assert!(
                p.fqn.contains('.'),
                "Policy FQN should be dot-separated: {}",
                p.fqn
            );
            assert!(!p.name.is_empty(), "Policy {} has no name", p.fqn);
            assert!(
                !p.description.is_empty(),
                "Policy {} has no description",
                p.fqn
            );
            assert!(!p.domain.is_empty(), "Policy {} has no domain", p.fqn);
            assert!(p.enabled, "Policy {} should be enabled", p.fqn);
            assert!(!p.actions.is_empty(), "Policy {} has no actions", p.fqn);
        }
    }

    #[test]
    fn test_policy_fqns_unique() {
        let policies = core_policies();
        let mut fqns: Vec<&str> = policies.iter().map(|p| p.fqn.as_str()).collect();
        let original_len = fqns.len();
        fqns.sort();
        fqns.dedup();
        assert_eq!(fqns.len(), original_len, "Duplicate policy FQNs found");
    }

    #[test]
    fn test_viewer_read_only() {
        let policy = viewer_read_only_policy();
        assert_eq!(policy.fqn, "access.viewer-read-only");
        assert_eq!(policy.priority, 10);
        assert_eq!(policy.predicates.len(), 1);
        assert_eq!(policy.predicates[0].kind, "actor_role");
    }

    #[test]
    fn test_pep_enhanced_dd() {
        let policy = pep_enhanced_dd_policy();
        assert_eq!(policy.fqn, "kyc.pep-enhanced-dd");
        assert_eq!(policy.actions.len(), 2);
        assert_eq!(policy.actions[0].kind, "require_evidence");
        assert_eq!(policy.actions[1].kind, "flag_review");
    }

    #[test]
    fn test_proof_evidence_governance() {
        let policy = proof_evidence_governance_policy();
        assert_eq!(policy.predicates[0].kind, "trust_class");
        assert_eq!(policy.predicates[0].value, serde_json::json!("proof"));
    }

    #[test]
    fn test_policy_serde_round_trip() {
        for p in &core_policies() {
            let json = serde_json::to_value(p).unwrap();
            let back: PolicyRuleBody = serde_json::from_value(json).unwrap();
            assert_eq!(back.fqn, p.fqn);
            assert_eq!(back.predicates.len(), p.predicates.len());
            assert_eq!(back.actions.len(), p.actions.len());
        }
    }
}
