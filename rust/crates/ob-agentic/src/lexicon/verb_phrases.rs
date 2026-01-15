//! Verb phrase matching using invocation_phrases from verb YAML configs.
//!
//! This module loads invocation phrases from DSL verb definitions and
//! provides phrase-based intent matching as a fallback/enhancement to
//! the grammar parser.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ob_agentic::lexicon::verb_phrases::VerbPhraseIndex;
//!
//! let index = VerbPhraseIndex::load_from_verbs_dir("rust/config/verbs")?;
//!
//! // Find matching verbs for a user phrase
//! let matches = index.find_matches("add counterparty");
//! // Returns: [("trading-profile.add-isda-config", 0.9), ...]
//! ```

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

/// Index of verb invocation phrases for intent matching.
#[derive(Debug, Clone, Default)]
pub struct VerbPhraseIndex {
    /// Map from normalized phrase to (domain.verb, score_weight)
    phrase_to_verb: HashMap<String, Vec<VerbMatch>>,

    /// Domain hints: keyword -> domain
    domain_hints: HashMap<String, String>,

    /// All registered verbs with their phrases
    verb_phrases: HashMap<String, VerbEntry>,
}

/// A verb entry with its invocation phrases
#[derive(Debug, Clone)]
pub struct VerbEntry {
    /// Fully qualified verb name: "domain.verb-name"
    pub fq_name: String,
    /// Domain name
    pub domain: String,
    /// Verb name
    pub verb: String,
    /// Description from YAML
    pub description: String,
    /// Invocation phrases
    pub phrases: Vec<String>,
    /// Whether this is an internal verb
    pub internal: bool,
}

/// A potential verb match
#[derive(Debug, Clone)]
pub struct VerbMatch {
    /// Fully qualified verb name
    pub fq_name: String,
    /// Match confidence (0.0 - 1.0)
    pub confidence: f32,
    /// The phrase that matched
    pub matched_phrase: String,
}

impl VerbPhraseIndex {
    /// Create an empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load verb phrases from a verbs directory.
    ///
    /// Recursively scans for YAML files and extracts invocation_phrases.
    pub fn load_from_verbs_dir(verbs_dir: impl AsRef<Path>) -> Result<Self> {
        let mut index = Self::new();
        index.scan_directory(verbs_dir.as_ref())?;
        Ok(index)
    }

    /// Scan a directory recursively for verb YAML files.
    fn scan_directory(&mut self, dir: &Path) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_directory(&path)?;
            } else if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                if let Err(e) = self.load_yaml_file(&path) {
                    // Log but don't fail on individual file errors
                    tracing::warn!("Failed to load verb file {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// Load a single YAML verb file.
    fn load_yaml_file(&mut self, path: &Path) -> Result<()> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("Failed to read {:?}", path))?;

        // Parse as generic YAML to extract domains
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {:?}", path))?;

        // Extract domains
        if let Some(domains) = yaml.get("domains").and_then(|d| d.as_mapping()) {
            for (domain_name, domain_config) in domains {
                let domain_name = domain_name.as_str().unwrap_or_default();

                // Extract domain-level invocation_hints
                if let Some(hints) = domain_config
                    .get("invocation_hints")
                    .and_then(|h| h.as_sequence())
                {
                    for hint in hints {
                        if let Some(hint_str) = hint.as_str() {
                            self.domain_hints
                                .insert(hint_str.to_lowercase(), domain_name.to_string());
                        }
                    }
                }

                // Extract verb-level invocation_phrases
                if let Some(verbs) = domain_config.get("verbs").and_then(|v| v.as_mapping()) {
                    for (verb_name, verb_config) in verbs {
                        let verb_name = verb_name.as_str().unwrap_or_default();
                        let fq_name = format!("{}.{}", domain_name, verb_name);

                        // Check if internal
                        let internal = verb_config
                            .get("metadata")
                            .and_then(|m| m.get("internal"))
                            .and_then(|i| i.as_bool())
                            .unwrap_or(false);

                        // Skip internal verbs for phrase matching
                        if internal {
                            continue;
                        }

                        let description = verb_config
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or_default()
                            .to_string();

                        // Extract invocation_phrases
                        let phrases: Vec<String> = verb_config
                            .get("invocation_phrases")
                            .and_then(|p| p.as_sequence())
                            .map(|seq| {
                                seq.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default();

                        if !phrases.is_empty() {
                            // Register the verb entry
                            self.verb_phrases.insert(
                                fq_name.clone(),
                                VerbEntry {
                                    fq_name: fq_name.clone(),
                                    domain: domain_name.to_string(),
                                    verb: verb_name.to_string(),
                                    description,
                                    phrases: phrases.clone(),
                                    internal,
                                },
                            );

                            // Index each phrase
                            for phrase in &phrases {
                                let normalized = phrase.to_lowercase();
                                self.phrase_to_verb
                                    .entry(normalized.clone())
                                    .or_default()
                                    .push(VerbMatch {
                                        fq_name: fq_name.clone(),
                                        confidence: 1.0,
                                        matched_phrase: phrase.clone(),
                                    });
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Find verbs matching a user phrase.
    ///
    /// Returns matches sorted by confidence (highest first).
    pub fn find_matches(&self, input: &str) -> Vec<VerbMatch> {
        let normalized = input.to_lowercase();
        let mut matches = Vec::new();

        // Exact phrase match
        if let Some(verb_matches) = self.phrase_to_verb.get(&normalized) {
            for m in verb_matches {
                matches.push(VerbMatch {
                    fq_name: m.fq_name.clone(),
                    confidence: 1.0,
                    matched_phrase: m.matched_phrase.clone(),
                });
            }
        }

        // Substring match (lower confidence)
        for (phrase, verb_matches) in &self.phrase_to_verb {
            if normalized.contains(phrase) && phrase != &normalized {
                for m in verb_matches {
                    // Calculate confidence based on phrase coverage
                    let coverage = phrase.len() as f32 / normalized.len() as f32;
                    matches.push(VerbMatch {
                        fq_name: m.fq_name.clone(),
                        confidence: coverage * 0.8, // Substring match penalty
                        matched_phrase: m.matched_phrase.clone(),
                    });
                }
            }
        }

        // Deduplicate by fq_name, keeping highest confidence
        let mut seen: HashMap<String, VerbMatch> = HashMap::new();
        for m in matches {
            seen.entry(m.fq_name.clone())
                .and_modify(|existing| {
                    if m.confidence > existing.confidence {
                        *existing = m.clone();
                    }
                })
                .or_insert(m);
        }

        let mut result: Vec<VerbMatch> = seen.into_values().collect();
        result.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        result
    }

    /// Get domain hint for a keyword.
    pub fn get_domain_hint(&self, keyword: &str) -> Option<&String> {
        self.domain_hints.get(&keyword.to_lowercase())
    }

    /// Get a verb entry by fully qualified name.
    pub fn get_verb(&self, fq_name: &str) -> Option<&VerbEntry> {
        self.verb_phrases.get(fq_name)
    }

    /// Get all registered verbs.
    pub fn all_verbs(&self) -> impl Iterator<Item = &VerbEntry> {
        self.verb_phrases.values()
    }

    /// Get statistics about the index.
    pub fn stats(&self) -> VerbPhraseStats {
        VerbPhraseStats {
            total_verbs: self.verb_phrases.len(),
            total_phrases: self.phrase_to_verb.len(),
            total_domain_hints: self.domain_hints.len(),
        }
    }
}

/// Statistics about the verb phrase index.
#[derive(Debug, Clone)]
pub struct VerbPhraseStats {
    pub total_verbs: usize,
    pub total_phrases: usize,
    pub total_domain_hints: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_index() {
        let index = VerbPhraseIndex::new();
        assert!(index.find_matches("add counterparty").is_empty());
    }

    #[test]
    fn test_load_from_verbs_dir() {
        // This test requires the actual config directory
        let verbs_dir = std::path::Path::new("../../config/verbs");
        if verbs_dir.exists() {
            let index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir).unwrap();
            let stats = index.stats();
            println!(
                "Loaded {} verbs with {} phrases",
                stats.total_verbs, stats.total_phrases
            );

            // Test finding matches
            let matches = index.find_matches("add ISDA");
            println!("Matches for 'add ISDA': {:?}", matches);
        }
    }
}
