//! Bootstrap Resolution Logic for V2 REPL ScopeGate
//!
//! Resolves user input against client groups during the session bootstrap
//! phase. Supports exact match, substring match, fuzzy word matching,
//! and optional semantic search via `PgClientGroupResolver`.
//!
//! The bootstrap only needs Stage 1 (alias → group). Stage 2 (group → anchor)
//! is handled by `session.load-cluster` DSL execution after resolution.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A candidate client group from bootstrap resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapCandidate {
    pub group_id: Uuid,
    pub group_name: String,
    pub match_source: String, // "exact", "substring", "fuzzy", "semantic"
    pub confidence: f32,
}

/// Outcome of resolving user input against client groups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BootstrapOutcome {
    /// Single clear match — proceed to scope selection.
    Resolved { group_id: Uuid, group_name: String },
    /// Multiple plausible matches — show numbered options.
    Ambiguous {
        candidates: Vec<BootstrapCandidate>,
        original_input: String,
    },
    /// No match found.
    NoMatch { original_input: String },
    /// Database is empty — skip bootstrap entirely.
    Empty,
}

// ---------------------------------------------------------------------------
// Stop words for fuzzy matching
// ---------------------------------------------------------------------------

const STOP_WORDS: &[&str] = &[
    "set", "select", "client", "group", "to", "as", "the", "a", "an", "please", "i", "want",
    "work", "with", "on", "use", "load", "choose", "pick", "switch", "change", "my", "for", "is",
    "it", "let", "me", "can", "you", "do", "of",
];

/// Extract significant words from user input, removing stop words.
fn extract_significant_words(input: &str) -> Vec<String> {
    input
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !STOP_WORDS.contains(w))
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty() && w.len() >= 2)
        .collect()
}

// ---------------------------------------------------------------------------
// Resolution (database)
// ---------------------------------------------------------------------------

/// Resolve user input against all client groups in the database.
///
/// Strategy:
/// 1. Fetch all client groups
/// 2. Try exact case-insensitive match on canonical_name
/// 3. Try substring match (input words in group name or vice versa)
/// 4. Try fuzzy word overlap matching
/// 5. If 0 → NoMatch; if 1 → Resolved; if 2+ → Ambiguous
#[cfg(feature = "database")]
pub async fn resolve_client_input(input: &str, pool: &PgPool) -> BootstrapOutcome {
    use crate::database::deal_repository::DealRepository;

    let groups = match DealRepository::get_all_client_groups(pool).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("Failed to fetch client groups: {}", e);
            return BootstrapOutcome::NoMatch {
                original_input: input.to_string(),
            };
        }
    };

    if groups.is_empty() {
        return BootstrapOutcome::Empty;
    }

    let input_lower = input.trim().to_lowercase();
    let significant_words = extract_significant_words(input);

    // Phase 1: Exact match on canonical_name
    for group in &groups {
        if group.canonical_name.to_lowercase() == input_lower {
            return BootstrapOutcome::Resolved {
                group_id: group.id,
                group_name: group.canonical_name.clone(),
            };
        }
    }

    // Phase 2: Substring match — any significant word appears in a group name,
    // OR the group name appears in the input.
    let mut candidates: Vec<BootstrapCandidate> = Vec::new();

    for group in &groups {
        let name_lower = group.canonical_name.to_lowercase();

        // Check if any significant word from input is a substring of the group name
        let word_in_name = significant_words
            .iter()
            .any(|w| w.len() >= 3 && name_lower.contains(w.as_str()));

        // Check if group name (or any word in it) is a substring of the input
        let name_words: Vec<&str> = name_lower.split_whitespace().collect();
        let name_in_input = name_words
            .iter()
            .any(|w| w.len() >= 3 && input_lower.contains(*w));

        if word_in_name || name_in_input {
            // Score: higher for more word overlap
            let overlap_count = significant_words
                .iter()
                .filter(|w| name_lower.contains(w.as_str()))
                .count();
            let confidence = if name_lower == input_lower {
                1.0
            } else {
                0.7 + (overlap_count as f32 * 0.1).min(0.25)
            };

            candidates.push(BootstrapCandidate {
                group_id: group.id,
                group_name: group.canonical_name.clone(),
                match_source: "substring".to_string(),
                confidence,
            });
        }
    }

    // If no substring matches, try fuzzy word overlap
    if candidates.is_empty() && !significant_words.is_empty() {
        for group in &groups {
            let name_lower = group.canonical_name.to_lowercase();
            let name_words: Vec<String> = name_lower
                .split_whitespace()
                .map(|w| w.to_string())
                .collect();

            // Count how many significant words partially match any word in the group name
            let fuzzy_hits: usize = significant_words
                .iter()
                .filter(|sw| {
                    name_words
                        .iter()
                        .any(|nw| nw.starts_with(sw.as_str()) || sw.starts_with(nw.as_str()))
                })
                .count();

            if fuzzy_hits > 0 {
                let confidence = 0.5 + (fuzzy_hits as f32 * 0.15).min(0.3);
                candidates.push(BootstrapCandidate {
                    group_id: group.id,
                    group_name: group.canonical_name.clone(),
                    match_source: "fuzzy".to_string(),
                    confidence,
                });
            }
        }
    }

    // Sort by confidence descending
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Determine outcome
    match candidates.len() {
        0 => BootstrapOutcome::NoMatch {
            original_input: input.to_string(),
        },
        1 => BootstrapOutcome::Resolved {
            group_id: candidates[0].group_id,
            group_name: candidates[0].group_name.clone(),
        },
        _ => {
            // If top candidate is significantly better, auto-resolve
            let margin = candidates[0].confidence - candidates[1].confidence;
            if margin >= 0.15 && candidates[0].confidence >= 0.7 {
                BootstrapOutcome::Resolved {
                    group_id: candidates[0].group_id,
                    group_name: candidates[0].group_name.clone(),
                }
            } else {
                // Cap at 5 candidates
                candidates.truncate(5);
                BootstrapOutcome::Ambiguous {
                    candidates,
                    original_input: input.to_string(),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Numeric / name selection from disambiguation candidates
// ---------------------------------------------------------------------------

/// Try to resolve a user selection from a list of disambiguation candidates.
///
/// Accepts:
/// - Numeric index ("1", "2", etc.) — 1-based
/// - Partial name match ("allianz" matches "Allianz Global Investors")
pub fn try_numeric_or_name_selection<'a>(
    input: &str,
    candidates: &'a [BootstrapCandidate],
) -> Option<&'a BootstrapCandidate> {
    let trimmed = input.trim();

    // Try numeric index (1-based)
    if let Ok(idx) = trimmed.parse::<usize>() {
        if idx >= 1 && idx <= candidates.len() {
            return Some(&candidates[idx - 1]);
        }
        return None;
    }

    // Try name match (case-insensitive substring)
    let input_lower = trimmed.to_lowercase();
    candidates.iter().find(|c| {
        let name_lower = c.group_name.to_lowercase();
        name_lower.contains(&input_lower) || input_lower.contains(&name_lower)
    })
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Generate the initial greeting message for session creation.
pub fn format_greeting() -> String {
    "Welcome! Which client group would you like to work with today?".to_string()
}

/// Format the disambiguation prompt showing numbered candidates.
pub fn format_disambiguation(candidates: &[BootstrapCandidate], input: &str) -> String {
    let mut lines = vec![format!(
        "I found multiple matches for \"{}\". Which did you mean?",
        input
    )];
    for (i, c) in candidates.iter().enumerate() {
        lines.push(format!("  {}. {}", i + 1, c.group_name));
    }
    lines.push(String::new());
    lines.push("Enter a number or type the name.".to_string());
    lines.join("\n")
}

/// Format the "ready" message after successful scope resolution.
pub fn format_ready_message(group_name: &str, examples: &[String]) -> String {
    let mut msg = format!("Scope set to {}.", group_name);
    if !examples.is_empty() {
        msg.push_str("\n\nFor example, try:\n");
        for ex in examples.iter().take(5) {
            msg.push_str(&format!("  - \"{}\"\n", ex));
        }
    }
    msg
}

/// Default example phrases when no verb config is available.
pub fn default_example_phrases() -> Vec<String> {
    vec![
        "Set up a new fund structure".to_string(),
        "Start a KYC case".to_string(),
        "Show the ownership structure".to_string(),
        "Add custody products".to_string(),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_candidates() -> Vec<BootstrapCandidate> {
        vec![
            BootstrapCandidate {
                group_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                group_name: "Allianz Global Investors".to_string(),
                match_source: "substring".to_string(),
                confidence: 0.85,
            },
            BootstrapCandidate {
                group_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
                group_name: "Aviva Investors".to_string(),
                match_source: "substring".to_string(),
                confidence: 0.80,
            },
            BootstrapCandidate {
                group_id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
                group_name: "BlackRock".to_string(),
                match_source: "fuzzy".to_string(),
                confidence: 0.65,
            },
        ]
    }

    #[test]
    fn test_format_greeting() {
        let greeting = format_greeting();
        assert!(greeting.contains("client group"));
    }

    #[test]
    fn test_format_ready_message_with_examples() {
        let msg = format_ready_message(
            "Allianz Global Investors",
            &["Set up a fund".to_string(), "Start KYC".to_string()],
        );
        assert!(msg.contains("Allianz Global Investors"));
        assert!(msg.contains("For example"));
        assert!(msg.contains("Set up a fund"));
    }

    #[test]
    fn test_format_ready_message_no_examples() {
        let msg = format_ready_message("Allianz Global Investors", &[]);
        assert!(msg.contains("Allianz Global Investors"));
        assert!(!msg.contains("For example"));
    }

    #[test]
    fn test_try_numeric_selection() {
        let candidates = sample_candidates();
        let result = try_numeric_or_name_selection("1", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().group_name, "Allianz Global Investors");
    }

    #[test]
    fn test_try_numeric_selection_second() {
        let candidates = sample_candidates();
        let result = try_numeric_or_name_selection("2", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().group_name, "Aviva Investors");
    }

    #[test]
    fn test_try_name_selection() {
        let candidates = sample_candidates();
        let result = try_numeric_or_name_selection("allianz", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().group_name, "Allianz Global Investors");
    }

    #[test]
    fn test_try_invalid_selection() {
        let candidates = sample_candidates();
        let result = try_numeric_or_name_selection("99", &candidates);
        assert!(result.is_none());
    }

    #[test]
    fn test_try_zero_selection() {
        let candidates = sample_candidates();
        let result = try_numeric_or_name_selection("0", &candidates);
        assert!(result.is_none());
    }

    #[test]
    fn test_try_name_selection_partial() {
        let candidates = sample_candidates();
        let result = try_numeric_or_name_selection("aviva", &candidates);
        assert!(result.is_some());
        assert_eq!(result.unwrap().group_name, "Aviva Investors");
    }

    #[test]
    fn test_try_name_selection_no_match() {
        let candidates = sample_candidates();
        let result = try_numeric_or_name_selection("vanguard", &candidates);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_significant_words() {
        let words = extract_significant_words("set client to allianz");
        assert!(words.contains(&"allianz".to_string()));
        assert!(!words.contains(&"set".to_string()));
        assert!(!words.contains(&"client".to_string()));
        assert!(!words.contains(&"to".to_string()));
    }

    #[test]
    fn test_extract_significant_words_just_name() {
        let words = extract_significant_words("allianz");
        assert_eq!(words, vec!["allianz"]);
    }

    #[test]
    fn test_format_disambiguation() {
        let candidates = sample_candidates();
        let msg = format_disambiguation(&candidates, "invest");
        assert!(msg.contains("Allianz Global Investors"));
        assert!(msg.contains("Aviva Investors"));
        assert!(msg.contains("BlackRock"));
        assert!(msg.contains("1."));
        assert!(msg.contains("2."));
        assert!(msg.contains("3."));
    }

    #[test]
    fn test_default_example_phrases() {
        let phrases = default_example_phrases();
        assert!(!phrases.is_empty());
        assert!(phrases.len() <= 5);
    }
}
