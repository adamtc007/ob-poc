//! Pack Router — Deterministic pack selection from user input
//!
//! # Routing Priority (spec §4.2)
//!
//! 1. **Explicit name match** — "use the onboarding journey" → exact pack id/name match.
//!    HIGHEST PRIORITY, always wins.
//! 2. **Substring match on invocation_phrases** — Phase 0 implementation.
//! 3. **Candle BGE semantic match** — Phase 1: embed query, score against pack phrases.
//! 4. **Fallback** — list available packs and ask user.

use std::path::Path;
use std::sync::Arc;

use crate::journey::pack::{load_packs_from_dir, PackLoadError, PackManifest};
use crate::repl::types_v2::PackCandidate;

// ---------------------------------------------------------------------------
// PackSemanticScorer trait
// ---------------------------------------------------------------------------

/// Trait for semantic scoring — decouples Candle from PackRouter for testing.
///
/// Implementations score a query against a list of target phrases and return
/// per-phrase similarity scores.
pub trait PackSemanticScorer: Send + Sync {
    /// Score a query against a list of target phrases.
    /// Returns a Vec of similarity scores (one per phrase), values in 0.0–1.0.
    fn score(&self, query: &str, phrases: &[String]) -> Result<Vec<f32>, String>;
}

/// Semantic pack routing threshold. A pack needs at least this score from
/// the semantic scorer to be considered a candidate.
const SEMANTIC_PACK_THRESHOLD: f32 = 0.55;

// ---------------------------------------------------------------------------
// PackRouter
// ---------------------------------------------------------------------------

/// Routes user input to the best matching pack.
pub struct PackRouter {
    packs: Vec<(Arc<PackManifest>, String)>, // (manifest, hash)
    scorer: Option<Arc<dyn PackSemanticScorer>>,
}

impl PackRouter {
    /// Create a router from a list of loaded packs.
    pub fn new(packs: Vec<(Arc<PackManifest>, String)>) -> Self {
        Self {
            packs,
            scorer: None,
        }
    }

    /// Load packs from a directory and create a router.
    pub fn load(config_dir: &Path) -> Result<Self, PackLoadError> {
        let loaded = load_packs_from_dir(config_dir)?;
        let packs = loaded.into_iter().map(|(m, h)| (Arc::new(m), h)).collect();
        Ok(Self {
            packs,
            scorer: None,
        })
    }

    /// Attach a semantic scorer for Candle BGE pack routing.
    pub fn with_scorer(mut self, scorer: Arc<dyn PackSemanticScorer>) -> Self {
        self.scorer = Some(scorer);
        self
    }

    /// Route user input to a pack.
    ///
    /// Priority:
    /// 1. Explicit name/id match (force-select).
    /// 2. Substring match on invocation_phrases.
    /// 3. Fallback — no match.
    pub fn route(&self, input: &str) -> PackRouteOutcome {
        let input_lower = input.to_lowercase();

        // 1. Force-select: explicit pack name/id match.
        //    "use the onboarding journey" or "use onboarding-request"
        if let Some(outcome) = self.try_force_select(&input_lower) {
            return outcome;
        }

        // 2. Substring match on invocation_phrases.
        let mut candidates: Vec<(Arc<PackManifest>, String, f32)> = Vec::new();

        for (manifest, hash) in &self.packs {
            let mut best_score: f32 = 0.0;

            for phrase in &manifest.invocation_phrases {
                let phrase_lower = phrase.to_lowercase();

                // Exact phrase match.
                if input_lower.contains(&phrase_lower) || phrase_lower.contains(&input_lower) {
                    let len_ratio = phrase_lower.len().min(input_lower.len()) as f32
                        / phrase_lower.len().max(input_lower.len()) as f32;
                    let score = 0.5 + 0.4 * len_ratio; // 0.5–0.9 range
                    if score > best_score {
                        best_score = score;
                    }
                }

                // Word overlap scoring.
                let phrase_words: Vec<&str> = phrase_lower.split_whitespace().collect();
                let input_words: Vec<&str> = input_lower.split_whitespace().collect();
                let overlap = phrase_words
                    .iter()
                    .filter(|w| w.len() > 2 && input_words.contains(w))
                    .count();

                if overlap > 0 && !phrase_words.is_empty() {
                    let score = overlap as f32 / phrase_words.len() as f32 * 0.7;
                    if score > best_score {
                        best_score = score;
                    }
                }
            }

            if best_score > 0.3 {
                candidates.push((manifest.clone(), hash.clone(), best_score));
            }
        }

        // 3. Semantic scoring (Phase 1) — if a scorer is configured,
        //    boost existing candidates and discover new ones.
        if let Some(scorer) = &self.scorer {
            // Boost existing candidates whose substring score is weak.
            for (manifest, _hash, score) in &mut candidates {
                if *score >= 0.7 {
                    continue; // Strong substring match — no need for semantic.
                }
                let mut phrases: Vec<String> =
                    vec![manifest.description.clone(), manifest.name.clone()];
                phrases.extend(manifest.invocation_phrases.clone());
                if let Ok(sem_scores) = scorer.score(&input_lower, &phrases) {
                    let max_sem = sem_scores.iter().cloned().fold(0.0_f32, f32::max);
                    if max_sem > *score {
                        *score = max_sem;
                    }
                }
            }

            // Discover packs that had NO substring match but do match semantically.
            let existing_ids: Vec<String> =
                candidates.iter().map(|(m, _, _)| m.id.clone()).collect();
            for (manifest, hash) in &self.packs {
                if existing_ids.contains(&manifest.id) {
                    continue;
                }
                let mut phrases: Vec<String> =
                    vec![manifest.description.clone(), manifest.name.clone()];
                phrases.extend(manifest.invocation_phrases.clone());
                if let Ok(sem_scores) = scorer.score(&input_lower, &phrases) {
                    let max_sem = sem_scores.iter().cloned().fold(0.0_f32, f32::max);
                    if max_sem >= SEMANTIC_PACK_THRESHOLD {
                        candidates.push((manifest.clone(), hash.clone(), max_sem));
                    }
                }
            }
        }

        candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        match candidates.len() {
            0 => PackRouteOutcome::NoMatch,
            1 => {
                let (manifest, hash, _) = candidates.remove(0);
                PackRouteOutcome::Matched(manifest, hash)
            }
            _ => {
                // If top candidate is significantly better, use it.
                let top_score = candidates[0].2;
                let runner_up_score = candidates[1].2;
                if top_score - runner_up_score > 0.15 {
                    let (manifest, hash, _) = candidates.remove(0);
                    PackRouteOutcome::Matched(manifest, hash)
                } else {
                    let pack_candidates: Vec<PackCandidate> = candidates
                        .iter()
                        .map(|(m, _, s)| PackCandidate {
                            pack_id: m.id.clone(),
                            pack_name: m.name.clone(),
                            description: m.description.clone(),
                            score: *s,
                        })
                        .collect();
                    PackRouteOutcome::Ambiguous(pack_candidates)
                }
            }
        }
    }

    /// List all available packs.
    pub fn list_packs(&self) -> Vec<PackCandidate> {
        self.packs
            .iter()
            .map(|(m, _)| PackCandidate {
                pack_id: m.id.clone(),
                pack_name: m.name.clone(),
                description: m.description.clone(),
                score: 0.0,
            })
            .collect()
    }

    /// Get a specific pack by ID.
    pub fn get_pack(&self, pack_id: &str) -> Option<&(Arc<PackManifest>, String)> {
        self.packs.iter().find(|(m, _)| m.id == pack_id)
    }

    /// Get a pack by its manifest hash (for session rehydration after DB load).
    pub fn get_pack_by_hash(&self, hash: &str) -> Option<&(Arc<PackManifest>, String)> {
        self.packs.iter().find(|(_, h)| h == hash)
    }

    /// Try to match explicit force-select patterns.
    ///
    /// Detects patterns like:
    /// - "use the onboarding journey"
    /// - "use onboarding-request"
    /// - "start the kyc case pack"
    fn try_force_select(&self, input_lower: &str) -> Option<PackRouteOutcome> {
        // Look for "use" or "start" prefix patterns.
        let input_cleaned = input_lower
            .trim_start_matches("use ")
            .trim_start_matches("start ")
            .trim_start_matches("the ")
            .trim_start_matches("a ")
            .trim_end_matches(" pack")
            .trim_end_matches(" journey")
            .trim();

        for (manifest, hash) in &self.packs {
            // Match pack ID.
            if manifest.id.to_lowercase() == input_cleaned {
                return Some(PackRouteOutcome::Matched(manifest.clone(), hash.clone()));
            }

            // Match pack name (case-insensitive).
            if manifest.name.to_lowercase() == input_cleaned {
                return Some(PackRouteOutcome::Matched(manifest.clone(), hash.clone()));
            }

            // Match if pack name appears as substring of cleaned input.
            if input_cleaned.contains(&manifest.name.to_lowercase()) {
                return Some(PackRouteOutcome::Matched(manifest.clone(), hash.clone()));
            }

            // Match if pack id appears as substring of cleaned input.
            if input_cleaned.contains(&manifest.id.to_lowercase()) {
                return Some(PackRouteOutcome::Matched(manifest.clone(), hash.clone()));
            }
        }

        None
    }
}

// ---------------------------------------------------------------------------
// PackRouteOutcome
// ---------------------------------------------------------------------------

/// Result of pack routing.
#[derive(Debug, Clone)]
pub enum PackRouteOutcome {
    /// A single pack was clearly matched.
    Matched(Arc<PackManifest>, String), // (manifest, hash)

    /// Multiple packs could match — ask user.
    Ambiguous(Vec<PackCandidate>),

    /// No matching pack found.
    NoMatch,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey::pack::load_pack_from_bytes;

    fn onboarding_yaml() -> &'static str {
        r#"
id: onboarding-request
name: Onboarding Request
version: "1.0"
description: Onboard a new client structure
invocation_phrases:
  - "onboard a client"
  - "set up new client"
  - "start onboarding"
  - "new client setup"
"#
    }

    fn book_setup_yaml() -> &'static str {
        r#"
id: book-setup
name: Book Setup
version: "1.0"
description: Set up a fund book with products and trading matrix
invocation_phrases:
  - "set up a book"
  - "configure fund book"
  - "book setup"
"#
    }

    fn kyc_case_yaml() -> &'static str {
        r#"
id: kyc-case
name: KYC Case
version: "1.0"
description: Open and manage a KYC case
invocation_phrases:
  - "open kyc case"
  - "start kyc"
  - "compliance check"
"#
    }

    fn make_router() -> PackRouter {
        let packs: Vec<_> = [onboarding_yaml(), book_setup_yaml(), kyc_case_yaml()]
            .iter()
            .map(|yaml| {
                let (m, h) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
                (Arc::new(m), h)
            })
            .collect();
        PackRouter::new(packs)
    }

    #[test]
    fn test_force_select_by_id() {
        let router = make_router();

        match router.route("use onboarding-request") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "onboarding-request"),
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_force_select_by_name() {
        let router = make_router();

        match router.route("use the Onboarding Request") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "onboarding-request"),
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_force_select_with_journey_suffix() {
        let router = make_router();

        match router.route("use the onboarding request journey") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "onboarding-request"),
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_force_select_start_prefix() {
        let router = make_router();

        match router.route("start the kyc case pack") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "kyc-case"),
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_substring_match_on_phrases() {
        let router = make_router();

        match router.route("I want to onboard a client") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "onboarding-request"),
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_substring_match_kyc() {
        let router = make_router();

        match router.route("open kyc case for Allianz") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "kyc-case"),
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[test]
    fn test_no_match() {
        let router = make_router();

        match router.route("completely unrelated input") {
            PackRouteOutcome::NoMatch => {}
            other => panic!("Expected NoMatch, got {:?}", other),
        }
    }

    #[test]
    fn test_list_packs() {
        let router = make_router();
        let packs = router.list_packs();

        assert_eq!(packs.len(), 3);
        let ids: Vec<_> = packs.iter().map(|p| p.pack_id.as_str()).collect();
        assert!(ids.contains(&"onboarding-request"));
        assert!(ids.contains(&"book-setup"));
        assert!(ids.contains(&"kyc-case"));
    }

    #[test]
    fn test_get_pack_by_id() {
        let router = make_router();

        assert!(router.get_pack("onboarding-request").is_some());
        assert!(router.get_pack("nonexistent").is_none());
    }

    #[test]
    fn test_force_select_beats_substring() {
        // Even if "onboarding" matches a phrase for another pack,
        // "use onboarding-request" should force-select.
        let router = make_router();

        match router.route("use onboarding-request") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "onboarding-request"),
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Semantic scorer tests (Phase 1)
    // -----------------------------------------------------------------------

    /// Mock scorer that returns high scores for inputs containing "compliance"
    /// (mapping to kyc-case) and "fund structure" (mapping to book-setup).
    struct MockPackScorer;

    impl PackSemanticScorer for MockPackScorer {
        fn score(&self, query: &str, phrases: &[String]) -> Result<Vec<f32>, String> {
            Ok(phrases
                .iter()
                .map(|phrase| {
                    let p = phrase.to_lowercase();
                    let q = query.to_lowercase();
                    if q.contains("compliance") && (p.contains("kyc") || p.contains("compliance")) {
                        0.82
                    } else if q.contains("fund structure")
                        && (p.contains("book") || p.contains("fund"))
                    {
                        0.75
                    } else {
                        0.20
                    }
                })
                .collect())
        }
    }

    fn make_router_with_scorer() -> PackRouter {
        let packs: Vec<_> = [onboarding_yaml(), book_setup_yaml(), kyc_case_yaml()]
            .iter()
            .map(|yaml| {
                let (m, h) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
                (Arc::new(m), h)
            })
            .collect();
        PackRouter::new(packs).with_scorer(Arc::new(MockPackScorer))
    }

    #[test]
    fn test_semantic_discovers_pack_no_substring_match() {
        // "regulatory compliance review" has no substring overlap with any
        // invocation phrase, but the mock scorer maps "compliance" → kyc-case.
        let router = make_router_with_scorer();

        match router.route("regulatory compliance review") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "kyc-case"),
            other => panic!("Expected semantic match to kyc-case, got {:?}", other),
        }
    }

    #[test]
    fn test_semantic_boosts_weak_substring() {
        // "fund structure setup" partially overlaps with book-setup phrases
        // but the semantic scorer should boost the score above ambiguity.
        let router = make_router_with_scorer();

        match router.route("fund structure setup") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "book-setup"),
            other => panic!("Expected semantic boost for book-setup, got {:?}", other),
        }
    }

    #[test]
    fn test_force_select_beats_semantic() {
        // Force-select should still take priority even with a scorer attached.
        let router = make_router_with_scorer();

        match router.route("use onboarding-request") {
            PackRouteOutcome::Matched(m, _) => assert_eq!(m.id, "onboarding-request"),
            other => panic!("Expected force-select to win, got {:?}", other),
        }
    }

    #[test]
    fn test_no_match_even_with_scorer() {
        // If neither substring nor semantic produces a hit, still NoMatch.
        let router = make_router_with_scorer();

        match router.route("weather forecast today") {
            PackRouteOutcome::NoMatch => {}
            other => panic!("Expected NoMatch, got {:?}", other),
        }
    }

    #[test]
    fn test_get_pack_by_hash() {
        let router = make_router();

        // Get the hash of the first pack by looking it up by ID.
        let (_, hash) = router.get_pack("onboarding-request").unwrap();

        // Now look up by hash.
        let found = router.get_pack_by_hash(hash);
        assert!(found.is_some());
        assert_eq!(found.unwrap().0.id, "onboarding-request");

        // Unknown hash returns None.
        assert!(router.get_pack_by_hash("nonexistent-hash").is_none());
    }
}
