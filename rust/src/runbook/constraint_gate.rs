//! Pack Constraint Gate — post-expansion enforcement.
//!
//! Positioned **after** macro expansion, **before** validate/lint in the
//! compilation pipeline. Checks expanded verb lists against the effective
//! constraints from all active packs.
//!
//! ## Behavior
//!
//! - If no constraints are active → pass-through.
//! - If the allowed set intersection is empty → return ConstraintViolation
//!   with remediation options.
//! - If specific verbs violate constraints → return ConstraintViolation
//!   with per-verb detail + remediation options.
//! - If all verbs pass → return Ok.

use super::response::{
    ActiveConstraint, AlternativeVerb, ConstraintType, ConstraintViolationDetail, Remediation,
};
use crate::journey::pack_manager::{ConstraintViolation, EffectiveConstraints, ViolationReason};

/// Check expanded verbs against effective pack constraints.
///
/// # Arguments
///
/// * `expanded_verbs` — verbs from macro expansion (may include duplicates)
/// * `constraints` — effective constraints from PackManager
///
/// # Returns
///
/// `Ok(())` if all verbs pass, or a `ConstraintViolationDetail` with
/// violation info and remediation options.
pub fn check_pack_constraints(
    expanded_verbs: &[String],
    constraints: &EffectiveConstraints,
) -> Result<(), ConstraintViolationDetail> {
    // No active constraints → pass-through
    if !constraints.is_constrained() {
        return Ok(());
    }

    // Empty intersection between active packs → deadlock
    if constraints.is_empty_intersection() {
        return Err(ConstraintViolationDetail {
            explanation:
                "Active packs have no overlapping allowed verbs. Cannot proceed with any verb."
                    .to_string(),
            violating_verbs: expanded_verbs.to_vec(),
            active_constraints: constraints
                .contributing_packs
                .iter()
                .map(|source| ActiveConstraint {
                    pack_id: source.pack_id.clone(),
                    pack_name: source.pack_name.clone(),
                    constraint_type: ConstraintType::ForbiddenVerb {
                        verb: "(empty intersection)".to_string(),
                    },
                })
                .collect(),
            remediation_options: constraints
                .contributing_packs
                .iter()
                .map(|source| Remediation::SuspendPack {
                    pack_id: source.pack_id.clone(),
                    pack_name: source.pack_name.clone(),
                })
                .collect(),
        });
    }

    // Check each expanded verb
    let violations = constraints.check_verbs(expanded_verbs);

    if violations.is_empty() {
        return Ok(());
    }

    // Build active constraints from violations
    let active_constraints: Vec<ActiveConstraint> = violations
        .iter()
        .flat_map(|v| build_active_constraints(v, constraints))
        .collect();

    // Build remediation options
    let remediation_options = build_remediation(constraints, &violations);

    let violating_verbs: Vec<String> = violations.iter().map(|v| v.verb.clone()).collect();
    let explanation = format!(
        "{} verb(s) violate active pack constraints: {}",
        violating_verbs.len(),
        violating_verbs.join(", ")
    );

    Err(ConstraintViolationDetail {
        explanation,
        violating_verbs,
        active_constraints,
        remediation_options,
    })
}

/// Build `ActiveConstraint` entries for a violation.
fn build_active_constraints(
    violation: &ConstraintViolation,
    constraints: &EffectiveConstraints,
) -> Vec<ActiveConstraint> {
    constraints
        .contributing_packs
        .iter()
        .map(|source| ActiveConstraint {
            pack_id: source.pack_id.clone(),
            pack_name: source.pack_name.clone(),
            constraint_type: match violation.reason {
                ViolationReason::Forbidden => ConstraintType::ForbiddenVerb {
                    verb: violation.verb.clone(),
                },
                ViolationReason::NotInAllowedSet => ConstraintType::ForbiddenVerb {
                    verb: violation.verb.clone(),
                },
            },
        })
        .collect()
}

/// Build remediation options based on constraint violations.
fn build_remediation(
    constraints: &EffectiveConstraints,
    violations: &[ConstraintViolation],
) -> Vec<Remediation> {
    let mut remediations = Vec::new();

    // Option 1: Suggest suspending constraining packs
    for source in &constraints.contributing_packs {
        remediations.push(Remediation::SuspendPack {
            pack_id: source.pack_id.clone(),
            pack_name: source.pack_name.clone(),
        });
    }

    // Option 2: Suggest alternative verbs from the allowed set
    if let Some(allowed) = &constraints.allowed_verbs {
        let alternatives: Vec<AlternativeVerb> = allowed
            .iter()
            .filter(|v| {
                // Only suggest verbs with a similar domain
                violations.iter().any(|viol| {
                    let viol_domain = viol.verb.split('.').next().unwrap_or("");
                    let alt_domain = v.split('.').next().unwrap_or("");
                    viol_domain == alt_domain
                })
            })
            .take(5) // Limit suggestions
            .map(|v| AlternativeVerb {
                verb: v.clone(),
                description: format!("{} (allowed by pack constraints)", v),
                score: 0.0,
            })
            .collect();

        if !alternatives.is_empty() {
            remediations.push(Remediation::AlternativeVerbs { alternatives });
        }
    }

    remediations
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::journey::pack_manager::ConstraintSource;

    fn make_constraints(allowed: Option<&[&str]>, forbidden: &[&str]) -> EffectiveConstraints {
        EffectiveConstraints {
            allowed_verbs: allowed.map(|a| a.iter().map(|s| s.to_string()).collect()),
            forbidden_verbs: forbidden.iter().map(|s| s.to_string()).collect(),
            contributing_packs: vec![ConstraintSource {
                pack_id: "test-pack".to_string(),
                pack_name: "Test Pack".to_string(),
                allowed_count: allowed.map_or(0, |a| a.len()),
                forbidden_count: forbidden.len(),
            }],
        }
    }

    #[test]
    fn test_no_constraints_pass_through() {
        let constraints = EffectiveConstraints::unconstrained();
        let verbs = vec!["anything.goes".to_string()];
        assert!(check_pack_constraints(&verbs, &constraints).is_ok());
    }

    #[test]
    fn test_all_verbs_allowed() {
        let constraints = make_constraints(Some(&["cbu.create", "entity.create"]), &[]);
        let verbs = vec!["cbu.create".to_string()];
        assert!(check_pack_constraints(&verbs, &constraints).is_ok());
    }

    #[test]
    fn test_forbidden_verb_violation() {
        let constraints = make_constraints(None, &["cbu.delete"]);
        let verbs = vec!["cbu.delete".to_string()];
        let err = check_pack_constraints(&verbs, &constraints).unwrap_err();
        assert_eq!(err.violating_verbs, vec!["cbu.delete"]);
    }

    #[test]
    fn test_not_in_allowed_set_violation() {
        let constraints = make_constraints(Some(&["kyc.create-case"]), &[]);
        let verbs = vec!["entity.create".to_string()];
        let err = check_pack_constraints(&verbs, &constraints).unwrap_err();
        assert_eq!(err.violating_verbs, vec!["entity.create"]);
    }

    #[test]
    fn test_empty_intersection_violation() {
        let constraints = EffectiveConstraints {
            allowed_verbs: Some(HashSet::new()), // Empty intersection
            forbidden_verbs: HashSet::new(),
            contributing_packs: vec![
                ConstraintSource {
                    pack_id: "p1".to_string(),
                    pack_name: "Pack 1".to_string(),
                    allowed_count: 2,
                    forbidden_count: 0,
                },
                ConstraintSource {
                    pack_id: "p2".to_string(),
                    pack_name: "Pack 2".to_string(),
                    allowed_count: 2,
                    forbidden_count: 0,
                },
            ],
        };
        let verbs = vec!["cbu.create".to_string()];
        let err = check_pack_constraints(&verbs, &constraints).unwrap_err();
        assert!(err.explanation.contains("no overlapping"));
        // Should suggest suspending both packs
        assert!(err.remediation_options.len() >= 2);
    }

    #[test]
    fn test_mixed_pass_and_violation() {
        let constraints = make_constraints(
            Some(&["kyc.create-case", "kyc.submit-case"]),
            &["cbu.delete"],
        );
        let verbs = vec![
            "kyc.create-case".to_string(), // allowed
            "cbu.delete".to_string(),      // forbidden
            "entity.create".to_string(),   // not in allowed set
        ];
        let err = check_pack_constraints(&verbs, &constraints).unwrap_err();
        assert_eq!(err.violating_verbs.len(), 2);
        assert!(err.violating_verbs.contains(&"cbu.delete".to_string()));
        assert!(err.violating_verbs.contains(&"entity.create".to_string()));
    }

    #[test]
    fn test_remediation_includes_alternatives() {
        let constraints = make_constraints(Some(&["kyc.create-case", "kyc.submit-case"]), &[]);
        let verbs = vec!["kyc.delete-case".to_string()]; // same domain, not allowed
        let err = check_pack_constraints(&verbs, &constraints).unwrap_err();

        // Should suggest kyc.* alternatives from allowed set
        let has_alternatives = err.remediation_options.iter().any(|r| {
            matches!(r, Remediation::AlternativeVerbs { alternatives } if !alternatives.is_empty())
        });
        assert!(has_alternatives);
    }
}
