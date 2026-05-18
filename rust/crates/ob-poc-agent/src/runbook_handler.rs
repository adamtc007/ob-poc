//! ACP method handlers for the LSP-shaped runbook lifecycle.
//!
//! Phase 4.4. Editor / agent traffic is routed through five custom
//! methods that mirror LSP's `textDocument/*` shape:
//!
//! - `runbook/didOpen`           — `{uri, source}` → `{ok: true}`
//! - `runbook/didChange`         — `{uri, source}` → `{ok: true}`
//! - `runbook/didClose`          — `{uri}` → `{ok: true}`
//! - `runbook/validateOnly`      — `{uri}` → `{validation}`
//! - `runbook/validateAndExecute`— `{uri}` → `{outcome}`
//!
//! All five methods delegate to a [`ReplChannelClient`]; the
//! `validate_and_execute` path refuses with `ApprovalRequired`
//! (the runbook channel never runs mutations — V&S §6.4 / §7.2).

use ob_poc_boundary::acp_protocol::{
    JsonRpcError, JsonRpcOutgoing, JsonRpcRequest, JsonRpcResponse,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::repl_channel::{ReplChannelClient, RunbookChannelError};

const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

pub fn is_runbook_method(method: &str) -> bool {
    matches!(
        method,
        "runbook/didOpen"
            | "runbook/didChange"
            | "runbook/didClose"
            | "runbook/validateOnly"
            | "runbook/validateAndExecute"
    )
}

/// Dispatch a runbook lifecycle method.
pub async fn try_handle_runbook(
    request: &JsonRpcRequest,
    channel: &dyn ReplChannelClient,
) -> Option<Vec<JsonRpcOutgoing>> {
    if !is_runbook_method(&request.method) {
        return None;
    }

    let id = request.id.clone();
    let response = match request.method.as_str() {
        "runbook/didOpen" => handle_did_open(id, request.params.clone(), channel).await,
        "runbook/didChange" => handle_did_change(id, request.params.clone(), channel).await,
        "runbook/didClose" => handle_did_close(id, request.params.clone(), channel).await,
        "runbook/validateOnly" => handle_validate_only(id, request.params.clone(), channel).await,
        "runbook/validateAndExecute" => {
            handle_validate_and_execute(id, request.params.clone(), channel).await
        }
        _ => unreachable!("guarded by is_runbook_method"),
    };

    Some(vec![JsonRpcOutgoing::Response(response)])
}

#[derive(Debug, Deserialize)]
struct OpenChangeParams {
    uri: String,
    source: String,
}

#[derive(Debug, Deserialize)]
struct UriOnlyParams {
    uri: String,
}

async fn handle_did_open(
    id: Option<Value>,
    params: Value,
    channel: &dyn ReplChannelClient,
) -> JsonRpcResponse {
    let parsed: OpenChangeParams = match serde_json::from_value(params) {
        Ok(parsed) => parsed,
        Err(error) => {
            return error_response(
                id,
                INVALID_PARAMS,
                format!("runbook/didOpen params malformed: {error}"),
                None,
            )
        }
    };
    match channel.open_runbook(&parsed.uri, &parsed.source).await {
        Ok(()) => success_response(id, json!({"ok": true, "uri": parsed.uri})),
        Err(error) => transport_error(id, "runbook/didOpen", &error),
    }
}

async fn handle_did_change(
    id: Option<Value>,
    params: Value,
    channel: &dyn ReplChannelClient,
) -> JsonRpcResponse {
    let parsed: OpenChangeParams = match serde_json::from_value(params) {
        Ok(parsed) => parsed,
        Err(error) => {
            return error_response(
                id,
                INVALID_PARAMS,
                format!("runbook/didChange params malformed: {error}"),
                None,
            )
        }
    };
    match channel.change_runbook(&parsed.uri, &parsed.source).await {
        Ok(()) => success_response(id, json!({"ok": true, "uri": parsed.uri})),
        Err(error) => transport_error(id, "runbook/didChange", &error),
    }
}

async fn handle_did_close(
    id: Option<Value>,
    params: Value,
    channel: &dyn ReplChannelClient,
) -> JsonRpcResponse {
    let parsed: UriOnlyParams = match serde_json::from_value(params) {
        Ok(parsed) => parsed,
        Err(error) => {
            return error_response(
                id,
                INVALID_PARAMS,
                format!("runbook/didClose params malformed: {error}"),
                None,
            )
        }
    };
    match channel.close_runbook(&parsed.uri).await {
        Ok(()) => success_response(id, json!({"ok": true, "uri": parsed.uri})),
        Err(error) => transport_error(id, "runbook/didClose", &error),
    }
}

async fn handle_validate_only(
    id: Option<Value>,
    params: Value,
    channel: &dyn ReplChannelClient,
) -> JsonRpcResponse {
    let parsed: UriOnlyParams = match serde_json::from_value(params) {
        Ok(parsed) => parsed,
        Err(error) => {
            return error_response(
                id,
                INVALID_PARAMS,
                format!("runbook/validateOnly params malformed: {error}"),
                None,
            )
        }
    };
    match channel.validate_only(&parsed.uri).await {
        Ok(validation) => success_response(
            id,
            json!({
                "uri": parsed.uri,
                "validation": {
                    "passed": validation.passed(),
                    "diagnostics": validation.diagnostics,
                    "source": validation.source,
                }
            }),
        ),
        Err(error) => transport_error(id, "runbook/validateOnly", &error),
    }
}

async fn handle_validate_and_execute(
    id: Option<Value>,
    params: Value,
    channel: &dyn ReplChannelClient,
) -> JsonRpcResponse {
    let parsed: UriOnlyParams = match serde_json::from_value(params) {
        Ok(parsed) => parsed,
        Err(error) => {
            return error_response(
                id,
                INVALID_PARAMS,
                format!("runbook/validateAndExecute params malformed: {error}"),
                None,
            )
        }
    };
    match channel.validate_and_execute(&parsed.uri).await {
        Ok(outcome) => success_response(
            id,
            json!({
                "uri": parsed.uri,
                "outcome": outcome,
            }),
        ),
        Err(error) => transport_error(id, "runbook/validateAndExecute", &error),
    }
}

fn transport_error(
    id: Option<Value>,
    method: &str,
    error: &RunbookChannelError,
) -> JsonRpcResponse {
    let kind = match error {
        RunbookChannelError::NotOpen(_) => "not_open",
        RunbookChannelError::AlreadyOpen(_) => "already_open",
        RunbookChannelError::Transport(_) => "transport",
    };
    error_response(
        id,
        INTERNAL_ERROR,
        format!("{method} failed: {error}"),
        Some(json!({"errorKind": kind})),
    )
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
    use crate::repl_channel::LocalRunbookChannel;

    fn req(id: i64, method: &str, params: Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(id)),
            method: method.to_string(),
            params,
        }
    }

    fn extract_response(outcome: &[JsonRpcOutgoing]) -> &JsonRpcResponse {
        assert_eq!(outcome.len(), 1);
        match &outcome[0] {
            JsonRpcOutgoing::Response(resp) => resp,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        }
    }

    #[tokio::test]
    async fn non_runbook_method_falls_through() {
        let channel = LocalRunbookChannel::new();
        let outcome = try_handle_runbook(&req(1, "session/prompt", json!({})), &channel).await;
        assert!(outcome.is_none());
    }

    #[tokio::test]
    async fn open_then_validate_only_returns_passed_outcome() {
        let channel = LocalRunbookChannel::new();
        let open = try_handle_runbook(
            &req(
                1,
                "runbook/didOpen",
                json!({"uri": "u", "source": "(cbu.create)"}),
            ),
            &channel,
        )
        .await
        .unwrap();
        let resp = extract_response(&open);
        assert!(resp.error.is_none(), "didOpen must succeed");

        let validate = try_handle_runbook(
            &req(2, "runbook/validateOnly", json!({"uri": "u"})),
            &channel,
        )
        .await
        .unwrap();
        let resp = extract_response(&validate);
        let result = resp.result.as_ref().unwrap();
        assert_eq!(result["validation"]["passed"], true);
    }

    #[tokio::test]
    async fn change_updates_source_seen_by_validate_only() {
        let channel = LocalRunbookChannel::new();
        try_handle_runbook(
            &req(
                1,
                "runbook/didOpen",
                json!({"uri": "u", "source": "(cbu.create)"}),
            ),
            &channel,
        )
        .await
        .unwrap();
        try_handle_runbook(
            &req(
                2,
                "runbook/didChange",
                json!({"uri": "u", "source": "garbage"}),
            ),
            &channel,
        )
        .await
        .unwrap();
        let validate = try_handle_runbook(
            &req(3, "runbook/validateOnly", json!({"uri": "u"})),
            &channel,
        )
        .await
        .unwrap();
        let resp = extract_response(&validate);
        assert_eq!(resp.result.as_ref().unwrap()["validation"]["passed"], false);
    }

    #[tokio::test]
    async fn validate_and_execute_refuses_with_approval_required() {
        let channel = LocalRunbookChannel::new();
        try_handle_runbook(
            &req(
                1,
                "runbook/didOpen",
                json!({"uri": "u", "source": "(cbu.create)"}),
            ),
            &channel,
        )
        .await
        .unwrap();
        let outcome = try_handle_runbook(
            &req(2, "runbook/validateAndExecute", json!({"uri": "u"})),
            &channel,
        )
        .await
        .unwrap();
        let resp = extract_response(&outcome);
        let result = resp.result.as_ref().unwrap();
        assert_eq!(result["outcome"]["outcome_kind"], "refused");
        assert_eq!(result["outcome"]["reason"], "approval_required");
    }

    #[tokio::test]
    async fn validate_only_on_unopened_uri_returns_not_open() {
        let channel = LocalRunbookChannel::new();
        let outcome = try_handle_runbook(
            &req(1, "runbook/validateOnly", json!({"uri": "u"})),
            &channel,
        )
        .await
        .unwrap();
        let resp = extract_response(&outcome);
        let err = resp.error.as_ref().unwrap();
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(err.data.as_ref().unwrap()["errorKind"], "not_open");
    }

    #[tokio::test]
    async fn close_then_validate_returns_not_open() {
        let channel = LocalRunbookChannel::new();
        try_handle_runbook(
            &req(
                1,
                "runbook/didOpen",
                json!({"uri": "u", "source": "(cbu.create)"}),
            ),
            &channel,
        )
        .await
        .unwrap();
        try_handle_runbook(&req(2, "runbook/didClose", json!({"uri": "u"})), &channel)
            .await
            .unwrap();
        let outcome = try_handle_runbook(
            &req(3, "runbook/validateOnly", json!({"uri": "u"})),
            &channel,
        )
        .await
        .unwrap();
        let resp = extract_response(&outcome);
        assert_eq!(
            resp.error.as_ref().unwrap().data.as_ref().unwrap()["errorKind"],
            "not_open"
        );
    }

    #[tokio::test]
    async fn malformed_did_open_returns_invalid_params() {
        let channel = LocalRunbookChannel::new();
        let outcome = try_handle_runbook(&req(1, "runbook/didOpen", json!({"uri": "u"})), &channel)
            .await
            .unwrap();
        let resp = extract_response(&outcome);
        assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
    }
}
