//! Verb schema registry - loads and resolves verb specifications
//!
//! Handles:
//! - Loading schemas from YAML files
//! - Resolving verb aliases (with collision detection)
//! - Fuzzy search for verb discovery

use super::types::{SchemaFile, VerbSpec};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

/// Resolution result for a verb head (first token in s-expr)
#[derive(Debug, Clone)]
pub enum HeadResolution {
    /// Exact match on fully qualified name
    Exact(VerbSpec),

    /// Resolved via alias to a single verb
    Alias { alias: String, spec: VerbSpec },

    /// Alias matches multiple verbs - ambiguous
    Ambiguous {
        alias: String,
        candidates: Vec<VerbSpec>,
    },

    /// No match found
    NotFound {
        input: String,
        suggestions: Vec<String>,
    },
}

/// Registry of all verb schemas
#[derive(Debug, Default)]
pub struct VerbRegistry {
    /// Verbs by fully qualified name (domain.verb)
    verbs: HashMap<String, VerbSpec>,

    /// Alias → list of verb FQNs (may have collisions)
    aliases: HashMap<String, Vec<String>>,

    /// Domain → list of verb names
    domains: HashMap<String, Vec<String>>,
}

impl VerbRegistry {
    /// Create empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Load schemas from a directory (recursive)
    pub fn load(dir: &Path) -> Result<Self> {
        let mut registry = Self::new();

        if !dir.exists() {
            warn!("Schema directory does not exist: {:?}", dir);
            return Ok(registry);
        }

        registry.load_dir_recursive(dir)?;

        info!(
            "VerbRegistry loaded: {} verbs, {} aliases, {} domains",
            registry.verbs.len(),
            registry.aliases.len(),
            registry.domains.len()
        );

        Ok(registry)
    }

    /// Load schemas from verb YAML files (source format, not schema format)
    pub fn load_from_verbs(dir: &Path) -> Result<Self> {
        let mut registry = Self::new();

        if !dir.exists() {
            warn!("Verbs directory does not exist: {:?}", dir);
            return Ok(registry);
        }

        registry.load_dir_recursive(dir)?;

        info!(
            "VerbRegistry loaded from verbs: {} verbs, {} aliases, {} domains",
            registry.verbs.len(),
            registry.aliases.len(),
            registry.domains.len()
        );

        Ok(registry)
    }

    /// Recursively load YAML files from directory
    fn load_dir_recursive(&mut self, dir: &Path) -> Result<()> {
        for entry in std::fs::read_dir(dir).with_context(|| format!("Reading dir {:?}", dir))? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.load_dir_recursive(&path)?;
            } else if path
                .extension()
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false)
            {
                // Skip index and meta files
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('_') {
                        continue;
                    }
                }

                match self.load_file(&path) {
                    Ok(count) => debug!("Loaded {} verbs from {:?}", count, path),
                    Err(e) => warn!("Failed to load {:?}: {}", path, e),
                }
            }
        }
        Ok(())
    }

    /// Load a single schema file
    fn load_file(&mut self, path: &Path) -> Result<usize> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("Reading {:?}", path))?;

        let schema: SchemaFile =
            serde_yaml::from_str(&content).with_context(|| format!("Parsing {:?}", path))?;

        let mut count = 0;
        for (domain_name, domain) in schema.domains {
            for (verb_name, verb_content) in domain.verbs {
                let spec = verb_content.to_spec(&domain_name, &verb_name);
                self.register(spec);
                count += 1;
            }
        }

        Ok(count)
    }

    /// Register a verb spec
    pub fn register(&mut self, spec: VerbSpec) {
        let fqn = spec.name.clone();
        let domain = spec.domain.clone();

        // Collect all aliases (including verb name), deduplicating
        let mut all_aliases: Vec<String> = spec.aliases.iter().map(|a| a.to_lowercase()).collect();

        // Also register the short verb name as alias
        if let Some(verb_name) = fqn.split('.').next_back() {
            let verb_lower = verb_name.to_lowercase();
            if !all_aliases.contains(&verb_lower) {
                all_aliases.push(verb_lower);
            }
        }

        // Register aliases (with collision tracking)
        for alias_lower in all_aliases {
            let entry = self.aliases.entry(alias_lower).or_default();
            // Only add if not already present (avoid duplicates for same verb)
            if !entry.contains(&fqn) {
                entry.push(fqn.clone());
            }
        }

        // Track domain membership
        self.domains.entry(domain).or_default().push(fqn.clone());

        // Store spec
        self.verbs.insert(fqn, spec);
    }

    /// Get verb by fully qualified name
    pub fn get(&self, fqn: &str) -> Option<&VerbSpec> {
        self.verbs.get(fqn)
    }

    /// Resolve a verb head (first token in s-expr)
    pub fn resolve_head(&self, input: &str) -> HeadResolution {
        let input_lower = input.to_lowercase();

        // 1. Try exact FQN match
        if let Some(spec) = self.verbs.get(&input_lower) {
            return HeadResolution::Exact(spec.clone());
        }

        // Also try as-is (preserving case)
        if let Some(spec) = self.verbs.get(input) {
            return HeadResolution::Exact(spec.clone());
        }

        // 2. Try alias lookup
        if let Some(fqns) = self.aliases.get(&input_lower) {
            match fqns.len() {
                0 => {} // Fall through to not found
                1 => {
                    if let Some(spec) = self.verbs.get(&fqns[0]) {
                        return HeadResolution::Alias {
                            alias: input.to_string(),
                            spec: spec.clone(),
                        };
                    }
                }
                _ => {
                    // Multiple matches - ambiguous
                    let candidates: Vec<VerbSpec> = fqns
                        .iter()
                        .filter_map(|f| self.verbs.get(f).cloned())
                        .collect();

                    if !candidates.is_empty() {
                        return HeadResolution::Ambiguous {
                            alias: input.to_string(),
                            candidates,
                        };
                    }
                }
            }
        }

        // 3. Not found - generate suggestions
        let suggestions = self.suggest_similar(input, 5);
        HeadResolution::NotFound {
            input: input.to_string(),
            suggestions,
        }
    }

    /// Find similar verbs for suggestions
    fn suggest_similar(&self, input: &str, limit: usize) -> Vec<String> {
        let input_lower = input.to_lowercase();
        let mut scored: Vec<(String, usize)> = Vec::new();

        for fqn in self.verbs.keys() {
            let fqn_lower = fqn.to_lowercase();

            // Simple scoring: prefix match, contains, edit distance proxy
            let score = if fqn_lower.starts_with(&input_lower) {
                100
            } else if fqn_lower.contains(&input_lower) {
                50
            } else if let Some(verb) = fqn.split('.').next_back() {
                if verb.to_lowercase().starts_with(&input_lower) {
                    80
                } else if verb.to_lowercase().contains(&input_lower) {
                    40
                } else {
                    0
                }
            } else {
                0
            };

            if score > 0 {
                scored.push((fqn.clone(), score));
            }
        }

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().take(limit).map(|(s, _)| s).collect()
    }

    /// Fuzzy search across all verbs
    pub fn fuzzy_search(&self, query: &str, limit: usize) -> Vec<(VerbSpec, f32)> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(VerbSpec, f32)> = Vec::new();

        for spec in self.verbs.values() {
            let mut score: f32 = 0.0;

            // Match against FQN
            let fqn_lower = spec.name.to_lowercase();
            if fqn_lower.contains(&query_lower) {
                score += 0.8;
            }

            // Match against aliases
            for alias in &spec.aliases {
                let alias_lower = alias.to_lowercase();
                if alias_lower == query_lower {
                    score += 1.0;
                } else if alias_lower.contains(&query_lower) {
                    score += 0.6;
                }
            }

            // Match individual words against tags and domain
            for word in &query_words {
                if spec.domain.to_lowercase().contains(word) {
                    score += 0.3;
                }
                for tag in &spec.tags {
                    if tag.to_lowercase().contains(word) {
                        score += 0.2;
                    }
                }
            }

            if score > 0.0 {
                scored.push((spec.clone(), score));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(limit).collect()
    }

    /// Get all verbs
    pub fn all(&self) -> impl Iterator<Item = &VerbSpec> {
        self.verbs.values()
    }

    /// Number of registered verbs
    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    /// Get verbs for a domain
    pub fn domain_verbs(&self, domain: &str) -> Vec<&VerbSpec> {
        self.domains
            .get(domain)
            .map(|fqns| fqns.iter().filter_map(|f| self.verbs.get(f)).collect())
            .unwrap_or_default()
    }

    /// List all domains
    pub fn domain_names(&self) -> Vec<&String> {
        self.domains.keys().collect()
    }

    /// Get alias collision report
    pub fn alias_collisions(&self) -> Vec<(&String, &Vec<String>)> {
        self.aliases
            .iter()
            .filter(|(_, fqns)| fqns.len() > 1)
            .collect()
    }
}
