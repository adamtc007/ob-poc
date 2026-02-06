//! VerbConfigIndex — Read-Only Verb Projection
//!
//! Provides a flat, indexed view of verb configurations for use by the
//! v2 REPL orchestrator, sentence generator, and template instantiation.
//!
//! Instead of passing loose `HashMap<String, Vec<String>>` for invocation
//! phrases and `HashMap<String, String>` for descriptions, consumers use
//! `VerbConfigIndex` for structured access to verb metadata.
//!
//! # Phase 2: YAML Sentences + Hardcoded Fallback
//!
//! Sentence templates are sourced with priority:
//! 1. YAML `VerbConfig.sentences.step[]` (highest — for verbs that exist in YAML)
//! 2. Hardcoded `pack_verb_sentence_templates()` (for pack-only FQNs that don't exist in YAML)
//! 3. Empty (sentence_gen falls back to invocation_phrases)
//!
//! The hardcoded map shrinks as pack FQN alignment improves in later phases.

use std::collections::HashMap;

use dsl_core::config::types::{ConfirmPolicyConfig, VerbSentences, VerbsConfig};

use super::runbook::ConfirmPolicy;

// ---------------------------------------------------------------------------
// VerbConfigIndex
// ---------------------------------------------------------------------------

/// Read-only index over verb configuration for the v2 REPL pipeline.
///
/// Built once from `VerbsConfig` at startup, then shared via `Arc`.
#[derive(Debug, Clone)]
pub struct VerbConfigIndex {
    entries: HashMap<String, VerbIndexEntry>,
}

/// Summary of a single verb's configuration.
#[derive(Debug, Clone)]
pub struct VerbIndexEntry {
    /// Fully-qualified verb name: "domain.action" (e.g. "cbu.assign-product").
    pub fqn: String,
    /// Human-readable description.
    pub description: String,
    /// Invocation phrases from YAML (for semantic search / sentence gen fallback).
    pub invocation_phrases: Vec<String>,
    /// Sentence templates — Phase 2: from VerbConfig YAML, fallback to hardcoded pack templates.
    pub sentence_templates: Vec<String>,
    /// Full VerbSentences from YAML (Phase 2) — includes step, summary, clarify, completed.
    pub sentences: Option<VerbSentences>,
    /// Argument metadata.
    pub args: Vec<ArgSummary>,
    /// Confirm policy for this verb.
    pub confirm_policy: ConfirmPolicy,
}

/// Compact argument summary for display and validation.
#[derive(Debug, Clone)]
pub struct ArgSummary {
    pub name: String,
    pub arg_type: String,
    pub required: bool,
    pub description: Option<String>,
}

impl VerbConfigIndex {
    /// Create an empty index (for tests / Phase 0 fallback).
    pub fn empty() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Build index from VerbsConfig.
    ///
    /// Merges sentence templates with priority:
    /// 1. YAML `verb_config.sentences.step[]` (highest — Phase 2)
    /// 2. Hardcoded `pack_verb_sentence_templates()` (for pack-only FQNs)
    /// 3. Empty (fallback — sentence_gen uses invocation_phrases)
    ///
    /// Merges confirm policy with priority:
    /// 1. YAML `verb_config.confirm_policy` (highest — Phase 2)
    /// 2. Hardcoded `pack_verb_confirm_policies()`
    /// 3. `ConfirmPolicy::Always` (default)
    pub fn from_verbs_config(config: &VerbsConfig) -> Self {
        let hardcoded_templates = pack_verb_sentence_templates();
        let hardcoded_policies = pack_verb_confirm_policies();

        let mut entries = HashMap::new();

        for (domain_name, domain) in &config.domains {
            for (verb_name, verb_config) in &domain.verbs {
                let fqn = format!("{}.{}", domain_name, verb_name);

                // Sentence templates: prefer YAML sentences.step[], fall back to hardcoded
                let yaml_sentences = verb_config.sentences.clone();
                let templates = if let Some(ref s) = yaml_sentences {
                    if !s.step.is_empty() {
                        s.step.clone()
                    } else {
                        hardcoded_templates.get(&fqn).cloned().unwrap_or_default()
                    }
                } else {
                    hardcoded_templates.get(&fqn).cloned().unwrap_or_default()
                };

                // Confirm policy: prefer YAML, fall back to hardcoded
                let policy = verb_config
                    .confirm_policy
                    .map(|cp| match cp {
                        ConfirmPolicyConfig::Always => ConfirmPolicy::Always,
                        ConfirmPolicyConfig::QuickConfirm => ConfirmPolicy::QuickConfirm,
                        ConfirmPolicyConfig::PackConfigured => ConfirmPolicy::PackConfigured,
                    })
                    .or_else(|| hardcoded_policies.get(&fqn).copied())
                    .unwrap_or(ConfirmPolicy::Always);

                let args = verb_config
                    .args
                    .iter()
                    .map(|a| ArgSummary {
                        name: a.name.clone(),
                        arg_type: format!("{:?}", a.arg_type),
                        required: a.required,
                        description: a.description.clone(),
                    })
                    .collect();

                entries.insert(
                    fqn.clone(),
                    VerbIndexEntry {
                        fqn,
                        description: verb_config.description.clone(),
                        invocation_phrases: verb_config.invocation_phrases.clone(),
                        sentence_templates: templates,
                        sentences: yaml_sentences,
                        args,
                        confirm_policy: policy,
                    },
                );
            }
        }

        Self { entries }
    }

    /// Look up a verb by fully-qualified name.
    pub fn get(&self, verb_fqn: &str) -> Option<&VerbIndexEntry> {
        self.entries.get(verb_fqn)
    }

    /// List all verbs in a given domain.
    pub fn verbs_for_domain(&self, domain: &str) -> Vec<&VerbIndexEntry> {
        let prefix = format!("{}.", domain);
        self.entries
            .values()
            .filter(|e| e.fqn.starts_with(&prefix))
            .collect()
    }

    /// Iterate over all verbs.
    pub fn all_verbs(&self) -> impl Iterator<Item = &VerbIndexEntry> {
        self.entries.values()
    }

    /// Number of indexed verbs.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get invocation phrases for a verb (convenience accessor).
    pub fn invocation_phrases(&self, verb_fqn: &str) -> &[String] {
        self.entries
            .get(verb_fqn)
            .map(|e| e.invocation_phrases.as_slice())
            .unwrap_or(&[])
    }

    /// Get description for a verb (convenience accessor).
    pub fn description(&self, verb_fqn: &str) -> &str {
        self.entries
            .get(verb_fqn)
            .map(|e| e.description.as_str())
            .unwrap_or("")
    }

    /// Get sentence templates for a verb (convenience accessor).
    pub fn sentence_templates(&self, verb_fqn: &str) -> &[String] {
        self.entries
            .get(verb_fqn)
            .map(|e| e.sentence_templates.as_slice())
            .unwrap_or(&[])
    }

    /// Get full VerbSentences for a verb (if available from YAML).
    pub fn verb_sentences(&self, verb_fqn: &str) -> Option<&VerbSentences> {
        self.entries
            .get(verb_fqn)
            .and_then(|e| e.sentences.as_ref())
    }

    /// Get confirm policy for a verb.
    pub fn confirm_policy(&self, verb_fqn: &str) -> ConfirmPolicy {
        self.entries
            .get(verb_fqn)
            .map(|e| e.confirm_policy)
            .unwrap_or(ConfirmPolicy::Always)
    }

    /// Build a HashMap of verb FQN → invocation phrases (for backwards compat).
    pub fn all_invocation_phrases(&self) -> HashMap<String, Vec<String>> {
        self.entries
            .iter()
            .filter(|(_, e)| !e.invocation_phrases.is_empty())
            .map(|(fqn, e)| (fqn.clone(), e.invocation_phrases.clone()))
            .collect()
    }

    /// Build a HashMap of verb FQN → description (for backwards compat).
    pub fn all_descriptions(&self) -> HashMap<String, String> {
        self.entries
            .iter()
            .filter(|(_, e)| !e.description.is_empty())
            .map(|(fqn, e)| (fqn.clone(), e.description.clone()))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Pack Verb Sentence Templates (Phase 1 bridge — replaced by VerbConfig.sentences in Phase 2)
// ---------------------------------------------------------------------------

/// Hardcoded sentence templates for the ~15 verbs used by starter packs.
///
/// These provide better human-readable sentences than the generic
/// invocation_phrases fallback. In Phase 2, these move to `sentences.step[]`
/// on VerbConfig YAML and this function is deleted.
pub fn pack_verb_sentence_templates() -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();

    // -- CBU verbs --
    m.insert(
        "cbu.create".to_string(),
        vec![
            "Create {name} structure in {jurisdiction}".to_string(),
            "Set up new structure {name}".to_string(),
        ],
    );
    m.insert(
        "cbu.assign-product".to_string(),
        vec![
            "Assign {product} product to {cbu-name}".to_string(),
            "Add {product} to {cbu-name} product list".to_string(),
        ],
    );
    m.insert(
        "cbu.assign-manco".to_string(),
        vec!["Assign management company {manco-name} to {cbu-name}".to_string()],
    );

    // -- Trading profile verbs --
    m.insert(
        "trading-profile.create-matrix".to_string(),
        vec!["Create trading matrix for {cbu-name}".to_string()],
    );
    m.insert(
        "trading-profile.add-counterparty".to_string(),
        vec!["Add counterparty {counterparty-name} to {cbu-name} trading profile".to_string()],
    );
    m.insert(
        "trading-profile.add-instrument".to_string(),
        vec!["Add {instrument-class} instrument to {cbu-name} trading profile".to_string()],
    );

    // -- Entity verbs --
    m.insert(
        "entity.ensure-or-create".to_string(),
        vec!["Ensure entity {name} exists (create if missing)".to_string()],
    );

    // -- KYC verbs --
    m.insert(
        "kyc.open-case".to_string(),
        vec![
            "Open KYC case for {entity-name}".to_string(),
            "Start KYC review for {entity-name}".to_string(),
        ],
    );
    m.insert(
        "kyc.request-docs".to_string(),
        vec!["Request {doc-type} documents from {entity-name}".to_string()],
    );
    m.insert(
        "kyc.review-gate".to_string(),
        vec!["Run review gate for KYC case {case-id}".to_string()],
    );

    // -- Onboarding verbs --
    m.insert(
        "onboarding.create-request".to_string(),
        vec!["Create onboarding request for {client-name}".to_string()],
    );

    // -- Session/navigation verbs --
    m.insert(
        "session.load-galaxy".to_string(),
        vec!["Load {apex-name} book into session".to_string()],
    );
    m.insert(
        "session.load-cbu".to_string(),
        vec!["Load {cbu-name} into session".to_string()],
    );

    // -- Contract verbs --
    m.insert(
        "contract.subscribe".to_string(),
        vec!["Subscribe {cbu-name} to {product} under contract".to_string()],
    );
    m.insert(
        "contract.add-product".to_string(),
        vec!["Add {product} to contract {contract-ref}".to_string()],
    );

    m
}

/// Hardcoded confirm policies for pack verbs.
///
/// Navigation verbs get QuickConfirm (no confirmation needed).
/// Data-modifying verbs default to Always.
pub fn pack_verb_confirm_policies() -> HashMap<String, ConfirmPolicy> {
    let mut m = HashMap::new();

    // Navigation — quick confirm (low risk)
    m.insert(
        "session.load-galaxy".to_string(),
        ConfirmPolicy::QuickConfirm,
    );
    m.insert("session.load-cbu".to_string(), ConfirmPolicy::QuickConfirm);
    m.insert("session.info".to_string(), ConfirmPolicy::QuickConfirm);
    m.insert("session.list".to_string(), ConfirmPolicy::QuickConfirm);

    // All other (data-modifying) verbs default to ConfirmPolicy::Always
    // via the VerbConfigIndex constructor.

    m
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::config::types::{ArgConfig, ArgType, DomainConfig, VerbBehavior, VerbConfig};

    fn make_test_config() -> VerbsConfig {
        let mut domains = HashMap::new();

        let mut cbu_verbs = HashMap::new();
        cbu_verbs.insert(
            "create".to_string(),
            VerbConfig {
                description: "Create a new CBU".to_string(),
                behavior: VerbBehavior::Plugin,
                invocation_phrases: vec![
                    "create cbu".to_string(),
                    "add new client unit".to_string(),
                ],
                args: vec![
                    ArgConfig {
                        name: "name".to_string(),
                        arg_type: ArgType::String,
                        required: true,
                        description: Some("CBU name".to_string()),
                        ..default_arg_config()
                    },
                    ArgConfig {
                        name: "jurisdiction".to_string(),
                        arg_type: ArgType::String,
                        required: false,
                        description: Some("Jurisdiction code".to_string()),
                        ..default_arg_config()
                    },
                ],
                ..default_verb_config()
            },
        );
        cbu_verbs.insert(
            "assign-product".to_string(),
            VerbConfig {
                description: "Assign a product to a CBU".to_string(),
                behavior: VerbBehavior::Plugin,
                invocation_phrases: vec!["assign product".to_string()],
                args: vec![
                    ArgConfig {
                        name: "product".to_string(),
                        arg_type: ArgType::String,
                        required: true,
                        ..default_arg_config()
                    },
                    ArgConfig {
                        name: "cbu-name".to_string(),
                        arg_type: ArgType::String,
                        required: true,
                        ..default_arg_config()
                    },
                ],
                ..default_verb_config()
            },
        );

        domains.insert(
            "cbu".to_string(),
            DomainConfig {
                description: "Client Business Unit".to_string(),
                verbs: cbu_verbs,
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );

        let mut session_verbs = HashMap::new();
        session_verbs.insert(
            "load-galaxy".to_string(),
            VerbConfig {
                description: "Load all CBUs under apex entity".to_string(),
                behavior: VerbBehavior::Plugin,
                invocation_phrases: vec!["load book".to_string()],
                args: vec![ArgConfig {
                    name: "apex-name".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                    ..default_arg_config()
                }],
                ..default_verb_config()
            },
        );

        domains.insert(
            "session".to_string(),
            DomainConfig {
                description: "Session management".to_string(),
                verbs: session_verbs,
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );

        VerbsConfig {
            version: "1.0".to_string(),
            domains,
        }
    }

    fn default_arg_config() -> ArgConfig {
        ArgConfig {
            name: String::new(),
            arg_type: ArgType::String,
            required: false,
            maps_to: None,
            lookup: None,
            valid_values: None,
            default: None,
            description: None,
            validation: None,
            fuzzy_check: None,
            slot_type: None,
            preferred_roles: vec![],
        }
    }

    fn default_verb_config() -> VerbConfig {
        VerbConfig {
            description: String::new(),
            behavior: VerbBehavior::Plugin,
            crud: None,
            handler: None,
            graph_query: None,
            args: vec![],
            returns: None,
            produces: None,
            consumes: vec![],
            lifecycle: None,
            metadata: None,
            invocation_phrases: vec![],
            policy: None,
            sentences: None,
            confirm_policy: None,
        }
    }

    #[test]
    fn test_from_verbs_config() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        assert_eq!(index.len(), 3);
        assert!(!index.is_empty());
    }

    #[test]
    fn test_get_verb() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        let entry = index.get("cbu.create").unwrap();
        assert_eq!(entry.fqn, "cbu.create");
        assert_eq!(entry.description, "Create a new CBU");
        assert_eq!(entry.args.len(), 2);
        assert!(entry.args[0].required || entry.args[1].required);
    }

    #[test]
    fn test_get_nonexistent_verb() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);
        assert!(index.get("nonexistent.verb").is_none());
    }

    #[test]
    fn test_verbs_for_domain() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        let cbu_verbs = index.verbs_for_domain("cbu");
        assert_eq!(cbu_verbs.len(), 2);

        let session_verbs = index.verbs_for_domain("session");
        assert_eq!(session_verbs.len(), 1);

        let empty = index.verbs_for_domain("nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_sentence_templates_populated() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        let templates = index.sentence_templates("cbu.create");
        assert!(!templates.is_empty());
        assert!(templates[0].contains("{name}"));

        let templates = index.sentence_templates("cbu.assign-product");
        assert!(!templates.is_empty());
        assert!(templates[0].contains("{product}"));
    }

    #[test]
    fn test_confirm_policy() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        assert_eq!(
            index.confirm_policy("session.load-galaxy"),
            ConfirmPolicy::QuickConfirm
        );
        assert_eq!(index.confirm_policy("cbu.create"), ConfirmPolicy::Always);
        assert_eq!(
            index.confirm_policy("nonexistent.verb"),
            ConfirmPolicy::Always
        );
    }

    #[test]
    fn test_invocation_phrases_accessor() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        let phrases = index.invocation_phrases("cbu.create");
        assert_eq!(phrases.len(), 2);
        assert!(phrases.contains(&"create cbu".to_string()));

        let empty = index.invocation_phrases("nonexistent.verb");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_all_invocation_phrases() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        let all = index.all_invocation_phrases();
        assert!(all.contains_key("cbu.create"));
        assert!(all.contains_key("session.load-galaxy"));
    }

    #[test]
    fn test_all_descriptions() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        let all = index.all_descriptions();
        assert_eq!(all.get("cbu.create").unwrap(), "Create a new CBU");
    }

    #[test]
    fn test_pack_verb_sentence_templates_coverage() {
        let templates = pack_verb_sentence_templates();
        // Verify all ~15 pack verbs have templates
        let expected_verbs = vec![
            "cbu.create",
            "cbu.assign-product",
            "cbu.assign-manco",
            "trading-profile.create-matrix",
            "trading-profile.add-counterparty",
            "trading-profile.add-instrument",
            "entity.ensure-or-create",
            "kyc.open-case",
            "kyc.request-docs",
            "kyc.review-gate",
            "onboarding.create-request",
            "session.load-galaxy",
            "session.load-cbu",
            "contract.subscribe",
            "contract.add-product",
        ];

        for verb in expected_verbs {
            assert!(
                templates.contains_key(verb),
                "Missing sentence template for pack verb: {}",
                verb
            );
            let t = &templates[verb];
            assert!(!t.is_empty(), "Empty templates for: {}", verb);
            // Each template should contain at least one {placeholder}
            assert!(
                t.iter().any(|s| s.contains('{')),
                "Template for {} has no placeholders: {:?}",
                verb,
                t
            );
        }
    }

    #[test]
    fn test_pack_verb_confirm_policies_coverage() {
        let policies = pack_verb_confirm_policies();
        // Navigation verbs should be QuickConfirm
        assert_eq!(policies["session.load-galaxy"], ConfirmPolicy::QuickConfirm);
        assert_eq!(policies["session.load-cbu"], ConfirmPolicy::QuickConfirm);
    }

    #[test]
    fn test_arg_summary() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        let entry = index.get("cbu.create").unwrap();
        let name_arg = entry.args.iter().find(|a| a.name == "name").unwrap();
        assert!(name_arg.required);
        assert_eq!(name_arg.description.as_deref(), Some("CBU name"));

        let juris_arg = entry
            .args
            .iter()
            .find(|a| a.name == "jurisdiction")
            .unwrap();
        assert!(!juris_arg.required);
    }

    #[test]
    fn test_yaml_sentences_override_hardcoded() {
        // cbu.create has BOTH hardcoded templates AND YAML sentences.
        // YAML sentences.step[] should win.
        let mut config = make_test_config();

        let yaml_sentences = VerbSentences {
            step: vec!["YAML: Create {name} in {jurisdiction}".to_string()],
            summary: vec!["YAML: created {name}".to_string()],
            clarify: {
                let mut m = std::collections::HashMap::new();
                m.insert("name".to_string(), "What name?".to_string());
                m
            },
            completed: Some("YAML: {name} done".to_string()),
        };

        // Set YAML sentences on cbu.create
        config
            .domains
            .get_mut("cbu")
            .unwrap()
            .verbs
            .get_mut("create")
            .unwrap()
            .sentences = Some(yaml_sentences);

        let index = VerbConfigIndex::from_verbs_config(&config);

        // sentence_templates should come from YAML, not hardcoded
        let templates = index.sentence_templates("cbu.create");
        assert_eq!(templates.len(), 1);
        assert!(templates[0].starts_with("YAML:"));

        // Full VerbSentences should be accessible
        let sentences = index.verb_sentences("cbu.create").unwrap();
        assert_eq!(sentences.clarify.get("name").unwrap(), "What name?");
        assert_eq!(sentences.completed.as_deref(), Some("YAML: {name} done"));
    }

    #[test]
    fn test_pack_only_fqns_get_hardcoded_templates() {
        // cbu.assign-product has hardcoded templates but NO YAML sentences.
        // It should still get the hardcoded templates.
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        let templates = index.sentence_templates("cbu.assign-product");
        assert!(!templates.is_empty());
        assert!(templates[0].contains("{product}"));

        // No YAML sentences for this verb
        assert!(index.verb_sentences("cbu.assign-product").is_none());
    }

    #[test]
    fn test_yaml_confirm_policy_overrides_hardcoded() {
        let mut config = make_test_config();

        // session.load-galaxy has hardcoded QuickConfirm.
        // Set YAML confirm_policy to Always — YAML should win.
        config
            .domains
            .get_mut("session")
            .unwrap()
            .verbs
            .get_mut("load-galaxy")
            .unwrap()
            .confirm_policy = Some(ConfirmPolicyConfig::Always);

        let index = VerbConfigIndex::from_verbs_config(&config);
        assert_eq!(
            index.confirm_policy("session.load-galaxy"),
            ConfirmPolicy::Always
        );
    }

    #[test]
    fn test_verb_sentences_accessor() {
        let config = make_test_config();
        let index = VerbConfigIndex::from_verbs_config(&config);

        // No YAML sentences on test verbs by default
        assert!(index.verb_sentences("cbu.create").is_none());
        assert!(index.verb_sentences("nonexistent.verb").is_none());
    }
}
