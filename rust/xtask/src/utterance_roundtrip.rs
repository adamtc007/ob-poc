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
    /// If true, and the initial utterance's proposal matches expected_verb,
    /// post a follow-up "confirm" utterance to actually execute it — proves
    /// the verb runs live through the real pipeline, not just that it's
    /// discoverable.
    #[serde(default)]
    execute: bool,
    /// If true (requires `execute: true`), verify execution by counting
    /// `"ob-poc".kyc_intent_events` rows for `expected_verb` before/after the
    /// confirm call — the durable stream is the authoritative "did this
    /// really run" signal, since the HTTP response's Executed variant does
    /// not currently surface per-step success/result (see
    /// docs/research/control-plane-ownership-ledger.md, 2026-07-15 entry).
    #[serde(default)]
    verify_kyc_stream: bool,
}

#[derive(Debug, Deserialize, Default)]
struct ChatEnvelope {
    #[serde(default)]
    response: Option<ChatPayload>,
    #[serde(default)]
    chat: Option<ChatPayload>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct DslState {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    can_execute: bool,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct ChatPayload {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    drafter_proposal: Option<DraftProposal>,
    #[serde(default)]
    discovery_bootstrap: Option<DiscoveryBootstrap>,
    #[serde(default)]
    sage_explain: Option<SageExplain>,
    #[serde(default)]
    dsl: Option<DslState>,
    #[serde(default)]
    acp_trace: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct DraftProposal {
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
pub(crate) struct HarnessReport {
    pub(crate) summary: Summary,
    pub(crate) by_domain: BTreeMap<String, Bucket>,
    pub(crate) by_root_cause: BTreeMap<String, Bucket>,
    pub(crate) metadata_gap_signals: MetadataGapSignals,
    pub(crate) rows: Vec<Row>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Summary {
    pub(crate) total: usize,
    pub(crate) passed: usize,
    pub(crate) failed: usize,
    pub(crate) executable: usize,
    pub(crate) discovery_stage: usize,
    pub(crate) no_proposal: usize,
    pub(crate) metadata_gap_failures: usize,
    /// Cases with `execute: true` that reached the confirm call.
    pub(crate) execution_attempted: usize,
    /// Of those, the confirm call itself returned without an error signal.
    pub(crate) execution_succeeded: usize,
    /// Cases with `verify_kyc_stream: true` where a new stream row was
    /// confirmed to appear — the authoritative "actually executed" count.
    pub(crate) stream_verified: usize,
}

#[derive(Debug, Serialize, Default)]
pub(crate) struct Bucket {
    pub(crate) total: usize,
    pub(crate) passed: usize,
}

#[derive(Debug, Serialize, Default)]
pub(crate) struct MetadataGapSignals {
    pub(crate) top_missing_inputs: Vec<CountRow>,
    pub(crate) top_discovery_domains: Vec<CountRow>,
    pub(crate) top_discovery_families: Vec<CountRow>,
    pub(crate) top_wrong_predictions: Vec<CountRow>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CountRow {
    pub(crate) key: String,
    pub(crate) count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Row {
    pub(crate) name: String,
    pub(crate) domain: String,
    pub(crate) utterance: String,
    pub(crate) expected_verb: String,
    pub(crate) predicted_verb: Option<String>,
    pub(crate) predicted_dsl: Option<String>,
    pub(crate) ready_to_execute: bool,
    pub(crate) requires_confirmation: bool,
    pub(crate) pass: bool,
    pub(crate) root_cause: String,
    pub(crate) likely_metadata_gap: bool,
    pub(crate) grounding_readiness: Option<String>,
    pub(crate) top_discovery_domain: Option<String>,
    pub(crate) top_discovery_family: Option<String>,
    pub(crate) top_discovery_constellation: Option<String>,
    pub(crate) missing_inputs: Vec<String>,
    pub(crate) entry_question: Option<String>,
    pub(crate) scope_summary: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) notes: Option<String>,
    /// Set only when the case has `execute: true`. `None` = execution not
    /// attempted (initial proposal didn't match/wasn't ready); `Some(false)`
    /// = confirm call errored or returned an error-shaped message;
    /// `Some(true)` = confirm call returned without an error signal (this is
    /// a weak signal — see `stream_verified` for the authoritative check).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) executed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) execute_message: Option<String>,
    /// Set only when the case has `verify_kyc_stream: true`. `Some(true)` =
    /// a new `"ob-poc".kyc_intent_events` row for `expected_verb` appeared
    /// after the confirm call — authoritative proof the verb executed and
    /// wrote to the durable stream, not just that the HTTP call succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stream_verified: Option<bool>,
}

/// Run the live utterance round-trip harness.
///
/// # Examples
/// ```
/// cargo xtask utterance-roundtrip --base-url http://127.0.0.1:3000
/// cargo xtask utterance-roundtrip --filter cbu --strict
/// ```
pub(crate) async fn run(
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

    let needs_db = fixture.cases.iter().any(|c| c.verify_kyc_stream);
    let pool: Option<sqlx::PgPool> = if needs_db {
        let db_url = std::env::var("DATABASE_URL").context(
            "DATABASE_URL must be set — a fixture case has verify_kyc_stream: true",
        )?;
        Some(
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(2)
                .connect(&db_url)
                .await
                .context("failed to connect to DATABASE_URL for kyc stream verification")?,
        )
    } else {
        None
    };

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

        let execute = case.execute;
        let verify_kyc_stream = case.verify_kyc_stream;
        let expected_verb = case.expected_verb.clone();

        let session_id = create_session(&client, base_url, &case.name, &case.domain).await?;
        for turn in &case.bootstrap {
            let _ = post_utterance(&client, base_url, &session_id, turn).await?;
        }

        let envelope = post_utterance(&client, base_url, &session_id, &case.utterance).await?;
        let payload = envelope.response.or(envelope.chat).unwrap_or_default();
        let mut row = build_row(case, payload);

        if execute && row.pass && (row.requires_confirmation || row.ready_to_execute) {
            let before_count = if verify_kyc_stream {
                match kyc_stream_event_count(pool.as_ref().unwrap(), &expected_verb).await {
                    Ok(n) => Some(n),
                    Err(e) => {
                        row.execute_message = Some(format!("pre-count query failed: {e}"));
                        None
                    }
                }
            } else {
                None
            };

            match post_utterance(&client, base_url, &session_id, "confirm").await {
                Ok(confirm_envelope) => {
                    let confirm_payload = confirm_envelope
                        .response
                        .or(confirm_envelope.chat)
                        .unwrap_or_default();
                    let looks_like_error = confirm_payload
                        .message
                        .as_deref()
                        .map(|m| {
                            let lower = m.to_ascii_lowercase();
                            lower.contains("error") || lower.contains("fail")
                        })
                        .unwrap_or(false);
                    row.executed = Some(!looks_like_error);
                    row.execute_message = confirm_payload.message;
                }
                Err(e) => {
                    row.executed = Some(false);
                    row.execute_message = Some(format!("confirm request failed: {e}"));
                }
            }

            if verify_kyc_stream {
                if let Some(before) = before_count {
                    match kyc_stream_event_count(pool.as_ref().unwrap(), &expected_verb).await {
                        Ok(after) => row.stream_verified = Some(after > before),
                        Err(e) => {
                            row.execute_message = Some(format!(
                                "{} | post-count query failed: {e}",
                                row.execute_message.unwrap_or_default()
                            ));
                        }
                    }
                }
            }
        }

        rows.push(row);
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

async fn create_session(
    client: &Client,
    base_url: &str,
    name: &str,
    domain: &str,
) -> Result<String> {
    let response = client
        .post(format!("{base_url}/api/session"))
        .header("content-type", "application/json")
        .header("x-obpoc-actor-id", "xtask-utterance-roundtrip")
        .header("x-obpoc-roles", "admin")
        .json(&json!({
            "name": format!("utterance-roundtrip-{name}"),
            "domain_hint": domain
        }))
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

/// Count `"ob-poc".kyc_intent_events` rows for `verb_fqn` — the durable,
/// authoritative signal that a `dsl.kyc` verb actually executed and
/// appended to the stream (K-16 system of record), independent of whether
/// the HTTP response surfaces a structured success field.
async fn kyc_stream_event_count(pool: &sqlx::PgPool, verb_fqn: &str) -> Result<i64> {
    let row: (i64,) =
        sqlx::query_as(r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE verb_fqn = $1"#)
            .bind(verb_fqn)
            .fetch_one(pool)
            .await
            .context("kyc_intent_events count query failed")?;
    Ok(row.0)
}

fn build_row(case: FixtureCase, payload: ChatPayload) -> Row {
    let coder = payload.drafter_proposal.clone().unwrap_or_default();
    let discovery = payload.discovery_bootstrap.clone().unwrap_or_default();
    let acp_trace = payload.acp_trace.as_ref();

    let predicted_verb = coder.verb_fqn.clone().or_else(|| {
        acp_trace
            .and_then(|t| t.get("selected_verb"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });

    let predicted_dsl = coder
        .dsl
        .clone()
        .or_else(|| payload.dsl.as_ref().and_then(|d| d.source.clone()));

    let ready_to_execute = coder.ready_to_execute
        || acp_trace
            .and_then(|t| t.get("dry_run_valid"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

    let requires_confirmation =
        coder.requires_confirmation || (acp_trace.is_some() && predicted_dsl.is_some());

    let pass = predicted_verb.as_deref().is_some_and(|verb| {
        verb == case.expected_verb || case.alt_verbs.iter().any(|alt| alt == verb)
    });

    let root_cause = classify_root_cause(
        &case.expected_verb,
        predicted_verb.as_deref(),
        &coder,
        &discovery,
        payload.message.as_deref(),
        acp_trace,
    );

    let likely_metadata_gap = matches!(
        root_cause.as_str(),
        "sem_os_discovery_gap"
            | "sem_os_metadata_gap"
            | "domain_routing_gap"
            | "missing_grounding_input"
    );

    let grounding_readiness = discovery.grounding_readiness.clone().or_else(|| {
        acp_trace
            .and_then(|t| t.get("status"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });

    let top_discovery_domain = discovery
        .matched_domains
        .first()
        .map(|domain| {
            format!(
                "{}:{} ({:.2})",
                domain.domain_id, domain.label, domain.score
            )
        })
        .or_else(|| {
            acp_trace
                .and_then(|t| t.get("selected_dispatch_kind"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    let top_discovery_family = discovery
        .matched_families
        .first()
        .map(|family| {
            format!(
                "{}:{} [{}] ({:.2})",
                family.family_id, family.label, family.domain_id, family.score
            )
        })
        .or_else(|| {
            acp_trace
                .and_then(|t| t.get("pack_ref"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    let top_discovery_constellation = discovery.matched_constellations.first().map(|item| {
        format!(
            "{}:{} ({:.2})",
            item.constellation_id, item.label, item.score
        )
    });

    let missing_inputs = if !discovery.missing_inputs.is_empty() {
        discovery
            .missing_inputs
            .iter()
            .map(|item| {
                format!(
                    "{}:{}:{}:{}",
                    item.key, item.label, item.required, item.input_type
                )
            })
            .collect()
    } else {
        acp_trace
            .and_then(|t| t.get("needed_from_user"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|val| val.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    };

    let entry_question = discovery
        .entry_questions
        .first()
        .map(|question| {
            format!(
                "{}:{}:{}:{}",
                question.question_id, question.prompt, question.maps_to, question.priority
            )
        })
        .or_else(|| {
            acp_trace
                .and_then(|t| t.get("pending_question_code"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    let scope_summary = payload
        .sage_explain
        .and_then(|explain| explain.scope_summary)
        .or_else(|| {
            acp_trace
                .and_then(|t| t.get("human_summary"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    Row {
        name: case.name,
        domain: case.domain,
        utterance: case.utterance,
        expected_verb: case.expected_verb,
        predicted_verb,
        predicted_dsl,
        ready_to_execute,
        requires_confirmation,
        pass,
        root_cause,
        likely_metadata_gap,
        grounding_readiness,
        top_discovery_domain,
        top_discovery_family,
        top_discovery_constellation,
        missing_inputs,
        entry_question,
        scope_summary,
        message: payload.message,
        notes: case.notes,
        executed: None,
        execute_message: None,
        stream_verified: None,
    }
}

fn classify_root_cause(
    expected_verb: &str,
    predicted_verb: Option<&str>,
    coder: &DraftProposal,
    discovery: &DiscoveryBootstrap,
    message: Option<&str>,
    acp_trace: Option<&serde_json::Value>,
) -> String {
    let expected_domain = expected_verb.split('.').next().unwrap_or_default();
    let message = message.unwrap_or_default();

    if let Some(predicted) = predicted_verb {
        if predicted == expected_verb {
            let has_dsl = coder.dsl.is_some()
                || acp_trace
                    .and_then(|t| t.get("dry_run_valid").and_then(|v| v.as_bool()))
                    .unwrap_or(false);
            let ready = coder.ready_to_execute
                || acp_trace
                    .and_then(|t| t.get("dry_run_valid").and_then(|v| v.as_bool()))
                    .unwrap_or(false);
            if ready || coder.requires_confirmation || has_dsl {
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

    if let Some(trace) = acp_trace {
        if let Some(status) = trace.get("status").and_then(|v| v.as_str()) {
            if status == "pending_question" {
                if let Some(needed) = trace.get("needed_from_user").and_then(|v| v.as_array()) {
                    if !needed.is_empty() {
                        return "missing_grounding_input".to_string();
                    }
                }
                return "sem_os_discovery_gap".to_string();
            }
            if status == "structured_refusal" {
                return "sem_os_metadata_gap".to_string();
            }
        }
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
        execution_attempted: rows.iter().filter(|row| row.executed.is_some()).count(),
        execution_succeeded: rows
            .iter()
            .filter(|row| row.executed == Some(true))
            .count(),
        stream_verified: rows
            .iter()
            .filter(|row| row.stream_verified == Some(true))
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
        "- Total: {}\n- Passed: {}\n- Failed: {}\n- Executable: {}\n- Discovery-stage misses: {}\n- Metadata-gap failures: {}\n- Execution attempted: {}\n- Execution succeeded (HTTP-level, weak signal): {}\n- Stream-verified (DB-level, authoritative): {}\n\n",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.executable,
        report.summary.discovery_stage,
        report.summary.metadata_gap_failures,
        report.summary.execution_attempted,
        report.summary.execution_succeeded,
        report.summary.stream_verified,
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
