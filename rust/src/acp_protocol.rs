//! Agent Client Protocol JSON-RPC surface.
//!
//! This is the launchable ACP boundary for Zed and other ACP clients. It keeps
//! transport concerns separate from the domain adapter in `acp.rs`: protocol
//! methods create/load/cancel sessions and dispatch allowed ob-poc extension
//! calls into the same dry-run/context functions used by the HTTP API.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use crate::acp::{self, AcpAdapterKind, AcpSession};
use crate::runbook::KycUpdateStatusDryRunInput;

pub const ACP_PROTOCOL_VERSION: &str = "0.4.3";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcOutgoing {
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(
        id: Option<Value>,
        code: i64,
        message: impl Into<String>,
        data: Option<Value>,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpInitializeResponse {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "agentCapabilities")]
    pub agent_capabilities: AcpAgentCapabilities,
    #[serde(rename = "authMethods")]
    pub auth_methods: Vec<AcpAuthMethod>,
    #[serde(rename = "agentInfo")]
    pub agent_info: AcpAgentInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpAgentCapabilities {
    #[serde(rename = "loadSession")]
    pub load_session: bool,
    #[serde(rename = "promptCapabilities")]
    pub prompt_capabilities: AcpPromptCapabilities,
    #[serde(rename = "sessionCapabilities")]
    pub session_capabilities: AcpSessionCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPromptCapabilities {
    pub image: bool,
    pub audio: bool,
    #[serde(rename = "embeddedContext")]
    pub embedded_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpSessionCapabilities {
    pub close: bool,
    pub list: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpAuthMethod {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpAgentInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcpSessionRecord {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub title: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcpContent {
    #[serde(flatten)]
    pub content: AcpContentBlock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AcpContentBlock {
    Text {
        text: String,
    },
    ResourceLink {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpPromptRequest {
    pub session_id: String,
    #[serde(default)]
    pub prompt: Vec<AcpContentBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionIdRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpCloseSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpCancelNotification {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcpContextExtensionRequest {
    #[serde(default = "default_adapter")]
    pub adapter: AcpAdapterKind,
    pub probe_id: String,
    pub subject_kind: String,
    pub subject_id: String,
    #[serde(default)]
    pub context: BTreeMap<String, Value>,
    #[serde(default)]
    pub observations: Vec<sem_os_core::domain_pack::DiscoveryObservation>,
    #[serde(default)]
    pub provenance: Vec<sem_os_core::domain_pack::DiscoveryProvenance>,
    #[serde(default)]
    pub first_class_state_mutated: bool,
}

fn default_adapter() -> AcpAdapterKind {
    AcpAdapterKind::Zed
}

#[derive(Debug, Default)]
pub struct AcpJsonRpcAgent {
    sessions: BTreeMap<Uuid, AcpSession>,
    cancelled_sessions: BTreeSet<Uuid>,
}

impl AcpJsonRpcAgent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_line(&mut self, line: &str) -> Vec<JsonRpcOutgoing> {
        let request: JsonRpcRequest = match serde_json::from_str(line) {
            Ok(request) => request,
            Err(error) => {
                return vec![JsonRpcOutgoing::Response(JsonRpcResponse::error(
                    None,
                    PARSE_ERROR,
                    error.to_string(),
                    None,
                ))]
            }
        };

        if request.jsonrpc != "2.0" {
            return vec![JsonRpcOutgoing::Response(JsonRpcResponse::error(
                request.id,
                INVALID_REQUEST,
                "ACP messages must use JSON-RPC 2.0",
                None,
            ))];
        }

        self.handle_request(request)
    }

    pub fn handle_request(&mut self, request: JsonRpcRequest) -> Vec<JsonRpcOutgoing> {
        let id = request.id.clone();
        match request.method.as_str() {
            "initialize" => self.response(id, self.initialize_result()),
            "authenticate" => self.response(id, json!({})),
            "session/new" => self.session_new(id),
            "session/load" => self.session_load(id, request.params),
            "session/list" => self.session_list(id),
            "session/close" => self.session_close(id, request.params),
            "session/cancel" => self.session_cancel(request.params),
            "session/prompt" => self.session_prompt(id, request.params),
            "obpoc/policy" => self.obpoc_policy(id, request.params),
            "obpoc/context" => self.obpoc_context(id, request.params),
            "obpoc/kyc_update_status_dry_run" => self.obpoc_kyc_dry_run(id, request.params),
            "obpoc/mutation" => self.error(
                id,
                INVALID_REQUEST,
                "ACP mutation is not supported; use dry-run, approval, and runbook gates",
                Some(json!({"capability": "none"})),
            ),
            _ => self.error(
                id,
                METHOD_NOT_FOUND,
                format!("Unknown ACP method: {}", request.method),
                None,
            ),
        }
    }

    fn initialize_result(&self) -> Value {
        serde_json::to_value(AcpInitializeResponse {
            protocol_version: ACP_PROTOCOL_VERSION.to_string(),
            agent_capabilities: AcpAgentCapabilities {
                load_session: true,
                prompt_capabilities: AcpPromptCapabilities {
                    image: false,
                    audio: false,
                    embedded_context: true,
                },
                session_capabilities: AcpSessionCapabilities {
                    close: true,
                    list: true,
                },
            },
            auth_methods: vec![AcpAuthMethod {
                kind: "agent".to_string(),
                id: "ob-poc-local".to_string(),
                name: "ob-poc local governance".to_string(),
                description: Some("Local SemOS/ob-poc ACP adapter".to_string()),
            }],
            agent_info: AcpAgentInfo {
                name: "ob-poc-acp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
        .expect("ACP initialize result serializes")
    }

    fn session_new(&mut self, id: Option<Value>) -> Vec<JsonRpcOutgoing> {
        let session_id = Uuid::new_v4();
        let session = acp::open_acp_session(session_id, AcpAdapterKind::Zed);
        self.sessions.insert(session_id, session);
        let now = Utc::now().to_rfc3339();
        self.response(
            id,
            json!({
                "sessionId": session_id.to_string(),
                "session": self.session_record(session_id, "ob-poc ACP session", &now, &now),
                "modeState": {
                    "currentModeId": "sage",
                    "availableModes": [
                        {"id": "sage", "name": "Sage Discovery", "description": "Read-only discovery and context assembly"},
                        {"id": "dsl-coder", "name": "DSL Coder Dry Run", "description": "Workbook dry-run validation without mutation"}
                    ]
                }
            }),
        )
    }

    fn session_load(&mut self, id: Option<Value>, params: Value) -> Vec<JsonRpcOutgoing> {
        let request: AcpSessionIdRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let session_id = match Uuid::parse_str(&request.session_id) {
            Ok(session_id) => session_id,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        self.sessions
            .entry(session_id)
            .or_insert_with(|| acp::open_acp_session(session_id, AcpAdapterKind::Zed));
        let now = Utc::now().to_rfc3339();
        self.response(
            id,
            json!({
                "sessionId": session_id.to_string(),
                "session": self.session_record(session_id, "ob-poc ACP session", &now, &now),
            }),
        )
    }

    fn session_list(&self, id: Option<Value>) -> Vec<JsonRpcOutgoing> {
        let now = Utc::now().to_rfc3339();
        let sessions = self
            .sessions
            .keys()
            .map(|session_id| self.session_record(*session_id, "ob-poc ACP session", &now, &now))
            .collect::<Vec<_>>();
        self.response(id, json!({"sessions": sessions, "nextCursor": null}))
    }

    fn session_close(&mut self, id: Option<Value>, params: Value) -> Vec<JsonRpcOutgoing> {
        let request: AcpCloseSessionRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let session_id = match Uuid::parse_str(&request.session_id) {
            Ok(session_id) => session_id,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        if let Some(session) = self.sessions.get_mut(&session_id) {
            acp::close_acp_session(session);
        }
        self.cancelled_sessions.remove(&session_id);
        self.response(id, json!({}))
    }

    fn session_cancel(&mut self, params: Value) -> Vec<JsonRpcOutgoing> {
        if let Ok(request) = serde_json::from_value::<AcpCancelNotification>(params) {
            if let Ok(session_id) = Uuid::parse_str(&request.session_id) {
                self.cancelled_sessions.insert(session_id);
            }
        }
        vec![]
    }

    fn session_prompt(&mut self, id: Option<Value>, params: Value) -> Vec<JsonRpcOutgoing> {
        let request: AcpPromptRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let session_id = match Uuid::parse_str(&request.session_id) {
            Ok(session_id) => session_id,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        self.sessions
            .entry(session_id)
            .or_insert_with(|| acp::open_acp_session(session_id, AcpAdapterKind::Zed));

        if self.cancelled_sessions.remove(&session_id) {
            return self.response(id, json!({"stopReason": "cancelled"}));
        }

        let prompt_text = request
            .prompt
            .iter()
            .filter_map(|block| match block {
                AcpContentBlock::Text { text } => Some(text.as_str()),
                AcpContentBlock::ResourceLink { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let text = if prompt_text.trim().is_empty() {
            "ACP session is open. Available ob-poc operations: context assembly, KYC update-status dry-run, restricted mutation refusal.".to_string()
        } else {
            format!(
                "Received ACP prompt. This adapter is governance-gated: discovery and dry-run are available, direct mutation is refused. Prompt hash input length: {} bytes.",
                prompt_text.len()
            )
        };
        vec![
            JsonRpcOutgoing::Notification(JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": text}
                    }
                }),
            }),
            JsonRpcOutgoing::Response(JsonRpcResponse::success(
                id,
                json!({"stopReason": "end_turn"}),
            )),
        ]
    }

    fn obpoc_context(&mut self, id: Option<Value>, params: Value) -> Vec<JsonRpcOutgoing> {
        let session_id = match extract_session_id(&params) {
            Ok(session_id) => session_id,
            Err(error) => return self.error(id, INVALID_PARAMS, error, None),
        };
        let request: AcpContextExtensionRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let manifest = match load_ob_poc_kyc_domain_pack() {
            Ok(manifest) => manifest,
            Err(error) => return self.acp_error(id, error),
        };
        let session = self
            .sessions
            .entry(session_id)
            .or_insert_with(|| acp::open_acp_session(session_id, request.adapter))
            .clone();
        let subject = sem_os_core::domain_pack::DiscoverySubject {
            subject_kind: request.subject_kind,
            subject_id: request.subject_id,
        };
        let discovery_request = sem_os_core::domain_pack::DiscoveryRequest {
            pack_id: manifest.pack_id.clone(),
            probe_id: request.probe_id.clone(),
            subject: subject.clone(),
            context: request.context,
        };
        let discovery_response = sem_os_core::domain_pack::DiscoveryResponse {
            probe_id: request.probe_id,
            subject,
            observations: request.observations,
            provenance: request.provenance,
            first_class_state_mutated: request.first_class_state_mutated,
        };
        match acp::assemble_sage_context_for_acp(
            &session,
            &manifest,
            discovery_request,
            discovery_response,
        ) {
            Ok(bundle) => self.response(id, json!({"bundle": bundle})),
            Err(error) => self.acp_error(id, error),
        }
    }

    fn obpoc_policy(&mut self, id: Option<Value>, params: Value) -> Vec<JsonRpcOutgoing> {
        let session_id = match extract_session_id(&params) {
            Ok(session_id) => session_id,
            Err(error) => return self.error(id, INVALID_PARAMS, error, None),
        };
        let adapter = params
            .get("adapter")
            .cloned()
            .and_then(|value| serde_json::from_value::<AcpAdapterKind>(value).ok())
            .unwrap_or(AcpAdapterKind::Zed);
        let manifest = match load_ob_poc_kyc_domain_pack() {
            Ok(manifest) => manifest,
            Err(error) => return self.acp_error(id, error),
        };
        let session = self
            .sessions
            .entry(session_id)
            .or_insert_with(|| acp::open_acp_session(session_id, adapter))
            .clone();

        match acp::acp_policy_capabilities(&session, &manifest) {
            Ok(policy) => self.response(id, json!({"policy": policy})),
            Err(error) => self.acp_error(id, error),
        }
    }

    fn obpoc_kyc_dry_run(&mut self, id: Option<Value>, params: Value) -> Vec<JsonRpcOutgoing> {
        let input: KycUpdateStatusDryRunInput = match serde_json::from_value(params) {
            Ok(input) => input,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let session = self
            .sessions
            .entry(input.session_id)
            .or_insert_with(|| acp::open_acp_session(input.session_id, AcpAdapterKind::Zed))
            .clone();
        match acp::acp_dry_run_kyc_update_status(&session, input) {
            Ok(output) => {
                self.response(id, json!({"status": "dry_run_validated", "output": output}))
            }
            Err(error) => self.acp_error(id, error),
        }
    }

    fn acp_error(&self, id: Option<Value>, error: acp::AcpAdapterError) -> Vec<JsonRpcOutgoing> {
        self.error(
            id,
            INVALID_REQUEST,
            format!("{error:?}"),
            Some(serde_json::to_value(error).expect("ACP error serializes")),
        )
    }

    fn session_record(
        &self,
        session_id: Uuid,
        title: &str,
        created_at: &str,
        updated_at: &str,
    ) -> AcpSessionRecord {
        AcpSessionRecord {
            session_id: session_id.to_string(),
            title: title.to_string(),
            created_at: created_at.to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    fn response(&self, id: Option<Value>, result: Value) -> Vec<JsonRpcOutgoing> {
        vec![JsonRpcOutgoing::Response(JsonRpcResponse::success(
            id, result,
        ))]
    }

    fn error(
        &self,
        id: Option<Value>,
        code: i64,
        message: impl Into<String>,
        data: Option<Value>,
    ) -> Vec<JsonRpcOutgoing> {
        vec![JsonRpcOutgoing::Response(JsonRpcResponse::error(
            id, code, message, data,
        ))]
    }
}

fn extract_session_id(params: &Value) -> Result<Uuid, String> {
    let raw = params
        .get("session_id")
        .or_else(|| params.get("sessionId"))
        .and_then(Value::as_str)
        .ok_or_else(|| "session_id is required".to_string())?;
    Uuid::parse_str(raw).map_err(|error| error.to_string())
}

fn load_ob_poc_kyc_domain_pack(
) -> Result<sem_os_core::domain_pack::DomainPackManifest, acp::AcpAdapterError> {
    serde_yaml::from_str(include_str!(
        "../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
    ))
    .map_err(|err| acp::AcpAdapterError::PackInvalid {
        reason: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

    fn request(id: i64, method: &str, params: Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(id)),
            method: method.to_string(),
            params,
        }
    }

    fn only_response(outgoing: Vec<JsonRpcOutgoing>) -> JsonRpcResponse {
        assert_eq!(outgoing.len(), 1);
        match outgoing.into_iter().next().unwrap() {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        }
    }

    #[test]
    fn initialize_advertises_baseline_acp_capabilities() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(1, "initialize", json!({}))));

        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], ACP_PROTOCOL_VERSION);
        assert_eq!(
            result["agentCapabilities"]["sessionCapabilities"]["close"],
            true
        );
        assert_eq!(
            result["agentCapabilities"]["sessionCapabilities"]["list"],
            true
        );
        assert_eq!(
            result["agentCapabilities"]["promptCapabilities"]["embeddedContext"],
            true
        );
    }

    #[test]
    fn session_lifecycle_supports_new_list_cancel_close() {
        let mut agent = AcpJsonRpcAgent::new();
        let created = only_response(agent.handle_request(request(1, "session/new", json!({}))));
        let session_id = created.result.unwrap()["sessionId"]
            .as_str()
            .unwrap()
            .to_string();

        let listed = only_response(agent.handle_request(request(2, "session/list", json!({}))));
        assert_eq!(
            listed.result.unwrap()["sessions"].as_array().unwrap().len(),
            1
        );

        let cancel = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "session/cancel".to_string(),
            params: json!({"sessionId": session_id}),
        };
        assert!(agent.handle_request(cancel).is_empty());

        let prompt = only_response(agent.handle_request(request(
            3,
            "session/prompt",
            json!({"sessionId": session_id, "prompt": [{"type": "text", "text": "hello"}]}),
        )));
        assert_eq!(prompt.result.unwrap()["stopReason"], "cancelled");

        let closed = only_response(agent.handle_request(request(
            4,
            "session/close",
            json!({"sessionId": session_id}),
        )));
        assert_eq!(closed.result.unwrap(), json!({}));
    }

    #[test]
    fn prompt_streams_session_update_then_end_turn() {
        let mut agent = AcpJsonRpcAgent::new();
        agent.handle_request(request(
            1,
            "session/load",
            json!({"sessionId": SESSION_ID.to_string()}),
        ));

        let outgoing = agent.handle_request(request(
            2,
            "session/prompt",
            json!({
                "sessionId": SESSION_ID.to_string(),
                "prompt": [{"type": "text", "text": "assemble context"}]
            }),
        ));

        assert_eq!(outgoing.len(), 2);
        match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => {
                assert_eq!(notification.method, "session/update");
                assert_eq!(notification.params["sessionId"], SESSION_ID.to_string());
                assert_eq!(
                    notification.params["update"]["sessionUpdate"],
                    "agent_message_chunk"
                );
            }
            _ => panic!("expected session/update notification"),
        }
        match &outgoing[1] {
            JsonRpcOutgoing::Response(response) => {
                assert_eq!(response.result.as_ref().unwrap()["stopReason"], "end_turn");
            }
            _ => panic!("expected response"),
        }
    }

    #[test]
    fn extension_context_assembles_redacted_bundle() {
        let mut agent = AcpJsonRpcAgent::new();
        agent.handle_request(request(
            1,
            "session/load",
            json!({"sessionId": SESSION_ID.to_string()}),
        ));

        let response = only_response(agent.handle_request(request(
            2,
            "obpoc/context",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "test_harness",
                "probe_id": "kyc-case.read-evidence-summary",
                "subject_kind": "kyc_case",
                "subject_id": CASE_ID.to_string(),
                "observations": [
                    {"key": "case.status", "value": "INTAKE", "classification": "internal"},
                    {"key": "case.confidential_evidence.summary", "value": "raw", "classification": "internal"}
                ]
            }),
        )));

        let result = response.result.unwrap();
        assert_eq!(result["bundle"]["pack_id"], "ob-poc.kyc");
        assert_eq!(
            result["bundle"]["prompt_context"]["redacted"][0]["key"],
            "case.confidential_evidence.summary"
        );
    }

    #[test]
    fn extension_policy_exposes_semos_policy_surface() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            2,
            "obpoc/policy",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed"
            }),
        )));

        let result = response.result.unwrap();
        assert_eq!(result["policy"]["pack_id"], "ob-poc.kyc");
        assert_eq!(
            result["policy"]["adapter_policy"]["policy_authority"],
            "SemOS Domain Pack + Workbook + Runbook Gate"
        );
        assert_eq!(
            result["policy"]["adapter_policy"]["direct_mutation_supported"],
            false
        );
        assert_eq!(
            result["policy"]["transition_policy"][0]["mutation_allowed"],
            false
        );
    }

    #[test]
    fn extension_kyc_dry_run_uses_existing_workbook_gate() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/kyc_update_status_dry_run",
            json!({
                "session_id": SESSION_ID,
                "case_id": CASE_ID,
                "actor_id": "analyst@example.com",
                "actor_roles": ["analyst"],
                "transition_ref": "kyc-case.intake-to-discovery",
                "current_state": "INTAKE",
                "requested_state": "DISCOVERY",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence",
                "llm_trace_ref": null
            }),
        )));

        let result = response.result.unwrap();
        assert_eq!(result["status"], "dry_run_validated");
        assert_eq!(
            result["output"]["dry_run"]["transition_ref"],
            "kyc-case.intake-to-discovery"
        );
    }

    #[test]
    fn mutation_extension_is_explicitly_refused() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(1, "obpoc/mutation", json!({}))));

        assert_eq!(response.error.unwrap().code, INVALID_REQUEST);
    }
}
