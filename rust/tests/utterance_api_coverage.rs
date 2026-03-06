//! Utterance coverage harness over the live server API.
//!
//! This test calls:
//! - `POST /api/session`
//! - `POST /api/session/:id/execute` with `(registry.discover-dsl ...)`
//!
//! It validates utterance -> top intent verb mapping and writes coverage artifacts:
//! - JSON report
//! - Markdown summary
//! - Expected-verb -> predicted-verb cross-reference CSV
//!
//! Usage:
//!   cargo test --test utterance_api_coverage -- --ignored --nocapture
//!
//! Environment:
//!   UTTERANCE_API_BASE_URL      Base URL for ob-poc-web (default: http://127.0.0.1:3002)
//!   UTTERANCE_FIXTURE_PATH      TOML fixture path (default: ../docs/todo/intent_test_utterances.toml,
//!                               fallback: tests/fixtures/intent_test_utterances.toml)
//!   UTTERANCE_OUTPUT_DIR        Output directory (default: target/utterance-api-coverage)
//!   UTTERANCE_ACTOR_ID          Header x-obpoc-actor-id (default: coverage-bot)
//!   UTTERANCE_ROLES             Header x-obpoc-roles (default: admin)
//!   UTTERANCE_MAX_CHAIN_LENGTH  `max-chain-length` arg (default: 5)
//!   UTTERANCE_FILTER            Optional filter: easy|medium|hard|expert|direct|natural|indirect|adversarial|multi_intent
//!   UTTERANCE_MIN_ACCURACY      Optional required minimum top1 accuracy in [0.0,1.0]

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ob_poc::sage::{
    CoderEngine, DeterministicSage, LlmSage, ObservationPlane, SageContext, SageEngine,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize, Clone)]
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
    expected_plane: Option<String>,
    #[serde(default)]
    alt_verbs: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
struct RowResult {
    idx: usize,
    utterance: String,
    expected_verb: String,
    alt_verbs: Vec<String>,
    predicted_top_verb: Option<String>,
    predicted_top3: Vec<String>,
    pass: bool,
    sage_verb: Option<String>,
    sage_dsl: Option<String>,
    sage_match: bool,
    category: String,
    difficulty: String,
    intent_count: usize,
    execute_success: bool,
}

#[derive(Debug, Serialize, Default, Clone)]
struct BucketStats {
    total: usize,
    passed: usize,
    accuracy: f64,
}

#[derive(Debug, Serialize)]
struct Summary {
    total: usize,
    passed_top1_or_alt: usize,
    sage_passed_top1_or_alt: usize,
    failed: usize,
    sage_failed: usize,
    accuracy: f64,
    sage_accuracy: f64,
}

#[derive(Debug, Serialize)]
struct CoverageReport {
    summary: Summary,
    by_category: BTreeMap<String, BucketStats>,
    by_difficulty: BTreeMap<String, BucketStats>,
    by_entity_prefix: BTreeMap<String, BucketStats>,
    expected_verb_distribution: BTreeMap<String, usize>,
    predicted_top_distribution: BTreeMap<String, usize>,
    mismatches: Vec<RowResult>,
    rows: Vec<RowResult>,
}

fn update_bucket(map: &mut HashMap<String, (usize, usize)>, key: &str, pass: bool) {
    let (total, passed) = map.entry(key.to_owned()).or_insert((0, 0));
    *total += 1;
    if pass {
        *passed += 1;
    }
}

fn finalize_bucket(map: HashMap<String, (usize, usize)>) -> BTreeMap<String, BucketStats> {
    map.into_iter()
        .map(|(k, (total, passed))| {
            let accuracy = if total == 0 {
                0.0
            } else {
                passed as f64 / total as f64
            };
            (
                k,
                BucketStats {
                    total,
                    passed,
                    accuracy,
                },
            )
        })
        .collect()
}

fn default_fixture_path() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let docs_path = root.join("../docs/todo/intent_test_utterances.toml");
    if docs_path.exists() {
        docs_path
    } else {
        root.join("tests/fixtures/intent_test_utterances.toml")
    }
}

fn fixture_path() -> PathBuf {
    std::env::var("UTTERANCE_FIXTURE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_fixture_path())
}

fn output_dir() -> PathBuf {
    std::env::var("UTTERANCE_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/utterance-api-coverage")
        })
}

fn load_fixture(path: &Path) -> anyhow::Result<TestFixture> {
    let raw = fs::read_to_string(path)?;
    let fixture: TestFixture = toml::from_str(&raw)?;
    Ok(fixture)
}

fn should_include(case: &TestCase, filter: &str) -> bool {
    match filter {
        "easy" => case.difficulty == "easy",
        "medium" => case.difficulty == "medium",
        "hard" => case.difficulty == "hard",
        "expert" => case.difficulty == "expert",
        "direct" => case.category == "direct",
        "natural" => case.category == "natural",
        "indirect" => case.category == "indirect",
        "adversarial" => case.category == "adversarial",
        "multi_intent" => case.category == "multi_intent",
        "" => true,
        _ => true,
    }
}

fn escape_dsl_string(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_top_intents(value: &serde_json::Value) -> (Option<String>, Vec<String>, usize) {
    let intents = value
        .get("intent_matches")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut top3 = Vec::new();
    for item in intents.iter().take(3) {
        if let Some(verb) = item.get("verb").and_then(serde_json::Value::as_str) {
            top3.push(verb.to_owned());
        }
    }
    let top = top3.first().cloned();
    (top, top3, intents.len())
}

fn build_sage_engine() -> Arc<dyn SageEngine> {
    if std::env::var("SAGE_LLM").ok().as_deref() == Some("1") {
        if let Ok(client) = ob_agentic::client_factory::create_llm_client() {
            return Arc::new(LlmSage::new(client));
        }
    }

    Arc::new(DeterministicSage)
}

fn parse_plane(value: Option<&str>) -> Option<ObservationPlane> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "instance" => Some(ObservationPlane::Instance),
        "structure" => Some(ObservationPlane::Structure),
        "registry" => Some(ObservationPlane::Registry),
        _ => None,
    }
}

fn sage_context_for_case(case: &TestCase) -> SageContext {
    let stage_focus = match parse_plane(case.expected_plane.as_deref()) {
        Some(ObservationPlane::Structure) => Some("semos-data-management".to_string()),
        Some(ObservationPlane::Registry) => Some("semos-stewardship".to_string()),
        _ => None,
    };

    SageContext {
        session_id: None,
        stage_focus,
        goals: Vec::new(),
        entity_kind: None,
        dominant_entity_name: None,
        last_intents: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires a running ob-poc-web server
    async fn utterance_api_coverage() -> anyhow::Result<()> {
        let base_url = std::env::var("UTTERANCE_API_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3002".to_owned());
        let actor_id =
            std::env::var("UTTERANCE_ACTOR_ID").unwrap_or_else(|_| "coverage-bot".to_owned());
        let roles = std::env::var("UTTERANCE_ROLES").unwrap_or_else(|_| "admin".to_owned());
        let max_chain_length = std::env::var("UTTERANCE_MAX_CHAIN_LENGTH")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(5);
        let filter = std::env::var("UTTERANCE_FILTER").unwrap_or_default();
        let min_accuracy = std::env::var("UTTERANCE_MIN_ACCURACY")
            .ok()
            .and_then(|v| v.parse::<f64>().ok());

        let fixture_path = fixture_path();
        let out_dir = output_dir();
        fs::create_dir_all(&out_dir)?;

        let fixture = load_fixture(&fixture_path)?;
        let tests: Vec<TestCase> = fixture
            .tests
            .into_iter()
            .filter(|case| should_include(case, filter.as_str()))
            .collect();

        if tests.is_empty() {
            anyhow::bail!("No test cases after applying filter '{filter}'");
        }

        let client = reqwest::Client::new();
        let sage_engine = build_sage_engine();
        let coder_engine = CoderEngine::load()?;
        let mut rows = Vec::with_capacity(tests.len());
        let mut by_category: HashMap<String, (usize, usize)> = HashMap::new();
        let mut by_difficulty: HashMap<String, (usize, usize)> = HashMap::new();
        let mut by_entity: HashMap<String, (usize, usize)> = HashMap::new();
        let mut expected_dist: BTreeMap<String, usize> = BTreeMap::new();
        let mut predicted_dist: BTreeMap<String, usize> = BTreeMap::new();
        let mut sage_predicted_dist: BTreeMap<String, usize> = BTreeMap::new();
        let mut xref: BTreeMap<(String, String), usize> = BTreeMap::new();
        let mut sage_xref: BTreeMap<(String, String), usize> = BTreeMap::new();

        println!();
        println!("=======================================================================");
        println!(
            "  UTTERANCE API COVERAGE -- {} utterances (filter='{}')",
            tests.len(),
            if filter.is_empty() { "all" } else { &filter }
        );
        println!("  base_url={base_url}");
        println!("  fixture={}", fixture_path.display());
        println!("  output={}", out_dir.display());
        println!("=======================================================================");
        println!();

        for (idx, case) in tests.iter().enumerate() {
            let session_resp = client
                .post(format!("{base_url}/api/session"))
                .header("content-type", "application/json")
                .header("x-obpoc-actor-id", &actor_id)
                .header("x-obpoc-roles", &roles)
                .json(&json!({ "name": format!("cov-{}", idx + 1) }))
                .send()
                .await?;
            let session_resp = session_resp.error_for_status()?;
            let session_json: serde_json::Value = session_resp.json().await?;
            let session_id = session_json
                .get("session_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing session_id in session response"))?;

            let dsl = format!(
                "(registry.discover-dsl :utterance \"{}\" :max-chain-length {})",
                escape_dsl_string(&case.utterance),
                max_chain_length
            );

            let exec_resp = client
                .post(format!("{base_url}/api/session/{session_id}/execute"))
                .header("content-type", "application/json")
                .header("x-obpoc-actor-id", &actor_id)
                .header("x-obpoc-roles", &roles)
                .json(&json!({ "dsl": dsl }))
                .send()
                .await?;
            let exec_resp = exec_resp.error_for_status()?;
            let exec_json: serde_json::Value = exec_resp.json().await?;

            let mut execute_success = false;
            let mut payload = serde_json::Value::Null;
            if exec_json
                .get("success")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                if let Some(statement) = exec_json
                    .get("results")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|arr| arr.first())
                {
                    if statement
                        .get("success")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                    {
                        execute_success = true;
                        payload = statement
                            .get("result")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                    }
                }
            }

            let (predicted_top_verb, predicted_top3, intent_count) = parse_top_intents(&payload);
            let pass = predicted_top_verb
                .as_ref()
                .map(|v| v == &case.expected_verb || case.alt_verbs.iter().any(|alt| alt == v))
                .unwrap_or(false);

            let sage_context = sage_context_for_case(case);
            let sage_outcome = sage_engine
                .classify(&case.utterance, &sage_context)
                .await
                .ok();
            let sage_coder = sage_outcome
                .as_ref()
                .and_then(|outcome| coder_engine.resolve(outcome).ok());
            let sage_verb = sage_coder.as_ref().map(|r| r.verb_fqn.clone());
            let sage_dsl = sage_coder.as_ref().map(|r| r.dsl.clone());
            let sage_match = sage_verb
                .as_ref()
                .map(|v| v == &case.expected_verb || case.alt_verbs.iter().any(|alt| alt == v))
                .unwrap_or(false);

            let expected_prefix = case
                .expected_verb
                .split('.')
                .next()
                .unwrap_or(case.expected_verb.as_str())
                .to_owned();

            update_bucket(&mut by_category, case.category.as_str(), pass);
            update_bucket(&mut by_difficulty, case.difficulty.as_str(), pass);
            update_bucket(&mut by_entity, expected_prefix.as_str(), pass);

            *expected_dist.entry(case.expected_verb.clone()).or_insert(0) += 1;
            if let Some(pred) = predicted_top_verb.as_ref() {
                *predicted_dist.entry(pred.clone()).or_insert(0) += 1;
            }
            if let Some(pred) = sage_verb.as_ref() {
                *sage_predicted_dist.entry(pred.clone()).or_insert(0) += 1;
            }
            *xref
                .entry((
                    case.expected_verb.clone(),
                    predicted_top_verb
                        .clone()
                        .unwrap_or_else(|| "<none>".to_owned()),
                ))
                .or_insert(0) += 1;
            *sage_xref
                .entry((
                    case.expected_verb.clone(),
                    sage_verb.clone().unwrap_or_else(|| "<none>".to_owned()),
                ))
                .or_insert(0) += 1;

            let row = RowResult {
                idx: idx + 1,
                utterance: case.utterance.clone(),
                expected_verb: case.expected_verb.clone(),
                alt_verbs: case.alt_verbs.clone(),
                predicted_top_verb,
                predicted_top3,
                pass,
                sage_verb,
                sage_dsl,
                sage_match,
                category: case.category.clone(),
                difficulty: case.difficulty.clone(),
                intent_count,
                execute_success,
            };
            rows.push(row);
        }

        let passed = rows.iter().filter(|r| r.pass).count();
        let sage_passed = rows.iter().filter(|r| r.sage_match).count();
        let total = rows.len();
        let accuracy = if total == 0 {
            0.0
        } else {
            passed as f64 / total as f64
        };
        let sage_accuracy = if total == 0 {
            0.0
        } else {
            sage_passed as f64 / total as f64
        };

        let mut mismatches: Vec<RowResult> = rows.iter().filter(|r| !r.pass).cloned().collect();
        mismatches.sort_by_key(|r| (r.category.clone(), r.difficulty.clone(), r.idx));

        let report = CoverageReport {
            summary: Summary {
                total,
                passed_top1_or_alt: passed,
                sage_passed_top1_or_alt: sage_passed,
                failed: total.saturating_sub(passed),
                sage_failed: total.saturating_sub(sage_passed),
                accuracy,
                sage_accuracy,
            },
            by_category: finalize_bucket(by_category),
            by_difficulty: finalize_bucket(by_difficulty),
            by_entity_prefix: finalize_bucket(by_entity),
            expected_verb_distribution: expected_dist,
            predicted_top_distribution: predicted_dist,
            mismatches: mismatches.clone(),
            rows: rows.clone(),
        };

        let json_path = out_dir.join("utterance_coverage_results.json");
        let md_path = out_dir.join("utterance_coverage_report.md");
        let csv_path = out_dir.join("utterance_verb_xref.csv");

        fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;

        let mut md = String::new();
        md.push_str("# Utterance Coverage Report (API Execute Path)\n\n");
        md.push_str(&format!("- Total utterances: {total}\n"));
        md.push_str(&format!(
            "- Pass (top1 == expected or in alt_verbs): {passed}\n"
        ));
        md.push_str(&format!(
            "- Sage+Coder pass (top1 == expected or in alt_verbs): {sage_passed}\n"
        ));
        md.push_str(&format!("- Fail: {}\n", total.saturating_sub(passed)));
        md.push_str(&format!(
            "- Sage+Coder fail: {}\n",
            total.saturating_sub(sage_passed)
        ));
        md.push_str(&format!("- Accuracy: {:.2}%\n\n", accuracy * 100.0));
        md.push_str(&format!(
            "- Sage+Coder accuracy: {:.2}%\n\n",
            sage_accuracy * 100.0
        ));

        md.push_str("## Accuracy by category\n");
        for (k, v) in &report.by_category {
            md.push_str(&format!(
                "- {k}: {}/{} ({:.2}%)\n",
                v.passed,
                v.total,
                v.accuracy * 100.0
            ));
        }
        md.push('\n');

        md.push_str("## Accuracy by difficulty\n");
        for (k, v) in &report.by_difficulty {
            md.push_str(&format!(
                "- {k}: {}/{} ({:.2}%)\n",
                v.passed,
                v.total,
                v.accuracy * 100.0
            ));
        }
        md.push('\n');

        md.push_str("## Accuracy by entity prefix (from expected verb)\n");
        for (k, v) in &report.by_entity_prefix {
            md.push_str(&format!(
                "- {k}: {}/{} ({:.2}%)\n",
                v.passed,
                v.total,
                v.accuracy * 100.0
            ));
        }
        md.push('\n');

        md.push_str("## First 40 mismatches\n");
        for row in mismatches.iter().take(40) {
            md.push_str(&format!(
                "- #{}: expected `{}` got API=`{}` Sage=`{}` | {}\n",
                row.idx,
                row.expected_verb,
                row.predicted_top_verb.as_deref().unwrap_or("<none>"),
                row.sage_verb.as_deref().unwrap_or("<none>"),
                row.utterance
            ));
        }
        fs::write(&md_path, md)?;

        let mut csv = String::from("expected_verb,predicted_top_verb,count\n");
        for ((expected, predicted), count) in xref {
            csv.push_str(&format!("{expected},{predicted},{count}\n"));
        }
        csv.push_str("\nexpected_verb,sage_verb,count\n");
        for ((expected, predicted), count) in sage_xref {
            csv.push_str(&format!("{expected},{predicted},{count}\n"));
        }
        fs::write(&csv_path, csv)?;

        println!("Report JSON: {}", json_path.display());
        println!("Report MD:   {}", md_path.display());
        println!("Verb XRef:   {}", csv_path.display());
        println!("Coverage: {:.2}% ({}/{})", accuracy * 100.0, passed, total);
        println!(
            "Sage+Coder Coverage: {:.2}% ({}/{})",
            sage_accuracy * 100.0,
            sage_passed,
            total
        );

        if let Some(min) = min_accuracy {
            anyhow::ensure!(
                accuracy >= min,
                "Accuracy {:.4} is below UTTERANCE_MIN_ACCURACY {:.4}",
                accuracy,
                min
            );
        }

        Ok(())
    }
}
