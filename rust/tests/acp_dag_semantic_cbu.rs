use ob_poc::acp_protocol::{AcpJsonRpcAgent, JsonRpcOutgoing, JsonRpcRequest};
use regex::Regex;
use serde_json::{json, Value};
use uuid::Uuid;

fn cbu_phrase_scenarios() -> Vec<(String, String)> {
    let source = include_str!("helpers/cbu_phrase_scenarios.rs");
    let pattern = Regex::new(
        r#"TestScenario::(?:matched|safety_first)\(\s*"[^"]+"\s*,\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,?\s*\)"#,
    )
    .expect("fixture regex compiles");

    pattern
        .captures_iter(source)
        .map(|captures| (captures[1].to_string(), captures[2].to_string()))
        .collect()
}

fn request(id: usize, method: &str, params: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}

fn response_result(outgoing: &[JsonRpcOutgoing]) -> &Value {
    outgoing
        .iter()
        .find_map(|item| match item {
            JsonRpcOutgoing::Response(response) => response.result.as_ref(),
            JsonRpcOutgoing::Notification(_) => None,
        })
        .expect("ACP response result")
}

fn agent_messages(outgoing: &[JsonRpcOutgoing]) -> String {
    outgoing
        .iter()
        .filter_map(|item| match item {
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["sessionUpdate"] == "agent_message_chunk" =>
            {
                notification.params["update"]["content"]["text"]
                    .as_str()
                    .map(str::to_string)
            }
            JsonRpcOutgoing::Notification(_) | JsonRpcOutgoing::Response(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn acp_session_prompt_routes_all_cbu_phrase_scenarios_to_structured_dag_semantic() {
    let scenarios = cbu_phrase_scenarios();
    assert_eq!(scenarios.len(), 80, "CBU fixture count changed");

    let mut agent = AcpJsonRpcAgent::new();
    let session_id = Uuid::new_v4();
    let mut expected_candidate_hits = 0;

    for (index, (phrase, expected_verb)) in scenarios.iter().enumerate() {
        let outgoing = agent.handle_request(request(
            index + 1,
            "session/prompt",
            json!({
                "sessionId": session_id.to_string(),
                "prompt": [{"type": "text", "text": phrase}]
            }),
        ));

        assert_eq!(
            outgoing.len(),
            4,
            "unexpected ACP update shape for {phrase:?}"
        );
        let result = response_result(&outgoing);
        assert_eq!(result["stopReason"], "end_turn", "{phrase:?}");
        assert!(
            result["status"] == "pending_question"
                || result["status"] == "dag_semantic_proposal"
                || result["status"] == "structured_refusal",
            "unexpected ACP status for {phrase:?}: {:?}",
            result["status"]
        );
        assert!(
            result["dag_semantic"].is_object(),
            "missing DAG semantic payload for {phrase:?}"
        );
        assert_eq!(
            result["observability"]["conversationEfficiency"]["proseOnlyFailure"], false,
            "{phrase:?}"
        );
        assert!(
            !agent_messages(&outgoing).contains("Received ACP prompt."),
            "generic ACP acknowledgement leaked for {phrase:?}"
        );

        let candidates = result["dag_semantic"]["top_candidates"]
            .as_array()
            .expect("top candidates");
        let expected_or_safe_delete = candidates.iter().any(|candidate| {
            candidate["verb"] == *expected_verb
                || (expected_verb.starts_with("cbu.delete") && candidate["verb"] == "cbu.delete")
        });
        if expected_or_safe_delete {
            expected_candidate_hits += 1;
        } else {
            panic!(
                "expected {expected_verb} in ACP candidates for {phrase:?}, got {:?}",
                candidates
                    .iter()
                    .map(|candidate| candidate["verb"].clone())
                    .collect::<Vec<_>>()
            );
        }
    }

    assert_eq!(expected_candidate_hits, 80);
}
