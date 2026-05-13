//! SemTaxonomy replacement contracts.
//!
//! These types define the replacement path for utterance grounding, discovery,
//! and runbook composition without depending on the legacy Sage/Coder structs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SageSession {
    pub session_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub cascade_result: Option<Value>,
    pub active_entity: Option<EntityRef>,
    pub entity_candidates: Vec<EntityCandidate>,
    pub domain_scope: Option<String>,
    pub aspect: Option<String>,
    pub verb_surface: Vec<VerbSurfaceEntry>,
    pub entity_state: Option<Value>,
    pub domain_state_summaries: Vec<DomainStateSummary>,
    pub data_snapshots: HashMap<String, Value>,
    pub utterance_history: Vec<String>,
    pub research_cache: HashMap<String, Value>,
    pub likely_intents: Vec<IntentHint>,
    pub grounding_strategy: Option<String>,
    pub grounding_confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCandidate {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub name: String,
    pub match_score: f64,
    pub match_field: Option<String>,
    pub summary: Option<Value>,
    pub source_kind: Option<String>,
    pub linked_cbu_ids: Vec<Uuid>,
    pub is_onboarding_member: bool,
    pub candidate_for_cbu: bool,
    pub lifecycle_populated: bool,
    pub linked_entity_count: usize,
    pub has_active_workflow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSurfaceEntry {
    pub verb_id: String,
    pub domain: String,
    pub name: String,
    pub description: String,
    pub polarity: String,
    pub phase_tags: Vec<String>,
    pub subject_kinds: Vec<String>,
    pub parameters: Vec<VerbParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbParameter {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentHint {
    pub intent: String,
    pub confidence: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainStateSummary {
    pub domain: String,
    pub active_count: usize,
    pub blocked_count: usize,
    pub notable_gaps: Vec<String>,
    pub next_action_candidates: Vec<String>,
}

fn stop_vocabulary() -> &'static HashSet<&'static str> {
    static STOP_VOCABULARY: OnceLock<HashSet<&'static str>> = OnceLock::new();
    STOP_VOCABULARY.get_or_init(|| {
        [
            "show",
            "list",
            "what",
            "which",
            "read",
            "view",
            "inspect",
            "describe",
            "display",
            "create",
            "add",
            "update",
            "change",
            "delete",
            "remove",
            "assign",
            "set",
            "open",
            "who",
            "how",
            "can",
            "do",
            "does",
            "is",
            "are",
            "was",
            "were",
            "will",
            "would",
            "could",
            "run",
            "check",
            "tell",
            "find",
            "get",
            "give",
            "look",
            "search",
            "help",
            "move",
            "progress",
            "forward",
            "next",
            "start",
            "begin",
            "continue",
            "resume",
            "verify",
            "validate",
            "review",
            "approve",
            "reject",
            "close",
            "complete",
            "need",
            "want",
            "like",
            "should",
            "must",
            "try",
            "able",
            "owns",
            "own",
            "onboard",
            "cbu",
            "cbus",
            "deal",
            "deals",
            "document",
            "documents",
            "doc",
            "docs",
            "entity",
            "entities",
            "onboarding",
            "screening",
            "sanctions",
            "pep",
            "ownership",
            "ubo",
            "beneficial",
            "owner",
            "owners",
            "fund",
            "funds",
            "subfund",
            "subfunds",
            "share",
            "class",
            "umbrella",
            "structure",
            "relationship",
            "relationships",
            "graph",
            "party",
            "parties",
            "group",
            "groups",
            "client",
            "clients",
            "mandate",
            "mandates",
            "kyc",
            "aml",
            "compliance",
            "case",
            "cases",
            "evidence",
            "rate",
            "card",
            "adverse",
            "media",
            "workstream",
            "checklist",
            "the",
            "a",
            "an",
            "for",
            "of",
            "on",
            "in",
            "at",
            "to",
            "by",
            "with",
            "from",
            "this",
            "that",
            "these",
            "those",
            "it",
            "its",
            "them",
            "their",
            "me",
            "my",
            "i",
            "we",
            "our",
            "us",
            "you",
            "your",
            "all",
            "every",
            "each",
            "some",
            "any",
            "no",
            "new",
            "current",
            "existing",
            "active",
            "pending",
            "missing",
            "available",
            "up",
            "out",
            "about",
            "into",
            "around",
            "through",
            "between",
            "against",
            "and",
            "or",
            "but",
            "if",
            "then",
            "so",
            "yet",
            "not",
            "also",
            "just",
            "only",
            "please",
            "ok",
            "sure",
            "yes",
            "hey",
            "hi",
            "right",
            "actually",
            "what's",
            "who's",
            "how's",
            "where's",
            "there's",
            "let's",
            "don't",
            "doesn't",
            "isn't",
            "aren't",
            "whats",
            "whos",
            "hows",
            "as",
            "aon",
            "anew",
        ]
        .into_iter()
        .collect()
    })
}

fn clean_token_for_lookup(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '.')
        .trim_end_matches("'s")
        .trim_end_matches("’s")
        .to_ascii_lowercase()
}

fn original_token_is_capitalized(token: &str) -> bool {
    token
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
}

fn run_after_marker_score(tokens: &[&str], run: &str) -> bool {
    let markers = ["for", "called", "named", "on", "about", "regarding"];
    let lower_run = run.to_ascii_lowercase();
    tokens.iter().enumerate().any(|(idx, token)| {
        let lower = token.to_ascii_lowercase();
        markers.contains(&lower.as_str())
            && tokens
                .get(idx + 1)
                .map(|next| lower_run.starts_with(&next.to_ascii_lowercase()))
                .unwrap_or(false)
    })
}

/// Extract candidate entity names from a raw utterance.
///
/// The extractor removes known intent words, domain nouns, and connective
/// tissue, then returns ranked residual candidate runs suitable for
/// `discovery.search-entities`.
///
/// # Examples
///
/// ```ignore
/// let result = ob_poc::semtaxonomy::extract_entity_candidates(
///     "show me the deals for Allianz Global Investors"
/// );
/// assert_eq!(result, vec!["Allianz Global Investors"]);
/// ```
pub fn extract_entity_candidates(utterance: &str) -> Vec<String> {
    let stop_words = stop_vocabulary();
    let tokens = utterance.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() {
        return Vec::new();
    }

    let is_candidate = tokens
        .iter()
        .map(|token| {
            let cleaned = clean_token_for_lookup(token);
            !cleaned.is_empty() && cleaned.len() > 1 && !stop_words.contains(cleaned.as_str())
        })
        .collect::<Vec<_>>();

    let mut runs = Vec::new();
    let mut i = 0usize;
    while i < tokens.len() {
        if !is_candidate[i] {
            i += 1;
            continue;
        }
        let mut run_tokens = vec![tokens[i]];
        let mut j = i + 1;
        while j < tokens.len() {
            if is_candidate[j] {
                run_tokens.push(tokens[j]);
                j += 1;
                continue;
            }
            if j + 1 < tokens.len()
                && is_candidate[j + 1]
                && original_token_is_capitalized(tokens[j])
            {
                run_tokens.push(tokens[j]);
                run_tokens.push(tokens[j + 1]);
                j += 2;
                continue;
            }
            break;
        }
        let mut run = run_tokens.join(" ");
        run = run
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '\'')
            .trim_end_matches("'s")
            .trim_end_matches("’s")
            .to_string();
        if run.len() > 1 {
            runs.push(run);
        }
        i = j;
    }

    runs.sort_by(|a, b| {
        let a_marker = run_after_marker_score(&tokens, a);
        let b_marker = run_after_marker_score(&tokens, b);
        let a_capitalized = a.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        let b_capitalized = b.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        b_marker
            .cmp(&a_marker)
            .then(b_capitalized.cmp(&a_capitalized))
            .then(b.len().cmp(&a.len()))
    });
    runs.dedup();
    runs
}

/// Refresh a `SageSession` ledger from discovery outputs.
///
/// # Examples
///
/// ```ignore
/// let mut state = ob_poc::semtaxonomy::SageSession::default();
/// let hydration = ob_poc::semtaxonomy::SageSessionHydration {
///     cascade_result: Some(serde_json::json!({"query": "Allianz"})),
///     domain_scope: Some("deal".to_string()),
///     ..Default::default()
/// };
/// ob_poc::semtaxonomy::hydrate_sage_session(&mut state, hydration);
/// assert_eq!(state.domain_scope.as_deref(), Some("deal"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SageSessionHydration {
    pub cascade_result: Option<Value>,
    pub active_entity: Option<EntityRef>,
    pub entity_candidates: Vec<EntityCandidate>,
    pub domain_scope: Option<String>,
    pub aspect: Option<String>,
    pub verb_surface: Vec<VerbSurfaceEntry>,
    pub entity_state: Option<Value>,
    pub domain_state_summaries: Vec<DomainStateSummary>,
    pub intent_hints: Vec<IntentHint>,
    pub grounding_strategy: Option<String>,
    pub grounding_confidence: Option<String>,
}

pub fn hydrate_sage_session(session: &mut SageSession, hydration: SageSessionHydration) {
    if let Some(cascade_result) = hydration.cascade_result {
        session.cascade_result = Some(cascade_result);
    }
    session.active_entity = hydration.active_entity;
    session.entity_candidates = hydration.entity_candidates;
    session.domain_scope = hydration.domain_scope;
    session.aspect = hydration.aspect;
    session.verb_surface = hydration.verb_surface;
    session.entity_state = hydration.entity_state;
    session.domain_state_summaries = hydration.domain_state_summaries;
    session.likely_intents = hydration.intent_hints;
    session.grounding_strategy = hydration.grounding_strategy;
    session.grounding_confidence = hydration.grounding_confidence;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_simple_entity_after_for() {
        let result = extract_entity_candidates("show me the deals for Allianz");
        assert_eq!(result, vec!["Allianz"]);
    }

    #[test]
    fn extract_multi_word_entity() {
        let result = extract_entity_candidates("create a CBU for Allianz Global Investors");
        assert_eq!(result, vec!["Allianz Global Investors"]);
    }

    #[test]
    fn extract_entity_at_end() {
        let result = extract_entity_candidates("who owns BNP Paribas");
        assert_eq!(result, vec!["BNP Paribas"]);
    }

    #[test]
    fn extract_entity_with_ag_suffix() {
        let result = extract_entity_candidates("run screening on Deutsche Bank AG");
        assert_eq!(result, vec!["Deutsche Bank AG"]);
    }

    #[test]
    fn extract_no_entity() {
        let result = extract_entity_candidates("what can I do next");
        assert!(result.is_empty());
    }

    #[test]
    fn extract_multiple_entities() {
        let result =
            extract_entity_candidates("show relationship between Allianz and Deutsche Bank");
        assert!(result.contains(&"Allianz".to_string()));
        assert!(result.contains(&"Deutsche Bank".to_string()));
    }

    #[test]
    fn extract_entity_with_domain_noun_inside() {
        let result = extract_entity_candidates("show me BlackRock Fund Services");
        assert_eq!(result, vec!["BlackRock Fund Services"]);
    }

    #[test]
    fn extract_possessive() {
        let result = extract_entity_candidates("show me BNP's deals");
        assert_eq!(result, vec!["BNP"]);
    }

    #[test]
    fn extract_entity_from_natural_language() {
        let result = extract_entity_candidates("I need to onboard Vanguard as a new client");
        assert_eq!(result, vec!["Vanguard"]);
    }
}
