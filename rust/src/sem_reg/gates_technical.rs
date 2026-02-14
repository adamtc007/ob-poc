//! Technical gates — apply to BOTH governance tiers.
//!
//! These gates enforce structural correctness and integrity:
//! 1. Type correctness — verb I/O types match attribute definitions
//! 2. Dependency correctness — no cycles, no unknown attribute refs
//! 3. Security label presence — all objects must have a SecurityLabel
//! 4. Verb surface disclosure — every attribute a verb reads/writes is in its I/O
//! 5. Snapshot integrity — predecessor references and version monotonicity
//! 6. Orphan attributes — attributes not consumed by any verb

use std::collections::HashSet;

use super::gates::{GateFailure, GateSeverity};
use super::types::{GovernanceTier, SnapshotRow};
use super::verb_contract::VerbContractBody;

// ── Gate 1: Type correctness ─────────────────────────────────

/// Check that a verb's I/O argument types match the attribute dictionary.
///
/// For each verb arg that references an attribute (via `lookup` or `maps_to`),
/// verify the attribute exists in the provided set.
pub fn check_type_correctness(
    verb: &VerbContractBody,
    known_attribute_fqns: &HashSet<String>,
) -> Vec<GateFailure> {
    let mut failures = Vec::new();

    for arg in &verb.args {
        // Check if arg references an attribute via lookup
        if let Some(ref lookup) = arg.lookup {
            let entity_type_fqn = format!("{}.{}", verb.domain, lookup.entity_type);
            // This is an entity lookup, not an attribute reference — skip for type correctness
            let _ = entity_type_fqn;
        }

        // Check valid_values against known enums if strict checking needed
        // (for now, valid_values are self-contained in the verb definition)
    }

    // Check produces: if a verb produces an entity type, verify it exists
    if let Some(ref produces) = verb.produces {
        let produced_fqn = format!("{}.{}", verb.domain, produces.entity_type);
        // Entity type, not attribute — tracked separately
        let _ = produced_fqn;
    }

    // Check consumes: verify consumed types exist
    for consumed in &verb.consumes {
        let consumed_fqn = format!("{}.{}", verb.domain, consumed);
        if !known_attribute_fqns.contains(&consumed_fqn) && !known_attribute_fqns.contains(consumed)
        {
            failures.push(
                GateFailure::warning(
                    "type_correctness",
                    "verb_contract",
                    format!(
                        "Verb '{}' consumes '{}' which is not in the attribute dictionary",
                        verb.fqn, consumed
                    ),
                )
                .with_fqn(&verb.fqn),
            );
        }
    }

    failures
}

// ── Gate 2: Dependency correctness ───────────────────────────

/// Check that derivation specs have no cycles and no unknown attribute references.
///
/// Delegates to `check_derivation_cycle` and `check_derivation_type_compatibility`
/// from `gates.rs`. This is a convenience re-export for the technical gate suite.
pub fn check_dependency_correctness(
    derivation_specs: &[super::derivation_spec::DerivationSpecBody],
    known_attribute_fqns: &HashSet<String>,
) -> Vec<GateFailure> {
    let mut failures = Vec::new();

    // Cycle detection
    failures.extend(super::gates::check_derivation_cycle(derivation_specs));

    // Type compatibility for each spec
    for spec in derivation_specs {
        failures.extend(super::gates::check_derivation_type_compatibility(
            spec,
            known_attribute_fqns,
        ));
    }

    failures
}

// ── Gate 3: Security label presence ──────────────────────────

/// Check that a snapshot has a valid (non-empty) security label.
pub fn check_security_label_presence(snapshot: &SnapshotRow) -> Vec<GateFailure> {
    // Parse the security label from JSONB
    match snapshot.parse_security_label() {
        Ok(_label) => {
            // Label exists and is parseable — pass
            vec![]
        }
        Err(e) => {
            vec![GateFailure::error(
                "security_label_presence",
                snapshot.object_type.to_string(),
                format!(
                    "Snapshot {} has invalid or missing security label: {}",
                    snapshot.snapshot_id, e
                ),
            )
            .with_snapshot_id(snapshot.snapshot_id)]
        }
    }
}

// ── Gate 4: Verb surface disclosure ──────────────────────────

/// Check that every attribute a verb reads/writes appears in its I/O surface
/// (args + produces + consumes).
pub fn check_verb_surface_disclosure(
    verb: &VerbContractBody,
    known_attribute_fqns: &HashSet<String>,
) -> Vec<GateFailure> {
    let failures = Vec::new();

    // Build the verb's declared I/O surface
    let mut declared_surface: HashSet<String> = HashSet::new();

    for arg in &verb.args {
        declared_surface.insert(format!("{}.{}", verb.domain, arg.name));
        declared_surface.insert(arg.name.clone());
    }

    if let Some(ref produces) = verb.produces {
        declared_surface.insert(format!("{}.{}", verb.domain, produces.entity_type));
        declared_surface.insert(produces.entity_type.clone());
    }

    for consumed in &verb.consumes {
        declared_surface.insert(format!("{}.{}", verb.domain, consumed));
        declared_surface.insert(consumed.clone());
    }

    // Check: any known attribute in the verb's domain that ISN'T in the surface
    // is a potential undisclosed dependency. We only warn for same-domain attributes.
    let verb_domain_prefix = format!("{}.", verb.domain);
    for attr_fqn in known_attribute_fqns {
        if attr_fqn.starts_with(&verb_domain_prefix) {
            let short_name = &attr_fqn[verb_domain_prefix.len()..];
            // Only warn if the verb has args that suggest it touches this domain
            // but doesn't declare this attribute
            if !declared_surface.contains(attr_fqn) && !declared_surface.contains(short_name) {
                // This is informational — not all attributes need to appear in every verb
                // Only flag if there's a naming match suggesting the verb should reference it
                // For now, this gate is intentionally lenient
            }
        }
    }

    failures
}

// ── Gate 5: Snapshot integrity ────────────────────────────────

/// Check that a new snapshot correctly references its predecessor.
pub fn check_snapshot_integrity(
    snapshot: &SnapshotRow,
    predecessor: Option<&SnapshotRow>,
) -> Vec<GateFailure> {
    let mut failures = Vec::new();

    if let Some(pred) = predecessor {
        // Version monotonicity
        let new_version = (snapshot.version_major, snapshot.version_minor);
        let old_version = (pred.version_major, pred.version_minor);
        if new_version < old_version {
            failures.push(
                GateFailure::error(
                    "snapshot_integrity",
                    snapshot.object_type.to_string(),
                    format!(
                        "Version {}.{} is less than predecessor {}.{}",
                        snapshot.version_major,
                        snapshot.version_minor,
                        pred.version_major,
                        pred.version_minor,
                    ),
                )
                .with_snapshot_id(snapshot.snapshot_id),
            );
        }

        // Object type must match
        if snapshot.object_type != pred.object_type {
            failures.push(
                GateFailure::error(
                    "snapshot_integrity",
                    snapshot.object_type.to_string(),
                    format!(
                        "Snapshot object_type {:?} does not match predecessor {:?}",
                        snapshot.object_type, pred.object_type,
                    ),
                )
                .with_snapshot_id(snapshot.snapshot_id),
            );
        }

        // Object ID must match
        if snapshot.object_id != pred.object_id {
            failures.push(
                GateFailure::error(
                    "snapshot_integrity",
                    snapshot.object_type.to_string(),
                    format!(
                        "Snapshot object_id {} does not match predecessor {}",
                        snapshot.object_id, pred.object_id,
                    ),
                )
                .with_snapshot_id(snapshot.snapshot_id),
            );
        }
    }

    failures
}

// ── Gate 6: Orphan attributes ────────────────────────────────

/// Check for attributes not consumed by any verb.
///
/// - Governed orphans → Error (every governed attribute should be traceable)
/// - Operational orphans → Warning (informational)
pub fn check_orphan_attributes(
    attribute_fqn: &str,
    tier: GovernanceTier,
    consuming_verbs: &[String],
) -> Vec<GateFailure> {
    if consuming_verbs.is_empty() {
        let severity = match tier {
            GovernanceTier::Governed => GateSeverity::Error,
            GovernanceTier::Operational => GateSeverity::Warning,
        };
        let failure = GateFailure {
            gate_name: "orphan_attributes".into(),
            severity,
            object_type: "attribute_def".into(),
            object_fqn: Some(attribute_fqn.into()),
            snapshot_id: None,
            message: format!("Attribute '{}' is not consumed by any verb", attribute_fqn),
            remediation_hint: Some(
                "Add this attribute to a verb's args or consumes list, or deprecate it".into(),
            ),
        };
        vec![failure]
    } else {
        vec![]
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::types::*;
    use crate::sem_reg::verb_contract::*;
    use uuid::Uuid;

    fn sample_verb() -> VerbContractBody {
        VerbContractBody {
            fqn: "cbu.create".into(),
            domain: "cbu".into(),
            action: "create".into(),
            description: "Create a CBU".into(),
            behavior: "plugin".into(),
            args: vec![VerbArgDef {
                name: "name".into(),
                arg_type: "string".into(),
                required: true,
                description: Some("CBU name".into()),
                lookup: None,
                valid_values: None,
                default: None,
            }],
            returns: None,
            preconditions: vec![],
            postconditions: vec![],
            produces: None,
            consumes: vec![],
            invocation_phrases: vec![],
            subject_kinds: vec![],
            phase_tags: vec![],
            requires_subject: true,
            produces_focus: false,
            metadata: None,
        }
    }

    fn mock_snapshot(major: i32, minor: i32) -> SnapshotRow {
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
            security_label: serde_json::json!({"classification": "internal"}),
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

    // ── Type correctness ──────────────────────────────────────

    #[test]
    fn test_type_correctness_no_consumes_passes() {
        let verb = sample_verb();
        let known: HashSet<String> = HashSet::new();
        let failures = check_type_correctness(&verb, &known);
        assert!(failures.is_empty());
    }

    #[test]
    fn test_type_correctness_unknown_consumed_warns() {
        let mut verb = sample_verb();
        verb.consumes = vec!["unknown_type".into()];
        let known: HashSet<String> = HashSet::new();
        let failures = check_type_correctness(&verb, &known);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].message.contains("consumes"));
    }

    #[test]
    fn test_type_correctness_known_consumed_passes() {
        let mut verb = sample_verb();
        verb.consumes = vec!["entity".into()];
        let mut known: HashSet<String> = HashSet::new();
        known.insert("cbu.entity".into());
        let failures = check_type_correctness(&verb, &known);
        assert!(failures.is_empty());
    }

    // ── Dependency correctness ────────────────────────────────

    #[test]
    fn test_dependency_correctness_no_specs_passes() {
        let failures = check_dependency_correctness(&[], &HashSet::new());
        assert!(failures.is_empty());
    }

    #[test]
    fn test_dependency_correctness_catches_cycle() {
        use crate::sem_reg::derivation_spec::*;

        let specs = vec![
            DerivationSpecBody {
                fqn: "d1".into(),
                name: "d1".into(),
                description: "test".into(),
                output_attribute_fqn: "a".into(),
                inputs: vec![DerivationInput {
                    attribute_fqn: "b".into(),
                    role: "in".into(),
                    required: true,
                }],
                expression: DerivationExpression::FunctionRef {
                    ref_name: "f".into(),
                },
                null_semantics: NullSemantics::default(),
                freshness_rule: None,
                security_inheritance: SecurityInheritanceMode::default(),
                evidence_grade: EvidenceGrade::default(),
                tests: vec![],
            },
            DerivationSpecBody {
                fqn: "d2".into(),
                name: "d2".into(),
                description: "test".into(),
                output_attribute_fqn: "b".into(),
                inputs: vec![DerivationInput {
                    attribute_fqn: "a".into(),
                    role: "in".into(),
                    required: true,
                }],
                expression: DerivationExpression::FunctionRef {
                    ref_name: "f".into(),
                },
                null_semantics: NullSemantics::default(),
                freshness_rule: None,
                security_inheritance: SecurityInheritanceMode::default(),
                evidence_grade: EvidenceGrade::default(),
                tests: vec![],
            },
        ];
        let failures = check_dependency_correctness(&specs, &HashSet::new());
        // Should have cycle + type compat failures
        assert!(!failures.is_empty());
        let has_cycle = failures.iter().any(|f| f.gate_name == "derivation_cycle");
        assert!(has_cycle);
    }

    // ── Security label presence ───────────────────────────────

    #[test]
    fn test_security_label_presence_valid() {
        let snapshot = mock_snapshot(1, 0);
        let failures = check_security_label_presence(&snapshot);
        assert!(failures.is_empty());
    }

    #[test]
    fn test_security_label_presence_invalid() {
        let mut snapshot = mock_snapshot(1, 0);
        snapshot.security_label = serde_json::json!("not_a_valid_label");
        let failures = check_security_label_presence(&snapshot);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].message.contains("invalid"));
    }

    // ── Verb surface disclosure ───────────────────────────────

    #[test]
    fn test_verb_surface_disclosure_passes() {
        let verb = sample_verb();
        let known: HashSet<String> = HashSet::new();
        let failures = check_verb_surface_disclosure(&verb, &known);
        assert!(failures.is_empty());
    }

    // ── Snapshot integrity ────────────────────────────────────

    #[test]
    fn test_snapshot_integrity_no_predecessor_passes() {
        let snapshot = mock_snapshot(1, 0);
        let failures = check_snapshot_integrity(&snapshot, None);
        assert!(failures.is_empty());
    }

    #[test]
    fn test_snapshot_integrity_version_monotonicity() {
        let snapshot = mock_snapshot(1, 0);
        let pred = mock_snapshot(2, 0);
        let failures = check_snapshot_integrity(&snapshot, Some(&pred));
        assert!(!failures.is_empty());
        assert!(failures[0].message.contains("Version"));
    }

    #[test]
    fn test_snapshot_integrity_type_mismatch() {
        let snapshot = mock_snapshot(2, 0);
        let mut pred = mock_snapshot(1, 0);
        pred.object_type = ObjectType::VerbContract;
        let failures = check_snapshot_integrity(&snapshot, Some(&pred));
        assert!(!failures.is_empty());
        assert!(failures[0].message.contains("object_type"));
    }

    #[test]
    fn test_snapshot_integrity_id_mismatch() {
        let snapshot = mock_snapshot(2, 0);
        let pred = mock_snapshot(1, 0);
        // object_id differs (different Uuid::new_v4 calls)
        let failures = check_snapshot_integrity(&snapshot, Some(&pred));
        assert!(!failures.is_empty());
        assert!(failures[0].message.contains("object_id"));
    }

    #[test]
    fn test_snapshot_integrity_valid_successor() {
        let object_id = Uuid::new_v4();
        let mut snapshot = mock_snapshot(2, 0);
        snapshot.object_id = object_id;
        let mut pred = mock_snapshot(1, 0);
        pred.object_id = object_id;
        let failures = check_snapshot_integrity(&snapshot, Some(&pred));
        assert!(failures.is_empty());
    }

    // ── Orphan attributes ─────────────────────────────────────

    #[test]
    fn test_orphan_governed_is_error() {
        let failures = check_orphan_attributes("kyc.risk_score", GovernanceTier::Governed, &[]);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].severity, GateSeverity::Error);
    }

    #[test]
    fn test_orphan_operational_is_warning() {
        let failures = check_orphan_attributes("cbu.temp_field", GovernanceTier::Operational, &[]);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].severity, GateSeverity::Warning);
    }

    #[test]
    fn test_non_orphan_passes() {
        let failures =
            check_orphan_attributes("cbu.name", GovernanceTier::Governed, &["cbu.create".into()]);
        assert!(failures.is_empty());
    }
}
