//! ACP prompt harness for KYC-adjacent multi-verb confusion.

use ob_poc::acp_protocol::{AcpJsonRpcAgent, JsonRpcOutgoing, JsonRpcRequest};
use serde_json::{json, Value};
use uuid::{uuid, Uuid};

const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");
const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");

#[derive(Debug, Clone)]
struct Scenario {
    name: &'static str,
    utterance: &'static str,
    current_state: &'static str,
    expected_status: &'static str,
    expected_transition: Option<&'static str>,
    expected_refusal: Option<&'static str>,
    expect_language_loop: bool,
}

#[test]
fn workflow_acp_prompt_multiverb_confusion_reports_routing_precision() {
    let scenarios = scenarios();
    let mut total = 0usize;
    let mut language_loop_routed = 0usize;
    let mut dry_run_valid = 0usize;
    let mut structured_refusal = 0usize;
    let mut pending_question = 0usize;
    let mut prose_only_failure = 0usize;
    let mut unexpected_fallback = 0usize;

    for scenario in &scenarios {
        total += 1;
        let mut agent = AcpJsonRpcAgent::new();
        let outgoing = agent.handle_request(request(
            total as i64,
            "session/prompt",
            prompt_params(scenario),
        ));
        let routed = outgoing.iter().any(is_language_pack_tool_call);
        if routed {
            language_loop_routed += 1;
        }
        assert_eq!(
            routed, scenario.expect_language_loop,
            "{} language-loop routing",
            scenario.name
        );

        let response = response_result(&outgoing)
            .unwrap_or_else(|| panic!("{}: expected JSON-RPC response", scenario.name));
        match response["status"].as_str() {
            Some("dry_run_validated") => {
                assert_eq!(
                    scenario.expected_status, "dry_run_validated",
                    "{}",
                    scenario.name
                );
                dry_run_valid += 1;
                assert!(outgoing.iter().any(is_semantic_diff), "{}", scenario.name);
                if let Some(expected_transition) = scenario.expected_transition {
                    assert_eq!(
                        response["output"]["dry_run"]["transition_ref"], expected_transition,
                        "{}",
                        scenario.name
                    );
                }
            }
            Some("structured_refusal") => {
                assert_eq!(
                    scenario.expected_status, "structured_refusal",
                    "{}",
                    scenario.name
                );
                structured_refusal += 1;
                assert!(
                    !outgoing.iter().any(is_semantic_diff),
                    "{} produced semantic diff after refusal",
                    scenario.name
                );
                if let Some(expected_refusal) = scenario.expected_refusal {
                    assert_eq!(
                        response["refusal"]["refusal_code"], expected_refusal,
                        "{}",
                        scenario.name
                    );
                }
            }
            Some("pending_question") => {
                assert_eq!(
                    scenario.expected_status, "pending_question",
                    "{}",
                    scenario.name
                );
                pending_question += 1;
                assert!(
                    !outgoing.iter().any(is_semantic_diff),
                    "{} produced semantic diff after pending question",
                    scenario.name
                );
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
    println!(
        "  ACP PROMPT MULTI-VERB CONFUSION HARNESS -- {} scenarios",
        total
    );
    println!("=======================================================================");
    println!(
        "  Language-loop routed:     {}/{} ({:.1}%)",
        language_loop_routed,
        total,
        pct(language_loop_routed, total)
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

    assert_eq!(language_loop_routed, 5);
    assert_eq!(dry_run_valid, 4);
    assert_eq!(structured_refusal, 1);
    assert_eq!(pending_question, 5);
    assert_eq!(prose_only_failure, 0);
    assert_eq!(unexpected_fallback, 0);
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "read-create-update wording still advances to discovery",
            utterance: "Read the KYC case, create any missing checklist, then move the case to discovery with evidence sha256:evidence",
            current_state: "INTAKE",
            expected_status: "dry_run_validated",
            expected_transition: Some("kyc-case.intake-to-discovery"),
            expected_refusal: None,
            expect_language_loop: true,
        },
        Scenario {
            name: "screening-document wording still advances to assessment",
            utterance: "After screening and document collection, advance the KYC case to assessment with evidence sha256:evidence",
            current_state: "DISCOVERY",
            expected_status: "dry_run_validated",
            expected_transition: Some("kyc-case.discovery-to-assessment"),
            expected_refusal: None,
            expect_language_loop: true,
        },
        Scenario {
            name: "explicit transition disambiguates neighboring verbs",
            utterance: "Use transition kyc-case.discovery-to-assessment after reading the case and checking documents with evidence sha256:evidence",
            current_state: "DISCOVERY",
            expected_status: "dry_run_validated",
            expected_transition: Some("kyc-case.discovery-to-assessment"),
            expected_refusal: None,
            expect_language_loop: true,
        },
        Scenario {
            name: "target without evidence refuses structurally",
            utterance: "Move the KYC case to discovery after checking screening",
            current_state: "INTAKE",
            expected_status: "structured_refusal",
            expected_transition: None,
            expected_refusal: Some("missing_evidence_digest"),
            expect_language_loop: true,
        },
        Scenario {
            name: "set status wording routes to assessment",
            utterance: "Set status for the KYC case to assessment with evidence sha256:evidence",
            current_state: "DISCOVERY",
            expected_status: "dry_run_validated",
            expected_transition: Some("kyc-case.discovery-to-assessment"),
            expected_refusal: None,
            expect_language_loop: true,
        },
        Scenario {
            name: "read-only case status is not converted to update-status",
            utterance: "Read the current KYC case status and summarize the evidence",
            current_state: "DISCOVERY",
            expected_status: "pending_question",
            expected_transition: None,
            expected_refusal: None,
            expect_language_loop: false,
        },
        Scenario {
            name: "create case request is held for verb clarification",
            utterance: "Create a new KYC case for this entity and collect documents",
            current_state: "INTAKE",
            expected_status: "pending_question",
            expected_transition: None,
            expected_refusal: None,
            expect_language_loop: false,
        },
        Scenario {
            name: "screening-only prompt is held for clarification",
            utterance: "Run screening and show adverse media hits for the KYC case",
            current_state: "DISCOVERY",
            expected_status: "pending_question",
            expected_transition: None,
            expected_refusal: None,
            expect_language_loop: false,
        },
        Scenario {
            name: "update intent without target state asks question",
            utterance: "Update status on the KYC case after document review with evidence sha256:evidence",
            current_state: "DISCOVERY",
            expected_status: "pending_question",
            expected_transition: None,
            expected_refusal: None,
            expect_language_loop: false,
        },
        Scenario {
            name: "progress wording without target state asks question",
            utterance: "Progress the due diligence case after the document and screening work",
            current_state: "DISCOVERY",
            expected_status: "pending_question",
            expected_transition: None,
            expected_refusal: None,
            expect_language_loop: false,
        },
    ]
}

fn prompt_params(scenario: &Scenario) -> Value {
    let case_state = json!({
        "case_state": {
            "subject_id": CASE_ID,
            "current_state": scenario.current_state,
            "configuration_version": "config-1",
            "state_snapshot_id": "snapshot-1"
        }
    });

    json!({
        "sessionId": SESSION_ID.to_string(),
        "prompt": [
            {"type": "text", "text": scenario.utterance},
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

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}
