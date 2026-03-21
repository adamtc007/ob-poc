//! Live utterance round-trip harness.
//!
//! Posts natural-language phrases through the unified session input API,
//! captures Sem OS discovery/bootstrap and coder proposal outputs, then
//! classifies misses into likely root causes for metadata and grounding gaps.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Fixture {
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    name: String,
    domain: String,
    utterance: String,
    expected_verb: String,
    #[serde(default)]
    alt_verbs: Vec<String>,
    #[serde(default)]
    bootstrap: Vec<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ChatEnvelope {
    #[serde(default)]
    response: Option<ChatPayload>,
    #[serde(default)]
    chat: Option<ChatPayload>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct ChatPayload {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    coder_proposal: Option<CoderProposal>,
    #[serde(default)]
    discovery_bootstrap: Option<DiscoveryBootstrap>,
    #[serde(default)]
    sage_explain: Option<SageExplain>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct CoderProposal {
    #[serde(default)]
    verb_fqn: Option<String>,
    #[serde(default)]
    dsl: Option<String>,
    #[serde(default)]
    requires_confirmation: bool,
    #[serde(default)]
    ready_to_execute: bool,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct DiscoveryBootstrap {
    #[serde(default)]
    grounding_readiness: Option<String>,
    #[serde(default)]
    matched_domains: Vec<DiscoveryDomain>,
    #[serde(default)]
    matched_families: Vec<DiscoveryFamily>,
    #[serde(default)]
    matched_constellations: Vec<DiscoveryConstellation>,
    #[serde(default)]
    missing_inputs: Vec<DiscoveryInput>,
    #[serde(default)]
    entry_questions: Vec<DiscoveryQuestion>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct DiscoveryDomain {
    domain_id: String,
    label: String,
    score: f64,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct DiscoveryFamily {
    family_id: String,
    label: String,
    domain_id: String,
    score: f64,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct DiscoveryConstellation {
    constellation_id: String,
    label: String,
    score: f64,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct DiscoveryInput {
    key: String,
    label: String,
    required: bool,
    input_type: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct DiscoveryQuestion {
    question_id: String,
    prompt: String,
    maps_to: String,
    priority: u8,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct SageExplain {
    #[serde(default)]
    scope_summary: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HarnessReport {
    pub summary: Summary,
    pub by_domain: BTreeMap<String, Bucket>,
    pub by_root_cause: BTreeMap<String, Bucket>,
    pub metadata_gap_signals: MetadataGapSignals,
    pub rows: Vec<Row>,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub executable: usize,
    pub discovery_stage: usize,
    pub no_proposal: usize,
    pub metadata_gap_failures: usize,
}

#[derive(Debug, Serialize, Default)]
pub struct Bucket {
    pub total: usize,
    pub passed: usize,
}

#[derive(Debug, Serialize, Default)]
pub struct MetadataGapSignals {
    pub top_missing_inputs: Vec<CountRow>,
    pub top_discovery_domains: Vec<CountRow>,
    pub top_discovery_families: Vec<CountRow>,
    pub top_wrong_predictions: Vec<CountRow>,
}

#[derive(Debug, Serialize)]
pub struct CountRow {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub name: String,
    pub domain: String,
    pub utterance: String,
    pub expected_verb: String,
    pub predicted_verb: Option<String>,
    pub predicted_dsl: Option<String>,
    pub ready_to_execute: bool,
    pub requires_confirmation: bool,
    pub pass: bool,
    pub root_cause: String,
    pub likely_metadata_gap: bool,
    pub grounding_readiness: Option<String>,
    pub top_discovery_domain: Option<String>,
    pub top_discovery_family: Option<String>,
    pub top_discovery_constellation: Option<String>,
    pub missing_inputs: Vec<String>,
    pub entry_question: Option<String>,
    pub scope_summary: Option<String>,
    pub message: Option<String>,
    pub notes: Option<String>,
}

/// Run the live utterance round-trip harness.
///
/// # Examples
/// ```
/// cargo xtask utterance-roundtrip --base-url http://127.0.0.1:3000
/// cargo xtask utterance-roundtrip --filter cbu --strict
/// ```
pub async fn run(
    base_url: &str,
    fixture_path: &Path,
    out_dir: &Path,
    filter: Option<&str>,
    limit: Option<usize>,
) -> Result<HarnessReport> {
    let fixture = load_fixture(fixture_path)?;
    let filter = filter.map(|value| value.to_ascii_lowercase());
    let client = Client::new();
    let mut rows = Vec::new();

    for case in fixture
        .cases
        .into_iter()
        .filter(|case| matches_filter(case, filter.as_deref()))
    {
        if let Some(max) = limit {
            if rows.len() >= max {
                break;
            }
        }

        let session_id = create_session(&client, base_url, &case.name).await?;
        for turn in &case.bootstrap {
            let _ = post_utterance(&client, base_url, &session_id, turn).await?;
        }

        let envelope = post_utterance(&client, base_url, &session_id, &case.utterance).await?;
        let payload = envelope.response.or(envelope.chat).unwrap_or_default();
        rows.push(build_row(case, payload));
    }

    let report = build_report(rows);
    fs::create_dir_all(out_dir)?;
    fs::write(
        out_dir.join("utterance_roundtrip_report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write(
        out_dir.join("utterance_roundtrip_report.md"),
        render_markdown(&report),
    )?;

    println!(
        "Utterance round-trip: {}/{} passed, {} metadata-gap failures",
        report.summary.passed, report.summary.total, report.summary.metadata_gap_failures
    );
    println!(
        "Artifacts: {}, {}",
        out_dir.join("utterance_roundtrip_report.json").display(),
        out_dir.join("utterance_roundtrip_report.md").display()
    );

    Ok(report)
}

fn load_fixture(path: &Path) -> Result<Fixture> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read fixture {}", path.display()))?;
    serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse fixture {}", path.display()))
}

fn matches_filter(case: &FixtureCase, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let haystack = format!("{} {} {}", case.name, case.domain, case.utterance).to_ascii_lowercase();
    haystack.contains(filter)
}

async fn create_session(client: &Client, base_url: &str, name: &str) -> Result<String> {
    let response = client
        .post(format!("{base_url}/api/session"))
        .header("content-type", "application/json")
        .header("x-obpoc-actor-id", "xtask-utterance-roundtrip")
        .header("x-obpoc-roles", "admin")
        .json(&json!({ "name": format!("utterance-roundtrip-{name}") }))
        .send()
        .await?
        .error_for_status()?;

    let payload: serde_json::Value = response.json().await?;
    payload["session_id"]
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("missing session_id in create_session response"))
}

async fn post_utterance(
    client: &Client,
    base_url: &str,
    session_id: &str,
    utterance: &str,
) -> Result<ChatEnvelope> {
    let response = client
        .post(format!("{base_url}/api/session/{session_id}/input"))
        .header("content-type", "application/json")
        .header("x-obpoc-actor-id", "xtask-utterance-roundtrip")
        .header("x-obpoc-roles", "admin")
        .json(&json!({ "kind": "utterance", "message": utterance }))
        .send()
        .await?
        .error_for_status()?;

    response
        .json::<ChatEnvelope>()
        .await
        .context("failed to decode chat envelope")
}

fn build_row(case: FixtureCase, payload: ChatPayload) -> Row {
    let coder = payload.coder_proposal.clone().unwrap_or_default();
    let discovery = payload.discovery_bootstrap.clone().unwrap_or_default();
    let predicted_verb = coder.verb_fqn.clone();
    let pass = predicted_verb.as_deref().is_some_and(|verb| {
        verb == case.expected_verb || case.alt_verbs.iter().any(|alt| alt == verb)
    });
    let root_cause = classify_root_cause(
        &case.expected_verb,
        predicted_verb.as_deref(),
        &coder,
        &discovery,
        payload.message.as_deref(),
    );
    let likely_metadata_gap = matches!(
        root_cause.as_str(),
        "sem_os_discovery_gap"
            | "sem_os_metadata_gap"
            | "domain_routing_gap"
            | "missing_grounding_input"
    );

    Row {
        name: case.name,
        domain: case.domain,
        utterance: case.utterance,
        expected_verb: case.expected_verb,
        predicted_verb,
        predicted_dsl: coder.dsl,
        ready_to_execute: coder.ready_to_execute,
        requires_confirmation: coder.requires_confirmation,
        pass,
        root_cause,
        likely_metadata_gap,
        grounding_readiness: discovery.grounding_readiness.clone(),
        top_discovery_domain: discovery.matched_domains.first().map(|domain| {
            format!(
                "{}:{} ({:.2})",
                domain.domain_id, domain.label, domain.score
            )
        }),
        top_discovery_family: discovery.matched_families.first().map(|family| {
            format!(
                "{}:{} [{}] ({:.2})",
                family.family_id, family.label, family.domain_id, family.score
            )
        }),
        top_discovery_constellation: discovery.matched_constellations.first().map(|item| {
            format!(
                "{}:{} ({:.2})",
                item.constellation_id, item.label, item.score
            )
        }),
        missing_inputs: discovery
            .missing_inputs
            .iter()
            .map(|item| {
                format!(
                    "{}:{}:{}:{}",
                    item.key, item.label, item.required, item.input_type
                )
            })
            .collect(),
        entry_question: discovery.entry_questions.first().map(|question| {
            format!(
                "{}:{}:{}:{}",
                question.question_id, question.prompt, question.maps_to, question.priority
            )
        }),
        scope_summary: payload
            .sage_explain
            .and_then(|explain| explain.scope_summary),
        message: payload.message,
        notes: case.notes,
    }
}

fn classify_root_cause(
    expected_verb: &str,
    predicted_verb: Option<&str>,
    coder: &CoderProposal,
    discovery: &DiscoveryBootstrap,
    message: Option<&str>,
) -> String {
    let expected_domain = expected_verb.split('.').next().unwrap_or_default();
    let message = message.unwrap_or_default();

    if let Some(predicted) = predicted_verb {
        if predicted == expected_verb {
            if coder.ready_to_execute || coder.requires_confirmation || coder.dsl.is_some() {
                return "pass".to_string();
            }
            return "matched_but_not_executable".to_string();
        }

        let predicted_domain = predicted.split('.').next().unwrap_or_default();
        if predicted_domain == expected_domain {
            return "verb_phrase_ranking_gap".to_string();
        }
        if predicted.starts_with("discovery.") || predicted.starts_with("schema.") {
            return "sem_os_metadata_gap".to_string();
        }
        return "domain_routing_gap".to_string();
    }

    if !discovery.entry_questions.is_empty() || !discovery.missing_inputs.is_empty() {
        return "missing_grounding_input".to_string();
    }
    if !discovery.matched_domains.is_empty()
        || !discovery.matched_families.is_empty()
        || !discovery.matched_constellations.is_empty()
    {
        return "sem_os_discovery_gap".to_string();
    }
    if message.contains("Sem OS unavailable") {
        return "sem_os_unavailable".to_string();
    }
    if message.contains("I need") || message.to_ascii_lowercase().contains("clar") {
        return "clarification_needed".to_string();
    }
    "no_proposal".to_string()
}

fn build_report(rows: Vec<Row>) -> HarnessReport {
    let mut by_domain = BTreeMap::<String, Bucket>::new();
    let mut by_root_cause = BTreeMap::<String, Bucket>::new();
    let mut missing_inputs = BTreeMap::<String, usize>::new();
    let mut discovery_domains = BTreeMap::<String, usize>::new();
    let mut discovery_families = BTreeMap::<String, usize>::new();
    let mut wrong_predictions = BTreeMap::<String, usize>::new();

    for row in &rows {
        let domain_bucket = by_domain.entry(row.domain.clone()).or_default();
        domain_bucket.total += 1;
        if row.pass {
            domain_bucket.passed += 1;
        }

        let cause_bucket = by_root_cause.entry(row.root_cause.clone()).or_default();
        cause_bucket.total += 1;
        if row.pass {
            cause_bucket.passed += 1;
        }

        for key in &row.missing_inputs {
            *missing_inputs.entry(key.clone()).or_insert(0) += 1;
        }
        if let Some(domain) = row.top_discovery_domain.as_ref() {
            *discovery_domains.entry(domain.clone()).or_insert(0) += 1;
        }
        if let Some(family) = row.top_discovery_family.as_ref() {
            *discovery_families.entry(family.clone()).or_insert(0) += 1;
        }
        if !row.pass {
            if let Some(predicted) = row.predicted_verb.as_ref() {
                *wrong_predictions.entry(predicted.clone()).or_insert(0) += 1;
            }
        }
    }

    let summary = Summary {
        total: rows.len(),
        passed: rows.iter().filter(|row| row.pass).count(),
        failed: rows.iter().filter(|row| !row.pass).count(),
        executable: rows.iter().filter(|row| row.ready_to_execute).count(),
        discovery_stage: rows
            .iter()
            .filter(|row| {
                matches!(
                    row.root_cause.as_str(),
                    "missing_grounding_input" | "sem_os_discovery_gap"
                )
            })
            .count(),
        no_proposal: rows
            .iter()
            .filter(|row| row.predicted_verb.is_none())
            .count(),
        metadata_gap_failures: rows
            .iter()
            .filter(|row| !row.pass && row.likely_metadata_gap)
            .count(),
    };

    HarnessReport {
        summary,
        by_domain,
        by_root_cause,
        metadata_gap_signals: MetadataGapSignals {
            top_missing_inputs: top_counts(missing_inputs),
            top_discovery_domains: top_counts(discovery_domains),
            top_discovery_families: top_counts(discovery_families),
            top_wrong_predictions: top_counts(wrong_predictions),
        },
        rows,
    }
}

fn top_counts(map: BTreeMap<String, usize>) -> Vec<CountRow> {
    let mut rows: Vec<CountRow> = map
        .into_iter()
        .map(|(key, count)| CountRow { key, count })
        .collect();
    rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    rows.truncate(12);
    rows
}

fn render_markdown(report: &HarnessReport) -> String {
    let mut out = String::new();
    out.push_str("# Utterance Round-Trip Report\n\n");
    out.push_str(&format!(
        "- Total: {}\n- Passed: {}\n- Failed: {}\n- Executable: {}\n- Discovery-stage misses: {}\n- Metadata-gap failures: {}\n\n",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.executable,
        report.summary.discovery_stage,
        report.summary.metadata_gap_failures,
    ));

    out.push_str("## Root Cause Buckets\n\n");
    for (cause, bucket) in &report.by_root_cause {
        out.push_str(&format!(
            "- `{}`: {}/{} passed\n",
            cause, bucket.passed, bucket.total
        ));
    }

    out.push_str("\n## Metadata Signals\n\n");
    for row in &report.metadata_gap_signals.top_missing_inputs {
        out.push_str(&format!("- Missing input `{}`: {}\n", row.key, row.count));
    }
    for row in &report.metadata_gap_signals.top_discovery_domains {
        out.push_str(&format!(
            "- Discovery domain `{}`: {}\n",
            row.key, row.count
        ));
    }
    for row in &report.metadata_gap_signals.top_discovery_families {
        out.push_str(&format!(
            "- Discovery family `{}`: {}\n",
            row.key, row.count
        ));
    }

    out.push_str("\n## Failures\n\n");
    for row in report.rows.iter().filter(|row| !row.pass) {
        out.push_str(&format!(
            "- `{}` expected `{}` got `{}` cause=`{}`\n",
            row.utterance,
            row.expected_verb,
            row.predicted_verb.as_deref().unwrap_or("<none>"),
            row.root_cause
        ));
        if let Some(question) = row.entry_question.as_ref() {
            out.push_str(&format!("  question: {}\n", question));
        }
        if !row.missing_inputs.is_empty() {
            out.push_str(&format!(
                "  missing_inputs: {}\n",
                row.missing_inputs.join(", ")
            ));
        }
        if let Some(domain) = row.top_discovery_domain.as_ref() {
            out.push_str(&format!("  top_domain: {}\n", domain));
        }
    }

    out
}
