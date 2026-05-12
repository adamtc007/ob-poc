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

use crate::acp::{self, AcpAdapterKind, AcpKycCaseStateSnapshot, AcpPersonaMode, AcpSession};
use crate::acp_dag_semantic::{
    resolve_acp_dag_semantic_prompt_with_verified_envelopes, AcpDagSemanticResolution,
    AcpDagSemanticStatus,
};
use crate::runbook::{
    KycLanguagePackRequest, KycUpdateStatusDryRunInput, KycUpdateStatusDryRunOutput,
    KycUpdateStatusWorkbookDraft, LanguageAcquisitionMetrics, SemOsLanguagePack,
    StructuredWorkbookRefusal, WorkbookDiagnostic, WorkbookDraftAttempt, WorkbookRevisionOutcome,
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
    #[serde(default)]
    pub verb: Option<String>,
    #[serde(default)]
    pub subject_uuid_field: Option<String>,
    #[serde(default)]
    pub state_field: Option<String>,
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
    #[serde(default = "default_language_pack_subject_kind")]
    pub subject_kind: String,
    #[serde(default = "default_language_pack_verb")]
    pub verb: String,
    pub current_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default)]
    pub subject_uuid_field: Option<String>,
    #[serde(default)]
    pub state_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub prompt_route_ms: Option<u64>,
    #[serde(default)]
    pub prompt_route_us: Option<u64>,
    #[serde(default)]
    pub state_discovery: Option<Value>,
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

fn default_language_pack_subject_kind() -> String {
    "kyc_case".to_string()
}

fn default_language_pack_verb() -> String {
    "kyc-case.update-status".to_string()
}

fn default_language_pack_verb_for_subject_kind(subject_kind: &str) -> String {
    match subject_kind {
        "kyc_case" => default_language_pack_verb(),
        other => {
            let namespace = other
                .trim_end_matches("_case")
                .replace('_', "-")
                .trim_matches('-')
                .to_string();
            format!("{namespace}.update-status")
        }
    }
}

#[derive(Debug, Default)]
pub struct AcpJsonRpcAgent {
    sessions: BTreeMap<Uuid, AcpSession>,
    cancelled_sessions: BTreeSet<Uuid>,
    case_state_cache: BTreeMap<(Uuid, Uuid), AcpKycCaseStateSnapshot>,
}

impl AcpJsonRpcAgent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kyc_update_status_language_loop_request_from_prompt(
        &self,
        session_id: Uuid,
        prompt: &[AcpContentBlock],
    ) -> Result<AcpKycLanguageLoopRequest, Vec<&'static str>> {
        let subject_hint = prompt_subject_id(prompt);
        let cached_case_state = subject_hint.and_then(|subject_id| {
            self.case_state_cache
                .get(&(session_id, subject_id))
                .cloned()
        });
        let request =
            kyc_language_loop_request_from_prompt(session_id, prompt, cached_case_state.as_ref())?;
        Ok(serde_json::from_value(request).expect("prompt request shape is typed"))
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
            // `request_permission` is the ACP-standard method name; the
            // `obpoc/` prefix is the namespaced form. Both dispatch to the
            // same handler so ACP clients can use either convention.
            "request_permission" | "obpoc/request_permission" => {
                self.obpoc_request_permission(id, request.params)
            }
            // Explicit-refuse list. These ACP-standard method names cover
            // editor file/terminal authority that ACP visibility never grants
            // here. Variants (`write_text_file` vs `fs/write_text_file`,
            // `terminal/new` vs `terminal/create`) exist because different
            // ACP client implementations use different forms — we refuse
            // every form so clients get a structured authority-denied error
            // rather than a generic METHOD_NOT_FOUND from the catch-all.
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
        if let Some(outgoing) =
            self.try_session_prompt_dag_semantic(id.clone(), session_id, &request.prompt)
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

    fn try_session_prompt_dag_semantic(
        &mut self,
        id: Option<Value>,
        session_id: Uuid,
        prompt: &[AcpContentBlock],
    ) -> Option<Vec<JsonRpcOutgoing>> {
        let route_started_at = Instant::now();
        let utterance_text = prompt_utterance_text(prompt);
        let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
        let resolution = match resolve_acp_dag_semantic_prompt_with_verified_envelopes(
            &utterance_text,
            config_root,
        ) {
            Ok(Some(resolution)) => resolution,
            Ok(None) | Err(_) => return None,
        };
        let route_us = elapsed_us(route_started_at);
        Some(self.dag_semantic_outgoing(id, session_id, resolution, route_us))
    }

    fn dag_semantic_outgoing(
        &self,
        id: Option<Value>,
        session_id: Uuid,
        resolution: AcpDagSemanticResolution,
        route_us: u64,
    ) -> Vec<JsonRpcOutgoing> {
        let candidate_verbs = resolution
            .top_candidates
            .iter()
            .map(|candidate| candidate.fqn.clone())
            .collect::<Vec<_>>();
        let selected_or_top = resolution
            .selected_verb
            .clone()
            .or_else(|| candidate_verbs.first().cloned())
            .unwrap_or_else(|| "unknown".to_string());
        let diagnostic_codes = resolution
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.error_code.clone())
            .collect::<Vec<_>>();
        let structured_failure_mode = dag_semantic_failure_mode(&resolution);
        let is_blocked = structured_failure_mode.is_some();
        let status = match resolution.status {
            AcpDagSemanticStatus::Refused => "structured_refusal",
            AcpDagSemanticStatus::Ambiguous => "pending_question",
            AcpDagSemanticStatus::Matched if structured_failure_mode.is_some() => {
                "pending_question"
            }
            AcpDagSemanticStatus::Matched => "dag_semantic_proposal",
        };
        let workflow_phase = match status {
            "pending_question" => "clarification",
            "structured_refusal" => "refusal",
            "dag_semantic_proposal" => "planning",
            _ => "planning",
        };
        let resource_uri = resolution
            .pack
            .as_ref()
            .map(|pack| format!("semos://journey-pack/{}", pack.pack_id))
            .or_else(|| {
                resolution
                    .selected_verb
                    .as_ref()
                    .map(|verb| format!("semos://verb/{verb}"))
            })
            .unwrap_or_else(|| format!("semos://dag-semantic/{session_id}"));
        let resource_name = resolution
            .pack
            .as_ref()
            .map(|pack| pack.pack_name.clone())
            .unwrap_or_else(|| selected_or_top.clone());
        let pack_trace = resolution.pack.clone();
        let registry_trace = resolution.registry_trace.clone();
        let envelope_trace = resolution.envelope_trace.clone();
        let runtime_trace = resolution.runtime_trace.clone();
        let workflow_plan_trace = resolution.workflow_plan.clone();
        let template_trace = resolution.selected_template.clone();
        let draft_dsl = resolution.draft_dsl.clone();
        let first_pass_valid_dsl_draft = resolution.status == AcpDagSemanticStatus::Matched
            && resolution.draft_dsl.is_some()
            && resolution.missing_required_args.is_empty()
            && resolution.unresolved_refs.is_empty();
        let message = dag_semantic_human_message(&resolution);
        let semantic_resolution =
            serde_json::to_value(&resolution).expect("ACP DAG semantic resolution serializes");
        let mut acp_mechanisms = vec![
            "dag_semantic_router",
            "journey_pack_context",
            "authored_verb_config",
            "dsl_draft_projection",
            "dry_run_only",
            "no_mutation",
        ];
        if registry_trace.is_some() {
            acp_mechanisms.push("verified_registry_state_v2");
        }
        if envelope_trace.is_some() {
            acp_mechanisms.push("verified_pack_context_envelope_v2");
        }
        if runtime_trace.is_some() {
            acp_mechanisms.push("runtime_context_projection_v1");
        }
        let refusal = (resolution.status == AcpDagSemanticStatus::Refused).then(|| {
            let code = diagnostic_codes
                .first()
                .cloned()
                .unwrap_or_else(|| "dag_semantic_refused".to_string());
            let reason = resolution
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.clone())
                .unwrap_or_else(|| "The utterance is not allowed on this route".to_string());
            json!({
                "refusal_code": code,
                "reason": reason,
                "selected_verb": resolution.selected_verb,
                "pack": resolution.pack
            })
        });

        vec![
            JsonRpcOutgoing::Notification(JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": format!("tool:dag-semantic:{session_id}"),
                        "status": "completed",
                        "kind": "read",
                        "persona": AcpPersonaMode::SagePlanning.as_str(),
                        "workflowPhase": "discovery",
                        "title": "ACP DAG semantic surface",
                        "content": {
                            "type": "resource_link",
                            "uri": resource_uri,
                            "name": resource_name,
                            "description": "Authored journey-pack and DSL verb projection; read-only ACP proposal, no mutation"
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
                        "workflowPhase": workflow_phase,
                        "goalProposalTrace": {
                            "status": status,
                            "pack": pack_trace,
                            "registryTrace": registry_trace,
                            "envelopeTrace": envelope_trace,
                            "runtimeTrace": runtime_trace,
                            "workflowPlan": workflow_plan_trace,
                            "selectedTemplate": template_trace,
                            "selectedVerb": resolution.selected_verb.clone(),
                            "draftDsl": draft_dsl.clone(),
                            "refusal": refusal.clone(),
                            "candidateVerbs": candidate_verbs.clone(),
                            "missingRequiredArgs": resolution.missing_required_args.clone(),
                            "unresolvedRefs": resolution.unresolved_refs.clone(),
                            "structuredFailureMode": structured_failure_mode.clone(),
                            "diagnosticCodes": diagnostic_codes.clone(),
                            "firstPassValid": first_pass_valid_dsl_draft,
                            "dryRunValid": false,
                            "mutationExecuted": false,
                            "proseOnlyFailure": false,
                            "acpMechanismSummary": acp_mechanisms,
                            "acpFallbackSummary": [],
                            "projectionCount": 1,
                            "projectionLatencyMs": millis_from_micros(route_us),
                            "projectionLatencyUs": route_us
                        },
                        "entries": [
                            {"id": "resolve", "status": "completed", "label": "Resolve utterance against authored DSL/DAG verbs"},
                            {"id": "draft", "status": "completed", "label": "Build non-executing DSL workbook proposal"},
                            {"id": "bind", "status": if is_blocked { "blocked" } else { "in_progress" }, "label": "Collect required bindings and HITL approval"},
                            {"id": "execute", "status": "blocked", "label": "Execution remains behind workbook/runbook gate"}
                        ]
                    }
                }),
            }),
            agent_message_update(session_id, message),
            JsonRpcOutgoing::Response(JsonRpcResponse::success(
                id,
                json!({
                    "stopReason": "end_turn",
                    "status": status,
                    "refusal": refusal,
                    "dsl": draft_dsl,
                    "traceProjection": {
                        "outcome": status,
                        "selectedVerb": resolution.selected_verb.clone(),
                        "selectedTemplate": resolution.selected_template.clone(),
                        "draftDsl": resolution.draft_dsl.clone(),
                        "firstPassValid": first_pass_valid_dsl_draft,
                        "dryRunValid": false,
                        "mutationExecuted": false,
                        "proseOnlyFailure": false,
                        "structuredFailureMode": structured_failure_mode.clone(),
                        "diagnosticCodes": diagnostic_codes.clone(),
                        "registryTrace": resolution.registry_trace.clone(),
                        "envelopeTrace": resolution.envelope_trace.clone(),
                        "runtimeTrace": resolution.runtime_trace.clone(),
                        "neededFromUser": resolution.missing_required_args.clone()
                    },
                    "dag_semantic": semantic_resolution,
                    "observability": {
                        "projectionCount": 1,
                        "performance": {
                            "prompt_route_ms": millis_from_micros(route_us),
                            "prompt_route_us": route_us,
                            "acp_emit_ms": 0,
                            "acp_emit_us": 0,
                            "total_ms": millis_from_micros(route_us),
                            "total_us": route_us
                        },
                        "conversationEfficiency": {
                            "proseOnlyFailure": false,
                            "structuredFailureMode": structured_failure_mode.clone(),
                            "candidateVerbCount": candidate_verbs.len()
                        },
                        "acpMechanismSummary": acp_mechanisms
                    }
                }),
            )),
        ]
    }

    fn try_session_prompt_kyc_update_status(
        &mut self,
        id: Option<Value>,
        session_id: Uuid,
        prompt: &[AcpContentBlock],
    ) -> Option<Vec<JsonRpcOutgoing>> {
        let prompt_route_started_at = Instant::now();
        let utterance_text = prompt_utterance_text(prompt);
        if !looks_like_kyc_update_status_prompt(&utterance_text) {
            if looks_like_kyc_domain_prompt(&utterance_text) {
                let prompt_route_us = elapsed_us(prompt_route_started_at);
                let candidate_verbs = vec![
                    "kyc-case.read",
                    "kyc-case.create",
                    "kyc-case.update-status",
                    "screening.update-status",
                    "document.collect",
                ];
                let message = explain_kyc_ambiguous_prompt(&candidate_verbs);
                return Some(pending_question_outgoing(
                    id,
                    session_id,
                    json!({
                        "stopReason": "end_turn",
                        "status": "pending_question",
                        "pending_question": {
                            "code": "kyc_prompt_ambiguous",
                            "candidate_verbs": candidate_verbs,
                            "needs": ["explicit_verb_or_update_status_intent"]
                        },
                        "observability": {
                            "performance": {
                                "prompt_route_ms": millis_from_micros(prompt_route_us),
                                "prompt_route_us": prompt_route_us,
                                "language_pack_ms": 0,
                                "language_pack_us": 0,
                                "revision_loop_ms": 0,
                                "revision_loop_us": 0,
                                "dry_run_ms": 0,
                                "dry_run_us": 0,
                                "acp_emit_ms": 0,
                                "acp_emit_us": 0,
                                "total_ms": millis_from_micros(prompt_route_us),
                                "total_us": prompt_route_us
                            },
                            "conversationEfficiency": pending_question_conversation_efficiency("kyc_prompt_ambiguous"),
                            "acpMechanismSummary": ["prompt_router", "structured_pending_question"]
                        }
                    }),
                    "kyc_prompt_ambiguous",
                    message,
                ));
            }
            return None;
        }

        let request = match self
            .kyc_update_status_language_loop_request_from_prompt(session_id, prompt)
        {
            Ok(request) => request,
            Err(error) => {
                let prompt_route_us = elapsed_us(prompt_route_started_at);
                let message = explain_kyc_incomplete_prompt(&error);
                return Some(pending_question_outgoing(
                    id,
                    session_id,
                    json!({
                        "stopReason": "end_turn",
                        "status": "pending_question",
                        "pending_question": {
                            "code": "kyc_update_status_prompt_incomplete",
                            "missing": error,
                            "needs": [
                                "case_uuid",
                                "current_state",
                                "configuration_version",
                                "state_snapshot_id",
                                "requested_state",
                                "evidence_digest"
                            ]
                        },
                        "observability": {
                            "performance": {
                                "prompt_route_ms": millis_from_micros(prompt_route_us),
                                "prompt_route_us": prompt_route_us,
                                "language_pack_ms": 0,
                                "language_pack_us": 0,
                                "revision_loop_ms": 0,
                                "revision_loop_us": 0,
                                "dry_run_ms": 0,
                                "dry_run_us": 0,
                                "acp_emit_ms": 0,
                                "acp_emit_us": 0,
                                "total_ms": millis_from_micros(prompt_route_us),
                                "total_us": prompt_route_us
                            },
                            "conversationEfficiency": pending_question_conversation_efficiency("kyc_update_status_prompt_incomplete"),
                            "acpMechanismSummary": ["prompt_router", "structured_pending_question"]
                        }
                    }),
                    "kyc_update_status_prompt_incomplete",
                    message,
                ));
            }
        };

        let prompt_route_us = elapsed_us(prompt_route_started_at);
        let mut request = request;
        request.prompt_route_ms = Some(millis_from_micros(prompt_route_us));
        request.prompt_route_us = Some(prompt_route_us);
        let request = serde_json::to_value(request).expect("typed prompt request serializes");
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
        let facade = match crate::acp_facade::AcpFacade::for_default_pack(request.adapter) {
            Ok(facade) => facade,
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
            pack_id: facade.manifest().pack_id.clone(),
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
        match facade.context_assemble_for(&session, discovery_request, discovery_response) {
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
        let facade = match crate::acp_facade::AcpFacade::for_default_pack(adapter) {
            Ok(facade) => facade,
            Err(error) => return self.acp_error(id, error),
        };
        let session = self
            .sessions
            .entry(session_id)
            .or_insert_with(|| acp::open_acp_session(session_id, adapter))
            .clone();

        match facade.policy_for(&session) {
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
        let facade = match crate::acp_facade::AcpFacade::for_default_pack(adapter) {
            Ok(facade) => facade,
            Err(error) => return self.acp_error(id, error),
        };
        let pack_id = facade.manifest().pack_id.clone();
        let session = self
            .sessions
            .entry(session_id)
            .or_insert_with(|| acp::open_acp_session(session_id, adapter))
            .clone();

        match facade.projections_list_for(&session) {
            Ok(projections) => self.response(
                id,
                json!({
                    "status": "acp_projection_catalog",
                    "session_id": session_id,
                    "pack_id": pack_id,
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
            Some(crate::runbook::UpdateStatusLanguagePackRequest {
                subject_id,
                subject_kind: subject.subject_kind.clone(),
                verb: request.verb.unwrap_or_else(|| {
                    default_language_pack_verb_for_subject_kind(&subject.subject_kind)
                }),
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
                subject_uuid_field: request.subject_uuid_field,
                state_field: request.state_field,
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
                self.case_state_cache
                    .insert((session_id, case_state.subject_id), case_state.clone());
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
                            "projectionLatencyMs": elapsed_ms(started_at)
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
        match acp::acp_update_status_language_pack(
            &session,
            &manifest,
            crate::runbook::UpdateStatusLanguagePackRequest {
                subject_id: request.subject_id,
                subject_kind: request.subject_kind,
                verb: request.verb,
                current_state: request.current_state,
                configuration_version: request.configuration_version,
                state_snapshot_id: request.state_snapshot_id,
                objective: request.objective,
                subject_uuid_field: request.subject_uuid_field,
                state_field: request.state_field,
            },
        ) {
            Ok(language_pack) => {
                let language_pack_us = elapsed_us(started_at);
                let language_pack_ms = millis_from_micros(language_pack_us);
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
                            "projectionLatencyMs": language_pack_ms,
                            "performance": {
                                "prompt_route_ms": 0,
                                "prompt_route_us": 0,
                                "language_pack_ms": language_pack_ms,
                                "language_pack_us": language_pack_us,
                                "revision_loop_ms": 0,
                                "revision_loop_us": 0,
                                "dry_run_ms": 0,
                                "dry_run_us": 0,
                                "acp_emit_ms": 0,
                                "acp_emit_us": 0,
                                "total_ms": language_pack_ms,
                                "total_us": language_pack_us
                            }
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
        let prompt_route_us = request
            .prompt_route_us
            .unwrap_or_else(|| request.prompt_route_ms.unwrap_or(0).saturating_mul(1_000));
        let state_discovery = request.state_discovery.clone();

        let started_at = Instant::now();
        let outcome = match acp::acp_run_kyc_update_status_language_loop_timed(
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

        let language_pack = outcome.language_pack;
        let revision_outcome = outcome.revision_outcome;
        let timings = outcome.timings;
        let mut trace_projection =
            language_loop_trace_projection(&language_pack, &revision_outcome);
        attach_state_discovery(&mut trace_projection, state_discovery.as_ref());
        let acp_emit_started_at = Instant::now();
        let mut outgoing = Vec::new();
        if let Some(notification) =
            state_discovery_tool_update(session_id, state_discovery.as_ref(), &trace_projection)
        {
            outgoing.push(notification);
        }
        outgoing.extend([
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
                        "goalProposalTrace": language_loop_trace_summary(&revision_outcome, &trace_projection),
                        "entries": language_loop_plan_entries(&revision_outcome)
                    }
                }),
            }),
        ]);

        match revision_outcome {
            WorkbookRevisionOutcome::DryRunValid {
                output,
                attempts,
                metrics,
                trace,
            } => {
                let human_summary = trace_projection["humanSummary"]
                    .as_str()
                    .unwrap_or("I validated a dry-run workbook; no mutation was executed.");
                let discovery_summary =
                    state_discovery_human_summary(trace_projection.get("stateDiscovery"));
                let explanation = format!(
                    "{}{} {}",
                    discovery_summary,
                    human_summary,
                    explain_kyc_dry_run_success(output.as_ref(), &metrics)
                );
                let acp_emit_us = elapsed_us(acp_emit_started_at);
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
                            "traceProjection": trace_projection.clone(),
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
                outgoing.push(agent_message_update(session_id, explanation));
                outgoing.push(JsonRpcOutgoing::Response(JsonRpcResponse::success(
                    id,
                    json!({
                        "status": "dry_run_validated",
                        "language_pack": language_pack,
                        "output": output,
                        "attempts": attempts,
                        "metrics": metrics,
                        "trace": trace,
                        "prompt_context_variant": {
                            "id": trace_projection["promptContextVariant"].clone()
                        },
                        "traceProjection": trace_projection,
                        "observability": {
                            "projectionLatencyMs": elapsed_ms(started_at),
                            "performance": language_loop_performance(&timings, prompt_route_us, acp_emit_us),
                            "conversationEfficiency": language_loop_conversation_efficiency(
                                &metrics,
                                "dry_run_validated",
                                None
                            ),
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
                let human_summary = trace_projection["humanSummary"]
                    .as_str()
                    .unwrap_or("I stopped with a structured refusal; no mutation was executed.");
                let discovery_summary =
                    state_discovery_human_summary(trace_projection.get("stateDiscovery"));
                let explanation = format!(
                    "{}{} {}",
                    discovery_summary,
                    human_summary,
                    explain_kyc_refusal(&refusal)
                );
                let acp_emit_us = elapsed_us(acp_emit_started_at);
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
                            "traceProjection": trace_projection.clone(),
                            "content": {
                                "type": "embedded_resource",
                                "uri": format!("semos://diagnostic/{}", refusal.refusal_code),
                                "name": "Structured refusal",
                                "text": serde_json::to_string(&refusal).unwrap_or_default()
                            }
                        }
                    }),
                }));
                outgoing.push(agent_message_update(session_id, explanation));
                outgoing.push(JsonRpcOutgoing::Response(JsonRpcResponse::success(
                    id,
                    json!({
                        "status": "structured_refusal",
                        "language_pack": language_pack,
                        "refusal": refusal,
                        "attempts": attempts,
                        "metrics": metrics,
                        "trace": trace,
                        "prompt_context_variant": {
                            "id": trace_projection["promptContextVariant"].clone()
                        },
                        "traceProjection": trace_projection,
                        "observability": {
                            "projectionLatencyMs": elapsed_ms(started_at),
                            "performance": language_loop_performance(&timings, prompt_route_us, acp_emit_us),
                            "conversationEfficiency": language_loop_conversation_efficiency(
                                &metrics,
                                "structured_refusal",
                                Some(refusal.refusal_code.as_str())
                            ),
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
        let dry_run_started_at = Instant::now();
        match acp::acp_dry_run_kyc_update_status(&session, input) {
            Ok(output) => {
                let dry_run_us = elapsed_us(dry_run_started_at);
                let dry_run_ms = millis_from_micros(dry_run_us);
                vec![
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
                        json!({
                            "status": "dry_run_validated",
                            "output": output,
                            "observability": {
                                "performance": {
                                    "prompt_route_ms": 0,
                                    "prompt_route_us": 0,
                                    "language_pack_ms": 0,
                                    "language_pack_us": 0,
                                    "revision_loop_ms": 0,
                                    "revision_loop_us": 0,
                                    "dry_run_ms": dry_run_ms,
                                    "dry_run_us": dry_run_us,
                                    "acp_emit_ms": 0,
                                    "acp_emit_us": 0,
                                    "total_ms": dry_run_ms,
                                    "total_us": dry_run_us
                                }
                            }
                        }),
                    )),
                ]
            }
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

fn dag_semantic_failure_mode(resolution: &AcpDagSemanticResolution) -> Option<String> {
    match resolution.status {
        AcpDagSemanticStatus::Refused => resolution
            .diagnostics
            .first()
            .map(|diagnostic| diagnostic.error_code.clone())
            .or_else(|| Some("structured_refusal".to_string())),
        AcpDagSemanticStatus::Ambiguous => Some("ambiguous_verb".to_string()),
        AcpDagSemanticStatus::Matched if !resolution.missing_required_args.is_empty() => {
            Some("missing_required_args".to_string())
        }
        AcpDagSemanticStatus::Matched => None,
    }
}

pub(crate) fn dag_semantic_human_message(resolution: &AcpDagSemanticResolution) -> String {
    let pack_phrase = resolution
        .pack
        .as_ref()
        .map(|pack| format!(" inside the `{}` journey pack", pack.pack_name))
        .unwrap_or_default();
    match resolution.status {
        AcpDagSemanticStatus::Refused => {
            let reason = resolution
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.as_str())
                .unwrap_or("This utterance is not allowed on the Slice 1 route");
            if let Some(verb) = resolution.selected_verb.as_deref() {
                return format!(
                    "I refused this{pack_phrase} for `{verb}`: {reason}. No mutation has run."
                );
            }
            format!("I refused this utterance: {reason}. No mutation has run.")
        }
        AcpDagSemanticStatus::Ambiguous => {
            let candidates = resolution
                .top_candidates
                .iter()
                .take(3)
                .map(|candidate| format!("`{}`", candidate.fqn))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "I found multiple plausible DSL verbs{pack_phrase}: {candidates}. Please choose the intended verb or provide a more specific request. No mutation has run."
            )
        }
        AcpDagSemanticStatus::Matched if !resolution.missing_required_args.is_empty() => {
            let verb = resolution
                .selected_verb
                .as_deref()
                .unwrap_or("the selected DSL verb");
            let missing = resolution
                .missing_required_args
                .iter()
                .map(|arg| format!("`{arg}`"))
                .collect::<Vec<_>>()
                .join(", ");
            if let Some(plan) = &resolution.workflow_plan {
                return format!(
                    "I resolved this{pack_phrase} to `{verb}` and built a read-only `{}` workflow plan, but it is blocked until these required bindings are supplied: {missing}. No mutation has run.",
                    plan.plan_id
                );
            }
            format!(
                "I resolved this{pack_phrase} to `{verb}`, but the workbook draft is blocked until these required bindings are supplied: {missing}. No mutation has run."
            )
        }
        AcpDagSemanticStatus::Matched => {
            let verb = resolution
                .selected_verb
                .as_deref()
                .unwrap_or("the selected DSL verb");
            if let Some(plan) = &resolution.workflow_plan {
                return format!(
                    "I resolved this{pack_phrase} to `{verb}` and built a read-only `{}` workflow plan. No mutation has run; execution remains behind the workbook, HITL, and runbook gates.",
                    plan.plan_id
                );
            }
            format!(
                "I resolved this{pack_phrase} to `{verb}` and drafted a non-executing DSL proposal. No mutation has run; execution remains behind the workbook, HITL, and runbook gates."
            )
        }
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
    cached_case_state: Option<&AcpKycCaseStateSnapshot>,
) -> Result<Value, Vec<&'static str>> {
    let prompt_text = prompt_semantic_text(prompt);
    let utterance_text = prompt_utterance_text(prompt);
    let mut case_state = extract_prompt_case_state(prompt, &utterance_text);
    let subject_id = case_state
        .as_ref()
        .and_then(|state| state.subject_id)
        .or_else(|| prompt_subject_id(prompt));
    if let Some(cached) = cached_case_state {
        if subject_id
            .map(|subject_id| subject_id == cached.subject_id)
            .unwrap_or(true)
        {
            merge_cached_case_state(&mut case_state, cached);
        }
    }

    let subject_id = case_state
        .as_ref()
        .and_then(|state| state.subject_id)
        .or(subject_id);
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
        .or_else(|| extract_token_with_prefix(&prompt_text, "config-"));
    let state_snapshot_id = case_state
        .as_ref()
        .and_then(|state| state.state_snapshot_id.clone())
        .or_else(|| extract_token_with_prefix(&prompt_text, "snapshot-"));

    let mut missing = Vec::new();
    if subject_id.is_none() {
        missing.push("case_uuid");
    }
    if current_state.is_none() {
        missing.push("current_state");
    }
    if configuration_version.is_none() {
        missing.push("configuration_version");
    }
    if state_snapshot_id.is_none() {
        missing.push("state_snapshot_id");
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
    let configuration_version = configuration_version.expect("checked above");
    let state_snapshot_id = state_snapshot_id.expect("checked above");
    let transition_ref = extract_transition_ref(&utterance_text)
        .unwrap_or_else(|| transition_ref_for_states(&current_state, &requested_state));
    let evidence_digest = extract_token_with_prefix(&utterance_text, "sha256:");
    let state_discovery = case_state
        .as_ref()
        .and_then(|state| state_discovery_trace_value(state, subject_id));

    Ok(json!({
        "session_id": session_id.to_string(),
        "adapter": "zed",
        "subject_id": subject_id,
        "current_state": current_state,
        "configuration_version": configuration_version,
        "state_snapshot_id": state_snapshot_id,
        "objective": utterance_text,
        "state_discovery": state_discovery,
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
    source: Option<&'static str>,
    probe_id: Option<String>,
    snapshot_refs: Vec<String>,
}

fn prompt_subject_id(prompt: &[AcpContentBlock]) -> Option<Uuid> {
    prompt.iter().find_map(|block| match block {
        AcpContentBlock::ResourceLink { uri, .. }
        | AcpContentBlock::EmbeddedResource { uri, .. } => parse_semos_resource_uri(uri)
            .filter(|parsed| parsed.resource_kind == "entity")
            .and_then(|parsed| Uuid::parse_str(&parsed.resource_id).ok()),
        AcpContentBlock::Text { text } => extract_first_uuid(text),
    })
}

fn merge_cached_case_state(
    case_state: &mut Option<PromptCaseState>,
    cached: &AcpKycCaseStateSnapshot,
) {
    let state = case_state.get_or_insert_with(PromptCaseState::default);
    state.subject_id.get_or_insert(cached.subject_id);
    state
        .current_state
        .get_or_insert_with(|| cached.current_state.clone());
    state
        .configuration_version
        .get_or_insert_with(|| cached.configuration_version.clone());
    state
        .state_snapshot_id
        .get_or_insert_with(|| cached.state_snapshot_id.clone());
    if state.snapshot_refs.is_empty() {
        state.snapshot_refs = cached.snapshot_refs.clone();
    }
    if state.source.is_none() || state.source == Some("prompt_text_anchor") {
        state.source = Some(cached_case_state_trace_source(cached));
        state.probe_id = Some("kyc-case.read-state".to_string());
    }
}

fn cached_case_state_trace_source(cached: &AcpKycCaseStateSnapshot) -> &'static str {
    if cached.state_snapshot_id.starts_with("postgres:")
        || cached
            .snapshot_refs
            .iter()
            .any(|snapshot_ref| snapshot_ref.starts_with("postgres:"))
    {
        "live_read_only_discovery_probe"
    } else {
        "cached_read_only_discovery_probe"
    }
}

fn state_discovery_trace_value(state: &PromptCaseState, subject_id: Uuid) -> Option<Value> {
    let source = state.source?;
    Some(json!({
        "source": source,
        "probeId": state.probe_id.as_deref().unwrap_or("kyc-case.read-state"),
        "subjectId": subject_id,
        "currentState": state.current_state.as_deref(),
        "configurationVersion": state.configuration_version.as_deref(),
        "stateSnapshotId": state.state_snapshot_id.as_deref(),
        "snapshotRefs": state.snapshot_refs.clone(),
        "firstClassStateMutated": false,
    }))
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
                source: Some("prompt_text_anchor"),
                probe_id: None,
                snapshot_refs: vec![],
            })
        })
}

fn prompt_case_state_from_value(value: &Value) -> Option<PromptCaseState> {
    if let Some(state) = prompt_case_state_from_discovery_value(value) {
        return Some(state);
    }
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
        source: Some("embedded_case_state_anchor"),
        probe_id: None,
        snapshot_refs: vec![],
    })
}

fn prompt_case_state_from_discovery_value(value: &Value) -> Option<PromptCaseState> {
    let source = value
        .get("discovery_response")
        .or_else(|| value.get("discoveryResponse"))
        .unwrap_or(value);
    let observations = source.get("observations")?.as_array()?;
    if source
        .get("first_class_state_mutated")
        .or_else(|| source.get("firstClassStateMutated"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Some(PromptCaseState {
            source: Some("mutated_discovery_rejected"),
            ..PromptCaseState::default()
        });
    }

    let snapshot_refs = source
        .get("provenance")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("snapshot_ref").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(PromptCaseState {
        subject_id: source
            .get("subject")
            .and_then(|subject| subject.get("subject_id"))
            .or_else(|| source.get("subject_id"))
            .or_else(|| source.get("case_id"))
            .and_then(Value::as_str)
            .and_then(|raw| Uuid::parse_str(raw).ok()),
        current_state: observation_value(observations, &["case.status", "kyc_case.status"]),
        configuration_version: observation_value(
            observations,
            &["case.configuration_version", "configuration_version"],
        ),
        state_snapshot_id: observation_value(
            observations,
            &["case.state_snapshot_id", "state_snapshot_id"],
        )
        .or_else(|| snapshot_refs.first().cloned()),
        source: Some("read_only_discovery_probe"),
        probe_id: source
            .get("probe_id")
            .or_else(|| source.get("probeId"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| Some("kyc-case.read-state".to_string())),
        snapshot_refs,
    })
}

fn observation_value(observations: &[Value], keys: &[&str]) -> Option<String> {
    observations
        .iter()
        .find(|observation| {
            observation
                .get("key")
                .and_then(Value::as_str)
                .map(|key| keys.contains(&key))
                .unwrap_or(false)
        })
        .and_then(|observation| observation.get("value"))
        .and_then(Value::as_str)
        .map(str::to_string)
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

fn language_loop_trace_summary(
    outcome: &WorkbookRevisionOutcome,
    trace_projection: &Value,
) -> Value {
    match outcome {
        WorkbookRevisionOutcome::DryRunValid { metrics, trace, .. } => json!({
            "status": "dry_run_validated",
            "acpMechanismSummary": ["language_pack", "deterministic_revision_loop", "dry_run_only"],
            "acpFallbackSummary": [],
            "promptContextVariant": trace_projection["promptContextVariant"].clone(),
            "outcomeLayer": trace_projection["outcomeLayer"].clone(),
            "diagnosticCodes": trace_projection["diagnosticCodes"].clone(),
            "humanSummary": trace_projection["humanSummary"].clone(),
            "revisionCount": metrics.revision_count,
            "decodeRepairCount": 0,
            "firstPassValid": metrics.first_pass_valid,
            "dryRunValid": metrics.dry_run_valid,
            "dryRunMs": metrics.dry_run_ms,
            "dryRunUs": metrics.dry_run_us,
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
            "promptContextVariant": trace_projection["promptContextVariant"].clone(),
            "outcomeLayer": trace_projection["outcomeLayer"].clone(),
            "diagnosticCodes": trace_projection["diagnosticCodes"].clone(),
            "humanSummary": trace_projection["humanSummary"].clone(),
            "revisionCount": metrics.revision_count,
            "decodeRepairCount": 0,
            "firstPassValid": metrics.first_pass_valid,
            "dryRunValid": metrics.dry_run_valid,
            "dryRunMs": metrics.dry_run_ms,
            "dryRunUs": metrics.dry_run_us,
            "refusalCode": refusal.refusal_code,
            "trace": trace
        }),
    }
}

fn language_loop_trace_projection(
    language_pack: &SemOsLanguagePack,
    outcome: &WorkbookRevisionOutcome,
) -> Value {
    match outcome {
        WorkbookRevisionOutcome::DryRunValid {
            output,
            attempts,
            metrics,
            ..
        } => {
            let semantic = &output.dry_run.semantic_diff;
            let diagnostic_codes = diagnostic_codes_from_attempts(attempts);
            let human_summary = language_loop_human_summary_for_dry_run(
                Some(semantic.from_state.as_str()),
                Some(semantic.to_state.as_str()),
                metrics.revision_count,
            );
            json!({
                "outcome": "dry_run_validated",
                "packId": language_pack.pack_id,
                "packRef": format!("{}@{}", language_pack.pack_id, language_pack.pack_version),
                "subjectId": language_pack.subject.id,
                "verb": "kyc-case.update-status",
                "currentState": semantic.from_state,
                "requestedState": semantic.to_state,
                "transitionRef": output.dry_run.transition_ref,
                "workbookId": output.workbook.id,
                "semanticDiffUri": output.dry_run.semantic_diff_uri,
                "promptContextVariant": "deterministic_language_loop",
                "decodeRepairCount": 0,
                "revisionCount": metrics.revision_count,
                "outcomeLayer": "dry_run_validated",
                "diagnosticCodes": diagnostic_codes,
                "firstPassValid": metrics.first_pass_valid,
                "dryRunValid": metrics.dry_run_valid,
                "humanSummary": human_summary,
                "neededFromUser": []
            })
        }
        WorkbookRevisionOutcome::Refused {
            refusal,
            attempts,
            metrics,
            ..
        } => {
            let diagnostic_codes = diagnostic_codes_from_refusal(attempts, refusal);
            let outcome_layer = deterministic_language_loop_outcome_layer(attempts, metrics);
            let last_draft = attempts.last().map(|attempt| &attempt.draft);
            let current_state = last_draft
                .map(|draft| draft.current_state.as_str())
                .unwrap_or(language_pack.current_state.as_str());
            let requested_state = last_draft.map(|draft| draft.requested_state.as_str());
            let transition_ref = last_draft.map(|draft| draft.transition_ref.as_str());
            let human_summary = language_loop_human_summary_for_refusal(
                current_state,
                outcome_layer,
                &diagnostic_codes,
                refusal.refusal_code.as_str(),
            );
            let needed_from_user = refusal
                .diagnostics
                .first()
                .map(needed_from_diagnostic)
                .unwrap_or_else(|| vec!["corrected_workbook_draft".to_string()]);

            json!({
                "outcome": "structured_refusal",
                "packId": language_pack.pack_id,
                "packRef": format!("{}@{}", language_pack.pack_id, language_pack.pack_version),
                "subjectId": language_pack.subject.id,
                "verb": "kyc-case.update-status",
                "currentState": current_state,
                "requestedState": requested_state,
                "transitionRef": transition_ref,
                "refusalCode": refusal.refusal_code,
                "diagnosticSourcePath": refusal
                    .diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.source_path.as_str()),
                "promptContextVariant": "deterministic_language_loop",
                "decodeRepairCount": 0,
                "revisionCount": metrics.revision_count,
                "outcomeLayer": outcome_layer,
                "diagnosticCodes": diagnostic_codes,
                "firstPassValid": metrics.first_pass_valid,
                "dryRunValid": metrics.dry_run_valid,
                "humanSummary": human_summary,
                "neededFromUser": needed_from_user
            })
        }
    }
}

fn attach_state_discovery(trace_projection: &mut Value, state_discovery: Option<&Value>) {
    let Some(state_discovery) = state_discovery else {
        return;
    };
    if let Some(fields) = trace_projection.as_object_mut() {
        fields.insert("stateDiscovery".to_string(), state_discovery.clone());
    }
}

fn state_discovery_tool_update(
    session_id: Uuid,
    state_discovery: Option<&Value>,
    trace_projection: &Value,
) -> Option<JsonRpcOutgoing> {
    let state_discovery = state_discovery?;
    let source = state_discovery
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(
        source,
        "read_only_discovery_probe"
            | "cached_read_only_discovery_probe"
            | "live_read_only_discovery_probe"
    ) {
        return None;
    }
    let subject_id = state_discovery
        .get("subjectId")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    Some(JsonRpcOutgoing::Notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "session/update".to_string(),
        params: json!({
            "sessionId": session_id.to_string(),
            "update": {
                "sessionUpdate": "tool_call_update",
                "toolCallId": format!("tool:case-state-discovery:{subject_id}"),
                "status": "completed",
                "kind": "read",
                "persona": AcpPersonaMode::SagePlanning.as_str(),
                "workflowPhase": "discovery",
                "title": "KYC case-state discovery",
                "traceProjection": trace_projection,
                "content": {
                    "type": "resource_link",
                    "uri": format!("semos://entity/{subject_id}"),
                    "name": "Read-only KYC case-state anchor",
                    "description": "Resolved current state, configuration version, and state snapshot before workbook drafting"
                }
            }
        }),
    }))
}

fn state_discovery_human_summary(state_discovery: Option<&Value>) -> String {
    let Some(state_discovery) = state_discovery else {
        return String::new();
    };
    let source = state_discovery
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(
        source,
        "read_only_discovery_probe"
            | "cached_read_only_discovery_probe"
            | "live_read_only_discovery_probe"
    ) {
        return String::new();
    }
    let current_state = state_discovery
        .get("currentState")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let configuration_version = state_discovery
        .get("configurationVersion")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let state_snapshot_id = state_discovery
        .get("stateSnapshotId")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!(
        "I read the KYC case state via read-only `kyc-case.read-state`: current state `{current_state}`, config `{configuration_version}`, snapshot `{state_snapshot_id}`. "
    )
}

fn deterministic_language_loop_outcome_layer(
    attempts: &[WorkbookDraftAttempt],
    metrics: &LanguageAcquisitionMetrics,
) -> &'static str {
    if attempts.is_empty() {
        "pre_llm_refusal"
    } else if metrics.revision_count > 0 {
        "revision_refusal"
    } else {
        "validation_refusal"
    }
}

fn diagnostic_codes_from_refusal(
    attempts: &[WorkbookDraftAttempt],
    refusal: &StructuredWorkbookRefusal,
) -> Vec<String> {
    let mut codes = diagnostic_codes_from_attempts(attempts);
    codes.extend(
        refusal
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.error_code.clone()),
    );
    codes.sort();
    codes.dedup();
    codes
}

fn diagnostic_codes_from_attempts(attempts: &[WorkbookDraftAttempt]) -> Vec<String> {
    let mut codes = attempts
        .iter()
        .flat_map(|attempt| attempt.diagnostics.iter())
        .map(|diagnostic| diagnostic.error_code.clone())
        .collect::<Vec<_>>();
    codes.sort();
    codes.dedup();
    codes
}

fn language_loop_human_summary_for_dry_run(
    current_state: Option<&str>,
    requested_state: Option<&str>,
    revision_count: u8,
) -> String {
    if revision_count > 0 {
        let revision_word = if revision_count == 1 {
            "revision"
        } else {
            "revisions"
        };
        format!(
            "I revised the draft after {revision_count} local {revision_word} using structured diagnostics, then validated a dry-run workbook{}; no mutation was executed.",
            transition_phrase(current_state, requested_state)
        )
    } else {
        format!(
            "I found a valid transition{} and drafted a dry-run workbook; no mutation was executed.",
            transition_phrase(current_state, requested_state)
        )
    }
}

fn language_loop_human_summary_for_refusal(
    current_state: &str,
    outcome_layer: &str,
    diagnostic_codes: &[String],
    refusal_code: &str,
) -> String {
    if diagnostic_codes
        .iter()
        .any(|code| code == "missing_evidence_digest")
    {
        "I stopped because required evidence digest is missing; no mutation was executed."
            .to_string()
    } else if outcome_layer == "revision_refusal"
        && diagnostic_codes
            .iter()
            .any(|code| code == "unknown_transition")
    {
        format!(
            "I stopped because no transition is valid from {current_state}; no mutation was executed."
        )
    } else {
        format!("I stopped with structured refusal {refusal_code}; no mutation was executed.")
    }
}

fn transition_phrase(current_state: Option<&str>, requested_state: Option<&str>) -> String {
    match (current_state, requested_state) {
        (Some(current_state), Some(requested_state)) => {
            format!(" from {current_state} to {requested_state}")
        }
        _ => String::new(),
    }
}

fn needed_from_diagnostic(diagnostic: &WorkbookDiagnostic) -> Vec<String> {
    match diagnostic.error_code.as_str() {
        "missing_evidence_digest" => vec!["evidence_digest".to_string()],
        "missing_uuid_binding" => diagnostic
            .missing_uuid_binding
            .clone()
            .map(|binding| vec![binding])
            .unwrap_or_else(|| vec!["case_uuid".to_string()]),
        "invented_verb" => vec!["valid_verb".to_string()],
        "unknown_transition" => vec!["valid_transition".to_string()],
        "current_state_mismatch" => vec!["current_state".to_string()],
        "requested_state_mismatch" => vec!["requested_state".to_string()],
        "stale_replan_required" => vec!["fresh_state_anchor".to_string()],
        _ => vec!["corrected_workbook_draft".to_string()],
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

fn language_loop_performance(
    timings: &acp::AcpKycLanguageLoopTimings,
    prompt_route_us: u64,
    acp_emit_us: u64,
) -> Value {
    let total_us = timings
        .total_us
        .saturating_add(prompt_route_us)
        .saturating_add(acp_emit_us);
    json!({
        "prompt_route_ms": millis_from_micros(prompt_route_us),
        "prompt_route_us": prompt_route_us,
        "language_pack_ms": timings.language_pack_ms,
        "language_pack_us": timings.language_pack_us,
        "revision_loop_ms": timings.revision_loop_ms,
        "revision_loop_us": timings.revision_loop_us,
        "dry_run_ms": timings.dry_run_ms,
        "dry_run_us": timings.dry_run_us,
        "acp_emit_ms": millis_from_micros(acp_emit_us),
        "acp_emit_us": acp_emit_us,
        "total_ms": millis_from_micros(total_us),
        "total_us": total_us
    })
}

fn language_loop_conversation_efficiency(
    metrics: &LanguageAcquisitionMetrics,
    outcome: &str,
    pending_reason: Option<&str>,
) -> Value {
    let pending_user_turn_required = !metrics.dry_run_valid;
    let estimated_user_repair_turns_avoided = if metrics.dry_run_valid {
        u64::from(metrics.revision_count)
    } else {
        0
    };

    json!({
        "outcome": outcome,
        "localRevisionCount": metrics.revision_count,
        "estimatedUserRepairTurnsAvoided": estimated_user_repair_turns_avoided,
        "pendingUserTurnRequired": pending_user_turn_required,
        "pendingReason": pending_reason,
        "firstPassValid": metrics.first_pass_valid,
        "dryRunValid": metrics.dry_run_valid,
        "structuredFailureMode": pending_reason,
        "proseOnlyFailure": false
    })
}

fn pending_question_conversation_efficiency(code: &str) -> Value {
    json!({
        "outcome": "pending_question",
        "localRevisionCount": 0,
        "estimatedUserRepairTurnsAvoided": 0,
        "pendingUserTurnRequired": true,
        "pendingReason": code,
        "firstPassValid": false,
        "dryRunValid": false,
        "structuredFailureMode": code,
        "proseOnlyFailure": false
    })
}

fn pending_question_outgoing(
    id: Option<Value>,
    session_id: Uuid,
    mut result: Value,
    code: &str,
    message: String,
) -> Vec<JsonRpcOutgoing> {
    let trace_projection = pending_question_trace_projection(code, &message, &result);
    if let Value::Object(fields) = &mut result {
        fields.insert("traceProjection".to_string(), trace_projection.clone());
    }

    vec![
        JsonRpcOutgoing::Notification(JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "session/update".to_string(),
            params: json!({
                "sessionId": session_id.to_string(),
                "update": {
                    "sessionUpdate": "plan",
                    "persona": AcpPersonaMode::SagePlanning.as_str(),
                    "workflowPhase": "clarification",
                    "goalProposalTrace": {
                        "status": "pending_question",
                        "pendingQuestionCode": code,
                        "promptContextVariant": trace_projection["promptContextVariant"].clone(),
                        "outcomeLayer": trace_projection["outcomeLayer"].clone(),
                        "diagnosticCodes": trace_projection["diagnosticCodes"].clone(),
                        "humanSummary": trace_projection["humanSummary"].clone(),
                        "decodeRepairCount": 0,
                        "revisionCount": 0,
                        "acpMechanismSummary": ["prompt_router", "structured_pending_question"],
                        "acpFallbackSummary": []
                    },
                    "entries": [
                        {"id": "understand", "status": "completed", "label": "Identify KYC prompt"},
                        {"id": "clarify", "status": "blocked", "label": "Need HITL clarification before workbook draft"},
                        {"id": "dry-run", "status": "blocked", "label": "No dry-run started"}
                    ]
                }
            }),
        }),
        agent_message_update(session_id, message),
        JsonRpcOutgoing::Response(JsonRpcResponse::success(id, result)),
    ]
}

fn pending_question_trace_projection(code: &str, message: &str, result: &Value) -> Value {
    json!({
        "outcome": "pending_question",
        "promptContextVariant": "pre_language_pack_prompt_router",
        "decodeRepairCount": 0,
        "revisionCount": 0,
        "outcomeLayer": "pre_llm_pending",
        "diagnosticCodes": [],
        "pendingQuestionCode": code,
        "humanSummary": message,
        "neededFromUser": pending_question_needs(result, code),
        "firstPassValid": false,
        "dryRunValid": false
    })
}

fn pending_question_needs(result: &Value, code: &str) -> Vec<String> {
    let mut needs = value_string_array(result, &["pending_question", "needs"]);
    needs.extend(value_string_array(result, &["pending_question", "missing"]));
    if needs.is_empty() {
        needs = match code {
            "kyc_prompt_ambiguous" => vec!["explicit_verb_or_update_status_intent".to_string()],
            "kyc_update_status_prompt_incomplete" => vec![
                "case_uuid".to_string(),
                "current_state".to_string(),
                "configuration_version".to_string(),
                "state_snapshot_id".to_string(),
                "requested_state".to_string(),
                "evidence_digest".to_string(),
            ],
            _ => vec!["hitl_clarification".to_string()],
        };
    }
    needs.sort();
    needs.dedup();
    needs
}

fn value_string_array(value: &Value, path: &[&str]) -> Vec<String> {
    let mut current = value;
    for segment in path {
        let Some(next) = current.get(*segment) else {
            return Vec::new();
        };
        current = next;
    }
    current
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn elapsed_us(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn millis_from_micros(micros: u64) -> u64 {
    micros / 1_000
}

fn agent_message_update(session_id: Uuid, text: impl Into<String>) -> JsonRpcOutgoing {
    let text = text.into();
    JsonRpcOutgoing::Notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "session/update".to_string(),
        params: json!({
            "sessionId": session_id.to_string(),
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {
                    "type": "text",
                    "text": text
                }
            }
        }),
    })
}

fn explain_kyc_ambiguous_prompt(candidate_verbs: &[&str]) -> String {
    format!(
        "I can see this is KYC-related, but I cannot safely choose the private DSL verb yet. Candidate verbs are {}. Please choose the verb, or say: Move case <uuid> from INTAKE to DISCOVERY with evidence sha256:... No workbook dry-run or mutation has run.",
        inline_code_list(candidate_verbs.iter().copied())
    )
}

fn explain_kyc_incomplete_prompt(missing: &[&str]) -> String {
    format!(
        "I found a KYC update-status intent, but I am stuck because the prompt is missing {}. I need the missing value(s); this workflow requires a case UUID, a read-only case-state anchor with current state/config/snapshot, requested state, and evidence digest before I can draft and validate the workbook. Example: Move case <uuid> to DISCOVERY with evidence sha256:... plus a kyc-case.read-state resource. No workbook dry-run or mutation has run.",
        inline_code_list(missing.iter().copied())
    )
}

fn explain_kyc_dry_run_success(
    output: &KycUpdateStatusDryRunOutput,
    metrics: &LanguageAcquisitionMetrics,
) -> String {
    let semantic = &output.dry_run.semantic_diff;
    let revision = if metrics.revision_count == 0 {
        "The workbook draft validated on the first pass.".to_string()
    } else {
        format!(
            "The workbook draft needed {} structured revision(s) before validation.",
            metrics.revision_count
        )
    };
    let evidence_count = output.workbook.core.evidence_refs.len();
    let evidence_suffix = if evidence_count == 1 { "" } else { "s" };

    format!(
        "I understood this as `kyc-case.update-status`. I selected transition `{}` for case `{}`: `{}` -> `{}` using pack `{}` and snapshot `{}`. {} Evidence binding is present ({} reference{}). This is dry-run only; no mutation was executed and the normal runbook gate remains the only mutation path.",
        output.dry_run.transition_ref,
        output.workbook.core.subject.subject_id,
        semantic.from_state,
        semantic.to_state,
        output.workbook.core.pack_id,
        output.workbook.core.state_snapshot_id,
        revision,
        evidence_count,
        evidence_suffix
    )
}

fn explain_kyc_refusal(refusal: &StructuredWorkbookRefusal) -> String {
    let mut parts = vec![format!(
        "I could not validate the KYC update-status workbook: `{}`.",
        refusal.refusal_code
    )];

    if let Some(diagnostic) = refusal.diagnostics.first() {
        parts.extend(explain_workbook_diagnostic(diagnostic));
    }

    parts.push(
        "Correct the blocked field or provide the missing input, then I can retry the dry-run. No mutation was executed.".to_string(),
    );
    parts.join(" ")
}

fn explain_workbook_diagnostic(diagnostic: &WorkbookDiagnostic) -> Vec<String> {
    let mut parts = vec![format!(
        "The validator stopped at `{}` with `{}`.",
        diagnostic.source_path, diagnostic.error_code
    )];

    if let Some(verb) = diagnostic.attempted_verb.as_deref() {
        parts.push(format!("Attempted verb: `{verb}`."));
    }
    if let Some(transition) = diagnostic.attempted_transition.as_deref() {
        parts.push(format!("Attempted transition: `{transition}`."));
    }
    if let Some(binding) = diagnostic.missing_uuid_binding.as_deref() {
        parts.push(format!("Missing UUID binding: `{binding}`."));
    }
    if diagnostic.expected_state.is_some() || diagnostic.actual_state.is_some() {
        parts.push(format!(
            "Expected `{}`, got `{}`.",
            diagnostic.expected_state.as_deref().unwrap_or("unknown"),
            diagnostic.actual_state.as_deref().unwrap_or("unknown")
        ));
    }
    if let Some(reason) = diagnostic.blocked_transition_reason.as_deref() {
        parts.push(format!("Reason: {reason}."));
    }
    if !diagnostic.suggested_verbs.is_empty() {
        parts.push(format!(
            "Valid verb(s): {}.",
            inline_code_list(
                diagnostic
                    .suggested_verbs
                    .iter()
                    .take(3)
                    .map(String::as_str)
            )
        ));
    }
    if !diagnostic.suggested_transitions.is_empty() {
        parts.push(format!(
            "Valid transition candidate(s): {}.",
            inline_code_list(
                diagnostic
                    .suggested_transitions
                    .iter()
                    .take(3)
                    .map(String::as_str)
            )
        ));
    }
    parts.push(format!(
        "Pack anchor: `{}` / config `{}` / snapshot `{}`.",
        diagnostic.pack_ref, diagnostic.configuration_version, diagnostic.state_snapshot_id
    ));

    parts
}

fn inline_code_list<I, S>(values: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let quoted = values
        .into_iter()
        .map(|value| format!("`{}`", value.as_ref()))
        .collect::<Vec<_>>();
    if quoted.is_empty() {
        "`unknown`".to_string()
    } else {
        quoted.join(", ")
    }
}

fn load_ob_poc_kyc_domain_pack(
) -> Result<sem_os_core::domain_pack::DomainPackManifest, acp::AcpAdapterError> {
    crate::acp_facade::load_ob_poc_kyc_domain_pack()
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

    fn agent_message_text(outgoing: &[JsonRpcOutgoing]) -> String {
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
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
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
    fn session_prompt_routes_cbu_to_dag_semantic_surface() {
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
                "prompt": [{"type": "text", "text": "assign role to cbu"}]
            }),
        ));

        assert_eq!(outgoing.len(), 4);
        match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => {
                assert_eq!(
                    notification.params["update"]["title"],
                    "ACP DAG semantic surface"
                );
                assert_eq!(
                    notification.params["update"]["content"]["uri"],
                    "semos://journey-pack/cbu-maintenance"
                );
            }
            JsonRpcOutgoing::Response(_) => panic!("expected tool notification"),
        }
        match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => {
                let trace = &notification.params["update"]["goalProposalTrace"];
                assert_eq!(trace["status"], "pending_question");
                assert_eq!(trace["selectedVerb"], "cbu.assign-role");
                assert_eq!(trace["structuredFailureMode"], "missing_required_args");
                assert_eq!(trace["proseOnlyFailure"], false);
                assert!(trace["acpMechanismSummary"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|mechanism| mechanism == "dag_semantic_router"));
                assert_eq!(trace["registryTrace"]["verified"], true);
                assert_eq!(trace["envelopeTrace"]["verified"], true);
                assert_eq!(trace["envelopeTrace"]["pack_id"], "cbu-maintenance");
                assert_eq!(trace["runtimeTrace"]["verified"], true);
                assert_eq!(trace["runtimeTrace"]["pack_id"], "cbu-maintenance");
                assert!(
                    trace["runtimeTrace"]["runtime_hash"]
                        .as_str()
                        .unwrap()
                        .len()
                        >= 64
                );
                assert!(trace["envelopeTrace"]["envelope_hash"]
                    .as_str()
                    .unwrap()
                    .starts_with("sha256:"));
                assert!(trace["acpMechanismSummary"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|mechanism| mechanism == "verified_pack_context_envelope_v2"));
            }
            JsonRpcOutgoing::Response(_) => panic!("expected plan notification"),
        }

        let response = match &outgoing[3] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "pending_question");
        assert_eq!(result["dag_semantic"]["selected_verb"], "cbu.assign-role");
        assert_eq!(result["traceProjection"]["registryTrace"]["verified"], true);
        assert_eq!(
            result["traceProjection"]["envelopeTrace"]["pack_id"],
            "cbu-maintenance"
        );
        assert_eq!(result["traceProjection"]["runtimeTrace"]["verified"], true);
        assert_eq!(
            result["traceProjection"]["runtimeTrace"]["pack_id"],
            "cbu-maintenance"
        );
        assert_eq!(result["dag_semantic"]["envelope_trace"]["verified"], true);
        assert_eq!(result["dag_semantic"]["runtime_trace"]["verified"], true);
        assert_eq!(
            result["observability"]["conversationEfficiency"]["proseOnlyFailure"],
            false
        );
        assert!(agent_message_text(&outgoing).contains("No mutation has run"));
    }

    #[test]
    fn session_prompt_routes_non_cbu_dag_to_same_semantic_surface() {
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
                "prompt": [{"type": "text", "text": "create a new deal"}]
            }),
        ));

        assert_eq!(outgoing.len(), 4);
        match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => {
                let trace = &notification.params["update"]["goalProposalTrace"];
                assert_eq!(trace["selectedVerb"], "deal.create");
                assert_eq!(trace["proseOnlyFailure"], false);
                assert!(trace["missingRequiredArgs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|arg| arg == "deal-name"));
            }
            JsonRpcOutgoing::Response(_) => panic!("expected plan notification"),
        }
        let response = match &outgoing[3] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        assert_eq!(
            response.result.as_ref().unwrap()["dag_semantic"]["selected_verb"],
            "deal.create"
        );
    }

    #[test]
    fn session_prompt_routes_instrument_matrix_to_journey_pack_projection() {
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
                "prompt": [{"type": "text", "text": "show trading matrix"}]
            }),
        ));

        assert_eq!(outgoing.len(), 4);
        match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => {
                assert_eq!(
                    notification.params["update"]["content"]["uri"],
                    "semos://journey-pack/instrument-matrix"
                );
                assert_eq!(
                    notification.params["update"]["content"]["name"],
                    "Instrument Matrix"
                );
            }
            JsonRpcOutgoing::Response(_) => panic!("expected tool notification"),
        }
        match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => {
                let trace = &notification.params["update"]["goalProposalTrace"];
                assert_eq!(trace["pack"]["pack_id"], "instrument-matrix");
                assert_eq!(trace["pack"]["pack_name"], "Instrument Matrix");
                assert!(trace["pack"]["allowed_verb_count"].as_u64().unwrap() > 100);
                assert!(trace["pack"]["allowed_verbs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|verb| verb == "trading-profile.read"));
                assert!(trace["pack"]["optional_questions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|question| question["field"] == "profile_action"));
                assert!(trace["acpMechanismSummary"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|mechanism| mechanism == "journey_pack_context"));
                assert_eq!(trace["registryTrace"]["verified"], true);
                assert!(trace["envelopeTrace"].is_null());
            }
            JsonRpcOutgoing::Response(_) => panic!("expected plan notification"),
        }

        let response = match &outgoing[3] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(
            result["dag_semantic"]["pack"]["pack_id"],
            "instrument-matrix"
        );
        assert!(agent_message_text(&outgoing).contains("Instrument Matrix"));
        assert_eq!(
            result["observability"]["conversationEfficiency"]["proseOnlyFailure"],
            false
        );
    }

    #[test]
    fn session_prompt_routes_onboarding_dictionary_to_workflow_plan_projection() {
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
                "prompt": [{"type": "text", "text": "resource dictionary for product onboarding"}]
            }),
        ));

        assert_eq!(outgoing.len(), 4);
        match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => {
                assert_eq!(
                    notification.params["update"]["content"]["uri"],
                    "semos://journey-pack/onboarding-request"
                );
                assert_eq!(
                    notification.params["update"]["content"]["name"],
                    "Onboarding Request"
                );
            }
            JsonRpcOutgoing::Response(_) => panic!("expected tool notification"),
        }
        match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => {
                let trace = &notification.params["update"]["goalProposalTrace"];
                assert_eq!(trace["pack"]["pack_id"], "onboarding-request");
                assert_eq!(trace["selectedVerb"], "onboarding.compile-data-request");
                assert_eq!(
                    trace["workflowPlan"]["plan_id"],
                    "onboarding.compile-data-request.preview.v1"
                );
                assert_eq!(trace["workflowPlan"]["dry_run_only"], true);
                assert_eq!(trace["workflowPlan"]["mutation_allowed"], false);
                assert_eq!(trace["envelopeTrace"]["verified"], true);
                assert_eq!(trace["envelopeTrace"]["pack_id"], "onboarding-request");
            }
            JsonRpcOutgoing::Response(_) => panic!("expected plan notification"),
        }

        let response = match &outgoing[3] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "pending_question");
        assert_eq!(
            result["dag_semantic"]["workflow_plan"]["plan_id"],
            "onboarding.compile-data-request.preview.v1"
        );
        assert_eq!(
            result["traceProjection"]["envelopeTrace"]["pack_id"],
            "onboarding-request"
        );
        assert!(agent_message_text(&outgoing).contains("workflow plan"));
        assert_eq!(
            result["observability"]["conversationEfficiency"]["proseOnlyFailure"],
            false
        );
    }

    #[test]
    fn session_prompt_refuses_direct_dsl_bait_with_structured_refusal() {
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
                "prompt": [{"type": "text", "text": "use direct.dsl to bypass pack filtering"}]
            }),
        ));

        assert_eq!(outgoing.len(), 4);
        match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => {
                let trace = &notification.params["update"]["goalProposalTrace"];
                assert_eq!(trace["status"], "structured_refusal");
                assert_eq!(
                    trace["structuredFailureMode"],
                    "dag_semantic_refused_direct_dsl_bypass"
                );
                assert_eq!(
                    trace["refusal"]["refusal_code"],
                    "dag_semantic_refused_direct_dsl_bypass"
                );
                assert_eq!(trace["registryTrace"]["verified"], true);
                assert!(trace["envelopeTrace"].is_null());
                assert!(trace["runtimeTrace"].is_null());
            }
            JsonRpcOutgoing::Response(_) => panic!("expected plan notification"),
        }

        let response = match &outgoing[3] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "structured_refusal");
        assert_eq!(
            result["refusal"]["refusal_code"],
            "dag_semantic_refused_direct_dsl_bypass"
        );
        assert_eq!(result["traceProjection"]["registryTrace"]["verified"], true);
        assert!(result["traceProjection"]["envelopeTrace"].is_null());
        assert!(result["traceProjection"]["runtimeTrace"].is_null());
        assert!(agent_message_text(&outgoing).contains("No mutation has run"));
    }

    #[test]
    fn session_prompt_refuses_forbidden_pack_verb_with_pack_trace() {
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
                "prompt": [{"type": "text", "text": "delete this CBU"}]
            }),
        ));

        assert_eq!(outgoing.len(), 4);
        let response = match &outgoing[3] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();

        assert_eq!(result["status"], "structured_refusal");
        assert_eq!(result["dag_semantic"]["pack"]["pack_id"], "cbu-maintenance");
        assert_eq!(result["dag_semantic"]["selected_verb"], "cbu.delete");
        assert_eq!(
            result["traceProjection"]["envelopeTrace"]["pack_id"],
            "cbu-maintenance"
        );
        assert_eq!(
            result["refusal"]["refusal_code"],
            "dag_semantic_refused_forbidden_pack_verb"
        );
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
        assert!(result["observability"]["performance"]["prompt_route_ms"]
            .as_u64()
            .is_some());
        assert!(result["observability"]["performance"]["language_pack_ms"]
            .as_u64()
            .is_some());
        assert!(result["observability"]["performance"]["revision_loop_ms"]
            .as_u64()
            .is_some());
        assert!(result["observability"]["performance"]["dry_run_ms"]
            .as_u64()
            .is_some());
        assert_eq!(
            result["observability"]["conversationEfficiency"]["pendingUserTurnRequired"],
            false
        );
        assert_eq!(
            result["observability"]["conversationEfficiency"]["proseOnlyFailure"],
            false
        );
        assert_eq!(
            result["traceProjection"]["promptContextVariant"],
            "deterministic_language_loop"
        );
        assert_eq!(
            result["traceProjection"]["outcomeLayer"],
            "dry_run_validated"
        );
        assert_eq!(result["traceProjection"]["decodeRepairCount"], 0);
        assert!(result["traceProjection"]["humanSummary"]
            .as_str()
            .unwrap()
            .contains("I found a valid transition from DISCOVERY to ASSESSMENT"));
        let message = agent_message_text(&outgoing);
        assert!(message.contains("I found a valid transition from DISCOVERY to ASSESSMENT"));
        assert!(message.contains("kyc-case.update-status"));
        assert!(message.contains("kyc-case.discovery-to-assessment"));
        assert!(message.contains("DISCOVERY"));
        assert!(message.contains("ASSESSMENT"));
        assert!(message.to_ascii_lowercase().contains("no mutation"));
    }

    #[test]
    fn session_prompt_uses_read_only_discovery_resource_for_state_anchor() {
        let mut agent = AcpJsonRpcAgent::new();
        let discovery_response = json!({
            "probe_id": "kyc-case.read-state",
            "subject": {
                "subject_kind": "kyc_case",
                "subject_id": CASE_ID
            },
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
                },
                {
                    "key": "case.state_snapshot_id",
                    "value": "snapshot-live-1",
                    "classification": "internal"
                }
            ],
            "provenance": [
                {
                    "source": "sem_os.session_state",
                    "snapshot_ref": "snapshot-live-1"
                }
            ],
            "first_class_state_mutated": false
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
                        "name": "KYC read-state probe",
                        "mime_type": "application/json",
                        "text": discovery_response.to_string()
                    }
                ]
            }),
        ));

        assert_eq!(outgoing.len(), 7);
        let discovery_update = match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected discovery notification"),
        };
        assert_eq!(
            discovery_update["toolCallId"],
            format!("tool:case-state-discovery:{CASE_ID}")
        );
        assert_eq!(
            discovery_update["traceProjection"]["stateDiscovery"]["source"],
            "read_only_discovery_probe"
        );
        assert_eq!(
            discovery_update["traceProjection"]["stateDiscovery"]["currentState"],
            "DISCOVERY"
        );

        let response = match &outgoing[6] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "dry_run_validated");
        assert_eq!(
            result["output"]["dry_run"]["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
        assert_eq!(
            result["language_pack"]["configuration_version"],
            "config-live-1"
        );
        assert_eq!(
            result["traceProjection"]["stateDiscovery"]["stateSnapshotId"],
            "snapshot-live-1"
        );
        let message = agent_message_text(&outgoing);
        assert!(message.contains("read-only `kyc-case.read-state`"));
        assert!(message.contains("config-live-1"));
        assert!(message.contains("snapshot-live-1"));
    }

    #[test]
    fn session_prompt_uses_cached_read_only_discovery_state() {
        let mut agent = AcpJsonRpcAgent::new();
        let discovery = agent.handle_request(request(
            1,
            "obpoc/kyc_case_state/discover",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "subject_id": CASE_ID,
                "observations": [
                    {"key": "case.status", "value": "DISCOVERY", "classification": "internal"},
                    {"key": "case.configuration_version", "value": "config-live-1", "classification": "internal"},
                    {"key": "case.state_snapshot_id", "value": "snapshot-live-1", "classification": "internal"}
                ],
                "provenance": [
                    {"source": "sem_os.session_state", "snapshot_ref": "snapshot-live-1"}
                ],
                "first_class_state_mutated": false
            }),
        ));
        let discovered = only_response(discovery).result.unwrap();
        assert_eq!(discovered["status"], "kyc_case_state_discovered");

        let outgoing = agent.handle_request(request(
            2,
            "session/prompt",
            json!({
                "sessionId": SESSION_ID.to_string(),
                "prompt": [
                    {
                        "type": "text",
                        "text": format!("Advance KYC case {CASE_ID} to ASSESSMENT with evidence sha256:evidence")
                    }
                ]
            }),
        ));

        assert_eq!(outgoing.len(), 7);
        let discovery_update = match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected discovery notification"),
        };
        assert_eq!(
            discovery_update["traceProjection"]["stateDiscovery"]["source"],
            "cached_read_only_discovery_probe"
        );
        let response = match &outgoing[6] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "dry_run_validated");
        assert_eq!(
            result["traceProjection"]["stateDiscovery"]["configurationVersion"],
            "config-live-1"
        );
    }

    #[test]
    fn session_prompt_marks_postgres_cached_state_as_live_read_only_discovery() {
        let mut agent = AcpJsonRpcAgent::new();
        let snapshot_ref = format!("postgres:ob-poc.cases:{CASE_ID}:status:discovery");
        let discovery = agent.handle_request(request(
            1,
            "obpoc/kyc_case_state/discover",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "subject_id": CASE_ID,
                "observations": [
                    {"key": "case.status", "value": "DISCOVERY", "classification": "internal"},
                    {"key": "case.configuration_version", "value": "domain_pack:ob-poc.kyc@0.1.0", "classification": "internal"},
                    {"key": "case.state_snapshot_id", "value": snapshot_ref, "classification": "internal"}
                ],
                "provenance": [
                    {"source": "postgres.ob-poc.cases", "snapshot_ref": snapshot_ref}
                ],
                "first_class_state_mutated": false
            }),
        ));
        assert_eq!(
            only_response(discovery).result.unwrap()["status"],
            "kyc_case_state_discovered"
        );

        let outgoing = agent.handle_request(request(
            2,
            "session/prompt",
            json!({
                "sessionId": SESSION_ID.to_string(),
                "prompt": [
                    {
                        "type": "text",
                        "text": format!("Advance KYC case {CASE_ID} to ASSESSMENT with evidence sha256:evidence")
                    }
                ]
            }),
        ));

        let discovery_update = match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected discovery notification"),
        };
        assert_eq!(
            discovery_update["traceProjection"]["stateDiscovery"]["source"],
            "live_read_only_discovery_probe"
        );
        assert!(agent_message_text(&outgoing).contains("read-only `kyc-case.read-state`"));
    }

    #[test]
    fn session_prompt_with_case_uuid_but_no_state_discovery_is_pending() {
        let mut agent = AcpJsonRpcAgent::new();
        let outgoing = agent.handle_request(request(
            1,
            "session/prompt",
            json!({
                "sessionId": SESSION_ID.to_string(),
                "prompt": [
                    {
                        "type": "text",
                        "text": format!("Advance KYC case {CASE_ID} to ASSESSMENT with evidence sha256:evidence")
                    }
                ]
            }),
        ));

        assert_eq!(outgoing.len(), 3);
        let response = match &outgoing[2] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "pending_question");
        assert_eq!(
            result["pending_question"]["code"],
            "kyc_update_status_prompt_incomplete"
        );
        let missing = result["pending_question"]["missing"].as_array().unwrap();
        assert!(missing.iter().any(|item| item == "current_state"));
        assert!(missing.iter().any(|item| item == "configuration_version"));
        assert!(missing.iter().any(|item| item == "state_snapshot_id"));
        assert_eq!(result["traceProjection"]["outcomeLayer"], "pre_llm_pending");
        assert!(result["traceProjection"]["neededFromUser"]
            .as_array()
            .unwrap()
            .iter()
            .any(|need| need == "state_snapshot_id"));
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
        assert!(result["observability"]["performance"]["prompt_route_ms"]
            .as_u64()
            .is_some());
        assert!(result["observability"]["performance"]["dry_run_ms"]
            .as_u64()
            .is_some());
        assert_eq!(
            result["observability"]["conversationEfficiency"]["pendingUserTurnRequired"],
            true
        );
        assert_eq!(
            result["observability"]["conversationEfficiency"]["pendingReason"],
            "missing_evidence_digest"
        );
        assert_eq!(
            result["observability"]["conversationEfficiency"]["proseOnlyFailure"],
            false
        );
        assert_eq!(
            result["traceProjection"]["promptContextVariant"],
            "deterministic_language_loop"
        );
        assert_eq!(
            result["traceProjection"]["outcomeLayer"],
            "validation_refusal"
        );
        assert!(result["traceProjection"]["diagnosticCodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "missing_evidence_digest"));
        assert!(result["traceProjection"]["humanSummary"]
            .as_str()
            .unwrap()
            .contains("required evidence digest is missing"));
        let message = agent_message_text(&outgoing);
        assert!(message.contains("required evidence digest is missing"));
        assert!(message.contains("missing_evidence_digest"));
        assert!(message.contains("draft.evidence_digest"));
        assert!(message.contains("Correct") || message.contains("provide"));
        assert!(message.to_ascii_lowercase().contains("no mutation"));
    }

    #[test]
    fn session_prompt_kyc_incomplete_returns_pending_trace_projection() {
        let mut agent = AcpJsonRpcAgent::new();
        let outgoing = agent.handle_request(request(
            1,
            "session/prompt",
            json!({
                "sessionId": SESSION_ID.to_string(),
                "prompt": [
                    {
                        "type": "text",
                        "text": "update status for KYC case"
                    }
                ]
            }),
        ));

        assert_eq!(outgoing.len(), 3);
        let plan_update = match &outgoing[0] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected plan notification"),
        };
        assert_eq!(
            plan_update["goalProposalTrace"]["status"],
            "pending_question"
        );
        assert_eq!(
            plan_update["goalProposalTrace"]["outcomeLayer"],
            "pre_llm_pending"
        );
        assert_eq!(
            plan_update["goalProposalTrace"]["promptContextVariant"],
            "pre_language_pack_prompt_router"
        );
        assert!(plan_update["goalProposalTrace"]["humanSummary"]
            .as_str()
            .unwrap()
            .contains("missing"));

        let response = match &outgoing[2] {
            JsonRpcOutgoing::Response(response) => response,
            JsonRpcOutgoing::Notification(_) => panic!("expected response"),
        };
        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "pending_question");
        assert_eq!(result["traceProjection"]["outcomeLayer"], "pre_llm_pending");
        assert!(result["traceProjection"]["neededFromUser"]
            .as_array()
            .unwrap()
            .iter()
            .any(|need| need == "case_uuid"));
        let message = agent_message_text(&outgoing);
        assert!(message.contains("I found a KYC update-status intent"));
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
                "objective": "Move the KYC case to assessment",
                "verb": "kyc-case.update-status",
                "subject_uuid_field": "case_id",
                "state_field": "case.status"
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
        assert_eq!(
            projection["payload"]["transition_effects"][0]["field"],
            "case.status"
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
    fn extension_language_pack_get_accepts_explicit_task_shape() {
        let mut agent = AcpJsonRpcAgent::new();
        let response = only_response(agent.handle_request(request(
            1,
            "obpoc/language_pack/get",
            json!({
                "session_id": SESSION_ID.to_string(),
                "adapter": "zed",
                "subject_id": CASE_ID,
                "subject_kind": "kyc_case",
                "verb": "kyc-case.update-status",
                "current_state": "DISCOVERY",
                "configuration_version": "config-2",
                "state_snapshot_id": "snapshot-2",
                "subject_uuid_field": "case_id",
                "state_field": "case.status"
            }),
        )));

        let result = response.result.as_ref().unwrap();
        assert_eq!(result["status"], "sem_os_language_pack");
        assert_eq!(
            result["language_pack"]["valid_verbs"][0]["verb"],
            "kyc-case.update-status"
        );
        assert_eq!(result["language_pack"]["subject"]["kind"], "kyc_case");
        assert_eq!(
            result["language_pack"]["uuid_bindings"][0]["field"],
            "case_id"
        );
        assert_eq!(
            result["language_pack"]["transition_effects"][0]["field"],
            "case.status"
        );
        assert_eq!(
            result["language_pack"]["candidate_transitions"][0]["transition_ref"],
            "kyc-case.discovery-to-assessment"
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
        assert_eq!(
            plan_update["goalProposalTrace"]["promptContextVariant"],
            "deterministic_language_loop"
        );
        assert_eq!(
            plan_update["goalProposalTrace"]["outcomeLayer"],
            "dry_run_validated"
        );
        assert_eq!(plan_update["goalProposalTrace"]["decodeRepairCount"], 0);
        assert!(plan_update["goalProposalTrace"]["diagnosticCodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "unknown_transition"));
        assert!(plan_update["goalProposalTrace"]["humanSummary"]
            .as_str()
            .unwrap()
            .contains("I revised the draft after 1 local revision"));
        let validation_update = match &outgoing[2] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected validation notification"),
        };
        assert_eq!(
            validation_update["traceProjection"]["promptContextVariant"],
            "deterministic_language_loop"
        );
        assert_eq!(validation_update["traceProjection"]["revisionCount"], 1);
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
        assert_eq!(
            result["traceProjection"]["promptContextVariant"],
            "deterministic_language_loop"
        );
        assert_eq!(
            result["traceProjection"]["outcomeLayer"],
            "dry_run_validated"
        );
        let message = agent_message_text(&outgoing);
        assert!(message.contains("I revised the draft after 1 local revision"));
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
        assert_eq!(
            result["traceProjection"]["promptContextVariant"],
            "deterministic_language_loop"
        );
        assert_eq!(
            result["traceProjection"]["outcomeLayer"],
            "validation_refusal"
        );
        assert!(result["traceProjection"]["diagnosticCodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "invented_verb"));
        assert!(result["traceProjection"]["humanSummary"]
            .as_str()
            .unwrap()
            .contains("structured refusal invented_verb"));
        let plan_update = match &outgoing[1] {
            JsonRpcOutgoing::Notification(notification) => &notification.params["update"],
            JsonRpcOutgoing::Response(_) => panic!("expected plan notification"),
        };
        assert_eq!(
            plan_update["goalProposalTrace"]["outcomeLayer"],
            "validation_refusal"
        );
        let message = agent_message_text(&outgoing);
        assert!(message.contains("I stopped with structured refusal invented_verb"));
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
