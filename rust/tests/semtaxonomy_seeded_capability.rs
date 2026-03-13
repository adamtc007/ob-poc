//! Seeded capability harness for the SemTaxonomy path.
//!
//! This seeds a minimal client-group/deal/onboarding/KYC/screening/document
//! shape into the database, then exercises the live SemTaxonomy session input
//! path against that known state.
//!
//! Run with:
//! DATABASE_URL="postgresql:///data_designer" \
//! SEMTAXONOMY_API_BASE_URL="http://127.0.0.1:3000" \
//! cargo test -p ob-poc --test semtaxonomy_seeded_capability -- --ignored --nocapture

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use reqwest::Client;
use serde::Serialize;
use serde_json::json;

mod support;

use support::semtaxonomy_seed::{cleanup_state, get_pool, seed_state, SeedState};

#[derive(Debug, Clone)]
struct SeedCase {
    name: &'static str,
    prelude: Vec<String>,
    utterance: String,
    expected_verb: Option<&'static str>,
    require_business_verb: bool,
}

#[derive(Debug, Serialize)]
struct SeedRowResult {
    name: String,
    utterance: String,
    predicted_verb: Option<String>,
    requires_confirmation: bool,
    ready_to_execute: bool,
    has_sage_explain: bool,
    scope_summary: Option<String>,
    business_verb: bool,
    grounded: bool,
    stateful_response: bool,
    pass: bool,
}

#[derive(Debug, Serialize)]
struct SeedReport {
    total: usize,
    passed: usize,
    business_verbs: usize,
    grounded: usize,
    stateful_responses: usize,
    rows: Vec<SeedRowResult>,
}

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/semtaxonomy-seeded-capability")
}

async fn create_session(client: &Client, base_url: &str) -> Result<String> {
    let response = client
        .post(format!("{base_url}/api/session"))
        .header("content-type", "application/json")
        .header("x-obpoc-actor-id", "seeded-capability")
        .header("x-obpoc-roles", "admin")
        .json(&json!({ "name": "semtaxonomy-seeded-capability" }))
        .send()
        .await?
        .error_for_status()?;
    let payload: serde_json::Value = response.json().await?;
    Ok(payload["session_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing session_id"))?
        .to_string())
}

async fn post_utterance(
    client: &Client,
    base_url: &str,
    session_id: &str,
    utterance: &str,
) -> Result<serde_json::Value> {
    let response = client
        .post(format!("{base_url}/api/session/{session_id}/input"))
        .header("content-type", "application/json")
        .header("x-obpoc-actor-id", "seeded-capability")
        .header("x-obpoc-roles", "admin")
        .json(&json!({ "kind": "utterance", "message": utterance }))
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}

fn cases(state: &SeedState) -> Vec<SeedCase> {
    vec![
        SeedCase {
            name: "ground_client_group",
            prelude: vec![],
            utterance: format!("{} Allianz", state.prefix),
            expected_verb: Some("discovery.entity-context"),
            require_business_verb: false,
        },
        SeedCase {
            name: "list_deals",
            prelude: vec![format!("{} Allianz", state.prefix)],
            utterance: format!("what deals does {} Allianz have?", state.prefix),
            expected_verb: Some("deal.list"),
            require_business_verb: true,
        },
        SeedCase {
            name: "list_cbus",
            prelude: vec![format!("{} Allianz", state.prefix)],
            utterance: "show me the cbus".to_string(),
            expected_verb: Some("cbu.list"),
            require_business_verb: true,
        },
        SeedCase {
            name: "create_cbu",
            prelude: vec![format!("{} Allianz", state.prefix)],
            utterance: format!("create a new CBU for {} Growth Fund", state.prefix),
            expected_verb: Some("cbu.create"),
            require_business_verb: true,
        },
        SeedCase {
            name: "cbu_parties",
            prelude: vec![format!("{} SICAV", state.prefix)],
            utterance: "who are the parties on this CBU?".to_string(),
            expected_verb: Some("cbu.parties"),
            require_business_verb: true,
        },
        SeedCase {
            name: "screening_sanctions",
            prelude: vec![format!("{} Management Ltd", state.prefix)],
            utterance: "run sanctions screening on this entity".to_string(),
            expected_verb: Some("screening.sanctions"),
            require_business_verb: true,
        },
        SeedCase {
            name: "ubo_list_owners",
            prelude: vec![format!("{} Management Ltd", state.prefix)],
            utterance: "who owns this?".to_string(),
            expected_verb: Some("ubo.list-owners"),
            require_business_verb: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn semtaxonomy_seeded_capability() -> Result<()> {
        let base_url = std::env::var("SEMTAXONOMY_API_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
        let out_dir = output_dir();
        fs::create_dir_all(&out_dir)?;

        let pool = get_pool().await?;
        let state = seed_state(&pool).await?;
        let client = Client::new();
        let mut rows = Vec::new();

        for case in cases(&state) {
            let session_id = create_session(&client, &base_url).await?;
            for turn in &case.prelude {
                let _ = post_utterance(&client, &base_url, &session_id, turn).await?;
            }
            let payload = post_utterance(&client, &base_url, &session_id, &case.utterance).await?;
            let response = payload
                .get("response")
                .or_else(|| payload.get("chat").and_then(|value| value.get("response")))
                .ok_or_else(|| anyhow::anyhow!("missing chat response"))?;

            let predicted_verb = response
                .get("coder_proposal")
                .and_then(|value| value.get("verb_fqn"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            let requires_confirmation = response
                .get("coder_proposal")
                .and_then(|value| value.get("requires_confirmation"))
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let ready_to_execute = response
                .get("coder_proposal")
                .and_then(|value| value.get("ready_to_execute"))
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let has_sage_explain = response.get("sage_explain").is_some();
            let scope_summary = response
                .get("sage_explain")
                .and_then(|value| value.get("scope_summary"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            let business_verb = predicted_verb
                .as_ref()
                .map(|verb| !verb.starts_with("discovery."))
                .unwrap_or(false);
            let grounded = scope_summary.is_some();
            let stateful_response = has_sage_explain && grounded;
            let pass = predicted_verb
                .as_deref()
                .map(|verb| Some(verb) == case.expected_verb)
                .unwrap_or(false)
                && (!case.require_business_verb || business_verb);

            rows.push(SeedRowResult {
                name: case.name.to_string(),
                utterance: case.utterance,
                predicted_verb,
                requires_confirmation,
                ready_to_execute,
                has_sage_explain,
                scope_summary,
                business_verb,
                grounded,
                stateful_response,
                pass,
            });
        }

        cleanup_state(&pool, &state).await;

        let report = SeedReport {
            total: rows.len(),
            passed: rows.iter().filter(|row| row.pass).count(),
            business_verbs: rows.iter().filter(|row| row.business_verb).count(),
            grounded: rows
                .iter()
                .filter(|row| row.scope_summary.is_some())
                .count(),
            stateful_responses: rows.iter().filter(|row| row.stateful_response).count(),
            rows,
        };

        fs::write(
            out_dir.join("seeded_capability_report.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;

        if report.passed == 0 {
            anyhow::bail!("Seeded SemTaxonomy capability harness produced 0 passing cases");
        }

        Ok(())
    }
}
