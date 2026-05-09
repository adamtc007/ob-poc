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
use std::time::Instant;
use uuid::Uuid;

use crate::acp::{self, AcpAdapterKind, AcpPersonaMode, AcpSession};
use crate::runbook::{
    KycLanguagePackRequest, KycUpdateStatusDryRunInput, KycUpdateStatusWorkbookDraft,
    WorkbookRevisionOutcome,
};

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
    EmbeddedResource {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
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
    #[serde(default)]
    pub persona: Option<AcpPersonaMode>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct AcpProjectionGetRequest {
    pub session_id: String,
    pub kind: sem_os_core::acp_projection::AcpProjectionKind,
    #[serde(default = "default_adapter")]
    pub adapter: AcpAdapterKind,
    #[serde(default)]
    pub subject: Option<sem_os_core::acp_projection::AcpProjectionSubject>,
    #[serde(default)]
    pub current_state: Option<String>,
    #[serde(default)]
    pub configuration_version: Option<String>,
    #[serde(default)]
    pub state_snapshot_id: Option<String>,
    #[serde(default)]
    pub objective: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcpKycCaseStateDiscoverRequest {
    pub session_id: String,
    #[serde(default = "default_adapter")]
    pub adapter: AcpAdapterKind,
    pub subject_id: Uuid,
    #[serde(default)]
    pub observations: Vec<sem_os_core::domain_pack::DiscoveryObservation>,
    #[serde(default)]
    pub provenance: Vec<sem_os_core::domain_pack::DiscoveryProvenance>,
    #[serde(default)]
    pub first_class_state_mutated: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcpLanguagePackGetRequest {
    pub session_id: String,
    #[serde(default = "default_adapter")]
    pub adapter: AcpAdapterKind,
    pub subject_id: Uuid,
    pub current_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    #[serde(default)]
    pub objective: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcpKycLanguageLoopRequest {
    pub session_id: String,
    #[serde(default = "default_adapter")]
    pub adapter: AcpAdapterKind,
    pub subject_id: Uuid,
    pub current_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    #[serde(default)]
    pub objective: Option<String>,
    pub draft: KycUpdateStatusWorkbookDraft,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcpPermissionRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default, rename = "sessionId")]
    pub session_id_camel: Option<String>,
    #[serde(default)]
    pub persona: Option<AcpPersonaMode>,
    #[serde(default)]
    pub workbook_id: Option<String>,
    #[serde(default)]
    pub workbook_hash: Option<String>,
    #[serde(default)]
    pub state_snapshot_id: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<Value>,
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
            "obpoc/projections/list" => self.obpoc_projection_list(id, request.params),
            "obpoc/projection/get" => self.obpoc_projection_get(id, request.params),
            "obpoc/kyc_case_state/discover" => self.obpoc_kyc_case_state_discover(id, request.params),
            "obpoc/language_pack/get" => self.obpoc_language_pack_get(id, request.params),
            "obpoc/kyc_update_status_language_loop" => {
                self.obpoc_kyc_update_status_language_loop(id, request.params)
            }
            "obpoc/context" => self.obpoc_context(id, request.params),
            "obpoc/kyc_update_status_dry_run" => self.obpoc_kyc_dry_run(id, request.params),
            "request_permission" | "obpoc/request_permission" => {
                self.obpoc_request_permission(id, request.params)
            }
            "write_text_file" | "fs/write_text_file" | "create_text_file" | "terminal/new"
            | "terminal/create" => self.error(
                id,
                INVALID_REQUEST,
                format!(
                    "{} is outside the ACP discovery surface and is not permitted",
                    request.method
                ),
                Some(json!({
                    "capability": "none",
                    "authority_surface": request.method,
                    "reason": "ACP projects SemOS visibility only; execution remains behind workbook approval and the compiled runbook gate"
                })),
            ),
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
        let manifest =
            load_ob_poc_kyc_domain_pack().expect("bundled ob-poc KYC Domain Pack parses");
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
        .map(|mut value| {
            value["obpocCapabilities"] = obpoc_capability_summary(&manifest);
            value
        })
        .expect("ACP initialize result serializes")
    }

    fn session_new(&mut self, id: Option<Value>) -> Vec<JsonRpcOutgoing> {
        let session_id = Uuid::new_v4();
        let session = acp::open_acp_session_with_persona(
            session_id,
            AcpAdapterKind::Zed,
            AcpPersonaMode::SagePlanning,
        );
        self.sessions.insert(session_id, session);
        let now = Utc::now().to_rfc3339();
        self.response(
            id,
            json!({
                "sessionId": session_id.to_string(),
                "session": self.session_record(session_id, "ob-poc ACP session", &now, &now),
                "modeState": {
                    "currentModeId": AcpPersonaMode::SagePlanning.as_str(),
                    "availableModes": obpoc_mode_state()
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
        self.sessions.entry(session_id).or_insert_with(|| {
            acp::open_acp_session_with_persona(
                session_id,
                AcpAdapterKind::Zed,
                request.persona.unwrap_or(AcpPersonaMode::SagePlanning),
            )
        });
        let now = Utc::now().to_rfc3339();
        self.response(
            id,
            json!({
                "sessionId": session_id.to_string(),
                "session": self.session_record(session_id, "ob-poc ACP session", &now, &now),
                "modeState": {
                    "currentModeId": request.persona.unwrap_or(AcpPersonaMode::SagePlanning).as_str(),
                    "availableModes": obpoc_mode_state()
                },
                "restore": {
                    "status": "loaded_without_persisted_runtime_state",
                    "stalePolicy": "new_session_context_required",
                    "configurationVersion": null,
                    "stateSnapshotId": null,
                    "pinnedResourceLinks": []
                }
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

        if let Some(outgoing) =
            self.try_session_prompt_kyc_update_status(id.clone(), session_id, &request.prompt)
        {
            return outgoing;
        }

        let prompt_text = request
            .prompt
            .iter()
            .map(|block| match block {
                AcpContentBlock::Text { text } => text.as_str(),
                AcpContentBlock::ResourceLink { uri, .. }
                | AcpContentBlock::EmbeddedResource { uri, .. } => uri.as_str(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let resolved_refs = resolve_prompt_resource_refs(&request.prompt);
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
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": format!("tool:projection-catalog:{session_id}"),
                        "status": "completed",
                        "kind": "read",
                        "persona": AcpPersonaMode::SagePlanning.as_str(),
                        "workflowPhase": "discovery",
                        "title": "ACP projection catalogue",
                        "content": {
                            "type": "resource_link",
                            "uri": "semos://pack-manifest/ob-poc.kyc",
                            "name": "SemOS projection surface",
                            "description": "Pack, policy, DAG, verbs, lineage, materiality, and workspace projections"
                        }
                    }
                }),
            }),
            JsonRpcOutgoing::Notification(JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "plan",
                        "persona": AcpPersonaMode::SagePlanning.as_str(),
                        "workflowPhase": "planning",
                        "goalProposalTrace": {
                            "status": "projection_summary",
                            "resolvedResourceRefs": resolved_refs,
                            "acpMechanismSummary": ["resource_ref_resolution", "demand_driven"],
                            "acpFallbackSummary": [],
                            "projectionCount": 0,
                            "projectionBytes": prompt_text.len(),
                            "projectionLatencyMs": 0
                        },
                        "entries": [
                            {"id": "discover", "status": "completed", "label": "Read SemOS projection surface"},
                            {"id": "plan", "status": "in_progress", "label": "Assemble workbook-safe plan"},
                            {"id": "execute", "status": "blocked", "label": "Await DSL Coder and HITL gate"}
                        ]
                    }
                }),
            }),
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

    fn try_session_prompt_kyc_update_status(
        &mut self,
        id: Option<Value>,
        session_id: Uuid,
        prompt: &[AcpContentBlock],
    ) -> Option<Vec<JsonRpcOutgoing>> {
        let utterance_text = prompt_utterance_text(prompt);
        if !looks_like_kyc_update_status_prompt(&utterance_text) {
            if looks_like_kyc_domain_prompt(&utterance_text) {
                return Some(self.response(
                    id,
                    json!({
                        "stopReason": "end_turn",
                        "status": "pending_question",
                        "pending_question": {
                            "code": "kyc_prompt_ambiguous",
                            "candidate_verbs": [
                                "kyc-case.read",
                                "kyc-case.create",
                                "kyc-case.update-status",
                                "screening.update-status",
                                "document.collect"
                            ]
                        }
                    }),
                ));
            }
            return None;
        }

        let request = match kyc_language_loop_request_from_prompt(session_id, prompt) {
            Ok(request) => request,
            Err(error) => {
                return Some(self.response(
                    id,
                    json!({
                        "stopReason": "end_turn",
                        "status": "pending_question",
                        "pending_question": {
                            "code": "kyc_update_status_prompt_incomplete",
                            "missing": error
                        }
                    }),
                ))
            }
        };

        Some(self.obpoc_kyc_update_status_language_loop(id, request))
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

    fn obpoc_projection_list(&mut self, id: Option<Value>, params: Value) -> Vec<JsonRpcOutgoing> {
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

        match acp::list_acp_projections(&session, &manifest) {
            Ok(projections) => self.response(
                id,
                json!({
                    "status": "acp_projection_catalog",
                    "session_id": session_id,
                    "pack_id": manifest.pack_id,
                    "projections": projections,
                }),
            ),
            Err(error) => self.acp_error(id, error),
        }
    }

    fn obpoc_projection_get(&mut self, id: Option<Value>, params: Value) -> Vec<JsonRpcOutgoing> {
        let request: AcpProjectionGetRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let session_id = match Uuid::parse_str(&request.session_id) {
            Ok(session_id) => session_id,
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
        let language_pack_request = if request.kind
            == sem_os_core::acp_projection::AcpProjectionKind::LanguagePack
        {
            let subject = match request.subject.as_ref() {
                Some(subject) => subject,
                None => {
                    return self.error(
                        id,
                        INVALID_PARAMS,
                        "language_pack projection requires subject".to_string(),
                        None,
                    )
                }
            };
            let subject_id = match Uuid::parse_str(&subject.subject_id) {
                Ok(subject_id) => subject_id,
                Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
            };
            Some(KycLanguagePackRequest {
                subject_id,
                current_state: match request.current_state {
                    Some(current_state) => current_state,
                    None => {
                        return self.error(
                            id,
                            INVALID_PARAMS,
                            "language_pack projection requires current_state".to_string(),
                            None,
                        )
                    }
                },
                configuration_version: match request.configuration_version {
                    Some(configuration_version) => configuration_version,
                    None => {
                        return self.error(
                            id,
                            INVALID_PARAMS,
                            "language_pack projection requires configuration_version".to_string(),
                            None,
                        )
                    }
                },
                state_snapshot_id: match request.state_snapshot_id {
                    Some(state_snapshot_id) => state_snapshot_id,
                    None => {
                        return self.error(
                            id,
                            INVALID_PARAMS,
                            "language_pack projection requires state_snapshot_id".to_string(),
                            None,
                        )
                    }
                },
                objective: request.objective,
            })
        } else {
            None
        };

        let started_at = Instant::now();
        match acp::build_acp_projection(
            &session,
            &manifest,
            acp::AcpProjectionRequest {
                kind: request.kind,
                subject: request.subject,
                language_pack_request,
            },
        ) {
            Ok(envelope) => {
                let projection_latency_ms =
                    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                let projection_bytes = serde_json::to_vec(&envelope)
                    .map(|bytes| bytes.len())
                    .unwrap_or(0);
                self.response(
                    id,
                    json!({
                        "status": "acp_projection",
                        "projection": envelope,
                        "observability": {
                            "acpMechanismSummary": [
                                "projection_get",
                                "classification_policy",
                                "demand_driven"
                            ],
                            "acpFallbackSummary": [],
                            "projectionCount": 1,
                            "projectionBytes": projection_bytes,
                            "projectionLatencyMs": projection_latency_ms
                        }
                    }),
                )
            }
            Err(error) => self.acp_error(id, error),
        }
    }

    fn obpoc_kyc_case_state_discover(
        &mut self,
        id: Option<Value>,
        params: Value,
    ) -> Vec<JsonRpcOutgoing> {
        let request: AcpKycCaseStateDiscoverRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let session_id = match Uuid::parse_str(&request.session_id) {
            Ok(session_id) => session_id,
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
        let response = sem_os_core::domain_pack::DiscoveryResponse {
            probe_id: "kyc-case.read-state".to_string(),
            subject: sem_os_core::domain_pack::DiscoverySubject {
                subject_kind: "kyc_case".to_string(),
                subject_id: request.subject_id.to_string(),
            },
            observations: request.observations,
            provenance: request.provenance,
            first_class_state_mutated: request.first_class_state_mutated,
        };

        let started_at = Instant::now();
        match acp::acp_discover_kyc_case_state(&session, &manifest, request.subject_id, response) {
            Ok(case_state) => {
                let language_pack_request = json!({
                    "subject_id": case_state.subject_id,
                    "current_state": &case_state.current_state,
                    "configuration_version": &case_state.configuration_version,
                    "state_snapshot_id": &case_state.state_snapshot_id
                });
                self.response(
                    id,
                    json!({
                        "status": "kyc_case_state_discovered",
                        "case_state": case_state,
                        "language_pack_request": language_pack_request,
                        "observability": {
                            "acpMechanismSummary": [
                                "read_only_discovery_probe",
                                "kyc_case_state_anchor",
                                "language_pack_ready"
                            ],
                            "projectionLatencyMs": u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
                        }
                    }),
                )
            }
            Err(error) => self.acp_error(id, error),
        }
    }

    fn obpoc_language_pack_get(
        &mut self,
        id: Option<Value>,
        params: Value,
    ) -> Vec<JsonRpcOutgoing> {
        let request: AcpLanguagePackGetRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let session_id = match Uuid::parse_str(&request.session_id) {
            Ok(session_id) => session_id,
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

        let started_at = Instant::now();
        match acp::acp_kyc_update_status_language_pack(
            &session,
            &manifest,
            KycLanguagePackRequest {
                subject_id: request.subject_id,
                current_state: request.current_state,
                configuration_version: request.configuration_version,
                state_snapshot_id: request.state_snapshot_id,
                objective: request.objective,
            },
        ) {
            Ok(language_pack) => {
                let language_pack_bytes = serde_json::to_vec(&language_pack)
                    .map(|bytes| bytes.len())
                    .unwrap_or(0);
                self.response(
                    id,
                    json!({
                        "status": "sem_os_language_pack",
                        "session_id": session_id,
                        "language_pack": language_pack,
                        "observability": {
                            "acpMechanismSummary": [
                                "language_pack_get",
                                "bounded_private_dsl",
                                "dry_run_only"
                            ],
                            "projectionCount": 1,
                            "projectionBytes": language_pack_bytes,
                            "projectionLatencyMs": u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
                        }
                    }),
                )
            }
            Err(error) => self.acp_error(id, error),
        }
    }

    fn obpoc_kyc_update_status_language_loop(
        &mut self,
        id: Option<Value>,
        params: Value,
    ) -> Vec<JsonRpcOutgoing> {
        let request: AcpKycLanguageLoopRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let session_id = match Uuid::parse_str(&request.session_id) {
            Ok(session_id) => session_id,
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

        let started_at = Instant::now();
        let outcome = match acp::acp_run_kyc_update_status_language_loop(
            &session,
            &manifest,
            KycLanguagePackRequest {
                subject_id: request.subject_id,
                current_state: request.current_state,
                configuration_version: request.configuration_version,
                state_snapshot_id: request.state_snapshot_id,
                objective: request.objective,
            },
            request.draft,
        ) {
            Ok(outcome) => outcome,
            Err(error) => return self.acp_error(id, error),
        };

        let (language_pack, revision_outcome) = outcome;
        let mut outgoing = vec![
            JsonRpcOutgoing::Notification(JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": format!("tool:language-pack:{session_id}"),
                        "status": "completed",
                        "kind": "read",
                        "persona": AcpPersonaMode::SagePlanning.as_str(),
                        "workflowPhase": "discovery",
                        "title": "SemOS language pack",
                        "content": {
                            "type": "resource_link",
                            "uri": format!("semos://pack-manifest/{}", language_pack.pack_id),
                            "name": "KYC update-status language pack",
                            "description": "Bounded private DSL context for kyc-case.update-status"
                        }
                    }
                }),
            }),
            JsonRpcOutgoing::Notification(JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "plan",
                        "persona": AcpPersonaMode::SagePlanning.as_str(),
                        "workflowPhase": "planning",
                        "goalProposalTrace": language_loop_trace_summary(&revision_outcome),
                        "entries": language_loop_plan_entries(&revision_outcome)
                    }
                }),
            }),
        ];

        match revision_outcome {
            WorkbookRevisionOutcome::DryRunValid {
                output,
                attempts,
                metrics,
                trace,
            } => {
                outgoing.push(JsonRpcOutgoing::Notification(JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params: json!({
                        "sessionId": session_id.to_string(),
                        "update": {
                            "sessionUpdate": "tool_call_update",
                            "toolCallId": format!("tool:language-loop:{}", output.workbook.id),
                            "status": "completed",
                            "kind": "think",
                            "persona": AcpPersonaMode::SageExecution.as_str(),
                            "workflowPhase": "planning",
                            "title": "Workbook validation loop",
                            "content": {
                                "type": "resource_link",
                                "uri": format!("semos://workbook/{}", output.workbook.id),
                                "name": "Validated execution workbook",
                                "description": "Draft validated after bounded diagnostic revision"
                            }
                        }
                    }),
                }));
                outgoing.push(JsonRpcOutgoing::Notification(JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params: json!({
                        "sessionId": session_id.to_string(),
                        "update": {
                            "sessionUpdate": "semantic_diff",
                            "persona": AcpPersonaMode::SageExecution.as_str(),
                            "semanticDiffId": output.dry_run.semantic_diff_uri.clone(),
                            "fallbackSummary": ["resource_link"],
                            "diff": output.dry_run.semantic_diff.semantic_diff.clone(),
                            "transitionRef": output.dry_run.transition_ref.clone(),
                            "validationTrace": output.dry_run.validation_trace.clone()
                        }
                    }),
                }));
                outgoing.push(JsonRpcOutgoing::Notification(JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params: json!({
                        "sessionId": session_id.to_string(),
                        "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": {
                                "type": "text",
                                "text": "KYC update-status workbook dry-run validated. No mutation was executed."
                            }
                        }
                    }),
                }));
                outgoing.push(JsonRpcOutgoing::Response(JsonRpcResponse::success(
                    id,
                    json!({
                        "status": "dry_run_validated",
                        "language_pack": language_pack,
                        "output": output,
                        "attempts": attempts,
                        "metrics": metrics,
                        "trace": trace,
                        "observability": {
                            "projectionLatencyMs": u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                            "acpMechanismSummary": ["language_pack", "deterministic_revision_loop", "dry_run_only"]
                        }
                    }),
                )));
            }
            WorkbookRevisionOutcome::Refused {
                refusal,
                attempts,
                metrics,
                trace,
            } => {
                outgoing.push(JsonRpcOutgoing::Notification(JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params: json!({
                        "sessionId": session_id.to_string(),
                        "update": {
                            "sessionUpdate": "tool_call_update",
                            "toolCallId": format!("tool:language-loop-refusal:{session_id}"),
                            "status": "failed",
                            "kind": "think",
                            "persona": AcpPersonaMode::SageExecution.as_str(),
                            "workflowPhase": "planning",
                            "title": "Workbook validation loop",
                            "content": {
                                "type": "embedded_resource",
                                "uri": format!("semos://diagnostic/{}", refusal.refusal_code),
                                "name": "Structured refusal",
                                "text": serde_json::to_string(&refusal).unwrap_or_default()
                            }
                        }
                    }),
                }));
                outgoing.push(JsonRpcOutgoing::Notification(JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params: json!({
                        "sessionId": session_id.to_string(),
                        "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": {
                                "type": "text",
                                "text": format!("KYC update-status workbook refused: {}", refusal.refusal_code)
                            }
                        }
                    }),
                }));
                outgoing.push(JsonRpcOutgoing::Response(JsonRpcResponse::success(
                    id,
                    json!({
                        "status": "structured_refusal",
                        "language_pack": language_pack,
                        "refusal": refusal,
                        "attempts": attempts,
                        "metrics": metrics,
                        "trace": trace,
                        "observability": {
                            "projectionLatencyMs": u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                            "acpMechanismSummary": ["language_pack", "deterministic_revision_loop", "structured_refusal"]
                        }
                    }),
                )));
            }
        }

        outgoing
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
            Ok(output) => vec![
                JsonRpcOutgoing::Notification(JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params: json!({
                        "sessionId": output.workbook.core.session_id.to_string(),
                        "update": {
                            "sessionUpdate": "tool_call_update",
                            "toolCallId": format!("tool:dry-run:{}", output.workbook.id),
                            "status": "completed",
                            "kind": "think",
                            "persona": AcpPersonaMode::SageExecution.as_str(),
                            "title": "KYC update-status dry-run",
                            "content": {
                                "type": "resource_link",
                                "uri": format!("semos://workbook/{}", output.workbook.id),
                                "name": "Execution workbook",
                                "description": "Workbook validated without mutation"
                            }
                        }
                    }),
                }),
                JsonRpcOutgoing::Notification(JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "session/update".to_string(),
                    params: json!({
                        "sessionId": output.workbook.core.session_id.to_string(),
                        "update": {
                            "sessionUpdate": "semantic_diff",
                            "persona": AcpPersonaMode::SageExecution.as_str(),
                            "semanticDiffId": output.dry_run.semantic_diff_uri.clone(),
                            "fallbackSummary": ["resource_link"],
                            "diff": output.dry_run.semantic_diff.semantic_diff.clone(),
                            "transitionRef": output.dry_run.transition_ref.clone(),
                            "validationTrace": output.dry_run.validation_trace.clone()
                        }
                    }),
                }),
                JsonRpcOutgoing::Response(JsonRpcResponse::success(
                    id,
                    json!({"status": "dry_run_validated", "output": output}),
                )),
            ],
            Err(error) => self.acp_error(id, error),
        }
    }

    fn obpoc_request_permission(
        &mut self,
        id: Option<Value>,
        params: Value,
    ) -> Vec<JsonRpcOutgoing> {
        let request: AcpPermissionRequest = match serde_json::from_value(params) {
            Ok(request) => request,
            Err(error) => return self.error(id, INVALID_PARAMS, error.to_string(), None),
        };
        let raw_session_id = request.session_id.or(request.session_id_camel);
        let session_id = match raw_session_id
            .as_deref()
            .ok_or_else(|| "session_id is required".to_string())
            .and_then(|raw| Uuid::parse_str(raw).map_err(|error| error.to_string()))
        {
            Ok(session_id) => session_id,
            Err(error) => return self.error(id, INVALID_PARAMS, error, None),
        };
        if request.persona != Some(AcpPersonaMode::SageExecution) {
            return self.error(
                id,
                INVALID_REQUEST,
                "session/request_permission is only available to sage:execution",
                Some(json!({
                    "capability": "none",
                    "requiredPersona": AcpPersonaMode::SageExecution.as_str(),
                    "actualPersona": request.persona.map(AcpPersonaMode::as_str)
                })),
            );
        }
        self.sessions.entry(session_id).or_insert_with(|| {
            acp::open_acp_session_with_persona(
                session_id,
                AcpAdapterKind::Zed,
                AcpPersonaMode::SageExecution,
            )
        });
        let permission_id = format!("permission:hitl:{}", Uuid::new_v4());
        self.response(
            id,
            json!({
                "status": "permission_request_created",
                "permission_request_id": permission_id,
                "session_id": session_id,
                "authority_surface": "request_permission",
                "persona": AcpPersonaMode::SageExecution.as_str(),
                "scope": "hitl_approval_only",
                "execution_authority": false,
                "approval_binding": {
                    "workbook_id": request.workbook_id,
                    "workbook_hash": request.workbook_hash,
                    "state_snapshot_id": request.state_snapshot_id,
                    "evidence_refs": request.evidence_refs
                },
                "reason": "ACP may request attestation metadata, but mutation still requires workbook approval and the compiled runbook gate"
            }),
        )
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

fn obpoc_mode_state() -> Value {
    json!([
        {"id": "sage:planning", "name": "Sage Planning", "description": "Discovery, projection, planning, explanation, attestation prompting, and workbook drafting"},
        {"id": "sage:execution", "name": "Sage Execution", "description": "Workbook validation, dry-run, HITL approval routing, and approved REPL DSL execution"}
    ])
}

fn resolve_prompt_resource_refs(prompt: &[AcpContentBlock]) -> Vec<Value> {
    prompt
        .iter()
        .filter_map(|block| match block {
            AcpContentBlock::ResourceLink { uri, name, .. } => {
                parse_semos_resource_uri(uri).map(|parsed| {
                    json!({
                        "uri": uri,
                        "name": name,
                        "resource_kind": parsed.resource_kind,
                        "resource_id": parsed.resource_id,
                    })
                })
            }
            AcpContentBlock::EmbeddedResource {
                uri,
                name,
                mime_type,
                ..
            } => parse_semos_resource_uri(uri).map(|parsed| {
                json!({
                    "uri": uri,
                    "name": name,
                    "mime_type": mime_type,
                    "resource_kind": parsed.resource_kind,
                    "resource_id": parsed.resource_id,
                    "inbound_classification_required": true,
                })
            }),
            AcpContentBlock::Text { .. } => None,
        })
        .collect()
}

fn prompt_semantic_text(prompt: &[AcpContentBlock]) -> String {
    prompt
        .iter()
        .map(|block| match block {
            AcpContentBlock::Text { text } => text.as_str(),
            AcpContentBlock::ResourceLink { uri, .. } => uri.as_str(),
            AcpContentBlock::EmbeddedResource { uri, text, .. } => {
                text.as_deref().unwrap_or(uri.as_str())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn prompt_utterance_text(prompt: &[AcpContentBlock]) -> String {
    prompt
        .iter()
        .filter_map(|block| match block {
            AcpContentBlock::Text { text } => Some(text.as_str()),
            AcpContentBlock::ResourceLink { .. } | AcpContentBlock::EmbeddedResource { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn looks_like_kyc_update_status_prompt(prompt_text: &str) -> bool {
    let lower = prompt_text.to_ascii_lowercase();
    (lower.contains("kyc") || lower.contains("case"))
        && (lower.contains("update-status")
            || lower.contains("update status")
            || lower.contains("advance")
            || lower.contains("transition")
            || lower.contains("move")
            || lower.contains("change status")
            || lower.contains("set status"))
}

fn looks_like_kyc_domain_prompt(prompt_text: &str) -> bool {
    let lower = prompt_text.to_ascii_lowercase();
    (lower.contains("kyc") && lower.contains("case"))
        || lower.contains("kyc-case.")
        || lower.contains("screening")
        || lower.contains("due diligence")
}

fn kyc_language_loop_request_from_prompt(
    session_id: Uuid,
    prompt: &[AcpContentBlock],
) -> Result<Value, Vec<&'static str>> {
    let prompt_text = prompt_semantic_text(prompt);
    let utterance_text = prompt_utterance_text(prompt);
    let case_state = extract_prompt_case_state(prompt, &utterance_text);
    let subject_id = case_state
        .as_ref()
        .and_then(|state| state.subject_id)
        .or_else(|| extract_first_uuid(&prompt_text));
    let current_state = case_state
        .as_ref()
        .and_then(|state| state.current_state.clone())
        .or_else(|| {
            extract_state_after_marker(
                &prompt_text,
                &["current state", "current status", "from", "status"],
            )
        });
    let requested_state = extract_state_after_marker(
        &utterance_text,
        &["to", "target", "requested", "advance to", "move to"],
    )
    .or_else(|| {
        let lower = utterance_text.to_ascii_lowercase();
        if lower.contains("assessment") {
            Some("ASSESSMENT".to_string())
        } else if lower.contains("discovery") {
            Some("DISCOVERY".to_string())
        } else {
            None
        }
    });
    let configuration_version = case_state
        .as_ref()
        .and_then(|state| state.configuration_version.clone())
        .or_else(|| extract_token_with_prefix(&prompt_text, "config-"))
        .unwrap_or_else(|| "config-1".to_string());
    let state_snapshot_id = case_state
        .as_ref()
        .and_then(|state| state.state_snapshot_id.clone())
        .or_else(|| extract_token_with_prefix(&prompt_text, "snapshot-"))
        .unwrap_or_else(|| "snapshot-1".to_string());

    let mut missing = Vec::new();
    if subject_id.is_none() {
        missing.push("case_uuid");
    }
    if current_state.is_none() {
        missing.push("current_state");
    }
    if requested_state.is_none() {
        missing.push("requested_state");
    }
    if !missing.is_empty() {
        return Err(missing);
    }

    let subject_id = subject_id.expect("checked above");
    let current_state = current_state.expect("checked above");
    let requested_state = requested_state.expect("checked above");
    let transition_ref = extract_transition_ref(&utterance_text)
        .unwrap_or_else(|| transition_ref_for_states(&current_state, &requested_state));
    let evidence_digest = extract_token_with_prefix(&utterance_text, "sha256:");

    Ok(json!({
        "session_id": session_id.to_string(),
        "adapter": "zed",
        "subject_id": subject_id,
        "current_state": current_state,
        "configuration_version": configuration_version,
        "state_snapshot_id": state_snapshot_id,
        "objective": utterance_text,
        "draft": {
            "session_id": session_id,
            "actor_id": "sage:planning",
            "actor_roles": ["agent"],
            "verb": "kyc-case.update-status",
            "transition_ref": transition_ref,
            "subject_kind": "kyc_case",
            "case_id": subject_id,
            "current_state": current_state,
            "requested_state": requested_state,
            "configuration_version": configuration_version,
            "state_snapshot_id": state_snapshot_id,
            "evidence_digest": evidence_digest
        }
    }))
}

#[derive(Debug, Default)]
struct PromptCaseState {
    subject_id: Option<Uuid>,
    current_state: Option<String>,
    configuration_version: Option<String>,
    state_snapshot_id: Option<String>,
}

fn extract_prompt_case_state(
    prompt: &[AcpContentBlock],
    prompt_text: &str,
) -> Option<PromptCaseState> {
    prompt
        .iter()
        .filter_map(|block| match block {
            AcpContentBlock::EmbeddedResource {
                text: Some(text), ..
            } => Some(text.as_str()),
            _ => None,
        })
        .filter_map(|text| serde_json::from_str::<Value>(text).ok())
        .find_map(|value| prompt_case_state_from_value(&value))
        .or_else(|| {
            Some(PromptCaseState {
                subject_id: extract_first_uuid(prompt_text),
                current_state: extract_state_after_marker(
                    prompt_text,
                    &["current state", "current status", "from", "status"],
                ),
                configuration_version: extract_token_with_prefix(prompt_text, "config-"),
                state_snapshot_id: extract_token_with_prefix(prompt_text, "snapshot-"),
            })
        })
}

fn prompt_case_state_from_value(value: &Value) -> Option<PromptCaseState> {
    let source = value
        .get("case_state")
        .or_else(|| value.get("language_pack_request"))
        .unwrap_or(value);
    Some(PromptCaseState {
        subject_id: source
            .get("subject_id")
            .or_else(|| source.get("case_id"))
            .and_then(Value::as_str)
            .and_then(|raw| Uuid::parse_str(raw).ok()),
        current_state: source
            .get("current_state")
            .and_then(Value::as_str)
            .map(str::to_string),
        configuration_version: source
            .get("configuration_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        state_snapshot_id: source
            .get("state_snapshot_id")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn extract_first_uuid(text: &str) -> Option<Uuid> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';' | ')' | '(' | '"' | '\''))
        .map(|token| token.trim_matches(|ch: char| matches!(ch, '.' | ':' | ',' | ';' | '[' | ']')))
        .find_map(|token| Uuid::parse_str(token).ok())
}

fn extract_state_after_marker(text: &str, markers: &[&str]) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    markers.iter().find_map(|marker| {
        lower.find(marker).and_then(|start| {
            let segment = lower
                .get(start + marker.len()..)
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect::<String>();
            state_in_text(&segment)
        })
    })
}

fn state_in_text(text: &str) -> Option<String> {
    [
        ("INTAKE", "intake"),
        ("DISCOVERY", "discovery"),
        ("ASSESSMENT", "assessment"),
    ]
    .iter()
    .filter_map(|(state, needle)| text.find(needle).map(|index| (index, *state)))
    .min_by_key(|(index, _)| *index)
    .map(|(_, state)| state.to_string())
}

fn extract_token_with_prefix(text: &str, prefix: &str) -> Option<String> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']'))
        .map(|token| token.trim_matches(|ch: char| matches!(ch, ',' | ';' | '.')))
        .find(|token| token.starts_with(prefix))
        .map(str::to_string)
}

fn extract_transition_ref(text: &str) -> Option<String> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']'))
        .map(|token| token.trim_matches(|ch: char| matches!(ch, ',' | ';' | '.')))
        .find(|token| token.starts_with("kyc-case.") && token.contains("-to-"))
        .map(str::to_string)
}

fn transition_ref_for_states(current_state: &str, requested_state: &str) -> String {
    match (current_state, requested_state) {
        ("INTAKE", "DISCOVERY") => "kyc-case.intake-to-discovery".to_string(),
        ("DISCOVERY", "ASSESSMENT") => "kyc-case.discovery-to-assessment".to_string(),
        _ => "kyc-case.unknown-transition".to_string(),
    }
}

struct ParsedSemosResource {
    resource_kind: &'static str,
    resource_id: String,
}

fn parse_semos_resource_uri(uri: &str) -> Option<ParsedSemosResource> {
    let rest = uri.strip_prefix("semos://")?;
    let (kind, id) = rest.split_once('/')?;
    let resource_kind = match kind {
        "pack-manifest" => "pack_manifest",
        "entity" => "entity",
        "verb" => "verb",
        "transition" => "transition",
        "workbook" => "workbook",
        "semantic-diff" => "semantic_diff",
        _ => return None,
    };
    if id.trim().is_empty() {
        return None;
    }
    Some(ParsedSemosResource {
        resource_kind,
        resource_id: id.to_string(),
    })
}

fn obpoc_capability_summary(manifest: &sem_os_core::domain_pack::DomainPackManifest) -> Value {
    let session = acp::open_acp_session(Uuid::nil(), AcpAdapterKind::Zed);
    let policy = acp::acp_policy_capabilities(&session, manifest)
        .expect("bundled ob-poc KYC Domain Pack validates");
    json!({
        "pack": {
            "pack_id": manifest.pack_id,
            "version": manifest.version,
            "implementation_mode": manifest.implementation_mode,
            "compatibility_tier": manifest.compatibility_tier
        },
        "projections": manifest.projection_catalog,
        "probes": manifest.discovery_probes,
        "mentionNamespaces": manifest.mention_namespaces,
        "modes": manifest.acp_personas,
        "workflowPhases": manifest.workflow_phases,
        "resourceUriSchemes": manifest.resource_uri_schemes,
        "configOptions": {
            "personas": manifest.acp_personas,
            "defaultPersona": AcpPersonaMode::SagePlanning.as_str(),
            "workflowPhases": manifest.workflow_phases,
            "classificationTaxonomy": ["public", "internal", "confidential", "restricted"],
            "declinedAuthoritySurfaces": ["fs/write_text_file", "terminal/create"]
        },
        "classification": manifest.classification_policy,
        "authoritySurfaces": policy.authority_surfaces,
        "externalMcpTransports": manifest.external_mcp_transports,
        "typedExtensionPoints": manifest.typed_extension_points
    })
}

fn extract_session_id(params: &Value) -> Result<Uuid, String> {
    let raw = params
        .get("session_id")
        .or_else(|| params.get("sessionId"))
        .and_then(Value::as_str)
        .ok_or_else(|| "session_id is required".to_string())?;
    Uuid::parse_str(raw).map_err(|error| error.to_string())
}

fn language_loop_trace_summary(outcome: &WorkbookRevisionOutcome) -> Value {
    match outcome {
        WorkbookRevisionOutcome::DryRunValid { metrics, trace, .. } => json!({
            "status": "dry_run_validated",
            "acpMechanismSummary": ["language_pack", "deterministic_revision_loop", "dry_run_only"],
            "acpFallbackSummary": [],
            "revisionCount": metrics.revision_count,
            "firstPassValid": metrics.first_pass_valid,
            "dryRunValid": metrics.dry_run_valid,
            "refusalCode": metrics.refusal_code,
            "trace": trace
        }),
        WorkbookRevisionOutcome::Refused {
            refusal,
            metrics,
            trace,
            ..
        } => json!({
            "status": "structured_refusal",
            "acpMechanismSummary": ["language_pack", "deterministic_revision_loop", "structured_refusal"],
            "acpFallbackSummary": [],
            "revisionCount": metrics.revision_count,
            "firstPassValid": metrics.first_pass_valid,
            "dryRunValid": metrics.dry_run_valid,
            "refusalCode": refusal.refusal_code,
            "trace": trace
        }),
    }
}

fn language_loop_plan_entries(outcome: &WorkbookRevisionOutcome) -> Value {
    let (validation_status, dry_run_status) = match outcome {
        WorkbookRevisionOutcome::DryRunValid { .. } => ("completed", "completed"),
        WorkbookRevisionOutcome::Refused { .. } => ("failed", "blocked"),
    };
    json!([
        {"id": "language-pack", "status": "completed", "label": "Retrieve bounded SemOS language pack"},
        {"id": "draft", "status": "completed", "label": "Produce workbook draft"},
        {"id": "validate", "status": validation_status, "label": "Validate draft with structured diagnostics"},
        {"id": "dry-run", "status": dry_run_status, "label": "Run non-mutating workbook dry-run"}
    ])
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

        let result = response.result.as_ref().unwrap();
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
        assert!(result["obpocCapabilities"]["projections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "policy"));
        assert!(result["obpocCapabilities"]["authoritySurfaces"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |surface| surface["surface"] == "terminal/create" && surface["permitted"] == false
            ));
        assert_eq!(
            result["obpocCapabilities"]["configOptions"]["defaultPersona"],
            "sage:planning"
        );
        assert!(result["obpocCapabilities"]["resourceUriSchemes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|scheme| scheme["scheme"] == "semos://workbook/{id}"));
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

        assert_eq!(outgoing.len(), 4);
        match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => {
                assert_eq!(notification.method, "session/update");
                assert_eq!(notification.params["sessionId"], SESSION_ID.to_string());
                assert_eq!(
                    notification.params["update"]["sessionUpdate"],
                    "tool_call_update"
                );
            }
            _ => panic!("expected session/update notification"),
        }
        match &outgoing[3] {
            JsonRpcOutgoing::Response(response) => {
                assert_eq!(response.result.as_ref().unwrap()["stopReason"], "end_turn");
            }
            _ => panic!("expected response"),
        }
        match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => {
                let trace = &notification.params["update"]["goalProposalTrace"];
                assert_eq!(trace["projectionCount"], 0);
                assert!(trace["acpMechanismSummary"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|mechanism| mechanism == "demand_driven"));
            }
            _ => panic!("expected plan notification"),
        }
    }

    #[test]
    fn session_prompt_routes_kyc_update_status_to_language_loop() {
        let mut agent = AcpJsonRpcAgent::new();
        let case_state = json!({
            "case_state": {
                "subject_id": CASE_ID,
                "current_state": "DISCOVERY",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1"
            }
        });

        let outgoing = agent.handle_request(request(
            1,
            "session/prompt",
            json!({
                "sessionId": SESSION_ID.to_string(),
                "prompt": [
                    {
                        "type": "text",
                        "text": "Advance the KYC case to assessment with evidence sha256:evidence"
                    },
                    {
                        "type": "embedded_resource",
                        "uri": format!("semos://entity/{}", CASE_ID),
                        "name": "KYC case state",
                        "mime_type": "application/json",
                        "text": case_state.to_string()
                    }
                ]
            }),
        ));

        assert_eq!(outgoing.len(), 6);
        assert!(matches!(
            &outgoing[0],
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["toolCallId"]
                    .as_str()
                    .unwrap()
                    .starts_with("tool:language-pack:")
        ));
        assert!(matches!(
            &outgoing[3],
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["sessionUpdate"] == "semantic_diff"
        ));
        let response = match &outgoing[5] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "dry_run_validated");
        assert_eq!(
            result["output"]["dry_run"]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
    }

    #[test]
    fn session_prompt_kyc_missing_evidence_returns_structured_refusal() {
        let mut agent = AcpJsonRpcAgent::new();
        let case_state = json!({
            "case_state": {
                "subject_id": CASE_ID,
                "current_state": "INTAKE",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1"
            }
        });

        let outgoing = agent.handle_request(request(
            1,
            "session/prompt",
            json!({
                "sessionId": SESSION_ID.to_string(),
                "prompt": [
                    {
                        "type": "text",
                        "text": "Move the KYC case to discovery"
                    },
                    {
                        "type": "embedded_resource",
                        "uri": format!("semos://entity/{}", CASE_ID),
                        "name": "KYC case state",
                        "mime_type": "application/json",
                        "text": case_state.to_string()
                    }
                ]
            }),
        ));

        assert_eq!(outgoing.len(), 5);
        assert!(!outgoing.iter().any(|item| matches!(
            item,
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["sessionUpdate"] == "semantic_diff"
        )));
        let response = match &outgoing[4] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "structured_refusal");
        assert_eq!(result["refusal"]["refusal_code"], "missing_evidence_digest");
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

        let result = response.result.as_ref().unwrap();
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

        let result = response.result.as_ref().unwrap();
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
        assert!(result["policy"]["projection_catalog"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "lineage"));
        assert!(result["policy"]["authority_surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .any(|surface| surface["surface"] == "fs/write_text_file"
                && surface["permitted"] == false));
    }

    #[test]
    fn extension_projection_list_exposes_declared_catalogue() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/projections/list",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed"
            }),
        )));

        let result = response.result.unwrap();
        assert_eq!(result["status"], "acp_projection_catalog");
        assert!(result["projections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "dag"));
        assert!(result["projections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "verb_surface"));
        assert!(result["projections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "language_pack"));
    }

    #[test]
    fn extension_projection_get_returns_hashed_envelope() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/projection/get",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "kind": "transition_surface",
                "subject": {
                    "subject_kind": "kyc_case",
                    "subject_id": CASE_ID.to_string()
                }
            }),
        )));

        let result = response.result.unwrap();
        let projection = &result["projection"];
        assert_eq!(projection["projection_kind"], "transition_surface");
        assert_eq!(projection["pack_id"], "ob-poc.kyc");
        assert!(projection["projection_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert_eq!(result["observability"]["projectionCount"], 1);
        assert!(result["observability"]["projectionBytes"].as_u64().unwrap() > 0);
        assert!(result["observability"]["acpMechanismSummary"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mechanism| mechanism == "demand_driven"));
        assert_eq!(
            projection["payload"]["transitions"][0]["transition_ref"],
            "kyc-case.intake-to-discovery"
        );
        assert_eq!(
            projection["payload"]["language_pack_readiness"][0]["generator"],
            "kyc_update_status_language_pack_v1"
        );
    }

    #[test]
    fn extension_projection_get_returns_language_pack_envelope() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/projection/get",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "kind": "language_pack",
                "subject": {
                    "subject_kind": "kyc_case",
                    "subject_id": CASE_ID.to_string()
                },
                "current_state": "DISCOVERY",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "objective": "Move the KYC case to assessment"
            }),
        )));

        let result = response.result.as_ref().unwrap();
        let projection = &result["projection"];
        assert_eq!(projection["projection_kind"], "language_pack");
        assert_eq!(projection["payload"]["current_state"], "DISCOVERY");
        assert_eq!(
            projection["payload"]["candidate_transitions"][0]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
        assert!(projection["projection_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
    }

    #[test]
    fn extension_discovers_case_state_for_language_pack_anchors() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/kyc_case_state/discover",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "subject_id": CASE_ID,
                "observations": [
                    {
                        "key": "case.status",
                        "value": "DISCOVERY",
                        "classification": "internal"
                    },
                    {
                        "key": "case.configuration_version",
                        "value": "config-live-1",
                        "classification": "internal"
                    }
                ],
                "provenance": [
                    {
                        "source": "sem_os.session_state",
                        "snapshot_ref": "snapshot-live-1"
                    }
                ]
            }),
        )));

        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "kyc_case_state_discovered");
        assert_eq!(result["case_state"]["current_state"], "DISCOVERY");
        assert_eq!(
            result["language_pack_request"]["state_snapshot_id"],
            "snapshot-live-1"
        );
        assert!(result["observability"]["acpMechanismSummary"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mechanism| mechanism == "read_only_discovery_probe"));
    }

    #[test]
    fn extension_language_pack_get_returns_bounded_private_dsl_context() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/language_pack/get",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "subject_id": CASE_ID,
                "current_state": "INTAKE",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1"
            }),
        )));

        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "sem_os_language_pack");
        assert_eq!(result["language_pack"]["pack_id"], "ob-poc.kyc");
        assert_eq!(result["language_pack"]["subject"]["kind"], "kyc_case");
        assert_eq!(
            result["language_pack"]["valid_verbs"][0]["verb"],
            "kyc-case.update-status"
        );
        assert_eq!(
            result["language_pack"]["candidate_transitions"][0]["transition_ref"],
            "kyc-case.intake-to-discovery"
        );
        assert!(
            result["language_pack"]["canonical_patterns"]
                .as_array()
                .unwrap()
                .len()
                >= 3
        );
    }

    #[test]
    fn extension_language_loop_emits_visible_trace_before_dry_run() {
        let mut agent = AcpJsonRpcAgent::new();
        let outgoing = agent.handle_request(request(
            1,
            "obpoc/kyc_update_status_language_loop",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "subject_id": CASE_ID,
                "current_state": "INTAKE",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "draft": {
                    "session_id": SESSION_ID,
                    "actor_id": "analyst@example.com",
                    "actor_roles": ["analyst"],
                    "verb": "kyc-case.update-status",
                    "transition_ref": "kyc-case.review-to-approved",
                    "subject_kind": "kyc_case",
                    "case_id": CASE_ID,
                    "current_state": "INTAKE",
                    "requested_state": "DISCOVERY",
                    "configuration_version": "config-1",
                    "state_snapshot_id": "snapshot-1",
                    "evidence_digest": "sha256:evidence"
                }
            }),
        ));

        assert_eq!(outgoing.len(), 6);
        assert!(matches!(
            &outgoing[0],
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["sessionUpdate"] == "tool_call_update"
                    && notification.params["update"]["toolCallId"]
                        .as_str()
                        .unwrap()
                        .starts_with("tool:language-pack:")
        ));
        let plan_update = match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected plan notification"),
        };
        assert_eq!(plan_update["sessionUpdate"], "plan");
        assert_eq!(
            plan_update["goalProposalTrace"]["status"],
            "dry_run_validated"
        );
        assert_eq!(plan_update["goalProposalTrace"]["revisionCount"], 1);
        assert!(matches!(
            &outgoing[3],
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["sessionUpdate"] == "semantic_diff"
        ));
        let response = match &outgoing[5] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "dry_run_validated");
        assert_eq!(result["metrics"]["revision_count"], 1);
        assert_eq!(result["metrics"]["dry_run_valid"], true);
    }

    #[test]
    fn extension_language_loop_returns_structured_refusal_without_semantic_diff() {
        let mut agent = AcpJsonRpcAgent::new();
        let outgoing = agent.handle_request(request(
            1,
            "obpoc/kyc_update_status_language_loop",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "subject_id": CASE_ID,
                "current_state": "INTAKE",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "draft": {
                    "session_id": SESSION_ID,
                    "actor_id": "analyst@example.com",
                    "actor_roles": ["analyst"],
                    "verb": "kyc-case.force-approve",
                    "transition_ref": "kyc-case.intake-to-discovery",
                    "subject_kind": "kyc_case",
                    "case_id": CASE_ID,
                    "current_state": "INTAKE",
                    "requested_state": "DISCOVERY",
                    "configuration_version": "config-1",
                    "state_snapshot_id": "snapshot-1",
                    "evidence_digest": "sha256:evidence"
                }
            }),
        ));

        assert_eq!(outgoing.len(), 5);
        assert!(!outgoing.iter().any(|item| matches!(
            item,
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["sessionUpdate"] == "semantic_diff"
        )));
        let response = match &outgoing[4] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "structured_refusal");
        assert_eq!(result["refusal"]["refusal_code"], "invented_verb");
        assert_eq!(result["metrics"]["invented_verb_count"], 1);
        assert_eq!(result["metrics"]["dry_run_valid"], false);
    }

    #[test]
    fn extension_kyc_dry_run_uses_existing_workbook_gate() {
        let mut agent = AcpJsonRpcAgent::new();
        let outgoing = agent.handle_request(request(
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
        ));

        assert_eq!(outgoing.len(), 3);
        assert!(matches!(
            &outgoing[0],
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["sessionUpdate"] == "tool_call_update"
        ));
        assert!(matches!(
            &outgoing[1],
            JsonRpcOutgoing::Notification(notification)
                if notification.params["update"]["sessionUpdate"] == "semantic_diff"
        ));
        let semantic_update = match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected semantic diff notification"),
        };
        assert!(semantic_update["semanticDiffId"]
            .as_str()
            .unwrap()
            .starts_with("semos://semantic-diff/ewb:v1:"));
        assert!(semantic_update["validationTrace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["step_id"] == "integrity"));
        let response = match &outgoing[2] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "dry_run_validated");
        assert_eq!(
            result["output"]["dry_run"]["transition_ref"],
            "kyc-case.intake-to-discovery"
        );
        assert!(result["output"]["dry_run"]["semantic_diff_uri"]
            .as_str()
            .unwrap()
            .starts_with("semos://semantic-diff/ewb:v1:"));
    }

    #[test]
    fn mvp_dry_run_vertical_slice_preserves_acp_policy_and_refuses_mutation() {
        let mut agent = AcpJsonRpcAgent::new();

        let initialized = only_response(agent.handle_request(request(1, "initialize", json!({}))));
        assert_eq!(
            initialized.result.as_ref().unwrap()["obpocCapabilities"]["pack"]["pack_id"],
            "ob-poc.kyc"
        );

        let loaded = only_response(agent.handle_request(request(
            2,
            "session/load",
            json!({
                "sessionId": SESSION_ID.to_string(),
                "persona": "sage:execution"
            }),
        )));
        assert_eq!(
            loaded.result.as_ref().unwrap()["modeState"]["currentModeId"],
            "sage:execution"
        );

        let context = only_response(agent.handle_request(request(
            3,
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
        assert_eq!(
            context.result.as_ref().unwrap()["bundle"]["prompt_context"]["included"][0]["key"],
            "case.status"
        );
        assert_eq!(
            context.result.as_ref().unwrap()["bundle"]["prompt_context"]["redacted"][0]["key"],
            "case.confidential_evidence.summary"
        );

        let projection = only_response(agent.handle_request(request(
            4,
            "obpoc/projection/get",
            json!({
                "session_id": SESSION_ID.to_string(),
                "kind": "transition_surface",
                "subject": {
                    "subject_kind": "kyc_case",
                    "subject_id": CASE_ID.to_string()
                }
            }),
        )));
        assert!(
            projection.result.as_ref().unwrap()["projection"]["projection_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );

        let dry_run = agent.handle_request(request(
            5,
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
        ));
        assert_eq!(dry_run.len(), 3);
        let semantic_update = match &dry_run[1] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected semantic diff notification"),
        };
        assert!(semantic_update["semanticDiffId"]
            .as_str()
            .unwrap()
            .starts_with("semos://semantic-diff/ewb:v1:"));
        let validation_trace = semantic_update["validationTrace"].as_array().unwrap();
        assert!(validation_trace.len() >= 4);
        assert!(validation_trace
            .iter()
            .any(|step| step["step_id"] == "integrity"));
        assert!(validation_trace
            .iter()
            .any(|step| step["step_id"] == "dry-run"));
        let dry_run_response = match &dry_run[2] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected dry-run response"),
        };
        assert_eq!(
            dry_run_response.result.as_ref().unwrap()["output"]["workbook"]["core"]
                ["execution_mode"],
            "dry_run"
        );

        let mutation = only_response(agent.handle_request(request(
            6,
            "obpoc/mutation",
            json!({"session_id": SESSION_ID.to_string()}),
        )));
        assert_eq!(mutation.error.as_ref().unwrap().code, INVALID_REQUEST);
        assert_eq!(mutation.error.unwrap().data.unwrap()["capability"], "none");
    }

    #[test]
    fn execution_authority_methods_are_explicitly_refused() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(1, "terminal/new", json!({}))));

        let error = response.error.unwrap();
        assert_eq!(error.code, INVALID_REQUEST);
        assert_eq!(error.data.unwrap()["capability"], "none");
    }

    #[test]
    fn permission_request_is_hitl_only() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/request_permission",
            json!({
                "session_id": SESSION_ID.to_string(),
                "persona": "sage:execution",
                "workbook_id": "ewb:v1:test",
                "workbook_hash": "sha256:workbook",
                "state_snapshot_id": "snapshot-1",
                "evidence_refs": [{"kind": "case_id", "ref_id": CASE_ID.to_string()}]
            }),
        )));

        let result = response.result.unwrap();
        assert_eq!(result["status"], "permission_request_created");
        assert_eq!(result["scope"], "hitl_approval_only");
        assert_eq!(result["execution_authority"], false);
        assert_eq!(result["persona"], "sage:execution");
        assert_eq!(
            result["approval_binding"]["workbook_hash"],
            "sha256:workbook"
        );
    }

    #[test]
    fn planning_persona_cannot_request_permission() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/request_permission",
            json!({
                "session_id": SESSION_ID.to_string(),
                "persona": "sage:planning"
            }),
        )));

        let error = response.error.unwrap();
        assert_eq!(error.code, INVALID_REQUEST);
        assert_eq!(
            error.data.unwrap()["requiredPersona"],
            AcpPersonaMode::SageExecution.as_str()
        );
    }

    #[test]
    fn mutation_extension_is_explicitly_refused() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(1, "obpoc/mutation", json!({}))));

        assert_eq!(response.error.unwrap().code, INVALID_REQUEST);
    }
}
