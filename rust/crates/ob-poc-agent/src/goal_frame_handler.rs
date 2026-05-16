//! ACP method handlers for goal-frame lifecycle transitions.
//!
//! Phase 3.1d. Editor-driven transitions through dedicated ACP
//! custom methods. The boundary `AcpJsonRpcAgent` doesn't know about
//! goal frames — these methods are intercepted in the agent binary
//! before fall-through, the same pattern Phase 2.6 used for
//! `session/prompt`.
//!
//! ## Methods
//!
//! - `obpoc/goal_frame/get` — read the current state.
//! - `obpoc/goal_frame/confirm` — Proposed → Confirmed.
//! - `obpoc/goal_frame/refuse` — any non-terminal → Refused.
//! - `obpoc/goal_frame/start_execution` — Confirmed → InProgress.
//! - `obpoc/goal_frame/complete` — InProgress → Completed.
//!
//! Each takes `{sessionId: "uuid"}` and returns the updated frame
//! (or the existing frame for `get`). Frames in a terminal status
//! after the transition stay in the store; Phase 3.6 wires the
//! pruning policy.

use ob_poc_boundary::acp_protocol::{JsonRpcError, JsonRpcOutgoing, JsonRpcRequest, JsonRpcResponse};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::goal_frame::{GoalFrame, GoalFrameStore, GoalFrameTransitionError};

const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

/// Method names handled here. The binary checks
/// [`is_goal_frame_method`] before calling [`try_handle_goal_frame`]
/// so the dispatch tree stays explicit at the call site.
pub fn is_goal_frame_method(method: &str) -> bool {
    matches!(
        method,
        "obpoc/goal_frame/get"
            | "obpoc/goal_frame/confirm"
            | "obpoc/goal_frame/refuse"
            | "obpoc/goal_frame/refuse_draft"
            | "obpoc/goal_frame/start_execution"
            | "obpoc/goal_frame/complete"
    )
}

/// Dispatch a goal-frame method. Returns `None` if the method
/// doesn't belong to this handler; otherwise returns one
/// [`JsonRpcOutgoing::Response`].
pub async fn try_handle_goal_frame(
    request: &JsonRpcRequest,
    frames: &GoalFrameStore,
) -> Option<Vec<JsonRpcOutgoing>> {
    if !is_goal_frame_method(&request.method) {
        return None;
    }

    let id = request.id.clone();
    // The refuse_draft method needs an extra `verbFqn` field; the
    // other methods only need sessionId. Parse the broader shape
    // here and unpack per method below.
    let params: RefuseDraftParams = match serde_json::from_value(request.params.clone()) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Some(vec![JsonRpcOutgoing::Response(error_response(
                id,
                INVALID_PARAMS,
                format!("{} params malformed: {error}", request.method),
                None,
            ))]);
        }
    };
    let session_uuid = match Uuid::parse_str(&params.session_id) {
        Ok(uuid) => uuid,
        Err(error) => {
            return Some(vec![JsonRpcOutgoing::Response(error_response(
                id,
                INVALID_PARAMS,
                format!("sessionId must be a UUID: {error}"),
                None,
            ))]);
        }
    };

    let result = match request.method.as_str() {
        "obpoc/goal_frame/get" => frames.get(session_uuid).await.map(Ok),
        "obpoc/goal_frame/confirm" => frames
            .update(session_uuid, |frame| {
                let _ = frame.confirm();
            })
            .await
            .map(|frame| validate_transition(&frame, GoalFrame::confirm_check)),
        "obpoc/goal_frame/refuse" => frames
            .update(session_uuid, GoalFrame::refuse)
            .await
            .map(Ok),
        "obpoc/goal_frame/refuse_draft" => {
            let verb_fqn = match params.verb_fqn.as_deref() {
                Some(v) if !v.is_empty() => v.to_string(),
                _ => {
                    return Some(vec![JsonRpcOutgoing::Response(error_response(
                        id,
                        INVALID_PARAMS,
                        "refuse_draft requires non-empty verbFqn".to_string(),
                        None,
                    ))]);
                }
            };
            frames
                .update(session_uuid, |frame| frame.record_refused_draft(&verb_fqn))
                .await
                .map(Ok)
        }
        "obpoc/goal_frame/start_execution" => frames
            .update(session_uuid, |frame| {
                let _ = frame.start_execution();
            })
            .await
            .map(|frame| validate_transition(&frame, GoalFrame::start_execution_check)),
        "obpoc/goal_frame/complete" => frames
            .update(session_uuid, |frame| {
                let _ = frame.complete();
            })
            .await
            .map(|frame| validate_transition(&frame, GoalFrame::complete_check)),
        _ => unreachable!("guarded by is_goal_frame_method"),
    };

    let response = match result {
        Some(Ok(frame)) => JsonRpcOutgoing::Response(success_response(
            id,
            json!({
                "frame": frame,
            }),
        )),
        Some(Err(error)) => JsonRpcOutgoing::Response(error_response(
            id,
            INTERNAL_ERROR,
            format!("goal-frame transition rejected: {error}"),
            Some(json!({
                "sessionId": params.session_id,
                "transitionError": error.to_string(),
            })),
        )),
        None => JsonRpcOutgoing::Response(error_response(
            id,
            INTERNAL_ERROR,
            "no goal frame bound to session".to_string(),
            Some(json!({"sessionId": params.session_id})),
        )),
    };

    Some(vec![response])
}

/// After running an `update` that swallows the transition error
/// (because the FnOnce closure must not return), we re-check
/// whether the post-mutation status matches the expectation. The
/// boolean check functions live as `_check` helpers on `GoalFrame`
/// — this keeps the FnOnce signature clean while still surfacing
/// transition failures upstream.
fn validate_transition<F>(
    frame: &GoalFrame,
    expected: F,
) -> Result<GoalFrame, GoalFrameTransitionError>
where
    F: FnOnce(&GoalFrame) -> bool,
{
    if expected(frame) {
        Ok(frame.clone())
    } else {
        Err(GoalFrameTransitionError::InvalidFrom(frame.status))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefuseDraftParams {
    session_id: String,
    #[serde(default)]
    verb_fqn: Option<String>,
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
        error: Some(JsonRpcError {
            code,
            message,
            data,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_frame::{GoalFrame, GoalFrameStatus};
    use crate::index::SessionIndex;
    use chrono::Utc;
    use ob_poc_journey::pack::load_pack_from_bytes;
    use ob_poc_types::session::kinds::WorkspaceKind;

    fn manifest_yaml() -> &'static [u8] {
        br#"
id: book-setup
name: Book Setup
version: "0.1"
description: goal_frame_handler tests
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

    fn make_index() -> SessionIndex {
        let (pack, pack_hash) = load_pack_from_bytes(manifest_yaml()).unwrap();
        SessionIndex {
            pack,
            pack_hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        }
    }

    fn req(method: &str, sid: &str, request_id: i64) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(request_id)),
            method: method.to_string(),
            params: json!({"sessionId": sid}),
        }
    }

    async fn populate(store: &GoalFrameStore, sid: Uuid) -> String {
        let frame = GoalFrame::seed_for_spike("set up", &make_index());
        let id = frame.id.clone();
        store.put(sid, frame).await;
        id
    }

    #[tokio::test]
    async fn non_goal_frame_method_falls_through() {
        let store = GoalFrameStore::new();
        let outcome =
            try_handle_goal_frame(&req("session/prompt", "00000000-0000-0000-0000-000000000001", 1), &store).await;
        assert!(outcome.is_none());
    }

    #[tokio::test]
    async fn get_returns_current_frame() {
        let store = GoalFrameStore::new();
        let sid = Uuid::new_v4();
        let id = populate(&store, sid).await;
        let outcome =
            try_handle_goal_frame(&req("obpoc/goal_frame/get", &sid.to_string(), 1), &store)
                .await
                .unwrap();
        assert_eq!(outcome.len(), 1);
        match &outcome[0] {
            JsonRpcOutgoing::Response(resp) => {
                let result = resp.result.as_ref().unwrap();
                assert_eq!(result["frame"]["id"], id);
                assert_eq!(result["frame"]["status"], "proposed");
            }
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        }
    }

    #[tokio::test]
    async fn get_returns_error_for_unknown_session() {
        let store = GoalFrameStore::new();
        let sid = Uuid::new_v4();
        let outcome =
            try_handle_goal_frame(&req("obpoc/goal_frame/get", &sid.to_string(), 1), &store)
                .await
                .unwrap();
        match &outcome[0] {
            JsonRpcOutgoing::Response(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, INTERNAL_ERROR);
            }
            JsonRpcOutgoing::Notification(_) => panic!(),
        }
    }

    #[tokio::test]
    async fn confirm_advances_status() {
        let store = GoalFrameStore::new();
        let sid = Uuid::new_v4();
        populate(&store, sid).await;
        let outcome =
            try_handle_goal_frame(&req("obpoc/goal_frame/confirm", &sid.to_string(), 1), &store)
                .await
                .unwrap();
        match &outcome[0] {
            JsonRpcOutgoing::Response(resp) => {
                let frame = &resp.result.as_ref().unwrap()["frame"];
                assert_eq!(frame["status"], "confirmed");
            }
            JsonRpcOutgoing::Notification(_) => panic!(),
        }
        let stored = store.get(sid).await.unwrap();
        assert_eq!(stored.status, GoalFrameStatus::Confirmed);
    }

    #[tokio::test]
    async fn full_happy_path_through_methods() {
        let store = GoalFrameStore::new();
        let sid = Uuid::new_v4();
        populate(&store, sid).await;
        try_handle_goal_frame(&req("obpoc/goal_frame/confirm", &sid.to_string(), 1), &store)
            .await
            .unwrap();
        try_handle_goal_frame(
            &req("obpoc/goal_frame/start_execution", &sid.to_string(), 2),
            &store,
        )
        .await
        .unwrap();
        try_handle_goal_frame(&req("obpoc/goal_frame/complete", &sid.to_string(), 3), &store)
            .await
            .unwrap();
        let stored = store.get(sid).await.unwrap();
        assert_eq!(stored.status, GoalFrameStatus::Completed);
    }

    #[tokio::test]
    async fn refuse_draft_appends_to_refused_drafts_without_changing_status() {
        let store = GoalFrameStore::new();
        let sid = Uuid::new_v4();
        populate(&store, sid).await;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "obpoc/goal_frame/refuse_draft".to_string(),
            params: json!({"sessionId": sid.to_string(), "verbFqn": "cbu.create"}),
        };
        let outcome = try_handle_goal_frame(&request, &store).await.unwrap();
        match &outcome[0] {
            JsonRpcOutgoing::Response(resp) => {
                let frame = &resp.result.as_ref().unwrap()["frame"];
                assert_eq!(frame["refused_drafts"][0], "cbu.create");
                assert_eq!(frame["status"], "proposed");
            }
            JsonRpcOutgoing::Notification(_) => panic!(),
        }
    }

    #[tokio::test]
    async fn refuse_draft_without_verb_fqn_returns_invalid_params() {
        let store = GoalFrameStore::new();
        let sid = Uuid::new_v4();
        populate(&store, sid).await;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "obpoc/goal_frame/refuse_draft".to_string(),
            params: json!({"sessionId": sid.to_string()}),
        };
        let outcome = try_handle_goal_frame(&request, &store).await.unwrap();
        match &outcome[0] {
            JsonRpcOutgoing::Response(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
            }
            JsonRpcOutgoing::Notification(_) => panic!(),
        }
    }

    #[tokio::test]
    async fn confirm_on_refused_frame_returns_transition_error() {
        let store = GoalFrameStore::new();
        let sid = Uuid::new_v4();
        populate(&store, sid).await;
        try_handle_goal_frame(&req("obpoc/goal_frame/refuse", &sid.to_string(), 1), &store)
            .await
            .unwrap();
        let outcome =
            try_handle_goal_frame(&req("obpoc/goal_frame/confirm", &sid.to_string(), 2), &store)
                .await
                .unwrap();
        match &outcome[0] {
            JsonRpcOutgoing::Response(resp) => {
                let err = resp.error.as_ref().unwrap();
                assert_eq!(err.code, INTERNAL_ERROR);
                assert!(err.message.contains("transition rejected"));
            }
            JsonRpcOutgoing::Notification(_) => panic!(),
        }
    }
}
