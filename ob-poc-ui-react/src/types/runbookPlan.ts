/**
 * Runbook Plan Types
 *
 * TypeScript mirrors of Rust types from:
 * - rust/src/runbook/plan_types.rs (plan compilation + execution)
 * - rust/src/runbook/narration.rs (effect narration)
 * - rust/src/repl/session_replay.rs (trace replay)
 */

import type { SubjectKind, VerbRef, WorkspaceKind } from "../api/replV2";

// ============================================================================
// EntityBinding — literal or forward-ref entity references
// ============================================================================

/** Content-addressed plan identifier (SHA-256 hex). */
export type RunbookPlanId = string;

/** A known UUID (entity already exists). */
export interface EntityBindingLiteral {
  kind: "literal";
  id: string;
}

/** A forward reference to an output field of a prior step. */
export interface EntityBindingForwardRef {
  kind: "forward_ref";
  source_step: number;
  output_field: string;
}

/** How a plan step references an entity. */
export type EntityBinding = EntityBindingLiteral | EntityBindingForwardRef;

// ============================================================================
// BindingTable — tracks entity bindings and their resolved values
// ============================================================================

/** Tracks named entity bindings and their resolved UUIDs. */
export interface BindingTable {
  /** Named bindings (e.g. "$created_cbu_id" → EntityBinding). */
  entries: Record<string, EntityBinding>;
  /** Resolved UUIDs (populated during execution as forward refs are fulfilled). */
  resolved: Record<string, string>;
}

// ============================================================================
// Plan step types
// ============================================================================

/** Status of an individual plan step. */
export type PlanStepStatus =
  | "pending"
  | "ready"
  | "executing"
  | "succeeded"
  | "failed"
  | "skipped";

/** A single step in a multi-workspace runbook plan. */
export interface RunbookPlanStep {
  seq: number;
  workspace: WorkspaceKind;
  constellation_map: string;
  subject_kind: SubjectKind;
  subject_binding: EntityBinding;
  verb: VerbRef;
  sentence: string;
  args: Record<string, string>;
  preconditions: string[];
  expected_effect: string;
  depends_on: number[];
  status: PlanStepStatus;
}

// ============================================================================
// Plan-level types
// ============================================================================

/** Overall plan status — discriminated union on `status` field. */
export type RunbookPlanStatus =
  | { status: "compiled" }
  | { status: "awaiting_approval" }
  | { status: "approved" }
  | { status: "executing"; cursor: number }
  | { status: "completed"; completed_at: string }
  | { status: "failed"; error: string; failed_step?: number }
  | { status: "cancelled" };

/** Approval record for a runbook plan. */
export interface RunbookApproval {
  approved_by: string;
  approved_at: string;
  plan_hash: string;
}

/** Result of executing a single plan step. */
export interface StepResult {
  step_seq: number;
  verb_fqn: string;
  status: PlanStepStatus;
  output?: unknown;
  error?: string;
  executed_at: string;
}

/** A multi-workspace runbook plan. */
export interface RunbookPlan {
  id: RunbookPlanId;
  session_id: string;
  compiled_at: string;
  /** Trace entry sequence numbers that informed this plan. */
  source_research: number[];
  steps: RunbookPlanStep[];
  bindings: BindingTable;
  status: RunbookPlanStatus;
  approval?: RunbookApproval;
}

// ============================================================================
// Narration types (rust/src/runbook/narration.rs)
// ============================================================================

/** Narration outcome — discriminated union on `outcome` field. */
export type NarrationOutcome =
  | {
      outcome: "success";
      what_changed: string;
      state_now: string;
      what_next: string[];
    }
  | {
      outcome: "failed";
      error: string;
      recovery_hint?: string;
    }
  | {
      outcome: "skipped";
      reason: string;
    };

/** Narration for a single plan step. */
export interface StepNarration {
  step_index: number;
  verb_fqn: string;
  sentence: string;
  outcome: NarrationOutcome;
  workspace: WorkspaceKind;
  stale_warning?: string;
}

/** Aggregate narration for an entire plan. */
export interface PlanNarration {
  plan_id: string;
  total_steps: number;
  completed: number;
  failed: number;
  skipped: number;
  step_narrations: StepNarration[];
  aggregate_summary: string;
}

// ============================================================================
// Replay types (rust/src/repl/session_replay.rs)
// ============================================================================

/** Replay execution mode. */
export type ReplayMode = "strict" | "relaxed" | "dry_run";

/** A divergence detected during replay. */
export interface ReplayDivergence {
  sequence: number;
  expected: string;
  actual: string;
}

/** Result of a replay operation. */
export interface ReplayResult {
  mode: ReplayMode;
  entries_replayed: number;
  divergences: ReplayDivergence[];
  final_state?: unknown;
}
