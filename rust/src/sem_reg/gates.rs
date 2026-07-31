//! Publish gate pure functions for the Semantic Registry.
//!
//! Gates are evaluated before any snapshot is persisted. They enforce:
//! - **Proof Rule**: Operational tier cannot have TrustClass::Proof
//! - **Security label validation**: Classification level must be valid
//! - **Governed approval**: Governed-tier snapshots require an approver
//! - **Version monotonicity**: New versions must be >= predecessor

use super::types::{
    Classification, GovernanceTier, SecurityLabel, SnapshotMeta, SnapshotRow, TrustClass,
};

/// Result of a single gate check.
#[derive(Debug, Clone)]
pub struct GateResult {
    pub gate_name: &'static str,
    pub passed: bool,
    pub reason: Option<String>,
}

impl GateResult {
    fn pass(gate_name: &'static str) -> Self {
        Self {
            gate_name,
            passed: true,
            reason: None,
        }
    }

    fn fail(gate_name: &'static str, reason: impl Into<String>) -> Self {
        Self {
            gate_name,
            passed: false,
            reason: Some(reason.into()),
        }
    }
}

/// Aggregated result of all publish gates.
#[derive(Debug, Clone)]
pub struct PublishGateResult {
    pub results: Vec<GateResult>,
}

impl PublishGateResult {
    pub(crate) fn all_passed(&self) -> bool {
        self.results.iter().all(|r| r.passed)
    }

    pub(crate) fn failures(&self) -> Vec<&GateResult> {
        self.results.iter().filter(|r| !r.passed).collect()
    }

    pub(crate) fn failure_messages(&self) -> Vec<String> {
        self.failures()
            .iter()
            .filter_map(|r| {
                r.reason
                    .as_ref()
                    .map(|msg| format!("[{}] {}", r.gate_name, msg))
            })
            .collect()
    }
}

// ── Individual gate checks ────────────────────────────────────

/// **Proof Rule**: Only governed-tier objects may have TrustClass::Proof.
/// This mirrors the DB CHECK constraint: `trust_class != 'proof' OR governance_tier = 'governed'`
pub(crate) fn check_proof_rule(tier: GovernanceTier, trust: TrustClass) -> GateResult {
    if trust == TrustClass::Proof && tier == GovernanceTier::Operational {
        GateResult::fail(
            "proof_rule",
            "Operational-tier objects cannot have TrustClass::Proof — \
             promote to Governed tier first",
        )
    } else {
        GateResult::pass("proof_rule")
    }
}

/// **Security label validation**: Ensure the classification is populated.
pub(crate) fn check_security_label(label: &SecurityLabel) -> GateResult {
    // PII data must have at least Confidential classification
    if label.pii
        && matches!(
            label.classification,
            Classification::Public | Classification::Internal
        )
    {
        return GateResult::fail(
            "security_label",
            "PII-flagged objects must have classification >= Confidential",
        );
    }

    GateResult::pass("security_label")
}

/// **Governed approval gate**: Governed-tier snapshots must have an approver.
pub(crate) fn check_governed_approval(meta: &SnapshotMeta) -> GateResult {
    if meta.governance_tier == GovernanceTier::Governed && meta.approved_by.is_none() {
        GateResult::fail(
            "governed_approval",
            "Governed-tier snapshots require an approver",
        )
    } else {
        GateResult::pass("governed_approval")
    }
}

/// **Version monotonicity**: New version must be >= predecessor version.
pub(crate) fn check_version_monotonicity(
    meta: &SnapshotMeta,
    predecessor: Option<&SnapshotRow>,
) -> GateResult {
    if let Some(pred) = predecessor {
        let new_version = (meta.version_major, meta.version_minor);
        let old_version = (pred.version_major, pred.version_minor);
        if new_version < old_version {
            return GateResult::fail(
                "version_monotonicity",
                format!(
                    "New version {}.{} is less than predecessor {}.{}",
                    meta.version_major, meta.version_minor, pred.version_major, pred.version_minor,
                ),
            );
        }
    }
    GateResult::pass("version_monotonicity")
}

// ── Aggregate evaluator ───────────────────────────────────────

/// Evaluate all publish gates for a snapshot.
///
/// Returns a `PublishGateResult` containing the outcome of every gate.
/// The caller should check `all_passed()` before persisting.
// `pub` (not `pub(crate)`): called across the crate boundary by `xtask`
// (`xtask/src/sem_reg.rs` imports `ob_poc::sem_reg::gates::evaluate_publish_gates`).
pub fn evaluate_publish_gates(
    meta: &SnapshotMeta,
    predecessor: Option<&SnapshotRow>,
) -> PublishGateResult {
    let results = vec![
        check_proof_rule(meta.governance_tier, meta.trust_class),
        check_security_label(&meta.security_label),
        check_governed_approval(meta),
        check_version_monotonicity(meta, predecessor),
    ];
    PublishGateResult { results }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::types::*;
    use uuid::Uuid;

    #[test]
    fn test_proof_rule_operational_proof_fails() {
        let result = check_proof_rule(GovernanceTier::Operational, TrustClass::Proof);
        assert!(!result.passed);
        assert!(result.reason.unwrap().contains("Operational"));
    }

    #[test]
    fn test_proof_rule_governed_proof_passes() {
        let result = check_proof_rule(GovernanceTier::Governed, TrustClass::Proof);
        assert!(result.passed);
    }

    #[test]
    fn test_proof_rule_operational_convenience_passes() {
        let result = check_proof_rule(GovernanceTier::Operational, TrustClass::Convenience);
        assert!(result.passed);
    }

    #[test]
    fn test_proof_rule_operational_decision_support_passes() {
        let result = check_proof_rule(GovernanceTier::Operational, TrustClass::DecisionSupport);
        assert!(result.passed);
    }

    #[test]
    fn test_security_label_pii_requires_confidential() {
        let label = SecurityLabel {
            pii: true,
            classification: Classification::Internal,
            ..SecurityLabel::default()
        };
        let result = check_security_label(&label);
        assert!(!result.passed);
        assert!(result.reason.unwrap().contains("Confidential"));
    }

    #[test]
    fn test_security_label_pii_confidential_passes() {
        let label = SecurityLabel {
            pii: true,
            classification: Classification::Confidential,
            ..SecurityLabel::default()
        };
        let result = check_security_label(&label);
        assert!(result.passed);
    }

    #[test]
    fn test_security_label_no_pii_public_passes() {
        let label = SecurityLabel {
            pii: false,
            classification: Classification::Public,
            ..SecurityLabel::default()
        };
        let result = check_security_label(&label);
        assert!(result.passed);
    }

    #[test]
    fn test_governed_approval_required() {
        let meta = SnapshotMeta {
            object_type: ObjectType::PolicyRule,
            object_id: Uuid::new_v4(),
            version_major: 1,
            version_minor: 0,
            status: SnapshotStatus::Active,
            governance_tier: GovernanceTier::Governed,
            trust_class: TrustClass::Proof,
            security_label: SecurityLabel::default(),
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: "analyst".into(),
            approved_by: None, // missing!
            predecessor_id: None,
        };
        let result = check_governed_approval(&meta);
        assert!(!result.passed);
    }

    #[test]
    fn test_governed_approval_present_passes() {
        let meta = SnapshotMeta {
            object_type: ObjectType::PolicyRule,
            object_id: Uuid::new_v4(),
            version_major: 1,
            version_minor: 0,
            status: SnapshotStatus::Active,
            governance_tier: GovernanceTier::Governed,
            trust_class: TrustClass::Proof,
            security_label: SecurityLabel::default(),
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: "analyst".into(),
            approved_by: Some("supervisor".into()),
            predecessor_id: None,
        };
        let result = check_governed_approval(&meta);
        assert!(result.passed);
    }

    #[test]
    fn test_operational_no_approval_needed() {
        let meta =
            SnapshotMeta::new_operational(ObjectType::VerbContract, Uuid::new_v4(), "scanner");
        let result = check_governed_approval(&meta);
        assert!(result.passed);
    }

    #[test]
    fn test_version_monotonicity_pass() {
        let meta = SnapshotMeta {
            version_major: 2,
            version_minor: 0,
            ..SnapshotMeta::new_operational(ObjectType::AttributeDef, Uuid::new_v4(), "test")
        };
        let pred = mock_predecessor(1, 3);
        let result = check_version_monotonicity(&meta, Some(&pred));
        assert!(result.passed);
    }

    #[test]
    fn test_version_monotonicity_fail() {
        let meta = SnapshotMeta {
            version_major: 1,
            version_minor: 0,
            ..SnapshotMeta::new_operational(ObjectType::AttributeDef, Uuid::new_v4(), "test")
        };
        let pred = mock_predecessor(2, 0);
        let result = check_version_monotonicity(&meta, Some(&pred));
        assert!(!result.passed);
    }

    #[test]
    fn test_version_monotonicity_no_predecessor() {
        let meta = SnapshotMeta::new_operational(ObjectType::AttributeDef, Uuid::new_v4(), "test");
        let result = check_version_monotonicity(&meta, None);
        assert!(result.passed);
    }

    #[test]
    fn test_evaluate_all_gates_pass() {
        let meta = SnapshotMeta {
            object_type: ObjectType::VerbContract,
            object_id: Uuid::new_v4(),
            version_major: 1,
            version_minor: 0,
            status: SnapshotStatus::Active,
            governance_tier: GovernanceTier::Governed,
            trust_class: TrustClass::Proof,
            security_label: SecurityLabel::default(),
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: "analyst".into(),
            approved_by: Some("supervisor".into()),
            predecessor_id: None,
        };
        let gate = evaluate_publish_gates(&meta, None);
        assert!(gate.all_passed(), "Failures: {:?}", gate.failure_messages());
    }

    #[test]
    fn test_evaluate_gates_multiple_failures() {
        let meta = SnapshotMeta {
            object_type: ObjectType::AttributeDef,
            object_id: Uuid::new_v4(),
            version_major: 1,
            version_minor: 0,
            status: SnapshotStatus::Active,
            governance_tier: GovernanceTier::Operational,
            trust_class: TrustClass::Proof, // violates proof rule
            security_label: SecurityLabel {
                pii: true,
                classification: Classification::Public, // violates PII rule
                ..SecurityLabel::default()
            },
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: "test".into(),
            approved_by: None,
            predecessor_id: None,
        };
        let gate = evaluate_publish_gates(&meta, None);
        assert!(!gate.all_passed());
        assert_eq!(gate.failures().len(), 2);
    }

    // Helper to construct a mock predecessor row
    fn mock_predecessor(major: i32, minor: i32) -> SnapshotRow {
        SnapshotRow {
            snapshot_id: Uuid::new_v4(),
            snapshot_set_id: None,
            object_type: ObjectType::AttributeDef,
            object_id: Uuid::new_v4(),
            version_major: major,
            version_minor: minor,
            status: SnapshotStatus::Active,
            governance_tier: GovernanceTier::Operational,
            trust_class: TrustClass::Convenience,
            security_label: serde_json::json!({}),
            effective_from: chrono::Utc::now(),
            effective_until: None,
            predecessor_id: None,
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: "test".into(),
            approved_by: None,
            definition: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        }
    }

}
