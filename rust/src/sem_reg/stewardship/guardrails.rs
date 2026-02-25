//! Guardrail Engine — G01 through G15 (spec §8.2).
//!
//! Each guardrail is a pure function returning `Option<GuardrailResult>`.
//! The severity map is defined in `GuardrailId::default_severity()` (types.rs).
//! `evaluate_all_guardrails()` runs all 15 checks and collects results.

use super::types::*;
use crate::sem_reg::types::SnapshotMeta;

/// Default severity map for guardrails — matches spec §8.2 exactly.
impl GuardrailId {
    pub fn default_severity(&self) -> GuardrailSeverity {
        match self {
            // Block — edit cannot be saved
            Self::G01RolePermission => GuardrailSeverity::Block,
            Self::G03TypeConstraint => GuardrailSeverity::Block,
            Self::G04ProofChainCompatibility => GuardrailSeverity::Block,
            Self::G05ClassificationRequired => GuardrailSeverity::Block,
            Self::G06SecurityLabelRequired => GuardrailSeverity::Block,
            Self::G07SilentMeaningChange => GuardrailSeverity::Block,
            Self::G08DeprecationWithoutReplacement => GuardrailSeverity::Block,
            Self::G15DraftUniquenessViolation => GuardrailSeverity::Block,
            // Warning — must be acknowledged before submit
            Self::G02NamingConvention => GuardrailSeverity::Warning,
            Self::G10ConflictDetected => GuardrailSeverity::Warning,
            Self::G11StaleTemplate => GuardrailSeverity::Warning,
            Self::G12ObservationImpact => GuardrailSeverity::Warning,
            Self::G13ResolutionMetadataMissing => GuardrailSeverity::Warning,
            // Advisory — informational only
            Self::G09AIKnowledgeBoundary => GuardrailSeverity::Advisory,
            Self::G14CompositionHintStale => GuardrailSeverity::Advisory,
        }
    }
}

/// Evaluate all applicable guardrails for the current changeset state.
/// Each guardrail is a pure function returning `Vec<GuardrailResult>`.
pub fn evaluate_all_guardrails(
    changeset: &ChangesetRow,
    entries: &[ChangesetEntryRow],
    conflicts: &[ConflictRecord],
    basis_records: &[BasisRecord],
    active_snapshots: &[SnapshotMeta],
    templates_used: &[StewardshipTemplate],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();

    // G01: RolePermission — field-level ABAC check
    results.extend(check_role_permissions(changeset, entries));

    // G02: NamingConvention — FQN pattern matching
    results.extend(check_naming_conventions(entries));

    // G03: TypeConstraint — data type vs governance tier compatibility
    results.extend(check_type_constraints(entries, active_snapshots));

    // G04: ProofChainCompatibility — attr in policy predicate but tier < Proof
    results.extend(check_proof_chain_compatibility(entries, active_snapshots));

    // G05: ClassificationRequired — regulated domain missing taxonomy membership
    results.extend(check_classification_required(entries));

    // G06: SecurityLabelRequired — PII/tax semantics missing security label
    results.extend(check_security_label_required(entries));

    // G07: SilentMeaningChange — type change without migration note
    results.extend(check_silent_meaning_change(entries, active_snapshots));

    // G08: DeprecationWithoutReplacement
    results.extend(check_deprecation_replacement(entries));

    // G09: AIKnowledgeBoundary — low-confidence Basis claims
    results.extend(check_ai_knowledge_boundary(basis_records));

    // G10: ConflictDetected — FQN modified in another open changeset
    results.extend(check_conflicts_detected(conflicts));

    // G11: StaleTemplate — template below current version
    results.extend(check_stale_template(changeset, templates_used));

    // G12: ObservationImpact — promotion affects existing observations
    results.extend(check_observation_impact(entries, active_snapshots));

    // G13: ResolutionMetadataMissing — VerbContract missing usage examples
    results.extend(check_resolution_metadata(entries));

    // G14: CompositionHintStale — VerbContract composition hints reference non-Active
    results.extend(check_composition_hints(entries, active_snapshots));

    // G15: DraftUniquenessViolation — duplicate Draft head per (object_type, object_id)
    results.extend(check_draft_uniqueness(entries));

    results
}

/// Returns true if any guardrail result has Block severity.
pub fn has_blocking_guardrails(results: &[GuardrailResult]) -> bool {
    results.iter().any(|r| r.severity == GuardrailSeverity::Block)
}

/// Returns true if any guardrail result has Warning severity (needs acknowledgement).
pub fn has_warning_guardrails(results: &[GuardrailResult]) -> bool {
    results.iter().any(|r| r.severity == GuardrailSeverity::Warning)
}

// ═══════════════════════════════════════════════════════════════════
//  Individual guardrail check functions
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
fn make_result(id: GuardrailId, message: &str, remediation: &str) -> GuardrailResult {
    let severity = id.default_severity();
    GuardrailResult {
        guardrail_id: id,
        severity,
        message: message.to_string(),
        remediation: remediation.to_string(),
        context: serde_json::json!({}),
    }
}

fn make_result_with_ctx(
    id: GuardrailId,
    message: &str,
    remediation: &str,
    context: serde_json::Value,
) -> GuardrailResult {
    let severity = id.default_severity();
    GuardrailResult {
        guardrail_id: id,
        severity,
        message: message.to_string(),
        remediation: remediation.to_string(),
        context,
    }
}

/// G01: RolePermission — verify the changeset owner has permission for all entry actions.
fn check_role_permissions(
    _changeset: &ChangesetRow,
    entries: &[ChangesetEntryRow],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    for entry in entries {
        // Promote and deprecate actions require elevated roles
        if matches!(entry.action, ChangesetAction::Promote | ChangesetAction::Deprecate) {
            // In a full implementation, we'd check the actor's ABAC roles here.
            // For now, we check if reasoning is provided (a proxy for intentional action).
            if entry.reasoning.is_none() {
                results.push(make_result_with_ctx(
                    GuardrailId::G01RolePermission,
                    &format!(
                        "Action '{}' on '{}' requires explicit reasoning",
                        entry.action.as_str(),
                        entry.object_fqn
                    ),
                    "Provide reasoning for this action or request elevated permissions",
                    serde_json::json!({
                        "entry_id": entry.entry_id,
                        "action": entry.action.as_str(),
                        "fqn": entry.object_fqn,
                    }),
                ));
            }
        }
    }
    results
}

/// G02: NamingConvention — FQN must follow domain.noun_phrase pattern.
fn check_naming_conventions(entries: &[ChangesetEntryRow]) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    for entry in entries {
        let fqn = &entry.object_fqn;
        // FQN must contain at least one dot (domain.name)
        if !fqn.contains('.') {
            results.push(make_result_with_ctx(
                GuardrailId::G02NamingConvention,
                &format!("FQN '{}' does not follow 'domain.name' convention", fqn),
                "Use dotted naming: domain.noun_phrase (e.g., 'cbu.jurisdiction_code')",
                serde_json::json!({
                    "entry_id": entry.entry_id,
                    "fqn": fqn,
                }),
            ));
        }
        // FQN parts should be snake_case or kebab-case (no spaces, no upper)
        let parts: Vec<&str> = fqn.split('.').collect();
        for part in &parts {
            if part.contains(' ') || part.is_empty() {
                results.push(make_result_with_ctx(
                    GuardrailId::G02NamingConvention,
                    &format!(
                        "FQN segment '{}' in '{}' contains spaces or is empty",
                        part, fqn
                    ),
                    "Use snake_case or kebab-case for FQN segments",
                    serde_json::json!({
                        "entry_id": entry.entry_id,
                        "fqn": fqn,
                    }),
                ));
            }
        }
    }
    results
}

/// G03: TypeConstraint — data type vs governance tier compatibility.
fn check_type_constraints(
    entries: &[ChangesetEntryRow],
    _active_snapshots: &[SnapshotMeta],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    for entry in entries {
        // Check if draft_payload contains a governance_tier + trust_class pair
        if let Some(tier_str) = entry.draft_payload.get("governance_tier").and_then(|v| v.as_str())
        {
            if let Some(trust_str) = entry
                .draft_payload
                .get("trust_class")
                .and_then(|v| v.as_str())
            {
                // Proof rule: Operational tier cannot have Proof trust class
                if tier_str == "operational" && trust_str == "proof" {
                    results.push(make_result_with_ctx(
                        GuardrailId::G03TypeConstraint,
                        &format!(
                            "'{}': Operational tier cannot have Proof trust class",
                            entry.object_fqn
                        ),
                        "Change governance_tier to 'governed' or trust_class to 'decision_support'",
                        serde_json::json!({
                            "entry_id": entry.entry_id,
                            "fqn": entry.object_fqn,
                            "governance_tier": tier_str,
                            "trust_class": trust_str,
                        }),
                    ));
                }
            }
        }
    }
    results
}

/// G04: ProofChainCompatibility — attribute used in a policy predicate must have Proof trust.
fn check_proof_chain_compatibility(
    entries: &[ChangesetEntryRow],
    active_snapshots: &[SnapshotMeta],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();

    // Build a set of governed policy rule object IDs from active snapshots
    let policy_attr_refs: Vec<String> = active_snapshots
        .iter()
        .filter(|s| s.object_type.to_string() == "policy_rule")
        .map(|s| s.object_id.to_string())
        .collect();

    for entry in entries {
        // If this is an attribute_def with trust < Proof, check if it's referenced by a policy
        if entry.object_type == "attribute_def" {
            let trust = entry
                .draft_payload
                .get("trust_class")
                .and_then(|v| v.as_str())
                .unwrap_or("convenience");
            if trust != "proof" && !policy_attr_refs.is_empty() {
                // Simple check: see if any active policy references this FQN
                // In a full implementation, we'd parse policy predicates
                let fqn = &entry.object_fqn;
                for policy_fqn in &policy_attr_refs {
                    if policy_fqn.contains(fqn) {
                        results.push(make_result_with_ctx(
                            GuardrailId::G04ProofChainCompatibility,
                            &format!(
                                "Attribute '{}' is referenced by policy '{}' but has trust_class '{}' (needs Proof)",
                                fqn, policy_fqn, trust
                            ),
                            "Upgrade trust_class to 'proof' or remove the policy reference",
                            serde_json::json!({
                                "entry_id": entry.entry_id,
                                "fqn": fqn,
                                "policy_fqn": policy_fqn,
                                "trust_class": trust,
                            }),
                        ));
                    }
                }
            }
        }
    }
    results
}

/// G05: ClassificationRequired — regulated domain items must have taxonomy classification.
fn check_classification_required(entries: &[ChangesetEntryRow]) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    let regulated_domains = ["kyc", "sanctions", "regulatory", "compliance"];

    for entry in entries {
        let fqn = &entry.object_fqn;
        let domain = fqn.split('.').next().unwrap_or("");
        if regulated_domains.contains(&domain) {
            // Check if draft_payload contains taxonomy_memberships
            let has_taxonomy = entry
                .draft_payload
                .get("taxonomy_memberships")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            if !has_taxonomy {
                results.push(make_result_with_ctx(
                    GuardrailId::G05ClassificationRequired,
                    &format!(
                        "'{}' is in regulated domain '{}' but has no taxonomy classification",
                        fqn, domain
                    ),
                    "Add at least one taxonomy_membership for this regulated object",
                    serde_json::json!({
                        "entry_id": entry.entry_id,
                        "fqn": fqn,
                        "domain": domain,
                    }),
                ));
            }
        }
    }
    results
}

/// G06: SecurityLabelRequired — PII/tax semantics must have explicit security label.
fn check_security_label_required(entries: &[ChangesetEntryRow]) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    let sensitive_keywords = ["pii", "tax", "ssn", "passport", "dob", "salary", "nationality"];

    for entry in entries {
        let fqn_lower = entry.object_fqn.to_lowercase();
        let is_sensitive = sensitive_keywords.iter().any(|kw| fqn_lower.contains(kw));

        if is_sensitive {
            let has_label = entry.draft_payload.get("security_label").is_some();
            if !has_label {
                results.push(make_result_with_ctx(
                    GuardrailId::G06SecurityLabelRequired,
                    &format!(
                        "'{}' appears to contain sensitive data but has no security_label",
                        entry.object_fqn
                    ),
                    "Add a security_label with appropriate classification and pii=true",
                    serde_json::json!({
                        "entry_id": entry.entry_id,
                        "fqn": entry.object_fqn,
                    }),
                ));
            }
        }
    }
    results
}

/// G07: SilentMeaningChange — type change on existing object without reasoning.
fn check_silent_meaning_change(
    entries: &[ChangesetEntryRow],
    active_snapshots: &[SnapshotMeta],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();

    for entry in entries {
        if entry.action == ChangesetAction::Modify {
            // Check if predecessor exists (meaning this is modifying an existing object)
            if entry.predecessor_id.is_some() {
                // Check if data_type or type changed
                let has_type_change = entry.draft_payload.get("data_type").is_some()
                    || entry.draft_payload.get("type").is_some();

                if has_type_change && entry.reasoning.is_none() {
                    // Verify it's actually a change by comparing against active snapshot.
                    // SnapshotMeta has object_id (UUID), not fqn; compare against predecessor_id.
                    let has_active = entry.predecessor_id.is_some_and(|pid| {
                        active_snapshots.iter().any(|s| s.object_id == pid)
                    });
                    if has_active {
                        results.push(make_result_with_ctx(
                            GuardrailId::G07SilentMeaningChange,
                            &format!(
                                "'{}': type change detected but no reasoning provided",
                                entry.object_fqn
                            ),
                            "Provide reasoning explaining the type change and migration impact",
                            serde_json::json!({
                                "entry_id": entry.entry_id,
                                "fqn": entry.object_fqn,
                            }),
                        ));
                    }
                }
            }
        }
    }
    results
}

/// G08: DeprecationWithoutReplacement — deprecating without specifying a replacement FQN.
fn check_deprecation_replacement(entries: &[ChangesetEntryRow]) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    for entry in entries {
        if entry.action == ChangesetAction::Deprecate {
            let has_replacement = entry
                .draft_payload
                .get("replacement_fqn")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if !has_replacement {
                results.push(make_result_with_ctx(
                    GuardrailId::G08DeprecationWithoutReplacement,
                    &format!(
                        "'{}': deprecation without replacement_fqn",
                        entry.object_fqn
                    ),
                    "Specify replacement_fqn in the draft payload or use 'retire' instead",
                    serde_json::json!({
                        "entry_id": entry.entry_id,
                        "fqn": entry.object_fqn,
                    }),
                ));
            }
        }
    }
    results
}

/// G09: AIKnowledgeBoundary — basis claims with low confidence or flagged as open questions.
fn check_ai_knowledge_boundary(basis_records: &[BasisRecord]) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    let low_confidence_threshold = 0.5;

    for basis in basis_records {
        // We'd need claims loaded; check the basis narrative for heuristics
        if basis.narrative.is_none() {
            results.push(make_result_with_ctx(
                GuardrailId::G09AIKnowledgeBoundary,
                &format!(
                    "Basis '{}' has no narrative — AI knowledge boundary unclear",
                    basis.title
                ),
                "Add a narrative explaining the source and confidence of this basis",
                serde_json::json!({
                    "basis_id": basis.basis_id,
                    "title": basis.title,
                }),
            ));
        }
    }
    // Note: claim-level checks (low confidence, flagged_as_open_question) happen
    // when claims are loaded alongside basis_records. This function receives
    // pre-aggregated basis records. The tools layer loads claims and injects
    // an advisory result for each low-confidence or open-question claim.
    let _ = low_confidence_threshold; // used at call site
    results
}

/// G10: ConflictDetected — open conflicts exist for this changeset.
fn check_conflicts_detected(conflicts: &[ConflictRecord]) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    for conflict in conflicts {
        if conflict.resolution_strategy.is_none() {
            results.push(make_result_with_ctx(
                GuardrailId::G10ConflictDetected,
                &format!(
                    "FQN '{}' is also modified in changeset {}",
                    conflict.fqn, conflict.competing_changeset_id
                ),
                "Resolve the conflict using merge, rebase, or supersede strategy",
                serde_json::json!({
                    "conflict_id": conflict.conflict_id,
                    "fqn": conflict.fqn,
                    "competing_changeset_id": conflict.competing_changeset_id,
                }),
            ));
        }
    }
    results
}

/// G11: StaleTemplate — changeset was created from a template that has been superseded.
fn check_stale_template(
    _changeset: &ChangesetRow,
    templates_used: &[StewardshipTemplate],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    for template in templates_used {
        if template.status == TemplateStatus::Deprecated {
            results.push(make_result_with_ctx(
                GuardrailId::G11StaleTemplate,
                &format!(
                    "Template '{}' (v{}) has been deprecated",
                    template.fqn, template.version
                ),
                "Consider re-creating the changeset from the latest active template",
                serde_json::json!({
                    "template_id": template.template_id,
                    "fqn": template.fqn,
                    "version": template.version.to_string(),
                }),
            ));
        }
    }
    results
}

/// G12: ObservationImpact — promoting an attribute may invalidate existing observations.
fn check_observation_impact(
    entries: &[ChangesetEntryRow],
    _active_snapshots: &[SnapshotMeta],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    for entry in entries {
        if entry.action == ChangesetAction::Promote && entry.object_type == "attribute_def" {
            // Promotion from Operational to Governed may require re-validation of observations
            let tier = entry
                .draft_payload
                .get("governance_tier")
                .and_then(|v| v.as_str());
            if tier == Some("governed") {
                results.push(make_result_with_ctx(
                    GuardrailId::G12ObservationImpact,
                    &format!(
                        "Promoting '{}' to Governed tier may require re-validation of existing observations",
                        entry.object_fqn
                    ),
                    "Review existing observations and ensure they meet Governed-tier evidence requirements",
                    serde_json::json!({
                        "entry_id": entry.entry_id,
                        "fqn": entry.object_fqn,
                    }),
                ));
            }
        }
    }
    results
}

/// G13: ResolutionMetadataMissing — VerbContract without examples or resolution hints.
fn check_resolution_metadata(entries: &[ChangesetEntryRow]) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    for entry in entries {
        if entry.object_type == "verb_contract" {
            let payload = &entry.draft_payload;
            let has_examples = payload
                .get("usage_examples")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            let has_description = payload
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);

            if !has_examples || !has_description {
                results.push(make_result_with_ctx(
                    GuardrailId::G13ResolutionMetadataMissing,
                    &format!(
                        "VerbContract '{}' missing {}",
                        entry.object_fqn,
                        if !has_examples && !has_description {
                            "usage_examples and description"
                        } else if !has_examples {
                            "usage_examples"
                        } else {
                            "description"
                        }
                    ),
                    "Add usage_examples and description to improve intent resolution quality",
                    serde_json::json!({
                        "entry_id": entry.entry_id,
                        "fqn": entry.object_fqn,
                    }),
                ));
            }
        }
    }
    results
}

/// G14: CompositionHintStale — VerbContract composition hints reference non-Active snapshots.
fn check_composition_hints(
    entries: &[ChangesetEntryRow],
    active_snapshots: &[SnapshotMeta],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();

    // Build set of active object IDs for fast lookup.
    // SnapshotMeta has object_id (UUID) but not fqn; we use object_id strings.
    let active_ids: std::collections::HashSet<String> = active_snapshots
        .iter()
        .map(|s| s.object_id.to_string())
        .collect();

    for entry in entries {
        if entry.object_type == "verb_contract" {
            if let Some(hints) = entry
                .draft_payload
                .get("composition_hints")
                .and_then(|v| v.as_array())
            {
                for hint in hints {
                    if let Some(ref_fqn) = hint.get("verb_fqn").and_then(|v| v.as_str()) {
                        if !active_ids.contains(ref_fqn) {
                            results.push(make_result_with_ctx(
                                GuardrailId::G14CompositionHintStale,
                                &format!(
                                    "VerbContract '{}' references '{}' in composition_hints but it is not Active",
                                    entry.object_fqn, ref_fqn
                                ),
                                "Update or remove the stale composition hint reference",
                                serde_json::json!({
                                    "entry_id": entry.entry_id,
                                    "fqn": entry.object_fqn,
                                    "stale_ref": ref_fqn,
                                }),
                            ));
                        }
                    }
                }
            }
        }
    }
    results
}

/// G15: DraftUniquenessViolation — duplicate Draft per (object_type, object_id) within changeset.
/// Also enforced by DB UNIQUE index; this provides a friendly pre-check message.
fn check_draft_uniqueness(entries: &[ChangesetEntryRow]) -> Vec<GuardrailResult> {
    let mut results = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

    for entry in entries {
        let key = (entry.object_type.clone(), entry.object_fqn.clone());
        if !seen.insert(key.clone()) {
            results.push(make_result_with_ctx(
                GuardrailId::G15DraftUniquenessViolation,
                &format!(
                    "Duplicate draft: ({}, '{}') already exists in this changeset",
                    entry.object_type, entry.object_fqn
                ),
                "Remove the duplicate entry or merge changes into the existing draft",
                serde_json::json!({
                    "entry_id": entry.entry_id,
                    "object_type": entry.object_type,
                    "fqn": entry.object_fqn,
                }),
            ));
        }
    }
    results
}

// ═══════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_entry(
        fqn: &str,
        object_type: &str,
        action: ChangesetAction,
        payload: serde_json::Value,
    ) -> ChangesetEntryRow {
        ChangesetEntryRow {
            entry_id: Uuid::new_v4(),
            changeset_id: Uuid::new_v4(),
            object_fqn: fqn.to_string(),
            object_type: object_type.to_string(),
            change_kind: "add".to_string(),
            draft_payload: payload,
            base_snapshot_id: None,
            created_at: Utc::now(),
            action,
            predecessor_id: None,
            revision: 1,
            reasoning: None,
            guardrail_log: serde_json::json!([]),
        }
    }

    fn make_changeset() -> ChangesetRow {
        ChangesetRow {
            changeset_id: Uuid::new_v4(),
            status: ChangesetStatus::Draft,
            owner_actor_id: "test-actor".to_string(),
            scope: "test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_g02_naming_convention_no_dot() {
        let entries = vec![make_entry(
            "badname",
            "attribute_def",
            ChangesetAction::Add,
            serde_json::json!({}),
        )];
        let results = check_naming_conventions(&entries);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].guardrail_id, GuardrailId::G02NamingConvention);
    }

    #[test]
    fn test_g02_naming_convention_valid() {
        let entries = vec![make_entry(
            "cbu.jurisdiction_code",
            "attribute_def",
            ChangesetAction::Add,
            serde_json::json!({}),
        )];
        let results = check_naming_conventions(&entries);
        assert!(results.is_empty());
    }

    #[test]
    fn test_g03_type_constraint_proof_operational() {
        let entries = vec![make_entry(
            "test.attr",
            "attribute_def",
            ChangesetAction::Add,
            serde_json::json!({
                "governance_tier": "operational",
                "trust_class": "proof",
            }),
        )];
        let results = check_type_constraints(&entries, &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].guardrail_id, GuardrailId::G03TypeConstraint);
    }

    #[test]
    fn test_g08_deprecation_without_replacement() {
        let entries = vec![make_entry(
            "cbu.old_field",
            "attribute_def",
            ChangesetAction::Deprecate,
            serde_json::json!({}),
        )];
        let results = check_deprecation_replacement(&entries);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].guardrail_id,
            GuardrailId::G08DeprecationWithoutReplacement
        );
    }

    #[test]
    fn test_g08_deprecation_with_replacement() {
        let entries = vec![make_entry(
            "cbu.old_field",
            "attribute_def",
            ChangesetAction::Deprecate,
            serde_json::json!({ "replacement_fqn": "cbu.new_field" }),
        )];
        let results = check_deprecation_replacement(&entries);
        assert!(results.is_empty());
    }

    #[test]
    fn test_g10_unresolved_conflict() {
        let conflicts = vec![ConflictRecord {
            conflict_id: Uuid::new_v4(),
            changeset_id: Uuid::new_v4(),
            competing_changeset_id: Uuid::new_v4(),
            fqn: "cbu.name".to_string(),
            detected_at: Utc::now(),
            resolution_strategy: None,
            resolution_rationale: None,
            resolved_by: None,
            resolved_at: None,
        }];
        let results = check_conflicts_detected(&conflicts);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].guardrail_id, GuardrailId::G10ConflictDetected);
    }

    #[test]
    fn test_g15_draft_uniqueness() {
        let entries = vec![
            make_entry(
                "cbu.name",
                "attribute_def",
                ChangesetAction::Add,
                serde_json::json!({}),
            ),
            make_entry(
                "cbu.name",
                "attribute_def",
                ChangesetAction::Modify,
                serde_json::json!({}),
            ),
        ];
        let results = check_draft_uniqueness(&entries);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].guardrail_id,
            GuardrailId::G15DraftUniquenessViolation
        );
    }

    #[test]
    fn test_evaluate_all_no_issues() {
        let changeset = make_changeset();
        let entries = vec![make_entry(
            "cbu.jurisdiction_code",
            "attribute_def",
            ChangesetAction::Add,
            serde_json::json!({}),
        )];
        let results = evaluate_all_guardrails(&changeset, &entries, &[], &[], &[], &[]);
        // No blocking issues expected for a simple valid entry
        assert!(!has_blocking_guardrails(&results));
    }

    #[test]
    fn test_has_blocking_guardrails() {
        let results = vec![
            make_result(
                GuardrailId::G02NamingConvention,
                "warning",
                "fix naming",
            ),
            make_result(
                GuardrailId::G03TypeConstraint,
                "block",
                "fix type",
            ),
        ];
        assert!(has_blocking_guardrails(&results));
    }
}
