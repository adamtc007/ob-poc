//! Session trace infrastructure — append-only log capturing every session mutation.
//!
//! The trace is a monotonically sequenced log of operations applied to a session.
//! It powers replay (R9), compliance auditing, and regression testing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::session::{AgentMode, WorkspaceKind};

// ---------------------------------------------------------------------------
// SnapshotPolicy
// ---------------------------------------------------------------------------

/// Controls when hydrated state snapshots are captured in trace entries.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotPolicy {
    /// Never capture snapshots.
    #[default]
    Never,
    /// Capture every N operations.
    EveryN(u32),
    /// Capture on every stack operation.
    OnStackOp,
    /// Capture on every verb execution.
    OnExecution,
}

// ---------------------------------------------------------------------------
// FrameRef — lightweight stack snapshot
// ---------------------------------------------------------------------------

/// Lightweight reference to a workspace frame captured at trace time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrameRef {
    pub workspace: WorkspaceKind,
    pub constellation_map: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<Uuid>,
    #[serde(default)]
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceValidationStep {
    pub step_number: u8,
    pub step_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceLanguageLoopEvent {
    pub phase: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TracePerformanceMetrics {
    #[serde(default)]
    pub prompt_route_ms: u64,
    #[serde(default)]
    pub prompt_route_us: u64,
    #[serde(default)]
    pub language_pack_ms: u64,
    #[serde(default)]
    pub language_pack_us: u64,
    #[serde(default)]
    pub llm_draft_ms: u64,
    #[serde(default)]
    pub llm_draft_us: u64,
    #[serde(default)]
    pub revision_loop_ms: u64,
    #[serde(default)]
    pub revision_loop_us: u64,
    #[serde(default)]
    pub dry_run_ms: u64,
    #[serde(default)]
    pub dry_run_us: u64,
    #[serde(default)]
    pub acp_emit_ms: u64,
    #[serde(default)]
    pub acp_emit_us: u64,
    #[serde(default)]
    pub total_ms: u64,
    #[serde(default)]
    pub total_us: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceConversationEfficiency {
    pub outcome: String,
    #[serde(default)]
    pub local_revision_count: u8,
    #[serde(default)]
    pub estimated_user_repair_turns_avoided: u64,
    #[serde(default)]
    pub pending_user_turn_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_reason: Option<String>,
    #[serde(default)]
    pub first_pass_valid: bool,
    #[serde(default)]
    pub dry_run_valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_failure_mode: Option<String>,
    #[serde(default)]
    pub prose_only_failure: bool,
}

// ---------------------------------------------------------------------------
// TraceOp — discriminated operation tag
// ---------------------------------------------------------------------------

/// The operation that occurred at this trace entry.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TraceOp {
    StackPush {
        workspace: WorkspaceKind,
    },
    StackPop {
        workspace: WorkspaceKind,
    },
    StackCommit,
    VerbExecuted {
        verb_fqn: String,
        step_id: Uuid,
    },
    RunbookCompiled {
        runbook_id: String,
    },
    RunbookApproved {
        runbook_id: String,
    },
    AcpSessionOpened {
        adapter: String,
        mutation_capability: String,
        #[serde(default)]
        acp_persona_mode: String,
        #[serde(default)]
        capability_negotiation: Vec<String>,
    },
    AcpContextAssembled {
        pack_id: String,
        probe_id: String,
        context_hash: String,
        redacted_count: usize,
    },
    AcpProjectionServed {
        projection_kind: String,
        projection_hash: String,
        classification: String,
        redacted_count: usize,
        #[serde(default)]
        acp_mode: String,
        #[serde(default)]
        acp_persona_mode: String,
        #[serde(default)]
        sage_workflow_phase: String,
        #[serde(default)]
        mechanisms: Vec<String>,
        #[serde(default)]
        fallback_summary: Vec<String>,
        #[serde(default)]
        acp_mechanism_summary: Vec<String>,
        #[serde(default)]
        acp_fallback_summary: Vec<String>,
        #[serde(default)]
        projected_surface_summary: Vec<String>,
        #[serde(default)]
        capability_negotiation: Vec<String>,
        #[serde(default)]
        projection_count: usize,
        #[serde(default)]
        projection_bytes: usize,
        #[serde(default)]
        projection_latency_ms: u64,
    },
    WorkbookDryRunValidated {
        workbook_id: String,
        transition_ref: String,
        #[serde(default)]
        semantic_diff_uri: String,
        #[serde(default)]
        validation_trace: Vec<TraceValidationStep>,
    },
    AcpLanguageLoopTraced {
        outcome: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pack_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject_id: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verb: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current_state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requested_state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transition_ref: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workbook_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        semantic_diff_uri: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refusal_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pending_question_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        draft_source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        llm_trace_id: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        llm_provider: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        llm_model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        llm_prompt_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        llm_response_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        diagnostic_source_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt_context_variant: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        registry_schema_version: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        registry_projection_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        registry_verified: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        envelope_schema_version: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        envelope_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        envelope_pack_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        envelope_projection_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        envelope_verified: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_schema_version: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_pack_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_snapshot_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_redaction_policy: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_freshness_policy: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_static_envelope_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_projection_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_verified: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime_redacted_count: Option<usize>,
        #[serde(default)]
        runtime_blocked_field_codes: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        projection_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selected_template_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selected_macro_id: Option<String>,
        #[serde(default)]
        decode_repair_count: u64,
        #[serde(default)]
        outcome_layer: String,
        #[serde(default)]
        diagnostic_codes: Vec<String>,
        #[serde(default)]
        revision_count: u8,
        #[serde(default)]
        dry_run_valid: bool,
        #[serde(default)]
        first_pass_valid: bool,
        #[serde(default)]
        human_summary: String,
        #[serde(default)]
        needed_from_user: Vec<String>,
        #[serde(default)]
        trace: Vec<TraceLanguageLoopEvent>,
        #[serde(default)]
        performance: TracePerformanceMetrics,
        #[serde(default)]
        conversation_efficiency: TraceConversationEfficiency,
    },
    ApprovalTokenIssued {
        approval_token_id: String,
        workbook_id: String,
        approved_by_actor_id: String,
    },
    RestrictedMutationPreflightPrepared {
        workbook_id: String,
        approval_token_id: String,
        transition_ref: String,
    },
    LlmInferenceTraced {
        trace_id: Uuid,
        provider: String,
        model: String,
        #[serde(default)]
        model_id: String,
        #[serde(default)]
        prompt_template_version: String,
        prompt_hash: String,
        response_hash: String,
    },
    StateTransition {
        from: String,
        to: String,
    },
    Input {
        utterance_hash: String,
    },
    /// A shared fact was superseded (cross-workspace consistency).
    SharedFactSuperseded {
        atom_path: String,
        entity_id: Uuid,
        new_version: i32,
    },
    /// A consuming constellation was replayed after shared fact change.
    ConstellationReplayed {
        workspace: String,
        constellation_family: String,
        outcome: String,
    },
    /// A remediation event changed state.
    RemediationStateChange {
        remediation_id: Uuid,
        from_status: String,
        to_status: String,
    },
}

// ---------------------------------------------------------------------------
// TraceEntry — one row in the append-only trace log
// ---------------------------------------------------------------------------

/// A single entry in the session trace log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    pub session_id: Uuid,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub agent_mode: AgentMode,
    pub op: TraceOp,
    pub stack_snapshot: Vec<FrameRef>,
    /// Hydrated state snapshot (when `SnapshotPolicy` triggers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<serde_json::Value>,
    /// Session feedback snapshot at the time of this operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_feedback: Option<serde_json::Value>,
    /// Verb FQN if a verb was resolved during this turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb_resolved: Option<String>,
    /// Execution result snapshot (step outcome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_result: Option<serde_json::Value>,
}

impl TraceEntry {
    /// Create a new trace entry.
    pub fn new(
        session_id: Uuid,
        sequence: u64,
        agent_mode: AgentMode,
        op: TraceOp,
        stack_snapshot: Vec<FrameRef>,
    ) -> Self {
        Self {
            session_id,
            sequence,
            timestamp: Utc::now(),
            agent_mode,
            op,
            stack_snapshot,
            snapshot: None,
            session_feedback: None,
            verb_resolved: None,
            execution_result: None,
        }
    }

    /// Attach a session feedback snapshot.
    pub fn with_session_feedback(mut self, feedback: serde_json::Value) -> Self {
        self.session_feedback = Some(feedback);
        self
    }

    /// Attach the resolved verb FQN.
    pub fn with_verb_resolved(mut self, verb_fqn: String) -> Self {
        self.verb_resolved = Some(verb_fqn);
        self
    }

    /// Attach an execution result snapshot.
    pub fn with_execution_result(mut self, result: serde_json::Value) -> Self {
        self.execution_result = Some(result);
        self
    }

    /// Attach a hydrated state snapshot.
    pub fn with_snapshot(mut self, snapshot: serde_json::Value) -> Self {
        self.snapshot = Some(snapshot);
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_entry_serde_round_trip() {
        let entry = TraceEntry::new(
            Uuid::nil(),
            1,
            AgentMode::Sage,
            TraceOp::StackPush {
                workspace: WorkspaceKind::Deal,
            },
            vec![FrameRef {
                workspace: WorkspaceKind::Cbu,
                constellation_map: "cbu-onboarding".into(),
                subject_id: None,
                stale: false,
            }],
        );
        let json = serde_json::to_value(&entry).unwrap();
        let back: TraceEntry = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back.session_id, Uuid::nil());
        assert_eq!(back.sequence, 1);
        assert_eq!(
            back.op,
            TraceOp::StackPush {
                workspace: WorkspaceKind::Deal
            }
        );
        assert_eq!(back.stack_snapshot.len(), 1);
    }

    #[test]
    fn trace_op_serde_variants() {
        let ops = vec![
            TraceOp::StackPush {
                workspace: WorkspaceKind::Kyc,
            },
            TraceOp::StackPop {
                workspace: WorkspaceKind::Deal,
            },
            TraceOp::StackCommit,
            TraceOp::VerbExecuted {
                verb_fqn: "cbu.create".into(),
                step_id: Uuid::nil(),
            },
            TraceOp::RunbookCompiled {
                runbook_id: "abc123".into(),
            },
            TraceOp::RunbookApproved {
                runbook_id: "abc123".into(),
            },
            TraceOp::AcpSessionOpened {
                adapter: "zed".into(),
                mutation_capability: "none".into(),
                acp_persona_mode: "sage:planning".into(),
                capability_negotiation: vec![
                    "declined:fs/write_text_file".into(),
                    "declined:terminal/create".into(),
                ],
            },
            TraceOp::AcpContextAssembled {
                pack_id: "ob-poc.kyc".into(),
                probe_id: "kyc-case.read-state".into(),
                context_hash: "sha256:abc".into(),
                redacted_count: 1,
            },
            TraceOp::AcpProjectionServed {
                projection_kind: "dag".into(),
                projection_hash: "sha256:projection".into(),
                classification: "internal".into(),
                redacted_count: 0,
                acp_mode: "discovery".into(),
                acp_persona_mode: "sage:planning".into(),
                sage_workflow_phase: "discovery".into(),
                mechanisms: vec!["projection_get".into()],
                fallback_summary: vec![],
                acp_mechanism_summary: vec!["projection_get".into()],
                acp_fallback_summary: vec![],
                projected_surface_summary: vec!["dag:internal:sha256:projection".into()],
                capability_negotiation: vec![
                    "declined:fs/write_text_file".into(),
                    "declined:terminal/create".into(),
                ],
                projection_count: 1,
                projection_bytes: 128,
                projection_latency_ms: 3,
            },
            TraceOp::WorkbookDryRunValidated {
                workbook_id: "ewb:v1:abc".into(),
                transition_ref: "kyc-case.intake-to-discovery".into(),
                semantic_diff_uri: "semos://semantic-diff/ewb:v1:abc".into(),
                validation_trace: vec![TraceValidationStep {
                    step_number: 1,
                    step_id: "integrity".into(),
                    status: "passed".into(),
                    message: "workbook integrity hash verified".into(),
                }],
            },
            TraceOp::AcpLanguageLoopTraced {
                outcome: "dry_run_validated".into(),
                pack_id: Some("ob-poc.kyc".into()),
                subject_id: Some(Uuid::nil()),
                verb: Some("kyc-case.update-status".into()),
                current_state: Some("INTAKE".into()),
                requested_state: Some("DISCOVERY".into()),
                transition_ref: Some("kyc-case.intake-to-discovery".into()),
                workbook_id: Some("ewb:v1:abc".into()),
                semantic_diff_uri: Some("semos://semantic-diff/ewb:v1:abc".into()),
                refusal_code: None,
                pending_question_code: None,
                draft_source: Some("deterministic".into()),
                llm_trace_id: None,
                llm_provider: None,
                llm_model: None,
                llm_prompt_hash: None,
                llm_response_hash: None,
                diagnostic_source_path: None,
                prompt_context_variant: None,
                registry_schema_version: None,
                registry_projection_hash: None,
                registry_verified: None,
                envelope_schema_version: None,
                envelope_hash: None,
                envelope_pack_id: None,
                envelope_projection_hash: None,
                envelope_verified: None,
                runtime_schema_version: None,
                runtime_pack_id: None,
                runtime_snapshot_id: None,
                runtime_hash: None,
                runtime_redaction_policy: None,
                runtime_freshness_policy: None,
                runtime_static_envelope_hash: None,
                runtime_projection_hash: None,
                runtime_verified: None,
                runtime_redacted_count: None,
                runtime_blocked_field_codes: vec![],
                projection_hash: None,
                selected_template_id: None,
                selected_macro_id: None,
                decode_repair_count: 0,
                outcome_layer: "dry_run_validated".into(),
                diagnostic_codes: vec![],
                revision_count: 1,
                dry_run_valid: true,
                first_pass_valid: false,
                human_summary: "Validated a dry-run workbook after local revision.".into(),
                needed_from_user: vec![],
                trace: vec![TraceLanguageLoopEvent {
                    phase: "dry_run".into(),
                    status: "completed".into(),
                    message: "semos://semantic-diff/ewb:v1:abc".into(),
                }],
                performance: TracePerformanceMetrics {
                    language_pack_ms: 1,
                    revision_loop_ms: 2,
                    dry_run_ms: 1,
                    total_ms: 4,
                    ..TracePerformanceMetrics::default()
                },
                conversation_efficiency: TraceConversationEfficiency {
                    outcome: "dry_run_validated".into(),
                    local_revision_count: 1,
                    estimated_user_repair_turns_avoided: 1,
                    pending_user_turn_required: false,
                    first_pass_valid: false,
                    dry_run_valid: true,
                    prose_only_failure: false,
                    ..TraceConversationEfficiency::default()
                },
            },
            TraceOp::ApprovalTokenIssued {
                approval_token_id: "approval:v1:abc".into(),
                workbook_id: "ewb:v1:abc".into(),
                approved_by_actor_id: "approver@example.com".into(),
            },
            TraceOp::RestrictedMutationPreflightPrepared {
                workbook_id: "ewb:v1:abc".into(),
                approval_token_id: "approval:v1:abc".into(),
                transition_ref: "kyc-case.intake-to-discovery".into(),
            },
            TraceOp::LlmInferenceTraced {
                trace_id: Uuid::nil(),
                provider: "anthropic".into(),
                model: "claude-sonnet-4-6".into(),
                model_id: "claude-sonnet-4-6".into(),
                prompt_template_version: "sage_outcome_classifier_v2_sonnet_4_6".into(),
                prompt_hash: "sha256:prompt".into(),
                response_hash: "sha256:response".into(),
            },
            TraceOp::StateTransition {
                from: "draft".into(),
                to: "ready".into(),
            },
            TraceOp::Input {
                utterance_hash: "sha256:...".into(),
            },
        ];
        for op in &ops {
            let json = serde_json::to_value(op).unwrap();
            let back: TraceOp = serde_json::from_value(json).unwrap();
            assert_eq!(&back, op);
        }
    }

    #[test]
    fn sequence_monotonicity() {
        let mut seq = 0u64;
        let entries: Vec<TraceEntry> = (0..5)
            .map(|_| {
                seq += 1;
                TraceEntry::new(
                    Uuid::nil(),
                    seq,
                    AgentMode::Sage,
                    TraceOp::StackCommit,
                    vec![],
                )
            })
            .collect();
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.sequence, (i + 1) as u64);
        }
    }

    #[test]
    fn snapshot_policy_default() {
        assert_eq!(SnapshotPolicy::default(), SnapshotPolicy::Never);
    }
}
