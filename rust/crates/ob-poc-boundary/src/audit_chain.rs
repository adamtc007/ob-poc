//! Audit-chain reconstruction for the configuration-native workflow.
//!
//! This module validates the in-memory relationship between session trace
//! entries, an Execution Workbook, the DSL Coder dry-run result, and optional
//! LLM inference metadata. It intentionally stores hashes and references only;
//! raw prompts, model responses, and evidence payloads stay outside the chain.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::approval_token::ApprovalTokenId;
use crate::dsl_drafter::{DslDrafterDryRunResult, DslDrafterValidationStepStatus};
use crate::llm_trace::LlmInferenceTrace;
use crate::mutation_preflight::RestrictedMutationPreflight;
use crate::session_trace::{TraceEntry, TraceOp, TraceValidationStep};
use crate::workbook::{ExecutionWorkbook, ExecutionWorkbookId, ExecutionWorkbookValidationError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditChainProof {
    pub session_id: Uuid,
    pub workbook_id: ExecutionWorkbookId,
    pub transition_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_trace_id: Option<Uuid>,
    pub trace_sequences: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestrictedMutationAuditChainProof {
    pub session_id: Uuid,
    pub workbook_id: ExecutionWorkbookId,
    pub approval_token_id: ApprovalTokenId,
    pub transition_ref: String,
    pub trace_sequences: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditChainValidationError {
    WorkbookIntegrity {
        error: ExecutionWorkbookValidationError,
    },
    WorkbookSessionMismatch {
        expected: Uuid,
        actual: Uuid,
    },
    DryRunWorkbookMismatch {
        expected: String,
        actual: String,
    },
    DryRunTransitionMismatch {
        expected: String,
        actual: String,
    },
    DryRunSimulationMismatch,
    DryRunSemanticDiffUriMismatch {
        expected: String,
        actual: String,
    },
    DryRunValidationTraceMismatch {
        workbook_id: String,
    },
    MissingWorkbookDryRunTrace {
        workbook_id: String,
    },
    LlmTraceRequired {
        trace_id: Uuid,
    },
    UnexpectedLlmTrace {
        trace_id: Uuid,
    },
    LlmTraceRefMismatch {
        trace_id: Uuid,
    },
    MissingLlmTraceEntry {
        trace_id: Uuid,
    },
    MissingContextTrace {
        context_hash: String,
    },
    MissingApprovalTokenTrace {
        approval_token_id: String,
    },
    MissingRestrictedMutationPreflightTrace {
        workbook_id: String,
        approval_token_id: String,
    },
}

pub fn validate_audit_chain(
    session_id: Uuid,
    trace_entries: &[TraceEntry],
    workbook: &ExecutionWorkbook,
    dry_run: &DslDrafterDryRunResult,
    llm_trace: Option<&LlmInferenceTrace>,
) -> Result<AuditChainProof, AuditChainValidationError> {
    workbook
        .validate_integrity()
        .map_err(|error| AuditChainValidationError::WorkbookIntegrity { error })?;

    if workbook.core.session_id != session_id {
        return Err(AuditChainValidationError::WorkbookSessionMismatch {
            expected: session_id,
            actual: workbook.core.session_id,
        });
    }

    if dry_run.workbook_id != workbook.id.to_string() {
        return Err(AuditChainValidationError::DryRunWorkbookMismatch {
            expected: workbook.id.to_string(),
            actual: dry_run.workbook_id.clone(),
        });
    }

    if dry_run.transition_ref != workbook.core.transition_ref {
        return Err(AuditChainValidationError::DryRunTransitionMismatch {
            expected: workbook.core.transition_ref.clone(),
            actual: dry_run.transition_ref.clone(),
        });
    }

    if dry_run.semantic_diff != workbook.core.simulation {
        return Err(AuditChainValidationError::DryRunSimulationMismatch);
    }

    let expected_validation_trace = trace_validation_steps(dry_run);
    let mut trace_sequences = BTreeMap::new();
    let dry_run_sequence = trace_entries
        .iter()
        .find(|entry| {
            entry.session_id == session_id
                && matches!(
                    &entry.op,
                    TraceOp::WorkbookDryRunValidated {
                        workbook_id,
                        transition_ref,
                        semantic_diff_uri,
                        validation_trace,
                    } if workbook_id == &workbook.id.to_string()
                        && transition_ref == &workbook.core.transition_ref
                        && semantic_diff_uri == &dry_run.semantic_diff_uri
                        && validation_trace == &expected_validation_trace
                )
        })
        .map(|entry| entry.sequence)
        .ok_or_else(|| AuditChainValidationError::MissingWorkbookDryRunTrace {
            workbook_id: workbook.id.to_string(),
        })?;
    trace_sequences.insert("workbook_dry_run_validated".to_string(), dry_run_sequence);

    let llm_trace_id = match (&workbook.core.llm_trace_ref, llm_trace) {
        (Some(reference), Some(trace)) => {
            if reference.trace_id != trace.trace_id
                || reference.prompt_hash != trace.prompt_hash
                || reference.response_hash != trace.response_hash
            {
                return Err(AuditChainValidationError::LlmTraceRefMismatch {
                    trace_id: reference.trace_id,
                });
            }

            let llm_sequence = trace_entries
                .iter()
                .find(|entry| {
                    entry.session_id == session_id
                        && matches!(
                            &entry.op,
                            TraceOp::LlmInferenceTraced {
                                trace_id,
                                provider,
                                model,
                                model_id,
                                prompt_template_version,
                                prompt_hash,
                                response_hash,
                            } if trace_id == &trace.trace_id
                                && provider == &trace.provider
                                && model == &trace.model
                                && model_id == &trace.model_id
                                && prompt_template_version == &trace.prompt_template_version
                                && prompt_hash == &trace.prompt_hash
                                && response_hash == &trace.response_hash
                        )
                })
                .map(|entry| entry.sequence)
                .ok_or(AuditChainValidationError::MissingLlmTraceEntry {
                    trace_id: trace.trace_id,
                })?;
            trace_sequences.insert("llm_inference_traced".to_string(), llm_sequence);
            Some(trace.trace_id)
        }
        (Some(reference), None) => {
            return Err(AuditChainValidationError::LlmTraceRequired {
                trace_id: reference.trace_id,
            });
        }
        (None, Some(trace)) => {
            return Err(AuditChainValidationError::UnexpectedLlmTrace {
                trace_id: trace.trace_id,
            });
        }
        (None, None) => None,
    };

    let context_hash = llm_trace.and_then(|trace| trace.context_hash.clone());
    if let Some(context_hash) = context_hash.as_ref() {
        let context_sequence = trace_entries
            .iter()
            .find(|entry| {
                entry.session_id == session_id
                    && matches!(
                        &entry.op,
                        TraceOp::AcpContextAssembled {
                            context_hash: trace_context_hash,
                            ..
                        } if trace_context_hash == context_hash
                    )
            })
            .map(|entry| entry.sequence)
            .ok_or_else(|| AuditChainValidationError::MissingContextTrace {
                context_hash: context_hash.clone(),
            })?;
        trace_sequences.insert("acp_context_assembled".to_string(), context_sequence);
    }

    Ok(AuditChainProof {
        session_id,
        workbook_id: workbook.id.clone(),
        transition_ref: workbook.core.transition_ref.clone(),
        context_hash,
        llm_trace_id,
        trace_sequences,
    })
}

fn trace_validation_steps(dry_run: &DslDrafterDryRunResult) -> Vec<TraceValidationStep> {
    dry_run
        .validation_trace
        .iter()
        .map(|step| TraceValidationStep {
            step_number: step.step_number,
            step_id: step.step_id.clone(),
            status: match step.status {
                DslDrafterValidationStepStatus::Passed => "passed",
                DslDrafterValidationStepStatus::Failed => "failed",
                DslDrafterValidationStepStatus::Skipped => "skipped",
            }
            .to_string(),
            message: step.message.clone(),
        })
        .collect()
}

pub fn validate_restricted_mutation_audit_chain(
    session_id: Uuid,
    trace_entries: &[TraceEntry],
    preflight: &RestrictedMutationPreflight,
) -> Result<RestrictedMutationAuditChainProof, AuditChainValidationError> {
    let mut trace_sequences = BTreeMap::new();

    let approval_sequence = trace_entries
        .iter()
        .find(|entry| {
            entry.session_id == session_id
                && matches!(
                    &entry.op,
                    TraceOp::ApprovalTokenIssued {
                        approval_token_id,
                        workbook_id,
                        approved_by_actor_id,
                    } if approval_token_id == &preflight.approval.approval_token_id.to_string()
                        && workbook_id == &preflight.workbook_id.to_string()
                        && approved_by_actor_id == &preflight.approval.approved_by_actor_id
                )
        })
        .map(|entry| entry.sequence)
        .ok_or_else(|| AuditChainValidationError::MissingApprovalTokenTrace {
            approval_token_id: preflight.approval.approval_token_id.to_string(),
        })?;
    trace_sequences.insert("approval_token_issued".to_string(), approval_sequence);

    let preflight_sequence = trace_entries
        .iter()
        .find(|entry| {
            entry.session_id == session_id
                && matches!(
                    &entry.op,
                    TraceOp::RestrictedMutationPreflightPrepared {
                        workbook_id,
                        approval_token_id,
                        transition_ref,
                    } if workbook_id == &preflight.workbook_id.to_string()
                        && approval_token_id == &preflight.approval.approval_token_id.to_string()
                        && transition_ref == &preflight.transition_ref
                )
        })
        .map(|entry| entry.sequence)
        .ok_or_else(
            || AuditChainValidationError::MissingRestrictedMutationPreflightTrace {
                workbook_id: preflight.workbook_id.to_string(),
                approval_token_id: preflight.approval.approval_token_id.to_string(),
            },
        )?;
    trace_sequences.insert(
        "restricted_mutation_preflight_prepared".to_string(),
        preflight_sequence,
    );

    Ok(RestrictedMutationAuditChainProof {
        session_id,
        workbook_id: preflight.workbook_id.clone(),
        approval_token_id: preflight.approval.approval_token_id.clone(),
        transition_ref: preflight.transition_ref.clone(),
        trace_sequences,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval_token::{ApprovalTokenId, RestrictedMutationApprovalCheck};
    use crate::dsl_drafter::{
        DslDrafterDryRunResult, DslDrafterValidationStep, DslDrafterValidationStepStatus,
    };
    use crate::llm_trace::{
        record_llm_inference_trace, workbook_llm_trace_ref, LlmInferenceTraceInput,
    };
    use crate::mutation_preflight::{
        MutationExecutor, MutationSemanticDiff, RestrictedMutationPreflight,
    };
    use crate::session::AgentMode;
    use crate::session_trace::TraceEntry;
    use crate::workbook::{
        EvidenceRef, ExecutionWorkbook, ExecutionWorkbookCore, StaleWorkbookPolicy, WorkbookActor,
        WorkbookExecutionMode, WorkbookSubject,
    };
    use chrono::Utc;
    use sem_os_core::state_simulation::{
        SemanticStateDiff, SimulatedStateAdvance, StateSimulationResult,
    };
    use uuid::uuid;

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

    fn simulation() -> StateSimulationResult {
        StateSimulationResult {
            transition_ref: "kyc-case.discovery-to-assessment".to_string(),
            entity_id: CASE_ID,
            entity_type: "kyc_case".to_string(),
            state_machine: "kyc_case_lifecycle".to_string(),
            from_state: "DISCOVERY".to_string(),
            to_state: "ASSESSMENT".to_string(),
            verb: "kyc-case.update-status".to_string(),
            semantic_diff: SemanticStateDiff {
                field: "status".to_string(),
                before: "DISCOVERY".to_string(),
                after: "ASSESSMENT".to_string(),
            },
            predicted_advance: SimulatedStateAdvance {
                entity_id: CASE_ID,
                to_node: "ASSESSMENT".to_string(),
                slot_path: "kyc-case/workstream".to_string(),
                reason: "configuration transition".to_string(),
                writes_since_push_delta: 0,
            },
            state_snapshot_id: Some("snapshot-1".to_string()),
            configuration_version: Some("config-1".to_string()),
        }
    }

    fn llm_trace() -> LlmInferenceTrace {
        record_llm_inference_trace(LlmInferenceTraceInput {
            provider: "anthropic",
            model: "claude-sonnet-4-6",
            model_id: Some("claude-sonnet-4-6"),
            prompt_template_version: "sage_outcome_classifier_v2_sonnet_4_6",
            prompt: "redacted prompt",
            response: "{\"intent\":\"kyc-case.update-status\"}",
            context_hash: Some("sha256:context"),
            input_tokens: Some(10),
            output_tokens: Some(8),
            latency_ms: Some(25),
        })
    }

    fn workbook(with_llm_trace: bool) -> ExecutionWorkbook {
        let trace = llm_trace();
        ExecutionWorkbook::new(ExecutionWorkbookCore {
            schema_version: 1,
            pack_id: "ob-poc.kyc".to_string(),
            transition_ref: "kyc-case.discovery-to-assessment".to_string(),
            execution_mode: WorkbookExecutionMode::DryRun,
            session_id: SESSION_ID,
            subject: WorkbookSubject {
                subject_kind: "kyc_case".to_string(),
                subject_id: CASE_ID,
            },
            actor: WorkbookActor {
                actor_id: "analyst@example.com".to_string(),
                roles: vec!["analyst".to_string()],
            },
            configuration_version: "config-1".to_string(),
            state_snapshot_id: "snapshot-1".to_string(),
            objective: "Move KYC case from discovery to assessment".to_string(),
            user_prompt_ref: None,
            editor_context_refs: vec![],
            evidence_refs: vec![EvidenceRef {
                kind: "case_id".to_string(),
                ref_id: CASE_ID.to_string(),
                digest: "sha256:evidence".to_string(),
                source_system: None,
                field_path: None,
                classification: None,
            }],
            llm_trace_ref: with_llm_trace.then(|| workbook_llm_trace_ref(&trace)),
            expected_preconditions: vec![],
            expected_postconditions: vec![],
            invariant_checks: vec![],
            governance_checks: vec![],
            simulation: simulation(),
            stale_policy: StaleWorkbookPolicy::Reject,
            previous_workbook_id: None,
            metadata: BTreeMap::new(),
        })
        .expect("workbook")
    }

    fn dry_run(workbook: &ExecutionWorkbook) -> DslDrafterDryRunResult {
        DslDrafterDryRunResult {
            workbook_id: workbook.id.to_string(),
            transition_ref: workbook.core.transition_ref.clone(),
            semantic_diff: workbook.core.simulation.clone(),
            semantic_diff_uri: format!("semos://semantic-diff/{}", workbook.id),
            validation_trace: vec![DslDrafterValidationStep {
                step_number: 3,
                step_id: "integrity".to_string(),
                status: DslDrafterValidationStepStatus::Passed,
                message: "workbook integrity hash verified".to_string(),
            }],
        }
    }

    fn mutation_preflight(workbook: &ExecutionWorkbook) -> RestrictedMutationPreflight {
        let approval = RestrictedMutationApprovalCheck {
            workbook_id: workbook.id.clone(),
            approval_token_id: ApprovalTokenId("approval:v1:abc".to_string()),
            transition_ref: workbook.core.transition_ref.clone(),
            approved_by_actor_id: "approver@example.com".to_string(),
            expires_at: Utc::now(),
        };

        RestrictedMutationPreflight {
            workbook_id: workbook.id.clone(),
            approval,
            verb: workbook.core.simulation.verb.clone(),
            transition_ref: workbook.core.transition_ref.clone(),
            intended_diff: MutationSemanticDiff {
                subject_id: workbook.core.subject.subject_id,
                field: "status".to_string(),
                before: "DISCOVERY".to_string(),
                after: "ASSESSMENT".to_string(),
            },
            predicted_diff: workbook.core.simulation.clone(),
            actual_diff: None,
            executor: MutationExecutor::ExistingRunbookGateOnly,
            runbook_args: BTreeMap::new(),
        }
    }

    fn trace_entries(workbook: &ExecutionWorkbook, trace: &LlmInferenceTrace) -> Vec<TraceEntry> {
        vec![
            TraceEntry::new(
                SESSION_ID,
                1,
                AgentMode::Sage,
                TraceOp::AcpContextAssembled {
                    pack_id: "ob-poc.kyc".to_string(),
                    probe_id: "kyc-case.read-state".to_string(),
                    context_hash: "sha256:context".to_string(),
                    redacted_count: 1,
                },
                vec![],
            ),
            TraceEntry::new(
                SESSION_ID,
                2,
                AgentMode::Sage,
                TraceOp::LlmInferenceTraced {
                    trace_id: trace.trace_id,
                    provider: trace.provider.clone(),
                    model: trace.model.clone(),
                    model_id: trace.model_id.clone(),
                    prompt_template_version: trace.prompt_template_version.clone(),
                    prompt_hash: trace.prompt_hash.clone(),
                    response_hash: trace.response_hash.clone(),
                },
                vec![],
            ),
            TraceEntry::new(
                SESSION_ID,
                3,
                AgentMode::Repl,
                TraceOp::WorkbookDryRunValidated {
                    workbook_id: workbook.id.to_string(),
                    transition_ref: workbook.core.transition_ref.clone(),
                    semantic_diff_uri: format!("semos://semantic-diff/{}", workbook.id),
                    validation_trace: trace_validation_steps(&dry_run(workbook)),
                },
                vec![],
            ),
        ]
    }

    #[test]
    fn reconstructs_trace_chain_for_workbook_dry_run_and_llm_trace() {
        let trace = llm_trace();
        let workbook = workbook(true);
        let dry_run = dry_run(&workbook);
        let proof = validate_audit_chain(
            SESSION_ID,
            &trace_entries(&workbook, &trace),
            &workbook,
            &dry_run,
            Some(&trace),
        )
        .expect("audit chain");

        assert_eq!(proof.workbook_id, workbook.id);
        assert_eq!(proof.llm_trace_id, Some(trace.trace_id));
        assert_eq!(proof.context_hash.as_deref(), Some("sha256:context"));
        assert_eq!(
            proof.trace_sequences.get("workbook_dry_run_validated"),
            Some(&3)
        );
    }

    #[test]
    fn refuses_missing_workbook_dry_run_trace() {
        let trace = llm_trace();
        let workbook = workbook(true);
        let dry_run = dry_run(&workbook);
        let err = validate_audit_chain(SESSION_ID, &[], &workbook, &dry_run, Some(&trace))
            .expect_err("missing trace refused");

        assert!(matches!(
            err,
            AuditChainValidationError::MissingWorkbookDryRunTrace { .. }
        ));
    }

    #[test]
    fn refuses_llm_trace_hash_mismatch() {
        let trace = llm_trace();
        let mut workbook = workbook(true);
        workbook.core.llm_trace_ref.as_mut().unwrap().prompt_hash = "sha256:other".to_string();
        workbook.id = crate::workbook::compute_workbook_id(&workbook.core);
        let dry_run = dry_run(&workbook);
        let err = validate_audit_chain(
            SESSION_ID,
            &trace_entries(&workbook, &trace),
            &workbook,
            &dry_run,
            Some(&trace),
        )
        .expect_err("hash mismatch refused");

        assert!(matches!(
            err,
            AuditChainValidationError::LlmTraceRefMismatch { .. }
        ));
    }

    #[test]
    fn permits_dry_run_chain_without_llm_trace_reference() {
        let trace = llm_trace();
        let workbook = workbook(false);
        let dry_run = dry_run(&workbook);
        let entries = vec![TraceEntry::new(
            SESSION_ID,
            1,
            AgentMode::Repl,
            TraceOp::WorkbookDryRunValidated {
                workbook_id: workbook.id.to_string(),
                transition_ref: workbook.core.transition_ref.clone(),
                semantic_diff_uri: dry_run.semantic_diff_uri.clone(),
                validation_trace: trace_validation_steps(&dry_run),
            },
            vec![],
        )];

        let proof = validate_audit_chain(SESSION_ID, &entries, &workbook, &dry_run, None)
            .expect("audit chain without llm");

        assert_eq!(proof.llm_trace_id, None);
        assert_eq!(proof.context_hash, None);
        assert_ne!(workbook.id.to_string(), trace.trace_id.to_string());
    }

    #[test]
    fn reconstructs_restricted_mutation_preflight_trace_chain() {
        let workbook = workbook(false);
        let preflight = mutation_preflight(&workbook);
        let entries = vec![
            TraceEntry::new(
                SESSION_ID,
                10,
                AgentMode::Repl,
                TraceOp::ApprovalTokenIssued {
                    approval_token_id: preflight.approval.approval_token_id.to_string(),
                    workbook_id: preflight.workbook_id.to_string(),
                    approved_by_actor_id: preflight.approval.approved_by_actor_id.clone(),
                },
                vec![],
            ),
            TraceEntry::new(
                SESSION_ID,
                11,
                AgentMode::Repl,
                TraceOp::RestrictedMutationPreflightPrepared {
                    workbook_id: preflight.workbook_id.to_string(),
                    approval_token_id: preflight.approval.approval_token_id.to_string(),
                    transition_ref: preflight.transition_ref.clone(),
                },
                vec![],
            ),
        ];

        let proof = validate_restricted_mutation_audit_chain(SESSION_ID, &entries, &preflight)
            .expect("restricted mutation trace chain");

        assert_eq!(proof.workbook_id, workbook.id);
        assert_eq!(
            proof.trace_sequences.get("approval_token_issued"),
            Some(&10)
        );
        assert_eq!(
            proof
                .trace_sequences
                .get("restricted_mutation_preflight_prepared"),
            Some(&11)
        );
    }

    #[test]
    fn refuses_restricted_mutation_chain_without_preflight_trace() {
        let workbook = workbook(false);
        let preflight = mutation_preflight(&workbook);
        let entries = vec![TraceEntry::new(
            SESSION_ID,
            10,
            AgentMode::Repl,
            TraceOp::ApprovalTokenIssued {
                approval_token_id: preflight.approval.approval_token_id.to_string(),
                workbook_id: preflight.workbook_id.to_string(),
                approved_by_actor_id: preflight.approval.approved_by_actor_id.clone(),
            },
            vec![],
        )];

        let err = validate_restricted_mutation_audit_chain(SESSION_ID, &entries, &preflight)
            .expect_err("missing preflight trace refused");

        assert!(matches!(
            err,
            AuditChainValidationError::MissingRestrictedMutationPreflightTrace { .. }
        ));
    }
}
