//! ACP `session/prompt` interception for the planning loop.
//!
//! Phase 2.6 wiring. The Sage ACP binary parses each stdio JSON-RPC
//! line; for `session/prompt` requests it routes through
//! [`try_handle_prompt`] which runs the planning loop and emits a
//! structured response. All other methods fall through to
//! `ob_poc_boundary::acp_protocol::AcpJsonRpcAgent`, preserving the
//! existing discovery / projection / KYC dry-run surface.
//!
//! Keeping the wiring as a small intercept function (not a
//! replacement dispatcher) lets the boundary's `AcpJsonRpcAgent`
//! evolve independently and avoids forking the ACP protocol surface
//! during the spike.

use ob_poc_boundary::acp_protocol::{
    AcpContentBlock, AcpPromptRequest, JsonRpcNotification, JsonRpcOutgoing, JsonRpcRequest,
    JsonRpcResponse,
};
use serde_json::{json, Value};

use crate::planning::{DraftSource, PlanningLoop, PlanningOutcome};

const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

/// If `request` is a `session/prompt`, run the planning loop and
/// return the response messages. Otherwise return `None` so the
/// caller can fall back to the boundary's `AcpJsonRpcAgent`.
///
/// Spike contract:
/// - Concatenates text blocks into one utterance (resource URIs are
///   ignored for now — Phase 4 wires `semos://...` resolution).
/// - Calls `PlanningLoop::propose_draft` once.
/// - On success emits two messages: a `session/update` notification
///   carrying the `goalProposalTrace` shape used downstream by the
///   Phase 2.9 audit emitter, and a final response with the draft.
/// - On failure (e.g. constrained-composition violation) emits a
///   JSON-RPC error using the standard `-32603` internal-error code
///   with structured `data` describing the violation.
pub async fn try_handle_prompt(
    request: &JsonRpcRequest,
    planning: &PlanningLoop,
) -> Option<Vec<JsonRpcOutgoing>> {
    if request.method != "session/prompt" {
        return None;
    }

    let id = request.id.clone();
    let parsed: AcpPromptRequest = match serde_json::from_value(request.params.clone()) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Some(vec![JsonRpcOutgoing::Response(error_response(
                id,
                INVALID_PARAMS,
                format!("session/prompt params malformed: {error}"),
                None,
            ))]);
        }
    };

    let utterance = collect_utterance(&parsed.prompt);

    match planning.propose_draft(&utterance).await {
        Ok(outcome) => Some(success_messages(id, &parsed.session_id, outcome, &utterance)),
        Err(error) => Some(vec![JsonRpcOutgoing::Response(error_response(
            id,
            INTERNAL_ERROR,
            "planning loop refused to emit a draft".to_string(),
            Some(json!({
                "reason": error.to_string(),
                "pack_id": planning.index().pack.id,
                "pack_hash": planning.index().pack_hash,
            })),
        ))]),
    }
}

fn collect_utterance(blocks: &[AcpContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            AcpContentBlock::Text { text } => Some(text.as_str()),
            // Resource URIs are not resolved in the Phase 2.6 spike —
            // Phase 4 wires `semos://...` resolution through the MCP
            // knowledge surface.
            AcpContentBlock::ResourceLink { .. } | AcpContentBlock::EmbeddedResource { .. } => {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn success_messages(
    id: Option<Value>,
    session_id: &str,
    outcome: PlanningOutcome,
    utterance: &str,
) -> Vec<JsonRpcOutgoing> {
    let source = match outcome.source {
        DraftSource::LlmTool => "llm_tool",
        DraftSource::DeterministicFallback => "deterministic_fallback",
    };
    let plan_update = JsonRpcOutgoing::Notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "session/update".to_string(),
        params: json!({
            "sessionId": session_id,
            "update": {
                "sessionUpdate": "plan",
                "persona": "sage:planning",
                "workflowPhase": "planning",
                "goalProposalTrace": {
                    "goalFrameId": outcome.goal_frame.id,
                    "packId": outcome.goal_frame.pack_id,
                    "packHash": outcome.goal_frame.pack_hash,
                    "createdAt": outcome.goal_frame.created_at.to_rfc3339(),
                    "verbFqn": outcome.verb_fqn,
                    "draftSource": source,
                    "utteranceLength": utterance.len(),
                },
                "entries": [
                    {
                        "id": "draft",
                        "status": "completed",
                        "label": format!("Drafted {} via {}", outcome.verb_fqn, source)
                    }
                ]
            }
        }),
    });

    let response = JsonRpcOutgoing::Response(success_response(
        id,
        json!({
            "stopReason": "drafted",
            "draft": {
                "goalFrameId": outcome.goal_frame.id,
                "verbFqn": outcome.verb_fqn,
                "source": source,
                "packId": outcome.goal_frame.pack_id,
                "packHash": outcome.goal_frame.pack_hash,
            }
        }),
    ));

    vec![plan_update, response]
}

fn success_response(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    }
}

fn error_response(
    id: Option<Value>,
    code: i64,
    message: String,
    data: Option<Value>,
) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(ob_poc_boundary::acp_protocol::JsonRpcError {
            code,
            message,
            data,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SessionIndex;
    use chrono::Utc;
    use ob_poc_journey::pack::load_pack_from_bytes;
    use ob_poc_types::session::kinds::WorkspaceKind;

    fn manifest_yaml() -> &'static [u8] {
        br#"
id: book-setup
name: Book Setup
version: "0.1"
description: prompt-handler test fixture
invocation_phrases: []
required_context: []
optional_context: []
workspaces:
  - cbu
allowed_verbs:
  - cbu.create
forbidden_verbs: []
required_questions: []
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done: []
progress_signals: []
"#
    }

    fn make_planning_loop() -> PlanningLoop {
        let (pack, pack_hash) = load_pack_from_bytes(manifest_yaml()).unwrap();
        let index = SessionIndex {
            pack,
            pack_hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        };
        PlanningLoop::new(index, None)
    }

    #[tokio::test]
    async fn non_prompt_method_falls_through() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: json!({}),
        };
        let outcome = try_handle_prompt(&request, &make_planning_loop()).await;
        assert!(outcome.is_none());
    }

    #[tokio::test]
    async fn prompt_emits_plan_update_and_response() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "session/prompt".to_string(),
            params: json!({
                "sessionId": "00000000-0000-0000-0000-000000000001",
                "prompt": [{"type": "text", "text": "set up a new book"}]
            }),
        };
        let outcome = try_handle_prompt(&request, &make_planning_loop())
            .await
            .expect("session/prompt must be handled");
        assert_eq!(outcome.len(), 2);
        match &outcome[0] {
            JsonRpcOutgoing::Notification(note) => {
                assert_eq!(note.method, "session/update");
                let trace = &note.params["update"]["goalProposalTrace"];
                assert_eq!(trace["verbFqn"], "cbu.create");
                assert_eq!(trace["draftSource"], "deterministic_fallback");
            }
            JsonRpcOutgoing::Response(_) => panic!("expected notification first"),
        }
        match &outcome[1] {
            JsonRpcOutgoing::Response(resp) => {
                let draft = &resp.result.as_ref().unwrap()["draft"];
                assert_eq!(draft["verbFqn"], "cbu.create");
                assert_eq!(draft["source"], "deterministic_fallback");
            }
            JsonRpcOutgoing::Notification(_) => panic!("expected response second"),
        }
    }

    #[tokio::test]
    async fn malformed_prompt_returns_invalid_params() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(3)),
            method: "session/prompt".to_string(),
            params: json!({"not": "a prompt"}),
        };
        let outcome = try_handle_prompt(&request, &make_planning_loop())
            .await
            .expect("handled");
        assert_eq!(outcome.len(), 1);
        match &outcome[0] {
            JsonRpcOutgoing::Response(resp) => {
                let err = resp.error.as_ref().expect("error response");
                assert_eq!(err.code, INVALID_PARAMS);
            }
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        }
    }
}
