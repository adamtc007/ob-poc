//! Governance gates — **report-only by default**, promote to enforce later.
//!
//! These gates enforce governance-tier policies:
//! 1. Taxonomy membership — governed objects must belong to ≥1 taxonomy
//! 2. Stewardship — governed objects must have a steward (changed_by)
//! 3. Evidence grade policy — governed derivations with AllowedWithConstraints
//!    need passing tests + policy link

use super::derivation_spec::{DerivationSpecBody, EvidenceGrade};
use super::gates::{GateFailure, GateSeverity};
use super::types::{GovernanceTier, SnapshotRow};

// ── Gate 1: Taxonomy membership ──────────────────────────────

/// Check that governed objects belong to at least one taxonomy node.
///
/// - Governed tier with zero memberships → Error
/// - Operational tier with zero memberships → Warning (informational)
pub fn check_taxonomy_membership(
    object_fqn: &str,
    tier: GovernanceTier,
    memberships: &[String],
) -> Vec<GateFailure> {
    if memberships.is_empty() {
        let (severity, message) = match tier {
            GovernanceTier::Governed => (
                GateSeverity::Error,
                format!(
                    "Governed object '{}' has no taxonomy membership — \
                     assign it to at least one taxonomy node",
                    object_fqn
                ),
            ),
            GovernanceTier::Operational => (
                GateSeverity::Warning,
                format!(
                    "Operational object '{}' has no taxonomy membership — \
                     consider classifying it",
                    object_fqn
                ),
            ),
        };
        vec![GateFailure {
            gate_name: "taxonomy_membership".into(),
            severity,
            object_type: "snapshot".into(),
            object_fqn: Some(object_fqn.into()),
            snapshot_id: None,
            message,
            remediation_hint: Some(
                "Add a MembershipRule linking this object to a taxonomy node".into(),
            ),
        }]
    } else {
        vec![]
    }
}

// ── Gate 2: Stewardship ──────────────────────────────────────

/// Check that governed objects have a steward (non-empty `created_by`).
///
/// - Governed tier with empty/placeholder steward → Error
/// - Operational tier → pass (no steward requirement)
pub fn check_stewardship(snapshot: &SnapshotRow, tier: GovernanceTier) -> Vec<GateFailure> {
    match tier {
        GovernanceTier::Operational => vec![],
        GovernanceTier::Governed => {
            let steward = snapshot.created_by.trim();
            if steward.is_empty() || steward == "system" || steward == "unknown" {
                vec![GateFailure::error(
                    "stewardship",
                    snapshot.object_type.to_string(),
                    format!(
                        "Governed snapshot {} has no identifiable steward (created_by = '{}')",
                        snapshot.snapshot_id, snapshot.created_by,
                    ),
                )
                .with_snapshot_id(snapshot.snapshot_id)
                .with_hint(
                    "Set created_by to the steward's identity (e.g., email or service account)",
                )]
            } else {
                vec![]
            }
        }
    }
}

// ── Gate 3: Evidence grade policy ────────────────────────────

/// Check that governed derivations with `AllowedWithConstraints` evidence grade
/// have passing tests and a policy link.
///
/// - `EvidenceGrade::Prohibited` → always passes (no evidence concerns)
/// - `EvidenceGrade::AllowedWithConstraints` on governed tier → needs tests + policy
/// - Operational tier → pass (no evidence policy enforcement)
pub fn check_evidence_grade_policy(
    derivation: &DerivationSpecBody,
    tier: GovernanceTier,
    has_passing_tests: bool,
    has_policy_link: bool,
) -> Vec<GateFailure> {
    match tier {
        GovernanceTier::Operational => vec![],
        GovernanceTier::Governed => match derivation.evidence_grade {
            EvidenceGrade::Prohibited => vec![],
            EvidenceGrade::AllowedWithConstraints => {
                let mut failures = Vec::new();

                if !has_passing_tests {
                    failures.push(
                        GateFailure::error(
                            "evidence_grade_policy",
                            "derivation_spec",
                            format!(
                                "Governed derivation '{}' with AllowedWithConstraints \
                                 evidence grade must have passing test cases",
                                derivation.fqn
                            ),
                        )
                        .with_fqn(&derivation.fqn)
                        .with_hint(
                            "Add test cases to the derivation spec's `tests` field \
                             and ensure they pass",
                        ),
                    );
                }

                if !has_policy_link {
                    failures.push(
                        GateFailure::error(
                            "evidence_grade_policy",
                            "derivation_spec",
                            format!(
                                "Governed derivation '{}' with AllowedWithConstraints \
                                 evidence grade must reference a policy rule",
                                derivation.fqn
                            ),
                        )
                        .with_fqn(&derivation.fqn)
                        .with_hint(
                            "Link this derivation to a PolicyRule that authorizes \
                             evidence-grade use",
                        ),
                    );
                }

                failures
            }
        },
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::derivation_spec::*;
    use crate::sem_reg::types::*;
    use uuid::Uuid;

    fn mock_snapshot(tier: GovernanceTier, created_by: &str) -> SnapshotRow {
        SnapshotRow {
            snapshot_id: Uuid::new_v4(),
            snapshot_set_id: None,
            object_type: ObjectType::AttributeDef,
            object_id: Uuid::new_v4(),
            version_major: 1,
            version_minor: 0,
            status: SnapshotStatus::Active,
            governance_tier: tier,
            trust_class: TrustClass::Convenience,
            security_label: serde_json::json!({"classification": "internal"}),
            effective_from: chrono::Utc::now(),
            effective_until: None,
            predecessor_id: None,
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: created_by.into(),
            approved_by: None,
            definition: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        }
    }

    fn sample_derivation(evidence_grade: EvidenceGrade) -> DerivationSpecBody {
        DerivationSpecBody {
            fqn: "kyc.risk_score_derived".into(),
            name: "Risk Score".into(),
            description: "Derived risk score".into(),
            output_attribute_fqn: "kyc.risk_score".into(),
            inputs: vec![DerivationInput {
                attribute_fqn: "kyc.raw_score".into(),
                role: "primary".into(),
                required: true,
            }],
            expression: DerivationExpression::FunctionRef {
                ref_name: "compute_risk_score".into(),
            },
            null_semantics: NullSemantics::default(),
            freshness_rule: None,
            security_inheritance: SecurityInheritanceMode::default(),
            evidence_grade,
            tests: vec![],
        }
    }

    // ── Taxonomy membership ───────────────────────────────────

    #[test]
    fn test_taxonomy_governed_no_membership_error() {
        let failures = check_taxonomy_membership("kyc.risk_score", GovernanceTier::Governed, &[]);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].severity, GateSeverity::Error);
        assert!(failures[0].message.contains("no taxonomy membership"));
    }

    #[test]
    fn test_taxonomy_operational_no_membership_warning() {
        let failures =
            check_taxonomy_membership("cbu.temp_field", GovernanceTier::Operational, &[]);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].severity, GateSeverity::Warning);
    }

    #[test]
    fn test_taxonomy_with_membership_passes() {
        let failures = check_taxonomy_membership(
            "kyc.risk_score",
            GovernanceTier::Governed,
            &["kyc.risk_taxonomy.scores".into()],
        );
        assert!(failures.is_empty());
    }

    // ── Stewardship ───────────────────────────────────────────

    #[test]
    fn test_stewardship_governed_with_steward_passes() {
        let snapshot = mock_snapshot(GovernanceTier::Governed, "alice@example.com");
        let failures = check_stewardship(&snapshot, GovernanceTier::Governed);
        assert!(failures.is_empty());
    }

    #[test]
    fn test_stewardship_governed_empty_fails() {
        let snapshot = mock_snapshot(GovernanceTier::Governed, "");
        let failures = check_stewardship(&snapshot, GovernanceTier::Governed);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].message.contains("no identifiable steward"));
    }

    #[test]
    fn test_stewardship_governed_system_fails() {
        let snapshot = mock_snapshot(GovernanceTier::Governed, "system");
        let failures = check_stewardship(&snapshot, GovernanceTier::Governed);
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn test_stewardship_operational_skips() {
        let snapshot = mock_snapshot(GovernanceTier::Operational, "");
        let failures = check_stewardship(&snapshot, GovernanceTier::Operational);
        assert!(failures.is_empty());
    }

    // ── Evidence grade policy ─────────────────────────────────

    #[test]
    fn test_evidence_prohibited_always_passes() {
        let spec = sample_derivation(EvidenceGrade::Prohibited);
        let failures = check_evidence_grade_policy(&spec, GovernanceTier::Governed, false, false);
        assert!(failures.is_empty());
    }

    #[test]
    fn test_evidence_allowed_governed_missing_both() {
        let spec = sample_derivation(EvidenceGrade::AllowedWithConstraints);
        let failures = check_evidence_grade_policy(&spec, GovernanceTier::Governed, false, false);
        assert_eq!(failures.len(), 2);
        let has_test_failure = failures.iter().any(|f| f.message.contains("passing test"));
        let has_policy_failure = failures.iter().any(|f| f.message.contains("policy rule"));
        assert!(has_test_failure);
        assert!(has_policy_failure);
    }

    #[test]
    fn test_evidence_allowed_governed_with_both() {
        let spec = sample_derivation(EvidenceGrade::AllowedWithConstraints);
        let failures = check_evidence_grade_policy(&spec, GovernanceTier::Governed, true, true);
        assert!(failures.is_empty());
    }

    #[test]
    fn test_evidence_allowed_operational_skips() {
        let spec = sample_derivation(EvidenceGrade::AllowedWithConstraints);
        let failures =
            check_evidence_grade_policy(&spec, GovernanceTier::Operational, false, false);
        assert!(failures.is_empty());
    }
}
