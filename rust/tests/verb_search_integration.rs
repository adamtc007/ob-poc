//! Integration tests for verb search and semantic matching
//!
//! Tests the full verb discovery pipeline with real database and embeddings.
//! Use this to verify semantic matching after teaching new phrases.
//!
//! Run all tests:
//!   cargo test --features database --test verb_search_integration -- --ignored --nocapture
//!
//! Run specific scenario:
//!   cargo test --features database --test verb_search_integration test_taught_phrases -- --ignored --nocapture
//!
//! Run threshold sweep:
//!   cargo test --features database --test verb_search_integration test_threshold_sweep -- --ignored --nocapture
//!
//! Quick smoke test (no DB required):
//!   cargo test --test verb_search_integration test_ambiguity_detection

// NOTE: We do NOT use #![cfg(feature = "database")] at file level
// so that unit tests (test_ambiguity_detection, test_normalize_candidates)
// can run without the database feature enabled.

use ob_poc::mcp::verb_search::{
    normalize_candidates, VerbSearchOutcome, VerbSearchResult, VerbSearchSource, AMBIGUITY_MARGIN,
};

// Extended CBU phrase scenarios for accelerated learning
// Named with _mod suffix to avoid being auto-discovered as a separate test binary
#[cfg(feature = "database")]
#[path = "helpers/cbu_phrase_scenarios.rs"]
mod cbu_phrase_scenarios;

// Database-dependent imports (only used in #[cfg(feature = "database")] tests)
#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "database")]
use sqlx::PgPool;
#[cfg(feature = "database")]
use std::collections::HashMap;
#[cfg(feature = "database")]
use std::sync::Arc;
#[cfg(feature = "database")]
use tokio::sync::OnceCell;

#[cfg(feature = "database")]
use ob_poc::agent::learning::embedder::CandleEmbedder;
#[cfg(feature = "database")]
use ob_poc::agent::learning::warmup::LearningWarmup;
#[cfg(feature = "database")]
use ob_poc::database::VerbService;
#[cfg(feature = "database")]
use ob_poc::mcp::verb_search::HybridVerbSearcher;

// =============================================================================
// SINGLETON RESOURCES (initialized once per test binary)
// =============================================================================

#[cfg(feature = "database")]
static SHARED_POOL: OnceCell<PgPool> = OnceCell::const_new();

#[cfg(feature = "database")]
static SHARED_EMBEDDER: OnceCell<Arc<CandleEmbedder>> = OnceCell::const_new();

#[cfg(feature = "database")]
async fn get_shared_pool() -> &'static PgPool {
    SHARED_POOL
        .get_or_init(|| async {
            let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                panic!(
                    "\n\
                    ╔══════════════════════════════════════════════════════════════╗\n\
                    ║  DATABASE_URL not set!                                       ║\n\
                    ║                                                              ║\n\
                    ║  Set DATABASE_URL to your ob-poc database:                   ║\n\
                    ║    export DATABASE_URL=\"postgresql:///data_designer\"         ║\n\
                    ╚══════════════════════════════════════════════════════════════╝\n"
                )
            });
            println!("Initializing shared database pool...");
            PgPool::connect(&url)
                .await
                .expect("Failed to connect to database")
        })
        .await
}

#[cfg(feature = "database")]
async fn get_shared_embedder() -> &'static Arc<CandleEmbedder> {
    SHARED_EMBEDDER
        .get_or_init(|| async {
            println!("Initializing shared Candle embedder (one-time, ~3-5s)...");
            let start = std::time::Instant::now();
            // spawn_blocking to avoid blocking the async runtime during model load
            let embedder = tokio::task::spawn_blocking(|| {
                CandleEmbedder::new().expect("Failed to load Candle embedder")
            })
            .await
            .expect("Embedder task panicked");
            println!("Embedder ready in {}ms", start.elapsed().as_millis());
            Arc::new(embedder)
        })
        .await
}

// =============================================================================
// AMBIGUITY HELPERS (no DB required)
// =============================================================================

/// Check for ambiguity with custom threshold and margin
///
/// This is a local helper that mirrors the logic in verb_search.rs but allows
/// varying the margin for threshold sweeps.
pub fn check_ambiguity_with_margin(
    candidates: &[VerbSearchResult],
    threshold: f32,
    margin: f32,
) -> VerbSearchOutcome {
    match candidates.first() {
        None => VerbSearchOutcome::NoMatch,
        Some(top) if top.score < threshold => VerbSearchOutcome::NoMatch,
        Some(top) => match candidates.get(1) {
            None => VerbSearchOutcome::Matched(top.clone()),
            Some(runner_up) if runner_up.score < threshold => {
                VerbSearchOutcome::Matched(top.clone())
            }
            Some(runner_up) => {
                let actual_margin = top.score - runner_up.score;
                if actual_margin < margin {
                    VerbSearchOutcome::Ambiguous {
                        top: top.clone(),
                        runner_up: runner_up.clone(),
                        margin: actual_margin,
                    }
                } else {
                    VerbSearchOutcome::Matched(top.clone())
                }
            }
        },
    }
}

// =============================================================================
// TEST INFRASTRUCTURE (DB required)
// =============================================================================

/// Test harness for verb search integration tests
#[cfg(feature = "database")]
struct VerbSearchTestHarness {
    #[allow(dead_code)]
    pool: &'static PgPool,
    verb_service: Arc<VerbService>,
    embedder: &'static Arc<CandleEmbedder>,
    learned_data: ob_poc::agent::learning::warmup::SharedLearnedData,
    searcher: HybridVerbSearcher,
}

#[cfg(feature = "database")]
impl VerbSearchTestHarness {
    /// Create a new test harness using shared singleton resources
    ///
    /// Uses OnceCell singletons for pool and embedder - initialized once per test binary.
    /// First test pays the ~3-5s initialization cost, subsequent tests are instant.
    ///
    /// REQUIRES: DATABASE_URL environment variable must be set.
    async fn new() -> Result<Self> {
        // Get shared resources (initialized once, reused across tests)
        let pool = get_shared_pool().await;
        let embedder = get_shared_embedder().await;

        let verb_service = Arc::new(VerbService::new(pool.clone()));

        // Load learned data (invocation_phrases, entity_aliases, etc.)
        let warmup = LearningWarmup::new(pool.clone());
        let (learned_data, _stats) = warmup.warmup().await?;

        // Cast Arc<CandleEmbedder> to Arc<dyn Embedder> for with_embedder
        let dyn_embedder: Arc<dyn ob_poc::agent::learning::embedder::Embedder> =
            embedder.clone() as Arc<dyn ob_poc::agent::learning::embedder::Embedder>;
        let searcher = HybridVerbSearcher::new(verb_service.clone(), Some(learned_data.clone()))
            .with_embedder(dyn_embedder);

        Ok(Self {
            pool,
            verb_service,
            embedder,
            learned_data,
            searcher,
        })
    }

    /// Search with custom decision threshold (for sweep tests)
    ///
    /// NOTE: This only varies the decision gate, not the retrieval cutoff.
    /// Use `search_with_full_thresholds` for full pipeline sweeps.
    async fn search_with_decision_threshold(
        &self,
        query: &str,
        decision_threshold: f32,
    ) -> Result<(VerbSearchOutcome, Vec<VerbSearchResult>)> {
        let results = self.searcher.search(query, None, None, 5).await?;
        let outcome = check_ambiguity_with_margin(&results, decision_threshold, AMBIGUITY_MARGIN);
        Ok((outcome, results))
    }

    /// Create a temporary searcher with custom thresholds for full pipeline sweeps
    ///
    /// This allows sweeping both:
    /// - `fallback_threshold`: retrieval cutoff (what candidates are fetched from DB)
    /// - `semantic_threshold`: decision gate (what scores are accepted)
    fn create_searcher_with_thresholds(
        &self,
        semantic_threshold: f32,
        fallback_threshold: f32,
    ) -> HybridVerbSearcher {
        let dyn_embedder: Arc<dyn ob_poc::agent::learning::embedder::Embedder> =
            self.embedder.clone() as Arc<dyn ob_poc::agent::learning::embedder::Embedder>;
        // Use learned_data so exact matches work in sweeps too
        HybridVerbSearcher::new(self.verb_service.clone(), Some(self.learned_data.clone()))
            .with_semantic_threshold(semantic_threshold)
            .with_fallback_threshold(fallback_threshold)
            .with_embedder(dyn_embedder)
    }

    /// Search using a temporary searcher with custom thresholds (full pipeline)
    async fn search_with_full_thresholds(
        &self,
        query: &str,
        semantic_threshold: f32,
        fallback_threshold: f32,
        margin: f32,
    ) -> Result<(VerbSearchOutcome, Vec<VerbSearchResult>)> {
        let searcher = self.create_searcher_with_thresholds(semantic_threshold, fallback_threshold);
        let results = searcher.search(query, None, None, 5).await?;
        // Belt & braces: normalize candidates explicitly so sweep logic stays correct
        // even if searcher internals change
        let candidates = normalize_candidates(results, 5);
        let outcome = check_ambiguity_with_margin(&candidates, semantic_threshold, margin);
        Ok((outcome, candidates))
    }

    /// Search and return outcome with default threshold
    #[allow(dead_code)]
    async fn search_with_outcome(&self, query: &str) -> Result<VerbSearchOutcome> {
        let results = self.searcher.search(query, None, None, 5).await?;
        let threshold = self.searcher.semantic_threshold();
        Ok(check_ambiguity_with_margin(
            &results,
            threshold,
            AMBIGUITY_MARGIN,
        ))
    }

    /// Search and return raw results (for inspection)
    async fn search_raw(&self, query: &str, limit: usize) -> Result<Vec<VerbSearchResult>> {
        self.searcher.search(query, None, None, limit).await
    }
}

// =============================================================================
// ENHANCED TEST SCENARIOS (ChatGPT suggestions)
// =============================================================================

#[cfg(feature = "database")]
/// Expected outcome type for a test scenario
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedOutcome {
    /// Should match a specific verb with high confidence
    Matched,
    /// Should trigger ambiguity (multiple close candidates)
    Ambiguous,
    /// Should not match anything above threshold
    NoMatch,
    /// Safety-first: Either Matched(correct) or Ambiguous is acceptable
    /// Use for dangerous verbs where forcing clarification is preferable to guessing wrong
    MatchedOrAmbiguous,
}

#[cfg(feature = "database")]
/// Enhanced test scenario with outcome type and allowed alternatives
#[derive(Debug, Clone)]
pub struct TestScenario {
    /// Human-readable name
    pub name: &'static str,
    /// Input phrase to search
    pub query: &'static str,
    /// Expected outcome type
    pub expected_outcome: ExpectedOutcome,
    /// Expected verb (for Matched scenarios)
    pub expected_verb: Option<&'static str>,
    /// Alternative acceptable verbs (top-3 should contain one of these)
    pub allowed_verbs: Vec<&'static str>,
    /// Minimum acceptable score (if matched)
    pub min_score: Option<f32>,
    /// Is this a "hard negative" pair? (dangerous confusion test)
    pub is_hard_negative: bool,
    /// Category for grouping in reports
    pub category: &'static str,
}

#[cfg(feature = "database")]
impl TestScenario {
    /// Expect a specific verb match
    pub fn matched(name: &'static str, query: &'static str, verb: &'static str) -> Self {
        Self {
            name,
            query,
            expected_outcome: ExpectedOutcome::Matched,
            expected_verb: Some(verb),
            allowed_verbs: vec![verb],
            min_score: None,
            is_hard_negative: false,
            category: "general",
        }
    }

    /// Expect a match with minimum score
    pub fn matched_with_score(
        name: &'static str,
        query: &'static str,
        verb: &'static str,
        min_score: f32,
    ) -> Self {
        Self {
            name,
            query,
            expected_outcome: ExpectedOutcome::Matched,
            expected_verb: Some(verb),
            allowed_verbs: vec![verb],
            min_score: Some(min_score),
            is_hard_negative: false,
            category: "general",
        }
    }

    /// Expect ambiguity (multiple close candidates)
    pub fn ambiguous(name: &'static str, query: &'static str) -> Self {
        Self {
            name,
            query,
            expected_outcome: ExpectedOutcome::Ambiguous,
            expected_verb: None,
            allowed_verbs: vec![],
            min_score: None,
            is_hard_negative: false,
            category: "general",
        }
    }

    /// Expect no match (below threshold)
    pub fn no_match(name: &'static str, query: &'static str) -> Self {
        Self {
            name,
            query,
            expected_outcome: ExpectedOutcome::NoMatch,
            expected_verb: None,
            allowed_verbs: vec![],
            min_score: None,
            is_hard_negative: false,
            category: "general",
        }
    }

    /// Mark as hard negative (dangerous confusion test)
    pub fn hard_negative(mut self) -> Self {
        self.is_hard_negative = true;
        self.category = "hard_negative";
        self
    }

    /// Set category
    pub fn with_category(mut self, category: &'static str) -> Self {
        self.category = category;
        self
    }

    /// Add alternative acceptable verbs
    pub fn with_alternatives(mut self, alts: &[&'static str]) -> Self {
        self.allowed_verbs.extend(alts.iter().copied());
        self
    }

    /// Safety-first: either correct match OR ambiguity is acceptable
    /// Use for dangerous verbs where forcing clarification is preferable to guessing wrong
    pub fn safety_first(
        name: &'static str,
        query: &'static str,
        preferred_verb: &'static str,
    ) -> Self {
        Self {
            name,
            query,
            expected_outcome: ExpectedOutcome::MatchedOrAmbiguous,
            expected_verb: Some(preferred_verb),
            allowed_verbs: vec![preferred_verb],
            min_score: None,
            is_hard_negative: true,
            category: "safety_first",
        }
    }
}

// =============================================================================
// DECISION TRACE (for regression tracking)
// =============================================================================

#[cfg(feature = "database")]
/// Full decision trace for a single query
#[derive(Debug, Clone, Serialize)]
pub struct DecisionTrace {
    pub query: String,
    pub threshold: f32,
    pub margin: f32,
    pub top_k_candidates: Vec<CandidateTrace>,
    pub outcome: String,
    pub selected_verb: Option<String>,
    pub correct: bool,
    pub elapsed_ms: f64,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateTrace {
    pub rank: usize,
    pub verb: String,
    pub score: f32,
    pub source: String,
    pub matched_phrase: String,
}

#[cfg(feature = "database")]
impl DecisionTrace {
    fn from_results(
        query: &str,
        results: &[VerbSearchResult],
        outcome: &VerbSearchOutcome,
        threshold: f32,
        correct: bool,
        elapsed_ms: f64,
    ) -> Self {
        let top_k_candidates: Vec<CandidateTrace> = results
            .iter()
            .enumerate()
            .map(|(i, r)| CandidateTrace {
                rank: i + 1,
                verb: r.verb.clone(),
                score: r.score,
                source: format!("{:?}", r.source),
                matched_phrase: r.matched_phrase.clone(),
            })
            .collect();

        let (outcome_str, selected_verb) = match outcome {
            VerbSearchOutcome::Matched(r) => ("Matched".to_string(), Some(r.verb.clone())),
            VerbSearchOutcome::Ambiguous { top, runner_up, .. } => (
                format!("Ambiguous({} vs {})", top.verb, runner_up.verb),
                None,
            ),
            VerbSearchOutcome::Suggest { candidates } => {
                let verbs: Vec<_> = candidates.iter().take(3).map(|c| c.verb.as_str()).collect();
                (format!("Suggest({})", verbs.join(", ")), None)
            }
            VerbSearchOutcome::NoMatch => ("NoMatch".to_string(), None),
        };

        Self {
            query: query.to_string(),
            threshold,
            margin: AMBIGUITY_MARGIN,
            top_k_candidates,
            outcome: outcome_str,
            selected_verb,
            correct,
            elapsed_ms,
        }
    }
}

// =============================================================================
// TEST REPORT (enhanced metrics)
// =============================================================================

#[cfg(feature = "database")]
#[derive(Debug, Default)]
pub struct TestReport {
    // Basic counts
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,

    // Retrieval quality metrics
    pub top1_correct: usize,
    pub top3_contains_correct: usize,
    pub ambiguity_triggered: usize,
    pub no_match_count: usize,
    pub confidently_wrong: usize, // Matched wrong verb with high confidence

    // Hard negative metrics
    pub hard_negative_total: usize,
    pub hard_negative_correct: usize,
    pub dangerous_confusions: Vec<(String, String, String)>, // (query, expected, got)

    // Decision traces for regression tracking
    pub traces: Vec<DecisionTrace>,

    // Results by category
    pub by_category: HashMap<String, CategoryStats>,
}

#[cfg(feature = "database")]
#[derive(Debug, Default, Clone)]
pub struct CategoryStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

#[cfg(feature = "database")]
impl TestReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(
        &mut self,
        scenario: &TestScenario,
        outcome: &VerbSearchOutcome,
        results: &[VerbSearchResult],
        elapsed_ms: f64,
        threshold: f32,
    ) {
        self.total += 1;

        // Check if correct
        let correct = self.check_correct(scenario, outcome, results);

        // Update basic counts
        if correct {
            self.passed += 1;
        } else {
            self.failed += 1;
        }

        // Update retrieval quality metrics
        match outcome {
            VerbSearchOutcome::Matched(r) => {
                // Check if verb is in allowed_verbs (respects alternatives)
                let is_allowed = scenario.allowed_verbs.contains(&r.verb.as_str());
                if is_allowed {
                    self.top1_correct += 1;
                } else if scenario.expected_outcome == ExpectedOutcome::Matched {
                    // Matched a verb NOT in allowed_verbs when we expected a match
                    // This is "confidently wrong" - the dangerous case
                    self.confidently_wrong += 1;
                }
            }
            VerbSearchOutcome::Ambiguous { .. } | VerbSearchOutcome::Suggest { .. } => {
                self.ambiguity_triggered += 1;
            }
            VerbSearchOutcome::NoMatch => {
                self.no_match_count += 1;
            }
        }

        // Check top-3 contains correct
        if let Some(expected) = scenario.expected_verb {
            let top3_verbs: Vec<&str> = results.iter().take(3).map(|r| r.verb.as_str()).collect();
            if top3_verbs.contains(&expected)
                || scenario
                    .allowed_verbs
                    .iter()
                    .any(|v| top3_verbs.contains(v))
            {
                self.top3_contains_correct += 1;
            }
        }

        // Hard negative tracking
        if scenario.is_hard_negative {
            self.hard_negative_total += 1;
            if correct {
                self.hard_negative_correct += 1;
            } else if let VerbSearchOutcome::Matched(r) = outcome {
                self.dangerous_confusions.push((
                    scenario.query.to_string(),
                    scenario.expected_verb.unwrap_or("?").to_string(),
                    r.verb.clone(),
                ));
            }
        }

        // Category stats
        let cat = self
            .by_category
            .entry(scenario.category.to_string())
            .or_default();
        cat.total += 1;
        if correct {
            cat.passed += 1;
        } else {
            cat.failed += 1;
        }

        // Record trace
        self.traces.push(DecisionTrace::from_results(
            scenario.query,
            results,
            outcome,
            threshold,
            correct,
            elapsed_ms,
        ));
    }

    fn check_correct(
        &self,
        scenario: &TestScenario,
        outcome: &VerbSearchOutcome,
        _results: &[VerbSearchResult],
    ) -> bool {
        match (scenario.expected_outcome, outcome) {
            // Expected Matched, got Matched
            (ExpectedOutcome::Matched, VerbSearchOutcome::Matched(r)) => {
                // Check verb matches expected or allowed
                let verb_ok = scenario.allowed_verbs.contains(&r.verb.as_str());
                // Check score meets minimum
                let score_ok = scenario.min_score.is_none_or(|min| r.score >= min);
                verb_ok && score_ok
            }
            // Expected Ambiguous, got Ambiguous
            (ExpectedOutcome::Ambiguous, VerbSearchOutcome::Ambiguous { .. }) => true,
            // Expected NoMatch, got NoMatch
            (ExpectedOutcome::NoMatch, VerbSearchOutcome::NoMatch) => true,

            // Safety-first policy: MatchedOrAmbiguous
            // Matched(correct verb) is acceptable
            (ExpectedOutcome::MatchedOrAmbiguous, VerbSearchOutcome::Matched(r)) => {
                scenario.allowed_verbs.contains(&r.verb.as_str())
            }
            // Ambiguous is also acceptable (forcing clarification is safe)
            (ExpectedOutcome::MatchedOrAmbiguous, VerbSearchOutcome::Ambiguous { .. }) => true,

            // Any other combination is wrong
            _ => false,
        }
    }

    pub fn print_summary(&self) {
        println!("\n============================================================");
        println!("                    TEST REPORT");
        println!("============================================================\n");

        // Basic stats
        println!(
            "OVERALL: {}/{} passed ({:.1}%)",
            self.passed,
            self.total,
            100.0 * self.passed as f64 / self.total.max(1) as f64
        );
        println!();

        // Retrieval quality
        println!("RETRIEVAL QUALITY:");
        println!(
            "  Top-1 correct:      {}/{} ({:.1}%)",
            self.top1_correct,
            self.total,
            100.0 * self.top1_correct as f64 / self.total.max(1) as f64
        );
        println!(
            "  Top-3 contains:     {}/{} ({:.1}%)",
            self.top3_contains_correct,
            self.total,
            100.0 * self.top3_contains_correct as f64 / self.total.max(1) as f64
        );
        println!(
            "  Ambiguity rate:     {}/{} ({:.1}%)",
            self.ambiguity_triggered,
            self.total,
            100.0 * self.ambiguity_triggered as f64 / self.total.max(1) as f64
        );
        println!(
            "  NoMatch rate:       {}/{} ({:.1}%)",
            self.no_match_count,
            self.total,
            100.0 * self.no_match_count as f64 / self.total.max(1) as f64
        );
        println!("  Confidently wrong:  {} ⚠️", self.confidently_wrong);
        println!();

        // Hard negatives
        if self.hard_negative_total > 0 {
            println!("HARD NEGATIVES (dangerous confusion):");
            println!(
                "  Correct: {}/{} ({:.1}%)",
                self.hard_negative_correct,
                self.hard_negative_total,
                100.0 * self.hard_negative_correct as f64 / self.hard_negative_total as f64
            );
            if !self.dangerous_confusions.is_empty() {
                println!("  Dangerous confusions:");
                for (query, expected, got) in &self.dangerous_confusions {
                    println!("    \"{}\" → expected {}, got {}", query, expected, got);
                }
            }
            println!();
        }

        // By category
        if self.by_category.len() > 1 {
            println!("BY CATEGORY:");
            let mut cats: Vec<_> = self.by_category.iter().collect();
            cats.sort_by_key(|(k, _)| *k);
            for (cat, stats) in cats {
                println!(
                    "  {}: {}/{} ({:.1}%)",
                    cat,
                    stats.passed,
                    stats.total,
                    100.0 * stats.passed as f64 / stats.total.max(1) as f64
                );
            }
            println!();
        }

        // Failed scenarios
        let failed_traces: Vec<_> = self.traces.iter().filter(|t| !t.correct).collect();
        if !failed_traces.is_empty() {
            println!("FAILED SCENARIOS:");
            for trace in failed_traces.iter().take(10) {
                println!("  ✗ \"{}\"", trace.query);
                println!("    outcome: {}", trace.outcome);
                if !trace.top_k_candidates.is_empty() {
                    println!(
                        "    top candidate: {} ({:.3})",
                        trace.top_k_candidates[0].verb, trace.top_k_candidates[0].score
                    );
                }
            }
            if failed_traces.len() > 10 {
                println!("  ... and {} more", failed_traces.len() - 10);
            }
        }
    }

    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.errors == 0
    }
}

// =============================================================================
// THRESHOLD SWEEP (Full Pipeline)
// =============================================================================

#[cfg(feature = "database")]
#[derive(Debug)]
pub struct SweepResult {
    /// Decision threshold (gate for accepting top match)
    pub semantic_threshold: f32,
    /// Retrieval threshold (cutoff for DB queries)
    pub fallback_threshold: f32,
    /// Margin for ambiguity detection
    pub margin: f32,
    /// Top-1 hit rate percentage
    pub top1_hit: f64,
    /// Ambiguity trigger rate percentage
    pub ambiguity_rate: f64,
    /// Count of confidently wrong matches
    pub confidently_wrong: usize,
    /// Count of NoMatch results
    pub no_match_count: usize,
}

#[cfg(feature = "database")]
/// Configuration for a threshold sweep
pub struct SweepConfig {
    /// Decision thresholds to test (gate for accepting matches)
    pub semantic_thresholds: Vec<f32>,
    /// Retrieval thresholds to test (cutoff for DB queries)
    pub fallback_thresholds: Vec<f32>,
    /// Margins for ambiguity detection
    pub margins: Vec<f32>,
}

#[cfg(feature = "database")]
impl Default for SweepConfig {
    fn default() -> Self {
        Self {
            semantic_thresholds: vec![0.80, 0.84, 0.88, 0.90, 0.92],
            fallback_thresholds: vec![0.70, 0.75, 0.78, 0.80],
            margins: vec![0.03, 0.05, 0.07],
        }
    }
}

#[cfg(feature = "database")]
impl SweepConfig {
    /// Quick sweep - fewer combinations for faster iteration
    pub fn quick() -> Self {
        Self {
            semantic_thresholds: vec![0.85, 0.88, 0.90],
            fallback_thresholds: vec![0.78],
            margins: vec![0.05],
        }
    }

    /// Full sweep - comprehensive threshold exploration
    pub fn full() -> Self {
        Self {
            semantic_thresholds: vec![0.75, 0.78, 0.80, 0.82, 0.84, 0.86, 0.88, 0.90, 0.92],
            fallback_thresholds: vec![0.65, 0.70, 0.75, 0.78, 0.80],
            margins: vec![0.03, 0.05, 0.07, 0.10],
        }
    }
}

#[cfg(feature = "database")]
async fn run_threshold_sweep(
    harness: &VerbSearchTestHarness,
    scenarios: &[TestScenario],
    config: &SweepConfig,
) -> Vec<SweepResult> {
    let mut results = Vec::new();
    let total_combos =
        config.semantic_thresholds.len() * config.fallback_thresholds.len() * config.margins.len();
    let mut combo_num = 0;

    for &semantic_threshold in &config.semantic_thresholds {
        for &fallback_threshold in &config.fallback_thresholds {
            for &margin in &config.margins {
                combo_num += 1;
                print!(
                    "\r  Sweep {}/{}: semantic={:.2} fallback={:.2} margin={:.2}   ",
                    combo_num, total_combos, semantic_threshold, fallback_threshold, margin
                );

                let mut top1_correct = 0;
                let mut ambiguity_count = 0;
                let mut confidently_wrong = 0;
                let mut no_match_count = 0;
                let total = scenarios.len();

                for scenario in scenarios {
                    // Use full pipeline search with both thresholds
                    if let Ok((outcome, _results)) = harness
                        .search_with_full_thresholds(
                            scenario.query,
                            semantic_threshold,
                            fallback_threshold,
                            margin,
                        )
                        .await
                    {
                        match &outcome {
                            VerbSearchOutcome::Matched(r) => {
                                if scenario.allowed_verbs.contains(&r.verb.as_str()) {
                                    top1_correct += 1;
                                } else if scenario.expected_outcome == ExpectedOutcome::Matched {
                                    confidently_wrong += 1;
                                }
                            }
                            VerbSearchOutcome::Ambiguous { .. }
                            | VerbSearchOutcome::Suggest { .. } => {
                                ambiguity_count += 1;
                            }
                            VerbSearchOutcome::NoMatch => {
                                no_match_count += 1;
                            }
                        }
                    }
                }

                results.push(SweepResult {
                    semantic_threshold,
                    fallback_threshold,
                    margin,
                    top1_hit: 100.0 * top1_correct as f64 / total as f64,
                    ambiguity_rate: 100.0 * ambiguity_count as f64 / total as f64,
                    confidently_wrong,
                    no_match_count,
                });
            }
        }
    }
    println!(); // Clear the progress line

    results
}

#[cfg(feature = "database")]
fn print_sweep_results(results: &[SweepResult]) {
    println!("\nTHRESHOLD SWEEP RESULTS (Full Pipeline):");
    println!(
        "{:>8} {:>8} {:>6} {:>9} {:>9} {:>8} {:>8}",
        "sem_thr", "fall_thr", "margin", "top1_hit%", "ambig%", "wrong", "no_match"
    );
    println!("{}", "-".repeat(68));

    for r in results {
        let wrong_marker = if r.confidently_wrong > 0 {
            " ⚠️"
        } else {
            ""
        };
        println!(
            "{:>8.2} {:>8.2} {:>6.2} {:>9.1} {:>9.1} {:>8}{} {:>8}",
            r.semantic_threshold,
            r.fallback_threshold,
            r.margin,
            r.top1_hit,
            r.ambiguity_rate,
            r.confidently_wrong,
            wrong_marker,
            r.no_match_count
        );
    }

    // Find best configuration (maximize top1_hit, minimize confidently_wrong)
    let best = results
        .iter()
        .filter(|r| r.confidently_wrong == 0)
        .max_by(|a, b| a.top1_hit.partial_cmp(&b.top1_hit).unwrap());

    if let Some(best) = best {
        println!(
            "\n✓ BEST (0 wrong): semantic={:.2} fallback={:.2} margin={:.2} → {:.1}% top-1",
            best.semantic_threshold, best.fallback_threshold, best.margin, best.top1_hit
        );
    } else {
        // All configs have some wrong - find minimum wrong
        let min_wrong = results
            .iter()
            .map(|r| r.confidently_wrong)
            .min()
            .unwrap_or(0);
        let best = results
            .iter()
            .filter(|r| r.confidently_wrong == min_wrong)
            .max_by(|a, b| a.top1_hit.partial_cmp(&b.top1_hit).unwrap());
        if let Some(best) = best {
            println!(
                "\n⚠️ BEST ({} wrong): semantic={:.2} fallback={:.2} margin={:.2} → {:.1}% top-1",
                min_wrong,
                best.semantic_threshold,
                best.fallback_threshold,
                best.margin,
                best.top1_hit
            );
        }
    }
}

// =============================================================================
// TEST SCENARIOS
// =============================================================================

#[cfg(feature = "database")]
fn taught_phrase_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched_with_score(
            "spin up a fund (taught)",
            "spin up a fund",
            "cbu.create",
            0.90,
        )
        .with_category("taught"),
        TestScenario::matched_with_score(
            "load the allianz book (taught)",
            "load the allianz book",
            "session.load-cluster",
            0.75, // Client name patterns on load-cluster
        )
        .with_category("taught"),
        TestScenario::matched_with_score(
            "show me the ownership (taught)",
            "show me the ownership",
            "control.build-graph",
            0.90,
        )
        .with_category("taught"),
        TestScenario::matched_with_score(
            "who controls this entity (taught)",
            "who controls this entity",
            "control.build-graph",
            0.90,
        )
        .with_category("taught"),
        TestScenario::matched_with_score(
            "find the ultimate beneficial owners (taught)",
            "find the ultimate beneficial owners",
            "control.identify-ubos",
            0.90,
        )
        .with_category("taught"),
        // "spin up a new fund called Alpha" - semantic model correctly identifies
        // this as cbu.create intent. The extra words don't dilute the core meaning.
        TestScenario::matched_with_score(
            "spin up a new fund (variation)",
            "spin up a new fund called Alpha",
            "cbu.create",
            0.80,
        )
        .with_category("taught"),
        TestScenario::matched_with_score(
            "load allianz (shorter)",
            "load allianz",
            "session.load-cluster",
            0.70, // Client name patterns - lower threshold for semantic matches
        )
        .with_category("taught"),
    ]
}

#[cfg(feature = "database")]
fn session_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched(
            "load galaxy",
            "load the allianz galaxy",
            "session.load-galaxy",
        )
        .with_category("session"),
        TestScenario::matched(
            "load book",
            "load the blackrock book",
            "session.load-galaxy",
        )
        .with_category("session"),
        TestScenario::matched("load cbu", "load cbu acme fund", "session.load-cbu")
            .with_category("session"),
        TestScenario::matched(
            "load jurisdiction",
            "load all luxembourg cbus",
            "session.load-jurisdiction",
        )
        .with_category("session"),
        TestScenario::matched("clear session", "clear the session", "session.clear")
            .with_category("session"),
        TestScenario::matched("undo", "undo the last action", "session.undo")
            .with_category("session"),
        TestScenario::matched("redo", "redo", "session.redo").with_category("session"),
        TestScenario::matched("session info", "show session info", "session.info")
            .with_category("session"),
        // Bare client names should trigger session.load-cluster (scope selection)
        TestScenario::matched("bare allianz", "allianz", "session.load-cluster")
            .with_category("session"),
        TestScenario::matched("bare blackrock", "blackrock", "session.load-cluster")
            .with_category("session"),
        TestScenario::matched("bare aviva", "aviva", "session.load-cluster")
            .with_category("session"),
        // Client name with "work on" prefix
        TestScenario::matched("work on allianz", "work on allianz", "session.load-cluster")
            .with_category("session"),
        TestScenario::matched(
            "focus on blackrock",
            "focus on blackrock",
            "session.load-cluster",
        )
        .with_category("session"),
        // Client book patterns
        TestScenario::matched("allianz book", "allianz book", "session.load-cluster")
            .with_category("session"),
        TestScenario::matched(
            "load cluster",
            "load the allianz cluster",
            "session.load-cluster",
        )
        .with_category("session"),
    ]
}

#[cfg(feature = "database")]
fn cbu_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched("create cbu", "create a new cbu", "cbu.create").with_category("cbu"),
        TestScenario::matched(
            "create fund",
            "create a fund called Alpha Growth",
            "cbu.create",
        )
        .with_category("cbu"),
        TestScenario::matched(
            "onboard client",
            "onboard new client Acme Corp",
            "cbu.create",
        )
        .with_category("cbu"),
        TestScenario::matched(
            "assign role",
            "assign custody role to BNY",
            "cbu.assign-role",
        )
        .with_category("cbu"),
        TestScenario::matched("list cbus", "list all cbus", "cbu.list").with_category("cbu"),
    ]
}

#[cfg(feature = "database")]
fn entity_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched(
            "create company",
            "create a limited company",
            "entity.create-limited-company",
        )
        .with_category("entity"),
        TestScenario::matched(
            "create person",
            "add a natural person",
            "entity.create-proper-person",
        )
        .with_category("entity"),
        TestScenario::matched("search entity", "find entity BlackRock", "entity.query")
            .with_category("entity"),
        TestScenario::matched("entity details", "show entity details", "entity.read")
            .with_category("entity"),
    ]
}

#[cfg(feature = "database")]
fn kyc_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched(
            "discover ubo",
            "discover ultimate beneficial owners",
            "control.identify-ubos",
        )
        .with_category("kyc"),
        TestScenario::matched("who owns", "who owns this company", "control.identify-ubos")
            .with_category("kyc"),
        TestScenario::matched(
            "ownership chain",
            "show the ownership chain",
            "control.trace-chain",
        )
        .with_category("kyc"),
        TestScenario::matched("create kyc case", "open a kyc case", "kyc-case.create")
            .with_category("kyc"),
    ]
}

#[cfg(feature = "database")]
fn view_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched("view universe", "show the universe", "view.universe")
            .with_category("view"),
        TestScenario::matched("view cbu", "focus on this cbu", "view.cbu").with_category("view"),
        TestScenario::matched("drill down", "drill into entity", "view.drill")
            .with_category("view"),
        TestScenario::matched("surface up", "surface back up", "view.surface")
            .with_category("view"),
    ]
}

#[cfg(feature = "database")]
/// Hard negative pairs - dangerous confusions that MUST NOT happen
///
/// Two categories:
/// 1. MUST match correctly (create vs update - clear semantic difference)
/// 2. Safety-first (delete vs archive - ambiguity acceptable, wrong match dangerous)
fn hard_negative_scenarios() -> Vec<TestScenario> {
    vec![
        // =====================================================================
        // MUST MATCH CORRECTLY - clear semantic difference
        // =====================================================================

        // create vs update - distinct actions
        TestScenario::matched("create not update", "create a new cbu", "cbu.create")
            .hard_negative(),
        TestScenario::matched("update not create", "update cbu details", "cbu.update")
            .hard_negative(),
        // load vs unload - opposite actions
        TestScenario::matched("load cbu", "load cbu into session", "session.load-system")
            .hard_negative(),
        TestScenario::matched(
            "unload cbu",
            "unload cbu from session",
            "session.unload-system",
        )
        .hard_negative(),
        // add vs remove - opposite actions
        TestScenario::matched(
            "add instrument",
            "add equity instruments",
            "trading-profile.add-instrument-class",
        )
        .hard_negative(),
        TestScenario::matched(
            "remove instrument",
            "remove equity instruments",
            "trading-profile.remove-instrument-class",
        )
        .hard_negative(),
        // =====================================================================
        // SAFETY-FIRST - ambiguity acceptable, wrong match dangerous
        // For these, triggering disambiguation UI is SAFER than guessing wrong
        // =====================================================================

        // delete vs archive - both destructive-ish, wrong choice is bad
        // If system is unsure, better to ask than delete when user meant archive
        TestScenario::safety_first(
            "delete entity (safety)",
            "delete this entity",
            "entity.delete",
        )
        .with_alternatives(&["entity.archive"]),
        // approve vs submit - workflow state transitions
        // Approving when user meant submit (or vice versa) is a workflow error
        TestScenario::safety_first(
            "approve case (safety)",
            "approve the kyc case",
            "kyc.approve-case",
        ),
        TestScenario::safety_first(
            "submit case (safety)",
            "submit the kyc case for review",
            "kyc.submit-case",
        ),
        // disable vs delete - disable is reversible, delete is not
        // If unsure, better to ask than accidentally delete
        TestScenario::safety_first("disable cbu (safety)", "disable this cbu", "cbu.disable"),
    ]
}

#[cfg(feature = "database")]
fn edge_case_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::no_match("garbage input", "asdfghjkl qwerty").with_category("edge"),
        TestScenario::no_match("random words", "purple elephant dancing").with_category("edge"),
        TestScenario::no_match("single word nonsense", "xyz").with_category("edge"),
        TestScenario::ambiguous("ambiguous create", "create something").with_category("edge"),
    ]
}

// =============================================================================
// TEST RUNNER
// =============================================================================

#[cfg(feature = "database")]
async fn run_scenarios(harness: &VerbSearchTestHarness, scenarios: &[TestScenario]) -> TestReport {
    let mut report = TestReport::new();
    let decision_threshold = harness.searcher.semantic_threshold();

    for scenario in scenarios {
        let start = std::time::Instant::now();
        match harness
            .search_with_decision_threshold(scenario.query, decision_threshold)
            .await
        {
            Ok((outcome, results)) => {
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                report.record(scenario, &outcome, &results, elapsed_ms, decision_threshold);
            }
            Err(e) => {
                report.errors += 1;
                report.total += 1;
                eprintln!("Error for \"{}\": {}", scenario.query, e);
            }
        }
    }

    report
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_taught_phrases() {
    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");
    let report = run_scenarios(&harness, &taught_phrase_scenarios()).await;
    report.print_summary();
    assert!(report.all_passed(), "Some taught phrase tests failed");
}

#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_session_verbs() {
    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");
    let report = run_scenarios(&harness, &session_scenarios()).await;
    report.print_summary();
    assert!(report.all_passed(), "Some session verb tests failed");
}

#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_hard_negatives() {
    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");
    let report = run_scenarios(&harness, &hard_negative_scenarios()).await;
    report.print_summary();

    // Hard negatives are critical - fail if ANY dangerous confusions
    assert!(
        report.dangerous_confusions.is_empty(),
        "CRITICAL: Dangerous confusions detected! These could cause data loss."
    );
}

#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_all_scenarios() {
    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");

    let mut all_scenarios = Vec::new();
    all_scenarios.extend(taught_phrase_scenarios());
    all_scenarios.extend(session_scenarios());
    all_scenarios.extend(cbu_scenarios());
    all_scenarios.extend(entity_scenarios());
    all_scenarios.extend(kyc_scenarios());
    all_scenarios.extend(view_scenarios());
    all_scenarios.extend(hard_negative_scenarios());
    all_scenarios.extend(edge_case_scenarios());

    let report = run_scenarios(&harness, &all_scenarios).await;
    report.print_summary();
}

/// Extended CBU phrase tests - comprehensive coverage for accelerated learning
///
/// This test runs ~150+ CBU-specific scenarios to identify phrases that need teaching.
/// Run this BEFORE teaching to identify gaps, and AFTER to validate improvements.
///
/// Usage:
///   cargo test --features database --test verb_search_integration test_cbu_extended -- --ignored --nocapture
#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_cbu_extended() {
    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");

    let scenarios = cbu_phrase_scenarios::all_cbu_scenarios();

    println!("Running {} extended CBU scenarios...\n", scenarios.len());

    let report = run_scenarios(&harness, &scenarios).await;
    report.print_summary();

    // Output phrases that need teaching (failed scenarios)
    let failed_traces: Vec<_> = report.traces.iter().filter(|t| !t.correct).collect();
    if !failed_traces.is_empty() {
        println!("\n============================================================");
        println!(
            "  PHRASES THAT NEED TEACHING ({} total)",
            failed_traces.len()
        );
        println!("============================================================\n");

        // Group by expected verb (from scenarios)
        let mut by_verb: std::collections::HashMap<String, Vec<&str>> =
            std::collections::HashMap::new();
        for trace in &failed_traces {
            // Find the matching scenario to get expected verb
            if let Some(scenario) = scenarios.iter().find(|s| s.query == trace.query) {
                if let Some(verb) = scenario.expected_verb {
                    by_verb
                        .entry(verb.to_string())
                        .or_default()
                        .push(&trace.query);
                }
            }
        }

        // Print as teachable SQL
        println!("-- Copy-paste into psql to teach these phrases:\n");
        for (verb, phrases) in &by_verb {
            println!("-- {} ({} phrases)", verb, phrases.len());
            let json_array: Vec<String> = phrases
                .iter()
                .map(|p| format!(r#"{{"phrase": "{}", "verb": "{}"}}"#, p, verb))
                .collect();
            println!(
                "SELECT * FROM agent.teach_phrases_batch('[{}]'::jsonb, 'accelerated_learning');\n",
                json_array.join(", ")
            );
        }
    }
}

/// Run CBU scenarios and output mismatch report for learning analysis
///
/// This generates a detailed JSON report of what's working and what needs teaching.
///
/// Usage:
///   VERB_SEARCH_DUMP_MISMATCH=cbu_mismatches.json \
///     cargo test --features database --test verb_search_integration test_cbu_dump_mismatches -- --ignored --nocapture
#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_cbu_dump_mismatches() {
    let output_path = match std::env::var("VERB_SEARCH_DUMP_MISMATCH") {
        Ok(path) => std::path::PathBuf::from(path),
        Err(_) => {
            println!("VERB_SEARCH_DUMP_MISMATCH not set, using default: cbu_mismatches.json");
            std::path::PathBuf::from("cbu_mismatches.json")
        }
    };

    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");

    let scenarios = cbu_phrase_scenarios::all_cbu_scenarios();

    println!("Running {} extended CBU scenarios...", scenarios.len());

    let report = collect_mismatches(&harness, &scenarios).await;

    // Print summary
    println!("\n============================================================");
    println!("                CBU PHRASE MISMATCH REPORT");
    println!("============================================================\n");
    println!("Total tests:        {}", report.total_tests);
    println!("Passed:             {}", report.passed);
    println!("Failed:             {}", report.failed);
    println!("Pass rate:          {:.1}%", report.pass_rate);
    println!("Confidently wrong:  {}", report.confidently_wrong);

    // Group mismatches by category
    let mut by_category: std::collections::HashMap<&str, Vec<&MismatchEntry>> =
        std::collections::HashMap::new();
    for m in &report.mismatches {
        by_category.entry(&m.category).or_default().push(m);
    }

    if !by_category.is_empty() {
        println!("\n--- Mismatches by Category ---");
        for (cat, entries) in &by_category {
            println!("\n{} ({} failures):", cat, entries.len());
            for m in entries.iter().take(5) {
                println!("  ✗ \"{}\"", m.query);
                println!("    Expected: {:?}", m.expected_verb);
                if let Some(actual) = &m.actual_verb {
                    println!("    Got:      {}", actual);
                } else {
                    println!("    Got:      {}", m.actual_outcome);
                }
            }
            if entries.len() > 5 {
                println!("  ... and {} more", entries.len() - 5);
            }
        }
    }

    // Write JSON output
    let json = serde_json::to_string_pretty(&report).expect("Failed to serialize report");
    std::fs::write(&output_path, &json).expect("Failed to write output file");
    println!("\n============================================================");
    println!("  Report written to: {}", output_path.display());
    println!("============================================================");
}

#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_threshold_sweep() {
    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");

    let mut scenarios = Vec::new();
    scenarios.extend(taught_phrase_scenarios());
    scenarios.extend(session_scenarios());
    scenarios.extend(cbu_scenarios());

    // Use quick sweep by default (faster iteration)
    // Change to SweepConfig::full() for comprehensive exploration
    let config = SweepConfig::quick();

    println!(
        "Running threshold sweep with {} scenarios...",
        scenarios.len()
    );
    println!(
        "  Testing {} semantic × {} fallback × {} margin = {} combinations",
        config.semantic_thresholds.len(),
        config.fallback_thresholds.len(),
        config.margins.len(),
        config.semantic_thresholds.len() * config.fallback_thresholds.len() * config.margins.len()
    );

    let results = run_threshold_sweep(&harness, &scenarios, &config).await;
    print_sweep_results(&results);
}

#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_threshold_sweep_full() {
    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");

    let mut scenarios = Vec::new();
    scenarios.extend(taught_phrase_scenarios());
    scenarios.extend(session_scenarios());
    scenarios.extend(cbu_scenarios());
    scenarios.extend(hard_negative_scenarios());

    // Full sweep - comprehensive exploration
    let config = SweepConfig::full();

    println!(
        "Running FULL threshold sweep with {} scenarios...",
        scenarios.len()
    );
    println!(
        "  Testing {} semantic × {} fallback × {} margin = {} combinations",
        config.semantic_thresholds.len(),
        config.fallback_thresholds.len(),
        config.margins.len(),
        config.semantic_thresholds.len() * config.fallback_thresholds.len() * config.margins.len()
    );
    println!("  (This may take a few minutes...)\n");

    let results = run_threshold_sweep(&harness, &scenarios, &config).await;
    print_sweep_results(&results);
}

#[cfg(feature = "database")]
/// Interactive exploration - search and show top-5 results
#[tokio::test]
#[ignore]
async fn explore_query() {
    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");

    // Change this query to explore different inputs
    let query = "load the allianz book";

    println!("Query: \"{}\"\n", query);

    let results = harness.search_raw(query, 5).await.unwrap();

    if results.is_empty() {
        println!("No results found (below threshold)");
    } else {
        println!("Top {} results:", results.len());
        for (i, r) in results.iter().enumerate() {
            println!(
                "  {}. {} ({:.3}) via {:?}",
                i + 1,
                r.verb,
                r.score,
                r.source
            );
            println!("     matched: \"{}\"", r.matched_phrase);
        }
    }

    let outcome = check_ambiguity_with_margin(
        &results,
        harness.searcher.semantic_threshold(),
        AMBIGUITY_MARGIN,
    );
    println!("\nOutcome: {:?}", outcome);
}

// =============================================================================
// UNIT TESTS (no DB required)
// =============================================================================

#[test]
fn test_ambiguity_detection() {
    let threshold = 0.88;

    // Clear winner
    let clear_winner = vec![
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.95,
            source: VerbSearchSource::Semantic,
            matched_phrase: "create cbu".to_string(),
            description: None,
        },
        VerbSearchResult {
            verb: "entity.create-limited-company".to_string(),
            score: 0.85,
            source: VerbSearchSource::Semantic,
            matched_phrase: "create company".to_string(),
            description: None,
        },
    ];

    match check_ambiguity_with_margin(&clear_winner, threshold, AMBIGUITY_MARGIN) {
        VerbSearchOutcome::Matched(r) => assert_eq!(r.verb, "cbu.create"),
        other => panic!("Expected Matched, got {:?}", other),
    }

    // Ambiguous - top two scores are within AMBIGUITY_MARGIN of each other
    let ambiguous = vec![
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.92,
            source: VerbSearchSource::Semantic,
            matched_phrase: "create cbu".to_string(),
            description: None,
        },
        VerbSearchResult {
            verb: "entity.create-limited-company".to_string(),
            score: 0.90,
            source: VerbSearchSource::Semantic,
            matched_phrase: "create company".to_string(),
            description: None,
        },
    ];

    match check_ambiguity_with_margin(&ambiguous, threshold, AMBIGUITY_MARGIN) {
        VerbSearchOutcome::Ambiguous { .. } => {}
        other => panic!("Expected Ambiguous, got {:?}", other),
    }

    // No match (below threshold)
    let below = vec![VerbSearchResult {
        verb: "cbu.create".to_string(),
        score: 0.80,
        source: VerbSearchSource::Semantic,
        matched_phrase: "create cbu".to_string(),
        description: None,
    }];

    assert!(matches!(
        check_ambiguity_with_margin(&below, threshold, AMBIGUITY_MARGIN),
        VerbSearchOutcome::NoMatch
    ));
}

#[test]
fn test_normalize_candidates() {
    let candidates = vec![
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.80,
            source: VerbSearchSource::LearnedSemantic,
            matched_phrase: "make a cbu".to_string(),
            description: None,
        },
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.95,
            source: VerbSearchSource::PatternEmbedding,
            matched_phrase: "create cbu".to_string(),
            description: None,
        },
        VerbSearchResult {
            verb: "entity.create-limited-company".to_string(),
            score: 0.85,
            source: VerbSearchSource::Semantic,
            matched_phrase: "create company".to_string(),
            description: None,
        },
    ];

    let normalized = normalize_candidates(candidates, 10);

    assert_eq!(normalized.len(), 2);
    assert_eq!(normalized[0].verb, "cbu.create");
    assert!((normalized[0].score - 0.95).abs() < 0.001);
}

// =============================================================================
// MISMATCH DUMP (for analysis)
// =============================================================================

#[cfg(feature = "database")]
/// Mismatch entry for JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MismatchEntry {
    /// Test scenario name
    pub name: String,
    /// Input query
    pub query: String,
    /// Expected outcome type
    pub expected_outcome: String,
    /// Expected verb (if applicable)
    pub expected_verb: Option<String>,
    /// Allowed alternative verbs
    pub allowed_verbs: Vec<String>,
    /// Actual outcome type
    pub actual_outcome: String,
    /// Actual selected verb (if matched)
    pub actual_verb: Option<String>,
    /// Top-5 candidates with scores
    pub top_candidates: Vec<CandidateTrace>,
    /// Category of the test
    pub category: String,
    /// Is this a hard negative test?
    pub is_hard_negative: bool,
}

#[cfg(feature = "database")]
/// Full mismatch report for JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MismatchReport {
    /// Timestamp of the report
    pub timestamp: String,
    /// Total tests run
    pub total_tests: usize,
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests (mismatches)
    pub failed: usize,
    /// Pass rate percentage
    pub pass_rate: f64,
    /// Number of confidently wrong matches
    pub confidently_wrong: usize,
    /// Decision threshold used
    pub semantic_threshold: f32,
    /// Ambiguity margin used
    pub ambiguity_margin: f32,
    /// All mismatches
    pub mismatches: Vec<MismatchEntry>,
}

#[cfg(feature = "database")]
async fn collect_mismatches(
    harness: &VerbSearchTestHarness,
    scenarios: &[TestScenario],
) -> MismatchReport {
    let decision_threshold = harness.searcher.semantic_threshold();
    let mut report = TestReport::new();
    let mut mismatches = Vec::new();

    for scenario in scenarios {
        let start = std::time::Instant::now();
        match harness
            .search_with_decision_threshold(scenario.query, decision_threshold)
            .await
        {
            Ok((outcome, results)) => {
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                report.record(scenario, &outcome, &results, elapsed_ms, decision_threshold);

                // Check if this is a mismatch
                let is_correct = match (&scenario.expected_outcome, &outcome) {
                    (ExpectedOutcome::Matched, VerbSearchOutcome::Matched(r)) => {
                        scenario.allowed_verbs.contains(&r.verb.as_str())
                    }
                    (ExpectedOutcome::Ambiguous, VerbSearchOutcome::Ambiguous { .. })
                    | (ExpectedOutcome::Ambiguous, VerbSearchOutcome::Suggest { .. }) => true,
                    (ExpectedOutcome::NoMatch, VerbSearchOutcome::NoMatch) => true,
                    (ExpectedOutcome::MatchedOrAmbiguous, VerbSearchOutcome::Matched(r)) => {
                        scenario.allowed_verbs.contains(&r.verb.as_str())
                    }
                    (ExpectedOutcome::MatchedOrAmbiguous, VerbSearchOutcome::Ambiguous { .. })
                    | (ExpectedOutcome::MatchedOrAmbiguous, VerbSearchOutcome::Suggest { .. }) => {
                        true
                    }
                    _ => false,
                };

                if !is_correct {
                    let (actual_outcome_str, actual_verb) = match &outcome {
                        VerbSearchOutcome::Matched(r) => {
                            ("Matched".to_string(), Some(r.verb.clone()))
                        }
                        VerbSearchOutcome::Ambiguous { top, runner_up, .. } => (
                            format!("Ambiguous({} vs {})", top.verb, runner_up.verb),
                            None,
                        ),
                        VerbSearchOutcome::Suggest { candidates } => {
                            let verbs: Vec<_> =
                                candidates.iter().take(3).map(|c| c.verb.as_str()).collect();
                            (format!("Suggest({})", verbs.join(", ")), None)
                        }
                        VerbSearchOutcome::NoMatch => ("NoMatch".to_string(), None),
                    };

                    let top_candidates: Vec<CandidateTrace> = results
                        .iter()
                        .take(5)
                        .enumerate()
                        .map(|(i, r)| CandidateTrace {
                            rank: i + 1,
                            verb: r.verb.clone(),
                            score: r.score,
                            source: format!("{:?}", r.source),
                            matched_phrase: r.matched_phrase.clone(),
                        })
                        .collect();

                    mismatches.push(MismatchEntry {
                        name: scenario.name.to_string(),
                        query: scenario.query.to_string(),
                        expected_outcome: format!("{:?}", scenario.expected_outcome),
                        expected_verb: scenario.expected_verb.map(String::from),
                        allowed_verbs: scenario
                            .allowed_verbs
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                        actual_outcome: actual_outcome_str,
                        actual_verb,
                        top_candidates,
                        category: scenario.category.to_string(),
                        is_hard_negative: scenario.is_hard_negative,
                    });
                }
            }
            Err(e) => {
                report.errors += 1;
                report.total += 1;
                mismatches.push(MismatchEntry {
                    name: scenario.name.to_string(),
                    query: scenario.query.to_string(),
                    expected_outcome: format!("{:?}", scenario.expected_outcome),
                    expected_verb: scenario.expected_verb.map(String::from),
                    allowed_verbs: scenario
                        .allowed_verbs
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    actual_outcome: format!("Error: {}", e),
                    actual_verb: None,
                    top_candidates: vec![],
                    category: scenario.category.to_string(),
                    is_hard_negative: scenario.is_hard_negative,
                });
            }
        }
    }

    MismatchReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        total_tests: report.total,
        passed: report.passed,
        failed: report.failed,
        pass_rate: 100.0 * report.passed as f64 / report.total.max(1) as f64,
        confidently_wrong: report.confidently_wrong,
        semantic_threshold: decision_threshold,
        ambiguity_margin: AMBIGUITY_MARGIN,
        mismatches,
    }
}

/// Dump all mismatches to a JSON file for analysis
///
/// Usage: cargo x test-verbs --dump-mismatch /path/to/output.json
///
/// This runs all test scenarios and outputs detailed information about
/// any mismatches in JSON format for easier analysis.
#[cfg(feature = "database")]
#[tokio::test]
#[ignore]
async fn test_dump_mismatches() {
    // Get output path from environment variable
    let output_path = match std::env::var("VERB_SEARCH_DUMP_MISMATCH") {
        Ok(path) => std::path::PathBuf::from(path),
        Err(_) => {
            println!("VERB_SEARCH_DUMP_MISMATCH not set, using default: mismatches.json");
            std::path::PathBuf::from("mismatches.json")
        }
    };

    let harness = VerbSearchTestHarness::new()
        .await
        .expect("Failed to create harness");

    // Collect all scenarios
    let mut all_scenarios = Vec::new();
    all_scenarios.extend(taught_phrase_scenarios());
    all_scenarios.extend(session_scenarios());
    all_scenarios.extend(cbu_scenarios());
    all_scenarios.extend(entity_scenarios());
    all_scenarios.extend(kyc_scenarios());
    all_scenarios.extend(view_scenarios());
    all_scenarios.extend(hard_negative_scenarios());
    all_scenarios.extend(edge_case_scenarios());

    println!("Running {} test scenarios...", all_scenarios.len());

    let report = collect_mismatches(&harness, &all_scenarios).await;

    // Print summary
    println!("\n============================================================");
    println!("                    MISMATCH REPORT");
    println!("============================================================\n");
    println!("Total tests:        {}", report.total_tests);
    println!("Passed:             {}", report.passed);
    println!("Failed:             {}", report.failed);
    println!("Pass rate:          {:.1}%", report.pass_rate);
    println!("Confidently wrong:  {}", report.confidently_wrong);
    println!("\nMismatches: {}", report.mismatches.len());

    if !report.mismatches.is_empty() {
        println!("\n--- Mismatches ---");
        for (i, m) in report.mismatches.iter().enumerate() {
            println!("\n{}. {} [{}]", i + 1, m.name, m.category);
            println!("   Query: \"{}\"", m.query);
            println!(
                "   Expected: {} → {:?}",
                m.expected_outcome, m.expected_verb
            );
            println!("   Actual:   {} → {:?}", m.actual_outcome, m.actual_verb);
            if !m.top_candidates.is_empty() {
                println!("   Top candidates:");
                for c in m.top_candidates.iter().take(3) {
                    println!(
                        "     {}. {} ({:.3}) \"{}\"",
                        c.rank, c.verb, c.score, c.matched_phrase
                    );
                }
            }
        }
    }

    // Write JSON output
    let json = serde_json::to_string_pretty(&report).expect("Failed to serialize report");
    std::fs::write(&output_path, &json).expect("Failed to write output file");
    println!("\n============================================================");
    println!("  Report written to: {}", output_path.display());
    println!("============================================================");
}
