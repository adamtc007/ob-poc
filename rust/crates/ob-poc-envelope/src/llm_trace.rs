//! LLM inference trace contract.
//!
//! The trace stores deterministic hashes and operational metadata only. Raw
//! prompts and raw model responses stay outside the workbook/audit reference so
//! sensitive prompt context can be retained under a separate policy.

use chrono::{DateTime, Utc};
use sem_os_core::context_policy::PromptContextAssembly;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use uuid::Uuid;

/// Boundary handoff: links an executed workbook to the LLM inference trace
/// that produced it. Defined here (not in `runbook::workbook`) because the
/// recorder lives in this crate and `runbook` (execution tier) must depend
/// on the boundary tier, not vice-versa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmTraceRef {
    pub trace_id: Uuid,
    pub prompt_hash: String,
    pub response_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmInferenceTrace {
    pub trace_id: Uuid,
    pub provider: String,
    pub model: String,
    pub model_id: String,
    pub prompt_template_version: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<&'a str>,
    pub prompt_template_version: &'a str,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptContextPolicyAudit {
    pub audit_id: Uuid,
    pub trace_id: Uuid,
    pub policy_version: String,
    pub context_hash: String,
    pub included_count: usize,
    pub redacted_count: usize,
    pub external_llm_allowed: bool,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct O3SourceAttributionMetric {
    pub metric_id: Uuid,
    pub trace_id: Uuid,
    pub required_source_count: usize,
    pub attributed_source_count: usize,
    pub missing_source_count: usize,
    pub coverage_basis_points: u16,
    pub source_ref_hashes: Vec<String>,
    pub recorded_at: DateTime<Utc>,
}

pub fn record_llm_inference_trace(input: LlmInferenceTraceInput<'_>) -> LlmInferenceTrace {
    let prompt_hash = sha256_digest(input.prompt.as_bytes());
    let response_hash = sha256_digest(input.response.as_bytes());
    let trace_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "ob-poc:llm-trace:{}:{}:{}:{}:{}",
            input.provider,
            input.model_id.unwrap_or(input.model),
            input.prompt_template_version,
            prompt_hash,
            response_hash
        )
        .as_bytes(),
    );

    LlmInferenceTrace {
        trace_id,
        provider: input.provider.to_string(),
        model: input.model.to_string(),
        model_id: input.model_id.unwrap_or(input.model).to_string(),
        prompt_template_version: input.prompt_template_version.to_string(),
        prompt_hash,
        response_hash,
        context_hash: input.context_hash.map(str::to_string),
        input_tokens: input.input_tokens,
        output_tokens: input.output_tokens,
        latency_ms: input.latency_ms,
        created_at: Utc::now(),
    }
}

pub fn record_prompt_context_policy_audit(
    trace: &LlmInferenceTrace,
    policy_version: impl AsRef<str>,
    assembly: &PromptContextAssembly,
) -> PromptContextPolicyAudit {
    let context_hash = trace.context_hash.clone().unwrap_or_else(|| {
        sha256_digest(
            serde_json::to_vec(assembly)
                .expect("prompt context assembly serializes")
                .as_slice(),
        )
    });
    let audit_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "ob-poc:prompt-context-policy:{}:{}:{}:{}",
            trace.trace_id,
            policy_version.as_ref(),
            context_hash,
            assembly.redacted.len()
        )
        .as_bytes(),
    );

    PromptContextPolicyAudit {
        audit_id,
        trace_id: trace.trace_id,
        policy_version: policy_version.as_ref().to_string(),
        context_hash,
        included_count: assembly.included.len(),
        redacted_count: assembly.redacted.len(),
        external_llm_allowed: assembly.external_llm_allowed,
        recorded_at: Utc::now(),
    }
}

pub fn record_o3_source_attribution_metric(
    trace: &LlmInferenceTrace,
    source_refs: &[impl AsRef<str>],
    required_source_count: usize,
) -> O3SourceAttributionMetric {
    let unique_source_hashes = source_refs
        .iter()
        .map(|source_ref| sha256_digest(source_ref.as_ref().as_bytes()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let attributed_source_count = unique_source_hashes.len();
    let missing_source_count = required_source_count.saturating_sub(attributed_source_count);
    let coverage_basis_points = (attributed_source_count.min(required_source_count) * 10_000)
        .checked_div(required_source_count)
        .unwrap_or(10_000) as u16;
    let metric_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "ob-poc:o3-source-attribution:{}:{}:{}",
            trace.trace_id,
            required_source_count,
            unique_source_hashes.join(",")
        )
        .as_bytes(),
    );

    O3SourceAttributionMetric {
        metric_id,
        trace_id: trace.trace_id,
        required_source_count,
        attributed_source_count,
        missing_source_count,
        coverage_basis_points,
        source_ref_hashes: unique_source_hashes,
        recorded_at: Utc::now(),
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
    use sem_os_core::context_policy::assemble_prompt_context;
    use sem_os_core::domain_pack::{
        ClassificationLimit, ContextClassificationPolicy, DiscoveryObservation, DiscoveryResponse,
        DiscoverySubject,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn input<'a>() -> LlmInferenceTraceInput<'a> {
        LlmInferenceTraceInput {
            provider: "openai",
            model: "gpt-test",
            model_id: Some("gpt-test"),
            prompt_template_version: "test_prompt_v1",
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

    #[test]
    fn prompt_context_policy_audit_records_redaction_counts_without_raw_context() {
        let trace = record_llm_inference_trace(input());
        let assembly = assemble_prompt_context(
            &ContextClassificationPolicy {
                max_prompt_classification: ClassificationLimit::Internal,
                allow_external_llm: false,
                required_redactions: vec!["case.secret".to_string()],
            },
            &DiscoveryResponse {
                probe_id: "kyc-case.read-evidence-summary".to_string(),
                subject: DiscoverySubject {
                    subject_kind: "kyc_case".to_string(),
                    subject_id: "case-1".to_string(),
                },
                observations: vec![
                    DiscoveryObservation {
                        key: "case.status".to_string(),
                        value: serde_json::json!("DISCOVERY"),
                        classification: Some(ClassificationLimit::Internal),
                    },
                    DiscoveryObservation {
                        key: "case.secret".to_string(),
                        value: serde_json::json!("do not expose"),
                        classification: Some(ClassificationLimit::Restricted),
                    },
                ],
                provenance: vec![],
                first_class_state_mutated: false,
            },
        );

        let audit = record_prompt_context_policy_audit(&trace, "kyc-policy-v1", &assembly);
        let value = serde_json::to_value(&audit).unwrap();

        assert_eq!(audit.trace_id, trace.trace_id);
        assert_eq!(audit.included_count, 1);
        assert_eq!(audit.redacted_count, 1);
        assert!(!audit.external_llm_allowed);
        assert_eq!(value.get("observations"), None);
        assert_eq!(value.get("prompt"), None);
    }

    #[test]
    fn o3_source_attribution_metric_hashes_source_refs_and_records_coverage() {
        let trace = record_llm_inference_trace(input());

        let metric = record_o3_source_attribution_metric(
            &trace,
            &[
                "semos://projection/kyc/case/1",
                "semos://projection/kyc/case/1",
                "semos://evidence/case/1/status",
            ],
            3,
        );

        assert_eq!(metric.trace_id, trace.trace_id);
        assert_eq!(metric.required_source_count, 3);
        assert_eq!(metric.attributed_source_count, 2);
        assert_eq!(metric.missing_source_count, 1);
        assert_eq!(metric.coverage_basis_points, 6666);
        assert_eq!(metric.source_ref_hashes.len(), 2);
        assert!(metric
            .source_ref_hashes
            .iter()
            .all(|hash| hash.starts_with("sha256:")));
    }

    #[test]
    fn raw_llm_provider_calls_are_confined_to_approved_wrappers() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // manifest_dir = repo/rust/crates/ob-poc-envelope; repo_root = repo/
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("ob-poc-envelope crate has repo grand-grandparent");
        let search_roots = [
            repo_root.join("rust/src"),
            repo_root.join("rust/crates/ob-agentic/src"),
        ];
        let allowed_files = BTreeSet::from([
            normalize_path(
                repo_root,
                &repo_root.join("rust/src/research/llm_client.rs"),
            ),
            normalize_path(
                repo_root,
                &repo_root.join("rust/crates/ob-agentic/src/anthropic_client.rs"),
            ),
            normalize_path(
                repo_root,
                &repo_root.join("rust/crates/ob-agentic/src/client_factory.rs"),
            ),
            normalize_path(
                repo_root,
                &repo_root.join("rust/crates/ob-agentic/src/claude_code_cli_client.rs"),
            ),
            normalize_path(
                repo_root,
                &repo_root.join("rust/crates/ob-agentic/src/openai_client.rs"),
            ),
        ]);
        let raw_llm_markers = vec![
            ["https://api.", "openai.com/v1/chat/completions"].concat(),
            ["https://api.", "anthropic.com/v1/messages"].concat(),
            ["OPENAI", "_API_KEY"].concat(),
            ["ANTHROPIC", "_API_KEY"].concat(),
        ];
        let mut violations = vec![];

        for root in search_roots {
            visit_rs_files(&root, &mut |path| {
                let source = fs::read_to_string(path).expect("read Rust source");
                if raw_llm_markers
                    .iter()
                    .any(|marker| source.contains(marker.as_str()))
                {
                    let relative = normalize_path(repo_root, path);
                    if !allowed_files.contains(&relative) {
                        violations.push(relative);
                    }
                }
            });
        }

        violations.sort();
        violations.dedup();
        assert!(
            violations.is_empty(),
            "raw LLM provider calls must use approved wrappers: {violations:?}"
        );
    }

    fn visit_rs_files(root: &Path, visitor: &mut impl FnMut(&Path)) {
        for entry in fs::read_dir(root).expect("read source directory") {
            let entry = entry.expect("read source entry");
            let path = entry.path();
            if path.is_dir() {
                visit_rs_files(&path, visitor);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                visitor(&path);
            }
        }
    }

    fn normalize_path(repo_root: &Path, path: &Path) -> String {
        path.strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}
