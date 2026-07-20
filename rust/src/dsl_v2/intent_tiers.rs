//! Intent Tier Taxonomy - Hierarchical clarification for ambiguous verb matches
//!
//! When semantic search returns candidates spanning multiple intents (navigate vs
//! create vs modify), we first ask the user to clarify their intent before showing
//! specific verbs. This reduces cognitive load and creates richer learning signals.
//!
//! ## Flow Example
//!
//! ```text
//! User: "load something"
//! → Tier 1: "What are you trying to do?" [Navigate, Create, Modify, Analyze]
//! → User picks "Navigate"
//! → Tier 2: "What scope?" [Single structure, Client book, Jurisdiction]
//! → User picks "Client book"
//! → Verb options: [session.load-galaxy, session.load-cluster]
//! ```
//!
//! Loaded from `config/verb_schemas/intent_tiers.yaml`

use ob_poc_types::{IntentTierOption, IntentTierRequest, IntentTierSelection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use uuid::Uuid;

/// Global intent tier taxonomy (lazy-loaded)
static INTENT_TIERS: OnceLock<IntentTierTaxonomy> = OnceLock::new();


/// Load intent tier taxonomy from config file
fn load_intent_tiers_from_config() -> anyhow::Result<IntentTierTaxonomy> {
    let config_path = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
    let tiers_path = format!("{}/verb_schemas/intent_tiers.yaml", config_path);

    let content = std::fs::read_to_string(&tiers_path)
        .map_err(|e| anyhow::anyhow!("Failed to read intent tiers file {}: {}", tiers_path, e))?;

    let mut taxonomy: IntentTierTaxonomy = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse intent tiers YAML: {}", e))?;

    taxonomy.build_indexes();

    tracing::info!(
        tier1_options = taxonomy.tier_1.options.len(),
        tier2_intents = taxonomy.tier_2.len(),
        verb_mappings = taxonomy.verb_tier_mapping.len(),
        "Loaded intent tier taxonomy"
    );

    Ok(taxonomy)
}

/// Root intent tier taxonomy structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct IntentTierTaxonomy {
    /// Tier 1: Primary action intent (navigate, create, modify, analyze, workflow)
    pub tier_1: Tier1Config,

    /// Tier 2: Domain/scope options per tier 1 selection
    #[serde(default)]
    pub tier_2: HashMap<String, Tier2Config>,

    /// Verb → [tier1_id, tier2_id] mapping for reverse lookup
    #[serde(default)]
    pub verb_tier_mapping: HashMap<String, Vec<String>>,

    /// Thresholds for disambiguation behavior
    #[serde(default)]
    pub thresholds: TierThresholds,

    /// Indexes built after loading
    #[serde(skip)]
    indexes: TierIndexes,
}

/// Tier 1 configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Tier1Config {
    pub id: String,
    pub prompt: String,
    #[serde(default)]
    pub options: Vec<Tier1Option>,
}

/// A tier 1 option (action intent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Tier1Option {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub hint: Option<String>,
}

/// Tier 2 configuration for a specific tier 1 intent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Tier2Config {
    pub id: String,
    pub prompt: String,
    #[serde(default)]
    pub options: Vec<Tier2Option>,
}

/// A tier 2 option (scope/domain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Tier2Option {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(default)]
    pub verbs: Vec<String>,
}

/// Thresholds for tier disambiguation behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TierThresholds {
    /// If candidates span more than this many tier 1 intents, show tier 1 first
    #[serde(default = "default_multi_intent_threshold")]
    pub multi_intent_threshold: usize,

    /// If a single tier 1 has more than this many tier 2 options represented, show tier 2
    #[serde(default = "default_multi_scope_threshold")]
    pub multi_scope_threshold: usize,

    /// Skip tiers if top candidate score is above this (very confident match)
    #[serde(default = "default_skip_tiers_confidence")]
    pub skip_tiers_confidence: f32,
}

impl Default for TierThresholds {
    fn default() -> Self {
        Self {
            multi_intent_threshold: default_multi_intent_threshold(),
            multi_scope_threshold: default_multi_scope_threshold(),
            skip_tiers_confidence: default_skip_tiers_confidence(),
        }
    }
}

fn default_multi_intent_threshold() -> usize {
    1
}
fn default_multi_scope_threshold() -> usize {
    1
}
fn default_skip_tiers_confidence() -> f32 {
    0.92
}

/// Indexes for fast lookup
#[derive(Debug, Clone, Default)]
struct TierIndexes {
    /// keyword → tier1 option id
    keyword_to_tier1: HashMap<String, String>,
    /// verb → (tier1_id, tier2_id)
    verb_to_tiers: HashMap<String, (String, String)>,
    /// tier1_id → set of verbs
    tier1_verbs: HashMap<String, HashSet<String>>,
    /// (tier1_id, tier2_id) → set of verbs
    tier2_verbs: HashMap<(String, String), HashSet<String>>,
}

impl IntentTierTaxonomy {
    /// Build indexes after loading
    pub(crate) fn build_indexes(&mut self) {
        let mut indexes = TierIndexes::default();

        // Index tier 1 keywords
        for opt in &self.tier_1.options {
            for keyword in &opt.keywords {
                indexes
                    .keyword_to_tier1
                    .insert(keyword.to_lowercase(), opt.id.clone());
            }
        }

        // Index verb mappings
        for (verb, tiers) in &self.verb_tier_mapping {
            if tiers.len() >= 2 {
                let tier1_id = &tiers[0];
                let tier2_id = &tiers[1];

                indexes
                    .verb_to_tiers
                    .insert(verb.clone(), (tier1_id.clone(), tier2_id.clone()));

                indexes
                    .tier1_verbs
                    .entry(tier1_id.clone())
                    .or_default()
                    .insert(verb.clone());

                indexes
                    .tier2_verbs
                    .entry((tier1_id.clone(), tier2_id.clone()))
                    .or_default()
                    .insert(verb.clone());
            }
        }

        self.indexes = indexes;
    }

    /// Get the tier mapping for a verb
    pub(crate) fn get_verb_tiers(&self, verb: &str) -> Option<(&str, &str)> {
        self.indexes
            .verb_to_tiers
            .get(verb)
            .map(|(t1, t2)| (t1.as_str(), t2.as_str()))
    }

    /// Analyze which tiers are represented in a set of candidate verbs
    pub(crate) fn analyze_candidates(&self, verbs: &[&str]) -> TierAnalysis {
        let mut tier1_intents: HashMap<String, Vec<String>> = HashMap::new();
        let mut tier2_scopes: HashMap<(String, String), Vec<String>> = HashMap::new();
        let mut unmapped_verbs: Vec<String> = Vec::new();

        for verb in verbs {
            if let Some((tier1, tier2)) = self.get_verb_tiers(verb) {
                tier1_intents
                    .entry(tier1.to_string())
                    .or_default()
                    .push(verb.to_string());

                tier2_scopes
                    .entry((tier1.to_string(), tier2.to_string()))
                    .or_default()
                    .push(verb.to_string());
            } else {
                unmapped_verbs.push(verb.to_string());
            }
        }

        let spans_multiple_intents = tier1_intents.len() > self.thresholds.multi_intent_threshold;

        // Check if any single intent has multiple scopes
        let needs_scope_clarification = tier1_intents.iter().any(|(tier1, _)| {
            let scope_count = tier2_scopes.keys().filter(|(t1, _)| t1 == tier1).count();
            scope_count > self.thresholds.multi_scope_threshold
        });

        TierAnalysis {
            spans_multiple_intents,
            needs_scope_clarification,
            tier1_intents,
            tier2_scopes,
            unmapped_verbs,
        }
    }



    /// Get verbs matching a tier selection path
    pub(crate) fn get_matching_verbs(
        &self,
        tier1_selection: &str,
        tier2_selection: Option<&str>,
    ) -> Vec<String> {
        if let Some(tier2) = tier2_selection {
            // Return verbs for specific tier2
            self.indexes
                .tier2_verbs
                .get(&(tier1_selection.to_string(), tier2.to_string()))
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect()
        } else {
            // Return all verbs for tier1
            self.indexes
                .tier1_verbs
                .get(tier1_selection)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect()
        }
    }

}

/// Analysis of which tiers are represented in candidate verbs
#[derive(Debug, Clone)]
pub(crate) struct TierAnalysis {
    /// Whether candidates span multiple tier 1 intents
    pub spans_multiple_intents: bool,
    /// Whether scope clarification (tier 2) is needed within a single intent
    pub needs_scope_clarification: bool,
    /// Tier 1 intent → list of matching verbs
    pub tier1_intents: HashMap<String, Vec<String>>,
    /// (Tier1, Tier2) → list of matching verbs
    pub tier2_scopes: HashMap<(String, String), Vec<String>>,
    /// Verbs without tier mapping
    pub unmapped_verbs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_intent_tiers() {
        // This test requires the config file to exist
        if let Ok(taxonomy) = load_intent_tiers_from_config() {
            assert!(!taxonomy.tier_1.options.is_empty());
            assert!(!taxonomy.verb_tier_mapping.is_empty());
        }
    }

    #[test]
    fn test_analyze_candidates() {
        let mut taxonomy = IntentTierTaxonomy::default();
        taxonomy.verb_tier_mapping.insert(
            "session.load-galaxy".to_string(),
            vec!["navigate".to_string(), "client_book".to_string()],
        );
        taxonomy.verb_tier_mapping.insert(
            "cbu.create".to_string(),
            vec!["create".to_string(), "structure".to_string()],
        );
        taxonomy.build_indexes();

        let candidates = vec!["session.load-galaxy", "cbu.create"];
        let analysis = taxonomy.analyze_candidates(&candidates);

        assert!(analysis.spans_multiple_intents);
        assert_eq!(analysis.tier1_intents.len(), 2);
        assert!(analysis.tier1_intents.contains_key("navigate"));
        assert!(analysis.tier1_intents.contains_key("create"));
    }
}
