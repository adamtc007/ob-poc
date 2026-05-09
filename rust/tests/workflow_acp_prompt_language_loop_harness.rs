//! ACP `session/prompt` harness for KYC update-status language-loop routing.

use std::fs;
use std::path::Path;

use ob_poc::acp_protocol::{AcpJsonRpcAgent, JsonRpcOutgoing, JsonRpcRequest};
use ob_poc::runbook::KycUpdateStatusWorkbookDraft;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::{uuid, Uuid};

const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");
const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
const FIXTURE_DIR: &str = "tests/fixtures/workflow_validity/kyc_update_status";

#[derive(Debug, Deserialize)]
struct Fixture {
    name: String,
    language_current_state: String,
    draft: KycUpdateStatusWorkbookDraft,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
struct Expected {
    outcome: String,
}

#[test]
fn workflow_acp_prompt_language_loop_harness_reports_prompt_to_dry_run_rates() {
    let fixtures = load_fixtures();
    assert!(fixtures.len() >= 20, "expected at least 20 fixtures");

    let mut total = 0usize;
    let mut prompt_routed = 0usize;
    let mut dry_run_valid = 0usize;
    let mut structured_refusal = 0usize;
    let mut draft_failure_canonicalized_to_valid = 0usize;
    let mut pending_question = 0usize;
    let mut prose_only_failure = 0usize;
    let mut unexpected_fallback = 0usize;

    for fixture in fixtures {
        total += 1;
        let mut agent = AcpJsonRpcAgent::new();
        let outgoing = agent.handle_request(request(
            total as i64,
            "session/prompt",
            prompt_params(&fixture),
        ));

        if outgoing.iter().any(is_language_pack_tool_call) {
            prompt_routed += 1;
        }

        let response = response_result(&outgoing)
            .unwrap_or_else(|| panic!("{}: expected JSON-RPC response", fixture.name));
        let expected_prompt = expected_prompt_outcome(&fixture);
        match response["status"].as_str() {
            Some("dry_run_validated") => {
                assert_eq!(expected_prompt.outcome, "dry_run_valid", "{}", fixture.name);
                dry_run_valid += 1;
                if fixture.expected.outcome == "refused" {
                    draft_failure_canonicalized_to_valid += 1;
                }
                assert!(outgoing.iter().any(is_semantic_diff), "{}", fixture.name);
            }
            Some("structured_refusal") => {
                assert_eq!(expected_prompt.outcome, "refused", "{}", fixture.name);
                structured_refusal += 1;
                assert!(
                    !outgoing.iter().any(is_semantic_diff),
                    "{} produced semantic diff after refusal",
                    fixture.name
                );
                let expected_refusal = expected_prompt
                    .refusal_code
                    .as_deref()
                    .expect("refusal fixture has code");
                assert_eq!(
                    response["refusal"]["refusal_code"], expected_refusal,
                    "{}",
                    fixture.name
                );
            }
            Some("pending_question") => {
                pending_question += 1;
            }
            _ if response["stopReason"] == "end_turn" => {
                unexpected_fallback += 1;
            }
            _ => {
                prose_only_failure += 1;
            }
        }
    }

    println!("\n=======================================================================");
    println!("  ACP PROMPT LANGUAGE LOOP HARNESS -- {} fixtures", total);
    println!("=======================================================================");
    println!(
        "  Prompt routed:            {}/{} ({:.1}%)",
        prompt_routed,
        total,
        pct(prompt_routed, total)
    );
    println!(
        "  Dry-run valid:            {}/{} ({:.1}%)",
        dry_run_valid,
        total,
        pct(dry_run_valid, total)
    );
    println!(
        "  Structured refusals:      {}/{} ({:.1}%)",
        structured_refusal,
        total,
        pct(structured_refusal, total)
    );
    println!(
        "  Draft failures canonical: {}/{} ({:.1}%)",
        draft_failure_canonicalized_to_valid,
        total,
        pct(draft_failure_canonicalized_to_valid, total)
    );
    println!(
        "  Pending questions:        {}/{} ({:.1}%)",
        pending_question,
        total,
        pct(pending_question, total)
    );
    println!(
        "  Prose-only failures:      {}/{} ({:.1}%)",
        prose_only_failure,
        total,
        pct(prose_only_failure, total)
    );
    println!(
        "  Unexpected fallback:      {}/{} ({:.1}%)",
        unexpected_fallback,
        total,
        pct(unexpected_fallback, total)
    );
    println!("=======================================================================\n");

    assert_eq!(prompt_routed, 20);
    assert_eq!(dry_run_valid, 18);
    assert_eq!(structured_refusal, 2);
    assert_eq!(draft_failure_canonicalized_to_valid, 8);
    assert_eq!(pending_question, 0);
    assert_eq!(prose_only_failure, 0);
    assert_eq!(unexpected_fallback, 0);
}

#[derive(Debug, Clone)]
struct ExpectedPromptOutcome {
    outcome: &'static str,
    refusal_code: Option<&'static str>,
}

fn expected_prompt_outcome(fixture: &Fixture) -> ExpectedPromptOutcome {
    if fixture
        .draft
        .evidence_digest
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        ExpectedPromptOutcome {
            outcome: "refused",
            refusal_code: Some("missing_evidence_digest"),
        }
    } else {
        ExpectedPromptOutcome {
            outcome: "dry_run_valid",
            refusal_code: None,
        }
    }
}

fn prompt_params(fixture: &Fixture) -> Value {
    let evidence_text = fixture
        .draft
        .evidence_digest
        .as_deref()
        .filter(|digest| !digest.trim().is_empty())
        .map(|digest| format!(" with evidence {digest}"))
        .unwrap_or_default();
    let transition_hint = if fixture.draft.transition_ref.starts_with("kyc-case.") {
        format!(" transition {}", fixture.draft.transition_ref)
    } else {
        String::new()
    };
    let text = format!(
        "Move the KYC case from {} to {}{}{}",
        fixture.draft.current_state, fixture.draft.requested_state, transition_hint, evidence_text
    );
    let case_state = json!({
        "case_state": {
            "subject_id": fixture.draft.case_id.unwrap_or(CASE_ID),
            "current_state": fixture.language_current_state,
            "configuration_version": "config-1",
            "state_snapshot_id": "snapshot-1"
        }
    });

    json!({
        "sessionId": SESSION_ID.to_string(),
        "prompt": [
            {"type": "text", "text": text},
            {
                "type": "embedded_resource",
                "uri": format!("semos://entity/{}", CASE_ID),
                "name": "KYC case state",
                "mime_type": "application/json",
                "text": case_state.to_string()
            }
        ]
    })
}

fn request(id: i64, method: &str, params: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}

fn response_result(outgoing: &[JsonRpcOutgoing]) -> Option<&Value> {
    outgoing.iter().find_map(|item| match item {
        JsonRpcOutgoing::Response(response) => response.result.as_ref(),
        JsonRpcOutgoing::Notification(_) => None,
    })
}

fn is_language_pack_tool_call(item: &JsonRpcOutgoing) -> bool {
    matches!(
        item,
        JsonRpcOutgoing::Notification(notification)
            if notification.params["update"]["toolCallId"]
                .as_str()
                .unwrap_or_default()
                .starts_with("tool:language-pack:")
    )
}

fn is_semantic_diff(item: &JsonRpcOutgoing) -> bool {
    matches!(
        item,
        JsonRpcOutgoing::Notification(notification)
            if notification.params["update"]["sessionUpdate"] == "semantic_diff"
    )
}

fn load_fixtures() -> Vec<Fixture> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIR);
    let mut paths = fs::read_dir(&base)
        .unwrap_or_else(|error| panic!("read fixture dir {}: {error}", base.display()))
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let raw = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()));
            serde_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("parse fixture {}: {error}", path.display()))
        })
        .collect()
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}
