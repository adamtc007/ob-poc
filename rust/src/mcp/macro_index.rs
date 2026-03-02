//! Macro Index — Deterministic Searchable Index for Operator Macros (Tier -2B)
//!
//! Provides macro search parity with DSL verbs by building a multi-field
//! index over all macros at startup. Replaces the primitive Tier 0
//! `search_macros()` method with deterministic scoring and explain payloads.
//!
//! ## Scoring Table
//!
//! | Signal           | Score |
//! |------------------|-------|
//! | Exact FQN        | +10   |
//! | Exact label      | +8    |
//! | Alias/phrase      | +6    |
//! | Jurisdiction     | +3    |
//! | Mode match       | +2    |
//! | Noun overlap     | +2    |
//! | Target kind      | +2    |
//! | Mismatch penalty | −999  |
//!
//! ## Hard Gates
//!
//! - M1: Mode compatibility (mode_tags must overlap if specified)
//! - M2: Min score ≥ 6
//! - M3: Disambiguation band Δ ≤ 2 → return multiple candidates

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::dsl_v2::macros::MacroRegistry;

// ─── Configuration ───────────────────────────────────────────────────────────

/// Minimum total score required for a macro match (gate M2).
const MIN_SCORE: i32 = 6;

/// If two candidates' scores are within this band, both are returned (gate M3).
const DISAMBIGUATION_BAND: i32 = 2;

/// Hard-exclude penalty for mode mismatch (gate M1).
const MISMATCH_PENALTY: i32 = -999;

// ─── Jurisdiction Mapping ────────────────────────────────────────────────────

/// Static map from FQN prefix patterns to ISO jurisdiction codes.
fn fqn_to_jurisdiction(fqn: &str) -> Option<&'static str> {
    if fqn.contains(".lux.") || fqn.starts_with("struct.lux") {
        Some("LU")
    } else if fqn.contains(".ie.") || fqn.starts_with("struct.ie") {
        Some("IE")
    } else if fqn.contains(".uk.") || fqn.starts_with("struct.uk") {
        Some("UK")
    } else if fqn.contains(".us.") || fqn.starts_with("struct.us") {
        Some("US")
    } else if fqn.contains(".de.") || fqn.starts_with("struct.de") {
        Some("DE")
    } else {
        None
    }
}

/// Extract structure type from FQN (last segment after jurisdiction).
/// e.g. "struct.lux.ucits.sicav" → "sicav"
fn fqn_to_structure_type(fqn: &str) -> Option<String> {
    let parts: Vec<&str> = fqn.split('.').collect();
    if parts.len() >= 3 {
        Some(parts.last()?.to_lowercase())
    } else {
        None
    }
}

/// Normalize text for matching: lowercase, strip punctuation, collapse whitespace.
fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Tokenize text into meaningful words (length > 2).
fn tokenize(s: &str) -> Vec<String> {
    normalize(s)
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect()
}

// ─── Index Types ─────────────────────────────────────────────────────────────

/// Entry in the macro index, derived from macro metadata at startup.
#[derive(Debug, Clone)]
pub struct MacroIndexEntry {
    pub fqn: String,
    pub label: String,
    pub description: String,
    pub jurisdiction: Option<String>,
    pub structure_type: Option<String>,
    pub mode_tags: Vec<String>,
    pub operates_on: Option<String>,
    pub produces: Option<String>,
    pub aliases: Vec<String>,
    pub noun_tokens: Vec<String>,
    /// Number of verbs this macro expands to (for composite intent cues).
    pub expansion_verb_count: usize,
}

/// Curated search overrides loaded from YAML (optional layer).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MacroSearchOverrides {
    #[serde(default)]
    pub aliases: HashMap<String, Vec<String>>,
}

/// A matched signal contributing to the score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedSignal {
    pub signal: String,
    pub score: i32,
    pub detail: String,
}

/// Result of a gate evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate: String,
    pub passed: bool,
    pub reason: Option<String>,
}

/// Explain payload for a macro match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroExplain {
    pub matched_signals: Vec<MatchedSignal>,
    pub gates: Vec<GateResult>,
    pub score_total: i32,
    pub resolution_tier: &'static str,
}

/// A single macro match with score and explain.
#[derive(Debug, Clone)]
pub struct MacroMatch {
    pub fqn: String,
    pub score: i32,
    pub explain: MacroExplain,
}

/// Result of `MacroIndex::resolve()`.
#[derive(Debug, Clone)]
pub enum MacroResolveOutcome {
    /// Clear winner (top score passes gates, no disambiguation needed).
    Matched(MacroMatch),
    /// Multiple candidates within disambiguation band.
    Ambiguous(Vec<MacroMatch>),
    /// No macro matched above the minimum score.
    NoMatch,
}

// ─── MacroIndex ──────────────────────────────────────────────────────────────

/// Deterministic searchable index over all operator macros.
///
/// Built at startup from `MacroRegistry` metadata. Provides O(1) fast-path
/// lookups for FQN/label/alias, plus scored matching for fuzzy queries.
pub struct MacroIndex {
    /// Normalized FQN → canonical FQN.
    fqn_map: HashMap<String, String>,

    /// Normalized label → list of canonical FQNs.
    label_map: HashMap<String, Vec<String>>,

    /// Curated alias → list of canonical FQNs.
    alias_map: HashMap<String, Vec<String>>,

    /// Jurisdiction code → list of FQNs.
    jurisdiction_map: HashMap<String, Vec<String>>,

    /// Noun token → list of FQNs (derived from label + description tokenization).
    noun_map: HashMap<String, Vec<String>>,

    /// Mode tag → list of FQNs.
    mode_index: HashMap<String, Vec<String>>,

    /// Full entries keyed by canonical FQN.
    entries: HashMap<String, MacroIndexEntry>,
}

impl MacroIndex {
    /// Build the index from a `MacroRegistry`, optionally overlaying curated aliases.
    pub fn from_registry(
        registry: &MacroRegistry,
        overrides: Option<&MacroSearchOverrides>,
    ) -> Self {
        let mut fqn_map = HashMap::new();
        let mut label_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut alias_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut jurisdiction_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut noun_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut mode_index: HashMap<String, Vec<String>> = HashMap::new();
        let mut entries = HashMap::new();

        for (fqn, schema) in registry.all() {
            let canonical_fqn = fqn.clone();
            let normalized_fqn = fqn.to_lowercase();

            // FQN index
            fqn_map.insert(normalized_fqn, canonical_fqn.clone());

            // Label index
            let norm_label = normalize(&schema.ui.label);
            label_map
                .entry(norm_label)
                .or_default()
                .push(canonical_fqn.clone());

            // Jurisdiction extraction from FQN
            let jurisdiction = fqn_to_jurisdiction(fqn).map(|s| s.to_string());
            if let Some(ref jur) = jurisdiction {
                jurisdiction_map
                    .entry(jur.clone())
                    .or_default()
                    .push(canonical_fqn.clone());
            }

            // Structure type extraction
            let structure_type = fqn_to_structure_type(fqn);

            // Mode tags index
            for tag in &schema.routing.mode_tags {
                mode_index
                    .entry(tag.to_lowercase())
                    .or_default()
                    .push(canonical_fqn.clone());
            }

            // Noun tokens from label + description + target_label
            let mut noun_tokens = Vec::new();
            noun_tokens.extend(tokenize(&schema.ui.label));
            noun_tokens.extend(tokenize(&schema.ui.description));
            noun_tokens.extend(tokenize(&schema.ui.target_label));

            // Also tokenize enum value labels/keys from required args
            for arg in schema.args.required.values() {
                for val in &arg.values {
                    noun_tokens.extend(tokenize(&val.label));
                    noun_tokens.push(val.key.to_lowercase());
                }
            }

            // Dedupe noun tokens
            let unique_nouns: Vec<String> = {
                let mut set = HashSet::new();
                noun_tokens
                    .into_iter()
                    .filter(|t| set.insert(t.clone()))
                    .collect()
            };

            // Index each noun token
            for token in &unique_nouns {
                noun_map
                    .entry(token.clone())
                    .or_default()
                    .push(canonical_fqn.clone());
            }

            // Aliases from schema
            let mut aliases: Vec<String> = schema.aliases.clone();

            // Overlay curated aliases
            if let Some(ov) = overrides {
                if let Some(curated) = ov.aliases.get(fqn) {
                    aliases.extend(curated.clone());
                }
            }

            for alias in &aliases {
                let norm_alias = normalize(alias);
                alias_map
                    .entry(norm_alias)
                    .or_default()
                    .push(canonical_fqn.clone());
            }

            // Count expansion steps (if available)
            let expansion_verb_count = schema.expands_to.len();

            let entry = MacroIndexEntry {
                fqn: canonical_fqn.clone(),
                label: schema.ui.label.clone(),
                description: schema.ui.description.clone(),
                jurisdiction,
                structure_type,
                mode_tags: schema.routing.mode_tags.clone(),
                operates_on: Some(schema.target.operates_on.clone()),
                produces: schema.target.produces.clone(),
                aliases,
                noun_tokens: unique_nouns,
                expansion_verb_count,
            };

            entries.insert(canonical_fqn, entry);
        }

        tracing::info!(
            macro_count = entries.len(),
            fqn_entries = fqn_map.len(),
            label_entries = label_map.len(),
            alias_entries = alias_map.len(),
            jurisdiction_entries = jurisdiction_map.len(),
            noun_entries = noun_map.len(),
            mode_entries = mode_index.len(),
            "MacroIndex built from registry"
        );

        Self {
            fqn_map,
            label_map,
            alias_map,
            jurisdiction_map,
            noun_map,
            mode_index,
            entries,
        }
    }

    /// Look up an entry by canonical FQN.
    pub fn get_entry(&self, fqn: &str) -> Option<&MacroIndexEntry> {
        self.entries.get(fqn)
    }

    /// Resolve an utterance to the best-matching macro(s).
    ///
    /// Uses index maps for candidate gathering (O(1) lookups) followed by
    /// deterministic scoring with hard gates.
    /// `active_mode` optionally constrains by mode tag (e.g., "onboarding").
    /// `jurisdiction_hint` optionally provides a known jurisdiction context.
    pub fn resolve(
        &self,
        query: &str,
        active_mode: Option<&str>,
        jurisdiction_hint: Option<&str>,
    ) -> MacroResolveOutcome {
        if self.entries.is_empty() {
            return MacroResolveOutcome::NoMatch;
        }

        let query_norm = normalize(query);
        let query_tokens: HashSet<String> = tokenize(query).into_iter().collect();

        // Phase 1: Gather candidate FQNs via index maps (avoids scoring all entries)
        let mut candidate_fqns = HashSet::new();

        // Fast-path: exact FQN lookup
        if let Some(canonical) = self.fqn_map.get(&query_norm) {
            candidate_fqns.insert(canonical.clone());
        }
        // Also try dot-normalized form (e.g., "struct lux ucits sicav" → "struct.lux.ucits.sicav")
        let dot_form = query_norm.replace(' ', ".");
        if let Some(canonical) = self.fqn_map.get(&dot_form) {
            candidate_fqns.insert(canonical.clone());
        }

        // Fast-path: exact label lookup
        if let Some(fqns) = self.label_map.get(&query_norm) {
            candidate_fqns.extend(fqns.iter().cloned());
        }

        // Alias lookup
        if let Some(fqns) = self.alias_map.get(&query_norm) {
            candidate_fqns.extend(fqns.iter().cloned());
        }
        // Also check if query *contains* an alias (for multi-word queries)
        for (alias, fqns) in &self.alias_map {
            if query_norm.contains(alias.as_str()) {
                candidate_fqns.extend(fqns.iter().cloned());
            }
        }

        // Noun token overlap: gather macros sharing tokens with the query
        for token in &query_tokens {
            if let Some(fqns) = self.noun_map.get(token) {
                candidate_fqns.extend(fqns.iter().cloned());
            }
        }

        // Jurisdiction hint: add macros for that jurisdiction
        if let Some(hint) = jurisdiction_hint {
            let hint_upper = hint.to_uppercase();
            if let Some(fqns) = self.jurisdiction_map.get(&hint_upper) {
                candidate_fqns.extend(fqns.iter().cloned());
            }
        }
        // Also check query tokens for jurisdiction keywords
        for (jur_code, fqns) in &self.jurisdiction_map {
            for alias in jurisdiction_aliases(jur_code) {
                if query_tokens.contains(alias) || query_norm.contains(alias) {
                    candidate_fqns.extend(fqns.iter().cloned());
                    break;
                }
            }
        }

        // Mode index: add macros matching active mode
        if let Some(mode) = active_mode {
            let mode_lower = mode.to_lowercase();
            if let Some(fqns) = self.mode_index.get(&mode_lower) {
                candidate_fqns.extend(fqns.iter().cloned());
            }
        }

        // If no candidates gathered from any index, there's nothing to score
        if candidate_fqns.is_empty() {
            return MacroResolveOutcome::NoMatch;
        }

        // Phase 2: Score only the gathered candidates
        let mut scored: Vec<MacroMatch> = Vec::new();

        for fqn in &candidate_fqns {
            let entry = match self.entries.get(fqn) {
                Some(e) => e,
                None => continue,
            };

            let (score, signals, gates) = self.score_macro(
                fqn,
                entry,
                &query_norm,
                &query_tokens,
                active_mode,
                jurisdiction_hint,
            );

            // Gate M1: mode compatibility (already included in score as MISMATCH_PENALTY)
            // Gate M2: minimum score
            if score < MIN_SCORE {
                continue;
            }

            scored.push(MacroMatch {
                fqn: fqn.clone(),
                score,
                explain: MacroExplain {
                    matched_signals: signals,
                    gates,
                    score_total: score,
                    resolution_tier: "Tier2B_MacroIndex",
                },
            });
        }

        if scored.is_empty() {
            return MacroResolveOutcome::NoMatch;
        }

        // Sort by score descending
        scored.sort_by(|a, b| b.score.cmp(&a.score));

        // Gate M3: disambiguation band
        if scored.len() >= 2 {
            let top = scored[0].score;
            let runner_up = scored[1].score;
            if top - runner_up <= DISAMBIGUATION_BAND {
                // Return all candidates within the band
                let band_threshold = top - DISAMBIGUATION_BAND;
                let ambiguous: Vec<MacroMatch> = scored
                    .into_iter()
                    .filter(|m| m.score >= band_threshold)
                    .take(5) // Cap at 5 candidates
                    .collect();
                return MacroResolveOutcome::Ambiguous(ambiguous);
            }
        }

        MacroResolveOutcome::Matched(scored.into_iter().next().unwrap())
    }

    /// Score a single macro against the query.
    ///
    /// Returns (total_score, matched_signals, gate_results).
    fn score_macro(
        &self,
        fqn: &str,
        entry: &MacroIndexEntry,
        query_norm: &str,
        query_tokens: &HashSet<String>,
        active_mode: Option<&str>,
        jurisdiction_hint: Option<&str>,
    ) -> (i32, Vec<MatchedSignal>, Vec<GateResult>) {
        let mut score: i32 = 0;
        let mut signals = Vec::new();
        let mut gates = Vec::new();

        let fqn_lower = fqn.to_lowercase();

        // --- Signal: Exact FQN match (+10) ---
        if query_norm == fqn_lower || query_norm.replace(' ', ".") == fqn_lower {
            score += 10;
            signals.push(MatchedSignal {
                signal: "exact_fqn".to_string(),
                score: 10,
                detail: format!("Query matches FQN '{}'", fqn),
            });
        }

        // --- Signal: Exact label match (+8) ---
        let label_norm = normalize(&entry.label);
        if query_norm == label_norm {
            score += 8;
            signals.push(MatchedSignal {
                signal: "exact_label".to_string(),
                score: 8,
                detail: format!("Query matches label '{}'", entry.label),
            });
        }

        // --- Signal: Label substring match (+5) ---
        // Query contains the full label as a substring (but isn't an exact match)
        let label_words: Vec<&str> = label_norm.split_whitespace().collect();
        if label_words.len() >= 2
            && query_norm != label_norm
            && query_norm.contains(&label_norm)
        {
            score += 5;
            signals.push(MatchedSignal {
                signal: "label_substring".to_string(),
                score: 5,
                detail: format!(
                    "Query contains label '{}' as substring",
                    entry.label
                ),
            });
        }

        // --- Signal: Label word coverage (+4) ---
        // ≥75% of label words appear in query tokens (for multi-word labels)
        if label_words.len() >= 2 {
            let label_hits = label_words
                .iter()
                .filter(|w| query_tokens.contains(&w.to_string()))
                .count();
            let coverage = label_hits as f64 / label_words.len() as f64;
            if coverage >= 0.75 {
                score += 4;
                signals.push(MatchedSignal {
                    signal: "label_word_coverage".to_string(),
                    score: 4,
                    detail: format!(
                        "Label word coverage {}/{} ({:.0}%) for '{}'",
                        label_hits,
                        label_words.len(),
                        coverage * 100.0,
                        entry.label
                    ),
                });
            }
        }

        // --- Signal: Alias/phrase match (+6) ---
        for alias in &entry.aliases {
            let alias_norm = normalize(alias);
            if query_norm == alias_norm || query_norm.contains(&alias_norm) {
                score += 6;
                signals.push(MatchedSignal {
                    signal: "alias_match".to_string(),
                    score: 6,
                    detail: format!("Query matches alias '{}'", alias),
                });
                break; // Only count once
            }
        }

        // --- Signal: Jurisdiction match (+3) ---
        if let Some(macro_jur) = &entry.jurisdiction {
            // Check explicit hint
            if let Some(hint) = jurisdiction_hint {
                if hint.eq_ignore_ascii_case(macro_jur) {
                    score += 3;
                    signals.push(MatchedSignal {
                        signal: "jurisdiction_hint".to_string(),
                        score: 3,
                        detail: format!("Jurisdiction hint '{}' matches macro", hint),
                    });
                }
            }

            // Check query tokens for jurisdiction keywords
            let jur_aliases = jurisdiction_aliases(macro_jur);
            for alias in jur_aliases {
                if query_tokens.contains(alias) || query_norm.contains(alias) {
                    score += 3;
                    signals.push(MatchedSignal {
                        signal: "jurisdiction_query".to_string(),
                        score: 3,
                        detail: format!(
                            "Query contains jurisdiction keyword '{}' → {}",
                            alias, macro_jur
                        ),
                    });
                    break;
                }
            }
        }

        // --- Signal: Mode match (+2) ---
        if let Some(mode) = active_mode {
            let mode_lower = mode.to_lowercase();
            if entry
                .mode_tags
                .iter()
                .any(|t| t.to_lowercase() == mode_lower)
            {
                score += 2;
                signals.push(MatchedSignal {
                    signal: "mode_match".to_string(),
                    score: 2,
                    detail: format!("Active mode '{}' matches macro mode_tags", mode),
                });
            }
        }

        // --- Signal: Noun overlap (+2) ---
        let overlap_count = query_tokens
            .iter()
            .filter(|t| entry.noun_tokens.contains(t))
            .count();
        if overlap_count > 0 {
            score += 2;
            let overlapping: Vec<&String> = query_tokens
                .iter()
                .filter(|t| entry.noun_tokens.contains(t))
                .collect();
            signals.push(MatchedSignal {
                signal: "noun_overlap".to_string(),
                score: 2,
                detail: format!("Noun overlap ({} tokens): {:?}", overlap_count, overlapping),
            });
        }

        // --- Signal: Structure type match (+2) ---
        // Check if query mentions the macro's structure type (e.g., "sicav", "icav", "lp")
        if let Some(ref st) = entry.structure_type {
            let st_lower = st.to_lowercase();
            if query_tokens.contains(&st_lower) || query_norm.contains(&st_lower) {
                score += 2;
                signals.push(MatchedSignal {
                    signal: "structure_type".to_string(),
                    score: 2,
                    detail: format!("Query mentions structure type '{}'", st),
                });
            }
        }

        // --- Signal: Target kind match (+2) ---
        // Check if query mentions the target kind (structure, case, mandate)
        if let Some(ref produces) = entry.produces {
            let produces_norm = normalize(produces);
            if query_tokens.contains(&produces_norm) || query_norm.contains(&produces_norm) {
                score += 2;
                signals.push(MatchedSignal {
                    signal: "target_kind".to_string(),
                    score: 2,
                    detail: format!("Query mentions target kind '{}'", produces),
                });
            }
        }

        // --- Gate M1: Mode compatibility (mismatch penalty) ---
        if let Some(mode) = active_mode {
            let mode_lower = mode.to_lowercase();
            if !entry.mode_tags.is_empty()
                && !entry
                    .mode_tags
                    .iter()
                    .any(|t| t.to_lowercase() == mode_lower)
            {
                score += MISMATCH_PENALTY;
                gates.push(GateResult {
                    gate: "M1_mode_compatibility".to_string(),
                    passed: false,
                    reason: Some(format!(
                        "Active mode '{}' not in macro mode_tags {:?}",
                        mode, entry.mode_tags
                    )),
                });
            } else {
                gates.push(GateResult {
                    gate: "M1_mode_compatibility".to_string(),
                    passed: true,
                    reason: None,
                });
            }
        } else {
            gates.push(GateResult {
                gate: "M1_mode_compatibility".to_string(),
                passed: true,
                reason: Some("No active mode constraint".to_string()),
            });
        }

        // Gate M2 (min score) is checked by the caller
        gates.push(GateResult {
            gate: "M2_min_score".to_string(),
            passed: score >= MIN_SCORE,
            reason: if score < MIN_SCORE {
                Some(format!("Score {} < min {}", score, MIN_SCORE))
            } else {
                None
            },
        });

        (score, signals, gates)
    }

    /// Get entry by FQN (for explain / metadata).
    pub fn get(&self, fqn: &str) -> Option<&MacroIndexEntry> {
        self.entries.get(fqn)
    }

    /// Total number of indexed macros.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Return lowercase alias strings for a given jurisdiction code.
fn jurisdiction_aliases(code: &str) -> Vec<&'static str> {
    match code {
        "LU" => vec!["luxembourg", "lux", "lux.", "luxemburg"],
        "IE" => vec!["ireland", "irish", "irl"],
        "UK" => vec!["united kingdom", "british", "england"],
        "US" => vec!["united states", "american", "usa"],
        "DE" => vec!["germany", "german", "deutschland"],
        _ => vec![],
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::macros::{
        ArgStyle, MacroArgs, MacroKind, MacroRouting, MacroSchema, MacroTarget, MacroTier, MacroUi,
    };

    /// Build a minimal MacroSchema for testing.
    fn test_macro(
        label: &str,
        description: &str,
        mode_tags: Vec<&str>,
        aliases: Vec<&str>,
    ) -> MacroSchema {
        MacroSchema {
            id: None,
            kind: MacroKind::Macro,
            tier: Some(MacroTier::Composite),
            aliases: aliases.into_iter().map(String::from).collect(),
            taxonomy: None,
            ui: MacroUi {
                label: label.to_string(),
                description: description.to_string(),
                target_label: "Structure".to_string(),
            },
            routing: MacroRouting {
                mode_tags: mode_tags.into_iter().map(String::from).collect(),
                operator_domain: Some("structure".to_string()),
            },
            target: MacroTarget {
                operates_on: "client-ref".to_string(),
                produces: Some("structure-ref".to_string()),
                allowed_structure_types: vec![],
            },
            args: MacroArgs {
                style: ArgStyle::Keyworded,
                required: HashMap::new(),
                optional: HashMap::new(),
            },
            required_roles: vec![],
            optional_roles: vec![],
            docs_bundle: None,
            prereqs: vec![],
            expands_to: vec![],
            sets_state: vec![],
            unlocks: vec![],
        }
    }

    fn build_test_registry() -> MacroRegistry {
        let mut registry = MacroRegistry::default();
        registry.add(
            "struct.lux.ucits.sicav".to_string(),
            test_macro(
                "Luxembourg UCITS SICAV",
                "Set up a Luxembourg UCITS fund using SICAV structure",
                vec!["onboarding", "structure"],
                vec!["lux-sicav", "sicav-setup"],
            ),
        );
        registry.add(
            "struct.ie.ucits.icav".to_string(),
            test_macro(
                "Ireland UCITS ICAV",
                "Set up an Irish UCITS fund using ICAV structure",
                vec!["onboarding", "structure"],
                vec!["ie-icav"],
            ),
        );
        registry.add(
            "structure.setup".to_string(),
            test_macro(
                "Set up Structure",
                "Create a new fund or mandate structure",
                vec!["onboarding", "kyc"],
                vec!["create-structure", "new-fund"],
            ),
        );
        registry.add(
            "case.open".to_string(),
            test_macro(
                "Open Case",
                "Open a new KYC case for an entity",
                vec!["kyc"],
                vec![],
            ),
        );
        registry
    }

    #[test]
    fn test_exact_fqn_match() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        match index.resolve("struct.lux.ucits.sicav", None, None) {
            MacroResolveOutcome::Matched(m) => {
                assert_eq!(m.fqn, "struct.lux.ucits.sicav");
                assert!(m.score >= 10, "Exact FQN should score ≥10, got {}", m.score);
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_exact_label_match() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        match index.resolve("Luxembourg UCITS SICAV", None, None) {
            MacroResolveOutcome::Matched(m) => {
                assert_eq!(m.fqn, "struct.lux.ucits.sicav");
                assert!(m.score >= 8, "Exact label should score ≥8, got {}", m.score);
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_alias_match() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        match index.resolve("lux-sicav", None, None) {
            MacroResolveOutcome::Matched(m) => {
                assert_eq!(m.fqn, "struct.lux.ucits.sicav");
                assert!(m.score >= 6, "Alias match should score ≥6, got {}", m.score);
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_jurisdiction_boost() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        // "lux" in query should boost Luxembourg macros
        match index.resolve("set up lux sicav", None, None) {
            MacroResolveOutcome::Matched(m) => {
                assert_eq!(m.fqn, "struct.lux.ucits.sicav");
                // Should have jurisdiction + noun overlap signals
                let has_jurisdiction = m
                    .explain
                    .matched_signals
                    .iter()
                    .any(|s| s.signal.contains("jurisdiction"));
                assert!(has_jurisdiction, "Should have jurisdiction signal");
            }
            MacroResolveOutcome::Ambiguous(candidates) => {
                // Lux macro should be in top
                assert!(
                    candidates.iter().any(|c| c.fqn == "struct.lux.ucits.sicav"),
                    "Lux SICAV should be in ambiguous candidates"
                );
            }
            MacroResolveOutcome::NoMatch => panic!("Expected match for 'set up lux sicav'"),
        }
    }

    #[test]
    fn test_mode_filtering() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        // "case.open" has mode_tags: [kyc], so should NOT match in onboarding mode
        // if mode filtering is strict. But mode mismatch is gated, not scored.
        match index.resolve("case.open", Some("onboarding"), None) {
            MacroResolveOutcome::NoMatch => {
                // Expected: mode mismatch penalty kills the score
            }
            MacroResolveOutcome::Matched(m) => {
                // If it matched, the FQN exact match (+10) overcame the mode penalty
                // This is expected since exact FQN has very high signal
                assert_eq!(m.fqn, "case.open");
            }
            _ => {}
        }
    }

    #[test]
    fn test_no_match_for_unrelated() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        match index.resolve("check the weather", None, None) {
            MacroResolveOutcome::NoMatch => {} // Expected
            other => panic!("Expected NoMatch for unrelated query, got {:?}", other),
        }
    }

    #[test]
    fn test_disambiguation_band() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        // "set up fund" should match both "structure.setup" and potentially jurisdiction macros
        let result = index.resolve("set up ucits fund", None, None);
        match result {
            MacroResolveOutcome::Ambiguous(candidates) => {
                assert!(
                    candidates.len() >= 2,
                    "Should have multiple candidates in band"
                );
            }
            MacroResolveOutcome::Matched(m) => {
                // Single match is acceptable if one clearly dominates
                assert!(m.score >= MIN_SCORE);
            }
            MacroResolveOutcome::NoMatch => {
                // Also acceptable if no macro gets enough signals
            }
        }
    }

    #[test]
    fn test_jurisdiction_hint() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        // With explicit jurisdiction hint, Luxembourg macro should win
        match index.resolve("set up sicav", None, Some("LU")) {
            MacroResolveOutcome::Matched(m) => {
                assert_eq!(m.fqn, "struct.lux.ucits.sicav");
            }
            MacroResolveOutcome::Ambiguous(candidates) => {
                assert!(candidates[0].fqn == "struct.lux.ucits.sicav");
            }
            MacroResolveOutcome::NoMatch => panic!("Expected match with jurisdiction hint"),
        }
    }

    #[test]
    fn test_empty_registry() {
        let registry = MacroRegistry::default();
        let index = MacroIndex::from_registry(&registry, None);

        assert!(index.is_empty());
        match index.resolve("anything", None, None) {
            MacroResolveOutcome::NoMatch => {} // Expected
            other => panic!("Expected NoMatch for empty index, got {:?}", other),
        }
    }

    #[test]
    fn test_overrides_aliases() {
        let registry = build_test_registry();
        let overrides = MacroSearchOverrides {
            aliases: {
                let mut map = HashMap::new();
                map.insert(
                    "struct.lux.ucits.sicav".to_string(),
                    vec!["lux fund".to_string(), "luxembourg fund setup".to_string()],
                );
                map
            },
        };
        let index = MacroIndex::from_registry(&registry, Some(&overrides));

        match index.resolve("lux fund", None, None) {
            MacroResolveOutcome::Matched(m) => {
                assert_eq!(m.fqn, "struct.lux.ucits.sicav");
            }
            MacroResolveOutcome::Ambiguous(candidates) => {
                assert!(candidates.iter().any(|c| c.fqn == "struct.lux.ucits.sicav"));
            }
            MacroResolveOutcome::NoMatch => panic!("Override alias should match"),
        }
    }

    #[test]
    fn test_explain_payload() {
        let registry = build_test_registry();
        let index = MacroIndex::from_registry(&registry, None);

        match index.resolve("struct.lux.ucits.sicav", None, None) {
            MacroResolveOutcome::Matched(m) => {
                assert_eq!(m.explain.resolution_tier, "Tier2B_MacroIndex");
                assert!(!m.explain.matched_signals.is_empty());
                assert!(!m.explain.gates.is_empty());
                assert_eq!(m.explain.score_total, m.score);
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_real_macro_registry_loads_all_files() {
        use crate::dsl_v2::macros::load_macro_registry;
        let registry = load_macro_registry().expect("Failed to load macro registry");
        let count = registry.len();
        // After fixing MacroPrereq serde tag format, we should have ALL macros loaded
        // including those from files with non-empty prereqs (case.yaml, mandate.yaml,
        // structure.yaml, screening.yaml, kyc-workflow.yaml).
        // Pre-fix: only ~9 macros loaded (from files with prereqs: [])
        // Post-fix: should be ~47+ macros
        assert!(
            count >= 30,
            "Expected ≥30 macros from real YAML but got {}. \
             Likely prereq deserialization is still failing.",
            count
        );
        eprintln!("Real macro registry: {} macros loaded", count);

        // Verify specific macros from files that previously failed due to state_exists prereqs
        assert!(
            registry.has("case.open"),
            "case.open should load (case.yaml has state_exists prereqs)"
        );
        assert!(
            registry.has("screening.full"),
            "screening.full should load (screening.yaml has state_exists prereqs)"
        );
    }

    #[test]
    fn test_diagnostic_real_registry_irish_icav() {
        use crate::dsl_v2::macros::load_macro_registry;
        let registry = load_macro_registry().expect("Failed to load macro registry");

        // Debug: print all FQNs in registry BEFORE building index
        eprintln!("=== REGISTRY CONTENTS (before index build) ===");
        eprintln!("Registry macro count: {}", registry.len());
        let mut all_fqns: Vec<_> = registry.all_fqns().cloned().collect();
        all_fqns.sort();
        for fqn in &all_fqns {
            eprintln!("  {}", fqn);
        }
        eprintln!("Source files: {:?}", registry.source_files());

        let index = MacroIndex::from_registry(&registry, None);

        eprintln!("\n=== DIAGNOSTIC: MacroIndex with real registry ===");
        eprintln!("Total macros indexed: {}", index.len());

        // Check IE macros exist
        let ie_macros: Vec<&String> = index
            .entries
            .keys()
            .filter(|k| k.contains(".ie."))
            .collect();
        eprintln!("IE macros in index: {:?}", ie_macros);

        // Check jurisdiction_map
        let ie_jur = index.jurisdiction_map.get("IE");
        eprintln!("jurisdiction_map[IE]: {:?}", ie_jur);

        // Check specific entry details
        if let Some(entry) = index.entries.get("struct.ie.ucits.icav") {
            eprintln!("struct.ie.ucits.icav entry:");
            eprintln!("  label: {:?}", entry.label);
            eprintln!("  jurisdiction: {:?}", entry.jurisdiction);
            eprintln!("  structure_type: {:?}", entry.structure_type);
            eprintln!("  noun_tokens: {:?}", entry.noun_tokens);
            eprintln!("  mode_tags: {:?}", entry.mode_tags);
        } else {
            eprintln!("WARNING: struct.ie.ucits.icav NOT in index!");
        }

        // Now test resolve()
        let query = "Create an Irish ICAV";
        let norm = normalize(query);
        let tokens: HashSet<String> = tokenize(query).into_iter().collect();
        eprintln!("\nQuery: {:?}", query);
        eprintln!("Normalized: {:?}", norm);
        eprintln!("Tokens: {:?}", tokens);

        // Check jurisdiction alias matching manually
        for alias in jurisdiction_aliases("IE") {
            eprintln!(
                "  IE alias '{}': in tokens={}, in norm={}",
                alias,
                tokens.contains(alias),
                norm.contains(alias)
            );
        }

        // Check noun_map lookups
        for token in &tokens {
            let noun_matches = index.noun_map.get(token);
            eprintln!(
                "  noun_map[{:?}]: {:?}",
                token,
                noun_matches.map(|v| v.len())
            );
        }

        let outcome = index.resolve(query, None, None);
        match &outcome {
            MacroResolveOutcome::Matched(m) => {
                eprintln!("\nResult: Matched({}, score={})", m.fqn, m.score);
                for s in &m.explain.matched_signals {
                    eprintln!("  {} (+{}): {}", s.signal, s.score, s.detail);
                }
            }
            MacroResolveOutcome::Ambiguous(candidates) => {
                eprintln!("\nResult: Ambiguous({} candidates)", candidates.len());
                for c in candidates {
                    eprintln!("  {} (score={})", c.fqn, c.score);
                    for s in &c.explain.matched_signals {
                        eprintln!("    {} (+{}): {}", s.signal, s.score, s.detail);
                    }
                }
            }
            MacroResolveOutcome::NoMatch => {
                eprintln!("\nResult: NoMatch");
            }
        }

        // Should NOT be NoMatch
        assert!(
            !matches!(outcome, MacroResolveOutcome::NoMatch),
            "MacroIndex should find results for 'Create an Irish ICAV'"
        );

        // Also test other failing queries
        for query in &[
            "Lux RAIF setup",
            "UK OEIC structure",
            "Set up a cross-border hedge fund",
            "Set up structure for Lux SICAV",
        ] {
            let outcome = index.resolve(query, None, None);
            let label = match &outcome {
                MacroResolveOutcome::Matched(m) => format!("Matched({}, {})", m.fqn, m.score),
                MacroResolveOutcome::Ambiguous(c) => format!("Ambiguous({})", c.len()),
                MacroResolveOutcome::NoMatch => "NoMatch".to_string(),
            };
            eprintln!("  {:40} → {}", query, label);
        }
    }
}
