//! Canonical entity-kind normalization shared across intent, discovery, and
//! subject-kind filtering.

/// Normalize an entity-kind alias to the canonical vocabulary used for
/// subject-kind matching.
///
/// # Examples
///
/// ```
/// use ob_poc::entity_kind::canonicalize;
///
/// assert_eq!(canonicalize("kyc_case"), "kyc-case");
/// assert_eq!(canonicalize("client_group"), "client-group");
/// assert_eq!(canonicalize("investor-register"), "investor");
/// assert_eq!(canonicalize("umbrella"), "fund");
/// ```
pub fn canonicalize(kind: &str) -> String {
    let normalized = kind.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "kyc_case" | "case" => "kyc-case".to_string(),
        "client_group" => "client-group".to_string(),
        "legal-entity" | "legal_entity" | "organization" | "org" => "company".to_string(),
        "individual" | "natural_person" => "person".to_string(),
        "client-book" | "client_book" => "client-group".to_string(),
        "investor-register" | "investor_register" => "investor".to_string(),
        "investment-fund" | "umbrella" | "sub-fund" | "compartment" => "fund".to_string(),
        "doc" | "evidence-document" => "document".to_string(),
        "legal-contract" | "agreement" | "msa" => "contract".to_string(),
        "mandate" | "trading-mandate" => "trading-profile".to_string(),
        "deal-record" | "sales-deal" => "deal".to_string(),
        "client-business-unit" | "structure" | "trading-unit" => "cbu".to_string(),
        other => other.to_string(),
    }
}

/// Compare two entity kinds after canonicalization.
///
/// # Examples
///
/// ```
/// use ob_poc::entity_kind::matches;
///
/// assert!(matches("kyc_case", "kyc-case"));
/// assert!(matches("organization", "company"));
/// assert!(matches("umbrella", "fund"));
/// ```
pub fn matches(left: &str, right: &str) -> bool {
    canonicalize(left) == canonicalize(right)
}

#[cfg(test)]
mod tests {
    use super::{canonicalize, matches};

    #[test]
    fn canonicalizes_known_aliases() {
        assert_eq!(canonicalize("kyc_case"), "kyc-case");
        assert_eq!(canonicalize("client_group"), "client-group");
        assert_eq!(canonicalize("organization"), "company");
        assert_eq!(canonicalize("investor-register"), "investor");
        assert_eq!(canonicalize("umbrella"), "fund");
        assert_eq!(canonicalize("doc"), "document");
        assert_eq!(canonicalize("agreement"), "contract");
        assert_eq!(canonicalize("mandate"), "trading-profile");
        assert_eq!(canonicalize("deal-record"), "deal");
        assert_eq!(canonicalize("structure"), "cbu");
    }

    #[test]
    fn compares_aliases_by_canonical_value() {
        assert!(matches("kyc_case", "kyc-case"));
        assert!(matches("client_group", "client-group"));
        assert!(matches("organization", "company"));
        assert!(matches("investor-register", "investor"));
        assert!(matches("umbrella", "fund"));
        assert!(matches("agreement", "contract"));
        assert!(matches("mandate", "trading-profile"));
        assert!(matches("deal-record", "deal"));
        assert!(matches("trading-unit", "cbu"));
    }
}
