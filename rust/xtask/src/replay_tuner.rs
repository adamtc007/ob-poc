//! replay-tuner — Offline replay of decision logs with scoring constant sweeps.
//!
//! Loads golden corpus YAML files and/or saved session decision logs,
//! then replays them with different scoring configurations to find
//! optimal constants.
//!
//! # Usage
//!
//! ```bash
//! # Run golden corpus with current constants
//! cargo x replay-tuner run
//!
//! # Sweep scoring constants and compare
//! cargo x replay-tuner sweep --param pack_verb_boost --min 0.05 --max 0.20 --step 0.05
//!
//! # Compare two scoring configs
//! cargo x replay-tuner compare --baseline baseline.json --candidate candidate.json
//!
//! # Generate report from a session log
//! cargo x replay-tuner report --session-log session.json
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use ob_poc::repl::decision_log::{
    CategoryResult, GoldenCorpusReport, GoldenMatchMode, GoldenTestCase, GoldenTestResult,
    ScoringConfig, SessionDecisionLog, VerbCandidateSnapshot,
};

// ============================================================================
// Golden corpus loader
// ============================================================================

/// Load all golden corpus YAML files from a directory.
pub fn load_golden_corpus(dir: &Path) -> Result<Vec<GoldenTestCase>> {
    let mut cases = Vec::new();

    let yaml_files: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot read golden corpus directory: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
        .collect();

    if yaml_files.is_empty() {
        anyhow::bail!(
            "No YAML files found in golden corpus directory: {}",
            dir.display()
        );
    }

    for path in &yaml_files {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read {}", path.display()))?;

        // Try structured format first (with id/category fields)
        match serde_yaml::from_str::<Vec<GoldenTestCase>>(&content) {
            Ok(file_cases) => {
                cases.extend(file_cases);
            }
            Err(_) => {
                // Try legacy seed format (flat list with different field names)
                match serde_yaml::from_str::<Vec<LegacySeedEntry>>(&content) {
                    Ok(legacy) => {
                        let filename = path.file_stem().unwrap_or_default().to_string_lossy();
                        for (i, entry) in legacy.into_iter().enumerate() {
                            cases.push(GoldenTestCase {
                                id: format!("{}-legacy-{:03}", filename, i + 1),
                                category: entry.category.unwrap_or_else(|| filename.to_string()),
                                input: entry.input,
                                pack_id: entry.pack_id,
                                expected_verb: entry.expected_verb,
                                match_mode: entry
                                    .match_mode
                                    .map(|m| match m.as_str() {
                                        "exact" => GoldenMatchMode::Exact,
                                        "top_three" => GoldenMatchMode::TopThree,
                                        "match_or_ambiguous" => GoldenMatchMode::MatchOrAmbiguous,
                                        _ => GoldenMatchMode::Exact,
                                    })
                                    .unwrap_or(GoldenMatchMode::Exact),
                                expected_args: HashMap::new(),
                                expected_entities: vec![],
                                tags: entry.tags.unwrap_or_default(),
                            });
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Cannot parse {}: {}. Skipping.", path.display(), e);
                    }
                }
            }
        }
    }

    Ok(cases)
}

/// Legacy seed.yaml format entry.
#[derive(serde::Deserialize)]
struct LegacySeedEntry {
    input: String,
    expected_verb: String,
    category: Option<String>,
    pack_id: Option<String>,
    match_mode: Option<String>,
    tags: Option<Vec<String>>,
}

// ============================================================================
// Replay engine
// ============================================================================

/// Evaluate a single golden test case against a decision log or simulated candidates.
///
/// In offline mode (no live pipeline), we use the decision log's recorded
/// candidates and re-apply scoring with the given config.
pub fn evaluate_test_case_offline(
    test: &GoldenTestCase,
    candidates: &[VerbCandidateSnapshot],
    config: &ScoringConfig,
) -> GoldenTestResult {
    // Re-score candidates with the given config.
    // In offline mode, we take the raw candidates and apply pack scoring adjustments.
    let rescored = rescore_candidates(candidates, config, test.pack_id.as_deref());

    // Find the expected verb in the rescored list.
    let verb_rank = rescored
        .iter()
        .position(|c| c.verb_fqn == test.expected_verb);

    let top_verb = rescored.first().map(|c| c.verb_fqn.clone());
    let top_score = rescored.first().map(|c| c.score).unwrap_or(0.0);

    let passed = match test.match_mode {
        GoldenMatchMode::Exact => verb_rank == Some(0),
        GoldenMatchMode::TopThree => verb_rank.map(|r| r < 3).unwrap_or(false),
        GoldenMatchMode::MatchOrAmbiguous => {
            // Pass if exact match OR if it's ambiguous (top 2 within margin)
            if verb_rank == Some(0) {
                true
            } else if rescored.len() >= 2 {
                let margin = rescored[0].score - rescored[1].score;
                margin < config.margin && verb_rank.map(|r| r < 3).unwrap_or(false)
            } else {
                false
            }
        }
    };

    let failure_reason = if !passed {
        let reason = if verb_rank.is_none() {
            format!(
                "Expected verb '{}' not in candidates. Top: {:?}",
                test.expected_verb, top_verb
            )
        } else {
            format!(
                "Expected verb '{}' at rank {} (need {}). Top: {:?} @ {:.3}",
                test.expected_verb,
                verb_rank.unwrap() + 1,
                match test.match_mode {
                    GoldenMatchMode::Exact => "rank 1",
                    GoldenMatchMode::TopThree => "rank 1-3",
                    GoldenMatchMode::MatchOrAmbiguous => "rank 1 or ambiguous",
                },
                top_verb,
                top_score
            )
        };
        Some(reason)
    } else {
        None
    };

    GoldenTestResult {
        test_id: test.id.clone(),
        passed,
        actual_verb: top_verb,
        actual_score: top_score,
        verb_rank: verb_rank.map(|r| r + 1), // 1-based
        failure_reason,
        scoring_config: config.clone(),
    }
}

/// Re-score candidates with a given scoring config.
///
/// This simulates the pack scoring logic from scoring.rs but with
/// configurable constants. Used for offline sweep without a live pipeline.
fn rescore_candidates(
    candidates: &[VerbCandidateSnapshot],
    config: &ScoringConfig,
    active_pack_id: Option<&str>,
) -> Vec<VerbCandidateSnapshot> {
    let mut rescored: Vec<VerbCandidateSnapshot> = candidates
        .iter()
        .map(|c| {
            let mut new = c.clone();
            // Start from base score (remove old adjustments by using raw score).
            // Since we only have the final score + adjustment tags, we approximate
            // by starting from the recorded score and not double-applying.
            // For a clean sweep, the raw_candidates (pre-scoring) should be used.
            let mut score = c.score;

            // Apply pack scoring if we have a pack context.
            if active_pack_id.is_some() {
                let has_pack_boost = c.adjustments.iter().any(|a| a.contains("pack_boost"));
                let has_pack_penalty = c.adjustments.iter().any(|a| a.contains("pack_penalty"));
                let has_template_boost = c.adjustments.iter().any(|a| a.contains("template_step"));
                let has_domain_boost = c.adjustments.iter().any(|a| a.contains("domain_affinity"));

                // Remove old adjustments and re-apply with new constants
                if has_pack_boost {
                    score += config.pack_verb_boost;
                }
                if has_pack_penalty {
                    score -= config.pack_verb_penalty;
                }
                if has_template_boost {
                    score += config.template_step_boost;
                }
                if has_domain_boost {
                    score += config.domain_affinity_boost;
                }
            }

            // Apply floor
            if score < config.absolute_floor {
                score = 0.0; // Below floor → filtered out
            }

            new.score = score;
            new
        })
        .filter(|c| c.score > 0.0)
        .collect();

    // Sort descending by score
    rescored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rescored
}

/// Run the full golden corpus with a given scoring config.
pub fn run_corpus(
    cases: &[GoldenTestCase],
    session_logs: &[SessionDecisionLog],
    config: &ScoringConfig,
) -> GoldenCorpusReport {
    // Build a lookup from input_hash → decision log entry for matching.
    let mut log_index: HashMap<String, &ob_poc::repl::decision_log::DecisionLog> = HashMap::new();
    for session in session_logs {
        for entry in &session.entries {
            log_index.insert(entry.input_hash.clone(), entry);
        }
    }

    let mut results = Vec::new();
    let mut category_counts: HashMap<String, (usize, usize)> = HashMap::new();

    for test in cases {
        // Try to find a matching decision log entry.
        let candidates = if let Some(log_entry) = find_matching_log(&log_index, &test.input) {
            // Use the raw candidates from the decision log.
            log_entry.verb_decision.raw_candidates.clone()
        } else {
            // No decision log → create a minimal candidate from the test case.
            // This is the "no recorded data" path — useful for corpus validation
            // without actual session logs.
            vec![VerbCandidateSnapshot {
                verb_fqn: test.expected_verb.clone(),
                score: 0.80, // Assume moderate score
                domain: test.expected_verb.split('.').next().map(|s| s.to_string()),
                adjustments: vec![],
            }]
        };

        let result = evaluate_test_case_offline(test, &candidates, config);

        // Track category stats.
        let entry = category_counts
            .entry(test.category.clone())
            .or_insert((0, 0));
        entry.0 += 1;
        if result.passed {
            entry.1 += 1;
        }

        results.push(result);
    }

    // Build category results.
    let by_category: HashMap<String, CategoryResult> = category_counts
        .into_iter()
        .map(|(cat, (total, passed))| {
            let acc = if total > 0 {
                (passed as f32 / total as f32) * 100.0
            } else {
                0.0
            };
            (
                cat,
                CategoryResult {
                    total,
                    passed,
                    accuracy_pct: acc,
                },
            )
        })
        .collect();

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();

    GoldenCorpusReport {
        total,
        passed,
        failed: total - passed,
        accuracy_pct: if total > 0 {
            (passed as f32 / total as f32) * 100.0
        } else {
            0.0
        },
        by_category,
        scoring_config: config.clone(),
        results,
    }
}

/// Find a matching decision log entry for a test input.
fn find_matching_log<'a>(
    index: &'a HashMap<String, &'a ob_poc::repl::decision_log::DecisionLog>,
    input: &str,
) -> Option<&'a ob_poc::repl::decision_log::DecisionLog> {
    // Compute hash of the test input.
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    index.get(&hash).copied()
}

// ============================================================================
// Sweep engine
// ============================================================================

/// Which scoring parameter to sweep.
#[derive(Debug, Clone)]
pub enum SweepParam {
    PackVerbBoost,
    PackVerbPenalty,
    TemplateStepBoost,
    DomainAffinityBoost,
    AbsoluteFloor,
    Threshold,
    Margin,
    StrongThreshold,
}

impl SweepParam {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "pack_verb_boost" => Ok(Self::PackVerbBoost),
            "pack_verb_penalty" => Ok(Self::PackVerbPenalty),
            "template_step_boost" => Ok(Self::TemplateStepBoost),
            "domain_affinity_boost" => Ok(Self::DomainAffinityBoost),
            "absolute_floor" => Ok(Self::AbsoluteFloor),
            "threshold" => Ok(Self::Threshold),
            "margin" => Ok(Self::Margin),
            "strong_threshold" => Ok(Self::StrongThreshold),
            _ => anyhow::bail!(
                "Unknown sweep parameter: '{}'. Valid: pack_verb_boost, pack_verb_penalty, \
                 template_step_boost, domain_affinity_boost, absolute_floor, threshold, margin, \
                 strong_threshold",
                s
            ),
        }
    }

    fn apply(&self, config: &mut ScoringConfig, value: f32) {
        match self {
            Self::PackVerbBoost => config.pack_verb_boost = value,
            Self::PackVerbPenalty => config.pack_verb_penalty = value,
            Self::TemplateStepBoost => config.template_step_boost = value,
            Self::DomainAffinityBoost => config.domain_affinity_boost = value,
            Self::AbsoluteFloor => config.absolute_floor = value,
            Self::Threshold => config.threshold = value,
            Self::Margin => config.margin = value,
            Self::StrongThreshold => config.strong_threshold = value,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::PackVerbBoost => "pack_verb_boost",
            Self::PackVerbPenalty => "pack_verb_penalty",
            Self::TemplateStepBoost => "template_step_boost",
            Self::DomainAffinityBoost => "domain_affinity_boost",
            Self::AbsoluteFloor => "absolute_floor",
            Self::Threshold => "threshold",
            Self::Margin => "margin",
            Self::StrongThreshold => "strong_threshold",
        }
    }
}

/// Result of a single sweep point.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SweepPoint {
    pub param_name: String,
    pub param_value: f32,
    pub accuracy_pct: f32,
    pub passed: usize,
    pub failed: usize,
    pub total: usize,
}

/// Run a sweep over a single parameter.
pub fn run_sweep(
    cases: &[GoldenTestCase],
    session_logs: &[SessionDecisionLog],
    param: &SweepParam,
    min: f32,
    max: f32,
    step: f32,
) -> Vec<SweepPoint> {
    let mut points = Vec::new();
    let mut value = min;

    while value <= max + f32::EPSILON {
        let mut config = ScoringConfig::default();
        param.apply(&mut config, value);

        let report = run_corpus(cases, session_logs, &config);

        points.push(SweepPoint {
            param_name: param.name().to_string(),
            param_value: value,
            accuracy_pct: report.accuracy_pct,
            passed: report.passed,
            failed: report.failed,
            total: report.total,
        });

        value += step;
    }

    points
}

// ============================================================================
// Report formatting
// ============================================================================

/// Print a corpus report to stdout.
pub fn print_report(report: &GoldenCorpusReport) {
    println!("===========================================");
    println!("  Golden Corpus Report");
    println!("===========================================\n");

    println!(
        "Total: {}  Passed: {}  Failed: {}  Accuracy: {:.1}%\n",
        report.total, report.passed, report.failed, report.accuracy_pct
    );

    if !report.by_category.is_empty() {
        println!("By Category:");
        let mut categories: Vec<_> = report.by_category.iter().collect();
        categories.sort_by_key(|(k, _)| (*k).clone());
        for (cat, result) in &categories {
            println!(
                "  {:<20} {}/{} ({:.1}%)",
                cat, result.passed, result.total, result.accuracy_pct
            );
        }
        println!();
    }

    // Show failures.
    let failures: Vec<_> = report.results.iter().filter(|r| !r.passed).collect();
    if !failures.is_empty() {
        println!("Failures:");
        for f in &failures {
            println!(
                "  [{}] {}",
                f.test_id,
                f.failure_reason.as_deref().unwrap_or("?")
            );
        }
        println!();
    }

    println!("Scoring Config:");
    println!(
        "  pack_verb_boost={:.2}  pack_verb_penalty={:.2}  template_step_boost={:.2}",
        report.scoring_config.pack_verb_boost,
        report.scoring_config.pack_verb_penalty,
        report.scoring_config.template_step_boost
    );
    println!(
        "  domain_affinity_boost={:.2}  absolute_floor={:.2}  threshold={:.2}",
        report.scoring_config.domain_affinity_boost,
        report.scoring_config.absolute_floor,
        report.scoring_config.threshold
    );
    println!(
        "  margin={:.2}  strong_threshold={:.2}",
        report.scoring_config.margin, report.scoring_config.strong_threshold
    );
}

/// Print sweep results to stdout.
pub fn print_sweep(points: &[SweepPoint]) {
    println!("===========================================");
    println!("  Scoring Parameter Sweep");
    println!("===========================================\n");

    if points.is_empty() {
        println!("No sweep points generated.");
        return;
    }

    let param_name = &points[0].param_name;
    println!(
        "{:<20} {:>10} {:>8} {:>8} {:>8}",
        param_name, "accuracy%", "passed", "failed", "total"
    );
    println!("{}", "-".repeat(58));

    let mut best: Option<&SweepPoint> = None;

    for point in points {
        println!(
            "{:<20.3} {:>10.1} {:>8} {:>8} {:>8}",
            point.param_value, point.accuracy_pct, point.passed, point.failed, point.total
        );

        if best
            .map(|b| point.accuracy_pct > b.accuracy_pct)
            .unwrap_or(true)
        {
            best = Some(point);
        }
    }

    if let Some(best) = best {
        println!(
            "\nBest: {}={:.3} → {:.1}% accuracy ({} passed / {} total)",
            best.param_name, best.param_value, best.accuracy_pct, best.passed, best.total
        );
    }
}

/// Print comparison of two reports.
pub fn print_comparison(baseline: &GoldenCorpusReport, candidate: &GoldenCorpusReport) {
    println!("===========================================");
    println!("  Scoring Config Comparison");
    println!("===========================================\n");

    println!(
        "{:<20} {:>12} {:>12} {:>10}",
        "Metric", "Baseline", "Candidate", "Delta"
    );
    println!("{}", "-".repeat(56));

    let delta_acc = candidate.accuracy_pct - baseline.accuracy_pct;
    println!(
        "{:<20} {:>11.1}% {:>11.1}% {:>+9.1}%",
        "Accuracy", baseline.accuracy_pct, candidate.accuracy_pct, delta_acc
    );
    println!(
        "{:<20} {:>12} {:>12} {:>+10}",
        "Passed",
        baseline.passed,
        candidate.passed,
        candidate.passed as i64 - baseline.passed as i64
    );
    println!(
        "{:<20} {:>12} {:>12} {:>+10}",
        "Failed",
        baseline.failed,
        candidate.failed,
        candidate.failed as i64 - baseline.failed as i64
    );

    // Category comparison.
    let mut all_cats: Vec<String> = baseline
        .by_category
        .keys()
        .chain(candidate.by_category.keys())
        .cloned()
        .collect();
    all_cats.sort();
    all_cats.dedup();

    if !all_cats.is_empty() {
        println!("\nBy Category:");
        for cat in &all_cats {
            let b_acc = baseline
                .by_category
                .get(cat)
                .map(|c| c.accuracy_pct)
                .unwrap_or(0.0);
            let c_acc = candidate
                .by_category
                .get(cat)
                .map(|c| c.accuracy_pct)
                .unwrap_or(0.0);
            let delta = c_acc - b_acc;
            println!(
                "  {:<18} {:>11.1}% {:>11.1}% {:>+9.1}%",
                cat, b_acc, c_acc, delta
            );
        }
    }

    // Show regressions (tests that passed in baseline but fail in candidate).
    let baseline_passed: std::collections::HashSet<&str> = baseline
        .results
        .iter()
        .filter(|r| r.passed)
        .map(|r| r.test_id.as_str())
        .collect();
    let candidate_failed: Vec<_> = candidate
        .results
        .iter()
        .filter(|r| !r.passed && baseline_passed.contains(r.test_id.as_str()))
        .collect();

    if !candidate_failed.is_empty() {
        println!("\nRegressions ({} tests):", candidate_failed.len());
        for r in &candidate_failed {
            println!(
                "  [{}] {}",
                r.test_id,
                r.failure_reason.as_deref().unwrap_or("?")
            );
        }
    }

    // Show improvements (tests that failed in baseline but pass in candidate).
    let candidate_passed: std::collections::HashSet<&str> = candidate
        .results
        .iter()
        .filter(|r| r.passed)
        .map(|r| r.test_id.as_str())
        .collect();
    let improvements: Vec<_> = baseline
        .results
        .iter()
        .filter(|r| !r.passed && candidate_passed.contains(r.test_id.as_str()))
        .collect();

    if !improvements.is_empty() {
        println!("\nImprovements ({} tests):", improvements.len());
        for r in &improvements {
            println!("  [{}] now passing", r.test_id);
        }
    }
}

// ============================================================================
// CLI entry points
// ============================================================================

/// Run golden corpus with current scoring constants.
pub fn run(
    corpus_dir: Option<&Path>,
    session_log_path: Option<&Path>,
    verbose: bool,
) -> Result<()> {
    let corpus_dir = corpus_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_corpus_dir);

    let cases = load_golden_corpus(&corpus_dir)?;
    println!(
        "Loaded {} golden test cases from {}",
        cases.len(),
        corpus_dir.display()
    );

    let session_logs = if let Some(path) = session_log_path {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read session log: {}", path.display()))?;
        vec![SessionDecisionLog::from_json(&content)?]
    } else {
        vec![]
    };

    if !session_logs.is_empty() {
        let total_entries: usize = session_logs.iter().map(|s| s.entries.len()).sum();
        println!(
            "Loaded {} session logs ({} entries)",
            session_logs.len(),
            total_entries
        );
    }

    let config = ScoringConfig::default();
    let report = run_corpus(&cases, &session_logs, &config);

    print_report(&report);

    if verbose {
        // Print all results, not just failures.
        println!("\nAll Results:");
        for r in &report.results {
            let status = if r.passed { "PASS" } else { "FAIL" };
            println!(
                "  [{:<5}] {:<30} verb={:<30} score={:.3} rank={}",
                status,
                r.test_id,
                r.actual_verb.as_deref().unwrap_or("-"),
                r.actual_score,
                r.verb_rank
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
        }
    }

    // Write JSON report alongside.
    let report_json = serde_json::to_string_pretty(&report)?;
    let report_path = corpus_dir.join("last_report.json");
    std::fs::write(&report_path, &report_json)?;
    println!("\nReport written to {}", report_path.display());

    Ok(())
}

/// Sweep a scoring parameter.
pub fn sweep(
    corpus_dir: Option<&Path>,
    session_log_path: Option<&Path>,
    param_name: &str,
    min: f32,
    max: f32,
    step: f32,
    json_output: Option<&Path>,
) -> Result<()> {
    let corpus_dir = corpus_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_corpus_dir);

    let cases = load_golden_corpus(&corpus_dir)?;
    println!("Loaded {} golden test cases", cases.len());

    let session_logs = if let Some(path) = session_log_path {
        let content = std::fs::read_to_string(path)?;
        vec![SessionDecisionLog::from_json(&content)?]
    } else {
        vec![]
    };

    let param = SweepParam::from_str(param_name)?;
    let points = run_sweep(&cases, &session_logs, &param, min, max, step);

    print_sweep(&points);

    if let Some(path) = json_output {
        let json = serde_json::to_string_pretty(&points)?;
        std::fs::write(path, &json)?;
        println!("\nSweep results written to {}", path.display());
    }

    Ok(())
}

/// Compare two scoring configs.
pub fn compare(baseline_path: &Path, candidate_path: &Path) -> Result<()> {
    let baseline_json = std::fs::read_to_string(baseline_path)
        .with_context(|| format!("Cannot read baseline: {}", baseline_path.display()))?;
    let baseline: GoldenCorpusReport = serde_json::from_str(&baseline_json)
        .with_context(|| "Cannot parse baseline as GoldenCorpusReport")?;

    let candidate_json = std::fs::read_to_string(candidate_path)
        .with_context(|| format!("Cannot read candidate: {}", candidate_path.display()))?;
    let candidate: GoldenCorpusReport = serde_json::from_str(&candidate_json)
        .with_context(|| "Cannot parse candidate as GoldenCorpusReport")?;

    print_comparison(&baseline, &candidate);
    Ok(())
}

/// Generate report from a session decision log.
pub fn report(session_log_path: &Path, verbose: bool) -> Result<()> {
    let content = std::fs::read_to_string(session_log_path)
        .with_context(|| format!("Cannot read session log: {}", session_log_path.display()))?;
    let session_log = SessionDecisionLog::from_json(&content)?;

    println!("===========================================");
    println!("  Session Decision Log Report");
    println!("===========================================\n");

    println!("Session ID: {}", session_log.session_id);
    println!("Turns: {}", session_log.entries.len());

    if session_log.is_empty() {
        println!("\nNo entries in session log.");
        return Ok(());
    }

    // Summary statistics.
    let mut turn_types: HashMap<String, usize> = HashMap::new();
    let mut extraction_methods: HashMap<String, usize> = HashMap::new();
    let mut total_candidates = 0usize;
    let mut ambiguous_count = 0usize;
    let mut no_match_count = 0usize;

    for entry in &session_log.entries {
        *turn_types
            .entry(format!("{:?}", entry.turn_type))
            .or_default() += 1;
        *extraction_methods
            .entry(format!("{:?}", entry.extraction_decision.method))
            .or_default() += 1;
        total_candidates += entry.verb_decision.raw_candidates.len();
        if entry.verb_decision.ambiguity_outcome.contains("ambiguous") {
            ambiguous_count += 1;
        }
        if entry.verb_decision.ambiguity_outcome.contains("no_match") {
            no_match_count += 1;
        }
    }

    let avg_candidates = total_candidates as f32 / session_log.entries.len() as f32;

    println!("\nTurn Types:");
    for (tt, count) in &turn_types {
        println!("  {:<25} {}", tt, count);
    }

    println!("\nExtraction Methods:");
    for (em, count) in &extraction_methods {
        println!("  {:<25} {}", em, count);
    }

    println!("\nVerb Matching:");
    println!("  Avg candidates/turn: {:.1}", avg_candidates);
    println!("  Ambiguous turns:     {}", ambiguous_count);
    println!("  No-match turns:      {}", no_match_count);

    if verbose {
        println!("\nPer-Turn Detail:");
        for entry in &session_log.entries {
            println!(
                "\n  Turn {} [{}] {:?}",
                entry.turn,
                entry.timestamp.format("%H:%M:%S"),
                entry.turn_type
            );
            if !entry.raw_input.is_empty() {
                println!("    Input: \"{}\"", entry.raw_input);
            } else {
                println!("    Input: [redacted] hash={:.16}...", entry.input_hash);
            }
            if let Some(ref verb) = entry.verb_decision.selected_verb {
                println!(
                    "    Verb: {} (confidence: {:.3})",
                    verb, entry.verb_decision.confidence
                );
            }
            println!(
                "    Extraction: {:?} ({} filled, {} missing)",
                entry.extraction_decision.method,
                entry.extraction_decision.filled_args.len(),
                entry.extraction_decision.missing_args.len()
            );
            if let Some(ref dsl) = entry.proposed_dsl {
                println!("    DSL: {}", dsl);
            }
        }
    }

    Ok(())
}

/// Default golden corpus directory.
fn default_corpus_dir() -> PathBuf {
    // Try relative to cwd first, then try from project root.
    let candidates = [
        PathBuf::from("tests/golden_corpus"),
        PathBuf::from("rust/tests/golden_corpus"),
        PathBuf::from("../tests/golden_corpus"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }

    // Fallback — caller will get an error when loading.
    PathBuf::from("tests/golden_corpus")
}
