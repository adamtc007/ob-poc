//! Verb Registry - central lookup for all verb definitions.

use std::collections::HashMap;
use std::sync::LazyLock;
use crate::forth_engine::schema::types::VerbDef;
use crate::forth_engine::schema::verbs::{cbu, entity, document, kyc};

/// Central registry for all verb definitions.
pub struct VerbRegistry {
    verbs: HashMap<&'static str, &'static VerbDef>,
    by_domain: HashMap<&'static str, Vec<&'static VerbDef>>,
}

impl VerbRegistry {
    /// Create a new registry with all verb definitions.
    pub fn new() -> Self {
        let mut verbs = HashMap::new();
        let mut by_domain: HashMap<&'static str, Vec<&'static VerbDef>> = HashMap::new();

        // All verb definitions
        let all_verbs: &[&'static VerbDef] = &[
            // CBU domain
            &cbu::CBU_ENSURE,
            &cbu::CBU_CREATE,
            &cbu::CBU_ATTACH_ENTITY,
            &cbu::CBU_DETACH_ENTITY,
            &cbu::CBU_LIST_ENTITIES,
            &cbu::CBU_READ,
            &cbu::CBU_UPDATE,
            &cbu::CBU_DELETE,
            &cbu::CBU_FINALIZE,

            // Entity domain
            &entity::ENTITY_CREATE_LIMITED_COMPANY,
            &entity::ENTITY_CREATE_PROPER_PERSON,
            &entity::ENTITY_CREATE_PARTNERSHIP,
            &entity::ENTITY_CREATE_TRUST,
            &entity::ENTITY_ENSURE_OWNERSHIP,

            // Document domain
            &document::DOCUMENT_REQUEST,
            &document::DOCUMENT_RECEIVE,
            &document::DOCUMENT_VERIFY,
            &document::DOCUMENT_EXTRACT_ATTRIBUTES,
            &document::DOCUMENT_LINK,
            &document::DOCUMENT_CATALOG,

            // KYC domain
            &kyc::INVESTIGATION_CREATE,
            &kyc::INVESTIGATION_UPDATE_STATUS,
            &kyc::INVESTIGATION_COMPLETE,
            &kyc::RISK_ASSESS_CBU,
            &kyc::RISK_SET_RATING,
            &kyc::SCREENING_PEP,
            &kyc::SCREENING_SANCTIONS,
            &kyc::DECISION_RECORD,
        ];

        for verb in all_verbs {
            verbs.insert(verb.name, *verb);
            by_domain.entry(verb.domain).or_default().push(*verb);
        }

        Self { verbs, by_domain }
    }

    /// Get a verb definition by name.
    pub fn get(&self, name: &str) -> Option<&'static VerbDef> {
        self.verbs.get(name).copied()
    }

    /// Get all verbs in a domain.
    pub fn get_by_domain(&self, domain: &str) -> &[&'static VerbDef] {
        self.by_domain.get(domain).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Iterate over all verbs.
    pub fn all(&self) -> impl Iterator<Item = &'static VerbDef> + '_ {
        self.verbs.values().copied()
    }

    /// Get all domain names.
    pub fn domains(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.by_domain.keys().copied()
    }

    /// Suggest similar verbs for typo correction.
    pub fn suggest(&self, name: &str) -> Vec<&'static str> {
        let mut suggestions: Vec<_> = self.verbs
            .keys()
            .filter(|k| {
                levenshtein_distance(k, name) <= 3 
                    || k.contains(name) 
                    || name.contains(*k)
            })
            .copied()
            .collect();
        suggestions.sort_by_key(|k| levenshtein_distance(k, name));
        suggestions.truncate(3);
        suggestions
    }

    /// Check if a verb exists.
    pub fn exists(&self, name: &str) -> bool {
        self.verbs.contains_key(name)
    }

    /// Get count of registered verbs.
    pub fn count(&self) -> usize {
        self.verbs.len()
    }
}

impl Default for VerbRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global verb registry instance.
pub static VERB_REGISTRY: LazyLock<VerbRegistry> = LazyLock::new(VerbRegistry::new);

/// Calculate Levenshtein distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len { matrix[i][0] = i; }
    for j in 0..=b_len { matrix[0][j] = j; }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_get() {
        let registry = VerbRegistry::new();
        
        let verb = registry.get("cbu.ensure");
        assert!(verb.is_some());
        assert_eq!(verb.unwrap().name, "cbu.ensure");
        
        let verb = registry.get("nonexistent.verb");
        assert!(verb.is_none());
    }

    #[test]
    fn test_registry_get_by_domain() {
        let registry = VerbRegistry::new();
        
        let cbu_verbs = registry.get_by_domain("cbu");
        assert!(!cbu_verbs.is_empty());
        
        let unknown = registry.get_by_domain("unknown");
        assert!(unknown.is_empty());
    }

    #[test]
    fn test_suggest() {
        let registry = VerbRegistry::new();
        
        let suggestions = registry.suggest("cbu.ensur");
        assert!(suggestions.contains(&"cbu.ensure"));
        
        let suggestions = registry.suggest("cbu.creat");
        assert!(suggestions.contains(&"cbu.create"));
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", "ab"), 1);
        assert_eq!(levenshtein_distance("abc", "abd"), 1);
        assert_eq!(levenshtein_distance("abc", "xyz"), 3);
    }

    #[test]
    fn test_global_registry() {
        assert!(VERB_REGISTRY.exists("cbu.ensure"));
        assert!(VERB_REGISTRY.count() > 0);
    }
}
