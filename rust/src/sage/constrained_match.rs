//! Constrained match layer — resolves utterances against a small set of valid verbs
//! using keyword matching. Fast (~1ms), deterministic, no embeddings or LLM calls.

use anyhow::Result;
use uuid::Uuid;

use super::valid_verb_set::{ValidVerbSet, VerbCandidate};
use crate::mcp::verb_search::HybridVerbSearcher;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// How the constrained match resolved (or didn't).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchStrategy {
    /// Resolved via keyword overlap scoring.
    Keyword,
    /// Resolved via semantic search constrained to the valid verb set.
    ScopedEmbedding,
    /// No confident match — caller should fall through to open search.
    Fallthrough,
}

/// Result of constrained match attempt.
#[derive(Debug, Clone)]
pub struct ConstrainedResult {
    pub verb_fqn: Option<String>,
    pub entity_id: Option<Uuid>,
    pub confidence: f32,
    pub strategy: MatchStrategy,
    pub keyword_hits: usize,
    pub candidate_count: usize,
}

impl ConstrainedResult {
    /// Whether this result should be used (high enough confidence).
    pub fn resolved(&self) -> bool {
        matches!(
            self.strategy,
            MatchStrategy::Keyword | MatchStrategy::ScopedEmbedding
        ) && self.verb_fqn.is_some()
    }

    /// Create a fallthrough result (no match).
    pub fn fallthrough() -> Self {
        Self {
            verb_fqn: None,
            entity_id: None,
            confidence: 0.0,
            strategy: MatchStrategy::Fallthrough,
            keyword_hits: 0,
            candidate_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Stop words
// ---------------------------------------------------------------------------

const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "for", "this", "that", "my", "me", "it", "is", "to", "of", "in", "and", "or",
    "with", "from", "on", "at", "by", "i", "we", "you", "can", "please", "want", "need", "would",
    "like", "just", "do", "does", "did", "be", "been", "being", "have", "has", "had", "will",
    "shall", "should", "could", "may", "might",
];

const COMPOUND_SIGNAL_WORDS: &[&str] = &[
    "struct.",
    "macro:",
    "scenario:",
    "set up",
    "setup",
    "onboard",
    "onboard the",
    "full onboarding",
    "complete structure",
    "create structure",
    "standard setup",
];

const QUESTION_WORDS: &[&str] = &[
    "what", "who", "where", "when", "how", "which", "show", "list", "display", "get", "find",
    "tell", "describe", "explain",
];

const OBSERVATION_ACTIONS: &[&str] = &[
    "read",
    "list",
    "show",
    "get",
    "for-entity",
    "inspect",
    "search",
    "find",
    "lookup",
    "display",
    "describe",
    "compute",
    "trace",
    "analyze",
    "summary",
    "timeline",
];

const MUTATION_ACTIONS: &[&str] = &[
    "create", "update", "delete", "submit", "approve", "reject", "assign", "set", "add", "remove",
    "move", "cancel", "close", "escalate",
];

const GENERIC_MATCH_WORDS: &[&str] = &[
    "add",
    "case",
    "check",
    "company",
    "create",
    "document",
    "entity",
    "get",
    "information",
    "list",
    "open",
    "read",
    "request",
    "show",
    "start",
    "state",
    "status",
    "update",
];

const DELETE_WORDS: &[&str] = &["delete", "remove", "nuke", "erase"];
const SCREENING_WORDS: &[&str] = &[
    "pep",
    "sanction",
    "screen",
    "ofac",
    "aml",
    "adverse",
    "media",
    "politically",
    "exposed",
    "rba",
    "cdd",
    "edd",
];
const DOCUMENT_COLLECTION_WORDS: &[&str] = &[
    "document",
    "identity",
    "passport",
    "certificate",
    "address",
    "paper",
    "kyc",
    "evidence",
    "upload",
];
const OWNERSHIP_WORDS: &[&str] = &[
    "ownership",
    "owner",
    "owners",
    "ubo",
    "beneficial",
    "control",
    "controls",
    "parent",
    "waterfall",
];
const NAVIGATION_WORDS: &[&str] = &["back", "previous", "return"];
const IMPERATIVE_WORDS: &[&str] = &[
    "run",
    "request",
    "check",
    "delete",
    "create",
    "open",
    "calculate",
    "compute",
    "start",
];

const DOMAIN_SIGNAL_WORDS: &[(&str, &[&str])] = &[
    (
        "ownership",
        &["ownership", "owner", "waterfall", "chain", "pierce", "veil"],
    ),
    ("ubo", &["ubo", "beneficial", "threshold", "25%"]),
    ("control", &["control", "voting", "controller"]),
    (
        "deal",
        &["deal", "rate card", "fee", "pricing", "negotiate"],
    ),
    (
        "fund",
        &[
            "fund",
            "umbrella",
            "subfund",
            "share class",
            "sicav",
            "icav",
            "ucits",
        ],
    ),
    ("entity", &["person", "company", "register", "smith"]),
    ("booking-principal", &["booking", "custody", "coverage"]),
    ("client-group", &["client group", "group", "discovery"]),
    ("cbu", &["cbu", "delete", "nuke"]),
    ("gleif", &["lei", "gleif", "hierarchy"]),
    ("trading-profile", &["trading", "mandate", "profile"]),
    ("struct", &["structure", "oeic", "sicav", "icav", "lp"]),
];

#[derive(Debug, Clone, Copy, Default)]
struct MatchStats {
    hits: usize,
    distinctive_hits: usize,
    score: f32,
    domain_hit: bool,
    action_hit: bool,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

fn tokenize_utterance(utterance: &str) -> Vec<String> {
    utterance
        .to_lowercase()
        .split(|c: char| {
            c.is_whitespace()
                || c == ','
                || c == '.'
                || c == ';'
                || c == ':'
                || c == '?'
                || c == '!'
                || c == '-'
                || c == '\''
                || c == '"'
        })
        .filter(|t| !t.is_empty())
        .filter(|t| !STOP_WORDS.contains(t))
        .map(normalize_token)
        .collect()
}

fn normalize_token(token: &str) -> String {
    let token = token.to_lowercase();
    if token.ends_with("ments") && token.len() > 6 {
        return token[..token.len() - 1].to_string();
    }
    if token.ends_with("ions") && token.len() > 5 {
        return token[..token.len() - 1].to_string();
    }
    if token.ends_with("ing") && token.len() > 5 {
        return token[..token.len() - 3].to_string();
    }
    if token.ends_with("ies") && token.len() > 4 {
        return format!("{}y", &token[..token.len() - 3]);
    }
    if token.ends_with("ses")
        && token.len() > 5
        && !matches!(token.as_str(), "cases" | "bases" | "phases")
    {
        return token[..token.len() - 2].to_string();
    }
    if token.ends_with("es") && token.len() > 4 {
        return token[..token.len() - 2].to_string();
    }
    if token.ends_with('s')
        && !token.ends_with("ss")
        && token.len() > 4
        && !STOP_WORDS.contains(&token.as_str())
    {
        return token[..token.len() - 1].to_string();
    }
    token
}

fn has_any_token(tokens: &[String], needles: &[&str]) -> bool {
    needles
        .iter()
        .map(|needle| normalize_token(needle))
        .any(|needle| tokens.iter().any(|token| token == &needle))
}

fn utterance_has_signal(utterance_lower: &str, tokens: &[String], signal: &str) -> bool {
    if signal.contains(' ') || signal.contains('%') {
        return utterance_lower.contains(signal);
    }

    let normalized = normalize_token(signal);
    tokens.iter().any(|token| {
        token == &normalized || token.contains(&normalized) || normalized.contains(token)
    })
}

fn fqn_token_parts(verb_fqn: &str) -> (Vec<String>, Vec<String>) {
    let mut parts = verb_fqn.split('.');
    let domain_tokens = parts
        .next()
        .map(|domain| {
            domain
                .split('-')
                .map(normalize_token)
                .filter(|token| !token.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let action_tokens = parts
        .next_back()
        .or_else(|| verb_fqn.rsplit('.').next())
        .map(|action| {
            action
                .split('-')
                .map(normalize_token)
                .filter(|token| !token.is_empty())
                .collect()
        })
        .unwrap_or_default();
    (domain_tokens, action_tokens)
}

fn action_is_generic(candidate: &VerbCandidate) -> bool {
    let action = candidate.verb_fqn.rsplit('.').next().unwrap_or("");
    matches!(
        action,
        "read"
            | "list"
            | "show"
            | "get"
            | "create"
            | "open-case"
            | "state"
            | "update"
            | "update-status"
    )
}

fn score_candidate(utterance_tokens: &[String], candidate: &VerbCandidate) -> MatchStats {
    if candidate.keywords.is_empty() {
        return MatchStats::default();
    }

    let mut stats = MatchStats::default();
    let mut partial_hits = 0usize;

    for keyword in &candidate.keywords {
        let keyword = normalize_token(keyword);
        let is_generic = GENERIC_MATCH_WORDS.contains(&keyword.as_str());

        if utterance_tokens.iter().any(|token| token == &keyword) {
            stats.hits += 1;
            if !is_generic {
                stats.distinctive_hits += 1;
            }
            continue;
        }

        if utterance_tokens
            .iter()
            .any(|token| token.contains(&keyword) || keyword.contains(token.as_str()))
        {
            partial_hits += 1;
            if !is_generic {
                stats.distinctive_hits += 1;
            }
        }
    }

    let (domain_tokens, action_tokens) = fqn_token_parts(&candidate.verb_fqn);
    stats.domain_hit = domain_tokens
        .iter()
        .any(|token| utterance_tokens.iter().any(|utterance| utterance == token));
    stats.action_hit = action_tokens
        .iter()
        .any(|token| utterance_tokens.iter().any(|utterance| utterance == token));

    let total = stats.hits as f32 + (partial_hits as f32 * 0.45);
    stats.score = total / candidate.keywords.len() as f32;
    stats
}

// ---------------------------------------------------------------------------
// Main resolution function
// ---------------------------------------------------------------------------

/// Resolve an utterance against a constrained set of valid verbs.
///
/// Returns a result with confidence. Caller checks `resolved()` and
/// falls through to open search if false.
///
/// Strategy:
/// 1. Tokenize utterance (lowercase, split, remove stop words)
/// 2. For each valid verb, count keyword hits (verb keywords + entity type + action word)
/// 3. Score = hits / total_keywords
/// 4. Return top candidate if: ≥2 hits AND score ≥ 0.5 AND sufficient margin over second place
/// 5. Strong match (≥3 hits, score ≥ 0.6) resolves regardless of margin
pub fn resolve_constrained(utterance: &str, valid_verbs: &ValidVerbSet) -> ConstrainedResult {
    if valid_verbs.is_empty() {
        return ConstrainedResult::fallthrough();
    }

    let utterance_lower = utterance.to_lowercase();
    if COMPOUND_SIGNAL_WORDS
        .iter()
        .any(|signal| utterance_lower.contains(signal))
    {
        return ConstrainedResult::fallthrough();
    }

    let tokens = tokenize_utterance(utterance);
    if tokens.is_empty() {
        return ConstrainedResult::fallthrough();
    }

    let valid_domains: std::collections::HashSet<&str> = valid_verbs
        .verbs
        .iter()
        .filter_map(|verb| verb.verb_fqn.split('.').next())
        .collect();
    let detected_domain = DOMAIN_SIGNAL_WORDS.iter().find_map(|(domain, signals)| {
        signals
            .iter()
            .any(|signal| utterance_has_signal(&utterance_lower, &tokens, signal))
            .then_some(*domain)
    });
    if let Some(domain) = detected_domain {
        if !valid_domains.contains(domain) {
            return ConstrainedResult::fallthrough();
        }
    }

    let starts_with_question = QUESTION_WORDS
        .iter()
        .any(|question| utterance_lower.starts_with(question));

    let has_delete_intent = has_any_token(&tokens, DELETE_WORDS);
    let has_screening_intent = has_any_token(&tokens, SCREENING_WORDS);
    let has_document_collection_intent = has_any_token(&tokens, DOCUMENT_COLLECTION_WORDS);
    let has_ownership_intent = has_any_token(&tokens, OWNERSHIP_WORDS);
    let has_navigation_intent = has_any_token(&tokens, NAVIGATION_WORDS);
    let has_imperative_intent = has_any_token(&tokens, IMPERATIVE_WORDS);

    let mut scored: Vec<(&VerbCandidate, MatchStats)> = valid_verbs
        .verbs
        .iter()
        .map(|candidate| {
            let mut stats = score_candidate(&tokens, candidate);
            let mut score = stats.score;
            let verb_fqn = candidate.verb_fqn.as_str();
            let action = verb_fqn.rsplit('.').next().unwrap_or("");

            if !candidate.entity_type.is_empty() {
                let entity_tokens: Vec<String> = candidate
                    .entity_type
                    .split('_')
                    .map(normalize_token)
                    .collect();
                if entity_tokens
                    .iter()
                    .any(|entity_token| tokens.iter().any(|token| token == entity_token))
                {
                    stats.hits += 1;
                    score += 0.08;
                }
            }

            if stats.domain_hit {
                score += 0.12;
            }
            if stats.action_hit {
                score += 0.10;
            }
            if stats.distinctive_hits > 0 {
                score += 0.06 * stats.distinctive_hits.min(3) as f32;
            }

            if starts_with_question {
                if OBSERVATION_ACTIONS.iter().any(|verb| action.contains(verb)) {
                    score += 0.15;
                }
                if MUTATION_ACTIONS.iter().any(|verb| action.contains(verb)) {
                    score -= 0.15;
                }
            }

            if has_delete_intent && !action.contains("delete") && !action.contains("remove") {
                score -= 0.25;
            }
            if has_screening_intent {
                if verb_fqn.starts_with("screening.") {
                    score += 0.24;
                } else if action_is_generic(candidate) {
                    score -= 0.18;
                }
            }
            if has_document_collection_intent {
                if verb_fqn.starts_with("document.") {
                    score += 0.20;
                } else if verb_fqn.starts_with("request.") || verb_fqn.starts_with("requirement.") {
                    score += 0.06;
                } else if action_is_generic(candidate) {
                    score -= 0.14;
                }
            }
            if has_ownership_intent {
                if verb_fqn.starts_with("ubo.") || verb_fqn.starts_with("ownership.") {
                    score += 0.20;
                } else if action_is_generic(candidate) {
                    score -= 0.16;
                }
            }
            if has_navigation_intent {
                if verb_fqn.starts_with("view.navigate") {
                    score += 0.25;
                } else {
                    score -= 0.20;
                }
            }
            if has_imperative_intent && action_is_generic(candidate) && stats.distinctive_hits == 0
            {
                score -= 0.12;
            }

            score = score.clamp(0.0, 1.0);
            stats.score = score;

            (candidate, stats)
        })
        .collect();

    let matched_entity_types: std::collections::HashSet<&str> = scored
        .iter()
        .filter_map(|(candidate, stats)| {
            if stats.hits > 0 && !candidate.entity_type.is_empty() {
                Some(candidate.entity_type.as_str())
            } else {
                None
            }
        })
        .collect();
    let entity_spread_penalty = if matched_entity_types.len() >= 4 {
        0.15
    } else if matched_entity_types.len() >= 3 {
        0.08
    } else {
        0.0
    };

    // Sort by score descending (primary), then hits (secondary)
    scored.sort_by(|a, b| {
        b.1.score
            .partial_cmp(&a.1.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.distinctive_hits.cmp(&a.1.distinctive_hits))
            .then_with(|| b.1.hits.cmp(&a.1.hits))
    });

    let candidate_count = scored.len();

    // Check top candidate
    if let Some((top, top_stats)) = scored.first() {
        let same_entity_verbs = valid_verbs
            .verbs
            .iter()
            .filter(|verb| verb.entity_type == top.entity_type)
            .count();
        if same_entity_verbs == 1 {
            let action_part = top.verb_fqn.rsplit('.').next().unwrap_or("");
            let has_matching_action = tokens
                .iter()
                .any(|token| action_part.contains(token.as_str()) || token.contains(action_part));
            if !has_matching_action && !top_stats.action_hit && top_stats.distinctive_hits <= 1 {
                return ConstrainedResult::fallthrough();
            }
        }

        let second_score = scored.get(1).map(|(_, stats)| stats.score).unwrap_or(0.0);
        let margin = top_stats.score - second_score;
        let adjusted_score = (top_stats.score - entity_spread_penalty).max(0.0);

        if top_stats.distinctive_hits >= 2 && adjusted_score >= 0.50 && margin >= 0.15 {
            return ConstrainedResult {
                verb_fqn: Some(top.verb_fqn.clone()),
                entity_id: top.entity_id,
                confidence: adjusted_score.min(0.95),
                strategy: MatchStrategy::Keyword,
                keyword_hits: top_stats.hits,
                candidate_count,
            };
        }
    }

    ConstrainedResult::fallthrough_with_candidates(candidate_count)
}

impl ConstrainedResult {
    fn fallthrough_with_candidates(candidate_count: usize) -> Self {
        Self {
            verb_fqn: None,
            entity_id: None,
            confidence: 0.0,
            strategy: MatchStrategy::Fallthrough,
            keyword_hits: 0,
            candidate_count,
        }
    }
}

/// Resolve an utterance against a valid verb set using a keyword fast-path
/// followed by a semantic search constrained to that verb set.
///
/// The keyword matcher only accepts slam-dunk matches. Everything else falls
/// through to scoped semantic search against the current `valid_verbs`.
///
/// # Examples
/// ```ignore
/// let result = resolve_constrained_hybrid(
///     "request identity documents",
///     &valid_verbs,
///     &searcher,
///     None,
/// ).await?;
/// assert!(result.resolved() || matches!(result.strategy, ob_poc::sage::constrained_match::MatchStrategy::Fallthrough));
/// # Ok::<(), anyhow::Error>(())
/// ```
pub async fn resolve_constrained_hybrid(
    utterance: &str,
    valid_verbs: &ValidVerbSet,
    searcher: &HybridVerbSearcher,
    session_domain: Option<&str>,
) -> Result<ConstrainedResult> {
    let keyword = resolve_constrained(utterance, valid_verbs);
    if keyword.resolved() {
        return Ok(keyword);
    }

    let allowed_verbs = valid_verbs.to_allowed_set();
    if allowed_verbs.is_empty() {
        return Ok(keyword);
    }

    let scoped_results = searcher
        .search_embeddings_only(utterance, 5, None, Some(&allowed_verbs))
        .await?;

    let Some(top) = scoped_results.first() else {
        return Ok(ConstrainedResult::fallthrough_with_candidates(0));
    };
    if !allowed_verbs.contains(&top.verb) {
        return Ok(ConstrainedResult::fallthrough_with_candidates(
            scoped_results.len(),
        ));
    }

    let result_domain = top.verb.split('.').next().unwrap_or("");
    let session_domain = session_domain.unwrap_or("");
    let second_score = scoped_results
        .get(1)
        .map(|result| result.score)
        .unwrap_or(0.0);
    let margin = top.score - second_score;
    let domain_ok = session_domain.is_empty() || result_domain == session_domain;
    let accept = domain_ok
        && ((top.score >= 0.80 && margin >= 0.06) || (top.score >= 0.74 && margin >= 0.30));
    if !accept {
        log::debug!(
            "SemOS scoped near-miss: '{}' top-3: [{}] (margin: {:.3})",
            utterance,
            scoped_results
                .iter()
                .take(3)
                .map(|result| format!("{}={:.3}", result.verb, result.score))
                .collect::<Vec<_>>()
                .join(", "),
            margin
        );
        return Ok(ConstrainedResult::fallthrough_with_candidates(
            scoped_results.len(),
        ));
    }

    let entity_id = valid_verbs
        .verbs
        .iter()
        .find(|candidate| candidate.verb_fqn == top.verb)
        .and_then(|candidate| candidate.entity_id);

    Ok(ConstrainedResult {
        verb_fqn: Some(top.verb.clone()),
        entity_id,
        confidence: top.score.min(0.95),
        strategy: MatchStrategy::ScopedEmbedding,
        keyword_hits: 0,
        candidate_count: scoped_results.len(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sage::valid_verb_set::{VerbCandidate, VerbSource};
    use chrono::Utc;

    fn make_test_verb_set(entries: Vec<(&str, &str, Vec<&str>)>) -> ValidVerbSet {
        ValidVerbSet {
            verbs: entries
                .into_iter()
                .map(|(fqn, etype, kw)| VerbCandidate {
                    verb_fqn: fqn.to_string(),
                    entity_id: None,
                    entity_type: etype.to_string(),
                    source: VerbSource::FsmTransition,
                    priority: 10,
                    keywords: kw.into_iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
            client_group_id: Uuid::new_v4(),
            constellation_id: "test".to_string(),
            computed_at: Utc::now(),
        }
    }

    #[test]
    fn test_document_solicit_resolves() {
        let valid = make_test_verb_set(vec![
            (
                "document.solicit",
                "document",
                vec![
                    "document",
                    "solicit",
                    "request",
                    "identity",
                    "papers",
                    "certificate",
                ],
            ),
            (
                "kyc-case.create",
                "kyc_case",
                vec!["kyc", "case", "create", "open", "compliance"],
            ),
            ("cbu.update", "cbu", vec!["cbu", "update", "edit", "modify"]),
            (
                "document.upload",
                "document",
                vec!["document", "upload", "attach", "file"],
            ),
            (
                "ubo.discover",
                "ubo",
                vec!["ubo", "beneficial", "owner", "discover", "ownership"],
            ),
        ]);

        let result = resolve_constrained("solicit identity certificate documents", &valid);
        assert_eq!(result.verb_fqn.as_deref(), Some("document.solicit"));
        assert!(result.confidence >= 0.30);
        assert_eq!(result.strategy, MatchStrategy::Keyword);
    }

    #[test]
    fn test_ambiguous_returns_fallthrough() {
        let valid = make_test_verb_set(vec![
            ("cbu.update", "cbu", vec!["cbu", "update", "edit"]),
            (
                "entity.update",
                "entity",
                vec!["entity", "update", "modify"],
            ),
            (
                "deal.update-record",
                "deal",
                vec!["deal", "update", "record"],
            ),
        ]);

        let result = resolve_constrained("update something", &valid);
        // "update" matches all three — ambiguous
        assert_eq!(result.strategy, MatchStrategy::Fallthrough);
    }

    #[test]
    fn test_create_kyc_case_resolves() {
        let valid = make_test_verb_set(vec![
            (
                "kyc-case.create",
                "kyc_case",
                vec![
                    "kyc",
                    "case",
                    "create",
                    "open",
                    "new",
                    "start",
                    "begin",
                    "compliance",
                ],
            ),
            (
                "document.solicit",
                "document",
                vec!["document", "solicit", "request", "ask", "upload"],
            ),
            (
                "cbu.update",
                "cbu",
                vec!["cbu", "update", "modify", "change"],
            ),
        ]);

        let result = resolve_constrained("create a new kyc case", &valid);
        assert_eq!(result.verb_fqn.as_deref(), Some("kyc-case.create"));
    }

    #[test]
    fn test_empty_verb_set_returns_fallthrough() {
        let valid = ValidVerbSet {
            verbs: vec![],
            client_group_id: Uuid::new_v4(),
            constellation_id: "test".to_string(),
            computed_at: Utc::now(),
        };

        let result = resolve_constrained("do anything", &valid);
        assert_eq!(result.strategy, MatchStrategy::Fallthrough);
    }

    #[test]
    fn test_strong_match_with_clear_margin_resolves() {
        let valid = make_test_verb_set(vec![(
            "screening.full",
            "screening",
            vec![
                "screening",
                "sanctions",
                "pep",
                "politically",
                "exposed",
                "adverse",
                "media",
                "run",
                "check",
                "full",
            ],
        )]);

        let result = resolve_constrained(
            "run a full sanctions pep and adverse media screening check",
            &valid,
        );
        assert_eq!(result.verb_fqn.as_deref(), Some("screening.full"));
    }

    #[test]
    fn test_compound_signal_bypasses_constrained_match() {
        let valid = make_test_verb_set(vec![(
            "kyc-case.create",
            "kyc_case",
            vec!["kyc", "case", "create", "open"],
        )]);

        let result = resolve_constrained("set up full onboarding for this client", &valid);

        assert_eq!(result.strategy, MatchStrategy::Fallthrough);
        assert!(!result.resolved());
    }

    #[test]
    fn test_question_words_bias_toward_observation_verbs() {
        let valid = make_test_verb_set(vec![
            (
                "document.show",
                "document",
                vec![
                    "document", "show", "display", "read", "view", "detail", "inspect", "lookup",
                ],
            ),
            (
                "document.create",
                "document",
                vec!["document", "create", "add", "new", "generate", "submit"],
            ),
        ]);

        let result = resolve_constrained("show and display the document details", &valid);

        assert_eq!(result.verb_fqn.as_deref(), Some("document.show"));
        assert!(result.resolved());
    }

    #[test]
    fn test_multi_entity_type_match_falls_through() {
        let valid = make_test_verb_set(vec![
            ("cbu.read", "cbu", vec!["show"]),
            ("document.read", "document", vec!["show"]),
            ("ubo.read", "ubo", vec!["show"]),
        ]);

        let result = resolve_constrained("show everything", &valid);

        assert_eq!(result.strategy, MatchStrategy::Fallthrough);
        assert!(!result.resolved());
    }

    #[test]
    fn test_tokenizer_removes_stop_words() {
        let tokens = tokenize_utterance("request the identity documents for this entity");
        assert!(tokens.contains(&"request".to_string()));
        assert!(tokens.contains(&"identity".to_string()));
        assert!(tokens.contains(&"document".to_string()));
        assert!(tokens.contains(&"entity".to_string()));
        assert!(!tokens.contains(&"the".to_string()));
        assert!(!tokens.contains(&"for".to_string()));
        assert!(!tokens.contains(&"this".to_string()));
    }
}
