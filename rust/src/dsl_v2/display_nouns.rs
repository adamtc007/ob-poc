//! Display Nouns - Maps internal entity kinds to operator-facing labels
//!
//! The UI NEVER shows internal names like "cbu", "trading-profile", "kyc-case".
//! This module provides the translation layer.
//!
//! # Example
//!
//! ```rust
//! use ob_poc::dsl_v2::display_nouns::display_noun;
//!
//! // Internal kind → Display noun
//! assert_eq!(display_noun("cbu"), "Structure");
//! assert_eq!(display_noun("trading-profile"), "Mandate");
//! assert_eq!(display_noun("kyc-case"), "Case");
//! ```

use std::collections::HashMap;
use std::sync::LazyLock;

/// Map internal kind to display noun
///
/// Returns the operator-facing label for any internal entity kind.
/// Falls back to titlecasing the input if unknown.
pub fn display_noun(internal_kind: &str) -> &'static str {
    DISPLAY_NOUN_MAP.get(internal_kind).copied().unwrap_or(
        // Fallback: return a static "Unknown" for truly unknown types
        // In production, this should log a warning
        "Unknown",
    )
}

/// Map internal kind to display noun, with fallback to input
///
/// Like `display_noun` but returns the input as-is if not found.
/// Useful when you want to show something rather than "Unknown".
pub fn display_noun_or_self(internal_kind: &str) -> &str {
    DISPLAY_NOUN_MAP
        .get(internal_kind)
        .copied()
        .unwrap_or(internal_kind)
}

/// Check if an internal kind has a display mapping
pub fn has_display_noun(internal_kind: &str) -> bool {
    DISPLAY_NOUN_MAP.contains_key(internal_kind)
}

/// Get all known internal kinds
pub fn all_internal_kinds() -> impl Iterator<Item = &'static str> {
    DISPLAY_NOUN_MAP.keys().copied()
}

/// Static mapping of internal kinds to display nouns
static DISPLAY_NOUN_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Structure/Fund types
    m.insert("cbu", "Structure");
    m.insert("client-business-unit", "Structure");

    // Party types
    m.insert("entity", "Party");
    m.insert("person", "Person");
    m.insert("company", "Company");
    m.insert("trust", "Trust");
    m.insert("natural-person", "Person");
    m.insert("legal-person", "Company");

    // Role types
    m.insert("cbu-role", "Role");
    m.insert("role", "Role");
    m.insert("general-partner", "General Partner");
    m.insert("limited-partner", "Limited Partner");
    m.insert("investment-manager", "Investment Manager");
    m.insert("management-company", "Management Company");
    m.insert("custodian", "Custodian");
    m.insert("administrator", "Administrator");
    m.insert("director", "Director");
    m.insert("beneficial-owner", "Beneficial Owner");
    m.insert("authorized-signatory", "Authorized Signatory");

    // Mandate/Trading types
    m.insert("trading-profile", "Mandate");
    m.insert("mandate", "Mandate");
    m.insert("investment-mandate", "Mandate");

    // Case types
    m.insert("kyc-case", "Case");
    m.insert("case", "Case");
    m.insert("kyc", "Case");

    // Document types
    m.insert("document", "Document");
    m.insert("doc", "Document");

    // Client types
    m.insert("client", "Client");
    m.insert("commercial-client", "Client");

    // Contract types
    m.insert("contract", "Contract");
    m.insert("legal-contract", "Contract");

    // Product types
    m.insert("product", "Product");
    m.insert("custody-product", "Custody Product");
    m.insert("trading-product", "Trading Product");

    // Ownership/Control
    m.insert("ownership", "Ownership");
    m.insert("control", "Control");
    m.insert("ubo", "Beneficial Owner");
    m.insert("ultimate-beneficial-owner", "Beneficial Owner");

    // Structure types (as nouns)
    m.insert("private-equity", "Private Equity");
    m.insert("pe", "Private Equity");
    m.insert("sicav", "SICAV");
    m.insert("hedge", "Hedge Fund");
    m.insert("hedge-fund", "Hedge Fund");
    m.insert("etf", "ETF");
    m.insert("pension", "Pension");
    m.insert("trust-fund", "Trust");
    m.insert("fund-of-funds", "Fund of Funds");
    m.insert("fof", "Fund of Funds");

    // Identifier types
    m.insert("lei", "LEI");
    m.insert("isin", "ISIN");
    m.insert("cusip", "CUSIP");
    m.insert("sedol", "SEDOL");
    m.insert("bloomberg", "Bloomberg ID");
    m.insert("reuters", "Reuters ID");

    m
});

/// Forbidden tokens that should NEVER appear in operator-facing UI
///
/// Used by lint rules to ensure implementation details don't leak.
pub const FORBIDDEN_UI_TOKENS: &[&str] = &[
    "cbu",
    "cbu_id",
    "cbu-id",
    "client-business-unit",
    "entity_ref",
    "entity-ref",
    "trading-profile",
    "trading_profile",
    "kyc-case",
    "kyc_case",
    "cbu-role",
    "cbu_role",
    "entity_id",
    "entity-id",
];

/// Check if a string contains any forbidden UI tokens
pub fn contains_forbidden_token(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    FORBIDDEN_UI_TOKENS
        .iter()
        .find(|&token| lower.contains(token))
        .copied()
}

/// Pluralize a display noun
pub fn pluralize(noun: &str) -> String {
    match noun {
        "Person" => "People".to_string(),
        "Company" => "Companies".to_string(),
        "Party" => "Parties".to_string(),
        "Case" => "Cases".to_string(),
        "Structure" => "Structures".to_string(),
        "Mandate" => "Mandates".to_string(),
        "Document" => "Documents".to_string(),
        "Role" => "Roles".to_string(),
        "Client" => "Clients".to_string(),
        "Contract" => "Contracts".to_string(),
        "Product" => "Products".to_string(),
        "Beneficial Owner" => "Beneficial Owners".to_string(),
        "General Partner" => "General Partners".to_string(),
        "Limited Partner" => "Limited Partners".to_string(),
        "Investment Manager" => "Investment Managers".to_string(),
        "Management Company" => "Management Companies".to_string(),
        "Authorized Signatory" => "Authorized Signatories".to_string(),
        // Default: add 's'
        _ => format!("{}s", noun),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_noun_mapping() {
        assert_eq!(display_noun("cbu"), "Structure");
        assert_eq!(display_noun("trading-profile"), "Mandate");
        assert_eq!(display_noun("kyc-case"), "Case");
        assert_eq!(display_noun("person"), "Person");
        assert_eq!(display_noun("company"), "Company");
        assert_eq!(display_noun("cbu-role"), "Role");
    }

    #[test]
    fn test_display_noun_fallback() {
        assert_eq!(display_noun("unknown-type"), "Unknown");
    }

    #[test]
    fn test_display_noun_or_self() {
        assert_eq!(display_noun_or_self("cbu"), "Structure");
        assert_eq!(display_noun_or_self("unknown-type"), "unknown-type");
    }

    #[test]
    fn test_has_display_noun() {
        assert!(has_display_noun("cbu"));
        assert!(has_display_noun("trading-profile"));
        assert!(!has_display_noun("unknown-type"));
    }

    #[test]
    fn test_forbidden_tokens() {
        // cbu_id contains "cbu" which is checked first in the list
        assert!(contains_forbidden_token("cbu_id").is_some());
        assert!(contains_forbidden_token("The CBU is ready").is_some());
        assert!(contains_forbidden_token("trading-profile").is_some());
        assert_eq!(contains_forbidden_token("Structure is ready"), None);
        assert_eq!(contains_forbidden_token("Mandate created"), None);
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize("Person"), "People");
        assert_eq!(pluralize("Company"), "Companies");
        assert_eq!(pluralize("Party"), "Parties");
        assert_eq!(pluralize("Structure"), "Structures");
        assert_eq!(pluralize("Case"), "Cases");
        assert_eq!(pluralize("Beneficial Owner"), "Beneficial Owners");
        assert_eq!(pluralize("CustomNoun"), "CustomNouns");
    }

    #[test]
    fn test_all_internal_kinds() {
        let kinds: Vec<_> = all_internal_kinds().collect();
        assert!(kinds.contains(&"cbu"));
        assert!(kinds.contains(&"trading-profile"));
        assert!(kinds.contains(&"kyc-case"));
    }

    #[test]
    fn test_no_implementation_jargon_in_display() {
        // Verify that display nouns never contain forbidden tokens
        for kind in all_internal_kinds() {
            let display = display_noun(kind);
            assert!(
                contains_forbidden_token(display).is_none(),
                "Display noun '{}' for '{}' contains forbidden token",
                display,
                kind
            );
        }
    }
}
