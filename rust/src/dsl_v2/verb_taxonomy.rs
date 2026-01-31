//! Verb Taxonomy - Hierarchical organization of DSL verbs
//!
//! Provides domain→category→verb hierarchy for:
//! 1. Domain inference during verb search
//! 2. Guided verb picker UI
//! 3. Disambiguation UX with domain context
//!
//! Loaded from `config/verb_schemas/taxonomy.yaml`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Global taxonomy instance (lazy-loaded)
static TAXONOMY: OnceLock<VerbTaxonomy> = OnceLock::new();

/// Get the global verb taxonomy (loads on first access)
pub fn verb_taxonomy() -> &'static VerbTaxonomy {
    TAXONOMY.get_or_init(|| {
        load_taxonomy_from_config().unwrap_or_else(|e| {
            tracing::warn!("Failed to load verb taxonomy: {}, using empty", e);
            VerbTaxonomy::default()
        })
    })
}

/// Load taxonomy from config file
fn load_taxonomy_from_config() -> anyhow::Result<VerbTaxonomy> {
    let config_path = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
    let taxonomy_path = format!("{}/verb_schemas/taxonomy.yaml", config_path);

    let content = std::fs::read_to_string(&taxonomy_path)
        .map_err(|e| anyhow::anyhow!("Failed to read taxonomy file {}: {}", taxonomy_path, e))?;

    let taxonomy: VerbTaxonomy = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse taxonomy YAML: {}", e))?;

    tracing::info!(domains = taxonomy.domains.len(), "Loaded verb taxonomy");

    Ok(taxonomy)
}

/// Root taxonomy structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerbTaxonomy {
    pub version: u32,
    #[serde(default)]
    pub generated: bool,
    pub domains: Vec<TaxonomyDomain>,

    /// Indexes built after loading
    #[serde(skip)]
    indexes: TaxonomyIndexes,
}

/// Indexes for fast lookup
#[derive(Debug, Clone, Default)]
struct TaxonomyIndexes {
    /// domain_id → domain
    by_domain_id: HashMap<String, usize>,
    /// verb_fqn → (domain_id, category_id)
    verb_to_domain: HashMap<String, (String, String)>,
    /// keyword → domain_id (for domain inference)
    keyword_to_domain: HashMap<String, String>,
}

impl VerbTaxonomy {
    /// Build indexes after loading
    pub fn build_indexes(&mut self) {
        self.indexes = TaxonomyIndexes::default();

        for (idx, domain) in self.domains.iter().enumerate() {
            // Index by domain ID
            self.indexes.by_domain_id.insert(domain.id.clone(), idx);

            // Index keywords
            for keyword in &domain.keywords {
                self.indexes
                    .keyword_to_domain
                    .insert(keyword.to_lowercase(), domain.id.clone());
            }

            // Index verbs
            for category in &domain.categories {
                for verb_fqn in &category.verbs {
                    self.indexes
                        .verb_to_domain
                        .insert(verb_fqn.clone(), (domain.id.clone(), category.id.clone()));
                }
            }
        }
    }

    /// Get domain by ID
    pub fn get_domain(&self, domain_id: &str) -> Option<&TaxonomyDomain> {
        self.indexes
            .by_domain_id
            .get(domain_id)
            .map(|&idx| &self.domains[idx])
    }

    /// Get domain for a verb FQN
    pub fn domain_for_verb(&self, verb_fqn: &str) -> Option<&TaxonomyDomain> {
        self.indexes
            .verb_to_domain
            .get(verb_fqn)
            .and_then(|(domain_id, _)| self.get_domain(domain_id))
    }

    /// Get domain and category for a verb
    pub fn location_for_verb(&self, verb_fqn: &str) -> Option<VerbLocation> {
        self.indexes
            .verb_to_domain
            .get(verb_fqn)
            .and_then(|(domain_id, category_id)| {
                let domain = self.get_domain(domain_id)?;
                let category = domain.categories.iter().find(|c| &c.id == category_id)?;
                Some(VerbLocation {
                    domain_id: domain_id.clone(),
                    domain_label: domain.label.clone(),
                    category_id: category_id.clone(),
                    category_label: category.label.clone(),
                })
            })
    }

    /// Infer domain from phrase using taxonomy keywords
    ///
    /// This is more accurate than the hardcoded `infer_domain_from_phrase`
    /// because it uses the taxonomy's keyword definitions.
    pub fn infer_domain(&self, phrase: &str) -> Option<String> {
        let lower = phrase.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();

        // Check multi-word keywords first (more specific)
        for domain in &self.domains {
            for keyword in &domain.keywords {
                if keyword.contains(' ') && lower.contains(keyword) {
                    return Some(domain.id.clone());
                }
            }
        }

        // Then check single-word keywords
        for word in &words {
            if let Some(domain_id) = self.indexes.keyword_to_domain.get(*word) {
                return Some(domain_id.clone());
            }
        }

        None
    }

    /// Get domains sorted by priority
    pub fn domains_by_priority(&self) -> Vec<&TaxonomyDomain> {
        let mut domains: Vec<_> = self.domains.iter().collect();
        domains.sort_by_key(|d| d.priority);
        domains
    }

    /// Get all domains as UI-friendly list
    pub fn domain_list(&self) -> Vec<DomainSummary> {
        self.domains_by_priority()
            .iter()
            .map(|d| DomainSummary {
                id: d.id.clone(),
                label: d.label.clone(),
                description: d.description.clone(),
                icon: d.icon.clone(),
                category_count: d.categories.len(),
                verb_count: d.categories.iter().map(|c| c.verbs.len()).sum(),
            })
            .collect()
    }
}

/// A domain in the taxonomy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyDomain {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: u32,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub categories: Vec<TaxonomyCategory>,
}

fn default_priority() -> u32 {
    50
}

/// A category within a domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyCategory {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub verbs: Vec<String>,
}

/// Location of a verb in the taxonomy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbLocation {
    pub domain_id: String,
    pub domain_label: String,
    pub category_id: String,
    pub category_label: String,
}

/// Summary of a domain for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSummary {
    pub id: String,
    pub label: String,
    pub description: String,
    pub icon: Option<String>,
    pub category_count: usize,
    pub verb_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_taxonomy() -> VerbTaxonomy {
        let mut taxonomy = VerbTaxonomy {
            version: 1,
            generated: false,
            domains: vec![
                TaxonomyDomain {
                    id: "session".to_string(),
                    label: "Session & Navigation".to_string(),
                    description: "Load CBUs, navigate".to_string(),
                    icon: Some("compass".to_string()),
                    priority: 1,
                    keywords: vec![
                        "session".to_string(),
                        "load".to_string(),
                        "load the".to_string(),
                    ],
                    categories: vec![TaxonomyCategory {
                        id: "load".to_string(),
                        label: "Load CBUs".to_string(),
                        description: "Load CBUs into session".to_string(),
                        verbs: vec![
                            "session.load-cbu".to_string(),
                            "session.load-galaxy".to_string(),
                        ],
                    }],
                },
                TaxonomyDomain {
                    id: "view".to_string(),
                    label: "View".to_string(),
                    description: "Visualization".to_string(),
                    icon: Some("eye".to_string()),
                    priority: 2,
                    keywords: vec!["view".to_string(), "show".to_string()],
                    categories: vec![],
                },
            ],
            indexes: TaxonomyIndexes::default(),
        };
        taxonomy.build_indexes();
        taxonomy
    }

    #[test]
    fn test_get_domain() {
        let taxonomy = sample_taxonomy();

        let session = taxonomy.get_domain("session");
        assert!(session.is_some());
        assert_eq!(session.unwrap().label, "Session & Navigation");

        assert!(taxonomy.get_domain("nonexistent").is_none());
    }

    #[test]
    fn test_domain_for_verb() {
        let taxonomy = sample_taxonomy();

        let domain = taxonomy.domain_for_verb("session.load-galaxy");
        assert!(domain.is_some());
        assert_eq!(domain.unwrap().id, "session");

        assert!(taxonomy.domain_for_verb("unknown.verb").is_none());
    }

    #[test]
    fn test_location_for_verb() {
        let taxonomy = sample_taxonomy();

        let loc = taxonomy.location_for_verb("session.load-cbu");
        assert!(loc.is_some());
        let loc = loc.unwrap();
        assert_eq!(loc.domain_id, "session");
        assert_eq!(loc.category_id, "load");
        assert_eq!(loc.category_label, "Load CBUs");
    }

    #[test]
    fn test_infer_domain() {
        let taxonomy = sample_taxonomy();

        // Single keyword
        assert_eq!(
            taxonomy.infer_domain("load something"),
            Some("session".to_string())
        );

        // Multi-word keyword (higher priority)
        assert_eq!(
            taxonomy.infer_domain("load the allianz book"),
            Some("session".to_string())
        );

        // View keyword
        assert_eq!(
            taxonomy.infer_domain("show me stuff"),
            Some("view".to_string())
        );

        // No match
        assert_eq!(taxonomy.infer_domain("random gibberish"), None);
    }

    #[test]
    fn test_domains_by_priority() {
        let taxonomy = sample_taxonomy();
        let sorted = taxonomy.domains_by_priority();

        assert_eq!(sorted[0].id, "session"); // priority 1
        assert_eq!(sorted[1].id, "view"); // priority 2
    }

    #[test]
    fn test_domain_list() {
        let taxonomy = sample_taxonomy();
        let list = taxonomy.domain_list();

        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "session");
        assert_eq!(list[0].verb_count, 2);
        assert_eq!(list[0].category_count, 1);
    }
}
