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

export interface TraceEntry {
  session_id: string;
  sequence: number;
  timestamp: string;
  agent_mode: string;
  op: Record<string, unknown>;
  stack_snapshot?: unknown[];
}

// --- Replay types ---

export interface ReplayResult {
  mode: string;
  total_entries: number;
  replayed: number;
  skipped: number;
  errors: string[];
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
  async executeRunbookPlanStep(sessionId: string): Promise<Record<string, unknown>> {
    return api.post<Record<string, unknown>>(`/session/${sessionId}/runbook/execute`);
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
