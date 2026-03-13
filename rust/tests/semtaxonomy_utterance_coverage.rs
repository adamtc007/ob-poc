//! Utterance coverage harness over the SemTaxonomy session input path.
//!
//! This test calls:
//! - `POST /api/session`
//! - `POST /api/session/:id/input` with `{"kind":"utterance","message":...}`
//!
//! It validates utterance -> top proposed verb mapping from the SemTaxonomy
//! response payload and writes coverage artifacts:
//! - JSON report
//! - Markdown summary
//!
//! Usage:
//!   cargo test --test semtaxonomy_utterance_coverage -- --ignored --nocapture

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

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
    alt_verbs: Vec<String>,
    #[serde(default)]
    bootstrap: Vec<String>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ContractClass {
    FreshEntity,
    FreshGeneric,
    ScopedDeictic,
}

impl ContractClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::FreshEntity => "fresh_entity",
            Self::FreshGeneric => "fresh_generic",
            Self::ScopedDeictic => "scoped_deictic",
        }
    }
}

#[derive(Debug, Serialize, Clone)]
struct RowResult {
    idx: usize,
    utterance: String,
    expected_verb: String,
    alt_verbs: Vec<String>,
    predicted_verb: Option<String>,
    predicted_dsl: Option<String>,
    requires_confirmation: bool,
    ready_to_execute: bool,
    has_sage_explain: bool,
    step1_resolved: bool,
    step1_ambiguous: bool,
    step1_unresolved: bool,
    step2_stateful: bool,
    step3_proposed: bool,
    grounded: bool,
    business_verb: bool,
    stateful_response: bool,
    pass: bool,
    category: String,
    difficulty: String,
    contract_class: String,
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
    passed: usize,
    failed: usize,
    accuracy: f64,
    grounded: usize,
    grounded_accuracy: f64,
    step1_resolved: usize,
    step1_ambiguous: usize,
    step1_unresolved: usize,
    step2_stateful: usize,
    step3_proposed: usize,
    business_proposals: usize,
    stateful_responses: usize,
}

#[derive(Debug, Serialize)]
struct CoverageReport {
    summary: Summary,
    by_category: BTreeMap<String, BucketStats>,
    by_difficulty: BTreeMap<String, BucketStats>,
    by_contract: BTreeMap<String, BucketStats>,
    by_domain_prefix: BTreeMap<String, BucketStats>,
    mismatches: Vec<RowResult>,
    rows: Vec<RowResult>,
}

fn default_fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/intent_test_utterances.toml")
}

fn fixture_path() -> PathBuf {
    std::env::var("SEMTAXONOMY_FIXTURE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_fixture_path())
}

fn output_dir() -> PathBuf {
    std::env::var("SEMTAXONOMY_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/semtaxonomy-utterance-coverage")
        })
}

fn load_fixture(path: &Path) -> anyhow::Result<TestFixture> {
    let raw = fs::read_to_string(path)?;
    Ok(toml::from_str(&raw)?)
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

fn contract_class_for_utterance(utterance: &str) -> ContractClass {
    let lower = utterance.to_ascii_lowercase();
    if [
        " this ",
        " that ",
        " these ",
        " those ",
        " current ",
        " it ",
        " its ",
        " their ",
        " them ",
        " this?",
        " this cbu",
        " this entity",
        " this deal",
        " this case",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return ContractClass::ScopedDeictic;
    }
    if [
        " for ",
        " between ",
        " named ",
        " called ",
        " on ",
        " regarding ",
        " about ",
        " who owns ",
        " who controls ",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return ContractClass::FreshEntity;
    }
    if utterance.chars().any(|ch| ch.is_uppercase()) && utterance.split_whitespace().count() <= 6 {
        return ContractClass::FreshEntity;
    }
    ContractClass::FreshGeneric
}

fn bootstrap_turns(case: &TestCase, contract_class: ContractClass) -> Vec<String> {
    if !case.bootstrap.is_empty() {
        return case.bootstrap.clone();
    }

    if contract_class != ContractClass::ScopedDeictic {
        return Vec::new();
    }

    let domain = case.expected_verb.split('.').next().unwrap_or_default();
    match domain {
        "deal" => vec![
            "Allianz Global Investors".to_string(),
            "show me the deals for Allianz Global Investors".to_string(),
        ],
        "cbu" | "document" | "screening" | "ubo" | "entity" | "kyc" | "case" => {
            vec!["Allianz Global Investors".to_string()]
        }
        _ => vec!["Allianz Global Investors".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn semtaxonomy_utterance_coverage() -> anyhow::Result<()> {
        let base_url = std::env::var("SEMTAXONOMY_API_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
        let actor_id =
            std::env::var("UTTERANCE_ACTOR_ID").unwrap_or_else(|_| "coverage-bot".to_owned());
        let roles = std::env::var("UTTERANCE_ROLES").unwrap_or_else(|_| "admin".to_owned());
        let filter = std::env::var("UTTERANCE_FILTER").unwrap_or_default();
        let min_accuracy = std::env::var("SEMTAXONOMY_MIN_ACCURACY")
            .ok()
            .and_then(|v| v.parse::<f64>().ok());

        let fixture = load_fixture(&fixture_path())?;
        let tests: Vec<TestCase> = fixture
            .tests
            .into_iter()
            .filter(|case| should_include(case, &filter))
            .collect();
        if tests.is_empty() {
            anyhow::bail!("No test cases after applying filter '{filter}'");
        }

        let out_dir = output_dir();
        fs::create_dir_all(&out_dir)?;

        let client = reqwest::Client::new();
        let mut rows = Vec::with_capacity(tests.len());
        let mut by_category: HashMap<String, (usize, usize)> = HashMap::new();
        let mut by_difficulty: HashMap<String, (usize, usize)> = HashMap::new();
        let mut by_contract: HashMap<String, (usize, usize)> = HashMap::new();
        let mut by_domain_prefix: HashMap<String, (usize, usize)> = HashMap::new();

        for (idx, case) in tests.iter().enumerate() {
            let contract_class = contract_class_for_utterance(&case.utterance);
            let session_resp = client
                .post(format!("{base_url}/api/session"))
                .header("content-type", "application/json")
                .header("x-obpoc-actor-id", &actor_id)
                .header("x-obpoc-roles", &roles)
                .json(&json!({ "name": format!("semtax-cov-{}", idx + 1) }))
                .send()
                .await?
                .error_for_status()?;
            let session_json: serde_json::Value = session_resp.json().await?;
            let session_id = session_json
                .get("session_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing session_id in session response"))?;

            for bootstrap in bootstrap_turns(case, contract_class) {
                client
                    .post(format!("{base_url}/api/session/{session_id}/input"))
                    .header("content-type", "application/json")
                    .header("x-obpoc-actor-id", &actor_id)
                    .header("x-obpoc-roles", &roles)
                    .json(&json!({
                        "kind": "utterance",
                        "message": bootstrap,
                    }))
                    .send()
                    .await?
                    .error_for_status()?;
            }

            let input_resp = client
                .post(format!("{base_url}/api/session/{session_id}/input"))
                .header("content-type", "application/json")
                .header("x-obpoc-actor-id", &actor_id)
                .header("x-obpoc-roles", &roles)
                .json(&json!({
                    "kind": "utterance",
                    "message": case.utterance,
                }))
                .send()
                .await?
                .error_for_status()?;
            let input_json: serde_json::Value = input_resp.json().await?;
            let response = input_json
                .get("response")
                .or_else(|| {
                    input_json
                        .get("chat")
                        .and_then(|value| value.get("response"))
                })
                .ok_or_else(|| anyhow::anyhow!("Missing chat response payload"))?;

            let predicted_verb = response
                .get("coder_proposal")
                .and_then(|value| value.get("verb_fqn"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            let predicted_dsl = response
                .get("coder_proposal")
                .and_then(|value| value.get("dsl"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            let requires_confirmation = response
                .get("coder_proposal")
                .and_then(|value| value.get("requires_confirmation"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let ready_to_execute = response
                .get("coder_proposal")
                .and_then(|value| value.get("ready_to_execute"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let has_sage_explain = response.get("sage_explain").is_some();
            let sage_mode = response
                .get("sage_explain")
                .and_then(|value| value.get("mode"))
                .and_then(serde_json::Value::as_str);
            let clarification_count = response
                .get("sage_explain")
                .and_then(|value| value.get("clarifications"))
                .and_then(serde_json::Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0);
            let grounded = response
                .get("sage_explain")
                .and_then(|value| value.get("scope_summary"))
                .and_then(serde_json::Value::as_str)
                .is_some();
            let step1_ambiguous =
                sage_mode == Some("scope_clarification") && clarification_count > 1;
            let step1_unresolved =
                sage_mode == Some("scope_clarification") && clarification_count == 0 && !grounded;
            let step1_resolved = grounded && !step1_ambiguous && !step1_unresolved;
            let step2_stateful =
                grounded && has_sage_explain && !step1_ambiguous && !step1_unresolved;
            let step3_proposed = predicted_verb.is_some();
            let business_verb = predicted_verb
                .as_ref()
                .map(|verb| !verb.starts_with("discovery."))
                .unwrap_or(false);
            let stateful_response = grounded && has_sage_explain;
            let pass = predicted_verb
                .as_ref()
                .map(|verb| {
                    verb == &case.expected_verb || case.alt_verbs.iter().any(|alt| alt == verb)
                })
                .unwrap_or(false);

            update_bucket(&mut by_category, &case.category, pass);
            update_bucket(&mut by_difficulty, &case.difficulty, pass);
            update_bucket(&mut by_contract, contract_class.as_str(), pass);
            update_bucket(
                &mut by_domain_prefix,
                case.expected_verb.split('.').next().unwrap_or("unknown"),
                pass,
            );

            rows.push(RowResult {
                idx: idx + 1,
                utterance: case.utterance.clone(),
                expected_verb: case.expected_verb.clone(),
                alt_verbs: case.alt_verbs.clone(),
                predicted_verb,
                predicted_dsl,
                requires_confirmation,
                ready_to_execute,
                has_sage_explain,
                step1_resolved,
                step1_ambiguous,
                step1_unresolved,
                step2_stateful,
                step3_proposed,
                grounded,
                business_verb,
                stateful_response,
                pass,
                category: case.category.clone(),
                difficulty: case.difficulty.clone(),
                contract_class: contract_class.as_str().to_string(),
            });
        }

        let passed = rows.iter().filter(|row| row.pass).count();
        let total = rows.len();
        let accuracy = if total == 0 {
            0.0
        } else {
            passed as f64 / total as f64
        };
        let grounded = rows.iter().filter(|row| row.grounded).count();
        let grounded_accuracy = if total == 0 {
            0.0
        } else {
            grounded as f64 / total as f64
        };
        let step1_resolved = rows.iter().filter(|row| row.step1_resolved).count();
        let step1_ambiguous = rows.iter().filter(|row| row.step1_ambiguous).count();
        let step1_unresolved = rows.iter().filter(|row| row.step1_unresolved).count();
        let step2_stateful = rows.iter().filter(|row| row.step2_stateful).count();
        let step3_proposed = rows.iter().filter(|row| row.step3_proposed).count();
        let business_proposals = rows.iter().filter(|row| row.business_verb).count();
        let stateful_responses = rows.iter().filter(|row| row.stateful_response).count();
        let mut mismatches: Vec<RowResult> = rows.iter().filter(|row| !row.pass).cloned().collect();
        mismatches.sort_by_key(|row| row.idx);

        let report = CoverageReport {
            summary: Summary {
                total,
                passed,
                failed: total.saturating_sub(passed),
                accuracy,
                grounded,
                grounded_accuracy,
                step1_resolved,
                step1_ambiguous,
                step1_unresolved,
                step2_stateful,
                step3_proposed,
                business_proposals,
                stateful_responses,
            },
            by_category: finalize_bucket(by_category),
            by_difficulty: finalize_bucket(by_difficulty),
            by_contract: finalize_bucket(by_contract),
            by_domain_prefix: finalize_bucket(by_domain_prefix),
            mismatches,
            rows,
        };

        let json_path = out_dir.join("semtaxonomy_coverage_results.json");
        let md_path = out_dir.join("semtaxonomy_coverage_report.md");
        fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;

        let mut md = String::new();
        md.push_str("# SemTaxonomy Utterance Coverage Report\n\n");
        md.push_str(&format!("- Total utterances: {}\n", report.summary.total));
        md.push_str(&format!("- Passed: {}\n", report.summary.passed));
        md.push_str(&format!("- Failed: {}\n", report.summary.failed));
        md.push_str(&format!(
            "- Accuracy: {:.2}%\n\n",
            report.summary.accuracy * 100.0
        ));
        md.push_str(&format!(
            "- Grounded responses: {} ({:.2}%)\n",
            report.summary.grounded,
            report.summary.grounded_accuracy * 100.0
        ));
        md.push_str(&format!(
            "- Step 1 resolved: {}\n",
            report.summary.step1_resolved
        ));
        md.push_str(&format!(
            "- Step 1 ambiguous: {}\n",
            report.summary.step1_ambiguous
        ));
        md.push_str(&format!(
            "- Step 1 unresolved: {}\n",
            report.summary.step1_unresolved
        ));
        md.push_str(&format!(
            "- Step 2 stateful: {}\n",
            report.summary.step2_stateful
        ));
        md.push_str(&format!(
            "- Step 3 proposed: {}\n",
            report.summary.step3_proposed
        ));
        md.push_str(&format!(
            "- Business proposals: {}\n",
            report.summary.business_proposals
        ));
        md.push_str(&format!(
            "- Stateful responses: {}\n\n",
            report.summary.stateful_responses
        ));
        md.push_str("## Accuracy by category\n");
        for (key, value) in &report.by_category {
            md.push_str(&format!(
                "- {key}: {}/{} ({:.2}%)\n",
                value.passed,
                value.total,
                value.accuracy * 100.0
            ));
        }
        md.push('\n');
        md.push_str("## Accuracy by difficulty\n");
        for (key, value) in &report.by_difficulty {
            md.push_str(&format!(
                "- {key}: {}/{} ({:.2}%)\n",
                value.passed,
                value.total,
                value.accuracy * 100.0
            ));
        }
        md.push('\n');
        md.push_str("## Accuracy by contract\n");
        for (key, value) in &report.by_contract {
            md.push_str(&format!(
                "- {key}: {}/{} ({:.2}%)\n",
                value.passed,
                value.total,
                value.accuracy * 100.0
            ));
        }
        md.push('\n');
        md.push_str("## Top mismatches\n");
        for row in report.mismatches.iter().take(50) {
            md.push_str(&format!(
                "- [{}] `{}` expected `{}` got `{}`\n",
                row.idx,
                row.utterance,
                row.expected_verb,
                row.predicted_verb.as_deref().unwrap_or("<none>")
            ));
        }
        fs::write(&md_path, md)?;

        println!();
        println!("=======================================================================");
        println!(
            "  SEMTAXONOMY COVERAGE -- {} utterances",
            report.summary.total
        );
        println!("  base_url={base_url}");
        println!("  output={}", out_dir.display());
        println!("=======================================================================");
        println!("  Passed: {}", report.summary.passed);
        println!("  Failed: {}", report.summary.failed);
        println!("  Accuracy: {:.2}%", report.summary.accuracy * 100.0);
        println!(
            "  Grounded: {} ({:.2}%)",
            report.summary.grounded,
            report.summary.grounded_accuracy * 100.0
        );
        println!("  Step 1 resolved: {}", report.summary.step1_resolved);
        println!("  Step 1 ambiguous: {}", report.summary.step1_ambiguous);
        println!("  Step 1 unresolved: {}", report.summary.step1_unresolved);
        println!("  Step 2 stateful: {}", report.summary.step2_stateful);
        println!("  Step 3 proposed: {}", report.summary.step3_proposed);
        println!(
            "  Business proposals: {}",
            report.summary.business_proposals
        );
        println!(
            "  Stateful responses: {}",
            report.summary.stateful_responses
        );
        println!();

        if let Some(min_accuracy) = min_accuracy {
            assert!(
                accuracy >= min_accuracy,
                "SemTaxonomy accuracy {:.4} below required {:.4}",
                accuracy,
                min_accuracy
            );
        }

        Ok(())
    }
}
