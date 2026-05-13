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
use crate::repl_channel::{minimal_source_for_verb, ReplChannelClient, ValidationOutcome};

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
    channel: &dyn ReplChannelClient,
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
        Ok(outcome) => {
            // Phase 2.7: round-trip the draft through the LSP-shaped
            // channel before the response leaves the agent. The spike
            // channel is parse-only; Phase 4 swaps for a real LSP
            // client.
            let source = minimal_source_for_verb(&outcome.verb_fqn);
            let validation = channel.validate(&source).await;
            Some(success_messages(
                id,
                &parsed.session_id,
                outcome,
                &utterance,
                validation,
            ))
        }
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
    validation: ValidationOutcome,
) -> Vec<JsonRpcOutgoing> {
    let source = match outcome.source {
        DraftSource::LlmTool => "llm_tool",
        DraftSource::DeterministicFallback => "deterministic_fallback",
    };
    let validation_passed = validation.passed();
    let diagnostics_json = serde_json::to_value(&validation.diagnostics).unwrap_or(json!([]));

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
                    "validationPassed": validation_passed,
                },
                "entries": [
                    {
                        "id": "draft",
                        "status": "completed",
                        "label": format!("Drafted {} via {}", outcome.verb_fqn, source)
                    },
                    {
                        "id": "validate",
                        "status": if validation_passed { "completed" } else { "failed" },
                        "label": format!(
                            "Validated draft against REPL channel ({} diagnostics)",
                            validation.diagnostics.len()
                        )
                    }
                ]
            }
        }),
    });

    // LSP-shaped publishDiagnostics — even when there are zero
    // diagnostics the notification is emitted so the editor can
    // clear any previously-published markers.
    let diagnostics_notification = JsonRpcOutgoing::Notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/publishDiagnostics".to_string(),
        params: json!({
            "uri": format!("runbook://session/{session_id}/draft/{}", outcome.goal_frame.id),
            "diagnostics": diagnostics_json,
        }),
    });

    let response = JsonRpcOutgoing::Response(success_response(
        id,
        json!({
            "stopReason": if validation_passed { "drafted" } else { "draft_failed_validation" },
            "draft": {
                "goalFrameId": outcome.goal_frame.id,
                "verbFqn": outcome.verb_fqn,
                "source": source,
                "packId": outcome.goal_frame.pack_id,
                "packHash": outcome.goal_frame.pack_hash,
            },
            "validation": {
                "passed": validation_passed,
                "diagnosticCount": validation.diagnostics.len(),
                "source": validation.source,
            }
        }),
    ));

    vec![plan_update, diagnostics_notification, response]
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
    use crate::repl_channel::LocalParseChannel;
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
        let channel = LocalParseChannel::new();
        let outcome = try_handle_prompt(&request, &make_planning_loop(), &channel).await;
        assert!(outcome.is_none());
    }

    #[tokio::test]
    async fn prompt_emits_plan_update_diagnostics_and_response() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "session/prompt".to_string(),
            params: json!({
                "sessionId": "00000000-0000-0000-0000-000000000001",
                "prompt": [{"type": "text", "text": "set up a new book"}]
            }),
        };
        let channel = LocalParseChannel::new();
        let outcome = try_handle_prompt(&request, &make_planning_loop(), &channel)
            .await
            .expect("session/prompt must be handled");
        assert_eq!(outcome.len(), 3, "plan update + diagnostics + response");
        match &outcome[0] {
            JsonRpcOutgoing::Notification(note) => {
                assert_eq!(note.method, "session/update");
                let trace = &note.params["update"]["goalProposalTrace"];
                assert_eq!(trace["verbFqn"], "cbu.create");
                assert_eq!(trace["draftSource"], "deterministic_fallback");
                assert_eq!(trace["validationPassed"], true);
            }
            JsonRpcOutgoing::Response(_) => panic!("expected notification first"),
        }
        match &outcome[1] {
            JsonRpcOutgoing::Notification(note) => {
                assert_eq!(note.method, "textDocument/publishDiagnostics");
                assert!(note.params["uri"]
                    .as_str()
                    .unwrap()
                    .starts_with("runbook://session/"));
                assert_eq!(note.params["diagnostics"].as_array().unwrap().len(), 0);
            }
            JsonRpcOutgoing::Response(_) => panic!("expected diagnostics notification second"),
        }
        match &outcome[2] {
            JsonRpcOutgoing::Response(resp) => {
                let result = resp.result.as_ref().unwrap();
                assert_eq!(result["stopReason"], "drafted");
                assert_eq!(result["draft"]["verbFqn"], "cbu.create");
                assert_eq!(result["validation"]["passed"], true);
            }
            JsonRpcOutgoing::Notification(_) => panic!("expected response third"),
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
        let channel = LocalParseChannel::new();
        let outcome = try_handle_prompt(&request, &make_planning_loop(), &channel)
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
