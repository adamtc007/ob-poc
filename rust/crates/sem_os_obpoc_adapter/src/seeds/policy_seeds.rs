//! Pure policy rule seed builders for the semantic registry.
//!
//! All functions are **pure** (no DB, no I/O). The DB-publishing orchestrator
//! remains in `ob-poc/src/sem_reg/seeds/policy_seeds.rs`.

use sem_os_core::policy_rule::{PolicyAction, PolicyPredicate, PolicyRuleBody};

/// Core policy rules to bootstrap.
pub fn core_policies() -> Vec<PolicyRuleBody> {
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

pub fn viewer_read_only_policy() -> PolicyRuleBody {
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

pub fn operator_full_access_policy() -> PolicyRuleBody {
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

pub fn pep_enhanced_dd_policy() -> PolicyRuleBody {
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

pub fn proof_evidence_governance_policy() -> PolicyRuleBody {
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

pub fn pii_masking_policy() -> PolicyRuleBody {
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

pub fn sanctions_restricted_policy() -> PolicyRuleBody {
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

pub fn governed_approval_policy() -> PolicyRuleBody {
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

pub fn review_cycle_policy() -> PolicyRuleBody {
    PolicyRuleBody {
        fqn: "governance.review-cycle-compliance".into(),
        name: "Review Cycle Compliance".into(),
        description: "Governed objects must be reviewed within their designated review cycle period. Stale objects are flagged for review.".into(),
        domain: "governance".into(),
        scope: Some("global".into()),
        priority: 20,
        predicates: vec![PolicyPredicate {
            kind: "governance_tier".into(),
            field: "snapshot.governance_tier".into(),
            operator: "eq".into(),
            value: serde_json::json!("governed"),
        }],
        actions: vec![PolicyAction {
            kind: "flag_review".into(),
            params: serde_json::json!({
                "review_cycle_days": 365,
                "warning_days_before": 30,
                "escalation_days_after": 14
            }),
            description: Some("Flag governed objects for review when approaching or exceeding review deadline".into()),
        }],
        enabled: true,
    }
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
            assert!(!p.name.is_empty());
            assert!(!p.description.is_empty());
            assert!(!p.domain.is_empty());
            assert!(p.enabled);
            assert!(!p.actions.is_empty());
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
