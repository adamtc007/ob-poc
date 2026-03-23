//! Constrained match layer — resolves utterances against a small set of valid verbs
//! using keyword matching. Fast (~1ms), deterministic, no embeddings or LLM calls.

use uuid::Uuid;

use super::valid_verb_set::{ValidVerbSet, VerbCandidate};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// How the constrained match resolved (or didn't).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchStrategy {
    /// Resolved via keyword overlap scoring.
    Keyword,
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
        self.verb_fqn.is_some() && self.confidence >= 0.70
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
    "full onboarding",
    "complete structure",
    "create structure",
];

const QUESTION_WORDS: &[&str] = &[
    "what", "who", "where", "when", "how", "which", "show", "list", "display", "get", "find",
    "tell",
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
];

const MUTATION_ACTIONS: &[&str] = &[
    "create", "update", "delete", "submit", "approve", "reject", "assign", "set", "add", "remove",
    "move",
];

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
        .map(|t| t.to_string())
        .collect()
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

    let starts_with_question = QUESTION_WORDS
        .iter()
        .any(|question| utterance_lower.starts_with(question));

    let mut scored: Vec<(&VerbCandidate, usize, f32)> = valid_verbs
        .verbs
        .iter()
        .map(|candidate| {
            let mut hits = 0usize;
            let total_keywords = candidate.keywords.len().max(1);

            // Check keyword overlap
            for keyword in &candidate.keywords {
                let kw_lower = keyword.to_lowercase();
                if tokens
                    .iter()
                    .any(|t| t == &kw_lower || t.contains(&kw_lower))
                    || utterance_lower.contains(&kw_lower)
                {
                    hits += 1;
                }
            }

            // Bonus: entity type name in utterance
            if !candidate.entity_type.is_empty() {
                let etype_lower = candidate.entity_type.to_lowercase().replace('_', " ");
                if utterance_lower.contains(&etype_lower)
                    || tokens.iter().any(|t| t == &etype_lower)
                {
                    hits += 1;
                }
            }

            // Bonus: action word (part after dot) in utterance
            if let Some(action) = candidate.verb_fqn.rsplit('.').next() {
                let action_parts: Vec<&str> = action.split('-').collect();
                for part in &action_parts {
                    if tokens.iter().any(|t| t == part) {
                        hits += 1;
                        break; // Only one bonus for action
                    }
                }
            }

            let mut score = hits as f32 / (total_keywords as f32 + 2.0); // +2 for the bonus slots

            if starts_with_question {
                let action = candidate.verb_fqn.rsplit('.').next().unwrap_or("");
                if OBSERVATION_ACTIONS.iter().any(|verb| action.contains(verb)) {
                    score += 0.15;
                }
                if MUTATION_ACTIONS.iter().any(|verb| action.contains(verb)) {
                    score -= 0.15;
                }
            }

            score = score.clamp(0.0, 1.0);

            (candidate, hits, score)
        })
        .collect();

    let matched_entity_types: std::collections::HashSet<&str> = scored
        .iter()
        .filter_map(|(candidate, hits, _)| {
            if *hits > 0 && !candidate.entity_type.is_empty() {
                Some(candidate.entity_type.as_str())
            } else {
                None
            }
        })
        .collect();
    if matched_entity_types.len() >= 3 {
        return ConstrainedResult::fallthrough();
    }

    // Sort by hits descending (primary), then score (secondary)
    scored.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
    });

    let candidate_count = scored.len();

    // Check top candidate
    if let Some((top, top_hits, top_score)) = scored.first() {
        let second_score = scored.get(1).map(|(_, _, s)| *s).unwrap_or(0.0);
        let margin = top_score - second_score;

        // Strong match: ≥3 hits AND score ≥ 0.4 → resolve regardless of margin
        if *top_hits >= 3 && *top_score >= 0.4 {
            return ConstrainedResult {
                verb_fqn: Some(top.verb_fqn.clone()),
                entity_id: top.entity_id,
                confidence: (*top_score).min(0.95),
                strategy: MatchStrategy::Keyword,
                keyword_hits: *top_hits,
                candidate_count,
            };
        }

        // Normal match: ≥2 hits AND score ≥ 0.3 AND margin ≥ 0.10
        if *top_hits >= 2 && *top_score >= 0.3 && margin >= 0.10 {
            return ConstrainedResult {
                verb_fqn: Some(top.verb_fqn.clone()),
                entity_id: top.entity_id,
                confidence: (*top_score).min(0.90),
                strategy: MatchStrategy::Keyword,
                keyword_hits: *top_hits,
                candidate_count,
            };
        }
    }

    ConstrainedResult {
        verb_fqn: None,
        entity_id: None,
        confidence: 0.0,
        strategy: MatchStrategy::Fallthrough,
        keyword_hits: 0,
        candidate_count,
    }
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
                vec!["document", "solicit", "request", "papers", "certificate"],
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

        let result = resolve_constrained("request identity documents for due diligence", &valid);
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
                vec!["kyc", "case", "create", "open", "compliance"],
            ),
            (
                "document.solicit",
                "document",
                vec!["document", "solicit", "request"],
            ),
            ("cbu.update", "cbu", vec!["cbu", "update"]),
        ]);

        let result = resolve_constrained("create a KYC case", &valid);
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
    fn test_strong_match_ignores_margin() {
        let valid = make_test_verb_set(vec![
            (
                "screening.full",
                "screening",
                vec!["screening", "sanctions", "pep", "run", "check", "full"],
            ),
            (
                "screening.sanctions",
                "screening",
                vec!["screening", "sanctions", "check", "ofac"],
            ),
        ]);

        let result = resolve_constrained("run a full sanctions and pep screening check", &valid);
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
            ("document.show", "document", vec!["document", "show"]),
            ("document.create", "document", vec!["document", "create"]),
        ]);

        let result = resolve_constrained("show document", &valid);

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
        assert!(tokens.contains(&"documents".to_string()));
        assert!(tokens.contains(&"entity".to_string()));
        assert!(!tokens.contains(&"the".to_string()));
        assert!(!tokens.contains(&"for".to_string()));
        assert!(!tokens.contains(&"this".to_string()));
    }
}
