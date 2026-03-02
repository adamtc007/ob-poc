//! NounIndex — Entity-Centric Intent Resolution (ECIR)
//!
//! Deterministic Tier -1 verb resolution via noun→verb cross-linking.
//! Extracts domain nouns from utterances, classifies actions, and resolves
//! to verb FQN candidates without embedding search.
//!
//! Resolution chain (highest to lowest priority):
//! 1. ExplicitMapping: `action_verbs[action]` → specific verb FQNs
//! 2. NounKeyMatch: `noun_keys` → verbs with matching `metadata.noun`
//! 3. SubjectKindMatch: `entity_type_fqn` → verbs with matching `subject_kinds`
//! 4. NoMatch → fall through to embedding tiers

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;

use dsl_core::config::types::VerbsConfig;

// ---------------------------------------------------------------------------
// YAML deserialization types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct NounIndexYaml {
    #[allow(dead_code)]
    version: Option<String>,
    nouns: HashMap<String, NounEntryYaml>,
}

#[derive(Debug, Deserialize)]
struct NounEntryYaml {
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    natural_aliases: Vec<String>,
    #[serde(default)]
    entity_type_fqn: Option<String>,
    #[serde(default)]
    noun_keys: Vec<String>,
    #[serde(default)]
    action_verbs: HashMap<String, Vec<String>>,
}

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single noun entry in the taxonomy.
#[derive(Debug, Clone)]
pub struct NounEntry {
    /// Canonical key (e.g., "cbu", "kyc-case", "fund")
    pub key: String,
    /// Entity type FQN for subject_kinds fallback (e.g., "cbu", "entity")
    pub entity_type_fqn: Option<String>,
    /// Maps to `metadata.noun` values on verbs
    pub noun_keys: Vec<String>,
    /// Explicit action→verb FQN mappings (highest priority resolution)
    pub action_verbs: HashMap<String, Vec<String>>,
}

/// A noun extracted from an utterance.
#[derive(Debug, Clone)]
pub struct NounMatch {
    pub noun: Arc<NounEntry>,
    pub matched_alias: String,
    pub is_canonical: bool,
    /// Character span in the lowercased utterance
    pub span: (usize, usize),
}

/// Result of noun→verb resolution.
#[derive(Debug, Clone)]
pub struct NounResolution {
    /// Verb FQN candidates
    pub candidates: Vec<String>,
    /// Which noun matched
    pub noun_key: String,
    /// Classified action (if any)
    pub action: Option<ActionCategory>,
    /// How the candidates were found
    pub resolution_path: ResolutionPath,
}

/// How ECIR resolved the verb candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionPath {
    /// `action_verbs` explicit mapping — highest confidence
    ExplicitMapping,
    /// `noun_keys` → `metadata.noun` match — medium confidence
    NounKeyMatch,
    /// `subject_kinds` match — lower confidence
    SubjectKindMatch,
    /// No match — fall through
    NoMatch,
}

/// Action category classified from utterance surface patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionCategory {
    Create,
    List,
    Update,
    Delete,
    Assign,
    Compute,
    Import,
    Search,
}

impl ActionCategory {
    /// Returns the string key used in `action_verbs` YAML mapping.
    pub fn as_key(&self) -> &'static str {
        match self {
            ActionCategory::Create => "create",
            ActionCategory::List => "list",
            ActionCategory::Update => "update",
            ActionCategory::Delete => "delete",
            ActionCategory::Assign => "assign",
            ActionCategory::Compute => "compute",
            ActionCategory::Import => "import",
            ActionCategory::Search => "search",
        }
    }
}

// ---------------------------------------------------------------------------
// VerbContractSummary — lightweight verb metadata cache
// ---------------------------------------------------------------------------

/// Lightweight verb metadata for noun→verb resolution.
/// Built once at startup from VerbsConfig.
#[derive(Debug, Clone)]
pub struct VerbContractSummary {
    pub fqn: String,
    pub domain: String,
    pub action: String,
    pub description: String,
    pub noun: Option<String>,
    pub subject_kinds: Vec<String>,
    pub phase_tags: Vec<String>,
}

/// Index for fast noun→verb lookup at resolution time.
#[derive(Debug, Clone)]
pub struct VerbContractIndex {
    /// `metadata.noun` → vec of verb FQNs
    pub by_noun: HashMap<String, Vec<String>>,
    /// `subject_kind` → vec of verb FQNs
    pub by_subject_kind: HashMap<String, Vec<String>>,
    /// All summaries by FQN
    pub by_fqn: HashMap<String, VerbContractSummary>,
}

impl VerbContractIndex {
    /// Build from loaded VerbsConfig, replicating the scanner's subject_kinds derivation.
    pub fn from_verbs_config(config: &VerbsConfig) -> Self {
        let mut by_noun: HashMap<String, Vec<String>> = HashMap::new();
        let mut by_subject_kind: HashMap<String, Vec<String>> = HashMap::new();
        let mut by_fqn: HashMap<String, VerbContractSummary> = HashMap::new();

        for (domain, domain_config) in &config.domains {
            for (action, verb_config) in &domain_config.verbs {
                let fqn = format!("{}.{}", domain, action);
                let meta = verb_config.metadata.as_ref();

                let noun = meta.and_then(|m| m.noun.clone());
                let phase_tags = meta.map(|m| m.phase_tags.clone()).unwrap_or_default();

                // Derive subject_kinds using the same chain as the scanner
                let subject_kinds = derive_subject_kinds(domain, verb_config);

                let summary = VerbContractSummary {
                    fqn: fqn.clone(),
                    domain: domain.clone(),
                    action: action.clone(),
                    description: verb_config.description.clone(),
                    noun: noun.clone(),
                    subject_kinds: subject_kinds.clone(),
                    phase_tags,
                };

                // Index by noun
                if let Some(ref n) = noun {
                    by_noun.entry(n.clone()).or_default().push(fqn.clone());
                }

                // Index by subject_kind
                for sk in &subject_kinds {
                    by_subject_kind
                        .entry(sk.clone())
                        .or_default()
                        .push(fqn.clone());
                }

                by_fqn.insert(fqn, summary);
            }
        }

        VerbContractIndex {
            by_noun,
            by_subject_kind,
            by_fqn,
        }
    }

    /// Number of verb summaries in the index.
    pub fn len(&self) -> usize {
        self.by_fqn.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.by_fqn.is_empty()
    }
}

/// Derive subject_kinds from VerbConfig using the scanner's 3-level fallback.
fn derive_subject_kinds(domain: &str, config: &dsl_core::config::types::VerbConfig) -> Vec<String> {
    // Priority 1: explicit metadata.subject_kinds
    if let Some(ref meta) = config.metadata {
        if !meta.subject_kinds.is_empty() {
            return meta.subject_kinds.clone();
        }
    }

    // Priority 2: produces.entity_type
    if let Some(ref p) = config.produces {
        return vec![p.produced_type.clone()];
    }

    // Priority 3: required args with lookup.entity_type
    let mut kinds: Vec<String> = config
        .args
        .iter()
        .filter(|a| a.required)
        .filter_map(|a| a.lookup.as_ref().and_then(|l| l.entity_type.clone()))
        .collect();
    kinds.dedup();
    if !kinds.is_empty() {
        return kinds;
    }

    // Priority 4: domain heuristic
    vec![domain_to_subject_kind(domain)]
}

/// Map a verb domain name to its primary subject kind (lowest-priority heuristic).
fn domain_to_subject_kind(domain: &str) -> String {
    match domain {
        "cbu" | "cbu-role" => "cbu".into(),
        "entity" | "entity-role" => "entity".into(),
        "kyc" | "kyc-case" | "screening" => "kyc-case".into(),
        "deal" => "deal".into(),
        "contract" | "contract-pack" => "contract".into(),
        "billing" => "billing-profile".into(),
        "trading-profile" | "custody" | "ssi" => "trading-profile".into(),
        "investor" | "holding" => "investor-register".into(),
        "document" | "requirement" => "document".into(),
        "session" | "view" => "session".into(),
        "gleif" | "research" => "entity".into(),
        "workflow" | "bpmn" => "workflow".into(),
        _ => domain.into(),
    }
}

// ---------------------------------------------------------------------------
// NounIndex — the main ECIR data structure
// ---------------------------------------------------------------------------

/// In-memory noun taxonomy for Entity-Centric Intent Resolution (ECIR).
///
/// Loaded once at startup from `config/noun_index.yaml` + VerbsConfig.
/// Provides O(1) noun→verb resolution without database or embedding queries.
pub struct NounIndex {
    /// Canonical alias (lowercased) → NounEntry
    canonical: HashMap<String, Arc<NounEntry>>,
    /// Natural alias (lowercased) → NounEntry
    natural: HashMap<String, Arc<NounEntry>>,
    /// All aliases sorted by token count descending (for longest-match scanning)
    sorted_aliases: Vec<(String, Arc<NounEntry>, bool)>,
    /// Verb metadata index for NounKeyMatch and SubjectKindMatch resolution
    pub verb_index: Arc<VerbContractIndex>,
}

impl std::fmt::Debug for NounIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NounIndex")
            .field("canonical_count", &self.canonical.len())
            .field("natural_count", &self.natural.len())
            .field("alias_count", &self.sorted_aliases.len())
            .finish()
    }
}

impl NounIndex {
    /// Load NounIndex from YAML file + VerbsConfig.
    pub fn load(path: &Path, verb_index: VerbContractIndex) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read noun index from {}", path.display()))?;
        Self::from_yaml(&content, verb_index)
    }

    /// Parse from YAML string + VerbContractIndex (for testing).
    pub fn from_yaml(yaml: &str, verb_index: VerbContractIndex) -> Result<Self> {
        let parsed: NounIndexYaml =
            serde_yaml::from_str(yaml).context("Failed to parse noun_index.yaml")?;

        let mut canonical: HashMap<String, Arc<NounEntry>> = HashMap::new();
        let mut natural: HashMap<String, Arc<NounEntry>> = HashMap::new();
        let mut sorted_aliases: Vec<(String, Arc<NounEntry>, bool)> = Vec::new();

        for (key, entry_yaml) in parsed.nouns {
            let entry = Arc::new(NounEntry {
                key: key.clone(),
                entity_type_fqn: entry_yaml.entity_type_fqn,
                noun_keys: entry_yaml.noun_keys,
                action_verbs: entry_yaml.action_verbs,
            });

            // The noun key itself is always a canonical alias
            let lower_key = key.to_lowercase();
            canonical.insert(lower_key.clone(), Arc::clone(&entry));
            sorted_aliases.push((lower_key, Arc::clone(&entry), true));

            // Canonical aliases
            for alias in &entry_yaml.aliases {
                let lower = alias.to_lowercase();
                canonical.insert(lower.clone(), Arc::clone(&entry));
                sorted_aliases.push((lower, Arc::clone(&entry), true));
            }

            // Natural aliases (softer matches)
            for alias in &entry_yaml.natural_aliases {
                let lower = alias.to_lowercase();
                natural.insert(lower.clone(), Arc::clone(&entry));
                sorted_aliases.push((lower, Arc::clone(&entry), false));
            }
        }

        // Sort by word count descending for longest-match-first extraction
        sorted_aliases.sort_by(|a, b| {
            let a_words = a.0.split_whitespace().count();
            let b_words = b.0.split_whitespace().count();
            b_words.cmp(&a_words).then(b.0.cmp(&a.0))
        });

        Ok(NounIndex {
            canonical,
            natural,
            sorted_aliases,
            verb_index: Arc::new(verb_index),
        })
    }

    /// Number of canonical aliases in the index.
    pub fn canonical_count(&self) -> usize {
        self.canonical.len()
    }

    /// Extract nouns from utterance using longest-match-first scanning.
    ///
    /// Matches canonical aliases first (exact), then natural aliases.
    /// Returns non-overlapping matches.
    pub fn extract(&self, utterance: &str) -> Vec<NounMatch> {
        let lower = utterance.to_lowercase();
        let mut matches: Vec<NounMatch> = Vec::new();
        let mut covered: Vec<bool> = vec![false; lower.len()];

        for (alias, entry, is_canonical) in &self.sorted_aliases {
            // Find all occurrences of this alias in the utterance
            let mut search_from = 0;
            while let Some(pos) = lower[search_from..].find(alias.as_str()) {
                let start = search_from + pos;
                let end = start + alias.len();

                // Check word boundaries
                let left_ok = start == 0
                    || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
                let right_ok = end == lower.len()
                    || !lower.as_bytes()[end].is_ascii_alphanumeric();

                if left_ok && right_ok {
                    // Check for overlap with already-matched spans
                    let overlaps = covered[start..end].iter().any(|&c| c);
                    if !overlaps {
                        // Mark span as covered
                        for c in &mut covered[start..end] {
                            *c = true;
                        }
                        matches.push(NounMatch {
                            noun: Arc::clone(entry),
                            matched_alias: alias.clone(),
                            is_canonical: *is_canonical,
                            span: (start, end),
                        });
                    }
                }
                search_from = start + 1;
            }
        }

        // Sort by position in utterance
        matches.sort_by_key(|m| m.span.0);
        matches
    }

    /// Classify action from utterance surface patterns.
    ///
    /// Uses imperative verb at start of utterance and question patterns.
    /// Returns None for unrecognizable patterns (ECIR still works — just broader candidates).
    pub fn classify_action(utterance: &str) -> Option<ActionCategory> {
        let lower = utterance.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("");

        // Imperative verb at start
        match first {
            "create" | "add" | "new" | "register" | "establish" | "onboard" | "open" => {
                return Some(ActionCategory::Create);
            }
            "set" => {
                if words.get(1).copied() == Some("up") {
                    return Some(ActionCategory::Create);
                }
            }
            "show" | "list" | "display" | "get" | "view" | "describe" | "see" => {
                return Some(ActionCategory::List);
            }
            "update" | "change" | "modify" | "edit" | "rename" | "amend" => {
                return Some(ActionCategory::Update);
            }
            "delete" | "remove" | "drop" | "cancel" | "revoke" | "archive" | "close" => {
                return Some(ActionCategory::Delete);
            }
            "assign" | "link" | "connect" | "attach" | "map" | "bind" | "associate" => {
                return Some(ActionCategory::Assign);
            }
            "compute" | "calculate" | "run" | "check" | "screen" | "verify" | "validate"
            | "evaluate" | "assess" | "refresh" => {
                return Some(ActionCategory::Compute);
            }
            "import" | "pull" | "fetch" | "sync" | "enrich" | "load" | "ingest" => {
                return Some(ActionCategory::Import);
            }
            "trace" | "find" | "search" | "discover" | "lookup" | "look" => {
                if first == "look" && words.get(1).copied() == Some("up") {
                    return Some(ActionCategory::Search);
                }
                if first != "look" {
                    return Some(ActionCategory::Search);
                }
            }
            _ => {}
        }

        // Question patterns
        match first {
            "who" | "where" => return Some(ActionCategory::Search),
            "what" | "how" => {
                if words.get(1).copied() == Some("many") {
                    return Some(ActionCategory::List);
                }
                return Some(ActionCategory::List);
            }
            _ => {}
        }

        None
    }

    /// Resolve: given extracted nouns + optional action, return verb FQN candidates.
    ///
    /// Resolution chain:
    /// 1. If noun has `action_verbs[action]` → return those (ExplicitMapping)
    /// 2. If noun has `noun_keys` → find all verbs whose `metadata.noun` is in noun_keys (NounKeyMatch)
    /// 3. If noun has `entity_type_fqn` → find all verbs whose `subject_kinds` contains it (SubjectKindMatch)
    /// 4. No match → return empty (NoMatch)
    pub fn resolve(
        &self,
        nouns: &[NounMatch],
        action: Option<ActionCategory>,
    ) -> NounResolution {
        if nouns.is_empty() {
            return NounResolution {
                candidates: vec![],
                noun_key: String::new(),
                action,
                resolution_path: ResolutionPath::NoMatch,
            };
        }

        // Use the first (leftmost) noun match as primary
        let primary = &nouns[0];
        let noun_key = primary.noun.key.clone();

        // Path 1: Explicit action→verb mapping
        if let Some(action_cat) = action {
            let action_key = action_cat.as_key();
            if let Some(verbs) = primary.noun.action_verbs.get(action_key) {
                if !verbs.is_empty() {
                    return NounResolution {
                        candidates: verbs.clone(),
                        noun_key,
                        action: Some(action_cat),
                        resolution_path: ResolutionPath::ExplicitMapping,
                    };
                }
            }
        }

        // Path 2: noun_keys → metadata.noun match
        if !primary.noun.noun_keys.is_empty() {
            let mut candidates: Vec<String> = Vec::new();
            for nk in &primary.noun.noun_keys {
                if let Some(verbs) = self.verb_index.by_noun.get(nk) {
                    candidates.extend(verbs.iter().cloned());
                }
            }
            candidates.sort();
            candidates.dedup();

            if !candidates.is_empty() {
                return NounResolution {
                    candidates,
                    noun_key,
                    action,
                    resolution_path: ResolutionPath::NounKeyMatch,
                };
            }
        }

        // Path 3: entity_type_fqn → subject_kinds match
        if let Some(ref etf) = primary.noun.entity_type_fqn {
            if let Some(verbs) = self.verb_index.by_subject_kind.get(etf) {
                let mut candidates = verbs.clone();
                candidates.sort();
                candidates.dedup();

                if !candidates.is_empty() {
                    return NounResolution {
                        candidates,
                        noun_key,
                        action,
                        resolution_path: ResolutionPath::SubjectKindMatch,
                    };
                }
            }
        }

        // Path 4: No match
        NounResolution {
            candidates: vec![],
            noun_key,
            action,
            resolution_path: ResolutionPath::NoMatch,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_yaml() -> &'static str {
        r#"
version: "1.0"
nouns:
  cbu:
    aliases:
      - cbu
      - client business unit
      - structure
    natural_aliases:
      - fund structure
      - business unit
    entity_type_fqn: cbu
    noun_keys:
      - cbu
    action_verbs:
      create: [cbu.create]
      list: [cbu.list]
      delete: [cbu.delete]
      assign: [cbu.assign-role]

  ubo:
    aliases:
      - ubo
      - beneficial owner
      - ultimate beneficial owner
    natural_aliases:
      - who owns
      - ownership structure
    entity_type_fqn: entity
    noun_keys:
      - ubo
    action_verbs:
      search: [ubo.discover]
      list: [ubo.list-beneficiaries]

  screening:
    aliases:
      - screening
      - sanctions screening
      - sanctions
    natural_aliases:
      - ofac
      - sanctions check
    entity_type_fqn: entity
    noun_keys:
      - screening
    action_verbs:
      create: [screening.run]
      search: [screening.run]
      list: [screening.list]

  share-class:
    aliases:
      - share class
      - share classes
    natural_aliases:
      - accumulating
      - distributing
    entity_type_fqn: share_class
    noun_keys:
      - capital
      - fund
    action_verbs:
      create: [fund.create-share-class, capital.share-class.create]
      list: [fund.list-share-classes]

  session:
    aliases:
      - session
    natural_aliases:
      - current session
    noun_keys:
      - agent_session
    action_verbs:
      create: [session.create]
      list: [session.list]
"#
    }

    fn test_verb_index() -> VerbContractIndex {
        let mut by_noun: HashMap<String, Vec<String>> = HashMap::new();
        let mut by_subject_kind: HashMap<String, Vec<String>> = HashMap::new();
        let mut by_fqn: HashMap<String, VerbContractSummary> = HashMap::new();

        // Add some test verbs
        let verbs = vec![
            ("cbu.create", "cbu", "create", "Create a CBU", Some("cbu"), vec!["cbu"]),
            ("cbu.list", "cbu", "list", "List CBUs", Some("cbu"), vec!["cbu"]),
            ("cbu.delete", "cbu", "delete", "Delete a CBU", Some("cbu"), vec!["cbu"]),
            ("cbu.assign-role", "cbu", "assign-role", "Assign role to CBU", Some("cbu_role"), vec!["cbu"]),
            ("ubo.discover", "ubo", "discover", "Discover UBOs", Some("ubo"), vec!["entity"]),
            ("ubo.list-beneficiaries", "ubo", "list-beneficiaries", "List beneficiaries", Some("ubo"), vec!["entity"]),
            ("screening.run", "screening", "run", "Run screening", Some("screening"), vec!["entity"]),
            ("screening.list", "screening", "list", "List screenings", Some("screening"), vec!["entity"]),
            ("fund.create-share-class", "fund", "create-share-class", "Create share class", Some("fund"), vec!["fund"]),
            ("fund.list-share-classes", "fund", "list-share-classes", "List share classes", Some("fund"), vec!["fund"]),
            ("capital.share-class.create", "capital", "share-class.create", "Create share class", Some("capital"), vec!["capital"]),
            ("session.create", "session", "create", "Create session", Some("agent_session"), vec!["session"]),
            ("session.list", "session", "list", "List sessions", Some("agent_session"), vec!["session"]),
        ];

        for (fqn, domain, action, desc, noun, sks) in verbs {
            let summary = VerbContractSummary {
                fqn: fqn.to_string(),
                domain: domain.to_string(),
                action: action.to_string(),
                description: desc.to_string(),
                noun: noun.map(|n| n.to_string()),
                subject_kinds: sks.iter().map(|s| s.to_string()).collect(),
                phase_tags: vec![],
            };
            if let Some(ref n) = summary.noun {
                by_noun.entry(n.clone()).or_default().push(fqn.to_string());
            }
            for sk in &summary.subject_kinds {
                by_subject_kind.entry(sk.clone()).or_default().push(fqn.to_string());
            }
            by_fqn.insert(fqn.to_string(), summary);
        }

        VerbContractIndex {
            by_noun,
            by_subject_kind,
            by_fqn,
        }
    }

    fn test_noun_index() -> NounIndex {
        NounIndex::from_yaml(test_yaml(), test_verb_index()).unwrap()
    }

    #[test]
    fn test_load_noun_index() {
        let idx = test_noun_index();
        assert!(idx.canonical.len() >= 10);
        assert!(!idx.sorted_aliases.is_empty());
    }

    #[test]
    fn test_extract_cbu() {
        let idx = test_noun_index();
        let matches = idx.extract("create a new client business unit");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].noun.key, "cbu");
        assert_eq!(matches[0].matched_alias, "client business unit");
        assert!(matches[0].is_canonical);
    }

    #[test]
    fn test_extract_ubo() {
        let idx = test_noun_index();
        // Use exact alias text — word boundary check means "owners" won't match "owner"
        let matches = idx.extract("who is the beneficial owner?");
        assert!(!matches.is_empty());
        let ubo_match = matches.iter().find(|m| m.noun.key == "ubo");
        assert!(ubo_match.is_some(), "Should extract 'beneficial owner' → ubo");
    }

    #[test]
    fn test_extract_screening_ofac() {
        let idx = test_noun_index();
        let matches = idx.extract("check for ofac hits");
        assert!(!matches.is_empty());
        let scr = matches.iter().find(|m| m.noun.key == "screening");
        assert!(scr.is_some(), "Should extract 'ofac' → screening");
    }

    #[test]
    fn test_classify_action_create() {
        assert_eq!(
            NounIndex::classify_action("create a new CBU"),
            Some(ActionCategory::Create)
        );
        assert_eq!(
            NounIndex::classify_action("set up a fund"),
            Some(ActionCategory::Create)
        );
        assert_eq!(
            NounIndex::classify_action("add an entity"),
            Some(ActionCategory::Create)
        );
    }

    #[test]
    fn test_classify_action_search() {
        assert_eq!(
            NounIndex::classify_action("who controls this?"),
            Some(ActionCategory::Search)
        );
        assert_eq!(
            NounIndex::classify_action("find all beneficiaries"),
            Some(ActionCategory::Search)
        );
    }

    #[test]
    fn test_classify_action_list() {
        assert_eq!(
            NounIndex::classify_action("what documents are missing?"),
            Some(ActionCategory::List)
        );
        assert_eq!(
            NounIndex::classify_action("show all CBUs"),
            Some(ActionCategory::List)
        );
    }

    #[test]
    fn test_classify_action_none() {
        assert_eq!(NounIndex::classify_action("hmm let me think"), None);
    }

    #[test]
    fn test_resolve_explicit_mapping() {
        let idx = test_noun_index();
        let matches = idx.extract("create a new cbu");
        let action = NounIndex::classify_action("create a new cbu");
        let resolution = idx.resolve(&matches, action);

        assert_eq!(resolution.resolution_path, ResolutionPath::ExplicitMapping);
        assert_eq!(resolution.candidates, vec!["cbu.create"]);
        assert_eq!(resolution.noun_key, "cbu");
    }

    #[test]
    fn test_resolve_explicit_multi_verb() {
        let idx = test_noun_index();
        let matches = idx.extract("create a share class");
        let action = NounIndex::classify_action("create a share class");
        let resolution = idx.resolve(&matches, action);

        assert_eq!(resolution.resolution_path, ResolutionPath::ExplicitMapping);
        assert_eq!(resolution.candidates.len(), 2);
        assert!(resolution.candidates.contains(&"fund.create-share-class".to_string()));
        assert!(resolution.candidates.contains(&"capital.share-class.create".to_string()));
    }

    #[test]
    fn test_resolve_no_noun_empty() {
        let idx = test_noun_index();
        let matches = idx.extract("do something random");
        let resolution = idx.resolve(&matches, None);

        assert_eq!(resolution.resolution_path, ResolutionPath::NoMatch);
        assert!(resolution.candidates.is_empty());
    }

    #[test]
    fn test_resolve_noun_key_match_fallback() {
        let idx = test_noun_index();

        // Create a NounMatch manually for a noun that has noun_keys but no matching action
        let noun = Arc::new(NounEntry {
            key: "cbu".to_string(),
            entity_type_fqn: Some("cbu".to_string()),
            noun_keys: vec!["cbu".to_string()],
            action_verbs: HashMap::new(), // No action_verbs at all
        });
        let matches = vec![NounMatch {
            noun,
            matched_alias: "cbu".to_string(),
            is_canonical: true,
            span: (0, 3),
        }];

        let resolution = idx.resolve(&matches, Some(ActionCategory::Compute));
        // Should fall through to NounKeyMatch since there's no "compute" in action_verbs
        assert_eq!(resolution.resolution_path, ResolutionPath::NounKeyMatch);
        assert!(!resolution.candidates.is_empty());
    }

    #[test]
    fn test_longest_match_first() {
        let idx = test_noun_index();
        let matches = idx.extract("check the ultimate beneficial owner now");
        // "ultimate beneficial owner" (3 words) should match before "beneficial owner" (2 words)
        let ubo_match = matches.iter().find(|m| m.noun.key == "ubo");
        assert!(ubo_match.is_some());
        assert_eq!(ubo_match.unwrap().matched_alias, "ultimate beneficial owner");
    }

    #[test]
    fn test_load_real_noun_index_yaml() {
        // Test that the actual noun_index.yaml parses without error
        let yaml_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config/noun_index.yaml");
        if yaml_path.exists() {
            let content = std::fs::read_to_string(&yaml_path).unwrap();
            let result = NounIndex::from_yaml(&content, VerbContractIndex {
                by_noun: HashMap::new(),
                by_subject_kind: HashMap::new(),
                by_fqn: HashMap::new(),
            });
            assert!(result.is_ok(), "Failed to parse noun_index.yaml: {:?}", result.err());
            let idx = result.unwrap();
            assert!(idx.canonical.len() >= 20, "Expected at least 20 canonical aliases, got {}", idx.canonical.len());
        }
    }
}
