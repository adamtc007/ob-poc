//! Security label inheritance for the semantic registry.
//!
//! Pure functions — no database dependency. Computes inherited labels
//! when outputs are derived from multiple inputs (e.g., derivation specs,
//! verb side-effects).
//!
//! Default policy: **most restrictive wins**.
//! - Classification: highest ordinal
//! - PII: true if ANY input has PII
//! - Jurisdictions: union
//! - Purpose limitation: intersection (empty = no restriction)
//! - Handling controls: union

use super::types::{Classification, HandlingControl, SecurityLabel};

// ── Core functions ────────────────────────────────────────────

/// Compute the inherited security label from multiple inputs.
///
/// Policy: most restrictive wins.
/// - Classification: highest ordinal
/// - PII: true if any input has PII
/// - Jurisdictions: union of all
/// - Purpose limitation: intersection (empty input = no restriction, so skip)
/// - Handling controls: union of all
///
/// If `inputs` is empty, returns `default_label()`.
pub(crate) fn compute_inherited_label(inputs: &[SecurityLabel]) -> SecurityLabel {
    if inputs.is_empty() {
        return default_label();
    }

    let classification = inputs
        .iter()
        .map(|l| l.classification)
        .max_by_key(classification_level)
        .unwrap_or_default();

    let pii = inputs.iter().any(|l| l.pii);

    // Union of jurisdictions (deduplicated)
    let mut jurisdictions: Vec<String> = inputs
        .iter()
        .flat_map(|l| l.jurisdictions.iter().cloned())
        .collect();
    jurisdictions.sort();
    jurisdictions.dedup();

    // Intersection of purpose limitations.
    // Empty purpose_limitation means "no restriction" — skip those inputs.
    let restricted_inputs: Vec<&Vec<String>> = inputs
        .iter()
        .map(|l| &l.purpose_limitation)
        .filter(|p| !p.is_empty())
        .collect();

    let purpose_limitation = if restricted_inputs.is_empty() {
        vec![] // No restriction from any input
    } else {
        // Start with first restricted set, intersect with the rest
        let mut result = restricted_inputs[0].clone();
        for other in &restricted_inputs[1..] {
            result.retain(|p| other.contains(p));
        }
        result.sort();
        result
    };

    // Union of handling controls (deduplicated)
    let mut handling_controls: Vec<HandlingControl> = inputs
        .iter()
        .flat_map(|l| l.handling_controls.iter().cloned())
        .collect();
    handling_controls.sort_by_key(handling_control_ordinal);
    handling_controls.dedup_by_key(|c| handling_control_ordinal(c));

    SecurityLabel {
        classification,
        pii,
        jurisdictions,
        purpose_limitation,
        handling_controls,
    }
}

/// Default security label: Internal classification, no PII, no restrictions.
pub(crate) fn default_label() -> SecurityLabel {
    SecurityLabel::default()
}

// ── Helpers ───────────────────────────────────────────────────

/// Numeric level for classification comparison (higher = more restrictive).
fn classification_level(c: &Classification) -> u8 {
    match c {
        Classification::Public => 0,
        Classification::Internal => 1,
        Classification::Confidential => 2,
        Classification::Restricted => 3,
    }
}

/// Ordinal for dedup of handling controls.
fn handling_control_ordinal(c: &HandlingControl) -> u8 {
    match c {
        HandlingControl::MaskByDefault => 0,
        HandlingControl::NoExport => 1,
        HandlingControl::DualControl => 2,
        HandlingControl::SecureViewerOnly => 3,
        HandlingControl::NoLlmExternal => 4,
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn internal_label() -> SecurityLabel {
        SecurityLabel {
            classification: Classification::Internal,
            pii: false,
            jurisdictions: vec!["LU".into()],
            purpose_limitation: vec![],
            handling_controls: vec![],
        }
    }

    fn confidential_pii_label() -> SecurityLabel {
        SecurityLabel {
            classification: Classification::Confidential,
            pii: true,
            jurisdictions: vec!["DE".into()],
            purpose_limitation: vec!["operations".into(), "audit".into()],
            handling_controls: vec![HandlingControl::MaskByDefault],
        }
    }

    fn restricted_label() -> SecurityLabel {
        SecurityLabel {
            classification: Classification::Restricted,
            pii: true,
            jurisdictions: vec!["US".into()],
            purpose_limitation: vec!["operations".into()],
            handling_controls: vec![HandlingControl::NoExport, HandlingControl::DualControl],
        }
    }

    // ── compute_inherited_label tests ─────────────────────────

    #[test]
    fn test_empty_inputs_returns_default() {
        let result = compute_inherited_label(&[]);
        assert_eq!(result.classification, Classification::Internal);
        assert!(!result.pii);
        assert!(result.jurisdictions.is_empty());
        assert!(result.purpose_limitation.is_empty());
        assert!(result.handling_controls.is_empty());
    }

    #[test]
    fn test_single_input_passthrough() {
        let input = confidential_pii_label();
        let result = compute_inherited_label(std::slice::from_ref(&input));
        assert_eq!(result.classification, Classification::Confidential);
        assert!(result.pii);
        assert_eq!(result.jurisdictions, vec!["DE"]);
        assert_eq!(result.purpose_limitation, vec!["audit", "operations"]);
        assert_eq!(
            result.handling_controls,
            vec![HandlingControl::MaskByDefault]
        );
    }

    #[test]
    fn test_classification_takes_highest() {
        let result = compute_inherited_label(&[
            internal_label(),
            confidential_pii_label(),
            restricted_label(),
        ]);
        assert_eq!(result.classification, Classification::Restricted);
    }

    #[test]
    fn test_pii_true_if_any_input() {
        let result = compute_inherited_label(&[internal_label(), confidential_pii_label()]);
        assert!(result.pii);
    }

    #[test]
    fn test_pii_false_if_none() {
        let mut a = internal_label();
        a.pii = false;
        let mut b = internal_label();
        b.jurisdictions = vec!["IE".into()];
        b.pii = false;
        let result = compute_inherited_label(&[a, b]);
        assert!(!result.pii);
    }

    #[test]
    fn test_jurisdictions_union_deduplicated() {
        let mut a = internal_label(); // LU
        a.jurisdictions = vec!["LU".into(), "DE".into()];
        let mut b = internal_label();
        b.jurisdictions = vec!["DE".into(), "US".into()];
        let result = compute_inherited_label(&[a, b]);
        assert_eq!(result.jurisdictions, vec!["DE", "LU", "US"]);
    }

    #[test]
    fn test_purpose_limitation_intersection() {
        // A allows [operations, audit], B allows [operations]
        // Intersection = [operations]
        let result = compute_inherited_label(&[confidential_pii_label(), restricted_label()]);
        assert_eq!(result.purpose_limitation, vec!["operations"]);
    }

    #[test]
    fn test_purpose_limitation_empty_means_no_restriction() {
        // internal_label has empty purpose_limitation → no restriction
        // confidential has [operations, audit]
        // Result: [operations, audit] (empty is skipped)
        let result = compute_inherited_label(&[internal_label(), confidential_pii_label()]);
        assert_eq!(result.purpose_limitation, vec!["audit", "operations"]);
    }

    #[test]
    fn test_handling_controls_union() {
        let result = compute_inherited_label(&[confidential_pii_label(), restricted_label()]);
        assert!(result
            .handling_controls
            .contains(&HandlingControl::MaskByDefault));
        assert!(result
            .handling_controls
            .contains(&HandlingControl::NoExport));
        assert!(result
            .handling_controls
            .contains(&HandlingControl::DualControl));
    }

    // ── default_label tests ──────────────────────────────────

    #[test]
    fn test_default_label() {
        let label = default_label();
        assert_eq!(label.classification, Classification::Internal);
        assert!(!label.pii);
        assert!(label.jurisdictions.is_empty());
        assert!(label.purpose_limitation.is_empty());
        assert!(label.handling_controls.is_empty());
    }
}
