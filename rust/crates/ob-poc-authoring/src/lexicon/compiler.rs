//! LexiconCompiler - Build lexicon snapshot from YAML config files.
//!
//! The compiler reads YAML config files and verb schemas, then builds
//! the indexed LexiconSnapshot for runtime use.
//!
//! ## Build Process
//!
//! 1. Load lexicon YAML files (verb_concepts, entity_types, domains)
//! 2. Load verb schemas for invocation phrases
//! 3. Build label→concept and token→concept indexes
//! 4. Compute deterministic hash from inputs
//! 5. Save binary snapshot
//!
//! ## Usage
//!
//! ```rust,ignore
//! let compiler = LexiconCompiler::new(Path::new("rust/config"));
//! let snapshot = compiler.build()?;
//! snapshot.save_binary(Path::new("rust/assets/lexicon.snapshot.bin"))?;
//! ```

use sha2::{Digest, Sha256};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::snapshot::LexiconSnapshot;
use super::types::*;

/// Compiler for building LexiconSnapshot from YAML config files.
pub struct LexiconCompiler {
    config_root: PathBuf,
}

impl LexiconCompiler {
    /// Create a new compiler with the given config root directory.
    pub fn new(config_root: &Path) -> Self {
        Self {
            config_root: config_root.to_path_buf(),
        }
    }

    /// Build the lexicon snapshot from config files.
    ///
    /// If YAML files are missing, returns an empty snapshot with a warning
    /// (does NOT panic or return error - this allows gradual adoption).
    pub fn build(&self) -> anyhow::Result<LexiconSnapshot> {
        let mut label_to_concepts: HashMap<String, SmallVec<[String; 4]>> = HashMap::new();
        let mut token_to_concepts: HashMap<String, SmallVec<[String; 8]>> = HashMap::new();
        let mut verb_meta: HashMap<String, VerbMeta> = HashMap::new();
        let mut entity_types: HashMap<String, EntityTypeMeta> = HashMap::new();
        let mut domains: HashMap<String, DomainMeta> = HashMap::new();
        let mut keyword_to_domain: HashMap<String, String> = HashMap::new();

        // Try to load verb_concepts.yaml
        let verb_concepts_path = self.config_root.join("lexicon/verb_concepts.yaml");
        if verb_concepts_path.exists() {
            self.load_verb_concepts(
                &verb_concepts_path,
                &mut verb_meta,
                &mut label_to_concepts,
                &mut token_to_concepts,
            )?;
        } else {
            tracing::warn!(
                path = %verb_concepts_path.display(),
                "verb_concepts.yaml not found, lexicon will be empty"
            );
        }

        // Try to load entity_types.yaml
        let entity_types_path = self.config_root.join("lexicon/entity_types.yaml");
        if entity_types_path.exists() {
            self.load_entity_types(
                &entity_types_path,
                &mut entity_types,
                &mut label_to_concepts,
            )?;
        } else {
            tracing::warn!(
                path = %entity_types_path.display(),
                "entity_types.yaml not found"
            );
        }

        // Try to load domains.yaml
        let domains_path = self.config_root.join("lexicon/domains.yaml");
        if domains_path.exists() {
            self.load_domains(&domains_path, &mut domains, &mut keyword_to_domain)?;
        } else {
            tracing::warn!(path = %domains_path.display(), "domains.yaml not found");
        }

        // Compute deterministic hash
        let hash = self.compute_hash()?;

        let snapshot = LexiconSnapshot {
            hash,
            version: "1.0.0".to_string(),
            label_to_concepts,
            token_to_concepts,
            verb_meta,
            entity_types,
            domains,
            keyword_to_domain,
        };

        Ok(snapshot)
    }

    /// Load verb concepts from YAML.
    fn load_verb_concepts(
        &self,
        path: &Path,
        verb_meta: &mut HashMap<String, VerbMeta>,
        label_to_concepts: &mut HashMap<String, SmallVec<[String; 4]>>,
        token_to_concepts: &mut HashMap<String, SmallVec<[String; 8]>>,
    ) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(path)?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

        // Supports two formats:
        // 1. Wrapped: verbs: { cbu.create: {...} }
        // 2. Top-level: cbu.create: {...}
        let verbs = yaml
            .get("verbs")
            .and_then(|v| v.as_mapping())
            .or_else(|| yaml.as_mapping());

        if let Some(verbs) = verbs {
            for (verb_key, verb_value) in verbs {
                if let Some(dsl_verb) = verb_key.as_str() {
                    let meta = self.parse_verb_meta(dsl_verb, verb_value);

                    // Index by pref_label
                    let label_norm = normalize(&meta.pref_label);
                    if !label_norm.is_empty() {
                        add_to_label_index(
                            label_to_concepts,
                            &label_norm,
                            &format!("verb.{}", dsl_verb),
                        );
                    }

                    // Index by alt_labels
                    for alt in &meta.alt_labels {
                        let alt_norm = normalize(alt);
                        if !alt_norm.is_empty() {
                            add_to_label_index(
                                label_to_concepts,
                                &alt_norm,
                                &format!("verb.{}", dsl_verb),
                            );
                        }
                    }

                    // Index by invocation_phrases
                    for phrase in &meta.invocation_phrases {
                        let phrase_norm = normalize(phrase);
                        if !phrase_norm.is_empty() {
                            add_to_label_index(
                                label_to_concepts,
                                &phrase_norm,
                                &format!("verb.{}", dsl_verb),
                            );
                        }
                    }

                    // Index individual tokens (from pref_label and alt_labels)
                    for token in extract_tokens(&meta.pref_label) {
                        add_to_token_index(
                            token_to_concepts,
                            &token,
                            &format!("verb.{}", dsl_verb),
                        );
                    }
                    for alt in &meta.alt_labels {
                        for token in extract_tokens(alt) {
                            add_to_token_index(
                                token_to_concepts,
                                &token,
                                &format!("verb.{}", dsl_verb),
                            );
                        }
                    }

                    verb_meta.insert(dsl_verb.to_string(), meta);
                }
            }
        }

        Ok(())
    }

    /// Parse verb metadata from YAML value.
    fn parse_verb_meta(&self, dsl_verb: &str, value: &serde_yaml::Value) -> VerbMeta {
        VerbMeta {
            dsl_verb: dsl_verb.to_string(),
            pref_label: value
                .get("pref_label")
                .and_then(|v| v.as_str())
                .unwrap_or(dsl_verb)
                .to_string(),
            domain: value
                .get("domain")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            target_types: value
                .get("target_types")
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            produces_type: value
                .get("produces_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            crud_type: value
                .get("crud_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            invocation_phrases: value
                .get("invocation_phrases")
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            alt_labels: value
                .get("alt_labels")
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Load entity types from YAML.
    fn load_entity_types(
        &self,
        path: &Path,
        entity_types: &mut HashMap<String, EntityTypeMeta>,
        label_to_concepts: &mut HashMap<String, SmallVec<[String; 4]>>,
    ) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(path)?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

        // Supports two formats:
        // 1. Wrapped: entity_types: { fund: {...} }
        // 2. Top-level: fund: {...}
        let types = yaml
            .get("entity_types")
            .and_then(|v| v.as_mapping())
            .or_else(|| yaml.as_mapping());

        if let Some(types) = types {
            for (type_key, type_value) in types {
                if let Some(type_name) = type_key.as_str() {
                    let meta = EntityTypeMeta {
                        type_name: type_name.to_string(),
                        pref_label: type_value
                            .get("pref_label")
                            .and_then(|v| v.as_str())
                            .unwrap_or(type_name)
                            .to_string(),
                        aliases: type_value
                            .get("aliases")
                            .and_then(|v| v.as_sequence())
                            .map(|seq| {
                                seq.iter()
                                    .filter_map(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .collect()
                            })
                            .unwrap_or_default(),
                        domain: type_value
                            .get("domain")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    };

                    // Index by pref_label
                    let label_norm = normalize(&meta.pref_label);
                    if !label_norm.is_empty() {
                        add_to_label_index(
                            label_to_concepts,
                            &label_norm,
                            &format!("entity_type.{}", type_name),
                        );
                    }

                    // Index by aliases
                    for alias in &meta.aliases {
                        let alias_norm = normalize(alias);
                        if !alias_norm.is_empty() {
                            add_to_label_index(
                                label_to_concepts,
                                &alias_norm,
                                &format!("entity_type.{}", type_name),
                            );
                        }
                    }

                    entity_types.insert(type_name.to_string(), meta);
                }
            }
        }

        Ok(())
    }

    /// Load domains from YAML.
    fn load_domains(
        &self,
        path: &Path,
        domains: &mut HashMap<String, DomainMeta>,
        keyword_to_domain: &mut HashMap<String, String>,
    ) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(path)?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

        // Supports two formats:
        // 1. Wrapped: domains: { cbu: {...} }
        // 2. Top-level: cbu: {...}
        let doms = yaml
            .get("domains")
            .and_then(|v| v.as_mapping())
            .or_else(|| yaml.as_mapping());

        if let Some(doms) = doms {
            for (domain_key, domain_value) in doms {
                if let Some(domain_id) = domain_key.as_str() {
                    let meta = DomainMeta {
                        domain_id: domain_id.to_string(),
                        label: domain_value
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or(domain_id)
                            .to_string(),
                        parent: domain_value
                            .get("parent")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        inference_keywords: domain_value
                            .get("inference_keywords")
                            .and_then(|v| v.as_sequence())
                            .map(|seq| {
                                seq.iter()
                                    .filter_map(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .collect()
                            })
                            .unwrap_or_default(),
                    };

                    // Add inference keywords to keyword→domain map
                    for keyword in &meta.inference_keywords {
                        let keyword_norm = normalize(keyword);
                        if !keyword_norm.is_empty() {
                            keyword_to_domain.insert(keyword_norm, domain_id.to_string());
                        }
                    }

                    domains.insert(domain_id.to_string(), meta);
                }
            }
        }

        Ok(())
    }

    /// Compute deterministic hash from input YAML files.
    fn compute_hash(&self) -> anyhow::Result<String> {
        let mut hasher = Sha256::new();
        hasher.update(b"lexicon_snapshot_v1");

        // Hash contents of each YAML file (in deterministic order)
        for rel in [
            "lexicon/verb_concepts.yaml",
            "lexicon/entity_types.yaml",
            "lexicon/domains.yaml",
            "lexicon/schemes.yaml",
        ] {
            let path = self.config_root.join(rel);
            if path.exists() {
                let content = std::fs::read(&path)?;
                hasher.update(&content);
            }
        }

        Ok(format!("{:x}", hasher.finalize()))
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Normalize a string for lookup: lowercase, collapse whitespace.
fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract tokens from a string (for token index).
fn extract_tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split_whitespace()
        .filter(|t| t.len() >= 2) // Skip single-char tokens
        .map(|t| t.to_string())
        .collect()
}

/// Add a concept to the label→concepts index (bounded to 4).
fn add_to_label_index(
    index: &mut HashMap<String, SmallVec<[String; 4]>>,
    label: &str,
    concept_id: &str,
) {
    let entry = index.entry(label.to_string()).or_default();
    if entry.len() < 4 && !entry.contains(&concept_id.to_string()) {
        entry.push(concept_id.to_string());
    }
}

/// Add a concept to the token→concepts index (bounded to 8).
fn add_to_token_index(
    index: &mut HashMap<String, SmallVec<[String; 8]>>,
    token: &str,
    concept_id: &str,
) {
    let entry = index.entry(token.to_string()).or_default();
    if entry.len() < 8 && !entry.contains(&concept_id.to_string()) {
        entry.push(concept_id.to_string());
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("  Create  CBU  "), "create cbu");
        assert_eq!(normalize("SPIN UP"), "spin up");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn test_extract_tokens() {
        let tokens = extract_tokens("Create a new CBU");
        assert_eq!(tokens, vec!["create", "new", "cbu"]); // "a" filtered (single char)
    }

    #[test]
    fn test_build_empty_config() {
        let dir = tempdir().unwrap();
        let compiler = LexiconCompiler::new(dir.path());
        let snapshot = compiler.build().unwrap();

        // Should succeed with empty snapshot
        assert!(snapshot.verb_meta.is_empty());
        assert!(snapshot.label_to_concepts.is_empty());
    }

    #[test]
    fn test_build_with_verb_concepts() {
        let dir = tempdir().unwrap();
        let lexicon_dir = dir.path().join("lexicon");
        std::fs::create_dir_all(&lexicon_dir).unwrap();

        let verb_yaml = r#"
verbs:
  cbu.create:
    pref_label: "Create CBU"
    alt_labels:
      - "spin up"
      - "new fund"
    domain: "cbu"
    target_types:
      - "fund"
    crud_type: "create"
    invocation_phrases:
      - "create a cbu"
      - "spin up a fund"
"#;
        std::fs::write(lexicon_dir.join("verb_concepts.yaml"), verb_yaml).unwrap();

        let compiler = LexiconCompiler::new(dir.path());
        let snapshot = compiler.build().unwrap();

        // Should have verb metadata
        assert!(snapshot.verb_meta.contains_key("cbu.create"));
        let meta = snapshot.verb_meta.get("cbu.create").unwrap();
        assert_eq!(meta.pref_label, "Create CBU");
        assert_eq!(meta.domain, Some("cbu".to_string()));

        // Should have label indexes
        assert!(snapshot.label_to_concepts.contains_key("create cbu"));
        assert!(snapshot.label_to_concepts.contains_key("spin up"));

        // Should have token indexes
        assert!(snapshot.token_to_concepts.contains_key("create"));
        assert!(snapshot.token_to_concepts.contains_key("cbu"));
    }

    #[test]
    fn test_hash_is_deterministic() {
        let dir = tempdir().unwrap();
        let lexicon_dir = dir.path().join("lexicon");
        std::fs::create_dir_all(&lexicon_dir).unwrap();

        let verb_yaml = "verbs:\n  test.verb:\n    pref_label: Test\n";
        std::fs::write(lexicon_dir.join("verb_concepts.yaml"), verb_yaml).unwrap();

        let compiler = LexiconCompiler::new(dir.path());
        let snapshot1 = compiler.build().unwrap();
        let snapshot2 = compiler.build().unwrap();

        assert_eq!(
            snapshot1.hash, snapshot2.hash,
            "Hash should be deterministic"
        );
    }
}
