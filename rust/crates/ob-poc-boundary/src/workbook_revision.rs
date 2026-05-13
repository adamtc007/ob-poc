//! Bounded deterministic revision loop for KYC update-status workbook drafts.
//!
//! This module models the safety loop a future LLM adapter must use. The LLM
//! may draft, but validation decides. The loop terminates after at most two
//! revisions with either a valid dry-run or a structured refusal.

use sem_os_core::domain_pack::DomainPackManifest;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

use crate::kyc_dry_run::{
    build_kyc_update_status_dry_run_with_manifest, KycUpdateStatusDryRunInput,
    KycUpdateStatusDryRunOutput,
};
use crate::language_pack::SemOsLanguagePack;
use crate::workbook_diagnostics::{diagnostics_from_dry_run_refusal, WorkbookDiagnostic};

pub const MAX_WORKBOOK_REVISIONS: u8 = 2;
const KYC_UPDATE_STATUS_VERB: &str = "kyc-case.update-status";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KycUpdateStatusWorkbookDraft {
    pub session_id: Uuid,
    pub actor_id: String,
    #[serde(default)]
    pub actor_roles: Vec<String>,
    pub verb: String,
    pub transition_ref: String,
    pub subject_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_id: Option<Uuid>,
    pub current_state: String,
    pub requested_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_trace_ref: Option<crate::llm_trace::LlmTraceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbookDraftAttempt {
    pub attempt_number: u8,
    pub draft: KycUpdateStatusWorkbookDraft,
    pub diagnostics: Vec<WorkbookDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageAcquisitionMetrics {
    pub language_pack_generated: bool,
    pub invented_verb_count: u32,
    pub uuid_binding_complete: bool,
    pub state_valid_transition_selected: bool,
    pub first_pass_valid: bool,
    pub revision_count: u8,
    pub dry_run_valid: bool,
    #[serde(default)]
    pub dry_run_ms: u64,
    #[serde(default)]
    pub dry_run_us: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredWorkbookRefusal {
    pub refusal_code: String,
    pub diagnostics: Vec<WorkbookDiagnostic>,
    pub revision_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkbookRevisionOutcome {
    DryRunValid {
        output: Box<KycUpdateStatusDryRunOutput>,
        attempts: Vec<WorkbookDraftAttempt>,
        metrics: LanguageAcquisitionMetrics,
        trace: Vec<LanguageLoopTraceEvent>,
    },
    Refused {
        refusal: StructuredWorkbookRefusal,
        attempts: Vec<WorkbookDraftAttempt>,
        metrics: LanguageAcquisitionMetrics,
        trace: Vec<LanguageLoopTraceEvent>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageLoopTraceEvent {
    pub phase: String,
    pub status: String,
    pub message: String,
}

pub fn run_kyc_update_status_revision_loop(
    manifest: &DomainPackManifest,
    pack: &SemOsLanguagePack,
    draft: KycUpdateStatusWorkbookDraft,
) -> WorkbookRevisionOutcome {
    let mut trace = vec![trace_event(
        "language_pack",
        "completed",
        "SemOS language pack retrieved",
    )];
    let mut attempts = Vec::new();
    let mut current = draft;
    let mut revision_count = 0;
    let mut invented_verb_count = 0;
    let mut first_pass_valid = false;
    let mut dry_run_us = 0;

    loop {
        trace.push(trace_event(
            "draft",
            "completed",
            format!("Workbook draft attempt {}", revision_count + 1),
        ));

        let diagnostics = validate_draft_preflight(&current, pack);
        invented_verb_count += diagnostics
            .iter()
            .filter(|diag| diag.error_code == "invented_verb")
            .count() as u32;

        if !diagnostics.is_empty() {
            attempts.push(WorkbookDraftAttempt {
                attempt_number: revision_count + 1,
                draft: current.clone(),
                diagnostics: diagnostics.clone(),
            });
            trace.push(trace_event(
                "validation",
                "failed",
                diagnostics[0].error_code.clone(),
            ));
            if revision_count >= MAX_WORKBOOK_REVISIONS || !can_revise(&diagnostics) {
                return refused(
                    pack,
                    attempts,
                    trace,
                    revision_count,
                    invented_verb_count,
                    first_pass_valid,
                    dry_run_us,
                    diagnostics,
                );
            }
            current = revise_draft(current, pack, &diagnostics);
            revision_count += 1;
            trace.push(trace_event(
                "revision",
                "completed",
                "Draft revised from diagnostics",
            ));
            continue;
        }

        let input = draft_to_input(&current);
        let dry_run_started_at = Instant::now();
        let dry_run_result = build_kyc_update_status_dry_run_with_manifest(manifest, input);
        dry_run_us = dry_run_us.saturating_add(elapsed_us(dry_run_started_at));
        match dry_run_result {
            Ok(output) => {
                let attempt = WorkbookDraftAttempt {
                    attempt_number: revision_count + 1,
                    draft: current,
                    diagnostics: vec![],
                };
                attempts.push(attempt);
                first_pass_valid = revision_count == 0;
                trace.push(trace_event(
                    "validation",
                    "passed",
                    "Workbook dry-run is valid",
                ));
                trace.push(trace_event(
                    "dry_run",
                    "completed",
                    output.dry_run.semantic_diff_uri.clone(),
                ));
                let metrics = metrics(
                    pack,
                    &attempts,
                    revision_count,
                    invented_verb_count,
                    first_pass_valid,
                    true,
                    dry_run_us,
                    None,
                );
                return WorkbookRevisionOutcome::DryRunValid {
                    output: Box::new(output),
                    attempts,
                    metrics,
                    trace,
                };
            }
            Err(refusal) => {
                let diagnostics = diagnostics_from_dry_run_refusal(&refusal, pack);
                attempts.push(WorkbookDraftAttempt {
                    attempt_number: revision_count + 1,
                    draft: current.clone(),
                    diagnostics: diagnostics.clone(),
                });
                trace.push(trace_event(
                    "validation",
                    "failed",
                    diagnostics[0].error_code.clone(),
                ));
                if revision_count >= MAX_WORKBOOK_REVISIONS || !can_revise(&diagnostics) {
                    return refused(
                        pack,
                        attempts,
                        trace,
                        revision_count,
                        invented_verb_count,
                        first_pass_valid,
                        dry_run_us,
                        diagnostics,
                    );
                }
                current = revise_draft(current, pack, &diagnostics);
                revision_count += 1;
                trace.push(trace_event(
                    "revision",
                    "completed",
                    "Draft revised from diagnostics",
                ));
            }
        }
    }
}

pub fn validate_kyc_update_status_draft_without_revision(
    manifest: &DomainPackManifest,
    pack: &SemOsLanguagePack,
    draft: &KycUpdateStatusWorkbookDraft,
) -> Result<KycUpdateStatusDryRunOutput, Vec<WorkbookDiagnostic>> {
    let diagnostics = validate_draft_preflight(draft, pack);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    build_kyc_update_status_dry_run_with_manifest(manifest, draft_to_input(draft))
        .map_err(|refusal| diagnostics_from_dry_run_refusal(&refusal, pack))
}

fn validate_draft_preflight(
    draft: &KycUpdateStatusWorkbookDraft,
    pack: &SemOsLanguagePack,
) -> Vec<WorkbookDiagnostic> {
    let mut diagnostics = Vec::new();

    if draft.verb != KYC_UPDATE_STATUS_VERB {
        diagnostics.push(WorkbookDiagnostic::invented_verb(pack, draft.verb.clone()));
    }
    if draft.subject_kind != pack.subject.kind {
        diagnostics.push(WorkbookDiagnostic::wrong_subject_kind(
            pack,
            draft.subject_kind.clone(),
        ));
    }
    if draft.case_id.is_none() {
        diagnostics.push(WorkbookDiagnostic::missing_uuid(pack, "case_id"));
    }
    if draft.configuration_version != pack.configuration_version {
        diagnostics.push(WorkbookDiagnostic::stale_anchor(
            pack,
            "configuration_version",
            draft.configuration_version.clone(),
        ));
    }
    if draft.state_snapshot_id != pack.state_snapshot_id {
        diagnostics.push(WorkbookDiagnostic::stale_anchor(
            pack,
            "state_snapshot_id",
            draft.state_snapshot_id.clone(),
        ));
    }
    if draft
        .evidence_digest
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        diagnostics.push(WorkbookDiagnostic::missing_evidence(pack));
    }

    diagnostics
}

fn can_revise(diagnostics: &[WorkbookDiagnostic]) -> bool {
    diagnostics.iter().all(|diag| {
        matches!(
            diag.error_code.as_str(),
            "unknown_transition"
                | "current_state_mismatch"
                | "requested_state_mismatch"
                | "missing_uuid_binding"
        )
    })
}

fn revise_draft(
    mut draft: KycUpdateStatusWorkbookDraft,
    pack: &SemOsLanguagePack,
    diagnostics: &[WorkbookDiagnostic],
) -> KycUpdateStatusWorkbookDraft {
    for diag in diagnostics {
        match diag.error_code.as_str() {
            "unknown_transition" => {
                if let Some(transition) =
                    transition_for_requested_state(pack, &draft.requested_state)
                        .or_else(|| pack.candidate_transitions.first())
                {
                    draft.transition_ref = transition.transition_ref.clone();
                    draft.current_state = transition.from_state.clone();
                    draft.requested_state = transition.to_state.clone();
                }
            }
            "current_state_mismatch" => {
                draft.current_state = pack.current_state.clone();
            }
            "requested_state_mismatch" => {
                if let Some(transition) = transition_by_ref(pack, &draft.transition_ref)
                    .or_else(|| pack.candidate_transitions.first())
                {
                    draft.requested_state = transition.to_state.clone();
                }
            }
            "missing_uuid_binding" => {
                draft.case_id = Some(pack.subject.id);
            }
            _ => {}
        }
    }
    draft
}

fn transition_for_requested_state<'a>(
    pack: &'a SemOsLanguagePack,
    requested_state: &str,
) -> Option<&'a crate::language_pack::LanguagePackTransition> {
    pack.candidate_transitions
        .iter()
        .find(|transition| transition.to_state == requested_state)
}

fn transition_by_ref<'a>(
    pack: &'a SemOsLanguagePack,
    transition_ref: &str,
) -> Option<&'a crate::language_pack::LanguagePackTransition> {
    pack.candidate_transitions
        .iter()
        .find(|transition| transition.transition_ref == transition_ref)
}

fn draft_to_input(draft: &KycUpdateStatusWorkbookDraft) -> KycUpdateStatusDryRunInput {
    KycUpdateStatusDryRunInput {
        session_id: draft.session_id,
        case_id: draft.case_id.expect("case_id preflight checked"),
        actor_id: draft.actor_id.clone(),
        actor_roles: draft.actor_roles.clone(),
        transition_ref: draft.transition_ref.clone(),
        current_state: draft.current_state.clone(),
        requested_state: draft.requested_state.clone(),
        configuration_version: draft.configuration_version.clone(),
        state_snapshot_id: draft.state_snapshot_id.clone(),
        evidence_digest: draft
            .evidence_digest
            .clone()
            .expect("evidence preflight checked"),
        llm_trace_ref: draft.llm_trace_ref.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn refused(
    pack: &SemOsLanguagePack,
    attempts: Vec<WorkbookDraftAttempt>,
    mut trace: Vec<LanguageLoopTraceEvent>,
    revision_count: u8,
    invented_verb_count: u32,
    first_pass_valid: bool,
    dry_run_us: u64,
    diagnostics: Vec<WorkbookDiagnostic>,
) -> WorkbookRevisionOutcome {
    let refusal_code = diagnostics
        .first()
        .map(|diag| diag.error_code.clone())
        .unwrap_or_else(|| "unknown_refusal".to_string());
    trace.push(trace_event_refusal(&refusal_code));
    let metrics = metrics(
        pack,
        &attempts,
        revision_count,
        invented_verb_count,
        first_pass_valid,
        false,
        dry_run_us,
        Some(refusal_code.clone()),
    );
    WorkbookRevisionOutcome::Refused {
        refusal: StructuredWorkbookRefusal {
            refusal_code,
            diagnostics,
            revision_count,
        },
        attempts,
        metrics,
        trace,
    }
}

#[allow(clippy::too_many_arguments)]
fn metrics(
    pack: &SemOsLanguagePack,
    attempts: &[WorkbookDraftAttempt],
    revision_count: u8,
    invented_verb_count: u32,
    first_pass_valid: bool,
    dry_run_valid: bool,
    dry_run_us: u64,
    refusal_code: Option<String>,
) -> LanguageAcquisitionMetrics {
    let last_draft = attempts.last().map(|attempt| &attempt.draft);
    LanguageAcquisitionMetrics {
        language_pack_generated: true,
        invented_verb_count,
        uuid_binding_complete: last_draft.and_then(|draft| draft.case_id).is_some(),
        state_valid_transition_selected: last_draft
            .map(|draft| transition_by_ref(pack, &draft.transition_ref).is_some())
            .unwrap_or(false),
        first_pass_valid,
        revision_count,
        dry_run_valid,
        dry_run_ms: millis_from_micros(dry_run_us),
        dry_run_us,
        refusal_code,
    }
}

fn elapsed_us(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn millis_from_micros(micros: u64) -> u64 {
    micros / 1_000
}

fn trace_event(
    phase: impl Into<String>,
    status: impl Into<String>,
    message: impl Into<String>,
) -> LanguageLoopTraceEvent {
    LanguageLoopTraceEvent {
        phase: phase.into(),
        status: status.into(),
        message: message.into(),
    }
}

fn trace_event_refusal(refusal_code: &str) -> LanguageLoopTraceEvent {
    trace_event("refusal", "completed", refusal_code.to_string())
}
