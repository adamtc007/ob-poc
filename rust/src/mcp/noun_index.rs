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

use crate::entity_kind::canonicalize as canonical_entity_kind;

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
            return dedupe_subject_kinds(
                meta.subject_kinds
                    .iter()
                    .map(|kind| canonical_entity_kind(kind))
                    .collect(),
            );
        }
    }

    let mut inferred = Vec::new();

    if let Some(ref p) = config.produces {
        inferred.push(canonical_entity_kind(&p.produced_type));
    }

    inferred.extend(
        config
            .consumes
            .iter()
            .map(|consume| canonical_entity_kind(&consume.consumed_type)),
    );

    inferred.extend(derive_subject_kinds_from_crud(config));

    inferred.extend(
        config
            .args
            .iter()
            .filter(|arg| arg.required || arg.lookup.is_some())
            .filter_map(derive_subject_kind_from_arg),
    );

    if let Some(entity_arg) = config
        .lifecycle
        .as_ref()
        .and_then(|lifecycle| lifecycle.entity_arg.as_deref())
    {
        if let Some(arg) = config.args.iter().find(|arg| arg.name == entity_arg) {
            if let Some(kind) = derive_subject_kind_from_arg(arg) {
                inferred.push(kind);
            }
        }
    }

    if let Some(meta) = &config.metadata {
        inferred.extend(
            meta.noun
                .iter()
                .filter_map(|noun| derive_subject_kind_from_hint(noun)),
        );
        inferred.extend(
            meta.tags
                .iter()
                .filter_map(|tag| derive_subject_kind_from_hint(tag)),
        );
    }

    let inferred = dedupe_subject_kinds(inferred);
    if !inferred.is_empty() {
        return inferred;
    }

    vec![canonical_entity_kind(&domain_to_subject_kind(domain))]
}

fn dedupe_subject_kinds(mut kinds: Vec<String>) -> Vec<String> {
    kinds.retain(|kind| !kind.is_empty());
    kinds.sort();
    kinds.dedup();
    kinds
}

fn derive_subject_kinds_from_crud(config: &dsl_core::config::types::VerbConfig) -> Vec<String> {
    let Some(ref crud) = config.crud else {
        return Vec::new();
    };

    [
        crud.table.as_deref(),
        crud.base_table.as_deref(),
        crud.extension_table.as_deref(),
        crud.junction.as_deref(),
        crud.primary_table.as_deref(),
        crud.join_table.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(derive_subject_kind_from_hint)
    .collect()
}

fn derive_subject_kind_from_arg(arg: &dsl_core::config::types::ArgConfig) -> Option<String> {
    arg.lookup
        .as_ref()
        .and_then(|lookup| {
            lookup
                .entity_type
                .as_deref()
                .filter(|kind| !is_generic_lookup_kind(kind))
                .map(canonical_entity_kind)
                .or_else(|| derive_subject_kind_from_hint(&lookup.table))
        })
        .or_else(|| derive_subject_kind_from_arg_name(&arg.name))
}

fn derive_subject_kind_from_arg_name(name: &str) -> Option<String> {
    let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
    let trimmed = normalized
        .trim_end_matches("-id")
        .trim_end_matches("-ref")
        .trim_end_matches("-uuid");
    derive_subject_kind_from_hint(trimmed)
}

fn derive_subject_kind_from_hint(hint: &str) -> Option<String> {
    let normalized = hint.trim().to_ascii_lowercase().replace('_', "-");
    let kind = match normalized.as_str() {
        "cbu" | "cbus" | "client-business-unit" | "client-business-units" | "structure" => "cbu",
        "entity"
        | "entities"
        | "party"
        | "parties"
        | "company"
        | "companies"
        | "person"
        | "people"
        | "legal-entity"
        | "legal-entities"
        | "counterparty"
        | "counterparties"
        | "investment-manager"
        | "investment-managers"
        | "management-company"
        | "management-companies"
        | "depositary"
        | "depositaries" => "entity",
        "deal" | "deals" => "deal",
        "contract" | "contracts" | "contract-pack" | "contract-packs" | "agreement" => "contract",
        "document" | "documents" | "requirement" | "requirements" | "evidence" | "attachments" => {
            "document"
        }
        "trading-profile"
        | "trading-profiles"
        | "mandate"
        | "mandates"
        | "ssi"
        | "custody"
        | "cbu-trading-profiles" => "trading-profile",
        "billing" | "billings" | "billing-profile" | "billing-profiles" | "invoice"
        | "invoices" | "fee" | "fees" => "billing-profile",
        "fund" | "funds" | "sub-fund" | "sub-funds" | "umbrella" | "umbrellas" => "fund",
        "investor" | "investors" | "holding" | "holdings" | "investor-register" => "investor",
        "kyc-case"
        | "kyc"
        | "case"
        | "cases"
        | "tollgate"
        | "tollgate-evaluations"
        | "screening"
        | "screenings" => "kyc-case",
        "session" | "view" => "session",
        "workflow" => "workflow",
        _ => return None,
    };
    Some(canonical_entity_kind(kind))
}

fn is_generic_lookup_kind(kind: &str) -> bool {
    matches!(
        canonical_entity_kind(kind).as_str(),
        "jurisdiction"
            | "country"
            | "currency"
            | "role"
            | "status"
            | "market"
            | "user"
            | "team"
            | "security"
    )
}

/// Map a verb domain name to its primary subject kind (lowest-priority heuristic).
fn domain_to_subject_kind(domain: &str) -> String {
    match domain {
        "cbu" | "role" => "cbu".into(),
        "entity" | "entity-role" | "party" | "ownership" | "legal-entity" => "entity".into(),
        "kyc" | "kyc-case" | "screening" => "kyc-case".into(),
        "case" | "tollgate" => "kyc-case".into(),
        "deal" => "deal".into(),
        "contract" | "contract-pack" => "contract".into(),
        "billing" => "billing-profile".into(),
        "trading-profile" | "custody" | "ssi" | "mandate" => "trading-profile".into(),
        "fund" => "fund".into(),
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
                entity_type_fqn: entry_yaml
                    .entity_type_fqn
                    .map(|kind| canonical_entity_kind(&kind)),
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
                let left_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
                let right_ok = end == lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();

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

    /// Extract nouns from utterance, masking out pre-identified spans.
    ///
    /// Identical to [`extract()`] but pre-seeds the coverage array with
    /// `exclusion_spans` so that characters in those ranges are never matched
    /// by the noun scanner. This prevents entity names (e.g., "Goldman Sachs
    /// Group") from polluting the noun scan when the entity linker has already
    /// identified them.
    ///
    /// Each span is a `(start, end)` pair of **byte offsets** into the
    /// **lowercased** utterance (consistent with `extract()`).
    pub fn extract_with_exclusions(
        &self,
        utterance: &str,
        exclusion_spans: &[(usize, usize)],
    ) -> Vec<NounMatch> {
        let lower = utterance.to_lowercase();
        let mut matches: Vec<NounMatch> = Vec::new();
        let mut covered: Vec<bool> = vec![false; lower.len()];

        // Pre-seed covered array with exclusion spans
        for &(start, end) in exclusion_spans {
            let clamped_end = end.min(covered.len());
            let clamped_start = start.min(clamped_end);
            for c in &mut covered[clamped_start..clamped_end] {
                *c = true;
            }
        }

        for (alias, entry, is_canonical) in &self.sorted_aliases {
            let mut search_from = 0;
            while let Some(pos) = lower[search_from..].find(alias.as_str()) {
                let start = search_from + pos;
                let end = start + alias.len();

                let left_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
                let right_ok = end == lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();

                if left_ok && right_ok {
                    let overlaps = covered[start..end].iter().any(|&c| c);
                    if !overlaps {
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
            "create" | "add" | "new" | "register" | "establish" | "onboard" | "open"
            | "request" | "solicit" | "ask" => {
                return Some(ActionCategory::Create);
            }
            "set" => {
                if words.get(1).copied() == Some("up") {
                    return Some(ActionCategory::Create);
                }
            }
            "show" | "list" | "display" | "get" | "view" | "describe" | "see" | "read"
            | "status" | "tell" => {
                return Some(ActionCategory::List);
            }
            "update" | "change" | "modify" | "edit" | "rename" | "amend" => {
                return Some(ActionCategory::Update);
            }
            "delete" | "remove" | "drop" | "cancel" | "revoke" | "archive" | "close"
            | "withdraw" | "terminate" | "nuke" | "destroy" | "purge" => {
                return Some(ActionCategory::Delete);
            }
            "assign" | "link" | "connect" | "attach" | "map" | "bind" | "associate"
            | "appoint" | "designate" | "make" | "nominate" => {
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
    pub fn resolve(&self, nouns: &[NounMatch], action: Option<ActionCategory>, raw_utterance: Option<&str>) -> NounResolution {
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
        // Try both the ActionCategory key ("create") AND the raw first word ("request")
        // because YAML action_verbs may use either convention.
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

        // Path 1b: Try raw first word as action key (handles "request" → "request" in YAML)
        // YAML action_verbs may use raw verb words ("request", "solicit", "reject")
        // instead of ActionCategory keys ("create", "delete").
        if let Some(utterance) = raw_utterance {
            let first_word = utterance.split_whitespace().next().unwrap_or("").to_lowercase();
            if !first_word.is_empty() {
                if let Some(verbs) = primary.noun.action_verbs.get(&first_word) {
                    if !verbs.is_empty() {
                        return NounResolution {
                            candidates: verbs.clone(),
                            noun_key,
                            action,
                            resolution_path: ResolutionPath::ExplicitMapping,
                        };
                    }
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
            let canonical_kind = canonical_entity_kind(etf);
            if let Some(verbs) = self.verb_index.by_subject_kind.get(&canonical_kind) {
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
      create: [fund.create, capital.share-class.create]
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
            (
                "cbu.create",
                "cbu",
                "create",
                "Create a CBU",
                Some("cbu"),
                vec!["cbu"],
            ),
            (
                "cbu.list",
                "cbu",
                "list",
                "List CBUs",
                Some("cbu"),
                vec!["cbu"],
            ),
            (
                "cbu.delete",
                "cbu",
                "delete",
                "Delete a CBU",
                Some("cbu"),
                vec!["cbu"],
            ),
            (
                "cbu.assign-role",
                "cbu",
                "assign-role",
                "Assign role to CBU",
                Some("role"),
                vec!["cbu"],
            ),
            (
                "ubo.discover",
                "ubo",
                "discover",
                "Discover UBOs",
                Some("ubo"),
                vec!["entity"],
            ),
            (
                "ubo.list-beneficiaries",
                "ubo",
                "list-beneficiaries",
                "List beneficiaries",
                Some("ubo"),
                vec!["entity"],
            ),
            (
                "screening.run",
                "screening",
                "run",
                "Run screening",
                Some("screening"),
                vec!["entity"],
            ),
            (
                "screening.list",
                "screening",
                "list",
                "List screenings",
                Some("screening"),
                vec!["entity"],
            ),
            (
                "fund.create",
                "fund",
                "create",
                "Create share class",
                Some("fund"),
                vec!["fund"],
            ),
            (
                "fund.list-share-classes",
                "fund",
                "list-share-classes",
                "List share classes",
                Some("fund"),
                vec!["fund"],
            ),
            (
                "capital.share-class.create",
                "capital",
                "share-class.create",
                "Create share class",
                Some("capital"),
                vec!["capital"],
            ),
            (
                "session.create",
                "session",
                "create",
                "Create session",
                Some("agent_session"),
                vec!["session"],
            ),
            (
                "session.list",
                "session",
                "list",
                "List sessions",
                Some("agent_session"),
                vec!["session"],
            ),
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
                by_subject_kind
                    .entry(sk.clone())
                    .or_default()
                    .push(fqn.to_string());
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
        assert!(
            ubo_match.is_some(),
            "Should extract 'beneficial owner' → ubo"
        );
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
        let resolution = idx.resolve(&matches, action, None);

        assert_eq!(resolution.resolution_path, ResolutionPath::ExplicitMapping);
        assert_eq!(resolution.candidates, vec!["cbu.create"]);
        assert_eq!(resolution.noun_key, "cbu");
    }

    #[test]
    fn test_resolve_explicit_multi_verb() {
        let idx = test_noun_index();
        let matches = idx.extract("create a share class");
        let action = NounIndex::classify_action("create a share class");
        let resolution = idx.resolve(&matches, action, None);

        assert_eq!(resolution.resolution_path, ResolutionPath::ExplicitMapping);
        assert_eq!(resolution.candidates.len(), 2);
        assert!(resolution.candidates.contains(&"fund.create".to_string()));
        assert!(resolution
            .candidates
            .contains(&"capital.share-class.create".to_string()));
    }

    #[test]
    fn test_resolve_no_noun_empty() {
        let idx = test_noun_index();
        let matches = idx.extract("do something random");
        let resolution = idx.resolve(&matches, None, None);

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

        let resolution = idx.resolve(&matches, Some(ActionCategory::Compute), None);
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
        assert_eq!(
            ubo_match.unwrap().matched_alias,
            "ultimate beneficial owner"
        );
    }

    #[test]
    fn test_load_real_noun_index_yaml() {
        // Test that the actual noun_index.yaml parses without error
        let yaml_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/noun_index.yaml");
        if yaml_path.exists() {
            let content = std::fs::read_to_string(&yaml_path).unwrap();
            let result = NounIndex::from_yaml(
                &content,
                VerbContractIndex {
                    by_noun: HashMap::new(),
                    by_subject_kind: HashMap::new(),
                    by_fqn: HashMap::new(),
                },
            );
            assert!(
                result.is_ok(),
                "Failed to parse noun_index.yaml: {:?}",
                result.err()
            );
            let idx = result.unwrap();
            assert!(
                idx.canonical.len() >= 20,
                "Expected at least 20 canonical aliases, got {}",
                idx.canonical.len()
            );
        }
    }

    #[test]
    fn test_subject_kind_match_normalizes_kebab_and_snake_case() {
        let mut verb_index = test_verb_index();
        verb_index
            .by_subject_kind
            .insert("kyc-case".to_string(), vec!["kyc-case.create".to_string()]);

        let idx = NounIndex::from_yaml(
            r#"
version: "1.0"
nouns:
  kyc-case:
    aliases: ["kyc case"]
    entity_type_fqn: kyc_case
    noun_keys: [kyc_case]
"#,
            verb_index,
        )
        .unwrap();

        let matches = idx.extract("open a kyc case");
        let resolution = idx.resolve(&matches, Some(ActionCategory::Create), None);
        assert_eq!(resolution.resolution_path, ResolutionPath::SubjectKindMatch);
        assert_eq!(resolution.candidates, vec!["kyc-case.create".to_string()]);
    }

    #[test]
    fn test_subject_kind_hint_maps_party_and_case_domains() {
        assert_eq!(
            derive_subject_kind_from_hint("party"),
            Some("entity".to_string())
        );
        assert_eq!(
            derive_subject_kind_from_hint("tollgate_evaluations"),
            Some("kyc-case".to_string())
        );
        assert_eq!(domain_to_subject_kind("party"), "entity");
        assert_eq!(domain_to_subject_kind("case"), "kyc-case");
    }

    // -----------------------------------------------------------------------
    // extract_with_exclusions tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_exclusion_masks_entity_name_containing_noun() {
        let idx = test_noun_index();
        // "Goldman Sachs Group" contains "group" which is NOT in our test YAML,
        // but "structure" IS an alias for "cbu". Simulate an entity name
        // containing a domain noun: "Acme Structure Holdings" where "structure"
        // is a cbu alias.
        let utterance = "create acme structure holdings";
        let lower = utterance.to_lowercase();

        // Without exclusion: "structure" should match cbu
        let no_excl = idx.extract(&lower);
        let has_structure = no_excl.iter().any(|m| m.matched_alias == "structure");
        assert!(
            has_structure,
            "Without exclusion, 'structure' should match cbu alias"
        );

        // With exclusion spanning "acme structure holdings" (bytes 7..30)
        let entity_start = lower.find("acme structure holdings").unwrap();
        let entity_end = entity_start + "acme structure holdings".len();
        let with_excl = idx.extract_with_exclusions(&lower, &[(entity_start, entity_end)]);
        let still_has = with_excl.iter().any(|m| m.matched_alias == "structure");
        assert!(
            !still_has,
            "With exclusion, 'structure' inside entity span should NOT match"
        );
    }

    #[test]
    fn test_exclusion_preserves_nouns_outside_spans() {
        let idx = test_noun_index();
        // "run screening for acme structure holdings" — "screening" is outside
        // the entity span, "structure" is inside it.
        let utterance = "run screening for acme structure holdings";
        let lower = utterance.to_lowercase();

        let entity_start = lower.find("acme structure holdings").unwrap();
        let entity_end = entity_start + "acme structure holdings".len();
        let matches = idx.extract_with_exclusions(&lower, &[(entity_start, entity_end)]);

        // "screening" should still match (outside exclusion)
        let has_screening = matches.iter().any(|m| m.noun.key == "screening");
        assert!(has_screening, "Nouns outside exclusion span should match");

        // "structure" should NOT match (inside exclusion)
        let has_structure = matches.iter().any(|m| m.matched_alias == "structure");
        assert!(!has_structure, "Nouns inside exclusion span should not match");
    }

    #[test]
    fn test_exclusion_empty_spans_behaves_like_extract() {
        let idx = test_noun_index();
        let utterance = "create a new client business unit";
        let normal = idx.extract(utterance);
        let with_empty = idx.extract_with_exclusions(utterance, &[]);
        assert_eq!(normal.len(), with_empty.len());
        for (a, b) in normal.iter().zip(with_empty.iter()) {
            assert_eq!(a.noun.key, b.noun.key);
            assert_eq!(a.span, b.span);
        }
    }
}
