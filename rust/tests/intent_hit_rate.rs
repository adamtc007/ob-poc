//! Intent Pipeline Hit Rate Test Suite
//!
//! Measures utterance -> verb selection accuracy against the fixture corpus.
//! Generates a detailed report with per-category and per-domain breakdowns.
//!
//! Usage:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test intent_hit_rate -- --ignored --nocapture
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test intent_hit_rate -- --ignored --nocapture easy
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test intent_hit_rate -- --ignored --nocapture hard
//!   INTENT_THRESHOLD=0.60 DATABASE_URL="postgresql:///data_designer" cargo test --features database --test intent_hit_rate -- --ignored --nocapture
//!
//! Environment:
//!   DATABASE_URL    -- PostgreSQL connection (required)
//!   INTENT_THRESHOLD -- Override semantic_threshold (default: from config)
//!   INTENT_MARGIN    -- Override ambiguity_margin (default: from config)
//!   INTENT_VERBOSE   -- Set to "1" for per-utterance trace output

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Deserialize;
use sqlx::PgPool;

// ============================================================================
// Fixture Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct TestFixture {
    #[serde(rename = "test")]
    tests: Vec<TestCase>,
}

#[derive(Debug, Deserialize, Clone)]
struct TestCase {
    utterance: String,
    expected_verb: String,
    category: String,
    difficulty: String,
    #[serde(default)]
    domain_hint: Option<String>,
    #[serde(default)]
    alt_verbs: Vec<String>,
    #[serde(default)]
    notes: Option<String>,
    // ECIR (Entity-Centric Intent Resolution) fields
    #[serde(default)]
    expected_noun: Option<String>,
    #[serde(default)]
    expected_action: Option<String>,
    #[serde(default)]
    ecir_path: Option<String>,
    // Tier -2 (Scenario / MacroIndex) fields
    #[serde(default)]
    expected_tier: Option<String>,
    #[serde(default)]
    expected_scenario_id: Option<String>,
    #[serde(default)]
    expected_route_target: Option<String>,
}

// ============================================================================
// Result Types
// ============================================================================

#[derive(Debug, Clone)]
struct TestResult {
    case: TestCase,
    outcome: Outcome,
    selected_verb: Option<String>,
    selected_score: Option<f32>,
    top_candidates: Vec<(String, f32)>,
    latency: Duration,
    pipeline_outcome: String, // Ready, NeedsClarification, NoMatch, etc.
    /// Source of the top candidate (e.g., "NounTaxonomy", "GlobalSemantic", etc.)
    top_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    /// Pipeline selected the expected verb (or an acceptable alt)
    Hit,
    /// Pipeline selected correct verb but through disambiguation (2nd prompt)
    HitWithClarification,
    /// Pipeline returned NeedsClarification and expected verb is in options
    ClarificationAvailable,
    /// Pipeline selected wrong verb
    Miss,
    /// Pipeline returned NoMatch
    NoMatch,
    /// Pipeline error
    Error(String),
}

impl Outcome {
    fn is_first_attempt_hit(&self) -> bool {
        matches!(self, Outcome::Hit)
    }

    fn is_two_attempt_hit(&self) -> bool {
        matches!(
            self,
            Outcome::Hit | Outcome::HitWithClarification | Outcome::ClarificationAvailable
        )
    }

    fn symbol(&self) -> &str {
        match self {
            Outcome::Hit => "✅",
            Outcome::HitWithClarification => "🟡",
            Outcome::ClarificationAvailable => "🔵",
            Outcome::Miss => "❌",
            Outcome::NoMatch => "⬛",
            Outcome::Error(_) => "💥",
        }
    }
}

// ============================================================================
// Test Runner
// ============================================================================

#[cfg(test)]
#[cfg(feature = "database")]
mod tests {
    use super::*;

    /// Main test entry point -- runs all fixture utterances through the pipeline
    #[tokio::test]
    #[ignore] // Requires DATABASE_URL and populated embeddings
    async fn intent_hit_rate() {
        // Load fixtures
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/intent_test_utterances.toml"
        );
        let fixture_content =
            std::fs::read_to_string(fixture_path).expect("Failed to read test fixtures");
        let fixture: TestFixture =
            toml::from_str(&fixture_content).expect("Failed to parse test fixtures");

        // Filter by command-line arg if provided
        let filter = std::env::args().last().unwrap_or_default();
        let tests: Vec<TestCase> = fixture
            .tests
            .into_iter()
            .filter(|t| match filter.as_str() {
                "easy" => t.difficulty == "easy",
                "medium" => t.difficulty == "medium",
                "hard" => t.difficulty == "hard" || t.difficulty == "expert",
                "expert" => t.difficulty == "expert",
                "adversarial" => t.category == "adversarial",
                "direct" => t.category == "direct",
                "natural" => t.category == "natural",
                "indirect" => t.category == "indirect",
                "scenario" => t.category == "scenario",
                "macro_match" => t.category == "macro_match",
                "tier2_blocker" => t.category == "tier2_blocker",
                "tier2" => {
                    t.category == "scenario"
                        || t.category == "macro_match"
                        || t.category == "tier2_blocker"
                }
                _ => true,
            })
            .collect();

        let verbose = std::env::var("INTENT_VERBOSE").unwrap_or_default() == "1";

        // Connect to DB
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL required for intent tests");
        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database");

        // Build the verb searcher (same as production)
        let searcher = build_test_searcher(&pool).await;

        // Run each test case
        let mut results: Vec<TestResult> = Vec::new();
        let total = tests.len();

        println!("\n=======================================================================");
        println!("  INTENT PIPELINE HIT RATE TEST -- {} utterances", total);
        println!("=======================================================================\n");

        for (i, case) in tests.iter().enumerate() {
            let start = Instant::now();

            let search_result = searcher
                .search(
                    &case.utterance,
                    None,                        // user_id
                    case.domain_hint.as_deref(), // domain_filter
                    5,                           // limit
                    None,                        // allowed_verbs (no SemReg in test)
                )
                .await;

            let latency = start.elapsed();

            let result = match search_result {
                Ok(candidates) => {
                    let top_candidates: Vec<(String, f32)> = candidates
                        .iter()
                        .take(5)
                        .map(|c| (c.verb.clone(), c.score))
                        .collect();

                    let top_source = candidates.first().map(|c| format!("{:?}", c.source));

                    // Check ambiguity (same logic as production pipeline)
                    let threshold = searcher.semantic_threshold();
                    let ambiguity =
                        ob_poc::mcp::verb_search::check_ambiguity(&candidates, threshold);

                    let (outcome, pipeline_outcome, selected_verb, selected_score) =
                        classify_outcome(
                            &ambiguity,
                            &candidates,
                            &case.expected_verb,
                            &case.alt_verbs,
                            threshold,
                        );

                    TestResult {
                        case: case.clone(),
                        outcome,
                        selected_verb,
                        selected_score,
                        top_candidates,
                        latency,
                        pipeline_outcome,
                        top_source,
                    }
                }
                Err(e) => TestResult {
                    case: case.clone(),
                    outcome: Outcome::Error(e.to_string()),
                    selected_verb: None,
                    selected_score: None,
                    top_candidates: vec![],
                    latency,
                    pipeline_outcome: "Error".into(),
                    top_source: None,
                },
            };

            if verbose {
                print_verbose_result(&result, i + 1, total);
            } else {
                print!("{} ", result.outcome.symbol());
                if (i + 1) % 40 == 0 {
                    println!(" [{}/{}]", i + 1, total);
                }
            }

            results.push(result);
        }

        println!("\n");

        // Generate report
        print_summary_report(&results);
        print_ecir_report(&results);
        print_tier2_report(&results);
        print_category_breakdown(&results);
        print_difficulty_breakdown(&results);
        print_domain_breakdown(&results);
        print_failures(&results);
        print_latency_stats(&results);

        // Assert minimum thresholds
        let first_attempt = results
            .iter()
            .filter(|r| r.outcome.is_first_attempt_hit())
            .count();
        let two_attempt = results
            .iter()
            .filter(|r| r.outcome.is_two_attempt_hit())
            .count();
        let total = results.len();

        let first_rate = first_attempt as f64 / total as f64 * 100.0;
        let two_rate = two_attempt as f64 / total as f64 * 100.0;

        // These are aspirational -- adjust as pipeline improves
        // Phase 0 (baseline): first >= 30%, two >= 50%
        // Phase 1 (filled phrases): first >= 50%, two >= 70%
        // Phase 2 (LLM classifier): first >= 70%, two >= 90%
        println!("\n=======================================================================");
        println!(
            "  ASSERTIONS: first-attempt={:.1}% (target: 35%), two-attempt={:.1}% (target: 55%)",
            first_rate, two_rate
        );
        println!("=======================================================================");

        assert!(
            first_rate >= 25.0,
            "First-attempt hit rate {:.1}% is below minimum 25%",
            first_rate
        );
    }
}

// ============================================================================
// Classification
// ============================================================================

fn classify_outcome(
    ambiguity: &ob_poc::mcp::verb_search::VerbSearchOutcome,
    candidates: &[ob_poc::mcp::verb_search::VerbSearchResult],
    expected: &str,
    alts: &[String],
    _threshold: f32,
) -> (Outcome, String, Option<String>, Option<f32>) {
    use ob_poc::mcp::verb_search::VerbSearchOutcome;

    let is_expected = |verb: &str| -> bool { verb == expected || alts.iter().any(|a| a == verb) };

    match ambiguity {
        VerbSearchOutcome::Matched(matched) => {
            if is_expected(&matched.verb) {
                (
                    Outcome::Hit,
                    "Ready".into(),
                    Some(matched.verb.clone()),
                    Some(matched.score),
                )
            } else {
                (
                    Outcome::Miss,
                    "Ready (wrong verb)".into(),
                    Some(matched.verb.clone()),
                    Some(matched.score),
                )
            }
        }
        VerbSearchOutcome::Ambiguous { top, runner_up, .. } => {
            if is_expected(&top.verb) {
                (
                    Outcome::HitWithClarification,
                    "Ambiguous (expected is top)".into(),
                    Some(top.verb.clone()),
                    Some(top.score),
                )
            } else if is_expected(&runner_up.verb) {
                (
                    Outcome::ClarificationAvailable,
                    "Ambiguous (expected is runner-up)".into(),
                    Some(top.verb.clone()),
                    Some(top.score),
                )
            } else if candidates.iter().any(|c| is_expected(&c.verb)) {
                (
                    Outcome::ClarificationAvailable,
                    "Ambiguous (expected in candidates)".into(),
                    Some(top.verb.clone()),
                    Some(top.score),
                )
            } else {
                (
                    Outcome::Miss,
                    "Ambiguous (expected not in candidates)".into(),
                    Some(top.verb.clone()),
                    Some(top.score),
                )
            }
        }
        VerbSearchOutcome::Suggest {
            candidates: suggestions,
        } => {
            if suggestions.iter().any(|c| is_expected(&c.verb)) {
                (
                    Outcome::ClarificationAvailable,
                    "Suggest (expected in suggestions)".into(),
                    suggestions.first().map(|c| c.verb.clone()),
                    suggestions.first().map(|c| c.score),
                )
            } else {
                (
                    Outcome::Miss,
                    "Suggest (expected not found)".into(),
                    suggestions.first().map(|c| c.verb.clone()),
                    suggestions.first().map(|c| c.score),
                )
            }
        }
        VerbSearchOutcome::NoMatch => (Outcome::NoMatch, "NoMatch".into(), None, None),
    }
}

// ============================================================================
// Report Printing
// ============================================================================

fn print_summary_report(results: &[TestResult]) {
    let total = results.len();
    let hits = results
        .iter()
        .filter(|r| r.outcome.is_first_attempt_hit())
        .count();
    let two_hits = results
        .iter()
        .filter(|r| r.outcome.is_two_attempt_hit())
        .count();
    let misses = results
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Miss))
        .count();
    let no_matches = results
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::NoMatch))
        .count();
    let errors = results
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Error(_)))
        .count();

    println!("=======================================================================");
    println!("  SUMMARY");
    println!("=======================================================================");
    println!("  Total test cases:          {}", total);
    println!(
        "  First-attempt hits:        {} ({:.1}%)",
        hits,
        hits as f64 / total as f64 * 100.0
    );
    println!(
        "  Hit w/ clarification:      {}",
        results
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::HitWithClarification))
            .count()
    );
    println!(
        "  Available in menu:         {}",
        results
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::ClarificationAvailable))
            .count()
    );
    println!(
        "  Two-attempt hit rate:      {} ({:.1}%)",
        two_hits,
        two_hits as f64 / total as f64 * 100.0
    );
    println!("  Wrong verb selected:       {}", misses);
    println!("  No match at all:           {}", no_matches);
    println!("  Errors:                    {}", errors);
    println!();
}

fn print_ecir_report(results: &[TestResult]) {
    // Count ECIR-sourced results (NounTaxonomy tier)
    let ecir_hits: Vec<&TestResult> = results
        .iter()
        .filter(|r| {
            r.top_source
                .as_ref()
                .map_or(false, |s| s.contains("NounTaxonomy"))
        })
        .collect();

    let deterministic = ecir_hits
        .iter()
        .filter(|r| r.selected_score == Some(0.95))
        .count();
    let narrow = ecir_hits
        .iter()
        .filter(|r| r.selected_score == Some(0.80))
        .count();

    // Count annotated test cases
    let annotated_total = results
        .iter()
        .filter(|r| r.case.ecir_path.is_some())
        .count();
    let annotated_deterministic = results
        .iter()
        .filter(|r| r.case.ecir_path.as_deref() == Some("deterministic"))
        .count();
    let annotated_narrow = results
        .iter()
        .filter(|r| r.case.ecir_path.as_deref() == Some("narrow"))
        .count();
    let annotated_fallthrough = results
        .iter()
        .filter(|r| r.case.ecir_path.as_deref() == Some("fallthrough"))
        .count();

    // ECIR accuracy on annotated cases
    let ecir_correct = results
        .iter()
        .filter(|r| {
            let is_ecir = r
                .top_source
                .as_ref()
                .map_or(false, |s| s.contains("NounTaxonomy"));
            let expected_ecir = matches!(
                r.case.ecir_path.as_deref(),
                Some("deterministic") | Some("narrow")
            );
            // Correct if: ECIR fired AND expected to fire AND got right verb
            is_ecir && expected_ecir && r.outcome.is_first_attempt_hit()
        })
        .count();
    let ecir_expected = annotated_deterministic + annotated_narrow;

    // ECIR false positives (fired but wasn't expected)
    let ecir_false_pos = results
        .iter()
        .filter(|r| {
            let is_ecir = r
                .top_source
                .as_ref()
                .map_or(false, |s| s.contains("NounTaxonomy"));
            let expected_fallthrough = r.case.ecir_path.as_deref() == Some("fallthrough");
            is_ecir && expected_fallthrough
        })
        .count();

    println!("  ECIR (Entity-Centric Intent Resolution)");
    println!("  {:-<68}", "");
    println!(
        "  Total ECIR resolutions:    {} / {} ({:.1}%)",
        ecir_hits.len(),
        results.len(),
        ecir_hits.len() as f64 / results.len() as f64 * 100.0
    );
    println!("    Deterministic (0.95):    {}", deterministic);
    println!("    Narrow set (0.80):       {}", narrow);
    println!();
    println!(
        "  Annotated test cases:      {} / {}",
        annotated_total,
        results.len()
    );
    println!("    deterministic:           {}", annotated_deterministic);
    println!("    narrow:                  {}", annotated_narrow);
    println!("    fallthrough:             {}", annotated_fallthrough);
    println!();
    if ecir_expected > 0 {
        println!(
            "  ECIR accuracy (annotated): {} / {} ({:.1}%)",
            ecir_correct,
            ecir_expected,
            ecir_correct as f64 / ecir_expected as f64 * 100.0
        );
    }
    if ecir_false_pos > 0 {
        println!(
            "  ECIR false positives:      {} (fired on fallthrough cases)",
            ecir_false_pos
        );
    }

    // Show individual ECIR-resolved cases
    if !ecir_hits.is_empty() {
        println!();
        println!("  ECIR-resolved utterances:");
        for r in &ecir_hits {
            let mark = if r.outcome.is_first_attempt_hit() {
                "✓"
            } else {
                "✗"
            };
            println!(
                "    {} \"{}\" → {} ({:.2})",
                mark,
                truncate(&r.case.utterance, 45),
                r.selected_verb.as_deref().unwrap_or("?"),
                r.selected_score.unwrap_or(0.0),
            );
        }
    }
    println!();
}

fn print_tier2_report(results: &[TestResult]) {
    // --- Tier distribution across all results ---
    let tier2a_count = results
        .iter()
        .filter(|r| {
            r.top_source
                .as_ref()
                .map_or(false, |s| s.contains("ScenarioIndex"))
        })
        .count();
    let tier2b_count = results
        .iter()
        .filter(|r| r.top_source.as_ref().map_or(false, |s| s == "MacroIndex"))
        .count();
    let ecir_count = results
        .iter()
        .filter(|r| {
            r.top_source
                .as_ref()
                .map_or(false, |s| s.contains("NounTaxonomy"))
        })
        .count();
    let macro_count = results
        .iter()
        .filter(|r| r.top_source.as_ref().map_or(false, |s| s == "Macro"))
        .count();
    let other_count = results.len() - tier2a_count - tier2b_count - ecir_count - macro_count;

    println!("  TIER -2 (Scenario-Based Intent Resolution)");
    println!("  {:-<68}", "");
    println!("  Resolution tier distribution:");
    println!(
        "    Tier -2A (ScenarioIndex):   {:3} ({:.1}%)",
        tier2a_count,
        tier2a_count as f64 / results.len() as f64 * 100.0
    );
    println!(
        "    Tier -2B (MacroIndex):      {:3} ({:.1}%)",
        tier2b_count,
        tier2b_count as f64 / results.len() as f64 * 100.0
    );
    println!(
        "    Tier -1  (ECIR/NounTax):    {:3} ({:.1}%)",
        ecir_count,
        ecir_count as f64 / results.len() as f64 * 100.0
    );
    println!(
        "    Tier  0  (Macro exact):     {:3} ({:.1}%)",
        macro_count,
        macro_count as f64 / results.len() as f64 * 100.0
    );
    println!(
        "    Tiers 1+ (Embedding/etc):   {:3} ({:.1}%)",
        other_count,
        other_count as f64 / results.len() as f64 * 100.0
    );
    println!();

    // --- Scenario match rate (category = "scenario") ---
    let scenario_cases: Vec<&TestResult> = results
        .iter()
        .filter(|r| r.case.category == "scenario")
        .collect();
    if !scenario_cases.is_empty() {
        let scenario_correct = scenario_cases
            .iter()
            .filter(|r| r.outcome.is_first_attempt_hit())
            .count();
        let scenario_tier_correct = scenario_cases
            .iter()
            .filter(|r| {
                r.top_source
                    .as_ref()
                    .map_or(false, |s| s.contains("ScenarioIndex"))
                    && r.outcome.is_first_attempt_hit()
            })
            .count();
        let scenario_route_correct = scenario_cases
            .iter()
            .filter(|r| {
                if let Some(ref target) = r.case.expected_route_target {
                    // Check if the journey route matches the expected target
                    r.selected_verb.as_deref() == Some(target.as_str())
                        && r.outcome.is_first_attempt_hit()
                } else {
                    r.outcome.is_first_attempt_hit()
                }
            })
            .count();

        println!("  Scenario test cases ({} total):", scenario_cases.len());
        println!(
            "    Correct verb:              {:3}/{:3} ({:.1}%)  target: >=80%",
            scenario_correct,
            scenario_cases.len(),
            scenario_correct as f64 / scenario_cases.len() as f64 * 100.0
        );
        println!(
            "    Via ScenarioIndex tier:    {:3}/{:3} ({:.1}%)",
            scenario_tier_correct,
            scenario_cases.len(),
            scenario_tier_correct as f64 / scenario_cases.len() as f64 * 100.0
        );
        println!(
            "    Correct route target:      {:3}/{:3} ({:.1}%)  target: >=90%",
            scenario_route_correct,
            scenario_cases.len(),
            scenario_route_correct as f64 / scenario_cases.len() as f64 * 100.0
        );
        println!();

        // Individual scenario results
        println!("  Scenario-resolved utterances:");
        for r in &scenario_cases {
            let mark = if r.outcome.is_first_attempt_hit() {
                "✓"
            } else {
                "✗"
            };
            let tier = r.top_source.as_deref().unwrap_or("?");
            println!(
                "    {} \"{}\" → {} ({:.2}) [{}]",
                mark,
                truncate(&r.case.utterance, 42),
                r.selected_verb.as_deref().unwrap_or("?"),
                r.selected_score.unwrap_or(0.0),
                tier,
            );
        }
        println!();
    }

    // --- MacroIndex match rate (category = "macro_match") ---
    let macro_match_cases: Vec<&TestResult> = results
        .iter()
        .filter(|r| r.case.category == "macro_match")
        .collect();
    if !macro_match_cases.is_empty() {
        let macro_correct = macro_match_cases
            .iter()
            .filter(|r| r.outcome.is_first_attempt_hit())
            .count();
        let macro_tier_correct = macro_match_cases
            .iter()
            .filter(|r| {
                r.top_source.as_ref().map_or(false, |s| s == "MacroIndex")
                    && r.outcome.is_first_attempt_hit()
            })
            .count();

        println!(
            "  MacroIndex test cases ({} total):",
            macro_match_cases.len()
        );
        println!(
            "    Correct verb:              {:3}/{:3} ({:.1}%)  target: >=75%",
            macro_correct,
            macro_match_cases.len(),
            macro_correct as f64 / macro_match_cases.len() as f64 * 100.0
        );
        println!(
            "    Via MacroIndex tier:       {:3}/{:3} ({:.1}%)",
            macro_tier_correct,
            macro_match_cases.len(),
            macro_tier_correct as f64 / macro_match_cases.len() as f64 * 100.0
        );
        println!();

        println!("  MacroIndex-resolved utterances:");
        for r in &macro_match_cases {
            let mark = if r.outcome.is_first_attempt_hit() {
                "✓"
            } else {
                "✗"
            };
            let tier = r.top_source.as_deref().unwrap_or("?");
            println!(
                "    {} \"{}\" → {} ({:.2}) [{}]",
                mark,
                truncate(&r.case.utterance, 42),
                r.selected_verb.as_deref().unwrap_or("?"),
                r.selected_score.unwrap_or(0.0),
                tier,
            );
        }
        println!();
    }

    // --- False positive rate (category = "tier2_blocker") ---
    let blocker_cases: Vec<&TestResult> = results
        .iter()
        .filter(|r| r.case.category == "tier2_blocker")
        .collect();
    if !blocker_cases.is_empty() {
        let blocker_intercepted = blocker_cases
            .iter()
            .filter(|r| {
                r.top_source
                    .as_ref()
                    .map_or(false, |s| s.contains("ScenarioIndex") || s == "MacroIndex")
            })
            .count();
        let blocker_correct = blocker_cases
            .iter()
            .filter(|r| r.outcome.is_first_attempt_hit())
            .count();

        println!(
            "  Tier -2 blocker test cases ({} total):",
            blocker_cases.len()
        );
        println!(
            "    Correct verb (any tier):   {:3}/{:3} ({:.1}%)",
            blocker_correct,
            blocker_cases.len(),
            blocker_correct as f64 / blocker_cases.len() as f64 * 100.0
        );
        println!(
            "    FALSE POSITIVE (Tier -2):  {:3}/{:3} ({:.1}%)  target: <5%",
            blocker_intercepted,
            blocker_cases.len(),
            blocker_intercepted as f64 / blocker_cases.len() as f64 * 100.0
        );
        if blocker_intercepted > 0 {
            println!("    Intercepted utterances:");
            for r in &blocker_cases {
                let is_tier2 = r
                    .top_source
                    .as_ref()
                    .map_or(false, |s| s.contains("ScenarioIndex") || s == "MacroIndex");
                if is_tier2 {
                    println!(
                        "      ! \"{}\" → {} ({:.2}) [{}]",
                        truncate(&r.case.utterance, 42),
                        r.selected_verb.as_deref().unwrap_or("?"),
                        r.selected_score.unwrap_or(0.0),
                        r.top_source.as_deref().unwrap_or("?"),
                    );
                }
            }
        }
        println!();
    }
}

fn print_category_breakdown(results: &[TestResult]) {
    println!("  BY CATEGORY");
    println!("  {:-<68}", "");

    let categories = [
        "direct",
        "natural",
        "indirect",
        "contextual",
        "adversarial",
        "multi_intent",
        "scenario",
        "macro_match",
        "tier2_blocker",
    ];
    for cat in categories {
        let subset: Vec<&TestResult> = results.iter().filter(|r| r.case.category == cat).collect();
        if subset.is_empty() {
            continue;
        }
        let hits = subset
            .iter()
            .filter(|r| r.outcome.is_first_attempt_hit())
            .count();
        let two = subset
            .iter()
            .filter(|r| r.outcome.is_two_attempt_hit())
            .count();
        println!(
            "  {:15} {:3}/{:3} first ({:5.1}%)  {:3}/{:3} two-attempt ({:5.1}%)",
            cat,
            hits,
            subset.len(),
            hits as f64 / subset.len() as f64 * 100.0,
            two,
            subset.len(),
            two as f64 / subset.len() as f64 * 100.0,
        );
    }
    println!();
}

fn print_difficulty_breakdown(results: &[TestResult]) {
    println!("  BY DIFFICULTY");
    println!("  {:-<68}", "");

    for diff in ["easy", "medium", "hard", "expert"] {
        let subset: Vec<&TestResult> = results
            .iter()
            .filter(|r| r.case.difficulty == diff)
            .collect();
        if subset.is_empty() {
            continue;
        }
        let hits = subset
            .iter()
            .filter(|r| r.outcome.is_first_attempt_hit())
            .count();
        let two = subset
            .iter()
            .filter(|r| r.outcome.is_two_attempt_hit())
            .count();
        println!(
            "  {:15} {:3}/{:3} first ({:5.1}%)  {:3}/{:3} two-attempt ({:5.1}%)",
            diff,
            hits,
            subset.len(),
            hits as f64 / subset.len() as f64 * 100.0,
            two,
            subset.len(),
            two as f64 / subset.len() as f64 * 100.0,
        );
    }
    println!();
}

fn print_domain_breakdown(results: &[TestResult]) {
    println!("  BY DOMAIN");
    println!("  {:-<68}", "");

    let mut domain_stats: HashMap<String, (usize, usize, usize)> = HashMap::new();
    for r in results {
        let domain = r
            .case
            .expected_verb
            .split('.')
            .next()
            .unwrap_or("unknown")
            .to_string();
        let entry = domain_stats.entry(domain).or_insert((0, 0, 0));
        entry.0 += 1; // total
        if r.outcome.is_first_attempt_hit() {
            entry.1 += 1;
        }
        if r.outcome.is_two_attempt_hit() {
            entry.2 += 1;
        }
    }

    let mut domains: Vec<_> = domain_stats.into_iter().collect();
    domains.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));

    for (domain, (total, first, two)) in &domains {
        println!(
            "  {:20} {:3}/{:3} first ({:5.1}%)  {:3}/{:3} two-attempt ({:5.1}%)",
            domain,
            first,
            total,
            *first as f64 / *total as f64 * 100.0,
            two,
            total,
            *two as f64 / *total as f64 * 100.0,
        );
    }
    println!();
}

fn print_failures(results: &[TestResult]) {
    let failures: Vec<&TestResult> = results
        .iter()
        .filter(|r| !r.outcome.is_two_attempt_hit())
        .collect();

    if failures.is_empty() {
        return;
    }

    println!("  FAILURES ({} cases)", failures.len());
    println!("  {:-<68}", "");

    for r in failures {
        println!(
            "  {} \"{}\"",
            r.outcome.symbol(),
            truncate(&r.case.utterance, 50),
        );
        println!(
            "      Expected: {}  Got: {}  Score: {:.3}",
            r.case.expected_verb,
            r.selected_verb.as_deref().unwrap_or("(none)"),
            r.selected_score.unwrap_or(0.0),
        );
        if !r.top_candidates.is_empty() {
            let cands: Vec<String> = r
                .top_candidates
                .iter()
                .take(3)
                .map(|(v, s)| format!("{}:{:.2}", v, s))
                .collect();
            println!("      Top-3: {}", cands.join(", "));
        }
        if let Some(ref notes) = r.case.notes {
            println!("      Notes: {}", notes);
        }
        println!();
    }
}

fn print_latency_stats(results: &[TestResult]) {
    let mut latencies: Vec<f64> = results
        .iter()
        .map(|r| r.latency.as_millis() as f64)
        .collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
    let p99 = latencies[(latencies.len() as f64 * 0.99) as usize];

    println!("  LATENCY (verb search only, excludes LLM arg extraction)");
    println!("  {:-<68}", "");
    println!(
        "  Avg: {:.0}ms  P50: {:.0}ms  P95: {:.0}ms  P99: {:.0}ms",
        avg, p50, p95, p99
    );
    println!();
}

fn print_verbose_result(result: &TestResult, index: usize, total: usize) {
    println!(
        "[{:3}/{}] {} {} -> {}",
        index,
        total,
        result.outcome.symbol(),
        truncate(&result.case.utterance, 45),
        result.selected_verb.as_deref().unwrap_or("(none)"),
    );
    if !result.outcome.is_first_attempt_hit() {
        println!(
            "         Expected: {}  Pipeline: {}  Score: {:.3}  Latency: {:?}",
            result.case.expected_verb,
            result.pipeline_outcome,
            result.selected_score.unwrap_or(0.0),
            result.latency,
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

// ============================================================================
// Searcher Construction (mirrors production setup)
// ============================================================================

async fn build_test_searcher(pool: &PgPool) -> ob_poc::mcp::verb_search::HybridVerbSearcher {
    use dsl_core::config::ConfigLoader;
    use ob_poc::agent::learning::embedder::CandleEmbedder;
    use ob_poc::agent::learning::embedder::Embedder;
    use ob_poc::agent::learning::warmup::LearningWarmup;
    use ob_poc::database::verb_service::VerbService;
    use ob_poc::dsl_v2::macros::load_macro_registry_from_dir;
    use ob_poc::mcp::macro_index::MacroIndex;
    use ob_poc::mcp::noun_index::{NounIndex, VerbContractIndex};
    use ob_poc::mcp::scenario_index::ScenarioIndex;
    use ob_poc::mcp::verb_search::HybridVerbSearcher;
    use std::path::Path;
    use std::sync::Arc;

    // Env overrides for threshold sweeps
    let threshold_override = std::env::var("INTENT_THRESHOLD")
        .ok()
        .and_then(|v| v.parse::<f32>().ok());
    let fallback_override = std::env::var("INTENT_FALLBACK")
        .ok()
        .and_then(|v| v.parse::<f32>().ok());

    // Build embedder (BGE-small-en-v1.5, 384-dim)
    // Note: CandleEmbedder::new() is synchronous (not async)
    let embedder = Arc::new(CandleEmbedder::new().expect("Failed to initialize BGE embedder"));
    let dyn_embedder: Arc<dyn Embedder> = embedder as Arc<dyn Embedder>;

    // Build verb service (DB-backed verb patterns)
    let verb_service = Arc::new(VerbService::new(pool.clone()));

    // Load learned data (invocation_phrases, entity_aliases, user corrections)
    let warmup = LearningWarmup::new(pool.clone());
    let (learned_data, stats) = warmup
        .warmup()
        .await
        .expect("Failed to warmup learning data");
    println!(
        "  Warmup: {} phrases, {} entity aliases loaded",
        stats.invocation_phrases_loaded, stats.entity_aliases_loaded
    );

    // Build VerbContractIndex from verb YAML config (needed for NounIndex ECIR)
    let verb_contract_index = {
        let config_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/config");
        let loader = ConfigLoader::new(config_dir);
        match loader.load_verbs() {
            Ok(verbs_config) => {
                let vci = VerbContractIndex::from_verbs_config(&verbs_config);
                println!("  VerbContractIndex: {} verbs indexed", vci.len());
                Some(vci)
            }
            Err(e) => {
                eprintln!("  VerbContractIndex: failed to load verbs config: {}", e);
                None
            }
        }
    };

    // Load NounIndex for ECIR (Tier -1 deterministic resolution)
    let noun_index = {
        let yaml_paths = [
            concat!(env!("CARGO_MANIFEST_DIR"), "/config/noun_index.yaml"),
            "config/noun_index.yaml",
        ];
        let mut loaded = None;
        if let Some(vci) = verb_contract_index {
            for path in &yaml_paths {
                if let Ok(content) = std::fs::read_to_string(path) {
                    match NounIndex::from_yaml(&content, vci.clone()) {
                        Ok(ni) => {
                            println!(
                                "  NounIndex: loaded {} nouns from {}",
                                ni.canonical_count(),
                                path
                            );
                            loaded = Some(Arc::new(ni));
                            break;
                        }
                        Err(e) => {
                            eprintln!("  NounIndex: failed to parse {}: {}", path, e);
                        }
                    }
                }
            }
        }
        if loaded.is_none() {
            println!("  NounIndex: not found (ECIR disabled in test)");
        }
        loaded
    };

    // Load MacroRegistry + MacroIndex (Tier -2B)
    let macro_index = {
        let macro_dir_paths = [
            concat!(env!("CARGO_MANIFEST_DIR"), "/config/verb_schemas/macros"),
            "config/verb_schemas/macros",
        ];
        let mut loaded = None;
        for path in &macro_dir_paths {
            let dir = Path::new(path);
            if dir.is_dir() {
                match load_macro_registry_from_dir(dir) {
                    Ok(registry) => {
                        let index = MacroIndex::from_registry(&registry, None);
                        println!(
                            "  MacroIndex: built from {} macros ({})",
                            registry.len(),
                            path
                        );
                        loaded = Some(Arc::new(index));
                        break;
                    }
                    Err(e) => {
                        eprintln!("  MacroIndex: failed to load registry from {}: {}", path, e);
                    }
                }
            }
        }
        if loaded.is_none() {
            println!("  MacroIndex: not found (Tier -2B disabled in test)");
        }
        loaded
    };

    // Load ScenarioIndex (Tier -2A)
    let scenario_index = {
        let yaml_paths = [
            concat!(env!("CARGO_MANIFEST_DIR"), "/config/scenario_index.yaml"),
            "config/scenario_index.yaml",
        ];
        let mut loaded = None;
        for path in &yaml_paths {
            let p = Path::new(path);
            if p.is_file() {
                match ScenarioIndex::from_yaml_file(p) {
                    Ok(si) => {
                        println!("  ScenarioIndex: loaded from {}", path);
                        loaded = Some(Arc::new(si));
                        break;
                    }
                    Err(e) => {
                        eprintln!("  ScenarioIndex: failed to parse {}: {}", path, e);
                    }
                }
            }
        }
        if loaded.is_none() {
            println!("  ScenarioIndex: not found (Tier -2A disabled in test)");
        }
        loaded
    };

    // Construct searcher with production-equivalent settings
    let mut searcher =
        HybridVerbSearcher::new(verb_service, Some(learned_data)).with_embedder(dyn_embedder);

    if let Some(ni) = noun_index {
        searcher = searcher.with_noun_index(ni);
    }
    if let Some(mi) = macro_index {
        searcher = searcher.with_macro_index(mi);
    }
    if let Some(si) = scenario_index {
        searcher = searcher.with_scenario_index(si);
    }

    if let Some(t) = threshold_override {
        println!("  Overriding semantic_threshold to {:.2}", t);
        searcher = searcher.with_semantic_threshold(t);
    }
    if let Some(f) = fallback_override {
        println!("  Overriding fallback_threshold to {:.2}", f);
        searcher = searcher.with_fallback_threshold(f);
    }

    searcher
}
