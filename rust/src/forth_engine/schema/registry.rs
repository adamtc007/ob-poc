//! Verb Registry - central lookup for all verb definitions.

use std::collections::HashMap;
use std::sync::LazyLock;
use crate::forth_engine::schema::types::VerbDef;
use crate::forth_engine::schema::verbs::{cbu, entity, document, kyc, screening, decision, monitoring, attribute};

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
            // CBU domain (9 verbs)
            &cbu::CBU_ENSURE,
            &cbu::CBU_CREATE,
            &cbu::CBU_ATTACH_ENTITY,
            &cbu::CBU_DETACH_ENTITY,
            &cbu::CBU_LIST_ENTITIES,
            &cbu::CBU_READ,
            &cbu::CBU_UPDATE,
            &cbu::CBU_DELETE,
            &cbu::CBU_FINALIZE,

            // Entity domain (5 verbs)
            &entity::ENTITY_CREATE_LIMITED_COMPANY,
            &entity::ENTITY_CREATE_PROPER_PERSON,
            &entity::ENTITY_CREATE_PARTNERSHIP,
            &entity::ENTITY_CREATE_TRUST,
            &entity::ENTITY_ENSURE_OWNERSHIP,

            // Document domain (6 verbs)
            &document::DOCUMENT_REQUEST,
            &document::DOCUMENT_RECEIVE,
            &document::DOCUMENT_VERIFY,
            &document::DOCUMENT_EXTRACT_ATTRIBUTES,
            &document::DOCUMENT_LINK,
            &document::DOCUMENT_CATALOG,

            // KYC domain (5 verbs)
            &kyc::INVESTIGATION_CREATE,
            &kyc::INVESTIGATION_UPDATE_STATUS,
            &kyc::INVESTIGATION_COMPLETE,
            &kyc::RISK_ASSESS_CBU,
            &kyc::RISK_SET_RATING,

            // Screening domain (7 verbs)
            &screening::SCREENING_PEP,
            &screening::SCREENING_SANCTIONS,
            &screening::SCREENING_ADVERSE_MEDIA,
            &screening::SCREENING_RESOLVE_HIT,
            &screening::SCREENING_DISMISS_HIT,
            &screening::SCREENING_BATCH,
            &screening::SCREENING_REFRESH,

            // Decision domain (7 verbs)
            &decision::DECISION_RECORD,
            &decision::DECISION_APPROVE,
            &decision::DECISION_REJECT,
            &decision::DECISION_ESCALATE,
            &decision::DECISION_ADD_CONDITION,
            &decision::DECISION_SATISFY_CONDITION,
            &decision::DECISION_DEFER,

            // Monitoring domain (7 verbs)
            &monitoring::MONITORING_SCHEDULE_REVIEW,
            &monitoring::MONITORING_TRIGGER_REVIEW,
            &monitoring::MONITORING_UPDATE_RISK,
            &monitoring::MONITORING_COMPLETE_REVIEW,
            &monitoring::MONITORING_CLOSE_CASE,
            &monitoring::MONITORING_ADD_ALERT_RULE,
            &monitoring::MONITORING_RECORD_ACTIVITY,

            // Attribute domain (7 verbs)
            &attribute::ATTRIBUTE_SET,
            &attribute::ATTRIBUTE_GET,
            &attribute::ATTRIBUTE_BULK_SET,
            &attribute::ATTRIBUTE_VALIDATE,
            &attribute::ATTRIBUTE_CLEAR,
            &attribute::ATTRIBUTE_HISTORY,
            &attribute::ATTRIBUTE_COPY_FROM_DOCUMENT,
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

    #[test]
    fn test_all_domains_registered() {
        let registry = VerbRegistry::new();
        
        let domains: Vec<_> = registry.domains().collect();
        assert!(domains.contains(&"cbu"));
        assert!(domains.contains(&"entity"));
        assert!(domains.contains(&"document"));
        assert!(domains.contains(&"kyc"));
        assert!(domains.contains(&"screening"));
        assert!(domains.contains(&"decision"));
        assert!(domains.contains(&"monitoring"));
        assert!(domains.contains(&"attribute"));
    }

    #[test]
    fn test_verb_count() {
        let registry = VerbRegistry::new();
        // 9 cbu + 5 entity + 6 document + 5 kyc + 7 screening + 7 decision + 7 monitoring + 7 attribute = 53
        assert!(registry.count() >= 50, "Expected at least 50 verbs, got {}", registry.count());
    }

    #[test]
    fn test_new_screening_verbs() {
        let registry = VerbRegistry::new();
        assert!(registry.exists("screening.pep"));
        assert!(registry.exists("screening.sanctions"));
        assert!(registry.exists("screening.adverse-media"));
        assert!(registry.exists("screening.resolve-hit"));
        assert!(registry.exists("screening.dismiss-hit"));
        assert!(registry.exists("screening.batch"));
        assert!(registry.exists("screening.refresh"));
    }

    #[test]
    fn test_new_decision_verbs() {
        let registry = VerbRegistry::new();
        assert!(registry.exists("decision.record"));
        assert!(registry.exists("decision.approve"));
        assert!(registry.exists("decision.reject"));
        assert!(registry.exists("decision.escalate"));
        assert!(registry.exists("decision.add-condition"));
        assert!(registry.exists("decision.satisfy-condition"));
        assert!(registry.exists("decision.defer"));
    }

    #[test]
    fn test_new_monitoring_verbs() {
        let registry = VerbRegistry::new();
        assert!(registry.exists("monitoring.schedule-review"));
        assert!(registry.exists("monitoring.trigger-review"));
        assert!(registry.exists("monitoring.update-risk"));
        assert!(registry.exists("monitoring.complete-review"));
        assert!(registry.exists("monitoring.close-case"));
        assert!(registry.exists("monitoring.add-alert-rule"));
        assert!(registry.exists("monitoring.record-activity"));
    }

    #[test]
    fn test_new_attribute_verbs() {
        let registry = VerbRegistry::new();
        assert!(registry.exists("attribute.set"));
        assert!(registry.exists("attribute.get"));
        assert!(registry.exists("attribute.bulk-set"));
        assert!(registry.exists("attribute.validate"));
        assert!(registry.exists("attribute.clear"));
        assert!(registry.exists("attribute.history"));
        assert!(registry.exists("attribute.copy-from-document"));
    }
}
