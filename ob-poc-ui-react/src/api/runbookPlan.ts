/**
 * Runbook Plan API Client
 *
 * API calls for multi-workspace runbook plan compilation, approval, execution,
 * and session trace replay.
 * Maps to backend routes at /api/session/:id/runbook/* and /api/session/:id/trace/*
 * (see repl_routes_v2.rs).
 */

import { api } from "./client";

// ============================================================================
// Types matching Rust backend (plan_types.rs, narration.rs, session_trace.rs)
// ============================================================================

export type WorkspaceKind =
  | "product_maintenance"
  | "deal"
  | "cbu"
  | "kyc"
  | "instrument_matrix"
  | "on_boarding";

export type SubjectKind =
  | "client_group"
  | "cbu"
  | "deal"
  | "case"
  | "handoff"
  | "matrix"
  | "product"
  | "service"
  | "resource"
  | "attribute";

export type PlanStepStatus =
  | "pending"
  | "ready"
  | "executing"
  | "succeeded"
  | "failed"
  | "skipped";

/** How a plan step references an entity. */
export type EntityBinding =
  | { kind: "literal"; id: string }
  | { kind: "forward_ref"; source_step: number; output_field: string };

/** A single step in a multi-workspace runbook plan. */
export interface RunbookPlanStep {
  seq: number;
  workspace: WorkspaceKind;
  constellation_map: string;
  subject_kind: SubjectKind;
  subject_binding: EntityBinding;
  verb: { verb_fqn: string; display_name: string };
  sentence: string;
  args: Record<string, string>;
  preconditions: string[];
  expected_effect: string;
  depends_on: number[];
  status: PlanStepStatus;
}

/** Named entity bindings and their resolved values. */
export interface BindingTable {
  entries: Record<string, EntityBinding>;
  resolved: Record<string, string>;
}

/** Overall plan status (tagged union). */
export type RunbookPlanStatus =
  | { status: "compiled" }
  | { status: "awaiting_approval" }
  | { status: "approved" }
  | { status: "executing"; cursor: number }
  | { status: "completed"; completed_at: string }
  | { status: "failed"; error: string; failed_step?: number }
  | { status: "cancelled" };

/** Approval record. */
export interface RunbookApproval {
  approved_by: string;
  approved_at: string;
  plan_hash: string;
}

/** Full runbook plan as returned by GET /api/session/:id/runbook/plan. */
export interface RunbookPlan {
  id: string;
  session_id: string;
  compiled_at: string;
  source_research: number[];
  steps: RunbookPlanStep[];
  bindings: BindingTable;
  status: RunbookPlanStatus;
  approval?: RunbookApproval;
}

// --- Narration types (narration.rs) ---

export type NarrationOutcome =
  | {
      outcome: "success";
      what_changed: string;
      state_now: string;
      what_next: string[];
    }
  | { outcome: "failed"; error: string; recovery_hint?: string }
  | { outcome: "skipped"; reason: string };

export interface StepNarration {
  step_index: number;
  verb_fqn: string;
  sentence: string;
  outcome: NarrationOutcome;
  workspace: WorkspaceKind;
  stale_warning?: string;
}

export interface PlanNarration {
  plan_id: string;
  total_steps: number;
  completed: number;
  failed: number;
  skipped: number;
  step_narrations: StepNarration[];
  aggregate_summary: string;
}

// --- Trace types (session_trace.rs) ---

export interface FrameRef {
  workspace: WorkspaceKind;
  constellation_map: string;
  subject_id?: string;
  stale: boolean;
}

export interface TraceValidationStep {
  step_number: number;
  step_id: string;
  status: "passed" | "failed" | "skipped" | string;
  message: string;
}

export type TraceOp =
  | { op: "stack_push"; workspace: WorkspaceKind }
  | { op: "stack_pop"; workspace: WorkspaceKind }
  | { op: "stack_commit" }
  | { op: "verb_executed"; verb_fqn: string; step_id: string }
  | { op: "runbook_compiled"; runbook_id: string }
  | { op: "runbook_approved"; runbook_id: string }
  | {
      op: "acp_session_opened";
      adapter: string;
      mutation_capability: string;
    }
  | {
      op: "acp_context_assembled";
      pack_id: string;
      probe_id: string;
      context_hash: string;
      redacted_count: number;
    }
  | {
      op: "acp_projection_served";
      projection_kind: string;
      projection_hash: string;
      classification: string;
      redacted_count: number;
      acp_mode?: string;
      acp_persona_mode?: string;
      sage_workflow_phase?: string;
      mechanisms?: string[];
      fallback_summary?: string[];
      acp_mechanism_summary?: string[];
      acp_fallback_summary?: string[];
      projected_surface_summary?: string[];
      capability_negotiation?: string[];
      projection_count?: number;
      projection_bytes?: number;
      projection_latency_ms?: number;
    }
  | {
      op: "workbook_dry_run_validated";
      workbook_id: string;
      transition_ref: string;
      semantic_diff_uri?: string;
      validation_trace?: TraceValidationStep[];
    }
  | {
      op: "approval_token_issued";
      approval_token_id: string;
      workbook_id: string;
      approved_by_actor_id: string;
    }
  | {
      op: "restricted_mutation_preflight_prepared";
      workbook_id: string;
      approval_token_id: string;
      transition_ref: string;
    }
  | {
      op: "llm_inference_traced";
      trace_id: string;
      provider: string;
      model: string;
      model_id?: string;
      prompt_template_version?: string;
      prompt_hash: string;
      response_hash: string;
    }
  | { op: "state_transition"; from: string; to: string }
  | { op: "input"; utterance_hash: string }
  | {
      op: "shared_fact_superseded";
      atom_path: string;
      entity_id: string;
      new_version: number;
    }
  | {
      op: "constellation_replayed";
      workspace: string;
      constellation_family: string;
      outcome: string;
    }
  | {
      op: "remediation_state_change";
      remediation_id: string;
      from_status: string;
      to_status: string;
    };

export interface TraceEntry {
  session_id: string;
  sequence: number;
  timestamp: string;
  agent_mode: string;
  op: TraceOp;
  stack_snapshot: FrameRef[];
  snapshot?: unknown;
  session_feedback?: unknown;
  verb_resolved?: string;
  execution_result?: unknown;
}

// --- Replay types ---

export interface ReplayResult {
  mode: string;
  entries_replayed: number;
  divergences: Array<{
    sequence: number;
    expected: string;
    actual: string;
  }>;
  final_state?: unknown;
}

// ============================================================================
// Compile / Approve / Execute / Cancel / Status
// ============================================================================

export interface CompileResult {
  status: string;
  plan_id: string;
  step_count: number;
}

export interface ApproveResult {
  status: string;
  plan_id: string;
}

export interface CancelResult {
  status: string;
  steps_cancelled: number;
}

export interface RunbookStatusResult {
  plan_id?: string;
  status: RunbookPlanStatus | string;
  total_steps?: number;
  cursor?: number;
}

// ============================================================================
// Execution Workbook Dry-Run
// ============================================================================

export interface KycUpdateStatusDryRunRequest {
  case_id: string;
  transition_ref?: string;
  current_state: string;
  requested_state: string;
  configuration_version: string;
  state_snapshot_id: string;
  evidence_digest: string;
  actor_id: string;
  actor_roles?: string[];
}

export interface WorkbookEvidenceRef {
  kind: string;
  ref_id: string;
  digest: string;
  source_system?: string;
  field_path?: string;
  classification?: string;
}

export interface WorkbookCheck {
  check_id: string;
  status: "passed" | "failed" | "not_evaluated";
  message: string;
}

export interface WorkbookSubject {
  subject_kind: string;
  subject_id: string;
}

export interface WorkbookActor {
  actor_id: string;
  roles: string[];
}

export interface SemanticStateDiff {
  field: string;
  before: string;
  after: string;
}

export interface SimulatedStateAdvance {
  entity_id: string;
  to_node: string;
  slot_path: string;
  reason: string;
  writes_since_push_delta: number;
}

export interface StateSimulationResult {
  transition_ref: string;
  entity_id: string;
  entity_type: string;
  state_machine: string;
  from_state: string;
  to_state: string;
  verb: string;
  semantic_diff: SemanticStateDiff;
  predicted_advance: SimulatedStateAdvance;
  state_snapshot_id?: string;
  configuration_version?: string;
}

export interface ExecutionWorkbookCore {
  schema_version: number;
  pack_id: string;
  transition_ref: string;
  execution_mode: "dry_run" | "execute_after_approval" | "execute";
  session_id: string;
  subject: WorkbookSubject;
  actor: WorkbookActor;
  configuration_version: string;
  state_snapshot_id: string;
  objective: string;
  user_prompt_ref?: string;
  editor_context_refs?: string[];
  evidence_refs: WorkbookEvidenceRef[];
  llm_trace_ref?: unknown;
  expected_preconditions?: string[];
  expected_postconditions?: string[];
  invariant_checks?: WorkbookCheck[];
  governance_checks?: WorkbookCheck[];
  simulation: StateSimulationResult;
  stale_policy: "reject" | "revalidate" | "rebind_if_equivalent";
  previous_workbook_id?: string;
  metadata: Record<string, string>;
}

export interface ExecutionWorkbook {
  id: string;
  core: ExecutionWorkbookCore;
  status: "draft" | "validated" | "superseded" | "executed" | "rejected";
  created_at: string;
}

export interface DslCoderDryRunResult {
  workbook_id: string;
  transition_ref: string;
  semantic_diff: StateSimulationResult;
  semantic_diff_uri: string;
  validation_trace: TraceValidationStep[];
}

export interface KycUpdateStatusDryRunResult {
  status: "dry_run_validated";
  workbook: ExecutionWorkbook;
  dry_run: DslCoderDryRunResult;
}

export interface KycApprovalTokenRequest {
  workbook: ExecutionWorkbook;
  approved_by_actor_id: string;
  approval_text: string;
  expires_at: string;
}

export interface MutationApprovalTokenCore {
  schema_version: number;
  workbook_id: string;
  session_id: string;
  pack_id: string;
  transition_ref: string;
  subject: WorkbookSubject;
  requested_by_actor_id: string;
  approved_by_actor_id: string;
  approval_text: string;
  configuration_version: string;
  state_snapshot_id: string;
  evidence_refs: WorkbookEvidenceRef[];
  expires_at: string;
}

export interface MutationApprovalToken {
  id: string;
  core: MutationApprovalTokenCore;
  issued_at: string;
  status: "active" | "consumed" | "revoked";
}

export interface KycApprovalTokenResult {
  status: "approval_token_issued";
  approval_token: MutationApprovalToken;
}

export interface KycRestrictedMutationPreflightRequest {
  workbook: ExecutionWorkbook;
  approval_token: MutationApprovalToken;
  observed_configuration_version: string;
  observed_state_snapshot_id: string;
  observed_evidence_refs: WorkbookEvidenceRef[];
  consumed_token_ids?: string[];
}

export interface MutationSemanticDiff {
  subject_id: string;
  field: string;
  before: string;
  after: string;
}

export interface RestrictedMutationApprovalCheck {
  workbook_id: string;
  approval_token_id: string;
  transition_ref: string;
  approved_by_actor_id: string;
  expires_at: string;
}

export interface RestrictedMutationPreflight {
  workbook_id: string;
  approval: RestrictedMutationApprovalCheck;
  verb: string;
  transition_ref: string;
  intended_diff: MutationSemanticDiff;
  predicted_diff: StateSimulationResult;
  actual_diff?: MutationSemanticDiff | null;
  executor: "existing_runbook_gate_only";
  runbook_args: Record<string, string>;
}

export interface KycRestrictedMutationPreflightResult {
  status: "restricted_mutation_preflight_prepared";
  preflight: RestrictedMutationPreflight;
}

export interface KycRestrictedMutationCompileRunbookRequest {
  preflight: RestrictedMutationPreflight;
}

export interface RestrictedMutationCompiledStep {
  step_id: string;
  sentence: string;
  verb: string;
  dsl: string;
  args: Record<string, string>;
  depends_on: string[];
  execution_mode: "sync" | "durable" | "human_gate";
  write_set: string[];
  verb_contract_snapshot_id?: string | null;
}

export interface RestrictedMutationCompiledRunbook {
  id: string;
  session_id: string;
  version: number;
  steps: RestrictedMutationCompiledStep[];
  envelope: unknown;
  status: { status: "compiled" } | Record<string, unknown>;
  created_at: string;
}

export interface RestrictedMutationRunbookCompilation {
  compiled_runbook_id: string;
  workbook_id: string;
  approval_token_id: string;
  transition_ref: string;
  expected_diff: MutationSemanticDiff;
  compiled_runbook: RestrictedMutationCompiledRunbook;
}

export interface KycRestrictedMutationCompileRunbookResult {
  status: "restricted_mutation_runbook_compiled";
  compilation: RestrictedMutationRunbookCompilation;
}

// ============================================================================
// API Client
// ============================================================================

export const runbookPlanApi = {
  /**
   * Compile a multi-workspace runbook plan from the current session state.
   * POST /api/session/:id/runbook/compile
   */
  async compileRunbookPlan(sessionId: string): Promise<CompileResult> {
    return api.post<CompileResult>(`/session/${sessionId}/runbook/compile`);
  },

  /**
   * Get the current compiled runbook plan for rendering.
   * GET /api/session/:id/runbook/plan
   */
  async getRunbookPlan(sessionId: string): Promise<RunbookPlan> {
    return api.get<RunbookPlan>(`/session/${sessionId}/runbook/plan`);
  },

  /**
   * Approve the compiled plan (Compiled -> Approved).
   * POST /api/session/:id/runbook/approve
   */
  async approveRunbookPlan(sessionId: string): Promise<ApproveResult> {
    return api.post<ApproveResult>(`/session/${sessionId}/runbook/approve`);
  },

  /**
   * Execute the next step in the approved plan.
   * POST /api/session/:id/runbook/execute
   */
  async executeRunbookPlanStep(
    sessionId: string,
  ): Promise<Record<string, unknown>> {
    return api.post<Record<string, unknown>>(
      `/session/${sessionId}/runbook/execute`,
    );
  },

  /**
   * Cancel the current runbook plan.
   * POST /api/session/:id/runbook/cancel
   */
  async cancelRunbookPlan(sessionId: string): Promise<CancelResult> {
    return api.post<CancelResult>(`/session/${sessionId}/runbook/cancel`);
  },

  /**
   * Get the current plan status (lightweight poll).
   * GET /api/session/:id/runbook/status
   */
  async getRunbookStatus(sessionId: string): Promise<RunbookStatusResult> {
    return api.get<RunbookStatusResult>(`/session/${sessionId}/runbook/status`);
  },

  /**
   * Validate a non-mutating KYC update-status workbook dry-run.
   * POST /api/session/:id/workbook/kyc/update-status/dry-run
   */
  async dryRunKycUpdateStatusWorkbook(
    sessionId: string,
    request: KycUpdateStatusDryRunRequest,
  ): Promise<KycUpdateStatusDryRunResult> {
    return api.post<KycUpdateStatusDryRunResult>(
      `/session/${sessionId}/workbook/kyc/update-status/dry-run`,
      request,
    );
  },

  /**
   * Issue a workbook-bound restricted-mutation approval token.
   * POST /api/session/:id/workbook/kyc/approval-token
   */
  async issueKycApprovalToken(
    sessionId: string,
    request: KycApprovalTokenRequest,
  ): Promise<KycApprovalTokenResult> {
    return api.post<KycApprovalTokenResult>(
      `/session/${sessionId}/workbook/kyc/approval-token`,
      request,
    );
  },

  /**
   * Prepare restricted mutation preflight without executing mutation.
   * POST /api/session/:id/workbook/kyc/restricted-mutation/preflight
   */
  async prepareKycRestrictedMutationPreflight(
    sessionId: string,
    request: KycRestrictedMutationPreflightRequest,
  ): Promise<KycRestrictedMutationPreflightResult> {
    return api.post<KycRestrictedMutationPreflightResult>(
      `/session/${sessionId}/workbook/kyc/restricted-mutation/preflight`,
      request,
    );
  },

  /**
   * Compile a prepared restricted mutation preflight into a stored runbook.
   * POST /api/session/:id/workbook/kyc/restricted-mutation/compile-runbook
   */
  async compileKycRestrictedMutationRunbook(
    sessionId: string,
    request: KycRestrictedMutationCompileRunbookRequest,
  ): Promise<KycRestrictedMutationCompileRunbookResult> {
    return api.post<KycRestrictedMutationCompileRunbookResult>(
      `/session/${sessionId}/workbook/kyc/restricted-mutation/compile-runbook`,
      request,
    );
  },

  /**
   * Get the session trace (append-only operation log).
   * GET /api/session/:id/trace
   */
  async getSessionTrace(sessionId: string): Promise<TraceEntry[]> {
    return api.get<TraceEntry[]>(`/session/${sessionId}/trace`);
  },

  /**
   * Replay the session trace in the given mode.
   * POST /api/session/:id/trace/replay
   */
  async replaySessionTrace(
    sessionId: string,
    mode: "strict" | "relaxed" | "dry_run",
  ): Promise<ReplayResult> {
    return api.post<ReplayResult>(`/session/${sessionId}/trace/replay`, {
      mode,
    });
  },
};

export default runbookPlanApi;
