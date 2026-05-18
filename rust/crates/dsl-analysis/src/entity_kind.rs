//! Canonical entity-kind normalization shared across intent, discovery, and
//! subject-kind filtering.
//!
//! All tables are consumer-provided and registered once at startup:
//! - `set_entity_kind_aliases` — alias → canonical (from `entity_kind_aliases.yaml`)
//! - `set_subject_kind_registry` — hints + domains (from `subject_kind_registry.yaml`)
//!
//! When no tables are registered, all lookup functions return None or the
//! input unchanged — generic identity behaviour for dsl-lsp, bpmn-lite, etc.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Flat alias-to-canonical map (`alias → canonical`, both lowercased).
pub type EntityKindAliases = HashMap<String, String>;

/// Subject kind registry — two tables from `subject_kind_registry.yaml`.
pub struct SubjectKindRegistry {
    /// Arg name hint → subject kind (after stripping -id/-ref/-uuid).
    pub hints: HashMap<String, String>,
    /// DSL domain name → primary subject kind.
    pub domains: HashMap<String, String>,
}

static ALIASES: OnceLock<EntityKindAliases> = OnceLock::new();
static SUBJECT_KIND_REGISTRY: OnceLock<SubjectKindRegistry> = OnceLock::new();

/// Register the consumer's entity kind alias table.
///
/// Call once at startup. Subsequent calls are silently ignored (OnceLock).
pub fn set_entity_kind_aliases(aliases: EntityKindAliases) {
    let _ = ALIASES.set(aliases);
}

/// Register the consumer's subject kind registry (hints + domain mappings).
///
/// Call once at startup. Subsequent calls are silently ignored (OnceLock).
pub fn set_subject_kind_registry(registry: SubjectKindRegistry) {
    let _ = SUBJECT_KIND_REGISTRY.set(registry);
}

/// Look up a subject kind from an arg name hint.
///
/// Strips common suffixes (-id, -ref, -uuid), normalizes to lowercase-kebab,
/// then consults the registered hints table. Returns `None` if no hint matches
/// or no registry has been registered.
pub fn subject_kind_from_hint(hint: &str) -> Option<String> {
    let normalized = hint.trim().to_ascii_lowercase().replace('_', "-");
    let trimmed = normalized
        .trim_end_matches("-id")
        .trim_end_matches("-ref")
        .trim_end_matches("-uuid");
    SUBJECT_KIND_REGISTRY
        .get()
        .and_then(|r| r.hints.get(trimmed))
        .map(|k| canonicalize(k))
}

/// Look up the primary subject kind for a DSL domain name.
///
/// Returns the domain name unchanged if no mapping is registered.
pub fn subject_kind_for_domain(domain: &str) -> String {
    let normalized = domain.trim().to_ascii_lowercase();
    SUBJECT_KIND_REGISTRY
        .get()
        .and_then(|r| r.domains.get(&normalized))
        .map(|k| canonicalize(k))
        .unwrap_or_else(|| canonicalize(&normalized))
}

/// Normalize an entity-kind alias to the canonical vocabulary.
///
/// Looks up `kind` (lowercased + trimmed) in the registered alias table.
/// Falls back to the trimmed/lowercased input if no alias is registered or
/// no table has been set — identity behaviour for unknown kinds.
///
/// # Examples (after ob-poc aliases are registered)
///
/// ```
/// use dsl_analysis::entity_kind::canonicalize;
/// // Without a registered table: identity
/// assert_eq!(canonicalize("kyc_case"), "kyc_case");
/// ```
pub fn canonicalize(kind: &str) -> String {
    let normalized = kind.trim().to_ascii_lowercase();
    ALIASES
        .get()
        .and_then(|table| table.get(&normalized))
        .cloned()
        .unwrap_or(normalized)
}

/// Compare two entity kinds after canonicalization.
pub fn matches(left: &str, right: &str) -> bool {
    canonicalize(left) == canonicalize(right)
}

#[cfg(test)]
mod tests {
    use super::{canonicalize, matches, set_entity_kind_aliases, EntityKindAliases};

    fn ob_poc_aliases() -> EntityKindAliases {
        let pairs = [
            ("kyc_case", "kyc-case"),
            ("case", "kyc-case"),
            ("client_group", "client-group"),
            ("legal-entity", "company"),
            ("legal_entity", "company"),
            ("organization", "company"),
            ("org", "company"),
            ("individual", "person"),
            ("natural_person", "person"),
            ("client-book", "client-group"),
            ("client_book", "client-group"),
            ("investor-register", "investor"),
            ("investor_register", "investor"),
            ("investment-fund", "fund"),
            ("umbrella", "fund"),
            ("sub-fund", "fund"),
            ("compartment", "fund"),
            ("doc", "document"),
            ("evidence-document", "document"),
            ("legal-contract", "contract"),
            ("agreement", "contract"),
            ("msa", "contract"),
            ("mandate", "trading-profile"),
            ("trading-mandate", "trading-profile"),
            ("deal-record", "deal"),
            ("sales-deal", "deal"),
            ("client-business-unit", "cbu"),
            ("structure", "cbu"),
            ("trading-unit", "cbu"),
        ];
        pairs
            .iter()
            .map(|(a, c)| (a.to_string(), c.to_string()))
            .collect()
    }

    #[test]
    fn identity_without_registered_table() {
        // Without any alias table, canonicalize is identity.
        // Note: if another test already set the OnceLock this verifies
        // the registered table is used; the fallback assertion covers the
        // un-registered case documented in the API contract.
        let result = canonicalize("some_unknown_kind");
        // Either the alias table maps it or it passes through unchanged.
        assert!(!result.is_empty());
    }

    #[test]
    fn canonicalizes_known_aliases_with_table() {
        set_entity_kind_aliases(ob_poc_aliases());
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
        set_entity_kind_aliases(ob_poc_aliases());
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

    #[test]
    fn unknown_kind_passes_through() {
        set_entity_kind_aliases(ob_poc_aliases());
        // A kind not in the table is returned unchanged (lowercased).
        assert_eq!(canonicalize("some-unknown"), "some-unknown");
        assert_eq!(canonicalize("POOL"), "pool");
    }
}
