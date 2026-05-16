//! LLM draft adapter for KYC update-status workbook drafts.
//!
//! The adapter is deliberately not an authority path. It asks an LLM for one
//! typed draft, records hash-only inference metadata, then hands the draft to
//! the deterministic revision/dry-run harness.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use ob_agentic::llm_client::{LlmClient, ToolDefinition};
use sem_os_policy::domain_pack::DomainPackManifest;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::llm_trace::{record_llm_inference_trace, workbook_llm_trace_ref, LlmInferenceTrace};

use super::{
    run_kyc_update_status_revision_loop, KycUpdateStatusWorkbookDraft, SemOsLanguagePack,
    WorkbookDiagnostic, WorkbookRevisionOutcome,
};

pub const KYC_UPDATE_STATUS_LLM_DRAFT_PROMPT_TEMPLATE_VERSION: &str =
    "kyc_update_status_workbook_draft_v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmDraftAdapterRefusal {
    pub refusal_code: String,
    pub message: String,
    #[serde(default)]
    pub diagnostics: Vec<WorkbookDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_trace: Option<LlmInferenceTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum LlmDraftLoopOutcome {
    HarnessCompleted {
        llm_trace: LlmInferenceTrace,
        draft: KycUpdateStatusWorkbookDraft,
        #[serde(default)]
        adapter_diagnostics: Vec<WorkbookDiagnostic>,
        outcome: WorkbookRevisionOutcome,
    },
    AdapterRefused {
        refusal: LlmDraftAdapterRefusal,
    },
}

pub async fn run_kyc_update_status_llm_draft_loop(
    manifest: &DomainPackManifest,
    pack: &SemOsLanguagePack,
    session_id: Uuid,
    actor_id: impl Into<String>,
    actor_roles: Vec<String>,
    evidence_digest: Option<String>,
    client: Arc<dyn LlmClient>,
) -> LlmDraftLoopOutcome {
    run_kyc_update_status_llm_draft_loop_with_prompt_pack(
        manifest,
        pack,
        pack,
        session_id,
        actor_id,
        actor_roles,
        evidence_digest,
        client,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn run_kyc_update_status_llm_draft_loop_with_prompt_pack(
    manifest: &DomainPackManifest,
    prompt_pack: &SemOsLanguagePack,
    validation_pack: &SemOsLanguagePack,
    session_id: Uuid,
    actor_id: impl Into<String>,
    actor_roles: Vec<String>,
    evidence_digest: Option<String>,
    client: Arc<dyn LlmClient>,
) -> LlmDraftLoopOutcome {
    let actor_id = actor_id.into();
    let system_prompt = system_prompt();
    let user_prompt = match user_prompt(
        prompt_pack,
        session_id,
        &actor_id,
        &actor_roles,
        evidence_digest.as_deref(),
    ) {
        Ok(prompt) => prompt,
        Err(err) => {
            return LlmDraftLoopOutcome::AdapterRefused {
                refusal: adapter_refusal(
                    validation_pack,
                    "language_pack_serialization_failed",
                    "llm_draft_adapter.language_pack",
                    err.to_string(),
                    None,
                ),
            };
        }
    };

    let llm_started_at = Instant::now();
    let result = match client
        .chat_with_tool(system_prompt, &user_prompt, &workbook_draft_tool())
        .await
    {
        Ok(result) => result,
        Err(err) => {
            return LlmDraftLoopOutcome::AdapterRefused {
                refusal: adapter_refusal(
                    validation_pack,
                    "llm_draft_failed",
                    "llm_draft_adapter.client",
                    err.to_string(),
                    None,
                ),
            };
        }
    };
    let llm_latency_ms = elapsed_ms(llm_started_at);

    if result.tool_name != "draft_kyc_update_status_workbook" {
        return LlmDraftLoopOutcome::AdapterRefused {
            refusal: adapter_refusal(
                validation_pack,
                "unexpected_tool_call",
                "llm_draft_adapter.tool_name",
                result.tool_name,
                None,
            ),
        };
    }

    let response_json = match serde_json::to_string(&result.arguments) {
        Ok(json) => json,
        Err(err) => {
            return LlmDraftLoopOutcome::AdapterRefused {
                refusal: adapter_refusal(
                    validation_pack,
                    "llm_response_serialization_failed",
                    "llm_draft_adapter.response",
                    err.to_string(),
                    None,
                ),
            };
        }
    };
    let mut draft_arguments = result.arguments;
    let mut adapter_diagnostics = normalize_known_draft_bindings(
        &mut draft_arguments,
        validation_pack,
        session_id,
        &actor_id,
        &actor_roles,
        evidence_digest.as_deref(),
    );
    adapter_diagnostics.extend(repair_decode_fields(&mut draft_arguments, validation_pack));
    let trace = record_llm_inference_trace(crate::llm_trace::LlmInferenceTraceInput {
        provider: client.provider_name(),
        model: client.model_name(),
        model_id: Some(client.model_name()),
        prompt_template_version: KYC_UPDATE_STATUS_LLM_DRAFT_PROMPT_TEMPLATE_VERSION,
        prompt: &user_prompt,
        response: &response_json,
        context_hash: None,
        input_tokens: None,
        output_tokens: None,
        latency_ms: Some(llm_latency_ms),
    });

    let mut draft: KycUpdateStatusWorkbookDraft =
        match decode_workbook_draft(draft_arguments, validation_pack) {
            Ok(draft) => draft,
            Err(diagnostics) => {
                let refusal_code = diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.error_code.clone())
                    .unwrap_or_else(|| "llm_draft_decode_failed".to_string());
                let message = diagnostics
                    .first()
                    .and_then(|diagnostic| diagnostic.blocked_transition_reason.clone())
                    .unwrap_or_else(|| "LLM draft did not satisfy workbook schema".to_string());
                return LlmDraftLoopOutcome::AdapterRefused {
                    refusal: LlmDraftAdapterRefusal {
                        refusal_code,
                        message,
                        diagnostics,
                        llm_trace: Some(trace),
                    },
                };
            }
        };
    draft.session_id = session_id;
    draft.actor_id = actor_id;
    draft.actor_roles = actor_roles;
    draft.llm_trace_ref = Some(workbook_llm_trace_ref(&trace));

    let outcome = run_kyc_update_status_revision_loop(manifest, validation_pack, draft.clone());
    LlmDraftLoopOutcome::HarnessCompleted {
        llm_trace: trace,
        draft,
        adapter_diagnostics,
        outcome,
    }
}

fn adapter_refusal(
    pack: &SemOsLanguagePack,
    refusal_code: impl Into<String>,
    source_path: impl Into<String>,
    message: impl Into<String>,
    llm_trace: Option<LlmInferenceTrace>,
) -> LlmDraftAdapterRefusal {
    let refusal_code = refusal_code.into();
    let message = message.into();
    LlmDraftAdapterRefusal {
        refusal_code: refusal_code.clone(),
        message: message.clone(),
        diagnostics: vec![WorkbookDiagnostic::llm_adapter_failure(
            pack,
            refusal_code,
            source_path,
            message,
        )],
        llm_trace,
    }
}

fn decode_workbook_draft(
    arguments: serde_json::Value,
    pack: &SemOsLanguagePack,
) -> Result<KycUpdateStatusWorkbookDraft, Vec<WorkbookDiagnostic>> {
    let Some(object) = arguments.as_object() else {
        return Err(vec![WorkbookDiagnostic::invalid_llm_draft_shape(
            pack,
            json_type_name(&arguments),
        )]);
    };

    let attempted_verb = object
        .get("verb")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let attempted_transition = object
        .get("transition_ref")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let missing_diagnostics: Vec<WorkbookDiagnostic> = [
        "session_id",
        "actor_id",
        "actor_roles",
        "verb",
        "transition_ref",
        "subject_kind",
        "case_id",
        "current_state",
        "requested_state",
        "configuration_version",
        "state_snapshot_id",
    ]
    .into_iter()
    .filter(|field| required_field_missing(object.get(*field)))
    .map(|field| {
        WorkbookDiagnostic::missing_required_workbook_field(
            pack,
            field,
            attempted_verb.clone(),
            attempted_transition.clone(),
        )
    })
    .collect();

    if !missing_diagnostics.is_empty() {
        return Err(missing_diagnostics);
    }

    serde_json::from_value(arguments).map_err(|err| {
        vec![WorkbookDiagnostic::llm_draft_decode_failed(
            pack,
            err.to_string(),
            attempted_verb,
            attempted_transition,
        )]
    })
}

fn required_field_missing(value: Option<&serde_json::Value>) -> bool {
    match value {
        None | Some(serde_json::Value::Null) => true,
        Some(serde_json::Value::String(value)) => value.trim().is_empty(),
        Some(serde_json::Value::Array(values)) => values.is_empty(),
        Some(_) => false,
    }
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn normalize_known_draft_bindings(
    arguments: &mut serde_json::Value,
    pack: &SemOsLanguagePack,
    session_id: Uuid,
    actor_id: &str,
    actor_roles: &[String],
    evidence_digest: Option<&str>,
) -> Vec<WorkbookDiagnostic> {
    let Some(object) = arguments.as_object_mut() else {
        return vec![];
    };
    let current_state_was_missing = required_field_missing(object.get("current_state"));
    insert_if_missing(object, "session_id", serde_json::json!(session_id));
    insert_if_missing(object, "actor_id", serde_json::json!(actor_id));
    insert_if_missing(object, "actor_roles", serde_json::json!(actor_roles));
    insert_if_missing(object, "verb", serde_json::json!("kyc-case.update-status"));
    insert_if_missing(object, "subject_kind", serde_json::json!(pack.subject.kind));
    insert_if_missing(object, "case_id", serde_json::json!(pack.subject.id));
    insert_if_missing(
        object,
        "current_state",
        serde_json::json!(pack.current_state),
    );
    insert_if_missing(
        object,
        "configuration_version",
        serde_json::json!(pack.configuration_version),
    );
    insert_if_missing(
        object,
        "state_snapshot_id",
        serde_json::json!(pack.state_snapshot_id),
    );
    if let Some(evidence_digest) = evidence_digest {
        insert_if_missing(
            object,
            "evidence_digest",
            serde_json::json!(evidence_digest),
        );
    }
    if current_state_was_missing {
        vec![WorkbookDiagnostic::repaired_required_workbook_field(
            pack,
            "current_state",
            pack.current_state.clone(),
            "repaired from read-only case-state anchor",
            attempted_string(object, "verb"),
            attempted_string(object, "transition_ref"),
        )]
    } else {
        vec![]
    }
}

fn repair_decode_fields(
    arguments: &mut serde_json::Value,
    pack: &SemOsLanguagePack,
) -> Vec<WorkbookDiagnostic> {
    let Some(object) = arguments.as_object_mut() else {
        return vec![];
    };

    let mut diagnostics = Vec::new();
    if required_field_missing(object.get("transition_ref")) {
        if let Some(transition) = unambiguous_transition_for_decode_repair(object, pack) {
            object.insert(
                "transition_ref".to_string(),
                serde_json::json!(transition.transition_ref.clone()),
            );
            diagnostics.push(WorkbookDiagnostic::repaired_required_workbook_field(
                pack,
                "transition_ref",
                transition.transition_ref.clone(),
                "repaired from unambiguous language-pack transition",
                attempted_string(object, "verb"),
                Some(transition.transition_ref.clone()),
            ));
        }
    }

    if required_field_missing(object.get("requested_state")) {
        if let Some(transition) = object
            .get("transition_ref")
            .and_then(|value| value.as_str())
            .and_then(|transition_ref| transition_by_ref(pack, transition_ref))
        {
            object.insert(
                "requested_state".to_string(),
                serde_json::json!(transition.to_state.clone()),
            );
            diagnostics.push(WorkbookDiagnostic::repaired_required_workbook_field(
                pack,
                "requested_state",
                transition.to_state.clone(),
                "repaired from declared transition effect",
                attempted_string(object, "verb"),
                Some(transition.transition_ref.clone()),
            ));
        }
    }

    diagnostics
}

fn unambiguous_transition_for_decode_repair<'a>(
    object: &serde_json::Map<String, serde_json::Value>,
    pack: &'a SemOsLanguagePack,
) -> Option<&'a super::LanguagePackTransition> {
    if let Some(requested_state) = object
        .get("requested_state")
        .and_then(|value| value.as_str())
    {
        let mut matches = pack
            .candidate_transitions
            .iter()
            .filter(|transition| transition.to_state == requested_state);
        let first = matches.next()?;
        if matches.next().is_none() {
            return Some(first);
        }
        return None;
    }

    if pack.candidate_transitions.len() == 1 {
        return pack.candidate_transitions.first();
    }

    None
}

fn transition_by_ref<'a>(
    pack: &'a SemOsLanguagePack,
    transition_ref: &str,
) -> Option<&'a super::LanguagePackTransition> {
    pack.candidate_transitions
        .iter()
        .find(|transition| transition.transition_ref == transition_ref)
}

fn attempted_string(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    object
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn insert_if_missing(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
    value: serde_json::Value,
) {
    let is_missing = object
        .get(field)
        .map(|value| value.is_null())
        .unwrap_or(true);
    if is_missing {
        object.insert(field.to_string(), value);
    }
}

fn system_prompt() -> &'static str {
    "You draft one private REPL workbook DSL object for kyc-case.update-status. \
Return only the requested tool arguments. Do not invent verbs, transitions, UUIDs, \
states, evidence, or mutation paths. Validation and dry-run decide whether the \
draft is usable."
}

fn user_prompt(
    pack: &SemOsLanguagePack,
    session_id: Uuid,
    actor_id: &str,
    actor_roles: &[String],
    evidence_digest: Option<&str>,
) -> anyhow::Result<String> {
    let payload = serde_json::json!({
        "objective": pack.objective,
        "session_id": session_id,
        "actor_id": actor_id,
        "actor_roles": actor_roles,
        "evidence": {
            "digest": evidence_digest,
            "instruction": "Use this exact digest when present. If absent, leave evidence_digest absent or null; do not invent one."
        },
        "language_pack": pack,
        "draft_contract": {
            "verb": "kyc-case.update-status",
            "subject_kind": pack.subject.kind,
            "case_id": pack.subject.id,
            "configuration_version": pack.configuration_version,
            "state_snapshot_id": pack.state_snapshot_id,
            "evidence_digest_required": true,
            "evidence_digest_may_be_absent_for_structured_refusal": true
        }
    });
    serde_json::to_string(&payload).context("serialize language pack prompt")
}

fn workbook_draft_tool() -> ToolDefinition {
    ToolDefinition {
        name: "draft_kyc_update_status_workbook".to_string(),
        description: "Draft a dry-run-only workbook for kyc-case.update-status".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": [
                "session_id",
                "actor_id",
                "actor_roles",
                "verb",
                "transition_ref",
                "subject_kind",
                "case_id",
                "current_state",
                "requested_state",
                "configuration_version",
                "state_snapshot_id"
            ],
            "properties": {
                "session_id": { "type": "string", "format": "uuid" },
                "actor_id": { "type": "string" },
                "actor_roles": { "type": "array", "items": { "type": "string" } },
                "verb": { "type": "string", "const": "kyc-case.update-status" },
                "transition_ref": { "type": "string" },
                "subject_kind": { "type": "string", "const": "kyc_case" },
                "case_id": { "type": "string", "format": "uuid" },
                "current_state": { "type": "string" },
                "requested_state": { "type": "string" },
                "configuration_version": { "type": "string" },
                "state_snapshot_id": { "type": "string" },
                "evidence_digest": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ob_agentic::llm_client::ToolCallResult;

    use crate::runbook::{
        build_kyc_update_status_language_pack, KycLanguagePackRequest, WorkbookRevisionOutcome,
    };

    const SESSION_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    const CASE_ID: &str = "11111111-1111-1111-1111-111111111111";

    struct StubLlmClient {
        arguments: serde_json::Value,
    }

    #[async_trait]
    impl LlmClient for StubLlmClient {
        async fn chat(&self, _system_prompt: &str, _user_prompt: &str) -> anyhow::Result<String> {
            unreachable!("draft adapter uses tool calls")
        }

        async fn chat_json(
            &self,
            _system_prompt: &str,
            _user_prompt: &str,
        ) -> anyhow::Result<String> {
            unreachable!("draft adapter uses tool calls")
        }

        async fn chat_with_tool(
            &self,
            system_prompt: &str,
            user_prompt: &str,
            tool: &ToolDefinition,
        ) -> anyhow::Result<ToolCallResult> {
            assert!(system_prompt.contains("Validation and dry-run decide"));
            assert!(user_prompt.contains("kyc-case.update-status"));
            assert_eq!(tool.name, "draft_kyc_update_status_workbook");
            Ok(ToolCallResult {
                tool_name: tool.name.clone(),
                arguments: self.arguments.clone(),
            })
        }

        fn model_name(&self) -> &str {
            "stub-model"
        }

        fn provider_name(&self) -> &str {
            "stub-provider"
        }
    }

    #[tokio::test]
    async fn llm_draft_adapter_runs_draft_behind_deterministic_harness() {
        let manifest: DomainPackManifest = serde_yaml::from_str(include_str!(
            "../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
        ))
        .unwrap();
        let session_id = Uuid::parse_str(SESSION_ID).unwrap();
        let case_id = Uuid::parse_str(CASE_ID).unwrap();
        let pack = build_kyc_update_status_language_pack(
            &manifest,
            KycLanguagePackRequest {
                subject_id: case_id,
                current_state: "INTAKE".to_string(),
                configuration_version: "config-1".to_string(),
                state_snapshot_id: "snapshot-1".to_string(),
                objective: Some("Move the KYC case from INTAKE to DISCOVERY".to_string()),
            },
        )
        .unwrap();
        let client = Arc::new(StubLlmClient {
            arguments: serde_json::json!({
                "session_id": session_id,
                "actor_id": "sage",
                "actor_roles": ["ops"],
                "verb": "kyc-case.update-status",
                "transition_ref": "kyc-case.intake-to-discovery",
                "subject_kind": "kyc_case",
                "case_id": case_id,
                "current_state": "INTAKE",
                "requested_state": "DISCOVERY",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let outcome = run_kyc_update_status_llm_draft_loop(
            &manifest,
            &pack,
            session_id,
            "sage",
            vec!["ops".to_string()],
            Some("sha256:evidence".to_string()),
            client,
        )
        .await;

        let LlmDraftLoopOutcome::HarnessCompleted {
            llm_trace,
            draft,
            adapter_diagnostics,
            outcome,
        } = outcome
        else {
            panic!("expected harness completion");
        };
        assert_eq!(llm_trace.provider, "stub-provider");
        assert!(llm_trace.latency_ms.is_some());
        assert!(draft.llm_trace_ref.is_some());
        assert!(adapter_diagnostics.is_empty());
        assert!(matches!(
            outcome,
            WorkbookRevisionOutcome::DryRunValid { .. }
        ));
    }

    #[tokio::test]
    async fn llm_draft_adapter_repairs_missing_transition_ref_from_requested_state() {
        let manifest: DomainPackManifest = serde_yaml::from_str(include_str!(
            "../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
        ))
        .unwrap();
        let session_id = Uuid::parse_str(SESSION_ID).unwrap();
        let case_id = Uuid::parse_str(CASE_ID).unwrap();
        let pack = build_kyc_update_status_language_pack(
            &manifest,
            KycLanguagePackRequest {
                subject_id: case_id,
                current_state: "DISCOVERY".to_string(),
                configuration_version: "config-1".to_string(),
                state_snapshot_id: "snapshot-1".to_string(),
                objective: Some("Move the KYC case from DISCOVERY to ASSESSMENT".to_string()),
            },
        )
        .unwrap();
        let client = Arc::new(StubLlmClient {
            arguments: serde_json::json!({
                "verb": "kyc-case.update-status",
                "subject_kind": "kyc_case",
                "case_id": case_id,
                "current_state": "DISCOVERY",
                "requested_state": "ASSESSMENT",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let outcome = run_kyc_update_status_llm_draft_loop(
            &manifest,
            &pack,
            session_id,
            "sage",
            vec!["ops".to_string()],
            Some("sha256:evidence".to_string()),
            client,
        )
        .await;

        let LlmDraftLoopOutcome::HarnessCompleted {
            draft,
            adapter_diagnostics,
            outcome,
            ..
        } = outcome
        else {
            panic!("expected harness completion");
        };
        assert_eq!(draft.transition_ref, "kyc-case.discovery-to-assessment");
        assert_eq!(adapter_diagnostics.len(), 1);
        assert_eq!(
            adapter_diagnostics[0].error_code,
            "repaired_required_workbook_field"
        );
        assert_eq!(adapter_diagnostics[0].source_path, "draft.transition_ref");
        assert_eq!(
            adapter_diagnostics[0].attempted_verb.as_deref(),
            Some("kyc-case.update-status")
        );
        assert_eq!(
            adapter_diagnostics[0].actual_state.as_deref(),
            Some("missing")
        );
        assert_eq!(
            adapter_diagnostics[0].expected_state.as_deref(),
            Some("kyc-case.discovery-to-assessment")
        );
        assert!(matches!(
            outcome,
            WorkbookRevisionOutcome::DryRunValid { .. }
        ));
    }

    #[tokio::test]
    async fn llm_draft_adapter_can_prompt_with_reduced_pack_and_validate_with_full_pack() {
        let manifest: DomainPackManifest = serde_yaml::from_str(include_str!(
            "../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
        ))
        .unwrap();
        let session_id = Uuid::parse_str(SESSION_ID).unwrap();
        let case_id = Uuid::parse_str(CASE_ID).unwrap();
        let validation_pack = build_kyc_update_status_language_pack(
            &manifest,
            KycLanguagePackRequest {
                subject_id: case_id,
                current_state: "DISCOVERY".to_string(),
                configuration_version: "config-1".to_string(),
                state_snapshot_id: "snapshot-1".to_string(),
                objective: Some("Set this KYC case status to ASSESSMENT".to_string()),
            },
        )
        .unwrap();
        let mut prompt_pack = validation_pack.clone();
        prompt_pack.candidate_transitions.clear();
        prompt_pack.transition_effects.clear();
        prompt_pack.canonical_patterns.clear();
        let client = Arc::new(StubLlmClient {
            arguments: serde_json::json!({
                "verb": "kyc-case.update-status",
                "subject_kind": "kyc_case",
                "case_id": case_id,
                "current_state": "DISCOVERY",
                "requested_state": "ASSESSMENT",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let outcome = run_kyc_update_status_llm_draft_loop_with_prompt_pack(
            &manifest,
            &prompt_pack,
            &validation_pack,
            session_id,
            "sage",
            vec!["ops".to_string()],
            Some("sha256:evidence".to_string()),
            client,
        )
        .await;

        let LlmDraftLoopOutcome::HarnessCompleted {
            draft,
            adapter_diagnostics,
            outcome,
            ..
        } = outcome
        else {
            panic!("expected harness completion");
        };
        assert_eq!(draft.transition_ref, "kyc-case.discovery-to-assessment");
        assert_eq!(adapter_diagnostics[0].source_path, "draft.transition_ref");
        assert!(matches!(
            outcome,
            WorkbookRevisionOutcome::DryRunValid { .. }
        ));
    }

    #[tokio::test]
    async fn llm_draft_adapter_repairs_missing_requested_state_from_transition_ref() {
        let manifest: DomainPackManifest = serde_yaml::from_str(include_str!(
            "../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
        ))
        .unwrap();
        let session_id = Uuid::parse_str(SESSION_ID).unwrap();
        let case_id = Uuid::parse_str(CASE_ID).unwrap();
        let pack = build_kyc_update_status_language_pack(
            &manifest,
            KycLanguagePackRequest {
                subject_id: case_id,
                current_state: "DISCOVERY".to_string(),
                configuration_version: "config-1".to_string(),
                state_snapshot_id: "snapshot-1".to_string(),
                objective: Some("Move the KYC case from DISCOVERY to ASSESSMENT".to_string()),
            },
        )
        .unwrap();
        let client = Arc::new(StubLlmClient {
            arguments: serde_json::json!({
                "verb": "kyc-case.update-status",
                "transition_ref": "kyc-case.discovery-to-assessment",
                "subject_kind": "kyc_case",
                "case_id": case_id,
                "current_state": "DISCOVERY",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let outcome = run_kyc_update_status_llm_draft_loop(
            &manifest,
            &pack,
            session_id,
            "sage",
            vec!["ops".to_string()],
            Some("sha256:evidence".to_string()),
            client,
        )
        .await;

        let LlmDraftLoopOutcome::HarnessCompleted {
            draft,
            adapter_diagnostics,
            outcome,
            ..
        } = outcome
        else {
            panic!("expected harness completion");
        };
        assert_eq!(draft.requested_state, "ASSESSMENT");
        assert_eq!(adapter_diagnostics[0].source_path, "draft.requested_state");
        assert!(matches!(
            outcome,
            WorkbookRevisionOutcome::DryRunValid { .. }
        ));
    }

    #[tokio::test]
    async fn llm_draft_adapter_refuses_ambiguous_missing_transition_ref() {
        let manifest: DomainPackManifest = serde_yaml::from_str(include_str!(
            "../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
        ))
        .unwrap();
        let session_id = Uuid::parse_str(SESSION_ID).unwrap();
        let case_id = Uuid::parse_str(CASE_ID).unwrap();
        let mut pack = build_kyc_update_status_language_pack(
            &manifest,
            KycLanguagePackRequest {
                subject_id: case_id,
                current_state: "DISCOVERY".to_string(),
                configuration_version: "config-1".to_string(),
                state_snapshot_id: "snapshot-1".to_string(),
                objective: Some("Move the KYC case forward".to_string()),
            },
        )
        .unwrap();
        let mut alternate = pack.candidate_transitions[0].clone();
        alternate.transition_ref = "kyc-case.discovery-to-review".to_string();
        alternate.to_state = "REVIEW".to_string();
        pack.candidate_transitions.push(alternate);
        let client = Arc::new(StubLlmClient {
            arguments: serde_json::json!({
                "verb": "kyc-case.update-status",
                "subject_kind": "kyc_case",
                "case_id": case_id,
                "current_state": "DISCOVERY",
                "configuration_version": "config-1",
                "state_snapshot_id": "snapshot-1",
                "evidence_digest": "sha256:evidence"
            }),
        });

        let outcome = run_kyc_update_status_llm_draft_loop(
            &manifest,
            &pack,
            session_id,
            "sage",
            vec!["ops".to_string()],
            Some("sha256:evidence".to_string()),
            client,
        )
        .await;

        let LlmDraftLoopOutcome::AdapterRefused { refusal } = outcome else {
            panic!("expected adapter refusal");
        };
        assert_eq!(refusal.refusal_code, "missing_required_workbook_field");
        assert!(refusal
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source_path == "draft.transition_ref"));
    }

    #[test]
    fn normalizes_known_bindings_without_overwriting_llm_choices() {
        let manifest: DomainPackManifest = serde_yaml::from_str(include_str!(
            "../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
        ))
        .unwrap();
        let session_id = Uuid::parse_str(SESSION_ID).unwrap();
        let case_id = Uuid::parse_str(CASE_ID).unwrap();
        let pack = build_kyc_update_status_language_pack(
            &manifest,
            KycLanguagePackRequest {
                subject_id: case_id,
                current_state: "DISCOVERY".to_string(),
                configuration_version: "config-2".to_string(),
                state_snapshot_id: "snapshot-2".to_string(),
                objective: None,
            },
        )
        .unwrap();
        let mut arguments = serde_json::json!({
            "transition_ref": "kyc-case.discovery-to-assessment",
            "requested_state": "ASSESSMENT",
            "current_state": "WRONG"
        });

        normalize_known_draft_bindings(
            &mut arguments,
            &pack,
            session_id,
            "sage",
            &["ops".to_string()],
            Some("sha256:evidence"),
        );

        assert_eq!(arguments["session_id"], SESSION_ID);
        assert_eq!(arguments["actor_id"], "sage");
        assert_eq!(arguments["verb"], "kyc-case.update-status");
        assert_eq!(arguments["case_id"], CASE_ID);
        assert_eq!(arguments["configuration_version"], "config-2");
        assert_eq!(arguments["state_snapshot_id"], "snapshot-2");
        assert_eq!(arguments["evidence_digest"], "sha256:evidence");
        assert_eq!(arguments["current_state"], "WRONG");
        assert_eq!(
            arguments["transition_ref"],
            "kyc-case.discovery-to-assessment"
        );
    }
}
