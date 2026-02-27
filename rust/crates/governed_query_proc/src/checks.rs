//! Governance checks for `#[governed_query]`.
//!
//! Five compile-time checks that verify governance policy:
//! 1. Verb lifecycle — verb must be Active
//! 2. Principal requirement — Governed verbs require `&Principal` param
//! 3. PII authorization — PII-labelled verbs/attrs require `allow_pii = true`
//! 4. Proof rule — Proof trust class requires Governed tier
//! 5. Attribute lifecycle — referenced attributes must be Active

use crate::parse::GovernedQueryArgs;
use crate::registry_types::{GovernanceTier, GovernedCache, SnapshotStatus, TrustClass};

/// A governance violation found during compile-time checking.
#[derive(Debug)]
pub struct Violation {
    pub message: String,
}

/// Check whether any function parameter is a reference to `Principal`.
///
/// Looks for patterns like `&Principal`, `&sem_os_core::Principal`,
/// `principal: &Principal`, etc.
fn has_principal_param(sig: &syn::Signature) -> bool {
    for input in &sig.inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            if is_principal_type(&pat_type.ty) {
                return true;
            }
        }
    }
    false
}

fn is_principal_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Reference(type_ref) => is_principal_type(&type_ref.elem),
        syn::Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                segment.ident == "Principal"
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Run all 5 governance checks. Returns a list of violations.
pub fn run_checks(
    args: &GovernedQueryArgs,
    sig: &syn::Signature,
    cache: &GovernedCache,
) -> Vec<Violation> {
    let mut violations = Vec::new();

    // ── Check 1: Verb lifecycle ───────────────────────────────
    let verb_entry = cache.lookup_verb(&args.verb);
    match verb_entry {
        None => {
            violations.push(Violation {
                message: format!(
                    "governed_query: verb `{}` not found in governance cache. \
                     Ensure the verb is registered and run `cargo x governed-cache refresh`.",
                    args.verb
                ),
            });
            // Cannot run further checks without a verb entry
            return violations;
        }
        Some(entry) => {
            match entry.status {
                SnapshotStatus::Deprecated => {
                    violations.push(Violation {
                        message: format!(
                            "governed_query: verb `{}` is Deprecated. \
                             Migrate to its successor before it is Retired.",
                            args.verb
                        ),
                    });
                }
                SnapshotStatus::Retired => {
                    violations.push(Violation {
                        message: format!(
                            "governed_query: verb `{}` is Retired and must not be used.",
                            args.verb
                        ),
                    });
                }
                SnapshotStatus::Draft => {
                    violations.push(Violation {
                        message: format!(
                            "governed_query: verb `{}` is still in Draft status. \
                             Only Active verbs may be used in production code.",
                            args.verb
                        ),
                    });
                }
                SnapshotStatus::Active => {
                    // OK — pass
                }
            }

            // ── Check 2: Principal requirement ─────────────────
            if entry.governance_tier == GovernanceTier::Governed
                && !args.skip_principal_check
                && !has_principal_param(sig)
            {
                violations.push(Violation {
                    message: format!(
                        "governed_query: verb `{}` has Governed tier — \
                         function must accept a `&Principal` parameter \
                         (or set `skip_principal_check = true`).",
                        args.verb
                    ),
                });
            }

            // ── Check 3: PII authorization (verb-level) ───────
            if entry.pii && !args.allow_pii {
                violations.push(Violation {
                    message: format!(
                        "governed_query: verb `{}` carries PII label — \
                         add `allow_pii = true` to acknowledge PII handling.",
                        args.verb
                    ),
                });
            }

            // ── Check 4: Proof rule ────────────────────────────
            if entry.trust_class == TrustClass::Proof
                && entry.governance_tier != GovernanceTier::Governed
            {
                violations.push(Violation {
                    message: format!(
                        "governed_query: verb `{}` has Proof trust class but \
                         is not Governed tier — this violates the Proof Rule invariant.",
                        args.verb
                    ),
                });
            }
        }
    }

    // ── Check 5: Attribute lifecycle ──────────────────────────
    for attr_fqn in &args.attrs {
        match cache.lookup_attribute(attr_fqn) {
            None => {
                violations.push(Violation {
                    message: format!(
                        "governed_query: attribute `{attr_fqn}` not found in governance cache. \
                         Ensure the attribute is registered and run `cargo x governed-cache refresh`.",
                    ),
                });
            }
            Some(attr_entry) => {
                match attr_entry.status {
                    SnapshotStatus::Deprecated => {
                        violations.push(Violation {
                            message: format!(
                                "governed_query: attribute `{attr_fqn}` is Deprecated. \
                                 Migrate to its successor.",
                            ),
                        });
                    }
                    SnapshotStatus::Retired => {
                        violations.push(Violation {
                            message: format!(
                                "governed_query: attribute `{attr_fqn}` is Retired \
                                 and must not be referenced.",
                            ),
                        });
                    }
                    SnapshotStatus::Draft => {
                        violations.push(Violation {
                            message: format!(
                                "governed_query: attribute `{attr_fqn}` is still in Draft status.",
                            ),
                        });
                    }
                    SnapshotStatus::Active => {
                        // OK — pass
                    }
                }

                // PII check for attributes too
                if attr_entry.pii && !args.allow_pii {
                    violations.push(Violation {
                        message: format!(
                            "governed_query: attribute `{attr_fqn}` carries PII label — \
                             add `allow_pii = true` to acknowledge PII handling.",
                        ),
                    });
                }
            }
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry_types::*;
    use std::collections::HashMap;

    fn make_cache(entries: Vec<CacheEntry>) -> GovernedCache {
        let map: HashMap<String, CacheEntry> =
            entries.into_iter().map(|e| (e.fqn.clone(), e)).collect();
        GovernedCache {
            version: GovernedCache::CURRENT_VERSION,
            generated_at: "2026-02-27T00:00:00Z".to_string(),
            entries: map,
        }
    }

    fn make_verb(fqn: &str, status: SnapshotStatus, tier: GovernanceTier) -> CacheEntry {
        CacheEntry {
            fqn: fqn.to_string(),
            object_type: ObjectType::VerbContract,
            status,
            governance_tier: tier,
            trust_class: TrustClass::DecisionSupport,
            pii: false,
            classification: Classification::Internal,
        }
    }

    fn make_attr(fqn: &str, status: SnapshotStatus, pii: bool) -> CacheEntry {
        CacheEntry {
            fqn: fqn.to_string(),
            object_type: ObjectType::AttributeDef,
            status,
            governance_tier: GovernanceTier::Operational,
            trust_class: TrustClass::Convenience,
            pii,
            classification: Classification::Internal,
        }
    }

    fn parse_sig(code: &str) -> syn::Signature {
        let item: syn::ItemFn = syn::parse_str(code).expect("parse fn");
        item.sig
    }

    #[test]
    fn test_active_verb_passes() {
        let cache = make_cache(vec![make_verb(
            "cbu.create",
            SnapshotStatus::Active,
            GovernanceTier::Operational,
        )]);
        let args = GovernedQueryArgs {
            verb: "cbu.create".to_string(),
            attrs: vec![],
            allow_pii: false,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn create_cbu() {}");
        let violations = run_checks(&args, &sig, &cache);
        assert!(
            violations.is_empty(),
            "Expected no violations: {violations:?}"
        );
    }

    #[test]
    fn test_retired_verb_fails() {
        let cache = make_cache(vec![make_verb(
            "cbu.create",
            SnapshotStatus::Retired,
            GovernanceTier::Operational,
        )]);
        let args = GovernedQueryArgs {
            verb: "cbu.create".to_string(),
            attrs: vec![],
            allow_pii: false,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn create_cbu() {}");
        let violations = run_checks(&args, &sig, &cache);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("Retired"));
    }

    #[test]
    fn test_missing_verb_fails() {
        let cache = make_cache(vec![]);
        let args = GovernedQueryArgs {
            verb: "nonexistent.verb".to_string(),
            attrs: vec![],
            allow_pii: false,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn foo() {}");
        let violations = run_checks(&args, &sig, &cache);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("not found"));
    }

    #[test]
    fn test_governed_verb_requires_principal() {
        let cache = make_cache(vec![make_verb(
            "cbu.create",
            SnapshotStatus::Active,
            GovernanceTier::Governed,
        )]);
        let args = GovernedQueryArgs {
            verb: "cbu.create".to_string(),
            attrs: vec![],
            allow_pii: false,
            skip_principal_check: false,
        };
        // No Principal parameter
        let sig = parse_sig("fn create_cbu(pool: &PgPool) {}");
        let violations = run_checks(&args, &sig, &cache);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("Principal"));
    }

    #[test]
    fn test_governed_verb_with_principal_passes() {
        let cache = make_cache(vec![make_verb(
            "cbu.create",
            SnapshotStatus::Active,
            GovernanceTier::Governed,
        )]);
        let args = GovernedQueryArgs {
            verb: "cbu.create".to_string(),
            attrs: vec![],
            allow_pii: false,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn create_cbu(pool: &PgPool, principal: &Principal) {}");
        let violations = run_checks(&args, &sig, &cache);
        assert!(
            violations.is_empty(),
            "Expected no violations: {violations:?}"
        );
    }

    #[test]
    fn test_pii_verb_requires_authorization() {
        let mut verb = make_verb(
            "entity.get-pii",
            SnapshotStatus::Active,
            GovernanceTier::Operational,
        );
        verb.pii = true;
        let cache = make_cache(vec![verb]);
        let args = GovernedQueryArgs {
            verb: "entity.get-pii".to_string(),
            attrs: vec![],
            allow_pii: false,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn get_pii() {}");
        let violations = run_checks(&args, &sig, &cache);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("PII"));
    }

    #[test]
    fn test_pii_verb_with_authorization_passes() {
        let mut verb = make_verb(
            "entity.get-pii",
            SnapshotStatus::Active,
            GovernanceTier::Operational,
        );
        verb.pii = true;
        let cache = make_cache(vec![verb]);
        let args = GovernedQueryArgs {
            verb: "entity.get-pii".to_string(),
            attrs: vec![],
            allow_pii: true,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn get_pii() {}");
        let violations = run_checks(&args, &sig, &cache);
        assert!(
            violations.is_empty(),
            "Expected no violations: {violations:?}"
        );
    }

    #[test]
    fn test_deprecated_attr_fails() {
        let cache = make_cache(vec![
            make_verb(
                "cbu.create",
                SnapshotStatus::Active,
                GovernanceTier::Operational,
            ),
            make_attr("cbu.old_field", SnapshotStatus::Deprecated, false),
        ]);
        let args = GovernedQueryArgs {
            verb: "cbu.create".to_string(),
            attrs: vec!["cbu.old_field".to_string()],
            allow_pii: false,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn create_cbu() {}");
        let violations = run_checks(&args, &sig, &cache);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("Deprecated"));
    }

    #[test]
    fn test_pii_attr_requires_authorization() {
        let cache = make_cache(vec![
            make_verb(
                "entity.create",
                SnapshotStatus::Active,
                GovernanceTier::Operational,
            ),
            make_attr("entity.tax_id", SnapshotStatus::Active, true),
        ]);
        let args = GovernedQueryArgs {
            verb: "entity.create".to_string(),
            attrs: vec!["entity.tax_id".to_string()],
            allow_pii: false,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn create_entity() {}");
        let violations = run_checks(&args, &sig, &cache);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("PII"));
    }

    #[test]
    fn test_proof_rule_violation() {
        let mut verb = make_verb(
            "cbu.create",
            SnapshotStatus::Active,
            GovernanceTier::Operational,
        );
        verb.trust_class = TrustClass::Proof;
        let cache = make_cache(vec![verb]);
        let args = GovernedQueryArgs {
            verb: "cbu.create".to_string(),
            attrs: vec![],
            allow_pii: false,
            skip_principal_check: false,
        };
        let sig = parse_sig("fn create_cbu() {}");
        let violations = run_checks(&args, &sig, &cache);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("Proof Rule"));
    }
}
