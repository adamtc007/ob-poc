//! LLM inference trace contract.
//!
//! The trace stores deterministic hashes and operational metadata only. Raw
//! prompts and raw model responses stay outside the workbook/audit reference so
//! sensitive prompt context can be retained under a separate policy.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::runbook::LlmTraceRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmInferenceTrace {
    pub trace_id: Uuid,
    pub provider: String,
    pub model: String,
    pub prompt_hash: String,
    pub response_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmInferenceTraceInput<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub prompt: &'a str,
    pub response: &'a str,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hash: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

pub fn record_llm_inference_trace(input: LlmInferenceTraceInput<'_>) -> LlmInferenceTrace {
    let prompt_hash = sha256_digest(input.prompt.as_bytes());
    let response_hash = sha256_digest(input.response.as_bytes());
    let trace_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "ob-poc:llm-trace:{}:{}:{}:{}",
            input.provider, input.model, prompt_hash, response_hash
        )
        .as_bytes(),
    );

    LlmInferenceTrace {
        trace_id,
        provider: input.provider.to_string(),
        model: input.model.to_string(),
        prompt_hash,
        response_hash,
        context_hash: input.context_hash.map(str::to_string),
        input_tokens: input.input_tokens,
        output_tokens: input.output_tokens,
        latency_ms: input.latency_ms,
        created_at: Utc::now(),
    }
}

pub fn workbook_llm_trace_ref(trace: &LlmInferenceTrace) -> LlmTraceRef {
    LlmTraceRef {
        trace_id: trace.trace_id,
        prompt_hash: trace.prompt_hash.clone(),
        response_hash: trace.response_hash.clone(),
    }
}

fn sha256_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>() -> LlmInferenceTraceInput<'a> {
        LlmInferenceTraceInput {
            provider: "openai",
            model: "gpt-test",
            prompt: "system: classify\nuser: hello",
            response: "{\"intent\":\"read\"}",
            context_hash: Some("sha256:context"),
            input_tokens: Some(12),
            output_tokens: Some(5),
            latency_ms: Some(42),
        }
    }

    #[test]
    fn llm_trace_hashes_prompt_and_response_without_raw_payloads() {
        let trace = record_llm_inference_trace(input());
        let value = serde_json::to_value(&trace).unwrap();

        assert!(trace.prompt_hash.starts_with("sha256:"));
        assert!(trace.response_hash.starts_with("sha256:"));
        assert_eq!(trace.context_hash.as_deref(), Some("sha256:context"));
        assert_eq!(value.get("prompt"), None);
        assert_eq!(value.get("response"), None);
    }

    #[test]
    fn llm_trace_id_is_stable_for_same_provider_model_and_hashes() {
        let first = record_llm_inference_trace(input());
        let second = record_llm_inference_trace(input());

        assert_eq!(first.trace_id, second.trace_id);
        assert_eq!(first.prompt_hash, second.prompt_hash);
        assert_eq!(first.response_hash, second.response_hash);
    }

    #[test]
    fn llm_trace_ref_binds_to_workbook_reference_shape() {
        let trace = record_llm_inference_trace(input());
        let reference = workbook_llm_trace_ref(&trace);

        assert_eq!(reference.trace_id, trace.trace_id);
        assert_eq!(reference.prompt_hash, trace.prompt_hash);
        assert_eq!(reference.response_hash, trace.response_hash);
    }
}
