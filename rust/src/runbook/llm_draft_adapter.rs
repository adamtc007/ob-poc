//! LLM draft adapter for KYC update-status workbook drafts.
//!
//! The adapter is deliberately not an authority path. It asks an LLM for one
//! typed draft, records hash-only inference metadata, then hands the draft to
//! the deterministic revision/dry-run harness.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use ob_agentic::llm_client::{LlmClient, ToolDefinition};
use sem_os_core::domain_pack::DomainPackManifest;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::llm_trace::{record_llm_inference_trace, workbook_llm_trace_ref, LlmInferenceTrace};

use super::{
    run_kyc_update_status_revision_loop, KycUpdateStatusWorkbookDraft, SemOsLanguagePack,
    WorkbookRevisionOutcome,
};

pub const KYC_UPDATE_STATUS_LLM_DRAFT_PROMPT_TEMPLATE_VERSION: &str =
    "kyc_update_status_workbook_draft_v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmDraftAdapterRefusal {
    pub refusal_code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LlmDraftLoopOutcome {
    HarnessCompleted {
        llm_trace: LlmInferenceTrace,
        draft: KycUpdateStatusWorkbookDraft,
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
    let actor_id = actor_id.into();
    let system_prompt = system_prompt();
    let user_prompt = match user_prompt(
        pack,
        session_id,
        &actor_id,
        &actor_roles,
        evidence_digest.as_deref(),
    ) {
        Ok(prompt) => prompt,
        Err(err) => {
            return LlmDraftLoopOutcome::AdapterRefused {
                refusal: LlmDraftAdapterRefusal {
                    refusal_code: "language_pack_serialization_failed".to_string(),
                    message: err.to_string(),
                },
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
                refusal: LlmDraftAdapterRefusal {
                    refusal_code: "llm_draft_failed".to_string(),
                    message: err.to_string(),
                },
            };
        }
    };
    let llm_latency_ms = elapsed_ms(llm_started_at);

    if result.tool_name != "draft_kyc_update_status_workbook" {
        return LlmDraftLoopOutcome::AdapterRefused {
            refusal: LlmDraftAdapterRefusal {
                refusal_code: "unexpected_tool_call".to_string(),
                message: result.tool_name,
            },
        };
    }

    let response_json = match serde_json::to_string(&result.arguments) {
        Ok(json) => json,
        Err(err) => {
            return LlmDraftLoopOutcome::AdapterRefused {
                refusal: LlmDraftAdapterRefusal {
                    refusal_code: "llm_response_serialization_failed".to_string(),
                    message: err.to_string(),
                },
            };
        }
    };
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

    let mut draft: KycUpdateStatusWorkbookDraft = match serde_json::from_value(result.arguments) {
        Ok(draft) => draft,
        Err(err) => {
            return LlmDraftLoopOutcome::AdapterRefused {
                refusal: LlmDraftAdapterRefusal {
                    refusal_code: "llm_draft_decode_failed".to_string(),
                    message: err.to_string(),
                },
            };
        }
    };
    draft.session_id = session_id;
    draft.actor_id = actor_id;
    draft.actor_roles = actor_roles;
    draft.llm_trace_ref = Some(workbook_llm_trace_ref(&trace));

    let outcome = run_kyc_update_status_revision_loop(manifest, pack, draft.clone());
    LlmDraftLoopOutcome::HarnessCompleted {
        llm_trace: trace,
        draft,
        outcome,
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
            outcome,
        } = outcome
        else {
            panic!("expected harness completion");
        };
        assert_eq!(llm_trace.provider, "stub-provider");
        assert!(llm_trace.latency_ms.is_some());
        assert!(draft.llm_trace_ref.is_some());
        assert!(matches!(
            outcome,
            WorkbookRevisionOutcome::DryRunValid { .. }
        ));
    }
}
