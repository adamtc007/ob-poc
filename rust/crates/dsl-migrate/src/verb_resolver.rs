//! Service task implementation string → SemOS verb FQN.
//!
//! Returns `None` if no mapping found (caller emits `[HUMAN-RESOLVE]` marker).

/// Attempt to map a Camunda task implementation string to a known SemOS verb FQN.
pub fn resolve_verb(implementation: &str) -> Option<String> {
    let lower = implementation.to_lowercase();

    // Already looks like a SemOS FQN (domain.verb pattern, no Java class indicators).
    if looks_like_fqn(&lower) {
        return Some(implementation.to_string());
    }

    // Known Camunda worker topic → SemOS verb mappings.
    static MAPPINGS: &[(&str, &str)] = &[
        ("kyc-verify", "kyc.verify"),
        ("kyc-initiate", "kyc.initiate"),
        ("kyc-complete", "kyc.complete"),
        ("cbu-create", "cbu.create"),
        ("cbu-activate", "cbu.activate"),
        ("cbu-onboard", "cbu.onboard"),
        ("entity-create", "entity.create"),
        ("entity-verify", "entity.verify-identity"),
        ("entity-resolve", "entity.resolve"),
        ("sanctions-check", "screening.check-sanctions"),
        ("sanctions-screen", "screening.check-sanctions"),
        ("pep-check", "screening.check-pep"),
        ("aml-check", "screening.check-aml"),
        ("deal-create", "deal.create"),
        ("deal-advance", "deal.advance-stage"),
        ("deal-approve", "deal.approve"),
        ("sign-off", "workflow.final-sign-off"),
        ("final-sign-off", "workflow.final-sign-off"),
        ("escalate", "workflow.escalate-to-head"),
        ("send-notification", "workflow.send-reminder"),
        ("send-reminder", "workflow.send-reminder"),
        ("gleif-lookup", "gleif.lookup-lei"),
        ("lei-lookup", "gleif.lookup-lei"),
        ("document-request", "document.request"),
        ("document-verify", "document.verify"),
        ("invoice-generate", "billing.generate-invoice"),
        ("trading-profile-create", "trading-profile.create"),
        ("trading-profile-update", "trading-profile.update"),
    ];

    for (pattern, verb) in MAPPINGS {
        if lower.contains(pattern) {
            return Some(verb.to_string());
        }
    }

    None
}

/// Returns true if the string looks like a SemOS FQN (`domain.verb` form,
/// no Java package indicators).
fn looks_like_fqn(s: &str) -> bool {
    if s.contains("com.")
        || s.contains("org.")
        || s.contains("io.")
        || s.contains("java")
        || s.contains("::")
        || s.contains('/')
    {
        return false;
    }
    // Must contain exactly one dot and both sides non-empty
    let parts: Vec<&str> = s.splitn(2, '.').collect();
    parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_kyc_verify() {
        assert_eq!(resolve_verb("kyc-verify"), Some("kyc.verify".to_string()));
    }

    #[test]
    fn maps_sanctions_check() {
        assert_eq!(
            resolve_verb("sanctions-check"),
            Some("screening.check-sanctions".to_string())
        );
    }

    #[test]
    fn fqn_passthrough() {
        assert_eq!(
            resolve_verb("entity.verify-identity"),
            Some("entity.verify-identity".to_string())
        );
    }

    #[test]
    fn java_class_returns_none() {
        assert!(resolve_verb("com.example.tasks.KycVerifyDelegate").is_none());
    }

    #[test]
    fn unknown_returns_none() {
        assert!(resolve_verb("mystery-worker-topic-xyz").is_none());
    }
}
